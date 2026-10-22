//! Task A: macro candidates, pathfinding, expansion and simulation, on the content build's
//! flavor (stub or extracted).

use content::Emerald;
use engine::battle::{self, BattleGfx};
use engine::checkpoint::{Checkpoint, state_digest};
use engine::data::{
    BLOCK_ELEVATION_SHIFT, Behavior, Dir, Font, GameData, GfxId, MapData, MapId, MapScripts,
    NewGame, ObjectEvent, ScriptId, SpecialIds, SpriteData, TextId, TrainerId, UiText, WindowGfx,
};
use engine::mon::{BattleTables, LandEncounters, TrainerData};
use engine::script::{Ctx, Script};
use engine::{Buttons, World};
use explore::config::{Config, DetectConfig, Limits};
use explore::detect::Detectors;
use explore::expand::{self, ExpandError};
use explore::macros::{applicable, candidates};
use explore::model::*;
use explore::observe::{NoObserver, StepObserver};
use explore::path::{self, reach};
use explore::policy::Policy;
use explore::probe::Probe;
use explore::runs;
use explore::sim::simulate;
use game_native::Session;
use std::collections::BTreeSet;
use std::sync::Arc;

fn m(kind: MacroKind) -> Macro {
    Macro {
        kind,
        opts: Opts::default(),
    }
}

fn no_obs<G: GameData + 'static>() -> impl FnMut() -> Box<dyn StepObserver<G>> {
    || Box::new(NoObserver)
}

fn job(start: Option<Arc<Checkpoint>>, plan: Plan) -> Job {
    Job {
        index: 0,
        parent: start.as_ref().map(|_| 1),
        start,
        plan,
    }
}

fn run<G: GameData + Clone + 'static>(game: &G, j: &Job) -> JobResult {
    simulate(game, j, &Config::default(), &mut no_obs::<G>())
}

/// The settled new game (the root's end state).
fn root(game: &Emerald) -> Arc<Checkpoint> {
    let r = run(game, &job(None, Plan::Given(vec![m(MacroKind::Settle)])));
    assert!(r.stop.is_none(), "{:?}", r.stop);
    assert_eq!(r.steps.len(), 1);
    r.steps[0].end.clone()
}

fn player(cp: &Checkpoint) -> (i16, i16) {
    cp.world.objects.objs[0].pos
}

fn script_id(name: &str) -> ScriptId {
    let i = content::data::SCRIPT_NAMES
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("no script {name}"));
    ScriptId(i as u16)
}

/// Every step's end restores, and replaying its inputs from a restore of the previous step's
/// end (or the job's start, or a new game) reaches the end state's digest, capturable.
fn check_steps<G: GameData + Clone>(game: &G, start: Option<&Checkpoint>, r: &JobResult) {
    for (i, st) in r.steps.iter().enumerate() {
        World::restore(game, &st.end).unwrap_or_else(|e| panic!("step {i} restore: {e:?}"));
        let from = if i == 0 {
            start
        } else {
            Some(&*r.steps[i - 1].end)
        };
        let mut s = match from {
            Some(cp) => Session::restore(game.clone(), cp).unwrap(),
            None => Session::new(game.clone()),
        };
        assert_eq!(runs::frames(&st.inputs), st.summary.frames, "step {i}");
        for k in runs::keys(&st.inputs) {
            s.step(k);
        }
        let snap = s
            .world()
            .snapshot(game)
            .expect("capturable after the replay");
        assert_eq!(state_digest(&snap), state_digest(&st.end.world), "step {i}");
        assert_eq!(s.world().hash(), st.end.header.world_hash, "step {i}");
    }
}

#[test]
fn new_game_settles_capturable() {
    let s = expand::new_game(Emerald::for_shells());
    assert!(s.world().capturable().is_ok());
    assert!(s.world().objects().player().is_settled());
    let r = run(
        &Emerald::for_shells(),
        &job(None, Plan::Given(vec![m(MacroKind::Settle)])),
    );
    assert_eq!(r.steps[0].end.header.frame, s.world().frame());
    assert_eq!(r.steps[0].summary.frames, s.world().frame());
}

