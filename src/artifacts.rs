use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use chrono::Utc;
use serde::Serialize;

use crate::types::{
    AssetRecord, BurstCluster, BurstSequence, ReviewDecision, ReviewState, RunManifest,
    SuggestedAction, UserDecision,
};

const MANIFEST: &str = "manifest.json";
const REVIEW_STATE: &str = "review_state.json";

pub fn write_manifest(run_dir: &Path, manifest: &RunManifest) -> anyhow::Result<()> {
    fs::create_dir_all(run_dir)?;
    let path = run_dir.join(MANIFEST);
    let file = File::create(&path)?;
    serde_json::to_writer_pretty(file, manifest)?;
    Ok(())
}

pub fn read_manifest(run_dir: &Path) -> anyhow::Result<RunManifest> {
    let path = run_dir.join(MANIFEST);
    let file = File::open(&path).with_context(|| format!("opening {}", path.display()))?;
    Ok(serde_json::from_reader(file)?)
}

pub fn ensure_review_state(run_dir: &Path, manifest: &RunManifest) -> anyhow::Result<ReviewState> {
    let path = run_dir.join(REVIEW_STATE);
    if path.exists() {
        let state = read_review_state(run_dir)?;
        if state.run_created_at == manifest.created_at {
            return Ok(state);
        }
    }
    let now = Utc::now().to_rfc3339();
    let state = ReviewState {
        run_created_at: manifest.created_at.clone(),
        updated_at: now,
        decisions: Vec::new(),
    };
    write_review_state(run_dir, &state)?;
    Ok(state)
}

pub fn read_review_state(run_dir: &Path) -> anyhow::Result<ReviewState> {
    let path = run_dir.join(REVIEW_STATE);
    let file = File::open(&path).with_context(|| format!("opening {}", path.display()))?;
    Ok(serde_json::from_reader(file)?)
}

pub fn write_review_state(run_dir: &Path, state: &ReviewState) -> anyhow::Result<()> {
    fs::create_dir_all(run_dir)?;
    let path = run_dir.join(REVIEW_STATE);
    let file = File::create(&path)?;
    serde_json::to_writer_pretty(file, state)?;
    Ok(())
}

pub fn export_reviewed_artifacts(run_dir: &Path) -> anyhow::Result<()> {
    let manifest = read_manifest(run_dir)?;
    let review = ensure_review_state(run_dir, &manifest)?;
    let decisions: HashMap<String, &ReviewDecision> = review
        .decisions
        .iter()
        .map(|decision| (decision.asset_id.clone(), decision))
        .collect();

    write_assets_csv(run_dir, &manifest, &decisions)?;
    write_bursts_csv(run_dir, &manifest)?;
    write_clusters_csv(run_dir, &manifest)?;
    write_move_script(run_dir, &manifest, &decisions)?;
    write_review_launcher(run_dir)?;
    Ok(())
}

pub fn upsert_decision(
    run_dir: &Path,
    asset_id: String,
    decision: Option<UserDecision>,
    note: Option<String>,
) -> anyhow::Result<ReviewState> {
    let mut state = read_review_state(run_dir)?;
    let now = Utc::now().to_rfc3339();
    state.updated_at = now.clone();
    if let Some(existing) = state
        .decisions
        .iter_mut()
        .find(|entry| entry.asset_id == asset_id)
    {
        existing.decision = decision;
        existing.note = note;
        existing.updated_at = now;
    } else {
        state.decisions.push(ReviewDecision {
            asset_id,
            decision,
            note,
            updated_at: now,
        });
    }
    state.decisions.retain(|entry| {
        entry.decision.is_some() || entry.note.as_deref().is_some_and(|note| !note.is_empty())
    });
    write_review_state(run_dir, &state)?;
    export_reviewed_artifacts(run_dir)?;
    read_review_state(run_dir)
}

#[derive(Serialize)]
struct AssetCsvRow {
    asset_id: String,
    final_action: String,
    user_decision: String,
    suggested_action: String,
    burst_id: usize,
    cluster_id: usize,
    rank: usize,
    score: f64,
    reason: String,
    representative: String,
    files: String,
    sidecars: String,
    decoder: String,
    feature_backend: String,
    detector_backend: String,
    width: u32,
    height: u32,
    sharpness: f64,
    subject_sharpness: f64,
    completeness: f64,
    contrast: f64,
    exposure_score: f64,
    object_confidence: f64,
    nearest_visual_distance: f64,
    duplicate_confidence: f64,
    note: String,
}

