//! Wave C: the remaining Gen3 abilities and held items, on the synthetic tables. Same format
//! as [`super::tests`]; each test cites the decomp function or script it follows.

use super::tests::{contact_hit, draws, foe, has, me, nth, seed_where, wild};
use crate::battle::calc;
use crate::battle::core::tests::{Run, lines};
use crate::battle::core::{Action, Battle, Env, Weather};
use crate::battle::test_data::*;
use crate::battle::{Choice, Effectiveness, Side};
use crate::data::{ItemId, MoveId};
use crate::mon::{Ability, HoldEffect, Pokemon, Stat, Status, Type};
use crate::rng::Rng;
use alloc::string::String;
use alloc::vec::Vec;

const P: Side = Side::Player;
const O: Side = Side::Opponent;
const ITEM: ItemId = ItemId(77);

fn run_env<T>(
    r: &mut Run,
    seed: u32,
    f: impl FnOnce(&mut Battle, &mut Env) -> T,
) -> (T, Rng, Vec<String>) {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    let mut env = Env::new(&TABLES, &mut rng, &mut out);
    let t = f(&mut r.b, &mut env);
    (t, rng, lines(&out))
}

fn use_slot0(r: &mut Run, seed: u32, side: Side) -> Vec<String> {
    run_env(r, seed, |b, env| b.use_move(env, side, Action::Move(0))).2
}

/// `side`'s battler gets `ability`.
fn with(r: &mut Run, side: Side, ability: Ability) {
    r.b.battler_mut(side).ability = ability;
}

/// `side`'s battler (party slot 0) holds `ITEM` with `hold`.
fn hold(r: &mut Run, side: Side, hold: HoldEffect) {
    let m = r.b.battler_mut(side);
    m.x.item = Some(ITEM);
    m.x.hold = Some(hold);
    r.b.x.holds[side.s()][0] = Some(hold);
}

/// `moveend` (abilities, then items) after `side` hit with `mov` (5 HP dealt).
fn move_end(r: &mut Run, seed: u32, side: Side, mov: MoveId) -> (Rng, Vec<String>) {
    let ((), rng, l) = run_env(r, seed, |b, env| {
        let u = b.move_use(env, side, mov, Some(0));
        super::after_move(b, env, &u, contact_hit());
    });
    (rng, l)
}

// ---- status immunities ----

#[test]
fn status_moves_name_the_preventing_ability() {
    // BattleScript_EffectPoison / Paralyze: `jumpifability` first (before the accuracy roll).
    for (mov, ability, id) in [
        (
            POISON_POWDER,
            Ability::Immunity,
            "PkmnPreventsPoisoningWith",
        ),
        (TOXIC, Ability::Immunity, "PkmnPreventsPoisoningWith"),
        (STUN_SPORE, Ability::Limber, "PkmnPreventsParalysisWith"),
        (SUPERSONIC, Ability::OwnTempo, "PkmnPreventsConfusionWith"),
    ] {
        let mut r = wild(me(WATER, &[mov]), foe(NORMAL, &[SPLASH]), 1);
        with(&mut r, O, ability);
        let l = use_slot0(&mut r, 1, P);
        assert_eq!(
            l[1],
            format!("{id}@O AbilityName({ability:?}) ~64"),
            "{l:?}"
        );
        assert_eq!(l.len(), 2, "{l:?}");
        assert_eq!(r.b.battler(O).mon.status, Status::None);
        assert_eq!(r.b.battler(O).confusion, 0);
    }
}

#[test]
fn secondary_statuses_are_blocked_silently() {
    // SetMoveEffect: Immunity without `primary` / `certain` breaks out; Own Tempo too.
    for (mov, ability) in [
        (POISON_ALL, Ability::Immunity),
        (CONFUSE_ALL, Ability::OwnTempo),
    ] {
        let mut r = wild(me(WATER, &[mov]), foe(NORMAL, &[SPLASH]), 1);
        with(&mut r, O, ability);
        let l = use_slot0(&mut r, 3, P);
        assert!(!l.iter().any(|x| x.contains("Prevents")), "{l:?}");
        assert_eq!(r.b.battler(O).mon.status, Status::None);
        assert_eq!(r.b.battler(O).confusion, 0);
    }
}

