use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

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
    read_manifest_with_progress(run_dir, |_, _| {})
}

pub fn read_manifest_with_progress(
    run_dir: &Path,
    mut on_progress: impl FnMut(u64, u64),
) -> anyhow::Result<RunManifest> {
    let path = run_dir.join(MANIFEST);
    let file = File::open(&path).with_context(|| format!("opening {}", path.display()))?;
    let total = file.metadata()?.len();
    on_progress(0, total);
    let reader = ProgressReader {
        inner: file,
        current: 0,
        total,
        on_progress,
    };
    Ok(serde_json::from_reader(BufReader::with_capacity(
        256 * 1024,
        reader,
    ))?)
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
    Ok(serde_json::from_reader(BufReader::new(file))?)
}

struct ProgressReader<R, F> {
    inner: R,
    current: u64,
    total: u64,
    on_progress: F,
}

impl<R, F> Read for ProgressReader<R, F>
where
    R: Read,
    F: FnMut(u64, u64),
{
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        if count > 0 {
            self.current = self.current.saturating_add(count as u64);
            (self.on_progress)(self.current.min(self.total), self.total);
        }
        Ok(count)
    }
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
    write_move_scripts(run_dir, &manifest, &decisions)?;
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

#[derive(Debug, Clone, Serialize)]
pub struct MoveScripts {
    pub destination: PathBuf,
    pub posix: String,
    pub powershell: String,
}

pub fn move_scripts_for_run(
    run_dir: &Path,
    destination: Option<&Path>,
) -> anyhow::Result<MoveScripts> {
    let manifest = read_manifest(run_dir)?;
    let review = ensure_review_state(run_dir, &manifest)?;
    let decisions: HashMap<String, &ReviewDecision> = review
        .decisions
        .iter()
        .map(|decision| (decision.asset_id.clone(), decision))
        .collect();
    Ok(build_move_scripts(
        run_dir,
        &manifest,
        &decisions,
        destination,
    ))
}

fn write_move_scripts(
    run_dir: &Path,
    manifest: &RunManifest,
    decisions: &HashMap<String, &ReviewDecision>,
) -> anyhow::Result<()> {
    let scripts = build_move_scripts(run_dir, manifest, decisions, None);
    let path = run_dir.join("move_rejects.sh");
    let mut file = File::create(&path)?;
    file.write_all(scripts.posix.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;
    }
    fs::write(run_dir.join("move_rejects.ps1"), scripts.powershell)?;
    Ok(())
}

