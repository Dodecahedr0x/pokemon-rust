//! Task B, Milestone 8 Task K: the goal lists along the parity route (both flavors). Goals are
//! state predicates. Emerald's and the stub's acts only: not built on Silice
//! (`cfg(pkr_silice)`; `sil1_d_flavor.rs` checks the acts of every flavour).
#![cfg(not(pkr_silice))]

use content::Emerald;
use explore::goals::{self, Act};
use game_native::Session;
use game_native::scenario::cached_route;

/// Replays the route; the progress chains over every capturable frame where the player's map
/// or cell changed (as the explorer's nodes end after each walk, entry or crossing), and is
/// read at each route node.
fn route_progress(act: Act) -> Vec<(&'static str, u32)> {
    let rec = cached_route();
    let game = Emerald::for_shells();
    let mut s = Session::new(game);
    let mut out = Vec::new();
    let mut progress = 0;
    let mut last = None;
    let mut frame = 0u32;
    for &(name, node) in &rec.nodes {
        while frame < node {
            s.step(rec.keys_at(frame));
            frame += 1;
            let w = s.world();
            let p = w.objects().player();
            let here = (w.map(), p.x, p.y);
            if (last != Some(here) || frame == node)
                && let Ok(snap) = w.snapshot(s.game())
            {
                last = Some(here);
                let next = goals::progress(act, progress, &snap);
                assert!(next >= progress, "progress is monotone");
                progress = next;
            }
        }
        assert!(s.world().capturable().is_ok(), "nodes are capturable");
        out.push((name, progress));
    }
    out
}

#[test]
fn the_route_reaches_every_slice_goal_in_order() {
    // The acts that start at the new game (Act 2 and on start at the previous act's badge).
    for act in [Act::Slice, Act::Act1] {
        let got = route_progress(act);
        if content::STUB {
            assert_eq!(goals::goals_of(act).len(), 3);
            assert_eq!(
                got,
                [("stub_start", 0), ("stub_after_battle", 2), ("stub_end", 3)]
            );
        } else {
            // The route ends after May (the slice's 7 goals); Act 1 goes on from there.
            assert_eq!(
                got,
                [
                    ("lab", 4),
                    ("after_wild", 4),
                    ("oldale_center", 5),
                    ("route103_after_may", 7)
                ],
                "{act:?}"
            );
        }
    }
}

#[test]
fn goal_names_follow_the_story_order() {
    let slice: Vec<&str> = goals::goals_of(Act::Slice).iter().map(|g| g.name).collect();
    let act1: Vec<&str> = goals::goals_of(Act::Act1).iter().map(|g| g.name).collect();
    if content::STUB {
        assert_eq!(slice, ["wild_won", "east", "far_corner"]);
        assert_eq!(act1, slice, "the stub has no story: one list");
    } else {
        assert_eq!(
            slice,
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
        assert_eq!(act1[..7], slice[..], "Act 1 begins with the slice");
        assert_eq!(
            act1[7..],
            [
                "pokedex",
                "petalburg",
                "wally_tutorial",
                "route104",
                "woods_grunt_defeated",
                "rustboro",
                "roxanne_defeated",
                "badge1"
            ]
        );
    }
    // The default act is Act 1; `name` follows it.
    assert_eq!(Act::default(), Act::Act1);
    assert_eq!(goals::goals().len(), act1.len());
    assert_eq!(goals::name(0), Some(act1[0]));
    assert_eq!(goals::name(act1.len() as u32), None);
    assert_eq!(goals::name_of(Act::Slice, slice.len() as u32), None);
    for act in Act::ALL {
        assert_eq!(Act::parse(act.name()), Some(act));
    }
    assert_eq!(Act::parse("act99"), None);
    // Task S: the spine's later acts, each from the previous act's badge.
    if !content::STUB {
        let names = |a| {
            goals::goals_of(a)
                .iter()
                .map(|g| g.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(Act::Act2).last(), Some(&"badge2"));
        assert_eq!(names(Act::Act3).last(), Some(&"badge3"));
        assert_eq!(names(Act::Act4).last(), Some(&"badge4"));
    }
    // SIL1 Wave 0: Silice's Prologue act exists, with no goal yet (the stub keeps its list).
    assert_eq!(Act::parse("sil_ch0"), Some(Act::SilCh0));
    assert_eq!(Act::SILICE, [Act::SilCh0]);
    if !content::STUB {
        assert!(goals::goals_of(Act::SilCh0).is_empty());
    }
}

fn settle(game: Emerald) -> engine::checkpoint::WorldSnapshot {
    let mut s = Session::new(game);
    game_native::scenario::to_field(&mut s).unwrap_or_else(|e| panic!("{e}"));
    s.world().snapshot(s.game()).unwrap()
}

#[test]
fn progress_needs_the_earlier_goals_first() {
    // A new game's settled state satisfies no first goal.
    let snap = settle(Emerald::for_shells());
    for act in Act::EMERALD {
        assert_eq!(goals::progress(act, 0, &snap), 0);
        let all = goals::goals_of(act).len() as u32;
        assert_eq!(
            goals::progress(act, all, &snap),
            all,
            "past the end stays put"
        );
    }
}

#[test]
fn a_later_goal_alone_does_not_advance() {
    // A state satisfying goal 2 but not goal 1: progress stays at 0 from 0, and goes to 2 from 1.
    let mut snap = settle(Emerald::for_shells());
    if content::STUB {
        // Goal 2 (`east`) without goal 1 (`wild_won`: the lead's exp is its new-game exp) or
        // goal 3 (`far_corner`).
        snap.objects.objs[0].pos = (12, 5);
    } else {
        // Goal 2 (`starter`) without goal 1 (`rescue_started`, still in Littleroot) or goal 3.
        assert!(snap.party.is_empty());
        snap.party = settle(Emerald::after_starter()).party;
        assert!(!snap.party.is_empty());
    }
    let list = goals::goals();
    assert!(!(list[0].reached)(&snap));
    assert!((list[1].reached)(&snap));
    assert_eq!(goals::progress(Act::Act1, 0, &snap), 0);
    assert_eq!(goals::progress(Act::Act1, 1, &snap), 2);
}

#[test]
fn the_stub_battle_goal_is_the_lead_exp() {
    if !content::STUB {
        return;
    }
    let mut snap = settle(Emerald::for_shells());
    let won = goals::goals_of(Act::Act1)[0].reached;
    assert!(!won(&snap), "the new game's exp");
    snap.party[0].exp += 1;
    assert!(won(&snap), "more exp: a battle was won");
    snap.party.clear();
    assert!(!won(&snap), "no lead");
}
