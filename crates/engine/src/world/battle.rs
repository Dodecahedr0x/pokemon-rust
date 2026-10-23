//! Battles and the starter screen as World modes (Milestone 4 Task J).
//!
//! A battle starts from a script ([`Request::BattleWild`](crate::script::Request) /
//! `BattleTrainer`, or the first battle after the starter screen) or from a field step in tall
//! grass. Its [`BattleRun`] goes through these stages:
//! 1. **Transition** (`BattleSetup_StartWildBattle` / `_StartTrainerBattle` →
//!    `BattleTransition_StartOnField`): objects frozen for a grass encounter only (Gen3
//!    `DoStandardWildBattle`; scripted battles leave that to the script), the field (or, for
//!    the first battle, the starter screen: `CB2_GiveStarter` → `CB2_StartFirstBattle`) keeps
//!    being drawn under [`BattleTransition`], which ends black. The battle's start
//!    ([`Battle::start`]: the intro events, the first turn and the trainer AI's choice) runs on
//!    the transition's first frame, not on the frame that started the battle (Gen3 sets the
//!    battle up in `CB2_InitBattle`, after the transition): only video is drawn in between, so
//!    the start frame stays under the line budget. The RNG order stays the same unless an
//!    unfrozen object draws on the start frame (see the deviations).
//! 2. **Loading**: [`BattleScene::load`] chunks, one per black frame (`CB2_InitBattle`).
//! 3. **Running**: [`BattleScene::step`] each frame until [`BattleScene::finished`] (the scene
//!    fades to black after `End`). The player's party is written back; on a win the prize
//!    money is added (Gen3 `getmoneyreward`, `AddMoney`) and the mons that gained a level
//!    evolve (`TryEvolvePokemon`, level methods only, without the scene: reported).
//! 4. **Leaving** (`CB2_ReturnToField*`): on one black frame the field is restored (tilesets,
//!    palettes and BG controls re-uploaded, the view redrawn, object sprite caches dropped, the
//!    field text box rebuilt), then **Returning**: the field fades in
//!    (`FieldCB_ContinueScript*`: `FadeInFromBlack`, then `Task_WaitForFadeAndEnableScriptCtx`)
//!    with the objects updating, and only then is the script answered (and a grass
//!    encounter's objects unfrozen).
//! 5. **Whiteout** instead of Leaving when the player lost (`Lost` or `Drew`, Gen3
//!    `IsPlayerDefeated`), except in the first battle (`CB2_EndFirstBattle` always returns to its
//!    script): Gen3 `CB2_WhiteOut`: 120 black frames, then `DoWhiteOut`: the running script
//!    is aborted, `SpecialIds::script_whiteout` runs at once (`EventScript_WhiteOut`), the
//!    money is halved, the party healed and the player put on the respawn heal location
//!    (`setrespawn`) while black, facing down; then Returning. Emerald prints no nurse line.
//!
//! The battle's [`BattleReport`](crate::battle::BattleReport)s become
//! [`Event::Report`](super::Event::Report)s (and log lines) each frame.
//!
//! Deviations from Gen3 (documented):
//! - The opponent party and `Battle::new`'s draws happen when the battle starts, before the
//!   transition (Gen3 creates trainer parties and the scripted wild mon in `CB2_InitBattle`,
//!   after it). The order of draws within them is Gen3's.
//! - The start frame runs `Battle::new`, then the objects' update, and `Battle::start` (the
//!   first turn's and the AI's draws) one frame later. An idle object left unfrozen on the
//!   start frame (a scripted battle whose script did not `lock` it) that draws then takes its
//!   value between the two; in Gen3 every battle setup draw comes after the transition, after
//!   any such object draws.
//! - The field text box is closed when a battle starts: `trainerbattle`'s intro message stays
//!   open into the battle in Gen3 and disappears under the transition, which overwrites BG0.
//! - The environment is `Grass` on a tall-grass cell and `Plain` elsewhere
//!   (`BattleSetup_GetEnvironmentId`, simplified).
//! - Field wild encounters also require a party with a mon that can fight (Gen3 relies on the
//!   story never letting the player into grass without one).
//! - A script battle (`dowildbattle`, `trainerbattle`) without a mon that can fight is not
//!   started and answers `Won` at once (Gen3 would start it and lose at the first turn); a
//!   map script's (`immediate`) battle answers `Ran`.
//! - A starter screen answered without a battle fades back to the field with the script
//!   already resumed (see `World::step_starter`).

