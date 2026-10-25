//! World tests of Milestone 8 Task G (battle breadth: G1 and G2). Owned by that task.
//!
//! The synthetic game plus the data Task G reads: a money table entry for the trainer, a level
//! evolution, and a whiteout script.

use super::*;
use crate::battle::test_data::{FIRE, NORMAL, WATER};
use crate::battle::{BattleOutcome, BattleReport};
use crate::mon::{Ivs, Pokemon, exp_for_level};
use crate::script::Report;
use alloc::vec::Vec;

/// `EventScript_WhiteOut` of the test game.
const WHITEOUT_SCRIPT: ScriptId = ScriptId(40);
const FLAG_WHITEOUT_RAN: FlagId = FlagId(0x310);
/// `gTrainerMoneyTable` value of the test trainer's class.
const MONEY_TABLE: u8 = 4;
/// WATER evolves into FIRE at level 6.
static WATER_EVOS: [Evolution; 1] = [Evolution {
    method: EvoMethod::Level,
    param: 6,
    target: FIRE,
}];
/// NORMAL has two level entries (as Wurmple's Silcoon/Cascoon): the last reached one wins.
static NORMAL_EVOS: [Evolution; 3] = [
    Evolution {
        method: EvoMethod::Level,
        param: 3,
        target: WATER,
    },
    Evolution {
        method: EvoMethod::Friendship,
        param: 0,
        target: WATER,
    },
    Evolution {
        method: EvoMethod::Level,
        param: 3,
        target: FIRE,
    },
];

/// A trainer whose battles have rules of their own (`GameData::trainer_rules`, Silice's
/// Mélomane), with the test trainer's data.
const RULES_TRAINER: TrainerId = TrainerId(3);
static TRAINER_RULES: crate::battle::BattleRules = crate::battle::BattleRules {
    inverse: true,
    field: None,
    background: None,
    foe_back: false,
    clone_player: false,
    intro: None,
    no_catch: false,
    weather: None,
};

/// A double-battle trainer (as Gina & Mia on Route 104), with the test trainer's party.
const DOUBLE_TRAINER: TrainerId = TrainerId(2);
static DOUBLE_DATA: crate::mon::TrainerData = crate::mon::TrainerData {
    class_name: &[1, 0xFF],
    name: &[2, 0xFF],
    pic: &TRAINER_PIC,
    female: false,
    ai: 0,
    double: true,
    party: &[crate::mon::TrainerMon {
        species: NORMAL,
        level: 2,
        iv: 0,
        moves: None,
        item: None,
    }],
};

/// Wave E2: a rock smash table whose rate always rolls a WATER L3.
static ROCKS: crate::mon::WaterEncounters = crate::mon::WaterEncounters {
    rate: 255,
    mons: [crate::mon::WildMon {
        species: WATER,
        min_level: 3,
        max_level: 3,
    }; 5],
};

/// [`Game`] with Task G's data.
struct GGame;

async fn whiteout_script(ctx: Ctx) {
    ctx.set_flag(FLAG_WHITEOUT_RAN, true).await;
}

impl GameData for GGame {
    fn map(&self, id: MapId) -> &'static MapData {
        Game.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            script_whiteout: Some(WHITEOUT_SCRIPT),
            ..Game.special_ids()
        }
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
        Game.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id == WHITEOUT_SCRIPT {
            return Box::pin(whiteout_script(ctx));
        }
        Game.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn wild_encounters(&self, map: MapId, set: u16) -> Option<&'static crate::mon::LandEncounters> {
        Game.wild_encounters(map, set)
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static crate::mon::TrainerData> {
        if id == DOUBLE_TRAINER {
            return Some(&DOUBLE_DATA);
        }
        if id == RULES_TRAINER {
            return Game.trainer(TRAINER);
        }
        Game.trainer(id)
    }
    fn trainer_rules(&self, id: TrainerId) -> Option<&'static crate::battle::BattleRules> {
        (id == RULES_TRAINER).then_some(&TRAINER_RULES)
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        Game.heal_location(id)
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn trainer_money(&self, id: TrainerId) -> Option<u8> {
        self.trainer(id).map(|_| MONEY_TABLE)
    }
    fn rock_smash_encounters(
        &self,
        _map: MapId,
        _wild_set: u16,
    ) -> Option<&'static crate::mon::WaterEncounters> {
        Some(&ROCKS)
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        match species {
            WATER => &WATER_EVOS,
            NORMAL => &NORMAL_EVOS,
            _ => &[],
        }
    }
}

