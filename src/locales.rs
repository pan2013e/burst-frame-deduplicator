use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};

pub const SUPPORTED_LOCALES: &[&str] = &["en", "zh-CN"];

pub fn locale_directory() -> anyhow::Result<PathBuf> {
    candidate_directories()
        .into_iter()
        .find(|candidate| locale_files_exist(candidate))
        .ok_or_else(|| {
            anyhow!(
                "locale files were not found; set BURST_DEDUP_LOCALES_DIR or install the locales directory"
            )
        })
}

pub fn read_locale(code: &str) -> anyhow::Result<Vec<u8>> {
    if !SUPPORTED_LOCALES.contains(&code) {
        return Err(anyhow!("unsupported locale: {code}"));
    }
    if let Ok(directory) = locale_directory() {
        let path = directory.join(format!("{code}.json"));
        return std::fs::read(&path).with_context(|| format!("reading {}", path.display()));
    }
    Ok(embedded_locale(code).to_vec())
}

fn embedded_locale(code: &str) -> &'static [u8] {
    match code {
        "zh-CN" => include_bytes!("../locales/zh-CN.json"),
        _ => include_bytes!("../locales/en.json"),
    }
}

fn candidate_directories() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(configured) = std::env::var_os("BURST_DEDUP_LOCALES_DIR") {
        candidates.push(PathBuf::from(configured));
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join("../Resources/locales"));
        candidates.push(directory.join("locales"));
    }
    if let Ok(current) = std::env::current_dir() {
        candidates.push(current.join("locales"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales"));
    candidates
}

fn locale_files_exist(directory: &Path) -> bool {
    SUPPORTED_LOCALES
        .iter()
        .all(|code| directory.join(format!("{code}.json")).is_file())
}

#[cfg(test)]
mod tests {
    use super::{embedded_locale, read_locale};

    #[test]
    fn rejects_unknown_locale_names() {
        assert!(read_locale("../../secret").is_err());
    }

    #[test]
    fn embeds_every_supported_locale_for_standalone_binaries() {
        for code in super::SUPPORTED_LOCALES {
            let parsed: serde_json::Value = serde_json::from_slice(embedded_locale(code)).unwrap();
            assert_eq!(parsed["locale"], *code);
            assert!(parsed["reviewWeb"].is_object());
        }
    }
}
