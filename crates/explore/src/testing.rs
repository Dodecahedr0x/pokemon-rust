//! Fault-injection `GameData` wrappers for tests (Task E). Each delegates every method
//! (provided ones too) to `inner`, like `Probe`, except the one it breaks.
//!
//! - [`PanicOnScript`]: a game panic (`Panic` findings, the guard).
//! - [`WallAround`]: a trapped player (`Softlock` findings).
//! - [`Drifting`]: state the checkpoint does not carry (`RoundTrip` findings).
//! - [`FadeTrap`]: a script that leaves the screen black (`CapturableStuck` findings).
//! - [`Rebuilt`]: another build id with the same behaviour (revalidation).
//! - [`Reporting`]: every script reports first (`Unsupported` findings).
//!
//! [`same_answers`] checks that a wrapper answers the Milestone 8 methods like its inner game.

use engine::battle::{self, BattleGfx};
use engine::data::{
    BLOCK_COLLISION_SHIFT, Behavior, CoordEvent, Emote, Evolution, FlagId, Font, GameData, GfxId,
    ItemId, ItemInfo, MapData, MapId, MapScripts, NewGame, ScriptId, SpecialIds, SpeciesId,
    SpriteData, TextId, TrainerId, UiText, VarId, WarpEvent, WindowGfx,
};
use engine::mon::{BattleTables, LandEncounters, TrainerData};
use engine::script::{Ctx, Fade, Report, Script};
use std::cell::Cell;
use std::sync::RwLock;

