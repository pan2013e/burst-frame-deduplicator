use std::cell::RefCell;
use std::mem::size_of;
use std::slice;

use anyhow::anyhow;
use metal::{CompileOptions, Device, MTLResourceOptions, MTLSize};

pub struct MetalFocusMetrics {
    pub sharpness: f64,
    pub tenengrad: f64,
}

thread_local! {
    static SCORER: RefCell<Option<MetalScorer>> = RefCell::new(MetalScorer::new().ok());
}

pub fn focus_metrics(
    gray: &[f64],
    width: usize,
    height: usize,
) -> anyhow::Result<MetalFocusMetrics> {
    SCORER.with(|scorer| {
        let scorer = scorer.borrow();
        let scorer = scorer
            .as_ref()
            .ok_or_else(|| anyhow!("Metal scorer is unavailable"))?;
        scorer.focus_metrics(gray, width, height)
    })
}

pub fn is_available() -> bool {
    SCORER.with(|scorer| scorer.borrow().is_some())
}

struct MetalScorer {
    device: Device,
    queue: metal::CommandQueue,
    pipeline: metal::ComputePipelineState,
}

impl MetalScorer {
    fn new() -> anyhow::Result<Self> {
        let device = Device::system_default().ok_or_else(|| anyhow!("no Metal device found"))?;
        let options = CompileOptions::new();
        let library = device
            .new_library_with_source(SHADER, &options)
            .map_err(|err| anyhow!("compiling Metal shader: {err}"))?;
        let function = library
            .get_function("focus_kernel", None)
            .map_err(|err| anyhow!("loading Metal focus kernel: {err}"))?;
        let pipeline = device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|err| anyhow!("creating Metal pipeline: {err}"))?;
        let queue = device.new_command_queue();
        Ok(Self {
            device,
            queue,
            pipeline,
        })
    }

    fn focus_metrics(
        &self,
        gray: &[f64],
        width: usize,
        height: usize,
    ) -> anyhow::Result<MetalFocusMetrics> {
        if gray.len() != width * height || width < 3 || height < 3 {
            return Err(anyhow!("invalid grayscale buffer"));
        }

        let gray32: Vec<f32> = gray.iter().map(|value| *value as f32).collect();
        let pixel_count = width * height;
        let input = self.device.new_buffer_with_data(
            gray32.as_ptr().cast(),
            (gray32.len() * size_of::<f32>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );
        let output = self.device.new_buffer(
            (pixel_count * 4 * size_of::<f32>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );

        let width32 = width as u32;
        let height32 = height as u32;
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        encoder.set_compute_pipeline_state(&self.pipeline);
        encoder.set_buffer(0, Some(&input), 0);
        encoder.set_buffer(1, Some(&output), 0);
        encoder.set_bytes(2, size_of::<u32>() as u64, (&width32 as *const u32).cast());
        encoder.set_bytes(3, size_of::<u32>() as u64, (&height32 as *const u32).cast());

        let threads = MTLSize {
            width: width as u64,
            height: height as u64,
            depth: 1,
        };
        let group = MTLSize {
            width: 16,
            height: 16,
            depth: 1,
        };
        encoder.dispatch_threads(threads, group);
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();

        let status = command_buffer.status();
        if format!("{status:?}") == "Error" {
            return Err(anyhow!("Metal command buffer failed"));
        }

        let values =
            unsafe { slice::from_raw_parts(output.contents().cast::<f32>(), pixel_count * 4) };
        let mut lap_sum = 0.0;
        let mut lap_sq_sum = 0.0;
        let mut dx_sum = 0.0;
        let mut dy_sum = 0.0;
        for chunk in values.chunks_exact(4) {
            lap_sum += f64::from(chunk[0]);
            lap_sq_sum += f64::from(chunk[1]);
            dx_sum += f64::from(chunk[2]);
            dy_sum += f64::from(chunk[3]);
        }

        let lap_n = ((width - 2) * (height - 2)) as f64;
        let dx_n = (height * (width - 1)) as f64;
        let dy_n = ((height - 1) * width) as f64;
        let lap_mean = lap_sum / lap_n.max(1.0);
        let sharpness = (lap_sq_sum / lap_n.max(1.0)) - lap_mean * lap_mean;
        let tenengrad = dx_sum / dx_n.max(1.0) + dy_sum / dy_n.max(1.0);
        Ok(MetalFocusMetrics {
            sharpness,
            tenengrad,
        })
    }
}

const SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

kernel void focus_kernel(
    device const float* gray [[buffer(0)]],
    device float4* out [[buffer(1)]],
    constant uint& width [[buffer(2)]],
    constant uint& height [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]]
) {
    if (gid.x >= width || gid.y >= height) {
        return;
    }
    uint x = gid.x;
    uint y = gid.y;
    uint i = y * width + x;

    float lap = 0.0;
    float lap_sq = 0.0;
    if (x > 0 && x + 1 < width && y > 0 && y + 1 < height) {
        lap = -4.0 * gray[i] + gray[i - 1] + gray[i + 1] + gray[i - width] + gray[i + width];
        lap_sq = lap * lap;
    }

    float dx_sq = 0.0;
    if (x + 1 < width) {
        float dx = gray[i + 1] - gray[i];
        dx_sq = dx * dx;
    }

    float dy_sq = 0.0;
    if (y + 1 < height) {
        float dy = gray[i + width] - gray[i];
        dy_sq = dy * dy;
    }

    out[i] = float4(lap, lap_sq, dx_sq, dy_sq);
}
"#;