#[test]
fn candidates_are_deterministic_and_applicable() {
    let game = Emerald::for_shells();
    let cp = root(&game);
    let r1 = reach(&game, &cp.world);
    let c1 = candidates(&game, &cp.world, &r1);
    let c2 = candidates(&game, &cp.world, &reach(&game, &cp.world));
    assert_eq!(c1, c2);
    assert!(!c1.is_empty());
    let unique: BTreeSet<_> = c1.iter().collect();
    assert_eq!(unique.len(), c1.len(), "deduplicated");
    for c in &c1 {
        assert!(applicable(&game, &cp.world, &r1, c), "{c:?}");
        assert_eq!(c.opts, Opts::default());
    }
    // Kind order: Enter, Cross, Talk, Inspect, Grass, WalkTo.
    let rank = |k: &MacroKind| match k {
        MacroKind::Enter { .. } => 0,
        MacroKind::Cross { .. } => 1,
        MacroKind::Talk { .. } => 2,
        MacroKind::Inspect { .. } => 3,
        MacroKind::Grass { .. } => 4,
        MacroKind::WalkTo { .. } => 5,
        MacroKind::Settle => 6,
    };
    assert!(c1.windows(2).all(|w| rank(&w[0].kind) <= rank(&w[1].kind)));
    // The player's own cell is never a WalkTo; unreachable cells are not applicable.
    let p = player(&cp);
    assert!(!applicable(
        &game,
        &cp.world,
        &r1,
        &m(MacroKind::WalkTo { x: p.0, y: p.1 })
    ));
    assert!(!applicable(
        &game,
        &cp.world,
        &r1,
        &m(MacroKind::WalkTo { x: -1, y: 0 })
    ));
}

#[test]
fn stub_start_candidates() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let cp = root(&game);
    let r = reach(&game, &cp.world);
    let c = candidates(&game, &cp.world, &r);
    let count = |f: fn(&MacroKind) -> bool| c.iter().filter(|m| f(&m.kind)).count();
    assert_eq!(count(|k| matches!(k, MacroKind::Enter { .. })), 0);
    assert_eq!(count(|k| matches!(k, MacroKind::Cross { .. })), 0);
    assert_eq!(count(|k| matches!(k, MacroKind::Talk { .. })), 0);
    assert_eq!(count(|k| matches!(k, MacroKind::Inspect { .. })), 0);
    let grass: Vec<_> = c
        .iter()
        .filter(|m| matches!(m.kind, MacroKind::Grass { .. }))
        .collect();
    assert_eq!(grass.len(), 1);
    assert_eq!(grass[0].kind, MacroKind::Grass { x: 12, y: 3 });
    // One WalkTo per reachable 4x4 block but the player's.
    let shift = Config::default().key.cell_shift;
    let reachable: BTreeSet<_> = r.cells().map(|(x, y)| (x >> shift, y >> shift)).collect();
    let p = player(&cp);
    let walk: Vec<_> = c
        .iter()
        .filter_map(|m| match m.kind {
            MacroKind::WalkTo { x, y } => Some((x >> shift, y >> shift)),
            _ => None,
        })
        .collect();
    let mut want = reachable.clone();
    want.remove(&(p.0 >> shift, p.1 >> shift));
    assert_eq!(walk.iter().copied().collect::<BTreeSet<_>>(), want);
    assert_eq!(walk.len(), want.len());
    assert_eq!(reachable.len(), 16, "the whole 16x16 stub is reachable");
}