/// One delegating `GameData` method per name: `forward!(map has_map …)`.
macro_rules! forward {
    ($($m:ident)*) => { $( forward!(@ $m); )* };
    (@ map) => { fn map(&self, id: MapId) -> &'static MapData { self.inner.map(id) } };
    (@ has_map) => { fn has_map(&self, id: MapId) -> bool { self.inner.has_map(id) } };
    (@ special_ids) => { fn special_ids(&self) -> SpecialIds { self.inner.special_ids() } };
    (@ sprite) => {
        fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> { self.inner.sprite(gfx) }
    };
    (@ player_sprite) => {
        fn player_sprite(&self) -> &'static SpriteData { self.inner.player_sprite() }
    };
    (@ font) => { fn font(&self) -> &'static Font { self.inner.font() } };
    (@ window) => { fn window(&self) -> &'static WindowGfx { self.inner.window() } };
    (@ text) => { fn text(&self, id: TextId) -> &'static [u8] { self.inner.text(id) } };
    (@ ui_text) => { fn ui_text(&self, t: UiText) -> &'static [u8] { self.inner.ui_text(t) } };
    (@ behavior) => { fn behavior(&self, raw: u8) -> Behavior { self.inner.behavior(raw) } };
    (@ script) => {
        fn script(&self, id: ScriptId, ctx: Ctx) -> Script { self.inner.script(id, ctx) }
    };
    (@ new_game) => { fn new_game(&self) -> NewGame { self.inner.new_game() } };
    (@ build_id) => { fn build_id(&self) -> u64 { self.inner.build_id() } };
    (@ battle) => { fn battle(&self) -> &'static BattleTables { self.inner.battle() } };
    (@ encounters) => {
        fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> {
            self.inner.encounters(map)
        }
    };
    (@ trainer) => {
        fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
            self.inner.trainer(id)
        }
    };
    (@ battle_text) => {
        fn battle_text(&self, s: battle::Str) -> &'static [u8] { self.inner.battle_text(s) }
    };
    (@ battle_gfx) => { fn battle_gfx(&self) -> &'static BattleGfx { self.inner.battle_gfx() } };
    (@ heal_location) => {
        fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
            self.inner.heal_location(id)
        }
    };
    (@ wild_encounters) => {
        fn wild_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters> {
            self.inner.wild_encounters(map, wild_set)
        }
    };
    (@ item) => { fn item(&self, id: ItemId) -> Option<&'static ItemInfo> { self.inner.item(id) } };
    (@ multichoice) => {
        fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
            self.inner.multichoice(list)
        }
    };
    (@ std_string) => {
        fn std_string(&self, id: u8) -> Option<&'static [u8]> { self.inner.std_string(id) }
    };
    (@ trainer_flag_of) => {
        fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
            self.inner.trainer_flag_of(script)
        }
    };
    (@ emote) => {
        fn emote(&self, e: Emote) -> Option<&'static SpriteData> { self.inner.emote(e) }
    };
    (@ trainer_money) => {
        fn trainer_money(&self, id: TrainerId) -> Option<u8> { self.inner.trainer_money(id) }
    };
    (@ evolutions) => {
        fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
            self.inner.evolutions(species)
        }
    };
    // The Milestone 8 methods, except `wild_encounters` (see [`Drifting`]).
    (@ m8) => {
        forward!(item multichoice std_string trainer_flag_of emote trainer_money evolutions);
    };
}

/// `script()` returns a panicking script for `script`.
///
/// The panic is raised from this file (its `Location` names `testing.rs`) when the script is
/// first polled. `content::NEW_GAME_SCRIPT` runs inside `World::new` (an immediate script), so
/// on the stub world, which has no event scripts, the panic happens in `Session::new`, before
/// the first step.
#[derive(Clone, Copy, Debug)]
pub struct PanicOnScript<G> {
    pub inner: G,
    pub script: ScriptId,
}

/// The message of a [`PanicOnScript`] panic (`{}` = the script id).
pub const INJECTED_PANIC: &str = "injected panic in script";

impl<G: GameData> GameData for PanicOnScript<G> {
    forward!(map has_map special_ids sprite player_sprite font window text ui_text behavior
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id != self.script {
            return self.inner.script(id, ctx);
        }
        Box::pin(async move {
            panic!("{INJECTED_PANIC} {}", id.0);
        })
    }
}

/// A leaked `MapData` copy of `map` with the 8 cells around `at` blocked (collision 1), plus
/// one warp (to `NOWHERE`) on the first open cell in row-major order outside the ring, so that
/// `softlock::map_exits` is at least 1. The warp cell keeps its behaviour, so on a plain cell it
/// never fires.
#[derive(Clone, Copy, Debug)]
pub struct WallAround<G> {
    pub inner: G,
    pub map: MapId,
    pub at: (i16, i16),
}

/// The destination of the warp [`WallAround`] adds: a map no game has.
pub const NOWHERE: MapId = MapId(u16::MAX);

impl<G: GameData> GameData for WallAround<G> {
    forward!(has_map special_ids sprite player_sprite font window text ui_text behavior script
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn map(&self, id: MapId) -> &'static MapData {
        let base = self.inner.map(id);
        if id != self.map {
            return base;
        }
        leaked(Patch::Wall, base, self.at)
    }
}

/// A leaked `MapData` copy of `map` with the ring of cells two steps around `at` blocked: a
/// 3x3 pocket centred on `at`, plus the exit warp of [`WallAround`] outside it. Centred on the
/// stub's tall-grass cell, it is a sealed grass pocket: walks meet wild battles, and the party
/// can level up or faint without ever leaving.
#[derive(Clone, Copy, Debug)]
pub struct GrassPocket<G> {
    pub inner: G,
    pub map: MapId,
    pub at: (i16, i16),
}

impl<G: GameData> GameData for GrassPocket<G> {
    forward!(has_map special_ids sprite player_sprite font window text ui_text behavior script
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn map(&self, id: MapId) -> &'static MapData {
        let base = self.inner.map(id);
        if id != self.map {
            return base;
        }
        leaked(Patch::Pocket, base, self.at)
    }
}

/// `encounters()` depends on a call counter: it answers `None` for the first [`DRIFT_CALLS`]
/// calls of an instance. A clone starts again at 0 calls: the counter is state that a
/// checkpoint does not carry, so a session restored on a clone drifts from the live one at the
/// next tall-grass step (round-trip break).
#[derive(Debug)]
pub struct Drifting<G> {
    pub inner: G,
    pub calls: Cell<u32>,
}

/// Calls of [`Drifting::encounters`] answered `None`.
pub const DRIFT_CALLS: u32 = 16;

impl<G> Drifting<G> {
    pub fn new(inner: G) -> Self {
        Drifting {
            inner,
            calls: Cell::new(0),
        }
    }

    /// Counts a call; `false` for the first [`DRIFT_CALLS`] (answered `None`).
    fn drift(&self) -> bool {
        let n = self.calls.get();
        self.calls.set(n.saturating_add(1));
        n >= DRIFT_CALLS
    }
}

impl<G: Clone> Clone for Drifting<G> {
    /// A fresh counter (see [`Drifting`]).
    fn clone(&self) -> Self {
        Drifting::new(self.inner.clone())
    }
}

impl<G: GameData> GameData for Drifting<G> {
    forward!(map has_map special_ids sprite player_sprite font window text ui_text behavior
             script new_game build_id battle trainer battle_text battle_gfx heal_location m8);

    fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> {
        self.drift().then(|| self.inner.encounters(map)).flatten()
    }

    /// What the engine reads: drifts like [`Drifting::encounters`].
    fn wild_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters> {
        self.drift()
            .then(|| self.inner.wild_encounters(map, wild_set))
            .flatten()
    }
}

/// A leaked `MapData` copy of `map` with a coord event on `at` (any elevation, always armed)
/// whose script ([`FADE_TRAP_SCRIPT`]) fades the screen to black and ends: the field is idle
/// but never capturable again (`NotCapturable::Fade`).
#[derive(Clone, Copy, Debug)]
pub struct FadeTrap<G> {
    pub inner: G,
    pub map: MapId,
    pub at: (i16, i16),
}

/// The script id of the [`FadeTrap`] coord event (never a generated id).
pub const FADE_TRAP_SCRIPT: ScriptId = ScriptId(u16::MAX - 1);

impl<G: GameData> GameData for FadeTrap<G> {
    forward!(has_map special_ids sprite player_sprite font window text ui_text behavior
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn map(&self, id: MapId) -> &'static MapData {
        let base = self.inner.map(id);
        if id != self.map {
            return base;
        }
        leaked(Patch::Coord, base, self.at)
    }

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id != FADE_TRAP_SCRIPT {
            return self.inner.script(id, ctx);
        }
        Box::pin(async move { ctx.fade(Fade::ToBlack).await })
    }
}

/// A different build id, same behaviour.
#[derive(Clone, Copy, Debug)]
pub struct Rebuilt<G> {
    pub inner: G,
    pub build_id: u64,
}

impl<G: GameData> GameData for Rebuilt<G> {
    forward!(map has_map special_ids sprite player_sprite font window text ui_text behavior
             script new_game battle encounters trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn build_id(&self) -> u64 {
        self.build_id
    }
}

/// Every script reports `Report::Unsupported(`[`INJECTED_REPORT`]`)` before running as the
/// inner game's: one `Unsupported` finding per map where a script runs.
#[derive(Clone, Copy, Debug)]
pub struct Reporting<G> {
    pub inner: G,
}

/// The name a [`Reporting`] script reports.
pub const INJECTED_REPORT: &str = "special InjectedReport";

impl<G: GameData> GameData for Reporting<G> {
    forward!(map has_map special_ids sprite player_sprite font window text ui_text behavior
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        let inner = self.inner.script(id, ctx.clone());
        Box::pin(async move {
            ctx.report(Report::Unsupported(INJECTED_REPORT)).await;
            inner.await;
        })
    }
}

/// `Err` naming the first Milestone 8 method (`wild_encounters` on every map with wild sets 0
/// and 1, then `item`, `multichoice`, `std_string`, `trainer_flag_of`, `emote`,
/// `trainer_money`, `evolutions` over their id ranges) where `a` and `b` answer differently.
/// `&'static` answers compare by address: a forwarding wrapper returns the inner game's own
/// tables.
pub fn same_answers(a: &impl GameData, b: &impl GameData) -> Result<(), String> {
    fn addr<T: ?Sized>(x: Option<&'static T>) -> Option<*const u8> {
        x.map(|r| std::ptr::from_ref(r).cast::<u8>())
    }
    let differ = |what: String| Err(format!("{what} differs"));
    for m in 0..=u16::MAX {
        let map = MapId(m);
        if !b.has_map(map) {
            continue;
        }
        for set in [0, 1] {
            if addr(a.wild_encounters(map, set)) != addr(b.wild_encounters(map, set)) {
                return differ(format!("wild_encounters({map:?}, {set})"));
            }
        }
    }
    for i in 0..=u16::MAX {
        if a.item(ItemId(i)) != b.item(ItemId(i)) {
            return differ(format!("item({i})"));
        }
        if a.trainer_flag_of(ScriptId(i)) != b.trainer_flag_of(ScriptId(i)) {
            return differ(format!("trainer_flag_of({i})"));
        }
        if a.trainer_money(TrainerId(i)) != b.trainer_money(TrainerId(i)) {
            return differ(format!("trainer_money({i})"));
        }
        if a.evolutions(SpeciesId(i)) != b.evolutions(SpeciesId(i)) {
            return differ(format!("evolutions({i})"));
        }
    }
    for i in 0..=u8::MAX {
        if a.multichoice(i) != b.multichoice(i) {
            return differ(format!("multichoice({i})"));
        }
        if a.std_string(i) != b.std_string(i) {
            return differ(format!("std_string({i})"));
        }
    }
    for e in [Emote::Exclamation, Emote::Question, Emote::Heart] {
        if addr(a.emote(e)) != addr(b.emote(e)) {
            return differ(format!("emote({e:?})"));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Patch {
    Wall,
    Pocket,
    Coord,
}

type Leaked = (Patch, usize, (i16, i16), &'static MapData);

/// Patched map copies, leaked once per (patch, base map, cell): `GameData::map` runs every frame.
/// Read-mostly: workers only take the read lock once a copy exists.
static LEAKED: RwLock<Vec<Leaked>> = RwLock::new(Vec::new());

fn leaked(patch: Patch, base: &'static MapData, at: (i16, i16)) -> &'static MapData {
    let addr = std::ptr::from_ref(base) as usize;
    let find = |all: &[Leaked]| {
        all.iter()
            .find(|&&(p, a, c, _)| p == patch && a == addr && c == at)
            .map(|&(.., m)| m)
    };
    if let Some(m) = find(&LEAKED.read().unwrap_or_else(|e| e.into_inner())) {
        return m;
    }
    let mut all = LEAKED.write().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = find(&all) {
        return m;
    }
    let m: &'static MapData = Box::leak(Box::new(match patch {
        Patch::Wall => walled(base, at, 1),
        Patch::Pocket => walled(base, at, 2),
        Patch::Coord => with_coord(base, at),
    }));
    all.push((patch, addr, at, m));
    m
}

/// `base` with the ring of cells at Chebyshev distance `radius` from `at` blocked, plus the
/// exit warp described at [`WallAround`].
fn walled(base: &'static MapData, at: (i16, i16), radius: i32) -> MapData {
    let (w, h) = (i32::from(base.width), i32::from(base.height));
    let at = (i32::from(at.0), i32::from(at.1));
    let ring = |x: i32, y: i32| (x - at.0).abs().max((y - at.1).abs()) == radius;
    let mut blocks = base.blocks.to_vec();
    for y in 0..h {
        for x in 0..w {
            if ring(x, y) {
                let b = &mut blocks[(y * w + x) as usize];
                *b = (*b & !(3 << BLOCK_COLLISION_SHIFT)) | (1 << BLOCK_COLLISION_SHIFT);
            }
        }
    }
    let exit = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .find(|&(x, y)| {
            (x - at.0).abs().max((y - at.1).abs()) > radius
                && engine::data::block_collision(blocks[(y * w + x) as usize]) == 0
        });
    let mut warps = copy_warps(base.warps);
    if let Some((x, y)) = exit {
        warps.push(WarpEvent {
            x: x as i16,
            y: y as i16,
            elevation: 0,
            dest_map: NOWHERE,
            dest_warp: 0,
        });
    }
    MapData {
        blocks: Box::leak(blocks.into_boxed_slice()),
        warps: Box::leak(warps.into_boxed_slice()),
        ..copy_map(base)
    }
}

fn with_coord(base: &'static MapData, at: (i16, i16)) -> MapData {
    let mut coords: Vec<CoordEvent> = vec![CoordEvent {
        x: at.0,
        y: at.1,
        elevation: 0,
        // A literal (below `VARS_START`): `var_or_literal(1) == 1`, so it always fires.
        var: VarId(1),
        value: 1,
        script: FADE_TRAP_SCRIPT,
    }];
    coords.extend(base.coords.iter().map(|c| CoordEvent {
        x: c.x,
        y: c.y,
        elevation: c.elevation,
        var: c.var,
        value: c.value,
        script: c.script,
    }));
    MapData {
        coords: Box::leak(coords.into_boxed_slice()),
        ..copy_map(base)
    }
}

fn copy_warps(warps: &[WarpEvent]) -> Vec<WarpEvent> {
    warps
        .iter()
        .map(|w| WarpEvent {
            x: w.x,
            y: w.y,
            elevation: w.elevation,
            dest_map: w.dest_map,
            dest_warp: w.dest_warp,
        })
        .collect()
}

/// A field-by-field copy (`MapData` is not `Clone`); the slices are shared.
fn copy_map(base: &'static MapData) -> MapData {
    let s = &base.scripts;
    MapData {
        name: base.name,
        width: base.width,
        height: base.height,
        blocks: base.blocks,
        border: base.border,
        primary: base.primary,
        secondary: base.secondary,
        objects: base.objects,
        warps: base.warps,
        coords: base.coords,
        bgs: base.bgs,
        connections: base.connections,
        scripts: MapScripts {
            on_load: s.on_load,
            on_transition: s.on_transition,
            on_resume: s.on_resume,
            on_return_to_field: s.on_return_to_field,
            on_frame: s.on_frame,
            on_warp_into: s.on_warp_into,
        },
        allow_running: base.allow_running,
    }
}
