//! SIL2: Chapter 1 (Bois-Éther) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! From a new game, buttons only: the spine's Chapter 0 steps to Chapter 0's end state
//! (`VAR_SIL_STORY` 9 at the Bois-Éther edge), then Chapter 1's (`silice_spine::ch01`) to its
//! end, the Secteur Bêta barrier open (`VAR_SIL_STORY` 17), and the optional Brico-Bourg Labo.
//! The checkpoints assert the story steps, flags, the badge, the bag and the scene texts by
//! text id, never a French literal; the end asserts every Chapter 1 fallback was reported.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch00, ch01};

/// Every frame of the run, at most.
const FRAME_LIMIT: u64 = 200_000;

#[test]
fn chapter_1_plays_from_the_bois_ether_edge_to_the_open_barrier() {
    let mut r = Run::new(FRAME_LIMIT);
    r.play(&ch00::steps());
    r.play(&ch01::steps());
    ch01::check_end(&r);
}
