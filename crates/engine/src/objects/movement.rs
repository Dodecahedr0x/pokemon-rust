//! Idle movement per [`MovementType`] (Gen3 `MovementType_*` callbacks, `event_object_movement.c`,
//! direction tables in `movement_type_func_tables.h`).
//!
//! Not modelled: trainers turning towards a running player (`ObjectEventIsTrainerAndCloseToPlayer`,
//! `TryGetTrainerEncounterDirection`), and the movement types content maps to the closest
//! variant (copy-player, disguises, buried, berry trees).

use super::{ActionKind, Collision, Idle, Objects, Terrain, in_place_frames};
use crate::data::{Dir, Dirs, MovementType};
use crate::rng::Rng;
use crate::script::Speed;

/// `sMovementDelaysMedium`, picked with `Random() % 4` by the look-around, wander and
/// two-direction types.
pub(super) const DELAYS_MEDIUM: [u16; 4] = [32, 64, 96, 128];
/// `sMovementDelaysShort`, for the three-direction types.
pub(super) const DELAYS_SHORT: [u16; 4] = [32, 48, 64, 80];
/// `MovementType_Rotate*_Step1`: `SetMovementDelay(sprite, 48)`. `_Step2` returns FALSE when
/// the delay ends, so the turn is a frame later: the wait is `ROTATE_DELAY + 1`.
pub(super) const ROTATE_DELAY: u16 = 48;

/// Gen3 `gDownUpAndLeftDirections` … `gDownLeftAndRightDirections` (picked with `Random() & 3`),
/// by the direction left out.
const fn all_but(missing: Dir) -> [Dir; 4] {
    use Dir::*;
    match missing {
        Right => [Up, Down, Left, Down],
        Left => [Down, Up, Right, Down],
        Down => [Up, Left, Right, Up],
        Up => [Left, Right, Down, Down],
    }
}

/// Gen3 `gClockwiseDirections` / `gCounterclockwiseDirections`, indexed by the facing.
const fn rotate(d: Dir, clockwise: bool) -> Dir {
    use Dir::*;
    match (d, clockwise) {
        (Down, true) => Left,
        (Left, true) => Up,
        (Up, true) => Right,
        (Right, true) => Down,
        (Down, false) => Right,
        (Right, false) => Up,
        (Up, false) => Left,
        (Left, false) => Down,
    }
}

const fn horizontal(d: Dir) -> bool {
    matches!(d, Dir::Left | Dir::Right)
}

