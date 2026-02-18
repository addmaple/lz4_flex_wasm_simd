# Release Checklist

## Pre-release validation

1. Run native tests:
   - `cargo test`
   - `cargo test --no-default-features --features frame`
   - `cargo test --no-default-features --features frame,block`
2. Run wasm checks:
   - `cargo check --target wasm32-wasip1 --no-default-features --features frame,block`
   - `RUSTFLAGS='-C target-feature=+simd128' cargo check --target wasm32-wasip1 --no-default-features --features frame,block`
3. Run local size/perf helper:
   - `./scripts/benchmark_wasm.sh`

## Provenance and licensing

1. Ensure `ATTRIBUTION.md` is up to date.
2. Ensure `PROVENANCE.toml` includes all vendored files and commit references.
3. Ensure `LICENSE` is present and unchanged.

## CI status

1. Native matrix job green (`block`, `frame`, `frame+block`).
2. WASM scalar and SIMD check job green.
3. WASM size trend artifact uploaded (non-blocking).

## Package sanity

1. Verify publishable files:
   - `cargo package --allow-dirty --no-verify`
2. Check crate metadata:
   - `cargo metadata --no-deps`
3. Confirm README examples compile where applicable.

## Tagging and release

1. Update version in `Cargo.toml`.
2. Update release notes / changelog.
3. Create annotated git tag (`vX.Y.Z`).
4. Publish crate (`cargo publish`) once dry run passes.
