//! Battles and the starter screen as World modes (Milestone 4 Task J).
//!
//! A battle starts from a script ([`Request::BattleWild`](crate::script::Request) /
//! `BattleTrainer`, or the first battle after the starter screen) or from a field step in tall
//! grass. Its [`BattleRun`] goes through these stages:
//! 1. **Transition** (`BattleSetup_StartWildBattle` / `_StartTrainerBattle` →
//!    `BattleTransition_StartOnField`): objects frozen for a grass encounter only (Gen3
//!    `DoStandardWildBattle`; scripted battles leave that to the script), the field (or, for
//!    the first battle, the starter screen: `CB2_GiveStarter` → `CB2_StartFirstBattle`) keeps
//!    being drawn under [`BattleTransition`], which ends black. Objects keep animating under
//!    it unless the battle froze them (`DoStandardWildBattle`); the first battle's transition
//!    is the starter screen (`CB2_StartFirstBattle` does not run object events). The frame
//!    after the transition, `CB2_InitBattle` draws `BattleAI_HandleItemUseBeforeAISetup`, then
//!    builds the trainer party or the first battle's mon, then `SetWildMonHeldItem`.
//!    [`Battle::start`] (the intro and the first turn) is the frame after that, so the setup
//!    frame stays under the line budget.
//! 2. **Loading**: [`BattleScene::load`] chunks, one per black frame (`CB2_InitBattle`).
//! 3. **Running**: [`BattleScene::step`] each frame until [`BattleScene::finished`] (the scene
//!    fades to black after `End`). The player's party is written back; on a win the prize
//!    money is added (Gen3 `getmoneyreward`, `AddMoney`). Evolutions wait for Leaving.
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

use super::mode::Mode;
use super::{Ground, State, World, draw_field, load_map};
use crate::battle::BattleReport;
use crate::battle::scene::BattleScene;
use crate::battle::transition::BattleTransition;
use crate::battle::{Battle, BattleKind, BattleOutcome, BattleSetup, Environment, Side};
use crate::data::{
    Behavior, Dir, Floor, GameData, MapId, MapType, PLAYER, Pocket, SpeciesId, Water,
};
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

/// Who the battle is against, rolled when Gen3 rolls it.
pub(super) enum Foes {
    /// Already built: a field `CreateWildMon`, `setwildbattle`'s `CreateScriptedWildMon`, or a
    /// cloned party. Those draws are before the transition.
    Ready(Party),
    /// `CreateNPCTrainerParty`, after the transition and after [`Battle::draws_before_party`].
    Trainer {
        id: crate::data::TrainerId,
        second: Option<(crate::data::TrainerId, crate::data::TextId)>,
    },
    /// The first battle's `CreateMon` inside `SetUpBattleVarsAndBirchZigzagoon`, after the AI
    /// draws and before `SetWildMonHeldItem`.
    First { species: SpeciesId, level: u8 },
}

/// What [`State::open_pending`] needs once the transition has finished.
struct Pending {
    kind: BattleKind,
    environment: Environment,
    rules: crate::battle::BattleRules,
    double: Option<crate::battle::Double>,
    foes: Foes,
}

impl Pending {
    fn hash(&self, h: &mut Fnv64) {
        match self.kind {
            BattleKind::Wild { first_battle } => h.write(&[0, first_battle as u8]),
            BattleKind::Trainer { trainer, defeat } => {
                h.write(&[1]);
                h.write_u16(trainer.0);
                h.write_u16(defeat.0);
            }
        }
        h.write(&[self.environment as u8]);
        match self.double {
            None => h.write(&[0]),
            Some(d) => {
                h.write(&[1]);
                h.write_u16(d.second.map_or(0xFFFF, |(t, _)| t.0));
                h.write_u16(d.second.map_or(0xFFFF, |(_, x)| x.0));
                h.write_u16(d.partner.map_or(0xFFFF, |t| t.0));
                h.write(&d.split);
            }
        }
        if self.rules == crate::battle::BattleRules::default() {
            h.write(&[0]);
        } else {
            h.write(&[1]);
            self.rules.hash(h);
        }
        match &self.foes {
            Foes::Ready(p) => {
                h.write(&[0]);
                p.hash(h);
            }
            Foes::Trainer { id, second } => {
                h.write(&[1]);
                h.write_u16(id.0);
                h.write_u16(second.map_or(0xFFFF, |(t, _)| t.0));
                h.write_u16(second.map_or(0xFFFF, |(_, x)| x.0));
            }
            Foes::First { species, level } => {
                h.write(&[2]);
                h.write_u16(species.0);
                h.write(&[*level]);
            }
        }
    }
}

