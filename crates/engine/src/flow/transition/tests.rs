use super::super::test_data::*;
use super::*;
use crate::flow::{FADE_MAX, behavior_at};
use alloc::vec::Vec;

#[test]
fn idle_snapshot_round_trips_with_the_leftover_fields() {
    let mut t = Transition::new();
    let mut fader = Fader::new();
    t.start(&Game, MapId(1), None, 3, 4);
    run(&mut t, &mut fader, 4);
    assert!(!t.is_active());
    assert_eq!((t.map, t.x, t.y), (MapId(1), 3, 4));
    let r = Transition::from_snapshot(&t.snapshot());
    assert_eq!(r, t);
}

/// Runs to `Done`, simulating a player whose walks take `walk` frames. Returns every step and
/// the fader level after it.
fn run(t: &mut Transition, fader: &mut Fader, walk: u32) -> Vec<(TransitionStep, u8, bool)> {
    let mut out = Vec::new();
    let mut walking: u32 = 0;
    for _ in 0..200 {
        let s = t.update(fader, walking > 0);
        walking = walking.saturating_sub(1);
        if let TransitionStep::Walk(_) = s {
            walking = walk;
        }
        if let TransitionStep::Load(m, x, y) = s {
            // Arrival facing, checked through `t.facing` by the tests.
            t.arrive(behavior_at(&Game, Game.map(m), x, y), Dir::Up);
        }
        out.push((s, fader.level(), t.player_hidden()));
        if s == TransitionStep::Done {
            return out;
        }
    }
    panic!("transition never finished");
}

fn count(steps: &[(TransitionStep, u8, bool)], s: TransitionStep) -> usize {
    steps.iter().filter(|x| x.0 == s).count()
}

#[test]
fn idle_is_done() {
    let mut t = Transition::new();
    assert!(!t.is_active());
    assert_eq!(t.update(&mut Fader::new(), false), TransitionStep::Done);
}

#[test]
fn plain_warp_fades_out_loads_once_and_fades_in() {
    let mut t = Transition::new();
    let mut f = Fader::new();
    // House warp 1 is on a normal cell.
    t.start(&Game, HOUSE, Some(1), -1, -1);
    assert!(t.is_active());
    let steps = run(&mut t, &mut f, 0);
    assert_eq!(count(&steps, TransitionStep::FadingOut), 16);
    assert_eq!(steps[15].1, FADE_MAX, "black before the load");
    assert_eq!(steps[16].0, TransitionStep::Load(HOUSE, 2, 0));
    assert_eq!(steps[16].1, FADE_MAX, "load happens on a black screen");
    assert_eq!(count(&steps, TransitionStep::FadingIn), 15);
    assert_eq!(steps.last().unwrap().0, TransitionStep::Done);
    assert_eq!(steps.last().unwrap().1, 0);
    assert_eq!(steps.len(), 16 + 1 + 16);
    assert!(steps.iter().all(|s| !s.2), "player never hidden");
    assert_eq!(t.facing, Dir::Down);
    assert!(!t.is_active());
}

#[test]
fn the_fade_in_waits_on_black_while_the_map_uploads() {
    let mut t = Transition::new();
    let mut f = Fader::new();
    t.start(&Game, HOUSE, Some(1), -1, -1);
    let mut steps = Vec::new();
    // The upload takes three frames after the load frame.
    let mut uploading: u32 = 0;
    for _ in 0..200 {
        let s = t.update_loading(&mut f, false, uploading > 0);
        uploading = match s {
            TransitionStep::Load(m, x, y) => {
                t.arrive(behavior_at(&Game, Game.map(m), x, y), Dir::Up);
                3
            }
            _ => uploading.saturating_sub(1),
        };
        steps.push((s, f.level()));
        if s == TransitionStep::Done {
            break;
        }
    }
    let load = (steps.iter())
        .position(|s| matches!(s.0, TransitionStep::Load(..)))
        .unwrap();
    assert_eq!(
        steps[load + 1..load + 4],
        [(TransitionStep::Loading, FADE_MAX); 3]
    );
    assert_eq!(steps[load + 4].0, TransitionStep::FadingIn);
    assert_eq!(steps.len(), 16 + 1 + 3 + 16);
    assert_eq!(steps.last().unwrap(), &(TransitionStep::Done, 0));
}

#[test]
fn door_destination_walks_out_south_hidden_during_fade_in() {
    let mut t = Transition::new();
    let mut f = Fader::new();
    // House warp 0 is on a door.
    t.start(&Game, HOUSE, Some(0), -1, -1);
    let steps = run(&mut t, &mut f, 16);
    let load = steps
        .iter()
        .position(|s| matches!(s.0, TransitionStep::Load(..)));
    let load = load.unwrap();
    assert_eq!(steps[load].0, TransitionStep::Load(HOUSE, 1, 3));
    let walk = steps
        .iter()
        .position(|s| s.0 == TransitionStep::Walk(Dir::Down))
        .unwrap();
    assert_eq!(walk, load + 16, "walk starts the frame the fade-in ends");
    assert_eq!(steps[walk].1, 0);
    assert!(
        steps[load..walk].iter().all(|s| s.2),
        "hidden while fading in"
    );
    assert!(steps[walk..].iter().all(|s| !s.2));
    assert_eq!(count(&steps, TransitionStep::Walking), 16);
    assert_eq!(steps.len(), walk + 1 + 16 + 1);
}