use super::{Ground, State, World, draw_field, load_map};
use crate::battle::BattleReport;
use crate::battle::scene::BattleScene;
use crate::battle::transition::BattleTransition;
use crate::battle::{Battle, BattleKind, BattleOutcome, BattleSetup, Environment, Side};
use crate::data::{Behavior, Dir, EvoMethod, Floor, GameData, MapId, MapType, PLAYER, SpeciesId};
use crate::flow::{FADE_MAX, Fader, Input};
use crate::hash::Fnv64;
use crate::mon::{Ivs, Party, Pokemon};
use crate::objects::Terrain;
use crate::script::{Fade, Report};
use crate::starter::StarterChoice;
use crate::video::Frame;
use crate::wild;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Gen3 `CheckStandardWildEncounter`: steps after a reset (`RestartWildEncounterImmunitySteps`)
/// that never check for an encounter.
pub const ENCOUNTER_IMMUNITY_STEPS: u8 = 4;
/// Gen3 `CB2_WhiteOut`: `++gMain.state >= 120` black frames before the map loads.
pub const WHITEOUT_FRAMES: u8 = 120;

/// What owns the screen.
pub(crate) enum Mode {
    /// The overworld (field, objects, text box, scripts).
    Field,
    /// `special ChooseStarter`, until a starter is confirmed.
    Starter(Box<StarterRun>),
    Battle(Box<BattleRun>),
}

pub(crate) struct StarterRun {
    pub choice: StarterChoice,
    /// The script continued without a battle: fading the starter screen out before the field
    /// is restored (no Gen3 counterpart: `CB2_GiveStarter` always starts the first battle).
    pub leaving: Option<Fader>,
}

/// Who started the battle (and gets its outcome).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// A script request: answered with the outcome once the field is back.
    Script,
    /// A wild encounter on a field step.
    Field,
}

/// See the module docs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Transition,
    Loading,
    Running,
    /// The field is restored on this (black) frame, its uploads spread over the next frames.
    Leaving,
    /// Black while the field's spread upload runs (one `set_camera` a frame).
    Restoring,
    /// Black frames shown so far before the whiteout warp.
    Whiteout(u8),
    /// The field fades in.
    Returning,
}

impl Stage {
    fn code(self) -> [u8; 2] {
        match self {
            Stage::Transition => [0, 0],
            Stage::Loading => [1, 0],
            Stage::Running => [2, 0],
            Stage::Leaving => [3, 0],
            Stage::Whiteout(n) => [4, n],
            Stage::Returning => [5, 0],
            Stage::Restoring => [6, 0],
        }
    }
}

pub(crate) struct BattleRun {
    pub stage: Stage,
    pub origin: Origin,
    pub battle: Battle,
    pub scene: BattleScene,
    pub transition: BattleTransition,
    /// The starter screen the first battle's transition runs over (`CB2_StartFirstBattle`).
    pub backdrop: Option<Box<StarterChoice>>,
    pub outcome: Option<BattleOutcome>,
    /// The player whited out (no answer to the script, objects unfrozen).
    pub whiteout: bool,
    /// [`Battle::start`] ran (on the transition's first frame).
    pub started: bool,
}

