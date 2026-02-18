# 3-Library WASM Benchmark Results

Date: 2026-02-18

Command:

```bash
SAMPLES=5 WARMUP=2 INNER_ITERS=200 PAYLOAD_BYTES=262144 ./scripts/benchmark_wasm.sh
```

- Target: `wasm32-wasip1`
- Fixtures:
  - `bench-data/text_50kb.txt` (`sha256=5caae87b9112f265d87fb3589abffdabd975969441cb1f1cc6ed3dcc6fa7459c`)
  - `bench-data/json_50kb.json` (`sha256=3cda5cd146fd09191a61b188f7ab228769d733f363f29751bec1cf3fa7e0ba6d`)
- Raw report: `wasm-benchmark-report.md` (generated, gitignored)

## Implementations

| Implementation | Mode | WASM size |
|---|---|---:|
| `lz4_flex_wasm_simd` | scalar | 211,893 B |
| `lz4_flex_wasm_simd` | simd128 | 211,821 B |
| `lz4_flex` | scalar | 169,957 B |
| `lz_fear` | scalar | 173,905 B |

## Median Runtime (ms)

### Synthetic (`PAYLOAD_BYTES=262144`)

| Case | simdcrate/scalar | simdcrate/simd | lz4_flex/scalar | lz_fear/scalar |
|---|---:|---:|---:|---:|
| compress | 35 | 21 | 27 | 25 |
| decompress (repetitive-json) | 27 | 28 | 111 | 116 |
| decompress (wcol-index-like) | 46 | 47 | 68 | 279 |
| decompress (wcol-bitmap-like) | 23 | 22 | 52 | 78 |
| decompress (wcol-string-page-like) | 45 | 43 | 66 | 209 |

### Real-world Fixtures (50 KB)

| Case | simdcrate/scalar | simdcrate/simd | lz4_flex/scalar | lz_fear/scalar |
|---|---:|---:|---:|---:|
| compress (text) | 35 | 41 | 38 | 67 |
| decompress (text) | 26 | 27 | 28 | 81 |
| compress (json) | 27 | 30 | 26 | 39 |
| decompress (json) | 31 | 24 | 30 | 47 |

## Observations

- On synthetic WCOL-like decompress workloads, `lz4_flex_wasm_simd` is substantially faster than both `lz4_flex` and `lz_fear`.
- On real 50 KB fixtures:
  - Text fixture: `lz4_flex_wasm_simd` scalar is slightly better than `lz4_flex` in both compress and decompress.
  - JSON fixture: `lz4_flex` and `lz4_flex_wasm_simd` scalar are close; SIMD helps `lz4_flex_wasm_simd` on JSON decompress.
- `lz_fear` is consistently slower on decompress for these workloads.

## Repro Steps

1. `./scripts/prepare_bench_fixtures.sh`
2. `SAMPLES=5 WARMUP=2 INNER_ITERS=200 PAYLOAD_BYTES=262144 ./scripts/benchmark_wasm.sh`

