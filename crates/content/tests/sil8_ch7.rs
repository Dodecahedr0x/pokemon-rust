//! SIL4: Chapter 7 (Reverse-City) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 7 steps (`silice_spine::ch07`), from Chapter 6's end state
//! set directly (`ch07::start`, documented there): Route S7, Reverse-City, the lighthouse, the
//! Palais, the vault and the north gate to Route S8 (`VAR_SIL_STORY` 76). The checkpoints assert
//! the story steps, flags, the badge, the bag and the scene texts by text id, never a French
//! literal; the end asserts the Chapter 7 fallbacks were reported. The chapter's trainers are
//! the training, with the TYPHLOSION trained for Reverse; no hook. About 266 000 frames.

#![cfg(pkr_silice)]

mod silice_spine;

use content::consts::*;
use silice_spine::{Run, Step, ch07};

/// Every frame of the path, at most.
const FRAME_LIMIT: u64 = 350_000;

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

/// G-49 (the author's ruling): Reverse is not in his gym while Thomas is at the Breach. After
/// his flight he is shown again for his rematch at 123–126, and hidden from 127 (the Breach's camp).
#[test]
fn reverse_is_in_his_gym_until_thomas_camps_at_the_breach() {
    let mut r = Run::starting_at(2_000, ch07::gym_at_126());
    r.play(&[
        Step::Settle,
        Step::Check("reverse_126", |r| {
            assert!(!r.w.flag(FLAG_SIL_HIDE_GYM07_REVERSE))
        }),
    ]);
    let mut r = Run::starting_at(2_000, ch07::gym_at_127());
    r.play(&[
        Step::Settle,
        Step::Check("reverse_127", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_GYM07_REVERSE))
        }),
    ]);
}

/// D-30: the PIERRE LIEN used from the bag on the KADABRA evolves it (an ALAKAZAM keeps its
/// moves) and is used up.
#[test]
fn the_pierre_lien_evolves_the_kadabra_from_the_bag() {
    let mut r = Run::starting_at(20_000, ch07::with_pierre_lien());
    r.play(&[
        Step::Settle,
        Step::UseOn {
            item: ITEM_SIL_PIERRE_LIEN,
            mon: 0,
        },
        Step::Check("alakazam", |r| {
            let lead = r.w.party().iter().next().unwrap();
            assert_eq!(lead.species, SPECIES_ALAKAZAM);
            assert!(lead.moves.contains(&Some(MOVE_SHOCK_WAVE)));
            assert_eq!(r.count(ITEM_SIL_PIERRE_LIEN), 0);
        }),
    ]);
}
