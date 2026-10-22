//! World tests of Milestone 8 Task F (field at scale: view-window spawning, stale-view
//! crossings, warp slots).

use super::*;
use crate::script::WarpSlot;

/// Runs `script` to completion as a normal (not immediate) script.
fn run_script(w: &mut World, script: impl FnOnce(Ctx) -> Script) {
    let mut f = Frame::default();
    let mut runner = Runner::new();
    runner.start(script(runner.ctx()));
    let mut host = Host {
        game: &Game,
        s: &mut w.s,
        f: &mut f,
        immediate: false,
    };
    assert!(!runner.step(&mut host), "the script finished");
}

/// Lets a started warp transition run to its end.
fn finish_warp(w: &mut World) {
    assert!(w.transition_active(), "a warp started");
    for _ in 0..200 {
        if !w.transition_active() {
            return;
        }
        run(w, Buttons::NONE, 1);
    }
    panic!("the warp never ended");
}

#[test]
fn set_warp_stores_the_dynamic_and_escape_slots() {
    let mut w = started();
    run_script(&mut w, |ctx| {
        Box::pin(async move {
            ctx.set_warp(WarpSlot::Dynamic, HOUSE, Some(0), -1, -1)
                .await;
            ctx.set_warp(WarpSlot::Escape, ROUTE, None, 4, 6).await;
        })
    });
    let dynamic = WarpDest {
        map: HOUSE,
        warp: Some(0),
        x: -1,
        y: -1,
    };
    let escape = WarpDest {
        map: ROUTE,
        warp: None,
        x: 4,
        y: 6,
    };
    assert_eq!(
        *w.warp_slots(),
        WarpSlots {
            dynamic: Some(dynamic),
            escape: Some(escape)
        }
    );
}

/// The town, with a warp 0 to a map the game lacks (Gen3 `MAP_DYNAMIC`).
static DYNAMIC_EXIT: MapData = MapData {
    name: "dynamic exit",
    warps: &[WarpEvent {
        x: 1,
        y: 1,
        elevation: 0,
        dest_map: MapId(0x7F7F),
        dest_warp: 0,
    }],
    width: 10,
    height: 10,
    blocks: &TOWN_BLOCKS,
    border: [WALL; 4],
    primary: &TS,
    secondary: &TS2,
    objects: &[],
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: false,
};

#[test]
fn a_warp_to_a_missing_map_uses_the_dynamic_slot() {
    let mut w = started();
    w.start_warp(&Game, &DYNAMIC_EXIT, 0, false);
    assert!(!w.transition_active(), "no dynamic warp set: ignored");
    w.s.warp_slots.dynamic = Some(WarpDest {
        map: ROUTE,
        warp: None,
        x: 4,
        y: 6,
    });
    w.start_warp(&Game, &DYNAMIC_EXIT, 0, false);
    finish_warp(&mut w);
    assert_eq!((w.map(), pos(&w)), (ROUTE, (4, 6)));
}

#[test]
fn old_map_objects_despawn_once_out_of_view_and_respawn_on_return() {
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    assert_eq!((w.map(), pos(&w)), (ROUTE, (5, 9)));
    let town = |w: &World| w.objects().iter().filter(|o| o.map == TOWN).count();
    assert!(town(&w) > 0, "the town's NPCs stay in view");
    // The town's NPC 1 is on (6, 15) in route coordinates: out of the window from y = 5.
    walk(&mut w, Dir::Up, 3);
    assert!(town(&w) > 0);
    walk(&mut w, Dir::Up, 1);
    assert_eq!(town(&w), 0, "despawned by the view window");
    walk(&mut w, Dir::Down, 4);
    run(&mut w, Buttons::DOWN, 1);
    assert_eq!(w.map(), TOWN);
    let npc = w.objects().get(1).expect("respawned from its template");
    assert_eq!(npc.pos(), (6, 5));
}

#[test]
fn a_warp_load_uploads_over_black_frames_then_draws_then_fades_in() {
    let mut w = started();
    w.s.warp_slots.dynamic = Some(WarpDest {
        map: ROUTE,
        warp: None,
        x: 4,
        y: 6,
    });
    w.start_warp(&Game, &DYNAMIC_EXIT, 0, false);
    // Per frame: the map, tiles sent, field cells drawn, brightness.
    let mut frames = Vec::new();
    while w.transition_active() {
        assert!(frames.len() < 200, "the warp never ended");
        let f = run(&mut w, Buttons::NONE, 1);
        let tiles = f.cmds.iter().any(|c| matches!(c, Cmd::BgTiles { .. }));
        let drawn = (f.cmds.iter()).any(|c| {
            matches!(c, Cmd::MapEntries { screen_block, .. }
                if *screen_block != crate::video::layout::SCREEN_BLOCKS[0])
        });
        frames.push((w.map(), tiles, drawn, f.brightness));
    }
    let load = frames.iter().position(|f| f.0 == ROUTE).unwrap();
    let black = crate::flow::FADE_MAX;
    // The load frame sends the route's primary tiles (its secondary has none), the next four
    // its palettes and the view's ten rows, still black; the fade-in follows.
    assert_eq!(frames[load], (ROUTE, true, false, black));
    assert_eq!(frames[load + 1..load + 5], [(ROUTE, false, true, black); 4]);
    assert!(frames[load + 5].3 < black, "then the fade-in");
    assert!(frames[load + 1..].iter().all(|f| !f.1));
}
