#![allow(unexpected_cfgs)]

pub mod artifacts;
pub mod assets;
#[cfg(all(target_os = "linux", feature = "avx2-accel"))]
pub mod cpu_accel;
#[cfg(all(target_os = "linux", feature = "cuda-accel"))]
pub mod cuda_accel;
pub mod decode;
pub mod detector;
pub mod features;
pub mod ffi;
pub mod locales;
pub mod metadata;
#[cfg(all(target_os = "macos", feature = "metal-accel"))]
pub mod metal_accel;
pub mod operations;
pub mod pipeline;
pub mod progress;
pub mod run_storage;
pub mod server;
pub mod types;
