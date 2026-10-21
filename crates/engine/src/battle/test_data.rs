//! Synthetic battle tables shared by the engine's battle unit tests: a few species and moves,
//! a type chart subset in Gen3 order, and a `GameData` that serves them. Other tasks may extend
//! it (append: ids are indices, existing tests pin them).
//!
//! Test mons are built with explicit stats ([`mon`]) so tests do not depend on the stat
//! formulas. Species use the MediumFast growth rate (`exp = level³`) to keep exp goldens easy.

// Some entries wait for the tests of later tasks (E2's effects and abilities).
#![allow(dead_code)]

use crate::data::Dir;
use crate::data::{
    Behavior, Font, GameData, GfxId, MapData, MapId, MoveId, NewGame, ScriptId, SpeciesId,
    SpriteData, TextId, UiText, WindowGfx,
};
use crate::mon::{
    Ability, BattleTables, GrowthRate, LevelUpMove, MoveEffect, MoveFlags, MoveInfo, MoveTarget,
    NAME_LEN, Party, Pokemon, SpeciesInfo, Stat, Status, Type, TypeMatchup,
};
use crate::script::{Ctx, Script};

// ---- moves (MoveId = index) ----
pub const TACKLE: MoveId = MoveId(1);
pub const GROWL: MoveId = MoveId(2);
pub const EMBER: MoveId = MoveId(3);
pub const STRUGGLE: MoveId = MoveId(4);
pub const QUICK_ATTACK: MoveId = MoveId(5);
pub const HARDEN: MoveId = MoveId(6);
pub const WATER_GUN: MoveId = MoveId(7);
pub const SWORDS_DANCE: MoveId = MoveId(8);
pub const TAKE_DOWN: MoveId = MoveId(9);
/// A status move whose effect the engine does not implement (`Unsupported`).
pub const SPLASH: MoveId = MoveId(10);
/// A 250-power Normal hit, for quick knock-outs.
pub const MEGA_HIT: MoveId = MoveId(11);
/// Defense -2.
pub const SCREECH: MoveId = MoveId(12);
/// A Normal move that never misses in practice (accuracy 100) with priority -1.
pub const SLOW_HIT: MoveId = MoveId(13);
/// An unsupported damaging effect with accuracy 0 (an always-hit move, as Gen3 Swift).
pub const SWIFT: MoveId = MoveId(14);

// ---- species (SpeciesId = index) ----
/// Water, Torrent (Mudkip-like).
pub const WATER: SpeciesId = SpeciesId(1);
/// Normal, catch rate 255, exp yield 60 (Zigzagoon-like).
pub const NORMAL: SpeciesId = SpeciesId(2);
/// Fire, Blaze (Torchic-like).
pub const FIRE: SpeciesId = SpeciesId(3);
/// Bug, Shield Dust (Wurmple-like).
pub const BUG: SpeciesId = SpeciesId(4);
/// Dark, Run Away, exp yield 55 (Poochyena-like).
pub const DARK: SpeciesId = SpeciesId(5);
/// Ghost (immune to Normal).
pub const GHOST: SpeciesId = SpeciesId(6);

const fn mv(
    effect: MoveEffect,
    power: u8,
    ty: Type,
    accuracy: u8,
    pp: u8,
    target: MoveTarget,
    priority: i8,
) -> MoveInfo {
    MoveInfo {
        name: &[0xFF],
        effect,
        power,
        ty,
        accuracy,
        pp,
        effect_chance: 0,
        target,
        priority,
        flags: MoveFlags(0),
    }
}

use MoveTarget::{Selected, User};

