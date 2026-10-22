//! A trainer walking up to the player after seeing them (Gen3 `TrainerSeeFuncList`,
//! `trainer_see.c`), driven once per frame by the script host before objects move, as Gen3's
//! approach task runs before the sprite callbacks.
//!
//! Frames, from the frame the approach starts (the trainer script's `TrainerApproach` request):
//! - `lockfortrainer` (`IsFreezeObjectAndPlayerFinished`): the next frame on which the player
//!   and the trainer stand still, the `!` starts over the trainer, which faces the player
//!   (`TrainerExclamationMark`);
//! - when the bubble is gone ([`super::EMOTE_FRAMES`] + 1 frames later, then one more:
//!   `WaitTrainerExclamationMark` sees the effect stopped on the next frame), the trainer walks
//!   `distance - 1` cells with `walk_normal` (16 frames each, back to back), or turns to the
//!   player at once if adjacent;
//! - the frame after its last step it faces the player (`MOVEMENT_ACTION_FACE_PLAYER`);
//! - the next frame its movement type becomes `Face` that way (`SetTrainerMovementType`) and the
//!   player turns to it (`PlayerFaceApproachingTrainer`);
//! - two frames later the script goes on (`WaitPlayerFaceApproachingTrainer`, then
//!   `ScriptContext_Enable` takes effect on the next frame).
//!
//! Buried trainers (Gen3 `RevealBuriedTrainer`, the ash puff) are not modelled: they turn to the
//! player and walk like the others.

use super::{Objects, direction_to};
use crate::data::{Dir, Emote, PLAYER};
use crate::hash::Fnv64;
use crate::script::{Speed, Step};

/// Most `walk_normal` steps of an approach (Gen3 trainer sight is at most 7 in Emerald's maps);
/// a longer sight walks this many.
pub const APPROACH_WALKS_MAX: u8 = 15;

static WALKS: [[Step; APPROACH_WALKS_MAX as usize]; 4] = [
    [Step::Walk(Dir::Down, Speed::Normal); APPROACH_WALKS_MAX as usize],
    [Step::Walk(Dir::Up, Speed::Normal); APPROACH_WALKS_MAX as usize],
    [Step::Walk(Dir::Left, Speed::Normal); APPROACH_WALKS_MAX as usize],
    [Step::Walk(Dir::Right, Speed::Normal); APPROACH_WALKS_MAX as usize],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// Seen; waiting for the trainer script's `TrainerApproach`.
    Pending,
    /// `lockfortrainer`: until the player and the trainer stand still.
    Lock,
    /// The `!` shows.
    Exclamation,
    /// Walking up.
    Walk,
    /// Facing the player; next frame the player turns.
    Faced,
    /// Frames left before the script goes on.
    Settle(u8),
}

/// The approaching trainer (Gen3 `gApproachingTrainers[0]` and its task).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Approach {
    obj: u8,
    /// `walk_normal` steps left to start (Gen3 `tTrainerRange`).
    walks: u8,
    phase: Phase,
}

/// What [`Objects::approach_step`] did this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ApproachStep {
    Waiting,
    /// The trainer `obj` stopped at `pos` facing `dir`: its movement type is now `Face(dir)`;
    /// the host also updates its template (Gen3 `OverrideTemplateCoordsForObjectEvent`).
    Stopped {
        obj: u8,
        dir: Dir,
        pos: (i16, i16),
    },
    Done,
}

impl Objects {
    /// A trainer saw the player `distance` cells away (Gen3 `CheckTrainer`): the approach waits
    /// for its script's `TrainerApproach`. Everything stops at once (`lockfortrainer`).
    pub(crate) fn sight_pending(&mut self, obj: u8, distance: u8) {
        self.freeze_all();
        self.approach = Some(Approach {
            obj,
            walks: distance.saturating_sub(1).min(APPROACH_WALKS_MAX),
            phase: Phase::Pending,
        });
    }

    /// Starts the approach pending for `obj`, if any (Gen3 `lockfortrainer`, `DoTrainerApproach`).
    pub(crate) fn approach_begin(&mut self, obj: u8) -> bool {
        match &mut self.approach {
            Some(a) if a.obj == obj && a.phase == Phase::Pending => {
                a.phase = Phase::Lock;
                self.freeze_all();
                true
            }
            _ => false,
        }
    }

    /// Whether an approach is pending or running.
    pub fn approaching(&self) -> Option<u8> {
        self.approach.map(|a| a.obj)
    }

    /// Drops an approach that will not run (the script ended, a map load).
    pub(crate) fn cancel_approach(&mut self) {
        self.approach = None;
    }

    /// One frame of the approach (Gen3 `Task_RunTrainerSeeFuncList`).
    pub(crate) fn approach_step(&mut self) -> ApproachStep {
        let Some(mut a) = self.approach else {
            return ApproachStep::Done;
        };
        let Some(t) = self.get(a.obj) else {
            // Gen3: the task ends when the trainer object is gone.
            self.approach = None;
            return ApproachStep::Done;
        };
        let player = self.player().pos();
        let (tpos, settled, tmap) = (t.pos(), t.is_settled(), t.map);
        let mut out = ApproachStep::Waiting;
        match a.phase {
            Phase::Pending => {}
            Phase::Lock => {
                if settled && self.is_settled(PLAYER) {
                    self.start_emote(a.obj, tmap, Emote::Exclamation);
                    self.face(a.obj, direction_to(tpos, player));
                    a.phase = Phase::Exclamation;
                }
            }
            Phase::Exclamation => {
                if !self.emote_active(a.obj) {
                    let facing = self.get(a.obj).map_or(Dir::Down, |o| o.facing);
                    if a.walks > 0 {
                        let steps = &WALKS[facing as usize][..a.walks as usize];
                        self.start_moves(a.obj, steps);
                        a.walks = 0;
                        a.phase = Phase::Walk;
                    } else {
                        self.face(a.obj, direction_to(tpos, player));
                        a.phase = Phase::Faced;
                    }
                }
            }
            Phase::Walk => {
                if self.moves_done(a.obj) {
                    self.face(a.obj, direction_to(tpos, player));
                    a.phase = Phase::Faced;
                }
            }
            Phase::Faced => {
                let dir = self.get(a.obj).map_or(Dir::Down, |o| o.facing);
                self.set_movement_type(a.obj, crate::data::MovementType::Face(dir));
                self.face(PLAYER, dir.opposite());
                a.phase = Phase::Settle(2);
                out = ApproachStep::Stopped {
                    obj: a.obj,
                    dir,
                    pos: tpos,
                };
            }
            Phase::Settle(n) => {
                if n > 1 {
                    a.phase = Phase::Settle(n - 1);
                } else {
                    self.approach = None;
                    return ApproachStep::Done;
                }
            }
        }
        self.approach = Some(a);
        out
    }

    pub(super) fn hash_approach(&self, h: &mut Fnv64) {
        if let Some(a) = self.approach {
            let (p, n) = match a.phase {
                Phase::Pending => (0, 0),
                Phase::Lock => (1, 0),
                Phase::Exclamation => (2, 0),
                Phase::Walk => (3, 0),
                Phase::Faced => (4, 0),
                Phase::Settle(n) => (5, n),
            };
            h.write(&[0xA7, a.obj, a.walks, p, n]);
        }
    }
}
