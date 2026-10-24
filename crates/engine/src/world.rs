//! The overworld: composes the field, objects, text box, flow state and the script runner, and
//! implements [`ScriptHost`].
//!
//! Frame order of [`World::step`] (Gen3 `CB1_Overworld` / `CB2_Overworld` order where it
//! matters):
//! 1. input edges;
//! 2. a warp transition, if one is active (it drives the fader);
//! 3. else the running script, if any;
//! 4. else player control: `on_frame` map scripts, arrow warps, A interactions, door warps,
//!    then the player step and its step-end triggers (coord events, then warp cells) and
//!    connection crossing at step start;
//! 5. objects (NPC idle movement, movement scripts, the player's step when 4 did not run);
//! 6. the fader (when no transition drives it);
//! 7. the camera follows the player (field metatiles drawn as they come into view);
//! 8. object sprites;
//! 9. the text box.
//!
//! Map loads follow Gen3 `LoadMapFromWarp` + `LoadMapInStepsLocal`: temp flags and vars are
//! cleared, then `on_transition` (before the field switches maps, as Gen3 runs it before
//! `InitMap`), `on_load` (before the first draw, so `setmetatile` lands first), `on_resume`,
//! object spawning, then `on_warp_into`. A connection crossing runs `on_transition`, `on_load`
//! and `on_resume` the same way (`LoadMapFromCameraTransition`). Map scripts run to completion
//! at once (Gen3 `RunScriptImmediately`); a waiting request there is logged and answered at
//! once, and text, fades and warps there are logged and ignored. A warp load ends the running
//! script (Gen3 `CB2_LoadMap` resets the script context).

mod audio;
mod battle;
pub(crate) mod caught;
mod checkpoint;
mod evolve;
mod field_move;
pub(crate) mod friendship;
mod host;
mod maps;
mod mode;
mod puzzle;
mod save;
pub use save::MainMenuChoice;
mod screens;
mod sight;
mod templates;
#[cfg(test)]
mod tests;

pub use battle::{ENCOUNTER_IMMUNITY_STEPS, Origin, Stage, WHITEOUT_FRAMES};

use crate::bag::Bag;
use crate::battle::BattleOutcome;
use crate::battle::scene::BattleScene;
use crate::checkpoint::{Checkpoint, SaveError};
use crate::checkpoint::{PLAYER_NAME_SIZE, PlayerName};
use crate::data::ItemId;
use crate::data::{
    Behavior, BgKind, Dir, FlagId, GameData, MapId, ObjectEvent, PLAYER, ScriptId, SpecialIds,
    VarId,
};
use crate::field::Field;
use crate::flow::{
    Fader, Input, Target, Transition, Vars, block_behavior, coord_trigger, door_warp,
    facing_elevation, interact_target, map_script_on_frame, warp_on_enter, warp_on_push,
};
use crate::fx::ScreenFx;
use crate::hash::Fnv64;
use crate::mon::Party;
use crate::objects::{Objects, PlayerEvent, PlayerInput, Terrain};
use crate::options::Options;
use crate::pc::PcStorage;
use crate::pokedex::Pokedex;
use crate::script::{Fade, Report, Runner, Speed, Step};
use crate::starter::StarterChoice;
use crate::str_vars::{STR_VAR_LEN, StrVars};
use crate::text::TextBox;
use crate::ui::ScreenKindTag;
use crate::video::Frame;
use crate::{Buttons, Rng};
use alloc::vec::Vec;
use host::Host;
use maps::{cross_connection, load_map};
use mode::Mode;

/// Screen pixel of the player's cell top-left when the camera is centered on the player.
pub const CAMERA_OFFSET: (i32, i32) = (112, 64);
/// Gen3 door animation length: 4 frames shown 5 ticks each (`sDoorOpenAnimFrames`,
/// `AnimateDoorFrame`). Doors are not drawn yet; `Door` only takes this long.
pub const DOOR_FRAMES: u8 = 20;
/// Entries kept in the script log ring.
pub const LOG_LEN: usize = 16;
/// Immediate map scripts get this many runner steps (each up to `INSTANT_LIMIT` requests).
const IMMEDIATE_STEPS: u32 = 64;
/// [`Event`]s kept per frame; further ones are dropped and counted ([`World::events_dropped`]).
pub const EVENTS_MAX: usize = 32;
/// The deterministic clock's day: 24 hours at 60 frames a second ([`World::days`]).
pub const FRAMES_PER_DAY: u32 = 60 * 60 * 60 * 24;

/// Something that happened this frame, for tools (the explorer, the sweep, parity lanes). Not
/// hashed, not snapshotted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    /// A warp load or connection crossing entered this map.
    MapEntered(MapId),
    /// A script started (object, coord, bg, map and immediate scripts alike).
    ScriptStarted(ScriptId),
    BattleStarted,
    BattleEnded(BattleOutcome),
    /// A script's `Request::Report`.
    Report(Report),
    /// A screen opened (START, a request, or a screen over another one).
    ScreenOpened(ScreenKindTag),
    /// The last screen closed and the field is back.
    ScreenClosed,
    /// The in-game save ran: the shells persist [`World::save_snapshot`] when `ok`.
    Saved {
        ok: bool,
    },
    /// A party mon evolved into this species.
    Evolved(crate::data::SpeciesId),
    /// A script asked for the last save to be reloaded (`Request::ReloadLastSave`): the shells
    /// replace the World by it (GBA: the flash slot, without writing it; native: the last save
    /// they persisted, else a new game).
    ReloadLastSave,
}

/// The last script log lines (`Request::Log`), for tests and debugging.
#[derive(Clone, Copy, Debug)]
pub struct Log {
    lines: [&'static str; LOG_LEN],
    count: u32,
}

impl Log {
    const fn new() -> Self {
        Self {
            lines: [""; LOG_LEN],
            count: 0,
        }
    }

    fn push(&mut self, line: &'static str) {
        self.lines[self.count as usize % LOG_LEN] = line;
        self.count = self.count.wrapping_add(1);
    }

    /// Lines logged since the start.
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// The kept lines, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &'static str> + '_ {
        let n = (self.count as usize).min(LOG_LEN);
        let start = self.count as usize - n;
        (start..self.count as usize).map(|i| self.lines[i % LOG_LEN])
    }
}

/// A warp destination a script stored (Gen3 `struct WarpData`): warp `warp` of `map`, else
/// cell (x, y).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WarpDest {
    pub map: MapId,
    pub warp: Option<u8>,
    pub x: i16,
    pub y: i16,
}

