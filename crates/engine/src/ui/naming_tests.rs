use super::*;

#[test]
fn the_keyboard_is_gen3s_character_set_and_the_lengths_are_gen3s() {
    assert_eq!(max_chars(NamingKind::Player), 7);
    assert_eq!(max_chars(NamingKind::Rival), 7);
    assert_eq!(max_chars(NamingKind::Mon(0)), 10);
    assert_eq!(max_chars(NamingKind::Box(0)), 8);
    // Upper page, the cells the boot driver uses (`vanilla::boot::key_cell`).
    assert_eq!(key(PAGE_UPPER, 0, 0), Some(0xBB)); // A
    assert_eq!(key(PAGE_UPPER, 6, 2), Some(0xCD)); // S
    assert_eq!(key(PAGE_UPPER, 7, 0), Some(0xAD)); // .
    assert_eq!(key(PAGE_UPPER, 6, 1), Some(0x00)); // space
    assert_eq!(key(PAGE_UPPER, 8, 0), None);
    assert_eq!(key(PAGE_LOWER, 0, 0), Some(0xD5)); // a
    assert_eq!(key(PAGE_LOWER, 6, 3), Some(0xEE)); // z
    assert_eq!(key(PAGE_SYMBOLS, 0, 0), Some(0xA1)); // 0
    assert_eq!(key(PAGE_SYMBOLS, 0, 2), Some(0xAB)); // !
    assert_eq!(key(PAGE_SYMBOLS, 2, 2), Some(0xB5)); // ♂
    assert_eq!(key(PAGE_SYMBOLS, 5, 2), Some(0xAE)); // -
    assert_eq!(key(PAGE_SYMBOLS, 0, 3), Some(0xB0)); // …
    assert_eq!(key(PAGE_SYMBOLS, 4, 3), Some(0xB4)); // '
    assert_eq!(key(PAGE_SYMBOLS, 6, 0), None);
}

#[test]
fn a_full_player_name_is_seven_characters() {
    let mut s = NamingScreen::new(NamingKind::Player);
    for _ in 0..6 {
        assert!(!s.add_char(0xBB));
    }
    assert!(s.add_char(0xBB));
    assert_eq!(&s.text[..7], &[0xBB; 7]);
    assert_eq!(s.text[7], EOS);
    // The last cell is overwritten; an eighth character does not appear.
    assert!(s.add_char(0xBC));
    assert_eq!(s.text[6], 0xBC);
    assert_eq!(s.text[7], EOS);
}
