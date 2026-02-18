# WASM Size Baseline

Date: 2026-02-18

## Method

Command:

```bash
./scripts/benchmark_wasm.sh
```

Target: `wasm32-wasip1`  
Features: `frame,block,wasm-exports`  
Build mode: `release` (`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`)
Runtime: `wasmtime` with 3 warmups + 11 samples, each sample runs 800 in-wasm iterations over 256KiB payload.

## Baseline

- Scalar (`RUSTFLAGS=""`): `96,792` bytes
- SIMD (`RUSTFLAGS="-C target-feature=+simd128"`): `96,929` bytes

## Runtime baseline (wasmtime)

- Scalar compress median: `86 ms` (p95 `87 ms`)
- SIMD compress median: `40 ms` (p95 `42 ms`)
- Scalar decompress median: `118 ms` (p95 `118 ms`)
- SIMD decompress median: `118 ms` (p95 `118 ms`)
- Compress speedup (median): `2.15x`
- Decompress speedup (median): `1.00x`

## Notes

- These numbers are for the generated cdylib artifact at:
  - `target-scalar/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm`
  - `target-simd/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm`
- Runtime validation calls exported wasm entrypoints (`wasm_block_roundtrip`, `wasm_frame_roundtrip`,
  `wasm_hash_consistency`) to ensure codec paths are reachable and exercised.
- Decompress currently shows no SIMD speedup because there is no wasm SIMD decompression fast path wired yet.
- CI trend reporting is non-blocking and should be compared against this baseline.
