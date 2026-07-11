use std::fs;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use image::{DynamicImage, ImageFormat};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::artifacts::{
    MoveScripts, ensure_review_state, export_reviewed_artifacts, move_scripts_for_run,
    read_manifest, read_review_state, upsert_decision,
};
use crate::assets::is_raw_extension;
use crate::decode::load_preview;
use crate::locales::read_locale;
use crate::operations::{
    MoveRejectsResponse, MoveStatus, RestoreResponse, move_rejects, read_move_status,
    resolve_available_source, restore_moved,
};
use crate::types::{ReviewState, RunManifest, UserDecision};

const RAW_BROWSER_PREVIEW_SIZE: u32 = 2400;

#[derive(Clone)]
struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    run_dir: PathBuf,
    manifest: Mutex<RunManifest>,
    review: Mutex<ReviewState>,
}

#[derive(Serialize)]
struct ManifestResponse {
    manifest: RunManifest,
    review: ReviewState,
    move_status: MoveStatus,
}

#[derive(Deserialize)]
struct DecisionRequest {
    asset_id: String,
    decision: Option<UserDecision>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct MoveRejectsRequest {
    confirm: bool,
    destination: Option<PathBuf>,
}

#[derive(Deserialize)]
struct RestoreRejectsRequest {
    confirm: bool,
    asset_ids: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct MoveScriptsRequest {
    destination: Option<PathBuf>,
}

pub async fn serve(run_dir: PathBuf, addr: SocketAddr) -> anyhow::Result<()> {
    serve_with_shutdown(run_dir, addr, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
}

pub async fn serve_with_shutdown<F>(
    run_dir: PathBuf,
    addr: SocketAddr,
    shutdown: F,
) -> anyhow::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let manifest = read_manifest(&run_dir)?;
    let review = ensure_review_state(&run_dir, &manifest)?;
    let state = AppState {
        inner: Arc::new(AppStateInner {
            run_dir,
            manifest: Mutex::new(manifest),
            review: Mutex::new(review),
        }),
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/review.css", get(review_css))
        .route("/review.js", get(review_js))
        .route("/api/manifest", get(api_manifest))
        .route("/api/decision", post(api_decision))
        .route("/api/export", post(api_export))
        .route("/api/image/{id}", get(api_image))
        .route("/api/source/{id}", get(api_source))
        .route("/api/preview/{id}", get(api_preview))
        .route("/api/move-rejects", post(api_move_rejects))
        .route("/api/restore-rejects", post(api_restore_rejects))
        .route("/api/move-scripts", post(api_move_scripts))
        .route("/locales/{file}", get(locale_file))
        .route("/thumbs/{file}", get(thumb))
        .route("/vendor/libraw-wasm/{file}", get(vendor_libraw_wasm))
        .layer(middleware::from_fn(cross_origin_isolation_headers))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../web/review/index.html"))
}

async fn review_css() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../web/review/styles.css"),
    )
        .into_response()
}

async fn review_js() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/review/app.js"),
    )
        .into_response()
}

async fn cross_origin_isolation_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "Cross-Origin-Opener-Policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "Cross-Origin-Embedder-Policy",
        HeaderValue::from_static("require-corp"),
    );
    headers.insert(
        "Cross-Origin-Resource-Policy",
        HeaderValue::from_static("same-origin"),
    );
    response
}

async fn api_manifest(State(state): State<AppState>) -> Result<Json<ManifestResponse>, Response> {
    let move_status = read_move_status(&state.inner.run_dir).map_err(internal_error)?;
    Ok(Json(ManifestResponse {
        manifest: state.inner.manifest.lock().clone(),
        review: state.inner.review.lock().clone(),
        move_status,
    }))
}

async fn api_decision(
    State(state): State<AppState>,
    Json(request): Json<DecisionRequest>,
) -> Result<Json<ReviewState>, Response> {
    let review = upsert_decision(
        &state.inner.run_dir,
        request.asset_id,
        request.decision,
        request.note,
    )
    .map_err(internal_error)?;
    *state.inner.review.lock() = review.clone();
    Ok(Json(review))
}

async fn api_export(State(state): State<AppState>) -> Result<StatusCode, Response> {
    export_reviewed_artifacts(&state.inner.run_dir).map_err(internal_error)?;
    let review = read_review_state(&state.inner.run_dir).map_err(internal_error)?;
    *state.inner.review.lock() = review;
    Ok(StatusCode::NO_CONTENT)
}

