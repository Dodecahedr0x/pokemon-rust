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
