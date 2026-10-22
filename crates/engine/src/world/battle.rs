//! Battles and the starter screen as World modes (Milestone 4 Task J).
//!
//! A battle starts from a script ([`Request::BattleWild`](crate::script::Request) /
//! `BattleTrainer`, or the first battle after the starter screen) or from a field step in tall
//! grass. Its [`BattleRun`] goes through these stages:
//! 1. **Transition** (`BattleSetup_StartWildBattle` / `_StartTrainerBattle` →
//!    `BattleTransition_StartOnField`): objects frozen, the field (or, for the first battle,
//!    the starter screen: `CB2_GiveStarter` → `CB2_StartFirstBattle`) keeps being drawn under
//!    [`BattleTransition`], which ends black.
//! 2. **Loading**: [`BattleScene::load`] chunks, one per black frame (`CB2_InitBattle`).
//! 3. **Running**: [`BattleScene::step`] each frame until [`BattleScene::finished`] (the scene
//!    fades to black after `End`). The player's party is written back.
//! 4. **Leaving** (`CB2_ReturnToField*`): on one black frame the field is restored (tilesets,
//!    palettes and BG controls re-uploaded, the view redrawn, object sprite caches dropped, the
//!    field text box rebuilt), then **Returning**: the field fades in
//!    (`FieldCB_ContinueScript*`: `FadeInFromBlack`, then `Task_WaitForFadeAndEnableScriptCtx`)
//!    and only then is the script answered.
//! 5. **Whiteout** instead of Leaving when the player lost (`Lost` or `Drew`, Gen3
//!    `IsPlayerDefeated`), except in the first battle (`CB2_EndFirstBattle` always returns to its
//!    script): Gen3 `CB2_WhiteOut`, simplified: 120 black frames, then the script is aborted, the
//!    party healed and the player put on the respawn heal location (`setrespawn`) while black,
//!    facing down; then Returning. No money loss and no Mom / nurse script.
//!
//! Deviations from Gen3 (documented):
//! - The opponent party and `Battle::new`'s draws happen when the battle starts, before the
//!   transition (Gen3 creates trainer parties and the scripted wild mon in `CB2_InitBattle`,
//!   after it). The order of draws within them is Gen3's.
//! - The field text box is closed when a battle starts: `trainerbattle`'s intro message stays
//!   open into the battle in Gen3 and disappears under the transition, which overwrites BG0.
//! - The environment is `Grass` on a tall-grass cell and `Plain` elsewhere
//!   (`BattleSetup_GetEnvironmentId`, simplified).
//! - Field wild encounters also require a party with a mon that can fight (Gen3 relies on the
//!   story never letting the player into grass without one).

use super::{State, World, draw_field, load_map};
use crate::battle::scene::BattleScene;
use crate::battle::transition::BattleTransition;
use crate::battle::{Battle, BattleKind, BattleOutcome, BattleSetup, Environment, Side};
use crate::data::{Behavior, Dir, GameData, MapId, PLAYER, SpeciesId};
use crate::flow::{FADE_MAX, Fader, Input};
use crate::hash::Fnv64;
use crate::mon::{Ivs, Party, Pokemon};
use crate::objects::Terrain;
use crate::script::Fade;
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
    /// The field is restored on this (black) frame.
    Leaving,
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

    /// `BattleSetup_GetEnvironmentId`, simplified: tall grass or plain.
    fn environment(&self, game: &impl GameData) -> Environment {
        let (x, y) = self.objects.player().pos();
        if self.ground(game).behavior(x, y) == Behavior::TallGrass {
            Environment::Grass
        } else {
            Environment::Plain
        }
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
        let mut battle = Battle::new(game, setup, &self.party, &mut self.rng);
        let mut events = Vec::new();
        battle.start(game, &mut self.rng, &mut events);
        let mut scene = BattleScene::new(battle.setup());
        scene.push(&mut events);
        if self.text.is_open() {
            self.text.close(f);
        }
        self.objects.freeze_all();
        self.mode = Mode::Battle(Box::new(BattleRun {
            stage: Stage::Transition,
            origin,
            battle,
            scene,
            transition: BattleTransition::new(&kind),
            backdrop: None,
            outcome: None,
            whiteout: false,
        }));
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
        let found = match game.encounters(map) {
            Some(table) if self.can_battle() => {
                // `gPlayerParty[0]`, fainted or not.
                let lead = self.party.get(0).copied();
                wild::try_land_encounter(
                    table,
                    self.prev_behavior,
                    cur,
                    lead.as_ref(),
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

/// Re-uploads the field after a battle or the starter screen owned the screen (black frame):
/// tilesets, palettes and BG controls (the view is redrawn at the frame's camera update), the
/// object sprite caches dropped, and a new field text box (BG0 off and cleared).
fn restore_field(game: &impl GameData, s: &mut State, f: &mut Frame) {
    s.field.restore(game, f);
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
                    s.party = run.battle.party(Side::Player).clone();
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
                restore_field(game, s, f);
                s.fader.start(Fade::FromBlack);
                draw_field(game, s, f);
                run.stage = Stage::Returning;
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
                    s.mode = Mode::Field;
                    return;
                }
            }
        }
        self.s.mode = Mode::Battle(run);
    }

    /// Gen3 `CB2_WhiteOut` → `DoWhiteOut`, simplified: the script context is reset, the party
    /// healed, and the respawn heal location loaded (black frame), the player facing down.
    fn whiteout(&mut self, game: &impl GameData, f: &mut Frame) {
        self.runner.abort();
        let s = &mut self.s;
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
