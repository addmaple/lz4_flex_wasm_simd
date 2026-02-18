#!/usr/bin/env bash
set -euo pipefail

# Build scalar and SIMD variants for wasm32-wasip1 and print artifact sizes.
build_variant() {
  local name="$1"
  local rustflags="$2"
  local dir="target-${name}"

  echo "== building ${name} =="
  CARGO_TARGET_DIR="$dir" RUSTFLAGS="$rustflags" \
    cargo rustc --release --target wasm32-wasip1 --no-default-features --features frame,block --crate-type=cdylib

  local wasm="$dir/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm"
  if [[ -f "$wasm" ]]; then
    echo "${name} bytes: $(wc -c < "$wasm")"
  else
    echo "${name} build missing artifact"
  fi
}

build_variant scalar ""
build_variant simd "-C target-feature=+simd128"