/// Runs frames on [`GGame`], keeping every event.
struct G {
    w: World,
    events: Vec<Event>,
}

impl G {
    fn new() -> G {
        let mut g = G {
            w: World::new(&GGame),
            events: Vec::new(),
        };
        g.run(Buttons::NONE, 16);
        g
    }

    fn run(&mut self, keys: Buttons, frames: u32) {
        for _ in 0..frames {
            self.w.step(&GGame, keys).validate().unwrap();
            self.events.extend_from_slice(self.w.events());
        }
    }

    /// A new A press every other frame until `done` (panics after `max` frames).
    fn mash(&mut self, max: u32, done: impl Fn(&World) -> bool) {
        for n in 0..max {
            if done(&self.w) {
                return;
            }
            let keys = if n % 2 == 0 {
                Buttons::A
            } else {
                Buttons::NONE
            };
            self.run(keys, 1);
        }
        panic!("not done after {max} frames");
    }

    fn reports(&self) -> Vec<Report> {
        let reports = self.events.iter().filter_map(|e| match e {
            Event::Report(r) => Some(*r),
            _ => None,
        });
        reports.collect()
    }
}

#[test]
fn a_trainer_win_pays_the_prize_money() {
    // GetTrainerMoneyToGive: 4 × the last mon's level (NORMAL L2) × the table's 4 = 32, added
    // with AddMoney (capped at MAX_MONEY).
    for (before, after) in [(100, 132), (999_990, 999_999)] {
        let mut g = G::new();
        g.w.s.money = before;
        start(&mut g.w, |ctx| {
            Box::pin(async move {
                ctx.give_mon(WATER, 30).await;
                let o = ctx.battle_trainer(TRAINER, TextId(0)).await;
                ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
            })
        });
        g.run(Buttons::NONE, 1);
        assert_eq!(g.w.battle().unwrap().1.prize(), Some(32));
        g.mash(20_000, |w| !w.script_running());
        assert!(g.w.flag(FLAG_DONE));
        assert_eq!(g.w.money(), after);
    }
}

#[test]
fn a_trainer_without_a_money_entry_pays_nothing_and_is_reported() {
    // The synthetic game's `Game` has no money table.
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_trainer(TRAINER, TextId(0)).await;
        })
    });
    let mut reports = Vec::new();
    for _ in 0..20_000 {
        if !w.script_running() {
            break;
        }
        let keys = if w.frame().is_multiple_of(2) {
            Buttons::A
        } else {
            Buttons::NONE
        };
        run(&mut w, keys, 1);
        reports.extend(
            w.events()
                .iter()
                .filter(|e| matches!(e, Event::Report(_)))
                .copied(),
        );
    }
    assert!(!w.script_running());
    assert_eq!(w.money(), 0);
    assert_eq!(reports, [Event::Report(Report::Degraded("trainer money"))]);
}