/// Feeds a behavior into a hash.
pub(super) fn behavior_code(b: Behavior) -> [u8; 2] {
    match b {
        Behavior::Normal => [0, 0],
        Behavior::Blocked => [1, 0],
        Behavior::Ledge(d) => [2, d as u8],
        Behavior::Door => [3, 0],
        Behavior::Warp => [4, 0],
        Behavior::ArrowWarp(d) => [5, d as u8],
        Behavior::Counter => [6, 0],
        Behavior::TallGrass => [7, 0],
        Behavior::Impassable(s) => [8, s.0],
        Behavior::NonAnimDoor(d) => [9, d as u8],
        Behavior::Ladder => [10, 0],
        Behavior::Escalator(d) => [11, d as u8],
        // 12–19: the forced-movement branch's variants (see `Behavior::Floor`).
        Behavior::Floor(g) => [20, g as u8],
    }
}

/// Gen3 `BattleSetup_GetEnvironmentId` for the player's cell `b` on a map of type `map`:
/// tall grass, long grass and sand (both kinds) first, then the map type (caves; buildings;
/// underwater), then mountain tops, then Route 113, else plain.
///
/// Not modelled: the water branches (the player never stands on water: no Surf yet; water
/// behaviours are `Blocked`) and Gen3's `WEATHER_SANDSTORM` case (no field weather), so a
/// desert battle off sand is plain.
pub(crate) fn environment(b: Behavior, map: MapType) -> Environment {
    match b {
        Behavior::TallGrass => return Environment::Grass,
        Behavior::Floor(Floor::LongGrass) => return Environment::LongGrass,
        Behavior::Floor(Floor::Sand | Floor::DeepSand) => return Environment::Sand,
        _ => {}
    }
    match map {
        MapType::Underground if b == Behavior::Floor(Floor::IndoorEncounter) => {
            Environment::Building
        }
        MapType::Underground => Environment::Cave,
        MapType::Indoor => Environment::Building,
        MapType::Underwater => Environment::Underwater,
        MapType::OceanRoute => Environment::Plain,
        MapType::Outdoor | MapType::OutdoorSand if b == Behavior::Floor(Floor::MountainTop) => {
            Environment::Mountain
        }
        MapType::OutdoorSand => Environment::Sand,
        MapType::Outdoor => Environment::Plain,
    }
}

impl Mode {
    pub(super) fn hash(&self, h: &mut Fnv64) {
        match self {
            Mode::Field => h.write(&[0]),
            Mode::Starter(run) => {
                h.write(&[1]);
                run.choice.hash(h);
                match &run.leaving {
                    None => h.write(&[0xFF]),
                    Some(fader) => fader.hash(h),
                }
            }
            Mode::Battle(run) => {
                h.write(&[2]);
                h.write(&run.stage.code());
                h.write(&[
                    run.origin as u8,
                    run.outcome.map_or(0, |o| o as u8),
                    run.whiteout as u8,
                    run.started as u8,
                ]);
                run.transition.hash(h);
                run.battle.hash(h);
                run.scene.hash(h);
                match &run.backdrop {
                    None => h.write(&[0]),
                    Some(s) => {
                        h.write(&[1]);
                        s.hash(h);
                    }
                }
            }
        }
    }
}

impl State {
    /// Gen3 `CreateMon(mon, species, level, USE_RANDOM_IVS, FALSE, 0, OT_ID_PLAYER_ID, 0)`
    /// (`givemon`, `setwildbattle`, the first battle's Zigzagoon): personality `Random32()`
    /// first, then the IVs.
    pub(super) fn create_mon(
        &mut self,
        game: &impl GameData,
        species: SpeciesId,
        level: u8,
    ) -> Pokemon {
        let personality = self.rng.random32();
        Pokemon::new(
            game.battle(),
            species,
            level,
            Ivs::Random,
            personality,
            self.player_id,
            &mut self.rng,
        )
    }

    /// Gen3 `BattleSetup_GetEnvironmentId` on the player's cell of the current map.
    fn environment(&self, game: &impl GameData) -> Environment {
        let (x, y) = self.objects.player().pos();
        environment(
            self.ground(game).behavior(x, y),
            game.map_type(self.field.map()),
        )
    }

