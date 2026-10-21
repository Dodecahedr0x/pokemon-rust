/// Pressed buttons, one bit per key, in GBA KEYINPUT order (active-high).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Buttons(pub u16);

impl Buttons {
    pub const NONE: Self = Self(0);
    pub const A: Self = Self(1 << 0);
    pub const B: Self = Self(1 << 1);
    pub const SELECT: Self = Self(1 << 2);
    pub const START: Self = Self(1 << 3);
    pub const RIGHT: Self = Self(1 << 4);
    pub const LEFT: Self = Self(1 << 5);
    pub const UP: Self = Self(1 << 6);
    pub const DOWN: Self = Self(1 << 7);
    pub const R: Self = Self(1 << 8);
    pub const L: Self = Self(1 << 9);
    const MASK: u16 = 0x03FF;

    pub const fn from_raw(bits: u16) -> Self {
        Self(bits & Self::MASK)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for Buttons {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_match_gba_keyinput_order() {
        assert_eq!(Buttons::A.0, 1 << 0);
        assert_eq!(Buttons::RIGHT.0, 1 << 4);
        assert_eq!(Buttons::DOWN.0, 1 << 7);
        assert_eq!(Buttons::L.0, 1 << 9);
    }

    #[test]
    fn contains_and_union() {
        let b = Buttons::RIGHT | Buttons::UP;
        assert!(b.contains(Buttons::RIGHT));
        assert!(b.contains(Buttons::UP));
        assert!(!b.contains(Buttons::LEFT));
        assert_eq!(Buttons::NONE.0, 0);
    }

    #[test]
    fn from_raw_masks_unused_bits() {
        assert_eq!(Buttons::from_raw(0xFFFF).0, 0x03FF);
    }
}
