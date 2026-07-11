use std::ffi::{CStr, CString, c_char, c_void};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use anyhow::{Context, anyhow};
use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use serde::{Deserialize, Serialize};

use crate::artifacts::{
    ensure_review_state, export_reviewed_artifacts, read_manifest, upsert_decision,
};
use crate::decode::load_preview;
use crate::operations::move_rejects;
use crate::pipeline::run_scan;
use crate::progress::{ProgressReporter, ProgressUpdate};
use crate::types::{FileKind, ReviewState, RunManifest, ScanOptions, UserDecision};

pub type BfdProgressCallback = unsafe extern "C" fn(*const c_char, *mut c_void);

#[derive(Deserialize)]
struct ScanRequest {
    root: PathBuf,
    out: Option<PathBuf>,
    #[serde(default)]
    options: ScanOptions,
}

#[derive(Serialize)]
struct ScanResponse {
    run_dir: PathBuf,
}

#[derive(Deserialize)]
struct RunRequest {
    run_dir: PathBuf,
}

#[derive(Serialize)]
struct ReviewPayload {
    run_dir: PathBuf,
    manifest: RunManifest,
    review: ReviewState,
}

#[derive(Deserialize)]
struct DecisionRequest {
    run_dir: PathBuf,
    asset_id: String,
    decision: Option<UserDecision>,
}

#[derive(Deserialize)]
struct PreviewRequest {
    run_dir: PathBuf,
    asset_id: String,
    #[serde(default = "default_preview_size")]
    max_long_edge: u32,
}

#[derive(Serialize)]
struct PreviewResponse {
    path: PathBuf,
    generated: bool,
}

#[derive(Deserialize)]
struct MoveRequest {
    run_dir: PathBuf,
    confirmed: bool,
}

#[derive(Serialize)]
struct Envelope<T: Serialize> {
    ok: bool,
    value: Option<T>,
    error: Option<String>,
}

#[unsafe(no_mangle)]
pub extern "C" fn bfd_api_version() -> u32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn bfd_default_options() -> *mut c_char {
    ffi_call(|| Ok(ScanOptions::default()))
}

#[unsafe(no_mangle)]
/// Runs a scan from a UTF-8 JSON request.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string. The callback and
/// context must remain valid until this function returns.
pub unsafe extern "C" fn bfd_scan(
    request_json: *const c_char,
    callback: Option<BfdProgressCallback>,
    context: *mut c_void,
) -> *mut c_char {
    ffi_call(|| {
        let request: ScanRequest = parse_request(request_json)?;
        let reporter = progress_reporter(callback, context);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("creating scan runtime")?;
        let run_dir = runtime.block_on(run_scan(
            &request.root,
            request.out,
            request.options,
            reporter,
        ))?;
        Ok(ScanResponse { run_dir })
    })
}

#[unsafe(no_mangle)]
/// Loads a completed run from a UTF-8 JSON request.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_load_run(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: RunRequest = parse_request(request_json)?;
        load_review_payload(request.run_dir)
    })
}

#[unsafe(no_mangle)]
/// Persists a review decision from a UTF-8 JSON request.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_set_decision(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: DecisionRequest = parse_request(request_json)?;
        upsert_decision(&request.run_dir, request.asset_id, request.decision, None)?;
        load_review_payload(request.run_dir)
    })
}

#[unsafe(no_mangle)]
/// Prepares a browser-compatible preview from a UTF-8 JSON request.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_prepare_preview(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: PreviewRequest = parse_request(request_json)?;
        prepare_preview(&request)
    })
}

#[unsafe(no_mangle)]
/// Re-exports review artifacts from a UTF-8 JSON request.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_export_run(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: RunRequest = parse_request(request_json)?;
        export_reviewed_artifacts(&request.run_dir)?;
        load_review_payload(request.run_dir)
    })
}

#[unsafe(no_mangle)]
/// Moves confirmed rejects from a UTF-8 JSON request.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_move_rejects(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: MoveRequest = parse_request(request_json)?;
        move_rejects(&request.run_dir, request.confirmed)
    })
}

