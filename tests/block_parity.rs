#![cfg(feature = "block")]

use lz4_flex_wasm_simd::block::{
    compress, compress_prepend_size, decompress, decompress_size_prepended,
};

#[test]
fn block_roundtrip_variants() {
    let data = b"lz4 block parity sample payload repeated repeated repeated";
    let compressed = compress(data);
    let restored = decompress(&compressed, data.len()).expect("decompress");
    assert_eq!(restored, data);

    let compressed2 = compress_prepend_size(data);
    let restored2 = decompress_size_prepended(&compressed2).expect("decompress prepend");
    assert_eq!(restored2, data);
}

#[test]
fn block_matches_lz4_flex_output_roundtrip() {
    let data = b"cross crate differential check payload 1234567890";
    let compressed = lz4_flex::block::compress(data);
    let restored = decompress(&compressed, data.len()).expect("decompress lz4_flex block");
    assert_eq!(restored, data);

    let compressed_local = compress(data);
    let restored_upstream = lz4_flex::block::decompress(&compressed_local, data.len())
        .expect("lz4_flex decompress local output");
    assert_eq!(restored_upstream, data);
}
