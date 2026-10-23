//! SIL2: Chapter 2 (Gris-Échec) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! Buttons only, the spine's Chapter 2 steps (`silice_spine::ch02`) to the Gamma barrier open
//! (`VAR_SIL_STORY` 28) and the Usine's crates, two ways: chained from a new game through
//! Chapters 0 and 1, and from Chapter 1's end state set directly at Route S2's west entrance
//! (`ch02::start`, documented there). The checkpoints assert the story steps, flags, the badge,
//! the bag and the scene texts by text id, never a French literal; the end asserts every
//! Chapter 2 fallback was reported. The party trains on Route S2 and takes one hooked Pokémon
//! before Duval (`ch02::TOUR_LEVEL`, `ch02::THIRD`). About 348 000 frames chained (Chapter 2
//! from frame 221 000) and 149 000 from the start state.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch00, ch01, ch02};

/// Every frame of a run, at most (the training is a third of Chapter 2).
const FRAME_LIMIT: u64 = 450_000;

#[test]
fn chapters_0_to_2_chain_from_a_new_game_to_the_open_gamma_barrier() {
    let mut r = Run::new(FRAME_LIMIT);
    r.play(&ch00::steps());
    r.play(&ch01::steps());
    ch01::check_end(&r);
    r.play(&ch02::from_ch01());
    r.play(&ch02::steps());
    ch02::check_end(&r);
}

#[test]
fn chapter_2_plays_from_its_start_state_to_the_open_gamma_barrier() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch02::start());
    r.play(&ch02::steps());
    ch02::check_end(&r);
}
