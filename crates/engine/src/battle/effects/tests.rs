//! Effect, status, ability and catch tests. Event lists use the core tests' compact format
//! (`core::tests::fmt`); seeds are pinned. Hand goldens cite the decomp function and inputs.

use crate::battle::core::Env;
use crate::battle::core::tests::{Run, first_mover};
use crate::battle::test_data::*;
use crate::battle::{BattleOutcome, Choice, Need, Side};
use crate::data::MoveId;
use crate::mon::{Ability, Pokemon, Stat, Status};
use crate::rng::Rng;
use alloc::string::String;
use alloc::vec::Vec;

const P: Side = Side::Player;
const O: Side = Side::Opponent;

/// L5, 20 HP, Atk 13, Def 10, Speed 9 (fast enough to move first against the 5-speed foes).
fn me(species: crate::data::SpeciesId, moves: &[MoveId]) -> Pokemon {
    mon(species, 5, [20, 13, 10, 9, 9, 10], moves)
}

/// L5, 20 HP, Atk 10, Def 10, Speed 5.
fn foe(species: crate::data::SpeciesId, moves: &[MoveId]) -> Pokemon {
    mon(species, 5, [20, 10, 10, 5, 10, 10], moves)
}

fn wild(p: Pokemon, o: Pokemon, seed: u32) -> Run {
    Run::wild(&[p], o, seed)
}

/// The first seed in `1..` whose run satisfies `ok` (a search over pinned seeds, so a test
/// documents the situation rather than a magic number).
fn seed_where(mut ok: impl FnMut(u32) -> bool) -> u32 {
    (1..2000).find(|&s| ok(s)).expect("no seed")
}

fn has(lines: &[String], l: &str) -> bool {
    lines.iter().any(|x| x == l)
}

// ---- secondary effects ----

/// `seteffectwithchance`: `Random() % 100 < secondaryEffectChance`. With a sure hit the rolls
/// are crit, damage, then the secondary roll: a third roll of 9 burns with Ember (10%), 10
/// does not.
#[test]
fn secondary_chance_boundaries() {
    let third = |seed: u32| {
        let mut r = Rng::new(seed);
        r.random();
        r.random();
        r.random() % 100
    };
    for (roll, burned) in [(9, true), (10, false)] {
        let seed = seed_where(|s| third(s) == roll);
        let mut r = wild(me(FIRE, &[EMBER]), foe(NORMAL, &[SPLASH]), 1);
        let mut rng = Rng::new(seed);
        let mut out = Vec::new();
        let mut env = Env::new(&TABLES, &mut rng, &mut out);
        let u = r.b.move_use(&env, P, EMBER, Some(0));
        let hit = r.b.hit(
            &mut env,
            &u,
            super::moves::with(super::moves::status_hit),
            true,
        );
        assert!(hit.is_some());
        assert_eq!(r.b.mon(O).status == Status::Burn, burned, "roll {roll}");
        r.rng = Rng::new(0);
    }
}

