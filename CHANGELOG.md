# Changelog

## Unreleased

- Add `decompress-prof` counters and helpers so wasm builds can profile decode paths (new mix/offset counters, reset/read helpers) without affecting default builds.
- Extend the wasm benchmark script/export surface to query the new cases and profile counters, producing richer runtime reports.
