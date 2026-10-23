//! SIL4: Chapter 7 (Reverse-City) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 7 steps (`silice_spine::ch07`), from Chapter 6's end state
//! set directly (`ch07::start`, documented there): Route S7, Reverse-City, the lighthouse, the
//! Palais, the vault and the north gate to Route S8 (`VAR_SIL_STORY` 76). The checkpoints assert
//! the story steps, flags, the badge, the bag and the scene texts by text id, never a French
//! literal; the end asserts the Chapter 7 fallbacks were reported. The chapter's trainers are
//! the training; no hook. About 121 000 frames.

#![cfg(pkr_silice)]

mod silice_spine;

use content::consts::*;
use silice_spine::{Run, Step, ch07};

/// Every frame of the path, at most.
const FRAME_LIMIT: u64 = 160_000;

#[test]
fn chapter_7_plays_from_route_s7_to_the_seventh_badge_and_route_s8() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch07::start());
    r.play(&ch07::steps());
    ch07::check_end(&r);
}

/// Chapter 12's obligation on the lighthouse ("SIL6 Ch12 maps: review"): Thomas is shown at
/// `VAR_SIL_STORY` 122 only, written both ways by `ReverseCity_Phare_OnTransition`.
#[test]
fn the_lighthouse_shows_thomas_at_story_122_only() {
    let mut r = Run::starting_at(2_000, ch07::phare_at_122());
    r.play(&[
        Step::Settle,
        Step::Check("thomas_122", |r| {
            assert!(!r.w.flag(FLAG_SIL_HIDE_THOMAS_PHARE))
        }),
    ]);
    let mut r = Run::starting_at(2_000, ch07::phare_at_123());
    r.play(&[
        Step::Settle,
        Step::Check("thomas_123", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_THOMAS_PHARE))
        }),
    ]);
}
