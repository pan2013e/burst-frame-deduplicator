use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, anyhow};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::app_backend::{
    export_run, load_run, load_run_with_progress, prepare_preview, set_decision,
};
use crate::counterpart::{apply_counterparts, plan_counterparts, restore_counterparts};
use crate::operations::{move_rejects, restore_moved};
use crate::pipeline::run_scan_controlled;
use crate::progress::{CancellationToken, ProgressReporter, ProgressUpdate};
use crate::run_storage::{RelocationProgress, relocate_run};
use crate::types::{ScanOptions, UserDecision};

pub type BfdProgressCallback = unsafe extern "C" fn(*const c_char, *mut c_void);

static NEXT_SCAN_CONTROL_ID: AtomicU64 = AtomicU64::new(1);
static SCAN_CONTROLS: OnceLock<Mutex<HashMap<u64, CancellationToken>>> = OnceLock::new();

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
    #[serde(default = "default_generate_preview")]
    generate_if_missing: bool,
}

#[derive(Deserialize)]
struct MoveRequest {
    run_dir: PathBuf,
    destination: Option<PathBuf>,
    confirmed: bool,
}

#[derive(Deserialize)]
struct RestoreRequest {
    run_dir: PathBuf,
    asset_ids: Option<Vec<String>>,
    confirmed: bool,
}

#[derive(Deserialize)]
struct RelocateRequest {
    run_dir: PathBuf,
    destination_root: PathBuf,
}

#[derive(Deserialize)]
struct CounterpartPlanRequest {
    run_dir: PathBuf,
    card_root: PathBuf,
}

#[derive(Deserialize)]
struct CounterpartMoveRequest {
    run_dir: PathBuf,
    card_root: PathBuf,
    destination: Option<PathBuf>,
    confirmed: bool,
}

#[derive(Deserialize)]
struct CounterpartRestoreRequest {
    run_dir: PathBuf,
    card_root: PathBuf,
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
    5
}

#[unsafe(no_mangle)]
pub extern "C" fn bfd_scan_control_create() -> u64 {
    let id = NEXT_SCAN_CONTROL_ID.fetch_add(1, Ordering::Relaxed).max(1);
    scan_controls().lock().insert(id, CancellationToken::new());
    id
}

#[unsafe(no_mangle)]
pub extern "C" fn bfd_scan_control_cancel(id: u64) -> u8 {
    let control = scan_controls().lock().get(&id).cloned();
    if let Some(control) = control {
        control.cancel();
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bfd_scan_control_release(id: u64) {
    scan_controls().lock().remove(&id);
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
    scan_call(
        request_json,
        callback,
        context,
        Ok(CancellationToken::new()),
    )
}

#[unsafe(no_mangle)]
/// Runs a cancellable scan using a control created by `bfd_scan_control_create`.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string. The callback and
/// context must remain valid until this function returns. `control_id` must remain registered.
pub unsafe extern "C" fn bfd_scan_controlled(
    request_json: *const c_char,
    callback: Option<BfdProgressCallback>,
    context: *mut c_void,
    control_id: u64,
) -> *mut c_char {
    let cancellation = scan_controls()
        .lock()
        .get(&control_id)
        .cloned()
        .ok_or_else(|| anyhow!("scan cancellation control is unavailable"));
    scan_call(request_json, callback, context, cancellation)
}

fn scan_call(
    request_json: *const c_char,
    callback: Option<BfdProgressCallback>,
    context: *mut c_void,
    cancellation: anyhow::Result<CancellationToken>,
) -> *mut c_char {
    ffi_call(|| {
        let cancellation = cancellation?;
        let request: ScanRequest = parse_request(request_json)?;
        let reporter = progress_reporter(callback, context);
        let run_dir = run_scan_controlled(
            &request.root,
            request.out,
            request.options,
            reporter,
            cancellation,
        )?;
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
        load_run(request.run_dir)
    })
}

#[unsafe(no_mangle)]
/// Loads a completed run while reporting parse and preparation progress.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string. The callback and
/// context must remain valid until this function returns.
pub unsafe extern "C" fn bfd_load_run_with_progress(
    request_json: *const c_char,
    callback: Option<BfdProgressCallback>,
    context: *mut c_void,
) -> *mut c_char {
    ffi_call(|| {
        let request: RunRequest = parse_request(request_json)?;
        load_run_with_progress(request.run_dir, progress_reporter(callback, context))
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
        set_decision(&request.run_dir, request.asset_id, request.decision)
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
        prepare_preview(
            &request.run_dir,
            &request.asset_id,
            request.max_long_edge,
            request.generate_if_missing,
        )
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
        export_run(&request.run_dir)
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
        move_rejects(
            &request.run_dir,
            request.destination.as_deref(),
            request.confirmed,
        )
    })
}

#[unsafe(no_mangle)]
/// Restores previously moved files to their original paths from a UTF-8 JSON request.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_restore_rejects(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: RestoreRequest = parse_request(request_json)?;
        let selected = request
            .asset_ids
            .map(|ids| ids.into_iter().collect::<std::collections::HashSet<_>>());
        restore_moved(&request.run_dir, selected.as_ref(), request.confirmed)
    })
}

