# Local ML subject detection on Linux

The Linux CLI supports two optional, fully local subject-saliency models. They improve the advisory subject box, subject-focused sharpness, and out-of-frame risk. They do not replace the portable near-duplicate descriptor, and they never make source-photo changes.

| CLI choice | Model | ONNX size | Input | Intended use |
| --- | --- | ---: | ---: | --- |
| `--detector ml-light` | U²-Net-P | 4.57 MB | 320×320 | Fast scans and modest machines |
| `--detector ml-heavy` | IS-Net General Use | 178.65 MB | 1024×1024 | Higher-resolution subject analysis |

`--detector auto` deliberately remains the built-in heuristic. A scan only loads a model when `ml-light` or `ml-heavy` is explicit.

Device `auto` is also conservative: it selects the CPU provider and never initializes a GPU. GPU inference requires an explicit `--detector-device cuda`.

## Install the offline model pack

The CLI never downloads a model or runtime during a scan. Install the pinned CPU pack once:

```bash
pack="$HOME/.local/share/burst-frame-deduplicator/ml-model-pack"
scripts/install_linux_ml_models.sh --dest "$pack" --models both --runtime cpu
```

Then run either model:

```bash
cargo run -- scan samples \
  --out /tmp/bfd-ml-light \
  --detector ml-light \
  --detector-device cpu \
  --detector-model-pack "$pack"

cargo run -- scan samples \
  --out /tmp/bfd-ml-heavy \
  --detector ml-heavy \
  --detector-device cpu \
  --detector-model-pack "$pack"
```

`BFD_ML_MODEL_PACK` may be used instead of `--detector-model-pack`. The pack path is intentionally omitted from `manifest.json`; the manifest records only the model ID, exact SHA-256, size, runtime version, selected provider, capabilities, and fallback notes.

## CPU, AVX2, and CUDA are separate choices

The photo-scoring and ML controls are independent:

- `--acceleration cpu` means the explicitly portable scalar scoring path.
- `--acceleration avx2` means the explicitly requested, runtime-checked AVX2 scoring path, with scalar fallback on unsupported CPUs.
- `--detector-device cpu` means the ONNX Runtime CPU execution provider. ONNX Runtime may perform its own host-specific kernel dispatch; this is not the app's explicit AVX2 scoring backend and is reported separately.
- `--detector-device auto` is CPU-safe even when the pack contains a CUDA runtime; it never initializes a GPU.
- `--detector-device cuda` requests ONNX Runtime CUDA first. Initialization and inference failures retry on its CPU provider; if CPU also fails, the scan uses heuristic saliency.

To prepare the CUDA path without executing it:

```bash
scripts/install_linux_ml_models.sh \
  --dest "$pack" \
  --models both \
  --runtime both
```

The installer supports CPU packs on Linux x86_64 and aarch64; the published CLI archive is currently x86_64, so aarch64 users build the CLI from source. The pinned CUDA pack is x86_64-only and requires CUDA 12 plus cuDNN 9. The CUDA inference path is implemented but was not executed on the development server because all GPUs were occupied. Session initialization uses cuDNN's heuristic convolution selection and caps its search workspace; actual provider selection in `manifest.json` remains the source of truth.

## Provenance and licensing

- U²-Net-P comes from the [official U²-Net project](https://github.com/xuebinqin/U-2-Net), which is Apache-2.0. The author-linked checkpoint does not carry a separate weight license; the pack preserves project attribution and records this provenance.
- IS-Net General Use comes from the [official DIS/IS-Net project](https://github.com/xuebinqin/DIS), whose code is Apache-2.0. The author-linked general-use checkpoint has no separate weight-license statement. The authors describe it as optimized for general use, while also warning that the earlier academic DIS5K model has category-coverage limitations.
- The reproducible ONNX conversions are distributed by [rembg](https://github.com/danielgatis/rembg), which is MIT-licensed.
- ONNX Runtime is distributed by Microsoft under the MIT license.

The installer verifies every archive and model with a pinned SHA-256 and places the relevant license texts under `LICENSES/` in the external pack.

## Limitations and fallback behavior

Both models produce one foreground-saliency mask, not persistent instance identities. Connected components approximate a subject count; touching subjects can merge, and an intentionally non-salient object can be omitted. Inputs use the model projects' guarded per-image maximum normalization. Raw sigmoid outputs are used without output min/max normalization so an empty image is not forced to contain a high-confidence subject.

A missing pack, invalid checksum, incompatible tensor contract, unavailable runtime, or failed provider produces a recorded fallback note and keeps the heuristic detector active. Near-duplicate grouping continues to use the portable descriptor captured before detector metrics are merged.
