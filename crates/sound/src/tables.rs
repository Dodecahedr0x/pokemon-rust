//! `src/m4a_tables.c`.

pub const SCALE: [u8; 180] = {
    let mut t = [0u8; 180];
    // Octave o (from the top, 0xE0) and note n: (14 - o) << 4 | n, octaves of 12 notes; the
    // decomp's table holds 15 octaves (180 entries).
    let mut i = 0;
    while i < 180 {
        let (o, n) = (i / 12, i % 12);
        t[i] = (((14 - o) << 4) | n) as u8;
        i += 1;
    }
    t
};

pub const FREQ: [u32; 12] = [
    2147483648, 2275179671, 2410468894, 2553802834, 2705659852, 2866546760, 3037000500, 3217589947,
    3408917802, 3611622603, 3826380858, 4053909305,
];

pub const CGB_SCALE: [u8; 132] = {
    let mut t = [0u8; 132];
    let mut i = 0;
    while i < 132 {
        t[i] = (((i / 12) << 4) | (i % 12)) as u8;
        i += 1;
    }
    t
};

pub const CGB_FREQ: [i16; 12] = [
    -2004, -1891, -1785, -1685, -1591, -1501, -1417, -1337, -1262, -1192, -1125, -1062,
];

pub const NOISE: [u8; 60] = [
    0xD7, 0xD6, 0xD5, 0xD4, 0xC7, 0xC6, 0xC5, 0xC4, 0xB7, 0xB6, 0xB5, 0xB4, 0xA7, 0xA6, 0xA5, 0xA4,
    0x97, 0x96, 0x95, 0x94, 0x87, 0x86, 0x85, 0x84, 0x77, 0x76, 0x75, 0x74, 0x67, 0x66, 0x65, 0x64,
    0x57, 0x56, 0x55, 0x54, 0x47, 0x46, 0x45, 0x44, 0x37, 0x36, 0x35, 0x34, 0x27, 0x26, 0x25, 0x24,
    0x17, 0x16, 0x15, 0x14, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00,
];

pub const CGB3_VOL: [u8; 16] = [
    0x00, 0x00, 0x60, 0x60, 0x60, 0x60, 0x40, 0x40, 0x40, 0x40, 0x80, 0x80, 0x80, 0x80, 0x20, 0x20,
];

pub const CLOCK: [u8; 49] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 28,
    30, 32, 36, 40, 42, 44, 48, 52, 54, 56, 60, 64, 66, 68, 72, 76, 78, 80, 84, 88, 90, 92, 96,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_tables_match_the_decomp_rows() {
        assert_eq!(SCALE[0], 0xE0);
        assert_eq!(SCALE[11], 0xEB);
        assert_eq!(SCALE[12], 0xD0);
        assert_eq!(SCALE[168], 0x00);
        assert_eq!(CGB_SCALE[0], 0x00);
        assert_eq!(CGB_SCALE[131], 0xAB);
    }
}