#[test]
fn stub_walk_grass_and_determinism() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    // From a new game: an implicit settle step, then the walk.
    let walk = job(
        None,
        Plan::Given(vec![m(MacroKind::WalkTo { x: 10, y: 9 })]),
    );
    let r = run(&game, &walk);
    assert!(r.stop.is_none(), "{:?}", r.stop);
    assert_eq!(r.steps.len(), 2);
    assert_eq!(r.steps[0].macro_.kind, MacroKind::Settle);
    let end = &r.steps[1].end;
    assert_eq!(player(end), (10, 9));
    assert!(
        r.steps[1]
            .summary
            .cells
            .contains(&(end.world.field.map, 10, 9))
    );
    check_steps(&game, None, &r);

    // Same job, same result.
    let again = run(&game, &walk);
    for (a, b) in r.steps.iter().zip(&again.steps) {
        assert_eq!(a.inputs, b.inputs);
        assert_eq!(state_digest(&a.end.world), state_digest(&b.end.world));
        // Equal but for the host step time.
        let host = |s: &SimSummary| SimSummary {
            max_step_ns: 0,
            ..s.clone()
        };
        assert_eq!(host(&a.summary), host(&b.summary));
    }

    // The grass gives a won wild battle (default policy).
    let start = root(&game);
    let g = job(
        Some(start.clone()),
        Plan::Given(vec![m(MacroKind::Grass { x: 12, y: 3 })]),
    );
    let r = run(&game, &g);
    assert!(r.stop.is_none(), "{:?}", r.stop);
    let b = &r.steps[0].summary.battles;
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].kind, 0);
    assert_eq!(b[0].outcome, Some(engine::battle::BattleOutcome::Won as u8));
    assert!(b[0].frames > 0 && b[0].player_level > 0 && b[0].opponent_level > 0);
    check_steps(&game, Some(&start), &r);
    let again = run(&game, &g);
    assert_eq!(r.steps[0].inputs, again.steps[0].inputs);

    // Running from it with `run_wild`.
    let ran = run(
        &game,
        &job(
            Some(start.clone()),
            Plan::Given(vec![Macro {
                kind: MacroKind::Grass { x: 12, y: 3 },
                opts: Opts {
                    run_wild: true,
                    choice: 0,
                },
            }]),
        ),
    );
    assert!(ran.stop.is_none(), "{:?}", ran.stop);
    let b = &ran.steps[0].summary.battles;
    assert_eq!(b[0].outcome, Some(engine::battle::BattleOutcome::Ran as u8));
}

#[test]
fn random_plans_are_deterministic_and_replay() {
    let game = Emerald::for_shells();
    let start = root(&game);
    for seed in [1u64, 2, 3] {
        let j = job(Some(start.clone()), Plan::Random { len: 2, seed });
        let a = run(&game, &j);
        let b = run(&game, &j);
        assert!(!a.steps.is_empty(), "seed {seed}: {:?}", a.stop);
        assert_eq!(a.steps.len(), b.steps.len(), "seed {seed}");
        for (x, y) in a.steps.iter().zip(&b.steps) {
            assert_eq!(x.macro_, y.macro_);
            assert_eq!(x.inputs, y.inputs);
            assert_eq!(x.end, y.end);
        }
        assert_eq!(format!("{:?}", a.stop), format!("{:?}", b.stop));
        check_steps(&game, Some(&start), &a);
    }
}

#[test]
fn errors_prune() {
    let game = Emerald::for_shells();
    let start = root(&game);
    // Out of the map: no path, planned up front (no frame stepped).
    let mut s = Session::restore(Probe::new(game), &start).unwrap();
    let e = expand::expand_with(
        &mut s,
        &m(MacroKind::WalkTo { x: -3, y: 2 }),
        &mut Policy::new(),
        &mut NoObserver,
        &Limits::default(),
    )
    .err()
    .unwrap();
    assert_eq!(e.0, ExpandError::NoPath);
    assert!(e.1.keys.is_empty());
    let r = run(
        &game,
        &job(
            Some(start),
            Plan::Given(vec![m(MacroKind::Talk { local_id: 200 })]),
        ),
    );
    assert!(r.steps.is_empty());
    assert!(matches!(r.stop, Some(Stop::Pruned(_))), "{:?}", r.stop);
    let r = run(&game, &job(None, Plan::Softlock { k: 3, breadth: 8 }));
    assert!(matches!(&r.stop, Some(Stop::Pruned(s)) if s == "unwired"));
}

