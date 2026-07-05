use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::{fs, io};

use anyhow::Context;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Local;
use image::{DynamicImage, ImageFormat};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::artifacts::{
    ensure_review_state, export_reviewed_artifacts, read_manifest, read_review_state,
    upsert_decision,
};
use crate::assets::is_raw_extension;
use crate::decode::load_preview;
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
}

#[derive(Serialize)]
struct MoveRejectsResponse {
    destination: PathBuf,
    moved_files: usize,
    moved_assets: usize,
    missing_files: Vec<String>,
    failed_files: Vec<MoveFailure>,
}

#[derive(Serialize)]
struct MoveFailure {
    source: String,
    error: String,
}

pub async fn serve(run_dir: PathBuf, addr: SocketAddr) -> anyhow::Result<()> {
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
        .route("/api/manifest", get(api_manifest))
        .route("/api/decision", post(api_decision))
        .route("/api/export", post(api_export))
        .route("/api/image/{id}", get(api_image))
        .route("/api/source/{id}", get(api_source))
        .route("/api/preview/{id}", get(api_preview))
        .route("/api/move-rejects", post(api_move_rejects))
        .route("/thumbs/{file}", get(thumb))
        .route("/vendor/libraw-wasm/{file}", get(vendor_libraw_wasm))
        .layer(middleware::from_fn(cross_origin_isolation_headers))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
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

async fn api_manifest(State(state): State<AppState>) -> Json<ManifestResponse> {
    Json(ManifestResponse {
        manifest: state.inner.manifest.lock().clone(),
        review: state.inner.review.lock().clone(),
    })
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
    if !file.path.exists() {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "The original file is not accessible: {}. The card or source folder may be ejected.",
                file.path.display()
            ),
        )
            .into_response();
    }
    match fs::read(&file.path) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&file.path)
                .first_or_octet_stream()
                .to_string();
            ([(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not read {}: {err}", file.path.display()),
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
    if !file.path.exists() {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "The original RAW file is not accessible: {}. The card or source folder may be ejected.",
                file.path.display()
            ),
        )
            .into_response();
    }
    match fs::read(&file.path) {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (header::CACHE_CONTROL, "no-store, max-age=0".to_string()),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not read {}: {err}", file.path.display()),
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
    if !file.path.exists() {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "The original file is not accessible: {}. The card or source folder may be ejected.",
                file.path.display()
            ),
        )
            .into_response();
    }
    match load_preview(&file.path, &file.extension, RAW_BROWSER_PREVIEW_SIZE) {
        Ok(decoded) => {
            let mut out = std::io::Cursor::new(Vec::new());
            match DynamicImage::ImageRgb8(decoded.image).write_to(&mut out, ImageFormat::Jpeg) {
                Ok(()) => (
                    [
                        (header::CONTENT_TYPE, "image/jpeg".to_string()),
                        (header::CACHE_CONTROL, "no-store, max-age=0".to_string()),
                    ],
                    out.into_inner(),
                )
                    .into_response(),
                Err(err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "Could not encode preview for {}: {err}",
                        file.path.display()
                    ),
                )
                    .into_response(),
            }
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Could not decode preview for {}: {err}",
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
    if !request.confirm {
        return Err((
            StatusCode::BAD_REQUEST,
            "Move requires explicit confirmation.",
        )
            .into_response());
    }

    let manifest = state.inner.manifest.lock().clone();
    let review = state.inner.review.lock().clone();
    let destination = state
        .inner
        .run_dir
        .join("moved_rejects")
        .join(Local::now().format("%Y%m%d_%H%M%S").to_string());
    fs::create_dir_all(&destination).map_err(|err| {
        internal_error(anyhow::anyhow!(
            "creating move destination {}: {err}",
            destination.display()
        ))
    })?;

    let mut moved_files = 0usize;
    let mut moved_assets = 0usize;
    let mut missing_files = Vec::new();
    let mut failed_files = Vec::new();

    for asset in &manifest.assets {
        if final_action_for_asset(asset, &review) != UserDecision::Reject {
            continue;
        }
        let mut asset_moved = false;
        for file in asset.files.iter().chain(asset.sidecars.iter()) {
            if !file.path.exists() {
                missing_files.push(file.path.display().to_string());
                continue;
            }
            let target = unique_target(&destination.join(&file.rel_path));
            match move_file_verified(&file.path, &target) {
                Ok(()) => {
                    moved_files += 1;
                    asset_moved = true;
                }
                Err(err) => failed_files.push(MoveFailure {
                    source: file.path.display().to_string(),
                    error: err.to_string(),
                }),
            }
        }
        if asset_moved {
            moved_assets += 1;
        }
    }

    let response = MoveRejectsResponse {
        destination: destination.clone(),
        moved_files,
        moved_assets,
        missing_files,
        failed_files,
    };
    let report_path = state
        .inner
        .run_dir
        .join("move_reports")
        .join(format!("{}.json", Local::now().format("%Y%m%d_%H%M%S")));
    if let Some(parent) = report_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(file) = fs::File::create(report_path) {
        let _ = serde_json::to_writer_pretty(file, &response);
    }
    Ok(Json(response))
}