/// The warps scripts set for later (Gen3 `gSaveBlock1Ptr->dynamicWarp` and `escapeWarp`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WarpSlots {
    pub dynamic: Option<WarpDest>,
    pub escape: Option<WarpDest>,
}

impl WarpDest {
    fn hash(d: Option<WarpDest>, h: &mut Fnv64) {
        match d {
            None => h.write(&[0]),
            Some(d) => {
                h.write(&[1, d.warp.unwrap_or(0xFF), d.warp.is_some() as u8]);
                h.write_u16(d.map.0);
                h.write_u16(d.x as u16);
                h.write_u16(d.y as u16);
            }
        }
    }
}

/// Everything the script host touches (all of the world but the runner).
pub(crate) struct State {
    pub(crate) rng: Rng,
    pub(crate) input: Input,
    pub(crate) field: Field,
    pub(crate) objects: Objects,
    pub(crate) text: TextBox,
    pub(crate) vars: Vars,
    pub(crate) fader: Fader,
    pub(crate) transition: Transition,
    /// The current map's object templates for this visit (scripts move them and change their
    /// movement type), with their hash digest.
    pub(crate) templates: templates::Templates,
    /// The object whose interaction started the running script (Gen3 `gSelectedObjectEvent`):
    /// local id and map (the previous map's, for an NPC left in view by a crossing). Every
    /// script start sets it and captures happen outside scripts, so checkpoints and the hash
    /// keep the local id only (a restore puts it on the current map).
    pub(crate) selected: Option<(u8, MapId)>,
    /// Object of the last `Move` (Gen3 `sMovingNpcId`), for `WaitMove(0)`.
    pub(crate) last_mover: u8,
    /// The running warp keeps the map's object templates (Gen3 `sObjectEventLoadFlag ==
    /// SKIP_OBJECT_EVENT_LOAD`, set by `DoMossdeepGymWarp`): only while a transition runs
    /// (never captured), hashed.
    pub(crate) keep_templates: bool,
    /// Frames left of a script `Delay`.
    pub(crate) delay: u16,
    /// Frames left of a door animation.
    pub(crate) door: u8,
    pub(crate) log: Log,
    /// The player's party (Gen3 `gPlayerParty`).
    pub(crate) party: Party,
    /// Heal location of the last `setrespawn` (Gen3 `gSaveBlock1Ptr->lastHealLocation`).
    pub(crate) respawn: u8,
    /// The player's trainer id, the OT id of their mons (Gen3 `playerTrainerId`).
    pub(crate) player_id: u32,
    /// Gen3 `sPrevMetatileBehavior`: the cell behavior of the last checked step.
    pub(crate) prev_behavior: Behavior,
    /// Gen3 `sWildEncounterImmunitySteps`: steps since the last
    /// `RestartWildEncounterImmunitySteps` (map loads and battles), up to
    /// [`ENCOUNTER_IMMUNITY_STEPS`].
    pub(crate) encounter_immunity: u8,
    /// What owns the screen: the field, the starter screen or a battle.
    pub(crate) mode: Mode,
    /// The answer to the script's pending `ChooseStarter` / `BattleWild` / `BattleTrainer`.
    pub(crate) answer: Option<u16>,
    /// Gen3-encoded player name, 0xFF-terminated and 0xFF-padded (Gen3 `playerName`). Copied
    /// from `NewGame` at a new game; hashed last in `World::hash`. Texts, the starter screen and
    /// battles print this copy.
    pub(crate) player_name: PlayerName,
    // ---- Milestone 8 ----
    /// The player's bag (Gen3 `gSaveBlock1Ptr->bagPocket_*`).
    pub(crate) bag: Bag,
    /// Gen3 `gSaveBlock1Ptr->money`.
    pub(crate) money: u32,
    /// Gen3 `gStringVar1..3`.
    pub(crate) str_vars: StrVars,
    pub(crate) warp_slots: WarpSlots,
    // ---- Milestone 9 ----
    /// Gen3 `gSaveBlock2Ptr->optionsTextSpeed` and the other options.
    pub(crate) options: Options,
    /// Gen3 `gSaveBlock2Ptr->pokedex.seen/owned`.
    pub(crate) pokedex: Pokedex,
    /// Gen3 `pcItems` and `gPokemonStoragePtr`.
    pub(crate) pc: PcStorage,
    /// Gen3 `gSaveBlock1Ptr->coins`.
    pub(crate) coins: u16,
    /// Gen3 `gSaveBlock1Ptr->registeredItem` (SELECT).
    pub(crate) registered_item: Option<ItemId>,
    /// Wave E: Gen3 `berryTrees` (`SpecialIds::berry_trees` of them).
    pub(crate) berry_trees: crate::berry::BerryTrees,
    /// Screen flash, shake and tint (not snapshotted: a save or capture needs it clear).
    pub(crate) fx: ScreenFx,
    /// The field weather (`crate::weather`): hashed and snapshotted unless it is the map's
    /// settled default.
    pub(crate) weather: crate::weather::Weather,
    /// A script's palette override (`Request::SetPalette`): hashed and snapshotted while set.
    pub(crate) palette: Option<crate::sky::PaletteFx>,
    /// What the screen shows of the weather and the override (display only).
    pub(crate) sky: crate::sky::Display,
    /// Running field effects and their arguments (Wave C; not snapshotted, not hashed).
    pub(in crate::world) field_effects: host::FieldEffects,
    /// The rotating tile puzzle between `initrotatingtilepuzzle` and `freerotatingtilepuzzle`
    /// (Wave F; Gen3 `sRotatingTilePuzzle`, script RAM: not snapshotted, not hashed).
    pub(in crate::world) rotating_tiles: Option<host::RotatingTiles>,
    /// The running field-move effect (Wave C; hashed while it runs, never captured).
    pub(in crate::world) field_move: Option<field_move::FieldMoveRun>,
    /// Wave F: the party's backup and the choose-half order while a script uses them
    /// (hashed while not empty, never captured: cleared when the script ends).
    pub(in crate::world) stash: host::Stash,
    /// Wave G: a two-trainer sighting while its scripts run (Gen3 `gApproachingTrainers[1]`,
    /// `gTrainerBattleOpponent_B`): hashed while set, never captured (cleared when the script
    /// ends).
    pub(in crate::world) pair: sight::Pair,
    // Tool-facing, neither hashed nor snapshotted:
    /// This step's events (cleared at the start of `step`, except the first step of a new
    /// world, which keeps `World::new`'s), at most [`EVENTS_MAX`].
    pub(crate) events: Vec<Event>,
    /// Events dropped past [`EVENTS_MAX`] since the world was created or restored.
    pub(crate) events_dropped: u32,
    /// The last script started.
    pub(crate) last_script: Option<ScriptId>,
    /// Task S: Gen3 `gSaveFileStatus` / `gDifferentSaveFile` and the Hall of Fame's pending
    /// save (`world/save.rs`; hashed only while a save is pending).
    pub(crate) save: save::SaveStatus,
    /// The clock's day ([`World::days`]), copied at each step's start for the map loads'
    /// daily reset ([`State::update_per_day`]); derived, not state.
    pub(crate) today: u16,
    /// Milestone 12: the music players and sound tasks the game reads ([`crate::audio`]).
    pub(crate) audio: crate::audio::Audio,
}

