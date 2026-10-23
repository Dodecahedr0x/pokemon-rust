//! SIL4: Chapter 3 (Viber-City) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 3 steps (`silice_spine::ch03`) from Route S3's south edge
//! (`VAR_SIL_STORY` 28) to the east barrier open (36), with the optional Casino played, two
//! ways: chained from a new game through Chapters 0 to 2, and from Chapter 2's end state set
//! directly (`ch03::start`, documented there). The checkpoints assert the story steps, flags,
//! the badge, the bag and the scene texts by text id, never a French literal; the end asserts
//! every Chapter 3 fallback on the path was reported. The party is Chapter 2's, trained in
//! Viber-City's garden (`ch03::STATION_LEVEL`, `ch03::GYM_LEVEL`), plus the Casino's Porygon.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch00, ch01, ch02, ch03};

/// Every frame of a run, at most (the chained run ends at about frame 552 000, the start state
/// at about 310 000).
const FRAME_LIMIT: u64 = 700_000;

#[test]
fn chapters_0_to_3_chain_from_a_new_game_to_the_open_east_barrier() {
    let mut r = Run::new(FRAME_LIMIT);
    r.play(&ch00::steps());
    r.play(&ch01::steps());
    r.play(&ch02::from_ch01());
    r.play(&ch02::steps());
    ch02::check_end(&r);
    r.play(&ch03::from_ch02());
    r.play(&ch03::steps());
    ch03::check_end(&r);
    eprintln!("frames {}", r.frame);
}

#[test]
fn chapter_3_plays_from_its_start_state_to_the_open_east_barrier() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch03::start());
    r.play(&ch03::steps());
    ch03::check_end(&r);
    eprintln!("frames {}", r.frame);
}
