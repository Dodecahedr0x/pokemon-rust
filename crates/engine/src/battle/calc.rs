//! Battle maths: pure Gen3 formulas. Every random value is an explicit argument (one Gen3
//! `Random()` each), so the core decides when a roll is drawn and tests pin each formula.
//!
//! The damage pipeline runs in Gen3 script order (`BattleScript_EffectHit`: `damagecalc →
//! typecalc → adjustnormaldamage`): [`base_damage`] (`CalculateBaseDamage`), the crit
//! multiplier (`Cmd_damagecalc`), [`stab`] and [`type_effect`] (`Cmd_typecalc`,
//! `ModulateDmgByType`), then [`random_damage`] (`ApplyRandomDmgMultiplier`).

use super::Effectiveness;
use crate::mon::{Ability, Stat, Status, Type, TypeMatchup};

/// Gen3 `gStatStageRatios` (`pokemon.c`): numerator / denominator by stage -6..=6.
pub const STAT_STAGE_RATIOS: [(u16, u16); 13] = [
    (10, 40),
    (10, 35),
    (10, 30),
    (10, 25),
    (10, 20),
    (10, 15),
    (10, 10),
    (15, 10),
    (20, 10),
    (25, 10),
    (30, 10),
    (35, 10),
    (40, 10),
];

/// Gen3 `sAccuracyStageRatios` (`battle_script_commands.c`), by stage -6..=6.
pub const ACCURACY_STAGE_RATIOS: [(u16, u16); 13] = [
    (33, 100),
    (36, 100),
    (43, 100),
    (50, 100),
    (60, 100),
    (75, 100),
    (1, 1),
    (133, 100),
    (166, 100),
    (2, 1),
    (233, 100),
    (133, 50),
    (3, 1),
];

/// Gen3 `sCriticalHitChance`: one crit in N, by crit stage.
pub const CRIT_CHANCE: [u16; 5] = [16, 8, 4, 3, 2];

/// Lowest and highest stat stage (`MIN_STAT_STAGE` / `MAX_STAT_STAGE` around 0).
pub const MIN_STAGE: i8 = -6;
pub const MAX_STAGE: i8 = 6;

/// `B_WAIT_TIME_LONG`, the `waitmessage` length of most battle scripts.
pub const WAIT_LONG: u8 = 64;

/// What the formulas read of a battler (Gen3 `struct BattlePokemon` subset).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fighter {
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
    /// `Stat` order; `stats[0]` is max HP.
    pub stats: [u16; 6],
    /// Stage per `Stat`, -6..=6 (`stages[0]`, HP, is unused).
    pub stages: [i8; 8],
    pub types: [Type; 2],
    pub ability: Ability,
    pub status: Status,
}

impl Fighter {
    pub fn stage(&self, s: Stat) -> i8 {
        self.stages[s as usize]
    }

    pub fn stat(&self, s: Stat) -> u16 {
        self.stats[s as usize]
    }

    /// Gen3 `IS_BATTLER_OF_TYPE`.
    pub fn has_type(&self, t: Type) -> bool {
        self.types[0] == t || self.types[1] == t
    }
}

fn ratio(table: &[(u16, u16); 13], stage: i8) -> (u16, u16) {
    table[(stage.clamp(MIN_STAGE, MAX_STAGE) + 6) as usize]
}

/// Gen3 `APPLY_STAT_MOD`: `stat * num / den` for the stage.
pub fn apply_stage(stat: u16, stage: i8) -> u32 {
    let (num, den) = ratio(&STAT_STAGE_RATIOS, stage);
    stat as u32 * num as u32 / den as u32
}

