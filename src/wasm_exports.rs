use alloc::{string::String, vec};
use alloc::vec::Vec;
use core::hash::Hasher;

use crate::hash::XxHash32;

#[derive(Default, Clone, Copy)]
struct DecodeMix {
    literal_bytes: u64,
    match_bytes: u64,
    overlap_path_bytes: u64,
    non_overlap_path_bytes: u64,
    off_1_bytes: u64,
    off_2_bytes: u64,
    off_3_7_bytes: u64,
    off_8_15_bytes: u64,
    off_ge_16_bytes: u64,
}

const CASE_REPETITIVE_JSON: u32 = 0;
const CASE_WCOL_INDEX_LIKE: u32 = 1;
const CASE_WCOL_BITMAP_LIKE: u32 = 2;
const CASE_WCOL_STRING_PAGE_LIKE: u32 = 3;
const PROFILE_COUNTER_FAST_TOKEN_HITS: u32 = 0;
const PROFILE_COUNTER_DUP_NONOVERLAP_WILD: u32 = 1;
const PROFILE_COUNTER_DUP_NEAR_END_EXACT_NONOVERLAP: u32 = 2;
const PROFILE_COUNTER_DUP_OVERLAP_SMALL_U64: u32 = 3;
const PROFILE_COUNTER_DUP_OVERLAP_LARGE_OFFSET_CHUNK: u32 = 4;
const PROFILE_COUNTER_DUP_OVERLAP_FALLBACK_BYTE: u32 = 5;
const PROFILE_COUNTER_COPY_FROM_DICT_CALLS: u32 = 6;
const PROFILE_COUNTER_LITERAL_BYTES: u32 = 7;
const PROFILE_COUNTER_MATCH_BYTES: u32 = 8;
const PROFILE_COUNTER_CHECKSUM: u32 = 100;

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
        let null_off = if i % 3 == 0 {
            u64::MAX
        } else {
            data_off + 2048
        };
        let null_comp_len: u32 = if i % 3 == 0 { 0 } else { 128 };
        let null_raw_len: u32 = if i % 3 == 0 { 0 } else { 256 };
        let empty_mode = if i % 7 == 0 { 2u32 } else { 0u32 };
        let empty_count = if empty_mode == 2 { 11u32 } else { 0u32 };
        let empty_off = if empty_mode == 2 {
            data_off + 3072
        } else {
            u64::MAX
        };
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
    // Rough v1-like header prelude.
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
        let s = alloc::format!(
            "https://{domain}/search/{path_group}/item/{:05}",
            i % 10_000
        );
        let a = prev.as_bytes();
        let b = s.as_bytes();
        let mut lcp = 0usize;
        while lcp < a.len().min(b.len()) && a[lcp] == b[lcp] {
            lcp += 1;
        }
        let suffix = &b[lcp..];
        out.extend_from_slice(&(i as u16).to_le_bytes()); // perm
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

