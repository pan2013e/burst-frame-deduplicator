use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};

use crate::artifacts::{ensure_review_state, read_manifest};
use crate::assets::{AssetInput, discover_assets_with_progress};
use crate::operations::{
    MoveFailure, MoveRecord, MoveStatus, RestoreResponse, SourceSet, TransferPlan,
    active_asset_ids, move_status, move_target, read_move_state, remove_empty_destination_parents,
    rollback_completed_transfer, transfer_verified, unique_directory, unique_target,
    validate_destination, write_move_state, write_operation_report,
};
use crate::types::{AssetRecord, FileKind, UserDecision};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpartFile {
    pub path: PathBuf,
    pub rel_path: String,
    pub kind: FileKind,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpartMatch {
    pub asset_id: String,
    pub stem: String,
    pub expected_kind: FileKind,
    pub files: Vec<CounterpartFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpartPlanResponse {
    pub card_root: PathBuf,
    pub expected_assets: usize,
    pub matched_assets: usize,
    pub matched_files: usize,
    pub already_applied_assets: usize,
    pub skipped_paired_assets: usize,
    pub matches: Vec<CounterpartMatch>,
    pub unmatched_stems: Vec<String>,
    pub ambiguous_stems: Vec<String>,
    pub conflicting_run_stems: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpartMoveResponse {
    pub destination: PathBuf,
    pub moved_files: usize,
    pub moved_assets: usize,
    pub already_applied_assets: usize,
    pub moved_asset_ids: Vec<String>,
    pub source_available: bool,
    pub failed_files: Vec<MoveFailure>,
    pub message: Option<String>,
    pub plan: CounterpartPlanResponse,
    pub status: MoveStatus,
}

#[derive(Debug, Clone)]
struct ExpectedCounterpart {
    asset_id: String,
    stem: String,
    normalized_stem: String,
    kind: FileKind,
}

pub fn plan_counterparts(
    run_dir: &Path,
    card_root: &Path,
) -> anyhow::Result<CounterpartPlanResponse> {
    let manifest = read_manifest(run_dir)?;
    let review = ensure_review_state(run_dir, &manifest)?;
    if !card_root.is_dir() {
        bail!(
            "The counterpart card folder is unavailable: {}. Reconnect the card or select its photo folder and try again.",
            card_root.display()
        );
    }

    let mut primary_stem_counts = BTreeMap::<String, usize>::new();
    for asset in &manifest.assets {
        *primary_stem_counts
            .entry(normalize_stem(&asset.stem))
            .or_default() += 1;
    }

    let mut expected = Vec::new();
    let mut skipped_paired_assets = 0;
    for asset in &manifest.assets {
        if crate::operations::final_action_for_asset(asset, &review) != UserDecision::Reject {
            continue;
        }
        let Some(kind) = expected_counterpart_kind(asset) else {
            skipped_paired_assets += 1;
            continue;
        };
        expected.push(ExpectedCounterpart {
            asset_id: asset.id.clone(),
            stem: asset.stem.clone(),
            normalized_stem: normalize_stem(&asset.stem),
            kind,
        });
    }

    let state = read_move_state(run_dir)?;
    let active = active_asset_ids(&state, SourceSet::Counterpart);
    let discovered = discover_assets_with_progress(card_root, |_, _| {})?;
    build_plan(
        card_root,
        expected,
        skipped_paired_assets,
        &primary_stem_counts,
        &active,
        &discovered,
    )
}

pub fn apply_counterparts(
    run_dir: &Path,
    card_root: &Path,
    destination_root: Option<&Path>,
    confirmed: bool,
) -> anyhow::Result<CounterpartMoveResponse> {
    if !confirmed {
        bail!("counterpart move requires explicit confirmation");
    }
    let plan = plan_counterparts(run_dir, card_root)?;
    let destination = counterpart_destination(run_dir, destination_root)?;
    validate_destination(card_root, &destination)?;
    let mut state = read_move_state(run_dir)?;
    let mut response = CounterpartMoveResponse {
        destination: destination.clone(),
        moved_files: 0,
        moved_assets: 0,
        already_applied_assets: plan.already_applied_assets,
        moved_asset_ids: Vec::new(),
        source_available: true,
        failed_files: Vec::new(),
        message: None,
        plan,
        status: move_status(&state),
    };
    let mut reserved_targets = HashSet::new();

    for counterpart in &response.plan.matches {
        let missing: Vec<_> = counterpart
            .files
            .iter()
            .filter(|file| !file.path.is_file())
            .map(|file| file.path.display().to_string())
            .collect();
        if !missing.is_empty() {
            response.failed_files.push(MoveFailure {
                source: counterpart.stem.clone(),
                error: format!(
                    "Counterpart asset was not moved because {} grouped file(s) are unavailable",
                    missing.len()
                ),
            });
            continue;
        }

        let plans: Vec<_> = counterpart
            .files
            .iter()
            .map(|file| {
                let desired = move_target(&destination, &file.rel_path, &file.path);
                TransferPlan {
                    source: file.path.clone(),
                    target: unique_target(&desired, &mut reserved_targets),
                    size: file.size,
                }
            })
            .collect();
        if let Err(error) = transfer_verified(&plans, true) {
            response.failed_files.push(MoveFailure {
                source: counterpart.stem.clone(),
                error: error.to_string(),
            });
            continue;
        }

        let moved_at = Utc::now().to_rfc3339();
        let old_len = state.records.len();
        state.records.extend(
            plans
                .iter()
                .zip(&counterpart.files)
                .map(|(transfer, file)| MoveRecord {
                    asset_id: counterpart.asset_id.clone(),
                    source_set: SourceSet::Counterpart,
                    source_root: Some(card_root.to_path_buf()),
                    source_rel_path: Some(file.rel_path.clone()),
                    source: transfer.source.clone(),
                    destination: transfer.target.clone(),
                    size: transfer.size,
                    moved_at: moved_at.clone(),
                    restored_at: None,
                }),
        );
        state.updated_at = moved_at;
        if let Err(error) = write_move_state(run_dir, &state) {
            state.records.truncate(old_len);
            rollback_completed_transfer(&plans).with_context(|| {
                format!(
                    "counterpart move journal failed ({error}); restoring the card asset also failed"
                )
            })?;
            return Err(error.context("writing counterpart move journal; card files were restored"));
        }

        response.moved_files += plans.len();
        response.moved_assets += 1;
        response.moved_asset_ids.push(counterpart.asset_id.clone());
    }

    response.status = move_status(&state);
    if response.moved_assets == 0 && response.failed_files.is_empty() {
        response.message = Some(if response.already_applied_assets > 0 {
            "All matched counterpart assets were already moved.".to_string()
        } else if !response.plan.ambiguous_stems.is_empty()
            || !response.plan.conflicting_run_stems.is_empty()
        {
            "No counterpart files were moved because all remaining matches are ambiguous."
                .to_string()
        } else {
            "There are no matched counterpart assets to move.".to_string()
        });
    }
    write_operation_report(run_dir, "counterpart_move", &response)?;
    Ok(response)
}

pub fn restore_counterparts(
    run_dir: &Path,
    card_root: &Path,
    confirmed: bool,
) -> anyhow::Result<RestoreResponse> {
    if !confirmed {
        bail!("counterpart restore requires explicit confirmation");
    }
    let source_available = card_root.is_dir();
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
            "The counterpart card folder is unavailable: {}. Reconnect the card before restoring files.",
            card_root.display()
        ));
        return Ok(response);
    }

    let mut by_asset = BTreeMap::<String, Vec<usize>>::new();
    for (index, record) in state.records.iter().enumerate() {
        if record.restored_at.is_none() && record.source_set == SourceSet::Counterpart {
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
        let mut plans = Vec::with_capacity(records.len());
        let mut conflict = None;
        for record in &records {
            if !record.destination.is_file() {
                response
                    .missing_files
                    .push(record.destination.display().to_string());
                conflict = Some("A moved counterpart file is unavailable.".to_string());
                break;
            }
            let Some(relative) = record.source_rel_path.as_deref() else {
                conflict = Some("The move record has no counterpart relative path.".to_string());
                break;
            };
            let target = move_target(card_root, relative, &record.source);
            let Some(parent) = target.parent() else {
                conflict = Some(format!("Invalid counterpart path: {}", target.display()));
                break;
            };
            if !parent.is_dir() {
                conflict = Some(format!(
                    "The original counterpart folder is unavailable: {}",
                    parent.display()
                ));
                break;
            }
            if target.exists() {
                conflict = Some(format!(
                    "The counterpart card already contains a file at: {}",
                    target.display()
                ));
                break;
            }
            plans.push(TransferPlan {
                source: record.destination.clone(),
                target,
                size: record.size,
            });
        }
        if let Some(error) = conflict {
            response.failed_files.push(MoveFailure {
                source: records
                    .first()
                    .map(|record| record.destination.display().to_string())
                    .unwrap_or_else(|| asset_id.clone()),
                error,
            });
            continue;
        }
        if let Err(error) = transfer_verified(&plans, false) {
            response.failed_files.push(MoveFailure {
                source: records[0].destination.display().to_string(),
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
                format!(
                    "counterpart restore journal failed ({error}); returning files to the move destination also failed"
                )
            })?;
            return Err(error.context(
                "writing counterpart restore journal; files were returned to the move destination",
            ));
        }

        response.restored_files += plans.len();
        response.restored_assets += 1;
        response.restored_asset_ids.push(asset_id);
        remove_empty_destination_parents(&records);
    }

    response.status = move_status(&state);
    if response.restored_assets == 0 && response.failed_files.is_empty() {
        response.message = Some("There are no moved counterpart assets to restore.".to_string());
    }
    write_operation_report(run_dir, "counterpart_restore", &response)?;
    Ok(response)
}

fn build_plan(
    card_root: &Path,
    expected: Vec<ExpectedCounterpart>,
    skipped_paired_assets: usize,
    primary_stem_counts: &BTreeMap<String, usize>,
    active: &HashSet<String>,
    discovered: &[AssetInput],
) -> anyhow::Result<CounterpartPlanResponse> {
    let expected_assets = expected.len();
    let mut candidates = BTreeMap::<String, Vec<&AssetInput>>::new();
    for asset in discovered {
        candidates
            .entry(normalize_stem(&asset.stem))
            .or_default()
            .push(asset);
    }

    let mut matches = Vec::new();
    let mut already_applied_assets = 0;
    let mut unmatched = BTreeSet::new();
    let mut ambiguous = BTreeSet::new();
    let mut conflicting = BTreeSet::new();
    for expected_asset in expected {
        if active.contains(&expected_asset.asset_id) {
            already_applied_assets += 1;
            continue;
        }
        if primary_stem_counts
            .get(&expected_asset.normalized_stem)
            .copied()
            .unwrap_or_default()
            > 1
        {
            conflicting.insert(expected_asset.stem);
            continue;
        }
        let relevant: Vec<_> = candidates
            .get(&expected_asset.normalized_stem)
            .into_iter()
            .flatten()
            .filter(|candidate| {
                candidate
                    .files
                    .iter()
                    .any(|file| file.kind == expected_asset.kind)
            })
            .copied()
            .collect();
        if relevant.is_empty() {
            unmatched.insert(expected_asset.stem);
            continue;
        }
        if relevant.len() > 1 {
            ambiguous.insert(expected_asset.stem);
            continue;
        }

        let candidate = relevant[0];
        // Once the opposite format qualifies a candidate, keep the candidate's full
        // same-stem RAW/JPEG group and sidecars transactional.
        let entries = candidate.files.iter().chain(candidate.sidecars.iter());
        let files = entries
            .map(|file| {
                Ok(CounterpartFile {
                    path: file.path.clone(),
                    rel_path: file.rel_path.clone(),
                    kind: file.kind,
                    size: fs::metadata(&file.path)
                        .with_context(|| format!("reading {}", file.path.display()))?
                        .len(),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        matches.push(CounterpartMatch {
            asset_id: expected_asset.asset_id,
            stem: expected_asset.stem,
            expected_kind: expected_asset.kind,
            files,
        });
    }

    matches.sort_by_key(|item| normalize_stem(&item.stem));
    let matched_files = matches.iter().map(|item| item.files.len()).sum();
    Ok(CounterpartPlanResponse {
        card_root: card_root.to_path_buf(),
        expected_assets,
        matched_assets: matches.len(),
        matched_files,
        already_applied_assets,
        skipped_paired_assets,
        matches,
        unmatched_stems: unmatched.into_iter().collect(),
        ambiguous_stems: ambiguous.into_iter().collect(),
        conflicting_run_stems: conflicting.into_iter().collect(),
    })
}

fn expected_counterpart_kind(asset: &AssetRecord) -> Option<FileKind> {
    let has_raw = asset.files.iter().any(|file| file.kind == FileKind::Raw);
    let has_compressed = asset
        .files
        .iter()
        .any(|file| file.kind == FileKind::Compressed);
    match (has_raw, has_compressed) {
        (true, false) => Some(FileKind::Compressed),
        (false, true) => Some(FileKind::Raw),
        _ => None,
    }
}

fn normalize_stem(stem: &str) -> String {
    stem.to_lowercase()
}

fn counterpart_destination(
    run_dir: &Path,
    destination_root: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    let stamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let base = destination_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| run_dir.join("moved_counterparts"));
    let name = if destination_root.is_some() {
        format!("Burst Counterparts {stamp}")
    } else {
        stamp
    };
    unique_directory(&base.join(name))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashSet};
    use std::fs;

    use tempfile::tempdir;

    use super::{ExpectedCounterpart, apply_counterparts, build_plan, restore_counterparts};
    use crate::artifacts::write_manifest;
    use crate::assets::discover_assets_with_progress;
    use crate::operations::{SourceSet, read_move_state};
    use crate::types::{
        AccelerationPreference, AccelerationReport, AssetRecord, AssetTimings, DecoderReport,
        DetectorPreference, DetectorReport, FileEntry, FileKind, PhotoMetadata, RunManifest,
        SimilarityMetrics, SuggestedAction, Suggestion,
    };

    fn expected(id: &str, stem: &str, kind: FileKind) -> ExpectedCounterpart {
        ExpectedCounterpart {
            asset_id: id.to_string(),
            stem: stem.to_string(),
            normalized_stem: stem.to_lowercase(),
            kind,
        }
    }

    #[test]
    fn matching_ignores_directories_and_ascii_case() {
        let temp = tempdir().unwrap();
        let card = temp.path().join("card");
        fs::create_dir_all(card.join("OTHER/DCIM")).unwrap();
        fs::write(card.join("OTHER/DCIM/frame_0042.JPG"), b"jpeg").unwrap();
        let discovered = discover_assets_with_progress(&card, |_, _| {}).unwrap();
        let counts = BTreeMap::from([("frame_0042".to_string(), 1)]);
        let plan = build_plan(
            &card,
            vec![expected("asset", "FRAME_0042", FileKind::Compressed)],
            0,
            &counts,
            &HashSet::new(),
            &discovered,
        )
        .unwrap();

        assert_eq!(plan.matched_assets, 1);
        assert_eq!(
            plan.matches[0].files[0].rel_path,
            "OTHER/DCIM/frame_0042.JPG"
        );
    }

    #[test]
    fn duplicate_counterpart_stems_are_ambiguous() {
        let temp = tempdir().unwrap();
        let card = temp.path().join("card");
        fs::create_dir_all(card.join("A")).unwrap();
        fs::create_dir_all(card.join("B")).unwrap();
        fs::write(card.join("A/FRAME.RW2"), b"raw-a").unwrap();
        fs::write(card.join("B/frame.rw2"), b"raw-b").unwrap();
        let discovered = discover_assets_with_progress(&card, |_, _| {}).unwrap();
        let counts = BTreeMap::from([("frame".to_string(), 1)]);
        let plan = build_plan(
            &card,
            vec![expected("asset", "frame", FileKind::Raw)],
            0,
            &counts,
            &HashSet::new(),
            &discovered,
        )
        .unwrap();

        assert!(plan.matches.is_empty());
        assert_eq!(plan.ambiguous_stems, ["frame"]);
    }

    #[test]
    fn duplicate_primary_stems_are_never_guessed() {
        let temp = tempdir().unwrap();
        let card = temp.path().join("card");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("frame.jpg"), b"jpeg").unwrap();
        let discovered = discover_assets_with_progress(&card, |_, _| {}).unwrap();
        let counts = BTreeMap::from([("frame".to_string(), 2)]);
        let plan = build_plan(
            &card,
            vec![expected("asset", "frame", FileKind::Compressed)],
            0,
            &counts,
            &HashSet::new(),
            &discovered,
        )
        .unwrap();

        assert!(plan.matches.is_empty());
        assert_eq!(plan.conflicting_run_stems, ["frame"]);
    }

    #[test]
    fn matching_requires_the_opposite_file_kind() {
        let temp = tempdir().unwrap();
        let card = temp.path().join("card");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("frame.jpg"), b"jpeg").unwrap();
        let discovered = discover_assets_with_progress(&card, |_, _| {}).unwrap();
        let counts = BTreeMap::from([("frame".to_string(), 1)]);
        let plan = build_plan(
            &card,
            vec![expected("asset", "frame", FileKind::Raw)],
            0,
            &counts,
            &HashSet::new(),
            &discovered,
        )
        .unwrap();

        assert!(plan.matches.is_empty());
        assert_eq!(plan.unmatched_stems, ["frame"]);
    }

    #[test]
    fn counterpart_move_round_trips_after_the_card_mount_path_changes() {
        let test_root = std::env::current_dir().unwrap().join("target");
        fs::create_dir_all(&test_root).unwrap();
        let temp = tempfile::Builder::new()
            .prefix("counterpart-round-trip-")
            .tempdir_in(test_root)
            .unwrap();
        let run = temp.path().join("run");
        let primary = temp.path().join("primary/DCIM");
        let card = temp.path().join("raw-card");
        fs::create_dir_all(&primary).unwrap();
        fs::create_dir_all(card.join("DIFFERENT/FOLDER")).unwrap();
        let primary_file = primary.join("frame_0042.jpg");
        let counterpart_file = card.join("DIFFERENT/FOLDER/FRAME_0042.RW2");
        fs::write(&primary_file, b"primary jpeg").unwrap();
        fs::write(&counterpart_file, b"counterpart raw").unwrap();
        write_manifest(&run, &test_manifest(&primary, &primary_file)).unwrap();

        let moved = apply_counterparts(&run, &card, None, true).unwrap();
        assert_eq!(moved.moved_assets, 1);
        assert!(moved.status.active_asset_ids.is_empty());
        assert_eq!(
            moved.status.active_counterpart_asset_ids,
            ["asset-frame-0042"]
        );
        assert!(!counterpart_file.exists());
        assert_eq!(fs::read(&primary_file).unwrap(), b"primary jpeg");
        let state = read_move_state(&run).unwrap();
        assert_eq!(state.records[0].source_set, SourceSet::Counterpart);
        assert_eq!(
            state.records[0].source_rel_path.as_deref(),
            Some("DIFFERENT/FOLDER/FRAME_0042.RW2")
        );

        let remounted = temp.path().join("new-mount-name");
        fs::rename(&card, &remounted).unwrap();
        let restored = restore_counterparts(&run, &remounted, true).unwrap();
        assert_eq!(restored.restored_assets, 1);
        assert_eq!(
            fs::read(remounted.join("DIFFERENT/FOLDER/FRAME_0042.RW2")).unwrap(),
            b"counterpart raw"
        );
    }

    fn test_manifest(primary: &std::path::Path, primary_file: &std::path::Path) -> RunManifest {
        let entry = FileEntry {
            path: primary_file.to_path_buf(),
            rel_path: "frame_0042.jpg".to_string(),
            kind: FileKind::Compressed,
            extension: "jpg".to_string(),
        };
        RunManifest {
            app_version: "test".to_string(),
            root: primary.to_path_buf(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            options: Default::default(),
            acceleration: AccelerationReport {
                requested: AccelerationPreference::Auto,
                selected: "cpu".to_string(),
                capabilities: Vec::new(),
                notes: Vec::new(),
            },
            detector: DetectorReport {
                requested: DetectorPreference::Auto,
                selected: "off".to_string(),
                capabilities: Vec::new(),
                notes: Vec::new(),
                model: None,
            },
            decoders: DecoderReport {
                native_compressed: true,
                scaled_jpeg: true,
                imagemagick: None,
                sips: None,
                raw_strategy: "test".to_string(),
            },
            benchmarks: Vec::new(),
            summary: Default::default(),
            bursts: Vec::new(),
            clusters: Vec::new(),
            assets: vec![AssetRecord {
                id: "asset-frame-0042".to_string(),
                representative: entry.clone(),
                files: vec![entry],
                sidecars: Vec::new(),
                directory: String::new(),
                stem: "frame_0042".to_string(),
                prefix: "frame_".to_string(),
                seq: Some(42),
                created_ms: None,
                modified_ms: None,
                capture_ms: None,
                capture_time_source: String::new(),
                width: 1,
                height: 1,
                decoder: "test".to_string(),
                feature_backend: "test".to_string(),
                metadata: PhotoMetadata::default(),
                metrics: Default::default(),
                detector: None,
                timings: AssetTimings::default(),
                burst_id: 0,
                cluster_id: 0,
                similarity: SimilarityMetrics::default(),
                suggestion: Suggestion {
                    action: SuggestedAction::Reject,
                    rank: 2,
                    score: 0.2,
                    reason: "test".to_string(),
                    explanations: Vec::new(),
                },
                thumb: None,
                error: None,
            }],
        }
    }
}