#[test]
fn poison_sting_and_shield_dust() {
    // POISON_ALL: chance 100 (the roll is still drawn). Shield Dust blocks it silently.
    // 15 power Poison (physical): 13×15×4/10/50 + 2 = 3, roll → 2.
    let mut r = wild(me(WATER, &[POISON_ALL]), foe(NORMAL, &[SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l[..6],
        [
            "UsedMove@P move:40",
            "Anim P move:40",
            "Hit O Normal",
            "Hp O 20->18",
            "PkmnWasPoisoned@O ~64",
            "StatusIcon { side: Opponent, status: Poison }",
        ]
    );
    let mut r = wild(me(WATER, &[POISON_ALL]), foe(BUG, &[SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(!l.iter().any(|x| x.starts_with("PkmnWasPoisoned")), "{l:?}");
    assert_eq!(r.b.mon(O).status, Status::None);
    // A Poison type is not poisoned (SetMoveEffect), silently.
    let mut r = wild(me(WATER, &[POISON_ALL]), foe(POISON, &[SPLASH]), 3);
    r.choose(Choice::Move(0));
    assert_eq!(r.b.mon(O).status, Status::None);
}

#[test]
fn mud_slap_keen_eye_and_shield_dust() {
    let mut r = wild(me(WATER, &[MUD_SLAP]), foe(NORMAL, &[SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "StatAnim O false"), "{l:?}");
    assert!(has(&l, "DefenderStatFell@O Accuracy StatFell ~64"), "{l:?}");
    assert_eq!(r.b.battler(O).stages[Stat::Accuracy as usize], -1);
    // Secondary drops: Keen Eye (accuracy) and Shield Dust block without a message.
    for sp in [BIRD, BUG] {
        let mut r = wild(me(WATER, &[MUD_SLAP]), foe(sp, &[SPLASH]), 3);
        let l = r.choose(Choice::Move(0));
        assert!(!l.iter().any(|x| x.contains("Stat")), "{l:?}");
        assert_eq!(r.b.battler(O).stages[Stat::Accuracy as usize], 0);
    }
    // Sand-Attack (a status move) names Keen Eye: STRINGID_PKMNSXPREVENTSYLOSS.
    let mut r = wild(me(WATER, &[SAND_ATTACK]), foe(BIRD, &[SPLASH]), 3);
    assert_eq!(
        r.choose(Choice::Move(0))[..2],
        [
            "UsedMove@P move:17",
            "PkmnSXPreventsYLoss@O Accuracy AbilityName(KeenEye) ~64",
        ]
    );
}

#[test]
fn flinch_only_before_the_target_moves() {
    // Faster attacker: the foe flinches (and the flag clears at the end of the turn).
    let mut r = wild(me(WATER, &[FLINCH_ALL]), foe(NORMAL, &[TACKLE]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnFlinched@O ~64"), "{l:?}");
    assert!(!r.b.battler(O).flinched);
    // Slower attacker: the foe already moved, no flinch.
    let slow = mon(WATER, 5, [20, 13, 10, 1, 9, 10], &[FLINCH_ALL]);
    let mut r = wild(slow, foe(NORMAL, &[TACKLE]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(!has(&l, "PkmnFlinched@O ~64"), "{l:?}");
}

// ---- status moves ----

#[test]
fn status_moves_already_and_immune() {
    let statused = |s: Status, sp| {
        let mut f = foe(sp, &[SPLASH]);
        f.status = s;
        f
    };
    // "Already" checks come before accuracy (no roll), after the attack string.
    let cases: [(MoveId, Pokemon, &str); 6] = [
        (
            SLEEP_POWDER,
            statused(Status::Sleep(3), NORMAL),
            "PkmnAlreadyAsleep@O ~64",
        ),
        (
            POISON_POWDER,
            statused(Status::Toxic(1), NORMAL),
            "PkmnAlreadyPoisoned@O ~64",
        ),
        (
            POISON_POWDER,
            statused(Status::None, STEEL),
            "ItDoesntAffect@O ~64",
        ),
        (
            STUN_SPORE,
            statused(Status::Paralysis, NORMAL),
            "PkmnIsAlreadyParalyzed@O ~64",
        ),
        (
            STUN_SPORE,
            statused(Status::Burn, NORMAL),
            "ButItFailed@P ~64",
        ),
        (TOXIC, statused(Status::Burn, STEEL), "ButItFailed@P ~64"),
    ];
    for (mv, f, want) in cases {
        let mut r = wild(me(WATER, &[mv]), f, 3);
        let l = r.choose(Choice::Move(0));
        assert_eq!(
            l[..2],
            [alloc::format!("UsedMove@P move:{}", mv.0), want.into()]
        );
    }
}

#[test]
fn sleep_powder_counter_and_wake_up() {
    // Find a seed where Sleep Powder hits; the turns are `(Random() & 3) + 2`.
    let seed = seed_where(|s| {
        let mut r = wild(me(WATER, &[SLEEP_POWDER]), foe(NORMAL, &[SPLASH]), s);
        r.choose(Choice::Move(0));
        r.b.mon(O).status == Status::Sleep(1)
    });
    let mut r = wild(me(WATER, &[SLEEP_POWDER]), foe(NORMAL, &[TACKLE]), seed);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l,
        [
            "UsedMove@P move:27",
            "Anim P move:27",
            "PkmnFellAsleep@O ~64",
            "StatusIcon { side: Opponent, status: Sleep(2) }",
            "PkmnFastAsleep@O ~64",
        ]
    );
    // CANCELLER_ASLEEP decrements before checking: 2 → 1, still asleep.
    assert_eq!(r.b.mon(O).status, Status::Sleep(1));
    // Second try: the counter reaches 0, it wakes up and moves.
    let l = r.choose(Choice::Move(0));
    let woke = l
        .iter()
        .position(|x| x == "PkmnWokeUp@O ~64")
        .expect("woke");
    assert_eq!(l[woke + 1], "StatusIcon { side: Opponent, status: None }");
    assert_eq!(l[woke + 2], "UsedMove@O move:1");
}

#[test]
fn confusion_self_hit_damage() {
    // AtkCanceler: confusion turns drop first; `Random() & 1 == 0` hits itself with
    // CalculateBaseDamage(attacker, attacker, 40 power, Normal): Atk 13, Def 10, L5:
    // 13×40×(2×5/5+2)/10/50 = 4, +2 = 6, then the damage roll ×(85..=100)/100 → 5 or 6.
    let mut found = [false; 2];
    for seed in 1..60 {
        let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), seed);
        r.b.battler_mut(P).confusion = 3;
        let l = r.choose(Choice::Move(0));
        if !has(&l, "PkmnHurtItself@P ~64") {
            continue;
        }
        assert_eq!(
            l[..3],
            [
                "PkmnIsConfused@P ~64",
                "PkmnHurtItself@P ~64",
                "Hit P Normal"
            ]
        );
        let hp = r.b.mon(P).hp;
        assert!(hp == 14 || hp == 15, "{hp}");
        found[(hp - 14) as usize] = true;
        assert_eq!(r.b.battler(P).confusion, 2);
    }
    assert_eq!(found, [true, true]);
}

#[test]
fn supersonic_confuses_once() {
    let seed = seed_where(|s| {
        let mut r = wild(me(WATER, &[SUPERSONIC]), foe(NORMAL, &[SPLASH]), s);
        r.choose(Choice::Move(0));
        r.b.battler(O).confusion > 0
    });
    let mut r = wild(me(WATER, &[SUPERSONIC]), foe(NORMAL, &[SPLASH]), seed);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l[..3],
        [
            "UsedMove@P move:20",
            "Anim P move:20",
            "PkmnBecameConfused@O ~64"
        ]
    );
    // `Random() % 4 + 2` turns, minus the one the foe just spent.
    assert!((1..=4).contains(&r.b.battler(O).confusion));
    let l = r.choose(Choice::Move(0));
    assert_eq!(l[..2], ["UsedMove@P move:20", "PkmnAlreadyConfused@O ~64"]);
}

/// `ENDTURN_POISON` / `ENDTURN_BURN`: max(1, maxHP / 8); `ENDTURN_BAD_POISON`: max(1, maxHP /
/// 16) × the counter, raised first.
#[test]
fn residual_damage_is_an_eighth_minimum_one() {
    for (status, max_hp, lost, msg) in [
        (Status::Poison, 7, 1, "PkmnHurtByPoison@P ~64"),
        (Status::Burn, 20, 2, "PkmnHurtByBurn@P ~64"),
        (Status::Poison, 40, 5, "PkmnHurtByPoison@P ~64"),
        (Status::Toxic(0), 40, 2, "PkmnHurtByPoison@P ~64"),
    ] {
        let mut m = mon(WATER, 5, [max_hp, 13, 10, 9, 9, 10], &[REAL_SPLASH]);
        m.status = status;
        let mut r = wild(m, foe(NORMAL, &[REAL_SPLASH]), 3);
        let l = r.choose(Choice::Move(0));
        let at = l.iter().position(|x| x == msg).expect(msg);
        assert_eq!(
            l[at + 1],
            alloc::format!("Hp P {max_hp}->{}", max_hp - lost)
        );
    }
    // Toxic: 2, then 4.
    let mut m = mon(WATER, 5, [40, 13, 10, 9, 9, 10], &[REAL_SPLASH]);
    m.status = Status::Toxic(0);
    let mut r = wild(m, foe(NORMAL, &[REAL_SPLASH]), 3);
    r.choose(Choice::Move(0));
    r.choose(Choice::Move(0));
    assert_eq!(r.b.mon(P).hp, 40 - 2 - 4);
    assert_eq!(r.b.mon(P).status, Status::Toxic(2));
}

#[test]
fn paralysis_and_freeze_cancelers() {
    // CANCELLER_PARALYSED: `Random() % 4 == 0`; over seeds both outcomes occur.
    let mut seen = [false; 2];
    for seed in 1..40 {
        let mut m = me(WATER, &[TACKLE]);
        m.status = Status::Paralysis;
        let mut r = wild(m, foe(NORMAL, &[REAL_SPLASH]), seed);
        let l = r.choose(Choice::Move(0));
        seen[has(&l, "PkmnIsParalyzed@P ~64") as usize] = true;
    }
    assert_eq!(seen, [true, true]);
    // CANCELLER_FROZEN: thaws when `Random() % 5 == 0`.
    let mut seen = [false; 2];
    for seed in 1..40 {
        let mut m = me(WATER, &[TACKLE]);
        m.status = Status::Freeze;
        let mut r = wild(m, foe(NORMAL, &[REAL_SPLASH]), seed);
        let l = r.choose(Choice::Move(0));
        let thawed = has(&l, "PkmnWasDefrosted2@P ~64");
        assert_eq!(thawed, !has(&l, "PkmnIsFrozen@P ~64"));
        seen[thawed as usize] = true;
    }
    assert_eq!(seen, [true, true]);
}

// ---- other effects ----

#[test]
fn absorb_drains_half_the_hp_dealt() {
    let mut f = me(WATER, &[ABSORB]);
    f.hp = 10;
    let mut r = wild(f, foe(NORMAL, &[REAL_SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    // Absorb: 20 power Grass (special): 9×20×4/10/50 = 1, +2 = 3, roll → 2 or 3; heal =
    // max(1, dealt / 2) = 1.
    let dealt = 20 - r.b.mon(O).hp;
    assert!(dealt == 2 || dealt == 3, "{l:?}");
    assert_eq!(
        l[2..6],
        [
            "Hit O Normal".into(),
            alloc::format!("Hp O 20->{}", 20 - dealt),
            "Hp P 10->11".into(),
            "AbsorbDrain@O ~64".into(),
        ]
    );
}

#[test]
fn leech_seed_drains_an_eighth() {
    let seed = seed_where(|s| {
        let mut r = wild(me(WATER, &[LEECH_SEED]), foe(NORMAL, &[REAL_SPLASH]), s);
        r.choose(Choice::Move(0));
        r.b.battler(O).seeded
    });
    let mut m = me(WATER, &[LEECH_SEED]);
    m.hp = 15;
    let mut r = wild(m, foe(NORMAL, &[REAL_SPLASH]), seed);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l[..3],
        ["UsedMove@P move:24", "Anim P move:24", "PkmnSeeded@O ~64"]
    );
    // ENDTURN_LEECH_SEED: 20 / 8 = 2 from the seeded foe to the seeder.
    let n = l.len();
    assert_eq!(
        l[n - 3..],
        ["Hp O 20->18", "Hp P 15->17", "LeechSeedDrain@O ~64"]
    );
    // Seeded again: "evaded the attack"; a Grass target is not affected.
    let l = r.choose(Choice::Move(0));
    assert_eq!(l[1], "PkmnEvadedAttack@O ~64");
    let seed = seed_where(|s| {
        let mut r = wild(me(WATER, &[LEECH_SEED]), foe(NORMAL, &[REAL_SPLASH]), s);
        r.choose(Choice::Move(0));
        r.b.battler(O).seeded
    });
    let mut r = wild(me(WATER, &[LEECH_SEED]), foe(SPORE, &[REAL_SPLASH]), seed);
    assert_eq!(r.choose(Choice::Move(0))[1], "ItDoesntAffect@O ~64");
}

/// Bide: `setbide` (2 more turns), the battler is locked (not asked) while storing, then
/// `BattleScript_BideAttack` deals `gBideDmg × 2` with no PP and no damage roll. The foe's
/// Tackle: 10×35×4/10/50 + 2 = 4, STAB ×1.5 = 6, roll → 5, twice: 10 stored → 20.
#[test]
fn bide_returns_twice_the_damage_taken() {
    let mut r = wild(me(WATER, &[BIDE]), foe(NORMAL, &[TACKLE]), 5);
    // One answer plays all three turns: the locked player is not asked again.
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:25",
            "Anim P move:25",
            "UsedMove@O move:1",
            "Anim O move:1",
            "Hit P Normal",
            "Hp P 20->15",
            "PkmnStoringEnergy@P ~64",
            "UsedMove@O move:1",
            "Anim O move:1",
            "Hit P Normal",
            "Hp P 15->10",
            "PkmnUnleashedEnergy@P ~64",
            "Anim P move:25",
            "Hit O Normal",
            "Hp O 20->0",
            "Faint O",
            "PkmnFainted@O mon:O0",
            "PkmnGainedExp@P mon:P0 42",
            "Exp 0 125->167",
            "End(Won)",
        ]
    );
    // Bide's PP was used once.
    assert_eq!(r.b.mon(P).pp[0], 9);
}

#[test]
fn bide_without_damage_fails() {
    let mut r = wild(me(WATER, &[BIDE]), foe(NORMAL, &[REAL_SPLASH]), 5);
    let l = r.choose(Choice::Move(0));
    let at = l
        .iter()
        .position(|x| x == "PkmnUnleashedEnergy@P ~64")
        .expect("release");
    assert_eq!(l[at + 1], "ButItFailed@P ~64");
    assert_eq!(r.b.need(), Need::Action);
    assert_eq!(r.b.battler(P).bide_turns, 0);
}

#[test]
fn focus_energy_once() {
    let mut r = wild(me(WATER, &[FOCUS_ENERGY]), foe(NORMAL, &[REAL_SPLASH]), 5);
    assert_eq!(
        r.choose(Choice::Move(0))[..3],
        [
            "UsedMove@P move:23",
            "Anim P move:23",
            "GettingPumped@P ~64"
        ]
    );
    assert_eq!(
        r.choose(Choice::Move(0))[..2],
        ["UsedMove@P move:23", "ButItFailed@P ~64"]
    );
}

#[test]
fn nature_power_calls_swift_with_one_pp() {
    let mut r = wild(me(WATER, &[NATURE_POWER]), foe(NORMAL, &[REAL_SPLASH]), 5);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l[..4],
        [
            "UsedMove@P move:26",
            "NaturePowerTurnedInto@P ~64",
            "UsedMove@P move:14",
            "Anim P move:14",
        ]
    );
    assert_eq!(r.b.mon(P).pp[0], 19);
}

#[test]
fn multi_hit_sonic_boom_charge_curl_splash_vital_throw() {
    // Double Slap: 2–5 hits, one result line "Hit N time(s)!".
    let mut r = wild(me(WATER, &[DOUBLE_SLAP]), foe(NORMAL, &[REAL_SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    let hits = l.iter().filter(|x| *x == "Hit O Normal").count() as u32;
    assert!((2..=5).contains(&hits), "{l:?}");
    assert!(has(&l, &alloc::format!("HitXTimes@P {hits} ~64")), "{l:?}");
    // Sonic Boom: 20 HP flat; nothing on a Ghost.
    let mut r = wild(me(WATER, &[SONIC_BOOM]), foe(NORMAL, &[REAL_SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "Hp O 20->0"), "{l:?}");
    // Charge: see `charge_doubles_electric_damage`.
    // Defense Curl: Defense +1 (BattleScript_EffectDefenseCurl).
    let mut r = wild(me(WATER, &[DEFENSE_CURL]), foe(NORMAL, &[REAL_SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "DefenderStatRose@P Def StatRose ~64"), "{l:?}");
    // Splash: "But nothing happened!".
    let mut r = wild(me(WATER, &[REAL_SPLASH]), foe(NORMAL, &[REAL_SPLASH]), 3);
    assert_eq!(
        r.choose(Choice::Move(0))[..3],
        [
            "UsedMove@P move:37",
            "Anim P move:37",
            "ButNothingHappened@P ~64"
        ]
    );
    // Vital Throw: -1 priority, never misses.
    let mut r = wild(me(WATER, &[VITAL_THROW]), foe(NORMAL, &[TACKLE]), 3);
    let l = r.choose(Choice::Move(0));
    assert_eq!(first_mover(&l), 'O');
    assert!(has(&l, "Anim P move:34"), "{l:?}");
}

#[test]
fn charge_doubles_electric_damage() {
    // Same seed, same rolls for the hit: Thunder Shock with Charge deals about twice.
    let c = mon(ELECTRIC, 5, [20, 13, 10, 9, 9, 10], &[THUNDER_SHOCK]);
    let tank = mon(NORMAL, 50, [200, 10, 10, 5, 10, 10], &[REAL_SPLASH]);
    let mut a = wild(c, tank, 3);
    let mut b = wild(c, tank, 3);
    b.b.battler_mut(P).charge = 2;
    a.choose(Choice::Move(0));
    b.choose(Choice::Move(0));
    let (da, db) = (200 - a.b.mon(O).hp, 200 - b.b.mon(O).hp);
    // L5 SpAtk 9 vs SpDef 10, 40 power: 9×40×4/10/50 = 2, +2 = 4 (×2 = 8), STAB ×1.5 → 6
    // (12), roll ×85..100% → 5..6 (10..12).
    assert!(
        (5..=6).contains(&da) && (10..=12).contains(&db),
        "{da} {db}"
    );
    assert_eq!(b.b.battler(P).charge, 1);
}

// ---- abilities ----

#[test]
fn intimidate_and_trace_before_the_first_turn() {
    let r = wild(me(WATER, &[TACKLE]), foe(INTIM, &[REAL_SPLASH]), 3);
    let l = r.intro();
    let at = l
        .iter()
        .position(|x| x == "StatAnim P false")
        .expect("intimidate");
    assert_eq!(
        l[at + 1],
        "PkmnCutsAttackWith@O AbilityName(Intimidate) ~64"
    );
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], -1);
    // Trace copies Intimidate after the Intimidate pass (no second Intimidate).
    let r = wild(me(TRACER, &[TACKLE]), foe(INTIM, &[REAL_SPLASH]), 3);
    let l = r.intro();
    assert!(
        has(
            &l,
            "PkmnTraced@P mon:O0 AbilityName(Intimidate) AbilityName(Trace) ~64"
        ),
        "{l:?}"
    );
    assert_eq!(r.b.battler(P).ability, Ability::Intimidate);
    assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], 0);
}

#[test]
fn contact_abilities_and_synchronize() {
    // Static: 1/3 on contact; the attacker is paralysed, named by the owner's ability.
    let seed = seed_where(|s| {
        let mut r = wild(me(WATER, &[TACKLE]), foe(ELECTRIC, &[REAL_SPLASH]), s);
        r.choose(Choice::Move(0));
        r.b.mon(P).status == Status::Paralysis
    });
    let mut r = wild(me(WATER, &[TACKLE]), foe(ELECTRIC, &[REAL_SPLASH]), seed);
    let l = r.choose(Choice::Move(0));
    assert!(
        has(&l, "PkmnWasParalyzedBy@O AbilityName(Static) ~64"),
        "{l:?}"
    );
    // Not on a non-contact move.
    for s in 1..30 {
        let mut r = wild(me(WATER, &[WATER_GUN]), foe(ELECTRIC, &[REAL_SPLASH]), s);
        r.choose(Choice::Move(0));
        assert_eq!(r.b.mon(P).status, Status::None);
    }
    // Poison Point and Effect Spore show up over seeds too.
    for (sp, ab) in [(POISON, "PoisonPoint"), (SPORE, "EffectSpore")] {
        let seed = seed_where(|s| {
            let mut r = wild(me(WATER, &[TACKLE]), foe(sp, &[REAL_SPLASH]), s);
            r.choose(Choice::Move(0));
            r.b.mon(P).status != Status::None
        });
        let mut r = wild(me(WATER, &[TACKLE]), foe(sp, &[REAL_SPLASH]), seed);
        let l = r.choose(Choice::Move(0));
        assert!(
            l.iter()
                .any(|x| x.contains(&alloc::format!("AbilityName({ab})"))),
            "{l:?}"
        );
    }
    // Synchronize: poisoning the owner poisons the attacker (primary: no Shield Dust check).
    let mut r = wild(me(WATER, &[POISON_ALL]), foe(SYNC, &[REAL_SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    let at = l
        .iter()
        .position(|x| x == "PkmnWasPoisoned@O ~64")
        .expect("poisoned");
    assert_eq!(l[at + 2], "PkmnPoisonedBy@O AbilityName(Synchronize) ~64");
    assert_eq!(r.b.mon(P).status, Status::Poison);
    // On a Steel attacker, Synchronize's primary ability effect says it had no effect.
    let seed = seed_where(|s| {
        let mut r = wild(me(STEEL, &[TOXIC]), foe(SYNC, &[REAL_SPLASH]), s);
        r.choose(Choice::Move(0));
        matches!(r.b.mon(O).status, Status::Toxic(_))
    });
    let mut steel = wild(me(STEEL, &[TOXIC]), foe(SYNC, &[REAL_SPLASH]), seed);
    let l = steel.choose(Choice::Move(0));
    assert!(
        has(&l, "PkmnSXHadNoEffectOnY@O AbilityName(Synchronize) ~64"),
        "{l:?}"
    );
}

// ---- catching ----

#[test]
fn catch_with_forced_shakes() {
    // A sleeping mon at 1 HP: odds = 255 × (3×20 - 2) / 60 × 2 > 254 → caught, no roll.
    let mut f = foe(NORMAL, &[REAL_SPLASH]);
    f.hp = 1;
    f.status = Status::Sleep(5);
    let mut r = wild(me(WATER, &[TACKLE]), f, 3);
    let before = r.rng;
    let l = r.choose(Choice::Ball);
    assert_eq!(
        l,
        [
            "Ball { shakes: 4, caught: true }",
            "Gotcha@O",
            "End(Caught)",
        ]
    );
    // The ball drew no roll (the opponent's choice for this turn was drawn at the turn start).
    assert_eq!(r.rng, before);
    assert_eq!(r.b.party(P).len(), 2);
    assert_eq!(r.b.party(P).get(1).unwrap().hp, 1);
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::Caught));
    // A full party: the mon is dropped (no PC), kept for the caller to log.
    let mut f = foe(NORMAL, &[REAL_SPLASH]);
    f.status = Status::Sleep(5);
    f.hp = 1;
    let six = [me(WATER, &[TACKLE]); 6];
    let mut r = Run::wild(&six, f, 3);
    r.choose(Choice::Ball);
    assert_eq!(r.b.party(P).len(), 6);
    assert_eq!(r.b.caught_without_room().map(|p| p.hp), Some(1));
}

#[test]
fn catch_escape_messages_by_shakes() {
    // Full HP, catch rate 255: odds 85 → threshold 1048560 / √√(16711680 / 85) = 49931.
    let mut seen = [false; 4];
    for seed in 1..200 {
        let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[REAL_SPLASH]), seed);
        let l = r.choose(Choice::Ball);
        let want = [
            "PkmnBrokeFree@O ~64",
            "ItAppearedCaught@O ~64",
            "AarghAlmostHadIt@O ~64",
            "ShootSoClose@O ~64",
        ];
        for s in 0..4 {
            if l[0] == alloc::format!("Ball {{ shakes: {s}, caught: false }}") {
                assert_eq!(l[1], want[s]);
                seen[s] = true;
            }
        }
    }
    assert_eq!(seen, [true; 4]);
    assert_eq!(crate::battle::calc::shake_threshold(85), 49931);
}

// ---- RNG sequences: the number and order of `Random()` calls ----

/// `n` draws from `Rng::new(seed)`.
fn draws(seed: u32, n: usize) -> Rng {
    let mut r = Rng::new(seed);
    for _ in 0..n {
        r.random();
    }
    r
}

/// The `k`-th draw (0-based) from `Rng::new(seed)`.
fn nth(seed: u32, k: usize) -> u16 {
    let mut r = draws(seed, k);
    r.random()
}

/// A fresh battle's state with `rng` = `Rng::new(seed)` handed to `f` through an `Env`.
fn with_env<T>(
    r: &mut Run,
    seed: u32,
    f: impl FnOnce(&mut crate::battle::core::Battle, &mut Env) -> T,
) -> (T, Rng) {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    let mut env = Env::new(&TABLES, &mut rng, &mut out);
    let t = f(&mut r.b, &mut env);
    (t, rng)
}

#[test]
fn confusion_roll_then_damage_roll() {
    // CANCELER_CONFUSED: `Random() & 1`; a self-hit then draws only the damage roll
    // (`adjustnormaldamage2`, no crit roll). Both branches, by seed.
    for hurt in [true, false] {
        let seed = seed_where(|s| (nth(s, 0) & 1 == 0) == hurt);
        let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
        r.b.battler_mut(P).confusion = 3;
        let u = {
            let mut rng = Rng::new(1);
            let mut out = Vec::new();
            let env = Env::new(&TABLES, &mut rng, &mut out);
            r.b.move_use(&env, P, TACKLE, Some(0))
        };
        let (_, rng) = with_env(&mut r, seed, |b, env| super::before_move(b, env, &u));
        assert_eq!(rng, draws(seed, if hurt { 2 } else { 1 }), "hurt {hurt}");
        assert_eq!(r.b.mon(P).hp < 20, hurt);
    }
}

fn contact_hit() -> Option<crate::battle::core::HitResult> {
    Some(crate::battle::core::HitResult {
        damage: 5,
        hp_dealt: 5,
        crit: false,
        eff: crate::battle::Effectiveness::Normal,
    })
}

#[test]
fn effect_spore_rolls_ten_then_and_three_until_non_zero() {
    // ABILITYEFFECT_ON_DAMAGE, Effect Spore: `Random() % 10 == 0`, then `Random() & 3` until
    // non-zero (1 sleep, 2 poison, 3 paralysis). Pinned: a miss (1 draw), and a hit whose
    // first `& 3` is 0 and second is 2 (poison: 3 draws, poison draws nothing more).
    let miss = seed_where(|s| !nth(s, 0).is_multiple_of(10));
    let hit =
        seed_where(|s| nth(s, 0).is_multiple_of(10) && nth(s, 1) & 3 == 0 && nth(s, 2) & 3 == 2);
    for (seed, n, status) in [(miss, 1, Status::None), (hit, 3, Status::Poison)] {
        let mut r = wild(me(WATER, &[TACKLE]), foe(SPORE, &[SPLASH]), 1);
        let (_, rng) = with_env(&mut r, seed, |b, env| {
            let u = b.move_use(env, P, TACKLE, Some(0));
            super::abilities::move_end(b, env, &u, contact_hit());
        });
        assert_eq!(rng, draws(seed, n));
        assert_eq!(r.b.mon(P).status, status);
    }
}

#[test]
fn static_and_poison_point_roll_mod_three() {
    // `Random() % 3 == 0`: one draw either way.
    for (sp, status) in [(ELECTRIC, Status::Paralysis), (POISON, Status::Poison)] {
        for proc in [true, false] {
            let seed = seed_where(|s| nth(s, 0).is_multiple_of(3) == proc);
            let mut r = wild(me(WATER, &[TACKLE]), foe(sp, &[SPLASH]), 1);
            let (_, rng) = with_env(&mut r, seed, |b, env| {
                let u = b.move_use(env, P, TACKLE, Some(0));
                super::abilities::move_end(b, env, &u, contact_hit());
            });
            assert_eq!(rng, draws(seed, 1));
            let want = if proc { status } else { Status::None };
            assert_eq!(r.b.mon(P).status, want);
        }
    }
}

#[test]
fn multi_hit_count_follows_the_accuracy_roll() {
    // BattleScript_EffectMultiHit: accuracy (`Random() % 100`), then `setmultihitcounter`:
    // `Random() & 3`; above 1 a second `(Random() & 3) + 2`, else + 2.
    let mut seen = [false; 4];
    for seed in 1..80 {
        let mut r = wild(me(WATER, &[DOUBLE_SLAP]), foe(NORMAL, &[REAL_SPLASH]), seed);
        let start = r.rng;
        let l = r.choose(Choice::Move(0));
        let mut x = start;
        if x.random() % 100 >= 85 {
            continue;
        }
        let first = x.random() & 3;
        let count = if first > 1 {
            (x.random() & 3) + 2
        } else {
            first + 2
        };
        let hits = l.iter().filter(|e| *e == "Hit O Normal").count() as u16;
        assert_eq!(hits, count, "seed {seed}");
        seen[(count - 2) as usize] = true;
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn catch_shakes_stop_at_the_first_failure() {
    // Cmd_handleballthrow: `while (shakes < 4 && Random() < threshold) shakes++`: a failed
    // roll ends the loop (shakes + 1 draws), four passes draw four.
    let threshold = crate::battle::calc::shake_threshold(85) as u16;
    for (rolls, shakes, used) in [
        (&[0u16, 0, threshold, 0][..], 2, 3),
        (&[threshold, 0, 0, 0][..], 0, 1),
        (&[0, 0, 0, 0][..], 4, 4),
    ] {
        let mut i = 0;
        let got = crate::battle::calc::catch_shakes(85, || {
            i += 1;
            rolls[i - 1]
        });
        assert_eq!((got, i), (shakes, used));
    }
}

#[test]
fn full_paralysis_also_ends_bide() {
    // CANCELER_PARALYZED's own CancelMultiTurnMoves is commented out in Emerald, but its
    // script, BattleScript_MoveUsedIsParalyzed, runs `cancelmultiturnmoves BS_ATTACKER`
    // (data/battle_scripts_1.s), which clears STATUS2_BIDE: Bide still ends.
    let seed = seed_where(|s| nth(s, 0).is_multiple_of(4));
    let mut m = me(WATER, &[BIDE]);
    m.status = Status::Paralysis;
    let mut r = wild(m, foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(P).bide_turns = 2;
    let (c, rng) = with_env(&mut r, seed, |b, env| {
        let u = b.move_use(env, P, BIDE, Some(0));
        super::before_move(b, env, &u)
    });
    assert!(matches!(c, super::Canceler::Skip));
    assert_eq!(rng, draws(seed, 1));
    assert_eq!(r.b.battler(P).bide_turns, 0);
}
