// Source provenance: derived from https://github.com/shepmaster/twox-hash (MIT), commit bc5bb80b4857707e0372d2386157b1d31e4441d3, adapted for minimal internal use.
//! Minimal xxHash32 implementation used by the frame codec.

use core::hash::Hasher;

const PRIME32_1: u32 = 0x9E37_79B1;
const PRIME32_2: u32 = 0x85EB_CA77;
const PRIME32_3: u32 = 0xC2B2_AE3D;
const PRIME32_4: u32 = 0x27D4_EB2F;
const PRIME32_5: u32 = 0x1656_67B1;
const STRIPE_LEN: usize = 16;

#[inline]
#[cfg_attr(
    all(target_arch = "wasm32", target_feature = "simd128"),
    allow(dead_code)
)]
fn round(acc: u32, lane: u32) -> u32 {
    let acc = acc.wrapping_add(lane.wrapping_mul(PRIME32_2));
    acc.rotate_left(13).wrapping_mul(PRIME32_1)
}

#[inline]
fn avalanche(mut hash: u32) -> u32 {
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(PRIME32_2);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(PRIME32_3);
    hash ^ (hash >> 16)
}

/// Streaming xxHash32 state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XxHash32 {
    seed: u32,
    total_len: u64,
    v1: u32,
    v2: u32,
    v3: u32,
    v4: u32,
    mem: [u8; STRIPE_LEN],
    mem_size: usize,
}

impl XxHash32 {
    /// Create a hasher with the provided seed.
    #[inline]
    pub fn with_seed(seed: u32) -> Self {
        Self {
            seed,
            total_len: 0,
            v1: seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2),
            v2: seed.wrapping_add(PRIME32_2),
            v3: seed,
            v4: seed.wrapping_sub(PRIME32_1),
            mem: [0; STRIPE_LEN],
            mem_size: 0,
        }
    }

    /// Hash a full buffer in one shot.
    #[inline]
    pub fn oneshot(seed: u32, data: &[u8]) -> u32 {
        let mut hasher = Self::with_seed(seed);
        hasher.write(data);
        hasher.finish_32()
    }

    #[inline]
    #[cfg_attr(
        all(target_arch = "wasm32", target_feature = "simd128"),
        allow(dead_code)
    )]
    fn update_scalar<'a>(&mut self, mut input: &'a [u8]) -> &'a [u8] {
        while input.len() >= STRIPE_LEN {
            let lane1 = u32::from_le_bytes(input[0..4].try_into().expect("lane1"));
            let lane2 = u32::from_le_bytes(input[4..8].try_into().expect("lane2"));
            let lane3 = u32::from_le_bytes(input[8..12].try_into().expect("lane3"));
            let lane4 = u32::from_le_bytes(input[12..16].try_into().expect("lane4"));

            self.v1 = round(self.v1, lane1);
            self.v2 = round(self.v2, lane2);
            self.v3 = round(self.v3, lane3);
            self.v4 = round(self.v4, lane4);
            input = &input[STRIPE_LEN..];
        }
        input
    }

    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    #[inline]
    fn update_simd<'a>(&mut self, mut input: &'a [u8]) -> &'a [u8] {
        use core::arch::wasm32::*;

        let prime2 = i32x4_splat(PRIME32_2 as i32);
        let prime1 = i32x4_splat(PRIME32_1 as i32);
        let mut acc = unsafe {
            core::ptr::read_unaligned(
                (&[self.v1, self.v2, self.v3, self.v4]).as_ptr() as *const v128
            )
        };

        while input.len() >= STRIPE_LEN {
            let lanes = unsafe { core::ptr::read_unaligned(input.as_ptr() as *const v128) };
            let prod = i32x4_mul(lanes, prime2);
            acc = i32x4_add(acc, prod);
            let left = i32x4_shl(acc, 13);
            let right = u32x4_shr(acc, 19);
            acc = v128_or(left, right);
            acc = i32x4_mul(acc, prime1);
            input = &input[STRIPE_LEN..];
        }

        let mut out = [0u32; 4];
        unsafe { core::ptr::write_unaligned(out.as_mut_ptr() as *mut v128, acc) };
        self.v1 = out[0];
        self.v2 = out[1];
        self.v3 = out[2];
        self.v4 = out[3];

        input
    }

    #[inline]
    fn update<'a>(&mut self, input: &'a [u8]) -> &'a [u8] {
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            return self.update_simd(input);
        }

        #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
        {
            self.update_scalar(input)
        }
    }

    /// Finalize and return the 32-bit digest.
    pub fn finish_32(&self) -> u32 {
        let mut hash = if self.total_len >= STRIPE_LEN as u64 {
            self.v1
                .rotate_left(1)
                .wrapping_add(self.v2.rotate_left(7))
                .wrapping_add(self.v3.rotate_left(12))
                .wrapping_add(self.v4.rotate_left(18))
        } else {
            self.seed.wrapping_add(PRIME32_5)
        };

        hash = hash.wrapping_add(self.total_len as u32);

        let mut rem = &self.mem[..self.mem_size];
        while rem.len() >= 4 {
            let lane = u32::from_le_bytes(rem[0..4].try_into().expect("remainder lane"));
            hash = hash.wrapping_add(lane.wrapping_mul(PRIME32_3));
            hash = hash.rotate_left(17).wrapping_mul(PRIME32_4);
            rem = &rem[4..];
        }

        for &b in rem {
            hash = hash.wrapping_add((b as u32).wrapping_mul(PRIME32_5));
            hash = hash.rotate_left(11).wrapping_mul(PRIME32_1);
        }

        avalanche(hash)
    }
}

impl Default for XxHash32 {
    fn default() -> Self {
        Self::with_seed(0)
    }
}

impl Hasher for XxHash32 {
    #[inline]
    fn finish(&self) -> u64 {
        self.finish_32() as u64
    }

    fn write(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);

        if self.mem_size + data.len() < STRIPE_LEN {
            self.mem[self.mem_size..self.mem_size + data.len()].copy_from_slice(data);
            self.mem_size += data.len();
            return;
        }

        if self.mem_size > 0 {
            let fill = STRIPE_LEN - self.mem_size;
            self.mem[self.mem_size..].copy_from_slice(&data[..fill]);
            let block = self.mem;
            self.update(&block);
            self.mem_size = 0;
            data = &data[fill..];
        }

        let rem = self.update(data);
        let rem_len = rem.len();
        self.mem[..rem_len].copy_from_slice(rem);
        self.mem_size = rem_len;
    }
}

#[cfg(test)]
mod tests {
    use core::hash::Hasher;

    use super::XxHash32;

    #[test]
    fn known_vector_empty() {
        assert_eq!(XxHash32::oneshot(0, b""), 0x02cc_5d05);
    }

    #[test]
    fn matches_twox_hash_known_input() {
        let data = b"Hello, world!";
        assert_eq!(
            XxHash32::oneshot(0, data),
            twox_hash::XxHash32::oneshot(0, data)
        );
    }

    #[test]
    fn streaming_matches_oneshot() {
        let data = b"xxhash32 streaming test data for lz4 frame";
        let expected = XxHash32::oneshot(42, data);
        let mut h = XxHash32::with_seed(42);
        h.write(&data[..7]);
        h.write(&data[7..23]);
        h.write(&data[23..]);
        assert_eq!(h.finish_32(), expected);
    }
}