fn write_assets_csv(
    run_dir: &Path,
    manifest: &RunManifest,
    decisions: &HashMap<String, &ReviewDecision>,
) -> anyhow::Result<()> {
    let mut all = csv::Writer::from_path(run_dir.join("all_assets.csv"))?;
    let mut keepers = csv::Writer::from_path(run_dir.join("keepers.csv"))?;
    let mut rejects = csv::Writer::from_path(run_dir.join("rejects.csv"))?;
    let mut review = csv::Writer::from_path(run_dir.join("review.csv"))?;

    for asset in &manifest.assets {
        let row = csv_row(asset, decisions.get(&asset.id).copied());
        all.serialize(&row)?;
        match row.final_action.as_str() {
            "keep" => keepers.serialize(&row)?,
            "reject" => rejects.serialize(&row)?,
            _ => review.serialize(&row)?,
        }
    }
    all.flush()?;
    keepers.flush()?;
    rejects.flush()?;
    review.flush()?;
    Ok(())
}

fn csv_row(asset: &AssetRecord, decision: Option<&ReviewDecision>) -> AssetCsvRow {
    let user_decision = decision
        .and_then(|entry| entry.decision)
        .map(UserDecision::as_str)
        .unwrap_or("")
        .to_string();
    let final_action = decision
        .and_then(|entry| entry.decision)
        .map(|decision| decision.as_str().to_string())
        .unwrap_or_else(|| suggested_action_name(asset.suggestion.action).to_string());
    AssetCsvRow {
        asset_id: asset.id.clone(),
        final_action,
        user_decision,
        suggested_action: suggested_action_name(asset.suggestion.action).to_string(),
        burst_id: asset.burst_id,
        cluster_id: asset.cluster_id,
        rank: asset.suggestion.rank,
        score: asset.suggestion.score,
        reason: asset.suggestion.reason.clone(),
        representative: asset.representative.rel_path.clone(),
        files: asset
            .files
            .iter()
            .map(|file| file.rel_path.as_str())
            .collect::<Vec<_>>()
            .join("|"),
        sidecars: asset
            .sidecars
            .iter()
            .map(|file| file.rel_path.as_str())
            .collect::<Vec<_>>()
            .join("|"),
        decoder: asset.decoder.clone(),
        feature_backend: asset.feature_backend.clone(),
        detector_backend: asset
            .detector
            .as_ref()
            .map(|detector| detector.backend.clone())
            .unwrap_or_default(),
        width: asset.width,
        height: asset.height,
        sharpness: asset.metrics.sharpness,
        subject_sharpness: asset.metrics.subject_sharpness,
        completeness: asset.metrics.completeness,
        contrast: asset.metrics.contrast,
        exposure_score: asset.metrics.exposure_score,
        object_confidence: asset.metrics.object_confidence,
        nearest_visual_distance: asset.similarity.nearest_distance,
        duplicate_confidence: asset.similarity.duplicate_confidence,
        note: decision
            .and_then(|entry| entry.note.clone())
            .unwrap_or_default(),
    }
}

fn write_clusters_csv(run_dir: &Path, manifest: &RunManifest) -> anyhow::Result<()> {
    let mut writer = csv::Writer::from_path(run_dir.join("clusters.csv"))?;
    for cluster in &manifest.clusters {
        writer.serialize(ClusterCsvRow::from(cluster))?;
    }
    writer.flush()?;
    Ok(())
}

fn write_bursts_csv(run_dir: &Path, manifest: &RunManifest) -> anyhow::Result<()> {
    let mut writer = csv::Writer::from_path(run_dir.join("bursts.csv"))?;
    for burst in &manifest.bursts {
        writer.serialize(BurstCsvRow::from(burst))?;
    }
    writer.flush()?;
    Ok(())
}

#[derive(Serialize)]
struct BurstCsvRow {
    id: usize,
    asset_ids: String,
    cluster_ids: String,
    directory: String,
    prefix: String,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
}

