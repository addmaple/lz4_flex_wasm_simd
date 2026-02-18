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

## Provenance

This crate vendors selected code from:

- `pseitz/lz4_flex` (MIT)
- `shepmaster/twox-hash` (MIT)

See `ATTRIBUTION.md` and `PROVENANCE.toml`.
