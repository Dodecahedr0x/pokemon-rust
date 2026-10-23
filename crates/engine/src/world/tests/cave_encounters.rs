//! Off-grass land encounters in the World: a cave map (Gen3 `MB_CAVE` floor, a band of
//! `MB_LONG_GRASS`, plain ground), walked until a wild Pokémon appears. Every frame's RNG draws
//! are replayed with `wild::try_land_encounter` (Gen3 `CheckStandardWildEncounter`) from the
//! state before the frame, so the World draws exactly Gen3's rolls on every encounter-tile step
//! and nothing on plain ground; the battle that starts is the replay's mon, on the cave
//! background.

use super::*;
use crate::battle::Side;
use crate::battle::test_data::{MUDKIP, SPARK, WATER, ZIGZAGOON};
use crate::battle::{BattleKind, Environment};
use crate::mon::{LandEncounters, Pokemon, WildMon};
use crate::wild;

/// Raw behaviours of the cave's tileset (the synthetic tileset's metatile ids).
const CAVE_FLOOR: u16 = 4; // GRASS's metatile: `MB_CAVE` here
const LONG_GRASS: u16 = 3; // MAT's metatile: `MB_LONG_GRASS` here

/// A 10x10 cave, walled round: rows 1–3 plain ground, row 4 long grass, rows 5–8 cave floor.
const fn cave_blocks() -> [u16; 100] {
    let mut b = [FLOOR; 100];
    let mut i = 0;
    while i < 10 {
        b[i] = WALL;
        b[90 + i] = WALL;
        b[i * 10] = WALL;
        b[i * 10 + 9] = WALL;
        i += 1;
    }
    let mut x = 1;
    while x < 9 {
        b[40 + x] = LONG_GRASS;
        let mut y = 5;
        while y < 9 {
            b[y * 10 + x] = CAVE_FLOOR;
            y += 1;
        }
        x += 1;
    }
    b
}
static CAVE_BLOCKS: [u16; 100] = cave_blocks();

static CAVE: MapData = MapData {
    name: "cave",
    width: 10,
    height: 10,
    blocks: &CAVE_BLOCKS,
    border: [WALL; 4],
    primary: &TS,
    secondary: &TS2,
    objects: &[],
    warps: &[],
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: true,
};

const fn mon(species: crate::data::SpeciesId, min_level: u8, max_level: u8) -> WildMon {
    WildMon {
        species,
        min_level,
        max_level,
    }
}

/// Granite Cave 1F's rate (10): 160 / 2880 per checked step.
static CAVE_ENCOUNTERS: LandEncounters = LandEncounters {
    rate: 10,
    mons: [
        mon(ZIGZAGOON, 6, 8),
        mon(MUDKIP, 7, 9),
        mon(SPARK, 5, 5),
        mon(ZIGZAGOON, 9, 10),
        mon(MUDKIP, 6, 6),
        mon(SPARK, 8, 8),
        mon(ZIGZAGOON, 7, 7),
        mon(MUDKIP, 10, 10),
        mon(SPARK, 6, 7),
        mon(ZIGZAGOON, 8, 9),
        mon(MUDKIP, 11, 11),
        mon(SPARK, 12, 12),
    ],
};

/// [`Game`] with the cave as the town (an underground map with a land table).
struct Cave;

impl GameData for Cave {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == TOWN { &CAVE } else { Game.map(id) }
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
        match raw as u16 {
            CAVE_FLOOR => Behavior::Floor(Floor::Cave),
            LONG_GRASS => Behavior::Floor(Floor::LongGrass),
            _ => Game.behavior(raw),
        }
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        Game.script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            x: 4,
            y: 2,
            ..Game.new_game()
        }
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn wild_encounters(&self, map: MapId, _: u16) -> Option<&'static LandEncounters> {
        (map == TOWN).then_some(&CAVE_ENCOUNTERS)
    }
    fn map_type(&self, map: MapId) -> MapType {
        if map == TOWN {
            MapType::Underground
        } else {
            MapType::Outdoor
        }
    }
}

fn hold(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(&Cave, keys).validate().unwrap();
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

/// What one frame did to the encounter state, checked against Gen3's rolls.
enum Checked {
    Nothing,
    /// A completed step: the replay drew what the World drew.
    Step,
    /// A completed step that met this mon.
    Battle(Pokemon),
}

/// Steps one frame holding `k`, and checks its RNG draws against the replay of
/// `CheckStandardWildEncounter` from the state before the frame.
fn checked_step(w: &mut World, k: Buttons) -> Checked {
    let (rng, prev, immunity) = (w.s.rng, w.s.prev_behavior, w.s.encounter_immunity);
    hold(w, k, 1);
    if w.s.rng == rng && !w.in_battle() {
        return Checked::Nothing;
    }
    let now = w.objects().player().pos();
    let cur = crate::flow::behavior_at(&Cave, &CAVE, now.0, now.1);
    assert!(immunity >= 4, "a draw during the immunity steps");
    let mut r = rng;
    let t = Cave.battle();
    let lead = w.party().get(0).copied();
    let found = wild::try_land_encounter(
        &CAVE_ENCOUNTERS,
        prev,
        cur,
        wild::Lead::of(lead.as_ref(), t, |_| None),
        t,
        &mut r,
    )
    .map(|(species, level)| {
        wild::create_wild(t, species, level, lead.as_ref(), w.s.player_id, &mut r)
    });
    match found {
        None => {
            assert!(!w.in_battle());
            assert_eq!(w.s.rng, r, "the World drew other rolls than Gen3's");
            Checked::Step
        }
        Some(m) => {
            assert!(w.in_battle(), "the replay met {m:?}, the World did not");
            Checked::Battle(m)
        }
    }
}

/// The cave after the new-game fade, the player on (4, 2) with one mon, the RNG seeded from
/// `seed` (spread: xorshift32 starts small seeds with small values; `None`: the World's own).
fn cave(seed: Option<u32>) -> World {
    let mut w = World::new(&Cave);
    hold(&mut w, Buttons::NONE, 16);
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.give_mon(WATER, 30).await;
    }));
    hold(&mut w, Buttons::NONE, 4);
    assert_eq!(w.party().len(), 1);
    assert_eq!(w.objects().player().pos(), (4, 2));
    if let Some(seed) = seed {
        w.s.rng = crate::rng::Rng::new(seed.wrapping_mul(0x9E37_79B9));
    }
    w
}

