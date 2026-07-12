#!/usr/bin/env bash
set -euo pipefail

runtime="cpu"
models="both"
default_data_home="${XDG_DATA_HOME:-${HOME}/.local/share}"
destination="${default_data_home}/burst-frame-deduplicator/ml-model-pack"

usage() {
  cat <<'EOF'
Install the optional offline Linux subject-detector model pack.

Usage:
  scripts/install_linux_ml_models.sh [options]

Options:
  --dest DIR                 Install directory.
  --models light|heavy|both  Models to install (default: both).
  --runtime cpu|cuda|both    ONNX Runtime pack (default: cpu).
  -h, --help                 Show this help.

The CUDA runtime requires CUDA 12 and cuDNN 9 on the host. Installation does
not initialize a GPU. CPU packs support x86_64 and aarch64; CUDA packs are
x86_64-only. Inference never downloads files automatically.
EOF
}

while (($#)); do
  case "$1" in
    --dest)
      destination=${2:?missing value for --dest}
      shift 2
      ;;
    --models)
      models=${2:?missing value for --models}
      shift 2
      ;;
    --runtime)
      runtime=${2:?missing value for --runtime}
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$models" in
  light|heavy|both) ;;
  *) echo "--models must be light, heavy, or both" >&2; exit 2 ;;
esac
case "$runtime" in
  cpu|cuda|both) ;;
  *) echo "--runtime must be cpu, cuda, or both" >&2; exit 2 ;;
esac
if [[ -z "$destination" || "$destination" == / ]]; then
  echo "refusing unsafe install destination: '$destination'" >&2
  exit 2
fi

if [[ $(uname -s) != Linux ]]; then
  echo "this installer supports Linux only" >&2
  exit 1
fi

case $(uname -m) in
  x86_64|amd64)
    architecture="x86_64"
    cpu_runtime_url="https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-linux-x64-1.24.2.tgz"
    cpu_runtime_sha256="43725474ba5663642e17684717946693850e2005efbd724ac72da278fead25e6"
    cuda_runtime_url="https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-linux-x64-gpu-1.24.2.tgz"
    cuda_runtime_sha256="bcb42da041f42192e5579de175f7410313c114740a611e230afe9d79be65cc49"
    cuda_manifest_sha256="\"$cuda_runtime_sha256\""
    ;;
  aarch64|arm64)
    architecture="aarch64"
    cpu_runtime_url="https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-linux-aarch64-1.24.2.tgz"
    cpu_runtime_sha256="6715b3d19965a2a6981e78ed4ba24f17a8c30d2d26420dbed10aac7ceca0085e"
    cuda_runtime_url=""
    cuda_runtime_sha256=""
    cuda_manifest_sha256="null"
    if [[ "$runtime" != cpu ]]; then
      echo "ONNX Runtime 1.24.2 does not publish a Linux aarch64 CUDA pack; use --runtime cpu" >&2
      exit 1
    fi
    ;;
  *)
    echo "unsupported Linux architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

for command in curl sha256sum tar; do
  command -v "$command" >/dev/null || {
    echo "required command is unavailable: $command" >&2
    exit 1
  }
done

mkdir -p "$destination/models" "$destination/LICENSES"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

download() {
  local url=$1
  local expected=$2
  local output=$3
  curl --fail --location --retry 4 --retry-delay 2 --output "$output" "$url"
  printf '%s  %s\n' "$expected" "$output" | sha256sum --check --status || {
    echo "checksum verification failed for $(basename "$output")" >&2
    exit 1
  }
}

install_model() {
  local filename=$1
  local url=$2
  local sha256=$3
  download "$url" "$sha256" "$temporary/$filename"
  install -m 0644 "$temporary/$filename" "$destination/models/$filename"
}

install_runtime() {
  local name=$1
  local url=$2
  local sha256=$3
  local archive="$temporary/${name}.tgz"
  local unpack="$temporary/${name}-unpack"
  download "$url" "$sha256" "$archive"
  mkdir -p "$unpack"
  tar -xzf "$archive" -C "$unpack"
  local extracted
  extracted=$(find "$unpack" -mindepth 1 -maxdepth 1 -type d -print -quit)
  test -n "$extracted"
  rm -rf -- "${destination:?}/${name}.new"
  cp -a "$extracted" "$destination/${name}.new"
  rm -rf -- "${destination:?}/$name"
  mv "$destination/${name}.new" "$destination/$name"
  test -f "$destination/$name/lib/libonnxruntime.so"
}