#[test]
fn whiteout_runs_the_whiteout_script_and_halves_the_money() {
    // DoWhiteOut: RunScriptImmediately(EventScript_WhiteOut), then the money halved, the party
    // healed and the warp to the last heal location.
    let mut g = G::new();
    g.w.s.money = 101;
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(NORMAL, 2).await;
            ctx.battle_wild(WATER, 60, false).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    g.run(Buttons::NONE, 1);
    g.mash(20_000, |w| {
        w.battle()
            .is_some_and(|b| matches!(b.0, Stage::Whiteout(_)))
    });
    assert!(
        !g.w.flag(FLAG_WHITEOUT_RAN),
        "on the whiteout's last black frame"
    );
    assert_eq!(g.w.money(), 101);
    g.mash(400, |w| !w.in_battle());
    assert!(g.w.flag(FLAG_WHITEOUT_RAN));
    assert!(g.events.contains(&Event::ScriptStarted(WHITEOUT_SCRIPT)));
    assert_eq!(g.w.money(), 50);
    assert!(!g.w.flag(FLAG_DONE), "the battle's script was aborted");
    assert_eq!(g.w.map(), HOUSE);
    let p = g.w.party().get(0).unwrap();
    assert_eq!(p.hp, p.stats[0]);
}

/// Gives `species` at level 5 one exp point short of level 6, then wins a wild battle against
/// NORMAL L2 (enough exp for the level).
fn level_up_in_battle(g: &mut G, species: SpeciesId) {
    let t = GGame.battle();
    let mut mon = Pokemon::new(
        t,
        species,
        5,
        Ivs::Fixed(0),
        0,
        0,
        &mut crate::rng::Rng::new(1),
    );
    mon.exp = exp_for_level(t.species_info(species).growth, 6) - 1;
    g.w.s.party.push(mon).unwrap();
    start(&mut g.w, move |ctx| {
        Box::pin(async move {
            let o = ctx.battle_wild(NORMAL, 2, false).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
    g.mash(20_000, |w| !w.script_running());
    assert!(g.w.flag(FLAG_DONE));
}

#[test]
fn a_mon_that_reached_its_level_evolves_after_the_win() {
    // TryEvolvePokemon for each gLeveledUpInBattle bit: GetEvolutionTargetSpecies (EVO_LEVEL),
    // EvolutionRenameMon (the nickname was the species name) and CalculateMonStats.
    let mut g = G::new();
    level_up_in_battle(&mut g, WATER);
    let t = GGame.battle();
    let p = *g.w.party().get(0).unwrap();
    assert_eq!((p.species, p.level), (FIRE, 6));
    let mut expect = p;
    expect.recalc_stats(t);
    assert_eq!(p.stats, expect.stats, "stats of the new species");
    let fresh = Pokemon::new(
        t,
        FIRE,
        6,
        Ivs::Fixed(0),
        0,
        0,
        &mut crate::rng::Rng::new(1),
    );
    assert_eq!(p.nickname, fresh.nickname, "renamed to the new species");
    assert_eq!(g.reports(), []);
}

#[test]
fn evolution_needs_a_level_gained_in_battle_and_a_win() {
    // A mon already at its evolution level that gains none does not evolve (no
    // gLeveledUpInBattle bit), nor does one after a loss.
    let mut g = G::new();
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_wild(NORMAL, 2, false).await;
        })
    });
    g.run(Buttons::NONE, 1);
    g.mash(20_000, |w| !w.script_running());
    assert_eq!(g.w.party().get(0).unwrap().species, WATER);
    assert!(g.reports().is_empty());
}

#[test]
fn the_last_reached_level_evolution_wins_and_a_branch_is_reported() {
    // GetEvolutionTargetSpecies keeps the last matching entry; `EvoMethod::Level` folds
    // Gen3's personality and stat branches, so two matches are reported.
    let mut g = G::new();
    level_up_in_battle(&mut g, NORMAL);
    assert_eq!(g.w.party().get(0).unwrap().species, FIRE);
    assert_eq!(g.reports(), [Report::Degraded("evolution branch")]);
}

