use std::path::PathBuf;

pub use burst_core::QualityMetrics;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanOptions {
    pub preview_size: u32,
    pub refine_size: u32,
    pub refine_candidates_per_cluster: usize,
    pub disable_refinement: bool,
    pub thumb_size: u32,
    pub max_seq_gap: i64,
    pub max_time_gap_ms: i64,
    pub max_cluster_span_ms: i64,
    pub max_hash_gap: u32,
    pub max_duplicate_distance: f64,
    pub min_duplicate_confidence: f64,
    pub keepers_per_cluster: Option<usize>,
    pub cull_singletons: bool,
    pub workers: Option<usize>,
    pub acceleration: AccelerationPreference,
    pub detector: DetectorPreference,
    pub generate_thumbnails: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            preview_size: 1280,
            refine_size: 2048,
            refine_candidates_per_cluster: 2,
            disable_refinement: false,
            thumb_size: 320,
            max_seq_gap: 12,
            max_time_gap_ms: 1250,
            max_cluster_span_ms: 1800,
            max_hash_gap: 30,
            max_duplicate_distance: 0.20,
            min_duplicate_confidence: 0.52,
            keepers_per_cluster: None,
            cull_singletons: false,
            workers: None,
            acceleration: AccelerationPreference::Auto,
            detector: DetectorPreference::Auto,
            generate_thumbnails: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccelerationPreference {
    Auto,
    Cpu,
    Avx2,
    Metal,
    Cuda,
    OpenCl,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectorPreference {
    Auto,
    Off,
    Heuristic,
    Vision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationReport {
    pub requested: AccelerationPreference,
    pub selected: String,
    pub capabilities: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoderReport {
    pub native_compressed: bool,
    #[serde(default)]
    pub scaled_jpeg: bool,
    pub imagemagick: Option<PathBuf>,
    pub sips: Option<PathBuf>,
    pub raw_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorReport {
    pub requested: DetectorPreference,
    pub selected: String,
    pub capabilities: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub stage: String,
    pub elapsed_ms: f64,
    pub items: Option<usize>,
    pub items_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub app_version: String,
    pub root: PathBuf,
    pub created_at: String,
    pub options: ScanOptions,
    pub acceleration: AccelerationReport,
    pub detector: DetectorReport,
    pub decoders: DecoderReport,
    pub benchmarks: Vec<BenchmarkReport>,
    pub summary: Summary,
    #[serde(default)]
    pub bursts: Vec<BurstSequence>,
    pub clusters: Vec<BurstCluster>,
    pub assets: Vec<AssetRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Summary {
    pub discovered_assets: usize,
    pub image_files: usize,
    pub sidecar_files: usize,
    pub clusters: usize,
    pub bursts: usize,
    pub suggested_keep: usize,
    pub suggested_reject: usize,
    pub suggested_review: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRecord {
    pub id: String,
    pub representative: FileEntry,
    pub files: Vec<FileEntry>,
    pub sidecars: Vec<FileEntry>,
    pub directory: String,
    pub stem: String,
    pub prefix: String,
    pub seq: Option<i64>,
    pub created_ms: Option<i64>,
    pub modified_ms: Option<i64>,
    pub capture_ms: Option<i64>,
    #[serde(default)]
    pub capture_time_source: String,
    pub width: u32,
    pub height: u32,
    pub decoder: String,
    pub feature_backend: String,
    #[serde(default)]
    pub metadata: PhotoMetadata,
    pub metrics: QualityMetrics,
    pub detector: Option<DetectorOutput>,
    pub timings: AssetTimings,
    #[serde(default)]
    pub burst_id: usize,
    pub cluster_id: usize,
    #[serde(default)]
    pub similarity: SimilarityMetrics,
    pub suggestion: Suggestion,
    pub thumb: Option<String>,
    pub error: Option<String>,
}

impl AssetRecord {
    pub fn time_key_ms(&self) -> Option<i64> {
        self.capture_ms.or(self.created_ms).or(self.modified_ms)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub rel_path: String,
    pub kind: FileKind,
    pub extension: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Raw,
    Compressed,
    Sidecar,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PhotoMetadata {
    pub iso: Option<u32>,
    pub aperture: Option<f64>,
    pub shutter_s: Option<f64>,
    pub shutter: Option<String>,
    pub focal_length_mm: Option<f64>,
    pub focal_length_35mm: Option<u32>,
    pub captured_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SimilarityMetrics {
    pub subject_confidence: f64,
    pub subject_area_fraction: f64,
    pub nearest_distance: f64,
    pub nearest_subject_distance: f64,
    pub nearest_global_distance: f64,
    pub duplicate_confidence: f64,
    pub pose_novelty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorOutput {
    pub backend: String,
    pub confidence: f64,
    pub subject_count: usize,
    pub truncation_risk: f64,
    pub bbox_x1: f64,
    pub bbox_y1: f64,
    pub bbox_x2: f64,
    pub bbox_y2: f64,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AssetTimings {
    pub decode_ms: f64,
    pub feature_ms: f64,
    pub refine_decode_ms: f64,
    pub refine_feature_ms: f64,
    pub detector_ms: f64,
    pub thumbnail_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurstCluster {
    pub id: usize,
    #[serde(default)]
    pub burst_id: usize,
    pub asset_ids: Vec<String>,
    pub directory: String,
    pub prefix: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub keep_count: usize,
    pub best_asset_id: Option<String>,
    #[serde(default)]
    pub similarity_confidence: f64,
    #[serde(default)]
    pub max_distance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurstSequence {
    pub id: usize,
    pub asset_ids: Vec<String>,
    pub cluster_ids: Vec<usize>,
    pub directory: String,
    pub prefix: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub action: SuggestedAction,
    pub rank: usize,
    pub score: f64,
    pub reason: String,
    pub explanations: Vec<String>,
}

impl Default for Suggestion {
    fn default() -> Self {
        Self {
            action: SuggestedAction::Review,
            rank: 0,
            score: 0.0,
            reason: String::new(),
            explanations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedAction {
    Keep,
    Reject,
    Review,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewState {
    pub run_created_at: String,
    pub updated_at: String,
    pub decisions: Vec<ReviewDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDecision {
    pub asset_id: String,
    pub decision: Option<UserDecision>,
    pub note: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserDecision {
    Keep,
    Reject,
    Review,
}

impl UserDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Reject => "reject",
            Self::Review => "review",
        }
    }
}
