//! Milestone 8 Task I: the new movement types' step patterns, emote bubbles and the trainer
//! approach.

use super::*;
use crate::data::Dirs;

fn npc_at(m: MovementType, range: (u8, u8)) -> Objects {
    let mut o = player_at(0, 0, Dir::Down);
    o.add(&npc(1, 8, 8, m, range), |_| 0);
    o
}

/// The cells NPC 1 walks to (each step's destination, once), and whether it drew randoms.
fn walked(
    m: MovementType,
    range: (u8, u8),
    frames: u32,
    block: Option<(i16, i16)>,
) -> Vec<(i16, i16)> {
    let g = Grid::open();
    let mut o = npc_at(m, range);
    if let Some((x, y)) = block {
        o.add(&npc(2, x, y, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    }
    let mut rng = Rng::new(1);
    let mut cells = Vec::new();
    let mut last = (8, 8);
    for _ in 0..frames {
        o.update(&g, &mut rng, false);
        let p = o.get(1).unwrap().pos();
        if p != last {
            cells.push(p);
            last = p;
        }
    }
    assert_eq!(rng, Rng::new(1), "{m:?} draws no random");
    cells
}

#[test]
fn in_place_types_use_their_speed() {
    let g = Grid::open();
    for (m, frames) in [
        (MovementType::WalkInPlace(Dir::Left), 16),
        (MovementType::JogInPlace(Dir::Left), 8),
        (MovementType::RunInPlace(Dir::Left), 4),
        (MovementType::WalkSlowlyInPlace(Dir::Left), 32),
    ] {
        let mut o = npc_at(m, (0, 0));
        assert_eq!(o.get(1).unwrap().facing, Dir::Left, "{m:?} initial facing");
        let mut rng = Rng::new(1);
        o.update(&g, &mut rng, false);
        let a = o.get(1).unwrap().action.unwrap();
        assert_eq!(
            (a.kind, a.dir),
            (ActionKind::InPlace(frames), Dir::Left),
            "{m:?}"
        );
        // Back to back: a new one starts the frame after one ends.
        for _ in 0..frames as u32 * 3 {
            o.update(&g, &mut rng, false);
            assert!(!o.get(1).unwrap().is_settled() || o.get(1).unwrap().idle == Idle::Start);
        }
        assert_eq!(o.get(1).unwrap().pos(), (8, 8));
    }
}

#[test]
fn face_two_turns_between_its_pair_after_medium_delays() {
    let g = Grid::open();
    let m = MovementType::FaceTwo(Dirs::two(Dir::Up, Dir::Left));
    let mut o = npc_at(m, (0, 0));
    assert_eq!(
        o.get(1).unwrap().facing,
        Dir::Up,
        "the first is the initial facing"
    );
    let mut rng = Rng::new(3);
    let mut model = Rng::new(3);
    let r = |m: &mut Rng| (m.next_u32() >> 16) as usize;
    o.update(&g, &mut rng, false);
    let delay = DELAYS_MEDIUM[r(&mut model) % 4];
    assert_eq!(o.get(1).unwrap().idle, Idle::Wait(delay));
    for _ in 1..=delay {
        o.update(&g, &mut rng, false);
    }
    let d = [Dir::Up, Dir::Left][r(&mut model) & 1];
    assert_eq!(o.get(1).unwrap().facing, d);
    let mut seen = Vec::new();
    for _ in 0..5000 {
        o.update(&g, &mut rng, false);
        seen.push(o.get(1).unwrap().facing);
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen, [Dir::Up, Dir::Left]);
}

#[test]
fn face_all_but_never_faces_the_missing_way_and_waits_short() {
    let g = Grid::open();
    for missing in Dir::ALL {
        let mut o = npc_at(MovementType::FaceAllBut(missing), (0, 0));
        let start = if missing == Dir::Down {
            Dir::Up
        } else {
            Dir::Down
        };
        assert_eq!(o.get(1).unwrap().facing, start);
        let mut rng = Rng::new(9);
        let mut model = Rng::new(9);
        o.update(&g, &mut rng, false);
        let delay = DELAYS_SHORT[(model.next_u32() >> 16) as usize % 4];
        assert_eq!(o.get(1).unwrap().idle, Idle::Wait(delay));
        let mut seen = Vec::new();
        for _ in 0..5000 {
            o.update(&g, &mut rng, false);
            seen.push(o.get(1).unwrap().facing);
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 3, "{missing:?}: {seen:?}");
        assert!(!seen.contains(&missing));
    }
}

/// Gen3 `MovementType_Rotate*_Step2` returns FALSE when the 48-frame delay ends, so the turn
/// comes a frame later: one quarter turn every 49 frames.
#[test]
fn rotations_turn_every_49_frames_without_randoms() {
    let g = Grid::open();
    for (clockwise, order) in [
        (true, [Dir::Left, Dir::Up, Dir::Right, Dir::Down]),
        (false, [Dir::Right, Dir::Up, Dir::Left, Dir::Down]),
    ] {
        let mut o = npc_at(MovementType::Rotate { clockwise }, (0, 0));
        assert_eq!(o.get(1).unwrap().facing, Dir::Down);
        let mut rng = Rng::new(4);
        o.update(&g, &mut rng, false);
        assert_eq!(o.get(1).unwrap().idle, Idle::Wait(ROTATE_DELAY + 1));
        for d in order {
            for _ in 1..ROTATE_DELAY + 1 {
                o.update(&g, &mut rng, false);
            }
            assert_ne!(o.get(1).unwrap().facing, d);
            o.update(&g, &mut rng, false);
            assert_eq!(o.get(1).unwrap().facing, d, "clockwise {clockwise}");
        }
        assert_eq!(rng, Rng::new(4));
    }
}

#[test]
fn walk_back_and_forth_turns_at_the_range_and_at_home() {
    let cells = walked(MovementType::WalkBackAndForth(Dir::Up), (0, 2), 400, None);
    let ys: Vec<i16> = cells.iter().map(|c| c.1).collect();
    assert_eq!(&ys[..9], [7, 6, 7, 8, 7, 6, 7, 8, 7]);
    assert!(cells.iter().all(|c| c.0 == 8));
}

#[test]
fn walk_back_and_forth_walks_in_place_when_blocked() {
    let g = Grid::open();
    let mut o = npc_at(MovementType::WalkBackAndForth(Dir::Right), (2, 0));
    o.add(&npc(2, 9, 8, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    let mut rng = Rng::new(1);
    o.update(&g, &mut rng, false);
    let n = o.get(1).unwrap();
    assert_eq!(n.pos(), (8, 8));
    assert_eq!(n.action.unwrap().kind, ActionKind::InPlace(16));
    assert_eq!(n.facing, Dir::Right);
}

#[test]
fn walk_sequence_loops_its_square() {
    // Gen3 `MOVEMENT_TYPE_WALK_SEQUENCE_UP_LEFT_DOWN_RIGHT` with a 1-cell range: up, left (up
    // is out of range), down (left is), then right back home, where the index restarts.
    let m = MovementType::WalkSequence(Dirs::new([Dir::Up, Dir::Left, Dir::Down, Dir::Right]));
    let cells = walked(m, (1, 1), 600, None);
    assert_eq!(
        &cells[..8],
        [
            (8, 7),
            (7, 7),
            (7, 8),
            (8, 8),
            (8, 7),
            (7, 7),
            (7, 8),
            (8, 8)
        ]
    );
    assert_eq!(npc_at(m, (1, 1)).get(1).unwrap().facing, Dir::Up);
}

#[test]
fn walk_sequence_skips_the_leg_back_when_on_the_home_column() {
    // `UP_RIGHT_LEFT_DOWN`, 1-cell range: up, right, left back onto the home column, where the
    // left leg (index 2) is cut short (index 3, down) instead of going on to x - 1.
    let m = MovementType::WalkSequence(Dirs::new([Dir::Up, Dir::Right, Dir::Left, Dir::Down]));
    let cells = walked(m, (1, 1), 600, None);
    assert_eq!(
        &cells[..8],
        [
            (8, 7),
            (9, 7),
            (8, 7),
            (8, 8),
            (8, 7),
            (9, 7),
            (8, 7),
            (8, 8)
        ]
    );
}

#[test]
fn walk_sequence_walks_in_place_while_blocked() {
    let m = MovementType::WalkSequence(Dirs::new([Dir::Up, Dir::Right, Dir::Left, Dir::Down]));
    let cells = walked(m, (1, 1), 600, Some((9, 7)));
    assert_eq!(
        cells,
        [(8, 7)],
        "right stays blocked: in place for good, as Gen3"
    );
}

#[test]
fn sequence_index_round_trips_through_snapshots() {
    let g = Grid::open();
    let m = MovementType::WalkSequence(Dirs::new([Dir::Up, Dir::Left, Dir::Down, Dir::Right]));
    let mut o = npc_at(m, (1, 1));
    let mut rng = Rng::new(1);
    for _ in 0..40 {
        o.update(&g, &mut rng, false);
    }
    assert!(matches!(o.get(1).unwrap().idle, Idle::Seq(1..)));
    let snap = o.snapshot();
    Objects::check_snapshot(&snap).unwrap();
    let back = Objects::from_snapshot(&snap);
    assert_eq!(back.get(1).unwrap().idle, o.get(1).unwrap().idle);
    let hash = |o: &Objects| {
        let mut h = Fnv64::new();
        o.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&back), hash(&o));
}

#[test]
fn new_movement_codes_are_distinct() {
    let mut codes = Vec::new();
    for d in Dir::ALL {
        for m in [
            MovementType::FaceAllBut(d),
            MovementType::WalkBackAndForth(d),
            MovementType::JogInPlace(d),
            MovementType::RunInPlace(d),
            MovementType::WalkSlowlyInPlace(d),
            MovementType::Face(d),
            MovementType::WalkInPlace(d),
        ] {
            codes.push(movement_code(m));
        }
        for e in Dir::ALL {
            codes.push(movement_code(MovementType::FaceTwo(Dirs::two(d, e))));
            codes.push(movement_code(MovementType::WalkSequence(Dirs::new([
                d, e, e, d,
            ]))));
        }
    }
    codes.push(movement_code(MovementType::Rotate { clockwise: true }));
    codes.push(movement_code(MovementType::Rotate { clockwise: false }));
    let n = codes.len();
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), n);
}

// ---- emotes ----

#[test]
fn an_emote_shows_61_frames_and_bounces() {
    let g = Grid::open();
    let mut o = player_at(5, 8, Dir::Up);
    o.add(&npc(1, 5, 5, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    let mut rng = Rng::new(1);
    o.update(&g, &mut rng, false);
    o.start_moves(1, &[Step::Emote(Emote::Exclamation)]);
    let mut offsets = Vec::new();
    let mut frames = 0;
    loop {
        o.update(&g, &mut rng, false);
        if !o.emote_active(1) {
            break;
        }
        offsets.push(o.emotes[0].y_off);
        frames += 1;
        assert!(o.any_moves().is_some(), "not capturable while it shows");
    }
    assert_eq!(frames, EMOTE_FRAMES as u32 + 1);
    assert_eq!(
        &offsets[..12],
        [-5, -9, -12, -14, -15, -15, -14, -12, -9, -5, 0, 0]
    );
    assert!(o.any_moves().is_none());
}

#[test]
fn an_emote_ends_with_its_object_and_hashes_only_while_shown() {
    let g = Grid::open();
    let mut o = player_at(5, 8, Dir::Up);
    o.add(&npc(1, 5, 5, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    let hash = |o: &Objects| {
        let mut h = Fnv64::new();
        o.hash(&mut h);
        h.finish()
    };
    let before = hash(&o);
    o.start_emote(1, o.map(), Emote::Heart);
    assert_ne!(hash(&o), before);
    o.remove(1);
    o.update(&g, &mut Rng::new(1), false);
    assert!(o.emotes.is_empty());
}

#[test]
fn an_emote_draws_in_front_above_its_object() {
    let g = Grid::open();
    let mut o = player_at(5, 8, Dir::Up);
    o.add(&npc(1, 5, 5, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    o.update(&g, &mut Rng::new(1), false);
    o.start_emote(1, o.map(), Emote::Exclamation);
    o.start_emote(PLAYER, o.map(), Emote::Question); // no sprite: nothing drawn
    o.update(&g, &mut Rng::new(1), false);
    let cam = camera_on(&o);
    let mut f = Frame::default();
    o.draw(&Game, cam, &mut f);
    assert_eq!(f.objs.len(), 3, "the bubble, the NPC, the player");
    let bubble = f.objs[0];
    let npc = f.objs[2]; // the player is lower, so in front of it
    assert_eq!(bubble.priority, 1);
    assert_eq!(bubble.x, npc.x);
    // NPC 16x16: its center is 8 px below its top; the bubble's is 16 px above that, -5.
    assert_eq!(bubble.y, npc.y + 8 - 16 - 8 - 5);
    assert!(
        f.cmds
            .iter()
            .any(|c| matches!(c, Cmd::ObjTiles { tiles, .. } if tiles[0][0] == 30))
    );
}

// ---- trainer approach ----

#[test]
fn approach_frames_follow_gen3() {
    let g = Grid::open();
    let mut o = player_at(5, 9, Dir::Up);
    o.add(&npc(1, 5, 5, MovementType::LookAround, (0, 0)), |_| 0);
    let mut rng = Rng::new(1);
    o.update(&g, &mut rng, false);
    o.sight_pending(1, 4); // three walks
    assert!(o.player().frozen && o.get(1).unwrap().frozen);
    assert!(o.approach_begin());
    assert!(!o.approach_begin(), "once");
    // Frame-by-frame: the host steps the approach, then objects update.
    let mut events = Vec::new();
    for frame in 0..400u32 {
        let s = o.approach_step();
        o.update(&g, &mut rng, false);
        if s != ApproachStep::Waiting {
            events.push((frame, s));
        }
        if s == ApproachStep::Done {
            break;
        }
    }
    // Frame 0: the `!` starts; it shows 61 frames; frame 62 the first walk starts; three walks
    // of 16 frames end on frame 109; frame 110 faces; frame 111 stops; frame 113 done.
    assert_eq!(
        events,
        [
            (
                111,
                ApproachStep::Stopped {
                    obj: 1,
                    dir: Dir::Down,
                    pos: (5, 8)
                }
            ),
            (113, ApproachStep::Done)
        ]
    );
    assert_eq!(o.get(1).unwrap().movement, MovementType::Face(Dir::Down));
    assert_eq!(o.player().facing, Dir::Up);
    assert!(o.approaching().is_none());
}