/// All mutable game state. Deterministic: the same state and input give the same next state.
pub struct World {
    frame: u32,
    /// The hardware clock's day (an RTC read by the shell), or `None`: the deterministic
    /// clock, [`FRAMES_PER_DAY`] frames a day. A host input: neither hashed nor snapshotted.
    rtc_days: Option<u16>,
    runner: Runner,
    s: State,
    /// The first frame's uploads, built by `new` and taken by `initial_frame` (not state).
    boot: Option<Frame>,
}

/// The field blocks (with `setmetatile` edits and connections) and their behaviors.
struct Ground<'a, G: GameData> {
    game: &'a G,
    field: &'a Field,
}

impl<G: GameData> Terrain for Ground<'_, G> {
    fn block(&self, x: i16, y: i16) -> u16 {
        self.field.block(self.game, x, y)
    }

    fn behavior(&self, x: i16, y: i16) -> crate::data::Behavior {
        let b = self.field.block(self.game, x, y);
        block_behavior(self.game, self.field.data(self.game), b)
    }

    fn push_allowed(&self, local_id: u8, from: (i16, i16), to: (i16, i16)) -> bool {
        self.game.push_allowed(self.field.map(), local_id, from, to)
    }
}

/// One forced step per direction (warp walk-in/out).
/// Gen3 `MOVEMENT_ACTION_JUMP_*` (one cell), by `Dir`: a pop-out arrival's jump.
static JUMP: [[Step; 1]; 4] = [
    [Step::Jump(Dir::Down, 1)],
    [Step::Jump(Dir::Up, 1)],
    [Step::Jump(Dir::Left, 1)],
    [Step::Jump(Dir::Right, 1)],
];

static WALK: [[Step; 1]; 4] = [
    [Step::Walk(Dir::Down, Speed::Normal)],
    [Step::Walk(Dir::Up, Speed::Normal)],
    [Step::Walk(Dir::Left, Speed::Normal)],
    [Step::Walk(Dir::Right, Speed::Normal)],
];

const fn camera_for(pixel: (i32, i32)) -> (i32, i32) {
    (pixel.0 - CAMERA_OFFSET.0, pixel.1 - CAMERA_OFFSET.1)
}

impl State {
    fn ground<'a, G: GameData>(&'a self, game: &'a G) -> Ground<'a, G> {
        Ground {
            game,
            field: &self.field,
        }
    }

    fn template(&self, local: u8) -> Option<&ObjectEvent> {
        self.templates.iter().find(|t| t.local_id == local)
    }

    /// Edits template `local` (if any) with `f`.
    fn edit_template(&mut self, local: u8, f: impl FnOnce(&mut ObjectEvent)) {
        self.templates.edit(local, f);
    }

    /// Gen3 `ClearTempFieldEventData`, then `DoTimeBasedEvents`'s daily reset (both run on
    /// every map load, before the map's `on_transition`).
    fn clear_temp(&mut self, ids: &SpecialIds) {
        self.vars.clear_temp();
        for &f in ids.flags_cleared_on_map_change {
            self.vars.set_flag(f, false);
        }
        // The input map is the map's own: its scripts set it again (`SetInputMap`).
        self.set_var_opt(ids.var_input_map, 0);
        self.update_per_day(ids);
    }

    /// A map load's weather and palette (before `on_transition`): Gen3
    /// `SetSavedWeatherFromCurrMapHeader`; a palette override goes unless kept.
    fn enter_sky(&mut self, game: &impl GameData, map: MapId) {
        self.weather.load_header(game.map_weather(map));
        if self.palette.is_some_and(|p| !p.keep) {
            self.palette = None;
        }
    }

    /// Gen3 `UpdatePerDay` (`clock.c`): once the clock's day has passed the day kept in
    /// `SpecialIds::var_days`, the daily flags are cleared (`ClearDailyFlags`) and the day is
    /// kept. A clock set back changes nothing (Gen3's `*days <= localTime->days`).
    fn update_per_day(&mut self, ids: &SpecialIds) {
        let Some(v) = ids.var_days else { return };
        let last = self.vars.var(v);
        if last < self.today {
            for &f in ids.daily_flags {
                self.vars.set_flag(f, false);
            }
            self.weather.pass_days(self.today - last);
            self.vars.set_var(v, self.today);
        }
    }

    /// The field's d-pad mapping (`SpecialIds::var_input_map`).
    pub(crate) fn input_map(&self, ids: &SpecialIds) -> crate::InputMap {
        ids.var_input_map.map_or(crate::InputMap::Normal, |v| {
            crate::InputMap::from_code(self.vars.var(v))
        })
    }

    /// Spawns the templates whose hide flag is clear (Gen3 `TrySpawnObjectEvents`).
    fn spawn_templates(&mut self) {
        self.objects.spawn_templates(&self.templates, &self.vars);
    }

    fn set_var_opt(&mut self, v: Option<VarId>, value: u16) {
        if let Some(v) = v {
            self.vars.set_var(v, value);
        }
    }

    fn log(&mut self, line: &'static str) {
        self.log.push(line);
    }

    /// A report from outside the world modules (`item_use`): its log line and its event.
    pub(crate) fn report(&mut self, r: crate::script::Report) {
        battle::report(self, r);
    }

    /// Records an [`Event`] for this frame (dropped and counted past [`EVENTS_MAX`]).
    fn event(&mut self, e: Event) {
        if let Event::ScriptStarted(id) = e {
            self.last_script = Some(id);
        }
        if self.events.len() < EVENTS_MAX {
            self.events.push(e);
        } else {
            self.events_dropped = self.events_dropped.wrapping_add(1);
        }
    }
}

impl World {
    /// The seed of the world RNG.
    pub const SEED: u32 = 0x5EED_0001;

