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

## Baseline

- Scalar (`RUSTFLAGS=""`): `96,792` bytes
- SIMD (`RUSTFLAGS="-C target-feature=+simd128"`): `96,929` bytes

## Runtime baseline (wasmtime)

- Scalar compress repeated (`120 x 256KiB`): `22 ms`
- SIMD compress repeated (`120 x 256KiB`): `15 ms`
- Scalar decompress repeated (`120 x 256KiB`): `27 ms`
- SIMD decompress repeated (`120 x 256KiB`): `27 ms`

## Notes

- These numbers are for the generated cdylib artifact at:
  - `target-scalar/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm`
  - `target-simd/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm`
- Runtime validation calls exported wasm entrypoints (`wasm_block_roundtrip`, `wasm_frame_roundtrip`,
  `wasm_hash_consistency`) to ensure codec paths are reachable and exercised.
- CI trend reporting is non-blocking and should be compared against this baseline.
