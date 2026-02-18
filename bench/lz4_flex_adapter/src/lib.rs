use std::string::String;
use std::vec;
use std::vec::Vec;

const CASE_WCOL_INDEX_LIKE: u32 = 1;
const CASE_WCOL_BITMAP_LIKE: u32 = 2;
const CASE_WCOL_STRING_PAGE_LIKE: u32 = 3;
const FIXTURE_TEXT_50KB: u32 = 0;
const FIXTURE_JSON_50KB: u32 = 1;
const TEXT_50KB_BYTES: &[u8] = include_bytes!("../../../bench-data/text_50kb.txt");
const JSON_50KB_BYTES: &[u8] = include_bytes!("../../../bench-data/json_50kb.json");

fn payload_repetitive_json(size: usize) -> Vec<u8> {
    let pat =
        br#"{"id":12345,"name":"lz4_flex_wasm_simd","kind":"benchmark","values":[1,2,3,4,5]}"#;
    let mut out = vec![0u8; size.max(64)];
    for (i, b) in out.iter_mut().enumerate() {
        *b = pat[i % pat.len()];
    }
    out
}

fn payload_wcol_index_like(size: usize) -> Vec<u8> {
    let entry_size = 80usize;
    let n = (size.max(entry_size) / entry_size).max(1);
    let mut out = Vec::with_capacity(n * entry_size);
    for i in 0..n {
        let data_off = 1_000_000u64 + (i as u64 * 4096);
        let data_comp_len = 768u32 + ((i % 5) as u32 * 16);
        let data_raw_len = 1024u32;
        let null_off = if i % 3 == 0 { u64::MAX } else { data_off + 2048 };
        let null_comp_len: u32 = if i % 3 == 0 { 0 } else { 128 };
        let null_raw_len: u32 = if i % 3 == 0 { 0 } else { 256 };
        let empty_mode = if i % 7 == 0 { 2u32 } else { 0u32 };
        let empty_count = if empty_mode == 2 { 11u32 } else { 0u32 };
        let empty_off = if empty_mode == 2 { data_off + 3072 } else { u64::MAX };
        let empty_comp_len = if empty_mode == 2 { 96u32 } else { 0u32 };
        let empty_raw_len = if empty_mode == 2 { 256u32 } else { 0u32 };
        let min = i as f64;
        let max = (i as f64) + 100.0;
        let presence = if i % 5 == 0 { 0xFFFFu32 } else { 0x0FFFu32 };

        out.extend_from_slice(&data_off.to_le_bytes());
        out.extend_from_slice(&data_comp_len.to_le_bytes());
        out.extend_from_slice(&data_raw_len.to_le_bytes());
        out.extend_from_slice(&null_off.to_le_bytes());
        out.extend_from_slice(&null_comp_len.to_le_bytes());
        out.extend_from_slice(&null_raw_len.to_le_bytes());
        out.extend_from_slice(&empty_mode.to_le_bytes());
        out.extend_from_slice(&empty_count.to_le_bytes());
        out.extend_from_slice(&empty_off.to_le_bytes());
        out.extend_from_slice(&empty_comp_len.to_le_bytes());
        out.extend_from_slice(&empty_raw_len.to_le_bytes());
        out.extend_from_slice(&min.to_le_bytes());
        out.extend_from_slice(&max.to_le_bytes());
        out.extend_from_slice(&presence.to_le_bytes());
    }
    out
}

fn payload_wcol_bitmap_like(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size.max(128)];
    for i in (0..out.len()).step_by(257) {
        out[i] = 0xFF;
    }
    for i in (13..out.len()).step_by(521) {
        out[i] = 0x0F;
    }
    out
}

