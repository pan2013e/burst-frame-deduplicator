use std::collections::{BTreeMap, BTreeSet, HashMap};
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
use crate::assets::{AssetInput, discover_assets_with_progress};
use crate::decode::{decoder_report, load_preview, resize_rgb};
use crate::detector::{DetectorEngine, detect_subject, merge_detector_metrics};
use crate::features::{
    SimilarityComparison, SimilarityFeatures, compare_similarity, hash_distance, score_image,
    similarity_features, update_subject_focus,
};
use crate::metadata::read_photo_metadata;
use crate::progress::{ProgressReporter, ProgressStage};
use crate::types::{
    AccelerationPreference, AccelerationReport, AssetRecord, AssetTimings, BenchmarkReport,
    BurstCluster, BurstSequence, QualityMetrics, RunManifest, ScanOptions, SimilarityMetrics,
    SuggestedAction, Suggestion, Summary,
};

pub async fn run_scan(
    root: &Path,
    out: Option<PathBuf>,
    options: ScanOptions,
    progress: ProgressReporter,
) -> anyhow::Result<PathBuf> {
    let total_start = Instant::now();
    progress.emit(
        ProgressStage::Preparing,
        0,
        Some(1),
        Some(root.display().to_string()),
    );
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
    progress.emit(ProgressStage::Preparing, 1, Some(1), None);

    let discovery_start = Instant::now();
    progress.emit(ProgressStage::Discovering, 0, None, None);
    let discovery_progress = progress.clone();
    let inputs = discover_assets_with_progress(&root, move |visited, path| {
        if visited == 1 || visited.is_multiple_of(100) {
            discovery_progress.emit(
                ProgressStage::Discovering,
                visited,
                None,
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned()),
            );
        }
    })
    .context("discovering image assets")?;
    let discovery_ms = elapsed_ms(discovery_start);
    let image_files = inputs.iter().map(|asset| asset.files.len()).sum();
    let sidecar_files = inputs.iter().map(|asset| asset.sidecars.len()).sum();
    progress.emit(
        ProgressStage::Discovering,
        inputs.len(),
        Some(inputs.len()),
        Some(format!(
            "{image_files} image files, {sidecar_files} sidecars"
        )),
    );

    let pool = ThreadPoolBuilder::new()
        .num_threads(options.workers.unwrap_or_else(default_workers))
        .build()
        .context("creating scoring worker pool")?;
    let worker_count = pool.current_num_threads();
    let detector_initialization_start = Instant::now();
    let detector_engine = DetectorEngine::initialize(&options, worker_count);
    let detector_initialization_ms = elapsed_ms(detector_initialization_start);
    let thumb_root = options.generate_thumbnails.then_some(thumbs_dir.clone());
    let scoring_start = Instant::now();
    let scored = AtomicUsize::new(0);
    let total_inputs = inputs.len();
    progress.emit(ProgressStage::Analyzing, 0, Some(total_inputs), None);
    let scoring_progress = progress.clone();
    let score_results: Vec<ScoreResult> = pool.install(|| {
        inputs
            .par_iter()
            .map(|input| {
                let result = score_asset(input, &options, &detector_engine, thumb_root.as_deref());
                let done = scored.fetch_add(1, Ordering::Relaxed) + 1;
                scoring_progress.emit(
                    ProgressStage::Analyzing,
                    done,
                    Some(total_inputs),
                    Some(input.representative.rel_path.clone()),
                );
                result
            })
            .collect()
    });
    let scoring_ms = elapsed_ms(scoring_start);
    let mut assets = Vec::with_capacity(score_results.len());
    let mut similarity = Vec::with_capacity(score_results.len());
    for result in score_results {
        assets.push(result.asset);
        similarity.push(result.similarity);
    }

    let grouping_start = Instant::now();
    progress.emit(ProgressStage::Grouping, 0, Some(1), None);
    let index_bursts = build_bursts(&assets, &options);
    let index_clusters =
        build_near_duplicate_groups(&mut assets, &similarity, &index_bursts, &options);
    let grouping_ms = elapsed_ms(grouping_start);
    progress.emit(
        ProgressStage::Grouping,
        1,
        Some(1),
        Some(format!(
            "{} bursts, {} stacks",
            index_bursts.len(),
            index_clusters.len()
        )),
    );
    let refinement_start = Instant::now();
    let refined_count =
        refine_cluster_candidates(&mut assets, &index_clusters, &options, &pool, &progress);
    let refinement_ms = elapsed_ms(refinement_start);
    let ranking_start = Instant::now();
    progress.emit(ProgressStage::Ranking, 0, Some(1), None);
    let (clusters, bursts) = rank_clusters(&mut assets, index_clusters, &index_bursts, &options);
    let ranking_ms = elapsed_ms(ranking_start);
    progress.emit(ProgressStage::Ranking, 1, Some(1), None);
    let clustering_ms = grouping_ms + ranking_ms;
    let mut summary = Summary {
        discovered_assets: assets.len(),
        image_files,
        sidecar_files,
        clusters: clusters.len(),
        bursts: bursts.len(),
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

    let detector_timings = detector_engine.timing_snapshot();

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
        benchmark("detector_initialization", detector_initialization_ms, None),
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
            "detector_preprocessing_worker_sum",
            detector_timings.preprocessing_ms,
            Some(detector_timings.runs),
        ),
        benchmark(
            "detector_session_queue_wait_worker_sum",
            detector_timings.queue_wait_ms,
            Some(detector_timings.runs),
        ),
        benchmark(
            "detector_inference_worker_sum",
            detector_timings.inference_ms,
            Some(detector_timings.runs),
        ),
        benchmark(
            "detector_postprocessing_worker_sum",
            detector_timings.postprocessing_ms,
            Some(detector_timings.runs),
        ),
        benchmark(
            "thumbnail_generation_worker_sum",
            assets.iter().map(|asset| asset.timings.thumbnail_ms).sum(),
            Some(assets.len()),
        ),
        benchmark("burst_and_stack_grouping", grouping_ms, Some(assets.len())),
        benchmark("ranking_and_suggestions", ranking_ms, Some(assets.len())),
        benchmark("clustering_and_ranking", clustering_ms, Some(assets.len())),
    ];

    let acceleration = acceleration_report(options.acceleration, worker_count, &assets);
    let mut detector = detector_engine.report();
    let mut detector_usage = BTreeMap::new();
    let mut detector_not_run = 0usize;
    for asset in &assets {
        if asset.error.is_some() {
            detector_not_run += 1;
            continue;
        }
        let backend = asset
            .detector
            .as_ref()
            .map(|output| output.backend.as_str())
            .unwrap_or("off");
        *detector_usage.entry(backend).or_insert(0usize) += 1;
    }
    if detector_usage.len() == 1 {
        detector.selected = detector_usage
            .keys()
            .next()
            .map(|backend| (*backend).to_string())
            .unwrap_or_else(|| detector.selected.clone());
    } else if detector_usage.len() > 1 {
        detector.selected = "mixed".to_string();
    }
    let detector_usage_note = detector_usage
        .iter()
        .map(|(backend, count)| format!("{backend}={count}"))
        .collect::<Vec<_>>()
        .join(", ");
    detector.notes.push(format!(
        "Per-frame backend usage: {}.",
        if detector_usage_note.is_empty() {
            "none"
        } else {
            &detector_usage_note
        }
    ));
    if detector_not_run > 0 {
        detector.notes.push(format!(
            "Detector did not run for {detector_not_run} asset(s) that failed before subject detection."
        ));
    }
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
        bursts,
        clusters,
        assets,
    };

    let manifest_start = Instant::now();
    progress.emit(ProgressStage::Writing, 0, Some(1), None);
    write_manifest(&run_dir, &manifest)?;
    benchmarks.push(benchmark(
        "manifest_write",
        elapsed_ms(manifest_start),
        None,
    ));
    progress.emit(ProgressStage::Writing, 1, Some(1), None);
    let export_start = Instant::now();
    progress.emit(ProgressStage::Exporting, 0, Some(1), None);
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
    progress.emit(
        ProgressStage::Exporting,
        1,
        Some(1),
        Some(format!(
            "keep {}, reject {}, review {}, errors {}",
            manifest.summary.suggested_keep,
            manifest.summary.suggested_reject,
            manifest.summary.suggested_review,
            manifest.summary.errors
        )),
    );
    progress.emit(
        ProgressStage::Complete,
        1,
        Some(1),
        Some(run_dir.display().to_string()),
    );
    Ok(run_dir)
}