    /// A new game: draws the player's trainer id, runs `NewGame::script` at once, loads the
    /// start map with the screen black, and fades in over the first frames. The uploads go to
    /// [`World::initial_frame`].
    pub fn new(game: &impl GameData) -> Self {
        let ng = game.new_game();
        let mut rng = Rng::new(Self::SEED);
        // Gen3 `NewGameInitData` sets the trainer id before anything else draws.
        let player_id = rng.random32();
        let mut w = World {
            frame: 0,
            runner: Runner::new(),
            s: State {
                rng,
                input: Input::default(),
                field: Field::new(ng.map),
                objects: Objects::new(),
                text: TextBox::new(),
                vars: Vars::new(),
                fader: Fader::new(),
                transition: Transition::new(),
                templates: templates::Templates::default(),
                selected: None,
                last_mover: 0,
                keep_templates: false,
                delay: 0,
                door: 0,
                log: Log::new(),
                party: Party::default(),
                respawn: game.special_ids().respawn_default,
                player_id,
                prev_behavior: Behavior::Normal,
                encounter_immunity: 0,
                mode: Mode::Field,
                answer: None,
                player_name: player_name(ng.player_name),
                bag: Bag::new(game.special_ids().pocket_slots),
                money: 0,
                str_vars: StrVars::new(),
                warp_slots: WarpSlots::default(),
                options: Options::default(),
                pokedex: Pokedex::default(),
                pc: PcStorage::new(
                    game.special_ids().pc_item_slots,
                    game.special_ids().pc_boxes,
                ),
                coins: 0,
                registered_item: None,
                berry_trees: crate::berry::BerryTrees::new(game.special_ids().berry_trees),
                fx: ScreenFx::default(),
                weather: crate::weather::Weather::default(),
                palette: None,
                sky: crate::sky::Display::default(),
                field_effects: host::FieldEffects::default(),
                rotating_tiles: None,
                field_move: None,
                stash: host::Stash::default(),
                pair: sight::Pair::default(),
                events: Vec::with_capacity(EVENTS_MAX),
                events_dropped: 0,
                last_script: None,
                save: save::SaveStatus::new_game(),
                today: 0,
                audio: crate::audio::Audio::default(),
            },
            rtc_days: None,
            boot: None,
        };
        let mut f = Frame::default();
        if let Some(id) = ng.script {
            run_immediate(game, &mut w.s, &mut f, id);
        }
        load_map(game, &mut w.s, &mut f, ng.map, ng.x, ng.y);
        // Gen3 `CB2_NewGame` -> `Overworld_PlaySpecialMapMusic`.
        w.s.play_special_map_music(game);
        w.s.objects.face(PLAYER, ng.facing);
        w.s.fader.start(Fade::FromBlack);
        let reserved = crate::sky::reserved_banks(game, &w.s.weather, None);
        w.s.objects.reserve_banks(reserved);
        draw_field(game, &mut w.s, &mut f);
        w.apply_sky(game, true, &mut f);
        w.boot = Some(f);
        w
    }

    /// The clock's day: the hardware clock's ([`World::set_rtc_days`]) when the shell has one,
    /// else the deterministic clock's, `frame / FRAMES_PER_DAY` (the Silice post-game's daily
    /// flags, `SpecialIds::daily_flags`).
    pub fn days(&self) -> u16 {
        self.rtc_days
            .unwrap_or((self.frame / FRAMES_PER_DAY).min(u16::MAX as u32) as u16)
    }

    /// The hardware clock's day (Gen3 `gLocalTime.days`), read by the shell from the cartridge
    /// RTC; `None` goes back to the deterministic clock. Taken at the next step; the daily
    /// reset runs on the next map load.
    pub fn set_rtc_days(&mut self, days: Option<u16>) {
        self.rtc_days = days;
    }

    /// Frame 0: every upload of the start map, objects and registers. Shells apply it before
    /// the first step. Later calls return an empty frame.
    pub fn initial_frame(&mut self) -> Frame {
        self.boot.take().unwrap_or_default()
    }

    /// Runs one frame with `keys` held and returns its video output.
    pub fn step(&mut self, game: &impl GameData, keys: Buttons) -> Frame {
        let mut f = Frame::default();
        // A new world's boot events (`MapEntered`, its map scripts' `ScriptStarted`) stay
        // visible through the first step.
        if self.frame != 0 {
            self.s.events.clear();
        }
        self.frame = self.frame.wrapping_add(1);
        self.s.today = self.days();
        self.s.input.update_mode(keys, self.s.options.button_mode);
        let reserved =
            crate::sky::reserved_banks(game, &self.s.weather, self.s.fx.overlay.as_ref());
        self.s.objects.reserve_banks(reserved);
        let warping = self.s.transition.is_active();
        match core::mem::replace(&mut self.s.mode, Mode::Field) {
            Mode::Field => self.step_field(game, &mut f),
            Mode::Starter(run) => self.step_starter(game, run, &mut f),
            Mode::Battle(run) => self.step_battle(game, run, &mut f),
            Mode::Screens(stack) => self.step_screens(game, stack, &mut f),
        }
        // The weather's task and the palette override run on the field's frames (Gen3's
        // weather task lives with the overworld), then recolour and decorate the frame.
        let field = self.field_shown();
        if field {
            self.s.weather.tick(&mut self.s.rng);
        }
        if let Some(p) = &mut self.s.palette
            && p.tick()
        {
            self.s.palette = None;
        }
        self.apply_sky(game, field, &mut f);
        // Screen effects post-process the whole frame, last (Task R).
        crate::fx::apply(&mut self.s.fx, &mut f);
        // Gen3 `TryFadeOutOldMapMusic`, as each warp starts.
        if !warping && self.s.transition.is_active() {
            let dest = self.s.transition.dest();
            self.s.warp_fade_out_music(game, dest, &mut f);
        }
        // Milestone 12: the frame's sound tasks, `MapMusicMain` and `SoundMain`, last.
        self.s.audio.tick(game.audio(), &mut f.sound);
        // Task S: the Hall of Fame's save, on the first capturable frame after it.
        save::after_step(self, game);
        f
    }

    /// Whether the field shows this frame: the field itself, or a screen drawn over it.
    fn field_shown(&self) -> bool {
        match &self.s.mode {
            Mode::Field => true,
            Mode::Screens(stack) => !stack.host.full,
            Mode::Starter(_) | Mode::Battle(_) => false,
        }
    }

    /// [`crate::sky::apply`] of the current state.
    fn apply_sky(&mut self, game: &impl GameData, field: bool, f: &mut Frame) {
        let s = &mut self.s;
        let sky = crate::sky::Sky {
            weather: &s.weather,
            palette: s.palette.as_ref(),
            overlay: s.fx.overlay.as_ref(),
            field,
            camera: s.field.camera(),
        };
        crate::sky::apply(game, &mut s.sky, &sky, f);
    }