#[test]
fn stairs_destination_also_walks_out() {
    let mut t = Transition::new();
    // Town (4, 4) is a `NonAnimDoor(Down)` cell (Gen3 stairs, MB_NON_ANIMATED_DOOR).
    t.start(&Game, TOWN, Some(1), -1, -1);
    let steps = run(&mut t, &mut Fader::new(), 1);
    assert!(steps.contains(&(TransitionStep::Load(TOWN, 4, 4), FADE_MAX, true)));
    assert_eq!(count(&steps, TransitionStep::Walk(Dir::Down)), 1);
}

#[test]
fn arrow_warp_destination_faces_away_without_walking() {
    let mut t = Transition::new();
    // House warp 2 is a south arrow warp: arrive facing north.
    t.start(&Game, HOUSE, Some(2), -1, -1);
    let steps = run(&mut t, &mut Fader::new(), 1);
    assert!(
        steps
            .iter()
            .any(|s| s.0 == TransitionStep::Load(HOUSE, 3, 3))
    );
    assert_eq!(t.facing, Dir::Up);
    assert!(!steps.iter().any(|s| matches!(s.0, TransitionStep::Walk(_))));
}

#[test]
fn destination_falls_back_to_xy_then_center() {
    let load = |warp, x, y| {
        let mut t = Transition::new();
        t.start(&Game, TOWN, warp, x, y);
        run(&mut t, &mut Fader::new(), 1)
            .into_iter()
            .find_map(|s| match s.0 {
                TransitionStep::Load(m, x, y) => Some((m, x, y)),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(load(Some(0), 5, 5), (TOWN, 2, 1), "warp wins over xy");
    assert_eq!(load(None, 3, 2), (TOWN, 3, 2));
    assert_eq!(
        load(Some(9), 0, 4),
        (TOWN, 0, 4),
        "invalid warp falls back to xy"
    );
    assert_eq!(load(None, -1, 2), (TOWN, 3, 3), "map center");
}

#[test]
fn door_entry_walks_up_hides_then_fades() {
    let mut t = Transition::new();
    let mut f = Fader::new();
    t.start_door(&Game, HOUSE, Some(1), -1, -1);
    let steps = run(&mut t, &mut f, 16);
    assert_eq!(steps[0], (TransitionStep::Walk(Dir::Up), 0, false));
    assert!(
        steps[1..17]
            .iter()
            .all(|s| s.0 == TransitionStep::Walking && !s.2)
    );
    assert_eq!(
        steps[17],
        (TransitionStep::FadingOut, 1, true),
        "fade starts as walk ends"
    );
    assert!(
        steps
            .iter()
            .any(|s| s.0 == TransitionStep::Load(HOUSE, 2, 0))
    );
    assert!(
        !steps.last().unwrap().2,
        "visible again on a normal destination"
    );
}

#[test]
fn arrive_uses_the_live_behavior_and_defaults_to_plain() {
    let mut t = Transition::new();
    let mut f = Fader::new();
    t.start(&Game, HOUSE, Some(1), -1, -1);
    let mut step = t.update(&mut f, false);
    while step == TransitionStep::FadingOut {
        step = t.update(&mut f, false);
    }
    assert_eq!(step, TransitionStep::Load(HOUSE, 2, 0));
    // The static cell is plain, but a live override made it a door.
    assert_eq!(t.arrive(Behavior::Door, Dir::Up), Dir::Down);
    assert!(t.player_hidden());
    assert_eq!(
        t.arrive(Behavior::ArrowWarp(Dir::Left), Dir::Up),
        Dir::Right
    );
    assert!(!t.player_hidden());
    assert_eq!(t.arrive(Behavior::Door, Dir::Up), Dir::Down);
    let mut last = step;
    while last != TransitionStep::Done && !matches!(last, TransitionStep::Walk(_)) {
        last = t.update(&mut f, false);
    }
    assert_eq!(last, TransitionStep::Walk(Dir::Down));

    // No `arrive` call: plain arrival even onto a static door.
    let mut t = Transition::new();
    t.start(&Game, HOUSE, Some(0), -1, -1);
    let mut n = 0;
    loop {
        let s = t.update(&mut f, false);
        assert!(!matches!(s, TransitionStep::Walk(_)));
        n += 1;
        if s == TransitionStep::Done {
            break;
        }
    }
    assert_eq!(n, 33);
}

#[test]
fn a_blocked_exit_walk_still_finishes() {
    let mut t = Transition::new();
    t.start(&Game, HOUSE, Some(0), -1, -1);
    // The walk never starts moving (e.g. blocked): the next frame is Done.
    let steps = run(&mut t, &mut Fader::new(), 0);
    let n = steps.len();
    assert_eq!(steps[n - 2].0, TransitionStep::Walk(Dir::Down));
    assert_eq!(steps[n - 1].0, TransitionStep::Done);
}

#[test]
fn hash_changes_with_progress_and_is_deterministic() {
    let h = |t: &Transition| {
        let mut h = Fnv64::new();
        t.hash(&mut h);
        h.finish()
    };
    let mut a = Transition::new();
    let idle = h(&a);
    a.start(&Game, HOUSE, Some(0), -1, -1);
    let started = h(&a);
    assert_ne!(idle, started);
    let mut b = Transition::new();
    b.start(&Game, HOUSE, Some(0), -1, -1);
    assert_eq!(started, h(&b));
    let mut f = Fader::new();
    for _ in 0..17 {
        a.update(&mut f, false);
    }
    assert_ne!(h(&a), started);
}

/// Runs a started transition to `Done`, arriving on `behavior` with `warped_facing`. Returns
/// the arrival facing, the exit walk (if any), and whether the player was hidden on the
/// frame before it.
fn arrive_on(behavior: Behavior, warped_facing: Dir) -> (Dir, Option<Dir>, bool) {
    let mut t = Transition::new();
    let mut f = Fader::new();
    t.start(&Game, HOUSE, Some(1), -1, -1);
    let (mut facing, mut walk, mut hidden) = (None, None, false);
    for _ in 0..200 {
        let before = t.player_hidden();
        match t.update(&mut f, false) {
            TransitionStep::Load(..) => facing = Some(t.arrive(behavior, warped_facing)),
            TransitionStep::Walk(d) => (walk, hidden) = (Some(d), before),
            TransitionStep::Done => return (facing.unwrap(), walk, hidden),
            _ => {}
        }
    }
    panic!("transition never finished");
}

/// Gen3 `GetAdjustedInitialDirection` and `SetUpWarpExitTask`, per arrival behavior.
#[test]
fn each_warp_kind_has_its_gen3_arrival() {
    // `Task_ExitDoor`: face south, hidden, walk south.
    assert_eq!(
        arrive_on(Behavior::Door, Dir::Up),
        (Dir::Down, Some(Dir::Down), true)
    );
    // `Task_ExitNonAnimDoor`: stairs face south, `MB_DEEP_SOUTH_WARP` faces north; hidden,
    // then walk the way they face.
    assert_eq!(
        arrive_on(Behavior::NonAnimDoor(Dir::Down), Dir::Up),
        (Dir::Down, Some(Dir::Down), true)
    );
    assert_eq!(
        arrive_on(Behavior::NonAnimDoor(Dir::Up), Dir::Down),
        (Dir::Up, Some(Dir::Up), true)
    );
    // `Task_EscalatorWarpIn`: face and walk east, visible.
    assert_eq!(
        arrive_on(Behavior::Escalator(Dir::Right), Dir::Up),
        (Dir::Right, Some(Dir::Right), false)
    );
    // A ladder keeps the facing the player warped with; no exit walk.
    assert_eq!(
        arrive_on(Behavior::Ladder, Dir::Left),
        (Dir::Left, None, false)
    );
    // Arrow warps face away from their arrow; other warps face south; neither walks.
    assert_eq!(
        arrive_on(Behavior::ArrowWarp(Dir::Up), Dir::Up),
        (Dir::Down, None, false)
    );
    assert_eq!(arrive_on(Behavior::Warp, Dir::Up), (Dir::Down, None, false));
    assert_eq!(
        arrive_on(Behavior::Normal, Dir::Up),
        (Dir::Down, None, false)
    );
}

/// Gen3 `FieldCB_LavaridgeGymB1FWarpExit`: arriving on a pop-out warp, the player is hidden
/// during the fade-in, faces `d`, then jumps (not walks) one cell `d`; the transition finishes
/// once the jump lands, and the jump marker is cleared afterwards.
#[test]
fn a_pop_out_warp_arrival_jumps_out_hidden_during_the_fade_in() {
    let mut t = Transition::new();
    let mut f = Fader::new();
    t.start(&Game, HOUSE, Some(1), -1, -1);
    let (mut facing, mut jump, mut walked) = (None, None, false);
    let mut moving = 0u32;
    for _ in 0..300 {
        let before = t.player_hidden();
        match t.update(&mut f, moving > 0) {
            TransitionStep::Load(..) => {
                facing = Some(t.arrive(Behavior::PopOutWarp(Dir::Right), Dir::Up))
            }
            TransitionStep::Jump(d) => {
                jump = Some((d, before));
                moving = 16;
            }
            TransitionStep::Walk(_) => walked = true,
            TransitionStep::Done => break,
            _ => {}
        }
        moving = moving.saturating_sub(1);
    }
    assert_eq!(facing, Some(Dir::Right));
    assert_eq!(
        jump,
        Some((Dir::Right, true)),
        "jumps east, hidden until then"
    );
    assert!(!walked);
    assert!(!t.is_active());
    assert!(
        !t.exit_jump,
        "cleared once landed: idle hashes are unchanged"
    );
}
