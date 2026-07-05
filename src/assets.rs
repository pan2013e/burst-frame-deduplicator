use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use sha1::{Digest, Sha1};
use walkdir::WalkDir;

use crate::types::{FileEntry, FileKind};

pub const RAW_EXTS: &[&str] = &[
    "3fr", "arw", "cr2", "cr3", "dcr", "dng", "erf", "fff", "iiq", "kdc", "mef", "mos", "mrw",
    "nef", "nrw", "orf", "pef", "raf", "raw", "rw2", "rwl", "sr2", "srf", "x3f",
];

pub const COMPRESSED_EXTS: &[&str] = &[
    "avif", "bmp", "gif", "heic", "heif", "jpeg", "jpg", "jxl", "png", "tif", "tiff", "webp",
];

pub const SIDECAR_EXTS: &[&str] = &["aae", "dop", "json", "pp3", "xmp"];

#[derive(Debug, Clone)]
pub struct AssetInput {
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
}

impl AssetInput {
    pub fn time_key_ms(&self) -> Option<i64> {
        self.created_ms.or(self.modified_ms)
    }
}

#[derive(Default)]
struct AssetBuilder {
    files: Vec<FileEntry>,
    sidecars: Vec<FileEntry>,
}

pub fn discover_assets(root: &Path) -> anyhow::Result<Vec<AssetInput>> {
    let mut grouped: BTreeMap<(String, String), AssetBuilder> = BTreeMap::new();

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("._"))
        {
            continue;
        }
        let Some(ext) = extension(&path) else {
            continue;
        };
        let kind = if RAW_EXTS.contains(&ext.as_str()) {
            FileKind::Raw
        } else if COMPRESSED_EXTS.contains(&ext.as_str()) {
            FileKind::Compressed
        } else if SIDECAR_EXTS.contains(&ext.as_str()) {
            FileKind::Sidecar
        } else {
            continue;
        };

        let rel_path = normalize_rel(root, &path)?;
        let parent_rel = path
            .parent()
            .map(|p| normalize_rel(root, p))
            .transpose()?
            .unwrap_or_default();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let item = FileEntry {
            path: path.clone(),
            rel_path,
            kind,
            extension: ext,
        };
        let builder = grouped.entry((parent_rel, stem)).or_default();
        match kind {
            FileKind::Raw | FileKind::Compressed => builder.files.push(item),
            FileKind::Sidecar => builder.sidecars.push(item),
        }
    }

    let re = Regex::new(r"^(.*?)(\d+)$")?;
    let mut assets = Vec::new();
    for ((directory, stem), mut builder) in grouped {
        if builder.files.is_empty() {
            continue;
        }
        builder.files.sort_by_key(file_priority);
        builder.sidecars.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        let representative = builder.files[0].clone();
        let (prefix, seq) = split_stem_with_re(&re, &stem);
        let metadata = fs::metadata(&representative.path)?;
        let id = stable_asset_id(&representative.rel_path);
        assets.push(AssetInput {
            id,
            representative,
            files: builder.files,
            sidecars: builder.sidecars,
            directory,
            stem,
            prefix,
            seq,
            created_ms: metadata.created().ok().and_then(system_time_ms),
            modified_ms: metadata.modified().ok().and_then(system_time_ms),
        });
    }

    assets.sort_by(|a, b| {
        (
            &a.directory,
            &a.prefix,
            a.seq.unwrap_or(-1),
            a.time_key_ms().unwrap_or(i64::MAX),
            &a.stem,
        )
            .cmp(&(
                &b.directory,
                &b.prefix,
                b.seq.unwrap_or(-1),
                b.time_key_ms().unwrap_or(i64::MAX),
                &b.stem,
            ))
    });
    Ok(assets)
}

pub fn is_raw_extension(ext: &str) -> bool {
    RAW_EXTS.contains(&ext.to_ascii_lowercase().as_str())
}

pub fn normalize_rel(root: &Path, path: &Path) -> anyhow::Result<String> {
    let rel = path.strip_prefix(root)?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
}

fn split_stem_with_re(re: &Regex, stem: &str) -> (String, Option<i64>) {
    if let Some(caps) = re.captures(stem) {
        let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or(stem).to_string();
        let seq = caps.get(2).and_then(|m| m.as_str().parse::<i64>().ok());
        (prefix, seq)
    } else {
        (stem.to_string(), None)
    }
}

fn file_priority(file: &FileEntry) -> (u8, u8, String) {
    let kind = match file.kind {
        FileKind::Compressed => 0,
        FileKind::Raw => 1,
        FileKind::Sidecar => 2,
    };
    let ext = match file.extension.as_str() {
        "jpg" | "jpeg" => 0,
        "png" | "webp" => 1,
        "tif" | "tiff" => 2,
        _ => 3,
    };
    (kind, ext, file.rel_path.clone())
}

fn stable_asset_id(rel_path: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(rel_path.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn system_time_ms(time: SystemTime) -> Option<i64> {
    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

#[allow(dead_code)]
pub fn absolute(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
