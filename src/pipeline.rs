use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{Context, anyhow};
use chrono::Local;
use image::{DynamicImage, ImageFormat};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use crate::artifacts::{ensure_review_state, export_reviewed_artifacts, write_manifest};
use crate::assets::{AssetInput, discover_assets};
use crate::decode::{decoder_report, load_preview, resize_rgb};
use crate::detector::{detect_subject, detector_report, merge_detector_metrics};
use crate::features::{hash_distance, score_image};
use crate::metadata::read_photo_metadata;
use crate::types::{
    AccelerationPreference, AccelerationReport, AssetRecord, AssetTimings, BenchmarkReport,
    BurstCluster, QualityMetrics, RunManifest, ScanOptions, SuggestedAction, Suggestion, Summary,
};

pub async fn run_scan(
    root: &Path,
    out: Option<PathBuf>,
    options: ScanOptions,
) -> anyhow::Result<PathBuf> {
    let total_start = Instant::now();
    let root = root
        .canonicalize()
        .with_context(|| format!("source folder does not exist: {}", root.display()))?;
    if !root.is_dir() {
        return Err(anyhow!(
            "source path is not a directory: {}",
            root.display()
        ));
    }

    let run_dir = out.unwrap_or_else(default_run_dir);
    let thumbs_dir = run_dir.join("thumbs");
    fs::create_dir_all(&run_dir)?;
    if options.generate_thumbnails {
        fs::create_dir_all(&thumbs_dir)?;
    }

    let discovery_start = Instant::now();
    let inputs = discover_assets(&root).context("discovering image assets")?;
    let discovery_ms = elapsed_ms(discovery_start);
    let image_files = inputs.iter().map(|asset| asset.files.len()).sum();
    let sidecar_files = inputs.iter().map(|asset| asset.sidecars.len()).sum();
    eprintln!(
        "Discovered {} assets ({} image files, {} sidecars)",
        inputs.len(),
        image_files,
        sidecar_files
    );

    let pool = ThreadPoolBuilder::new()
        .num_threads(options.workers.unwrap_or_else(default_workers))
        .build()
        .context("creating scoring worker pool")?;
    let thumb_root = options.generate_thumbnails.then_some(thumbs_dir.clone());
    let scoring_start = Instant::now();
    let scored = AtomicUsize::new(0);
    let total_inputs = inputs.len();
    let mut score_results: Vec<ScoreResult> = pool.install(|| {
        inputs
            .par_iter()
            .map(|input| {
                let result = score_asset(input, &options, thumb_root.as_deref());
                let done = scored.fetch_add(1, Ordering::Relaxed) + 1;
                if done == total_inputs || done.is_multiple_of(100) {
                    eprintln!("Scored {done}/{total_inputs} assets");
                }
                result
            })
            .collect()
    });
    let scoring_ms = elapsed_ms(scoring_start);
    let mut assets: Vec<AssetRecord> = score_results.drain(..).map(|result| result.asset).collect();

    let index_clusters = build_clusters(&assets, &options);
    let refinement_start = Instant::now();
    let refined_count = refine_cluster_candidates(&mut assets, &index_clusters, &options, &pool);
    let refinement_ms = elapsed_ms(refinement_start);
    let cluster_start = Instant::now();
    let clusters = rank_clusters(&mut assets, index_clusters, &options);
    let clustering_ms = elapsed_ms(cluster_start);
    let mut summary = Summary {
        discovered_assets: assets.len(),
        image_files,
        sidecar_files,
        clusters: clusters.len(),
        ..Summary::default()
    };
    for asset in &assets {
        match asset.suggestion.action {
            SuggestedAction::Keep => summary.suggested_keep += 1,
            SuggestedAction::Reject => summary.suggested_reject += 1,
            SuggestedAction::Review => summary.suggested_review += 1,
            SuggestedAction::Error => summary.errors += 1,
        }
    }

    let mut benchmarks = vec![
        benchmark("discovery", discovery_ms, Some(inputs.len())),
        benchmark("scoring_total", scoring_ms, Some(assets.len())),
        benchmark(
            "decode_worker_sum",
            assets.iter().map(|asset| asset.timings.decode_ms).sum(),
            Some(assets.len()),
        ),
        benchmark(
            "feature_scoring_worker_sum",
            assets.iter().map(|asset| asset.timings.feature_ms).sum(),
            Some(assets.len()),
        ),
        benchmark("refinement_total", refinement_ms, Some(refined_count)),
        benchmark(
            "refinement_decode_worker_sum",
            assets
                .iter()
                .map(|asset| asset.timings.refine_decode_ms)
                .sum(),
            Some(refined_count),
        ),
        benchmark(
            "refinement_feature_worker_sum",
            assets
                .iter()
                .map(|asset| asset.timings.refine_feature_ms)
                .sum(),
            Some(refined_count),
        ),
        benchmark(
            "detector_worker_sum",
            assets.iter().map(|asset| asset.timings.detector_ms).sum(),
            Some(assets.len()),
        ),
        benchmark(
            "thumbnail_generation_worker_sum",
            assets.iter().map(|asset| asset.timings.thumbnail_ms).sum(),
            Some(assets.len()),
        ),
        benchmark("clustering_and_ranking", clustering_ms, Some(assets.len())),
    ];

    let acceleration = acceleration_report(options.acceleration);
    let detector = detector_report(options.detector);
    let mut manifest = RunManifest {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        root,
        created_at: chrono::Utc::now().to_rfc3339(),
        options,
        acceleration,
        detector,
        decoders: decoder_report(),
        benchmarks: benchmarks.clone(),
        summary,
        clusters,
        assets,
    };

    let manifest_start = Instant::now();
    write_manifest(&run_dir, &manifest)?;
    benchmarks.push(benchmark(
        "manifest_write",
        elapsed_ms(manifest_start),
        None,
    ));
    let export_start = Instant::now();
    ensure_review_state(&run_dir, &manifest)?;
    export_reviewed_artifacts(&run_dir)?;
    benchmarks.push(benchmark("review_export", elapsed_ms(export_start), None));
    benchmarks.push(benchmark(
        "scan_total",
        elapsed_ms(total_start),
        Some(manifest.assets.len()),
    ));
    manifest.benchmarks = benchmarks;
    write_manifest(&run_dir, &manifest)?;
    eprintln!(
        "Suggested: keep {}, reject {}, review {}, errors {} across {} clusters",
        manifest.summary.suggested_keep,
        manifest.summary.suggested_reject,
        manifest.summary.suggested_review,
        manifest.summary.errors,
        manifest.summary.clusters
    );
    Ok(run_dir)
}

