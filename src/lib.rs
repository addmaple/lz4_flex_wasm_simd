//! Opinionated LZ4 implementation optimized for WASM SIMD and small binaries.
//!
//! Feature variants:
//! - `block`: block compression/decompression API.
//! - `frame`: frame API (internally uses block primitives).
//! - `frame,block`: both APIs.

#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg_attr(test, macro_use)]
extern crate alloc;

#[cfg(test)]
#[macro_use]
extern crate more_asserts;

pub mod hash;

#[cfg(feature = "block")]
pub mod block;

#[cfg(all(feature = "frame", not(feature = "block")))]
#[path = "block/mod.rs"]
mod block;

#[cfg(feature = "frame")]
pub mod frame;

#[allow(dead_code)]
mod fastcpy;

#[cfg(not(all(feature = "safe-encode", feature = "safe-decode")))]
#[allow(dead_code)]
mod fastcpy_unsafe;

#[cfg(any(
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(
        feature = "wasm-simd",
        target_arch = "wasm32",
        target_feature = "simd128"
    )
))]
pub mod simd;

#[cfg_attr(
    all(feature = "safe-encode", feature = "safe-decode"),
    forbid(unsafe_code)
)]
pub(crate) mod sink;

#[cfg(feature = "wasm-exports")]
#[allow(missing_docs)]
mod wasm_exports;
