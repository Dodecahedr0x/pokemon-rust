//! Wave C field effects and object commands: `dofieldeffect` / `waitfieldeffect` timing,
//! `setfieldeffectargument` and `copyobjectxytoperm`, on the synthetic game.

use super::*;
use crate::hash::Fnv64;
use crate::checkpoint::RestoreError;
use crate::script::Request;

#[test]
fn wait_field_effect_waits_for_its_frames() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.field_effect(25, 10).await;
            ctx.wait_field_effect(25).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    // Frame 1 starts it; the countdown runs on frames 1..=10, the wait sees it gone on 11.
    run(&mut w, Buttons::NONE, 9);
    assert!(!w.flag(FLAG_DONE));
    assert!(w.s.field_effects.contains(25));
    until_done(&mut w, 5);
    assert!(w.flag(FLAG_DONE));
    assert!(!w.s.field_effects.contains(25));
}

#[test]
fn waiting_on_an_effect_not_running_answers_at_once() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.field_effect(30, 50).await;
            // Another id, and a zero-length start: neither waits.
            ctx.field_effect(31, 0).await;
            ctx.wait_field_effect(31).await;
            ctx.wait_field_effect(7).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    // Each wait checks from the next frame on (Gen3 `ScrCmd_waitfieldeffect` returns TRUE).
    run(&mut w, Buttons::NONE, 3);
    assert!(w.flag(FLAG_DONE));
    // The effect keeps running after the script (Gen3: its sprite outlives the script).
    assert!(w.s.field_effects.contains(30));
    run(&mut w, Buttons::NONE, 48);
    assert!(!w.s.field_effects.contains(30));
}

#[test]
fn field_effect_arguments_are_stored_and_out_of_range_ones_ignored() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.field_effect_arg(0, 12).await;
            ctx.field_effect_arg(2, -1).await;
            ctx.field_effect_arg(8, 99).await;
        })
    });
    until_done(&mut w, 5);
    assert_eq!(w.s.field_effects.args[..3], [12, 0, -1]);
}

#[test]
fn copy_object_pos_to_template_keeps_the_live_cell() {
    let mut w = started();
    assert_eq!(w.objects().get(2).unwrap().pos(), (7, 7));
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.set_object_pos(2, 4, 9).await;
            ctx.copy_object_pos_to_template(2).await;
            // No object 0x30 and the player have no template: nothing happens.
            ctx.copy_object_pos_to_template(0x30).await;
            ctx.copy_object_pos_to_template(PLAYER).await;
        })
    });
    until_done(&mut w, 5);
    let t = w.s.template(2).unwrap();
    assert_eq!((t.x, t.y), (4, 9));
}

#[test]
fn field_requests_hash_apart() {
    let h = |r: Request| {
        let mut h = Fnv64::new();
        r.hash_into(&mut h);
        h.finish()
    };
    let all = [
        h(Request::FieldEffectArg { n: 0, value: 1 }),
        h(Request::FieldEffect { id: 1, frames: 0 }),
        h(Request::WaitFieldEffect(1)),
        h(Request::CopyObjectPosToTemplate(1)),
        h(Request::SetMapLayout(1)),
        h(Request::CreateVirtualObject {
            gfx: 1,
            id: 1,
            x: 0,
            y: 0,
            elevation: 0,
            dir: Dir::Down,
        }),
        h(Request::CreateVirtualObject {
            gfx: 1,
            id: 1,
            x: 0,
            y: 0,
            elevation: 0,
            dir: Dir::Up,
        }),
        h(Request::TurnVirtualObject {
            id: 1,
            dir: Dir::Down,
        }),
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

/// The house's layout number of [`LGame`]'s variant.
const HOUSE_ALT_LAYOUT: u16 = 5;
/// The block of every cell of the variant.
const ALT_BLOCK: u16 = GRASS;
/// [`LGame`]'s house `on_transition` script: switches to [`HOUSE_ALT_LAYOUT`] while
/// `VAR_HOUSE` is 1.
const HOUSE_TRANSITION: ScriptId = ScriptId(900);

/// [`Game`] whose house switches layout in `on_transition` (Gen3 `setmaplayoutindex`).
struct LGame;

/// `HOUSE` with `blocks` and `on_transition`, leaked.
fn house_with(blocks: &'static [u16], on_transition: Option<ScriptId>) -> &'static MapData {
    let h = Game.map(HOUSE);
    Box::leak(Box::new(MapData {
        name: h.name,
        width: h.width,
        height: h.height,
        blocks,
        border: h.border,
        primary: h.primary,
        secondary: h.secondary,
        objects: h.objects,
        warps: h.warps,
        coords: h.coords,
        bgs: h.bgs,
        connections: h.connections,
        scripts: MapScripts {
            on_load: h.scripts.on_load,
            on_transition,
            on_resume: h.scripts.on_resume,
            on_return_to_field: h.scripts.on_return_to_field,
            on_frame: h.scripts.on_frame,
            on_warp_into: h.scripts.on_warp_into,
        },
        allow_running: h.allow_running,
        map_type: h.map_type,
    }))
}

std::thread_local! {
    static HOUSE_L: &'static MapData =
        house_with(Game.map(HOUSE).blocks, Some(HOUSE_TRANSITION));
    static HOUSE_ALT: &'static MapData = {
        let n = Game.map(HOUSE).blocks.len();
        house_with(Box::leak(alloc::vec![ALT_BLOCK; n].into_boxed_slice()), None)
    };
}

impl GameData for LGame {
    fn map(&self, id: MapId) -> &'static MapData {
        match id {
            HOUSE => HOUSE_L.with(|m| *m),
            _ => Game.map(id),
        }
    }
    fn map_layout(&self, id: MapId, layout: u16) -> Option<&'static MapData> {
        (id == HOUSE && layout == HOUSE_ALT_LAYOUT).then(|| HOUSE_ALT.with(|m| *m))
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
        match id {
            HOUSE_TRANSITION => Box::pin(async move {
                if ctx.var(VAR_HOUSE).await == 1 {
                    ctx.set_map_layout(HOUSE_ALT_LAYOUT).await;
                }
            }),
            _ => Game.script(id, ctx),
        }
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
}

fn run_l(w: &mut World, frames: u32) {
    for _ in 0..frames {
        w.step(&LGame, Buttons::NONE).validate().unwrap();
    }
}

/// Warps to `map` by a script and lets the load finish.
fn warp_l(w: &mut World, map: MapId) {
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.warp(map, Some(0), -1, -1).await;
    }));
    run_l(w, 120);
    assert_eq!(w.s.field.map(), map);
}

