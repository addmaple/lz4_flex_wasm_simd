#!/usr/bin/env bash
set -euo pipefail

SAMPLES="${SAMPLES:-11}"
WARMUP="${WARMUP:-3}"
INNER_ITERS="${INNER_ITERS:-800}"
PROFILE_ITERS="${PROFILE_ITERS:-200}"
PAYLOAD_BYTES="${PAYLOAD_BYTES:-262144}"
CASE_IDS=(0 1 2 3)
CASE_NAMES=("repetitive-json" "wcol-index-like" "wcol-bitmap-like" "wcol-string-page-like")
PROFILE_COUNTER_FAST_TOKEN_HITS=0
PROFILE_COUNTER_DUP_NONOVERLAP_WILD=1
PROFILE_COUNTER_DUP_NEAR_END_EXACT_NONOVERLAP=2
PROFILE_COUNTER_DUP_OVERLAP_SMALL_U64=3
PROFILE_COUNTER_DUP_OVERLAP_LARGE_OFFSET_CHUNK=4
PROFILE_COUNTER_DUP_OVERLAP_FALLBACK_BYTE=5
PROFILE_COUNTER_COPY_FROM_DICT_CALLS=6
PROFILE_COUNTER_LITERAL_BYTES=7
PROFILE_COUNTER_MATCH_BYTES=8
PROFILE_COUNTER_CHECKSUM=100

build_variant() {
  local name="$1"
  local rustflags="$2"
  local dir="target-${name}"

  echo "== building ${name} =="
  CARGO_TARGET_DIR="$dir" RUSTFLAGS="$rustflags" \
    cargo rustc --release --target wasm32-wasip1 --no-default-features --features frame,block,wasm-exports,decompress-prof --crate-type=cdylib

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
    echo "$c_line"

    c_med="$(echo "$c_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
    d_med=""
    if [[ "$mode" == "scalar" ]]; then
      C_MEDIAN_SCALAR="$c_med"
    else
      C_MEDIAN_SIMD="$c_med"
    fi

    {
      echo "## Runtime (${mode})"
      echo "- block roundtrip: ${block_ok}"
      echo "- frame roundtrip: ${frame_ok}"
      echo "- hash consistency: ${hash_ok}"
      echo "- benchmark shape: ${SAMPLES} samples, ${WARMUP} warmup, ${INNER_ITERS} iterations/sample, payload ${PAYLOAD_BYTES} bytes"
      echo "- ${c_line}"
      echo "- decompress cases:"
      for i in "${!CASE_IDS[@]}"; do
        case_id="${CASE_IDS[$i]}"
        case_name="${CASE_NAMES[$i]}"
        d_line="$(run_series "$module" wasm_decompress_repeated_case "${mode}-decompress-${case_name}" "$INNER_ITERS" "$PAYLOAD_BYTES" "$case_id")"
        mix_literal="$(invoke "$module" wasm_decompress_mix_literal_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_match="$(invoke "$module" wasm_decompress_mix_match_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_overlap="$(invoke "$module" wasm_decompress_mix_overlap_path_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_non_overlap="$(invoke "$module" wasm_decompress_mix_non_overlap_path_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_off1="$(invoke "$module" wasm_decompress_mix_offset_1_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_off2="$(invoke "$module" wasm_decompress_mix_offset_2_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_off3_7="$(invoke "$module" wasm_decompress_mix_offset_3_7_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_off8_15="$(invoke "$module" wasm_decompress_mix_offset_8_15_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        mix_off_ge16="$(invoke "$module" wasm_decompress_mix_offset_ge_16_bytes_case "$PAYLOAD_BYTES" "$case_id")"
        profile_checksum="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_CHECKSUM")"
        prof_fast_token="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_FAST_TOKEN_HITS")"
        prof_nonoverlap_wild="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_DUP_NONOVERLAP_WILD")"
        prof_near_end_exact="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_DUP_NEAR_END_EXACT_NONOVERLAP")"
        prof_overlap_small="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_DUP_OVERLAP_SMALL_U64")"
        prof_overlap_large="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_DUP_OVERLAP_LARGE_OFFSET_CHUNK")"
        prof_overlap_fallback="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_DUP_OVERLAP_FALLBACK_BYTE")"
        prof_dict_calls="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_COPY_FROM_DICT_CALLS")"
        prof_literal_bytes="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_LITERAL_BYTES")"
        prof_match_bytes="$(invoke "$module" wasm_decompress_profile_run_case_counter "$PROFILE_ITERS" "$PAYLOAD_BYTES" "$case_id" "$PROFILE_COUNTER_MATCH_BYTES")"
        echo "$d_line"
        echo "${mode}-decode-mix-${case_name} literal=${mix_literal} match=${mix_match} overlap_path=${mix_overlap} non_overlap_path=${mix_non_overlap} off1=${mix_off1} off2=${mix_off2} off3_7=${mix_off3_7} off8_15=${mix_off8_15} off_ge16=${mix_off_ge16}"
        echo "${mode}-decode-prof-${case_name} checksum=${profile_checksum} fast_token=${prof_fast_token} nonoverlap_wild=${prof_nonoverlap_wild} near_end_exact=${prof_near_end_exact} overlap_small=${prof_overlap_small} overlap_large=${prof_overlap_large} overlap_fallback=${prof_overlap_fallback} dict_calls=${prof_dict_calls} literal_bytes=${prof_literal_bytes} match_bytes=${prof_match_bytes}"
        if [[ "$case_id" == "0" ]]; then
          d_med="$(echo "$d_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
          if [[ "$mode" == "scalar" ]]; then
            D_MEDIAN_SCALAR="$d_med"
          else
            D_MEDIAN_SIMD="$d_med"
          fi
        fi
        echo "  - ${d_line}"
        echo "    mix literal=${mix_literal} match=${mix_match} overlap_path=${mix_overlap} non_overlap_path=${mix_non_overlap}"
        echo "    offsets off1=${mix_off1} off2=${mix_off2} off3_7=${mix_off3_7} off8_15=${mix_off8_15} off_ge16=${mix_off_ge16}"
        echo "    profile checksum=${profile_checksum} fast_token=${prof_fast_token} nonoverlap_wild=${prof_nonoverlap_wild} near_end_exact=${prof_near_end_exact}"
        echo "    profile overlap_small=${prof_overlap_small} overlap_large=${prof_overlap_large} overlap_fallback=${prof_overlap_fallback} dict_calls=${prof_dict_calls}"
        echo "    profile bytes literal=${prof_literal_bytes} match=${prof_match_bytes}"
      done
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
