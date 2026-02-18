use std::hash::Hasher;

use lz4_flex_wasm_simd::hash::XxHash32;

#[test]
fn xxhash32_matches_twox_hash_oneshot() {
    let data = b"hash differential payload";
    let a = XxHash32::oneshot(0, data);
    let b = twox_hash::XxHash32::oneshot(0, data);
    assert_eq!(a, b);
}

#[test]
fn xxhash32_streaming_matches_twox_hash_streaming() {
    let data = b"streaming hash payload for chunked writes";

    let mut a = XxHash32::with_seed(42);
    a.write(&data[..5]);
    a.write(&data[5..17]);
    a.write(&data[17..]);

    let mut b = twox_hash::XxHash32::with_seed(42);
    b.write(&data[..5]);
    b.write(&data[5..17]);
    b.write(&data[17..]);

    assert_eq!(a.finish_32(), b.finish_32());
}
