#!/usr/bin/env bash
set -euo pipefail

# Build script for the cheetah-ffmpeg-wasm codec pack.
#
# Usage:
#   scripts/build.sh [--variant baseline|simd|threads-simd] [--output-dir DIR] [--mock]
#
# In mock mode (the default for this build-system PR) the script compiles a
# small C shim that implements the pack ABI with every decode operation
# returning UNSUPPORTED.  In non-mock mode it would download/build FFmpeg
# 8.1.2 and link the real decoder shim.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SRC_DIR="${PACK_DIR}/src"

VARIANT="baseline"
OUTPUT_DIR="${PACK_DIR}/dist"
MOCK=1
MOCK_EXPLICIT=0
FFMPEG_VERSION="8.1.2"
FFMPEG_ENV="${FFMPEG:-0}"
FFMPEG_TARBALL="ffmpeg-${FFMPEG_VERSION}.tar.xz"
FFMPEG_URL="https://ffmpeg.org/releases/${FFMPEG_TARBALL}"
FFMPEG_SHA256="464beb5e7bf0c311e68b45ae2f04e9cc2af88851abb4082231742a74d97b524c"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --variant)
      VARIANT="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --mock)
      MOCK=1
      MOCK_EXPLICIT=1
      shift
      ;;
    --no-mock)
      MOCK=0
      MOCK_EXPLICIT=1
      shift
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

# If neither --mock nor --no-mock was supplied, honour the FFMPEG env variable.
if [[ "${MOCK_EXPLICIT}" -eq 0 && "${FFMPEG_ENV}" == "1" ]]; then
  MOCK=0
fi

mkdir -p "${OUTPUT_DIR}"
OUTPUT_DIR="$(cd "${OUTPUT_DIR}" && pwd)"

case "${VARIANT}" in
  baseline|simd|threads-simd) ;;
  *) echo "Unknown variant: ${VARIANT}" >&2; exit 1 ;;
esac

if [[ -n "${EMSDK:-}" ]]; then
  # shellcheck source=/dev/null
  source "${EMSDK}/emsdk_env.sh"
fi

if ! command -v emcc >/dev/null 2>&1; then
  echo "Emscripten not found. Please set EMSDK or add emcc to PATH." >&2
  exit 1
fi

EMCC_VERSION=$(emcc --version | head -n1 | sed 's/.* //')
echo "Building cheetah-ffmpeg-wasm variant=${VARIANT} mock=${MOCK} with Emscripten ${EMCC_VERSION}"

build_ffmpeg() {
  if [[ "${MOCK}" -eq 1 ]]; then
    return 0
  fi

  local cache_dir="${PACK_DIR}/.cache"
  local source_dir="${cache_dir}/ffmpeg-${FFMPEG_VERSION}"
  mkdir -p "${cache_dir}"

  if [[ ! -d "${source_dir}" ]]; then
    echo "Downloading FFmpeg ${FFMPEG_VERSION}..."
    local tarball_path="${cache_dir}/${FFMPEG_TARBALL}"
    if [[ ! -f "${tarball_path}" ]]; then
      curl -L --fail --max-time 180 -o "${tarball_path}" "${FFMPEG_URL}"
    fi
    echo "Verifying source hash..."
    echo "${FFMPEG_SHA256}  ${tarball_path}" | sha256sum -c - || {
      echo "FFmpeg source hash mismatch" >&2
      rm -f "${tarball_path}"
      exit 1
    }
    tar -xf "${tarball_path}" -C "${cache_dir}"
  fi

  local prefix="${cache_dir}/prefix-${FFMPEG_VERSION}-${VARIANT}"
  mkdir -p "${prefix}"

  if [[ ! -f "${prefix}/lib/libavutil.a" ]]; then
    cd "${source_dir}"
    ./configure --prefix="${prefix}" --enable-cross-compile --target-os=none \
      --arch=wasm32 --cpu=generic --cc=emcc --cxx=em++ --ar=emar --ranlib=emranlib \
      --disable-asm --disable-inline-asm --disable-stripping \
      --disable-programs --disable-doc --disable-avdevice --disable-postproc \
      --disable-avfilter --disable-network --disable-iconv --disable-encoders \
      --disable-muxers --disable-demuxers \
      --enable-decoder=h264 --enable-decoder=hevc --enable-decoder=aac --enable-decoder=mp3 \
      --enable-parser=h264 --enable-parser=hevc --enable-parser=aac --enable-parser=mpegaudio \
      --enable-swresample --enable-avutil
    make -j"$(nproc 2>/dev/null || echo 2)"
    make install
    cd - >/dev/null
  fi
}

# Real decoder shim sources would be added here when --no-mock is used.
# For the split PR we compile the mock implementation directly.
build_pack() {
  local emcc_flags=(
    -O3
    -sMODULARIZE=1
    -sEXPORT_NAME=createFfmpegPack
    -sEXPORTED_FUNCTIONS='["_malloc","_free","_cheetah_pack_abi_version","_cheetah_pack_init","_cheetah_pack_configure_track","_cheetah_pack_send_packet","_cheetah_pack_receive_frame","_cheetah_pack_flush","_cheetah_pack_close"]'
    -sEXPORTED_RUNTIME_METHODS='["ccall","cwrap","getValue","setValue","UTF8ToString","stringToUTF8"]'
    -sALLOW_MEMORY_GROWTH=1
    -sMALLOC=emmalloc
    -sENVIRONMENT=web,node,shell
    -sWASM=1
    -sNO_FILESYSTEM=1
    -I"${SRC_DIR}"
  )

  case "${VARIANT}" in
    simd)
      emcc_flags+=(-msimd128)
      ;;
    threads-simd)
      emcc_flags+=(-msimd128 -sUSE_PTHREADS=1 -sPTHREAD_POOL_SIZE=4 -sSHARED_MEMORY=1)
      ;;
  esac

  local input_files=()
  if [[ "${MOCK}" -eq 1 ]]; then
    input_files+=("${SRC_DIR}/cheetah_ffmpeg_mock.c")
  else
    build_ffmpeg
    # Real shim sources go here once implemented.
    input_files+=("${SRC_DIR}/cheetah_ffmpeg.c")
  fi

  local output_js="${OUTPUT_DIR}/cheetah_ffmpeg.${VARIANT}.js"
  local output_wasm="${OUTPUT_DIR}/cheetah_ffmpeg.${VARIANT}.wasm"
  local output_base="${OUTPUT_DIR}/cheetah_ffmpeg.${VARIANT}"

  emcc "${input_files[@]}" "${emcc_flags[@]}" -o "${output_js}"

  # Rename the generated wasm if emscripten used a different name.
  if [[ -f "${output_js%.*}.wasm" && ! -f "${output_wasm}" ]]; then
    mv "${output_js%.*}.wasm" "${output_wasm}"
  fi

  # Write a small offer file with hashes.
  local sha256_js sha256_wasm
  sha256_js=$(sha256sum "${output_js}" | awk '{print $1}')
  sha256_wasm=$(sha256sum "${output_wasm}" | awk '{print $1}')
  cat > "${OUTPUT_DIR}/offer-${VARIANT}.json" <<EOF
{
  "variant": "${VARIANT}",
  "mock": ${MOCK},
  "js": "$(basename "${output_js}")",
  "wasm": "$(basename "${output_wasm}")",
  "js_hash": "sha256:${sha256_js}",
  "wasm_hash": "sha256:${sha256_wasm}",
  "emscripten_version": "${EMCC_VERSION}",
  "license": "LGPL-2.1-or-later"
}
EOF
  echo "Built ${output_js} and ${output_wasm}"
}

build_pack
