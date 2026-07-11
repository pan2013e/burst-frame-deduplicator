extern "C" __global__ void bfd_focus_partials(
    const unsigned char* __restrict__ gray,
    unsigned long long pixel_count,
    unsigned long long width,
    unsigned long long height,
    double* __restrict__ partials
) {
    __shared__ double lap_sums[256];
    __shared__ double lap_sq_sums[256];
    __shared__ double dx_sums[256];
    __shared__ double dy_sums[256];

    const unsigned int lane = threadIdx.x;
    const unsigned long long i =
        (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;
    double lap = 0.0;
    double lap_sq = 0.0;
    double dx_sq = 0.0;
    double dy_sq = 0.0;

    if (i < pixel_count) {
        const unsigned long long x = i % width;
        const unsigned long long y = i / width;
        if (x > 0 && x + 1 < width && y > 0 && y + 1 < height) {
            lap = -4.0 * (double)gray[i]
                + (double)gray[i - 1]
                + (double)gray[i + 1]
                + (double)gray[i - width]
                + (double)gray[i + width];
            lap_sq = lap * lap;
        }
        if (x + 1 < width) {
            const double dx = (double)gray[i + 1] - (double)gray[i];
            dx_sq = dx * dx;
        }
        if (y + 1 < height) {
            const double dy = (double)gray[i + width] - (double)gray[i];
            dy_sq = dy * dy;
        }
    }

    lap_sums[lane] = lap;
    lap_sq_sums[lane] = lap_sq;
    dx_sums[lane] = dx_sq;
    dy_sums[lane] = dy_sq;
    __syncthreads();

    for (unsigned int stride = blockDim.x / 2; stride > 0; stride >>= 1) {
        if (lane < stride) {
            lap_sums[lane] += lap_sums[lane + stride];
            lap_sq_sums[lane] += lap_sq_sums[lane + stride];
            dx_sums[lane] += dx_sums[lane + stride];
            dy_sums[lane] += dy_sums[lane + stride];
        }
        __syncthreads();
    }

    if (lane == 0) {
        const unsigned long long output = (unsigned long long)blockIdx.x * 4;
        partials[output] = lap_sums[0];
        partials[output + 1] = lap_sq_sums[0];
        partials[output + 2] = dx_sums[0];
        partials[output + 3] = dy_sums[0];
    }
}