/// Walks down the plain ground (no draws), across the long grass, onto the cave floor, then up
/// and down rows 4-8 (long grass to cave floor and back: every kind of new-tile change),
/// checking every frame ([`checked_step`]), until a wild battle starts. Returns the checked
/// steps before it and the mon met.
fn walk_until_battle(w: &mut World) -> (u32, Pokemon) {
    let mut checked = 0;
    let start = [Dir::Down; 6].into_iter();
    let back_and_forth =
        core::iter::repeat([Dir::Up; 4].into_iter().chain([Dir::Down; 4])).flatten();
    for (steps, d) in start.chain(back_and_forth).enumerate() {
        let from = w.objects().player().pos();
        let mut frames = 0;
        loop {
            match checked_step(w, keys(d)) {
                Checked::Battle(m) => return (checked, m),
                Checked::Step => checked += 1,
                Checked::Nothing => {}
            }
            frames += 1;
            if w.objects().player().pos() != from && w.objects().player().is_settled() {
                break;
            }
            assert!(frames < 64, "stuck at {from:?} going {d:?}");
        }
        assert!(steps < 2000, "no encounter in {steps} steps");
    }
    unreachable!()
}

#[test]
fn cave_floor_steps_meet_wild_pokemon_with_gen3_rolls() {
    let mut checked = 0;
    let mut backgrounds = alloc::collections::BTreeSet::new();
    // The World's own RNG, then other seeds, for more steps and both floors.
    for seed in core::iter::once(None).chain((1..40).map(Some)) {
        let mut w = cave(seed);
        let (n, m) = walk_until_battle(&mut w);
        checked += n;
        let (_, battle, _) = w.battle().unwrap();
        assert!(matches!(battle.setup().kind, BattleKind::Wild { .. }));
        // `BattleSetup_GetEnvironmentId`: the cave's background, the long grass's on its row.
        let (x, y) = w.objects().player().pos();
        let env = battle.setup().environment;
        let want = if CAVE_BLOCKS[y as usize * 10 + x as usize] == LONG_GRASS {
            Environment::LongGrass
        } else {
            Environment::Cave
        };
        assert_eq!(env, want, "seed {seed:?} at {:?}", (x, y));
        backgrounds.insert(env as u8);
        let foe = battle.party(Side::Opponent).get(0).unwrap();
        assert_eq!(
            (foe.species, foe.level, foe.personality, foe.ivs),
            (m.species, m.level, m.personality, m.ivs),
            "seed {seed:?}"
        );
    }
    // 160/2880 per same-floor step: some hundreds of checked steps without a battle.
    assert!(checked > 200, "{checked}");
    assert_eq!(backgrounds.len(), 2, "battles on both floors");
}

#[test]
fn battle_background_follows_get_battle_environment() {
    use super::super::battle::environment;
    use Environment as E;
    let f = Behavior::Floor;
    for (b, map, want) in [
        (Behavior::TallGrass, MapType::Underground, E::Grass),
        (f(Floor::LongGrass), MapType::Outdoor, E::LongGrass),
        (f(Floor::Sand), MapType::Indoor, E::Sand),
        (f(Floor::DeepSand), MapType::Outdoor, E::Sand),
        (f(Floor::Cave), MapType::Underground, E::Cave),
        (Behavior::Normal, MapType::Underground, E::Cave),
        (f(Floor::IndoorEncounter), MapType::Underground, E::Building),
        (f(Floor::IndoorEncounter), MapType::Indoor, E::Building),
        (Behavior::Normal, MapType::Indoor, E::Building),
        (Behavior::Normal, MapType::Underwater, E::Underwater),
        (f(Floor::MountainTop), MapType::OceanRoute, E::Plain),
        (Behavior::Normal, MapType::OceanRoute, E::Plain),
        (f(Floor::MountainTop), MapType::Outdoor, E::Mountain),
        (f(Floor::MountainTop), MapType::OutdoorSand, E::Mountain),
        (f(Floor::AshGrass), MapType::OutdoorSand, E::Sand),
        (Behavior::Normal, MapType::OutdoorSand, E::Sand),
        (f(Floor::Cave), MapType::Outdoor, E::Plain),
        (Behavior::Normal, MapType::Outdoor, E::Plain),
    ] {
        assert_eq!(environment(b, map), want, "{b:?} on {map:?}");
    }
}