#[test]
fn next_dir_heads_for_the_target() {
    let game = Emerald::for_shells();
    let start = root(&game);
    let s = Session::restore(Probe::new(game), &start).unwrap();
    let r = reach(&game, &start.world);
    let p = player(&start);
    // A reachable neighbour cell: the first step goes straight to it.
    for d in Dir::ALL {
        let (dx, dy) = d.delta();
        let c = (p.0 + dx, p.1 + dy);
        let grass = engine::flow::behavior_at(&game, game.map(start.world.field.map), c.0, c.1)
            == Behavior::TallGrass;
        if r.dist(c.0, c.1) == Some(1) && !grass {
            assert_eq!(
                path::next_dir(&game, s.world(), &start.world.field.overrides, c),
                Some(d)
            );
        }
    }
    assert_eq!(r.dist(p.0, p.1), Some(0));
    assert_eq!(
        path::next_dir(&game, s.world(), &start.world.field.overrides, p),
        None
    );
}

/// The content build's game with faults: a panicking encounter table read (the stub's first
/// step), a panicking script start (the new game's, inside `World::new`), or a patched map.
#[derive(Clone, Copy)]
struct Fake {
    inner: Emerald,
    panic_encounters: bool,
    panic_scripts: bool,
    patch: Option<(MapId, &'static MapData)>,
}

impl Fake {
    fn new() -> Fake {
        Fake {
            inner: Emerald::for_shells(),
            panic_encounters: false,
            panic_scripts: false,
            patch: None,
        }
    }

    /// `map` with its blocks and objects replaced (leaked: tests only).
    fn patched(map: MapId, blocks: Vec<u16>, objects: Vec<ObjectEvent>) -> Fake {
        let g = Emerald::for_shells();
        let m = g.map(map);
        let data: &'static MapData = Box::leak(Box::new(MapData {
            name: m.name,
            width: m.width,
            height: m.height,
            blocks: Box::leak(blocks.into_boxed_slice()),
            border: m.border,
            primary: m.primary,
            secondary: m.secondary,
            objects: Box::leak(objects.into_boxed_slice()),
            warps: m.warps,
            coords: m.coords,
            bgs: m.bgs,
            connections: m.connections,
            scripts: MapScripts {
                on_load: m.scripts.on_load,
                on_transition: m.scripts.on_transition,
                on_resume: m.scripts.on_resume,
                on_return_to_field: m.scripts.on_return_to_field,
                on_frame: m.scripts.on_frame,
                on_warp_into: m.scripts.on_warp_into,
            },
            allow_running: m.allow_running,
        }));
        Fake {
            patch: Some((map, data)),
            ..Fake::new()
        }
    }
}

impl GameData for Fake {
    fn map(&self, id: MapId) -> &'static MapData {
        match self.patch {
            Some((p, data)) if p == id => data,
            _ => self.inner.map(id),
        }
    }
    fn has_map(&self, id: MapId) -> bool {
        self.inner.has_map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        self.inner.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        self.inner.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        self.inner.player_sprite()
    }
    fn font(&self) -> &'static Font {
        self.inner.font()
    }
    fn window(&self) -> &'static WindowGfx {
        self.inner.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        self.inner.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        self.inner.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        self.inner.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        assert!(!self.panic_scripts, "script started");
        self.inner.script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        self.inner.new_game()
    }
    fn build_id(&self) -> u64 {
        self.inner.build_id()
    }
    fn battle(&self) -> &'static BattleTables {
        self.inner.battle()
    }
    fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> {
        assert!(!self.panic_encounters, "encounter table read");
        self.inner.encounters(map)
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
        self.inner.trainer(id)
    }
    fn battle_text(&self, s: battle::Str) -> &'static [u8] {
        self.inner.battle_text(s)
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        self.inner.battle_gfx()
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        self.inner.heal_location(id)
    }
}

