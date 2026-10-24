//! Rotating gates (Gen3 `rotating_gate.c`): the gates of a puzzle turn a quarter when the
//! player walks into one of their arms, if the arms' sweep is clear of the map's walls, and
//! block the step otherwise.
//!
//! A puzzle is the map's [`RotatingGate`] list ([`crate::data::GameData::rotating_gates`]).
//! The orientations live in the temp vars, one byte per gate from `VAR_TEMP_0`
//! (`RotatingGate_GetGateOrientation`); here they are packed two bits per gate
//! ([`Orientations`]). Only the player collides with the gates
//! (`CheckForObjectEventCollision`); the gate sprites are drawn by no one yet.

use crate::data::{Dir, RotatingGate, VarId};
use crate::flow::{VARS_START, Vars};

/// The orientations of up to 16 gates, two bits each (gate `i` at bits `2 i`).
pub type Orientations = u32;

/// `ROTATE_ANTICLOCKWISE` (`ROTATE_CLOCKWISE` is 2).
const ANTICLOCKWISE: u8 = 1;
/// `GATE_ROT_NONE`.
const NONE: u8 = 0xFF;

/// `GATE_ROT(rotation, arm, long)`: the rotation in the high nibble, the arm (`GATE_ARM_*`,
/// north, east, south, west) and whether it is the long part below.
const fn rot(rotation: u8, arm: u8, long: u8) -> u8 {
    (rotation << 4) | (arm << 1) | long
}
const fn cw(arm: u8, long: u8) -> u8 {
    rot(2, arm, long)
}
const fn acw(arm: u8, long: u8) -> u8 {
    rot(ANTICLOCKWISE, arm, long)
}
const N: u8 = 0;
const E: u8 = 1;
const S: u8 = 2;
const W: u8 = 3;
const X: u8 = NONE;

/// `sRotatingGate_RotationInfo{North,South,West,East}` by the step's direction: the arm a
/// step onto a cell of the gate's 4x4 square (`x - gateX + 2`, `y - gateY + 2`) pushes, and
/// which way it turns.
#[rustfmt::skip]
const ROTATION: [[u8; 16]; 4] = [
    // North.
    [X, X, X, X,  cw(W, 1), cw(W, 0), acw(E, 0), acw(E, 1),  X, X, X, X,  X, X, X, X],
    // South.
    [X, X, X, X,  X, X, X, X,  acw(W, 1), acw(W, 0), cw(E, 0), cw(E, 1),  X, X, X, X],
    // West.
    [X, acw(N, 1), X, X,  X, acw(N, 0), X, X,  X, cw(S, 0), X, X,  X, cw(S, 1), X, X],
    // East.
    [X, X, cw(N, 1), X,  X, X, cw(N, 0), X,  X, X, acw(S, 0), X,  X, X, acw(S, 1), X],
];

/// `sRotatingGate_ArmPositions{AntiClockwise,Clockwise}Rotation`: the cells an arm sweeps,
/// relative to the pivot, by `2 * arm + long`.
const ARM_POSITIONS: [[(i16, i16); 8]; 2] = [
    [
        (-1, -1),
        (-1, -2),
        (0, -1),
        (1, -1),
        (0, 0),
        (0, 1),
        (-1, 0),
        (-2, 0),
    ],
    [
        (0, -1),
        (1, -2),
        (0, 0),
        (1, 0),
        (-1, 0),
        (-1, 1),
        (-1, -1),
        (-2, -1),
    ],
];

/// `sRotatingGate_ArmLayout` by `GATE_SHAPE_*`: per arm (north, east, south, west) at
/// orientation 0, its short then its long part.
#[rustfmt::skip]
pub const ARM_LAYOUT: [[u8; 8]; 12] = [
    [1, 0, 1, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 1, 0, 0, 0, 0],
    [1, 1, 1, 1, 0, 0, 0, 0],
    [1, 0, 1, 0, 1, 0, 0, 0],
    [1, 1, 1, 0, 1, 0, 0, 0],
    [1, 0, 1, 1, 1, 0, 0, 0],
    [1, 0, 1, 0, 1, 1, 0, 0],
    [1, 1, 1, 1, 1, 0, 0, 0],
    [1, 1, 1, 0, 1, 1, 0, 0],
    [1, 0, 1, 1, 1, 1, 0, 0],
    [1, 1, 1, 1, 1, 1, 0, 0],
];