#[test]
fn battle_reports_become_world_events() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_wild(NORMAL, 2, false).await;
        })
    });
    run(&mut w, Buttons::NONE, 2);
    let Mode::Battle(run_) = &mut w.s.mode else {
        panic!("in battle");
    };
    let mut events = alloc::vec![
        crate::battle::BattleEvent::Report(BattleReport::UnsupportedEffect(85)),
        crate::battle::BattleEvent::Report(BattleReport::Degraded("x")),
    ];
    run_.scene.push(&mut events);
    run(&mut w, Buttons::NONE, 1);
    assert_eq!(
        w.events(),
        [
            Event::Report(Report::Unsupported("move effect 085")),
            Event::Report(Report::Degraded("x"))
        ]
    );
}

#[test]
fn the_battle_starts_on_the_transitions_first_frame() {
    // Battle::start (the intro events and the first turn) runs on the frame after the one that
    // started the battle, before the transition draws; the draws are the same.
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_wild(NORMAL, 2, false).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    let (stage, battle, _) = w.battle().unwrap();
    assert_eq!(stage, Stage::Transition);
    assert_eq!(battle.need(), crate::battle::Need::Busy, "not started yet");
    let rng = w.s.rng.state();
    run(&mut w, Buttons::NONE, 1);
    let (_, battle, _) = w.battle().unwrap();
    assert_eq!(battle.need(), crate::battle::Need::Action, "started");
    assert_ne!(w.s.rng.state(), rng, "the first turn's draws");
}

#[test]
fn a_double_trainer_battles_in_doubles() {
    // A double trainer with two usable mons fights a double battle (Wave G); the prize is
    // doubled (BATTLE_TYPE_DOUBLE): 4 × 2 × 4 × 2 = 64.
    let mut g = G::new();
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.give_mon(WATER, 30).await;
            let o = ctx.battle_trainer(DOUBLE_TRAINER, TextId(0)).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
    g.mash(20_000, |w| !w.script_running());
    assert!(g.w.flag(FLAG_DONE));
    assert!(g.reports().is_empty(), "{:?}", g.reports());
    assert_eq!(g.w.money(), 64);
    // A single trainer reports nothing.
    let mut g = G::new();
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_trainer(TRAINER, TextId(0)).await;
        })
    });
    g.mash(20_000, |w| !w.script_running());
    assert!(g.reports().is_empty());
}

#[test]
fn the_battle_return_stays_black_while_the_field_uploads() {
    // Milestone 8 Task F: the field is restored with its uploads spread over frames
    // (`Field::restore_spread`); the screen stays black until the upload is done, then fades.
    let mut g = G::new();
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_trainer(TRAINER, TextId(0)).await;
        })
    });
    let mut restoring = 0;
    for n in 0..20_000 {
        if !g.w.script_running() {
            break;
        }
        let keys = if n % 2 == 0 {
            Buttons::A
        } else {
            Buttons::NONE
        };
        let f = g.w.step(&GGame, keys);
        f.validate().unwrap();
        match g.w.battle().map(|b| b.0) {
            Some(Stage::Restoring) => {
                restoring += 1;
                assert_eq!(f.brightness, crate::flow::FADE_MAX, "black while uploading");
                assert!(g.w.s.field.uploading());
            }
            Some(Stage::Returning) => assert!(!g.w.s.field.uploading()),
            _ => {}
        }
    }
    assert!(!g.w.script_running());
    assert!(restoring > 0, "the upload is spread over frames");
}

#[test]
fn a_field_whiteout_waits_120_frames_then_whites_out() {
    // Wave E2: `special SetCB2WhiteOut` (`EventScript_FieldWhiteOut`): `CB2_WhiteOut` counts
    // 120 frames, then `DoWhiteOut` as after a lost battle; the script never resumes.
    let mut g = G::new();
    g.w.s.money = 101;
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(NORMAL, 2).await;
            ctx.field_whiteout().await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    g.run(Buttons::NONE, u32::from(WHITEOUT_FRAMES));
    assert!(!g.w.flag(FLAG_WHITEOUT_RAN), "still counting");
    assert_eq!(g.w.money(), 101);
    g.run(Buttons::NONE, 2);
    assert!(g.w.flag(FLAG_WHITEOUT_RAN));
    assert_eq!(g.w.money(), 50);
    // The whiteout resets the script context: the script never goes on.
    g.run(Buttons::NONE, 60);
    assert!(!g.w.flag(FLAG_DONE));
    assert_eq!(g.w.map(), HOUSE);
}

