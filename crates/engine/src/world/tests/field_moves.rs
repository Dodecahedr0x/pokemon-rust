//! Wave C field moves on a synthetic lake: the town's south half is ocean water (elevation 1)
//! below land (elevation 3). Surf from the A press (`GetInteractedWaterScript`, the badge and
//! the party move), its effect's timing and the jump onto the water, fast steps on water, the
//! dismount (`CanStopSurfing`), water encounters, surfing kept by checkpoints; Strength's
//! boulder push (`TryPushBoulder`); a Cut-like effect that `waitstate` waits for.

use super::super::field_move::{POSE_FRAMES, SHOW_MON_OUTDOORS};
use super::*;
use crate::battle::Environment;
use crate::battle::test_data::{EMBER, MUDKIP, SPARK, WATER, ZIGZAGOON};
use crate::checkpoint::{Checkpoint, NotCapturable};
use crate::mon::{WaterEncounters, WildMon};
use crate::wild;

const LAND: u16 = FLOOR | 3 << BLOCK_ELEVATION_SHIFT;
/// The grass metatile (raw behaviour 4), as ocean water here.
const SEA: u16 = 4 | 1 << BLOCK_ELEVATION_SHIFT;

/// A 10x10 lake, walled round: rows 1–4 land, rows 5–8 water.
const fn lake_blocks() -> [u16; 100] {
    let mut b = [LAND; 100];
    let mut i = 0;
    while i < 10 {
        b[i] = WALL;
        b[90 + i] = WALL;
        b[i * 10] = WALL;
        b[i * 10 + 9] = WALL;
        i += 1;
    }
    let mut y = 5;
    while y < 9 {
        let mut x = 1;
        while x < 9 {
            b[y * 10 + x] = SEA;
            x += 1;
        }
        y += 1;
    }
    b
}
static LAKE_BLOCKS: [u16; 100] = lake_blocks();

const BOULDER_GFX: GfxId = NPC_GFX;
const FLAG_STRENGTH: FlagId = FlagId(0x880);
const FLAG_BADGE: FlagId = FlagId(0x867);
const FLAG_CUT_DONE: FlagId = FlagId(0x303);
/// The move the gate asks for (any move of the test tables).
const SURF_MOVE: MoveId = EMBER;
const SURF_SCRIPT: ScriptId = ScriptId(70);
const CUT_SCRIPT: ScriptId = ScriptId(71);

static LAKE_OBJECTS: [ObjectEvent; 1] = [{
    let mut o = scriptless(1, 6, 2);
    o.gfx = Gfx::Fixed(BOULDER_GFX);
    o
}];

static LAKE: MapData = MapData {
    name: "lake",
    width: 10,
    height: 10,
    blocks: &LAKE_BLOCKS,
    border: [WALL; 4],
    primary: &TS,
    secondary: &TS2,
    objects: &LAKE_OBJECTS,
    warps: &[],
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: true,
    map_type: crate::data::map_type::TOWN,
};

const fn mon(species: crate::data::SpeciesId, min_level: u8, max_level: u8) -> WildMon {
    WildMon {
        species,
        min_level,
        max_level,
    }
}

static LAKE_WATER: WaterEncounters = WaterEncounters {
    rate: 20,
    mons: [
        mon(ZIGZAGOON, 5, 10),
        mon(MUDKIP, 10, 20),
        mon(SPARK, 15, 20),
        mon(ZIGZAGOON, 20, 25),
        mon(MUDKIP, 25, 30),
    ],
};

/// [`Game`] with the lake as the town.
struct Lake;

std::thread_local! {
    /// Whether the lake has its water table (tests that walk the water without meeting
    /// anything turn it off).
    static WATER_TABLE: core::cell::Cell<bool> = const { core::cell::Cell::new(true) };
}

async fn use_surf(ctx: Ctx) {
    // `EventScript_UseSurf` past its yes/no: `dofieldeffect FLDEFF_USE_SURF`, `end`.
    ctx.field_move_effect(FieldMove::Surf).await;
}

async fn use_cut(ctx: Ctx) {
    ctx.field_move_effect(FieldMove::Cut).await;
    ctx.wait_state().await;
    ctx.set_flag(FLAG_CUT_DONE, true).await;
}

impl GameData for Lake {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == TOWN { &LAKE } else { Game.map(id) }
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
        match raw {
            4 => Behavior::Water(Water::Ocean),
            _ => Game.behavior(raw),
        }
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        match id {
            SURF_SCRIPT => Box::pin(use_surf(ctx)),
            CUT_SCRIPT => Box::pin(use_cut(ctx)),
            _ => Game.script(id, ctx),
        }
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            x: 4,
            y: 4,
            ..Game.new_game()
        }
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn water_encounters(&self, map: MapId, _: u16) -> Option<&'static WaterEncounters> {
        (map == TOWN && WATER_TABLE.with(|t| t.get())).then_some(&LAKE_WATER)
    }
    fn map_type(&self, _: MapId) -> MapType {
        MapType::Outdoor
    }
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        match m {
            FieldMove::Surf => FieldMoveGate::PartyMove {
                move_id: SURF_MOVE,
                badge: Some(FLAG_BADGE),
            },
            _ => FieldMoveGate::Never,
        }
    }
    fn field_move_script(&self, m: FieldMove) -> Option<ScriptId> {
        (m == FieldMove::Surf).then_some(SURF_SCRIPT)
    }
    fn pushable_boulder(&self) -> Option<(GfxId, FlagId)> {
        Some((BOULDER_GFX, FLAG_STRENGTH))
    }
}

