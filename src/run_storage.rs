use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::artifacts::{export_reviewed_artifacts, read_manifest};
use crate::operations::relocate_move_state_paths;

const COPY_BUFFER_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationProgress {
    pub stage: String,
    pub current: usize,
    pub total: Option<usize>,
    pub stage_fraction: Option<f32>,
    pub overall_fraction: f32,
    pub detail: Option<String>,
}

impl RelocationProgress {
    fn new(current: usize, total: usize, detail: Option<String>) -> Self {
        let fraction = if total == 0 {
            1.0
        } else {
            (current as f32 / total as f32).clamp(0.0, 1.0)
        };
        Self {
            stage: "relocating".to_string(),
            current,
            total: Some(total),
            stage_fraction: Some(fraction),
            overall_fraction: fraction,
            detail,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationResult {
    pub previous_run_dir: PathBuf,
    pub run_dir: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub used_atomic_rename: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct FileToCopy {
    relative: PathBuf,
    size: u64,
}

pub fn relocate_run<F>(
    run_dir: &Path,
    destination_root: &Path,
    progress: F,
) -> anyhow::Result<RelocationResult>
where
    F: Fn(RelocationProgress),
{
    let requested_source = if run_dir.is_absolute() {
        run_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .context("resolving the current directory")?
            .join(run_dir)
    };
    let source = run_dir
        .canonicalize()
        .with_context(|| format!("opening run directory {}", run_dir.display()))?;
    if !source.is_dir() {
        bail!("run directory is not a folder: {}", source.display());
    }
    read_manifest(&source)
        .with_context(|| format!("{} is not a completed burst-frame run", source.display()))?;

    fs::create_dir_all(destination_root)
        .with_context(|| format!("creating result directory {}", destination_root.display()))?;
    let destination_root = destination_root
        .canonicalize()
        .with_context(|| format!("opening result directory {}", destination_root.display()))?;
    if destination_root.starts_with(&source) {
        bail!(
            "the result directory cannot be inside the run being moved ({})",
            source.display()
        );
    }

    let source_parent = source
        .parent()
        .ok_or_else(|| anyhow!("run directory has no parent: {}", source.display()))?;
    if destination_root == source_parent {
        let (files, bytes) = inventory(&source)?;
        progress(RelocationProgress::new(
            progress_total(bytes, files.len()),
            progress_total(bytes, files.len()),
            Some("Run is already in the selected result directory".to_string()),
        ));
        return Ok(RelocationResult {
            previous_run_dir: source.clone(),
            run_dir: source,
            files: files.len(),
            bytes,
            used_atomic_rename: true,
            warnings: Vec::new(),
        });
    }

    let run_name = source
        .file_name()
        .ok_or_else(|| anyhow!("run directory has no name: {}", source.display()))?;
    let destination = unique_directory(&destination_root.join(run_name));
    let (files, bytes) = inventory(&source)?;
    progress(RelocationProgress::new(
        0,
        progress_total(bytes, files.len()),
        Some(format!("Moving to {}", destination.display())),
    ));

    match fs::rename(&source, &destination) {
        Ok(()) => finish_atomic_relocation(
            &source,
            &requested_source,
            &destination,
            files.len(),
            bytes,
            progress,
        ),
        Err(error) if crosses_devices(&error) => relocate_across_volumes(
            &source,
            &requested_source,
            &destination,
            &files,
            bytes,
            progress,
        ),
        Err(error) => Err(error).with_context(|| {
            format!(
                "moving run directory from {} to {}",
                source.display(),
                destination.display()
            )
        }),
    }
}

fn finish_atomic_relocation<F>(
    source: &Path,
    requested_source: &Path,
    destination: &Path,
    file_count: usize,
    bytes: u64,
    progress: F,
) -> anyhow::Result<RelocationResult>
where
    F: Fn(RelocationProgress),
{
    if let Err(error) = rewrite_move_state_paths(destination, source, requested_source, destination)
    {
        let rollback = fs::rename(destination, source);
        return match rollback {
            Ok(()) => Err(error.context("updating the moved-file journal; relocation was rolled back")),
            Err(rollback_error) => Err(error.context(format!(
                "updating the moved-file journal after relocation; rollback also failed: {rollback_error}"
            ))),
        };
    }

    let mut warnings = Vec::new();
    if let Err(error) = export_reviewed_artifacts(destination) {
        warnings.push(format!("Review exports could not be refreshed: {error:#}"));
    }
    progress(RelocationProgress::new(
        progress_total(bytes, file_count),
        progress_total(bytes, file_count),
        Some("Run moved".to_string()),
    ));
    Ok(RelocationResult {
        previous_run_dir: source.to_path_buf(),
        run_dir: destination.to_path_buf(),
        files: file_count,
        bytes,
        used_atomic_rename: true,
        warnings,
    })
}

fn relocate_across_volumes<F>(
    source: &Path,
    requested_source: &Path,
    destination: &Path,
    files: &[FileToCopy],
    bytes: u64,
    progress: F,
) -> anyhow::Result<RelocationResult>
where
    F: Fn(RelocationProgress),
{
    let destination_parent = destination
        .parent()
        .ok_or_else(|| anyhow!("destination has no parent: {}", destination.display()))?;
    let staging = unique_directory(&destination_parent.join(format!(
        ".bfd-relocation-{}-{}.partial",
        std::process::id(),
        timestamp_nonce()
    )));
    fs::create_dir(&staging)
        .with_context(|| format!("creating relocation staging folder {}", staging.display()))?;

    let copied = copy_inventory(source, &staging, files, bytes, &progress);
    if let Err(error) = copied {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if let Err(error) = rewrite_move_state_paths(&staging, source, requested_source, destination) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error.context("updating the moved-file journal in the relocated run"));
    }

    let tombstone = unique_directory(&source.parent().expect("validated source parent").join(
        format!(
            ".bfd-relocated-{}-{}.cleanup",
            std::process::id(),
            timestamp_nonce()
        ),
    ));
    fs::rename(source, &tombstone).with_context(|| {
        format!(
            "preparing the old run directory {} for cleanup",
            source.display()
        )
    })?;
    if let Err(error) = fs::rename(&staging, destination) {
        let rollback = fs::rename(&tombstone, source);
        let _ = fs::remove_dir_all(&staging);
        return match rollback {
            Ok(()) => Err(error).with_context(|| {
                format!("finalizing relocated run at {}", destination.display())
            }),
            Err(rollback_error) => Err(error).context(format!(
                "finalizing relocated run; restoring the original directory also failed: {rollback_error}"
            )),
        };
    }

    let mut warnings = Vec::new();
    if let Err(error) = export_reviewed_artifacts(destination) {
        warnings.push(format!("Review exports could not be refreshed: {error:#}"));
    }
    if let Err(error) = fs::remove_dir_all(&tombstone) {
        warnings.push(format!(
            "The old run cleanup folder remains at {}: {error}",
            tombstone.display()
        ));
    }
    progress(RelocationProgress::new(
        progress_total(bytes, files.len()),
        progress_total(bytes, files.len()),
        Some("Run copied, verified, and moved".to_string()),
    ));
    Ok(RelocationResult {
        previous_run_dir: source.to_path_buf(),
        run_dir: destination.to_path_buf(),
        files: files.len(),
        bytes,
        used_atomic_rename: false,
        warnings,
    })
}

fn rewrite_move_state_paths(
    run_dir: &Path,
    canonical_source: &Path,
    requested_source: &Path,
    destination: &Path,
) -> anyhow::Result<()> {
    relocate_move_state_paths(run_dir, canonical_source, destination)?;
    if requested_source != canonical_source {
        relocate_move_state_paths(run_dir, requested_source, destination)?;
    }
    Ok(())
}

fn inventory(root: &Path) -> anyhow::Result<(Vec<FileToCopy>, u64)> {
    let mut files = Vec::new();
    let mut bytes = 0_u64;
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.with_context(|| format!("reading {}", root.display()))?;
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            bail!(
                "run directories containing symbolic links cannot be relocated: {}",
                entry.path().display()
            );
        }
        if !file_type.is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("walked path is under root")
            .to_path_buf();
        let size = entry
            .metadata()
            .with_context(|| format!("reading {}", entry.path().display()))?
            .len();
        bytes = bytes.saturating_add(size);
        files.push(FileToCopy { relative, size });
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok((files, bytes))
}

fn copy_inventory<F>(
    source: &Path,
    staging: &Path,
    files: &[FileToCopy],
    total_bytes: u64,
    progress: &F,
) -> anyhow::Result<()>
where
    F: Fn(RelocationProgress),
{
    let mut copied_bytes = 0_u64;
    let total = progress_total(total_bytes, files.len());
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    for file in files {
        let from = source.join(&file.relative);
        let to = staging.join(&file.relative);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let input = fs::File::open(&from).with_context(|| format!("opening {}", from.display()))?;
        let output = fs::File::create(&to).with_context(|| format!("creating {}", to.display()))?;
        let mut input = BufReader::with_capacity(COPY_BUFFER_BYTES, input);
        let mut output = BufWriter::with_capacity(COPY_BUFFER_BYTES, output);
        loop {
            let read = input
                .read(&mut buffer)
                .with_context(|| format!("reading {}", from.display()))?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .with_context(|| format!("writing {}", to.display()))?;
            copied_bytes = copied_bytes.saturating_add(read as u64);
            progress(RelocationProgress::new(
                progress_units(copied_bytes),
                total,
                Some(file.relative.display().to_string()),
            ));
        }
        output
            .flush()
            .with_context(|| format!("flushing {}", to.display()))?;
        output
            .get_ref()
            .sync_all()
            .with_context(|| format!("syncing {}", to.display()))?;
        fs::set_permissions(&to, fs::metadata(&from)?.permissions())
            .with_context(|| format!("preserving permissions for {}", to.display()))?;
        let copied_size = fs::metadata(&to)
            .with_context(|| format!("verifying {}", to.display()))?
            .len();
        if copied_size != file.size {
            bail!(
                "copy verification failed for {}: expected {} bytes, copied {} bytes",
                file.relative.display(),
                file.size,
                copied_size
            );
        }
        if file.size == 0 {
            progress(RelocationProgress::new(
                progress_units(copied_bytes),
                total,
                Some(file.relative.display().to_string()),
            ));
        }
    }
    Ok(())
}

fn progress_units(bytes: u64) -> usize {
    bytes.min(usize::MAX as u64) as usize
}

fn progress_total(bytes: u64, _files: usize) -> usize {
    if bytes == 0 { 1 } else { progress_units(bytes) }
}

fn unique_directory(desired: &Path) -> PathBuf {
    if !desired.exists() {
        return desired.to_path_buf();
    }
    let parent = desired.parent().unwrap_or_else(|| Path::new("."));
    let name = desired
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("run");
    for index in 2..10_000 {
        let candidate = parent.join(format!("{name}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{name}-{}", timestamp_nonce()))
}

fn crosses_devices(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::CrossesDevices || matches!(error.raw_os_error(), Some(17 | 18))
}

fn timestamp_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{RelocationProgress, copy_inventory, inventory, relocate_run};
    use crate::operations::{MoveRecord, MoveState};
    use crate::types::{
        AccelerationPreference, AccelerationReport, DecoderReport, DetectorPreference,
        DetectorReport, RunManifest,
    };

    fn write_minimal_run(run: &std::path::Path) {
        fs::create_dir_all(run.join("thumbs")).unwrap();
        fs::write(run.join("thumbs/frame.jpg"), b"thumbnail").unwrap();
        let manifest = RunManifest {
            app_version: "test".to_string(),
            root: run.join("source"),
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
                selected: "heuristic".to_string(),
                capabilities: Vec::new(),
                notes: Vec::new(),
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
            assets: Vec::new(),
        };
        crate::artifacts::write_manifest(run, &manifest).unwrap();
    }

    #[test]
    fn relocates_a_run_and_reports_monotonic_progress() {
        let temp = tempdir().unwrap();
        let source_root = temp.path().join("old");
        let destination_root = temp.path().join("new");
        let run = source_root.join("run-one");
        write_minimal_run(&run);

        let progress = std::sync::Mutex::new(Vec::<RelocationProgress>::new());
        let result = relocate_run(&run, &destination_root, |update| {
            progress.lock().unwrap().push(update)
        })
        .unwrap();

        assert_eq!(
            result.run_dir,
            destination_root.canonicalize().unwrap().join("run-one")
        );
        assert!(!run.exists());
        assert!(result.run_dir.join("manifest.json").is_file());
        assert!(result.run_dir.join("thumbs/frame.jpg").is_file());
        let updates = progress.into_inner().unwrap();
        assert!(!updates.is_empty());
        assert!(
            updates
                .windows(2)
                .all(|pair| { pair[0].overall_fraction <= pair[1].overall_fraction })
        );
        assert_eq!(updates.last().unwrap().overall_fraction, 1.0);
    }

    #[test]
    fn chooses_a_unique_destination_without_overwriting_a_run() {
        let temp = tempdir().unwrap();
        let run = temp.path().join("source/run");
        let destination_root = temp.path().join("destination");
        write_minimal_run(&run);
        fs::create_dir_all(destination_root.join("run")).unwrap();

        let result = relocate_run(&run, &destination_root, |_| {}).unwrap();

        assert_eq!(
            result.run_dir,
            destination_root.canonicalize().unwrap().join("run-2")
        );
        assert!(destination_root.join("run/manifest.json").exists() == false);
        assert!(result.run_dir.join("manifest.json").is_file());
    }

    #[test]
    fn rewrites_internal_moved_file_destinations() {
        let temp = tempdir().unwrap();
        let run = temp.path().join("source/run");
        let destination_root = temp.path().join("destination");
        write_minimal_run(&run);
        let moved = run.join("moved_rejects/operation/frame.raw");
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        fs::write(&moved, b"raw").unwrap();
        let state = MoveState {
            version: 1,
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            records: vec![MoveRecord {
                asset_id: "frame".to_string(),
                source: temp.path().join("card/frame.raw"),
                destination: moved,
                size: 3,
                moved_at: "2026-01-01T00:00:00Z".to_string(),
                restored_at: None,
            }],
        };
        fs::write(
            run.join("move_state.json"),
            serde_json::to_vec_pretty(&state).unwrap(),
        )
        .unwrap();

        let result = relocate_run(&run, &destination_root, |_| {}).unwrap();
        let relocated = crate::operations::read_move_state(&result.run_dir).unwrap();

        assert_eq!(
            relocated.records[0].destination,
            result.run_dir.join("moved_rejects/operation/frame.raw")
        );
    }

    #[test]
    fn verified_copy_reports_byte_progress_and_preserves_content() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("source");
        let staging = temp.path().join("staging");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir_all(&staging).unwrap();
        let content = vec![0x5a; 5 * 1024 * 1024 + 17];
        fs::write(source.join("nested/large.bin"), &content).unwrap();
        let (files, bytes) = inventory(&source).unwrap();
        let updates = std::sync::Mutex::new(Vec::<RelocationProgress>::new());

        copy_inventory(&source, &staging, &files, bytes, &|update| {
            updates.lock().unwrap().push(update)
        })
        .unwrap();

        assert_eq!(fs::read(staging.join("nested/large.bin")).unwrap(), content);
        let updates = updates.into_inner().unwrap();
        assert!(updates.len() >= 2);
        assert_eq!(updates.last().unwrap().overall_fraction, 1.0);
        assert!(
            updates
                .windows(2)
                .all(|pair| { pair[0].overall_fraction <= pair[1].overall_fraction })
        );
    }
}
