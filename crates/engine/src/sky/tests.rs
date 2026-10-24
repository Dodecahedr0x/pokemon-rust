use super::*;

fn fx(amount: u8, frames: u16) -> PaletteFx {
    PaletteFx {
        mode: PaletteMode::Blend,
        color: 0x7FFF,
        amount,
        current: 0,
        frames,
        t: 0,
        pulse: 0,
        keep: false,
    }
}

#[test]
fn an_override_reaches_its_amount_over_its_frames() {
    let mut p = fx(16, 4);
    let mut seen = alloc::vec::Vec::new();
    for _ in 0..6 {
        assert!(!p.tick());
        seen.push(p.current);
    }
    assert_eq!(seen, [4, 8, 12, 16, 16, 16]);
    p.amount = 0;
    p.frames = 0;
    p.t = 0;
    assert!(p.tick(), "amount 0 reached at once ends it");
}

#[test]
fn modes_recolour() {
    let c = weather::rgb(31, 0, 0);
    let p = PaletteFx {
        current: 16,
        ..fx(16, 0)
    };
    assert_eq!(p.apply(c, 16), 0x7FFF);
    assert_eq!(p.apply(c, 0), c);
    let inv = PaletteFx {
        mode: PaletteMode::Invert,
        ..p
    };
    assert_eq!(inv.apply(c, 16), weather::rgb(0, 31, 31));
    let grey = PaletteFx {
        mode: PaletteMode::Grey,
        color: 0,
        ..p
    };
    let g = grey.apply(weather::rgb(30, 0, 0), 16);
    assert_eq!(g & 31, (g >> 5) & 31, "grey: equal channels");
}

#[test]
fn overlay_runs_end() {
    let mut o = OverlayRun {
        id: 0,
        frames: 2,
        t: 0,
    };
    assert!(!o.done());
    o.t = 2;
    assert!(o.done());
}

#[test]
fn reserved_tiles_fit_below_the_emotes() {
    assert!(PARTICLE_TILES + OVERLAY_TILES == crate::objects::SKY_TILES);
    assert!(OVERLAY_TILES >= 16);
    assert_ne!(PARTICLE_BANK, OVERLAY_BANK);
}
