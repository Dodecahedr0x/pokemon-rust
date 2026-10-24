/// Pressed buttons, one bit per key, in GBA KEYINPUT order (active-high).
#[derive(
    Clone, Copy, PartialEq, Eq, Debug, Default, Hash, serde::Serialize, serde::Deserialize,
)]
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

/// How the field reads the d-pad (a game rule Gen3 does not have: Silice's inverted controls,
/// `SpecialIds::var_input_map`). Only the player's walking and turning in the field are
/// mapped; menus, messages and battles read the buttons as they are.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum InputMap {
    #[default]
    Normal,
    /// Up and down swapped, left and right swapped.
    Inverted,
    /// Left and right swapped.
    MirrorHorizontal,
    /// Up and down swapped.
    MirrorVertical,
}

impl InputMap {
    /// The value kept in `SpecialIds::var_input_map` (0 is [`InputMap::Normal`]).
    pub const fn code(self) -> u16 {
        self as u16
    }

    /// The map a var value stands for (an unknown value is [`InputMap::Normal`]).
    pub const fn from_code(v: u16) -> InputMap {
        match v {
            1 => InputMap::Inverted,
            2 => InputMap::MirrorHorizontal,
            3 => InputMap::MirrorVertical,
            _ => InputMap::Normal,
        }
    }

    /// The buttons the field reads when `b` is held. Each mapping is its own inverse, so this
    /// also gives the buttons to hold for a wanted direction.
    pub const fn apply(self, b: Buttons) -> Buttons {
        let (h, v) = match self {
            InputMap::Normal => (false, false),
            InputMap::Inverted => (true, true),
            InputMap::MirrorHorizontal => (true, false),
            InputMap::MirrorVertical => (false, true),
        };
        const fn swap(b: u16, on: bool, x: u16, y: u16) -> u16 {
            let (hx, hy) = (b & x != 0, b & y != 0);
            let (hx, hy) = if on { (hy, hx) } else { (hx, hy) };
            (if hx { x } else { 0 }) | (if hy { y } else { 0 })
        }
        let dpad = Buttons::UP.0 | Buttons::DOWN.0 | Buttons::LEFT.0 | Buttons::RIGHT.0;
        let out = (b.0 & !dpad)
            | swap(b.0, h, Buttons::LEFT.0, Buttons::RIGHT.0)
            | swap(b.0, v, Buttons::UP.0, Buttons::DOWN.0);
        Buttons(out)
    }

    /// The d-pad button to hold for the field to walk or turn towards `d`.
    pub const fn key_for(self, d: crate::data::Dir) -> Buttons {
        let b = match d {
            crate::data::Dir::Up => Buttons::UP,
            crate::data::Dir::Down => Buttons::DOWN,
            crate::data::Dir::Left => Buttons::LEFT,
            crate::data::Dir::Right => Buttons::RIGHT,
        };
        self.apply(b)
    }
}

#[cfg(test)]
mod tests;