    /// Whether the party can battle.
    pub(super) fn can_battle(&self) -> bool {
        self.party.first_usable().is_some()
    }

    /// Starts a battle against `opponents` (non-empty; the party must be able to battle):
    /// the core and its intro events, the scene, the transition. Freezes objects and closes the
    /// field text box.
    pub(super) fn start_battle(
        &mut self,
        game: &impl GameData,
        kind: BattleKind,
        opponents: Party,
        origin: Origin,
        f: &mut Frame,
    ) {
        let setup = BattleSetup {
            kind,
            opponents,
            environment: self.environment(game),
        };
        let transition = BattleTransition::new(&kind);
        // The run (about 4 KiB) is built in its heap allocation: `Box::new` of a literal copies
        // it from the stack, 6 scanlines on the GBA's trainer-battle start, the route's worst
        // frame (Milestone 8 Wave 0).
        let mut run = Box::<BattleRun>::new_uninit();
        let p = run.as_mut_ptr();
        // Fails to compile when `BattleRun` gains or loses a field: the writes below must list
        // exactly these fields.
        const _: fn(BattleRun) = |BattleRun {
                                      stage: _,
                                      origin: _,
                                      battle: _,
                                      scene: _,
                                      transition: _,
                                      backdrop: _,
                                      outcome: _,
                                      whiteout: _,
                                      started: _,
                                  }| {};
        // SAFETY: `p` points to the allocation of an uninitialised `BattleRun`. Each field is
        // written exactly once through a raw field pointer before `assume_init`, and only the
        // `battle` field is borrowed before then, after it is written. The writes must cover
        // the field list of the destructuring guard above, which the compiler keeps in sync
        // with the struct.
        let battle = unsafe {
            let b = &raw mut (*p).battle;
            b.write(Battle::new(game, setup, &self.party, &mut self.rng));
            // The player's form changes need their flag (the Mega Ring). Their state lives in
            // the `Battle`, so the field guard above is unchanged. Emerald has no form changes:
            // this returns at once.
            (*b).enable_player_forms(game, |f| self.vars.flag(f));
            &*b
        };
        if let BattleKind::Trainer { trainer, .. } = battle.setup().kind
            && game.trainer(trainer).is_some_and(|t| t.double)
        {
            // A double battle runs as singles against the same party (decision Q3). The one
            // place this is reported, for every trainer battle (scripted or by sight).
            report(self, Report::Degraded("double battle"));
        }
        let scene = BattleScene::new(battle.setup(), self.player_name);
        // SAFETY: as above; this writes every remaining field.
        let run = unsafe {
            (&raw mut (*p).scene).write(scene);
            (&raw mut (*p).stage).write(Stage::Transition);
            (&raw mut (*p).origin).write(origin);
            (&raw mut (*p).transition).write(transition);
            (&raw mut (*p).backdrop).write(None);
            (&raw mut (*p).outcome).write(None);
            (&raw mut (*p).whiteout).write(false);
            (&raw mut (*p).started).write(false);
            run.assume_init()
        };
        self.event(super::Event::BattleStarted);
        if self.text.is_open() {
            self.text.close(f);
        }
        // Gen3: only `DoStandardWildBattle` calls `FreezeObjectEvents`; scripted battles
        // (`dowildbattle`, `trainerbattle`, the starter's) leave the frozen flags to the script.
        if origin == Origin::Field {
            self.objects.freeze_all();
        }
        self.mode = Mode::Battle(run);
    }

