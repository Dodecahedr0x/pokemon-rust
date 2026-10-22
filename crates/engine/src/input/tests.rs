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