async fn thumb(State(state): State<AppState>, Path(file): Path<String>) -> Response {
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = state.inner.run_dir.join("thumbs").join(file);
    match std::fs::read(&path) {
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response(),
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
    let content_type = match path.extension().and_then(|ext| ext.to_str()) {
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

fn final_action_for_asset(asset: &crate::types::AssetRecord, review: &ReviewState) -> UserDecision {
    if let Some(decision) = review
        .decisions
        .iter()
        .find(|decision| decision.asset_id == asset.id)
        .and_then(|decision| decision.decision)
    {
        return decision;
    }
    match asset.suggestion.action {
        crate::types::SuggestedAction::Keep => UserDecision::Keep,
        crate::types::SuggestedAction::Reject => UserDecision::Reject,
        crate::types::SuggestedAction::Review | crate::types::SuggestedAction::Error => {
            UserDecision::Review
        }
    }
}

fn unique_target(path: &FsPath) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| FsPath::new(""));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("file");
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    for index in 1.. {
        let file_name = if extension.is_empty() {
            format!("{stem}_{index}")
        } else {
            format!("{stem}_{index}.{extension}")
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn move_file_verified(source: &FsPath, target: &FsPath) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let source_len = fs::metadata(source)?.len();
    fs::copy(source, target)?;
    let target_len = fs::metadata(target)?.len();
    if source_len != target_len {
        return Err(io::Error::other(format!(
            "copied size mismatch: source {source_len} bytes, target {target_len} bytes"
        )));
    }
    fs::remove_file(source)?;
    Ok(())
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Burst Review</title>
  <style>
    :root {
      color-scheme: light dark;
      --bg: #f5f5f2;
      --panel: #fff;
      --ink: #171717;
      --muted: #5c615f;
      --line: #d7d7d0;
      --keep: #1b7f45;
      --reject: #8b6f00;
      --review: #7a4f00;
      --error: #b42318;
      --danger: #b42318;
      --focus: #2364aa;
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --bg: #111412;
        --panel: #191d1a;
        --ink: #f0f2ee;
        --muted: #a0a69f;
        --line: #30362f;
        --keep: #55b982;
        --reject: #d2aa35;
        --review: #dda047;
        --error: #f97066;
        --danger: #d92d20;
        --focus: #7db7ef;
      }
    }
    * { box-sizing: border-box; }
    [hidden] { display: none !important; }
    body {
      margin: 0;
      background: var(--bg);
      color: var(--ink);
      font: 14px/1.4 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    header {
      position: sticky;
      top: 0;
      z-index: 10;
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 14px;
      align-items: center;
      padding: 14px 22px;
      background: color-mix(in srgb, var(--bg) 94%, transparent);
      border-bottom: 1px solid var(--line);
      backdrop-filter: blur(10px);
    }
    h1 { margin: 0; font-size: 20px; letter-spacing: 0; }
    .summary { color: var(--muted); overflow-wrap: anywhere; }
    .toolbar { display: flex; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
    button, input, select, textarea {
      font: inherit;
      color: var(--ink);
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 6px;
    }
    button { min-height: 32px; padding: 0 10px; cursor: pointer; }
    button.danger {
      background: var(--danger);
      border-color: color-mix(in srgb, var(--danger) 80%, black);
      color: white;
      font-weight: 650;
    }
    button.danger:disabled {
      opacity: .55;
      cursor: default;
    }
    input, select { min-height: 32px; padding: 0 9px; }
    main { padding: 18px 22px 28px; }
    .statusbar {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
      gap: 8px;
      margin-bottom: 16px;
    }
    .stat {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 9px 11px;
    }
    .stat span { display: block; color: var(--muted); font-size: 12px; }
    .stat b { font-size: 18px; }
    .clusters { display: grid; gap: 18px; }
    .cluster {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
      overflow: hidden;
    }
    .cluster-head {
      display: grid;
      grid-template-columns: 1fr auto auto;
      gap: 12px;
      align-items: center;
      padding: 11px 13px;
      border-bottom: 1px solid var(--line);
    }
    .cluster h2 { margin: 0; font-size: 15px; letter-spacing: 0; }
    .cluster-meta { color: var(--muted); font-size: 12px; }
    .cluster.collapsed .grid { display: none; }
    .toggle {
      min-width: 34px;
      padding: 0;
      font-weight: 700;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
      gap: 10px;
      padding: 12px;
    }
    .item {
      border: 1px solid var(--line);
      background: color-mix(in srgb, var(--panel) 88%, var(--bg));
      border-radius: 7px;
      overflow: hidden;
      min-width: 0;
    }
    .item.keep { border-color: color-mix(in srgb, var(--keep) 70%, var(--line)); }
    .item.reject { opacity: .70; }
    .item.review { border-color: color-mix(in srgb, var(--review) 70%, var(--line)); }
    .item.error { border-color: color-mix(in srgb, var(--error) 70%, var(--line)); }
    .thumb {
      position: relative;
      aspect-ratio: 4 / 3;
      background: #242825;
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: zoom-in;
    }
    .thumb img { width: 100%; height: 100%; object-fit: contain; display: block; }
    .badge {
      position: absolute;
      top: 7px;
      left: 7px;
      color: white;
      border-radius: 4px;
      padding: 2px 6px;
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
      background: rgba(0,0,0,.72);
    }
    .meta { display: grid; gap: 7px; padding: 8px; font-size: 12px; }
    .name { font-weight: 620; overflow-wrap: anywhere; color: var(--ink); }
    .reason { color: var(--muted); }
    .exif {
      display: flex;
      gap: 5px;
      flex-wrap: wrap;
      min-height: 20px;
    }
    .exif span {
      border: 1px solid var(--line);
      border-radius: 4px;
      padding: 1px 5px;
      color: var(--muted);
      background: color-mix(in srgb, var(--panel) 78%, var(--bg));
    }
    .exif span.diff {
      color: var(--ink);
      border-color: color-mix(in srgb, var(--focus) 65%, var(--line));
      background: color-mix(in srgb, var(--focus) 14%, var(--panel));
      font-weight: 650;
    }
    .keepbox {
      display: flex;
      align-items: center;
      gap: 7px;
      font-size: 13px;
      font-weight: 620;
    }
    .keepbox input { width: 18px; height: 18px; accent-color: var(--keep); }
    .reset {
      border: 0;
      background: transparent;
      color: var(--focus);
      min-height: 0;
      padding: 0;
      justify-self: start;
    }
    details { color: var(--muted); }
    details ul { margin: 7px 0 0 18px; padding: 0; }
    .empty {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 28px;
      text-align: center;
      color: var(--muted);
    }
    .show-more {
      justify-self: center;
      min-width: 180px;
    }
    .viewer {
      position: fixed;
      inset: 0;
      z-index: 100;
      display: none;
      grid-template-rows: auto 1fr;
      background: rgba(0,0,0,.86);
      color: white;
    }
    .viewer.open { display: grid; }
    .viewerbar {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 10px;
      background: rgba(0,0,0,.72);
    }
    .viewerbar .title {
      flex: 1;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .viewer-keep {
      display: flex;
      align-items: center;
      gap: 6px;
      min-height: 32px;
      padding: 0 8px;
      border: 1px solid #3a3a3a;
      border-radius: 6px;
      background: #181818;
      color: white;
      font-weight: 650;
    }
    .viewer-keep input { width: 18px; height: 18px; accent-color: var(--keep); }
    .viewerbar button {
      background: #181818;
      color: white;
      border-color: #3a3a3a;
    }
    .viewport {
      position: relative;
      overflow: hidden;
      cursor: grab;
      touch-action: none;
    }
    .viewport.dragging { cursor: grabbing; }
    .viewport img {
      position: absolute;
      top: 50%;
      left: 50%;
      max-width: none;
      transform-origin: center center;
      user-select: none;
      -webkit-user-drag: none;
    }
    .viewer-error {
      position: absolute;
      inset: 0;
      display: grid;
      place-items: center;
      padding: 24px;
      text-align: center;
      color: #fff;
    }
    .viewer-loading {
      position: absolute;
      inset: 0;
      display: grid;
      place-items: center;
      gap: 12px;
      align-content: center;
      color: #fff;
      background: rgba(0,0,0,.24);
      pointer-events: none;
    }
    .spinner {
      width: 34px;
      height: 34px;
      border: 3px solid rgba(255,255,255,.28);
      border-top-color: #fff;
      border-radius: 50%;
      animation: spin .8s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
    @media (max-width: 820px) {
      header { grid-template-columns: 1fr; }
      .toolbar { justify-content: flex-start; }
      main { padding: 12px; }
    }
  </style>
</head>
<body>
  <header>
    <div>
      <h1>Burst Review</h1>
      <div class="summary" id="root"></div>
    </div>
    <div class="toolbar">
      <input id="search" type="search" placeholder="Find filename">
      <select id="filter">
        <option value="all">All frames</option>
        <option value="review">Needs review</option>
        <option value="keep">Kept</option>
        <option value="reject">Rejected</option>
        <option value="burst">Burst clusters</option>
      </select>
      <button id="exportBtn" title="Write updated keep/reject review files">Save Review</button>
      <button id="moveBtn" class="danger">Move rejects</button>
    </div>
  </header>
  <main>
    <section class="statusbar" id="stats"></section>
    <section class="clusters" id="clusters"></section>
  </main>
  <div class="viewer" id="viewer" aria-hidden="true">
    <div class="viewerbar">
      <div class="title" id="viewerTitle">Full resolution</div>
      <label class="viewer-keep"><input id="viewerKeepBox" type="checkbox"> Keep</label>
      <button id="zoomOutBtn">-</button>
      <button id="zoomInBtn">+</button>
      <button id="zoomResetBtn">Fit</button>
      <button id="viewerCloseBtn">Close</button>
    </div>
    <div class="viewport" id="viewport">
      <img id="viewerImg" alt="">
      <div class="viewer-loading" id="viewerLoading" hidden><div class="spinner"></div><span>Loading preview</span></div>
      <div class="viewer-error" id="viewerError" hidden></div>
    </div>
  </div>
  <script>
    let manifest = null;
    let review = null;
    const decisions = new Map();
    const assetById = new Map();
    const clusterByAsset = new Map();
    const clusterAssets = new Map();
    const manuallyOpenedClusters = new Set();
    const manuallyClosedClusters = new Set();
    const rawPreviewCache = new Map();
    let visibleClusterLimit = 80;
    let clusterRenderTimer = null;
    let libRawCtorPromise = null;
    let rawPreviewCacheBytes = 0;
    let currentViewerAssetId = null;
    let currentViewerClusterIds = [];
    let viewerUrl = null;
    let viewerLoadToken = 0;
    let viewerScale = 1;
    let viewerX = 0;
    let viewerY = 0;
    let dragging = false;
    let dragStart = null;
    const $ = (sel, root = document) => root.querySelector(sel);
    const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));
    const browserImageExts = new Set(['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp']);
    const RAW_PREVIEW_CACHE_MAX_BYTES = 128 * 1024 * 1024;
    const RAW_PREVIEW_CACHE_MAX_ITEMS = 24;

    async function load() {
      const res = await fetch('/api/manifest');
      const data = await res.json();
      manifest = data.manifest;
      review = data.review;
      decisions.clear();
      for (const item of review.decisions) decisions.set(item.asset_id, item);
      assetById.clear();
      clusterByAsset.clear();
      clusterAssets.clear();
      for (const asset of manifest.assets) assetById.set(asset.id, asset);
      for (const cluster of manifest.clusters) {
        const assets = cluster.asset_ids.map(id => assetById.get(id)).filter(Boolean);
        clusterAssets.set(cluster.id, assets);
        for (const asset of assets) clusterByAsset.set(asset.id, cluster.id);
      }
      render();
    }

    function finalAction(asset) {
      const user = decisions.get(asset.id);
      if (user && user.decision) return user.decision;
      return asset.suggestion.action;
    }

    function suggestedKeep(asset) {
      return asset.suggestion.action === 'keep';
    }

    function render() {
      $('#root').textContent = manifest.root;
      renderStats();
      renderClusters();
    }

    function renderStats() {
      const counts = { keep: 0, reject: 0, review: 0, error: 0 };
      for (const asset of manifest.assets) counts[finalAction(asset)]++;
      const manual = review.decisions.length;
      $('#stats').innerHTML = [
        ['Images', manifest.summary.discovered_assets],
        ['Clusters', manifest.summary.clusters],
        ['Keep', counts.keep],
        ['Reject', counts.reject],
        ['Review', counts.review],
        ['Manual edits', manual],
      ].map(([label, value]) => `<div class="stat"><span>${label}</span><b>${value}</b></div>`).join('');
      $('#moveBtn').disabled = counts.reject === 0;
      $('#moveBtn').textContent = counts.reject ? `Move ${counts.reject} rejects` : 'No rejects';
    }

    function renderClusters() {
      const query = $('#search').value.trim().toLowerCase();
      const filter = $('#filter').value;
      const rows = [];
      for (const cluster of manifest.clusters) {
        if (filter === 'burst' && cluster.asset_ids.length <= 1) continue;
        const allAssets = clusterAssets.get(cluster.id) || [];
        let assets = allAssets.slice();
        assets = assets.filter(asset => {
          const action = finalAction(asset);
          const text = `${asset.representative.rel_path} ${asset.stem}`.toLowerCase();
          if (query && !text.includes(query)) return false;
          if (filter === 'all' || filter === 'burst') return true;
          return action === filter;
        });
        if (!assets.length) continue;
        const collapsed = clusterCollapsed(cluster, allAssets);
        rows.push({ cluster, assets, allAssets, collapsed });
      }
      rows.sort((a, b) => Number(a.collapsed) - Number(b.collapsed) || a.cluster.id - b.cluster.id);
      const shownRows = rows.slice(0, visibleClusterLimit);
      let html = shownRows.map(row => clusterHtml(row.cluster, row.assets, row.allAssets, row.collapsed)).join('');
      if (rows.length > shownRows.length) {
        html += `<button class="show-more" data-show-more="1">Show ${Math.min(visibleClusterLimit, rows.length - shownRows.length)} more clusters</button>`;
      }
      $('#clusters').innerHTML = html || '<div class="empty">No frames match the current filter.</div>';
      $$('.item input[type="checkbox"][data-indeterminate="1"]').forEach(input => {
        input.indeterminate = true;
      });
    }

    function scheduleClusterRender(resetLimit = false) {
      if (resetLimit) visibleClusterLimit = 80;
      window.clearTimeout(clusterRenderTimer);
      clusterRenderTimer = window.setTimeout(renderClusters, 80);
    }

    function clusterCollapsed(cluster, allAssets) {
      const allKept = allAssets.length > 0 && allAssets.every(asset => finalAction(asset) === 'keep');
      const forcedClosed = manuallyClosedClusters.has(cluster.id);
      const forcedOpen = manuallyOpenedClusters.has(cluster.id);
      return forcedClosed || (allKept && !forcedOpen);
    }

    function clusterHtml(cluster, assets, allAssets, collapsed) {
      const shown = assets.length === cluster.asset_ids.length ? `${assets.length} frames` : `${assets.length} shown of ${cluster.asset_ids.length}`;
      const clusterStatus = collapsed ? 'collapsed' : 'expanded';
      const diffKeys = exifDiffKeys(allAssets);
      return `<section class="cluster ${collapsed ? 'collapsed' : ''}" data-cluster="${cluster.id}">
        <div class="cluster-head">
          <div>
            <h2>Cluster ${cluster.id}</h2>
            <div class="cluster-meta">${escapeHtml(cluster.directory || '.')} · ${escapeHtml(cluster.prefix || '(no prefix)')}</div>
          </div>
          <div class="cluster-meta">${shown} · ${clusterStatus} · suggested keep ${cluster.keep_count}</div>
          <button class="toggle" title="Collapse or expand cluster">${collapsed ? '+' : '-'}</button>
        </div>
        <div class="grid">${collapsed ? '' : assets.map(asset => frameHtml(asset, diffKeys)).join('')}</div>
      </section>`;
    }

    function frameHtml(asset, diffKeys) {
      const action = finalAction(asset);
      const user = decisions.get(asset.id);
      const checked = action === 'keep' ? 'checked' : '';
      const indeterminate = action === 'review' ? 'data-indeterminate="1"' : '';
      const thumb = asset.thumb ? `<img src="/${asset.thumb}" loading="lazy" alt="">` : '';
      return `<article class="item ${action}" data-id="${asset.id}">
        <div class="thumb open-full" title="Open full-resolution view">${thumb}<span class="badge">${badgeText(asset, action)}</span></div>
        <div class="meta">
          <label class="keepbox"><input type="checkbox" ${checked} ${indeterminate}> Keep</label>
          <div class="name">${escapeHtml(asset.representative.rel_path)}</div>
          <div class="exif">${exifHtml(asset, diffKeys)}</div>
          <div class="reason">${escapeHtml(shortReason(asset))}</div>
          <details>
            <summary>Why</summary>
            <ul>${detailLines(asset).map(line => `<li>${escapeHtml(line)}</li>`).join('')}</ul>
          </details>
          ${user && user.decision ? '<button class="reset">Reset to suggestion</button>' : ''}
        </div>
      </article>`;
    }

    function badgeText(asset, action) {
      if (action === 'error') return 'error';
      if (action === 'review') return 'review';
      return action === 'keep' ? 'keep' : 'reject';
    }

    function shortReason(asset) {
      if (asset.suggestion.action === 'keep') return asset.suggestion.reason;
      if (asset.suggestion.action === 'review') return 'Close call; inspect before moving rejects.';
      if (asset.suggestion.action === 'error') return asset.error || 'Could not decode.';
      return 'Lower-ranked duplicate in this burst.';
    }

    function detailLines(asset) {
      const detector = asset.detector ? `${asset.detector.backend}: ${asset.detector.explanation}` : 'Detector: not used';
      return [
        `Rank ${asset.suggestion.rank}, score ${asset.suggestion.score.toFixed(3)}.`,
        `Sharpness ${asset.metrics.sharpness.toFixed(1)}, completeness ${asset.metrics.completeness.toFixed(2)}, exposure ${asset.metrics.exposure_score.toFixed(2)}.`,
        `Feature backend: ${asset.feature_backend || 'unknown'}; decoder: ${asset.decoder || 'unknown'}.`,
        detector,
        ...asset.suggestion.explanations,
      ];
    }

    const exifFields = [
      ['iso', 'ISO'],
      ['aperture', 'f/'],
      ['shutter', ''],
      ['focal_length_mm', 'mm'],
      ['focal_length_35mm', 'eq'],
    ];

    function exifDiffKeys(assets) {
      const result = new Set();
      for (const [key] of exifFields) {
        const values = new Set();
        for (const asset of assets) {
          const value = metadataCompareValue(asset.metadata, key);
          if (value !== null) values.add(value);
        }
        if (values.size > 1) result.add(key);
      }
      return result;
    }

    function exifHtml(asset, diffKeys) {
      const metadata = asset.metadata || {};
      const parts = exifFields
        .map(([key]) => {
          const text = metadataDisplayValue(metadata, key);
          if (!text) return '';
          return `<span class="${diffKeys.has(key) ? 'diff' : ''}">${escapeHtml(text)}</span>`;
        })
        .filter(Boolean);
      return parts.length ? parts.join('') : '<span>EXIF unavailable</span>';
    }

    function metadataCompareValue(metadata = {}, key) {
      const value = metadata[key];
      if (value === null || value === undefined || value === '') return null;
      if (typeof value === 'number') return Number(value).toFixed(key === 'aperture' || key === 'focal_length_mm' ? 1 : 0);
      return String(value);
    }

    function metadataDisplayValue(metadata = {}, key) {
      const value = metadata[key];
      if (value === null || value === undefined || value === '') return '';
      if (key === 'iso') return `ISO ${value}`;
      if (key === 'aperture') return `f/${formatNumber(value, 1)}`;
      if (key === 'shutter') return value;
      if (key === 'focal_length_mm') return `${formatNumber(value, 1)}mm`;
      if (key === 'focal_length_35mm') return `${value}mm eq`;
      return String(value);
    }

    function formatNumber(value, digits) {
      return Number(value).toFixed(digits).replace(/\.0$/, '');
    }

    async function saveDecision(asset_id, decision, note) {
      const res = await fetch('/api/decision', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ asset_id, decision, note })
      });
      if (!res.ok) throw new Error(await res.text());
      review = await res.json();
      decisions.clear();
      for (const item of review.decisions) decisions.set(item.asset_id, item);
      render();
    }

    async function openViewer(asset) {
      const loadToken = ++viewerLoadToken;
      currentViewerAssetId = asset.id;
      const clusterId = clusterByAsset.get(asset.id);
      currentViewerClusterIds = clusterId ? (clusterAssets.get(clusterId) || []).map(item => item.id) : [asset.id];
      $('#viewerTitle').textContent = asset.representative.rel_path;
      $('#viewer').classList.add('open');
      $('#viewer').setAttribute('aria-hidden', 'false');
      $('#viewerError').hidden = true;
      $('#viewerImg').hidden = true;
      syncViewerKeep(asset);
      setViewerLoading(true);
      if (viewerUrl) URL.revokeObjectURL(viewerUrl);
      viewerUrl = null;
      try {
        if (rawOnly(asset)) {
          await openRawViewer(asset, loadToken);
        } else {
          const res = await fetch(`/api/image/${asset.id}`);
          if (!res.ok) throw new Error(await res.text());
          const blob = await res.blob();
          if (loadToken !== viewerLoadToken) return;
          showViewerBlob(blob, loadToken);
        }
      } catch (error) {
        if (loadToken !== viewerLoadToken) return;
        $('#viewerError').textContent = error.message;
        $('#viewerError').hidden = false;
        setViewerLoading(false);
      }
    }

    function rawOnly(asset) {
      const files = asset.files || [];
      return files.some(file => file.kind === 'raw')
        && !files.some(file => browserImageExts.has(String(file.extension || '').toLowerCase()));
    }

    async function openRawViewer(asset, loadToken) {
      $('#viewerTitle').textContent = `${asset.representative.rel_path} (RAW preview)`;
      const cached = getCachedRawPreview(asset.id);
      if (cached) {
        showViewerBlob(cached, loadToken);
        return;
      }
      try {
        const blob = await decodeRawWithWasm(asset);
        if (loadToken !== viewerLoadToken) return;
        cacheRawPreview(asset.id, blob);
        showViewerBlob(blob, loadToken);
      } catch (wasmError) {
        if (loadToken !== viewerLoadToken) return;
        const res = await fetch(`/api/preview/${asset.id}`);
        if (!res.ok) {
          throw new Error(`RAW preview failed. Browser decoder: ${wasmError.message}. Backend fallback: ${await res.text()}`);
        }
        const blob = await res.blob();
        if (loadToken !== viewerLoadToken) return;
        cacheRawPreview(asset.id, blob);
        showViewerBlob(blob, loadToken);
      }
    }

    function getCachedRawPreview(assetId) {
      const entry = rawPreviewCache.get(assetId);
      if (!entry) return null;
      rawPreviewCache.delete(assetId);
      rawPreviewCache.set(assetId, entry);
      return entry.blob;
    }

    function cacheRawPreview(assetId, blob) {
      if (!blob || !blob.size || blob.size > RAW_PREVIEW_CACHE_MAX_BYTES) return;
      const existing = rawPreviewCache.get(assetId);
      if (existing) {
        rawPreviewCacheBytes -= existing.size;
        rawPreviewCache.delete(assetId);
      }
      const entry = { blob, size: blob.size };
      rawPreviewCache.set(assetId, entry);
      rawPreviewCacheBytes += entry.size;
      while (
        rawPreviewCache.size > RAW_PREVIEW_CACHE_MAX_ITEMS
        || rawPreviewCacheBytes > RAW_PREVIEW_CACHE_MAX_BYTES
      ) {
        const oldestKey = rawPreviewCache.keys().next().value;
        if (!oldestKey) break;
        const oldest = rawPreviewCache.get(oldestKey);
        rawPreviewCacheBytes -= oldest ? oldest.size : 0;
        rawPreviewCache.delete(oldestKey);
      }
    }

    async function decodeRawWithWasm(asset) {
      const LibRaw = await getLibRawCtor();
      const res = await fetch(`/api/source/${asset.id}`);
      if (!res.ok) throw new Error(await res.text());
      const bytes = new Uint8Array(await res.arrayBuffer());
      const raw = new LibRaw();
      try {
        await raw.open(bytes, {
          halfSize: true,
          useCameraWb: true,
          outputColor: 1,
          outputBps: 8,
          userQual: 1,
        });
        const imageData = await raw.imageData();
        return await rawImageDataToBlob(imageData);
      } finally {
        if (typeof raw.dispose === 'function') raw.dispose();
      }
    }

    function getLibRawCtor() {
      if (!libRawCtorPromise) {
        libRawCtorPromise = import('/vendor/libraw-wasm/index.js').then(module => module.default);
      }
      return libRawCtorPromise;
    }

    function rawImageDataToBlob(imageData) {
      const width = imageData.width;
      const height = imageData.height;
      const rgb = imageData.data;
      if (!width || !height || !rgb || rgb.length < width * height * 3) {
        throw new Error('RAW decoder returned unsupported pixel data.');
      }
      const rgba = new Uint8ClampedArray(width * height * 4);
      for (let src = 0, dst = 0; src < width * height * 3; src += 3, dst += 4) {
        rgba[dst] = rgb[src];
        rgba[dst + 1] = rgb[src + 1];
        rgba[dst + 2] = rgb[src + 2];
        rgba[dst + 3] = 255;
      }
      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext('2d', { alpha: false });
      ctx.putImageData(new ImageData(rgba, width, height), 0, 0);
      return new Promise((resolve, reject) => {
        canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error('Could not encode RAW preview.')), 'image/jpeg', 0.92);
      });
    }

    function showViewerBlob(blob, loadToken) {
      if (loadToken !== viewerLoadToken) return;
      viewerUrl = URL.createObjectURL(blob);
      const img = $('#viewerImg');
      img.onload = () => {
        if (loadToken !== viewerLoadToken) return;
        fitViewer();
        setViewerLoading(false);
      };
      img.onerror = () => {
        if (loadToken !== viewerLoadToken) return;
        $('#viewerError').textContent = 'Preview image could not be loaded. The source path may be inaccessible or unsupported.';
        $('#viewerError').hidden = false;
        setViewerLoading(false);
      };
      img.src = viewerUrl;
      img.hidden = false;
    }

    function setViewerLoading(loading) {
      $('#viewerLoading').hidden = !loading;
    }

    function syncViewerKeep(asset) {
      const input = $('#viewerKeepBox');
      const action = finalAction(asset);
      input.checked = action === 'keep';
      input.indeterminate = action === 'review';
    }

    function showAdjacentViewer(delta) {
      if (!currentViewerAssetId || !currentViewerClusterIds.length) return;
      const index = currentViewerClusterIds.indexOf(currentViewerAssetId);
      if (index < 0) return;
      const nextIndex = Math.max(0, Math.min(currentViewerClusterIds.length - 1, index + delta));
      if (nextIndex === index) return;
      const asset = assetById.get(currentViewerClusterIds[nextIndex]);
      if (asset) openViewer(asset);
    }

    function closeViewer() {
      viewerLoadToken++;
      $('#viewer').classList.remove('open');
      $('#viewer').setAttribute('aria-hidden', 'true');
      currentViewerAssetId = null;
      currentViewerClusterIds = [];
      setViewerLoading(false);
      if (viewerUrl) URL.revokeObjectURL(viewerUrl);
      viewerUrl = null;
      $('#viewerImg').removeAttribute('src');
    }

    function fitViewer() {
      const img = $('#viewerImg');
      const viewport = $('#viewport');
      if (!img.naturalWidth || !img.naturalHeight) return;
      const scaleX = viewport.clientWidth / img.naturalWidth;
      const scaleY = viewport.clientHeight / img.naturalHeight;
      viewerScale = Math.min(scaleX, scaleY, 1);
      viewerX = 0;
      viewerY = 0;
      applyViewerTransform();
    }

    function applyViewerTransform() {
      $('#viewerImg').style.transform = `translate(calc(-50% + ${viewerX}px), calc(-50% + ${viewerY}px)) scale(${viewerScale})`;
    }

    function zoomViewer(factor) {
      viewerScale = Math.max(0.05, Math.min(8, viewerScale * factor));
      applyViewerTransform();
    }

    async function moveRejects() {
      const rejectCount = manifest.assets.filter(asset => finalAction(asset) === 'reject').length;
      if (!rejectCount) return;
      const ok = window.confirm(`Move ${rejectCount} rejected asset(s) into a local folder under this run directory?\\n\\nThis copies each file, verifies the copy size, then removes the original from the source card/folder. You can delete the local moved_rejects folder yourself later.`);
      if (!ok) return;
      const res = await fetch('/api/move-rejects', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ confirm: true })
      });
      const text = await res.text();
      if (!res.ok) {
        window.alert(text);
        return;
      }
      const result = JSON.parse(text);
      window.alert(`Moved ${result.moved_files} file(s) from ${result.moved_assets} asset(s).\\nDestination: ${result.destination}\\nMissing: ${result.missing_files.length}\\nFailed: ${result.failed_files.length}`);
      await load();
    }

    function escapeHtml(value) {
      return String(value ?? '').replace(/[&<>"']/g, ch => ({
        '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
      }[ch]));
    }

    $('#clusters').addEventListener('change', async event => {
      const input = event.target.closest('.item input[type="checkbox"]');
      if (!input) return;
      input.indeterminate = false;
      const card = input.closest('.item');
      try {
        await saveDecision(card.dataset.id, input.checked ? 'keep' : 'reject', null);
      } catch (error) {
        window.alert(error.message);
      }
    });
    $('#clusters').addEventListener('click', async event => {
      const showMore = event.target.closest('[data-show-more]');
      if (showMore) {
        visibleClusterLimit += 80;
        renderClusters();
        return;
      }
      const toggle = event.target.closest('.cluster .toggle');
      if (toggle) {
        const cluster = toggle.closest('.cluster');
        const id = Number(cluster.dataset.cluster);
        if (cluster.classList.contains('collapsed')) {
          manuallyOpenedClusters.add(id);
          manuallyClosedClusters.delete(id);
        } else {
          manuallyClosedClusters.add(id);
          manuallyOpenedClusters.delete(id);
        }
        renderClusters();
        return;
      }
      const reset = event.target.closest('.item .reset');
      if (reset) {
        const card = reset.closest('.item');
        try {
          await saveDecision(card.dataset.id, null, null);
        } catch (error) {
          window.alert(error.message);
        }
        return;
      }
      const thumb = event.target.closest('.item .open-full');
      if (thumb) {
        const card = thumb.closest('.item');
        const asset = manifest.assets.find(item => item.id === card.dataset.id);
        if (asset) openViewer(asset);
      }
    });
    $('#search').addEventListener('input', () => scheduleClusterRender(true));
    $('#filter').addEventListener('change', () => {
      visibleClusterLimit = 80;
      renderClusters();
    });
    $('#moveBtn').addEventListener('click', moveRejects);
    $('#exportBtn').addEventListener('click', async () => {
      try {
        const res = await fetch('/api/export', { method: 'POST' });
        if (!res.ok) throw new Error(await res.text());
        await load();
        window.alert('Review files saved in the run directory.');
      } catch (error) {
        window.alert(error.message);
      }
    });
    load().catch(error => {
      $('#clusters').innerHTML = `<div class="empty">${escapeHtml(error.message)}</div>`;
    });
    $('#viewerCloseBtn').addEventListener('click', closeViewer);
    $('#viewerKeepBox').addEventListener('change', async event => {
      if (!currentViewerAssetId) return;
      event.target.indeterminate = false;
      try {
        await saveDecision(currentViewerAssetId, event.target.checked ? 'keep' : 'reject', null);
        const asset = assetById.get(currentViewerAssetId);
        if (asset) syncViewerKeep(asset);
      } catch (error) {
        window.alert(error.message);
      }
    });
    $('#zoomInBtn').addEventListener('click', () => zoomViewer(1.25));
    $('#zoomOutBtn').addEventListener('click', () => zoomViewer(0.8));
    $('#zoomResetBtn').addEventListener('click', fitViewer);
    $('#viewport').addEventListener('wheel', event => {
      event.preventDefault();
      zoomViewer(event.deltaY < 0 ? 1.12 : 0.89);
    }, { passive: false });
    $('#viewport').addEventListener('pointerdown', event => {
      dragging = true;
      dragStart = { x: event.clientX, y: event.clientY, viewerX, viewerY };
      $('#viewport').classList.add('dragging');
      $('#viewport').setPointerCapture(event.pointerId);
    });
    $('#viewport').addEventListener('pointermove', event => {
      if (!dragging || !dragStart) return;
      viewerX = dragStart.viewerX + event.clientX - dragStart.x;
      viewerY = dragStart.viewerY + event.clientY - dragStart.y;
      applyViewerTransform();
    });
    $('#viewport').addEventListener('pointerup', event => {
      dragging = false;
      dragStart = null;
      $('#viewport').classList.remove('dragging');
      $('#viewport').releasePointerCapture(event.pointerId);
    });
    document.addEventListener('keydown', event => {
      if (!$('#viewer').classList.contains('open')) return;
      if (event.key === 'Escape') closeViewer();
      if (event.key === 'ArrowLeft') {
        event.preventDefault();
        showAdjacentViewer(-1);
      }
      if (event.key === 'ArrowRight') {
        event.preventDefault();
        showAdjacentViewer(1);
      }
    });
  </script>
</body>
</html>
"#;
