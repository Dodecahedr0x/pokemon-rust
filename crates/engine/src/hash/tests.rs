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
