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
#[cfg(any(
    test,
    not(any(
        target_pointer_width = "64",
        all(feature = "iwram", target_arch = "arm")
    ))
))]
#[inline(always)]
const fn mul_prime(x: u64) -> u64 {
    let (lo, hi) = (x as u32, (x >> 32) as u32);
    // lo * 0x1b3 in 16-bit halves, so each partial product fits in 32 bits.
    let lo_prod = ((lo & 0xFFFF) * 0x1b3) as u64 + ((((lo >> 16) * 0x1b3) as u64) << 16);
    // hi * 0x1b3 lands at bit 32: only its low 32 bits survive.
    let hi_prod = (hi.wrapping_mul(0x1b3) as u64) << 32;
    lo_prod.wrapping_add(hi_prod).wrapping_add(x << 40)
}

/// The FNV-1a loop over `bytes`, starting from `state`. 64-bit hosts multiply natively; 32-bit
/// targets without the IWRAM fold use `mul_prime`.
#[cfg(not(all(feature = "iwram", target_arch = "arm")))]
fn fold(mut state: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        #[cfg(target_pointer_width = "64")]
        {
            state = (state ^ b as u64).wrapping_mul(PRIME);
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            state = mul_prime(state ^ b as u64);
        }
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
#[unsafe(link_section = ".iwram.fnv64_fold")]
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

/// splitmix64 finalizer: spreads one (kind, id, value) word over 64 bits. The term function of
/// the digests maintained on write (`Vars`, `Bag`, `StrVars`).
pub const fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a 64-bit. Used for World hashes compared across GBA and native.
#[derive(Clone)]
pub struct Fnv64 {
    /// FNV state over every byte before `buf`.
    state: u64,
    buf: [u8; BLOCK],
    /// Bytes in `buf` (always below `BLOCK`: a full buffer is folded at once).
    len: u8,
}

/// Shows the hash so far, not the buffer.
impl core::fmt::Debug for Fnv64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Fnv64({:#018x})", self.finish())
    }
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
mod tests;
