//! Fault-injection `GameData` wrappers for tests (Task E). Each delegates every method
//! (provided ones too) to `inner`, like `Probe`, except the one it breaks.
//!
//! - [`PanicOnScript`]: a game panic (`Panic` findings, the guard).
//! - [`WallAround`]: a trapped player (`Softlock` findings).
//! - [`Drifting`]: state the checkpoint does not carry (`RoundTrip` findings).
//! - [`FadeTrap`]: a script that leaves the screen black (`CapturableStuck` findings).
//! - [`Rebuilt`]: another build id with the same behaviour (revalidation).

use engine::battle::{self, BattleGfx};
use engine::data::{
    BLOCK_COLLISION_SHIFT, Behavior, CoordEvent, Font, GameData, GfxId, MapData, MapId, MapScripts,
    NewGame, ScriptId, SpecialIds, SpriteData, TextId, TrainerId, UiText, VarId, WarpEvent,
    WindowGfx,
};
use engine::mon::{BattleTables, LandEncounters, TrainerData};
use engine::script::{Ctx, Fade, Script};
use std::cell::Cell;
use std::sync::Mutex;

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
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location);

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
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location);

    fn map(&self, id: MapId) -> &'static MapData {
        let base = self.inner.map(id);
        if id != self.map {
            return base;
        }
        leaked(Patch::Wall, base, self.at)
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
}

impl<G: Clone> Clone for Drifting<G> {
    /// A fresh counter (see [`Drifting`]).
    fn clone(&self) -> Self {
        Drifting::new(self.inner.clone())
    }
}

impl<G: GameData> GameData for Drifting<G> {
    forward!(map has_map special_ids sprite player_sprite font window text ui_text behavior
             script new_game build_id battle trainer battle_text battle_gfx heal_location);

    fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> {
        let n = self.calls.get();
        self.calls.set(n.saturating_add(1));
        if n < DRIFT_CALLS {
            None
        } else {
            self.inner.encounters(map)
        }
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
             new_game build_id battle encounters trainer battle_text battle_gfx heal_location);

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
             script new_game battle encounters trainer battle_text battle_gfx heal_location);

    fn build_id(&self) -> u64 {
        self.build_id
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Patch {
    Wall,
    Coord,
}

type Leaked = (Patch, usize, (i16, i16), &'static MapData);

/// Patched map copies, leaked once per (patch, base map, cell): `GameData::map` runs every frame.
static LEAKED: Mutex<Vec<Leaked>> = Mutex::new(Vec::new());

fn leaked(patch: Patch, base: &'static MapData, at: (i16, i16)) -> &'static MapData {
    let addr = std::ptr::from_ref(base) as usize;
    let mut all = LEAKED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&(.., m)) = all
        .iter()
        .find(|&&(p, a, c, _)| p == patch && a == addr && c == at)
    {
        return m;
    }
    let m: &'static MapData = Box::leak(Box::new(match patch {
        Patch::Wall => walled(base, at),
        Patch::Coord => with_coord(base, at),
    }));
    all.push((patch, addr, at, m));
    m
}

fn walled(base: &'static MapData, at: (i16, i16)) -> MapData {
    let (w, h) = (i32::from(base.width), i32::from(base.height));
    let at = (i32::from(at.0), i32::from(at.1));
    let ring = |x: i32, y: i32| (x - at.0).abs() <= 1 && (y - at.1).abs() <= 1 && (x, y) != at;
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
            !ring(x, y)
                && (x, y) != at
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