struct ScoreResult {
    asset: AssetRecord,
    similarity: SimilarityFeatures,
}

#[derive(Debug, Clone)]
struct IndexGroup {
    burst_id: usize,
    indices: Vec<usize>,
    similarity_confidence: f64,
    max_distance: f64,
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
    detector_engine: &DetectorEngine,
    thumbs_dir: Option<&Path>,
) -> ScoreResult {
    let mut timings = AssetTimings::default();
    let metadata = read_photo_metadata(&input.representative.path);
    let (capture_ms, capture_time_source) = if let Some(captured_at_ms) = metadata.captured_at_ms {
        (Some(captured_at_ms), "exif".to_string())
    } else if let Some(created_ms) = input.created_ms {
        (Some(created_ms), "filesystem_created".to_string())
    } else if let Some(modified_ms) = input.modified_ms {
        (Some(modified_ms), "filesystem_modified".to_string())
    } else {
        (None, "unavailable".to_string())
    };
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
        capture_ms,
        capture_time_source,
        width: 0,
        height: 0,
        decoder: String::new(),
        feature_backend: String::new(),
        metadata,
        metrics: QualityMetrics::default(),
        detector: None,
        timings,
        burst_id: 0,
        cluster_id: 0,
        similarity: SimilarityMetrics::default(),
        suggestion: Suggestion::default(),
        thumb: None,
        error: None,
    };
    let mut similarity = SimilarityFeatures::default();

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
            let descriptor_metrics = record.metrics.clone();

            let detector_start = Instant::now();
            let (detector, detector_notes) =
                detect_subject(&decoded.image, &record.metrics, detector_engine);
            timings.detector_ms = elapsed_ms(detector_start);
            if let Some(detector) = detector {
                merge_detector_metrics(&mut record.metrics, &detector);
                if detector.backend != "heuristic_saliency"
                    && subject_boxes_consistent(&descriptor_metrics, &record.metrics)
                {
                    update_subject_focus(&decoded.image, &mut record.metrics);
                }
                record.detector = Some(detector);
            }
            record.suggestion.explanations.extend(detector_notes);

            let similarity_start = Instant::now();
            similarity = similarity_features(&decoded.image, &descriptor_metrics);
            timings.feature_ms += elapsed_ms(similarity_start);
            record.similarity.subject_confidence = similarity.confidence;
            record.similarity.subject_area_fraction = similarity.area_fraction;

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
    ScoreResult {
        asset: record,
        similarity,
    }
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
    index_clusters: &[IndexGroup],
    options: &ScanOptions,
    pool: &rayon::ThreadPool,
    progress: &ProgressReporter,
) -> usize {
    if options.disable_refinement
        || options.refine_size <= options.preview_size
        || options.refine_candidates_per_cluster == 0
    {
        progress.emit(ProgressStage::Refining, 1, Some(1), None);
        return 0;
    }
    let candidates = refinement_candidates(assets, index_clusters, options);
    if candidates.is_empty() {
        progress.emit(ProgressStage::Refining, 1, Some(1), None);
        return 0;
    }
    progress.emit(
        ProgressStage::Refining,
        0,
        Some(candidates.len()),
        Some(format!("{}px long edge", options.refine_size)),
    );
    let refined = AtomicUsize::new(0);
    let total = candidates.len();
    let refinement_progress = progress.clone();
    let batch_size = refinement_batch_size(pool.current_num_threads(), options.refine_size);
    let mut results = Vec::with_capacity(candidates.len());
    for batch in candidates.chunks(batch_size) {
        results.extend(pool.install(|| {
            batch
                .par_iter()
                .map(|idx| {
                    let result = refine_asset(*idx, &assets[*idx], options);
                    let done = refined.fetch_add(1, Ordering::Relaxed) + 1;
                    refinement_progress.emit(
                        ProgressStage::Refining,
                        done,
                        Some(total),
                        Some(assets[*idx].representative.rel_path.clone()),
                    );
                    result
                })
                .collect::<Vec<_>>()
        }));
    }

    let mut applied = 0usize;
    for result in results.into_iter().flatten() {
        let asset = &mut assets[result.idx];
        asset.metrics = result.metrics;
        asset.feature_backend =
            format!("{}+refined_{}", result.feature_backend, options.refine_size);
        asset.timings.refine_decode_ms = result.decode_ms;
        asset.timings.refine_feature_ms = result.feature_ms;
        asset.suggestion.explanations.extend(result.notes);
        applied += 1;
    }
    applied
}