struct ScoreResult {
    asset: AssetRecord,
}

struct RefineResult {
    idx: usize,
    metrics: QualityMetrics,
    feature_backend: String,
    notes: Vec<String>,
    decode_ms: f64,
    feature_ms: f64,
}

fn score_asset(
    input: &AssetInput,
    options: &ScanOptions,
    thumbs_dir: Option<&Path>,
) -> ScoreResult {
    let mut timings = AssetTimings::default();
    let mut record = AssetRecord {
        id: input.id.clone(),
        representative: input.representative.clone(),
        files: input.files.clone(),
        sidecars: input.sidecars.clone(),
        directory: input.directory.clone(),
        stem: input.stem.clone(),
        prefix: input.prefix.clone(),
        seq: input.seq,
        created_ms: input.created_ms,
        modified_ms: input.modified_ms,
        capture_ms: input.time_key_ms(),
        width: 0,
        height: 0,
        decoder: String::new(),
        feature_backend: String::new(),
        metadata: read_photo_metadata(&input.representative.path),
        metrics: QualityMetrics::default(),
        detector: None,
        timings,
        cluster_id: 0,
        suggestion: Suggestion::default(),
        thumb: None,
        error: None,
    };

    let decode_start = Instant::now();
    match load_preview(
        &input.representative.path,
        &input.representative.extension,
        options.preview_size,
    ) {
        Ok(decoded) => {
            timings.decode_ms = elapsed_ms(decode_start);
            record.width = decoded.width;
            record.height = decoded.height;
            record.decoder = decoded.decoder;
            let feature_start = Instant::now();
            let feature = score_image(&decoded.image, options.acceleration);
            timings.feature_ms = elapsed_ms(feature_start);
            record.feature_backend = feature.backend;
            record.metrics = feature.metrics;
            record.suggestion.explanations.extend(feature.notes);

            let detector_start = Instant::now();
            let (detector, detector_notes) = detect_subject(
                &input.representative.path,
                &record.metrics,
                options.detector,
            );
            timings.detector_ms = elapsed_ms(detector_start);
            if let Some(detector) = detector {
                merge_detector_metrics(&mut record.metrics, &detector);
                record.detector = Some(detector);
            }
            record.suggestion.explanations.extend(detector_notes);

            if let Some(thumbs_dir) = thumbs_dir {
                let thumb_start = Instant::now();
                match write_thumbnail(&record.id, &decoded.image, thumbs_dir, options.thumb_size) {
                    Ok(rel) => record.thumb = Some(rel),
                    Err(err) => record
                        .suggestion
                        .explanations
                        .push(format!("Thumbnail generation failed: {err}")),
                }
                timings.thumbnail_ms = elapsed_ms(thumb_start);
            }
        }
        Err(err) => {
            timings.decode_ms = elapsed_ms(decode_start);
            record.error = Some(err.to_string());
            record.suggestion = Suggestion {
                action: SuggestedAction::Error,
                rank: 0,
                score: 0.0,
                reason: "decode error".to_string(),
                explanations: vec![err.to_string()],
            };
        }
    }
    record.timings = timings;
    ScoreResult { asset: record }
}

