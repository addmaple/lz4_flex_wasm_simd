# Attribution

This crate includes code derived from the following upstream projects, both under MIT license:

1. lz4_flex
- Upstream: https://github.com/pseitz/lz4_flex
- License: MIT
- Copyright: Pascal Seitz and contributors
- Components used: selected block/frame codec modules and low-level copy/sink helpers.

2. twox-hash
- Upstream: https://github.com/shepmaster/twox-hash
- License: MIT
- Copyright: Jake Goulding and contributors
- Components used: xxHash32 algorithm structure and constants, adapted to an internal minimal hasher with wasm SIMD lane updates.

All vendored files contain source provenance comments. A machine-readable mapping is provided in `PROVENANCE.toml`.
