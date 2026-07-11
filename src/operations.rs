use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::artifacts::{ensure_review_state, read_manifest};
use crate::types::{AssetRecord, ReviewState, SuggestedAction, UserDecision};

const MOVE_STATE_VERSION: u32 = 1;
const MOVE_STATE_FILE: &str = "move_state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRecord {
    pub asset_id: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub size: u64,
    pub moved_at: String,
    pub restored_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveState {
    pub version: u32,
    pub updated_at: String,
    pub records: Vec<MoveRecord>,
}

impl Default for MoveState {
    fn default() -> Self {
        Self {
            version: MOVE_STATE_VERSION,
            updated_at: Utc::now().to_rfc3339(),
            records: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MoveStatus {
    pub active_asset_ids: Vec<String>,
    pub active_files: usize,
    pub active_bytes: u64,
    pub destinations: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRejectsResponse {
    pub destination: PathBuf,
    pub moved_files: usize,
    pub moved_assets: usize,
    pub already_moved_assets: usize,
    pub moved_asset_ids: Vec<String>,
    pub source_available: bool,
    pub missing_files: Vec<String>,
    pub failed_files: Vec<MoveFailure>,
    pub message: Option<String>,
    pub status: MoveStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResponse {
    pub restored_files: usize,
    pub restored_assets: usize,
    pub restored_asset_ids: Vec<String>,
    pub source_available: bool,
    pub missing_files: Vec<String>,
    pub failed_files: Vec<MoveFailure>,
    pub message: Option<String>,
    pub status: MoveStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveFailure {
    pub source: String,
    pub error: String,
}

#[derive(Debug, Clone)]
struct TransferPlan {
    source: PathBuf,
    target: PathBuf,
    size: u64,
}

pub fn read_move_state(run_dir: &Path) -> anyhow::Result<MoveState> {
    let path = run_dir.join(MOVE_STATE_FILE);
    if !path.is_file() {
        return Ok(MoveState::default());
    }
    let file = fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
    let state: MoveState =
        serde_json::from_reader(file).with_context(|| format!("parsing {}", path.display()))?;
    if state.version != MOVE_STATE_VERSION {
        bail!(
            "unsupported move state version {} in {}",
            state.version,
            path.display()
        );
    }
    Ok(state)
}

pub fn read_move_status(run_dir: &Path) -> anyhow::Result<MoveStatus> {
    Ok(move_status(&read_move_state(run_dir)?))
}

pub fn resolve_available_source(run_dir: &Path, original: &Path) -> anyhow::Result<PathBuf> {
    if original.is_file() {
        return Ok(original.to_path_buf());
    }
    let state = read_move_state(run_dir)?;
    Ok(state
        .records
        .iter()
        .rev()
        .find(|record| {
            record.source == original
                && record.restored_at.is_none()
                && record.destination.is_file()
        })
        .map(|record| record.destination.clone())
        .unwrap_or_else(|| original.to_path_buf()))
}

pub fn move_rejects(
    run_dir: &Path,
    destination_root: Option<&Path>,
    confirmed: bool,
) -> anyhow::Result<MoveRejectsResponse> {
    if !confirmed {
        bail!("move requires explicit confirmation");
    }

    let manifest = read_manifest(run_dir)?;
    let review = ensure_review_state(run_dir, &manifest)?;
    let source_available = manifest.root.is_dir();
    let destination = move_destination(run_dir, destination_root)?;
    validate_destination(&manifest.root, &destination)?;
    let mut state = read_move_state(run_dir)?;
    let active_assets = active_asset_ids(&state);
    let mut response = MoveRejectsResponse {
        destination: destination.clone(),
        moved_files: 0,
        moved_assets: 0,
        already_moved_assets: 0,
        moved_asset_ids: Vec::new(),
        source_available,
        missing_files: Vec::new(),
        failed_files: Vec::new(),
        message: None,
        status: move_status(&state),
    };

    if !source_available {
        response.message = Some(format!(
            "The source folder is unavailable: {}. Reconnect the card or folder before moving files.",
            manifest.root.display()
        ));
        return Ok(response);
    }

    let mut reserved_targets = HashSet::new();
    for asset in &manifest.assets {
        if final_action_for_asset(asset, &review) != UserDecision::Reject {
            continue;
        }
        if active_assets.contains(&asset.id) {
            response.already_moved_assets += 1;
            continue;
        }

        let entries: Vec<_> = asset.files.iter().chain(asset.sidecars.iter()).collect();
        let missing: Vec<_> = entries
            .iter()
            .filter(|file| !file.path.is_file())
            .map(|file| file.path.display().to_string())
            .collect();
        if !missing.is_empty() {
            response.missing_files.extend(missing.iter().cloned());
            response.failed_files.push(MoveFailure {
                source: asset.representative.path.display().to_string(),
                error: format!(
                    "Asset was not moved because {} grouped file(s) are unavailable",
                    missing.len()
                ),
            });
            continue;
        }

        let mut plans = Vec::with_capacity(entries.len());
        for file in entries {
            let desired = move_target(&destination, &file.rel_path, &file.path);
            let target = unique_target(&desired, &mut reserved_targets);
            plans.push(TransferPlan {
                size: fs::metadata(&file.path)?.len(),
                source: file.path.clone(),
                target,
            });
        }

        if let Err(error) = transfer_verified(&plans, true) {
            response.failed_files.push(MoveFailure {
                source: asset.representative.path.display().to_string(),
                error: error.to_string(),
            });
            continue;
        }

        let moved_at = Utc::now().to_rfc3339();
        let old_len = state.records.len();
        state.records.extend(plans.iter().map(|plan| MoveRecord {
            asset_id: asset.id.clone(),
            source: plan.source.clone(),
            destination: plan.target.clone(),
            size: plan.size,
            moved_at: moved_at.clone(),
            restored_at: None,
        }));
        state.updated_at = moved_at;
        if let Err(error) = write_move_state(run_dir, &state) {
            state.records.truncate(old_len);
            rollback_completed_transfer(&plans).with_context(|| {
                format!("move journal failed ({error}); restoring the source asset also failed")
            })?;
            return Err(error.context("writing move journal; source files were restored"));
        }

        response.moved_files += plans.len();
        response.moved_assets += 1;
        response.moved_asset_ids.push(asset.id.clone());
    }

    response.status = move_status(&state);
    if response.moved_assets == 0 && response.failed_files.is_empty() {
        response.message = Some(if response.already_moved_assets > 0 {
            "All rejected assets were already moved.".to_string()
        } else {
            "There are no rejected assets to move.".to_string()
        });
    }
    write_operation_report(run_dir, "move", &response)?;
    Ok(response)
}

pub fn restore_moved(
    run_dir: &Path,
    asset_ids: Option<&HashSet<String>>,
    confirmed: bool,
) -> anyhow::Result<RestoreResponse> {
    if !confirmed {
        bail!("restore requires explicit confirmation");
    }

    let manifest = read_manifest(run_dir)?;
    let source_available = manifest.root.is_dir();
    let mut state = read_move_state(run_dir)?;
    let mut response = RestoreResponse {
        restored_files: 0,
        restored_assets: 0,
        restored_asset_ids: Vec::new(),
        source_available,
        missing_files: Vec::new(),
        failed_files: Vec::new(),
        message: None,
        status: move_status(&state),
    };

    if !source_available {
        response.message = Some(format!(
            "The original source folder is unavailable: {}. Reconnect it before restoring files.",
            manifest.root.display()
        ));
        return Ok(response);
    }

    let mut by_asset: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, record) in state.records.iter().enumerate() {
        if record.restored_at.is_none()
            && asset_ids.is_none_or(|selected| selected.contains(&record.asset_id))
        {
            by_asset
                .entry(record.asset_id.clone())
                .or_default()
                .push(index);
        }
    }

    for (asset_id, indices) in by_asset {
        let records: Vec<_> = indices
            .iter()
            .map(|index| state.records[*index].clone())
            .collect();
        let mut unavailable = Vec::new();
        let mut conflicts = Vec::new();
        for record in &records {
            if !record.destination.is_file() {
                unavailable.push(record.destination.display().to_string());
            }
            let Some(parent) = record.source.parent() else {
                conflicts.push(format!(
                    "Invalid original path: {}",
                    record.source.display()
                ));
                continue;
            };
            if !parent.is_dir() {
                conflicts.push(format!(
                    "Original folder is unavailable: {}",
                    parent.display()
                ));
            } else if record.source.exists() {
                conflicts.push(format!(
                    "Original path already contains a file: {}",
                    record.source.display()
                ));
            }
        }
        if !unavailable.is_empty() || !conflicts.is_empty() {
            response.missing_files.extend(unavailable);
            response.failed_files.push(MoveFailure {
                source: records
                    .first()
                    .map(|record| record.source.display().to_string())
                    .unwrap_or_else(|| asset_id.clone()),
                error: conflicts
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "A moved file is unavailable.".to_string()),
            });
            continue;
        }

        let plans: Vec<_> = records
            .iter()
            .map(|record| TransferPlan {
                source: record.destination.clone(),
                target: record.source.clone(),
                size: record.size,
            })
            .collect();
        if let Err(error) = transfer_verified(&plans, false) {
            response.failed_files.push(MoveFailure {
                source: records[0].source.display().to_string(),
                error: error.to_string(),
            });
            continue;
        }

        let restored_at = Utc::now().to_rfc3339();
        for index in &indices {
            state.records[*index].restored_at = Some(restored_at.clone());
        }
        state.updated_at = restored_at;
        if let Err(error) = write_move_state(run_dir, &state) {
            for index in &indices {
                state.records[*index].restored_at = None;
            }
            rollback_completed_transfer(&plans).with_context(|| {
                format!("restore journal failed ({error}); returning files to the move destination also failed")
            })?;
            return Err(error
                .context("writing restore journal; files were returned to the move destination"));
        }

        response.restored_files += plans.len();
        response.restored_assets += 1;
        response.restored_asset_ids.push(asset_id);
        remove_empty_destination_parents(&records);
    }

    response.status = move_status(&state);
    if response.restored_assets == 0 && response.failed_files.is_empty() {
        response.message = Some("There are no moved assets to restore.".to_string());
    }
    write_operation_report(run_dir, "restore", &response)?;
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

fn active_asset_ids(state: &MoveState) -> HashSet<String> {
    state
        .records
        .iter()
        .filter(|record| record.restored_at.is_none())
        .map(|record| record.asset_id.clone())
        .collect()
}

fn move_status(state: &MoveState) -> MoveStatus {
    let active: Vec<_> = state
        .records
        .iter()
        .filter(|record| record.restored_at.is_none())
        .collect();
    let asset_ids: BTreeSet<_> = active
        .iter()
        .map(|record| record.asset_id.clone())
        .collect();
    let destinations: BTreeSet<_> = active
        .iter()
        .filter_map(|record| record.destination.parent().map(Path::to_path_buf))
        .collect();
    MoveStatus {
        active_asset_ids: asset_ids.into_iter().collect(),
        active_files: active.len(),
        active_bytes: active.iter().map(|record| record.size).sum(),
        destinations: destinations.into_iter().collect(),
    }
}

fn move_destination(run_dir: &Path, destination_root: Option<&Path>) -> anyhow::Result<PathBuf> {
    let stamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let base = destination_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| run_dir.join("moved_rejects"));
    let name = if destination_root.is_some() {
        format!("Burst Rejects {stamp}")
    } else {
        stamp
    };
    unique_directory(&base.join(name))
}

fn unique_directory(path: &Path) -> anyhow::Result<PathBuf> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("invalid destination"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Burst Rejects");
    for index in 2.. {
        let candidate = parent.join(format!("{name} {index}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    unreachable!()
}

fn validate_destination(source_root: &Path, destination: &Path) -> anyhow::Result<()> {
    let source_root = lexical_absolute(source_root)?;
    let destination = lexical_absolute(destination)?;
    if destination.starts_with(&source_root) {
        bail!(
            "move destination must be outside the source folder or mounted card ({})",
            source_root.display()
        );
    }
    let mut temp_roots = vec![std::env::temp_dir()];
    #[cfg(unix)]
    temp_roots.extend([
        PathBuf::from("/tmp"),
        PathBuf::from("/private/tmp"),
        PathBuf::from("/var/tmp"),
    ]);
    for temp in temp_roots {
        let temp = lexical_absolute(&temp)?;
        if destination.starts_with(&temp) {
            bail!(
                "move destination cannot be temporary storage ({})",
                temp.display()
            );
        }
    }
    Ok(())
}

fn lexical_absolute(path: &Path) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    Ok(normalized)
}

fn unique_target(path: &Path, reserved: &mut HashSet<PathBuf>) -> PathBuf {
    if !path.exists() && reserved.insert(path.to_path_buf()) {
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
        if !candidate.exists() && reserved.insert(candidate.clone()) {
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

fn transfer_verified(plans: &[TransferPlan], create_target_parents: bool) -> anyhow::Result<()> {
    let mut copied = Vec::new();
    for plan in plans {
        if !plan.source.is_file() {
            cleanup_files(&copied);
            bail!("source file is unavailable: {}", plan.source.display());
        }
        let parent = plan
            .target
            .parent()
            .ok_or_else(|| anyhow!("target has no parent: {}", plan.target.display()))?;
        if create_target_parents {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating destination folder {}", parent.display()))?;
        } else if !parent.is_dir() {
            cleanup_files(&copied);
            bail!("original folder is unavailable: {}", parent.display());
        }
        if plan.target.exists() {
            cleanup_files(&copied);
            bail!("target already exists: {}", plan.target.display());
        }
        if let Err(error) = copy_verified(plan) {
            cleanup_files(&copied);
            return Err(error);
        }
        copied.push(plan.target.clone());
    }

    let mut removed = Vec::new();
    for plan in plans {
        if let Err(error) = fs::remove_file(&plan.source) {
            let rollback_error = rollback_partial_transfer(plans, &removed, &copied).err();
            let message = rollback_error.map_or_else(
                || error.to_string(),
                |rollback| format!("{error}; rollback also failed: {rollback}"),
            );
            bail!("removing {} failed: {message}", plan.source.display());
        }
        removed.push(plan.source.clone());
    }
    Ok(())
}

fn copy_verified(plan: &TransferPlan) -> anyhow::Result<()> {
    let actual_source_size = fs::metadata(&plan.source)?.len();
    if actual_source_size != plan.size {
        bail!(
            "source size changed before transfer: expected {} bytes, found {} bytes",
            plan.size,
            actual_source_size
        );
    }
    fs::copy(&plan.source, &plan.target).with_context(|| {
        format!(
            "copying {} to {}",
            plan.source.display(),
            plan.target.display()
        )
    })?;
    let target_len = fs::metadata(&plan.target)?.len();
    if target_len != plan.size {
        let _ = fs::remove_file(&plan.target);
        bail!(
            "copied size mismatch: source {} bytes, target {} bytes",
            plan.size,
            target_len
        );
    }
    Ok(())
}

fn rollback_partial_transfer(
    plans: &[TransferPlan],
    removed_sources: &[PathBuf],
    copied_targets: &[PathBuf],
) -> anyhow::Result<()> {
    for source in removed_sources {
        let plan = plans
            .iter()
            .find(|plan| &plan.source == source)
            .ok_or_else(|| anyhow!("missing rollback plan for {}", source.display()))?;
        fs::copy(&plan.target, &plan.source).with_context(|| {
            format!(
                "restoring {} after a failed transfer",
                plan.source.display()
            )
        })?;
        if fs::metadata(&plan.source)?.len() != plan.size {
            bail!("rollback size mismatch for {}", plan.source.display());
        }
    }
    cleanup_files(copied_targets);
    Ok(())
}

fn rollback_completed_transfer(plans: &[TransferPlan]) -> anyhow::Result<()> {
    let reverse: Vec<_> = plans
        .iter()
        .map(|plan| TransferPlan {
            source: plan.target.clone(),
            target: plan.source.clone(),
            size: plan.size,
        })
        .collect();
    transfer_verified(&reverse, false)
}

fn cleanup_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn write_move_state(run_dir: &Path, state: &MoveState) -> anyhow::Result<()> {
    fs::create_dir_all(run_dir)?;
    let path = run_dir.join(MOVE_STATE_FILE);
    let mut temp = NamedTempFile::new_in(run_dir)?;
    serde_json::to_writer_pretty(temp.as_file_mut(), state)?;
    temp.as_file_mut().write_all(b"\n")?;
    temp.as_file().sync_all()?;
    #[cfg(target_os = "windows")]
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("replacing {}", path.display()))?;
    }
    temp.persist(&path)
        .map_err(|error| error.error)
        .with_context(|| format!("persisting {}", path.display()))?;
    Ok(())
}

fn write_operation_report<T: Serialize>(
    run_dir: &Path,
    operation: &str,
    response: &T,
) -> anyhow::Result<()> {
    let report_dir = run_dir.join("move_reports");
    fs::create_dir_all(&report_dir)?;
    let stamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let report_path = report_dir.join(format!("{operation}_{stamp}.json"));
    let report = fs::File::create(&report_path)
        .with_context(|| format!("creating {}", report_path.display()))?;
    serde_json::to_writer_pretty(report, response)?;
    Ok(())
}

fn remove_empty_destination_parents(records: &[MoveRecord]) {
    for record in records {
        let Some(operation_root) = move_operation_root(&record.destination) else {
            continue;
        };
        let mut parent = record.destination.parent();
        while let Some(directory) = parent {
            if !directory.starts_with(&operation_root) || fs::remove_dir(directory).is_err() {
                break;
            }
            if directory == operation_root {
                break;
            }
            parent = directory.parent();
        }
    }
}

fn move_operation_root(destination: &Path) -> Option<PathBuf> {
    destination.ancestors().skip(1).find_map(|directory| {
        let name = directory.file_name()?.to_string_lossy();
        let is_custom_operation = name.starts_with("Burst Rejects ");
        let is_run_operation = directory
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|parent| parent == "moved_rejects");
        (is_custom_operation || is_run_operation).then(|| directory.to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{TransferPlan, move_operation_root, move_rejects, move_target, transfer_verified};

    #[test]
    fn move_requires_confirmation_before_reading_a_run() {
        let error = move_rejects(std::path::Path::new("missing-run"), None, false).unwrap_err();
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

    #[test]
    fn grouped_transfer_round_trips_without_data_loss() {
        let temp = tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        let jpeg = source_dir.join("frame.jpg");
        let raw = source_dir.join("frame.raw");
        fs::write(&jpeg, b"jpeg bytes").unwrap();
        fs::write(&raw, b"raw bytes").unwrap();
        let plans = vec![
            TransferPlan {
                source: jpeg.clone(),
                target: target_dir.join("frame.jpg"),
                size: 10,
            },
            TransferPlan {
                source: raw.clone(),
                target: target_dir.join("frame.raw"),
                size: 9,
            },
        ];

        transfer_verified(&plans, true).unwrap();
        assert!(!jpeg.exists());
        assert!(!raw.exists());
        let reverse = plans
            .iter()
            .map(|plan| TransferPlan {
                source: plan.target.clone(),
                target: plan.source.clone(),
                size: plan.size,
            })
            .collect::<Vec<_>>();
        transfer_verified(&reverse, false).unwrap();
        assert_eq!(fs::read(jpeg).unwrap(), b"jpeg bytes");
        assert_eq!(fs::read(raw).unwrap(), b"raw bytes");
    }

    #[test]
    fn grouped_transfer_does_not_move_a_partial_asset() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("source.jpg");
        let missing = temp.path().join("missing.raw");
        let target = temp.path().join("destination/source.jpg");
        fs::write(&source, b"source").unwrap();
        let plans = vec![
            TransferPlan {
                source: source.clone(),
                target: target.clone(),
                size: 6,
            },
            TransferPlan {
                source: missing,
                target: temp.path().join("destination/missing.raw"),
                size: 4,
            },
        ];

        assert!(transfer_verified(&plans, true).is_err());
        assert_eq!(fs::read(source).unwrap(), b"source");
        assert!(!target.exists());
    }

    #[test]
    fn selected_restore_filter_is_asset_based() {
        let selected = HashSet::from(["asset-a".to_string()]);
        assert!(selected.contains("asset-a"));
        assert!(!selected.contains("asset-b"));
    }

    #[test]
    fn custom_move_cleanup_stops_at_the_timestamped_subfolder() {
        let path = Path::new("chosen/Burst Rejects 20260712_020824/DCIM/frame.jpg");
        assert_eq!(
            move_operation_root(path),
            Some(Path::new("chosen/Burst Rejects 20260712_020824").to_path_buf())
        );
        assert_eq!(
            move_operation_root(Path::new(
                "run/moved_rejects/20260712_020824/DCIM/frame.jpg"
            )),
            Some(Path::new("run/moved_rejects/20260712_020824").to_path_buf())
        );
    }
}