fn write_thumbnail(
    id: &str,
    image: &image::RgbImage,
    thumbs_dir: &Path,
    size: u32,
) -> anyhow::Result<String> {
    let thumb = resize_rgb(image, size);
    let path = thumbs_dir.join(format!("{id}.jpg"));
    DynamicImage::ImageRgb8(thumb).save_with_format(&path, ImageFormat::Jpeg)?;
    Ok(format!("thumbs/{id}.jpg"))
}

fn refine_cluster_candidates(
    assets: &mut [AssetRecord],
    index_clusters: &[Vec<usize>],
    options: &ScanOptions,
    pool: &rayon::ThreadPool,
) -> usize {
    if options.disable_refinement
        || options.refine_size <= options.preview_size
        || options.refine_candidates_per_cluster == 0
    {
        return 0;
    }
    let candidates = refinement_candidates(assets, index_clusters, options);
    if candidates.is_empty() {
        return 0;
    }
    eprintln!(
        "Refining {}/{} candidate assets at {}px long edge",
        candidates.len(),
        assets.len(),
        options.refine_size
    );
    let refined = AtomicUsize::new(0);
    let total = candidates.len();
    let results: Vec<Option<RefineResult>> = pool.install(|| {
        candidates
            .par_iter()
            .map(|idx| {
                let result = refine_asset(*idx, &assets[*idx], options);
                let done = refined.fetch_add(1, Ordering::Relaxed) + 1;
                if done == total || done.is_multiple_of(100) {
                    eprintln!("Refined {done}/{total} candidate assets");
                }
                result
            })
            .collect()
    });

    let mut applied = 0usize;
    for result in results.into_iter().flatten() {
        let asset = &mut assets[result.idx];
        if let Some(detector) = asset.detector.clone() {
            asset.metrics = result.metrics;
            merge_detector_metrics(&mut asset.metrics, &detector);
        } else {
            asset.metrics = result.metrics;
        }
        asset.feature_backend =
            format!("{}+refined_{}", result.feature_backend, options.refine_size);
        asset.timings.refine_decode_ms = result.decode_ms;
        asset.timings.refine_feature_ms = result.feature_ms;
        asset.suggestion.explanations.extend(result.notes);
        applied += 1;
    }
    applied
}

fn refinement_candidates(
    assets: &[AssetRecord],
    index_clusters: &[Vec<usize>],
    options: &ScanOptions,
) -> Vec<usize> {
    let mut selected = BTreeSet::new();
    for indices in index_clusters {
        if indices.len() <= 1 {
            continue;
        }
        let keep_count = keep_count_for_cluster(indices.len(), options.keepers_per_cluster);
        let ranked = ranked_scores(assets, indices);
        let keep_threshold = ranked
            .get(keep_count.saturating_sub(1))
            .map(|(_, score)| *score)
            .unwrap_or(0.0);
        let budget = options.refine_candidates_per_cluster.min(indices.len());
        for (rank_zero, (idx, score)) in ranked.iter().enumerate() {
            if (rank_zero < budget || *score >= keep_threshold - 0.06)
                && assets[*idx].error.is_none()
            {
                selected.insert(*idx);
            }
        }
    }
    selected.into_iter().collect()
}

