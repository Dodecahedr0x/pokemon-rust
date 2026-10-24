//! SIL5: Chapter 10 (the Nébula Tower) end to end, native, in the Silice flavour
//! (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 10 steps (`silice_spine::ch10`), from a constructed Chapter
//! 9 end set directly (`ch10::start`, documented there, party included): the cabin, the four
//! floors and their leaders (Mathias, Kimiko, Arthur, Revenge), the scripted save at the
//! threshold and the vault elevator up to the Apex (`VAR_SIL_STORY` 109). The checkpoints assert
//! the story steps, flags, keys and scene texts by text id; the end asserts the Chapter 10
//! fallbacks were reported.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch10};

/// Every frame of the main path, at most.
const FRAME_LIMIT: u64 = 150_000;

#[test]
fn chapter_10_climbs_the_nebula_tower_to_the_apex() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch10::start());
    r.play(&ch10::steps());
    ch10::check_end(&r);
}