#[unsafe(no_mangle)]
/// Matches rejected decisions to opposite-format files on another card.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_plan_counterparts(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: CounterpartPlanRequest = parse_request(request_json)?;
        plan_counterparts(&request.run_dir, &request.card_root)
    })
}

#[unsafe(no_mangle)]
/// Moves matched rejected files from an opposite-format card.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_apply_counterparts(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: CounterpartMoveRequest = parse_request(request_json)?;
        apply_counterparts(
            &request.run_dir,
            &request.card_root,
            request.destination.as_deref(),
            request.confirmed,
        )
    })
}

#[unsafe(no_mangle)]
/// Restores previously moved opposite-format files to a selected card.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string.
pub unsafe extern "C" fn bfd_restore_counterparts(request_json: *const c_char) -> *mut c_char {
    ffi_call(|| {
        let request: CounterpartRestoreRequest = parse_request(request_json)?;
        restore_counterparts(&request.run_dir, &request.card_root, request.confirmed)
    })
}

#[unsafe(no_mangle)]
/// Relocates a completed run to a new parent directory.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string. The callback and
/// context must remain valid until this function returns.
pub unsafe extern "C" fn bfd_relocate_run(
    request_json: *const c_char,
    callback: Option<BfdProgressCallback>,
    context: *mut c_void,
) -> *mut c_char {
    ffi_call(|| {
        let request: RelocateRequest = parse_request(request_json)?;
        let callback_context = context as usize;
        relocate_run(
            &request.run_dir,
            &request.destination_root,
            |update: RelocationProgress| {
                emit_progress(callback, callback_context, &update);
            },
        )
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

fn scan_controls() -> &'static Mutex<HashMap<u64, CancellationToken>> {
    SCAN_CONTROLS.get_or_init(|| Mutex::new(HashMap::new()))
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
        emit_progress(Some(callback), context, &update);
    })
}

fn emit_progress<T: Serialize>(callback: Option<BfdProgressCallback>, context: usize, update: &T) {
    let Some(callback) = callback else {
        return;
    };
    let Ok(json) = serde_json::to_string(update) else {
        return;
    };
    let Ok(json) = CString::new(json) else {
        return;
    };
    unsafe { callback(json.as_ptr(), context as *mut c_void) };
}

fn default_preview_size() -> u32 {
    4096
}

fn default_generate_preview() -> bool {
    true
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

    use super::{
        bfd_default_options, bfd_free_string, bfd_scan_control_cancel, bfd_scan_control_create,
        bfd_scan_control_release, scan_controls,
    };

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

    #[test]
    fn scan_controls_cancel_and_release_shared_tokens() {
        let id = bfd_scan_control_create();
        let token = scan_controls().lock().get(&id).cloned().unwrap();
        assert_eq!(bfd_scan_control_cancel(id), 1);
        assert!(token.is_cancelled());
        bfd_scan_control_release(id);
        assert_eq!(bfd_scan_control_cancel(id), 0);
    }
}
