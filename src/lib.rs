#![allow(unexpected_cfgs)]

pub mod app_backend;
pub mod artifacts;
pub mod assets;
pub mod counterpart;
#[cfg(all(
    target_os = "linux",
    any(feature = "avx2-accel", feature = "neon-accel")
))]
pub mod cpu_accel;
#[cfg(all(target_os = "linux", feature = "cuda-accel"))]
pub mod cuda_accel;
pub mod decode;
pub mod detector;
pub mod features;
pub mod ffi;
#[cfg(all(target_os = "linux", feature = "libraw-preview"))]
mod libraw_preview;
#[cfg(all(target_os = "linux", feature = "linux-gui"))]
pub mod linux_gui;
pub mod locales;
pub mod metadata;
#[cfg(all(target_os = "macos", feature = "metal-accel"))]
pub mod metal_accel;
#[cfg(all(target_os = "linux", feature = "onnx-detector"))]
pub(crate) mod ml_detector;
pub mod operations;
pub mod pipeline;
pub mod progress;
pub mod run_storage;
pub mod server;
pub mod types;