fn refine_asset(idx: usize, asset: &AssetRecord, options: &ScanOptions) -> Option<RefineResult> {
    let decode_start = Instant::now();
    let decoded = match load_preview(
        &asset.representative.path,
        &asset.representative.extension,
        options.refine_size,
    ) {
        Ok(decoded) => decoded,
        Err(err) => {
            eprintln!(
                "High-resolution refinement failed for {}: {err}",
                asset.representative.rel_path
            );
            return None;
        }
    };
    let decode_ms = elapsed_ms(decode_start);
    let feature_start = Instant::now();
    let feature = score_image(&decoded.image, options.acceleration);
    let feature_ms = elapsed_ms(feature_start);
    Some(RefineResult {
        idx,
        metrics: feature.metrics,
        feature_backend: feature.backend,
        notes: feature.notes,
        decode_ms,
        feature_ms,
    })
}

fn rank_clusters(
    assets: &mut [AssetRecord],
    index_clusters: Vec<Vec<usize>>,
    options: &ScanOptions,
) -> Vec<BurstCluster> {
    let mut clusters = Vec::new();

    for (cluster_idx, indices) in index_clusters.into_iter().enumerate() {
        let cluster_id = cluster_idx + 1;
        let keep_count = keep_count_for_cluster(indices.len(), options.keepers_per_cluster);
        let ranked = ranked_scores(assets, &indices);
        let keep_threshold = ranked
            .get(keep_count.saturating_sub(1))
            .map(|(_, score)| *score)
            .unwrap_or(0.0);
        let best_asset_id = ranked.first().map(|(idx, _)| assets[*idx].id.clone());

        for (rank_zero, (idx, score)) in ranked.iter().enumerate() {
            let rank = rank_zero + 1;
            let cluster_len = indices.len();
            let asset = &mut assets[*idx];
            asset.cluster_id = cluster_id;
            let mut action = SuggestedAction::Reject;
            let reason;
            if let Some(error) = &asset.error {
                action = SuggestedAction::Error;
                reason = "decode error".to_string();
                asset.suggestion = Suggestion {
                    action,
                    rank,
                    score: *score,
                    reason,
                    explanations: vec![error.clone()],
                };
                continue;
            }
            if cluster_len == 1 && !options.cull_singletons {
                action = SuggestedAction::Keep;
                reason = "unique shot".to_string();
            } else if rank <= keep_count {
                action = SuggestedAction::Keep;
                reason = format!("top {rank} of {cluster_len}");
            } else if *score >= keep_threshold - 0.035 {
                action = SuggestedAction::Review;
                reason = format!("near tie with keeper in cluster {cluster_id}");
            } else {
                reason =
                    format!("duplicate burst frame; better ranked frame in cluster {cluster_id}");
            }
            asset.suggestion = Suggestion {
                action,
                rank,
                score: *score,
                reason,
                explanations: explanations_for(asset, cluster_len, keep_count, rank),
            };
        }

        let times: Vec<i64> = indices
            .iter()
            .filter_map(|idx| assets[*idx].time_key_ms())
            .collect();
        let first = indices.first().map(|idx| &assets[*idx]);
        clusters.push(BurstCluster {
            id: cluster_id,
            asset_ids: indices.iter().map(|idx| assets[*idx].id.clone()).collect(),
            directory: first.map(|a| a.directory.clone()).unwrap_or_default(),
            prefix: first.map(|a| a.prefix.clone()).unwrap_or_default(),
            start_ms: times.iter().min().copied(),
            end_ms: times.iter().max().copied(),
            keep_count,
            best_asset_id,
        });
    }

    assets.sort_by(|a, b| {
        (
            a.cluster_id,
            a.suggestion.rank,
            &a.directory,
            &a.prefix,
            a.seq.unwrap_or(-1),
            &a.stem,
        )
            .cmp(&(
                b.cluster_id,
                b.suggestion.rank,
                &b.directory,
                &b.prefix,
                b.seq.unwrap_or(-1),
                &b.stem,
            ))
    });
    clusters
}

