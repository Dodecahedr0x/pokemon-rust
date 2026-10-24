//! SIL3: Chapter 5 (Owlys) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 5 steps (`silice_spine::ch05`), from Chapter 4's end state
//! set directly (`ch05::start`, documented there): the rift, Owlys, the East Cave, the gym and
//! the lift to Route S6 (`VAR_SIL_STORY` 55). The engine has no Surf yet, so Route S5's islet
//! is a second start state (`ch05::ilot`, story 49) and the main path starts past the sea. The
//! checkpoints assert the story steps, flags, the badge, the bag and the scene texts by text id,
//! never a French literal; the end asserts the Chapter 5 fallbacks were reported. The silo's
//! and the cave's trainers are the training, with one `Step::Train` before the Silo Chief
//! (`ch05::CHEF_LEVEL`, documented there); no hook. About 460 000 frames for the main path,
//! 2 200 for the islet.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch05};

/// Every frame of the main path, at most.
const FRAME_LIMIT: u64 = 600_000;

#[test]
fn chapter_5_plays_from_the_rift_to_the_fifth_badge_and_route_s6() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch05::start());
    r.play(&ch05::steps());
    ch05::check_end(&r);
}

#[test]
fn chapter_5_islet_gives_the_punch_bottle_and_the_super_rod() {
    let mut r = Run::starting_at(20_000, ch05::ilot());
    r.play(&ch05::ilot_steps());
}
