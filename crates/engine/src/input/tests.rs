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

#[test]
fn input_maps_swap_the_dpad_only_and_are_their_own_inverse() {
    use crate::data::Dir;
    let m = InputMap::Inverted;
    assert_eq!(
        m.apply(Buttons::UP | Buttons::A),
        Buttons::DOWN | Buttons::A
    );
    assert_eq!(m.apply(Buttons::LEFT), Buttons::RIGHT);
    assert_eq!(m.key_for(Dir::Up), Buttons::DOWN);
    assert_eq!(InputMap::MirrorHorizontal.key_for(Dir::Up), Buttons::UP);
    assert_eq!(
        InputMap::MirrorHorizontal.key_for(Dir::Left),
        Buttons::RIGHT
    );
    assert_eq!(InputMap::MirrorVertical.key_for(Dir::Left), Buttons::LEFT);
    for map in [
        InputMap::Normal,
        InputMap::Inverted,
        InputMap::MirrorHorizontal,
        InputMap::MirrorVertical,
    ] {
        assert_eq!(InputMap::from_code(map.code()), map);
        for b in 0..0x400u16 {
            assert_eq!(map.apply(map.apply(Buttons(b))), Buttons(b));
        }
    }
    assert_eq!(InputMap::from_code(0), InputMap::Normal);
}