#[test]
fn a_panic_is_a_finding_with_a_replayable_tail() {
    if !content::STUB {
        return;
    }
    let game = Fake {
        panic_encounters: true,
        ..Fake::new()
    };
    // The encounter table is read on the first step taken.
    let grass = m(MacroKind::Grass { x: 12, y: 3 });
    let r = run(&game, &job(None, Plan::Given(vec![grass])));
    assert_eq!(r.steps.len(), 1, "the implicit settle");
    let Some(Stop::Finding(f)) = &r.stop else {
        panic!("{:?}", r.stop)
    };
    assert_eq!(f.kind, FindingKind::Panic);
    assert_eq!(f.message, "encounter table read");
    assert!(f.location.as_deref().unwrap().contains("a_expand.rs"));
    assert_eq!(f.macro_, Some(grass));
    // The tail runs from the job's start (a new game) through the settle to the panic.
    let steps: u32 = r.steps.iter().map(|s| runs::frames(&s.inputs)).sum();
    let tail = runs::frames(&f.tail);
    assert!(tail > steps);
    let mut s = Session::new(game);
    let keys: Vec<Buttons> = runs::keys(&f.tail).collect();
    for &k in &keys[..keys.len() - 1] {
        explore::guard::step_guarded(&mut s, k).expect("no panic before the last frame");
    }
    let e = explore::guard::step_guarded(&mut s, keys[keys.len() - 1]).unwrap_err();
    assert_eq!(e.message, f.message);
}

#[test]
fn real_littleroot_start() {
    if content::STUB {
        return;
    }
    use content::consts::*;
    let game = Emerald::for_shells();
    let cp = root(&game);
    assert_eq!(cp.world.field.map, MAP_LITTLEROOT_TOWN);
    let c = candidates(&game, &cp.world, &reach(&game, &cp.world));
    let enters: Vec<_> = c
        .iter()
        .filter_map(|m| match m.kind {
            MacroKind::Enter { x, y, dir } => Some((x, y, dir)),
            _ => None,
        })
        .collect();
    // The two houses and the lab, through their doors.
    assert!(enters.len() >= 3, "{enters:?}");
    assert!(enters.iter().all(|e| e.2 == Dir::Up), "{enters:?}");
    assert!(
        c.iter()
            .any(|m| matches!(m.kind, MacroKind::Cross { dir: Dir::Up, .. })),
        "{c:?}"
    );
}

#[test]
fn real_route_moments() {
    if content::STUB {
        return;
    }
    use content::consts::*;
    let game = Emerald::for_shells();
    let start = root(&game);
    let r = run(
        &game,
        &job(
            Some(start.clone()),
            Plan::Given(vec![
                m(MacroKind::WalkTo { x: 11, y: 2 }),
                m(MacroKind::WalkTo { x: 11, y: 1 }),
                m(MacroKind::Cross {
                    x: 10,
                    y: 0,
                    dir: Dir::Up,
                }),
            ]),
        ),
    );
    assert!(r.stop.is_none(), "{:?}", r.stop);
    assert_eq!(r.steps.len(), 3);
    assert_eq!(player(&r.steps[0].end), (11, 2));
    // The girl stops the player: the coord script runs, and the node ends after it.
    let coord2 = script_id("LittlerootTown_Coord2");
    assert!(r.steps[1].summary.scripts.contains(&coord2));
    assert!(!r.steps[0].summary.scripts.contains(&coord2));
    assert_eq!(r.steps[1].end.world.field.map, MAP_LITTLEROOT_TOWN);
    // North into Route 101, where Birch's chase starts. The rescue keeps the player near the
    // crossing, so the view stays stale (not capturable) until the driver talks to the only
    // scripted object in reach, Birch's bag: the starter screen, the first battle and the warp
    // to the lab follow, and the node ends there.
    let cross = &r.steps[2];
    assert_eq!(cross.summary.maps.first(), Some(&MAP_ROUTE101));
    assert!(
        cross
            .summary
            .scripts
            .contains(&script_id("Route101_Coord0")),
        "{:?}",
        cross.summary.scripts
    );
    assert_eq!(
        cross.end.world.field.map,
        MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB
    );
    assert_eq!(cross.end.world.party.len(), 1, "the starter");
    let b = &cross.summary.battles;
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].kind, 1, "the first battle");
    check_steps(&game, Some(&start), &r);
}

