//! FNV-1a 64-bit, buffered: `write` appends to a block buffer and `fold` runs the bytewise FNV
//! loop over each full block. On the GBA (`iwram` feature) `fold` is ARM code in IWRAM, so the
//! many small writes of `World::hash` stay inline Thumb stores and only one Thumb→ARM call is
//! paid per `BLOCK` bytes (Milestone 6, Task A). The hash value is the plain bytewise FNV-1a of
//! the concatenated input, whatever the chunking.

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;
const _: () = assert!(PRIME == (1 << 40) + 0x1b3);

/// Bytes buffered between two folds. 256 is the full range of `len: u8`, so the inline stores
/// need no index mask and no bounds check: on the parity route's worst frame (2253 bytes in 925
/// writes) `World::hash` took 63 lines at 256, 72 at 128 and 75 at 64 (Milestone 6, Task A).
const BLOCK: usize = 256;
const _: () = assert!(BLOCK == u8::MAX as usize + 1);
/// Writes of at most this many bytes are stored byte by byte, inline; longer ones are copied.
const SHORT: usize = 8;

/// `x * PRIME` (mod 2^64) with 32-bit multiplies only. Thumb (ARMv4T) has no long multiply, so
/// a plain u64 multiply is a slow `__aeabi_lmul` call per byte; PRIME = 2^40 + 0x1b3 lets the
/// product split into a shift and a 9-bit multiply.
#[cfg(any(test, not(all(feature = "iwram", target_arch = "arm"))))]
#[inline(always)]
const fn mul_prime(x: u64) -> u64 {
    let (lo, hi) = (x as u32, (x >> 32) as u32);
    // lo * 0x1b3 in 16-bit halves, so each partial product fits in 32 bits.
    let lo_prod = ((lo & 0xFFFF) * 0x1b3) as u64 + ((((lo >> 16) * 0x1b3) as u64) << 16);
    // hi * 0x1b3 lands at bit 32: only its low 32 bits survive.
    let hi_prod = (hi.wrapping_mul(0x1b3) as u64) << 32;
    lo_prod.wrapping_add(hi_prod).wrapping_add(x << 40)
}

/// The FNV-1a loop over `bytes`, starting from `state`.
#[cfg(not(all(feature = "iwram", target_arch = "arm")))]
fn fold(mut state: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        state = mul_prime(state ^ b as u64);
    }
    state
}

/// One FNV-1a byte on the state split in 32-bit halves: `((hi:lo) ^ b) * (2^40 + 0x1b3)` is one
/// `UMULL` (`lo * 0x1b3`), one `MUL` (`hi * 0x1b3`) and two adds in ARM state.
#[cfg(any(test, all(feature = "iwram", target_arch = "arm")))]
#[inline(always)]
fn step((lo, hi): (u32, u32), b: u8) -> (u32, u32) {
    let lo = lo ^ b as u32;
    let p = lo as u64 * 0x1b3;
    let hi = hi
        .wrapping_mul(0x1b3)
        .wrapping_add((p >> 32) as u32)
        .wrapping_add(lo << 8);
    (p as u32, hi)
}

/// The FNV-1a loop over `bytes`, starting from `state`: ARM code in IWRAM (zero wait states,
/// 32-bit fetches), where `UMULL` makes the plain u64 multiply cheap.
#[cfg(all(feature = "iwram", target_arch = "arm"))]
#[unsafe(link_section = ".iwram")]
#[instruction_set(arm::a32)]
#[inline(never)]
fn fold(state: u64, bytes: &[u8]) -> u64 {
    // Only primitives here: LLVM does not inline Thumb-compiled helpers (`chunks_exact`, ...)
    // into this ARM function, and each would be a call back into ROM.
    let mut s = (state as u32, (state >> 32) as u32);
    let mut rest = bytes;
    // Unrolled by 4: the loop counter and branch would otherwise cost a quarter of each byte.
    // Slice patterns, not indexing: no bounds checks in the loop.
    while let [a, b, c, d, tail @ ..] = rest {
        s = step(step(step(step(s, *a), *b), *c), *d);
        rest = tail;
    }
    while let [a, tail @ ..] = rest {
        s = step(s, *a);
        rest = tail;
    }
    s.0 as u64 | (s.1 as u64) << 32
}

/// FNV-1a 64-bit. Used for World hashes compared across GBA and native.
#[derive(Clone, Debug)]
pub struct Fnv64 {
    /// FNV state over every byte before `buf`.
    state: u64,
    buf: [u8; BLOCK],
    /// Bytes in `buf` (always below `BLOCK`: a full buffer is folded at once).
    len: u8,
}

impl Default for Fnv64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv64 {
    pub const fn new() -> Self {
        Self {
            state: OFFSET,
            buf: [0; BLOCK],
            len: 0,
        }
    }

    #[inline(always)]
    fn push(&mut self, b: u8) {
        let i = self.len as usize % BLOCK;
        self.buf[i] = b;
        if i + 1 == BLOCK {
            self.flush();
        } else {
            self.len = (i + 1) as u8;
        }
    }

    #[inline(never)]
    fn flush(&mut self) {
        self.state = fold(self.state, &self.buf);
        self.len = 0;
    }

    #[inline(always)]
    pub fn write(&mut self, bytes: &[u8]) {
        if bytes.len() <= SHORT {
            let i = self.len as usize % BLOCK;
            if i + bytes.len() < BLOCK {
                for (k, &b) in bytes.iter().enumerate() {
                    self.buf[(i + k) % BLOCK] = b;
                }
                self.len = (i + bytes.len()) as u8;
            } else {
                for &b in bytes {
                    self.push(b);
                }
            }
        } else {
            self.write_long(bytes);
        }
    }