#[test]
fn magma_armor_and_the_sun_prevent_freezing() {
    for (ability, weather) in [
        (Ability::MagmaArmor, Weather::None),
        (Ability::Other, Weather::Sun),
    ] {
        let mut r = wild(me(WATER, &[FREEZE_HIT]), foe(NORMAL, &[SPLASH]), 1);
        with(&mut r, O, ability);
        r.b.x.weather = weather;
        use_slot0(&mut r, 3, P);
        assert_eq!(r.b.battler(O).mon.status, Status::None);
    }
    // Control: the same hit freezes.
    let mut r = wild(me(WATER, &[FREEZE_HIT]), foe(NORMAL, &[SPLASH]), 1);
    use_slot0(&mut r, 3, P);
    assert_eq!(r.b.battler(O).mon.status, Status::Freeze);
}

#[test]
fn immunity_abilities_cure_at_move_end() {
    // MOVEEND_IMMUNITY_ABILITIES: BattleScript_AbilityCuredStatus, then the icon.
    for (ability, status) in [
        (Ability::Immunity, Status::Toxic(2)),
        (Ability::Limber, Status::Paralysis),
        (Ability::WaterVeil, Status::Burn),
        (Ability::MagmaArmor, Status::Freeze),
        (Ability::Insomnia, Status::Sleep(3)),
    ] {
        let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
        with(&mut r, O, ability);
        r.b.battler_mut(O).mon.status = status;
        let (_, l) = move_end(&mut r, 1, P, SPLASH);
        assert!(
            has(
                &l,
                &format!("PkmnSXCuredItsYProblem@O AbilityName({ability:?}) ~64")
            ),
            "{l:?}"
        );
        assert_eq!(r.b.battler(O).mon.status, Status::None);
    }
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::OwnTempo);
    r.b.battler_mut(O).confusion = 3;
    move_end(&mut r, 1, P, SPLASH);
    assert_eq!(r.b.battler(O).confusion, 0);
}

// ---- contact abilities ----

#[test]
fn flame_body_burns_a_contact_attacker_one_time_in_three() {
    for proc in [true, false] {
        let seed = seed_where(|s| nth(s, 0).is_multiple_of(3) == proc);
        let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
        with(&mut r, O, Ability::FlameBody);
        let (rng, _) = move_end(&mut r, seed, P, TACKLE);
        assert_eq!(rng, draws(seed, 1));
        let burned = r.b.battler(P).mon.status == Status::Burn;
        assert_eq!(burned, proc);
    }
}

#[test]
fn rough_skin_hurts_a_contact_attacker() {
    let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::RoughSkin);
    let (rng, l) = move_end(&mut r, 1, P, TACKLE);
    assert_eq!(rng, draws(1, 0));
    // BattleScript_RoughSkinActivates: HP (20 / 16 = 1), then the message.
    let hp = l.iter().position(|x| x == "Hp P 20->19").expect("hp");
    assert_eq!(l[hp + 1], "PkmnHurtsWith@O AbilityName(RoughSkin) ~64");
}

#[test]
fn color_change_takes_the_moves_type() {
    let mut r = wild(me(WATER, &[TACKLE]), foe(WATER, &[SPLASH]), 1);
    with(&mut r, O, Ability::ColorChange);
    let (_, l) = move_end(&mut r, 1, P, TACKLE);
    assert_eq!(r.b.battler(O).types, [Type::Normal, Type::Normal]);
    assert_eq!(
        l[0],
        "PkmnChangedTypeWith@O Type(Normal) AbilityName(ColorChange) ~64"
    );
    // Already of that type: nothing.
    let (_, l) = move_end(&mut r, 1, P, TACKLE);
    assert!(l.is_empty(), "{l:?}");
}

// ---- absorbing abilities, Wonder Guard ----

