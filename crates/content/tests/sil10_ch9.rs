//! SIL4: Chapter 9 (the Prise d'Eau and Nébula) end to end, native, in the Silice flavour
//! (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 9 steps (`silice_spine::ch09`), from Chapter 8's end state
//! set directly (`ch09::start`, documented there), where `sil9_ch8` ends: Route S9 (Benou's and
//! Julien's calls, Surf across the lake), the Station de Pompage's pumps, Dive into the conduit,
//! Nébula (the Gargote, the Centre, the Comptoir, the diversion), the Bassin's dive under the
//! Tower, B1F and the hall's security corridor, and the crystal elevator into Chapter 10's
//! cabin (`VAR_SIL_STORY` 99), whose first climb (Chapter 10's) leaves the player on 1F at 100.
//! The checkpoints assert the story steps, flags, trainers, the bag
//! and the scene texts by text id, never a French literal; the end asserts the Chapter 9
//! fallbacks were reported. No hook, no training.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch09};

/// Every frame of the main path, at most.
const FRAME_LIMIT: u64 = 200_000;

#[test]
fn chapter_9_plays_from_evalon_to_the_tower_elevator() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch09::start());
    r.play(&ch09::steps());
    ch09::check_end(&r);
}