/// `base` is what the request knew (the partner, trainer A's half); `built` is what
/// `CreateNPCTrainerParty` filled in (trainer B, the opponent split).
fn merge_double(
    base: Option<crate::battle::Double>,
    built: Option<crate::battle::Double>,
) -> Option<crate::battle::Double> {
    use crate::battle::Double;
    match (base, built) {
        (None, built) => built,
        (Some(base), None) => Some(base),
        (Some(base), Some(built)) => Some(Double {
            second: built.second.or(base.second),
            partner: base.partner.or(built.partner),
            split: [
                if base.split[0] != 0 {
                    base.split[0]
                } else {
                    built.split[0]
                },
                if built.split[1] != 0 {
                    built.split[1]
                } else {
                    base.split[1]
                },
            ],
        }),
    }
}

pub(crate) struct BattleRun {
    pub stage: Stage,
    pub origin: Origin,
    /// `None` until `CB2_InitBattle`, the frame after the transition.
    pub battle: Option<Battle>,
    pub scene: BattleScene,
    pub transition: BattleTransition,
    /// The starter screen the first battle's transition runs over (`CB2_StartFirstBattle`).
    pub backdrop: Option<Box<StarterChoice>>,
    pub outcome: Option<BattleOutcome>,
    /// The player whited out (no answer to the script, objects unfrozen).
    pub whiteout: bool,
    /// [`Battle::start`] ran (the frame after the party was built).
    pub started: bool,
    /// The transition has ended; the next frame builds the party (`CB2_InitBattle`).
    primed: bool,
    pending: Option<Pending>,
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
        Behavior::Ice => [12, 0],
        Behavior::ForcedWalk(d) => [13, d as u8],
        Behavior::Slide(d) => [14, d as u8],
        Behavior::MuddySlope => [15, 0],
        Behavior::ThinIce => [16, 0],
        Behavior::CrackedIce => [17, 0],
        Behavior::CrackedFloor => [18, 0],
        Behavior::CrackedFloorHole => [19, 0],
        Behavior::Floor(g) => [20, g as u8],
        Behavior::Water(w) => [21, w.code()],
        Behavior::PadWarp => [22, 0],
        Behavior::PopOutWarp(d) => [23, d as u8],
        Behavior::SecretBase(s) => [24, s as u8],
    }
}

/// Gen3 `BattleSetup_GetEnvironmentId` for the player's cell `b` on a map of type `map`:
/// tall grass, long grass and sand (both kinds) first, then the map type (caves; buildings;
/// underwater), then mountain tops, then Route 113, else plain.
///
/// Water (Wave C): pond in caves, water on ocean routes, else water on deep or ocean water and
/// pond on the other surfable cells. Not modelled: the surfing-on-a-bridge branch (bridges are
/// not told apart) and Gen3's `WEATHER_SANDSTORM` case (no field weather), so a
/// desert battle off sand is plain.
pub(crate) fn environment(b: Behavior, map: MapType) -> Environment {
    match b {
        Behavior::TallGrass => return Environment::Grass,
        Behavior::Floor(Floor::LongGrass) => return Environment::LongGrass,
        Behavior::Floor(Floor::Sand | Floor::DeepSand) => return Environment::Sand,
        _ => {}
    }
    let water = match b {
        Behavior::Water(w) => Some(w),
        _ => None,
    };
    match map {
        MapType::Underground if b == Behavior::Floor(Floor::IndoorEncounter) => {
            Environment::Building
        }
        MapType::Underground if water.is_some() => Environment::Pond,
        MapType::Underground => Environment::Cave,
        MapType::Indoor => Environment::Building,
        MapType::Underwater => Environment::Underwater,
        MapType::OceanRoute if water.is_some() => Environment::Water,
        MapType::OceanRoute => Environment::Plain,
        MapType::Outdoor | MapType::OutdoorSand if water.is_some_and(Water::deep_or_ocean) => {
            Environment::Water
        }
        MapType::Outdoor | MapType::OutdoorSand if water.is_some() => Environment::Pond,
        MapType::Outdoor | MapType::OutdoorSand if b == Behavior::Floor(Floor::MountainTop) => {
            Environment::Mountain
        }
        MapType::OutdoorSand => Environment::Sand,
        MapType::Outdoor => Environment::Plain,
    }
}