#[test]
fn set_map_layout_in_on_transition_loads_the_variant_until_the_next_map() {
    let mut w = World::new(&LGame);
    run_l(&mut w, 16);
    // VAR_HOUSE 0: the house keeps its own layout.
    warp_l(&mut w, HOUSE);
    assert_eq!(w.s.field.layout(), None);
    assert_ne!(w.s.field.block(&LGame, 1, 1), ALT_BLOCK);
    // VAR_HOUSE 1: on_transition switches it.
    w.s.vars.set_var(VAR_HOUSE, 1);
    warp_l(&mut w, TOWN);
    assert_eq!(w.s.field.layout(), None, "the town has its own");
    warp_l(&mut w, HOUSE);
    assert_eq!(w.s.field.layout(), Some(HOUSE_ALT_LAYOUT));
    assert_eq!(w.s.field.block(&LGame, 1, 1), ALT_BLOCK);
    // A battle's return to the field keeps it.
    w.s.field.restore_spread();
    assert_eq!(w.s.field.layout(), Some(HOUSE_ALT_LAYOUT));
    run_l(&mut w, 10);
    assert_eq!(w.s.field.block(&LGame, 1, 1), ALT_BLOCK);
    // Leaving resets it (Gen3 `LoadCurrentMapData`).
    w.s.vars.set_var(VAR_HOUSE, 0);
    warp_l(&mut w, TOWN);
    assert_eq!(w.s.field.layout(), None);
}

/// Wave F (checkpoint schema 6): a capture on a map under a layout override keeps it, the
/// restored world walks the variant and hashes the same, and the hash tells the override apart.
#[test]
fn a_layout_override_survives_a_checkpoint_and_is_hashed() {
    let mut w = World::new(&LGame);
    run_l(&mut w, 16);
    w.s.vars.set_var(VAR_HOUSE, 1);
    warp_l(&mut w, HOUSE);
    run_l(&mut w, 30);
    let cp = w.checkpoint(&LGame).expect("capturable");
    assert_eq!(cp.world.map_layout, Some(HOUSE_ALT_LAYOUT));
    let back = World::restore(&LGame, &cp).expect("restores");
    assert_eq!(back.s.field.layout(), Some(HOUSE_ALT_LAYOUT));
    assert_eq!(back.s.field.block(&LGame, 1, 1), ALT_BLOCK);
    assert_eq!(back.hash(), w.hash());
    // The same world on the house's own layout hashes differently.
    let mut own = cp.clone();
    own.world.map_layout = None;
    own.header.build.engine ^= 1; // another build: the header's hash is not checked
    let own = World::restore(&LGame, &own).expect("restores");
    assert_ne!(own.hash(), w.hash());
    // A layout the map does not have is refused.
    let mut bad = cp;
    bad.world.map_layout = Some(HOUSE_ALT_LAYOUT + 1);
    assert!(matches!(
        World::restore(&LGame, &bad),
        Err(RestoreError::Invalid(_))
    ));
}

