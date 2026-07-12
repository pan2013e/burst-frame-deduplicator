struct Parameters {
    width: u32,
    height: u32,
    pixel_count: u32,
    partial_count: u32,
}

@group(0) @binding(0) var<storage, read> rgba: array<u32>;
@group(0) @binding(1) var<storage, read_write> partials: array<vec4<i32>>;
@group(0) @binding(2) var<uniform> parameters: Parameters;

var<workgroup> sums: array<vec4<i32>, 256>;

fn luma(index: u32) -> i32 {
    let packed = rgba[index];
    let red = packed & 0xffu;
    let green = (packed >> 8u) & 0xffu;
    let blue = (packed >> 16u) & 0xffu;
    return i32((54u * red + 183u * green + 19u * blue + 128u) >> 8u);
}

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let index = global_id.x;
    var values = vec4<i32>(0);
    if (index < parameters.pixel_count) {
        let x = index % parameters.width;
        let y = index / parameters.width;
        let center = luma(index);

        if (x > 0u && x + 1u < parameters.width && y > 0u && y + 1u < parameters.height) {
            let laplacian = -4 * center
                + luma(index - 1u)
                + luma(index + 1u)
                + luma(index - parameters.width)
                + luma(index + parameters.width);
            values.x = laplacian;
            values.y = laplacian * laplacian;
        }
        if (x + 1u < parameters.width) {
            let dx = luma(index + 1u) - center;
            values.z = dx * dx;
        }
        if (y + 1u < parameters.height) {
            let dy = luma(index + parameters.width) - center;
            values.w = dy * dy;
        }
    }

    sums[lane] = values;
    workgroupBarrier();
    var stride = 128u;
    loop {
        if (stride == 0u) {
            break;
        }
        if (lane < stride) {
            sums[lane] += sums[lane + stride];
        }
        workgroupBarrier();
        stride >>= 1u;
    }
    if (lane == 0u && group_id.x < parameters.partial_count) {
        partials[group_id.x] = sums[0];
    }
}
