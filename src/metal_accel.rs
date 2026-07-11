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
    gray: &[u8],
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
        gray: &[u8],
        width: usize,
        height: usize,
    ) -> anyhow::Result<MetalFocusMetrics> {
        if gray.len() != width * height || width < 3 || height < 3 {
            return Err(anyhow!("invalid grayscale buffer"));
        }

        let pixel_count = width * height;
        const THREADS_PER_GROUP: usize = 256;
        let partial_count = pixel_count.div_ceil(THREADS_PER_GROUP);
        let input = self.device.new_buffer_with_data(
            gray.as_ptr().cast(),
            gray.len() as u64,
            MTLResourceOptions::StorageModeShared,
        );
        let output = self.device.new_buffer(
            (partial_count * 4 * size_of::<f32>()) as u64,
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

        let thread_groups = MTLSize {
            width: partial_count as u64,
            height: 1,
            depth: 1,
        };
        let group = MTLSize {
            width: THREADS_PER_GROUP as u64,
            height: 1,
            depth: 1,
        };
        encoder.dispatch_thread_groups(thread_groups, group);
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();

        let status = command_buffer.status();
        if format!("{status:?}") == "Error" {
            return Err(anyhow!("Metal command buffer failed"));
        }

        let values =
            unsafe { slice::from_raw_parts(output.contents().cast::<f32>(), partial_count * 4) };
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
    device const uchar* gray [[buffer(0)]],
    device float4* partials [[buffer(1)]],
    constant uint& width [[buffer(2)]],
    constant uint& height [[buffer(3)]],
    uint gid [[thread_position_in_grid]],
    uint lid [[thread_index_in_threadgroup]],
    uint group_id [[threadgroup_position_in_grid]]
) {
    threadgroup float4 sums[256];
    uint pixel_count = width * height;
    uint i = gid;
    uint x = i % width;
    uint y = i / width;

    float lap = 0.0;
    float lap_sq = 0.0;
    if (i < pixel_count && x > 0 && x + 1 < width && y > 0 && y + 1 < height) {
        lap = -4.0 * float(gray[i]) + float(gray[i - 1]) + float(gray[i + 1]) + float(gray[i - width]) + float(gray[i + width]);
        lap_sq = lap * lap;
    }

    float dx_sq = 0.0;
    if (i < pixel_count && x + 1 < width) {
        float dx = float(gray[i + 1]) - float(gray[i]);
        dx_sq = dx * dx;
    }

    float dy_sq = 0.0;
    if (i < pixel_count && y + 1 < height) {
        float dy = float(gray[i + width]) - float(gray[i]);
        dy_sq = dy * dy;
    }

    sums[lid] = float4(lap, lap_sq, dx_sq, dy_sq);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stride = 128; stride > 0; stride >>= 1) {
        if (lid < stride) {
            sums[lid] += sums[lid + stride];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (lid == 0) {
        partials[group_id] = sums[0];
    }
}
"#;