#[test]
fn volt_absorb_heals_a_quarter_or_makes_the_move_useless() {
    let mut r = wild(me(WATER, &[THUNDER_SHOCK]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::VoltAbsorb);
    r.b.battler_mut(O).mon.hp = 10;
    let seed = seed_where(|s| nth(s, 0) % 100 < 100);
    let l = use_slot0(&mut r, seed, P);
    assert!(has(&l, "Hp O 10->15"), "{l:?}");
    assert!(
        has(&l, "PkmnRestoredHpUsing@O AbilityName(VoltAbsorb) ~64"),
        "{l:?}"
    );
    assert!(!l.iter().any(|x| x.starts_with("Hit")), "{l:?}");
    r.b.battler_mut(O).mon.hp = 20;
    let l = use_slot0(&mut r, seed, P);
    assert!(
        has(&l, "PkmnSXMadeYUseless@O AbilityName(VoltAbsorb) ~64"),
        "{l:?}"
    );
}

#[test]
fn flash_fire_takes_fire_moves_and_boosts_its_own() {
    let mut r = wild(me(WATER, &[EMBER]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::FlashFire);
    let l = use_slot0(&mut r, 1, P);
    assert!(
        has(&l, "PkmnRaisedFirePowerWith@O AbilityName(FlashFire) ~64"),
        "{l:?}"
    );
    assert!(r.b.battler(O).x.flash_fire);
    assert_eq!(r.b.battler(O).mon.hp, 20);
    let l = use_slot0(&mut r, 1, P);
    assert!(
        has(&l, "PkmnSXMadeYIneffective@O AbilityName(FlashFire) ~64"),
        "{l:?}"
    );
    // Frozen: it takes the hit.
    let mut r = wild(me(WATER, &[EMBER]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::FlashFire);
    r.b.battler_mut(O).mon.status = Status::Freeze;
    use_slot0(&mut r, 1, P);
    assert!(!r.b.battler(O).x.flash_fire);
}

/// L50, 100 HP, 50 in every other stat.
fn fighter(types: [Type; 2]) -> calc::Fighter {
    calc::Fighter {
        level: 50,
        hp: 100,
        max_hp: 100,
        stats: [100, 50, 50, 50, 50, 50],
        stages: [0; 8],
        types,
        ability: Ability::Other,
        status: Status::None,
        hold: None,
    }
}

#[test]
fn flash_fire_boost_is_half_more_fire_damage() {
    let a = fighter([Type::Fire, Type::Fire]);
    let d = fighter([Type::Normal, Type::Normal]);
    let base = |flash_fire| {
        calc::base_damage_x(
            &a,
            &d,
            60,
            Type::Fire,
            false,
            calc::Field::default(),
            calc::Extra {
                flash_fire,
                ..Default::default()
            },
        )
    };
    let (off, on) = (base(false), base(true));
    assert_eq!(on - 2, (off - 2) * 15 / 10);
}

#[test]
fn wonder_guard_avoids_moves_that_are_not_super_effective() {
    let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::WonderGuard);
    let seed = seed_where(|s| nth(s, 0) % 100 < 95);
    let l = use_slot0(&mut r, seed, P);
    assert!(
        has(&l, "AvoidedDamage@O AbilityName(WonderGuard) ~64"),
        "{l:?}"
    );
    assert_eq!(r.b.battler(O).mon.hp, 20);
    assert!(!l.iter().any(|x| x.starts_with("Anim")), "{l:?}");
}

// ---- Pressure, weather, Plus / Minus, Forecast ----

#[test]
fn pressure_costs_one_more_pp() {
    let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::Pressure);
    let pp = r.b.battler(P).mon.pp[0];
    use_slot0(&mut r, 1, P);
    assert_eq!(r.b.battler(P).mon.pp[0], pp - 2);
}

#[test]
fn cloud_nine_suppresses_the_weather() {
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    r.b.x.weather = Weather::Sand;
    assert_eq!(r.b.weather(), Weather::Sand);
    with(&mut r, O, Ability::CloudNine);
    assert_eq!(r.b.weather(), Weather::None);
    // Cmd_weatherdamage: the sandstorm rages, but hurts nobody.
    let (_, _, l) = run_env(&mut r, 1, super::later::field_end_turn);
    assert_eq!(l, ["SandstormRages@P ~64"]);
}

#[test]
fn plus_needs_a_minus_on_the_field() {
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, P, Ability::Plus);
    assert!(!r.b.plus_minus(P));
    with(&mut r, O, Ability::Minus);
    assert!(r.b.plus_minus(P) && r.b.plus_minus(O));
    r.b.battler_mut(O).mon.hp = 0;
    assert!(!r.b.plus_minus(P));
}

#[test]
fn forecast_follows_the_weather() {
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::Forecast);
    r.b.x.weather = Weather::Rain;
    let (changed, _, l) = run_env(&mut r, 1, |b, env| super::abilities::forecast(b, env, O));
    assert!(changed);
    assert_eq!(r.b.battler(O).types, [Type::Water, Type::Water]);
    assert!(has(&l, "PkmnTransformed@O ~64"), "{l:?}");
    r.b.x.weather = Weather::None;
    run_env(&mut r, 1, |b, env| super::abilities::forecast(b, env, O));
    assert_eq!(r.b.battler(O).types, [Type::Normal, Type::Normal]);
}

