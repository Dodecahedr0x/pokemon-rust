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

/// The town, with a warp 0 to `MAP_DYNAMIC` and a warp 1 to a map the game lacks.
static DYNAMIC_EXIT: MapData = MapData {
    name: "dynamic exit",
    warps: &[
        WarpEvent {
            x: 1,
            y: 1,
            elevation: 0,
            dest_map: crate::grid::MAP_DYNAMIC,
            dest_warp: 0,
        },
        WarpEvent {
            x: 2,
            y: 1,
            elevation: 0,
            dest_map: MapId(0x6000),
            dest_warp: 0,
        },
    ],
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
fn a_map_dynamic_warp_uses_the_dynamic_slot_and_a_missing_map_is_ignored() {
    let mut w = started();
    w.start_warp(&Game, &DYNAMIC_EXIT, 0, false);
    assert!(!w.transition_active(), "no dynamic warp set: ignored");
    w.s.warp_slots.dynamic = Some(WarpDest {
        map: ROUTE,
        warp: None,
        x: 4,
        y: 6,
    });
    w.start_warp(&Game, &DYNAMIC_EXIT, 1, false);
    assert!(!w.transition_active(), "a missing map is not dynamic");
    w.start_warp(&Game, &DYNAMIC_EXIT, 0, false);
    finish_warp(&mut w);
    assert_eq!((w.map(), pos(&w)), (ROUTE, (4, 6)));
}

#[test]
fn the_view_window_follows_a_scripted_walk_from_each_step_start() {
    static UP: [Step; 4] = [Step::Walk(Dir::Up, Speed::Normal); 4];
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    assert_eq!((w.map(), pos(&w)), (ROUTE, (5, 9)));
    let town = |w: &World| w.objects().iter().filter(|o| o.map == TOWN).count();
    assert!(town(&w) > 0);
    super::start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.move_obj(PLAYER, &UP).await;
            ctx.wait_move(0).await;
        })
    });
    // The town's NPC 1 leaves the window as the player enters (5, 5): the frame that step
    // starts, not when it ends.
    for _ in 0..200 {
        run(&mut w, Buttons::NONE, 1);
        if pos(&w).1 == 5 {
            break;
        }
        assert!(town(&w) > 0, "still in view at {:?}", pos(&w));
    }
    assert_eq!(pos(&w), (5, 5));
    assert!(!w.objects().player().is_settled(), "mid-step");
    assert_eq!(town(&w), 0, "despawned by the scripted walk");
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

#[test]
fn restore_refuses_more_objects_than_the_cap() {
    let w = started();
    let good = w.checkpoint(&Game).unwrap();
    let npc = good.world.objects.objs[1];
    let mut cp = good.clone();
    let full = crate::objects::OBJECT_EVENTS_COUNT;
    while cp.world.objects.objs.len() < full + 1 {
        cp.world.objects.objs.push(npc);
    }
    assert_eq!(
        World::restore(&Game, &cp).err(),
        Some(crate::checkpoint::RestoreError::Invalid(
            "more objects than OBJECT_EVENTS_COUNT"
        ))
    );
    assert!(World::restore(&Game, &good).is_ok());
}

/// A secondary tileset only `Painted`'s route uses: more tiles than a visible frame sends.
static TS3: Tileset = Tileset {
    tiles: &[[0x5555_5555; 8]; 300],
    palettes: &[[0x4321; 16]; 7],
    metatiles: &[],
    attributes: &[],
};

static PAINTED_ROUTE: MapData = MapData {
    name: "painted route",
    width: 10,
    height: 10,
    blocks: &ROUTE_BLOCKS,
    border: [ROUTE_BORDER; 4],
    primary: &TS,
    secondary: &TS3,
    objects: &[],
    warps: &[],
    coords: &[],
    bgs: &[],
    connections: &[Connection {
        dir: Dir::Down,
        offset: 0,
        map: TOWN,
    }],
    scripts: NO_SCRIPTS,
    allow_running: true,
};

/// The test game, with a route whose secondary tileset differs from the town's.
struct Painted;

impl GameData for Painted {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == ROUTE {
            &PAINTED_ROUTE
        } else {
            Game.map(id)
        }
    }
    fn special_ids(&self) -> SpecialIds {
        Game.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        Game.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        Game.player_sprite()
    }
    fn font(&self) -> &'static Font {
        Game.font()
    }
    fn window(&self) -> &'static WindowGfx {
        Game.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        Game.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Game.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        Game.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        Game.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static crate::mon::TrainerData> {
        Game.trainer(id)
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        Game.heal_location(id)
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
}

#[test]
fn capture_is_refused_while_a_crossing_uploads_other_tilesets() {
    let mut w = World::new(&Painted);
    let step = |w: &mut World, keys| {
        let f = w.step(&Painted, keys);
        f.validate().unwrap();
    };
    for _ in 0..16 {
        step(&mut w, Buttons::NONE);
    }
    // Up from (5, 5) to the town's top row, then across into the route.
    let mut refused = 0;
    for _ in 0..200 {
        step(&mut w, Buttons::UP);
        if w.s.field.uploading() {
            assert_eq!(w.map(), ROUTE);
            assert_eq!(
                w.capturable(),
                Err(crate::checkpoint::NotCapturable::StaleView)
            );
            refused += 1;
        } else if w.map() == ROUTE {
            break;
        }
    }
    assert_eq!(w.map(), ROUTE);
    assert!(refused > 1, "the upload spans frames ({refused})");
    // Once the player stands still, capture works again.
    for _ in 0..32 {
        step(&mut w, Buttons::NONE);
    }
    assert_eq!(w.capturable(), Ok(()));
}
