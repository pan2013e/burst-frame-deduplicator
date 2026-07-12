mod model;

use burn::backend::{
    WebGpu,
    wgpu::{RuntimeOptions, WgpuDevice, graphics, init_setup_async},
};
use burn::tensor::{Bytes, Tensor, TensorData};
use burst_core::{SubjectDetection, detection_from_probability_mask};
use serde::Serialize;
use wasm_bindgen::prelude::*;

const MODEL_SIZE: usize = 320;

#[derive(Serialize)]
struct BrowserDetection {
    backend: &'static str,
    adapter: String,
    confidence: f64,
    subject_count: usize,
    truncation_risk: f64,
    bbox_x1: f64,
    bbox_y1: f64,
    bbox_x2: f64,
    bbox_y2: f64,
}

#[wasm_bindgen]
pub struct U2NetDetector {
    device: WgpuDevice,
    model: model::Model<WebGpu>,
    adapter: String,
}

#[wasm_bindgen]
impl U2NetDetector {
    #[wasm_bindgen(js_name = create)]
    pub async fn create(weights: &[u8]) -> Result<U2NetDetector, JsValue> {
        console_error_panic_hook::set_once();
        if weights.len() < 4_000_000 {
            return Err(JsValue::from_str(
                "U2-Net-P model weights are missing or incomplete",
            ));
        }
        let device = WgpuDevice::default();
        let setup = init_setup_async::<graphics::WebGpu>(&device, RuntimeOptions::default()).await;
        let adapter_name = setup.adapter.get_info().name;
        let adapter = if adapter_name.trim().is_empty() {
            "browser WebGPU adapter".to_string()
        } else {
            adapter_name
        };
        let model =
            model::Model::<WebGpu>::from_bytes(Bytes::from_bytes_vec(weights.to_vec()), &device);
        Ok(Self {
            device,
            model,
            adapter,
        })
    }

    pub fn adapter_name(&self) -> String {
        self.adapter.clone()
    }

    pub async fn detect_rgba(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<JsValue, JsValue> {
        let detection = self.infer_rgba_batch(width, height, 1, rgba).await?;
        serde_wasm_bindgen::to_value(&detection[0])
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub async fn detect_rgba_batch(
        &self,
        width: u32,
        height: u32,
        count: usize,
        rgba: &[u8],
    ) -> Result<JsValue, JsValue> {
        let detections = self.infer_rgba_batch(width, height, count, rgba).await?;
        serde_wasm_bindgen::to_value(&detections)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

impl U2NetDetector {
    async fn infer_rgba_batch(
        &self,
        width: u32,
        height: u32,
        count: usize,
        rgba: &[u8],
    ) -> Result<Vec<BrowserDetection>, JsValue> {
        let image_bytes = MODEL_SIZE * MODEL_SIZE * 4;
        if width as usize != MODEL_SIZE
            || height as usize != MODEL_SIZE
            || count == 0
            || count > 8
            || rgba.len() != image_bytes * count
        {
            return Err(JsValue::from_str(
                "U2-Net-P expects one to eight 320x320 RGBA previews",
            ));
        }
        let mut input = Vec::with_capacity(count * 3 * MODEL_SIZE * MODEL_SIZE);
        for image in rgba.chunks_exact(image_bytes) {
            input.extend(preprocess(image));
        }
        let input = Tensor::<WebGpu, 4>::from_data(
            TensorData::new(input, [count, 3, MODEL_SIZE, MODEL_SIZE]),
            &self.device,
        );
        let output = self.model.forward(input).0;
        let probabilities = output
            .into_data_async()
            .await
            .map_err(|error| JsValue::from_str(&error.to_string()))?
            .into_vec::<f32>()
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        if probabilities.len() != count * MODEL_SIZE * MODEL_SIZE {
            return Err(JsValue::from_str("U2-Net-P returned an invalid mask batch"));
        }
        Ok(probabilities
            .chunks_exact(MODEL_SIZE * MODEL_SIZE)
            .map(|mask| {
                let detection = detection_from_probability_mask(mask, MODEL_SIZE, MODEL_SIZE)
                    .unwrap_or(SubjectDetection {
                        confidence: 0.0,
                        subject_count: 0,
                        truncation_risk: 0.0,
                        bbox_x1: 0.0,
                        bbox_y1: 0.0,
                        bbox_x2: 1.0,
                        bbox_y2: 1.0,
                    });
                BrowserDetection {
                    backend: "burn_u2netp_webgpu",
                    adapter: self.adapter.clone(),
                    confidence: detection.confidence,
                    subject_count: detection.subject_count,
                    truncation_risk: detection.truncation_risk,
                    bbox_x1: detection.bbox_x1,
                    bbox_y1: detection.bbox_y1,
                    bbox_x2: detection.bbox_x2,
                    bbox_y2: detection.bbox_y2,
                }
            })
            .collect())
    }
}

fn preprocess(rgba: &[u8]) -> Vec<f32> {
    let maximum = rgba
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .max()
        .map(f32::from)
        .unwrap_or(0.0);
    let scale = if maximum > 0.0 { 1.0 / maximum } else { 0.0 };
    let mean = [0.485f32, 0.456, 0.406];
    let deviation = [0.229f32, 0.224, 0.225];
    let plane = MODEL_SIZE * MODEL_SIZE;
    let mut input = vec![0.0; plane * 3];
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        for channel in 0..3 {
            input[channel * plane + index] =
                (f32::from(pixel[channel]) * scale - mean[channel]) / deviation[channel];
        }
    }
    input
}

#[cfg(test)]
mod tests {
    use super::{MODEL_SIZE, preprocess};

    #[test]
    fn preprocessing_is_nchw_and_guards_black_images() {
        let mut rgba = vec![0u8; MODEL_SIZE * MODEL_SIZE * 4];
        for alpha in rgba.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }
        let input = preprocess(&rgba);
        assert_eq!(input.len(), 3 * MODEL_SIZE * MODEL_SIZE);
        assert!((input[0] + 0.485 / 0.229).abs() < 1e-6);
    }
}
