// Generated from the pinned U2-Net-P ONNX checkpoint by burn-onnx 0.21.
use burn::nn::PaddingConfig2d;
use burn::nn::conv::Conv2d;
use burn::nn::conv::Conv2dConfig;
use burn::nn::pool::MaxPool2d;
use burn::nn::pool::MaxPool2dConfig;
use burn::prelude::*;
use burn::tensor::Bytes;
use burn_store::BurnpackStore;
use burn_store::ModuleSnapshot;

#[derive(Module, Debug)]
pub struct Submodule1<B: Backend> {
    conv2d1: Conv2d<B>,
    conv2d2: Conv2d<B>,
    maxpool2d1: MaxPool2d,
    conv2d3: Conv2d<B>,
    maxpool2d2: MaxPool2d,
    conv2d4: Conv2d<B>,
    maxpool2d3: MaxPool2d,
    conv2d5: Conv2d<B>,
    maxpool2d4: MaxPool2d,
    conv2d6: Conv2d<B>,
    maxpool2d5: MaxPool2d,
    conv2d7: Conv2d<B>,
    conv2d8: Conv2d<B>,
    conv2d9: Conv2d<B>,
    concat2: burn::module::Param<Tensor<B, 1, Int>>,
    conv2d10: Conv2d<B>,
    concat5: burn::module::Param<Tensor<B, 1, Int>>,
    conv2d11: Conv2d<B>,
    concat8: burn::module::Param<Tensor<B, 1, Int>>,
    conv2d12: Conv2d<B>,
    concat11: burn::module::Param<Tensor<B, 1, Int>>,
    conv2d13: Conv2d<B>,
    concat14: burn::module::Param<Tensor<B, 1, Int>>,
    conv2d14: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule1<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let conv2d1 = Conv2dConfig::new([3, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d2 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d1 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d3 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d2 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d4 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d3 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d5 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d4 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d6 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d5 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d7 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d8 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d9 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let concat2: burn::module::Param<Tensor<B, 1, Int>> = burn::module::Param::uninitialized(
            burn::module::ParamId::new(),
            move |device, _require_grad| {
                Tensor::<B, 1, Int>::zeros([2], (device, burn::tensor::DType::I64))
            },
            device.clone(),
            false,
            [2].into(),
        );
        let conv2d10 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let concat5: burn::module::Param<Tensor<B, 1, Int>> = burn::module::Param::uninitialized(
            burn::module::ParamId::new(),
            move |device, _require_grad| {
                Tensor::<B, 1, Int>::zeros([2], (device, burn::tensor::DType::I64))
            },
            device.clone(),
            false,
            [2].into(),
        );
        let conv2d11 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let concat8: burn::module::Param<Tensor<B, 1, Int>> = burn::module::Param::uninitialized(
            burn::module::ParamId::new(),
            move |device, _require_grad| {
                Tensor::<B, 1, Int>::zeros([2], (device, burn::tensor::DType::I64))
            },
            device.clone(),
            false,
            [2].into(),
        );
        let conv2d12 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let concat11: burn::module::Param<Tensor<B, 1, Int>> = burn::module::Param::uninitialized(
            burn::module::ParamId::new(),
            move |device, _require_grad| {
                Tensor::<B, 1, Int>::zeros([2], (device, burn::tensor::DType::I64))
            },
            device.clone(),
            false,
            [2].into(),
        );
        let conv2d13 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let concat14: burn::module::Param<Tensor<B, 1, Int>> = burn::module::Param::uninitialized(
            burn::module::ParamId::new(),
            move |device, _require_grad| {
                Tensor::<B, 1, Int>::zeros([2], (device, burn::tensor::DType::I64))
            },
            device.clone(),
            false,
            [2].into(),
        );
        let conv2d14 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            conv2d1,
            conv2d2,
            maxpool2d1,
            conv2d3,
            maxpool2d2,
            conv2d4,
            maxpool2d3,
            conv2d5,
            maxpool2d4,
            conv2d6,
            maxpool2d5,
            conv2d7,
            conv2d8,
            conv2d9,
            concat2,
            conv2d10,
            concat5,
            conv2d11,
            concat8,
            conv2d12,
            concat11,
            conv2d13,
            concat14,
            conv2d14,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(&self, input_1: Tensor<B, 4>) -> Tensor<B, 4> {
        let conv2d1_out1 = self.conv2d1.forward(input_1);
        let relu1_out1 = burn::tensor::activation::relu(conv2d1_out1);
        let conv2d2_out1 = self.conv2d2.forward(relu1_out1.clone());
        let relu2_out1 = burn::tensor::activation::relu(conv2d2_out1);
        let maxpool2d1_out1 = self.maxpool2d1.forward(relu2_out1.clone());
        let conv2d3_out1 = self.conv2d3.forward(maxpool2d1_out1);
        let relu3_out1 = burn::tensor::activation::relu(conv2d3_out1);
        let maxpool2d2_out1 = self.maxpool2d2.forward(relu3_out1.clone());
        let conv2d4_out1 = self.conv2d4.forward(maxpool2d2_out1);
        let relu4_out1 = burn::tensor::activation::relu(conv2d4_out1);
        let maxpool2d3_out1 = self.maxpool2d3.forward(relu4_out1.clone());
        let conv2d5_out1 = self.conv2d5.forward(maxpool2d3_out1);
        let relu5_out1 = burn::tensor::activation::relu(conv2d5_out1);
        let maxpool2d4_out1 = self.maxpool2d4.forward(relu5_out1.clone());
        let conv2d6_out1 = self.conv2d6.forward(maxpool2d4_out1);
        let relu6_out1 = burn::tensor::activation::relu(conv2d6_out1);
        let maxpool2d5_out1 = self.maxpool2d5.forward(relu6_out1.clone());
        let conv2d7_out1 = self.conv2d7.forward(maxpool2d5_out1);
        let relu7_out1 = burn::tensor::activation::relu(conv2d7_out1);
        let conv2d8_out1 = self.conv2d8.forward(relu7_out1.clone());
        let relu8_out1 = burn::tensor::activation::relu(conv2d8_out1);
        let concat1_out1 = burn::tensor::Tensor::cat([relu8_out1, relu7_out1].into(), 1);
        let conv2d9_out1 = self.conv2d9.forward(concat1_out1);
        let relu9_out1 = burn::tensor::activation::relu(conv2d9_out1);
        let shape3_out1: [i64; 4] = {
            let axes = &relu9_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice1_out1: [i64; 2] = shape3_out1[0..2].try_into().unwrap();
        let skip_dims = relu6_out1.dims();
        let concat2_out1 = [skip_dims[2] as i64, skip_dims[3] as i64];
        let concat3_out1: [i64; 4usize] = [&slice1_out1[..], &concat2_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize1_out1 = {
            let target_height = concat3_out1[2] as usize;
            let target_width = concat3_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu9_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat4_out1 = burn::tensor::Tensor::cat([resize1_out1, relu6_out1].into(), 1);
        let conv2d10_out1 = self.conv2d10.forward(concat4_out1);
        let relu10_out1 = burn::tensor::activation::relu(conv2d10_out1);
        let shape6_out1: [i64; 4] = {
            let axes = &relu10_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice2_out1: [i64; 2] = shape6_out1[0..2].try_into().unwrap();
        let skip_dims = relu5_out1.dims();
        let concat5_out1 = [skip_dims[2] as i64, skip_dims[3] as i64];
        let concat6_out1: [i64; 4usize] = [&slice2_out1[..], &concat5_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize2_out1 = {
            let target_height = concat6_out1[2] as usize;
            let target_width = concat6_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu10_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat7_out1 = burn::tensor::Tensor::cat([resize2_out1, relu5_out1].into(), 1);
        let conv2d11_out1 = self.conv2d11.forward(concat7_out1);
        let relu11_out1 = burn::tensor::activation::relu(conv2d11_out1);
        let shape9_out1: [i64; 4] = {
            let axes = &relu11_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice3_out1: [i64; 2] = shape9_out1[0..2].try_into().unwrap();
        let skip_dims = relu4_out1.dims();
        let concat8_out1 = [skip_dims[2] as i64, skip_dims[3] as i64];
        let concat9_out1: [i64; 4usize] = [&slice3_out1[..], &concat8_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize3_out1 = {
            let target_height = concat9_out1[2] as usize;
            let target_width = concat9_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu11_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat10_out1 = burn::tensor::Tensor::cat([resize3_out1, relu4_out1].into(), 1);
        let conv2d12_out1 = self.conv2d12.forward(concat10_out1);
        let relu12_out1 = burn::tensor::activation::relu(conv2d12_out1);
        let shape12_out1: [i64; 4] = {
            let axes = &relu12_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice4_out1: [i64; 2] = shape12_out1[0..2].try_into().unwrap();
        let skip_dims = relu3_out1.dims();
        let concat11_out1 = [skip_dims[2] as i64, skip_dims[3] as i64];
        let concat12_out1: [i64; 4usize] = [&slice4_out1[..], &concat11_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize4_out1 = {
            let target_height = concat12_out1[2] as usize;
            let target_width = concat12_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu12_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat13_out1 = burn::tensor::Tensor::cat([resize4_out1, relu3_out1].into(), 1);
        let conv2d13_out1 = self.conv2d13.forward(concat13_out1);
        let relu13_out1 = burn::tensor::activation::relu(conv2d13_out1);
        let shape15_out1: [i64; 4] = {
            let axes = &relu13_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice5_out1: [i64; 2] = shape15_out1[0..2].try_into().unwrap();
        let skip_dims = relu2_out1.dims();
        let concat14_out1 = [skip_dims[2] as i64, skip_dims[3] as i64];
        let concat15_out1: [i64; 4usize] = [&slice5_out1[..], &concat14_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize5_out1 = {
            let target_height = concat15_out1[2] as usize;
            let target_width = concat15_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu13_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat16_out1 = burn::tensor::Tensor::cat([resize5_out1, relu2_out1].into(), 1);
        let conv2d14_out1 = self.conv2d14.forward(concat16_out1);
        let relu14_out1 = burn::tensor::activation::relu(conv2d14_out1);
        let add1_out1 = relu14_out1.add(relu1_out1);
        add1_out1
    }
}
#[derive(Module, Debug)]
pub struct Submodule2<B: Backend> {
    maxpool2d6: MaxPool2d,
    conv2d15: Conv2d<B>,
    conv2d16: Conv2d<B>,
    maxpool2d7: MaxPool2d,
    conv2d17: Conv2d<B>,
    maxpool2d8: MaxPool2d,
    conv2d18: Conv2d<B>,
    maxpool2d9: MaxPool2d,
    conv2d19: Conv2d<B>,
    maxpool2d10: MaxPool2d,
    conv2d20: Conv2d<B>,
    conv2d21: Conv2d<B>,
    conv2d22: Conv2d<B>,
    conv2d23: Conv2d<B>,
    conv2d24: Conv2d<B>,
    conv2d25: Conv2d<B>,
    conv2d26: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule2<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let maxpool2d6 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d15 = Conv2dConfig::new([64, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d16 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d7 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d17 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d8 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d18 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d9 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d19 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d10 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d20 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d21 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d22 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d23 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d24 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d25 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d26 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            maxpool2d6,
            conv2d15,
            conv2d16,
            maxpool2d7,
            conv2d17,
            maxpool2d8,
            conv2d18,
            maxpool2d9,
            conv2d19,
            maxpool2d10,
            conv2d20,
            conv2d21,
            conv2d22,
            conv2d23,
            conv2d24,
            conv2d25,
            conv2d26,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(&self, add1_out1: Tensor<B, 4>) -> Tensor<B, 4> {
        let maxpool2d6_out1 = self.maxpool2d6.forward(add1_out1);
        let conv2d15_out1 = self.conv2d15.forward(maxpool2d6_out1);
        let relu15_out1 = burn::tensor::activation::relu(conv2d15_out1);
        let conv2d16_out1 = self.conv2d16.forward(relu15_out1.clone());
        let relu16_out1 = burn::tensor::activation::relu(conv2d16_out1);
        let maxpool2d7_out1 = self.maxpool2d7.forward(relu16_out1.clone());
        let conv2d17_out1 = self.conv2d17.forward(maxpool2d7_out1);
        let relu17_out1 = burn::tensor::activation::relu(conv2d17_out1);
        let maxpool2d8_out1 = self.maxpool2d8.forward(relu17_out1.clone());
        let conv2d18_out1 = self.conv2d18.forward(maxpool2d8_out1);
        let relu18_out1 = burn::tensor::activation::relu(conv2d18_out1);
        let maxpool2d9_out1 = self.maxpool2d9.forward(relu18_out1.clone());
        let conv2d19_out1 = self.conv2d19.forward(maxpool2d9_out1);
        let relu19_out1 = burn::tensor::activation::relu(conv2d19_out1);
        let maxpool2d10_out1 = self.maxpool2d10.forward(relu19_out1.clone());
        let conv2d20_out1 = self.conv2d20.forward(maxpool2d10_out1);
        let relu20_out1 = burn::tensor::activation::relu(conv2d20_out1);
        let conv2d21_out1 = self.conv2d21.forward(relu20_out1.clone());
        let relu21_out1 = burn::tensor::activation::relu(conv2d21_out1);
        let concat17_out1 = burn::tensor::Tensor::cat([relu21_out1, relu20_out1].into(), 1);
        let conv2d22_out1 = self.conv2d22.forward(concat17_out1);
        let relu22_out1 = burn::tensor::activation::relu(conv2d22_out1);
        let shape16_out1: [i64; 4] = {
            let axes = &relu19_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather11_out1 = shape16_out1[2] as i64;
        let gather12_out1 = shape16_out1[3] as i64;
        let unsqueeze11_out1 = [gather11_out1 as i64];
        let unsqueeze12_out1 = [gather12_out1 as i64];
        let concat18_out1: [i64; 2usize] = [&unsqueeze11_out1[..], &unsqueeze12_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape18_out1: [i64; 4] = {
            let axes = &relu22_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice6_out1: [i64; 2] = shape18_out1[0..2].try_into().unwrap();
        let concat19_out1: [i64; 4usize] = [&slice6_out1[..], &concat18_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize6_out1 = {
            let target_height = concat19_out1[2] as usize;
            let target_width = concat19_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu22_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat20_out1 = burn::tensor::Tensor::cat([resize6_out1, relu19_out1].into(), 1);
        let conv2d23_out1 = self.conv2d23.forward(concat20_out1);
        let relu23_out1 = burn::tensor::activation::relu(conv2d23_out1);
        let shape19_out1: [i64; 4] = {
            let axes = &relu18_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather13_out1 = shape19_out1[2] as i64;
        let gather14_out1 = shape19_out1[3] as i64;
        let unsqueeze13_out1 = [gather13_out1 as i64];
        let unsqueeze14_out1 = [gather14_out1 as i64];
        let concat21_out1: [i64; 2usize] = [&unsqueeze13_out1[..], &unsqueeze14_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape21_out1: [i64; 4] = {
            let axes = &relu23_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice7_out1: [i64; 2] = shape21_out1[0..2].try_into().unwrap();
        let concat22_out1: [i64; 4usize] = [&slice7_out1[..], &concat21_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize7_out1 = {
            let target_height = concat22_out1[2] as usize;
            let target_width = concat22_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu23_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat23_out1 = burn::tensor::Tensor::cat([resize7_out1, relu18_out1].into(), 1);
        let conv2d24_out1 = self.conv2d24.forward(concat23_out1);
        let relu24_out1 = burn::tensor::activation::relu(conv2d24_out1);
        let shape22_out1: [i64; 4] = {
            let axes = &relu17_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather15_out1 = shape22_out1[2] as i64;
        let gather16_out1 = shape22_out1[3] as i64;
        let unsqueeze15_out1 = [gather15_out1 as i64];
        let unsqueeze16_out1 = [gather16_out1 as i64];
        let concat24_out1: [i64; 2usize] = [&unsqueeze15_out1[..], &unsqueeze16_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape24_out1: [i64; 4] = {
            let axes = &relu24_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice8_out1: [i64; 2] = shape24_out1[0..2].try_into().unwrap();
        let concat25_out1: [i64; 4usize] = [&slice8_out1[..], &concat24_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize8_out1 = {
            let target_height = concat25_out1[2] as usize;
            let target_width = concat25_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu24_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat26_out1 = burn::tensor::Tensor::cat([resize8_out1, relu17_out1].into(), 1);
        let conv2d25_out1 = self.conv2d25.forward(concat26_out1);
        let relu25_out1 = burn::tensor::activation::relu(conv2d25_out1);
        let shape25_out1: [i64; 4] = {
            let axes = &relu16_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather17_out1 = shape25_out1[2] as i64;
        let gather18_out1 = shape25_out1[3] as i64;
        let unsqueeze17_out1 = [gather17_out1 as i64];
        let unsqueeze18_out1 = [gather18_out1 as i64];
        let concat27_out1: [i64; 2usize] = [&unsqueeze17_out1[..], &unsqueeze18_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape27_out1: [i64; 4] = {
            let axes = &relu25_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice9_out1: [i64; 2] = shape27_out1[0..2].try_into().unwrap();
        let concat28_out1: [i64; 4usize] = [&slice9_out1[..], &concat27_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize9_out1 = {
            let target_height = concat28_out1[2] as usize;
            let target_width = concat28_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu25_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat29_out1 = burn::tensor::Tensor::cat([resize9_out1, relu16_out1].into(), 1);
        let conv2d26_out1 = self.conv2d26.forward(concat29_out1);
        let relu26_out1 = burn::tensor::activation::relu(conv2d26_out1);
        let add2_out1 = relu26_out1.add(relu15_out1);
        add2_out1
    }
}
#[derive(Module, Debug)]
pub struct Submodule3<B: Backend> {
    maxpool2d11: MaxPool2d,
    conv2d27: Conv2d<B>,
    conv2d28: Conv2d<B>,
    maxpool2d12: MaxPool2d,
    conv2d29: Conv2d<B>,
    maxpool2d13: MaxPool2d,
    conv2d30: Conv2d<B>,
    maxpool2d14: MaxPool2d,
    conv2d31: Conv2d<B>,
    conv2d32: Conv2d<B>,
    conv2d33: Conv2d<B>,
    conv2d34: Conv2d<B>,
    conv2d35: Conv2d<B>,
    conv2d36: Conv2d<B>,
    maxpool2d15: MaxPool2d,
    conv2d37: Conv2d<B>,
    conv2d38: Conv2d<B>,
    maxpool2d16: MaxPool2d,
    conv2d39: Conv2d<B>,
    maxpool2d17: MaxPool2d,
    conv2d40: Conv2d<B>,
    conv2d41: Conv2d<B>,
    conv2d42: Conv2d<B>,
    conv2d43: Conv2d<B>,
    conv2d44: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule3<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let maxpool2d11 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d27 = Conv2dConfig::new([64, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d28 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d12 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d29 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d13 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d30 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d14 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d31 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d32 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d33 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d34 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d35 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d36 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d15 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d37 = Conv2dConfig::new([64, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d38 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d16 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d39 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d17 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d40 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d41 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d42 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d43 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d44 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            maxpool2d11,
            conv2d27,
            conv2d28,
            maxpool2d12,
            conv2d29,
            maxpool2d13,
            conv2d30,
            maxpool2d14,
            conv2d31,
            conv2d32,
            conv2d33,
            conv2d34,
            conv2d35,
            conv2d36,
            maxpool2d15,
            conv2d37,
            conv2d38,
            maxpool2d16,
            conv2d39,
            maxpool2d17,
            conv2d40,
            conv2d41,
            conv2d42,
            conv2d43,
            conv2d44,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(&self, add2_out1: Tensor<B, 4>) -> (Tensor<B, 4>, Tensor<B, 4>) {
        let maxpool2d11_out1 = self.maxpool2d11.forward(add2_out1);
        let conv2d27_out1 = self.conv2d27.forward(maxpool2d11_out1);
        let relu27_out1 = burn::tensor::activation::relu(conv2d27_out1);
        let conv2d28_out1 = self.conv2d28.forward(relu27_out1.clone());
        let relu28_out1 = burn::tensor::activation::relu(conv2d28_out1);
        let maxpool2d12_out1 = self.maxpool2d12.forward(relu28_out1.clone());
        let conv2d29_out1 = self.conv2d29.forward(maxpool2d12_out1);
        let relu29_out1 = burn::tensor::activation::relu(conv2d29_out1);
        let maxpool2d13_out1 = self.maxpool2d13.forward(relu29_out1.clone());
        let conv2d30_out1 = self.conv2d30.forward(maxpool2d13_out1);
        let relu30_out1 = burn::tensor::activation::relu(conv2d30_out1);
        let maxpool2d14_out1 = self.maxpool2d14.forward(relu30_out1.clone());
        let conv2d31_out1 = self.conv2d31.forward(maxpool2d14_out1);
        let relu31_out1 = burn::tensor::activation::relu(conv2d31_out1);
        let conv2d32_out1 = self.conv2d32.forward(relu31_out1.clone());
        let relu32_out1 = burn::tensor::activation::relu(conv2d32_out1);
        let concat30_out1 = burn::tensor::Tensor::cat([relu32_out1, relu31_out1].into(), 1);
        let conv2d33_out1 = self.conv2d33.forward(concat30_out1);
        let relu33_out1 = burn::tensor::activation::relu(conv2d33_out1);
        let shape28_out1: [i64; 4] = {
            let axes = &relu30_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather19_out1 = shape28_out1[2] as i64;
        let gather20_out1 = shape28_out1[3] as i64;
        let unsqueeze19_out1 = [gather19_out1 as i64];
        let unsqueeze20_out1 = [gather20_out1 as i64];
        let concat31_out1: [i64; 2usize] = [&unsqueeze19_out1[..], &unsqueeze20_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape30_out1: [i64; 4] = {
            let axes = &relu33_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice10_out1: [i64; 2] = shape30_out1[0..2].try_into().unwrap();
        let concat32_out1: [i64; 4usize] = [&slice10_out1[..], &concat31_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize10_out1 = {
            let target_height = concat32_out1[2] as usize;
            let target_width = concat32_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu33_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat33_out1 = burn::tensor::Tensor::cat([resize10_out1, relu30_out1].into(), 1);
        let conv2d34_out1 = self.conv2d34.forward(concat33_out1);
        let relu34_out1 = burn::tensor::activation::relu(conv2d34_out1);
        let shape31_out1: [i64; 4] = {
            let axes = &relu29_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather21_out1 = shape31_out1[2] as i64;
        let gather22_out1 = shape31_out1[3] as i64;
        let unsqueeze21_out1 = [gather21_out1 as i64];
        let unsqueeze22_out1 = [gather22_out1 as i64];
        let concat34_out1: [i64; 2usize] = [&unsqueeze21_out1[..], &unsqueeze22_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape33_out1: [i64; 4] = {
            let axes = &relu34_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice11_out1: [i64; 2] = shape33_out1[0..2].try_into().unwrap();
        let concat35_out1: [i64; 4usize] = [&slice11_out1[..], &concat34_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize11_out1 = {
            let target_height = concat35_out1[2] as usize;
            let target_width = concat35_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu34_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat36_out1 = burn::tensor::Tensor::cat([resize11_out1, relu29_out1].into(), 1);
        let conv2d35_out1 = self.conv2d35.forward(concat36_out1);
        let relu35_out1 = burn::tensor::activation::relu(conv2d35_out1);
        let shape34_out1: [i64; 4] = {
            let axes = &relu28_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather23_out1 = shape34_out1[2] as i64;
        let gather24_out1 = shape34_out1[3] as i64;
        let unsqueeze23_out1 = [gather23_out1 as i64];
        let unsqueeze24_out1 = [gather24_out1 as i64];
        let concat37_out1: [i64; 2usize] = [&unsqueeze23_out1[..], &unsqueeze24_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape36_out1: [i64; 4] = {
            let axes = &relu35_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice12_out1: [i64; 2] = shape36_out1[0..2].try_into().unwrap();
        let concat38_out1: [i64; 4usize] = [&slice12_out1[..], &concat37_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize12_out1 = {
            let target_height = concat38_out1[2] as usize;
            let target_width = concat38_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu35_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat39_out1 = burn::tensor::Tensor::cat([resize12_out1, relu28_out1].into(), 1);
        let conv2d36_out1 = self.conv2d36.forward(concat39_out1);
        let relu36_out1 = burn::tensor::activation::relu(conv2d36_out1);
        let add3_out1 = relu36_out1.add(relu27_out1);
        let maxpool2d15_out1 = self.maxpool2d15.forward(add3_out1.clone());
        let conv2d37_out1 = self.conv2d37.forward(maxpool2d15_out1);
        let relu37_out1 = burn::tensor::activation::relu(conv2d37_out1);
        let conv2d38_out1 = self.conv2d38.forward(relu37_out1.clone());
        let relu38_out1 = burn::tensor::activation::relu(conv2d38_out1);
        let maxpool2d16_out1 = self.maxpool2d16.forward(relu38_out1.clone());
        let conv2d39_out1 = self.conv2d39.forward(maxpool2d16_out1);
        let relu39_out1 = burn::tensor::activation::relu(conv2d39_out1);
        let maxpool2d17_out1 = self.maxpool2d17.forward(relu39_out1.clone());
        let conv2d40_out1 = self.conv2d40.forward(maxpool2d17_out1);
        let relu40_out1 = burn::tensor::activation::relu(conv2d40_out1);
        let conv2d41_out1 = self.conv2d41.forward(relu40_out1.clone());
        let relu41_out1 = burn::tensor::activation::relu(conv2d41_out1);
        let concat40_out1 = burn::tensor::Tensor::cat([relu41_out1, relu40_out1].into(), 1);
        let conv2d42_out1 = self.conv2d42.forward(concat40_out1);
        let relu42_out1 = burn::tensor::activation::relu(conv2d42_out1);
        let shape37_out1: [i64; 4] = {
            let axes = &relu39_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather25_out1 = shape37_out1[2] as i64;
        let gather26_out1 = shape37_out1[3] as i64;
        let unsqueeze25_out1 = [gather25_out1 as i64];
        let unsqueeze26_out1 = [gather26_out1 as i64];
        let concat41_out1: [i64; 2usize] = [&unsqueeze25_out1[..], &unsqueeze26_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape39_out1: [i64; 4] = {
            let axes = &relu42_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice13_out1: [i64; 2] = shape39_out1[0..2].try_into().unwrap();
        let concat42_out1: [i64; 4usize] = [&slice13_out1[..], &concat41_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize13_out1 = {
            let target_height = concat42_out1[2] as usize;
            let target_width = concat42_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu42_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat43_out1 = burn::tensor::Tensor::cat([resize13_out1, relu39_out1].into(), 1);
        let conv2d43_out1 = self.conv2d43.forward(concat43_out1);
        let relu43_out1 = burn::tensor::activation::relu(conv2d43_out1);
        let shape40_out1: [i64; 4] = {
            let axes = &relu38_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather27_out1 = shape40_out1[2] as i64;
        let gather28_out1 = shape40_out1[3] as i64;
        let unsqueeze27_out1 = [gather27_out1 as i64];
        let unsqueeze28_out1 = [gather28_out1 as i64];
        let concat44_out1: [i64; 2usize] = [&unsqueeze27_out1[..], &unsqueeze28_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape42_out1: [i64; 4] = {
            let axes = &relu43_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice14_out1: [i64; 2] = shape42_out1[0..2].try_into().unwrap();
        let concat45_out1: [i64; 4usize] = [&slice14_out1[..], &concat44_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize14_out1 = {
            let target_height = concat45_out1[2] as usize;
            let target_width = concat45_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu43_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat46_out1 = burn::tensor::Tensor::cat([resize14_out1, relu38_out1].into(), 1);
        let conv2d44_out1 = self.conv2d44.forward(concat46_out1);
        let relu44_out1 = burn::tensor::activation::relu(conv2d44_out1);
        let add4_out1 = relu44_out1.add(relu37_out1);
        (add4_out1, add3_out1)
    }
}
#[derive(Module, Debug)]
pub struct Submodule4<B: Backend> {
    maxpool2d18: MaxPool2d,
    conv2d45: Conv2d<B>,
    conv2d46: Conv2d<B>,
    conv2d47: Conv2d<B>,
    conv2d48: Conv2d<B>,
    conv2d49: Conv2d<B>,
    conv2d50: Conv2d<B>,
    conv2d51: Conv2d<B>,
    conv2d52: Conv2d<B>,
    maxpool2d19: MaxPool2d,
    conv2d53: Conv2d<B>,
    conv2d54: Conv2d<B>,
    conv2d55: Conv2d<B>,
    conv2d56: Conv2d<B>,
    conv2d57: Conv2d<B>,
    conv2d58: Conv2d<B>,
    conv2d59: Conv2d<B>,
    conv2d60: Conv2d<B>,
    conv2d61: Conv2d<B>,
    conv2d62: Conv2d<B>,
    conv2d63: Conv2d<B>,
    conv2d64: Conv2d<B>,
    conv2d65: Conv2d<B>,
    conv2d66: Conv2d<B>,
    conv2d67: Conv2d<B>,
    conv2d68: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule4<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let maxpool2d18 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d45 = Conv2dConfig::new([64, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d46 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d47 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d48 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(4, 4, 4, 4))
            .with_dilation([4, 4])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d49 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(8, 8, 8, 8))
            .with_dilation([8, 8])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d50 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(4, 4, 4, 4))
            .with_dilation([4, 4])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d51 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d52 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d19 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d53 = Conv2dConfig::new([64, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d54 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d55 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d56 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(4, 4, 4, 4))
            .with_dilation([4, 4])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d57 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(8, 8, 8, 8))
            .with_dilation([8, 8])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d58 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(4, 4, 4, 4))
            .with_dilation([4, 4])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d59 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d60 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d61 = Conv2dConfig::new([128, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d62 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d63 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d64 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(4, 4, 4, 4))
            .with_dilation([4, 4])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d65 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(8, 8, 8, 8))
            .with_dilation([8, 8])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d66 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(4, 4, 4, 4))
            .with_dilation([4, 4])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d67 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d68 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            maxpool2d18,
            conv2d45,
            conv2d46,
            conv2d47,
            conv2d48,
            conv2d49,
            conv2d50,
            conv2d51,
            conv2d52,
            maxpool2d19,
            conv2d53,
            conv2d54,
            conv2d55,
            conv2d56,
            conv2d57,
            conv2d58,
            conv2d59,
            conv2d60,
            conv2d61,
            conv2d62,
            conv2d63,
            conv2d64,
            conv2d65,
            conv2d66,
            conv2d67,
            conv2d68,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(&self, add4_out1: Tensor<B, 4>) -> (Tensor<B, 4>, Tensor<B, 4>) {
        let maxpool2d18_out1 = self.maxpool2d18.forward(add4_out1);
        let conv2d45_out1 = self.conv2d45.forward(maxpool2d18_out1);
        let relu45_out1 = burn::tensor::activation::relu(conv2d45_out1);
        let conv2d46_out1 = self.conv2d46.forward(relu45_out1.clone());
        let relu46_out1 = burn::tensor::activation::relu(conv2d46_out1);
        let conv2d47_out1 = self.conv2d47.forward(relu46_out1.clone());
        let relu47_out1 = burn::tensor::activation::relu(conv2d47_out1);
        let conv2d48_out1 = self.conv2d48.forward(relu47_out1.clone());
        let relu48_out1 = burn::tensor::activation::relu(conv2d48_out1);
        let conv2d49_out1 = self.conv2d49.forward(relu48_out1.clone());
        let relu49_out1 = burn::tensor::activation::relu(conv2d49_out1);
        let concat47_out1 = burn::tensor::Tensor::cat([relu49_out1, relu48_out1].into(), 1);
        let conv2d50_out1 = self.conv2d50.forward(concat47_out1);
        let relu50_out1 = burn::tensor::activation::relu(conv2d50_out1);
        let concat48_out1 = burn::tensor::Tensor::cat([relu50_out1, relu47_out1].into(), 1);
        let conv2d51_out1 = self.conv2d51.forward(concat48_out1);
        let relu51_out1 = burn::tensor::activation::relu(conv2d51_out1);
        let concat49_out1 = burn::tensor::Tensor::cat([relu51_out1, relu46_out1].into(), 1);
        let conv2d52_out1 = self.conv2d52.forward(concat49_out1);
        let relu52_out1 = burn::tensor::activation::relu(conv2d52_out1);
        let add5_out1 = relu52_out1.add(relu45_out1);
        let maxpool2d19_out1 = self.maxpool2d19.forward(add5_out1.clone());
        let conv2d53_out1 = self.conv2d53.forward(maxpool2d19_out1);
        let relu53_out1 = burn::tensor::activation::relu(conv2d53_out1);
        let conv2d54_out1 = self.conv2d54.forward(relu53_out1.clone());
        let relu54_out1 = burn::tensor::activation::relu(conv2d54_out1);
        let conv2d55_out1 = self.conv2d55.forward(relu54_out1.clone());
        let relu55_out1 = burn::tensor::activation::relu(conv2d55_out1);
        let conv2d56_out1 = self.conv2d56.forward(relu55_out1.clone());
        let relu56_out1 = burn::tensor::activation::relu(conv2d56_out1);
        let conv2d57_out1 = self.conv2d57.forward(relu56_out1.clone());
        let relu57_out1 = burn::tensor::activation::relu(conv2d57_out1);
        let concat50_out1 = burn::tensor::Tensor::cat([relu57_out1, relu56_out1].into(), 1);
        let conv2d58_out1 = self.conv2d58.forward(concat50_out1);
        let relu58_out1 = burn::tensor::activation::relu(conv2d58_out1);
        let concat51_out1 = burn::tensor::Tensor::cat([relu58_out1, relu55_out1].into(), 1);
        let conv2d59_out1 = self.conv2d59.forward(concat51_out1);
        let relu59_out1 = burn::tensor::activation::relu(conv2d59_out1);
        let concat52_out1 = burn::tensor::Tensor::cat([relu59_out1, relu54_out1].into(), 1);
        let conv2d60_out1 = self.conv2d60.forward(concat52_out1);
        let relu60_out1 = burn::tensor::activation::relu(conv2d60_out1);
        let add6_out1 = relu60_out1.add(relu53_out1);
        let shape43_out1: [i64; 4] = {
            let axes = &add5_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather29_out1 = shape43_out1[2] as i64;
        let gather30_out1 = shape43_out1[3] as i64;
        let unsqueeze29_out1 = [gather29_out1 as i64];
        let unsqueeze30_out1 = [gather30_out1 as i64];
        let concat53_out1: [i64; 2usize] = [&unsqueeze29_out1[..], &unsqueeze30_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape45_out1: [i64; 4] = {
            let axes = &add6_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice15_out1: [i64; 2] = shape45_out1[0..2].try_into().unwrap();
        let concat54_out1: [i64; 4usize] = [&slice15_out1[..], &concat53_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize15_out1 = {
            let target_height = concat54_out1[2] as usize;
            let target_width = concat54_out1[3] as usize;
            burn::tensor::module::interpolate(
                add6_out1.clone(),
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat55_out1 = burn::tensor::Tensor::cat([resize15_out1, add5_out1].into(), 1);
        let conv2d61_out1 = self.conv2d61.forward(concat55_out1);
        let relu61_out1 = burn::tensor::activation::relu(conv2d61_out1);
        let conv2d62_out1 = self.conv2d62.forward(relu61_out1.clone());
        let relu62_out1 = burn::tensor::activation::relu(conv2d62_out1);
        let conv2d63_out1 = self.conv2d63.forward(relu62_out1.clone());
        let relu63_out1 = burn::tensor::activation::relu(conv2d63_out1);
        let conv2d64_out1 = self.conv2d64.forward(relu63_out1.clone());
        let relu64_out1 = burn::tensor::activation::relu(conv2d64_out1);
        let conv2d65_out1 = self.conv2d65.forward(relu64_out1.clone());
        let relu65_out1 = burn::tensor::activation::relu(conv2d65_out1);
        let concat56_out1 = burn::tensor::Tensor::cat([relu65_out1, relu64_out1].into(), 1);
        let conv2d66_out1 = self.conv2d66.forward(concat56_out1);
        let relu66_out1 = burn::tensor::activation::relu(conv2d66_out1);
        let concat57_out1 = burn::tensor::Tensor::cat([relu66_out1, relu63_out1].into(), 1);
        let conv2d67_out1 = self.conv2d67.forward(concat57_out1);
        let relu67_out1 = burn::tensor::activation::relu(conv2d67_out1);
        let concat58_out1 = burn::tensor::Tensor::cat([relu67_out1, relu62_out1].into(), 1);
        let conv2d68_out1 = self.conv2d68.forward(concat58_out1);
        let relu68_out1 = burn::tensor::activation::relu(conv2d68_out1);
        let add7_out1 = relu68_out1.add(relu61_out1);
        (add7_out1, add6_out1)
    }
}
#[derive(Module, Debug)]
pub struct Submodule5<B: Backend> {
    conv2d69: Conv2d<B>,
    conv2d70: Conv2d<B>,
    maxpool2d20: MaxPool2d,
    conv2d71: Conv2d<B>,
    maxpool2d21: MaxPool2d,
    conv2d72: Conv2d<B>,
    conv2d73: Conv2d<B>,
    conv2d74: Conv2d<B>,
    conv2d75: Conv2d<B>,
    conv2d76: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule5<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let conv2d69 = Conv2dConfig::new([128, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d70 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d20 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d71 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d21 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d72 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d73 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d74 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d75 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d76 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            conv2d69,
            conv2d70,
            maxpool2d20,
            conv2d71,
            maxpool2d21,
            conv2d72,
            conv2d73,
            conv2d74,
            conv2d75,
            conv2d76,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(
        &self,
        add4_out1: Tensor<B, 4>,
        add7_out1: Tensor<B, 4>,
        add3_out1: Tensor<B, 4>,
    ) -> (Tensor<B, 4>, Tensor<B, 4>) {
        let shape46_out1: [i64; 4] = {
            let axes = &add4_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather31_out1 = shape46_out1[2] as i64;
        let gather32_out1 = shape46_out1[3] as i64;
        let unsqueeze31_out1 = [gather31_out1 as i64];
        let unsqueeze32_out1 = [gather32_out1 as i64];
        let concat59_out1: [i64; 2usize] = [&unsqueeze31_out1[..], &unsqueeze32_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape48_out1: [i64; 4] = {
            let axes = &add7_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice16_out1: [i64; 2] = shape48_out1[0..2].try_into().unwrap();
        let concat60_out1: [i64; 4usize] = [&slice16_out1[..], &concat59_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize16_out1 = {
            let target_height = concat60_out1[2] as usize;
            let target_width = concat60_out1[3] as usize;
            burn::tensor::module::interpolate(
                add7_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat61_out1 = burn::tensor::Tensor::cat([resize16_out1, add4_out1].into(), 1);
        let conv2d69_out1 = self.conv2d69.forward(concat61_out1);
        let relu69_out1 = burn::tensor::activation::relu(conv2d69_out1);
        let conv2d70_out1 = self.conv2d70.forward(relu69_out1.clone());
        let relu70_out1 = burn::tensor::activation::relu(conv2d70_out1);
        let maxpool2d20_out1 = self.maxpool2d20.forward(relu70_out1.clone());
        let conv2d71_out1 = self.conv2d71.forward(maxpool2d20_out1);
        let relu71_out1 = burn::tensor::activation::relu(conv2d71_out1);
        let maxpool2d21_out1 = self.maxpool2d21.forward(relu71_out1.clone());
        let conv2d72_out1 = self.conv2d72.forward(maxpool2d21_out1);
        let relu72_out1 = burn::tensor::activation::relu(conv2d72_out1);
        let conv2d73_out1 = self.conv2d73.forward(relu72_out1.clone());
        let relu73_out1 = burn::tensor::activation::relu(conv2d73_out1);
        let concat62_out1 = burn::tensor::Tensor::cat([relu73_out1, relu72_out1].into(), 1);
        let conv2d74_out1 = self.conv2d74.forward(concat62_out1);
        let relu74_out1 = burn::tensor::activation::relu(conv2d74_out1);
        let shape49_out1: [i64; 4] = {
            let axes = &relu71_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather33_out1 = shape49_out1[2] as i64;
        let gather34_out1 = shape49_out1[3] as i64;
        let unsqueeze33_out1 = [gather33_out1 as i64];
        let unsqueeze34_out1 = [gather34_out1 as i64];
        let concat63_out1: [i64; 2usize] = [&unsqueeze33_out1[..], &unsqueeze34_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape51_out1: [i64; 4] = {
            let axes = &relu74_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice17_out1: [i64; 2] = shape51_out1[0..2].try_into().unwrap();
        let concat64_out1: [i64; 4usize] = [&slice17_out1[..], &concat63_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize17_out1 = {
            let target_height = concat64_out1[2] as usize;
            let target_width = concat64_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu74_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat65_out1 = burn::tensor::Tensor::cat([resize17_out1, relu71_out1].into(), 1);
        let conv2d75_out1 = self.conv2d75.forward(concat65_out1);
        let relu75_out1 = burn::tensor::activation::relu(conv2d75_out1);
        let shape52_out1: [i64; 4] = {
            let axes = &relu70_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather35_out1 = shape52_out1[2] as i64;
        let gather36_out1 = shape52_out1[3] as i64;
        let unsqueeze35_out1 = [gather35_out1 as i64];
        let unsqueeze36_out1 = [gather36_out1 as i64];
        let concat66_out1: [i64; 2usize] = [&unsqueeze35_out1[..], &unsqueeze36_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape54_out1: [i64; 4] = {
            let axes = &relu75_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice18_out1: [i64; 2] = shape54_out1[0..2].try_into().unwrap();
        let concat67_out1: [i64; 4usize] = [&slice18_out1[..], &concat66_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize18_out1 = {
            let target_height = concat67_out1[2] as usize;
            let target_width = concat67_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu75_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat68_out1 = burn::tensor::Tensor::cat([resize18_out1, relu70_out1].into(), 1);
        let conv2d76_out1 = self.conv2d76.forward(concat68_out1);
        let relu76_out1 = burn::tensor::activation::relu(conv2d76_out1);
        let add8_out1 = relu76_out1.add(relu69_out1);
        let shape55_out1: [i64; 4] = {
            let axes = &add3_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather37_out1 = shape55_out1[2] as i64;
        let gather38_out1 = shape55_out1[3] as i64;
        let unsqueeze37_out1 = [gather37_out1 as i64];
        let unsqueeze38_out1 = [gather38_out1 as i64];
        let concat69_out1: [i64; 2usize] = [&unsqueeze37_out1[..], &unsqueeze38_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape57_out1: [i64; 4] = {
            let axes = &add8_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice19_out1: [i64; 2] = shape57_out1[0..2].try_into().unwrap();
        let concat70_out1: [i64; 4usize] = [&slice19_out1[..], &concat69_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize19_out1 = {
            let target_height = concat70_out1[2] as usize;
            let target_width = concat70_out1[3] as usize;
            burn::tensor::module::interpolate(
                add8_out1.clone(),
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat71_out1 = burn::tensor::Tensor::cat([resize19_out1, add3_out1].into(), 1);
        (concat71_out1, add8_out1)
    }
}
#[derive(Module, Debug)]
pub struct Submodule6<B: Backend> {
    conv2d77: Conv2d<B>,
    conv2d78: Conv2d<B>,
    maxpool2d22: MaxPool2d,
    conv2d79: Conv2d<B>,
    maxpool2d23: MaxPool2d,
    conv2d80: Conv2d<B>,
    maxpool2d24: MaxPool2d,
    conv2d81: Conv2d<B>,
    conv2d82: Conv2d<B>,
    conv2d83: Conv2d<B>,
    conv2d84: Conv2d<B>,
    conv2d85: Conv2d<B>,
    conv2d86: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule6<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let conv2d77 = Conv2dConfig::new([128, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d78 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d22 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d79 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d23 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d80 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d24 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d81 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d82 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d83 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d84 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d85 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d86 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            conv2d77,
            conv2d78,
            maxpool2d22,
            conv2d79,
            maxpool2d23,
            conv2d80,
            maxpool2d24,
            conv2d81,
            conv2d82,
            conv2d83,
            conv2d84,
            conv2d85,
            conv2d86,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(
        &self,
        concat71_out1: Tensor<B, 4>,
        add2_out1: Tensor<B, 4>,
    ) -> (Tensor<B, 4>, Tensor<B, 4>) {
        let conv2d77_out1 = self.conv2d77.forward(concat71_out1);
        let relu77_out1 = burn::tensor::activation::relu(conv2d77_out1);
        let conv2d78_out1 = self.conv2d78.forward(relu77_out1.clone());
        let relu78_out1 = burn::tensor::activation::relu(conv2d78_out1);
        let maxpool2d22_out1 = self.maxpool2d22.forward(relu78_out1.clone());
        let conv2d79_out1 = self.conv2d79.forward(maxpool2d22_out1);
        let relu79_out1 = burn::tensor::activation::relu(conv2d79_out1);
        let maxpool2d23_out1 = self.maxpool2d23.forward(relu79_out1.clone());
        let conv2d80_out1 = self.conv2d80.forward(maxpool2d23_out1);
        let relu80_out1 = burn::tensor::activation::relu(conv2d80_out1);
        let maxpool2d24_out1 = self.maxpool2d24.forward(relu80_out1.clone());
        let conv2d81_out1 = self.conv2d81.forward(maxpool2d24_out1);
        let relu81_out1 = burn::tensor::activation::relu(conv2d81_out1);
        let conv2d82_out1 = self.conv2d82.forward(relu81_out1.clone());
        let relu82_out1 = burn::tensor::activation::relu(conv2d82_out1);
        let concat72_out1 = burn::tensor::Tensor::cat([relu82_out1, relu81_out1].into(), 1);
        let conv2d83_out1 = self.conv2d83.forward(concat72_out1);
        let relu83_out1 = burn::tensor::activation::relu(conv2d83_out1);
        let shape58_out1: [i64; 4] = {
            let axes = &relu80_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather39_out1 = shape58_out1[2] as i64;
        let gather40_out1 = shape58_out1[3] as i64;
        let unsqueeze39_out1 = [gather39_out1 as i64];
        let unsqueeze40_out1 = [gather40_out1 as i64];
        let concat73_out1: [i64; 2usize] = [&unsqueeze39_out1[..], &unsqueeze40_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape60_out1: [i64; 4] = {
            let axes = &relu83_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice20_out1: [i64; 2] = shape60_out1[0..2].try_into().unwrap();
        let concat74_out1: [i64; 4usize] = [&slice20_out1[..], &concat73_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize20_out1 = {
            let target_height = concat74_out1[2] as usize;
            let target_width = concat74_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu83_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat75_out1 = burn::tensor::Tensor::cat([resize20_out1, relu80_out1].into(), 1);
        let conv2d84_out1 = self.conv2d84.forward(concat75_out1);
        let relu84_out1 = burn::tensor::activation::relu(conv2d84_out1);
        let shape61_out1: [i64; 4] = {
            let axes = &relu79_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather41_out1 = shape61_out1[2] as i64;
        let gather42_out1 = shape61_out1[3] as i64;
        let unsqueeze41_out1 = [gather41_out1 as i64];
        let unsqueeze42_out1 = [gather42_out1 as i64];
        let concat76_out1: [i64; 2usize] = [&unsqueeze41_out1[..], &unsqueeze42_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape63_out1: [i64; 4] = {
            let axes = &relu84_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice21_out1: [i64; 2] = shape63_out1[0..2].try_into().unwrap();
        let concat77_out1: [i64; 4usize] = [&slice21_out1[..], &concat76_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize21_out1 = {
            let target_height = concat77_out1[2] as usize;
            let target_width = concat77_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu84_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat78_out1 = burn::tensor::Tensor::cat([resize21_out1, relu79_out1].into(), 1);
        let conv2d85_out1 = self.conv2d85.forward(concat78_out1);
        let relu85_out1 = burn::tensor::activation::relu(conv2d85_out1);
        let shape64_out1: [i64; 4] = {
            let axes = &relu78_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather43_out1 = shape64_out1[2] as i64;
        let gather44_out1 = shape64_out1[3] as i64;
        let unsqueeze43_out1 = [gather43_out1 as i64];
        let unsqueeze44_out1 = [gather44_out1 as i64];
        let concat79_out1: [i64; 2usize] = [&unsqueeze43_out1[..], &unsqueeze44_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape66_out1: [i64; 4] = {
            let axes = &relu85_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice22_out1: [i64; 2] = shape66_out1[0..2].try_into().unwrap();
        let concat80_out1: [i64; 4usize] = [&slice22_out1[..], &concat79_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize22_out1 = {
            let target_height = concat80_out1[2] as usize;
            let target_width = concat80_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu85_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat81_out1 = burn::tensor::Tensor::cat([resize22_out1, relu78_out1].into(), 1);
        let conv2d86_out1 = self.conv2d86.forward(concat81_out1);
        let relu86_out1 = burn::tensor::activation::relu(conv2d86_out1);
        let add9_out1 = relu86_out1.add(relu77_out1);
        let shape67_out1: [i64; 4] = {
            let axes = &add2_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather45_out1 = shape67_out1[2] as i64;
        let gather46_out1 = shape67_out1[3] as i64;
        let unsqueeze45_out1 = [gather45_out1 as i64];
        let unsqueeze46_out1 = [gather46_out1 as i64];
        let concat82_out1: [i64; 2usize] = [&unsqueeze45_out1[..], &unsqueeze46_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape69_out1: [i64; 4] = {
            let axes = &add9_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice23_out1: [i64; 2] = shape69_out1[0..2].try_into().unwrap();
        let concat83_out1: [i64; 4usize] = [&slice23_out1[..], &concat82_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize23_out1 = {
            let target_height = concat83_out1[2] as usize;
            let target_width = concat83_out1[3] as usize;
            burn::tensor::module::interpolate(
                add9_out1.clone(),
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat84_out1 = burn::tensor::Tensor::cat([resize23_out1, add2_out1].into(), 1);
        (concat84_out1, add9_out1)
    }
}
#[derive(Module, Debug)]
pub struct Submodule7<B: Backend> {
    conv2d87: Conv2d<B>,
    conv2d88: Conv2d<B>,
    maxpool2d25: MaxPool2d,
    conv2d89: Conv2d<B>,
    maxpool2d26: MaxPool2d,
    conv2d90: Conv2d<B>,
    maxpool2d27: MaxPool2d,
    conv2d91: Conv2d<B>,
    maxpool2d28: MaxPool2d,
    conv2d92: Conv2d<B>,
    conv2d93: Conv2d<B>,
    conv2d94: Conv2d<B>,
    conv2d95: Conv2d<B>,
    conv2d96: Conv2d<B>,
    conv2d97: Conv2d<B>,
    conv2d98: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule7<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let conv2d87 = Conv2dConfig::new([128, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d88 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d25 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d89 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d26 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d90 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d27 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d91 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d28 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d92 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d93 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d94 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d95 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d96 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d97 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d98 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            conv2d87,
            conv2d88,
            maxpool2d25,
            conv2d89,
            maxpool2d26,
            conv2d90,
            maxpool2d27,
            conv2d91,
            maxpool2d28,
            conv2d92,
            conv2d93,
            conv2d94,
            conv2d95,
            conv2d96,
            conv2d97,
            conv2d98,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(&self, concat84_out1: Tensor<B, 4>) -> Tensor<B, 4> {
        let conv2d87_out1 = self.conv2d87.forward(concat84_out1);
        let relu87_out1 = burn::tensor::activation::relu(conv2d87_out1);
        let conv2d88_out1 = self.conv2d88.forward(relu87_out1.clone());
        let relu88_out1 = burn::tensor::activation::relu(conv2d88_out1);
        let maxpool2d25_out1 = self.maxpool2d25.forward(relu88_out1.clone());
        let conv2d89_out1 = self.conv2d89.forward(maxpool2d25_out1);
        let relu89_out1 = burn::tensor::activation::relu(conv2d89_out1);
        let maxpool2d26_out1 = self.maxpool2d26.forward(relu89_out1.clone());
        let conv2d90_out1 = self.conv2d90.forward(maxpool2d26_out1);
        let relu90_out1 = burn::tensor::activation::relu(conv2d90_out1);
        let maxpool2d27_out1 = self.maxpool2d27.forward(relu90_out1.clone());
        let conv2d91_out1 = self.conv2d91.forward(maxpool2d27_out1);
        let relu91_out1 = burn::tensor::activation::relu(conv2d91_out1);
        let maxpool2d28_out1 = self.maxpool2d28.forward(relu91_out1.clone());
        let conv2d92_out1 = self.conv2d92.forward(maxpool2d28_out1);
        let relu92_out1 = burn::tensor::activation::relu(conv2d92_out1);
        let conv2d93_out1 = self.conv2d93.forward(relu92_out1.clone());
        let relu93_out1 = burn::tensor::activation::relu(conv2d93_out1);
        let concat85_out1 = burn::tensor::Tensor::cat([relu93_out1, relu92_out1].into(), 1);
        let conv2d94_out1 = self.conv2d94.forward(concat85_out1);
        let relu94_out1 = burn::tensor::activation::relu(conv2d94_out1);
        let shape70_out1: [i64; 4] = {
            let axes = &relu91_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather47_out1 = shape70_out1[2] as i64;
        let gather48_out1 = shape70_out1[3] as i64;
        let unsqueeze47_out1 = [gather47_out1 as i64];
        let unsqueeze48_out1 = [gather48_out1 as i64];
        let concat86_out1: [i64; 2usize] = [&unsqueeze47_out1[..], &unsqueeze48_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape72_out1: [i64; 4] = {
            let axes = &relu94_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice24_out1: [i64; 2] = shape72_out1[0..2].try_into().unwrap();
        let concat87_out1: [i64; 4usize] = [&slice24_out1[..], &concat86_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize24_out1 = {
            let target_height = concat87_out1[2] as usize;
            let target_width = concat87_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu94_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat88_out1 = burn::tensor::Tensor::cat([resize24_out1, relu91_out1].into(), 1);
        let conv2d95_out1 = self.conv2d95.forward(concat88_out1);
        let relu95_out1 = burn::tensor::activation::relu(conv2d95_out1);
        let shape73_out1: [i64; 4] = {
            let axes = &relu90_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather49_out1 = shape73_out1[2] as i64;
        let gather50_out1 = shape73_out1[3] as i64;
        let unsqueeze49_out1 = [gather49_out1 as i64];
        let unsqueeze50_out1 = [gather50_out1 as i64];
        let concat89_out1: [i64; 2usize] = [&unsqueeze49_out1[..], &unsqueeze50_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape75_out1: [i64; 4] = {
            let axes = &relu95_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice25_out1: [i64; 2] = shape75_out1[0..2].try_into().unwrap();
        let concat90_out1: [i64; 4usize] = [&slice25_out1[..], &concat89_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize25_out1 = {
            let target_height = concat90_out1[2] as usize;
            let target_width = concat90_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu95_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat91_out1 = burn::tensor::Tensor::cat([resize25_out1, relu90_out1].into(), 1);
        let conv2d96_out1 = self.conv2d96.forward(concat91_out1);
        let relu96_out1 = burn::tensor::activation::relu(conv2d96_out1);
        let shape76_out1: [i64; 4] = {
            let axes = &relu89_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather51_out1 = shape76_out1[2] as i64;
        let gather52_out1 = shape76_out1[3] as i64;
        let unsqueeze51_out1 = [gather51_out1 as i64];
        let unsqueeze52_out1 = [gather52_out1 as i64];
        let concat92_out1: [i64; 2usize] = [&unsqueeze51_out1[..], &unsqueeze52_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape78_out1: [i64; 4] = {
            let axes = &relu96_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice26_out1: [i64; 2] = shape78_out1[0..2].try_into().unwrap();
        let concat93_out1: [i64; 4usize] = [&slice26_out1[..], &concat92_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize26_out1 = {
            let target_height = concat93_out1[2] as usize;
            let target_width = concat93_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu96_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat94_out1 = burn::tensor::Tensor::cat([resize26_out1, relu89_out1].into(), 1);
        let conv2d97_out1 = self.conv2d97.forward(concat94_out1);
        let relu97_out1 = burn::tensor::activation::relu(conv2d97_out1);
        let shape79_out1: [i64; 4] = {
            let axes = &relu88_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather53_out1 = shape79_out1[2] as i64;
        let gather54_out1 = shape79_out1[3] as i64;
        let unsqueeze53_out1 = [gather53_out1 as i64];
        let unsqueeze54_out1 = [gather54_out1 as i64];
        let concat95_out1: [i64; 2usize] = [&unsqueeze53_out1[..], &unsqueeze54_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape81_out1: [i64; 4] = {
            let axes = &relu97_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice27_out1: [i64; 2] = shape81_out1[0..2].try_into().unwrap();
        let concat96_out1: [i64; 4usize] = [&slice27_out1[..], &concat95_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize27_out1 = {
            let target_height = concat96_out1[2] as usize;
            let target_width = concat96_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu97_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat97_out1 = burn::tensor::Tensor::cat([resize27_out1, relu88_out1].into(), 1);
        let conv2d98_out1 = self.conv2d98.forward(concat97_out1);
        let relu98_out1 = burn::tensor::activation::relu(conv2d98_out1);
        let add10_out1 = relu98_out1.add(relu87_out1);
        add10_out1
    }
}
#[derive(Module, Debug)]
pub struct Submodule8<B: Backend> {
    conv2d99: Conv2d<B>,
    conv2d100: Conv2d<B>,
    maxpool2d29: MaxPool2d,
    conv2d101: Conv2d<B>,
    maxpool2d30: MaxPool2d,
    conv2d102: Conv2d<B>,
    maxpool2d31: MaxPool2d,
    conv2d103: Conv2d<B>,
    maxpool2d32: MaxPool2d,
    conv2d104: Conv2d<B>,
    maxpool2d33: MaxPool2d,
    conv2d105: Conv2d<B>,
    conv2d106: Conv2d<B>,
    conv2d107: Conv2d<B>,
    conv2d108: Conv2d<B>,
    conv2d109: Conv2d<B>,
    conv2d110: Conv2d<B>,
    conv2d111: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule8<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let conv2d99 = Conv2dConfig::new([128, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d100 = Conv2dConfig::new([64, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d29 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d101 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d30 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d102 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d31 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d103 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d32 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d104 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let maxpool2d33 = MaxPool2dConfig::new([2, 2])
            .with_strides([2, 2])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_ceil_mode(true)
            .init();
        let conv2d105 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d106 = Conv2dConfig::new([16, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(2, 2, 2, 2))
            .with_dilation([2, 2])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d107 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d108 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d109 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d110 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d111 = Conv2dConfig::new([32, 16], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            conv2d99,
            conv2d100,
            maxpool2d29,
            conv2d101,
            maxpool2d30,
            conv2d102,
            maxpool2d31,
            conv2d103,
            maxpool2d32,
            conv2d104,
            maxpool2d33,
            conv2d105,
            conv2d106,
            conv2d107,
            conv2d108,
            conv2d109,
            conv2d110,
            conv2d111,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(
        &self,
        add1_out1: Tensor<B, 4>,
        add10_out1: Tensor<B, 4>,
    ) -> (Tensor<B, 4>, Tensor<B, 4>) {
        let shape82_out1: [i64; 4] = {
            let axes = &add1_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather55_out1 = shape82_out1[2] as i64;
        let gather56_out1 = shape82_out1[3] as i64;
        let unsqueeze55_out1 = [gather55_out1 as i64];
        let unsqueeze56_out1 = [gather56_out1 as i64];
        let concat98_out1: [i64; 2usize] = [&unsqueeze55_out1[..], &unsqueeze56_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape84_out1: [i64; 4] = {
            let axes = &add10_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice28_out1: [i64; 2] = shape84_out1[0..2].try_into().unwrap();
        let concat99_out1: [i64; 4usize] = [&slice28_out1[..], &concat98_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize28_out1 = {
            let target_height = concat99_out1[2] as usize;
            let target_width = concat99_out1[3] as usize;
            burn::tensor::module::interpolate(
                add10_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat100_out1 = burn::tensor::Tensor::cat([resize28_out1, add1_out1].into(), 1);
        let conv2d99_out1 = self.conv2d99.forward(concat100_out1);
        let relu99_out1 = burn::tensor::activation::relu(conv2d99_out1);
        let conv2d100_out1 = self.conv2d100.forward(relu99_out1.clone());
        let relu100_out1 = burn::tensor::activation::relu(conv2d100_out1);
        let maxpool2d29_out1 = self.maxpool2d29.forward(relu100_out1.clone());
        let conv2d101_out1 = self.conv2d101.forward(maxpool2d29_out1);
        let relu101_out1 = burn::tensor::activation::relu(conv2d101_out1);
        let maxpool2d30_out1 = self.maxpool2d30.forward(relu101_out1.clone());
        let conv2d102_out1 = self.conv2d102.forward(maxpool2d30_out1);
        let relu102_out1 = burn::tensor::activation::relu(conv2d102_out1);
        let maxpool2d31_out1 = self.maxpool2d31.forward(relu102_out1.clone());
        let conv2d103_out1 = self.conv2d103.forward(maxpool2d31_out1);
        let relu103_out1 = burn::tensor::activation::relu(conv2d103_out1);
        let maxpool2d32_out1 = self.maxpool2d32.forward(relu103_out1.clone());
        let conv2d104_out1 = self.conv2d104.forward(maxpool2d32_out1);
        let relu104_out1 = burn::tensor::activation::relu(conv2d104_out1);
        let maxpool2d33_out1 = self.maxpool2d33.forward(relu104_out1.clone());
        let conv2d105_out1 = self.conv2d105.forward(maxpool2d33_out1);
        let relu105_out1 = burn::tensor::activation::relu(conv2d105_out1);
        let conv2d106_out1 = self.conv2d106.forward(relu105_out1.clone());
        let relu106_out1 = burn::tensor::activation::relu(conv2d106_out1);
        let concat101_out1 = burn::tensor::Tensor::cat([relu106_out1, relu105_out1].into(), 1);
        let conv2d107_out1 = self.conv2d107.forward(concat101_out1);
        let relu107_out1 = burn::tensor::activation::relu(conv2d107_out1);
        let shape85_out1: [i64; 4] = {
            let axes = &relu104_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather57_out1 = shape85_out1[2] as i64;
        let gather58_out1 = shape85_out1[3] as i64;
        let unsqueeze57_out1 = [gather57_out1 as i64];
        let unsqueeze58_out1 = [gather58_out1 as i64];
        let concat102_out1: [i64; 2usize] = [&unsqueeze57_out1[..], &unsqueeze58_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape87_out1: [i64; 4] = {
            let axes = &relu107_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice29_out1: [i64; 2] = shape87_out1[0..2].try_into().unwrap();
        let concat103_out1: [i64; 4usize] = [&slice29_out1[..], &concat102_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize29_out1 = {
            let target_height = concat103_out1[2] as usize;
            let target_width = concat103_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu107_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat104_out1 = burn::tensor::Tensor::cat([resize29_out1, relu104_out1].into(), 1);
        let conv2d108_out1 = self.conv2d108.forward(concat104_out1);
        let relu108_out1 = burn::tensor::activation::relu(conv2d108_out1);
        let shape88_out1: [i64; 4] = {
            let axes = &relu103_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather59_out1 = shape88_out1[2] as i64;
        let gather60_out1 = shape88_out1[3] as i64;
        let unsqueeze59_out1 = [gather59_out1 as i64];
        let unsqueeze60_out1 = [gather60_out1 as i64];
        let concat105_out1: [i64; 2usize] = [&unsqueeze59_out1[..], &unsqueeze60_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape90_out1: [i64; 4] = {
            let axes = &relu108_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice30_out1: [i64; 2] = shape90_out1[0..2].try_into().unwrap();
        let concat106_out1: [i64; 4usize] = [&slice30_out1[..], &concat105_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize30_out1 = {
            let target_height = concat106_out1[2] as usize;
            let target_width = concat106_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu108_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat107_out1 = burn::tensor::Tensor::cat([resize30_out1, relu103_out1].into(), 1);
        let conv2d109_out1 = self.conv2d109.forward(concat107_out1);
        let relu109_out1 = burn::tensor::activation::relu(conv2d109_out1);
        let shape91_out1: [i64; 4] = {
            let axes = &relu102_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather61_out1 = shape91_out1[2] as i64;
        let gather62_out1 = shape91_out1[3] as i64;
        let unsqueeze61_out1 = [gather61_out1 as i64];
        let unsqueeze62_out1 = [gather62_out1 as i64];
        let concat108_out1: [i64; 2usize] = [&unsqueeze61_out1[..], &unsqueeze62_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape93_out1: [i64; 4] = {
            let axes = &relu109_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice31_out1: [i64; 2] = shape93_out1[0..2].try_into().unwrap();
        let concat109_out1: [i64; 4usize] = [&slice31_out1[..], &concat108_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize31_out1 = {
            let target_height = concat109_out1[2] as usize;
            let target_width = concat109_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu109_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat110_out1 = burn::tensor::Tensor::cat([resize31_out1, relu102_out1].into(), 1);
        let conv2d110_out1 = self.conv2d110.forward(concat110_out1);
        let relu110_out1 = burn::tensor::activation::relu(conv2d110_out1);
        let shape94_out1: [i64; 4] = {
            let axes = &relu101_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather63_out1 = shape94_out1[2] as i64;
        let gather64_out1 = shape94_out1[3] as i64;
        let unsqueeze63_out1 = [gather63_out1 as i64];
        let unsqueeze64_out1 = [gather64_out1 as i64];
        let concat111_out1: [i64; 2usize] = [&unsqueeze63_out1[..], &unsqueeze64_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape96_out1: [i64; 4] = {
            let axes = &relu110_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice32_out1: [i64; 2] = shape96_out1[0..2].try_into().unwrap();
        let concat112_out1: [i64; 4usize] = [&slice32_out1[..], &concat111_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize32_out1 = {
            let target_height = concat112_out1[2] as usize;
            let target_width = concat112_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu110_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat113_out1 = burn::tensor::Tensor::cat([resize32_out1, relu101_out1].into(), 1);
        let conv2d111_out1 = self.conv2d111.forward(concat113_out1);
        let relu111_out1 = burn::tensor::activation::relu(conv2d111_out1);
        let shape97_out1: [i64; 4] = {
            let axes = &relu100_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather65_out1 = shape97_out1[2] as i64;
        let gather66_out1 = shape97_out1[3] as i64;
        let unsqueeze65_out1 = [gather65_out1 as i64];
        let unsqueeze66_out1 = [gather66_out1 as i64];
        let concat114_out1: [i64; 2usize] = [&unsqueeze65_out1[..], &unsqueeze66_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape99_out1: [i64; 4] = {
            let axes = &relu111_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice33_out1: [i64; 2] = shape99_out1[0..2].try_into().unwrap();
        let concat115_out1: [i64; 4usize] = [&slice33_out1[..], &concat114_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize33_out1 = {
            let target_height = concat115_out1[2] as usize;
            let target_width = concat115_out1[3] as usize;
            burn::tensor::module::interpolate(
                relu111_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat116_out1 = burn::tensor::Tensor::cat([resize33_out1, relu100_out1].into(), 1);
        (concat116_out1, relu99_out1)
    }
}
#[derive(Module, Debug)]
pub struct Submodule9<B: Backend> {
    conv2d112: Conv2d<B>,
    conv2d113: Conv2d<B>,
    conv2d114: Conv2d<B>,
    conv2d115: Conv2d<B>,
    conv2d116: Conv2d<B>,
    conv2d117: Conv2d<B>,
    conv2d118: Conv2d<B>,
    conv2d119: Conv2d<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}
impl<B: Backend> Submodule9<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let conv2d112 = Conv2dConfig::new([32, 64], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d113 = Conv2dConfig::new([64, 1], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d114 = Conv2dConfig::new([64, 1], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d115 = Conv2dConfig::new([64, 1], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d116 = Conv2dConfig::new([64, 1], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d117 = Conv2dConfig::new([64, 1], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d118 = Conv2dConfig::new([64, 1], [3, 3])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        let conv2d119 = Conv2dConfig::new([6, 1], [1, 1])
            .with_stride([1, 1])
            .with_padding(PaddingConfig2d::Valid)
            .with_dilation([1, 1])
            .with_groups(1)
            .with_bias(true)
            .init(device);
        Self {
            conv2d112,
            conv2d113,
            conv2d114,
            conv2d115,
            conv2d116,
            conv2d117,
            conv2d118,
            conv2d119,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }
    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(
        &self,
        concat116_out1: Tensor<B, 4>,
        relu99_out1: Tensor<B, 4>,
        add10_out1: Tensor<B, 4>,
        add9_out1: Tensor<B, 4>,
        add8_out1: Tensor<B, 4>,
        add7_out1: Tensor<B, 4>,
        add6_out1: Tensor<B, 4>,
    ) -> (
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
    ) {
        let conv2d112_out1 = self.conv2d112.forward(concat116_out1);
        let relu112_out1 = burn::tensor::activation::relu(conv2d112_out1);
        let add11_out1 = relu112_out1.add(relu99_out1);
        let conv2d113_out1 = self.conv2d113.forward(add11_out1);
        let conv2d114_out1 = self.conv2d114.forward(add10_out1);
        let shape100_out1: [i64; 4] = {
            let axes = &conv2d113_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let gather67_out1 = shape100_out1[2] as i64;
        let gather68_out1 = shape100_out1[3] as i64;
        let unsqueeze67_out1 = [gather67_out1 as i64];
        let unsqueeze68_out1 = [gather68_out1 as i64];
        let concat117_out1: [i64; 2usize] = [&unsqueeze67_out1[..], &unsqueeze68_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape102_out1: [i64; 4] = {
            let axes = &conv2d114_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice34_out1: [i64; 2] = shape102_out1[0..2].try_into().unwrap();
        let concat118_out1: [i64; 4usize] = [&slice34_out1[..], &concat117_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize34_out1 = {
            let target_height = concat118_out1[2] as usize;
            let target_width = concat118_out1[3] as usize;
            burn::tensor::module::interpolate(
                conv2d114_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let conv2d115_out1 = self.conv2d115.forward(add9_out1);
        let gather69_out1 = shape100_out1[2] as i64;
        let gather70_out1 = shape100_out1[3] as i64;
        let unsqueeze69_out1 = [gather69_out1 as i64];
        let unsqueeze70_out1 = [gather70_out1 as i64];
        let concat119_out1: [i64; 2usize] = [&unsqueeze69_out1[..], &unsqueeze70_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape105_out1: [i64; 4] = {
            let axes = &conv2d115_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice35_out1: [i64; 2] = shape105_out1[0..2].try_into().unwrap();
        let concat120_out1: [i64; 4usize] = [&slice35_out1[..], &concat119_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize35_out1 = {
            let target_height = concat120_out1[2] as usize;
            let target_width = concat120_out1[3] as usize;
            burn::tensor::module::interpolate(
                conv2d115_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let conv2d116_out1 = self.conv2d116.forward(add8_out1);
        let gather71_out1 = shape100_out1[2] as i64;
        let gather72_out1 = shape100_out1[3] as i64;
        let unsqueeze71_out1 = [gather71_out1 as i64];
        let unsqueeze72_out1 = [gather72_out1 as i64];
        let concat121_out1: [i64; 2usize] = [&unsqueeze71_out1[..], &unsqueeze72_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape108_out1: [i64; 4] = {
            let axes = &conv2d116_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice36_out1: [i64; 2] = shape108_out1[0..2].try_into().unwrap();
        let concat122_out1: [i64; 4usize] = [&slice36_out1[..], &concat121_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize36_out1 = {
            let target_height = concat122_out1[2] as usize;
            let target_width = concat122_out1[3] as usize;
            burn::tensor::module::interpolate(
                conv2d116_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let conv2d117_out1 = self.conv2d117.forward(add7_out1);
        let gather73_out1 = shape100_out1[2] as i64;
        let gather74_out1 = shape100_out1[3] as i64;
        let unsqueeze73_out1 = [gather73_out1 as i64];
        let unsqueeze74_out1 = [gather74_out1 as i64];
        let concat123_out1: [i64; 2usize] = [&unsqueeze73_out1[..], &unsqueeze74_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape111_out1: [i64; 4] = {
            let axes = &conv2d117_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice37_out1: [i64; 2] = shape111_out1[0..2].try_into().unwrap();
        let concat124_out1: [i64; 4usize] = [&slice37_out1[..], &concat123_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize37_out1 = {
            let target_height = concat124_out1[2] as usize;
            let target_width = concat124_out1[3] as usize;
            burn::tensor::module::interpolate(
                conv2d117_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let conv2d118_out1 = self.conv2d118.forward(add6_out1);
        let gather75_out1 = shape100_out1[2] as i64;
        let gather76_out1 = shape100_out1[3] as i64;
        let unsqueeze75_out1 = [gather75_out1 as i64];
        let unsqueeze76_out1 = [gather76_out1 as i64];
        let concat125_out1: [i64; 2usize] = [&unsqueeze75_out1[..], &unsqueeze76_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let shape114_out1: [i64; 4] = {
            let axes = &conv2d118_out1.clone().dims()[0..4];
            let mut output = [0i64; 4];
            for i in 0..4 {
                output[i] = axes[i] as i64;
            }
            output
        };
        let slice38_out1: [i64; 2] = shape114_out1[0..2].try_into().unwrap();
        let concat126_out1: [i64; 4usize] = [&slice38_out1[..], &concat125_out1[..]]
            .concat()
            .try_into()
            .unwrap();
        let resize38_out1 = {
            let target_height = concat126_out1[2] as usize;
            let target_width = concat126_out1[3] as usize;
            burn::tensor::module::interpolate(
                conv2d118_out1,
                [target_height, target_width],
                burn::tensor::ops::InterpolateOptions::new(
                    burn::tensor::ops::InterpolateMode::Bilinear,
                )
                .with_align_corners(false),
            )
        };
        let concat127_out1 = burn::tensor::Tensor::cat(
            [
                conv2d113_out1.clone(),
                resize34_out1.clone(),
                resize35_out1.clone(),
                resize36_out1.clone(),
                resize37_out1.clone(),
                resize38_out1.clone(),
            ]
            .into(),
            1,
        );
        let conv2d119_out1 = self.conv2d119.forward(concat127_out1);
        let sigmoid1_out1 = burn::tensor::activation::sigmoid(conv2d119_out1);
        let sigmoid2_out1 = burn::tensor::activation::sigmoid(conv2d113_out1);
        let sigmoid3_out1 = burn::tensor::activation::sigmoid(resize34_out1);
        let sigmoid4_out1 = burn::tensor::activation::sigmoid(resize35_out1);
        let sigmoid5_out1 = burn::tensor::activation::sigmoid(resize36_out1);
        let sigmoid6_out1 = burn::tensor::activation::sigmoid(resize37_out1);
        let sigmoid7_out1 = burn::tensor::activation::sigmoid(resize38_out1);
        (
            sigmoid1_out1,
            sigmoid2_out1,
            sigmoid3_out1,
            sigmoid4_out1,
            sigmoid5_out1,
            sigmoid6_out1,
            sigmoid7_out1,
        )
    }
}

#[derive(Module, Debug)]
pub struct Model<B: Backend> {
    submodule1: Submodule1<B>,
    submodule2: Submodule2<B>,
    submodule3: Submodule3<B>,
    submodule4: Submodule4<B>,
    submodule5: Submodule5<B>,
    submodule6: Submodule6<B>,
    submodule7: Submodule7<B>,
    submodule8: Submodule8<B>,
    submodule9: Submodule9<B>,
    phantom: core::marker::PhantomData<B>,
    #[module(skip)]
    device: B::Device,
}

impl<B: Backend> Model<B> {
    /// Load model weights from in-memory bytes.
    ///
    /// The bytes must be the contents of a `.bpk` file.
    pub fn from_bytes(bytes: Bytes, device: &B::Device) -> Self {
        let mut model = Self::new(device);
        let mut store = BurnpackStore::from_bytes(Some(bytes));
        model
            .load_from(&mut store)
            .expect("Failed to load burnpack bytes");
        model
    }
}

impl<B: Backend> Model<B> {
    #[allow(unused_variables)]
    pub fn new(device: &B::Device) -> Self {
        let submodule1 = Submodule1::new(device);
        let submodule2 = Submodule2::new(device);
        let submodule3 = Submodule3::new(device);
        let submodule4 = Submodule4::new(device);
        let submodule5 = Submodule5::new(device);
        let submodule6 = Submodule6::new(device);
        let submodule7 = Submodule7::new(device);
        let submodule8 = Submodule8::new(device);
        let submodule9 = Submodule9::new(device);
        Self {
            submodule1,
            submodule2,
            submodule3,
            submodule4,
            submodule5,
            submodule6,
            submodule7,
            submodule8,
            submodule9,
            phantom: core::marker::PhantomData,
            device: device.clone(),
        }
    }

    #[allow(clippy::let_and_return, clippy::approx_constant)]
    pub fn forward(
        &self,
        input_1: Tensor<B, 4>,
    ) -> (
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
        Tensor<B, 4>,
    ) {
        let add1_out1 = self.submodule1.forward(input_1);
        let add2_out1 = self.submodule2.forward(add1_out1.clone());
        let (add4_out1, add3_out1) = self.submodule3.forward(add2_out1.clone());
        let (add7_out1, add6_out1) = self.submodule4.forward(add4_out1.clone());
        let (concat71_out1, add8_out1) =
            self.submodule5
                .forward(add4_out1, add7_out1.clone(), add3_out1);
        let (concat84_out1, add9_out1) = self.submodule6.forward(concat71_out1, add2_out1);
        let add10_out1 = self.submodule7.forward(concat84_out1);
        let (concat116_out1, relu99_out1) = self.submodule8.forward(add1_out1, add10_out1.clone());
        let (
            sigmoid1_out1,
            sigmoid2_out1,
            sigmoid3_out1,
            sigmoid4_out1,
            sigmoid5_out1,
            sigmoid6_out1,
            sigmoid7_out1,
        ) = self.submodule9.forward(
            concat116_out1,
            relu99_out1,
            add10_out1,
            add9_out1,
            add8_out1,
            add7_out1,
            add6_out1,
        );
        (
            sigmoid1_out1,
            sigmoid2_out1,
            sigmoid3_out1,
            sigmoid4_out1,
            sigmoid5_out1,
            sigmoid6_out1,
            sigmoid7_out1,
        )
    }
}
