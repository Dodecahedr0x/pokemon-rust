//! Battle maths: pure Gen3 formulas. Every random value is an explicit argument (one Gen3
//! `Random()` each), so the core decides when a roll is drawn and tests pin each formula.
//!
//! The damage pipeline runs in Gen3 script order (`BattleScript_EffectHit`: `damagecalc →
//! typecalc → adjustnormaldamage`): [`base_damage`] (`CalculateBaseDamage`), the crit
//! multiplier (`Cmd_damagecalc`), [`stab`] and [`type_effect`] (`Cmd_typecalc`,
//! `ModulateDmgByType`), then [`random_damage`] (`ApplyRandomDmgMultiplier`).

use super::Effectiveness;
use crate::mon::{Ability, HoldEffect, Stat, Status, Type, TypeMatchup};

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
    /// The held item's effect (`None`: no item, or one without a modelled effect).
    pub hold: Option<HoldEffect>,
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

/// The field state `CalculateBaseDamage` reads (`ABILITYEFFECT_FIELD_SPORT`): Mud Sport /
/// Water Sport on any battler.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Field {
    pub mud_sport: bool,
    pub water_sport: bool,
}

/// [`base_damage_in`] without Mud Sport or Water Sport.
/// `Cmd_weightdamagecalculation` (`sWeightToDamageTable`): the power for a target weighing
/// `weight` hectograms: the first bracket whose bound exceeds it, else 120.
pub fn low_kick_power(weight: u16) -> u8 {
    const TABLE: [(u16, u8); 5] = [(100, 20), (250, 40), (500, 60), (1000, 80), (2000, 100)];
    TABLE
        .iter()
        .find(|&&(bound, _)| weight < bound)
        .map_or(120, |&(_, power)| power)
}

pub fn base_damage(a: &Fighter, d: &Fighter, power: u8, ty: Type, crit: bool) -> i32 {
    base_damage_in(a, d, power, ty, crit, Field::default())
}

/// The weather `CalculateBaseDamage` reads (`gBattleWeather` with `WEATHER_HAS_EFFECT2`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DamageWeather {
    #[default]
    None,
    Rain,
    Sun,
    Sand,
}

/// What [`base_damage_x`] reads beyond [`base_damage_in`]: the weather, the attacker's Flash
/// Fire boost (`RESOURCE_FLAG_FLASH_FIRE`) and whether its Plus / Minus has its partner on
/// the field (`ABILITY_ON_FIELD2`); Wave C's move effects: the defender's screen, Explosion's
/// halved defense, Solar Beam in a weather other than sun.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Extra {
    pub weather: DamageWeather,
    pub flash_fire: bool,
    pub plus_minus: bool,
    /// Reflect for a physical move, Light Screen for a special one, on the defender's side
    /// and no crit (`sideStatus` with `gCritMultiplier == 1`): the damage is halved (singles).
    pub screen: bool,
    /// `EFFECT_EXPLOSION`: the defense is halved.
    pub halve_defense: bool,
    /// `MOVE_SOLAR_BEAM` in rain, sandstorm or hail: the damage is halved.
    pub weak_solar: bool,
    /// A double battle with two battlers on the defender's side
    /// (`CountAliveMonsInBattle(BATTLE_ALIVE_DEF_SIDE) == 2`): a screen takes a third
    /// (`2 * (damage / 3)`), and a `MOVE_TARGET_BOTH` move is halved.
    pub double_def: bool,
    /// The move is `MOVE_TARGET_BOTH` (halved with [`Extra::double_def`]).
    pub spread: bool,
}

/// [`base_damage_x`] with a Gen3 `u8` power and no weather.
pub fn base_damage_in(
    a: &Fighter,
    d: &Fighter,
    power: u8,
    ty: Type,
    crit: bool,
    field: Field,
) -> i32 {
    base_damage_x(a, d, power as u32, ty, crit, field, Extra::default())
}

