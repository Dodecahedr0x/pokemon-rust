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

mod battle;
mod checkpoint;
mod host;
mod maps;
mod sight;
#[cfg(test)]
mod tests;

pub use battle::{ENCOUNTER_IMMUNITY_STEPS, Origin, Stage, WHITEOUT_FRAMES};

use crate::bag::Bag;
use crate::battle::BattleOutcome;
use crate::battle::scene::BattleScene;
use crate::checkpoint::{PLAYER_NAME_SIZE, PlayerName};
use crate::data::{
    Behavior, BgKind, Dir, FlagId, GameData, MapId, ObjectEvent, PLAYER, ScriptId, SpecialIds,
    VarId,
};
use crate::field::Field;
use crate::flow::{
    Fader, Input, Target, Transition, Vars, block_behavior, coord_trigger, door_warp,
    facing_elevation, interact_target, map_script_on_frame, warp_on_enter, warp_on_push,
};
use crate::hash::Fnv64;
use crate::mon::Party;
use crate::objects::{Objects, PlayerEvent, PlayerInput, Terrain};
use crate::script::{Fade, Report, Runner, Speed, Step};
use crate::starter::StarterChoice;
use crate::str_vars::{STR_VAR_LEN, StrVars};
use crate::text::TextBox;
use crate::video::Frame;
use crate::{Buttons, Rng};
use alloc::vec::Vec;
use battle::Mode;
use host::Host;
use maps::{cross_connection, load_map};

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
    rng: Rng,
    input: Input,
    field: Field,
    objects: Objects,
    text: TextBox,
    vars: Vars,
    fader: Fader,
    transition: Transition,
    /// The current map's object templates for this visit (scripts move them and change their
    /// movement type).
    templates: Vec<ObjectEvent>,
    /// The object whose interaction started the running script (Gen3 `gSelectedObjectEvent`).
    selected: Option<u8>,
    /// Object of the last `Move` (Gen3 `sMovingNpcId`), for `WaitMove(0)`.
    last_mover: u8,
    /// Frames left of a script `Delay`.
    delay: u16,
    /// Frames left of a door animation.
    door: u8,
    log: Log,
    /// The player's party (Gen3 `gPlayerParty`).
    party: Party,
    /// Heal location of the last `setrespawn` (Gen3 `gSaveBlock1Ptr->lastHealLocation`).
    respawn: u8,
    /// The player's trainer id, the OT id of their mons (Gen3 `playerTrainerId`).
    player_id: u32,
    /// Gen3 `sPrevMetatileBehavior`: the cell behavior of the last checked step.
    prev_behavior: Behavior,
    /// Gen3 `sWildEncounterImmunitySteps`: steps since the last
    /// `RestartWildEncounterImmunitySteps` (map loads and battles), up to
    /// [`ENCOUNTER_IMMUNITY_STEPS`].
    encounter_immunity: u8,
    /// What owns the screen: the field, the starter screen or a battle.
    mode: Mode,
    /// The answer to the script's pending `ChooseStarter` / `BattleWild` / `BattleTrainer`.
    answer: Option<u16>,
    /// Gen3-encoded player name, 0xFF-terminated and 0xFF-padded (Gen3 `playerName`). Copied
    /// from `NewGame` at a new game; hashed last in `World::hash`. Texts, the starter screen and
    /// battles print this copy.
    player_name: PlayerName,
    // ---- Milestone 8 ----
    /// The player's bag (Gen3 `gSaveBlock1Ptr->bagPocket_*`).
    bag: Bag,
    /// Gen3 `gSaveBlock1Ptr->money`.
    money: u32,
    /// Gen3 `gStringVar1..3`.
    str_vars: StrVars,
    warp_slots: WarpSlots,
    // Tool-facing, neither hashed nor snapshotted:
    /// This step's events (cleared at the start of `step`), at most [`EVENTS_MAX`].
    events: Vec<Event>,
    /// Events dropped past [`EVENTS_MAX`] since the world was created or restored.
    events_dropped: u32,
    /// The last script started.
    last_script: Option<ScriptId>,
}

/// All mutable game state. Deterministic: the same state and input give the same next state.
pub struct World {
    frame: u32,
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
        block_behavior(self.game, self.game.map(self.field.map()), b)
    }
}

