#![cfg(target_os = "linux")]

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, anyhow, bail};
use burst_core::FocusMetrics;
use cudarc::driver::{CudaContext, CudaFunction, CudaModule, LaunchConfig, PushKernelArg};
use cudarc::nvrtc::{CompileOptions, compile_ptx_with_opts};
use libloading::Library;

const THREADS_PER_BLOCK: u32 = 256;
const PARTIAL_VALUES_PER_BLOCK: usize = 4;
const KERNEL_NAME: &str = "bfd_focus_partials";

const CUDA_DRIVER_LIBRARY_CANDIDATES: &[&str] = &["libcuda.so.1", "libcuda.so"];
const NVRTC_LIBRARY_CANDIDATES: &[&str] = &["libnvrtc.so.12", "libnvrtc.so.12.0", "libnvrtc.so"];

#[derive(Clone, Debug)]
pub struct CudaStatus {
    pub available: bool,
    pub device_name: Option<String>,
    pub note: Option<String>,
}

struct CudaBackend {
    context: Arc<CudaContext>,
    // CudaFunction also retains the module. Keeping the module explicitly documents and enforces
    // that the process-wide backend owns all three shared CUDA objects.
    _module: Arc<CudaModule>,
    function: CudaFunction,
    device_name: String,
    fatal_error: Mutex<Option<String>>,
}

static CUDA_BACKEND: OnceLock<Result<Arc<CudaBackend>, String>> = OnceLock::new();

pub fn status() -> CudaStatus {
    match cached_backend() {
        Ok(backend) => {
            let note = backend.fatal_error();
            CudaStatus {
                available: note.is_none(),
                device_name: Some(backend.device_name.clone()),
                note,
            }
        }
        Err(note) => CudaStatus {
            available: false,
            device_name: None,
            note: Some(format!("{note:#}")),
        },
    }
}

pub fn focus_metrics(gray: &[u8], width: usize, height: usize) -> anyhow::Result<FocusMetrics> {
    let pixel_count = validate_input(gray, width, height)?;
    let backend = cached_backend().context("CUDA focus scoring is unavailable")?;
    if let Some(note) = backend.fatal_error() {
        bail!("CUDA focus scoring is disabled after a prior failure: {note}");
    }

    match catch_unwind(AssertUnwindSafe(|| {
        backend.focus_metrics(gray, width, height, pixel_count)
    })) {
        Ok(Ok(metrics)) => Ok(metrics),
        Ok(Err(error)) => {
            let note =
                format!("CUDA focus scoring failed; disabling CUDA for this process: {error:#}");
            backend.set_fatal_error(note.clone());
            Err(anyhow!(note))
        }
        Err(payload) => {
            let note = format!(
                "CUDA dynamic loader panicked while scoring; disabling CUDA: {}",
                panic_message(payload.as_ref())
            );
            backend.set_fatal_error(note.clone());
            Err(anyhow!(note))
        }
    }
}

impl CudaBackend {
    fn initialize() -> anyhow::Result<Self> {
        preflight_dynamic_libraries()?;

        let context = CudaContext::new(0).context("initializing CUDA device 0")?;
        let device_name = context
            .name()
            .context("reading the selected CUDA device name")?;
        let (compute_major, compute_minor) = context
            .compute_capability()
            .context("reading CUDA compute capability")?;
        let architecture = format!("--gpu-architecture=compute_{compute_major}{compute_minor}");
        let options = CompileOptions {
            name: Some("burst_frame_focus.cu".to_string()),
            options: vec![architecture],
            ..CompileOptions::default()
        };
        let ptx = compile_ptx_with_opts(KERNEL_SOURCE, options)
            .context("compiling the CUDA focus kernel with NVRTC")?;
        let module = context
            .load_module(ptx)
            .context("loading the CUDA focus kernel module")?;
        let function = module
            .load_function(KERNEL_NAME)
            .context("loading the CUDA focus kernel function")?;

        Ok(Self {
            context,
            _module: module,
            function,
            device_name,
            fatal_error: Mutex::new(None),
        })
    }

    fn focus_metrics(
        &self,
        gray: &[u8],
        width: usize,
        height: usize,
        pixel_count: usize,
    ) -> anyhow::Result<FocusMetrics> {
        let block_count = pixel_count.div_ceil(THREADS_PER_BLOCK as usize);
        let grid_width = u32::try_from(block_count)
            .context("image requires more CUDA blocks than a one-dimensional grid supports")?;
        let partial_value_count = block_count
            .checked_mul(PARTIAL_VALUES_PER_BLOCK)
            .context("CUDA partial-result allocation size overflow")?;
        let pixel_count = u64::try_from(pixel_count).context("image pixel count exceeds u64")?;
        let width_u64 = u64::try_from(width).context("image width exceeds u64")?;
        let height_u64 = u64::try_from(height).context("image height exceeds u64")?;

        let stream = self
            .context
            .new_stream()
            .context("creating a CUDA scoring stream")?;
        let input = stream
            .clone_htod(gray)
            .context("copying the grayscale preview to CUDA")?;
        let mut partials = stream
            .alloc_zeros::<f64>(partial_value_count)
            .context("allocating CUDA focus partials")?;

        let config = LaunchConfig {
            grid_dim: (grid_width, 1, 1),
            block_dim: (THREADS_PER_BLOCK, 1, 1),
            shared_mem_bytes: 0,
        };
        let mut launch = stream.launch_builder(&self.function);
        launch
            .arg(&input)
            .arg(&pixel_count)
            .arg(&width_u64)
            .arg(&height_u64)
            .arg(&mut partials);
        // The embedded kernel accepts exactly the five arguments above, uses one 256-thread
        // block per output record, and bounds-checks every input access.
        unsafe { launch.launch(config) }.context("launching the CUDA focus kernel")?;

        let host_partials = stream
            .clone_dtoh(&partials)
            .context("copying CUDA focus partials to the host")?;
        stream
            .synchronize()
            .context("waiting for the CUDA focus stream")?;

        reduce_partials(&host_partials, block_count, width, height)
    }

