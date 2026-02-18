#!/usr/bin/env bash
set -euo pipefail

build_variant() {
  local name="$1"
  local rustflags="$2"
  local dir="target-${name}"

  echo "== building ${name} =="
  CARGO_TARGET_DIR="$dir" RUSTFLAGS="$rustflags" \
    cargo rustc --release --target wasm32-wasip1 --no-default-features --features frame,block,wasm-exports --crate-type=cdylib

  local wasm="$dir/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm"
  if [[ -f "$wasm" ]]; then
    echo "${name} bytes: $(wc -c < "$wasm")"
  else
    echo "${name} build missing artifact"
    return 1
  fi
}

invoke() {
  local module="$1"
  local fn="$2"
  shift 2

  if wasmtime run --invoke "$fn" "$module" "$@" >/dev/null 2>&1; then
    wasmtime run --invoke "$fn" "$module" "$@"
    return 0
  fi

  wasmtime --invoke "$fn" "$module" "$@"
}

measure_ms() {
  local module="$1"
  local fn="$2"
  shift 2

  local start end
  start=$(date +%s%N)
  local out
  out=$(invoke "$module" "$fn" "$@")
  end=$(date +%s%N)
  local elapsed_ms=$(( (end - start) / 1000000 ))
  printf "%s %s\n" "$elapsed_ms" "$out"
}

build_variant scalar ""
build_variant simd "-C target-feature=+simd128"

SCALAR_WASM="target-scalar/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm"
SIMD_WASM="target-simd/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm"

{
  echo "# WASM benchmark report"
  echo
  echo "target: wasm32-wasip1"
  echo "features: frame,block,wasm-exports"
  echo
  echo "## Size"
  echo "- scalar: $(wc -c < "$SCALAR_WASM") bytes"
  echo "- simd128: $(wc -c < "$SIMD_WASM") bytes"
  echo
} > wasm-benchmark-report.txt

if command -v wasmtime >/dev/null 2>&1; then
  echo "== wasm runtime validation (wasmtime) =="
  for mode in scalar simd; do
    module="target-${mode}/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm"
    block_ok=$(invoke "$module" wasm_block_roundtrip || true)
    frame_ok=$(invoke "$module" wasm_frame_roundtrip || true)
    hash_ok=$(invoke "$module" wasm_hash_consistency || true)

    echo "${mode} block_roundtrip=${block_ok} frame_roundtrip=${frame_ok} hash_consistency=${hash_ok}"
    if [[ "$block_ok" != "1" || "$frame_ok" != "1" || "$hash_ok" != "1" ]]; then
      echo "Runtime validation failed for ${mode}" >&2
      exit 1
    fi

    read -r c_ms c_out <<<"$(measure_ms "$module" wasm_compress_repeated 120 262144)"
    read -r d_ms d_out <<<"$(measure_ms "$module" wasm_decompress_repeated 120 262144)"
    echo "${mode} compress: ${c_ms} ms (result=${c_out})"
    echo "${mode} decompress: ${d_ms} ms (result=${d_out})"

    {
      echo "## Runtime (${mode})"
      echo "- block roundtrip: ${block_ok}"
      echo "- frame roundtrip: ${frame_ok}"
      echo "- hash consistency: ${hash_ok}"
      echo "- compress repeated (120 x 256KiB): ${c_ms} ms"
      echo "- decompress repeated (120 x 256KiB): ${d_ms} ms"
      echo
    } >> wasm-benchmark-report.txt
  done
else
  echo "wasmtime not found; skipping runtime invocation checks."
  echo >> wasm-benchmark-report.txt
  echo "wasmtime not found; runtime checks skipped." >> wasm-benchmark-report.txt
fi

echo
cat wasm-benchmark-report.txt