    /// One overworld frame (see the module docs for the order).
    fn step_field(&mut self, game: &impl GameData, f: &mut Frame) {
        let mut player_updated = false;
        let cell = (self.s.field.map(), self.s.objects.player().pos());
        if self.s.transition.is_active() {
            self.run_transition(game, f);
        } else if self.runner.is_running() {
            self.step_script(game, f);
        } else if !self.s.field_move_busy() {
            player_updated = self.control(game, f);
        }
        // Gen3 runs the field-move task after the script context and the field input.
        self.s.field_move_tick(game);
        let s = &mut self.s;
        let ground = Ground {
            game,
            field: &s.field,
        };
        s.objects.update(&ground, &mut s.rng, player_updated);
        // A script's movement walked the player across the map's edge (Wave C): Gen3
        // `CameraMove` follows the player's sprite and loads the connected map as the step
        // starts (`LoadMapFromCameraTransition`), cutscenes included (Mr. Briney's sails).
        if !player_updated && !s.transition.is_active() {
            let (x, y) = s.objects.player().pos();
            if !crate::grid::in_bounds(s.field.data(game), x, y)
                && let Some((to, shift)) = s.field.crossing(game, x, y)
            {
                cross_connection(game, s, f, to, shift);
            }
        }
        // Gen3's per-step callback task (the map's ice puzzle or cracked floors).
        puzzle::step(game, s, f);
        // The player entered a cell this frame (a step starts, by input or by a script's
        // movement, or a crossing or warp moved it): the view window follows (Gen3 spawns and
        // despawns on each camera tile move, `CameraMove` at step start, cutscenes included).
        if (s.field.map(), s.objects.player().pos()) != cell {
            s.objects.view_moved(&s.templates, &s.vars);
        }
        if !s.transition.is_active() {
            s.fader.update();
        }
        if s.door > 0 {
            s.door -= 1;
        }
        s.field_effects.tick();
        draw_field(game, s, f);
    }

    fn step_script(&mut self, game: &impl GameData, f: &mut Frame) {
        let mut host = Host {
            game,
            s: &mut self.s,
            f,
            immediate: false,
        };
        if !self.runner.step(&mut host) {
            self.s.selected = None;
            // Gen3 never freezes the player object (`FreezeObjectEvents` skips
            // `gPlayerAvatar.objectEventId`); `lock`/`lockall` lock the field controls, which
            // `ScriptContext_RunScript` unlocks when the script ends (`UnlockPlayerFieldControls`),
            // released or not. A script that ends without `release` (Space Center 2F's multi
            // battle without `BUGFIX`) leaves the other objects frozen, the player free.
            self.s.objects.unfreeze(crate::data::PLAYER);
            // The backup and the order belong to the script that made them (see `Stash`).
            self.s.stash = host::Stash::default();
            self.s.pair = sight::Pair::default();
        }
        // Silice `save_game`: the dialog's save without the dialog, then the answer.
        if matches!(
            self.runner.waiting_on(),
            Some(crate::script::Request::SaveGame)
        ) && self.s.answer.is_none()
        {
            let ok = save::dialog_save(self, game);
            self.s.answer = Some(ok as u16);
            self.s.event(Event::Saved { ok });
        }
        // Gen3 `CB2_WhiteOut` (`special SetCB2WhiteOut`): its 120 frames are over.
        if matches!(
            self.runner.waiting_on(),
            Some(crate::script::Request::FieldWhiteOut)
        ) && self.s.delay == 0
        {
            self.whiteout(game, f);
        }
    }

    /// Starts script `id`. `talked` = (local id, player facing, the object's map: the
    /// previous map's after a crossing) for object interactions.
    fn start_script(
        &mut self,
        game: &impl GameData,
        id: ScriptId,
        talked: Option<(u8, Dir, MapId)>,
    ) {
        maps::set_talk_target(&mut self.s, &game.special_ids(), talked);
        self.s.event(Event::ScriptStarted(id));
        let ctx = self.runner.ctx();
        self.runner.start(game.script(id, ctx));
    }

