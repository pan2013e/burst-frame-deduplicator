use std::borrow::Cow;

use futures_channel::oneshot;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wgpu::{
    Backends, BindGroupDescriptor, BindGroupEntry, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipelineDescriptor, DeviceDescriptor,
    InstanceDescriptor, MapMode, PipelineCompilationOptions, PowerPreference,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
};

use crate::reduce_focus_partials;

const THREADS_PER_GROUP: u32 = 256;
const BYTES_PER_PARTIAL: u64 = 16;

#[derive(Serialize)]
struct WebGpuFocusResult {
    sharpness: f64,
    tenengrad: f64,
    backend: &'static str,
    adapter: String,
}

#[wasm_bindgen]
pub struct WebGpuFocusScorer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    adapter: String,
}

#[wasm_bindgen]
impl WebGpuFocusScorer {
    #[wasm_bindgen(js_name = create)]
    pub async fn create() -> Result<WebGpuFocusScorer, JsValue> {
        let mut instance_descriptor = InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = Backends::BROWSER_WEBGPU;
        let instance = wgpu::Instance::new(instance_descriptor);
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .map_err(|error| js_error("requesting a WebGPU adapter", error))?;
        let adapter_info = adapter.get_info();
        let adapter_name = if adapter_info.name.trim().is_empty() {
            "browser WebGPU adapter".to_string()
        } else {
            adapter_info.name
        };
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("burst WebGPU focus device"),
                ..Default::default()
            })
            .await
            .map_err(|error| js_error("requesting a WebGPU device", error))?;
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("burst focus metrics"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("focus.wgsl"))),
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("burst focus pipeline"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            adapter: adapter_name,
        })
    }

    pub fn adapter_name(&self) -> String {
        self.adapter.clone()
    }

    pub async fn score_rgba(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<JsValue, JsValue> {
        let pixel_count = width
            .checked_mul(height)
            .ok_or_else(|| JsValue::from_str("WebGPU image dimensions overflow u32"))?;
        let expected_bytes = usize::try_from(pixel_count)
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| JsValue::from_str("WebGPU RGBA buffer length overflows usize"))?;
        if width < 3 || height < 3 || rgba.len() != expected_bytes {
            return Err(JsValue::from_str(
                "WebGPU focus scoring requires a valid image of at least 3x3 pixels",
            ));
        }

        let partial_count = pixel_count.div_ceil(THREADS_PER_GROUP);
        let partial_bytes = u64::from(partial_count)
            .checked_mul(BYTES_PER_PARTIAL)
            .ok_or_else(|| JsValue::from_str("WebGPU partial buffer size overflow"))?;
        let input = self.device.create_buffer(&BufferDescriptor {
            label: Some("burst RGBA input"),
            size: rgba.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = self.device.create_buffer(&BufferDescriptor {
            label: Some("burst focus partials"),
            size: partial_bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&BufferDescriptor {
            label: Some("burst focus readback"),
            size: partial_bytes,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let parameters = self.device.create_buffer(&BufferDescriptor {
            label: Some("burst focus parameters"),
            size: 16,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.queue.write_buffer(&input, 0, rgba);
        let mut parameter_bytes = [0u8; 16];
        parameter_bytes[0..4].copy_from_slice(&width.to_le_bytes());
        parameter_bytes[4..8].copy_from_slice(&height.to_le_bytes());
        parameter_bytes[8..12].copy_from_slice(&pixel_count.to_le_bytes());
        parameter_bytes[12..16].copy_from_slice(&partial_count.to_le_bytes());
        self.queue.write_buffer(&parameters, 0, &parameter_bytes);

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("burst focus bindings"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: parameters.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("burst focus commands"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("burst focus compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(partial_count, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, partial_bytes);

        let (sender, receiver) = oneshot::channel();
        encoder.map_buffer_on_submit(&readback, MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        self.queue.submit([encoder.finish()]);
        receiver
            .await
            .map_err(|_| JsValue::from_str("WebGPU readback callback was cancelled"))?
            .map_err(|error| js_error("mapping WebGPU focus results", error))?;

        let mapped = readback.slice(..).get_mapped_range();
        let mut partials = Vec::with_capacity(partial_count as usize * 4);
        for bytes in mapped.chunks_exact(4) {
            partials.push(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        }
        drop(mapped);
        readback.unmap();

        let metrics = reduce_focus_partials(&partials, width as usize, height as usize)
            .map_err(|error| JsValue::from_str(error))?;
        serde_wasm_bindgen::to_value(&WebGpuFocusResult {
            sharpness: metrics.sharpness,
            tenengrad: metrics.tenengrad,
            backend: "webgpu_wgpu",
            adapter: self.adapter.clone(),
        })
        .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

fn js_error(context: &str, error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&format!("{context}: {error}"))
}
