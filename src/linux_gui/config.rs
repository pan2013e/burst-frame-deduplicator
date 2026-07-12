use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::types::ScanOptions;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TutorialOutcome {
    Completed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TutorialProgress {
    pub schema_version: u32,
    pub outcome: TutorialOutcome,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiConfig {
    pub locale: String,
    pub appearance: Appearance,
    pub results_root: PathBuf,
    pub reject_destination: Option<PathBuf>,
    pub model_pack: Option<PathBuf>,
    pub options: ScanOptions,
    pub recent_runs: Vec<PathBuf>,
    pub tutorial_progress: Option<TutorialProgress>,
    #[serde(default, rename = "tutorial_finished", skip_serializing)]
    legacy_tutorial_finished: bool,
    pub window_width: i32,
    pub window_height: i32,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            locale: default_locale(),
            appearance: Appearance::System,
            results_root: default_results_root(),
            reject_destination: None,
            model_pack: None,
            options: ScanOptions::default(),
            recent_runs: Vec::new(),
            tutorial_progress: None,
            legacy_tutorial_finished: false,
            window_width: 1180,
            window_height: 780,
        }
    }
}

impl GuiConfig {
    pub fn load() -> Self {
        let path = config_path();
        let mut config: Self = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if config.tutorial_progress.is_none() && config.legacy_tutorial_finished {
            config.record_tutorial(TutorialOutcome::Completed);
        }
        config
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        let parent = path
            .parent()
            .context("Linux GUI config path has no parent")?;
        fs::create_dir_all(parent)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, self)?;
        temporary.write_all(b"\n")?;
        temporary.as_file_mut().sync_all()?;
        temporary
            .persist(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("saving {}", path.display()))?;
        Ok(())
    }

    pub fn register_run(&mut self, run_dir: PathBuf) {
        self.recent_runs.retain(|existing| existing != &run_dir);
        self.recent_runs.insert(0, run_dir);
        self.recent_runs.truncate(20);
    }

    pub fn tutorial_finished(&self) -> bool {
        self.tutorial_progress.is_some()
    }

    pub fn record_tutorial(&mut self, outcome: TutorialOutcome) {
        self.tutorial_progress = Some(TutorialProgress {
            schema_version: 1,
            outcome,
            updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        });
        self.legacy_tutorial_finished = false;
    }
}

pub fn config_path() -> PathBuf {
    if let Some(root) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(root)
            .join("burst-frame-deduplicator")
            .join("config.json");
    }
    home_dir()
        .join(".config")
        .join("burst-frame-deduplicator")
        .join("config.json")
}

pub fn default_results_root() -> PathBuf {
    home_dir()
        .join("Pictures")
        .join("Burst Frame Deduplicator Runs")
}

pub fn cache_bytes(run_dir: &Path) -> u64 {
    walkdir::WalkDir::new(run_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

fn default_locale() -> String {
    let language = std::env::var("LANG").unwrap_or_default();
    if language.to_ascii_lowercase().starts_with("zh") {
        "zh-CN".to_string()
    } else {
        "en".to_string()
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::{GuiConfig, TutorialOutcome};

    #[test]
    fn config_round_trip_keeps_tutorial_outcome_and_paths() {
        let mut config = GuiConfig::default();
        config.record_tutorial(TutorialOutcome::Skipped);
        config.register_run("/tmp/example-run".into());
        let encoded = serde_json::to_vec(&config).unwrap();
        let decoded: GuiConfig = serde_json::from_slice(&encoded).unwrap();
        assert!(decoded.tutorial_finished());
        assert_eq!(
            decoded.tutorial_progress.unwrap().outcome,
            TutorialOutcome::Skipped
        );
        assert_eq!(
            decoded.recent_runs[0],
            std::path::Path::new("/tmp/example-run")
        );
    }

    #[test]
    fn legacy_tutorial_flag_deserializes_for_migration() {
        let decoded: GuiConfig = serde_json::from_str(r#"{"tutorial_finished":true}"#).unwrap();
        assert!(decoded.legacy_tutorial_finished);
    }
}