/// Gen3 `CalculateBaseDamage` (`pokemon.c`) for a single battle without items, badges,
/// weather or screens:
/// - Guts ×150/100 attack with any status; Overgrow / Blaze / Torrent / Swarm ×150/100 power
///   for their type at HP ≤ max / 3.
/// - A crit ignores the attacker's attack drops and the defender's defense boosts.
/// - Physical: burn halves (not with Guts), minimum 1. Special has no minimum (as Gen3).
/// - `Mystery` skips both branches, so it returns the bare `+ 2`.
///
/// The defender's divisor is clamped to 1 (Gen3 divides by zero at -6 on a 1-point stat).
pub fn base_damage(a: &Fighter, d: &Fighter, power: u8, ty: Type, crit: bool) -> i32 {
    let mut power = power as u32;
    let pinch = match a.ability {
        Ability::Overgrow => ty == Type::Grass,
        Ability::Blaze => ty == Type::Fire,
        Ability::Torrent => ty == Type::Water,
        Ability::Swarm => ty == Type::Bug,
        _ => false,
    };
    if pinch && a.hp <= a.max_hp / 3 {
        power = 150 * power / 100;
    }
    let mut attack = a.stat(Stat::Atk) as u32;
    if a.ability == Ability::Guts && a.status != Status::None {
        attack = 150 * attack / 100;
    }
    // Both branches: (atk with stage) * power * (2 * level / 5 + 2) / (def with stage) / 50.
    let core = |atk: u32, atk_stage: i8, def: u32, def_stage: i8| -> i32 {
        let atk = if crit && atk_stage <= 0 {
            atk
        } else {
            atk * ratio(&STAT_STAGE_RATIOS, atk_stage).0 as u32
                / ratio(&STAT_STAGE_RATIOS, atk_stage).1 as u32
        };
        let def = if crit && def_stage >= 0 {
            def
        } else {
            def * ratio(&STAT_STAGE_RATIOS, def_stage).0 as u32
                / ratio(&STAT_STAGE_RATIOS, def_stage).1 as u32
        };
        let level_factor = 2 * a.level as u32 / 5 + 2;
        (atk * power * level_factor / def.max(1) / 50) as i32
    };
    let mut damage = 0;
    if ty.is_physical() {
        damage = core(
            attack.min(u16::MAX as u32),
            a.stage(Stat::Atk),
            d.stat(Stat::Def) as u32,
            d.stage(Stat::Def),
        );
        if a.status == Status::Burn && a.ability != Ability::Guts {
            damage /= 2;
        }
        if damage == 0 {
            damage = 1;
        }
    }
    if ty == Type::Mystery {
        damage = 0;
    }
    if (ty as u8) > Type::Mystery as u8 {
        damage = core(
            a.stat(Stat::SpAtk) as u32,
            a.stage(Stat::SpAtk),
            d.stat(Stat::SpDef) as u32,
            d.stage(Stat::SpDef),
        );
    }
    damage + 2
}

/// `Cmd_typecalc`'s same-type attack bonus: ×15/10.
pub fn stab(damage: i32) -> i32 {
    damage * 15 / 10
}

/// Result of [`type_effect`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypeResult {
    pub damage: i32,
    pub eff: Effectiveness,
}

/// Gen3 `Cmd_typecalc` + `ModulateDmgByType`: every chart entry for `move_ty` multiplies the
/// damage **in table order** (minimum 1 unless the multiplier is 0), once per defender type
/// (a mono-type defender is matched once). Super and not-very effective cancel each other as
/// Gen3's result flags do; they are only recorded for moves with power. Under Foresight the
/// entries from `foresight_split` on are ignored.
pub fn type_effect(
    chart: &[TypeMatchup],
    foresight_split: usize,
    foresight: bool,
    move_ty: Type,
    defender: [Type; 2],
    power: u8,
    damage: i32,
) -> TypeResult {
    let (mut damage, mut sup, mut not_very, mut none) = (damage, false, false, false);
    let mut modulate = |m: u8, damage: &mut i32| {
        *damage = *damage * m as i32 / 10;
        if *damage == 0 && m != 0 {
            *damage = 1;
        }
        match m {
            0 => {
                none = true;
                sup = false;
                not_very = false;
            }
            5 if power > 0 && !none => {
                if sup {
                    sup = false;
                } else {
                    not_very = true;
                }
            }
            20 if power > 0 && !none => {
                if not_very {
                    not_very = false;
                } else {
                    sup = true;
                }
            }
            _ => {}
        }
    };
    let end = if foresight {
        foresight_split.min(chart.len())
    } else {
        chart.len()
    };
    for e in chart[..end].iter().filter(|e| e.attack == move_ty) {
        if e.defend == defender[0] {
            modulate(e.multiplier, &mut damage);
        }
        if e.defend == defender[1] && defender[0] != defender[1] {
            modulate(e.multiplier, &mut damage);
        }
    }
    let eff = if none {
        Effectiveness::NoEffect
    } else if sup {
        Effectiveness::Super
    } else if not_very {
        Effectiveness::NotVery
    } else {
        Effectiveness::Normal
    };
    TypeResult { damage, eff }
}

