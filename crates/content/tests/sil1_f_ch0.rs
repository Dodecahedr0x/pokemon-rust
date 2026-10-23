//! SIL1 Task F: Chapter 0 end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! From a new game, buttons only, the spine's Chapter 0 steps (`silice_spine::ch00`): the bar,
//! the night chase, the house, the morning and the starter, the hidden Potion, the Labo's
//! brambles (cut early by a hook: `ch00::labo_brambles_probe`), Route S1, and `VAR_SIL_STORY` 9
//! at the Bois-Éther edge. The checkpoints assert the story steps, flags, vars, bag and party,
//! and the scene texts by text id (a `GameData` wrapper records every `text(id)` the world
//! prints), never a French literal.
//!
//! The engine directly (`engine::World` over `content::Game`); the runner and the step
//! vocabulary are `silice_spine` (shared with `sil2_ch1.rs` and the explorer's spine).

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch00};

/// Every frame of the run, at most (about 14 minutes of play).
const FRAME_LIMIT: u64 = 50_000;

#[test]
fn chapter_0_plays_from_a_new_game_to_the_bois_ether_edge() {
    assert!(!content::scripts::TRAINER_SCRIPTS.is_empty());
    let mut r = Run::new(FRAME_LIMIT);
    r.play(&ch00::steps_with_probes());
    ch00::check_end(&r);
    eprintln!("frames {}", r.frame);
}
