//! Rotating gates (Gen3 `rotating_gate.c`: Fortree's Gym, the Trick House's sixth puzzle) on
//! BPEE: a gate turns when the player walks into one of its arms and its arms clear the map,
//! and blocks otherwise. Our engine has no rotating gates (an engine gap: its walks pass
//! through them), so the vanilla walk plans over the gates' orientations itself.
//!
//! Everything comes from the running game: the puzzle (`sRotatingGate_PuzzleConfig`), the
//! tables (`sRotatingGate_ArmLayout`, the rotation infos, the arm positions) from the ROM, the
//! orientations from the temp vars (`VAR_TEMP_0` on, one byte per gate), the collisions from
//! `gBackupMapLayout`. Coordinates here are the map grid's (Gen3's `MAP_OFFSET` included).

use engine::data::Dir;
use std::collections::{HashSet, VecDeque};

/// `ROTATE_ANTICLOCKWISE` (`ROTATE_CLOCKWISE` is 2).
const ROTATE_ANTICLOCKWISE: u8 = 1;
const ROTATE_CLOCKWISE: u8 = 2;
/// `GATE_ROT_NONE`.
const ROT_NONE: u8 = 255;
/// The search's state cap: a plan past it gives up (the walk then uses the plain planner).
const MAX_STATES: usize = 1 << 21;

/// One gate of the puzzle (`struct RotatingGatePuzzle`), in map grid coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate {
    pub x: i16,
    pub y: i16,
    pub shape: u8,
}

/// The puzzle and the tables it is played with.
#[derive(Clone, Debug)]
pub struct Gates {
    pub gates: Vec<Gate>,
    /// `sRotatingGate_ArmLayout[shape]`: per arm (N, E, S, W), short then long.
    pub arm_layout: Vec<[u8; 8]>,
    /// `sRotatingGate_RotationInfo{North,South,West,East}`, by [`Dir`] (`ptr[y * 4 + x]`).
    pub rotation: [[u8; 16]; 4],
    /// `sRotatingGate_ArmPositions{AntiClockwise,Clockwise}Rotation`.
    pub arm_positions: [[(i8, i8); 8]; 2],
    /// The map grid's collision (0-3) per cell, `w` wide, `h` high.
    pub collision: Vec<u8>,
    pub w: i16,
    pub h: i16,
}

fn table(d: Dir) -> usize {
    match d {
        Dir::Up => 0,
        Dir::Down => 1,
        Dir::Left => 2,
        Dir::Right => 3,
    }
}