    #[inline(never)]
    fn write_long(&mut self, mut bytes: &[u8]) {
        loop {
            let i = self.len as usize % BLOCK;
            let room = BLOCK - i;
            if bytes.len() < room {
                self.buf[i..i + bytes.len()].copy_from_slice(bytes);
                self.len = (i + bytes.len()) as u8;
                return;
            }
            let (head, rest) = bytes.split_at(room);
            self.buf[i..].copy_from_slice(head);
            self.flush();
            bytes = rest;
        }
    }

    #[inline(always)]
    pub fn write_u16(&mut self, v: u16) {
        self.write(&v.to_le_bytes());
    }

    #[inline(always)]
    pub fn write_u32(&mut self, v: u32) {
        self.write(&v.to_le_bytes());
    }

    /// The hash of everything written so far (the buffer is folded on a copy of the state).
    pub fn finish(&self) -> u64 {
        fold(self.state, &self.buf[..self.len as usize % BLOCK])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_offset_basis() {
        assert_eq!(Fnv64::new().finish(), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn known_vector_a() {
        // FNV-1a 64 of b"a"
        let mut h = Fnv64::new();
        h.write(b"a");
        assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn integer_writes_are_little_endian_bytes() {
        let mut a = Fnv64::new();
        a.write_u32(0x0403_0201);
        let mut b = Fnv64::new();
        b.write(&[1, 2, 3, 4]);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn split_multiply_matches_u64_multiply() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..10_000 {
            assert_eq!(mul_prime(x), x.wrapping_mul(PRIME));
            x = x.rotate_left(7) ^ x.wrapping_mul(3).wrapping_add(1);
        }
        for x in [0, 1, u64::MAX, u32::MAX as u64, (u32::MAX as u64) << 32] {
            assert_eq!(mul_prime(x), x.wrapping_mul(PRIME));
        }
    }

    /// The unbuffered bytewise FNV-1a 64 (the pre-Milestone 6 `Fnv64::write`).
    fn bytewise(bytes: &[u8]) -> u64 {
        bytes
            .iter()
            .fold(OFFSET, |h, &b| (h ^ b as u64).wrapping_mul(PRIME))
    }

    /// xorshift32: a deterministic stream for the split tests.
    fn rng(seed: &mut u32) -> u32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        *seed
    }

    fn chunked(bytes: &[u8], sizes: impl IntoIterator<Item = usize>) -> u64 {
        let mut h = Fnv64::new();
        let mut rest = bytes;
        for n in sizes {
            let (head, tail) = rest.split_at(n.min(rest.len()));
            h.write(head);
            rest = tail;
        }
        h.write(rest);
        h.finish()
    }

    #[test]
    fn explicit_splits_match_bytewise() {
        let data: Vec<u8> = (0..3 * BLOCK as u32 + 17)
            .map(|i| (i * 31 + 7) as u8)
            .collect();
        let want = bytewise(&data);
        let b = BLOCK;
        for sizes in [
            vec![],
            vec![1; data.len()],
            vec![b],
            vec![b - 1, 1, b, b + 1],
            vec![b - 1, 2],
            vec![SHORT, SHORT + 1, b - 2 * SHORT - 1, 3],
            vec![0, 0, 5, 0, b * 2],
            vec![data.len()],
        ] {
            assert_eq!(chunked(&data, sizes.clone()), want, "{sizes:?}");
        }
        for n in 0..=data.len() {
            assert_eq!(chunked(&data[..n], []), bytewise(&data[..n]), "len {n}");
        }
    }

    #[test]
    fn random_splits_match_bytewise() {
        let mut seed = 0x1234_5678;
        for _ in 0..10_000 {
            let len = rng(&mut seed) as usize % (2 * BLOCK + 40);
            let data: Vec<u8> = (0..len).map(|_| rng(&mut seed) as u8).collect();
            let mut h = Fnv64::new();
            let mut rest = &data[..];
            while !rest.is_empty() {
                // Mostly short writes (the World::hash shape), sometimes long ones.
                let n = match rng(&mut seed) % 8 {
                    0 => rng(&mut seed) as usize % (BLOCK + 20),
                    1 => 4,
                    2 => 2,
                    _ => rng(&mut seed) as usize % (SHORT + 2),
                };
                let (head, tail) = rest.split_at(n.min(rest.len()));
                match head.len() {
                    2 => h.write_u16(u16::from_le_bytes([head[0], head[1]])),
                    4 => h.write_u32(u32::from_le_bytes(head.try_into().unwrap())),
                    _ => h.write(head),
                }
                rest = tail;
                // `finish` mid-stream does not disturb the state.
                if rng(&mut seed).is_multiple_of(16) {
                    let done = data.len() - rest.len();
                    assert_eq!(h.finish(), bytewise(&data[..done]));
                }
            }
            assert_eq!(h.finish(), bytewise(&data), "len {len}");
        }
    }

    #[test]
    fn split_step_matches_u64_multiply() {
        let mut seed = 0x9E37_79B9;
        for _ in 0..10_000 {
            let x = (rng(&mut seed) as u64) << 32 | rng(&mut seed) as u64;
            let b = rng(&mut seed) as u8;
            let (lo, hi) = step((x as u32, (x >> 32) as u32), b);
            assert_eq!(
                lo as u64 | (hi as u64) << 32,
                (x ^ b as u64).wrapping_mul(PRIME)
            );
        }
    }

    #[test]
    fn fold_matches_bytewise() {
        let data: Vec<u8> = (0..1000u32).map(|i| (i ^ (i >> 3)) as u8).collect();
        assert_eq!(fold(OFFSET, &data), bytewise(&data));
    }
}
