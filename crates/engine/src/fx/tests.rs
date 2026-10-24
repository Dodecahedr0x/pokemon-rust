use super::*;

#[test]
fn blend_and_light() {
    assert_eq!(blend(0, 0x7FFF, 16), 0x7FFF);
    assert_eq!(blend(0x7FFF, 0, 0), 0x7FFF);
    assert_eq!(blend(0, 0x001F, 8), 0x000F);
    assert!(light(0x7FFF));
    assert!(!light(0));
    assert!(!light(0x001F));
}

#[test]
fn a_tint_steps_to_its_amount_and_ends_once_cleared() {
    let mut fx = ScreenFx {
        tint: Some(Tint {
            banks: 1,
            obj_banks: 0,
            color: 0x7FFF,
            amount: 8,
            current: 0,
            frames: 4,
            t: 0,
        }),
        ..ScreenFx::default()
    };
    let mut f = Frame::default();
    for _ in 0..4 {
        apply(&mut fx, &mut f);
    }
    assert_eq!(fx.tint.unwrap().current, 8);
    fx.tint = Some(Tint {
        amount: 0,
        frames: 2,
        t: 0,
        ..fx.tint.unwrap()
    });
    for _ in 0..2 {
        apply(&mut fx, &mut f);
    }
    assert_eq!(fx.tint, None);
    assert!(!fx.is_active());
}
