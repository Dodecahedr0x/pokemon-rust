//! Trainer sight (owned by Milestone 8 Task I): checked first on every field-input frame and
//! first when a player step finishes (before map `on_frame` scripts and step triggers, as Gen3
//! `ProcessPlayerFieldInput`, field_control_avatar.c:147), a trainer whose line of sight holds
//! the player walks up and starts its script (`CheckForTrainersWantingBattle`, `trainer_see.c`).
//!
//! Gen3 rules kept:
//! - only spawned objects of the current map whose template's `trainer_type` is
//!   [`TRAINER_TYPE_NORMAL`] (looks along its facing) or [`TRAINER_TYPE_BURIED`] (looks all four
//!   ways, south, north, west, east) are checked; `TRAINER_TYPE_SEE_ALL_DIRECTIONS` (2) is
//!   skipped by Gen3's loop and so here (no Emerald map uses it);
//! - a trainer whose flag ([`GameData::trainer_flag_of`] of its script) is set, or whose script
//!   has none, does not look;
//! - the player's cell (the destination of a step in progress, `PlayerGetDestCoords`) is within
//!   `trainer_sight` cells straight ahead; every cell between is free for the trainer ignoring its
//!   wander range (`GetCollisionFlagsAtCoords`: collision bits, elevation, objects; water is not
//!   in Gen3's collision, so trainers see across it), and the player's cell gives exactly an
//!   object collision (`CheckPathBetweenTrainerAndPlayer`);
//! - a double trainer is skipped when the player has fewer than two usable mons
//!   (`trainer_see.c:216`, decision Q3);
//! - with one trainer found and fewer than two usable mons the scan stops; with two usable mons
//!   it looks for a second trainer. Gen3 then starts a two-trainer battle; here the first (in
//!   template order) battles and the second is reported `Degraded("double sight")`: it sees the
//!   player again after the battle.
//!
//! Cost: one pass over the map's object templates comparing a byte, then geometry for the
//! trainers only; the flag lookup comes last.

use super::{Event, State, World};
use crate::data::{Dir, GameData, ObjectEvent, ScriptId};
use crate::objects::{Collision, Terrain};
use crate::script::Report;

/// Gen3 `TRAINER_TYPE_NORMAL` (the `ObjectEvent::trainer_type` encoding).
pub const TRAINER_TYPE_NORMAL: u8 = 1;
/// Gen3 `TRAINER_TYPE_BURIED`: looks all four ways.
pub const TRAINER_TYPE_BURIED: u8 = 3;

/// Gen3 `sDirectionalApproachDistanceFuncs` order (`DIR_SOUTH` … `DIR_EAST`).
const ALL_WAYS: [Dir; 4] = [Dir::Down, Dir::Up, Dir::Left, Dir::Right];

/// A trainer that saw the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Sighting {
    /// Local id of the trainer object.
    pub obj: u8,
    /// Its object script.
    pub script: ScriptId,
    /// Cells from the trainer to the player (Gen3 `approachDistance`, at least 1).
    pub distance: u8,
    /// A second trainer also saw the player (Gen3's two-trainer battle, not modelled).
    pub second: bool,
}

/// Gen3 `GetMonsStateToDoubles_2 == PLAYER_HAS_TWO_USABLE_MONS`: two mons with HP (no eggs yet).
pub(super) fn two_usable(s: &State) -> bool {
    s.party.iter().filter(|m| m.hp != 0).count() >= 2
}

/// Whether trainer script `script` battles as a double (Gen3 reads the `trainerbattle` type,
/// `scriptPtr[1]`). `GameData` cannot say yet (see the Task I report), so every trainer counts
/// as single.
pub(super) fn is_double(game: &impl GameData, script: ScriptId) -> bool {
    let _ = (game, script);
    false
}

/// The trainer that sees the player now, if any.
pub(super) fn check(game: &impl GameData, s: &State) -> Option<Sighting> {
    scan(game, s, |sc| is_double(game, sc))
}

