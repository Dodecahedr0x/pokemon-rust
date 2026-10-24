//! SIL5: Chapter 11 (the Apex, the Zenith, the endings) end to end, native, in the Silice
//! flavour (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 11 steps (`silice_spine::ch11`), from Chapter 10's end state
//! set directly (`ch11::start`): the Apex's scenes and Poussin, the console's choices answered
//! No, Romain's kit, the crystal tube, the Zenith's last choice answered No and Cocorico, the
//! rupture and the night, the descent, Nébula's waking and the credits, the scripted save at
//! home and the epilogue (`VAR_SIL_STORY` 119). The alternate ending (Elysium, canonical) is
//! played twice, from the console and from the throne, to the reload of the last save.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch11};

/// Every frame of the main path, at most.
const FRAME_LIMIT: u64 = 150_000;

#[test]
fn chapter_11_plays_from_the_apex_to_the_epilogue() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch11::start());
    r.play(&ch11::steps());
    ch11::check_end(&r);
}

#[test]
fn the_alternate_ending_from_the_console_reloads_the_last_save() {
    let mut r = Run::starting_at(100_000, ch11::elysium_apex_start());
    r.play(&ch11::elysium_apex_steps());
}

#[test]
fn the_alternate_ending_from_the_throne_reloads_the_last_save() {
    let mut r = Run::starting_at(100_000, ch11::elysium_zenith_start());
    r.play(&ch11::elysium_zenith_steps());
}