impl BattleRun {
    /// The run's part of `Mode::hash` (after the mode tag).
    pub(super) fn hash(&self, h: &mut Fnv64) {
        h.write(&self.stage.code());
        h.write(&[
            self.origin as u8,
            self.outcome.map_or(0, |o| o as u8),
            self.whiteout as u8,
            self.started as u8,
        ]);
        self.transition.hash(h);
        match &self.battle {
            Some(b) => b.hash(h),
            None => {
                // Only while the transition is up: afterwards the battle hash above is the
                // whole core, as before.
                h.write(&[0xB1]);
                self.pending.as_ref().unwrap().hash(h);
            }
        }
        if self.stage == Stage::Transition {
            h.write(&[self.primed as u8]);
        }
        self.scene.hash(h);
        match &self.backdrop {
            None => h.write(&[0]),
            Some(s) => {
                h.write(&[1]);
                s.hash(h);
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

    /// `CB2_InitBattle`: AI setup draws, then the trainer party or the first battle's mon,
    /// then `SetWildMonHeldItem` and `gRandomTurnNumber` ([`Battle::after_party`]).
    fn open_pending(&mut self, game: &impl GameData, pending: Pending) -> Battle {
        Battle::draws_before_party(&mut self.rng, pending.double.is_some());
        let (opponents, double) = match pending.foes {
            Foes::Ready(p) => (p, pending.double),
            Foes::First { species, level } => {
                let mon = self.create_mon(game, species, level);
                let mut party = Party::default();
                let _ = party.push(mon);
                (party, pending.double)
            }
            Foes::Trainer { id, second } => {
                let (party, built) = trainer_party(game, &mut self.rng, id, second)
                    .expect("the trainer party was checked before the transition");
                (party, merge_double(pending.double, built))
            }
        };
        let setup = BattleSetup {
            kind: pending.kind,
            opponents,
            environment: pending.environment,
            double,
            rules: pending.rules,
        };
        let mut battle = Battle::after_party(game, setup, &self.party, &mut self.rng);
        // The player's form changes need their flag (the Mega Ring). Emerald has no form
        // changes: this returns at once. Battle style is the option at the request (Gen3
        // reads `OPTION` while the battle is set up).
        battle.set_battle_style(self.options.battle_style);
        battle.enable_player_forms(game, |f| self.vars.flag(f));
        battle
    }

    /// Starts a battle against `opponents` (non-empty; the party must be able to battle):
    /// the scene and the transition. The core is built after the transition. Freezes objects
    /// for a field battle and closes the field text box.
    pub(super) fn start_battle(
        &mut self,
        game: &impl GameData,
        kind: BattleKind,
        opponents: Party,
        origin: Origin,
        f: &mut Frame,
    ) {
        self.start_battle_with(
            game,
            kind,
            Foes::Ready(opponents),
            None,
            Default::default(),
            origin,
            f,
        );
    }

    /// Puts an in-game partner's `mons` in the party after the player's first
    /// [`crate::battle::MULTI_PARTY_SIZE`] (`FillPartnerParty`), the others on the bench
    /// until the battle ends. Returns how many of the player's own fight.
    pub(super) fn join_partner(&mut self, mons: &Party) -> u8 {
        let own = self
            .party
            .len()
            .min(crate::battle::MULTI_PARTY_SIZE as usize);
        let (mut party, mut bench) = (Party::default(), Party::default());
        for (i, m) in self.party.iter().enumerate() {
            let _ = if i < own {
                party.push(*m)
            } else {
                bench.push(*m)
            };
        }
        for m in mons.iter() {
            let _ = party.push(*m);
        }
        self.party = party;
        self.stash.bench = bench;
        own as u8
    }

    /// [`World::start_battle`] with the double battle's setup (two trainers, the in-game
    /// partner). A trainer whose data says double (`trainerbattle_double`, the twins) battles
    /// in doubles without one. `rules`: Silice's per-battle rules (`battle::rules`; its
    /// background replaces the cell's). The opponent party is not rolled here: `CB2_InitBattle`
    /// rolls it after the transition.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn start_battle_with(
        &mut self,
        game: &impl GameData,
        kind: BattleKind,
        foes: Foes,
        double: Option<crate::battle::Double>,
        rules: crate::battle::BattleRules,
        origin: Origin,
        f: &mut Frame,
    ) {
        // A trainer's own rules (`GameData::trainer_rules`) when the request names none.
        let rules = match kind {
            BattleKind::Trainer { trainer, .. } if rules == Default::default() => {
                game.trainer_rules(trainer).copied().unwrap_or_default()
            }
            _ => rules,
        };
        let mut double = double.or_else(|| match kind {
            BattleKind::Trainer { trainer, .. }
                if game.trainer(trainer).is_some_and(|t| t.double) =>
            {
                Some(crate::battle::Double::default())
            }
            _ => None,
        });
        // Two opponents are a double even when trainer A's data is not (`BATTLE_TYPE_TWO_OPPONENTS`).
        if let Foes::Trainer {
            second: Some(second),
            ..
        } = &foes
        {
            let mut d = double.unwrap_or_default();
            d.second = Some(*second);
            double = Some(d);
        }
        // Gen3 `ABILITYEFFECT_SWITCH_IN_WEATHER` reads the field's current weather.
        let rules = crate::battle::BattleRules {
            weather: crate::weather::battle_weather(self.weather.current),
            ..rules
        };
        let environment = rules.background.unwrap_or_else(|| self.environment(game));
        // The scene only needs the kind and whether it is a double. The party is rolled later.
        let scene_setup = BattleSetup {
            kind,
            opponents: Party::default(),
            environment,
            double,
            rules,
        };
        let transition = BattleTransition::new(&kind);
        let pending = Pending {
            kind,
            environment,
            rules,
            double,
            foes,
        };
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
                                      primed: _,
                                      pending: _,
                                  }| {};
        // SAFETY: `p` points to the allocation of an uninitialised `BattleRun`. Each field is
        // written exactly once through a raw field pointer before `assume_init`. The writes
        // must cover the field list of the destructuring guard above, which the compiler keeps
        // in sync with the struct.
        let mut scene = BattleScene::new(&scene_setup, self.player_name);
        scene.set_animations(self.options.battle_scene);
        if let Some(b) = (self.bag.pocket(Pocket::PokeBalls).iter()).find(|b| b.qty > 0) {
            scene.set_balls(b.item, b.qty);
        }
        let run = unsafe {
            (&raw mut (*p).scene).write(scene);
            (&raw mut (*p).stage).write(Stage::Transition);
            (&raw mut (*p).origin).write(origin);
            (&raw mut (*p).battle).write(None);
            (&raw mut (*p).transition).write(transition);
            (&raw mut (*p).backdrop).write(None);
            (&raw mut (*p).outcome).write(None);
            (&raw mut (*p).whiteout).write(false);
            (&raw mut (*p).started).write(false);
            (&raw mut (*p).primed).write(false);
            (&raw mut (*p).pending).write(Some(pending));
            run.assume_init()
        };
        // Gen3 `IncrementDailyWildBattles` in every wild battle's setup.
        if let BattleKind::Wild { .. } = kind
            && let Some(v) = game.special_ids().var_daily_wilds
        {
            let n = self.vars.var(v).wrapping_add(1);
            self.vars.set_var(v, n);
        }
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

    /// Gen3 `RockSmashWildEncounter` (`wild_encounter.c`): the current map's rock smash table
    /// (none: no battle), `wild::try_rock_smash_encounter`, `create_wild`, then
    /// `BattleSetup_StartWildBattle` (a field battle). Returns whether a battle started.
    pub(super) fn rock_smash_encounter(&mut self, game: &impl GameData, f: &mut Frame) -> bool {
        if !self.can_battle() {
            return false;
        }
        let t = game.battle();
        let map = self.field.map();
        let wild_set = game
            .special_ids()
            .var_wild_set
            .map_or(0, |v| self.vars.var(v));
        let Some(table) = game.rock_smash_encounters(map, wild_set) else {
            return false;
        };
        let lead = self.party.get(0).copied();
        let (vars, party, ids) = (&self.vars, &self.party, game.special_ids());
        let lead_of = wild::Lead::of(lead.as_ref(), t, |i| game.hold_effect(i));
        let repel = |level| wild::repel_blocks(level, vars, party, &ids);
        let Some((species, level)) =
            wild::try_rock_smash_encounter(table, lead_of, &mut self.rng, repel)
        else {
            return false;
        };
        let mon = wild::create_wild(
            t,
            species,
            level,
            lead.as_ref(),
            self.player_id,
            &mut self.rng,
        );
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
        // `gPlayerParty[0]`, fainted or not.
        let lead = self.party.get(0).copied();
        let (vars, party, ids) = (&self.vars, &self.party, game.special_ids());
        let lead_of = wild::Lead::of(lead.as_ref(), t, |i| game.hold_effect(i));
        let repel = |level| wild::repel_blocks(level, vars, party, &ids);
        let rolled = if !self.can_battle() {
            None
        } else if cur.water_encounter() {
            // The water branch (Wave C: surfing).
            game.water_encounters(map, wild_set).and_then(|table| {
                wild::try_water_encounter(
                    table,
                    self.prev_behavior,
                    cur,
                    lead_of,
                    t,
                    &mut self.rng,
                    repel,
                )
            })
        } else {
            game.wild_encounters(map, wild_set).and_then(|table| {
                wild::try_land_encounter_repel(
                    table,
                    self.prev_behavior,
                    cur,
                    lead_of,
                    t,
                    &mut self.rng,
                    repel,
                )
            })
        };
        let found = rolled.map(|(species, level)| {
            wild::create_wild(
                t,
                species,
                level,
                lead.as_ref(),
                self.player_id,
                &mut self.rng,
            )
        });
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

/// The species of the opponent's battler (its party mon's: a battle form, such as a Mega
/// Evolution, is seen as its base species).
fn opponent_species(battle: &Battle) -> Option<SpeciesId> {
    let slot = battle.mon(Side::Opponent).party;
    battle
        .party(Side::Opponent)
        .get(slot as usize)
        .map(|p| p.species)
}

/// Sets the Pokédex seen flag of the opponent out (Milestone 9 Wave 0).
fn see_opponent(s: &mut State, battle: &Battle) {
    if let Some(species) = opponent_species(battle) {
        s.pokedex.set(crate::pokedex::DexFlag::Seen, species);
    }
}

/// The battle hands the frame back (to the field, or to the evolution scenes): the objects a
/// field battle froze move again, a script's battle request gets its answer, and the encounter
/// immunity restarts.
fn end_battle(s: &mut State, run: &mut BattleRun) {
    if run.origin == Origin::Field || run.whiteout {
        s.objects.unfreeze_all();
    }
    if run.origin == Origin::Script && !run.whiteout {
        s.answer = run.outcome.map(|o| o as u16);
    }
    // `RestartWildEncounterImmunitySteps` (also done at the battle start).
    s.encounter_immunity = 0;
    drain_reports(s, &mut run.scene);
}

/// A world report: its log line and its event.
pub(super) fn report(s: &mut State, r: Report) {
    s.log(match r {
        Report::Unsupported(what) | Report::Degraded(what) => what,
    });
    s.event(super::Event::Report(r));
}

/// After a win (Gen3 `getmoneyreward`): the prize money and Pay Day's coins (`AddMoney`,
/// capped at `SpecialIds::max_money`). Evolutions play in [`super::evolve::after_battle`].
fn after_win(game: &impl GameData, s: &mut State, battle: &Battle) {
    // `getmoneyreward`, then Pay Day's `givepaydaymoney` (Wave D).
    for money in [battle.prize(), battle.pay_day()].into_iter().flatten() {
        let max = game.special_ids().max_money;
        if max == 0 && money > 0 {
            report(s, Report::Degraded(super::host::NO_MONEY));
        }
        s.money = s.money.saturating_add(money).min(max);
    }
    // Wave G: `SetBattledTrainersFlags` also sets trainer B's flag (content sets A's).
    if let Some((_, _, script)) = s.pair.b.take()
        && let Some(flag) = game.trainer_flag_of(script)
    {
        s.vars.set_flag(flag, true);
    }
}

/// Whether `id` has a party `CreateNPCTrainerParty` can fill. The mons themselves are rolled
/// later, after the transition.
pub(super) fn trainer_has_party(game: &impl GameData, id: crate::data::TrainerId) -> bool {
    game.trainer(id).is_some_and(|t| !t.party.is_empty())
}

/// The opponents of a trainer battle (Gen3 `CreateNPCTrainerParty`): trainer A's party, or,
/// with trainer B (`BATTLE_TYPE_TWO_OPPONENTS`, Wave G), at most three of A's
/// (`gEnemyParty[0..]`) then at most three of B's (`gEnemyParty[3..]`), with the battle's
/// [`crate::battle::Double`]. `None` when A is unknown or has no mon that can fight.
pub(super) fn trainer_party(
    game: &impl GameData,
    rng: &mut crate::rng::Rng,
    trainer: crate::data::TrainerId,
    second: Option<(crate::data::TrainerId, crate::data::TextId)>,
) -> Option<(Party, Option<crate::battle::Double>)> {
    use crate::battle::MULTI_PARTY_SIZE;
    use crate::trainer::{create_trainer_party, create_trainer_party_up_to};
    let t = game.battle();
    let a = game.trainer(trainer)?;
    let Some((b, b_defeat)) = second else {
        let p = create_trainer_party(t, a, rng);
        return p.first_usable().is_some().then_some((p, None));
    };
    let half = MULTI_PARTY_SIZE as usize;
    let mut p = create_trainer_party_up_to(t, a, rng, half);
    let split = p.len() as u8;
    if let Some(bt) = game.trainer(b) {
        for m in create_trainer_party_up_to(t, bt, rng, half).iter() {
            let _ = p.push(*m);
        }
    }
    let double = crate::battle::Double {
        second: Some((b, b_defeat)),
        partner: None,
        split: [0, split],
    };
    p.first_usable().is_some().then_some((p, Some(double)))
}

/// The in-game partner's trainer and mons (Gen3 `FillPartnerParty`, at most
/// `MULTI_PARTY_SIZE`): the game's [`GameData::partner`] when `partner` is `None` or its
/// trainer; else trainer `partner`'s own party (`CreateNPCTrainerParty`, a game's partner).
pub(super) fn partner_party(
    game: &impl GameData,
    rng: &mut crate::rng::Rng,
    partner: Option<crate::data::TrainerId>,
) -> Option<(crate::data::TrainerId, Party)> {
    let t = game.battle();
    if let Some(p) = game
        .partner()
        .filter(|p| partner.is_none_or(|id| id == p.trainer))
    {
        return Some((p.trainer, crate::trainer::create_partner_party(t, p, rng)));
    }
    let id = partner?;
    let half = crate::battle::MULTI_PARTY_SIZE as usize;
    let p = crate::trainer::create_trainer_party_up_to(t, game.trainer(id)?, rng, half);
    p.first_usable().is_some().then_some((id, p))
}

/// Re-uploads the field after a battle or the starter screen owned the screen (black frame):
/// tilesets, palettes and BG controls (the view is redrawn at the frame's camera update), the
/// object sprite caches dropped, and a new field text box (BG0 off and cleared).
fn restore_field(game: &impl GameData, s: &mut State, f: &mut Frame) {
    f.bulk = true;
    s.field.restore(game, f);
    s.objects.invalidate_vram();
    s.weather.resume(); // Gen3 `ResumeMap` → `ResumePausedWeather`
    s.text.close(f);
}

/// [`restore_field`] with the tilesets and view uploaded over the next frames
/// ([`crate::field::Field::restore_spread`]); the caller keeps the screen black while
/// [`crate::field::Field::uploading`] (Milestone 8 Task F).
fn restore_field_spread(s: &mut State, f: &mut Frame) {
    f.bulk = true;
    s.field.restore_spread();
    s.objects.invalidate_vram();
    s.weather.resume(); // Gen3 `ResumeMap` → `ResumePausedWeather`
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
                if run.battle.is_some() {
                    // The frame after the party: the intro and the first turn (`Battle::start`).
                    let _ = run.transition.update(f);
                    let mut events = Vec::new();
                    run.battle
                        .as_mut()
                        .unwrap()
                        .start(game, &mut s.rng, &mut events);
                    run.scene.push(&mut events);
                    run.started = true;
                    // Gen3 `HandleSetPokedexFlag(.., FLAG_SET_SEEN, ..)` at the battle intro.
                    see_opponent(s, run.battle.as_ref().unwrap());
                    run.stage = Stage::Loading;
                } else if run.primed {
                    // `CB2_InitBattle`: no object events. The screen stays black.
                    let _ = run.transition.update(f);
                    let pending = run.pending.take().unwrap();
                    run.battle = Some(s.open_pending(game, pending));
                } else {
                    match &mut run.backdrop {
                        Some(starter) => {
                            starter.update(game, &Input::default(), f);
                        }
                        None => {
                            // Gen3's overworld keeps running under `Task_BattleStart`.
                            let ground = Ground {
                                game,
                                field: &s.field,
                            };
                            s.objects.update(&ground, &mut s.rng, false);
                            draw_field(game, s, f);
                        }
                    }
                    if run.transition.update(f) {
                        run.backdrop = None;
                        run.primed = true;
                    }
                }
            }
            Stage::Loading => {
                if run.scene.load(game, run.battle.as_ref().unwrap(), f) {
                    run.stage = Stage::Running;
                }
            }
            Stage::Running => {
                run.scene
                    .step(game, run.battle.as_mut().unwrap(), &mut s.rng, &s.input, f);
                // Each opponent switched in (Gen3 `Cmd_switchindataupdate`'s seen flag): the
                // opponent out this frame, which every switch-in is for at least one frame.
                see_opponent(s, run.battle.as_ref().unwrap());
                if let Some(outcome) = run.scene.finished() {
                    run.outcome = Some(outcome);
                    s.event(super::Event::BattleEnded(outcome));
                    s.party = run.battle.as_ref().unwrap().party(Side::Player).clone();
                    // Wave G: the in-game partner's mons leave with it (Gen3's script loads
                    // the saved party back over `gPlayerParty`).
                    if run.battle.as_ref().unwrap().has_partner() {
                        let mut own = Party::default();
                        let n = run.battle.as_ref().unwrap().party_range(Side::Player).end as usize;
                        let bench = core::mem::take(&mut s.stash.bench);
                        for m in s.party.iter().take(n).chain(bench.iter()) {
                            let _ = own.push(*m);
                        }
                        s.party = own;
                    }
                    // Gen3 `HandleAction_UseItem` takes each ball out of the bag as it is used.
                    if let Some((ball, n)) = run.scene.balls_thrown() {
                        s.bag.remove(Pocket::PokeBalls, ball, n);
                    }
                    if outcome == BattleOutcome::Won {
                        after_win(game, s, run.battle.as_ref().unwrap());
                    }
                    if outcome == BattleOutcome::Caught
                        && let Some(species) = opponent_species(run.battle.as_ref().unwrap())
                    {
                        // Gen3 `Cmd_trysetcaughtmondexflags`.
                        s.pokedex.set_caught(species);
                    }
                    if let Some(&mon) = run.battle.as_ref().unwrap().caught_without_room() {
                        // Gen3 `GiveMonToPlayer` → `SendMonToPC` (Task M).
                        if super::caught::place_mon(game, s, mon) == super::caught::Placed::NoRoom {
                            s.log("caught a mon with a full party (no PC): released");
                        }
                    }
                    let first = matches!(
                        run.battle.as_ref().unwrap().setup().kind,
                        BattleKind::Wild { first_battle: true }
                    );
                    let lost = matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew);
                    let whiteout = super::host::whiteout_allowed(self.runner.waiting_on());
                    run.stage = if lost && !first && whiteout {
                        Stage::Whiteout(0)
                    } else {
                        Stage::Leaving
                    };
                }
            }
            Stage::Leaving => {
                // Gen3 `FreeResetData_ReturnToOvOrDoEvolutions`: after a win with a level-up,
                // the evolution scenes run over the black screen before the field comes back
                // (Task E opens them; their host restores the field when the last closes).
                if run.outcome == Some(BattleOutcome::Won)
                    && super::evolve::after_battle(game, s, run.battle.as_ref().unwrap(), f)
                {
                    end_battle(s, &mut run);
                    return;
                }
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
                    end_battle(s, &mut run);
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
    pub(super) fn whiteout(&mut self, game: &impl GameData, f: &mut Frame) {
        self.runner.abort();
        let s = &mut self.s;
        if let Some(id) = game.special_ids().script_whiteout {
            super::run_immediate(game, s, f, id);
        }
        s.money /= 2;
        s.selected = None;
        s.last_mover = 0;
        s.answer = None;
        // Wave F: a whiteout while a script holds the party's backup (Steven's multi battle):
        // the chosen mons go back and the backup is loaded first, as the script's
        // `frontier_saveparty` and `LoadPlayerParty` do before its `SetCB2WhiteOut`.
        if s.stash.saved.is_some() {
            s.stash
                .run(&mut s.party, crate::script::PartyStash::SaveSelected);
            s.stash.run(&mut s.party, crate::script::PartyStash::Load);
        }
        s.stash = super::host::Stash::default();
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