#[test]
fn a_panic_building_the_new_game_is_a_finding_on_its_map() {
    let game = Fake {
        panic_scripts: true,
        ..Fake::new()
    };
    let r = run(&game, &job(None, Plan::Given(vec![m(MacroKind::Settle)])));
    assert!(r.steps.is_empty());
    let Some(Stop::Finding(f)) = &r.stop else {
        panic!("{:?}", r.stop)
    };
    assert_eq!(f.kind, FindingKind::Panic);
    assert_eq!(f.message, "script started");
    assert!(f.tail.is_empty());
    assert_eq!(f.map, game.new_game().map);
}

#[test]
fn slow_frames_never_end_a_job() {
    let game = Emerald::for_shells();
    let start = root(&game);
    let cfg = Config::default();
    let slow = DetectConfig {
        slow_step_us: 1,
        ..DetectConfig::default()
    };
    for seed in [1u64, 2] {
        let j = job(Some(start.clone()), Plan::Random { len: 2, seed });
        let plain = run(&game, &j);
        let mut detectors =
            || -> Box<dyn StepObserver<Emerald>> { Box::new(Detectors::<Emerald>::new(&slow)) };
        let watched = simulate(&game, &j, &cfg, &mut detectors);
        assert!(!plain.steps.is_empty(), "seed {seed}: {:?}", plain.stop);
        assert_eq!(plain.steps.len(), watched.steps.len(), "seed {seed}");
        for (a, b) in plain.steps.iter().zip(&watched.steps) {
            assert_eq!(a.macro_, b.macro_);
            assert_eq!(a.inputs, b.inputs);
            assert_eq!(a.end, b.end);
        }
        assert_eq!(format!("{:?}", plain.stop), format!("{:?}", watched.stop));
    }
    // The flags are recorded, not dropped.
    let target = reach(&game, &start.world)
        .cells()
        .find(|&c| c != player(&start))
        .unwrap();
    let mut s = Session::restore(Probe::new(game), &start).unwrap();
    let ex = expand::expand_with(
        &mut s,
        &m(MacroKind::WalkTo {
            x: target.0,
            y: target.1,
        }),
        &mut Policy::new(),
        &mut Detectors::<Emerald>::new(&slow),
        &Limits::default(),
    )
    .unwrap_or_else(|e| panic!("{:?}", e.0));
    assert!(!ex.slow.is_empty() && ex.slow.len() <= expand::MAX_SLOW);
    assert!(ex.slow.iter().all(|f| f.kind == FindingKind::SlowFrame));
}