async fn api_image(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let manifest = state.inner.manifest.lock().clone();
    let Some(asset) = manifest.assets.iter().find(|asset| asset.id == id) else {
        return (StatusCode::NOT_FOUND, "Image is not in this run.").into_response();
    };
    let Some(file) = asset
        .files
        .iter()
        .find(|file| is_browser_image_ext(&file.extension))
    else {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "This asset has no browser-viewable full-resolution file. RAW-only assets need a paired JPEG/PNG/WebP for full-resolution browser preview.",
        )
            .into_response();
    };
    let path = match resolve_available_source(&state.inner.run_dir, &file.path) {
        Ok(path) => path,
        Err(error) => return internal_error(error),
    };
    if !path.is_file() {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "The original file is not accessible: {}. The card or source folder may be ejected.",
                file.path.display()
            ),
        )
            .into_response();
    }
    match fs::read(&path) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            ([(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not read {}: {error}", path.display()),
        )
            .into_response(),
    }
}

async fn api_source(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let manifest = state.inner.manifest.lock().clone();
    let Some(asset) = manifest.assets.iter().find(|asset| asset.id == id) else {
        return (StatusCode::NOT_FOUND, "Image is not in this run.").into_response();
    };
    let Some(file) = asset
        .files
        .iter()
        .find(|file| is_raw_extension(&file.extension))
    else {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "This asset has no RAW source file for browser-side RAW decoding.",
        )
            .into_response();
    };
    let path = match resolve_available_source(&state.inner.run_dir, &file.path) {
        Ok(path) => path,
        Err(error) => return internal_error(error),
    };
    if !path.is_file() {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "The original RAW file is not accessible: {}. The card or source folder may be ejected.",
                file.path.display()
            ),
        )
            .into_response();
    }
    match fs::read(&path) {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (header::CACHE_CONTROL, "no-store, max-age=0".to_string()),
            ],
            bytes,
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not read {}: {error}", path.display()),
        )
            .into_response(),
    }
}

async fn api_preview(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let manifest = state.inner.manifest.lock().clone();
    let Some(asset) = manifest.assets.iter().find(|asset| asset.id == id) else {
        return (StatusCode::NOT_FOUND, "Image is not in this run.").into_response();
    };
    let file = &asset.representative;
    let path = match resolve_available_source(&state.inner.run_dir, &file.path) {
        Ok(path) => path,
        Err(error) => return internal_error(error),
    };
    if !path.is_file() {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "The original file is not accessible: {}. The card or source folder may be ejected.",
                file.path.display()
            ),
        )
            .into_response();
    }
    match load_preview(&path, &file.extension, RAW_BROWSER_PREVIEW_SIZE) {
        Ok(decoded) => {
            let mut output = std::io::Cursor::new(Vec::new());
            match DynamicImage::ImageRgb8(decoded.image).write_to(&mut output, ImageFormat::Jpeg) {
                Ok(()) => (
                    [
                        (header::CONTENT_TYPE, "image/jpeg".to_string()),
                        (header::CACHE_CONTROL, "no-store, max-age=0".to_string()),
                    ],
                    output.into_inner(),
                )
                    .into_response(),
                Err(error) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "Could not encode preview for {}: {error}",
                        file.path.display()
                    ),
                )
                    .into_response(),
            }
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Could not decode preview for {}: {error}",
                file.path.display()
            ),
        )
            .into_response(),
    }
}

async fn api_move_rejects(
    State(state): State<AppState>,
    Json(request): Json<MoveRejectsRequest>,
) -> Result<Json<MoveRejectsResponse>, Response> {
    move_rejects(
        &state.inner.run_dir,
        request.destination.as_deref(),
        request.confirm,
    )
    .map(Json)
    .map_err(internal_error)
}

async fn api_restore_rejects(
    State(state): State<AppState>,
    Json(request): Json<RestoreRejectsRequest>,
) -> Result<Json<RestoreResponse>, Response> {
    let selected = request
        .asset_ids
        .map(|ids| ids.into_iter().collect::<std::collections::HashSet<_>>());
    restore_moved(&state.inner.run_dir, selected.as_ref(), request.confirm)
        .map(Json)
        .map_err(internal_error)
}

async fn api_move_scripts(
    State(state): State<AppState>,
    Json(request): Json<MoveScriptsRequest>,
) -> Result<Json<MoveScripts>, Response> {
    move_scripts_for_run(&state.inner.run_dir, request.destination.as_deref())
        .map(Json)
        .map_err(internal_error)
}

async fn thumb(State(state): State<AppState>, Path(file): Path<String>) -> Response {
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = state.inner.run_dir.join("thumbs").join(file);
    match fs::read(&path) {
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn locale_file(Path(file): Path<String>) -> Response {
    let Some(code) = file.strip_suffix(".json") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match read_locale(code) {
        Ok(bytes) => (
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn vendor_libraw_wasm(Path(file): Path<String>) -> Response {
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("web")
        .join("vendor")
        .join("libraw-wasm")
        .join(&file);
    let Ok(bytes) = fs::read(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match path.extension().and_then(|extension| extension.to_str()) {
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    };
    ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
}

fn internal_error(error: anyhow::Error) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
}

fn is_browser_image_ext(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp"
    )
}
