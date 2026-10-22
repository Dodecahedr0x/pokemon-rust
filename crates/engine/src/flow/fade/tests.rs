use super::*;

#[test]
fn starts_visible_and_idle() {
    let f = Fader::new();
    assert_eq!(f.level(), 0);
    assert!(f.is_done());
}

#[test]
fn to_black_takes_16_frames() {
    let mut f = Fader::new();
    f.start(Fade::ToBlack);
    assert!(!f.is_done());
    let levels: alloc::vec::Vec<u8> = (0..16).map(|_| f.update()).collect();
    assert_eq!(levels, (1..=16).collect::<alloc::vec::Vec<u8>>());
    assert!(f.is_done());
    assert_eq!(f.update(), FADE_MAX, "stays black");
}

#[test]
fn from_black_snaps_black_then_takes_16_frames() {
    let mut f = Fader::new();
    f.start(Fade::FromBlack);
    assert_eq!(f.level(), FADE_MAX);
    for i in 1..=16u8 {
        assert!(!f.is_done());
        assert_eq!(f.update(), 16 - i);
    }
    assert!(f.is_done());
    assert_eq!(f.update(), 0);
}

#[test]
fn to_black_from_partial_level_continues() {
    let mut f = Fader::new();
    f.start(Fade::FromBlack);
    for _ in 0..6 {
        f.update();
    }
    f.start(Fade::ToBlack);
    assert_eq!(f.update(), 11);
}

#[test]
fn snapshot_round_trips() {
    let mut f = Fader::new();
    f.start(Fade::ToBlack);
    f.update();
    assert_eq!(Fader::from_snapshot(f.snapshot()), f);
}

#[test]
fn hash_covers_level_and_target() {
    let h = |f: &Fader| {
        let mut h = Fnv64::new();
        f.hash(&mut h);
        h.finish()
    };
    let a = Fader::new();
    let mut b = Fader::new();
    b.start(Fade::ToBlack);
    let mut c = b;
    c.update();
    assert_ne!(h(&a), h(&b));
    assert_ne!(h(&b), h(&c));
}
