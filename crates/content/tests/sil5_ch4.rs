//! SIL3: Chapter 4 (Port-Ressac) end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! From Chapter 4's entry state ([`silice_spine::ch04::start`], Chapter 3's end: Chapters 2 and
//! 3 are not chained into the spine yet), buttons only: the Tunnel de Verre, the port, Crow's
//! lookout, the cargo's log, the quay cleaning, the Zenith-Grog and its hold, Samuel, Gwada, and
//! Julien's call. The checkpoints assert the story steps, flags, the badge, the bag and the scene
//! texts by text id, never a French literal; the end asserts every Chapter 4 fallback was
//! reported.

#![cfg(pkr_silice)]

mod silice_spine;

use silice_spine::{Run, ch04};

/// Every frame of the run, at most (the training is most of it).
const FRAME_LIMIT: u64 = 600_000;

#[test]
fn chapter_4_plays_from_the_glass_tunnel_to_julien_s_call() {
    let mut r = Run::starting_at(FRAME_LIMIT, ch04::start());
    r.play(&ch04::steps());
    ch04::check_end(&r);
}

/// Samuel's defeat branch (R-7): a first loss, then a lost rematch, each whiting out to the
/// Taverne's door, where the port's on-frame entry plays the branch once.
#[test]
fn losing_to_samuel_plays_the_defeat_branch_at_the_taverne_door() {
    let mut r = Run::starting_at(DEFEAT_LIMIT, ch04::samuel_first_loss());
    r.allow_losses = true;
    r.play(&ch04::samuel_first_loss_steps());
    let mut r = Run::starting_at(DEFEAT_LIMIT, ch04::samuel_rematch());
    r.allow_losses = true;
    r.play(&ch04::samuel_rematch_steps());
}

/// Every frame of a defeat run, at most.
const DEFEAT_LIMIT: u64 = 60_000;