fn payload_wcol_string_page_like(size: usize) -> Vec<u8> {
    let rows = (size / 24).clamp(64, 16_384);
    let mut out = Vec::with_capacity(size.max(rows * 12));
    out.extend_from_slice(&(rows as u16).to_le_bytes());
    out.extend_from_slice(&[2u8, 2u8, 0, 0, 0, 0]);
    while out.len() < 32 {
        out.push(0);
    }

    let mut prev = String::new();
    for i in 0..rows {
        let domain = match i % 4 {
            0 => "google.com",
            1 => "youtube.com",
            2 => "example.org",
            _ => "wikipedia.org",
        };
        let path_group = (i / 64) % 16;
        let s = format!("https://{domain}/search/{path_group}/item/{:05}", i % 10_000);
        let a = prev.as_bytes();
        let b = s.as_bytes();
        let mut lcp = 0usize;
        while lcp < a.len().min(b.len()) && a[lcp] == b[lcp] {
            lcp += 1;
        }
        let suffix = &b[lcp..];
        out.extend_from_slice(&(i as u16).to_le_bytes());
        out.extend_from_slice(&(lcp as u16).to_le_bytes());
        out.extend_from_slice(&(suffix.len() as u16).to_le_bytes());
        out.extend_from_slice(suffix);
        prev = s;
    }

    if out.len() > size {
        out.truncate(size);
    }
    out
}

fn payload_for_case(size: usize, case_id: u32) -> Vec<u8> {
    match case_id {
        CASE_WCOL_INDEX_LIKE => payload_wcol_index_like(size),
        CASE_WCOL_BITMAP_LIKE => payload_wcol_bitmap_like(size),
        CASE_WCOL_STRING_PAGE_LIKE => payload_wcol_string_page_like(size),
        _ => payload_repetitive_json(size),
    }
}

fn payload_for_fixture(fixture_id: u32) -> &'static [u8] {
    match fixture_id {
        FIXTURE_JSON_50KB => JSON_50KB_BYTES,
        FIXTURE_TEXT_50KB => TEXT_50KB_BYTES,
        _ => TEXT_50KB_BYTES,
    }
}

fn hash_update(mut h: u32, data: &[u8]) -> u32 {
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

#[no_mangle]
pub extern "C" fn wasm_block_roundtrip() -> i32 {
    let input = payload_repetitive_json(256 * 1024);
    let compressed = lz4_flex::block::compress(&input);
    match lz4_flex::block::decompress(&compressed, input.len()) {
        Ok(decoded) if decoded == input => 1,
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn wasm_hash_consistency() -> i32 {
    let input = payload_repetitive_json(64 * 1024);
    let mut expected = 0x811c9dc5u32;
    expected = hash_update(expected, &input);
    let split = input.len() / 3;
    let mut h = 0x811c9dc5u32;
    h = hash_update(h, &input[..split]);
    h = hash_update(h, &input[split..2 * split]);
    h = hash_update(h, &input[2 * split..]);
    if h == expected { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn wasm_compress_repeated(iters: u32, size: u32) -> u64 {
    let input = payload_repetitive_json(size as usize);
    let mut acc = 0u64;

    for i in 0..iters {
        let compressed = lz4_flex::block::compress(&input);
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
pub extern "C" fn wasm_compress_repeated_fixture(iters: u32, fixture_id: u32) -> u64 {
    let input = payload_for_fixture(fixture_id);
    let mut acc = 0u64;

    for i in 0..iters {
        let compressed = lz4_flex::block::compress(input);
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
pub extern "C" fn wasm_decompress_repeated_case(iters: u32, size: u32, case_id: u32) -> u64 {
    let input = payload_for_case(size as usize, case_id);
    let compressed = lz4_flex::block::compress(&input);
    let mut acc = 0u64;

    for _ in 0..iters {
        match lz4_flex::block::decompress(&compressed, input.len()) {
            Ok(decoded) if decoded == input => {
                acc = acc
                    .wrapping_add(decoded.len() as u64)
                    .wrapping_add(decoded.first().copied().unwrap_or(0) as u64);
            }
            _ => return 0,
        }
    }

    acc
}

#[no_mangle]
pub extern "C" fn wasm_decompress_repeated_fixture(iters: u32, fixture_id: u32) -> u64 {
    let input = payload_for_fixture(fixture_id);
    let compressed = lz4_flex::block::compress(input);
    let mut acc = 0u64;

    for _ in 0..iters {
        match lz4_flex::block::decompress(&compressed, input.len()) {
            Ok(decoded) if decoded == input => {
                acc = acc
                    .wrapping_add(decoded.len() as u64)
                    .wrapping_add(decoded.first().copied().unwrap_or(0) as u64);
            }
            _ => return 0,
        }
    }

    acc
}