fn hold(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(&Lake, keys).validate().unwrap();
    }
}

fn keys(d: Dir) -> Buttons {
    match d {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    }
}

/// Holds `d` until the player stands one cell on; returns the frames it took.
fn step(w: &mut World, d: Dir) -> u32 {
    let from = w.objects().player().pos();
    for n in 1..100 {
        hold(w, keys(d), 1);
        let p = w.objects().player();
        if p.pos() != from && p.is_settled() {
            return n;
        }
    }
    panic!("stuck at {from:?} going {d:?}");
}

/// The lake after the new-game fade: the player on the shore (4, 4) facing the water, with a
/// mon that knows (`moves`) the surf move or not.
fn lake(knows_surf: bool) -> World {
    let mut w = World::new(&Lake);
    hold(&mut w, Buttons::NONE, 16);
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.give_mon(WATER, 30).await;
    }));
    hold(&mut w, Buttons::NONE, 4);
    if knows_surf {
        w.s.party.get_mut(0).unwrap().moves = [None, Some(SURF_MOVE), None, None];
    }
    assert_eq!(w.objects().player().pos(), (4, 4));
    assert_eq!(w.objects().player().facing, Dir::Down);
    w
}

/// Presses A and runs the Surf script and effect; returns the frames the effect ran.
fn surf(w: &mut World) -> u32 {
    hold(w, Buttons::A, 1);
    assert_eq!(w.s.last_script, Some(SURF_SCRIPT));
    let mut busy = 0;
    for _ in 0..400 {
        hold(w, Buttons::NONE, 1);
        if w.s.field_move_busy() {
            busy += 1;
        } else if busy > 0 {
            return busy;
        }
    }
    panic!("the surf effect never ended");
}

fn round_trip(w: &World) -> World {
    let cp = w.checkpoint(&Lake).expect("capturable");
    let bytes = postcard::to_allocvec(&cp).unwrap();
    let cp: Checkpoint = postcard::from_bytes(&bytes).unwrap();
    World::restore(&Lake, &cp).expect("restores")
}

#[test]
fn water_blocks_on_foot_and_a_offers_surf_only_with_the_badge_and_the_move() {
    for (badge, knows) in [(false, true), (true, false), (false, false)] {
        let mut w = lake(knows);
        w.s.vars.set_flag(FLAG_BADGE, badge);
        // On foot the water is an elevation mismatch: a bump.
        hold(&mut w, Buttons::DOWN, 40);
        assert_eq!(w.objects().player().pos(), (4, 4));
        hold(&mut w, Buttons::A, 1);
        hold(&mut w, Buttons::NONE, 4);
        assert_ne!(
            w.s.last_script,
            Some(SURF_SCRIPT),
            "badge {badge}, knows {knows}"
        );
        assert!(!w.objects().surfing());
    }
}

#[test]
fn surf_effect_jumps_onto_the_water_then_steps_are_fast_and_land_dismounts() {
    let mut w = lake(true);
    w.s.vars.set_flag(FLAG_BADGE, true);
    let busy = surf(&mut w);
    // Init, the pose and the outdoor banner, then the 16-frame jump (its last frame is the
    // effect's end, which clears it).
    assert_eq!(busy as u16 + 1, 1 + POSE_FRAMES + SHOW_MON_OUTDOORS + 16);
    assert!(w.objects().surfing());
    assert_eq!(w.objects().player().pos(), (4, 5));
    assert!(w.capturable().is_ok());
    // `PlayerWalkFast` on the water, B or not.
    assert_eq!(step(&mut w, Dir::Down), 8);
    assert_eq!(step(&mut w, Dir::Right), 8);
    // Back to the shore: a jump (16 frames) onto the land, on foot again.
    assert_eq!(step(&mut w, Dir::Up), 8);
    assert_eq!(w.objects().player().pos(), (5, 5));
    assert_eq!(step(&mut w, Dir::Up), 16);
    assert_eq!(w.objects().player().pos(), (5, 4));
    assert!(!w.objects().surfing());
    assert_eq!(step(&mut w, Dir::Up), 16, "a normal walk on land");
}

