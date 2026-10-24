//! SIL6 (the Silice post-game): the clock and the daily flags (Gen3 `DoTimeBasedEvents` →
//! `UpdatePerDay` → `ClearDailyFlags`), and wild battles under per-battle rules (a boss that
//! cannot be caught).

use super::*;
use crate::battle::BattleRules;

const VAR_DAYS: VarId = VarId(0x4060);
const DAILY_A: FlagId = FlagId(0x0F10);
const DAILY_B: FlagId = FlagId(0x0F11);
const NOT_DAILY: FlagId = FlagId(0x0F12);
static DAILY: [FlagId; 2] = [DAILY_A, DAILY_B];

/// [`Game`] with a day var and two daily flags.
struct ClockGame;

impl GameData for ClockGame {
    fn map(&self, id: MapId) -> &'static MapData {
        Game.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            var_days: Some(VAR_DAYS),
            daily_flags: &DAILY,
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
        Game.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
}

fn step(w: &mut World, frames: u32) {
    for _ in 0..frames {
        w.step(&ClockGame, Buttons::NONE).validate().unwrap();
    }
}

fn set_daily(w: &mut World) {
    for f in [DAILY_A, DAILY_B, NOT_DAILY] {
        w.s.vars.set_flag(f, true);
    }
}

/// A map load's `ClearTempFieldEventData` (with the daily reset) on the clock's current day.
fn load(w: &mut World) {
    step(w, 1);
    w.s.clear_temp(&ClockGame.special_ids());
}

#[test]
fn the_deterministic_clock_counts_days_in_frames() {
    let mut w = World::new(&ClockGame);
    assert_eq!(w.days(), 0);
    w.frame = FRAMES_PER_DAY - 1;
    assert_eq!(w.days(), 0);
    w.frame = FRAMES_PER_DAY * 3;
    assert_eq!(w.days(), 3);
    // The hardware clock wins while the shell sets it.
    w.set_rtc_days(Some(40));
    assert_eq!(w.days(), 40);
    w.set_rtc_days(None);
    assert_eq!(w.days(), 3);
}

#[test]
fn the_daily_flags_clear_on_the_first_map_load_of_a_new_day_only() {
    let mut w = World::new(&ClockGame);
    step(&mut w, 2);
    set_daily(&mut w);
    // The same day: nothing changes.
    load(&mut w);
    assert!(w.flag(DAILY_A) && w.flag(DAILY_B) && w.flag(NOT_DAILY));
    // The next day, the clock alone changes nothing until a map load.
    w.set_rtc_days(Some(1));
    step(&mut w, 1);
    assert!(w.flag(DAILY_A));
    load(&mut w);
    assert!(!w.flag(DAILY_A) && !w.flag(DAILY_B));
    assert!(w.flag(NOT_DAILY), "only the daily flags");
    assert_eq!(w.var(VAR_DAYS), 1);
    // Set again the same day: kept.
    set_daily(&mut w);
    load(&mut w);
    assert!(w.flag(DAILY_A));
    // A clock set back changes nothing (Gen3 `*days <= localTime->days`).
    w.set_rtc_days(Some(0));
    load(&mut w);
    assert!(w.flag(DAILY_A));
    assert_eq!(w.var(VAR_DAYS), 1);
    // Several days later: one reset.
    w.set_rtc_days(Some(9));
    load(&mut w);
    assert!(!w.flag(DAILY_A));
    assert_eq!(w.var(VAR_DAYS), 9);
}

#[test]
fn a_game_without_a_day_var_keeps_its_flags() {
    let mut w = World::new(&Game);
    w.s.vars.set_flag(DAILY_A, true);
    w.set_rtc_days(Some(5));
    w.step(&Game, Buttons::NONE);
    w.s.clear_temp(&Game.special_ids());
    assert!(w.flag(DAILY_A));
}

static BOSS: BattleRules = BattleRules {
    inverse: false,
    field: None,
    background: None,
    foe_back: false,
    clone_player: false,
    intro: None,
    no_catch: true,
    weather: None,
};

#[test]
fn a_wild_battle_with_rules_hashes_apart_from_one_without() {
    use crate::script::Request;
    let h = |r: Request| {
        let mut f = crate::hash::Fnv64::new();
        r.hash_into(&mut f);
        f.finish()
    };
    let plain = Request::BattleWild {
        species: SpeciesId(1),
        level: 80,
        first_battle: false,
        rules: None,
    };
    let boss = Request::BattleWild {
        species: SpeciesId(1),
        level: 80,
        first_battle: false,
        rules: Some(&BOSS),
    };
    assert_ne!(h(plain), h(boss));
    assert_ne!(BOSS.digest(), BattleRules::default().digest());
}

#[test]
fn a_scripted_wild_battle_takes_its_rules_into_the_battle() {
    let mut w = World::new(&ClockGame);
    step(&mut w, 16);
    let _ = w.s.party.push(crate::battle::core::tests::water(20));
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.battle_wild_rules(crate::battle::test_data::NORMAL, 5, &BOSS)
                .await;
        })
    });
    for _ in 0..200 {
        step(&mut w, 1);
        if let Mode::Battle(run) = &w.s.mode {
            assert!(run.battle.setup.rules.no_catch);
            return;
        }
    }
    panic!("no battle started");
}
