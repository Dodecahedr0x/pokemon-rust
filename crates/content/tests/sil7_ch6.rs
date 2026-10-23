//! SIL3: Chapter 6 (Manga-Square) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! From Chapter 6's entry state ([`silice_spine::ch06::start`], Chapter 5's end: Chapters 4 and
//! 5 are not chained into the spine yet), buttons only: Route S6's rescue, the arrival under the
//! toriis, the Dojo's signing queue, the Bibliothèque's books and tale, the gym's samurai,
//! scrolls and Tak, Benou's modules and Adrien's promise. The checkpoints assert the story
//! steps, flags, the badge, the bag and the scene texts by text id, never a French literal; the
//! end asserts every Chapter 6 fallback on the path was reported.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch06};

/// Every frame of the run, at most (the training is most of it).
const FRAME_LIMIT: u64 = 900_000;

#[test]
fn chapter_6_plays_from_the_pilgrims_path_to_adrien_s_promise() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch06::start());
    r.play(&ch06::steps());
    ch06::check_end(&r);
}