#[unsafe(no_mangle)]
/// Releases a response returned by this library.
///
/// # Safety
/// `value` must be null or a pointer returned by this library that has not already been freed.
pub unsafe extern "C" fn bfd_free_string(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

fn load_review_payload(run_dir: PathBuf) -> anyhow::Result<ReviewPayload> {
    let manifest = read_manifest(&run_dir)?;
    let review = ensure_review_state(&run_dir, &manifest)?;
    Ok(ReviewPayload {
        run_dir,
        manifest,
        review,
    })
}

fn prepare_preview(request: &PreviewRequest) -> anyhow::Result<PreviewResponse> {
    let manifest = read_manifest(&request.run_dir)?;
    let asset = manifest
        .assets
        .iter()
        .find(|asset| asset.id == request.asset_id)
        .ok_or_else(|| anyhow!("asset not found: {}", request.asset_id))?;
    if asset.representative.kind != FileKind::Raw {
        return Ok(PreviewResponse {
            path: asset.representative.path.clone(),
            generated: false,
        });
    }

    let max_long_edge = request.max_long_edge.clamp(1024, 8192);
    let preview_dir = request.run_dir.join("native_previews");
    fs::create_dir_all(&preview_dir)?;
    let output = preview_dir.join(format!("{}_{}.jpg", asset.id, max_long_edge));
    if output.is_file() {
        return Ok(PreviewResponse {
            path: output,
            generated: true,
        });
    }

    let decoded = load_preview(
        &asset.representative.path,
        &asset.representative.extension,
        max_long_edge,
    )?;
    let file =
        fs::File::create(&output).with_context(|| format!("creating {}", output.display()))?;
    let mut encoder = JpegEncoder::new_with_quality(file, 92);
    encoder.encode_image(&DynamicImage::ImageRgb8(decoded.image))?;
    Ok(PreviewResponse {
        path: output,
        generated: true,
    })
}

fn progress_reporter(
    callback: Option<BfdProgressCallback>,
    context: *mut c_void,
) -> ProgressReporter {
    let Some(callback) = callback else {
        return ProgressReporter::default();
    };
    let context = context as usize;
    ProgressReporter::new(move |update: ProgressUpdate| {
        let Ok(json) = serde_json::to_string(&update) else {
            return;
        };
        let Ok(json) = CString::new(json) else {
            return;
        };
        unsafe { callback(json.as_ptr(), context as *mut c_void) };
    })
}

fn default_preview_size() -> u32 {
    4096
}

fn ffi_call<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> *mut c_char
where
    T: Serialize,
{
    let envelope = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => Envelope {
            ok: true,
            value: Some(value),
            error: None,
        },
        Ok(Err(error)) => Envelope::<T> {
            ok: false,
            value: None,
            error: Some(format!("{error:#}")),
        },
        Err(_) => Envelope::<T> {
            ok: false,
            value: None,
            error: Some("Rust backend panicked".to_string()),
        },
    };
    let json = serde_json::to_string(&envelope).unwrap_or_else(|_| {
        r#"{"ok":false,"value":null,"error":"response serialization failed"}"#.to_string()
    });
    CString::new(json).map_or(std::ptr::null_mut(), CString::into_raw)
}

fn parse_request<T>(request_json: *const c_char) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    if request_json.is_null() {
        return Err(anyhow!("request JSON pointer is null"));
    }
    let bytes = unsafe { CStr::from_ptr(request_json) }.to_bytes();
    serde_json::from_slice(bytes).context("parsing request JSON")
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::{bfd_default_options, bfd_free_string};

    #[test]
    fn default_options_round_trip_through_the_c_boundary() {
        let value = bfd_default_options();
        assert!(!value.is_null());
        let json = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        unsafe { bfd_free_string(value) };
        let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded["ok"], true);
        assert_eq!(decoded["value"]["preview_size"], 1280);
    }
}