// ---- escape, stat drops ----

#[test]
fn arena_trap_holds_grounded_battlers_and_smoke_ball_escapes() {
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::ArenaTrap);
    assert!(super::run_block(&r.b, P).is_some());
    assert!(super::switch_block(&r.b, P));
    with(&mut r, P, Ability::Levitate);
    assert!(super::run_block(&r.b, P).is_none());
    with(&mut r, P, Ability::Other);
    r.b.battler_mut(P).types = [Type::Flying, Type::Flying];
    assert!(super::run_block(&r.b, P).is_none());
    // IsRunningFromBattleImpossible: a Smoke Ball first; HandleEndTurn_RanFromBattle names it.
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::ArenaTrap);
    hold(&mut r, P, HoldEffect::CanAlwaysRun);
    assert!(super::run_block(&r.b, P).is_none());
    assert!(super::switch_block(&r.b, P));
    let l = r.choose(Choice::Run);
    assert!(has(&l, "FledUsingIts@P Item(ItemId(77)) !"), "{l:?}");
}

#[test]
fn hyper_cutter_and_white_smoke_keep_their_stats() {
    // ChangeStatBuffs: White Smoke as Clear Body; Hyper Cutter for attack only.
    let mut r = wild(me(WATER, &[GROWL]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::HyperCutter);
    let l = use_slot0(&mut r, 1, P);
    assert!(
        has(&l, "PkmnSXPreventsYLoss@O Atk AbilityName(HyperCutter) ~64"),
        "{l:?}"
    );
    assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], 0);
    let mut r = wild(me(WATER, &[SCREECH]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::HyperCutter);
    use_slot0(&mut r, 1, P);
    assert_eq!(r.b.battler(O).stages[Stat::Def as usize], -2);
    let mut r = wild(me(WATER, &[SCREECH]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::WhiteSmoke);
    let l = use_slot0(&mut r, 1, P);
    assert!(
        has(&l, "PkmnPreventsStatLossWith@O AbilityName(WhiteSmoke) ~64"),
        "{l:?}"
    );
    // BattleScript_IntimidatePrevented.
    for a in [Ability::HyperCutter, Ability::WhiteSmoke] {
        let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
        with(&mut r, P, Ability::Intimidate);
        with(&mut r, O, a);
        run_env(&mut r, 1, |b, env| super::abilities::intimidate(b, env, P));
        assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], 0);
    }
}

#[test]
fn serene_grace_doubles_the_secondary_chance() {
    // Cmd_seteffectwithchance: Ember's 10% becomes 20%; a sure hit draws crit, damage, then
    // the secondary roll.
    let seed = seed_where(|s| (10..20).contains(&(nth(s, 3) % 100)) && nth(s, 0) % 100 < 100);
    for (ability, burns) in [(Ability::Other, false), (Ability::SereneGrace, true)] {
        let mut r = wild(me(WATER, &[EMBER]), foe(NORMAL, &[SPLASH]), 1);
        with(&mut r, P, ability);
        use_slot0(&mut r, seed, P);
        assert_eq!(
            r.b.battler(O).mon.status == Status::Burn,
            burns,
            "{ability:?}"
        );
    }
}

// ---- held items ----

#[test]
fn pinch_berries_raise_a_stat_at_a_quarter_hp() {
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::StatUp(Stat::Atk, 4));
    r.b.battler_mut(P).mon.hp = 6;
    run_env(&mut r, 1, |b, env| super::items::end_turn(b, env, P, true));
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 0, "above 20 / 4");
    r.b.battler_mut(P).mon.hp = 5;
    let (_, _, l) = run_env(&mut r, 1, |b, env| super::items::end_turn(b, env, P, true));
    assert_eq!(
        l,
        [
            "StatAnim P true",
            "UsingItemStatRose@P Atk StatRose Item(ItemId(77)) ~64"
        ]
    );
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 1);
    assert_eq!(r.b.battler(P).x.item, None);
}

#[test]
fn starf_draws_a_stat_and_lansat_pumps() {
    let seed = 5;
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::RandomStatUp(4));
    r.b.battler_mut(P).mon.hp = 5;
    let (_, rng, _) = run_env(&mut r, seed, |b, env| {
        super::items::end_turn(b, env, P, true)
    });
    let stat = Stat::ALL[(nth(seed, 0) % 5) as usize + 1];
    assert_eq!(rng, draws(seed, 1));
    assert_eq!(r.b.battler(P).stages[stat as usize], 2);
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::CriticalUp(4));
    r.b.battler_mut(P).mon.hp = 5;
    let (_, _, l) = run_env(&mut r, 1, |b, env| super::items::end_turn(b, env, P, true));
    assert_eq!(l, ["UsedXToGetPumped@P Item(ItemId(77)) ~64"]);
    assert!(r.b.battler(P).focus_energy);
}

