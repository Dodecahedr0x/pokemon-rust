//! Golden hashes, one per scenario: they pin the GBA/native parity contract on the synthetic
//! game, so any change to the hashed state, its order or the frame logic shows up here. A
//! change that moves one scenario updates only its constant. Update them (and the README's
//! Tests section) only for an intended change.

use super::battles::{mash, walk_grass_until_battle};
use super::*;
use crate::battle::test_data::WATER;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The new game's first frame, then walking, talking and turning (the pre-Milestone 8 golden).
const GOLDEN_WALK: (u64, u64) = (0x66f3_9427_d1ed_0a8f, 0x0818_a372_6436_0e30);
/// Into the house through its door and back out over the doormat.
const GOLDEN_WARP: u64 = 0x779c_8acf_63d1_6570;
/// North across the connection to the route and back.
const GOLDEN_CROSSING: u64 = 0xc2b0_b5dc_632d_bb70;
/// Talking to the NPC through its whole script.
const GOLDEN_SCRIPT: u64 = 0x0fab_8f3d_8206_46d3;
/// A grass encounter on the route, fought to the end.
const GOLDEN_WILD_BATTLE: u64 = 0xf5f2_70e6_418e_c1ac;
/// A scripted trainer battle, fought to the end.
const GOLDEN_TRAINER: u64 = 0x53fa_55c0_cec2_d61b;

fn walk_scenario() -> (u64, u64) {
    let mut w = World::new(&Game);
    let h0 = w.hash();
    for k in [Buttons::RIGHT, Buttons::A, Buttons::NONE, Buttons::DOWN] {
        run(&mut w, k, 25);
    }
    (h0, w.hash())
}

fn warp_scenario() -> u64 {
    let mut w = started();
    walk(&mut w, Dir::Left, 2);
    walk(&mut w, Dir::Up, 2);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 120);
    assert_eq!(w.map(), HOUSE);
    run(&mut w, Buttons::DOWN, 10);
    run(&mut w, Buttons::NONE, 120);
    assert_eq!(w.map(), TOWN);
    w.hash()
}

fn crossing_scenario() -> u64 {
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    assert_eq!(w.map(), ROUTE);
    run(&mut w, Buttons::DOWN, 9);
    run(&mut w, Buttons::NONE, 16);
    assert_eq!(w.map(), TOWN);
    w.hash()
}

fn script_scenario() -> u64 {
    let mut w = started();
    tap(&mut w, Buttons::RIGHT, 10);
    tap(&mut w, Buttons::A, 4);
    run(&mut w, Buttons::NONE, 20);
    tap(&mut w, Buttons::A, 10);
    tap(&mut w, Buttons::A, 2);
    assert!(!w.script_running());
    w.hash()
}

fn wild_battle_scenario() -> u64 {
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    walk(&mut w, Dir::Up, 6);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
        })
    });
    until_done(&mut w, 3);
    walk_grass_until_battle(&mut w);
    mash(&mut w, 20_000, |w| !w.in_battle());
    w.hash()
}

fn trainer_scenario() -> u64 {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_trainer(TRAINER, TextId(0)).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(w.in_battle());
    mash(&mut w, 20_000, |w| !w.script_running());
    w.hash()
}

#[test]
fn golden_hashes() {
    let got = [
        ("GOLDEN_WALK.0", walk_scenario().0, GOLDEN_WALK.0),
        ("GOLDEN_WALK.1", walk_scenario().1, GOLDEN_WALK.1),
        ("GOLDEN_WARP", warp_scenario(), GOLDEN_WARP),
        ("GOLDEN_CROSSING", crossing_scenario(), GOLDEN_CROSSING),
        ("GOLDEN_SCRIPT", script_scenario(), GOLDEN_SCRIPT),
        (
            "GOLDEN_WILD_BATTLE",
            wild_battle_scenario(),
            GOLDEN_WILD_BATTLE,
        ),
        ("GOLDEN_TRAINER", trainer_scenario(), GOLDEN_TRAINER),
    ];
    let wrong: Vec<String> = (got.iter())
        .filter(|(_, have, want)| have != want)
        .map(|(name, have, want)| format!("{name}: {have:#018x} (pinned {want:#018x})"))
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