/// Gen3 `ApplyRandomDmgMultiplier`: `× (100 - roll % 16) / 100`, minimum 1 (0 stays 0).
pub fn random_damage(damage: i32, roll: u16) -> i32 {
    if damage == 0 {
        return 0;
    }
    (damage * (100 - (roll % 16) as i32) / 100).max(1)
}

/// Gen3 `Cmd_critcalc`'s stage: Focus Energy +2 (no items or high-crit effects modelled).
pub fn crit_chance_stage(focus_energy: bool) -> u8 {
    if focus_energy { 2 } else { 0 }
}

/// Gen3 `Cmd_critcalc`: a crit when `roll % sCriticalHitChance[stage] == 0`. The caller draws
/// no roll in a first battle (`BATTLE_TYPE_FIRST_BATTLE` short-circuits before `Random()`).
pub fn is_crit(stage: u8, roll: u16) -> bool {
    roll.is_multiple_of(CRIT_CHANCE[(stage as usize).min(CRIT_CHANCE.len() - 1)])
}

/// Gen3 `Cmd_accuracycheck`: the attacker's accuracy stage against the target's evasion
/// (clamped), `sAccuracyStageRatios × accuracy`, and a miss when `roll % 100 + 1 > calc`.
pub fn accuracy_hits(move_acc: u8, acc_stage: i8, eva_stage: i8, roll: u16) -> bool {
    let buff = (acc_stage - eva_stage).clamp(MIN_STAGE, MAX_STAGE);
    let (num, den) = ratio(&ACCURACY_STAGE_RATIOS, buff);
    let calc = (num * move_acc as u16) / den;
    roll % 100 < calc
}

/// Gen3 `Cmd_getexp`: `expYield * level / 7` (a u16), split among the participants still
/// standing (minimum 1), then ×150/100 in trainer battles. No Exp. Share, Lucky Egg or traded
/// bonus.
pub fn exp_yield(base_yield: u8, level: u8, trainer: bool, participants: u8) -> u32 {
    let calculated = (base_yield as u32 * level as u32 / 7) as u16 as u32;
    let mut exp = if participants == 0 {
        0
    } else {
        calculated / participants as u32
    };
    if exp == 0 {
        exp = 1;
    }
    if trainer {
        exp = exp * 150 / 100;
    }
    exp
}

/// Gen3 `TryRunFromBattle` (singles, no items or Run Away): `None` when the player always
/// flees (faster or equal speed: no roll is drawn), else the u8 `speedVar` (`speed * 128 /
/// foe_speed + run_tries * 30`, wrapping) that must be greater than `Random() & 0xFF`.
/// `run_tries` is the count before this attempt.
pub fn run_threshold(speed: u16, foe_speed: u16, run_tries: u8) -> Option<u8> {
    if speed >= foe_speed {
        return None;
    }
    let var = speed as u32 * 128 / foe_speed.max(1) as u32 + run_tries as u32 * 30;
    Some(var as u8)
}

/// [`run_threshold`] with its roll: whether the player gets away.
pub fn run_succeeds(speed: u16, foe_speed: u16, run_tries: u8, roll: u16) -> bool {
    run_threshold(speed, foe_speed, run_tries).is_none_or(|v| v > (roll & 0xFF) as u8)
}

