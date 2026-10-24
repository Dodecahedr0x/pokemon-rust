//! Fault-injection `GameData` wrappers for tests (Task E). Each delegates every method
//! (provided ones too) to `inner` ([`forward_game_data!`](crate::forward_game_data)), except
//! the one it breaks.
//!
//! - [`PanicOnScript`]: a game panic (`Panic` findings, the guard).
//! - [`WallAround`]: a trapped player (`Softlock` findings).
//! - [`Drifting`]: state the checkpoint does not carry (`RoundTrip` findings).
//! - [`FadeTrap`]: a script that leaves the screen black (`CapturableStuck` findings).
//! - [`VarTrap`]: a cell that sets a story var (expected divergences).
//! - [`Rebuilt`]: another build id with the same behaviour (stale findings).
//! - [`Reporting`]: every script reports first (`Unsupported` findings).
//!
//! [`same_answers`] checks that a wrapper answers every provided method like its inner game,
//! with [`Answers`] (a game answering each one on its own) as the inner game.

use engine::battle::{self, BattleGfx};
use engine::data::{
    BLOCK_COLLISION_SHIFT, CoordEvent, Emote, Evolution, FlagId, GameData, ItemId, ItemInfo,
    MapData, MapId, MapScripts, ScriptId, SpecialIds, SpeciesId, SpriteData, TrainerId, VarId,
    WarpEvent,
};
use engine::mon::{BattleTables, LandEncounters, TrainerData};
use engine::script::{Ctx, Fade, Report, Script};
use std::cell::Cell;
use std::sync::RwLock;