const fn table(d: Dir) -> usize {
    match d {
        Dir::Up => 0,
        Dir::Down => 1,
        Dir::Left => 2,
        Dir::Right => 3,
    }
}

/// Gate `i`'s orientation in `o`.
pub const fn orientation(o: Orientations, i: usize) -> u8 {
    ((o >> (2 * i)) & 3) as u8
}

/// The puzzle's starting orientations (`RotatingGate_ResetAllGateOrientations`).
pub fn initial(gates: &[RotatingGate]) -> Orientations {
    (gates.iter().enumerate()).fold(0, |o, (i, g)| o | (u32::from(g.orientation & 3) << (2 * i)))
}

/// The orientations from the temp vars (`RotatingGate_GetGateOrientation`: byte `i` of the
/// vars from `VAR_TEMP_0`, little-endian), `var` reading a var.
pub fn read(var: impl Fn(VarId) -> u16, gates: &[RotatingGate]) -> Orientations {
    (0..gates.len().min(16)).fold(0, |o, i| {
        let v = var(VarId(VARS_START + (i / 2) as u16));
        let byte = (v >> (8 * (i % 2))) as u8;
        o | (u32::from(byte & 3) << (2 * i))
    })
}

/// Writes the orientations back as the temp vars' bytes (`RotatingGate_SetGateOrientation`).
pub fn write(vars: &mut Vars, gates: &[RotatingGate], o: Orientations) {
    for pair in 0..gates.len().min(16).div_ceil(2) {
        let lo = u16::from(orientation(o, 2 * pair));
        let hi = if 2 * pair + 1 < gates.len() {
            u16::from(orientation(o, 2 * pair + 1))
        } else {
            vars.var(VarId(VARS_START + pair as u16)) >> 8
        };
        vars.set_var(VarId(VARS_START + pair as u16), lo | hi << 8);
    }
}

/// `RotatingGate_HasArm`.
fn has_arm(g: &RotatingGate, orientation: u8, info: u8) -> bool {
    let arm = info / 2;
    let long = (info % 2) as usize;
    let o = ((arm + 4 - orientation) % 4) as usize;
    ARM_LAYOUT[g.shape as usize % 12][o * 2 + long] != 0
}

/// `RotatingGate_CanRotate`, unfixed (only collision 1 stops an arm, as Emerald ships).
fn can_rotate(
    g: &RotatingGate,
    orientation: u8,
    rotation: u8,
    collision: &impl Fn(i16, i16) -> u8,
) -> bool {
    let pos = &ARM_POSITIONS[if rotation == ANTICLOCKWISE { 0 } else { 1 }];
    let layout = &ARM_LAYOUT[g.shape as usize % 12];
    for i in 0..4usize {
        for j in 0..2usize {
            if layout[2 * i + j] != 0 {
                let (dx, dy) = pos[2 * ((orientation as usize + i) % 4) + j];
                if collision(g.x + dx, g.y + dy) == 1 {
                    return false;
                }
            }
        }
    }
    true
}

/// Gen3 `CheckForRotatingGatePuzzleCollision`: the player steps `dir` onto `(x, y)` with the
/// gates at `o`. `None`: a gate blocks the step. Else the orientations after it (the gate the
/// step pushes has turned). `collision` is the map's collision bits at a cell
/// (`MapGridGetCollisionAt`).
pub fn step(
    gates: &[RotatingGate],
    o: Orientations,
    dir: Dir,
    (x, y): (i16, i16),
    collision: impl Fn(i16, i16) -> u8,
) -> Option<Orientations> {
    for (i, g) in gates.iter().enumerate().take(16) {
        if !(g.x - 2 <= x && x <= g.x + 1 && g.y - 2 <= y && y <= g.y + 1) {
            continue;
        }
        let (cx, cy) = (x - g.x + 2, y - g.y + 2);
        let info = ROTATION[table(dir)][(cy * 4 + cx) as usize];
        if info == NONE {
            continue;
        }
        let rotation = info >> 4;
        let cur = orientation(o, i);
        if has_arm(g, cur, info & 0xF) {
            if !can_rotate(g, cur, rotation, &collision) {
                return None;
            }
            let next = if rotation == ANTICLOCKWISE {
                (cur + 3) % 4
            } else {
                (cur + 1) % 4
            };
            return Some(o & !(3 << (2 * i)) | (u32::from(next) << (2 * i)));
        }
    }
    Some(o)
}

#[cfg(test)]
#[path = "gates_tests.rs"]
mod tests;
