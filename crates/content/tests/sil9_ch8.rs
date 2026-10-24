//! SIL4: Chapter 8 (Évalon) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 8 steps (`silice_spine::ch08`), from Chapter 7's end state
//! set directly (`ch08::start`, documented there): Route S8's palier 3, Évalon, its three
//! trials and three quests, the invasion, the Citadelle, Loulpix and Doris, and the departure
//! (`VAR_SIL_STORY` 86). Palier 1 (Route_S08_Sc01) is its own test: palier 1 and palier 3 are
//! only joined by Waterfall and Surf. The checkpoints assert the story steps, flags, the badge,
//! the bag and the scene texts by text id, never a French literal; the end asserts the Chapter 8
//! fallbacks were reported. Both leads are trained for the leaders' double, and the run heals
//! between Élise and the leaders; no hook. About 730 000 frames, most of them training.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch08};

/// Every frame of the main path, at most.
const FRAME_LIMIT: u64 = 850_000;

#[test]
fn chapter_8_plays_from_route_s8_to_the_eighth_badge_and_route_s9() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch08::start());
    r.play(&ch08::steps());
    ch08::check_end(&r);
}

/// Palier 1, where Chapter 7's e2e ends: Route_S08_Sc01 (76 → 80), the sign, the hiker, the
/// cape, the Ultra Ball and Gaston.
#[test]
fn route_s8_palier_1_plays_from_reverse_city() {
    let mut r = Run::starting_at(30_000, ch08::route_start());
    r.play(&ch08::route_steps());
}