/// Gen3 `CalculateBaseDamage` (`pokemon.c`) for a single battle without badges or screens, in
/// its order:
/// - Huge Power / Pure Power ×2 attack; the `sHoldEffectToType` item of the move's type
///   `(100 + param)%` of the attacking stat of the type's side, Choice Band ×150/100 attack;
///   Soul Dew ×150/100 special attack and defense, Deep Sea Tooth / Light Ball ×2 special
///   attack, Deep Sea Scale ×2 special defense, Metal Powder ×2 defense, Thick Club ×2 attack;
///   Plus / Minus ×150/100 special attack (after Hustle); Flash Fire ×15/10 Fire damage
///   (after the weather);
///   Solar Power ×150/100 special attack in the sun (Silice); the defender's Thick Fat halves the special attack
///   for Fire and Ice; Hustle and Guts (any status) ×150/100 attack; the defender's Marvel
///   Scale (any status) ×150/100 defense (u16 stats, as Gen3).
/// - Mud Sport / Water Sport halve Electric / Fire power, then Overgrow / Blaze / Torrent /
///   Swarm ×150/100 power for their type at HP ≤ max / 3.
/// - A crit ignores the attacker's attack drops and the defender's defense boosts.
/// - Physical: burn halves (not with Guts), minimum 1. Special has no minimum (as Gen3).
/// - `power` may exceed 255 (the Silice power-raising abilities).
/// - Special: rain halves Fire and ×15/10 Water, sun the reverse (`WEATHER_HAS_EFFECT2`).
/// - `Mystery` skips both branches, so it returns the bare `+ 2`.
///
/// The defender's divisor is clamped to 1 (Gen3 divides by zero at -6 on a 1-point stat).
pub fn base_damage_x(
    a: &Fighter,
    d: &Fighter,
    power: u32,
    ty: Type,
    crit: bool,
    field: Field,
    x: Extra,
) -> i32 {
    let mut attack = a.stat(Stat::Atk);
    let mut defense = d.stat(Stat::Def);
    let mut sp_attack = a.stat(Stat::SpAtk);
    let mut sp_defense = d.stat(Stat::SpDef);
    if matches!(a.ability, Ability::HugePower | Ability::PurePower) {
        attack = attack.wrapping_mul(2);
    }
    match a.hold {
        Some(HoldEffect::TypePower(t, p)) if t == ty => {
            if ty.is_physical() {
                attack = (attack as u32 * (p as u32 + 100) / 100) as u16;
            } else {
                sp_attack = (sp_attack as u32 * (p as u32 + 100) / 100) as u16;
            }
        }
        Some(HoldEffect::ChoiceBand) => attack = (150 * attack as u32 / 100) as u16,
        _ => {}
    }
    // The species items (the fighters' holds are already filtered by species).
    match a.hold {
        Some(HoldEffect::SoulDew(..)) => sp_attack = (150 * sp_attack as u32 / 100) as u16,
        Some(HoldEffect::DeepSeaTooth(_) | HoldEffect::LightBall(_)) => {
            sp_attack = sp_attack.wrapping_mul(2)
        }
        Some(HoldEffect::ThickClub(..)) => attack = attack.wrapping_mul(2),
        _ => {}
    }
    match d.hold {
        Some(HoldEffect::SoulDew(..)) => sp_defense = (150 * sp_defense as u32 / 100) as u16,
        Some(HoldEffect::DeepSeaScale(_)) => sp_defense = sp_defense.wrapping_mul(2),
        Some(HoldEffect::MetalPowder(_)) => defense = defense.wrapping_mul(2),
        _ => {}
    }
    if a.ability == Ability::SolarPower && x.weather == DamageWeather::Sun {
        sp_attack = (150 * sp_attack as u32 / 100) as u16;
    }
    if d.ability == Ability::ThickFat && matches!(ty, Type::Fire | Type::Ice) {
        sp_attack /= 2;
    }
    if a.ability == Ability::Hustle {
        attack = (150 * attack as u32 / 100) as u16;
    }
    if x.plus_minus && matches!(a.ability, Ability::Plus | Ability::Minus) {
        sp_attack = (150 * sp_attack as u32 / 100) as u16;
    }
    if a.ability == Ability::Guts && a.status != Status::None {
        attack = (150 * attack as u32 / 100) as u16;
    }
    if d.ability == Ability::MarvelScale && d.status != Status::None {
        defense = (150 * defense as u32 / 100) as u16;
    }
    // Self-Destruct / Explosion halve the defense (after the power modifiers in Gen3, which
    // do not read it).
    if x.halve_defense {
        defense /= 2;
    }
    let mut power = power;
    if (ty == Type::Electric && field.mud_sport) || (ty == Type::Fire && field.water_sport) {
        power /= 2;
    }
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
            attack as u32,
            a.stage(Stat::Atk),
            defense as u32,
            d.stage(Stat::Def),
        );
        if a.status == Status::Burn && a.ability != Ability::Guts {
            damage /= 2;
        }
        damage = screen_spread(damage, x);
        if damage == 0 {
            damage = 1;
        }
    }
    if ty == Type::Mystery {
        damage = 0;
    }
    if (ty as u8) > Type::Mystery as u8 {
        damage = core(
            sp_attack as u32,
            a.stage(Stat::SpAtk),
            sp_defense as u32,
            d.stage(Stat::SpDef),
        );
        damage = screen_spread(damage, x);
        match (x.weather, ty) {
            (DamageWeather::Rain, Type::Fire) => damage /= 2,
            (DamageWeather::Rain, Type::Water) => damage = 15 * damage / 10,
            _ => {}
        }
        if x.weak_solar {
            damage /= 2;
        }
        match (x.weather, ty) {
            (DamageWeather::Sun, Type::Water) => damage /= 2,
            (DamageWeather::Sun, Type::Fire) => damage = 15 * damage / 10,
            _ => {}
        }
        if x.flash_fire && ty == Type::Fire {
            damage = 15 * damage / 10;
        }
    }
    damage + 2
}