impl Gates {
    /// `MapGridGetCollisionAt`: off the grid, the border block's (its collision bits set: 3).
    fn collision_at(&self, x: i16, y: i16) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 3;
        }
        self.collision[(y * self.w + x) as usize]
    }

    /// `RotatingGate_HasArm`.
    fn has_arm(&self, gate: usize, orientation: u8, arm_info: u8) -> bool {
        let arm = (arm_info / 2) as i32;
        let long = (arm_info % 2) as usize;
        let o = ((arm - orientation as i32 + 4) % 4) as usize;
        self.arm_layout[self.gates[gate].shape as usize][o * 2 + long] != 0
    }

    /// `RotatingGate_CanRotate` (unfixed: only collision 1 blocks an arm).
    fn can_rotate(&self, gate: usize, orientation: u8, rotation: u8) -> bool {
        let pos = match rotation {
            ROTATE_ANTICLOCKWISE => &self.arm_positions[0],
            ROTATE_CLOCKWISE => &self.arm_positions[1],
            _ => return false,
        };
        let g = self.gates[gate];
        let layout = &self.arm_layout[g.shape as usize];
        for i in 0..4usize {
            for j in 0..2usize {
                let idx = 2 * ((orientation as usize + i) % 4) + j;
                if layout[2 * i + j] != 0 {
                    let (dx, dy) = pos[idx];
                    if self.collision_at(g.x + dx as i16, g.y + dy as i16) == 1 {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// `CheckForRotatingGatePuzzleCollision`: moving `dir` onto `(x, y)` with the gates at
    /// `orientations`; `None` when a gate blocks, else the orientations after the step.
    pub fn step(&self, orientations: u32, dir: Dir, (x, y): (i16, i16)) -> Option<u32> {
        for (i, g) in self.gates.iter().enumerate() {
            if !(g.x - 2 <= x && x <= g.x + 1 && g.y - 2 <= y && y <= g.y + 1) {
                continue;
            }
            let (cx, cy) = (x - g.x + 2, y - g.y + 2);
            let info = self.rotation[table(dir)][(cy * 4 + cx) as usize];
            if info == ROT_NONE {
                continue;
            }
            let rotation = (info & 0xF0) >> 4;
            let o = (orientations >> (2 * i) & 3) as u8;
            if self.has_arm(i, o, info & 0xF) {
                if !self.can_rotate(i, o, rotation) {
                    return None;
                }
                let o2 = if rotation == ROTATE_ANTICLOCKWISE {
                    (o + 3) % 4
                } else {
                    (o + 1) % 4
                };
                let mask = 3 << (2 * i);
                return Some(orientations & !mask | (u32::from(o2) << (2 * i)));
            }
        }
        Some(orientations)
    }

    /// The first direction of a shortest walk from `from` to `to` (map grid cells) over the
    /// gates' orientations; `open` says whether a cell can be stepped on (the gates aside).
    pub fn first_dir(
        &self,
        from: (i16, i16),
        to: (i16, i16),
        orientations: u32,
        open: impl Fn((i16, i16)) -> bool,
    ) -> Option<Dir> {
        if from == to {
            return None;
        }
        let mut seen: HashSet<((i16, i16), u32)> = HashSet::new();
        let mut queue: VecDeque<((i16, i16), u32, Dir)> = VecDeque::new();
        seen.insert((from, orientations));
        let push = |c: (i16, i16),
                    o: u32,
                    first: Option<Dir>,
                    seen: &mut HashSet<((i16, i16), u32)>,
                    q: &mut VecDeque<((i16, i16), u32, Dir)>| {
            for d in Dir::ALL {
                let (dx, dy) = d.delta();
                let n = (c.0 + dx, c.1 + dy);
                if n != to && !open(n) {
                    continue;
                }
                if let Some(o2) = self.step(o, d, n)
                    && seen.insert((n, o2))
                {
                    q.push_back((n, o2, first.unwrap_or(d)));
                }
            }
        };
        push(from, orientations, None, &mut seen, &mut queue);
        while let Some((c, o, first)) = queue.pop_front() {
            if c == to {
                return Some(first);
            }
            if seen.len() > MAX_STATES {
                return None;
            }
            push(c, o, Some(first), &mut seen, &mut queue);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `|`-shaped gate (arms north and south, short) at (5, 5) on an open 12x12 floor,
    /// with made-up tables: walking up into its south arm from the east turns it clockwise.
    /// The pivot's cell index: the made-up arm positions all sweep it.
    const PIVOT: usize = 5 * 12 + 5;

    fn one_gate() -> Gates {
        let mut rotation = [[ROT_NONE; 16]; 4];
        // Moving up onto (gate.x, gate.y + 1)... the south arm's cell east of the pivot.
        rotation[table(Dir::Up)][(3 * 4 + 2) as usize] = (ROTATE_CLOCKWISE << 4) | (2 << 1);
        Gates {
            gates: vec![Gate {
                x: 5,
                y: 5,
                shape: 0,
            }],
            arm_layout: vec![[1, 0, 0, 0, 1, 0, 0, 0]],
            rotation,
            arm_positions: [[(0, 0); 8], [(0, 0); 8]],
            collision: vec![0; 144],
            w: 12,
            h: 12,
        }
    }

    #[test]
    fn a_gate_turns_when_walked_into_and_blocks_when_it_cannot() {
        let mut g = one_gate();
        // (5, 6) is centre (2, 3): the gate's south arm, orientation 0.
        assert_eq!(g.step(0, Dir::Up, (5, 6)), Some(1));
        // Turned once, the south arm is now west: nothing to push.
        assert_eq!(g.step(1, Dir::Up, (5, 6)), Some(1));
        // A wall where the arm sweeps: blocked.
        g.collision[PIVOT] = 1;
        assert_eq!(g.step(0, Dir::Up, (5, 6)), None);
        // Collision 2 or 3 does not block (Gen3's unfixed check).
        g.collision[PIVOT] = 3;
        assert_eq!(g.step(0, Dir::Up, (5, 6)), Some(1));
    }

    #[test]
    fn the_plan_goes_around_a_blocking_gate() {
        let mut g = one_gate();
        g.collision[PIVOT] = 1;
        let open = |(x, y): (i16, i16)| (0..12).contains(&x) && (0..12).contains(&y);
        // Straight up from (5, 7) would push the blocked gate: the plan sidesteps first.
        let d = g.first_dir((5, 7), (5, 4), 0, open).unwrap();
        assert_ne!(d, Dir::Up);
        g.collision[PIVOT] = 0;
        assert_eq!(g.first_dir((5, 7), (5, 4), 0, open), Some(Dir::Up));
    }
}