/// Gen3 `Cmd_handleballthrow` odds for a Poké Ball (`sBallCatchBonuses` 10): `catchRate *
/// (3 maxHP - 2 hp) / (3 maxHP)`, ×2 asleep or frozen, ×15/10 poisoned, burned or paralysed.
pub fn catch_odds(catch_rate: u8, hp: u16, max_hp: u16, status: Status) -> u32 {
    let max_hp = max_hp.max(1) as u32;
    let mut odds =
        (catch_rate as u32 * 10 / 10) * (max_hp * 3 - (hp as u32).min(max_hp) * 2) / (3 * max_hp);
    match status {
        Status::Sleep(_) | Status::Freeze => odds *= 2,
        Status::Poison | Status::Burn | Status::Paralysis | Status::Toxic(_) => {
            odds = odds * 15 / 10
        }
        Status::None => {}
    }
    odds
}

/// Gen3 shake threshold for `odds` ≤ 254: `1048560 / Sqrt(Sqrt(16711680 / odds))`.
pub fn shake_threshold(odds: u32) -> u32 {
    1_048_560 / isqrt(isqrt(16_711_680 / odds.max(1))).max(1)
}

/// Shakes of a ball throw, 4 = caught (`BALL_3_SHAKES_SUCCESS`). Odds above 254 catch without
/// a roll; otherwise each shake draws one roll and the loop stops at the first roll ≥ the
/// threshold, as Gen3's `for (shakes = 0; shakes < 4 && Random() < odds; shakes++)`.
pub fn catch_shakes(odds: u32, mut roll: impl FnMut() -> u16) -> u8 {
    if odds > 254 {
        return 4;
    }
    let threshold = shake_threshold(odds);
    let mut shakes = 0;
    while shakes < 4 && (roll() as u32) < threshold {
        shakes += 1;
    }
    shakes
}