#[no_mangle]
pub extern "C" fn wasm_block_roundtrip() -> i32 {
    let input = payload_repetitive_json(256 * 1024);
    let compressed = crate::block::compress_prepend_size(&input);
    match crate::block::decompress_size_prepended(&compressed) {
        Ok(decoded) if decoded == input => 1,
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn wasm_hash_consistency() -> i32 {
    let input = payload_repetitive_json(64 * 1024);
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
    let input = payload_repetitive_json(size as usize);
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

fn read_varint_len(input: &[u8], ip: &mut usize, base: usize) -> Option<usize> {
    if base != 15 {
        return Some(base);
    }
    let mut len = 15usize;
    loop {
        let b = *input.get(*ip)? as usize;
        *ip += 1;
        len += b;
        if b != 255 {
            return Some(len);
        }
    }
}

fn decode_mix_for_block(input: &[u8], output_len: usize) -> Option<DecodeMix> {
    let mut ip = 0usize;
    let mut op = 0usize;
    let mut mix = DecodeMix::default();

    while ip < input.len() {
        let token = *input.get(ip)?;
        ip += 1;

        let literal_length = read_varint_len(input, &mut ip, (token >> 4) as usize)?;
        if ip + literal_length > input.len() {
            return None;
        }
        ip += literal_length;
        op = op.checked_add(literal_length)?;
        mix.literal_bytes = mix.literal_bytes.saturating_add(literal_length as u64);

        if ip == input.len() {
            break;
        }

        let lo = *input.get(ip)? as usize;
        let hi = *input.get(ip + 1)? as usize;
        let offset = lo | (hi << 8);
        ip += 2;
        if offset == 0 {
            return None;
        }

        let match_length =
            read_varint_len(input, &mut ip, (token & 0x0F) as usize)?.checked_add(4)?;
        if op + match_length > output_len {
            return None;
        }

        mix.match_bytes = mix.match_bytes.saturating_add(match_length as u64);
        if offset == 1 {
            mix.off_1_bytes = mix.off_1_bytes.saturating_add(match_length as u64);
        } else if offset == 2 {
            mix.off_2_bytes = mix.off_2_bytes.saturating_add(match_length as u64);
        } else if offset < 8 {
            mix.off_3_7_bytes = mix.off_3_7_bytes.saturating_add(match_length as u64);
        } else if offset < 16 {
            mix.off_8_15_bytes = mix.off_8_15_bytes.saturating_add(match_length as u64);
        } else {
            mix.off_ge_16_bytes = mix.off_ge_16_bytes.saturating_add(match_length as u64);
        }

        let remaining_output = output_len - op;
        if offset < match_length + 15 || remaining_output < match_length + 15 {
            if offset >= match_length {
                mix.non_overlap_path_bytes = mix
                    .non_overlap_path_bytes
                    .saturating_add(match_length as u64);
            } else {
                mix.overlap_path_bytes = mix.overlap_path_bytes.saturating_add(match_length as u64);
            }
        } else {
            mix.non_overlap_path_bytes = mix
                .non_overlap_path_bytes
                .saturating_add(match_length as u64);
        }
        op += match_length;
    }

    if op != output_len {
        return None;
    }
    Some(mix)
}

fn decode_mix_for_payload(size: u32, case_id: u32) -> DecodeMix {
    let input = payload_for_case(size as usize, case_id);
    let compressed = crate::block::compress(&input);
    decode_mix_for_block(&compressed, input.len()).unwrap_or_default()
}

fn profile_counter_value(counter_id: u32) -> u64 {
    let s = crate::block::decompress::read_decompress_profile();
    match counter_id {
        PROFILE_COUNTER_FAST_TOKEN_HITS => s.fast_token_hits,
        PROFILE_COUNTER_DUP_NONOVERLAP_WILD => s.duplicate_nonoverlap_wild,
        PROFILE_COUNTER_DUP_NEAR_END_EXACT_NONOVERLAP => s.duplicate_near_end_exact_nonoverlap,
        PROFILE_COUNTER_DUP_OVERLAP_SMALL_U64 => s.duplicate_overlap_small_u64,
        PROFILE_COUNTER_DUP_OVERLAP_LARGE_OFFSET_CHUNK => s.duplicate_overlap_large_offset_chunk,
        PROFILE_COUNTER_DUP_OVERLAP_FALLBACK_BYTE => s.duplicate_overlap_fallback_byte,
        PROFILE_COUNTER_COPY_FROM_DICT_CALLS => s.copy_from_dict_calls,
        PROFILE_COUNTER_LITERAL_BYTES => s.literal_bytes,
        PROFILE_COUNTER_MATCH_BYTES => s.match_bytes,
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn wasm_decompress_repeated(iters: u32, size: u32) -> u64 {
    wasm_decompress_repeated_case(iters, size, CASE_REPETITIVE_JSON)
}

#[no_mangle]
pub extern "C" fn wasm_decompress_repeated_case(iters: u32, size: u32, case_id: u32) -> u64 {
    let input = payload_for_case(size as usize, case_id);
    let compressed = crate::block::compress(&input);
    let mut output = vec![0u8; input.len()];
    let mut acc = 0u64;

    for _ in 0..iters {
        match crate::block::decompress_into(&compressed, &mut output) {
            Ok(decoded_len) => {
                let decoded = &output[..decoded_len];
                acc = acc
                    .wrapping_add(decoded.len() as u64)
                    .wrapping_add(decoded.first().copied().unwrap_or(0) as u64);
            }
            Err(_) => return 0,
        }
    }

    acc
}

#[no_mangle]
pub extern "C" fn wasm_decompress_profile_reset() {
    crate::block::decompress::reset_decompress_profile();
}

#[no_mangle]
pub extern "C" fn wasm_decompress_profile_run_case(iters: u32, size: u32, case_id: u32) -> u64 {
    wasm_decompress_repeated_case(iters, size, case_id)
}

#[no_mangle]
pub extern "C" fn wasm_decompress_profile_counter(counter_id: u32) -> u64 {
    profile_counter_value(counter_id)
}

#[no_mangle]
pub extern "C" fn wasm_decompress_profile_run_case_counter(
    iters: u32,
    size: u32,
    case_id: u32,
    counter_id: u32,
) -> u64 {
    crate::block::decompress::reset_decompress_profile();
    let checksum = wasm_decompress_repeated_case(iters, size, case_id);
    if counter_id == PROFILE_COUNTER_CHECKSUM {
        return checksum;
    }
    profile_counter_value(counter_id)
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_literal_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).literal_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_match_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).match_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_overlap_path_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).overlap_path_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_non_overlap_path_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).non_overlap_path_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_1_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).off_1_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_2_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).off_2_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_3_7_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).off_3_7_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_8_15_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).off_8_15_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_ge_16_bytes(size: u32) -> u64 {
    decode_mix_for_payload(size, CASE_REPETITIVE_JSON).off_ge_16_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_literal_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).literal_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_match_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).match_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_overlap_path_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).overlap_path_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_non_overlap_path_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).non_overlap_path_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_1_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).off_1_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_2_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).off_2_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_3_7_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).off_3_7_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_8_15_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).off_8_15_bytes
}

#[no_mangle]
pub extern "C" fn wasm_decompress_mix_offset_ge_16_bytes_case(size: u32, case_id: u32) -> u64 {
    decode_mix_for_payload(size, case_id).off_ge_16_bytes
}

#[cfg(feature = "frame")]
#[no_mangle]
pub extern "C" fn wasm_frame_roundtrip() -> i32 {
    use std::io::{Read, Write};

    let input = payload_repetitive_json(256 * 1024);

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