/// How a turning type picks its next direction.
enum Pick {
    /// `directions[Random() & (len - 1)]`; walks there if free.
    Wander(&'static [Dir]),
    /// `directions[Random() & (len - 1)]`; only turns.
    Look(&'static [Dir]),
    /// Gen3 `MovementType_FaceDownUpAndLeft` …: `all_but` with `Random() & 3`.
    LookAllBut(Dir),
    /// The next quarter turn; no `Random()`.
    Rotate(bool),
}

impl Objects {
    /// Idle movement per `MovementType` (Gen3 `MovementType_*` callbacks).
    pub(super) fn idle(&mut self, terrain: &impl Terrain, rng: &mut Rng, i: usize) {
        let o = &mut self.objs[i];
        // The common frame: a turning type waiting out its delay (only those wait).
        if let Idle::Wait(n) = o.idle
            && n > 1
        {
            o.idle = Idle::Wait(n - 1);
            return;
        }
        let (pick, delays): (Pick, &[u16; 4]) = match o.movement {
            MovementType::Face(d) => {
                if o.idle == Idle::Start {
                    o.face(d);
                    o.idle = Idle::Done;
                }
                return;
            }
            MovementType::Invisible => {
                if o.idle == Idle::Start {
                    o.visible = false;
                    o.idle = Idle::Done;
                }
                return;
            }
            MovementType::WalkInPlace(d) => return in_place(o, d, Speed::Normal),
            MovementType::JogInPlace(d) => return in_place(o, d, Speed::Fast),
            MovementType::RunInPlace(d) => return in_place(o, d, Speed::Faster),
            MovementType::WalkSlowlyInPlace(d) => return in_place(o, d, Speed::Slow),
            MovementType::WalkBackAndForth(d) => return self.back_and_forth(terrain, i, d),
            MovementType::WalkSequence(route) => return self.walk_sequence(terrain, i, route),
            MovementType::LookAround => (Pick::Look(&Dir::ALL), &DELAYS_MEDIUM),
            MovementType::Wander => (Pick::Wander(&Dir::ALL), &DELAYS_MEDIUM),
            // `gUpAndDownDirections` / `gLeftAndRightDirections`.
            MovementType::WanderVertical => (Pick::Wander(&[Dir::Down, Dir::Up]), &DELAYS_MEDIUM),
            MovementType::WanderHorizontal => {
                (Pick::Wander(&[Dir::Left, Dir::Right]), &DELAYS_MEDIUM)
            }
            MovementType::FaceTwo(p) => (Pick::Look(pair(p)), &DELAYS_MEDIUM),
            MovementType::FaceAllBut(m) => (Pick::LookAllBut(m), &DELAYS_SHORT),
            MovementType::Rotate { clockwise } => (Pick::Rotate(clockwise), &DELAYS_MEDIUM),
        };
        let random = |rng: &mut Rng| (rng.next_u32() >> 16) as u16;
        if let Idle::Wait(_) = o.idle {
            // Delay over: pick a direction, and walk if the type wanders and the way is free.
            let (d, walks) = match pick {
                Pick::Wander(dirs) => (dirs[random(rng) as usize & (dirs.len() - 1)], true),
                Pick::Look(dirs) => (dirs[random(rng) as usize & (dirs.len() - 1)], false),
                Pick::LookAllBut(m) => (all_but(m)[random(rng) as usize & 3], false),
                Pick::Rotate(cw) => (rotate(o.facing, cw), false),
            };
            o.set_dir(d);
            if walks && self.collision(terrain, self.objs[i].local_id, d) == Collision::None {
                let o = &mut self.objs[i];
                o.begin(ActionKind::Walk(Speed::Normal), d);
                o.idle = Idle::Start;
                return;
            }
        }
        // Face the current direction, then wait (`SetMovementDelay`).
        let o = &mut self.objs[i];
        let f = o.facing;
        o.face(f);
        o.idle = Idle::Wait(match pick {
            Pick::Rotate(_) => ROTATE_DELAY + 1,
            _ => delays[random(rng) as usize % 4],
        });
    }

    /// Gen3 `MovementType_WalkBackAndForth`: towards `first` (the opposite once the sequence
    /// index is set) until the range ends, then back; walks in place when blocked.
    fn back_and_forth(&mut self, terrain: &impl Terrain, i: usize, first: Dir) {
        let o = &mut self.objs[i];
        let mut seq = seq(o.idle);
        // Step1.
        let mut d = if seq != 0 { first.opposite() } else { first };
        o.set_dir(d);
        // Step2.
        if seq != 0 && o.pos() == o.home {
            seq = 0;
            d = d.opposite();
            o.set_dir(d);
        }
        let local = o.local_id;
        let mut c = self.collision(terrain, local, d);
        if c == Collision::OutsideRange {
            seq = seq.wrapping_add(1);
            d = d.opposite();
            self.objs[i].set_dir(d);
            c = self.collision(terrain, local, d);
        }
        self.step_or_walk_in_place(i, d, c, seq);
    }

    /// Gen3 `MovementType_WalkSequence*` and `MoveNextDirectionInSequence`.
    fn walk_sequence(&mut self, terrain: &impl Terrain, i: usize, route: Dirs) {
        let o = &mut self.objs[i];
        let mut seq = seq(o.idle);
        // Each `_Step1` skips the leg that would undo the one before it when the object is
        // already back on its spawn column (row): the first leg whose direction is the opposite
        // of an earlier one, index 1 or 2.
        let k: u8 = if route.get(1) == route.get(0).opposite() {
            1
        } else {
            2
        };
        let back = if horizontal(route.get(k as usize)) {
            o.x == o.home.0
        } else {
            o.y == o.home.1
        };
        if seq == k && back {
            seq = k + 1;
        }
        if seq == 3 && o.pos() == o.home {
            seq = 0;
        }
        let local = o.local_id;
        let mut d = route.get(seq as usize);
        o.set_dir(d);
        let mut c = self.collision(terrain, local, d);
        if c == Collision::OutsideRange {
            // Gen3 reads `route[4]` past the array after the last leg; wrap instead.
            seq = (seq + 1) & 3;
            d = route.get(seq as usize);
            self.objs[i].set_dir(d);
            c = self.collision(terrain, local, d);
        }
        self.step_or_walk_in_place(i, d, c, seq);
    }

    /// `walk_normal` towards `d` if free, else `walk_in_place_normal` (the facing).
    fn step_or_walk_in_place(&mut self, i: usize, d: Dir, c: Collision, seq: u8) {
        let o = &mut self.objs[i];
        if c == Collision::None {
            o.begin(ActionKind::Walk(Speed::Normal), d);
        } else {
            let f = o.facing;
            o.begin(ActionKind::InPlace(in_place_frames(Speed::Normal)), f);
        }
        o.idle = Idle::Seq(seq);
    }
}

/// Gen3 `MovementType_WalkInPlace` and its jog, run and slow variants: one in-place step after
/// another.
fn in_place(o: &mut super::Object, d: Dir, s: Speed) {
    o.begin(ActionKind::InPlace(in_place_frames(s)), d);
}

/// The sequence index of a walk sequence's idle state.
const fn seq(idle: Idle) -> u8 {
    match idle {
        Idle::Seq(n) => n,
        _ => 0,
    }
}

/// The two directions of a `FaceTwo` as a static slice (Gen3 `g*Directions` pairs).
fn pair(p: Dirs) -> &'static [Dir] {
    use Dir::*;
    static PAIRS: [[[Dir; 2]; 4]; 4] = {
        let all = [Down, Up, Left, Right];
        let mut t = [[[Down; 2]; 4]; 4];
        let mut i = 0;
        while i < 4 {
            let mut j = 0;
            while j < 4 {
                t[i][j] = [all[i], all[j]];
                j += 1;
            }
            i += 1;
        }
        t
    };
    &PAIRS[p.get(0) as usize][p.get(1) as usize]
}
