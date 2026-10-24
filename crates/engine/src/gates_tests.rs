use super::*;
use alloc::vec;

/// A `T1` gate (`|-` with a south arm: north, east and south short arms) at (5, 5).
const T1: RotatingGate = RotatingGate {
    x: 5,
    y: 5,
    shape: 4,
    orientation: 0,
};

#[test]
fn a_gate_turns_when_its_arm_is_walked_into_and_blocks_when_it_cannot() {
    let open = |_: i16, _: i16| 0;
    // Moving north onto (5, 4)... centre (2, 1): `acw(E, 0)`, the east arm: turned
    // anticlockwise to 270.
    assert_eq!(step(&[T1], 0, Dir::Up, (5, 4), open), Some(3));
    // Moving north onto (4, 4), centre (1, 1): `cw(W, 0)`, no west arm at 0: free.
    assert_eq!(step(&[T1], 0, Dir::Up, (4, 4), open), Some(0));
    // A wall where the arms sweep: blocked. Collision 3 does not block (unfixed).
    let wall = |x: i16, y: i16| u8::from((x, y) == (4, 4));
    assert_eq!(step(&[T1], 0, Dir::Up, (5, 4), wall), None);
    let wall3 = |x: i16, y: i16| if (x, y) == (4, 4) { 3 } else { 0 };
    assert_eq!(step(&[T1], 0, Dir::Up, (5, 4), wall3), Some(3));
    // Far from the gate: nothing.
    assert_eq!(step(&[T1], 2, Dir::Up, (9, 9), open), Some(2));
}

#[test]
fn orientations_round_trip_through_the_temp_vars() {
    let gates = vec![T1; 3];
    let mut vars = Vars::new();
    vars.set_var(VarId(VARS_START + 1), 0xAB00);
    let o = 0b10_01_11;
    write(&mut vars, &gates, o);
    assert_eq!(vars.var(VarId(VARS_START)), 0x0103);
    // The third gate's byte; the byte past the puzzle is kept.
    assert_eq!(vars.var(VarId(VARS_START + 1)), 0xAB02);
    assert_eq!(read(|v| vars.var(v), &gates), o);
    let start = [
        RotatingGate {
            orientation: 1,
            ..T1
        },
        RotatingGate {
            orientation: 2,
            ..T1
        },
    ];
    assert_eq!(initial(&start), 0b10_01);
}
