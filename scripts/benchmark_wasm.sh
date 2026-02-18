#!/usr/bin/env bash
set -euo pipefail

SAMPLES="${SAMPLES:-11}"
WARMUP="${WARMUP:-3}"
INNER_ITERS="${INNER_ITERS:-800}"
PAYLOAD_BYTES="${PAYLOAD_BYTES:-262144}"
BENCH_REAL_FIXTURES="${BENCH_REAL_FIXTURES:-1}"
BENCH_FIXTURE_DIR="${BENCH_FIXTURE_DIR:-./bench-data}"
REPORT_PATH="wasm-benchmark-report.md"

CASE_IDS=(0 1 2 3)
CASE_NAMES=("repetitive-json" "wcol-index-like" "wcol-bitmap-like" "wcol-string-page-like")
REAL_FIXTURE_IDS=(0 1)
REAL_FIXTURE_NAMES=("real-text-50kb" "real-json-50kb")
REAL_FIXTURE_FILES=("text_50kb.txt" "json_50kb.json")

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODULES_FILE="$(mktemp)"
SUMMARY_FILE="$(mktemp)"
trap 'rm -f "$MODULES_FILE" "$SUMMARY_FILE"' EXIT

sha256_file() {
  local path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
    return
  fi
  shasum -a 256 "$path" | awk '{print $1}'
}

manifest_hash_for() {
  local manifest="$1"
  local filename="$2"
  awk -v f="$filename" '$2 == f {print $1}' "$manifest"
}

