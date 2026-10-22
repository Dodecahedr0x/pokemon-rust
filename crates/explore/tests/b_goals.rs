//! Task B: the goal list along the parity route (both flavors).

use content::Emerald;
use engine::checkpoint::WorldSnapshot;
use explore::goals::{self, GoalCtx};
use explore::model::{BattleSummary, SimSummary};
use game_native::Session;
use game_native::scenario::cached_route;

/// Replays the route node by node; each node's progress chains from the previous one, with the
/// summary of the frames in between (maps entered, cells stood on, battles won or lost).
fn route_progress() -> Vec<(&'static str, u32)> {
    let rec = cached_route();
    let game = Emerald::for_shells();
    let mut s = Session::new(game);
    let mut out = Vec::new();
    let mut progress = 0;
    let mut start = 0u32;
    for &(name, frame) in &rec.nodes {
        let mut sum = SimSummary::default();
        let mut outcome = None;
        for i in start..frame {
            s.step(rec.keys_at(i));
            let w = s.world();
            if sum.maps.last() != Some(&w.map()) {
                sum.maps.push(w.map());
            }
            let p = w.objects().player();
            sum.cells.insert((w.map(), p.x, p.y));
            match w.battle() {
                Some((_, _, scene)) => outcome = scene.finished().map(|o| o as u8).or(outcome),
                None if outcome.is_some() => sum.battles.push(BattleSummary {
                    kind: 0,
                    trainer: None,
                    outcome: outcome.take(),
                    frames: 0,
                    player_level: 0,
                    opponent_level: 0,
                    hp_lost_permille: 0,
                }),
                None => {}
            }
        }
        assert_eq!(s.world().frame(), frame);
        let snap: WorldSnapshot = s.world().snapshot(s.game()).expect("nodes are capturable");
        let next = goals::progress(
            progress,
            &GoalCtx {
                snap: &snap,
                summary: &sum,
            },
        );
        assert!(next >= progress, "progress is monotone");
        progress = next;
        out.push((name, progress));
        start = frame;
    }
    out
}

#[test]
fn the_route_reaches_every_goal_in_order() {
    let got = route_progress();
    let all = goals::goals().len() as u32;
    if content::STUB {
        assert_eq!(all, 3);
        assert_eq!(
            got,
            [("stub_start", 0), ("stub_after_battle", 2), ("stub_end", 3)]
        );
    } else {
        assert_eq!(all, 7);
        assert_eq!(
            got,
            [
                ("lab", 4),
                ("after_wild", 4),
                ("oldale_center", 5),
                ("route103_after_may", 7)
            ]
        );
    }
}

#[test]
fn goal_names_follow_the_story_order() {
    let names: Vec<&str> = goals::goals().iter().map(|g| g.name).collect();
    if content::STUB {
        assert_eq!(names, ["wild_won", "east", "far_corner"]);
    } else {
        assert_eq!(
            names,
            [
                "rescue_started",
                "starter",
                "birch_rescued",
                "lab",
                "oldale",
                "route103",
                "may_defeated"
            ]
        );
    }
    assert_eq!(goals::name(0), Some(names[0]));
    assert_eq!(goals::name(names.len() as u32), None);
}

#[test]
fn progress_needs_the_earlier_goals_first() {
    // A new game's settled state satisfies no first goal; a later goal alone never counts.
    let mut s = Session::new(Emerald::for_shells());
    while s.world().capturable().is_err() {
        s.step(engine::input::Buttons::NONE);
    }
    let snap = s.world().snapshot(s.game()).unwrap();
    let sum = SimSummary::default();
    let ctx = GoalCtx {
        snap: &snap,
        summary: &sum,
    };
    assert_eq!(goals::progress(0, &ctx), 0);
    let all = goals::goals().len() as u32;
    assert_eq!(goals::progress(all, &ctx), all, "past the end stays put");
}

#[test]
fn a_later_goal_alone_does_not_advance() {
    // A state satisfying goal 2 but not goal 1: progress stays at 0 from 0, and goes to 2 from 1.
    let settle = |game: Emerald| {
        let mut s = Session::new(game);
        while s.world().capturable().is_err() {
            s.step(engine::input::Buttons::NONE);
        }
        s.world().snapshot(s.game()).unwrap()
    };
    let mut snap = settle(Emerald::for_shells());
    if content::STUB {
        // Goal 2 (`east`) without goal 1 (`wild_won`, no battle) or goal 3 (`far_corner`).
        snap.objects.objs[0].pos = (12, 5);
    } else {
        // Goal 2 (`starter`) without goal 1 (`rescue_started`, still in Littleroot) or goal 3.
        assert!(snap.party.is_empty());
        snap.party = settle(Emerald::after_starter()).party;
        assert!(!snap.party.is_empty());
    }
    let sum = SimSummary::default();
    let ctx = GoalCtx {
        snap: &snap,
        summary: &sum,
    };
    assert!(!(goals::goals()[0].reached)(&ctx));
    assert!((goals::goals()[1].reached)(&ctx));
    assert_eq!(goals::progress(0, &ctx), 0);
    assert_eq!(goals::progress(1, &ctx), 2);
}