    /// Player control and field triggers (Gen3 `ProcessPlayerFieldInput`, then `PlayerStep`).
    /// Returns whether `update_player` ran.
    fn control(&mut self, game: &impl GameData, f: &mut Frame) -> bool {
        let ids = game.special_ids();
        // The idle counter (`SpecialIds::var_idle_frames`): field frames with no button held.
        if let Some(v) = ids.var_idle_frames {
            let n = if self.s.input.held == Buttons::NONE {
                self.s.vars.var(v).saturating_add(1)
            } else {
                0
            };
            self.s.vars.set_var(v, n);
        }
        // The d-pad as the field reads it (`SpecialIds::var_input_map`).
        let held = self.s.input_map(&ids).apply(self.s.input.held);
        // Gen3 `ProcessPlayerFieldInput` checks trainer sight first, on every field-input frame
        // (`CheckForTrainersWantingBattle`, field_control_avatar.c:147; Milestone 8 Task I).
        if let Some(sighting) = sight::check(game, &self.s) {
            self.start_sighting(game, sighting);
            return false;
        }
        let map = self.s.field.data(game);
        if let Some(id) = map_script_on_frame(map, &self.s.vars) {
            self.start_script(game, id, None);
            return false;
        }
        let s = &self.s;
        let p = s.objects.player();
        // Gen3 `FieldGetPlayerInput`: at a step's end on a forced-movement cell, no A and no
        // held direction (arrow warps, doors) this frame.
        if p.is_settled() && !p.frozen && !s.objects.at_forced_tile_center(&s.ground(game)) {
            // Gen3 `GetPlayerPosition` / `GetInFrontOfPlayerPosition`: `PlayerGetElevation`.
            let ((x, y), facing, elevation) = (p.pos(), p.facing, p.position_elevation());
            let pushing = (crate::flow::Input {
                held,
                pressed: Buttons::NONE,
            })
            .dpad()
                == Some(facing);
            // Gen3 `TrySetupDiveEmergeScript` (B underwater), before the step checks.
            if s.input.is_pressed(Buttons::B)
                && let Some(id) = s.dive_emerge_prompt(game)
            {
                self.start_script(game, id, None);
                return false;
            }
            let here = s.field.block(game, x, y);
            if pushing
                && let Some(w) = warp_on_push(
                    map,
                    x,
                    y,
                    elevation,
                    block_behavior(game, map, here),
                    facing,
                )
            {
                self.start_warp(game, map, w, false);
                return false;
            }
            let (dx, dy) = facing.delta();
            let (fx, fy) = (x + dx, y + dy);
            let front = block_behavior(game, map, s.field.block(game, fx, fy));
            let fe = facing_elevation(here, elevation);
            if s.input.is_pressed(Buttons::A) {
                // Gen3 `GetInteractionScript`: the object ahead (or across a counter), then,
                // if it has no script, the bg event ahead.
                let (dx, dy) = facing.delta();
                let npc = |x, y| s.objects.at(x, y, fe);
                let found = match npc(fx, fy) {
                    None if front == Behavior::Counter => npc(fx + dx, fy + dy),
                    f => f,
                };
                let object_script = found.and_then(|(id, m)| {
                    let here = m == s.field.map();
                    let script = if here {
                        s.template(id).and_then(|t| t.script)
                    } else {
                        game.map(m)
                            .objects
                            .iter()
                            .find(|t| t.local_id == id)?
                            .script
                    };
                    script.map(|sc| (sc, Some((id, facing, m))))
                });
                let mut hidden = None;
                let script = object_script.or_else(|| {
                    match interact_target(map, fx, fy, fe, facing, front, |_, _, _| None) {
                        Some(Target::Bg(i)) => match map.bgs[i].kind {
                            BgKind::Script { script, .. } => Some((script, None)),
                            // Gen3 `GetInteractedBackgroundEventScript`: `VAR_0x8004` the
                            // flag, `VAR_0x8005` the item, nothing once the flag is set.
                            BgKind::HiddenItem { item, flag } if !s.vars.flag(flag) => {
                                let script = game.hidden_item_script()?;
                                hidden = Some((flag, item));
                                Some((script, None))
                            }
                            BgKind::HiddenItem { .. } => None,
                        },
                        _ => None,
                    }
                });
                // Gen3 `GetInteractedWaterScript` after the bg events (Wave C): Surf, then
                // Waterfall.
                let script = script
                    .or_else(|| s.surf_prompt(game).map(|id| (id, None)))
                    .or_else(|| s.waterfall_prompt(game).map(|id| (id, None)));
                if let Some((id, talked)) = script {
                    if let Some((flag, item)) = hidden {
                        self.s.vars.set_var(VarId(0x8004), flag.0);
                        self.s.vars.set_var(VarId(0x8005), item);
                    }
                    self.start_script(game, id, talked);
                    return false;
                }
            }
            if pushing && let Some(w) = door_warp(map, fx, fy, fe, front, facing) {
                self.start_warp(game, map, w, true);
                return false;
            }
            // Gen3 `TrySetupDiveDownScript` (A on diveable water), after the door warp.
            if self.s.input.is_pressed(Buttons::A)
                && let Some(id) = self.s.dive_down_prompt(game)
            {
                self.start_script(game, id, None);
                return false;
            }
            // Gen3 `ProcessPlayerFieldInput`: START (`ShowStartMenu`), then SELECT
            // (`UseRegisteredKeyItemOnField`), after the A interaction and the door warp.
            if self.s.input.is_pressed(Buttons::START)
                && screens::start_pressed(&mut self.s, game, f)
            {
                return false;
            }
            if self.s.input.is_pressed(Buttons::SELECT)
                && let Some(id) = crate::item_use::select_pressed(&mut self.s, game)
            {
                self.start_script(game, id, None);
                return false;
            }
        }
        let s = &mut self.s;
        let can_run = map.allow_running && ids.flag_running.is_some_and(|fl| s.vars.flag(fl));
        let input = PlayerInput::from_buttons(held, can_run);
        // Strength in use (Wave C): `TryPushBoulder`'s flag and graphics.
        let pushable = game
            .pushable_boulder()
            .filter(|&(_, fl)| s.vars.flag(fl))
            .map(|(g, _)| (g, game.pushable_objects()));
        s.objects.set_pushable(pushable);
        // The rotating gates (Gen3 `rotating_gate.c`), their orientations in the temp vars.
        let gates = game.rotating_gates(s.field.map());
        let gate_state =
            (!gates.is_empty()).then(|| (gates, crate::gates::read(|v| s.vars.var(v), gates)));
        s.objects.set_gates(gate_state);
        let ground = Ground {
            game,
            field: &s.field,
        };
        let event = s.objects.update_player(&ground, input);
        if let Some(o) = s.objects.gate_orientations()
            && gate_state.is_some_and(|(_, o0)| o0 != o)
        {
            crate::gates::write(&mut s.vars, gates, o);
        }
        match event {
            PlayerEvent::StepStarted { .. } => {
                let (x, y) = s.objects.player().pos();
                if !crate::grid::in_bounds(map, x, y)
                    && let Some((to, shift)) = s.field.crossing(game, x, y)
                {
                    cross_connection(game, s, f, to, shift);
                }
            }
            // Step triggers fire in the frame the step finishes; Gen3 checks them on the next
            // frame's field input (`ProcessPlayerFieldInput`, after `on_frame`), one frame later.
            PlayerEvent::StepFinished { at: (x, y) } => {
                let elevation = s.objects.player().position_elevation();
                let b = block_behavior(game, map, s.field.block(game, x, y));
                if let Some(sighting) = sight::check(game, s) {
                    // Trainer sight first, as on every field-input frame (above).
                    self.start_sighting(game, sighting);
                } else if b.is_forced_tile() {
                    // Gen3 `FieldGetPlayerInput`: no `tookStep` and no encounter check on a
                    // forced-movement cell (ice, arrows, slides, cracked floors).
                } else if let Some(id) = {
                    // Gen3 `GetCoordEventScriptAtPosition`: a weather coord event (no script)
                    // sets the weather (`DoCoordEventWeather`).
                    coord_weather(game, s, x, y, elevation);
                    coord_trigger(map, x, y, elevation, &s.vars)
                } {
                    self.start_script(game, id, None);
                } else {
                    let fall = game
                        .fall_script()
                        .filter(|_| b == Behavior::CrackedFloorHole);
                    if let Some(w) = warp_on_enter(map, x, y, elevation, b) {
                        if b == Behavior::PadWarp {
                            self.start_pad_warp(game, map, w);
                        } else {
                            self.start_warp(game, map, w, false);
                        }
                    } else if let Some(id) = fall {
                        // Gen3 `TryStartMiscWalkingScripts`: `EventScript_FallDownHole`.
                        self.start_script(game, id, None);
                    } else {
                        // Gen3 `TryStartStepBasedScript` after the coord and warp checks: the
                        // friendship step counter, then the repel counter (which may start
                        // its wear-off script), then `CheckStandardWildEncounter`.
                        friendship::on_step(s, game);
                        let step = crate::data::StepCount {
                            map: s.field.map(),
                            cell: (x, y),
                            vars: &mut s.vars,
                            party: &mut s.party,
                        };
                        if let Some(id) = game.step_count_script(step) {
                            self.start_script(game, id, None);
                        } else if let Some(id) = crate::item_use::on_step(s, game) {
                            self.start_script(game, id, None);
                        } else {
                            let id = s.field.map();
                            s.wild_step(game, id, b, f);
                        }
                    }
                }
            }
            PlayerEvent::PushedBoulder { id, to, .. } => {
                if let Some(script) = game.pushed_script(s.field.map()) {
                    s.vars.set_var(VarId(0x8004), id.into());
                    s.vars.set_var(VarId(0x8005), to.0 as u16);
                    s.vars.set_var(VarId(0x8006), to.1 as u16);
                    self.start_script(game, script, None);
                }
            }
            _ => {}
        }
        true
    }

