use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStage {
    Preparing,
    Discovering,
    Analyzing,
    Grouping,
    Refining,
    Ranking,
    Writing,
    Exporting,
    Complete,
    ReadingManifest,
    LoadingDecisions,
    LoadingMoveHistory,
    PreparingReview,
}

impl ProgressStage {
    pub fn locale_key(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Discovering => "discovering",
            Self::Analyzing => "analyzing",
            Self::Grouping => "grouping",
            Self::Refining => "refining",
            Self::Ranking => "ranking",
            Self::Writing => "writing",
            Self::Exporting => "exporting",
            Self::Complete => "complete",
            Self::ReadingManifest => "reading_manifest",
            Self::LoadingDecisions => "loading_decisions",
            Self::LoadingMoveHistory => "loading_move_history",
            Self::PreparingReview => "preparing_review",
        }
    }

    pub fn english_label(self) -> &'static str {
        match self {
            Self::Preparing => "Preparing scan",
            Self::Discovering => "Discovering photos",
            Self::Analyzing => "Analyzing previews",
            Self::Grouping => "Grouping bursts and stacks",
            Self::Refining => "Refining quality candidates",
            Self::Ranking => "Ranking suggestions",
            Self::Writing => "Writing run manifest",
            Self::Exporting => "Exporting review files",
            Self::Complete => "Scan complete",
            Self::ReadingManifest => "Reading run manifest",
            Self::LoadingDecisions => "Loading review decisions",
            Self::LoadingMoveHistory => "Loading move history",
            Self::PreparingReview => "Preparing review",
        }
    }

    fn overall_bounds(self) -> (f32, f32) {
        match self {
            Self::Preparing => (0.00, 0.02),
            Self::Discovering => (0.02, 0.08),
            Self::Analyzing => (0.08, 0.68),
            Self::Grouping => (0.68, 0.73),
            Self::Refining => (0.73, 0.90),
            Self::Ranking => (0.90, 0.93),
            Self::Writing => (0.93, 0.96),
            Self::Exporting => (0.96, 0.99),
            Self::Complete => (1.00, 1.00),
            Self::ReadingManifest => (0.00, 0.50),
            Self::LoadingDecisions => (0.50, 0.72),
            Self::LoadingMoveHistory => (0.72, 0.88),
            Self::PreparingReview => (0.88, 1.00),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCancelled;

impl fmt::Display for ScanCancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("scan cancelled")
    }
}

impl Error for ScanCancelled {}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn check(&self) -> Result<(), ScanCancelled> {
        if self.is_cancelled() {
            Err(ScanCancelled)
        } else {
            Ok(())
        }
    }
}

pub fn is_scan_cancelled(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ScanCancelled>().is_some()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub stage: ProgressStage,
    pub current: usize,
    pub total: Option<usize>,
    pub stage_fraction: Option<f32>,
    pub overall_fraction: f32,
    pub detail: Option<String>,
}

impl ProgressUpdate {
    pub fn new(
        stage: ProgressStage,
        current: usize,
        total: Option<usize>,
        detail: Option<String>,
    ) -> Self {
        let stage_fraction = total.map(|total| {
            if total == 0 {
                1.0
            } else {
                (current as f32 / total as f32).clamp(0.0, 1.0)
            }
        });
        let (start, end) = stage.overall_bounds();
        let fraction = stage_fraction.unwrap_or(0.0);
        Self {
            stage,
            current,
            total,
            stage_fraction,
            overall_fraction: (start + (end - start) * fraction).clamp(0.0, 1.0),
            detail,
        }
    }
}

#[derive(Clone, Default)]
pub struct ProgressReporter {
    callback: Option<Arc<dyn Fn(ProgressUpdate) + Send + Sync>>,
}

impl ProgressReporter {
    pub fn new(callback: impl Fn(ProgressUpdate) + Send + Sync + 'static) -> Self {
        Self {
            callback: Some(Arc::new(callback)),
        }
    }

    pub fn emit(
        &self,
        stage: ProgressStage,
        current: usize,
        total: Option<usize>,
        detail: Option<String>,
    ) {
        if let Some(callback) = &self.callback {
            callback(ProgressUpdate::new(stage, current, total, detail));
        }
    }
}

#[derive(Debug)]
struct TerminalState {
    stage: Option<ProgressStage>,
    last_percent: usize,
    last_emit: Instant,
}

pub fn terminal_progress_reporter() -> ProgressReporter {
    let state = Arc::new(Mutex::new(TerminalState {
        stage: None,
        last_percent: usize::MAX,
        last_emit: Instant::now() - Duration::from_secs(2),
    }));
    ProgressReporter::new(move |update| {
        let percent = (update.overall_fraction * 100.0).round() as usize;
        let mut state = state.lock();
        let stage_changed = state.stage != Some(update.stage);
        let completed_stage = update.total.is_some_and(|total| update.current >= total);
        let advanced = state.last_percent == usize::MAX || percent >= state.last_percent + 2;
        let elapsed = state.last_emit.elapsed() >= Duration::from_secs(1);
        if !stage_changed && !completed_stage && !advanced && !elapsed {
            return;
        }

        let item_progress = match update.total {
            Some(total) => format!(" {}/{}", update.current, total),
            None if update.current > 0 => format!(" {} found", update.current),
            None => String::new(),
        };
        let detail = update
            .detail
            .as_deref()
            .filter(|detail| !detail.is_empty())
            .map(|detail| format!(" - {detail}"))
            .unwrap_or_default();
        eprintln!(
            "[{percent:>3}%] {}{item_progress}{detail}",
            update.stage.english_label()
        );
        state.stage = Some(update.stage);
        state.last_percent = percent;
        state.last_emit = Instant::now();
    })
}

#[cfg(test)]
mod tests {
    use super::{CancellationToken, ProgressStage, ProgressUpdate};

    #[test]
    fn stage_progress_maps_to_monotonic_overall_progress() {
        let discovery = ProgressUpdate::new(ProgressStage::Discovering, 1, Some(2), None);
        let analysis = ProgressUpdate::new(ProgressStage::Analyzing, 0, Some(10), None);
        let complete = ProgressUpdate::new(ProgressStage::Complete, 1, Some(1), None);
        assert!(discovery.overall_fraction < analysis.overall_fraction);
        assert_eq!(complete.overall_fraction, 1.0);
    }

    #[test]
    fn cancellation_tokens_share_state_across_clones() {
        let token = CancellationToken::new();
        let worker = token.clone();
        assert!(worker.check().is_ok());
        token.cancel();
        assert!(worker.check().is_err());
    }
}