    /// Gen3 `CheckStandardWildEncounter` for a completed player step onto a `cur` cell of
    /// `map`: the immunity steps, then `StandardWildEncounter` (`try_land_encounter`, then at
    /// once `create_wild`, as `TryGenerateWildMon`), and `sPrevMetatileBehavior`. Returns
    /// whether a battle started.
    pub(super) fn wild_step(
        &mut self,
        game: &impl GameData,
        map: MapId,
        cur: Behavior,
        f: &mut Frame,
    ) -> bool {
        if self.encounter_immunity < ENCOUNTER_IMMUNITY_STEPS {
            self.encounter_immunity += 1;
            self.prev_behavior = cur;
            return false;
        }
        let t = game.battle();
        let wild_set = game
            .special_ids()
            .var_wild_set
            .map_or(0, |v| self.vars.var(v));
        let found = match game.wild_encounters(map, wild_set) {
            Some(table) if self.can_battle() => {
                // `gPlayerParty[0]`, fainted or not.
                let lead = self.party.get(0).copied();
                wild::try_land_encounter(
                    table,
                    self.prev_behavior,
                    cur,
                    wild::Lead::of(lead.as_ref(), t, |i| game.hold_effect(i)),
                    t,
                    &mut self.rng,
                )
                .map(|(species, level)| {
                    wild::create_wild(
                        t,
                        species,
                        level,
                        lead.as_ref(),
                        self.player_id,
                        &mut self.rng,
                    )
                })
            }
            _ => None,
        };
        self.prev_behavior = cur;
        let Some(mon) = found else {
            return false;
        };
        self.encounter_immunity = 0;
        let mut opponents = Party::default();
        let _ = opponents.push(mon);
        self.start_battle(
            game,
            BattleKind::Wild {
                first_battle: false,
            },
            opponents,
            Origin::Field,
            f,
        );
        true
    }
}

/// "move effect NNN" for each raw `EFFECT_*` number, built at compile time: the
/// [`Report::Unsupported`] text of an unsupported battle effect (logs hold `&'static str`).
const EFFECT_LINE: usize = 15;
static EFFECT_LINES: [u8; 256 * EFFECT_LINE] = {
    let prefix = b"move effect ";
    let mut out = [0u8; 256 * EFFECT_LINE];
    let mut n = 0;
    while n < 256 {
        let at = n * EFFECT_LINE;
        let mut i = 0;
        while i < prefix.len() {
            out[at + i] = prefix[i];
            i += 1;
        }
        out[at + 12] = b'0' + (n / 100) as u8;
        out[at + 13] = b'0' + (n / 10 % 10) as u8;
        out[at + 14] = b'0' + (n % 10) as u8;
        n += 1;
    }
    out
};

/// [`EFFECT_LINES`]' line for effect `n`.
fn effect_line(n: u8) -> &'static str {
    let at = n as usize * EFFECT_LINE;
    core::str::from_utf8(&EFFECT_LINES[at..at + EFFECT_LINE]).unwrap_or("move effect")
}

/// The scene's battle reports as world [`Report`]s: an event and a log line each.
fn drain_reports(s: &mut State, scene: &mut BattleScene) {
    for r in scene.take_reports() {
        let r = match r {
            BattleReport::UnsupportedEffect(n) => Report::Unsupported(effect_line(n)),
            BattleReport::Degraded(what) => Report::Degraded(what),
        };
        report(s, r);
    }
}

/// A world report: its log line and its event.
fn report(s: &mut State, r: Report) {
    s.log(match r {
        Report::Unsupported(what) | Report::Degraded(what) => what,
    });
    s.event(super::Event::Report(r));
}

/// After a win (Gen3 `getmoneyreward` and `TryEvolvePokemon`): the prize money
/// (`AddMoney`, capped at `SpecialIds::max_money`), then each mon that gained a level evolves if a level
/// evolution is reached, without the scene (`Report::Degraded("evolution scene")`).
fn after_win(game: &impl GameData, s: &mut State, battle: &Battle) {
    if let Some(money) = battle.prize() {
        let max = game.special_ids().max_money;
        if max == 0 && money > 0 {
            report(s, Report::Degraded(super::host::NO_MONEY));
        }
        s.money = s.money.saturating_add(money).min(max);
    }
    let t = game.battle();
    for i in 0..6 {
        if battle.leveled_up() & (1 << i) == 0 {
            continue;
        }
        let Some(mon) = s.party.get_mut(i) else {
            continue;
        };
        let evos = game.evolutions(mon.species);
        let target = mon.evolution_target(evos);
        if evos.iter().any(|e| e.method == EvoMethod::Friendship) {
            // EVO_FRIENDSHIP* (`friendship >= 220`) is not evaluated (M9).
            report(s, Report::Degraded("friendship evolution"));
        }
        let Some((target, ambiguous)) = target else {
            continue;
        };
        let Some(mon) = s.party.get_mut(i) else {
            continue;
        };
        mon.evolve(t, target);
        if ambiguous {
            report(s, Report::Degraded("evolution branch"));
        }
        report(s, Report::Degraded("evolution scene"));
    }
}