#[test]
fn a_planner_divergence_is_flagged_after_a_stall() {
    if !content::STUB {
        return;
    }
    // Every cell at elevation 3 but one at 4 on the player's way: the planner (elevation
    // ignored) holds the direction toward it, the engine refuses the step.
    let game = Emerald::for_shells();
    let start = root(&game);
    let p = player(&start);
    let map = start.world.field.map;
    let base = game.map(map);
    let wall = (p.0 + 1, p.1);
    let to = (p.0 + 3, p.1);
    let mask = 0xF << BLOCK_ELEVATION_SHIFT;
    let blocks = base
        .blocks
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            let c = (
                (i % base.width as usize) as i16,
                (i / base.width as usize) as i16,
            );
            let e: u16 = if c == wall { 4 } else { 3 };
            (b & !mask) | (e << BLOCK_ELEVATION_SHIFT)
        })
        .collect();
    let fake = Fake::patched(map, blocks, base.objects.to_vec());
    let r = run(
        &fake,
        &job(
            None,
            Plan::Given(vec![m(MacroKind::WalkTo { x: to.0, y: to.1 })]),
        ),
    );
    assert_eq!(r.steps.len(), 1, "the settle: {:?}", r.stop);
    let Some(Stop::Finding(f)) = &r.stop else {
        panic!("{:?}", r.stop)
    };
    assert_eq!(f.kind, FindingKind::PlannerDivergence, "{}", f.message);
    assert_eq!(f.map, map);
    let stalled = runs::frames(&f.tail) - r.steps[0].summary.frames;
    assert!(stalled >= expand::STALL_FRAMES, "{stalled}");
}

#[test]
fn real_enter_a_house() {
    if content::STUB {
        return;
    }
    use content::consts::*;
    let game = Emerald::for_shells();
    let start = root(&game);
    let c = candidates(&game, &start.world, &reach(&game, &start.world));
    let enter = *c
        .iter()
        .find(|m| matches!(m.kind, MacroKind::Enter { .. }))
        .expect("a door");
    let r = run(&game, &job(Some(start.clone()), Plan::Given(vec![enter])));
    assert!(r.stop.is_none(), "{:?}", r.stop);
    assert_eq!(r.steps.len(), 1);
    let end = &r.steps[0].end;
    assert_ne!(end.world.field.map, MAP_LITTLEROOT_TOWN);
    assert_eq!(r.steps[0].summary.maps.last(), Some(&end.world.field.map));
    check_steps(&game, Some(&start), &r);
}

#[test]
fn real_stale_view_tries_each_object_once() {
    if content::STUB {
        return;
    }
    use content::consts::*;
    // The first always-present scripted object of Route 101 (one that only shows text) is
    // moved next to where the rescue's triggers push the player back, so it is the nearest:
    // talking to it leaves the view stale, and the driver moves on to Birch's bag.
    let base = Emerald::for_shells().map(MAP_ROUTE101);
    let mut objects = base.objects.to_vec();
    let text = objects
        .iter_mut()
        .filter(|o| o.script.is_some() && o.hide_flag.is_none())
        .min_by_key(|o| o.local_id)
        .expect("a text-only object");
    (text.x, text.y) = (8, 15);
    let text_id = text.local_id;
    let game = Fake::patched(MAP_ROUTE101, base.blocks.to_vec(), objects);
    let mut s = Session::new(Probe::new(game));
    let mut policy = Policy::new();
    for kind in [
        MacroKind::Settle,
        MacroKind::WalkTo { x: 11, y: 2 },
        MacroKind::WalkTo { x: 11, y: 1 },
    ] {
        expand::expand_with(
            &mut s,
            &m(kind),
            &mut policy,
            &mut NoObserver,
            &Limits::default(),
        )
        .unwrap_or_else(|e| panic!("{kind:?}: {:?}", e.0));
    }
    let cross = m(MacroKind::Cross {
        x: 10,
        y: 0,
        dir: Dir::Up,
    });
    let ex = expand::expand_with(
        &mut s,
        &cross,
        &mut policy,
        &mut NoObserver,
        &Limits::default(),
    )
    .unwrap_or_else(|e| panic!("{:?}", e.0));
    assert_eq!(
        s.world().map(),
        MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB,
        "{:?}",
        ex.notes
    );
    assert_eq!(ex.notes.len(), 2, "{:?}", ex.notes);
    assert!(
        ex.notes[0].ends_with(&format!("object {text_id}")),
        "{:?}",
        ex.notes
    );
}