fn ranked_scores(assets: &[AssetRecord], indices: &[usize]) -> Vec<(usize, f64)> {
    let sharp_norms = norm(
        indices
            .iter()
            .map(|idx| assets[*idx].metrics.sharpness.max(0.0).ln_1p())
            .collect(),
    );
    let ten_norms = norm(
        indices
            .iter()
            .map(|idx| assets[*idx].metrics.tenengrad.max(0.0).ln_1p())
            .collect(),
    );

    let mut ranked: Vec<(usize, f64)> = indices
        .iter()
        .enumerate()
        .map(|(pos, idx)| {
            let asset = &assets[*idx];
            let sharp_component = 0.70 * sharp_norms[pos] + 0.30 * ten_norms[pos];
            (*idx, quality_score(asset, sharp_component))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
}

fn quality_score(asset: &AssetRecord, sharp_component: f64) -> f64 {
    if asset.error.is_some() {
        return 0.0;
    }
    0.56 * sharp_component
        + 0.22 * asset.metrics.completeness
        + 0.10 * asset.metrics.contrast
        + 0.07 * asset.metrics.object_confidence
        + 0.05 * asset.metrics.exposure_score
        - 0.08 * (asset.metrics.border_energy_fraction / 0.35).min(1.0)
}

fn build_clusters(assets: &[AssetRecord], options: &ScanOptions) -> Vec<Vec<usize>> {
    let mut indices: Vec<usize> = (0..assets.len()).collect();
    indices.sort_by(|a, b| {
        let left = &assets[*a];
        let right = &assets[*b];
        (
            &left.directory,
            &left.prefix,
            left.seq.unwrap_or(-1),
            left.time_key_ms().unwrap_or(i64::MAX),
            &left.stem,
        )
            .cmp(&(
                &right.directory,
                &right.prefix,
                right.seq.unwrap_or(-1),
                right.time_key_ms().unwrap_or(i64::MAX),
                &right.stem,
            ))
    });

    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut cluster_start_ms: Option<i64> = None;
    let mut previous_idx: Option<usize> = None;

    for idx in indices {
        let asset = &assets[idx];
        let split = if let Some(prev_idx) = previous_idx {
            let prev = &assets[prev_idx];
            should_split(prev, asset, cluster_start_ms, current.len(), options)
        } else {
            true
        };

        if split {
            if !current.is_empty() {
                clusters.push(std::mem::take(&mut current));
            }
            cluster_start_ms = asset.time_key_ms();
        }
        current.push(idx);
        previous_idx = Some(idx);
    }
    if !current.is_empty() {
        clusters.push(current);
    }
    clusters
}

fn should_split(
    prev: &AssetRecord,
    current: &AssetRecord,
    cluster_start_ms: Option<i64>,
    current_cluster_len: usize,
    options: &ScanOptions,
) -> bool {
    if current.directory != prev.directory || current.prefix != prev.prefix {
        return true;
    }
    if let (Some(now), Some(prev_time)) = (current.time_key_ms(), prev.time_key_ms()) {
        if (now - prev_time).abs() > options.max_time_gap_ms {
            return true;
        }
        if let Some(start) = cluster_start_ms
            && (now - start).abs() > options.max_cluster_span_ms
        {
            return true;
        }
    }
    if let (Some(now), Some(prev_seq)) = (current.seq, prev.seq)
        && now - prev_seq > options.max_seq_gap
    {
        return true;
    }
    if current_cluster_len >= 2
        && hash_distance(&prev.metrics.dhash, &current.metrics.dhash) > options.max_hash_gap
    {
        return true;
    }
    false
}

fn explanations_for(
    asset: &AssetRecord,
    cluster_len: usize,
    keep_count: usize,
    rank: usize,
) -> Vec<String> {
    let mut explanations = Vec::new();
    explanations.push(format!(
        "Ranked {} of {} in this burst; configured keeper count is {}.",
        rank, cluster_len, keep_count
    ));
    explanations.push(format!(
        "Sharpness {:.1}, gradient {:.1}, contrast {:.2}.",
        asset.metrics.sharpness, asset.metrics.tenengrad, asset.metrics.contrast
    ));
    explanations.push(format!(
        "Completeness {:.2}; border saliency {:.2} indicates how much subject-like detail touches frame edges.",
        asset.metrics.completeness, asset.metrics.border_energy_fraction
    ));
    explanations.push(format!(
        "Exposure score {:.2}; clipped pixels {:.2}%.",
        asset.metrics.exposure_score,
        asset.metrics.clipped_fraction * 100.0
    ));
    if asset.timings.refine_feature_ms > 0.0 {
        explanations.push(format!(
            "Quality metrics were refined at higher resolution; refinement decode {:.1} ms, feature scoring {:.1} ms.",
            asset.timings.refine_decode_ms, asset.timings.refine_feature_ms
        ));
    }
    if asset.files.len() > 1 {
        explanations.push(format!(
            "Decision applies to {} same-basename files, such as RAW+JPEG pairs.",
            asset.files.len()
        ));
    }
    explanations
}

fn keep_count_for_cluster(size: usize, requested: Option<usize>) -> usize {
    if let Some(requested) = requested {
        return requested.max(1).min(size.max(1));
    }
    if size <= 8 {
        1
    } else if size <= 18 {
        2
    } else {
        3.min(size)
    }
}

fn norm(values: Vec<f64>) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let lo = values
        .iter()
        .fold(f64::INFINITY, |acc, value| acc.min(*value));
    let hi = values
        .iter()
        .fold(f64::NEG_INFINITY, |acc, value| acc.max(*value));
    if (hi - lo).abs() < 1e-9 {
        vec![0.5; values.len()]
    } else {
        values
            .into_iter()
            .map(|value| (value - lo) / (hi - lo))
            .collect()
    }
}

fn default_run_dir() -> PathBuf {
    PathBuf::from("runs").join(format!("run_{}", Local::now().format("%Y%m%d_%H%M%S")))
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(1)
}

fn acceleration_report(requested: AccelerationPreference) -> AccelerationReport {
    let mut capabilities = vec![format!("rayon_cpu_workers:{}", default_workers())];
    let mut notes = vec!["CPU/Rayon scoring remains the fallback for every platform.".to_string()];
    #[cfg(all(target_os = "macos", feature = "metal-accel"))]
    let metal_available = crate::metal_accel::is_available();
    #[cfg(not(all(target_os = "macos", feature = "metal-accel")))]
    let metal_available = false;
    if cfg!(target_os = "macos") {
        capabilities.push("macos_platform_detected".to_string());
        notes.push(
            "RAW/HEIC decoding can use ImageMagick or macOS sips when available.".to_string(),
        );
    }
    if metal_available {
        capabilities.push("metal_focus_scoring".to_string());
        notes.push(
            "Metal acceleration is available for Laplacian sharpness and gradient scoring."
                .to_string(),
        );
    } else if cfg!(all(target_os = "macos", feature = "metal-accel")) {
        notes.push("Metal acceleration is compiled in but no usable Metal scorer initialized; CPU/Rayon will be used.".to_string());
    }
    let selected = match requested {
        AccelerationPreference::Cpu => "cpu_rayon",
        AccelerationPreference::Auto if metal_available => "metal_focus_cpu_rest",
        AccelerationPreference::Auto => "cpu_rayon",
        AccelerationPreference::Metal => {
            if metal_available {
                "metal_focus_cpu_rest"
            } else {
                if cfg!(all(target_os = "macos", feature = "metal-accel")) {
                    notes.push("Metal was requested but no usable Metal scorer initialized at runtime; falling back to CPU/Rayon.".to_string());
                } else {
                    notes.push("Metal was requested but this build cannot compile the Metal scorer on the current platform.".to_string());
                }
                "cpu_rayon"
            }
        }
        AccelerationPreference::Cuda => {
            notes.push("CUDA was requested; falling back to CPU because no CUDA scorer adapter is bundled yet.".to_string());
            "cpu_rayon"
        }
        AccelerationPreference::OpenCl => {
            notes.push("OpenCL was requested; falling back to CPU because no OpenCL scorer adapter is bundled yet.".to_string());
            "cpu_rayon"
        }
    };
    AccelerationReport {
        requested,
        selected: selected.to_string(),
        capabilities,
        notes,
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn benchmark(stage: &str, elapsed_ms: f64, items: Option<usize>) -> BenchmarkReport {
    let items_per_sec = items.and_then(|items| {
        if elapsed_ms > 0.0 {
            Some(items as f64 / (elapsed_ms / 1000.0))
        } else {
            None
        }
    });
    BenchmarkReport {
        stage: stage.to_string(),
        elapsed_ms,
        items,
        items_per_sec,
    }
}