fn refinement_batch_size(worker_count: usize, long_edge: u32) -> usize {
    const MEMORY_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const ESTIMATED_WORKING_BYTES_PER_PIXEL: u64 = 32;
    let estimated_per_frame = u64::from(long_edge)
        .saturating_mul(u64::from(long_edge))
        .saturating_mul(ESTIMATED_WORKING_BYTES_PER_PIXEL)
        .max(1);
    (MEMORY_BUDGET_BYTES / estimated_per_frame)
        .max(1)
        .min(worker_count.max(1) as u64) as usize
}

fn refinement_candidates(
    assets: &[AssetRecord],
    index_clusters: &[IndexGroup],
    options: &ScanOptions,
) -> Vec<usize> {
    let mut selected = BTreeSet::new();
    for cluster in index_clusters {
        let indices = &cluster.indices;
        if indices.len() <= 1 {
            continue;
        }
        let keep_count = keep_count_for_cluster(indices.len(), options.keepers_per_cluster);
        let ranked = ranked_scores(assets, indices);
        let budget = options
            .refine_candidates_per_cluster
            .max(keep_count)
            .min(indices.len());
        for (idx, _) in ranked.iter().take(budget) {
            if assets[*idx].error.is_none() {
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
    let mut metrics = feature.metrics;
    if let Some(detector) = &asset.detector {
        let heuristic_metrics = metrics.clone();
        merge_detector_metrics(&mut metrics, detector);
        if detector.backend != "heuristic_saliency"
            && subject_boxes_consistent(&heuristic_metrics, &metrics)
        {
            update_subject_focus(&decoded.image, &mut metrics);
        }
    }
    let feature_ms = elapsed_ms(feature_start);
    Some(RefineResult {
        idx,
        metrics,
        feature_backend: feature.backend,
        notes: feature.notes,
        decode_ms,
        feature_ms,
    })
}

fn subject_boxes_consistent(left: &QualityMetrics, right: &QualityMetrics) -> bool {
    let intersection_width =
        (left.bbox_x2.min(right.bbox_x2) - left.bbox_x1.max(right.bbox_x1)).max(0.0);
    let intersection_height =
        (left.bbox_y2.min(right.bbox_y2) - left.bbox_y1.max(right.bbox_y1)).max(0.0);
    let intersection = intersection_width * intersection_height;
    let left_area = ((left.bbox_x2 - left.bbox_x1) * (left.bbox_y2 - left.bbox_y1)).max(0.0);
    let right_area = ((right.bbox_x2 - right.bbox_x1) * (right.bbox_y2 - right.bbox_y1)).max(0.0);
    let union = left_area + right_area - intersection;
    union > 0.0 && intersection / union >= 0.25
}

fn rank_clusters(
    assets: &mut [AssetRecord],
    index_clusters: Vec<IndexGroup>,
    index_bursts: &[Vec<usize>],
    options: &ScanOptions,
) -> (Vec<BurstCluster>, Vec<BurstSequence>) {
    let mut clusters = Vec::new();
    let mut cluster_ids_by_burst = vec![Vec::new(); index_bursts.len()];

    for (cluster_idx, group) in index_clusters.into_iter().enumerate() {
        let cluster_id = cluster_idx + 1;
        let indices = group.indices;
        cluster_ids_by_burst[group.burst_id - 1].push(cluster_id);
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
            asset.burst_id = group.burst_id;
            let action;
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
                reason = "distinct frame".to_string();
            } else if rank <= keep_count {
                action = SuggestedAction::Keep;
                reason = format!("best quality in stack {cluster_id}");
            } else if asset.similarity.duplicate_confidence < options.min_duplicate_confidence {
                action = SuggestedAction::Review;
                reason = format!("similarity is uncertain in stack {cluster_id}");
            } else if *score >= keep_threshold - 0.035 {
                action = SuggestedAction::Review;
                reason = format!("near quality tie in stack {cluster_id}");
            } else {
                action = SuggestedAction::Reject;
                reason = format!("high-confidence duplicate in stack {cluster_id}");
            }
            let mut explanations = std::mem::take(&mut asset.suggestion.explanations);
            explanations.extend(explanations_for(asset, cluster_len, keep_count, rank));
            asset.suggestion = Suggestion {
                action,
                rank,
                score: *score,
                reason,
                explanations,
            };
        }

        let times: Vec<i64> = indices
            .iter()
            .filter_map(|idx| assets[*idx].time_key_ms())
            .collect();
        let first = indices.first().map(|idx| &assets[*idx]);
        clusters.push(BurstCluster {
            id: cluster_id,
            burst_id: group.burst_id,
            asset_ids: indices.iter().map(|idx| assets[*idx].id.clone()).collect(),
            directory: first.map(|a| a.directory.clone()).unwrap_or_default(),
            prefix: first.map(|a| a.prefix.clone()).unwrap_or_default(),
            start_ms: times.iter().min().copied(),
            end_ms: times.iter().max().copied(),
            keep_count,
            best_asset_id,
            similarity_confidence: group.similarity_confidence,
            max_distance: group.max_distance,
        });
    }

    let bursts = index_bursts
        .iter()
        .enumerate()
        .map(|(burst_zero, indices)| {
            let times: Vec<i64> = indices
                .iter()
                .filter_map(|idx| assets[*idx].time_key_ms())
                .collect();
            let first = indices.first().map(|idx| &assets[*idx]);
            BurstSequence {
                id: burst_zero + 1,
                asset_ids: indices.iter().map(|idx| assets[*idx].id.clone()).collect(),
                cluster_ids: cluster_ids_by_burst[burst_zero].clone(),
                directory: first
                    .map(|asset| asset.directory.clone())
                    .unwrap_or_default(),
                prefix: first.map(|asset| asset.prefix.clone()).unwrap_or_default(),
                start_ms: times.iter().min().copied(),
                end_ms: times.iter().max().copied(),
            }
        })
        .collect();

    assets.sort_by(|a, b| {
        (
            a.burst_id,
            a.cluster_id,
            a.suggestion.rank,
            &a.directory,
            &a.prefix,
            a.seq.unwrap_or(-1),
            &a.stem,
        )
            .cmp(&(
                b.burst_id,
                b.cluster_id,
                b.suggestion.rank,
                &b.directory,
                &b.prefix,
                b.seq.unwrap_or(-1),
                &b.stem,
            ))
    });
    (clusters, bursts)
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
    let subject_sharp_norms = norm(
        indices
            .iter()
            .map(|idx| assets[*idx].metrics.subject_sharpness.max(0.0).ln_1p())
            .collect(),
    );
    let subject_ten_norms = norm(
        indices
            .iter()
            .map(|idx| assets[*idx].metrics.subject_tenengrad.max(0.0).ln_1p())
            .collect(),
    );

    let mut ranked: Vec<(usize, f64)> = indices
        .iter()
        .enumerate()
        .map(|(pos, idx)| {
            let asset = &assets[*idx];
            let global_focus = 0.70 * sharp_norms[pos] + 0.30 * ten_norms[pos];
            let subject_focus = 0.70 * subject_sharp_norms[pos] + 0.30 * subject_ten_norms[pos];
            let sharp_component = if asset.similarity.subject_confidence >= 0.3 {
                0.72 * subject_focus + 0.28 * global_focus
            } else {
                global_focus
            };
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
    0.60 * sharp_component
        + 0.18 * asset.metrics.completeness
        + 0.10 * asset.metrics.contrast
        + 0.07 * asset.metrics.object_confidence
        + 0.05 * asset.metrics.exposure_score
        - 0.08 * (asset.metrics.border_energy_fraction / 0.35).min(1.0)
}

fn build_bursts(assets: &[AssetRecord], options: &ScanOptions) -> Vec<Vec<usize>> {
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
            should_split_burst(prev, asset, cluster_start_ms, options)
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

fn should_split_burst(
    prev: &AssetRecord,
    current: &AssetRecord,
    cluster_start_ms: Option<i64>,
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
    false
}

fn build_near_duplicate_groups(
    assets: &mut [AssetRecord],
    features: &[SimilarityFeatures],
    bursts: &[Vec<usize>],
    options: &ScanOptions,
) -> Vec<IndexGroup> {
    let mut groups = Vec::new();
    for (burst_zero, burst) in bursts.iter().enumerate() {
        let burst_id = burst_zero + 1;
        let mut current: Vec<usize> = Vec::new();
        let mut comparison_cache = HashMap::new();
        for idx in burst {
            assets[*idx].burst_id = burst_id;
            assets[*idx].similarity.subject_confidence = features[*idx].confidence;
            assets[*idx].similarity.subject_area_fraction = features[*idx].area_fraction;
            let split = if let Some(previous) = current.last().copied() {
                let invalid = assets[*idx].error.is_some() || assets[previous].error.is_some();
                let scene_change =
                    hash_distance(&assets[previous].metrics.dhash, &assets[*idx].metrics.dhash)
                        > options.max_hash_gap;
                if invalid || scene_change {
                    true
                } else {
                    !fits_complete_link(
                        &current,
                        *idx,
                        features,
                        options.max_duplicate_distance,
                        &mut comparison_cache,
                    )
                }
            } else {
                false
            };
            if split && !current.is_empty() {
                groups.push(finalize_similarity_group(
                    burst_id,
                    std::mem::take(&mut current),
                    assets,
                    features,
                    options.max_duplicate_distance,
                    &mut comparison_cache,
                ));
            }
            current.push(*idx);
        }
        if !current.is_empty() {
            groups.push(finalize_similarity_group(
                burst_id,
                current,
                assets,
                features,
                options.max_duplicate_distance,
                &mut comparison_cache,
            ));
        }
    }
    groups
}

fn fits_complete_link(
    current: &[usize],
    candidate: usize,
    features: &[SimilarityFeatures],
    threshold: f64,
    cache: &mut HashMap<(usize, usize), SimilarityComparison>,
) -> bool {
    current
        .iter()
        .all(|member| cached_similarity(*member, candidate, features, cache).distance <= threshold)
}

fn finalize_similarity_group(
    burst_id: usize,
    indices: Vec<usize>,
    assets: &mut [AssetRecord],
    features: &[SimilarityFeatures],
    threshold: f64,
    cache: &mut HashMap<(usize, usize), SimilarityComparison>,
) -> IndexGroup {
    let mut confidence_sum = 0.0;
    let mut max_distance: f64 = 0.0;
    for (position, idx) in indices.iter().enumerate() {
        if indices.len() == 1 {
            assets[*idx].similarity.nearest_distance = 1.0;
            assets[*idx].similarity.duplicate_confidence = 0.0;
            assets[*idx].similarity.pose_novelty = 1.0;
            continue;
        }
        let mut nearest = None;
        for (other_position, other) in indices.iter().enumerate() {
            if position == other_position {
                continue;
            }
            let comparison = cached_similarity(*idx, *other, features, cache);
            max_distance = max_distance.max(comparison.distance);
            if nearest
                .as_ref()
                .is_none_or(|current: &crate::features::SimilarityComparison| {
                    comparison.distance < current.distance
                })
            {
                nearest = Some(comparison);
            }
        }
        if let Some(nearest) = nearest {
            let distance_ratio = (nearest.distance / threshold.max(1e-9)).clamp(0.0, 1.0);
            let duplicate_confidence = nearest.confidence * (1.0 - 0.35 * distance_ratio);
            assets[*idx].similarity.nearest_distance = nearest.distance;
            assets[*idx].similarity.nearest_subject_distance = nearest.subject_distance;
            assets[*idx].similarity.nearest_global_distance = nearest.global_distance;
            assets[*idx].similarity.duplicate_confidence = duplicate_confidence;
            assets[*idx].similarity.pose_novelty = distance_ratio;
            confidence_sum += duplicate_confidence;
        }
    }
    let similarity_confidence = if indices.len() > 1 {
        confidence_sum / indices.len() as f64
    } else {
        0.0
    };
    IndexGroup {
        burst_id,
        indices,
        similarity_confidence,
        max_distance,
    }
}

fn cached_similarity(
    left: usize,
    right: usize,
    features: &[SimilarityFeatures],
    cache: &mut HashMap<(usize, usize), SimilarityComparison>,
) -> SimilarityComparison {
    let key = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    *cache
        .entry(key)
        .or_insert_with(|| compare_similarity(&features[left], &features[right]))
}

fn explanations_for(
    asset: &AssetRecord,
    cluster_len: usize,
    keep_count: usize,
    rank: usize,
) -> Vec<String> {
    let mut explanations = Vec::new();
    explanations.push(format!(
        "Ranked {} of {} in near-duplicate stack {}; keeper count is {}.",
        rank, cluster_len, asset.cluster_id, keep_count
    ));
    explanations.push(format!(
        "Burst {}; subject confidence {:.2}, nearest visual distance {:.3}, duplicate confidence {:.2}.",
        asset.burst_id,
        asset.similarity.subject_confidence,
        asset.similarity.nearest_distance,
        asset.similarity.duplicate_confidence
    ));
    explanations.push(format!(
        "Whole-frame sharpness {:.1}; subject sharpness {:.1}; contrast {:.2}.",
        asset.metrics.sharpness, asset.metrics.subject_sharpness, asset.metrics.contrast
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
    1
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
        .clamp(1, 8)
}

fn acceleration_report(
    requested: AccelerationPreference,
    worker_count: usize,
    assets: &[AssetRecord],
) -> AccelerationReport {
    let mut capabilities = vec![
        format!("rayon_cpu_workers:{worker_count}"),
        "cpu_scalar_focus_scoring".to_string(),
    ];
    let mut notes = vec![
        "Portable scalar focus scoring and Rayon asset workers remain available on every platform."
            .to_string(),
    ];
    #[cfg(all(
        target_os = "linux",
        feature = "avx2-accel",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    {
        capabilities.push("avx2_focus_scoring_compiled".to_string());
        if auto_cpu_backend() == "cpu_avx2" {
            capabilities.push("avx2_focus_scoring".to_string());
        } else {
            notes.push(
                "The AVX2 scorer is compiled in, but this CPU does not advertise AVX2; scalar scoring will be used."
                    .to_string(),
            );
        }
    }
    #[cfg(all(target_os = "linux", feature = "neon-accel", target_arch = "aarch64"))]
    {
        capabilities.push("neon_focus_scoring_compiled".to_string());
        if auto_cpu_backend() == "cpu_neon" {
            capabilities.push("neon_focus_scoring".to_string());
        } else {
            notes.push(
                "The NEON scorer is compiled in, but this CPU does not advertise NEON; scalar scoring will be used."
                    .to_string(),
            );
        }
    }
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

    #[cfg(all(target_os = "linux", feature = "cuda-accel"))]
    {
        capabilities.push("cuda_focus_scoring_compiled".to_string());
        if requested == AccelerationPreference::Cuda {
            let status = crate::cuda_accel::status();
            if status.available {
                capabilities.push("cuda_focus_scoring".to_string());
                if let Some(device_name) = status.device_name {
                    capabilities.push(format!("cuda_device:{device_name}"));
                }
            }
            if let Some(note) = status.note {
                notes.push(note);
            }
        } else if requested == AccelerationPreference::Auto {
            notes.push(
                "CUDA is opt-in while runtime parity and throughput validation is pending; pass --acceleration cuda to request it."
                    .to_string(),
            );
        }
    }

    #[cfg(not(all(target_os = "linux", feature = "cuda-accel")))]
    if requested == AccelerationPreference::Cuda {
        notes.push(
            "CUDA was requested, but this binary was built without Linux CUDA support; CPU fallback was used."
                .to_string(),
        );
    }

    if requested == AccelerationPreference::Metal && !metal_available {
        if cfg!(all(target_os = "macos", feature = "metal-accel")) {
            notes.push("Metal was requested but no usable Metal scorer initialized at runtime; falling back to native CPU scoring.".to_string());
        } else {
            notes.push("Metal was requested but this build cannot compile the Metal scorer on the current platform; native CPU fallback was used.".to_string());
        }
    }
    if requested == AccelerationPreference::OpenCl {
        notes.push(
            "OpenCL was requested, but no OpenCL adapter is bundled; native CPU fallback was used."
                .to_string(),
        );
    }
    if requested == AccelerationPreference::Avx2 && auto_cpu_backend() != "cpu_avx2" {
        notes.push(format!(
            "AVX2 was requested but is unavailable at runtime; {} was used.",
            auto_cpu_backend()
        ));
    }
    if requested == AccelerationPreference::Neon && auto_cpu_backend() != "cpu_neon" {
        notes.push(format!(
            "NEON was requested but is unavailable at runtime; {} was used.",
            auto_cpu_backend()
        ));
    }

    let mut usage = BTreeMap::new();
    for asset in assets {
        let backend = asset
            .feature_backend
            .split("+refined_")
            .next()
            .unwrap_or_default();
        if !backend.is_empty() {
            *usage.entry(backend.to_string()).or_insert(0usize) += 1;
        }
    }
    notes.push(format!(
        "Final per-asset focus backend usage: {}.",
        if usage.is_empty() {
            "none".to_string()
        } else {
            usage
                .iter()
                .map(|(backend, count)| format!("{backend}={count}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));

    let cuda_count = usage.get("cuda").copied().unwrap_or(0);
    let metal_count = usage.get("metal").copied().unwrap_or(0);
    let cpu_count = usage
        .iter()
        .filter(|(backend, _)| backend.starts_with("cpu_"))
        .map(|(_, count)| *count)
        .sum::<usize>();
    let selected = if cuda_count > 0 {
        if cpu_count > 0 {
            "cuda_focus_with_cpu_fallback"
        } else {
            "cuda_focus_cpu_rest"
        }
    } else if metal_count > 0 {
        if cpu_count > 0 {
            "metal_focus_with_cpu_fallback"
        } else {
            "metal_focus_cpu_rest"
        }
    } else if usage.contains_key("cpu_avx2") {
        "cpu_avx2_rayon"
    } else if usage.contains_key("cpu_neon") {
        "cpu_neon_rayon"
    } else if usage.contains_key("cpu_scalar") || usage.contains_key("cpu_small_image") {
        "cpu_scalar_rayon"
    } else {
        match requested {
            AccelerationPreference::Cpu => "cpu_scalar_rayon",
            _ if auto_cpu_backend() == "cpu_avx2" => "cpu_avx2_rayon",
            _ if auto_cpu_backend() == "cpu_neon" => "cpu_neon_rayon",
            _ => "cpu_scalar_rayon",
        }
    };
    AccelerationReport {
        requested,
        selected: selected.to_string(),
        capabilities,
        notes,
    }
}

fn auto_cpu_backend() -> &'static str {
    #[cfg(all(
        target_os = "linux",
        any(feature = "avx2-accel", feature = "neon-accel")
    ))]
    {
        crate::cpu_accel::backend_name()
    }

    #[cfg(not(all(
        target_os = "linux",
        any(feature = "avx2-accel", feature = "neon-accel")
    )))]
    {
        "cpu_scalar"
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{fits_complete_link, keep_count_for_cluster, refinement_batch_size};
    use crate::features::SimilarityFeatures;

    fn features(mask_start: usize) -> SimilarityFeatures {
        let mut mask = vec![0u8; 20];
        for value in &mut mask[mask_start..mask_start + 10] {
            *value = 1;
        }
        SimilarityFeatures {
            global_luma: vec![0, 64, 128, 255],
            subject_luma: vec![0, 64, 128, 255],
            subject_edges: vec![32; 20],
            subject_mask: mask,
            confidence: 1.0,
            area_fraction: 0.02,
        }
    }

    #[test]
    fn complete_link_prevents_similarity_chaining() {
        let features = vec![features(0), features(5), features(10)];
        let mut cache = HashMap::new();
        assert!(fits_complete_link(&[0], 1, &features, 0.32, &mut cache));
        assert!(fits_complete_link(&[1], 2, &features, 0.32, &mut cache));
        assert!(!fits_complete_link(&[0, 1], 2, &features, 0.32, &mut cache));
    }

    #[test]
    fn near_duplicate_stacks_default_to_one_keeper() {
        assert_eq!(keep_count_for_cluster(1, None), 1);
        assert_eq!(keep_count_for_cluster(120, None), 1);
        assert_eq!(keep_count_for_cluster(5, Some(2)), 2);
    }

    #[test]
    fn high_resolution_refinement_respects_the_memory_budget() {
        assert_eq!(refinement_batch_size(8, 2048), 8);
        assert_eq!(refinement_batch_size(8, 4096), 4);
        assert_eq!(refinement_batch_size(8, 8192), 1);
    }
}