fn build_move_scripts(
    run_dir: &Path,
    manifest: &RunManifest,
    decisions: &HashMap<String, &ReviewDecision>,
    destination: Option<&Path>,
) -> MoveScripts {
    let local_run_dir = run_dir
        .canonicalize()
        .unwrap_or_else(|_| run_dir.to_path_buf());
    let reject_root = destination
        .map(Path::to_path_buf)
        .unwrap_or_else(|| local_run_dir.join("moved_rejects_from_script"));
    let mut posix = format!(
        "#!/usr/bin/env bash\nset -euo pipefail\n\n# Generated by burst-frame-deduplicator.\n# Copies each grouped asset, verifies byte counts, then removes its originals.\n# Pass a destination as the first argument to override the default.\nDESTINATION=${{1:-{}}}\nmkdir -p \"$DESTINATION\"\n\n",
        shell_quote(&reject_root)
    );
    let mut powershell = format!(
        "param([string]$Destination = {})\n$ErrorActionPreference = 'Stop'\nNew-Item -ItemType Directory -Force -Path $Destination | Out-Null\n\n# Generated by burst-frame-deduplicator.\n# Copies each grouped asset, verifies byte counts, then removes its originals.\n\n",
        powershell_quote(&reject_root)
    );

    for asset in &manifest.assets {
        if final_decision(asset, decisions.get(&asset.id).copied()) != UserDecision::Reject {
            continue;
        }
        let files: Vec<_> = asset.files.iter().chain(asset.sidecars.iter()).collect();
        posix.push_str(&format!("# Asset {}\n", asset.id));
        powershell.push_str(&format!("# Asset {}\n", asset.id));
        for (index, file) in files.iter().enumerate() {
            let relative = safe_relative_path(&file.rel_path, &file.path);
            let relative_posix = shell_quote(&relative);
            let relative_ps = powershell_quote(&relative);
            posix.push_str(&format!(
                "source_{index}={}\ntarget_{index}=\"$DESTINATION\"/{}\n",
                shell_quote(&file.path),
                relative_posix
            ));
            powershell.push_str(&format!(
                "$source{index} = {}\n$target{index} = Join-Path $Destination {}\n",
                powershell_quote(&file.path),
                relative_ps
            ));
        }
        posix.push_str("asset_ready=1\n");
        for index in 0..files.len() {
            posix.push_str(&format!(
                "if [ ! -f \"$source_{index}\" ]; then printf 'Source unavailable: %s\\n' \"$source_{index}\" >&2; asset_ready=0; fi\n"
            ));
            posix.push_str(&format!(
                "if [ -e \"$target_{index}\" ]; then printf 'Destination exists: %s\\n' \"$target_{index}\" >&2; asset_ready=0; fi\n"
            ));
        }
        posix.push_str("if [ \"$asset_ready\" -eq 1 ]; then\n");
        let cleanup_targets = (0..files.len())
            .map(|index| format!(" \"$target_{index}\""))
            .collect::<String>();
        for index in 0..files.len() {
            posix.push_str(&format!(
                "  mkdir -p \"$(dirname \"$target_{index}\")\"\n  if ! cp -p -- \"$source_{index}\" \"$target_{index}\"; then rm -f --{cleanup_targets}; exit 1; fi\n  if [ \"$(wc -c < \"$source_{index}\")\" -ne \"$(wc -c < \"$target_{index}\")\" ]; then rm -f --{cleanup_targets}; printf 'Copy verification failed: %s\\n' \"$source_{index}\" >&2; exit 1; fi\n"
            ));
        }
        for index in 0..files.len() {
            posix.push_str(&format!(
                "  if ! rm -- \"$source_{index}\"; then\n    printf 'Could not remove source: %s; rolling back asset.\\n' \"$source_{index}\" >&2\n"
            ));
            for restored in 0..index {
                posix.push_str(&format!(
                    "    cp -p -- \"$target_{restored}\" \"$source_{restored}\"\n    [ \"$(wc -c < \"$source_{restored}\")\" -eq \"$(wc -c < \"$target_{restored}\")\" ]\n"
                ));
            }
            posix.push_str(&format!(
                "    rm -f --{cleanup_targets}\n    exit 1\n  fi\n"
            ));
        }
        posix.push_str("fi\n\n");

        powershell.push_str("$pairs = @(\n");
        for index in 0..files.len() {
            powershell.push_str(&format!(
                "  [pscustomobject]@{{ Source = $source{index}; Target = $target{index} }}\n"
            ));
        }
        powershell.push_str(")\n$assetReady = $true\n");
        for index in 0..files.len() {
            powershell.push_str(&format!(
                "if (-not (Test-Path -LiteralPath $source{index} -PathType Leaf)) {{ Write-Warning \"Source unavailable: $source{index}\"; $assetReady = $false }}\n"
            ));
        }
        powershell.push_str("if ($assetReady) {\n  $copied = @()\n  $removed = @()\n  try {\n    foreach ($pair in $pairs) {\n      if (Test-Path -LiteralPath $pair.Target) { throw \"Destination exists: $($pair.Target)\" }\n      New-Item -ItemType Directory -Force -Path (Split-Path -Parent $pair.Target) | Out-Null\n      Copy-Item -LiteralPath $pair.Source -Destination $pair.Target\n      $copied += $pair\n      if ((Get-Item -LiteralPath $pair.Source).Length -ne (Get-Item -LiteralPath $pair.Target).Length) { throw \"Copy verification failed: $($pair.Source)\" }\n    }\n    foreach ($pair in $pairs) {\n      Remove-Item -LiteralPath $pair.Source\n      $removed += $pair\n    }\n  } catch {\n    foreach ($pair in $removed) {\n      if (-not (Test-Path -LiteralPath $pair.Source)) {\n        Copy-Item -LiteralPath $pair.Target -Destination $pair.Source\n        if ((Get-Item -LiteralPath $pair.Source).Length -ne (Get-Item -LiteralPath $pair.Target).Length) { throw \"Rollback verification failed: $($pair.Source)\" }\n      }\n    }\n    foreach ($pair in $copied) {\n      if ((Test-Path -LiteralPath $pair.Source) -and (Test-Path -LiteralPath $pair.Target)) { Remove-Item -LiteralPath $pair.Target -Force }\n    }\n    throw\n  }\n}\n\n");
    }

    MoveScripts {
        destination: reject_root,
        posix,
        powershell,
    }
}

fn safe_relative_path(relative_path: &str, source: &Path) -> PathBuf {
    let mut relative = PathBuf::new();
    for component in Path::new(relative_path).components() {
        if let Component::Normal(value) = component {
            relative.push(value);
        }
    }
    if relative.as_os_str().is_empty() {
        relative.push(
            source
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("file")),
        );
    }
    relative
}

fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
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
