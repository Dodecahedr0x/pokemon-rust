use super::*;

#[test]
fn xorshift32_reference_value() {
    let mut r = Rng::new(1);
    assert_eq!(r.next_u32(), 270369);
}

#[test]
fn random_is_the_high_half_and_random32_draws_low_first() {
    let mut a = Rng::new(7);
    let mut b = Rng::new(7);
    assert_eq!(a.random(), (b.next_u32() >> 16) as u16);
    let (lo, hi) = (b.next_u32() >> 16, b.next_u32() >> 16);
    assert_eq!(a.random32(), lo | hi << 16);
    assert_eq!(a, b);
}

#[test]
fn zero_seed_is_replaced() {
    let mut r = Rng::new(0);
    assert_ne!(r.next_u32(), 0);
}

#[test]
fn from_state_resumes_the_stream_and_rejects_zero() {
    let mut a = Rng::new(9);
    a.next_u32();
    let mut b = Rng::from_state(a.state()).unwrap();
    assert_eq!(a.next_u32(), b.next_u32());
    assert_eq!(Rng::from_state(0), None);
}

#[test]
fn same_seed_same_sequence() {
    let (mut a, mut b) = (Rng::new(42), Rng::new(42));
    for _ in 0..100 {
        assert_eq!(a.next_u32(), b.next_u32());
    }
}