verify_real_fixtures() {
  local manifest="${BENCH_FIXTURE_DIR}/MANIFEST.sha256"
  if [[ ! -f "$manifest" ]]; then
    echo "missing fixture manifest: ${manifest}" >&2
    echo "run ./scripts/prepare_bench_fixtures.sh" >&2
    exit 1
  fi

  for i in "${!REAL_FIXTURE_FILES[@]}"; do
    local file="${REAL_FIXTURE_FILES[$i]}"
    local path="${BENCH_FIXTURE_DIR}/${file}"
    if [[ ! -f "$path" ]]; then
      echo "missing fixture file: ${path}" >&2
      echo "run ./scripts/prepare_bench_fixtures.sh" >&2
      exit 1
    fi

    local actual_size
    actual_size="$(wc -c < "$path" | tr -d ' ')"
    if [[ "$actual_size" != "51200" ]]; then
      echo "fixture size mismatch for ${path}: expected 51200 got ${actual_size}" >&2
      echo "run ./scripts/prepare_bench_fixtures.sh" >&2
      exit 1
    fi

    local expected_hash
    expected_hash="$(manifest_hash_for "$manifest" "$file")"
    if [[ -z "$expected_hash" ]]; then
      echo "manifest entry missing for ${file} in ${manifest}" >&2
      exit 1
    fi

    local actual_hash
    actual_hash="$(sha256_file "$path")"
    if [[ "$actual_hash" != "$expected_hash" ]]; then
      echo "fixture hash mismatch for ${path}" >&2
      echo "expected=${expected_hash} actual=${actual_hash}" >&2
      echo "run ./scripts/prepare_bench_fixtures.sh" >&2
      exit 1
    fi
  done
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

record_module() {
  local impl_id="$1"
  local mode="$2"
  local wasm_path="$3"
  local display="$4"
  echo "${impl_id}|${mode}|${wasm_path}|${display}" >> "$MODULES_FILE"
}

build_module() {
  local impl_id="$1"
  local mode="$2"
  local manifest_path="$3"
  local crate_name="$4"
  local rustflags="$5"
  local features="$6"
  local display="$7"
  local target_dir="target-bench-${impl_id}-${mode}"

  echo "== building ${display} (${mode}) =="
  if [[ -n "$features" ]]; then
    CARGO_TARGET_DIR="$target_dir" RUSTFLAGS="$rustflags" \
      cargo rustc --manifest-path "$manifest_path" --release --target wasm32-wasip1 --no-default-features --features "$features" --crate-type=cdylib
  else
    CARGO_TARGET_DIR="$target_dir" RUSTFLAGS="$rustflags" \
      cargo rustc --manifest-path "$manifest_path" --release --target wasm32-wasip1 --crate-type=cdylib
  fi

  local wasm_path="${target_dir}/wasm32-wasip1/release/${crate_name}.wasm"
  if [[ ! -f "$wasm_path" ]]; then
    echo "build missing artifact: ${wasm_path}" >&2
    exit 1
  fi
  echo "${display}-${mode} bytes: $(wc -c < "$wasm_path")"
  record_module "$impl_id" "$mode" "$wasm_path" "$display"
}

speedup_vs_baseline() {
  local metric="$1"
  local case_name="$2"
  local baseline
  baseline="$(awk -F'|' -v m="$metric" -v c="$case_name" '$1=="simdcrate" && $2=="scalar" && $3==m && $4==c {print $5}' "$SUMMARY_FILE" | head -n1)"
  if [[ -z "$baseline" ]]; then
    return
  fi

  echo "- ${metric} ${case_name}: baseline simdcrate/scalar median=${baseline}ms"
  while IFS='|' read -r impl_id mode metric_name case_label median_ms; do
    if [[ "$metric_name" != "$metric" || "$case_label" != "$case_name" ]]; then
      continue
    fi
    speedup="$(awk -v b="$baseline" -v v="$median_ms" 'BEGIN{if (v==0) print "inf"; else printf "%.2fx", b/v}')"
    echo "- ${impl_id}/${mode} median=${median_ms}ms speedup-vs-baseline=${speedup}"
  done < <(sort "$SUMMARY_FILE")
}

if [[ "$BENCH_REAL_FIXTURES" == "1" ]]; then
  verify_real_fixtures
fi

build_module "simdcrate" "scalar" "${ROOT_DIR}/Cargo.toml" "lz4_flex_wasm_simd" "" "frame,block,wasm-exports,decompress-prof" "lz4_flex_wasm_simd"
build_module "simdcrate" "simd" "${ROOT_DIR}/Cargo.toml" "lz4_flex_wasm_simd" "-C target-feature=+simd128" "frame,block,wasm-exports,decompress-prof" "lz4_flex_wasm_simd"
build_module "lz4_flex" "scalar" "${ROOT_DIR}/bench/lz4_flex_adapter/Cargo.toml" "lz4_flex_adapter" "" "" "lz4_flex"
build_module "lz_fear" "scalar" "${ROOT_DIR}/bench/lz_fear_adapter/Cargo.toml" "lz_fear_adapter" "" "" "lz_fear"

{
  echo "# WASM benchmark report"
  echo
  echo "target: wasm32-wasip1"
  echo "report path: ${REPORT_PATH}"
  echo "benchmark shape: ${SAMPLES} samples, ${WARMUP} warmup, ${INNER_ITERS} iterations/sample"
  echo
  echo "## Implementations"
  while IFS='|' read -r impl_id mode wasm_path display; do
    echo "- ${display}/${mode}: $(wc -c < "$wasm_path") bytes (${wasm_path})"
  done < "$MODULES_FILE"
  echo
  if [[ "$BENCH_REAL_FIXTURES" == "1" ]]; then
    echo "## Real-world Fixtures"
    echo "- fixture dir: ${BENCH_FIXTURE_DIR}"
    for i in "${!REAL_FIXTURE_FILES[@]}"; do
      file="${REAL_FIXTURE_FILES[$i]}"
      path="${BENCH_FIXTURE_DIR}/${file}"
      size="$(wc -c < "$path" | tr -d ' ')"
      hash="$(sha256_file "$path")"
      echo "- ${REAL_FIXTURE_NAMES[$i]} file=${file} size=${size} sha256=${hash}"
    done
    echo
  fi
} > "$REPORT_PATH"

if ! command -v wasmtime >/dev/null 2>&1; then
  echo "wasmtime not found; runtime checks skipped." | tee -a "$REPORT_PATH"
  exit 0
fi

echo "== wasm runtime validation (wasmtime) =="
while IFS='|' read -r impl_id mode wasm_path display; do
  block_ok="$(invoke "$wasm_path" wasm_block_roundtrip || true)"
  hash_ok="$(invoke "$wasm_path" wasm_hash_consistency || true)"
  echo "${display}/${mode} block_roundtrip=${block_ok} hash_consistency=${hash_ok}"
  if [[ "$block_ok" != "1" || "$hash_ok" != "1" ]]; then
    echo "runtime validation failed for ${display}/${mode}" >&2
    exit 1
  fi

  c_line="$(run_series "$wasm_path" wasm_compress_repeated "${impl_id}-${mode}-compress" "$INNER_ITERS" "$PAYLOAD_BYTES")"
  c_med="$(echo "$c_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
  echo "${impl_id}|${mode}|compress|synthetic-main|${c_med}" >> "$SUMMARY_FILE"

  {
    echo "## Runtime (${display}/${mode})"
    echo "- block roundtrip: ${block_ok}"
    echo "- hash consistency: ${hash_ok}"
    echo "- ${c_line}"
    echo "- decompress synthetic cases:"
  } >> "$REPORT_PATH"

  for i in "${!CASE_IDS[@]}"; do
    case_id="${CASE_IDS[$i]}"
    case_name="${CASE_NAMES[$i]}"
    d_line="$(run_series "$wasm_path" wasm_decompress_repeated_case "${impl_id}-${mode}-decompress-${case_name}" "$INNER_ITERS" "$PAYLOAD_BYTES" "$case_id")"
    d_med="$(echo "$d_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
    if [[ "$case_id" == "0" ]]; then
      echo "${impl_id}|${mode}|decompress|synthetic-main|${d_med}" >> "$SUMMARY_FILE"
    fi
    echo "- ${d_line}" >> "$REPORT_PATH"
  done

  if [[ "$BENCH_REAL_FIXTURES" == "1" ]]; then
    echo "- real-world fixtures:" >> "$REPORT_PATH"
    for i in "${!REAL_FIXTURE_IDS[@]}"; do
      fixture_id="${REAL_FIXTURE_IDS[$i]}"
      fixture_name="${REAL_FIXTURE_NAMES[$i]}"
      c_fixture_line="$(run_series "$wasm_path" wasm_compress_repeated_fixture "${impl_id}-${mode}-compress-${fixture_name}" "$INNER_ITERS" "$fixture_id")"
      d_fixture_line="$(run_series "$wasm_path" wasm_decompress_repeated_fixture "${impl_id}-${mode}-decompress-${fixture_name}" "$INNER_ITERS" "$fixture_id")"
      c_fix_med="$(echo "$c_fixture_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
      d_fix_med="$(echo "$d_fixture_line" | sed -E 's/.*median=([0-9]+).*/\1/')"
      echo "${impl_id}|${mode}|compress|${fixture_name}|${c_fix_med}" >> "$SUMMARY_FILE"
      echo "${impl_id}|${mode}|decompress|${fixture_name}|${d_fix_med}" >> "$SUMMARY_FILE"
      echo "- ${c_fixture_line}" >> "$REPORT_PATH"
      echo "- ${d_fixture_line}" >> "$REPORT_PATH"
    done
  fi

  echo >> "$REPORT_PATH"
done < "$MODULES_FILE"

{
  echo "## Comparison Summary"
  speedup_vs_baseline "compress" "synthetic-main"
  speedup_vs_baseline "decompress" "synthetic-main"
  if [[ "$BENCH_REAL_FIXTURES" == "1" ]]; then
    speedup_vs_baseline "compress" "real-text-50kb"
    speedup_vs_baseline "decompress" "real-text-50kb"
    speedup_vs_baseline "compress" "real-json-50kb"
    speedup_vs_baseline "decompress" "real-json-50kb"
  fi
} >> "$REPORT_PATH"

echo
cat "$REPORT_PATH"
