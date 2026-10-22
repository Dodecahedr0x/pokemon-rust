const PRIME: u64 = 0x0000_0100_0000_01b3;
const _: () = assert!(PRIME == (1 << 40) + 0x1b3);

/// `x * PRIME` (mod 2^64) with 32-bit multiplies only. Thumb (ARMv4T) has no long multiply, so
/// a plain u64 multiply is a slow `__aeabi_lmul` call per byte; PRIME = 2^40 + 0x1b3 lets the
/// product split into a shift and a 9-bit multiply.
#[inline(always)]
const fn mul_prime(x: u64) -> u64 {
    let (lo, hi) = (x as u32, (x >> 32) as u32);
    // lo * 0x1b3 in 16-bit halves, so each partial product fits in 32 bits.
    let lo_prod = ((lo & 0xFFFF) * 0x1b3) as u64 + ((((lo >> 16) * 0x1b3) as u64) << 16);
    // hi * 0x1b3 lands at bit 32: only its low 32 bits survive.
    let hi_prod = (hi.wrapping_mul(0x1b3) as u64) << 32;
    lo_prod.wrapping_add(hi_prod).wrapping_add(x << 40)
}

/// FNV-1a 64-bit. Used for World hashes compared across GBA and native.
#[derive(Clone, Debug)]
pub struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv64 {
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = mul_prime(self.0 ^ b as u64);
        }
    }

    pub fn write_u16(&mut self, v: u16) {
        self.write(&v.to_le_bytes());
    }

    pub fn write_u32(&mut self, v: u32) {
        self.write(&v.to_le_bytes());
    }

    pub const fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests;