#[test]
fn a_rock_smash_encounter_starts_a_field_wild_battle() {
    // Wave E2: `RockSmashWildEncounter` rolls the map's rock table, starts the wild battle
    // (`BattleSetup_StartWildBattle`) and answers TRUE at once; the script goes on after it.
    let mut g = G::new();
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            let started = ctx.rock_smash_encounter().await;
            ctx.set_flag(FLAG_DONE, started).await;
        })
    });
    g.run(Buttons::NONE, 2);
    let (_, battle, _) = g.w.battle().expect("a wild battle");
    let foe = battle.party(crate::battle::Side::Opponent).get(0).unwrap();
    assert_eq!((foe.species, foe.level), (WATER, 3));
    g.mash(20_000, |w| !w.in_battle() && !w.script_running());
    assert!(g.w.flag(FLAG_DONE));
}

#[test]
fn a_no_whiteout_battle_answers_lost_and_the_script_goes_on() {
    // Silice's Samuel (R-7): `battle_trainer_no_whiteout` answers `Lost`, no whiteout runs.
    let mut g = G::new();
    let t = GGame.battle();
    let mut mon = Pokemon::new(
        t,
        NORMAL,
        2,
        Ivs::Fixed(0),
        0,
        0,
        &mut crate::rng::Rng::new(1),
    );
    mon.hp = 1;
    g.w.s.party.push(mon).unwrap();
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            let o = ctx.battle_trainer_no_whiteout(TRAINER, TextId(0)).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Lost).await;
        })
    });
    g.mash(20_000, |w| !w.script_running());
    assert!(g.w.flag(FLAG_DONE), "answered Lost");
    assert!(!g.w.flag(FLAG_WHITEOUT_RAN));
    assert!(!g.events.contains(&Event::ScriptStarted(WHITEOUT_SCRIPT)));
}

#[test]
fn a_clone_battle_fights_a_healed_copy_of_the_party_with_the_rules_background() {
    static RULES: crate::battle::BattleRules = crate::battle::BattleRules {
        inverse: false,
        field: None,
        background: Some(crate::battle::Environment::Cave),
        foe_back: true,
        clone_player: true,
        intro: None,
        no_catch: false,
        weather: None,
    };
    let mut g = G::new();
    start(&mut g.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.give_mon(FIRE, 20).await;
            ctx.battle_trainer_rules(TRAINER, TextId(0), false, &RULES)
                .await;
        })
    });
    g.mash(20_000, |w| w.in_battle());
    let (_, b, _) = g.w.battle().unwrap();
    let foes: Vec<_> = b
        .party(crate::battle::Side::Opponent)
        .iter()
        .map(|m| (m.species, m.level))
        .collect();
    assert_eq!(foes, [(WATER, 30), (FIRE, 20)]);
    assert_eq!(b.setup().environment, crate::battle::Environment::Cave);
    assert!(b.setup().rules.foe_back);
}

#[test]
fn a_trainer_with_rules_battles_under_them_unless_the_request_names_its_own() {
    for (who, inverse) in [(RULES_TRAINER, true), (TRAINER, false)] {
        let mut g = G::new();
        start(&mut g.w, move |ctx| {
            Box::pin(async move {
                ctx.give_mon(WATER, 30).await;
                ctx.battle_trainer(who, TextId(0)).await;
            })
        });
        g.mash(20_000, |w| w.in_battle());
        let (_, b, _) = g.w.battle().unwrap();
        assert_eq!(b.setup().rules.inverse, inverse, "{who:?}");
    }
}
