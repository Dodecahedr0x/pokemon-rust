/// xorshift32. The only randomness source in the game; lives inside `World`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rng(u32);

impl Rng {
    /// Seed 0 is replaced by 0x1234_5678, so those two seeds produce the same stream.
    pub const fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x1234_5678 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Gen3 `Random()`: the high half of the next state (what `Request::Random` already uses,
    /// so M3 hashes do not move).
    pub fn random(&mut self) -> u16 {
        (self.next_u32() >> 16) as u16
    }

    /// Gen3 `Random32()`: `random() | random() << 16`, low half drawn first.
    pub fn random32(&mut self) -> u32 {
        let lo = self.random() as u32;
        lo | (self.random() as u32) << 16
    }

    pub const fn state(self) -> u32 {
        self.0
    }
}

// Milestone 5 Task B.
impl Rng {
    /// The generator in state `state` (what [`Rng::state`] returned); `None` for 0, which
    /// xorshift32 never reaches.
    pub const fn from_state(state: u32) -> Option<Rng> {
        if state == 0 { None } else { Some(Self(state)) }
    }
}

#[cfg(test)]
mod tests {
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
}
