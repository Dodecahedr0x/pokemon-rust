use super::*;

#[test]
fn numbers_align_as_gen3() {
    let mut v = Vec::new();
    push_number(&mut v, 7, 3, true);
    assert_eq!(v, [CHAR_SPACE, CHAR_SPACE, CHAR_0 + 7]);
    v.clear();
    push_number(&mut v, 1234, 3, false);
    // Gen3 keeps the low `n` digits.
    assert_eq!(v, [CHAR_0 + 2, CHAR_0 + 3, CHAR_0 + 4]);
    v.clear();
    push_number(&mut v, 0, 2, false);
    assert_eq!(v, [CHAR_0]);
}

#[test]
fn quantity_wraps_up_down_and_clamps_left_right() {
    let mut q = Quantity::new(15);
    assert!(q.input(Buttons::DOWN));
    assert_eq!(q.value, 15);
    assert!(q.input(Buttons::UP));
    assert_eq!(q.value, 1);
    assert!(q.input(Buttons::RIGHT));
    assert_eq!(q.value, 11);
    assert!(q.input(Buttons::RIGHT));
    assert_eq!(q.value, 15);
    assert!(!q.input(Buttons::RIGHT));
    assert!(q.input(Buttons::LEFT));
    assert_eq!(q.value, 5);
    assert!(q.input(Buttons::LEFT));
    assert_eq!(q.value, 1);
    assert_eq!(quantity_max(5, 200, 1000, 99), 5);
    assert_eq!(quantity_max(999, 200, 1000, 99), 5);
    assert_eq!(quantity_max(999, 0, 0, 120), 99);
}