/// [`check`] with the double-battle test given.
pub(super) fn scan(
    game: &impl GameData,
    s: &State,
    double: impl Fn(ScriptId) -> bool,
) -> Option<Sighting> {
    let mut first: Option<Sighting> = None;
    let mut two_mons: Option<bool> = None;
    for t in &s.templates {
        if t.trainer_type != TRAINER_TYPE_NORMAL && t.trainer_type != TRAINER_TYPE_BURIED {
            continue;
        }
        let Some((script, distance)) = trainer_sees(game, s, t) else {
            continue;
        };
        let two = *two_mons.get_or_insert_with(|| two_usable(s));
        if double(script) && !two {
            continue;
        }
        match &mut first {
            None => {
                first = Some(Sighting {
                    obj: t.local_id,
                    script,
                    distance,
                    second: false,
                });
                // One trainer and no double battle possible, or a double trainer: stop.
                if !two || double(script) {
                    break;
                }
            }
            Some(f) => {
                f.second = true;
                break;
            }
        }
    }
    first
}

/// The trainer `t`'s script and its distance to the player, if it is spawned, unbeaten and sees
/// the player (Gen3 `CheckTrainer` without the double test, `GetTrainerApproachDistance`).
fn trainer_sees(game: &impl GameData, s: &State, t: &ObjectEvent) -> Option<(ScriptId, u8)> {
    if t.trainer_sight == 0 {
        return None;
    }
    let o = s.objects.get(t.local_id)?;
    let player = s.objects.player().pos();
    let (ox, oy) = o.pos();
    let ways: &[Dir] = if t.trainer_type == TRAINER_TYPE_NORMAL {
        core::slice::from_ref(&o.facing)
    } else {
        &ALL_WAYS
    };
    let range = t.trainer_sight as i16;
    let ground = s.ground(game);
    let distance = ways.iter().find_map(|&d| {
        let (dx, dy) = d.delta();
        // Gen3 `GetTrainerApproachDistance{South,North,West,East}`.
        let n = if dx == 0 {
            (player.0 == ox).then_some((player.1 - oy) * dy)?
        } else {
            (player.1 == oy).then_some((player.0 - ox) * dx)?
        };
        if !(1..=range).contains(&n) {
            return None;
        }
        path_clear(s, &ground, t.local_id, (ox, oy), d, n).then_some(n as u8)
    })?;
    let script = t.script?;
    let flag = game.trainer_flag_of(script)?;
    (!s.vars.flag(flag)).then_some((script, distance))
}

/// Gen3 `CheckPathBetweenTrainerAndPlayer`: the `n - 1` cells ahead are free for the trainer
/// (wander range ignored), and the `n`th, the player's, collides with an object first.
fn path_clear(
    s: &State,
    ground: &impl Terrain,
    local: u8,
    from: (i16, i16),
    d: Dir,
    n: i16,
) -> bool {
    let (dx, dy) = d.delta();
    let at = |k: i16| (from.0 + dx * k, from.1 + dy * k);
    (1..n).all(|k| s.objects.sight_collision(ground, local, at(k)) == Collision::None)
        && s.objects.sight_collision(ground, local, at(n)) == Collision::Object
}

impl World {
    /// Starts a sighting: everything stops (Gen3 `lockfortrainer`), the approach waits for the
    /// trainer's script, which starts as if the player had talked to the trainer; its
    /// `trainer_battle` issues `TrainerApproach`.
    pub(super) fn start_sighting(&mut self, game: &impl GameData, sighting: Sighting) {
        if sighting.second {
            self.s.log("double sight");
            self.s
                .event(Event::Report(Report::Degraded("double sight")));
        }
        self.s
            .objects
            .sight_pending(sighting.obj, sighting.distance);
        let facing = self.s.objects.player().facing;
        self.start_script(game, sighting.script, Some((sighting.obj, facing, true)));
    }
}