impl From<&BurstSequence> for BurstCsvRow {
    fn from(value: &BurstSequence) -> Self {
        Self {
            id: value.id,
            asset_ids: value.asset_ids.join("|"),
            cluster_ids: value
                .cluster_ids
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join("|"),
            directory: value.directory.clone(),
            prefix: value.prefix.clone(),
            start_ms: value.start_ms,
            end_ms: value.end_ms,
        }
    }
}

#[derive(Serialize)]
struct ClusterCsvRow {
    id: usize,
    burst_id: usize,
    asset_ids: String,
    directory: String,
    prefix: String,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    keep_count: usize,
    best_asset_id: String,
    similarity_confidence: f64,
    max_distance: f64,
}

impl From<&BurstCluster> for ClusterCsvRow {
    fn from(value: &BurstCluster) -> Self {
        Self {
            id: value.id,
            burst_id: value.burst_id,
            asset_ids: value.asset_ids.join("|"),
            directory: value.directory.clone(),
            prefix: value.prefix.clone(),
            start_ms: value.start_ms,
            end_ms: value.end_ms,
            keep_count: value.keep_count,
            best_asset_id: value.best_asset_id.clone().unwrap_or_default(),
            similarity_confidence: value.similarity_confidence,
            max_distance: value.max_distance,
        }
    }
}

fn write_move_script(
    run_dir: &Path,
    manifest: &RunManifest,
    decisions: &HashMap<String, &ReviewDecision>,
) -> anyhow::Result<()> {
    let mut script = String::from(
        "#!/usr/bin/env bash\nset -euo pipefail\n\n# Helper generated by burst-frame-deduplicator.\n# It moves final reject assets into a local moved_rejects_from_script folder next to this run.\n# It never permanently deletes files; moved files remain in that local folder until you remove them yourself.\n\n",
    );
    let local_run_dir = run_dir
        .canonicalize()
        .unwrap_or_else(|_| run_dir.to_path_buf());
    let reject_root = local_run_dir.join("moved_rejects_from_script");
    for asset in &manifest.assets {
        if final_decision(asset, decisions.get(&asset.id).copied()) != UserDecision::Reject {
            continue;
        }
        for file in asset.files.iter().chain(asset.sidecars.iter()) {
            let source = &file.path;
            let target = reject_root.join(&file.rel_path);
            let target_dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
            script.push_str(&format!("mkdir -p {}\n", shell_quote(&target_dir)));
            script.push_str(&format!(
                "if [ -e {} ]; then mv -n {} {}; fi\n",
                shell_quote(source),
                shell_quote(source),
                shell_quote(&target)
            ));
        }
    }
    let path = run_dir.join("move_rejects.sh");
    let mut file = File::create(&path)?;
    file.write_all(script.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;
    }
    Ok(())
}

fn write_review_launcher(run_dir: &Path) -> anyhow::Result<()> {
    let html = r#"<!doctype html>
<meta charset="utf-8">
<title>Burst Frame Deduplicator Review</title>
<body style="font-family: system-ui, sans-serif; margin: 2rem; max-width: 760px;">
  <h1>Burst Frame Deduplicator Review</h1>
  <p>Start the local review app for this run directory:</p>
  <pre>cargo run -- serve --run .</pre>
  <p>Or from the project root, pass this run directory to <code>serve --run</code>.</p>
</body>
"#;
    fs::write(run_dir.join("review.html"), html)?;
    Ok(())
}

fn final_decision(asset: &AssetRecord, decision: Option<&ReviewDecision>) -> UserDecision {
    if let Some(user_decision) = decision.and_then(|entry| entry.decision) {
        return user_decision;
    }
    match asset.suggestion.action {
        SuggestedAction::Keep => UserDecision::Keep,
        SuggestedAction::Reject => UserDecision::Reject,
        SuggestedAction::Review | SuggestedAction::Error => UserDecision::Review,
    }
}

fn suggested_action_name(action: SuggestedAction) -> &'static str {
    match action {
        SuggestedAction::Keep => "keep",
        SuggestedAction::Reject => "reject",
        SuggestedAction::Review => "review",
        SuggestedAction::Error => "error",
    }
}

fn shell_quote(path: &Path) -> String {
    let raw = path.to_string_lossy();
    format!("'{}'", raw.replace('\'', "'\\''"))
}