#[test]
fn virtual_objects_are_drawn_turned_and_gone_after_a_warp() {
    let mut w = started();
    let objs = |f: &Frame| f.objs.len();
    let before = objs(&run(&mut w, Buttons::NONE, 1));
    let occupant = w.objects().at(6, 5, 3);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.create_virtual_object(NPC_GFX.0, 7, (6, 5), 3, Dir::Left)
                .await;
            // A graphics id the game has no sprite for is kept but not drawn.
            ctx.create_virtual_object(0x7F, 8, (4, 5), 3, Dir::Down)
                .await;
        })
    });
    let f = run(&mut w, Buttons::NONE, 2);
    assert_eq!(objs(&f), before + 1);
    let v = w.objects().virtuals();
    assert_eq!(v.len(), 2);
    assert_eq!((v[0].id, v[0].x, v[0].y, v[0].facing), (7, 6, 5, Dir::Left));
    // No collision: the cell's occupant is unchanged.
    assert_eq!(w.objects().at(6, 5, 3), occupant);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.turn_virtual_object(7, Dir::Right).await;
            ctx.turn_virtual_object(99, Dir::Up).await;
        })
    });
    run(&mut w, Buttons::NONE, 2);
    assert_eq!(w.objects().virtuals()[0].facing, Dir::Right);
    // A warp load clears them (Gen3 resets the sprites).
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.warp(HOUSE, Some(0), -1, -1).await;
        })
    });
    run(&mut w, Buttons::NONE, 120);
    assert!(w.objects().virtuals().is_empty());
}

/// Gen3 `applymovementat` / `waitmovementat` / `hideobjectat` name the object's map: after a
/// crossing they reach the previous map's object, which the plain forms (the current map's)
/// do not (Mr. Briney's boat on Route 105 is Route 104's).
#[test]
fn at_forms_reach_the_previous_maps_object() {
    static LEFT: [Step; 1] = [Step::Walk(Dir::Left, Speed::Normal)];
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    assert_eq!(w.map(), ROUTE);
    let npc = |w: &World, id: u8| {
        w.objects()
            .iter()
            .find(|o| o.map == TOWN && o.local_id == id)
            .map(|o| (o.pos(), o.visible))
    };
    let id = (w.objects().iter())
        .find(|o| o.map == TOWN && o.local_id != PLAYER)
        .unwrap()
        .local_id;
    let ((x, y), _) = npc(&w, id).unwrap();
    start(&mut w, move |ctx| {
        Box::pin(async move {
            // The plain forms look on the route: nothing moves, nothing to wait for.
            ctx.move_obj(id, &LEFT).await;
            ctx.wait_move(id).await;
            ctx.move_obj_at(id, TOWN, &LEFT).await;
            ctx.wait_move_at(id, TOWN).await;
            ctx.hide_object_at(id, TOWN).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    until_done(&mut w, 60);
    assert!(w.flag(FLAG_DONE));
    assert_eq!(npc(&w, id), Some(((x - 1, y), false)));
}

/// Gen3 `CameraMove`: the camera follows the player's sprite, so a scripted walk across a map
/// edge switches maps (`LoadMapFromCameraTransition`) as an input step does (Mr. Briney's
/// sails, `Route104_EventScript_SailToDewford`), and the state after it restores.
#[test]
fn a_scripted_walk_across_a_connection_switches_maps() {
    static UP: [Step; 8] = [Step::Walk(Dir::Up, Speed::Normal); 8];
    let mut w = started();
    walk(&mut w, Dir::Up, 3);
    assert_eq!((w.map(), pos(&w)), (TOWN, (5, 2)));
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.move_obj(PLAYER, &UP).await;
            // The route's `on_transition` waits on its object 1, which Gen3 keeps as the last
            // mover (`sMovingNpcId`): wait on the player by id.
            ctx.wait_move(PLAYER).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    let mut crossed = false;
    for _ in 0..400 {
        run(&mut w, Buttons::NONE, 1);
        if w.map() == ROUTE && !crossed {
            crossed = true;
            assert_eq!(pos(&w), (5, 9), "switched as the step starts");
        }
        if w.flag(FLAG_DONE) {
            break;
        }
    }
    assert!(crossed && w.flag(FLAG_DONE));
    assert_eq!((w.map(), pos(&w)), (ROUTE, (5, 4)));
    run(&mut w, Buttons::NONE, 2);
    let cp = w.checkpoint(&Game).expect("capturable after the walk");
    World::restore(&Game, &cp).expect("restores");
}
