use super::*;

#[test]
fn snapshot_round_trips() {
    let i = Input::new(Buttons::A, Buttons::A | Buttons::UP);
    assert_eq!(Input::from_snapshot(i.snapshot()), i);
}

#[test]
fn pressed_is_the_rising_edge() {
    let mut i = Input::default();
    i.update(Buttons::A);
    assert!(i.is_pressed(Buttons::A) && i.is_held(Buttons::A));
    i.update(Buttons::A | Buttons::B);
    assert!(!i.is_pressed(Buttons::A) && i.is_held(Buttons::A));
    assert!(i.is_pressed(Buttons::B));
    i.update(Buttons::NONE);
    assert_eq!(i, Input::default());
    i.update(Buttons::A);
    assert!(i.is_pressed(Buttons::A), "re-press after release");
}

#[test]
fn new_from_prev_and_cur() {
    let i = Input::new(Buttons::A | Buttons::UP, Buttons::UP | Buttons::B);
    assert_eq!(i.held, Buttons::UP | Buttons::B);
    assert_eq!(i.pressed, Buttons::B);
}

#[test]
fn dpad_priority_up_down_left_right() {
    let d = |b| Input::new(Buttons::NONE, b).dpad();
    assert_eq!(d(Buttons::NONE), None);
    assert_eq!(d(Buttons::RIGHT), Some(Dir::Right));
    assert_eq!(d(Buttons::RIGHT | Buttons::LEFT), Some(Dir::Left));
    assert_eq!(d(Buttons::LEFT | Buttons::DOWN), Some(Dir::Down));
    assert_eq!(
        d(Buttons::DOWN | Buttons::UP | Buttons::RIGHT),
        Some(Dir::Up)
    );
}

#[test]
fn hash_covers_both_fields() {
    let h = |i: Input| {
        let mut h = Fnv64::new();
        i.hash(&mut h);
        h.finish()
    };
    let a = Input::new(Buttons::NONE, Buttons::A);
    let b = Input::new(Buttons::A, Buttons::A);
    assert_ne!(h(a), h(b));
}
