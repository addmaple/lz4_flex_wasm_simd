# lz4_flex_wasm_simd

Opinionated LZ4 crate focused on two goals:

1. High WASM SIMD throughput.
2. Low final wasm binary size.

## Feature variants

- `block` (default): block codec API.
- `frame`: frame codec API (internally includes required block pieces).
- `frame,block`: both public APIs.

## WASM SIMD

SIMD paths are compiled automatically on `wasm32` when `target-feature=+simd128` is enabled.

Example:

```bash
RUSTFLAGS='-C target-feature=+simd128' cargo test --target wasm32-wasip1 --features frame,block
```

For runtime verification and cross-library benchmark numbers (`lz4_flex_wasm_simd`, `lz4_flex`, `lz_fear`):

```bash
./scripts/prepare_bench_fixtures.sh
./scripts/benchmark_wasm.sh
```

This script builds `frame,block,wasm-exports` variants, runs wasm entrypoint checks with `wasmtime`,
and writes `wasm-benchmark-report.md`.

Tuning knobs:
- `SAMPLES` (default `11`)
- `WARMUP` (default `3`)
- `INNER_ITERS` (default `800`)
- `PAYLOAD_BYTES` (default `262144`)
- `BENCH_REAL_FIXTURES` (default `1`)
- `BENCH_FIXTURE_DIR` (default `./bench-data`)

### Data shapes where this crate is faster (current benches)

From `/Users/addmaple/sites/lz4_flex_wasm_simd/BENCHMARK_RESULTS.md` (2026-02-18 run):

- Decompress data shapes (dense integer index stream, sparse bitmap stream, string-page block stream): `lz4_flex_wasm_simd` is materially faster than `lz4_flex` and `lz_fear`.
- Repetitive JSON decompress (synthetic): `lz4_flex_wasm_simd` is much faster than `lz4_flex` and `lz_fear`.
- Real-world 50 KiB fixtures:
  - text fixture: `lz4_flex_wasm_simd` and `lz4_flex` are close; `lz4_flex_wasm_simd` is competitive on compress/decompress.
  - json fixture: scalar paths are close; SIMD mode improves `lz4_flex_wasm_simd` decompress.

## Provenance

This crate vendors selected code from:

- `pseitz/lz4_flex` (MIT)
- `shepmaster/twox-hash` (MIT)

See `ATTRIBUTION.md` and `PROVENANCE.toml`.
