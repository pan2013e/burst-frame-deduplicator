use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use chrono::Local;
use serde::Serialize;

use crate::artifacts::{ensure_review_state, read_manifest};
use crate::types::{AssetRecord, ReviewState, SuggestedAction, UserDecision};

#[derive(Debug, Clone, Serialize)]
pub struct MoveRejectsResponse {
    pub destination: PathBuf,
    pub moved_files: usize,
    pub moved_assets: usize,
    pub missing_files: Vec<String>,
    pub failed_files: Vec<MoveFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoveFailure {
    pub source: String,
    pub error: String,
}

pub fn move_rejects(run_dir: &Path, confirmed: bool) -> anyhow::Result<MoveRejectsResponse> {
    if !confirmed {
        bail!("move requires explicit confirmation");
    }

    let manifest = read_manifest(run_dir)?;
    let review = ensure_review_state(run_dir, &manifest)?;
    let destination = run_dir
        .join("moved_rejects")
        .join(Local::now().format("%Y%m%d_%H%M%S").to_string());
    fs::create_dir_all(&destination)
        .with_context(|| format!("creating move destination {}", destination.display()))?;

    let mut response = MoveRejectsResponse {
        destination: destination.clone(),
        moved_files: 0,
        moved_assets: 0,
        missing_files: Vec::new(),
        failed_files: Vec::new(),
    };

    for asset in &manifest.assets {
        if final_action_for_asset(asset, &review) != UserDecision::Reject {
            continue;
        }
        let mut asset_moved = false;
        for file in asset.files.iter().chain(asset.sidecars.iter()) {
            if !file.path.exists() {
                response.missing_files.push(file.path.display().to_string());
                continue;
            }
            let target = unique_target(&move_target(&destination, &file.rel_path, &file.path));
            match move_file_verified(&file.path, &target) {
                Ok(()) => {
                    response.moved_files += 1;
                    asset_moved = true;
                }
                Err(error) => response.failed_files.push(MoveFailure {
                    source: file.path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        if asset_moved {
            response.moved_assets += 1;
        }
    }

    let report_path = run_dir
        .join("move_reports")
        .join(format!("{}.json", Local::now().format("%Y%m%d_%H%M%S")));
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = fs::File::create(&report_path)
        .with_context(|| format!("creating {}", report_path.display()))?;
    serde_json::to_writer_pretty(report, &response)?;
    Ok(response)
}

pub fn final_action_for_asset(asset: &AssetRecord, review: &ReviewState) -> UserDecision {
    if let Some(decision) = review
        .decisions
        .iter()
        .find(|decision| decision.asset_id == asset.id)
        .and_then(|decision| decision.decision)
    {
        return decision;
    }
    match asset.suggestion.action {
        SuggestedAction::Keep => UserDecision::Keep,
        SuggestedAction::Reject => UserDecision::Reject,
        SuggestedAction::Review | SuggestedAction::Error => UserDecision::Review,
    }
}

fn unique_target(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
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

fn move_target(destination: &Path, relative_path: &str, source: &Path) -> PathBuf {
    let mut safe_relative = PathBuf::new();
    for component in Path::new(relative_path).components() {
        if let Component::Normal(value) = component {
            safe_relative.push(value);
        }
    }
    if safe_relative.as_os_str().is_empty() {
        safe_relative.push(
            source
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("file")),
        );
    }
    destination.join(safe_relative)
}

fn move_file_verified(source: &Path, target: &Path) -> io::Result<()> {
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{move_rejects, move_target};

    #[test]
    fn move_requires_confirmation_before_reading_a_run() {
        let error = move_rejects(std::path::Path::new("missing-run"), false).unwrap_err();
        assert!(error.to_string().contains("explicit confirmation"));
    }

    #[test]
    fn move_target_cannot_escape_the_run_destination() {
        let destination = Path::new("run/moved_rejects/attempt");
        let target = move_target(
            destination,
            "../../outside/frame.jpg",
            Path::new("source/frame.jpg"),
        );
        assert_eq!(target, destination.join("outside/frame.jpg"));
        assert!(target.starts_with(destination));
    }
}