/// Re-uploads the field after a battle or the starter screen owned the screen (black frame):
/// tilesets, palettes and BG controls (the view is redrawn at the frame's camera update), the
/// object sprite caches dropped, and a new field text box (BG0 off and cleared).
fn restore_field(game: &impl GameData, s: &mut State, f: &mut Frame) {
    f.bulk = true;
    s.field.restore(game, f);
    s.objects.invalidate_vram();
    s.text.close(f);
}

/// [`restore_field`] with the tilesets and view uploaded over the next frames
/// ([`crate::field::Field::restore_spread`]); the caller keeps the screen black while
/// [`crate::field::Field::uploading`] (Milestone 8 Task F).
fn restore_field_spread(s: &mut State, f: &mut Frame) {
    f.bulk = true;
    s.field.restore_spread();
    s.objects.invalidate_vram();
    s.text.close(f);
}

impl World {
    /// One frame of the starter screen. On a confirmed choice, answers `ChooseStarter` and runs
    /// the script on in the same frame, so the first battle's transition starts over the
    /// starter screen (`CB2_GiveStarter` → `CB2_StartFirstBattle`).
    pub(super) fn step_starter(
        &mut self,
        game: &impl GameData,
        mut run: Box<StarterRun>,
        f: &mut Frame,
    ) {
        if let Some(fader) = &mut run.leaving {
            if fader.is_done() {
                restore_field(game, &mut self.s, f);
                self.s.fader.start(Fade::FromBlack);
                draw_field(game, &mut self.s, f);
                return;
            }
            run.choice.update(game, &Input::default(), f);
            f.brightness = fader.update();
            self.s.mode = Mode::Starter(run);
            return;
        }
        let Some(i) = run.choice.update(game, &self.s.input, f) else {
            self.s.mode = Mode::Starter(run);
            return;
        };
        self.s.answer = Some(i as u16);
        self.step_script(game, f);
        match &mut self.s.mode {
            Mode::Battle(b) => b.backdrop = Some(Box::new(run.choice)),
            _ => {
                // Not Gen3 (whose starter script always battles): the script has already run
                // on to its next wait while the starter screen still owns BG0, and the fade out
                // and field restore below close the field text box. Content that answers the
                // starter screen without a battle must not print before the field is back (a
                // `delay`/`waitstate` first); the Emerald content always battles.
                self.s.log("starter chosen without a battle");
                let mut fader = Fader::new();
                fader.start(Fade::ToBlack);
                run.leaving = Some(fader);
                self.s.mode = Mode::Starter(run);
            }
        }
    }

