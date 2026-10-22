//! Host tests for the GBA shell's pure save logic (`crates/game-gba/src/save_logic.rs`, which
//! has no agb imports and is included here as `parity.rs` includes `probe_scene.rs`).

#[path = "../../game-gba/src/save_logic.rs"]
mod save_logic;

use engine::Buttons;
use engine::checkpoint::{BuildHash, SCHEMA, SaveMeta};
use engine::data::MapId;
use engine::debug::boot;
use save_logic::{
    BootChoice, PendingSave, SAVE_WAIT_FRAMES, boot_choice, combo_pressed, scanlines,
};

const LR: Buttons = Buttons(Buttons::L.0 | Buttons::R.0);
const LRS: Buttons = Buttons(Buttons::L.0 | Buttons::R.0 | Buttons::SELECT.0);

fn meta(schema: u16) -> SaveMeta {
    SaveMeta {
        schema,
        build: BuildHash::default(),
        frame: 42,
        map: MapId(0),
    }
}

#[test]
fn combo_fires_on_the_select_edge_with_l_and_r_held() {
    assert!(combo_pressed(LR, LRS));
    assert!(combo_pressed(Buttons::NONE, LRS));
    // Extra keys do not matter.
    assert!(combo_pressed(LR, LRS | Buttons::A));
}

#[test]
fn held_select_does_not_repeat() {
    assert!(!combo_pressed(LRS, LRS));
    // SELECT held first, then L+R: no new SELECT press.
    assert!(!combo_pressed(Buttons::SELECT, LRS));
}

#[test]
fn combo_needs_both_shoulders() {
    assert!(!combo_pressed(Buttons::L, Buttons::L | Buttons::SELECT));
    assert!(!combo_pressed(Buttons::R, Buttons::R | Buttons::SELECT));
    assert!(!combo_pressed(LR, LR));
    assert!(!combo_pressed(Buttons::NONE, Buttons::SELECT));
}

#[test]
fn boot_choice_branches() {
    assert_eq!(
        boot_choice(None, Buttons::NONE),
        BootChoice::NewGame(boot::NEW_GAME)
    );
    assert_eq!(
        boot_choice(Some(meta(SCHEMA)), Buttons::NONE),
        BootChoice::Restore
    );
    assert_eq!(
        boot_choice(Some(meta(SCHEMA)), Buttons::B),
        BootChoice::NewGame(boot::SKIPPED)
    );
    // B is checked first.
    assert_eq!(
        boot_choice(None, Buttons::B | Buttons::A),
        BootChoice::NewGame(boot::SKIPPED)
    );
    assert_eq!(
        boot_choice(Some(meta(SCHEMA.wrapping_add(1))), Buttons::NONE),
        BootChoice::NewGame(boot::REJECTED)
    );
    // Other keys held at boot do not skip.
    assert_eq!(
        boot_choice(Some(meta(SCHEMA)), Buttons::A | Buttons::START),
        BootChoice::Restore
    );
}

#[test]
fn pending_save_times_out_after_the_wait() {
    assert_eq!(SAVE_WAIT_FRAMES, 600);
    let mut p = PendingSave::new();
    assert_eq!(p.frames_left, SAVE_WAIT_FRAMES);
    for _ in 0..SAVE_WAIT_FRAMES - 1 {
        assert!(!p.tick(), "still waiting");
    }
    assert!(p.tick(), "the 600th uncapturable frame times out");
    assert_eq!(p.frames_left, 0);
    // Stays expired.
    assert!(p.tick());
}

#[test]
fn scanlines_wrap_mod_one_frame() {
    assert_eq!(scanlines(160, 169), 9);
    assert_eq!(scanlines(220, 3), 11);
    assert_eq!(scanlines(5, 5), 0);
    assert_eq!(scanlines(0, 227), 227);
    assert!(PendingSave::default() == PendingSave::new());
}
