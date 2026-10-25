//! Options menu (Gen3 `option_menu.c`): the settings the menu edits, and that each one
//! changes play.

use super::*;
use crate::battle::test_data::{NORMAL, WATER};
use crate::options::{BattleStyle, ButtonMode, Sound, TextSpeed};
use crate::ui::ScreenKind;
use crate::video::Frame;

fn open_options(w: &mut World) {
    let mut f = Frame::default();
    crate::world::screens::open(&mut w.s, &Game, ScreenKind::Options, false, &mut f);
    // Fade out, draw, fade in. Input is ignored until the screen is running.
    for _ in 0..40 {
        w.step(&Game, Buttons::NONE);
    }
    assert!(w.screens_open() >= 1, "options menu stayed open");
}

fn close_options(w: &mut World) {
    tap(w, Buttons::B, 2);
    for _ in 0..80 {
        if w.screens_open() == 0 {
            return;
        }
        w.step(&Game, Buttons::NONE);
    }
    panic!("options menu did not close");
}

/// Move `downs` rows, then press right `rights` times, and leave.
fn edit(w: &mut World, downs: u32, rights: u32) {
    open_options(w);
    for _ in 0..downs {
        tap(w, Buttons::DOWN, 2);
    }
    for _ in 0..rights {
        tap(w, Buttons::RIGHT, 2);
    }
    close_options(w);
}

fn message_frames(w: &mut World) -> u32 {
    w.set_player_name(&[1, 2, 3, 1, 2, 3, 0xFF]);
    start(w, |ctx| {
        Box::pin(async move {
            ctx.message(PLAYER_TEXT).await;
        })
    });
    let mut started_at = None;
    for n in 1..2000 {
        run(w, Buttons::NONE, 1);
        match (started_at, w.text().is_printing()) {
            (None, true) => started_at = Some(n),
            (Some(s), false) => return n - s,
            _ => {}
        }
    }
    panic!("never printed");
}

/// The menu's first row is text speed. Right moves MID to FAST; B keeps it.
#[test]
fn options_menu_changes_text_speed_and_reads_it_back() {
    let mut w = started();
    assert_eq!(w.options().text_speed, TextSpeed::Mid);
    open_options(&mut w);
    tap(&mut w, Buttons::RIGHT, 2);
    close_options(&mut w);
    assert_eq!(w.options().text_speed, TextSpeed::Fast);
}

#[test]
fn fast_text_from_the_menu_prints_faster_than_mid() {
    let mut mid = started();
    let mid_frames = message_frames(&mut mid);
    let mut fast = started();
    edit(&mut fast, 0, 1);
    assert_eq!(fast.options().text_speed, TextSpeed::Fast);
    let fast_frames = message_frames(&mut fast);
    assert!(
        fast_frames < mid_frames,
        "fast {fast_frames} mid {mid_frames}"
    );
}

#[test]
fn options_menu_changes_the_other_settings_and_they_apply() {
    let mut scene = started();
    edit(&mut scene, 1, 1);
    assert!(!scene.options().battle_scene);

    let mut style = started();
    edit(&mut style, 2, 1);
    assert_eq!(style.options().battle_style, BattleStyle::Set);

    let mut sound = started();
    edit(&mut sound, 3, 1);
    assert_eq!(sound.options().sound, Sound::Stereo);
    assert_eq!(sound.sound(), Sound::Stereo, "the mixer mode is applied");

    let mut buttons = started();
    edit(&mut buttons, 4, 2);
    assert_eq!(buttons.options().button_mode, ButtonMode::LEqualsA);
    buttons.step(&Game, Buttons::L);
    assert!(
        buttons.s.input.is_pressed(Buttons::A),
        "L=A makes L press A"
    );

    // A battle started afterwards reads the options.
    start(&mut scene, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 20).await;
            ctx.battle_wild(NORMAL, 5, false).await;
        })
    });
    for _ in 0..160 {
        if scene.battle().is_some() {
            break;
        }
        run(&mut scene, Buttons::NONE, 1);
    }
    let (_, battle, view) = scene.battle().expect("battle");
    assert!(!view.animations(), "battle scene off skips move animations");
    assert_eq!(battle.battle_style(), BattleStyle::Shift);

    start(&mut style, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 20).await;
            ctx.battle_wild(NORMAL, 5, false).await;
        })
    });
    for _ in 0..160 {
        if style.battle().is_some() {
            break;
        }
        run(&mut style, Buttons::NONE, 1);
    }
    let (_, battle, view) = style.battle().expect("battle");
    assert!(view.animations());
    assert_eq!(battle.battle_style(), BattleStyle::Set);
}
