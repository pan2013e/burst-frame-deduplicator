use std::fs;
#[cfg(all(target_os = "linux", feature = "libraw-preview"))]
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(all(target_os = "linux", feature = "libraw-preview"))]
use anyhow::Context;
use anyhow::anyhow;
use serde::Serialize;
#[cfg(all(target_os = "linux", feature = "libraw-preview"))]
use tempfile::NamedTempFile;

use crate::artifacts::{
    ensure_review_state, export_reviewed_artifacts, read_manifest, read_manifest_with_progress,
    upsert_decision,
};
use crate::decode::write_preview_jpeg;
use crate::operations::{MoveStatus, read_move_status, resolve_available_source};
use crate::progress::{ProgressReporter, ProgressStage};
use crate::types::{FileKind, ReviewState, RunManifest, UserDecision};

#[derive(Debug, Clone, Serialize)]
pub struct ReviewPayload {
    pub run_dir: PathBuf,
    pub manifest: RunManifest,
    pub review: ReviewState,
    pub move_status: MoveStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewResponse {
    pub path: PathBuf,
    pub generated: bool,
}

pub fn load_run(run_dir: impl Into<PathBuf>) -> anyhow::Result<ReviewPayload> {
    load_run_with_progress(run_dir, ProgressReporter::default())
}

pub fn load_run_with_progress(
    run_dir: impl Into<PathBuf>,
    progress: ProgressReporter,
) -> anyhow::Result<ReviewPayload> {
    let run_dir = run_dir.into();
    progress.emit(ProgressStage::ReadingManifest, 0, Some(1), None);
    let manifest_progress = progress.clone();
    let manifest = read_manifest_with_progress(&run_dir, move |current, total| {
        manifest_progress.emit(
            ProgressStage::ReadingManifest,
            usize::try_from(current).unwrap_or(usize::MAX),
            Some(usize::try_from(total).unwrap_or(usize::MAX)),
            None,
        );
    })?;
    progress.emit(ProgressStage::LoadingDecisions, 0, Some(1), None);
    let review = ensure_review_state(&run_dir, &manifest)?;
    progress.emit(
        ProgressStage::LoadingDecisions,
        1,
        Some(1),
        Some(format!("{} decisions", review.decisions.len())),
    );
    progress.emit(ProgressStage::LoadingMoveHistory, 0, Some(1), None);
    let move_status = read_move_status(&run_dir)?;
    progress.emit(
        ProgressStage::LoadingMoveHistory,
        1,
        Some(1),
        Some(format!(
            "{} moved assets",
            move_status.active_asset_ids.len()
        )),
    );
    progress.emit(ProgressStage::PreparingReview, 0, Some(1), None);
    Ok(ReviewPayload {
        run_dir,
        manifest,
        review,
        move_status,
    })
}

pub fn set_decision(
    run_dir: &Path,
    asset_id: String,
    decision: Option<UserDecision>,
) -> anyhow::Result<ReviewPayload> {
    upsert_decision(run_dir, asset_id, decision, None)?;
    load_run(run_dir.to_path_buf())
}

pub fn export_run(run_dir: &Path) -> anyhow::Result<ReviewPayload> {
    export_reviewed_artifacts(run_dir)?;
    load_run(run_dir.to_path_buf())
}

pub fn prepare_preview(
    run_dir: &Path,
    asset_id: &str,
    max_long_edge: u32,
    generate_if_missing: bool,
) -> anyhow::Result<PreviewResponse> {
    let (asset, source_path) = preview_asset(run_dir, asset_id)?;
    if asset.representative.kind != FileKind::Raw {
        return Ok(PreviewResponse {
            path: source_path,
            generated: false,
        });
    }

    let max_long_edge = max_long_edge.clamp(1024, 8192);
    let preview_dir = run_dir.join("native_previews");
    fs::create_dir_all(&preview_dir)?;
    let output = preview_dir.join(format!("{}_{}.jpg", asset.id, max_long_edge));
    if output.is_file() {
        return Ok(PreviewResponse {
            path: output,
            generated: true,
        });
    }
    if !generate_if_missing {
        return Ok(PreviewResponse {
            path: source_path,
            generated: false,
        });
    }

    write_preview_jpeg(&source_path, max_long_edge, &output)?;
    Ok(PreviewResponse {
        path: output,
        generated: true,
    })
}

#[cfg(all(target_os = "linux", feature = "libraw-preview"))]
pub fn prepare_embedded_preview(run_dir: &Path, asset_id: &str) -> anyhow::Result<PreviewResponse> {
    let (asset, source_path) = preview_asset(run_dir, asset_id)?;
    if asset.representative.kind != FileKind::Raw {
        return Ok(PreviewResponse {
            path: source_path,
            generated: false,
        });
    }

    let preview_dir = run_dir.join("native_previews");
    fs::create_dir_all(&preview_dir)?;
    let output = preview_dir.join(format!("{}_embedded.preview", asset.id));
    if output.is_file() {
        return Ok(PreviewResponse {
            path: output,
            generated: true,
        });
    }

    let bytes = crate::libraw_preview::extract_embedded_preview(&source_path)?;
    write_atomic(&output, &bytes)?;
    Ok(PreviewResponse {
        path: output,
        generated: true,
    })
}

pub fn preview_needs_refinement(
    available_long_edge: u32,
    magnification: f64,
    display_scale: f64,
    target_long_edge: u32,
) -> bool {
    if available_long_edge == 0
        || target_long_edge == 0
        || !magnification.is_finite()
        || !display_scale.is_finite()
        || magnification <= 0.0
        || display_scale <= 0.0
    {
        return false;
    }
    let resolution_gain = f64::from(target_long_edge) / f64::from(available_long_edge);
    resolution_gain >= 1.15 && magnification * display_scale > 1.05
}

fn preview_asset(
    run_dir: &Path,
    asset_id: &str,
) -> anyhow::Result<(crate::types::AssetRecord, PathBuf)> {
    let manifest = read_manifest(run_dir)?;
    let asset = manifest
        .assets
        .into_iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| anyhow!("asset not found: {asset_id}"))?;
    let source_path = resolve_available_source(run_dir, &asset.representative.path)?;
    Ok((asset, source_path))
}

#[cfg(all(target_os = "linux", feature = "libraw-preview"))]
fn write_atomic(destination: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("preview destination has no parent"))?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)
        .with_context(|| format!("publishing {}", destination.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::preview_needs_refinement;

    #[test]
    fn preview_refinement_is_driven_by_device_pixel_demand() {
        assert!(!preview_needs_refinement(1920, 0.46, 2.0, 4096));
        assert!(preview_needs_refinement(1920, 0.60, 2.0, 4096));
        assert!(!preview_needs_refinement(3840, 1.0, 2.0, 4096));
    }
}
