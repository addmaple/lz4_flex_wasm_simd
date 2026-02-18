#![cfg(feature = "frame")]

use std::io::{Read, Write};

#[test]
fn frame_roundtrip_local() {
    let data = b"frame payload with enough bytes for checksum and block splitting";
    let mut enc = lz4_flex_wasm_simd::frame::FrameEncoder::new(Vec::new());
    enc.write_all(data).expect("write");
    let compressed = enc.finish().expect("finish");

    let mut dec = lz4_flex_wasm_simd::frame::FrameDecoder::new(&compressed[..]);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).expect("read");
    assert_eq!(out, data);
}

#[test]
fn frame_cross_compat_with_lz4_flex() {
    let data = b"frame compatibility payload abcabcabcabcabcabc";

    let mut enc = lz4_flex_wasm_simd::frame::FrameEncoder::new(Vec::new());
    enc.write_all(data).expect("write");
    let compressed = enc.finish().expect("finish");
    let mut out = Vec::new();
    let mut dec = lz4_flex::frame::FrameDecoder::new(&compressed[..]);
    dec.read_to_end(&mut out).expect("read local->upstream");
    assert_eq!(out, data);

    let mut enc2 = lz4_flex::frame::FrameEncoder::new(Vec::new());
    enc2.write_all(data).expect("write upstream");
    let compressed2 = enc2.finish().expect("finish upstream");
    let mut out2 = Vec::new();
    let mut dec2 = lz4_flex_wasm_simd::frame::FrameDecoder::new(&compressed2[..]);
    dec2.read_to_end(&mut out2).expect("read upstream->local");
    assert_eq!(out2, data);
}