/// The GBA BIOS `Sqrt`: floor square root.
pub fn isqrt(n: u32) -> u32 {
    if n < 2 {
        return n;
    }
    // Newton's method from above, all integers.
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Gen3 `GetWhoStrikesFirst` speed: `speed × stage ratio`, quartered when paralysed.
pub fn effective_speed(f: &Fighter) -> u32 {
    let mut s = apply_stage(f.stat(Stat::Speed), f.stage(Stat::Speed));
    if f.status == Status::Paralysis {
        s /= 4;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A battler with explicit stats (HP, Atk, Def, Speed, SpAtk, SpDef).
    fn fighter(level: u8, stats: [u16; 6], types: [Type; 2]) -> Fighter {
        Fighter {
            level,
            hp: stats[0],
            max_hp: stats[0],
            stats,
            stages: [0; 8],
            types,
            ability: Ability::None,
            status: Status::None,
        }
    }

    const NORMAL: [Type; 2] = [Type::Normal, Type::Normal];

    // Mudkip L5 (Atk 13) vs Zigzagoon L2 (Def 6); Tackle: Normal, power 35.
    fn mudkip() -> Fighter {
        fighter(5, [20, 13, 10, 9, 9, 10], [Type::Water; 2])
    }
    fn zigzagoon() -> Fighter {
        fighter(2, [13, 6, 6, 8, 5, 5], NORMAL)
    }

    #[test]
    fn tackle_golden() {
        // CalculateBaseDamage: 13 * 35 * (2*5/5 + 2) / 6 / 50 + 2 = 1820 / 6 = 303 / 50 = 6 + 2.
        let base = base_damage(&mudkip(), &zigzagoon(), 35, Type::Normal, false);
        assert_eq!(base, 8);
        // ApplyRandomDmgMultiplier: r=0 → ×100/100, r=15 → ×85/100.
        assert_eq!(random_damage(base, 0), 8);
        assert_eq!(random_damage(base, 15), 6);
        assert_eq!(random_damage(base, 16), 8, "roll % 16");
        // Cmd_damagecalc: × gCritMultiplier (2) before typecalc/random.
        let crit = base_damage(&mudkip(), &zigzagoon(), 35, Type::Normal, true) * 2;
        assert_eq!(crit, 16);
        assert_eq!(random_damage(crit, 0), 16);
        assert_eq!(random_damage(crit, 15), 13);
    }

    #[test]
    fn ember_stab_super_effective_golden() {
        // Torchic L5 SpA 13 (base 70, IV 31, Hardy) Ember (Fire, 40) vs Wurmple L3 SpD 7.
        let torchic = fighter(5, [20, 12, 9, 10, 13, 10], [Type::Fire; 2]);
        let wurmple = fighter(3, [15, 7, 7, 6, 5, 7], [Type::Bug; 2]);
        // 13 * 40 * 4 / 7 / 50 + 2 = 2080 / 7 = 297 / 50 = 5 + 2 = 7.
        let base = base_damage(&torchic, &wurmple, 40, Type::Fire, false);
        assert_eq!(base, 7);
        let d = stab(base);
        assert_eq!(d, 10);
        let chart = [TypeMatchup {
            attack: Type::Fire,
            defend: Type::Bug,
            multiplier: 20,
        }];
        let r = type_effect(&chart, 1, false, Type::Fire, wurmple.types, 40, d);
        assert_eq!(
            r,
            TypeResult {
                damage: 20,
                eff: Effectiveness::Super
            }
        );
        assert_eq!(random_damage(r.damage, 0), 20);
        assert_eq!(random_damage(r.damage, 15), 17);
    }

    #[test]
    fn stages_change_base_damage_and_crit_ignores_bad_ones() {
        let mut a = mudkip();
        a.stages[Stat::Atk as usize] = -1;
        // Growl -1: Atk 13 * 10 / 15 = 8; 8 * 35 * 4 / 6 / 50 + 2 = 1120 / 6 = 186 / 50 = 3 + 2.
        assert_eq!(apply_stage(13, -1), 8);
        assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 5);
        // A crit with a -1 attacker uses the raw stat: same base as no stages.
        assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, true), 8);
        // +2 attack is kept on a crit: 13 * 20 / 10 = 26; 26*35*4 = 3640 / 6 = 606 / 50 = 12 + 2.
        a.stages[Stat::Atk as usize] = 2;
        assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, true), 14);
        // Defender +1 Def: ignored on a crit, applied otherwise (6 * 15 / 10 = 9).
        let mut d = zigzagoon();
        d.stages[Stat::Def as usize] = 1;
        // 1820 / 9 = 202 / 50 = 4 + 2.
        assert_eq!(base_damage(&mudkip(), &d, 35, Type::Normal, false), 6);
        assert_eq!(base_damage(&mudkip(), &d, 35, Type::Normal, true), 8);
        // Defender -1 Def is kept on a crit: 6 * 10 / 15 = 4; 1820 / 4 = 455 / 50 = 9 + 2.
        d.stages[Stat::Def as usize] = -1;
        assert_eq!(base_damage(&mudkip(), &d, 35, Type::Normal, true), 11);
    }

    #[test]
    fn burn_guts_pinch_and_minimum() {
        let mut a = mudkip();
        a.status = Status::Burn;
        // 6 / 2 = 3 + 2.
        assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 5);
        // Guts: no burn halving, Atk ×1.5 = 19: 19*35*4 = 2660 / 6 = 443 / 50 = 8 + 2.
        a.ability = Ability::Guts;
        assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 10);
        // Physical minimum 1: a weak hit is 1 + 2; special has no minimum (0 + 2).
        let weak = fighter(1, [10, 1, 1, 1, 1, 1], NORMAL);
        let wall = fighter(50, [200, 200, 200, 200, 200, 200], NORMAL);
        assert_eq!(base_damage(&weak, &wall, 10, Type::Normal, false), 3);
        assert_eq!(base_damage(&weak, &wall, 10, Type::Water, false), 2);
        // Mystery skips both branches.
        assert_eq!(
            base_damage(&mudkip(), &zigzagoon(), 35, Type::Mystery, false),
            2
        );
        // Torrent at HP <= max/3: power 40 → 60 for Water. SpA 9: 9*60*4 = 2160 / 5 = 432 / 50
        // = 8 + 2; at full HP: 9*40*4 = 1440 / 5 = 288 / 50 = 5 + 2.
        let mut t = mudkip();
        t.ability = Ability::Torrent;
        assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Water, false), 7);
        t.hp = 7; // 7 > 20 / 3
        assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Water, false), 7);
        t.hp = 6; // 6 <= 20 / 3
        assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Water, false), 10);
        assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Fire, false), 7);
    }

    #[test]
    fn effectiveness_rounds_per_entry_in_table_order() {
        // gTypeEffectiveness lists FIRE→WATER (×0.5) before FIRE→GRASS (×2). On a Water/Grass
        // defender with 5 damage: 5*5/10 = 2, then 2*20/10 = 4. The other order gives 10 → 5.
        let water_first = [
            TypeMatchup {
                attack: Type::Fire,
                defend: Type::Water,
                multiplier: 5,
            },
            TypeMatchup {
                attack: Type::Fire,
                defend: Type::Grass,
                multiplier: 20,
            },
        ];
        let grass_first = [water_first[1], water_first[0]];
        let def = [Type::Water, Type::Grass];
        let a = type_effect(&water_first, 2, false, Type::Fire, def, 40, 5);
        let b = type_effect(&grass_first, 2, false, Type::Fire, def, 40, 5);
        assert_eq!(
            a,
            TypeResult {
                damage: 4,
                eff: Effectiveness::Normal
            }
        );
        assert_eq!(
            b,
            TypeResult {
                damage: 5,
                eff: Effectiveness::Normal
            }
        );
        // Both flags on the same type: a mono-type defender is matched once.
        let r = type_effect(&water_first, 2, false, Type::Fire, [Type::Water; 2], 40, 5);
        assert_eq!(r.damage, 2);
        assert_eq!(r.eff, Effectiveness::NotVery);
        // Minimum 1 unless ×0; no flags for power 0.
        let r = type_effect(&water_first, 2, false, Type::Fire, [Type::Water; 2], 0, 1);
        assert_eq!(
            r,
            TypeResult {
                damage: 1,
                eff: Effectiveness::Normal
            }
        );
    }

    #[test]
    fn immunity_and_foresight_split() {
        let chart = [
            TypeMatchup {
                attack: Type::Normal,
                defend: Type::Rock,
                multiplier: 5,
            },
            // TYPE_FORESIGHT marker here: entries from index 1 on are skipped under Foresight.
            TypeMatchup {
                attack: Type::Normal,
                defend: Type::Ghost,
                multiplier: 0,
            },
        ];
        let ghost = [Type::Ghost, Type::Ghost];
        let r = type_effect(&chart, 1, false, Type::Normal, ghost, 35, 8);
        assert_eq!(
            r,
            TypeResult {
                damage: 0,
                eff: Effectiveness::NoEffect
            }
        );
        let r = type_effect(&chart, 1, true, Type::Normal, ghost, 35, 8);
        assert_eq!(r.damage, 8);
        assert_eq!(r.eff, Effectiveness::Normal);
        assert_eq!(random_damage(0, 3), 0);
    }

    #[test]
    fn crit_chance_table() {
        assert_eq!(crit_chance_stage(false), 0);
        assert_eq!(crit_chance_stage(true), 2);
        assert!(is_crit(0, 0));
        assert!(is_crit(0, 32));
        assert!(!is_crit(0, 15));
        assert!(is_crit(2, 4));
        assert!(!is_crit(2, 5));
        assert!(is_crit(9, 2), "stages clamp to the last entry (1 in 2)");
    }

    #[test]
    fn accuracy_stage_ratios() {
        // 95% at stage 0: roll % 100 + 1 > 95 misses.
        assert!(accuracy_hits(95, 0, 0, 94));
        assert!(!accuracy_hits(95, 0, 0, 95));
        assert!(accuracy_hits(100, 0, 0, 99));
        // -1 accuracy: 75 * 100 / 100 = 75.
        assert!(accuracy_hits(100, -1, 0, 74));
        assert!(!accuracy_hits(100, -1, 0, 75));
        // +1 evasion is the same stage as -1 accuracy.
        assert!(!accuracy_hits(100, 0, 1, 75));
        // +6 accuracy vs -6 evasion clamps to +6: 3/1 * 50 = 150 always hits.
        assert!(accuracy_hits(50, 6, -6, 99));
        // Accuracy 0 never hits (Gen3 uses an effect to skip the check).
        assert!(!accuracy_hits(0, 0, 0, 0));
    }

    #[test]
    fn exp_goldens() {
        // Cmd_getexp: wild Zigzagoon (expYield 60) L2 → 60 * 2 / 7 = 17.
        assert_eq!(exp_yield(60, 2, false, 1), 17);
        // Trainer Poochyena (expYield 55) L5 → 55 * 5 / 7 = 39, ×150/100 = 58.
        assert_eq!(exp_yield(55, 5, false, 1), 39);
        assert_eq!(exp_yield(55, 5, true, 1), 58);
        // Split among two participants (17 / 2 = 8), minimum 1.
        assert_eq!(exp_yield(60, 2, false, 2), 8);
        assert_eq!(exp_yield(1, 1, false, 1), 1);
        assert_eq!(exp_yield(1, 1, true, 1), 1);
    }

    #[test]
    fn run_goldens() {
        // TryRunFromBattle: 9 * 128 / 10 = 115.
        assert_eq!(run_threshold(9, 10, 0), Some(115));
        assert!(run_succeeds(9, 10, 0, 114));
        assert!(!run_succeeds(9, 10, 0, 115));
        assert!(run_succeeds(9, 10, 0, 0x100), "roll & 0xFF");
        // 5 tries: (115 + 150) mod 256 = 9.
        assert_eq!(run_threshold(9, 10, 5), Some(9));
        // Faster or equal always flees, without a roll.
        assert_eq!(run_threshold(10, 10, 0), None);
        assert_eq!(run_threshold(11, 10, 0), None);
        assert!(run_succeeds(10, 10, 0, 0xFF));
    }

    #[test]
    fn catch_goldens() {
        // Zigzagoon catch rate 255, 13/13 HP: 255 * (39 - 26) / 39 = 85.
        let odds = catch_odds(255, 13, 13, Status::None);
        assert_eq!(odds, 85);
        // 16711680 / 85 = 196608; Sqrt → 443; Sqrt → 21; 1048560 / 21 = 49931.
        assert_eq!(isqrt(196_608), 443);
        assert_eq!(isqrt(443), 21);
        assert_eq!(shake_threshold(odds), 49931);
        // Rolls below the threshold shake; the first roll at or above stops.
        let mut rolls = [49930u16, 0, 49931, 0].into_iter();
        assert_eq!(catch_shakes(odds, || rolls.next().unwrap()), 2);
        assert_eq!(catch_shakes(odds, || 0), 4);
        assert_eq!(catch_shakes(255, || panic!("no roll above 254")), 4);
        // Status multipliers.
        assert_eq!(catch_odds(255, 13, 13, Status::Sleep(2)), 170);
        assert_eq!(catch_odds(255, 13, 13, Status::Paralysis), 127);
        // 1 HP of 13: 255 * (39 - 2) / 39 = 241.
        assert_eq!(catch_odds(255, 1, 13, Status::None), 241);
    }

    #[test]
    fn isqrt_floors() {
        for n in [0u32, 1, 2, 3, 4, 15, 16, 17, 99, 100, 101, u32::MAX] {
            let r = isqrt(n) as u64;
            assert!(r * r <= n as u64 && (r + 1) * (r + 1) > n as u64, "{n}");
        }
    }

    #[test]
    fn effective_speed_quarters_paralysis() {
        let mut f = mudkip();
        assert_eq!(effective_speed(&f), 9);
        f.stages[Stat::Speed as usize] = 1;
        assert_eq!(effective_speed(&f), 13);
        f.status = Status::Paralysis;
        assert_eq!(effective_speed(&f), 3);
    }
}