#[test]
fn flavour_berries_heal_and_confuse_a_disliking_nature() {
    // Nature 1 (Lonely): +Atk, -Def: dislikes sour (Iapapa, flavour 4).
    for (flavor, confused) in [(4, true), (0, false)] {
        let mut p = me(WATER, &[SPLASH]);
        p.personality = 1;
        let mut r = wild(p, foe(NORMAL, &[SPLASH]), 1);
        hold(&mut r, P, HoldEffect::ConfuseBerry { flavor, param: 8 });
        r.b.battler_mut(P).mon.hp = 10;
        let (_, _, l) = run_env(&mut r, 1, |b, env| super::items::end_turn(b, env, P, true));
        assert!(has(&l, "Hp P 10->12"), "{l:?}");
        assert_eq!(r.b.battler(P).confusion > 0, confused, "{l:?}");
        assert_eq!(has(&l, "ForXCommaYZ@P Item(ItemId(77)) ~64"), confused);
    }
}

#[test]
fn leppa_restores_the_first_empty_move() {
    let mut r = wild(me(WATER, &[TACKLE, GROWL]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::RestorePp(10));
    run_env(&mut r, 1, |b, env| super::items::end_turn(b, env, P, true));
    assert_eq!(r.b.battler(P).x.item, Some(ITEM), "no empty move");
    r.b.battler_mut(P).mon.pp[1] = 0;
    let (_, _, l) = run_env(&mut r, 1, |b, env| super::items::end_turn(b, env, P, true));
    assert_eq!(l, ["ItemRestoredPp@P move:2 Item(ItemId(77)) ~64"]);
    assert_eq!(r.b.battler(P).mon.pp[1], 10);
}

#[test]
fn herbs_restore_stats_and_cure_love() {
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::RestoreStats);
    r.b.battler_mut(P).stages[Stat::Def as usize] = -2;
    r.b.battler_mut(P).stages[Stat::Atk as usize] = 1;
    let (_, l) = move_end(&mut r, 1, O, SPLASH);
    assert!(
        has(&l, "ItemRestoredStatus@P Item(ItemId(77)) ~64"),
        "{l:?}"
    );
    assert_eq!(r.b.battler(P).stages[Stat::Def as usize], 0);
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 1);
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::CureAttract);
    r.b.battler_mut(P).v.infatuated = true;
    let (_, _, l) = run_env(&mut r, 1, |b, env| super::items::end_turn(b, env, P, false));
    assert!(has(&l, "ItemCuredProblem@P Item(ItemId(77)) ~64"), "{l:?}");
    assert!(!r.b.battler(P).v.infatuated);
}

#[test]
fn shell_bell_returns_an_eighth_of_the_damage() {
    let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::ShellBell(8));
    r.b.battler_mut(P).mon.hp = 10;
    let (_, l) = move_end(&mut r, 1, P, TACKLE);
    // 5 dealt / 8 = 0 → 1.
    assert!(
        has(&l, "ItemRestoredHpALittle@P Item(ItemId(77)) ~64"),
        "{l:?}"
    );
    assert!(has(&l, "Hp P 10->11"), "{l:?}");
    assert_eq!(r.b.battler(P).x.item, Some(ITEM));
}

#[test]
fn amulet_coin_doubles_the_prize_and_white_herb_acts_on_entry() {
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::DoublePrize);
    run_env(&mut r, 1, |b, env| super::items_on_switch_in(b, env, P));
    assert!(r.b.x.double_prize);
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, O, HoldEffect::DoublePrize);
    run_env(&mut r, 1, |b, env| super::items_on_switch_in(b, env, O));
    assert!(!r.b.x.double_prize, "the foe's coin does nothing");
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::RestoreStats);
    r.b.battler_mut(P).stages[Stat::Speed as usize] = -1;
    run_env(&mut r, 1, |b, env| super::items_on_switch_in(b, env, P));
    assert_eq!(r.b.battler(P).stages[Stat::Speed as usize], 0);
}

