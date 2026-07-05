use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{In, Reader, Tag, Value};

use crate::types::PhotoMetadata;

pub fn read_photo_metadata(path: &Path) -> PhotoMetadata {
    read_photo_metadata_inner(path).unwrap_or_default()
}

fn read_photo_metadata_inner(path: &Path) -> anyhow::Result<PhotoMetadata> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let exif = Reader::new().read_from_container(&mut reader)?;
    let shutter_s = rational_value(&exif, Tag::ExposureTime);
    Ok(PhotoMetadata {
        iso: uint_value(&exif, Tag::PhotographicSensitivity),
        aperture: rational_value(&exif, Tag::FNumber),
        shutter_s,
        shutter: shutter_s.map(format_shutter),
        focal_length_mm: rational_value(&exif, Tag::FocalLength),
        focal_length_35mm: uint_value(&exif, Tag::FocalLengthIn35mmFilm),
    })
}

fn uint_value(exif: &exif::Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
}

fn rational_value(exif: &exif::Exif, tag: Tag) -> Option<f64> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Rational(values) => values
            .first()
            .and_then(|value| (value.denom != 0).then_some(value.num as f64 / value.denom as f64)),
        Value::SRational(values) => values
            .first()
            .and_then(|value| (value.denom != 0).then_some(value.num as f64 / value.denom as f64)),
        Value::Short(values) => values.first().map(|value| f64::from(*value)),
        Value::Long(values) => values.first().map(|value| *value as f64),
        _ => None,
    }
}

fn format_shutter(seconds: f64) -> String {
    if seconds <= 0.0 {
        return String::new();
    }
    if seconds >= 1.0 {
        return format_trimmed(seconds, "s");
    }
    let denominator = (1.0 / seconds).round();
    if denominator >= 2.0 {
        format!("1/{denominator:.0}s")
    } else {
        format_trimmed(seconds, "s")
    }
}

fn format_trimmed(value: f64, suffix: &str) -> String {
    let text = if value >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    };
    format!(
        "{}{suffix}",
        text.trim_end_matches('0').trim_end_matches('.')
    )
}
