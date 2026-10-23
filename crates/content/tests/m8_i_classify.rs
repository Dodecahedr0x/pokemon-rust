//! Milestone 8 Task I: every `MOVEMENT_TYPE_*` maps to the engine's variant of that type; only
//! the types the engine has no behaviour for are approximated.

#[allow(dead_code, unused_imports)]
#[path = "../build/classify/mod.rs"]
mod classify;

use classify::movement;

/// The types still approximated (they face down).
const APPROXIMATED: &[&str] = &[
    "NONE",
    "PLAYER",
    "BERRY_TREE_GROWTH",
    "COPY_PLAYER",
    "COPY_PLAYER_OPPOSITE",
    "COPY_PLAYER_COUNTERCLOCKWISE",
    "COPY_PLAYER_CLOCKWISE",
    "TREE_DISGUISE",
    "MOUNTAIN_DISGUISE",
    "COPY_PLAYER_IN_GRASS",
    "COPY_PLAYER_OPPOSITE_IN_GRASS",
    "COPY_PLAYER_COUNTERCLOCKWISE_IN_GRASS",
    "COPY_PLAYER_CLOCKWISE_IN_GRASS",
    "BURIED",
];

fn dir(w: &str) -> &'static str {
    match w {
        "UP" => "Dir::Up",
        "DOWN" => "Dir::Down",
        "LEFT" => "Dir::Left",
        "RIGHT" => "Dir::Right",
        _ => panic!("{w}"),
    }
}

/// The expected expression, written independently of `classify::movement`.
fn expected(t: &str) -> String {
    let words: Vec<&str> = t.split('_').filter(|w| *w != "AND").collect();
    let ds = |from: usize| words[from..].iter().map(|w| dir(w)).collect::<Vec<_>>();
    let e = match words[..] {
        ["LOOK", "AROUND"] => "LookAround".into(),
        ["WANDER", "AROUND"] => "Wander".into(),
        ["WANDER", "UP", "DOWN"] | ["WANDER", "DOWN", "UP"] => "WanderVertical".into(),
        ["WANDER", "LEFT", "RIGHT"] | ["WANDER", "RIGHT", "LEFT"] => "WanderHorizontal".into(),
        ["INVISIBLE"] => "Invisible".into(),
        ["ROTATE", "CLOCKWISE"] => "Rotate { clockwise: true }".into(),
        ["ROTATE", "COUNTERCLOCKWISE"] => "Rotate { clockwise: false }".into(),
        ["FACE", _] => format!("Face({})", ds(1)[0]),
        ["FACE", _, _] => format!("FaceTwo(Dirs::two({}, {}))", ds(1)[0], ds(1)[1]),
        ["FACE", _, _, _] => {
            let have = ds(1);
            let m = ["Dir::Down", "Dir::Up", "Dir::Left", "Dir::Right"]
                .into_iter()
                .find(|d| !have.contains(d))
                .unwrap();
            format!("FaceAllBut({m})")
        }
        ["WALK", "IN", "PLACE", _] => format!("WalkInPlace({})", ds(3)[0]),
        ["JOG", "IN", "PLACE", _] => format!("JogInPlace({})", ds(3)[0]),
        ["RUN", "IN", "PLACE", _] => format!("RunInPlace({})", ds(3)[0]),
        ["WALK", "SLOWLY", "IN", "PLACE", _] => format!("WalkSlowlyInPlace({})", ds(4)[0]),
        ["WALK", "SEQUENCE", ..] => {
            let d = ds(2);
            format!("WalkSequence(Dirs::new([{}]))", d.join(", "))
        }
        ["WALK", _, _] => format!("WalkBackAndForth({})", ds(1)[0]),
        _ => panic!("unexpected movement type {t}"),
    };
    format!("MovementType::{e}")
}

#[test]
fn every_movement_type_is_its_variant_or_a_listed_approximation() {
    let (mut exact, mut sequences) = (0, 0);
    for (_, name) in extract::names::MOVEMENT_TYPES.iter() {
        let t = name.strip_prefix("MOVEMENT_TYPE_").unwrap();
        let (expr, is_exact) = movement(name);
        if APPROXIMATED.contains(&t) {
            assert_eq!(
                (expr.as_str(), is_exact),
                ("MovementType::Face(Dir::Down)", false),
                "{name}"
            );
            continue;
        }
        assert!(is_exact, "{name}: {expr}");
        assert_eq!(expr, expected(t), "{name}");
        exact += 1;
        sequences += t.starts_with("WALK_SEQUENCE_") as u32;
    }
    assert_eq!(sequences, 24);
    assert_eq!(exact + APPROXIMATED.len(), 81, "Gen3's 81 types");
}