/// `CalculateBaseDamage`'s screen (Reflect / Light Screen), then its "moves hitting both foes
/// deal less damage" halving.
fn screen_spread(damage: i32, x: Extra) -> i32 {
    let mut damage = damage;
    if x.screen {
        damage = if x.double_def {
            2 * (damage / 3)
        } else {
            damage / 2
        };
    }
    if x.spread && x.double_def {
        damage /= 2;
    }
    damage
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

/// `Cmd_critcalc`'s held-item stages: Scope Lens +1, Lucky Punch / Stick +2 (the hold is
/// already filtered by species).
pub fn crit_item_stage(hold: Option<HoldEffect>) -> u8 {
    match hold {
        Some(HoldEffect::ScopeLens) => 1,
        Some(HoldEffect::SpeciesCrit(_)) => 2,
        _ => 0,
    }
}

/// Gen3 `Cmd_critcalc`: a crit when `roll % sCriticalHitChance[stage] == 0`. The caller draws
/// no roll in a first battle (`BATTLE_TYPE_FIRST_BATTLE` short-circuits before `Random()`).
pub fn is_crit(stage: u8, roll: u16) -> bool {
    roll.is_multiple_of(CRIT_CHANCE[(stage as usize).min(CRIT_CHANCE.len() - 1)])
}

/// Gen3 `Cmd_accuracycheck`: the attacker's accuracy stage against the target's evasion
/// (clamped), `sAccuracyStageRatios × accuracy`, and a miss when `roll % 100 + 1 > calc`.
pub fn accuracy_hits(move_acc: u8, acc_stage: i8, eva_stage: i8, roll: u16) -> bool {
    accuracy_hits_by(move_acc, acc_stage, eva_stage, Ability::None, false, roll)
}

/// [`accuracy_hits`] with the attacker's ability: Compound Eyes ×130/100, then Hustle
/// ×80/100 for a `physical` move (no weather, no held items).
pub fn accuracy_hits_by(
    move_acc: u8,
    acc_stage: i8,
    eva_stage: i8,
    ability: Ability,
    physical: bool,
    roll: u16,
) -> bool {
    accuracy_hits_x(
        move_acc, acc_stage, eva_stage, ability, physical, false, None, roll,
    )
}

/// [`accuracy_hits_by`] with the target's side: `sand_veil` (its Sand Veil in a sandstorm,
/// ×80/100 between Compound Eyes and Hustle) and its held item (`HOLD_EFFECT_EVASION_UP`,
/// BrightPowder / Lax Incense, ×`(100 - param)`/100 after the abilities).
#[allow(clippy::too_many_arguments)]
pub fn accuracy_hits_x(
    move_acc: u8,
    acc_stage: i8,
    eva_stage: i8,
    ability: Ability,
    physical: bool,
    sand_veil: bool,
    target_hold: Option<HoldEffect>,
    roll: u16,
) -> bool {
    let buff = (acc_stage - eva_stage).clamp(MIN_STAGE, MAX_STAGE);
    let (num, den) = ratio(&ACCURACY_STAGE_RATIOS, buff);
    let mut calc = (num * move_acc as u16) / den;
    if ability == Ability::CompoundEyes {
        calc = (calc as u32 * 130 / 100) as u16;
    }
    if sand_veil {
        calc = (calc as u32 * 80 / 100) as u16;
    }
    if ability == Ability::Hustle && physical {
        calc = (calc as u32 * 80 / 100) as u16;
    }
    if let Some(HoldEffect::EvasionUp(p)) = target_hold {
        calc = (calc as u32 * (100 - p as u32) / 100) as u16;
    }
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
    effective_speed_x(f, 1)
}

/// [`effective_speed`] with Gen3's `speedMultiplierBattler` (2 for Swift Swim in rain,
/// Chlorophyll in sun): `(speed × multiplier) × stage ratio`.
pub fn effective_speed_x(f: &Fighter, multiplier: u32) -> u32 {
    let (num, den) = ratio(&STAT_STAGE_RATIOS, f.stage(Stat::Speed));
    let mut s = f.stat(Stat::Speed) as u32 * multiplier * num as u32 / den as u32;
    if f.hold == Some(HoldEffect::MachoBrace) {
        s /= 2;
    }
    if f.status == Status::Paralysis {
        s /= 4;
    }
    s
}

#[cfg(test)]
mod tests;
