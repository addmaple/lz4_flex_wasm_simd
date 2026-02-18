use alloc::vec::Vec;
use core::hash::Hasher;

use crate::hash::XxHash32;

fn payload(size: usize) -> Vec<u8> {
    let pat =
        br#"{"id":12345,"name":"lz4_flex_wasm_simd","kind":"benchmark","values":[1,2,3,4,5]}"#;
    let mut out = vec![0u8; size.max(64)];
    for (i, b) in out.iter_mut().enumerate() {
        *b = pat[i % pat.len()];
    }
    out
}

#[no_mangle]
pub extern "C" fn wasm_block_roundtrip() -> i32 {
    let input = payload(256 * 1024);
    let compressed = crate::block::compress_prepend_size(&input);
    match crate::block::decompress_size_prepended(&compressed) {
        Ok(decoded) if decoded == input => 1,
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn wasm_hash_consistency() -> i32 {
    let input = payload(64 * 1024);
    let expected = XxHash32::oneshot(0, &input);
    let mut stream = XxHash32::with_seed(0);
    let split = input.len() / 3;
    stream.write(&input[..split]);
    stream.write(&input[split..2 * split]);
    stream.write(&input[2 * split..]);
    if stream.finish_32() == expected {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn wasm_compress_repeated(iters: u32, size: u32) -> u64 {
    let input = payload(size as usize);
    let mut acc = 0u64;

    for i in 0..iters {
        let compressed = crate::block::compress(&input);
        let len = compressed.len() as u64;
        let sample = compressed
            .get((i as usize) % compressed.len().max(1))
            .copied()
            .unwrap_or(0) as u64;
        acc = acc.wrapping_add(len).wrapping_add(sample << 8);
    }

    acc
}

#[no_mangle]
pub extern "C" fn wasm_decompress_repeated(iters: u32, size: u32) -> u64 {
    let input = payload(size as usize);
    let compressed = crate::block::compress_prepend_size(&input);
    let mut acc = 0u64;

    for _ in 0..iters {
        match crate::block::decompress_size_prepended(&compressed) {
            Ok(decoded) => {
                acc = acc
                    .wrapping_add(decoded.len() as u64)
                    .wrapping_add(decoded.first().copied().unwrap_or(0) as u64);
            }
            Err(_) => return 0,
        }
    }

    acc
}

#[cfg(feature = "frame")]
#[no_mangle]
pub extern "C" fn wasm_frame_roundtrip() -> i32 {
    use std::io::{Read, Write};

    let input = payload(256 * 1024);

    let mut enc = crate::frame::FrameEncoder::new(Vec::new());
    if enc.write_all(&input).is_err() {
        return 0;
    }
    let compressed = match enc.finish() {
        Ok(v) => v,
        Err(_) => return 0,
    };

    let mut out = Vec::new();
    let mut dec = crate::frame::FrameDecoder::new(&compressed[..]);
    if dec.read_to_end(&mut out).is_err() {
        return 0;
    }

    if out == input {
        1
    } else {
        0
    }
}