/// `GameData` methods delegating to the field `$f` of `self`, one per name, for wrappers in
/// tests and tools: `forward_game_data!(inner; map has_map … m8)`. `m8` stands for the
/// Milestone 8 methods but `wild_encounters` (name it, or override it, separately: the engine
/// reads it), plus the Silice battle methods (`hold_effect`, `form_changes`), `map_type`
/// (the battle background) and `m9`: every provided method added since (menus, field moves,
/// puzzles, step-count scripts, encounters by method). Paths are absolute, so
/// the caller needs only the `engine` crate.
#[macro_export]
macro_rules! forward_game_data {
    ($f:tt; $($m:ident)*) => { $( $crate::forward_game_data!(@ $f $m); )* };
    (@ $f:tt map) => {
        fn map(&self, id: ::engine::data::MapId) -> &'static ::engine::data::MapData {
            self.$f.map(id)
        }
    };
    (@ $f:tt has_map) => {
        fn has_map(&self, id: ::engine::data::MapId) -> bool { self.$f.has_map(id) }
    };
    (@ $f:tt special_ids) => {
        fn special_ids(&self) -> ::engine::data::SpecialIds { self.$f.special_ids() }
    };
    (@ $f:tt sprite) => {
        fn sprite(
            &self,
            gfx: ::engine::data::GfxId,
        ) -> ::core::option::Option<&'static ::engine::data::SpriteData> {
            self.$f.sprite(gfx)
        }
    };
    (@ $f:tt player_sprite) => {
        fn player_sprite(&self) -> &'static ::engine::data::SpriteData {
            self.$f.player_sprite()
        }
    };
    (@ $f:tt font) => { fn font(&self) -> &'static ::engine::data::Font { self.$f.font() } };
    (@ $f:tt window) => {
        fn window(&self) -> &'static ::engine::data::WindowGfx { self.$f.window() }
    };
    (@ $f:tt text) => {
        fn text(&self, id: ::engine::data::TextId) -> &'static [u8] { self.$f.text(id) }
    };
    (@ $f:tt ui_text) => {
        fn ui_text(&self, t: ::engine::data::UiText) -> &'static [u8] { self.$f.ui_text(t) }
    };
    (@ $f:tt behavior) => {
        fn behavior(&self, raw: u8) -> ::engine::data::Behavior { self.$f.behavior(raw) }
    };
    (@ $f:tt script) => {
        fn script(
            &self,
            id: ::engine::data::ScriptId,
            ctx: ::engine::script::Ctx,
        ) -> ::engine::script::Script {
            self.$f.script(id, ctx)
        }
    };
    (@ $f:tt new_game) => {
        fn new_game(&self) -> ::engine::data::NewGame { self.$f.new_game() }
    };
    (@ $f:tt build_id) => { fn build_id(&self) -> u64 { self.$f.build_id() } };
    (@ $f:tt battle) => {
        fn battle(&self) -> &'static ::engine::mon::BattleTables { self.$f.battle() }
    };
    (@ $f:tt trainer) => {
        fn trainer(
            &self,
            id: ::engine::data::TrainerId,
        ) -> ::core::option::Option<&'static ::engine::mon::TrainerData> {
            self.$f.trainer(id)
        }
    };
    (@ $f:tt battle_text) => {
        fn battle_text(&self, s: ::engine::battle::Str) -> &'static [u8] {
            self.$f.battle_text(s)
        }
    };
    (@ $f:tt battle_gfx) => {
        fn battle_gfx(&self) -> &'static ::engine::battle::BattleGfx { self.$f.battle_gfx() }
    };
    (@ $f:tt heal_location) => {
        fn heal_location(
            &self,
            id: u8,
        ) -> ::core::option::Option<(::engine::data::MapId, i16, i16)> {
            self.$f.heal_location(id)
        }
    };
    (@ $f:tt wild_encounters) => {
        fn wild_encounters(
            &self,
            map: ::engine::data::MapId,
            wild_set: u16,
        ) -> ::core::option::Option<&'static ::engine::mon::LandEncounters> {
            self.$f.wild_encounters(map, wild_set)
        }
    };
    (@ $f:tt item) => {
        fn item(
            &self,
            id: ::engine::data::ItemId,
        ) -> ::core::option::Option<&'static ::engine::data::ItemInfo> {
            self.$f.item(id)
        }
    };
    (@ $f:tt multichoice) => {
        fn multichoice(&self, list: u8) -> ::core::option::Option<&'static [&'static [u8]]> {
            self.$f.multichoice(list)
        }
    };
    (@ $f:tt std_string) => {
        fn std_string(&self, id: u8) -> ::core::option::Option<&'static [u8]> {
            self.$f.std_string(id)
        }
    };
    (@ $f:tt trainer_flag_of) => {
        fn trainer_flag_of(
            &self,
            script: ::engine::data::ScriptId,
        ) -> ::core::option::Option<::engine::data::FlagId> {
            self.$f.trainer_flag_of(script)
        }
    };
    (@ $f:tt emote) => {
        fn emote(
            &self,
            e: ::engine::data::Emote,
        ) -> ::core::option::Option<&'static ::engine::data::SpriteData> {
            self.$f.emote(e)
        }
    };
    (@ $f:tt trainer_money) => {
        fn trainer_money(&self, id: ::engine::data::TrainerId) -> ::core::option::Option<u8> {
            self.$f.trainer_money(id)
        }
    };
    (@ $f:tt evolutions) => {
        fn evolutions(
            &self,
            species: ::engine::data::SpeciesId,
        ) -> &'static [::engine::data::Evolution] {
            self.$f.evolutions(species)
        }
    };
    (@ $f:tt hold_effect) => {
        fn hold_effect(
            &self,
            id: ::engine::data::ItemId,
        ) -> ::core::option::Option<::engine::mon::HoldEffect> {
            self.$f.hold_effect(id)
        }
    };
    (@ $f:tt form_changes) => {
        fn form_changes(&self) -> &'static [::engine::data::FormChange] {
            self.$f.form_changes()
        }
    };
    (@ $f:tt map_type) => {
        fn map_type(&self, map: ::engine::data::MapId) -> ::engine::data::MapType {
            self.$f.map_type(map)
        }
    };
    (@ $f:tt map_layout) => { fn map_layout(&self, id: ::engine::data::MapId, layout: u16) -> ::core::option::Option<&'static ::engine::data::MapData> { self.$f.map_layout(id, layout) } };
    (@ $f:tt trainer_battle_kind) => { fn trainer_battle_kind(&self, script: ::engine::data::ScriptId) -> ::core::option::Option<u8> { self.$f.trainer_battle_kind(script) } };
    (@ $f:tt start_menu) => { fn start_menu(&self) -> &'static [::engine::data::StartItem] { self.$f.start_menu() } };
    (@ $f:tt field_move_gate) => { fn field_move_gate(&self, m: ::engine::data::FieldMove) -> ::engine::data::FieldMoveGate { self.$f.field_move_gate(m) } };
    (@ $f:tt item_use) => { fn item_use(&self, id: ::engine::data::ItemId) -> ::engine::data::ItemUse { self.$f.item_use(id) } };
    (@ $f:tt mart_bonus) => { fn mart_bonus(&self) -> ::core::option::Option<(::engine::data::ItemId, u16, ::engine::data::ItemId)> { self.$f.mart_bonus() } };
    (@ $f:tt item_description) => { fn item_description(&self, id: ::engine::data::ItemId) -> ::core::option::Option<&'static [u8]> { self.$f.item_description(id) } };
    (@ $f:tt move_description) => { fn move_description(&self, id: ::engine::data::MoveId) -> ::core::option::Option<&'static [u8]> { self.$f.move_description(id) } };
    (@ $f:tt is_hm_move) => { fn is_hm_move(&self, m: ::engine::data::MoveId) -> bool { self.$f.is_hm_move(m) } };
    (@ $f:tt tm_move) => { fn tm_move(&self, id: ::engine::data::ItemId) -> ::core::option::Option<::engine::data::MoveId> { self.$f.tm_move(id) } };
    (@ $f:tt can_learn_tm) => { fn can_learn_tm(&self, species: ::engine::data::SpeciesId, tm: ::engine::data::ItemId) -> bool { self.$f.can_learn_tm(species, tm) } };
    (@ $f:tt ui_gfx) => { fn ui_gfx(&self, g: ::engine::data::UiGfx) -> ::core::option::Option<&'static ::engine::data::UiPicture> { self.$f.ui_gfx(g) } };
    (@ $f:tt window_frame) => { fn window_frame(&self, n: u8) -> ::core::option::Option<&'static ::engine::data::WindowGfx> { self.$f.window_frame(n) } };
    (@ $f:tt item_icon) => { fn item_icon(&self, id: ::engine::data::ItemId) -> ::core::option::Option<&'static ::engine::data::Icon> { self.$f.item_icon(id) } };
    (@ $f:tt mon_icon) => { fn mon_icon(&self, species: ::engine::data::SpeciesId) -> ::core::option::Option<&'static ::engine::data::Icon> { self.$f.mon_icon(species) } };
    (@ $f:tt in_game_trade) => { fn in_game_trade(&self, n: u8) -> ::core::option::Option<&'static ::engine::data::InGameTrade> { self.$f.in_game_trade(n) } };
    (@ $f:tt nature_name) => { fn nature_name(&self, n: u8) -> ::core::option::Option<&'static [u8]> { self.$f.nature_name(n) } };
    (@ $f:tt step_callback) => { fn step_callback(&self, map: ::engine::data::MapId) -> ::core::option::Option<::engine::data::StepCallback> { self.$f.step_callback(map) } };
    (@ $f:tt hole_warp) => { fn hole_warp(&self, map: ::engine::data::MapId) -> ::core::option::Option<::engine::data::MapId> { self.$f.hole_warp(map) } };
    (@ $f:tt fall_script) => { fn fall_script(&self) -> ::core::option::Option<::engine::data::ScriptId> { self.$f.fall_script() } };
    (@ $f:tt step_count_script) => { fn step_count_script(&self, step: ::engine::data::StepCount<'_>) -> ::core::option::Option<::engine::data::ScriptId> { self.$f.step_count_script(step) } };
    (@ $f:tt field_move_script) => { fn field_move_script(&self, m: ::engine::data::FieldMove) -> ::core::option::Option<::engine::data::ScriptId> { self.$f.field_move_script(m) } };
    (@ $f:tt waterfall_refused_script) => { fn waterfall_refused_script(&self) -> ::core::option::Option<::engine::data::ScriptId> { self.$f.waterfall_refused_script() } };
    (@ $f:tt dive_emerge_script) => { fn dive_emerge_script(&self) -> ::core::option::Option<::engine::data::ScriptId> { self.$f.dive_emerge_script() } };
    (@ $f:tt fly_destination) => { fn fly_destination(&self, i: u8) -> ::core::option::Option<::engine::data::FlyDestination> { self.$f.fly_destination(i) } };
    (@ $f:tt dive_warp) => { fn dive_warp(&self, map: ::engine::data::MapId, emerge: bool, at: (i16, i16)) -> ::core::option::Option<(::engine::data::MapId, ::core::option::Option<(i16, i16)>)> { self.$f.dive_warp(map, emerge, at) } };
    (@ $f:tt water_encounters) => { fn water_encounters(&self, map: ::engine::data::MapId, wild_set: u16) -> ::core::option::Option<&'static ::engine::mon::WaterEncounters> { self.$f.water_encounters(map, wild_set) } };
    (@ $f:tt rock_smash_encounters) => { fn rock_smash_encounters(&self, map: ::engine::data::MapId, wild_set: u16) -> ::core::option::Option<&'static ::engine::mon::WaterEncounters> { self.$f.rock_smash_encounters(map, wild_set) } };
    (@ $f:tt pushable_boulder) => { fn pushable_boulder(&self) -> ::core::option::Option<(::engine::data::GfxId, ::engine::data::FlagId)> { self.$f.pushable_boulder() } };
    (@ $f:tt rotating_gates) => { fn rotating_gates(&self, map: ::engine::data::MapId) -> &'static [::engine::data::RotatingGate] { self.$f.rotating_gates(map) } };
    (@ $f:tt player_surf_sprite) => { fn player_surf_sprite(&self) -> ::core::option::Option<&'static ::engine::data::SpriteData> { self.$f.player_surf_sprite() } };
    (@ $f:tt map_weather) => { fn map_weather(&self, map: ::engine::data::MapId) -> u8 { self.$f.map_weather(map) } };
    (@ $f:tt weather_coords) => { fn weather_coords(&self, map: ::engine::data::MapId) -> &'static [::engine::data::CoordWeather] { self.$f.weather_coords(map) } };
    (@ $f:tt weather_look) => { fn weather_look(&self, id: u8) -> ::core::option::Option<&'static ::engine::weather::Look> { self.$f.weather_look(id) } };
    (@ $f:tt overlay) => { fn overlay(&self, id: u8) -> ::core::option::Option<&'static ::engine::sky::Overlay> { self.$f.overlay(id) } };
    (@ $f:tt hidden_item_script) => { fn hidden_item_script(&self) -> ::core::option::Option<::engine::data::ScriptId> { self.$f.hidden_item_script() } };
    (@ $f:tt m8) => {
        $crate::forward_game_data!($f; item multichoice std_string trainer_flag_of emote
                                   trainer_money evolutions hold_effect form_changes map_type m9);
    };
    (@ $f:tt m9) => {
        $crate::forward_game_data!($f; map_layout trainer_battle_kind start_menu field_move_gate item_use mart_bonus item_description move_description is_hm_move tm_move can_learn_tm ui_gfx window_frame item_icon mon_icon in_game_trade nature_name step_callback hole_warp fall_script step_count_script field_move_script waterfall_refused_script dive_emerge_script dive_warp fly_destination water_encounters rock_smash_encounters pushable_boulder player_surf_sprite rotating_gates map_weather weather_coords weather_look overlay hidden_item_script);
    };
}

/// [`forward_game_data!`] on the field `inner`.
macro_rules! forward {
    ($($m:ident)*) => { $crate::forward_game_data!(inner; $($m)*); };
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
             new_game build_id battle trainer battle_text battle_gfx heal_location
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
             new_game build_id battle trainer battle_text battle_gfx heal_location
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
             new_game build_id battle trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn map(&self, id: MapId) -> &'static MapData {
        let base = self.inner.map(id);
        if id != self.map {
            return base;
        }
        leaked(Patch::Pocket, base, self.at)
    }
}

/// `wild_encounters()` depends on a call counter: it answers `None` for the first [`DRIFT_CALLS`]
/// calls of an instance. A clone starts again at 0 calls: the counter is state that a
/// checkpoint does not carry, so a session restored on a clone drifts from the live one at the
/// next tall-grass step (round-trip break).
#[derive(Debug)]
pub struct Drifting<G> {
    pub inner: G,
    pub calls: Cell<u32>,
}

/// Calls of [`Drifting::wild_encounters`] answered `None`.
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

    /// What the engine reads.
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
             new_game build_id battle trainer battle_text battle_gfx heal_location
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

/// [`FadeTrap`]'s coord event on `at`, but its script sets the story var [`VAR_TRAP_VAR`] to 1
/// and ends: a branch that steps there changes the progress state (an expected divergence).
#[derive(Clone, Copy, Debug)]
pub struct VarTrap<G> {
    pub inner: G,
    pub map: MapId,
    pub at: (i16, i16),
}

/// The story var a [`VarTrap`] sets.
pub const VAR_TRAP_VAR: VarId = VarId(0x40F0);

impl<G: GameData> GameData for VarTrap<G> {
    forward!(has_map special_ids sprite player_sprite font window text ui_text behavior
             new_game build_id battle trainer battle_text battle_gfx heal_location
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
        Box::pin(async move { ctx.set_var(VAR_TRAP_VAR, 1).await })
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
             script new_game battle trainer battle_text battle_gfx heal_location
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
             new_game build_id battle trainer battle_text battle_gfx heal_location
             wild_encounters m8);

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        let inner = self.inner.script(id, ctx.clone());
        Box::pin(async move {
            ctx.report(Report::Unsupported(INJECTED_REPORT)).await;
            inner.await;
        })
    }
}

/// `Err` naming the first provided `GameData` method (one with a default) where `a` and `b`
/// answer differently, over its id ranges: `has_map`, `encounters` and `wild_encounters`
/// (wild sets 0 and 1) on every map id, `special_ids`, `build_id`, `battle`, `battle_gfx`,
/// `trainer` and the Milestone 8 id methods over every id, `battle_text` on a sample of strings,
/// `heal_location`, `multichoice` and `std_string` over every `u8`, `emote` on every emote.
/// `&'static` answers compare by address: a forwarding wrapper returns the inner game's own
/// tables.
pub fn same_answers(a: &impl GameData, b: &impl GameData) -> Result<(), String> {
    fn addr<T: ?Sized>(x: Option<&'static T>) -> Option<*const u8> {
        x.map(|r| std::ptr::from_ref(r).cast::<u8>())
    }
    let differ = |what: String| Err(format!("{what} differs"));
    if a.special_ids() != b.special_ids() {
        return differ("special_ids()".into());
    }
    if a.build_id() != b.build_id() {
        return differ("build_id()".into());
    }
    if addr(Some(a.battle())) != addr(Some(b.battle())) {
        return differ("battle()".into());
    }
    if addr(Some(a.battle_gfx())) != addr(Some(b.battle_gfx())) {
        return differ("battle_gfx()".into());
    }
    for s in BATTLE_TEXT_SAMPLE {
        if a.battle_text(s) != b.battle_text(s) {
            return differ(format!("battle_text({s:?})"));
        }
    }
    for m in 0..=u16::MAX {
        let map = MapId(m);
        if a.has_map(map) != b.has_map(map) {
            return differ(format!("has_map({map:?})"));
        }
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
        if addr(a.trainer(TrainerId(i))) != addr(b.trainer(TrainerId(i))) {
            return differ(format!("trainer({i})"));
        }
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
        if a.hold_effect(ItemId(i)) != b.hold_effect(ItemId(i)) {
            return differ(format!("hold_effect({i})"));
        }
    }
    if a.form_changes() != b.form_changes() {
        return differ("form_changes()".into());
    }
    for i in 0..=u8::MAX {
        if a.heal_location(i) != b.heal_location(i) {
            return differ(format!("heal_location({i})"));
        }
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

/// The battle strings [`same_answers`] compares (`battle::Str` has no list of its variants).
const BATTLE_TEXT_SAMPLE: [battle::Str; 4] = [
    battle::Str::Intro1,
    battle::Str::IntroTrainer,
    battle::Str::GoPkmn,
    battle::Str::PkmnFainted,
];

/// A game with an answer of its own for every provided `GameData` method (the required ones
/// are `inner`'s): the fixture for [`same_answers`], against which a wrapper that forgets to
/// forward a method answers the default instead.
#[derive(Clone, Copy, Debug)]
pub struct Answers<G> {
    pub inner: G,
}

static LAND: LandEncounters = LandEncounters {
    rate: 7,
    mons: [engine::mon::WildMon {
        species: SpeciesId(1),
        min_level: 2,
        max_level: 3,
    }; 12],
};
static ITEM: ItemInfo = ItemInfo {
    name: &[0xBB, 0xFF],
    pocket: engine::data::Pocket::KeyItems,
    price: 5,
    key: true,
};
static CHOICES: [&[u8]; 2] = [&[0xBB, 0xFF], &[0xBC, 0xFF]];
static EVOS: [Evolution; 1] = [Evolution {
    method: engine::data::EvoMethod::Level,
    param: 16,
    target: SpeciesId(2),
}];
static TABLES: std::sync::OnceLock<BattleTables> = std::sync::OnceLock::new();
static GFX: std::sync::OnceLock<BattleGfx> = std::sync::OnceLock::new();
static TRAINER: std::sync::OnceLock<Option<TrainerData>> = std::sync::OnceLock::new();

impl<G: GameData> GameData for Answers<G> {
    forward!(map sprite player_sprite font window text ui_text behavior script new_game has_map);

    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            respawn_default: self.inner.special_ids().respawn_default.wrapping_add(1),
            ..self.inner.special_ids()
        }
    }
    fn build_id(&self) -> u64 {
        self.inner.build_id() ^ 0xA115
    }
    /// A copy of the inner game's tables (another address).
    fn battle(&self) -> &'static BattleTables {
        TABLES.get_or_init(|| *self.inner.battle())
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        GFX.get_or_init(|| *self.inner.battle_gfx())
    }
    fn battle_text(&self, s: battle::Str) -> &'static [u8] {
        if s == battle::Str::GoPkmn {
            &[0xBE, 0xFF]
        } else {
            self.inner.battle_text(s)
        }
    }
    fn wild_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters> {
        if wild_set == 1 || map == self.inner.new_game().map {
            Some(&LAND)
        } else {
            self.inner.wild_encounters(map, wild_set)
        }
    }
    /// Trainer 1 is a copy of the inner game's first trainer (`None` when it has none, as on
    /// the stub). The copies (`battle`, `battle_gfx`, `trainer`) are made once per process,
    /// from the first inner game asked.
    fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
        let copy = TRAINER
            .get_or_init(|| {
                (1..=u16::MAX)
                    .find_map(|i| self.inner.trainer(TrainerId(i)))
                    .copied()
            })
            .as_ref();
        if id == TrainerId(1) {
            copy
        } else {
            self.inner.trainer(id)
        }
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        if id == 200 {
            Some((MapId(1), 2, 3))
        } else {
            self.inner.heal_location(id)
        }
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        (id == ItemId(0x105)).then_some(&ITEM)
    }
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
        (list == 3).then_some(&CHOICES[..])
    }
    fn std_string(&self, id: u8) -> Option<&'static [u8]> {
        (id == 4).then_some(&[0xBD, 0xFF][..])
    }
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        (script == ScriptId(6)).then_some(FlagId(0x500))
    }
    fn emote(&self, e: Emote) -> Option<&'static SpriteData> {
        (e == Emote::Heart).then(|| self.inner.player_sprite())
    }
    fn trainer_money(&self, id: TrainerId) -> Option<u8> {
        (id == TrainerId(7)).then_some(9)
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        if species == SpeciesId(8) { &EVOS } else { &[] }
    }
    fn hold_effect(&self, id: ItemId) -> Option<engine::mon::HoldEffect> {
        (id == ItemId(0x106)).then_some(engine::mon::HoldEffect::Leftovers)
    }
    fn form_changes(&self) -> &'static [engine::data::FormChange] {
        &FORMS
    }
}

static FORMS: [engine::data::FormChange; 1] = [engine::data::FormChange {
    base: SpeciesId(3),
    item: ItemId(0x107),
    form: SpeciesId(4),
    kind: engine::data::FormKind::Mega,
    enable_flag: None,
}];

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
        map_type: base.map_type,
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

/// The spine of [`fill_db`]: two waits in the walled-in new game.
pub static FILL_STEPS: &[crate::spine::Step] = &[
    crate::spine::Step::Checkpoint("start"),
    crate::spine::Step::Wait(200),
    crate::spine::Step::Checkpoint("mid"),
    crate::spine::Step::Wait(200),
    crate::spine::Step::Checkpoint("end"),
];

/// Fills the db at `path` through the explore API (the studio's tests): the spine
/// [`FILL_STEPS`] of `inner` walled in at its new-game cell ([`WallAround`]), its branches, and
/// one more branch with a `Softlock` finding (the walled-in player). Returns the number of
/// branches stored.
pub fn fill_db<G: GameData + Clone + Send + Sync + 'static>(
    inner: G,
    path: &std::path::Path,
) -> anyhow::Result<usize> {
    use crate::branch::{self, Action, BranchResult, Outcome, Target, TargetKind};
    use crate::config::Config;
    use crate::db::{Db, RunRow};
    use crate::{Macro, MacroKind};
    let ng = inner.new_game();
    let game = WallAround {
        inner,
        map: ng.map,
        at: (ng.x, ng.y),
    };
    let cfg = Config::default();
    let s = crate::spine::run(&game, FILL_STEPS, &cfg);
    anyhow::ensure!(s.complete(), "the fill spine stopped: {:?}", s.error);
    let (mut db, _) = Db::open(path, engine::checkpoint::build_hash(&game), content::FLAVOR)?;
    let row = |command: &str, branches: u64| RunRow {
        started: "test".into(),
        command: command.into(),
        config: cfg.to_ron(),
        threads: 1,
        branches,
        frames: s.frames,
        wall_ms: 1,
    };
    let run = db.record_run(&row("spine", 0))?;
    db.put_spine(&s, 1, run)?;
    let origins = branch::origins(&s, &cfg.branch, None);
    let targets = branch::all_targets(&game, &s, &origins, &cfg);
    let mut results = branch::run_all(&game, &s, &targets, &cfg);
    let end = s.positions.last().expect("a position");
    results.push(BranchResult {
        target: Target {
            origin: end.index,
            action: Action::Macro(Macro::of(MacroKind::Wait { frames: 1 })),
            kind: TargetKind::Cells,
        },
        outcome: Outcome::Bug,
        explored: None,
        macros: Vec::new(),
        explore_frames: 1,
        recover_frames: 0,
        window_frames: 0,
        gain: crate::spine::Gain::default(),
        points: 0,
        coverage: crate::spine::Coverage::default(),
        findings: vec![crate::sim::softlock_finding(&end.end, 0, 1, 1)],
        reports: Vec::new(),
        message: "walled in".into(),
    });
    let run = db.record_run(&row("branch", results.len() as u64))?;
    db.put_branches(&s, &results, run)?;
    Ok(results.len())
}