    fn fatal_error(&self) -> Option<String> {
        self.fatal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn set_fatal_error(&self, note: String) {
        *self
            .fatal_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(note);
    }
}

fn cached_backend() -> anyhow::Result<Arc<CudaBackend>> {
    CUDA_BACKEND
        .get_or_init(
            || match catch_unwind(AssertUnwindSafe(CudaBackend::initialize)) {
                Ok(Ok(backend)) => Ok(Arc::new(backend)),
                Ok(Err(error)) => Err(format!("{error:#}")),
                Err(payload) => Err(format!(
                    "CUDA dynamic loader panicked during initialization: {}",
                    panic_message(payload.as_ref())
                )),
            },
        )
        .as_ref()
        .cloned()
        .map_err(|note| anyhow!(note.clone()))
}

fn preflight_dynamic_libraries() -> anyhow::Result<()> {
    preflight_library("CUDA driver", CUDA_DRIVER_LIBRARY_CANDIDATES)?;
    preflight_library("NVRTC", NVRTC_LIBRARY_CANDIDATES)?;
    Ok(())
}

fn preflight_library(label: &str, candidates: &[&str]) -> anyhow::Result<()> {
    let mut failures = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        // Loading is used only as a preflight. cudarc owns its separately loaded process-wide
        // handles, so this handle may be dropped immediately after the availability check.
        match unsafe { Library::new(*candidate) } {
            Ok(_) => return Ok(()),
            Err(error) => failures.push(format!("{candidate}: {error}")),
        }
    }
    bail!(
        "{label} shared library is unavailable (tried {}): {}",
        candidates.join(", "),
        failures.join("; ")
    )
}

fn validate_input(gray: &[u8], width: usize, height: usize) -> anyhow::Result<usize> {
    if width < 3 || height < 3 {
        bail!("CUDA focus scoring requires an image at least 3x3 pixels");
    }
    let pixel_count = width
        .checked_mul(height)
        .context("image dimensions overflow usize")?;
    if gray.len() != pixel_count {
        bail!(
            "grayscale buffer length {} does not match image dimensions {width}x{height}",
            gray.len()
        );
    }
    Ok(pixel_count)
}

fn reduce_partials(
    partials: &[f64],
    block_count: usize,
    width: usize,
    height: usize,
) -> anyhow::Result<FocusMetrics> {
    let expected = block_count
        .checked_mul(PARTIAL_VALUES_PER_BLOCK)
        .context("CUDA partial-result length overflow")?;
    if block_count == 0 || partials.len() != expected {
        bail!(
            "CUDA returned {} partial values; expected {expected}",
            partials.len()
        );
    }
    if width < 3 || height < 3 {
        bail!("cannot reduce focus metrics for an image smaller than 3x3 pixels");
    }

    let mut lap_sum = 0.0;
    let mut lap_sq_sum = 0.0;
    let mut dx_sum = 0.0;
    let mut dy_sum = 0.0;
    for partial in partials.chunks_exact(PARTIAL_VALUES_PER_BLOCK) {
        lap_sum += partial[0];
        lap_sq_sum += partial[1];
        dx_sum += partial[2];
        dy_sum += partial[3];
    }

    let lap_count = (width - 2)
        .checked_mul(height - 2)
        .context("Laplacian sample count overflow")? as f64;
    let dx_count = height
        .checked_mul(width - 1)
        .context("horizontal-gradient sample count overflow")? as f64;
    let dy_count = (height - 1)
        .checked_mul(width)
        .context("vertical-gradient sample count overflow")? as f64;
    let lap_mean = lap_sum / lap_count;

    Ok(FocusMetrics {
        sharpness: lap_sq_sum / lap_count - lap_mean * lap_mean,
        tenengrad: dx_sum / dx_count + dy_sum / dy_count,
    })
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else {
        "non-string panic payload".to_string()
    }
}

const KERNEL_SOURCE: &str = include_str!("cuda/focus.cu");

#[cfg(test)]
mod tests {
    use super::{KERNEL_NAME, KERNEL_SOURCE, reduce_partials, validate_input};

    #[test]
    fn validates_dimensions_without_initializing_cuda() {
        assert_eq!(validate_input(&[0; 9], 3, 3).unwrap(), 9);
        assert!(validate_input(&[0; 8], 3, 3).is_err());
        assert!(validate_input(&[0; 6], 2, 3).is_err());
        assert!(validate_input(&[], usize::MAX, 3).is_err());
    }

    #[test]
    fn reduces_compact_f64_partials() {
        let partials = [3.0, 5.0, 12.0, 8.0, 1.0, 5.0, 12.0, 8.0];
        let metrics = reduce_partials(&partials, 2, 4, 3).unwrap();
        assert!((metrics.sharpness - 1.0).abs() < 1e-12);
        assert!((metrics.tenengrad - (14.0 / 3.0)).abs() < 1e-12);
        assert!(reduce_partials(&partials[..4], 2, 4, 3).is_err());
    }

    #[test]
    fn embeds_the_expected_reduction_kernel() {
        assert!(KERNEL_SOURCE.contains(KERNEL_NAME));
        assert!(KERNEL_SOURCE.contains("double* __restrict__ partials"));
        assert!(KERNEL_SOURCE.contains("__syncthreads()"));
    }
}