    pub const fn frame(&self) -> u32 {
        self.frame
    }

    pub fn map(&self) -> MapId {
        self.s.field.map()
    }

    /// The current map's layout override (Gen3 `setmaplayoutindex`; `None`: its own).
    pub fn map_layout(&self) -> Option<u16> {
        self.s.field.layout()
    }

    /// The field weather (`crate::weather`).
    pub fn weather(&self) -> &crate::weather::Weather {
        &self.s.weather
    }

    /// The palette override (`Request::SetPalette`), if any.
    pub fn palette(&self) -> Option<&crate::sky::PaletteFx> {
        self.s.palette.as_ref()
    }

    pub fn objects(&self) -> &Objects {
        &self.s.objects
    }

    /// The field's d-pad mapping (`SpecialIds::var_input_map`): drivers press
    /// `input_map(game).key_for(dir)` to walk or turn towards `dir`.
    pub fn input_map(&self, game: &impl GameData) -> crate::InputMap {
        self.s.input_map(&game.special_ids())
    }

    pub fn text(&self) -> &TextBox {
        &self.s.text
    }

    pub fn flag(&self, f: FlagId) -> bool {
        self.s.vars.flag(f)
    }

    pub fn var(&self, v: VarId) -> u16 {
        self.s.vars.var(v)
    }

    pub fn log(&self) -> &Log {
        &self.s.log
    }

    pub fn bag(&self) -> &Bag {
        &self.s.bag
    }

    pub fn money(&self) -> u32 {
        self.s.money
    }

    /// String var `i` (0..3), 0xFF-terminated and padded.
    pub fn str_var(&self, i: usize) -> &[u8; STR_VAR_LEN] {
        self.s.str_vars.get(i)
    }

    pub fn warp_slots(&self) -> &WarpSlots {
        &self.s.warp_slots
    }

    /// This step's [`Event`]s, in order (at most [`EVENTS_MAX`]). After `World::new` and after
    /// the first step, they include the boot's (the start map entered, its map scripts).
    pub fn events(&self) -> &[Event] {
        &self.s.events
    }

    /// Events dropped past [`EVENTS_MAX`] since the world was created or restored.
    pub fn events_dropped(&self) -> u32 {
        self.s.events_dropped
    }

    /// The last script started, if any since the world was created or restored.
    pub fn last_script(&self) -> Option<ScriptId> {
        self.s.last_script
    }

    pub fn script_running(&self) -> bool {
        self.runner.is_running()
    }

    /// The request the running script waits on, if any (the sweep's diagnostics).
    pub fn script_waiting(&self) -> Option<&crate::script::Request> {
        self.runner.waiting_on()
    }

    pub fn transition_active(&self) -> bool {
        self.s.transition.is_active()
    }

    /// Whether a field-move effect runs (Surf's jump, Waterfall's climb, Dive's warp): the
    /// controls stay locked until it ends.
    pub fn field_move_active(&self) -> bool {
        self.s.field_move_busy()
    }

    /// The player's party.
    pub fn party(&self) -> &Party {
        &self.s.party
    }

    /// The heal location the player whites out to.
    pub fn respawn(&self) -> u8 {
        self.s.respawn
    }

    /// The player's trainer id.
    pub fn player_id(&self) -> u32 {
        self.s.player_id
    }

    /// The player's name: Gen3-encoded, 0xFF-terminated and 0xFF-padded.
    pub fn player_name(&self) -> &PlayerName {
        &self.s.player_name
    }

    /// Sets the player's name from Gen3-encoded bytes: stops at the first 0xFF, keeps at most 7
    /// characters and pads with 0xFF.
    pub fn set_player_name(&mut self, name: &[u8]) {
        self.s.player_name = player_name(name);
    }

    /// The options (Gen3 `OPTION` menu).
    pub fn options(&self) -> &Options {
        &self.s.options
    }

    /// The Pokédex seen and caught flags.
    pub fn pokedex(&self) -> &Pokedex {
        &self.s.pokedex
    }

    /// The PC's items and boxes.
    pub fn pc(&self) -> &PcStorage {
        &self.s.pc
    }

    /// Gen3 `coins`.
    pub fn coins(&self) -> u16 {
        self.s.coins
    }

    /// The item registered on SELECT.
    pub fn registered_item(&self) -> Option<ItemId> {
        self.s.registered_item
    }

    /// How many screens are open (0 when the field, a battle or the starter screen owns the
    /// frame).
    pub fn screens_open(&self) -> usize {
        match &self.s.mode {
            Mode::Screens(stack) => stack.len(),
            _ => 0,
        }
    }

    /// The open screens, bottom first (empty when no screen is open).
    pub fn screens(&self) -> Option<&crate::ui::ScreenStack> {
        match &self.s.mode {
            Mode::Screens(stack) => Some(stack),
            _ => None,
        }
    }

    /// A save of the current world, as the in-game save writes it (Task S, `world/save.rs`):
    /// the world as CONTINUE brings it back.
    pub fn save_snapshot(&self, game: &impl GameData) -> Result<Checkpoint, SaveError> {
        save::save_now(self, game)
    }

    /// The shell's write of the last save failed: the save dialog answers "not saved" (Gen3
    /// `SaveErrorTimer`).
    pub fn save_failed(&mut self) {
        save::save_failed(&mut self.s);
    }

    /// Whether the overworld owns the frame (no starter screen, no battle, no screen).
    pub fn field_active(&self) -> bool {
        matches!(self.s.mode, Mode::Field)
    }

