//! SIL Ch10 e2e.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch10};

const FRAME_LIMIT: u64 = 400_000;

#[test]
fn chapter_10_climbs_the_nebula_tower_to_the_apex() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch10::start());
    r.play(&ch10::steps());
    ch10::check_end(&r);
}
