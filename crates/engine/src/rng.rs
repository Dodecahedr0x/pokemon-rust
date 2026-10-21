/// xorshift32. The only randomness source in the game; lives inside `World`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rng(u32);

impl Rng {
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

    pub const fn state(self) -> u32 {
        self.0
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
    fn zero_seed_is_replaced() {
        let mut r = Rng::new(0);
        assert_ne!(r.next_u32(), 0);
    }

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