#[test]
fn checkpoints_keep_surfing_and_never_cut_an_effect() {
    WATER_TABLE.with(|t| t.set(false));
    let mut w = lake(true);
    w.s.vars.set_flag(FLAG_BADGE, true);
    hold(&mut w, Buttons::A, 1);
    hold(&mut w, Buttons::NONE, 10);
    assert!(w.s.field_move_busy());
    assert_eq!(w.capturable(), Err(NotCapturable::Script));
    while w.s.field_move_busy() {
        hold(&mut w, Buttons::NONE, 1);
    }
    hold(&mut w, Buttons::NONE, 1);
    let mut b = round_trip(&w);
    assert!(b.objects().surfing());
    assert_eq!(b.hash(), w.hash());
    for k in [Buttons::DOWN; 20].into_iter().chain([Buttons::UP; 80]) {
        w.step(&Lake, k).validate().unwrap();
        b.step(&Lake, k).validate().unwrap();
        assert_eq!(w.hash(), b.hash());
    }
    assert!(!b.objects().surfing(), "dismounted after the round trip");
}

#[test]
fn surfing_meets_the_water_table_with_gen3_slots() {
    let mut met = alloc::collections::BTreeSet::new();
    for seed in 1..12u32 {
        let mut w = lake(true);
        w.s.vars.set_flag(FLAG_BADGE, true);
        surf(&mut w);
        w.s.rng = crate::rng::Rng::new(seed.wrapping_mul(0x9E37_79B9));
        let mut steps = 0;
        while !w.in_battle() {
            let d = if steps % 6 < 3 { Dir::Down } else { Dir::Up };
            let from = w.objects().player().pos();
            for _ in 0..20 {
                hold(&mut w, keys(d), 1);
                if w.in_battle()
                    || (w.objects().player().pos() != from && w.objects().player().is_settled())
                {
                    break;
                }
            }
            steps += 1;
            assert!(steps < 2000, "no water encounter");
        }
        let (_, battle, _) = w.battle().unwrap();
        // `BattleSetup_GetEnvironmentId`: ocean water outdoors.
        assert_eq!(battle.setup().environment, Environment::Water);
        let foe = battle.party(crate::battle::Side::Opponent).get(0).unwrap();
        assert!(
            LAKE_WATER
                .mons
                .iter()
                .any(|m| m.species == foe.species
                    && (m.min_level..=m.max_level).contains(&foe.level)),
            "{foe:?}"
        );
        met.insert(foe.species.0);
    }
    assert!(met.len() > 1);
}

#[test]
fn water_slots_follow_choose_wild_mon_index_water_or_rocks() {
    for (roll, slot) in [
        (0, 0),
        (59, 0),
        (60, 1),
        (89, 1),
        (90, 2),
        (94, 2),
        (95, 3),
        (98, 3),
        (99, 4),
        (199, 4),
    ] {
        assert_eq!(wild::water_slot(roll), slot, "{roll}");
    }
}

#[test]
fn strength_pushes_boulders_onto_free_cells_only() {
    let mut w = lake(false);
    // From (5, 2), facing the boulder at (6, 2).
    let player = |w: &World| w.objects().player().pos();
    step(&mut w, Dir::Up);
    step(&mut w, Dir::Up);
    step(&mut w, Dir::Right);
    assert_eq!(player(&w), (5, 2));
    let boulder = |w: &World| w.objects().get(1).unwrap().pos();
    // Without Strength: a bump.
    hold(&mut w, Buttons::RIGHT, 40);
    hold(&mut w, Buttons::NONE, 40);
    assert_eq!((player(&w), boulder(&w)), ((5, 2), (6, 2)));
    w.s.vars.set_flag(FLAG_STRENGTH, true);
    hold(&mut w, Buttons::RIGHT, 1);
    assert_eq!(boulder(&w), (7, 2));
    // `walk_in_place_slow` (32 frames) and `walk_slow` (1 px every other frame).
    hold(&mut w, Buttons::NONE, 24);
    assert!(!w.objects().get(1).unwrap().is_settled());
    hold(&mut w, Buttons::NONE, 7);
    assert_eq!(player(&w), (5, 2), "the player walks in place");
    assert!(w.objects().get(1).unwrap().is_settled());
    step(&mut w, Dir::Right);
    hold(&mut w, Buttons::RIGHT, 1);
    hold(&mut w, Buttons::NONE, 40);
    assert_eq!(boulder(&w), (8, 2));
    step(&mut w, Dir::Right);
    // The wall beyond: it stays.
    hold(&mut w, Buttons::RIGHT, 64);
    assert_eq!((player(&w), boulder(&w)), ((7, 2), (8, 2)));
}

#[test]
fn waitstate_waits_for_a_field_move_effect() {
    let mut w = lake(false);
    w.start_script(&Lake, CUT_SCRIPT, None);
    let mut n = 0;
    while !w.flag(FLAG_CUT_DONE) {
        hold(&mut w, Buttons::NONE, 1);
        n += 1;
        assert!(n < 400);
    }
    // The pose, the banner, init and `Task_DoFieldMove_RunFunc`, and the script's frames.
    let effect = (1 + POSE_FRAMES + SHOW_MON_OUTDOORS + 1) as u32;
    assert!((effect..effect + 4).contains(&n), "{n}");
}