pub static MOVES: [MoveInfo; 15] = [
    mv(MoveEffect::Hit, 0, Type::Normal, 0, 0, Selected, 0),
    MoveInfo {
        flags: MoveFlags(MoveFlags::MAKES_CONTACT),
        ..mv(MoveEffect::Hit, 35, Type::Normal, 95, 35, Selected, 0)
    },
    mv(
        MoveEffect::StatDown {
            stat: Stat::Atk,
            stages: 1,
        },
        0,
        Type::Normal,
        100,
        40,
        Selected,
        0,
    ),
    MoveInfo {
        effect_chance: 10,
        ..mv(
            MoveEffect::StatusHit(crate::mon::StatusKind::Burn),
            40,
            Type::Fire,
            100,
            25,
            Selected,
            0,
        )
    },
    mv(MoveEffect::Recoil, 50, Type::Normal, 100, 1, Selected, 0),
    mv(
        MoveEffect::QuickAttack,
        40,
        Type::Normal,
        100,
        30,
        Selected,
        1,
    ),
    mv(
        MoveEffect::StatUp {
            stat: Stat::Def,
            stages: 1,
        },
        0,
        Type::Normal,
        0,
        30,
        User,
        0,
    ),
    mv(MoveEffect::Hit, 40, Type::Water, 100, 25, Selected, 0),
    mv(
        MoveEffect::StatUp {
            stat: Stat::Atk,
            stages: 2,
        },
        0,
        Type::Normal,
        0,
        30,
        User,
        0,
    ),
    mv(MoveEffect::Recoil, 90, Type::Normal, 85, 20, Selected, 0),
    mv(MoveEffect::Unsupported(85), 0, Type::Normal, 0, 40, User, 0),
    mv(MoveEffect::Hit, 250, Type::Normal, 100, 5, Selected, 0),
    mv(
        MoveEffect::StatDown {
            stat: Stat::Def,
            stages: 2,
        },
        0,
        Type::Normal,
        85,
        40,
        Selected,
        0,
    ),
    mv(MoveEffect::Hit, 40, Type::Normal, 100, 10, Selected, -1),
    mv(
        MoveEffect::Unsupported(17),
        60,
        Type::Normal,
        0,
        20,
        Selected,
        0,
    ),
];

const fn lv(level: u8, mov: MoveId) -> LevelUpMove {
    LevelUpMove { level, mov }
}

static WATER_LEARNSET: [LevelUpMove; 4] = [
    lv(1, TACKLE),
    lv(1, GROWL),
    lv(6, WATER_GUN),
    lv(10, HARDEN),
];
static NORMAL_LEARNSET: [LevelUpMove; 2] = [lv(1, TACKLE), lv(3, GROWL)];
/// Learns two moves at level 6 (a learn-move prompt per move when full).
static FIRE_LEARNSET: [LevelUpMove; 4] = [
    lv(1, TACKLE),
    lv(1, GROWL),
    lv(6, EMBER),
    lv(6, QUICK_ATTACK),
];

const fn species(
    base: [u8; 6],
    ty: Type,
    exp_yield: u8,
    ability: Ability,
    learnset: &'static [LevelUpMove],
) -> SpeciesInfo {
    SpeciesInfo {
        name: &[0xFF],
        base,
        types: [ty, ty],
        catch_rate: 255,
        exp_yield,
        ev_yield: [0, 0, 0, 1, 0, 0],
        gender_ratio: 127,
        friendship: 70,
        growth: GrowthRate::MediumFast,
        abilities: [ability, Ability::None],
        learnset,
        gfx: None,
    }
}

pub static SPECIES: [SpeciesInfo; 7] = [
    species([0; 6], Type::Normal, 0, Ability::None, &[]),
    species(
        [50, 70, 50, 40, 50, 50],
        Type::Water,
        65,
        Ability::Torrent,
        &WATER_LEARNSET,
    ),
    species(
        [38, 30, 41, 60, 30, 41],
        Type::Normal,
        60,
        Ability::Other,
        &NORMAL_LEARNSET,
    ),
    species(
        [45, 60, 40, 45, 70, 50],
        Type::Fire,
        65,
        Ability::Blaze,
        &FIRE_LEARNSET,
    ),
    species(
        [45, 45, 35, 20, 20, 30],
        Type::Bug,
        54,
        Ability::ShieldDust,
        &[],
    ),
    species(
        [35, 55, 35, 35, 30, 30],
        Type::Dark,
        55,
        Ability::RunAway,
        &[],
    ),
    species(
        [30, 35, 30, 80, 100, 35],
        Type::Ghost,
        95,
        Ability::None,
        &[],
    ),
];

