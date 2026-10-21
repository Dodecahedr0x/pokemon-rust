/// FNV-1a 64-bit. Used for World hashes compared across GBA and native.
pub struct Fnv64(u64);

impl Fnv64 {
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
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
}