/// One forced step per direction (warp walk-in/out).
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

    fn template_mut(&mut self, local: u8) -> Option<&mut ObjectEvent> {
        self.templates.iter_mut().find(|t| t.local_id == local)
    }

    /// Gen3 `ClearTempFieldEventData`.
    fn clear_temp(&mut self, ids: &SpecialIds) {
        self.vars.clear_temp();
        for &f in ids.flags_cleared_on_map_change {
            self.vars.set_flag(f, false);
        }
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
                templates: Vec::new(),
                selected: None,
                last_mover: 0,
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
                bag: Bag::new(),
                money: 0,
                str_vars: StrVars::new(),
                warp_slots: WarpSlots::default(),
                events: Vec::with_capacity(EVENTS_MAX),
                events_dropped: 0,
                last_script: None,
            },
            boot: None,
        };
        let mut f = Frame::default();
        if let Some(id) = ng.script {
            run_immediate(game, &mut w.s, &mut f, id);
        }
        load_map(game, &mut w.s, &mut f, ng.map, ng.x, ng.y);
        w.s.objects.face(PLAYER, ng.facing);
        w.s.fader.start(Fade::FromBlack);
        draw_field(game, &mut w.s, &mut f);
        w.boot = Some(f);
        w
    }

    /// Frame 0: every upload of the start map, objects and registers. Shells apply it before
    /// the first step. Later calls return an empty frame.
    pub fn initial_frame(&mut self) -> Frame {
        self.boot.take().unwrap_or_default()
    }

    /// Runs one frame with `keys` held and returns its video output.
    pub fn step(&mut self, game: &impl GameData, keys: Buttons) -> Frame {
        let mut f = Frame::default();
        self.s.events.clear();
        self.frame = self.frame.wrapping_add(1);
        self.s.input.update(keys);
        match core::mem::replace(&mut self.s.mode, Mode::Field) {
            Mode::Field => self.step_field(game, &mut f),
            Mode::Starter(run) => self.step_starter(game, run, &mut f),
            Mode::Battle(run) => self.step_battle(game, run, &mut f),
        }
        f
    }

    /// One overworld frame (see the module docs for the order).
    fn step_field(&mut self, game: &impl GameData, f: &mut Frame) {
        let mut player_updated = false;
        if self.s.transition.is_active() {
            self.run_transition(game, f);
        } else if self.runner.is_running() {
            self.step_script(game, f);
        } else {
            player_updated = self.control(game, f);
        }
        let s = &mut self.s;
        let ground = Ground {
            game,
            field: &s.field,
        };
        s.objects.update(&ground, &mut s.rng, player_updated);
        if !s.transition.is_active() {
            s.fader.update();
        }
        if s.door > 0 {
            s.door -= 1;
        }
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
        }
    }

    /// Starts script `id`. `talked` = (local id, player facing, whether the object belongs to
    /// the current map) for object interactions.
    fn start_script(
        &mut self,
        game: &impl GameData,
        id: ScriptId,
        talked: Option<(u8, Dir, bool)>,
    ) {
        maps::set_talk_target(&mut self.s, &game.special_ids(), talked);
        self.s.event(Event::ScriptStarted(id));
        let ctx = self.runner.ctx();
        self.runner.start(game.script(id, ctx));
    }

    /// Player control and field triggers (Gen3 `ProcessPlayerFieldInput`, then `PlayerStep`).
    /// Returns whether `update_player` ran.
    fn control(&mut self, game: &impl GameData, f: &mut Frame) -> bool {
        let map = game.map(self.s.field.map());
        if let Some(id) = map_script_on_frame(map, &self.s.vars) {
            self.start_script(game, id, None);
            return false;
        }
        let s = &self.s;
        let p = s.objects.player();
        if p.is_settled() && !p.frozen {
            let ((x, y), facing, elevation) = (p.pos(), p.facing, p.elevation);
            let pushing = s.input.dpad() == Some(facing);
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
                    script.map(|sc| (sc, Some((id, facing, here))))
                });
                let script = object_script.or_else(|| {
                    match interact_target(map, fx, fy, fe, facing, front, |_, _, _| None) {
                        Some(Target::Bg(i)) => match map.bgs[i].kind {
                            BgKind::Script { script, .. } => Some((script, None)),
                            BgKind::HiddenItem { .. } => None,
                        },
                        _ => None,
                    }
                });
                if let Some((id, talked)) = script {
                    self.start_script(game, id, talked);
                    return false;
                }
            }
            if pushing && let Some(w) = door_warp(map, fx, fy, fe, front, facing) {
                self.start_warp(game, map, w, true);
                return false;
            }
        }
        let ids = game.special_ids();
        let s = &mut self.s;
        let can_run = map.allow_running && ids.flag_running.is_some_and(|fl| s.vars.flag(fl));
        let input = PlayerInput::from_buttons(s.input.held, can_run);
        let ground = Ground {
            game,
            field: &s.field,
        };
        match s.objects.update_player(&ground, input) {
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
                let elevation = s.objects.player().elevation;
                if let Some(id) = coord_trigger(map, x, y, elevation, &s.vars) {
                    self.start_script(game, id, None);
                } else {
                    let b = block_behavior(game, map, s.field.block(game, x, y));
                    if let Some(w) = warp_on_enter(map, x, y, elevation, b) {
                        self.start_warp(game, map, w, false);
                    } else if let Some(sighting) = sight::check(game, s) {
                        // Gen3 `CheckForTrainersWantingBattle` (Milestone 8 Task I).
                        self.start_sighting(game, sighting);
                    } else {
                        // Gen3 `CheckStandardWildEncounter`, after the step-based scripts.
                        let id = s.field.map();
                        s.wild_step(game, id, b, f);
                    }
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

    pub fn objects(&self) -> &Objects {
        &self.s.objects
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

    /// This step's [`Event`]s, in order (at most [`EVENTS_MAX`]).
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

    pub fn transition_active(&self) -> bool {
        self.s.transition.is_active()
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

    /// Whether the overworld owns the frame (no starter screen, no battle).
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
        h.write(&[s.selected.map_or(0xFF, |x| x), s.last_mover, s.door]);
        h.write_u16(s.delay);
        h.write_u32(s.templates.len() as u32);
        for t in &s.templates {
            h.write(&[t.local_id]);
            h.write_u16(t.x as u16);
            h.write_u16(t.y as u16);
            h.write(&crate::objects::movement_code(t.movement));
        }
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
fn draw_field(game: &impl GameData, s: &mut State, f: &mut Frame) {
    let camera = camera_for(s.objects.player().pixel_pos());
    s.field.set_camera(game, camera, f);
    s.objects.draw(game, camera, f);
    s.text.update(game, s.input.pressed, s.input.held, f);
    f.brightness = s.fader.level();
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
