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
const GOLDEN_WALK: (u64, u64) = (0xa606_3af7_4c9c_3e21, 0x2d6a_1f1f_392d_3f76);
/// Into the house through its door and back out over the doormat.
const GOLDEN_WARP: u64 = 0x22aa_44b8_1c6d_aa00;
/// North across the connection to the route and back.
const GOLDEN_CROSSING: u64 = 0x0e29_9c5c_dc99_2190;
/// Talking to the NPC through its whole script.
const GOLDEN_SCRIPT: u64 = 0x725f_3659_70f0_84a5;
/// A grass encounter on the route, fought to the end (Wave E: `VAR_DAILY_WILDS` counts it).
const GOLDEN_WILD_BATTLE: u64 = 0x7d4f_2260_f4ea_638e;
/// A scripted trainer battle, fought to the end.
const GOLDEN_TRAINER: u64 = 0x8219_1acc_6865_5331;
/// START and SELECT on the idle field (Milestone 9; Tasks K and SM move it).
const GOLDEN_SCREEN: u64 = 0xc5c6_a95e_c6c4_4077;
/// A script's save dialog (Milestone 9; Task S moves it).
const GOLDEN_SAVE: u64 = 0x0232_08c7_a702_2b1a;

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

/// START, then SELECT, on the idle field (Milestone 9: the start menu; Wave 0 opens nothing).
fn screen_scenario() -> u64 {
    let mut w = started();
    tap(&mut w, Buttons::START, 10);
    tap(&mut w, Buttons::SELECT, 10);
    assert_eq!(w.screens_open(), 0);
    w.hash()
}

/// A script's save dialog (`special SaveGame`, Task S): B through it answers "not saved".
fn save_scenario() -> u64 {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            let saved = ctx.save_dialog().await;
            ctx.set_var(VAR_RESULT, saved as u16 + 10).await;
        })
    });
    for _ in 0..600 {
        if !w.script_running() {
            break;
        }
        tap(&mut w, Buttons::B, 2);
    }
    assert!(!w.script_running());
    assert_eq!(w.var(VAR_RESULT), 10);
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
        ("GOLDEN_SCREEN", screen_scenario(), GOLDEN_SCREEN),
        ("GOLDEN_SAVE", save_scenario(), GOLDEN_SAVE),
    ];
    let wrong: Vec<String> = (got.iter())
        .filter(|(_, have, want)| have != want)
        .map(|(name, have, want)| format!("{name}: {have:#018x} (pinned {want:#018x})"))
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