if [[ "$models" == light || "$models" == both ]]; then
  install_model \
    u2netp.onnx \
    https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx \
    309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8
fi

if [[ "$models" == heavy || "$models" == both ]]; then
  install_model \
    isnet-general-use.onnx \
    https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx \
    60920e99c45464f2ba57bee2ad08c919a52bbf852739e96947fbb4358c0d964a
fi

if [[ "$runtime" == cpu || "$runtime" == both ]]; then
  install_runtime \
    runtime-cpu \
    "$cpu_runtime_url" \
    "$cpu_runtime_sha256"
fi

if [[ "$runtime" == cuda || "$runtime" == both ]]; then
  install_runtime \
    runtime-cuda \
    "$cuda_runtime_url" \
    "$cuda_runtime_sha256"
fi

download \
  https://raw.githubusercontent.com/xuebinqin/U-2-Net/ac7e1c817ecab7c7dff5ce6b1abba61cd213ff29/LICENSE \
  c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4 \
  "$destination/LICENSES/U2NET-PROJECT-APACHE-2.0.txt"
download \
  https://raw.githubusercontent.com/xuebinqin/DIS/b6764e20381f6f42a70f83fa3324181529ed1403/LICENSE.md \
  c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4 \
  "$destination/LICENSES/ISNET-PROJECT-APACHE-2.0.txt"
download \
  https://raw.githubusercontent.com/danielgatis/rembg/91edaeca13be366ea6d5a3320a255a0c4a66ac98/LICENSE.txt \
  90a3215072968fd304669c5389f04f1274a587abdd0507d99dead0f5511f8999 \
  "$destination/LICENSES/REMBG-MIT.txt"
download \
  https://raw.githubusercontent.com/microsoft/onnxruntime/v1.24.2/LICENSE \
  2f07c72751aed99790b8a4869cf2311df85a860b22ded05fa22803587a48922c \
  "$destination/LICENSES/ONNXRUNTIME-MIT.txt"

case "$models" in
  light) rm -f -- "${destination:?}/models/isnet-general-use.onnx" ;;
  heavy) rm -f -- "${destination:?}/models/u2netp.onnx" ;;
esac
case "$runtime" in
  cpu) rm -rf -- "${destination:?}/runtime-cuda" ;;
  cuda) rm -rf -- "${destination:?}/runtime-cpu" ;;
esac

cat >"$destination/manifest.json" <<EOF
{
  "format": 1,
  "installation": {
    "models": "$models",
    "runtime": "$runtime",
    "architecture": "$architecture"
  },
  "onnxruntime": {
    "version": "1.24.2",
    "cpu_archive_sha256": "$cpu_runtime_sha256",
    "cuda12_archive_sha256": $cuda_manifest_sha256
  },
  "models": {
    "light": {
      "id": "u2netp-sod-v1",
      "file": "models/u2netp.onnx",
      "bytes": 4574861,
      "sha256": "309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8",
      "project": "https://github.com/xuebinqin/U-2-Net",
      "license_note": "Project code is Apache-2.0; the author-linked checkpoint has no separate weight license statement.",
      "conversion": "https://github.com/danielgatis/rembg/releases/tag/v0.0.0"
    },
    "heavy": {
      "id": "isnet-general-use-v1",
      "file": "models/isnet-general-use.onnx",
      "bytes": 178648008,
      "sha256": "60920e99c45464f2ba57bee2ad08c919a52bbf852739e96947fbb4358c0d964a",
      "project": "https://github.com/xuebinqin/DIS",
      "license_note": "Project code is Apache-2.0; the author-linked general-use checkpoint has no separate weight license statement.",
      "conversion": "https://github.com/danielgatis/rembg/releases/tag/v0.0.0"
    }
  }
}
EOF

echo "Installed local ML model pack at: $destination"
case "$runtime" in
  cuda)
    echo "Use: --detector ml-light|ml-heavy --detector-device cuda --detector-model-pack '$destination'"
    ;;
  both)
    echo "Automatic device selection stays on CPU; pass --detector-device cuda explicitly to initialize a GPU."
    echo "Use: --detector ml-light|ml-heavy --detector-model-pack '$destination'"
    ;;
  cpu)
    echo "Use: --detector ml-light|ml-heavy --detector-device cpu --detector-model-pack '$destination'"
    ;;
esac
