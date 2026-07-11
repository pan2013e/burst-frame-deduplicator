use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, anyhow};
use image::{ImageFormat, ImageReader, RgbImage, imageops};
use jpeg_decoder::{Decoder as JpegDecoder, PixelFormat};
use tempfile::TempDir;

use crate::assets::is_raw_extension;
use crate::types::DecoderReport;

pub struct DecodedPreview {
    pub image: RgbImage,
    pub width: u32,
    pub height: u32,
    pub decoder: String,
}

pub fn decoder_report() -> DecoderReport {
    let magick = which::which("magick").ok();
    let sips = which::which("sips").ok();
    let raw_strategy = if cfg!(target_os = "macos") && sips.is_some() {
        if magick.is_some() {
            "macOS Camera RAW via sips; ImageMagick compatibility fallback".to_string()
        } else {
            "macOS Camera RAW via sips; no ImageMagick fallback".to_string()
        }
    } else if magick.is_some() {
        "ImageMagick RAW/HEIC fallback".to_string()
    } else {
        "native compressed formats only; RAW requires ImageMagick or sips".to_string()
    };
    DecoderReport {
        native_compressed: true,
        scaled_jpeg: true,
        imagemagick: magick,
        sips,
        raw_strategy,
    }
}

pub fn load_preview(
    path: &Path,
    extension: &str,
    preview_size: u32,
) -> anyhow::Result<DecodedPreview> {
    if is_raw_extension(extension) {
        return load_external(path, preview_size, "raw");
    }

    match load_native(path, extension, preview_size) {
        Ok(decoded) => Ok(decoded),
        Err(native_error) => load_external(path, preview_size, "fallback").with_context(|| {
            format!(
                "native decode failed ({native_error}); external decoder fallback also failed for {}",
                path.display()
            )
        }),
    }
}

pub fn resize_rgb(image: &RgbImage, long_edge: u32) -> RgbImage {
    let (width, height) = image.dimensions();
    let max_edge = width.max(height);
    if max_edge <= long_edge || long_edge == 0 {
        return image.clone();
    }
    let scale = long_edge as f64 / max_edge as f64;
    let new_width = ((width as f64 * scale).round() as u32).max(1);
    let new_height = ((height as f64 * scale).round() as u32).max(1);
    imageops::resize(image, new_width, new_height, imageops::FilterType::Lanczos3)
}

fn load_native(path: &Path, extension: &str, preview_size: u32) -> anyhow::Result<DecodedPreview> {
    if matches!(extension, "jpg" | "jpeg") && preview_size > 0 {
        match load_scaled_jpeg(path, preview_size) {
            Ok(decoded) => return Ok(decoded),
            Err(err) => {
                eprintln!(
                    "Scaled JPEG decode failed for {}; using image-rs fallback: {err}",
                    path.display()
                );
            }
        }
    }
    let image = ImageReader::open(path)
        .with_context(|| format!("opening {}", path.display()))?
        .with_guessed_format()
        .context("guessing image format")?
        .decode()
        .context("decoding image")?;
    let width = image.width();
    let height = image.height();
    let rgb = image.to_rgb8();
    Ok(DecodedPreview {
        image: resize_rgb(&rgb, preview_size),
        width,
        height,
        decoder: "image-rs".to_string(),
    })
}

fn load_scaled_jpeg(path: &Path, preview_size: u32) -> anyhow::Result<DecodedPreview> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut decoder = JpegDecoder::new(BufReader::new(file));
    decoder.read_info().context("reading JPEG dimensions")?;
    let original = decoder
        .info()
        .ok_or_else(|| anyhow!("JPEG dimensions are unavailable"))?;
    let requested = preview_size.min(u32::from(u16::MAX)) as u16;
    decoder
        .scale(requested, requested)
        .context("configuring scaled JPEG decode")?;
    let pixels = decoder.decode().context("decoding scaled JPEG")?;
    let output = decoder
        .info()
        .ok_or_else(|| anyhow!("scaled JPEG dimensions are unavailable"))?;
    let rgb = match output.pixel_format {
        PixelFormat::RGB24 => {
            RgbImage::from_raw(u32::from(output.width), u32::from(output.height), pixels)
                .ok_or_else(|| anyhow!("scaled RGB JPEG buffer has an invalid length"))?
        }
        PixelFormat::L8 => {
            let mut rgb = Vec::with_capacity(pixels.len() * 3);
            for value in pixels {
                rgb.extend_from_slice(&[value, value, value]);
            }
            RgbImage::from_raw(u32::from(output.width), u32::from(output.height), rgb)
                .ok_or_else(|| anyhow!("scaled grayscale JPEG buffer has an invalid length"))?
        }
        PixelFormat::L16 | PixelFormat::CMYK32 => {
            return Err(anyhow!(
                "scaled decoder does not support {:?} output",
                output.pixel_format
            ));
        }
    };
    Ok(DecodedPreview {
        image: resize_rgb(&rgb, preview_size),
        width: u32::from(original.width),
        height: u32::from(original.height),
        decoder: "jpeg-decoder-scaled".to_string(),
    })
}

fn load_external(path: &Path, preview_size: u32, reason: &str) -> anyhow::Result<DecodedPreview> {
    if reason == "raw"
        && cfg!(target_os = "macos")
        && let Ok(sips) = which::which("sips")
        && let Ok(decoded) = load_with_sips(&sips, path, preview_size, reason)
    {
        return Ok(decoded);
    }
    if let Ok(magick) = which::which("magick")
        && let Ok(decoded) = load_with_magick(&magick, path, preview_size, reason)
    {
        return Ok(decoded);
    }
    if let Ok(sips) = which::which("sips") {
        return load_with_sips(&sips, path, preview_size, reason);
    }
    Err(anyhow!(
        "no external decoder found for {}; install ImageMagick or use a camera JPEG pair",
        path.display()
    ))
}

fn load_with_magick(
    magick: &Path,
    path: &Path,
    preview_size: u32,
    reason: &str,
) -> anyhow::Result<DecodedPreview> {
    let output = Command::new(magick)
        .arg(path)
        .arg("-auto-orient")
        .arg("-resize")
        .arg(format!("{preview_size}x{preview_size}>"))
        .arg("png:-")
        .output()
        .with_context(|| format!("running ImageMagick for {}", path.display()))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let image = image::load_from_memory_with_format(&output.stdout, ImageFormat::Png)
        .context("decoding ImageMagick PNG output")?;
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    Ok(DecodedPreview {
        image: rgb,
        width,
        height,
        decoder: format!("imagemagick-{reason}"),
    })
}

fn load_with_sips(
    sips: &Path,
    path: &Path,
    preview_size: u32,
    reason: &str,
) -> anyhow::Result<DecodedPreview> {
    let tmp = TempDir::new().context("creating temporary sips output directory")?;
    let out = tmp.path().join("preview.jpg");
    let output = Command::new(sips)
        .arg("-s")
        .arg("format")
        .arg("jpeg")
        .arg("--resampleHeightWidthMax")
        .arg(preview_size.to_string())
        .arg(path)
        .arg("--out")
        .arg(&out)
        .output()
        .with_context(|| format!("running sips for {}", path.display()))?;
    if !output.status.success() || !out.exists() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let image = ImageReader::open(&out)?.decode()?;
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    Ok(DecodedPreview {
        image: rgb,
        width,
        height,
        decoder: format!("sips-{reason}"),
    })
}

#[allow(dead_code)]
fn _canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