    /// One frame of a battle (see the module docs); sets the next mode.
    pub(super) fn step_battle(
        &mut self,
        game: &impl GameData,
        mut run: Box<BattleRun>,
        f: &mut Frame,
    ) {
        let s = &mut self.s;
        match run.stage {
            Stage::Transition => {
                if !run.started {
                    let mut events = Vec::new();
                    run.battle.start(game, &mut s.rng, &mut events);
                    run.scene.push(&mut events);
                    run.started = true;
                }
                match &mut run.backdrop {
                    Some(starter) => {
                        starter.update(game, &Input::default(), f);
                    }
                    None => draw_field(game, s, f),
                }
                if run.transition.update(f) {
                    run.backdrop = None;
                    run.stage = Stage::Loading;
                }
            }
            Stage::Loading => {
                if run.scene.load(game, &run.battle, f) {
                    run.stage = Stage::Running;
                }
            }
            Stage::Running => {
                run.scene
                    .step(game, &mut run.battle, &mut s.rng, &s.input, f);
                if let Some(outcome) = run.scene.finished() {
                    run.outcome = Some(outcome);
                    s.event(super::Event::BattleEnded(outcome));
                    s.party = run.battle.party(Side::Player).clone();
                    if outcome == BattleOutcome::Won {
                        after_win(game, s, &run.battle);
                    }
                    if run.battle.caught_without_room().is_some() {
                        s.log("caught a mon with a full party (no PC): released");
                    }
                    let first = matches!(
                        run.battle.setup().kind,
                        BattleKind::Wild { first_battle: true }
                    );
                    let lost = matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew);
                    run.stage = if lost && !first {
                        Stage::Whiteout(0)
                    } else {
                        Stage::Leaving
                    };
                }
            }
            Stage::Leaving => {
                // The fade from black starts black and only moves in `Returning`.
                restore_field_spread(s, f);
                s.fader.start(Fade::FromBlack);
                draw_field(game, s, f);
                run.stage = if s.field.uploading() {
                    Stage::Restoring
                } else {
                    Stage::Returning
                };
            }
            Stage::Restoring => {
                draw_field(game, s, f);
                if !s.field.uploading() {
                    run.stage = Stage::Returning;
                }
            }
            Stage::Whiteout(n) if n + 1 < WHITEOUT_FRAMES => {
                f.brightness = FADE_MAX;
                run.stage = Stage::Whiteout(n + 1);
            }
            Stage::Whiteout(_) => {
                self.whiteout(game, f);
                run.whiteout = true;
                run.stage = Stage::Returning;
            }
            Stage::Returning => {
                // The overworld runs during `FadeInFromBlack` (objects frozen by a grass
                // encounter stay put until the fade is over).
                let ground = Ground {
                    game,
                    field: &s.field,
                };
                s.objects.update(&ground, &mut s.rng, false);
                s.fader.update();
                draw_field(game, s, f);
                if s.fader.is_done() {
                    if run.origin == Origin::Field || run.whiteout {
                        s.objects.unfreeze_all();
                    }
                    if run.origin == Origin::Script && !run.whiteout {
                        s.answer = run.outcome.map(|o| o as u16);
                    }
                    // `RestartWildEncounterImmunitySteps` (also done at the battle start).
                    s.encounter_immunity = 0;
                    drain_reports(s, &mut run.scene);
                    s.mode = Mode::Field;
                    return;
                }
            }
        }
        drain_reports(&mut self.s, &mut run.scene);
        self.s.mode = Mode::Battle(run);
    }

    /// Gen3 `CB2_WhiteOut` → `DoWhiteOut`: the script context is reset, `EventScript_WhiteOut`
    /// runs at once (`RunScriptImmediately`), the money is halved, the party healed, and the
    /// respawn heal location loaded (black frame), the player facing down.
    /// `Overworld_ResetStateAfterWhiteOut`'s flag and weather resets are not modelled.
    fn whiteout(&mut self, game: &impl GameData, f: &mut Frame) {
        self.runner.abort();
        let s = &mut self.s;
        if let Some(id) = game.special_ids().script_whiteout {
            super::run_immediate(game, s, f, id);
        }
        s.money /= 2;
        s.selected = None;
        s.last_mover = 0;
        s.answer = None;
        s.party.heal(game.battle());
        s.text.close(f);
        s.objects.invalidate_vram();
        match game.heal_location(s.respawn) {
            Some((map, x, y)) if game.has_map(map) => {
                load_map(game, s, f, map, x, y);
                s.objects.face(PLAYER, Dir::Down);
            }
            _ => {
                s.log("whiteout: no respawn heal location; the field is restored in place");
                s.field.restore(game, f);
            }
        }
        s.fader.start(Fade::FromBlack);
        draw_field(game, s, f);
    }
}