    /// The field camera's top-left map pixel while the overworld owns the frame
    /// ([`World::field_active`]), else `None`. Read-only (not part of [`World::hash`]).
    pub fn camera(&self) -> Option<(i32, i32)> {
        self.field_active().then(|| self.s.field.camera())
    }

    /// Whether a battle owns the frame (from its transition until the field has faded back in).
    pub fn in_battle(&self) -> bool {
        matches!(self.s.mode, Mode::Battle(_))
    }

    /// The running battle's stage, core and scene.
    pub fn battle(&self) -> Option<(Stage, &crate::battle::Battle, &BattleScene)> {
        match &self.s.mode {
            Mode::Battle(run) => Some((run.stage, &run.battle, &run.scene)),
            _ => None,
        }
    }

    /// The starter screen, while it is shown (not during the first battle's transition).
    pub fn starter(&self) -> Option<&StarterChoice> {
        match &self.s.mode {
            Mode::Starter(run) if run.leaving.is_none() => Some(&run.choice),
            _ => None,
        }
    }

    /// Hash of every piece of state that affects future frames (the parity contract). Render
    /// caches (OBJ VRAM slots, text canvases), the log and the boot frame are left out. Cheap
    /// enough to run every frame on the GBA. Field order is part of the contract; append new
    /// fields at the end.
    pub fn hash(&self) -> u64 {
        #[cfg(test)]
        tests::HASH_CALLS.with(|n| n.set(n.get() + 1));
        let s = &self.s;
        let mut h = Fnv64::new();
        h.write_u32(self.frame);
        h.write_u32(s.rng.state());
        s.input.hash(&mut h);
        s.field.hash(&mut h);
        s.objects.hash(&mut h);
        s.text.hash(&mut h);
        s.vars.hash(&mut h);
        s.fader.hash(&mut h);
        s.transition.hash(&mut h);
        self.runner.hash(&mut h);
        h.write(&[s.selected.map_or(0xFF, |x| x.0), s.last_mover, s.door]);
        if s.keep_templates {
            h.write(&[0x50]);
        }
        h.write_u16(s.delay);
        s.templates.hash(&mut h);
        // Milestone 4.
        s.party.hash(&mut h);
        h.write(&[s.respawn, s.encounter_immunity]);
        h.write_u32(s.player_id);
        h.write(&battle::behavior_code(s.prev_behavior));
        match s.answer {
            None => h.write(&[0]),
            Some(v) => {
                h.write(&[1]);
                h.write_u16(v);
            }
        }
        s.mode.hash(&mut h);
        // Milestone 5.
        h.write(&s.player_name);
        // Milestone 8: the bag and string vars as digests maintained on write (O(1)).
        s.bag.hash(&mut h);
        h.write_u32(s.money);
        s.str_vars.hash(&mut h);
        WarpDest::hash(s.warp_slots.dynamic, &mut h);
        WarpDest::hash(s.warp_slots.escape, &mut h);
        match s.field.stale() {
            None => h.write(&[0]),
            Some(r) => {
                h.write(&[1]);
                for v in [r.x0, r.y0, r.x1, r.y1] {
                    h.write_u16(v as u16);
                }
            }
        }
        // Milestone 9: each piece only when it differs from a new game's (so routes that never
        // touch it keep their hashes), except the Pokédex once anything was seen. The screen
        // stack is in `s.mode` above (it writes nothing while empty, and `Mode::Screens` is
        // never empty).
        s.options.hash(&mut h);
        s.pokedex.hash(&mut h);
        s.pc.hash(&mut h);
        if s.coins != 0 {
            h.write(&[0xA9]);
            h.write_u16(s.coins);
        }
        if let Some(item) = s.registered_item {
            h.write(&[0xAA]);
            h.write_u16(item.0);
        }
        s.berry_trees.hash(&mut h);
        s.fx.hash(&mut h);
        s.weather.hash(&mut h);
        if let Some(p) = &s.palette {
            p.hash(&mut h);
        }
        // Wave C: a field move's effect, while it runs.
        if let Some(run) = &s.field_move {
            run.hash(&mut h);
        }
        // Wave F: the party's backup, while a script holds one.
        s.stash.hash(&mut h);
        // Task S: the Hall of Fame's pending save.
        s.save.hash(&mut h);
        // Wave G: a two-trainer sighting, while its scripts run.
        s.pair.hash(&mut h);
        // Milestone 12: the music players and sound tasks.
        s.audio.hash(&mut h);
        h.finish()
    }
}

/// A [`PlayerName`] from Gen3-encoded bytes: up to the first 0xFF, at most 7 characters, then
/// 0xFF padding (so the name is always terminated).
fn player_name(name: &[u8]) -> PlayerName {
    let mut out = [0xFF; PLAYER_NAME_SIZE];
    for (o, &c) in out[..PLAYER_NAME_SIZE - 1]
        .iter_mut()
        .zip(name.iter().take_while(|&&c| c != 0xFF))
    {
        *o = c;
    }
    out
}

/// Camera, sprites, text box and brightness: the end of a field frame.
/// The weather coord events of the current map at (x, y) ([`GameData::weather_coords`]): the
/// first one matching sets its weather (Gen3 `DoCoordEventWeather` → `SetWeather`).
fn coord_weather(game: &impl GameData, s: &mut State, x: i16, y: i16, elevation: u8) {
    let hit = (game.weather_coords(s.field.map()).iter())
        .find(|c| c.x == x && c.y == y && crate::flow::event_matches(c.elevation, elevation));
    if let Some(w) = hit.and_then(|c| crate::weather::from_coord(c.weather)) {
        s.weather.set_weather(w);
    }
}

fn draw_field(game: &impl GameData, s: &mut State, f: &mut Frame) {
    let camera = camera_for(s.objects.player().pixel_pos());
    s.field.set_camera(game, camera, f);
    s.objects.draw(game, camera, f);
    s.text.update(game, s.input.pressed, s.input.held, f);
    f.brightness = s.fader.level();
    // Milestone 9: white fades (Task R's `Fader`).
    f.lighten = s.fader.is_white();
}

/// Runs a map script to completion now (Gen3 `RunScriptImmediately`).
fn run_immediate(game: &impl GameData, s: &mut State, f: &mut Frame, id: ScriptId) {
    let mut runner = Runner::new();
    s.event(Event::ScriptStarted(id));
    runner.start(game.script(id, runner.ctx()));
    let mut host = Host {
        game,
        s,
        f,
        immediate: true,
    };
    for _ in 0..IMMEDIATE_STEPS {
        if !runner.step(&mut host) {
            return;
        }
    }
    host.s.log("immediate map script did not finish");
}

#[cfg(test)]
mod player_name_tests;
