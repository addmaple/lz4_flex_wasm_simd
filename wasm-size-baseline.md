# WASM Size Baseline

Date: 2026-02-18

## Method

Command:

```bash
./scripts/benchmark_wasm.sh
```

Target: `wasm32-wasip1`  
Features: `frame,block`  
Build mode: `release` (`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`)

## Baseline

- Scalar (`RUSTFLAGS=""`): `495` bytes
- SIMD (`RUSTFLAGS="-C target-feature=+simd128"`): `504` bytes

## Notes

- These numbers are for the generated cdylib artifact at:
  - `target-scalar/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm`
  - `target-simd/wasm32-wasip1/release/lz4_flex_wasm_simd.wasm`
- CI trend reporting is non-blocking and should be compared against this baseline.