#[test]
fn species_items_and_crit_items_apply_to_their_species() {
    let s = WATER;
    assert_eq!(
        HoldEffect::LightBall(s).for_species(s),
        Some(HoldEffect::LightBall(s))
    );
    assert_eq!(HoldEffect::LightBall(s).for_species(NORMAL), None);
    assert_eq!(
        HoldEffect::ThickClub(NORMAL, s).for_species(s),
        Some(HoldEffect::ThickClub(NORMAL, s))
    );
    assert_eq!(
        HoldEffect::ScopeLens.for_species(s),
        Some(HoldEffect::ScopeLens)
    );
    assert_eq!(calc::crit_item_stage(Some(HoldEffect::ScopeLens)), 1);
    assert_eq!(calc::crit_item_stage(Some(HoldEffect::SpeciesCrit(s))), 2);
    // Light Ball: x2 special attack.
    let (a, d) = (fighter([Type::Normal; 2]), fighter([Type::Normal; 2]));
    let dmg = |hold| {
        let a = calc::Fighter { hold, ..a };
        calc::base_damage_x(
            &a,
            &d,
            60,
            Type::Water,
            false,
            calc::Field::default(),
            calc::Extra::default(),
        )
    };
    assert!(dmg(Some(HoldEffect::LightBall(s))) > dmg(None));
    // The battler's fighter drops another species' item.
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    hold(&mut r, P, HoldEffect::LightBall(NORMAL));
    assert_eq!(r.b.battler(P).fighter().hold, None);
}

#[test]
fn macho_brace_halves_speed() {
    let f = fighter([Type::Normal; 2]);
    let slow = calc::Fighter {
        hold: Some(HoldEffect::MachoBrace),
        ..f
    };
    assert_eq!(
        calc::effective_speed_x(&slow, 1),
        calc::effective_speed_x(&f, 1) / 2
    );
}

#[test]
fn wonder_guard_spares_super_effective_moves() {
    // Cmd_typecalc: only a non-super-effective move "misses".
    let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    with(&mut r, O, Ability::WonderGuard);
    let ((), _, l) = run_env(&mut r, 1, |b, env| {
        let u = b.move_use(env, P, TACKLE, Some(0));
        let r = super::wonder_guard(b, env, &u, Effectiveness::Normal);
        assert!(r);
        let r = super::wonder_guard(b, env, &u, Effectiveness::Super);
        assert!(!r);
    });
    assert_eq!(l.len(), 1);
}

#[test]
fn macho_brace_doubles_ev_gains() {
    let mut p: Pokemon = me(WATER, &[SPLASH]);
    let mut q = p;
    p.gain_evs_x(&TABLES, NORMAL, false);
    q.gain_evs_x(&TABLES, NORMAL, true);
    for i in 0..6 {
        assert_eq!(q.evs[i] as u16, (p.evs[i] as u16 * 2).min(255));
    }
}

#[test]
fn exp_share_splits_the_exp_and_lucky_egg_adds_half() {
    // Cmd_getexp: with an Exp. Share in the party, half to the participants, half to the
    // holders (a holder that never came in still gets it); a Lucky Egg x150/100 per mon.
    let exp_of = |l: &[String], party: u8| -> Option<u32> {
        let tag = format!("PkmnGainedExp@P mon:P{party} ");
        let x = l.iter().find(|x| x.starts_with(&tag))?;
        x[tag.len()..].split(' ').next()?.parse().ok()
    };
    let run = |slot: usize, item: HoldEffect| {
        let mut r = Run::wild(
            &[me(WATER, &[TACKLE]), me(WATER, &[TACKLE])],
            foe(NORMAL, &[SPLASH]),
            1,
        );
        r.b.x.holds[0][slot] = Some(item);
        r.b.battler_mut(O).mon.hp = 1;
        r.rng = Rng::new(seed_where(|s| nth(s, 0) % 100 < 95));
        r.choose(Choice::Move(0))
    };
    let calculated = TABLES.species_info(NORMAL).exp_yield as u32 * 5 / 7;
    let l = run(1, HoldEffect::ExpShare);
    let half = (calculated / 2).max(1);
    assert_eq!(
        (exp_of(&l, 0), exp_of(&l, 1)),
        (Some(half), Some(half)),
        "{l:?}"
    );
    let l = run(0, HoldEffect::LuckyEgg);
    assert_eq!(exp_of(&l, 0), Some(calculated.max(1) * 150 / 100), "{l:?}");
    assert_eq!(exp_of(&l, 1), None, "{l:?}");
}