const fn tm(attack: Type, defend: Type, multiplier: u8) -> TypeMatchup {
    TypeMatchup {
        attack,
        defend,
        multiplier,
    }
}

/// A `gTypeEffectiveness` subset, in Gen3 order; Foresight-ignored entries last.
pub static TYPE_CHART: [TypeMatchup; 13] = [
    tm(Type::Normal, Type::Rock, 5),
    tm(Type::Normal, Type::Steel, 5),
    tm(Type::Fire, Type::Fire, 5),
    tm(Type::Fire, Type::Water, 5),
    tm(Type::Fire, Type::Grass, 20),
    tm(Type::Fire, Type::Bug, 20),
    tm(Type::Water, Type::Fire, 20),
    tm(Type::Water, Type::Water, 5),
    tm(Type::Water, Type::Grass, 5),
    tm(Type::Bug, Type::Fire, 5),
    tm(Type::Dark, Type::Ghost, 20),
    // TYPE_FORESIGHT
    tm(Type::Normal, Type::Ghost, 0),
    tm(Type::Fighting, Type::Ghost, 0),
];

pub static TABLES: BattleTables = BattleTables {
    species: &SPECIES,
    moves: &MOVES,
    type_chart: &TYPE_CHART,
    foresight_split: 11,
    struggle: STRUGGLE,
    nature_power: [SPLASH; 10],
};

/// A test mon with explicit stats (HP, Atk, Def, Speed, SpAtk, SpDef), full HP and PP, exp at
/// its level's minimum (MediumFast: `level³`), personality 0 (Hardy, female for ratio 127).
pub fn mon(species: SpeciesId, level: u8, stats: [u16; 6], moves: &[MoveId]) -> Pokemon {
    let mut m = [None; 4];
    let mut pp = [0; 4];
    for (i, &mv) in moves.iter().enumerate().take(4) {
        m[i] = Some(mv);
        pp[i] = TABLES.move_info(mv).pp;
    }
    let mut nickname = [0xFF; NAME_LEN];
    nickname[0] = 0xBB + species.0 as u8; // "A", "B", ...
    Pokemon {
        species,
        personality: 0,
        ot_id: 0,
        nickname,
        level,
        exp: level as u32 * level as u32 * level as u32,
        ivs: [0; 6],
        evs: [0; 6],
        moves: m,
        pp,
        pp_bonuses: 0,
        friendship: 70,
        ability_slot: 0,
        status: Status::None,
        hp: stats[0],
        stats,
    }
}

/// A party of the given mons.
pub fn party(mons: &[Pokemon]) -> Party {
    let mut p = Party::default();
    for &m in mons {
        p.push(m).unwrap();
    }
    p
}

/// Serves [`TABLES`]; everything else is unused by battle tests.
pub struct Game;

impl GameData for Game {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!()
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        unimplemented!()
    }
    fn window(&self) -> &'static WindowGfx {
        unimplemented!()
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[0xFF]
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        alloc::boxed::Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[0xFF],
        }
    }
    fn battle(&self) -> &'static BattleTables {
        &TABLES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_indexed_by_id() {
        assert_eq!(TABLES.move_info(STRUGGLE).effect, MoveEffect::Recoil);
        assert_eq!(TABLES.move_info(QUICK_ATTACK).priority, 1);
        assert_eq!(TABLES.species_info(GHOST).types, [Type::Ghost; 2]);
        assert_eq!(
            TABLES.type_chart[TABLES.foresight_split],
            tm(Type::Normal, Type::Ghost, 0)
        );
        let m = mon(WATER, 5, [20, 13, 10, 9, 9, 10], &[TACKLE, GROWL]);
        assert_eq!(m.pp, [35, 40, 0, 0]);
        assert_eq!(m.exp, 125);
    }
}
