#!/usr/bin/env bash
set -euo pipefail

SAMPLES="${SAMPLES:-11}"
WARMUP="${WARMUP:-3}"
INNER_ITERS="${INNER_ITERS:-800}"
PAYLOAD_BYTES="${PAYLOAD_BYTES:-262144}"

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
    wasmtime run --invoke "$fn" "$module" "$@" 2>/dev/null
    return 0
  fi

  wasmtime --invoke "$fn" "$module" "$@" 2>/dev/null
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

calc_stats() {
  # stdin: sorted integer ms values (ascending), one per line
  awk '
    {
      a[++n] = $1;
      sum += $1;
    }
    END {
      if (n == 0) {
        print "median=0 p95=0 min=0 max=0 mean=0";
        exit;
      }
      median_idx = int((n + 1) / 2);
      p95_idx = int((95 * n + 99) / 100);
      if (p95_idx < 1) p95_idx = 1;
      if (p95_idx > n) p95_idx = n;
      min = a[1];
      max = a[n];
      mean = sum / n;
      printf "median=%d p95=%d min=%d max=%d mean=%.2f\n", a[median_idx], a[p95_idx], min, max, mean;
    }
  '
}

run_series() {
  local module="$1"
  local fn="$2"
  local label="$3"
  shift 3

  local i
  for ((i=1; i<=WARMUP; i++)); do
    invoke "$module" "$fn" "$@" >/dev/null
  done

  local -a times=()
  local expected=""
  for ((i=1; i<=SAMPLES; i++)); do
    local ms out
    read -r ms out <<<"$(measure_ms "$module" "$fn" "$@")"
    times+=("$ms")
    if [[ -z "$expected" ]]; then
      expected="$out"
    elif [[ "$expected" != "$out" ]]; then
      echo "non-deterministic result for ${label}: expected=${expected} got=${out}" >&2
      exit 1
    fi
  done

  local stats
  stats=$(printf '%s\n' "${times[@]}" | sort -n | calc_stats)
  echo "${label} ${stats} checksum=${expected}"
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
  C_MEDIAN_SCALAR=""
  C_MEDIAN_SIMD=""
  D_MEDIAN_SCALAR=""
  D_MEDIAN_SIMD=""
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

    c_line="$(run_series "$module" wasm_compress_repeated "${mode}-compress" "$INNER_ITERS" "$PAYLOAD_BYTES")"
    d_line="$(run_series "$module" wasm_decompress_repeated "${mode}-decompress" "$INNER_ITERS" "$PAYLOAD_BYTES")"
    echo "$c_line"
    echo "$d_line"

    c_med="$(echo "$c_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
    d_med="$(echo "$d_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
    if [[ "$mode" == "scalar" ]]; then
      C_MEDIAN_SCALAR="$c_med"
      D_MEDIAN_SCALAR="$d_med"
    else
      C_MEDIAN_SIMD="$c_med"
      D_MEDIAN_SIMD="$d_med"
    fi

    {
      echo "## Runtime (${mode})"
      echo "- block roundtrip: ${block_ok}"
      echo "- frame roundtrip: ${frame_ok}"
      echo "- hash consistency: ${hash_ok}"
      echo "- benchmark shape: ${SAMPLES} samples, ${WARMUP} warmup, ${INNER_ITERS} iterations/sample, payload ${PAYLOAD_BYTES} bytes"
      echo "- ${c_line}"
      echo "- ${d_line}"
      echo
    } >> wasm-benchmark-report.txt
  done

  c_speedup="$(awk -v s="${C_MEDIAN_SCALAR}" -v v="${C_MEDIAN_SIMD}" 'BEGIN{if (v==0) print "inf"; else printf "%.2fx", s/v}')"
  d_speedup="$(awk -v s="${D_MEDIAN_SCALAR}" -v v="${D_MEDIAN_SIMD}" 'BEGIN{if (v==0) print "inf"; else printf "%.2fx", s/v}')"
  echo "speedup compress (median): ${c_speedup}"
  echo "speedup decompress (median): ${d_speedup}"
  {
    echo "## Speedup Summary"
    echo "- compress median speedup (scalar/simd): ${c_speedup}"
    echo "- decompress median speedup (scalar/simd): ${d_speedup}"
  } >> wasm-benchmark-report.txt
else
  echo "wasmtime not found; skipping runtime invocation checks."
  echo >> wasm-benchmark-report.txt
  echo "wasmtime not found; runtime checks skipped." >> wasm-benchmark-report.txt
fi

echo
cat wasm-benchmark-report.txt
