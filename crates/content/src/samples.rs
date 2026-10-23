//! Named fixed samples (Milestone 8): species and moves tests and tools pick from, by decomp
//! name (resolved through [`crate::consts`]), instead of the slice tables the full import
//! removed. On the extracted world they are the Milestone 7 slice's species and moves (once
//! `SLICE_SPECIES` and `SLICE_MOVES`); on the stub battle data, the stub's.

use alloc::vec::Vec;
use engine::data::{MapId, MoveId, SpeciesId};

macro_rules! samples {
    ($names:ident, $ids:ident: $ty:ty = [$($c:ident),* $(,)?]) => {
        /// Decomp names.
        pub const $names: &[&str] = &[$(stringify!($c)),*];
        /// The same, as ids.
        pub const $ids: &[$ty] = &[$(crate::consts::$c),*];
    };
}

#[cfg(not(pkr_stub_battle))]
samples!(SPECIES, SPECIES_IDS: SpeciesId = [
    SPECIES_LOTAD, SPECIES_MAGIKARP, SPECIES_MAKUHITA, SPECIES_MARILL, SPECIES_MUDKIP,
    SPECIES_POOCHYENA, SPECIES_RALTS, SPECIES_ROSELIA, SPECIES_SEEDOT, SPECIES_SHROOMISH,
    SPECIES_SKITTY, SPECIES_TAILLOW, SPECIES_TENTACOOL, SPECIES_TORCHIC, SPECIES_TREECKO,
    SPECIES_VOLTORB, SPECIES_WINGULL, SPECIES_WURMPLE, SPECIES_ZIGZAGOON,
]);

#[cfg(not(pkr_stub_battle))]
samples!(MOVES, MOVE_IDS: MoveId = [
    MOVE_ABSORB, MOVE_ARM_THRUST, MOVE_ASTONISH, MOVE_ATTRACT, MOVE_BIDE, MOVE_CHARGE,
    MOVE_CONSTRICT, MOVE_DEFENSE_CURL, MOVE_DOUBLE_SLAP, MOVE_EMBER, MOVE_FOCUS_ENERGY,
    MOVE_GROWL, MOVE_GROWTH, MOVE_HARDEN, MOVE_HOWL, MOVE_LEECH_SEED, MOVE_LEER, MOVE_MUD_SLAP,
    MOVE_PECK, MOVE_POISON_STING, MOVE_POUND, MOVE_QUICK_ATTACK, MOVE_ROLLOUT, MOVE_SAND_ATTACK,
    MOVE_SCRATCH, MOVE_SCREECH, MOVE_SING, MOVE_SONIC_BOOM, MOVE_SPLASH, MOVE_STRING_SHOT,
    MOVE_STRUGGLE, MOVE_STUN_SPORE, MOVE_SUPERSONIC, MOVE_TACKLE, MOVE_TAIL_WHIP,
    MOVE_VITAL_THROW, MOVE_WATER_GUN,
]);

#[cfg(pkr_stub_battle)]
samples!(SPECIES, SPECIES_IDS: SpeciesId = [SPECIES_STUB_A, SPECIES_STUB_B]);

#[cfg(pkr_stub_battle)]
samples!(MOVES, MOVE_IDS: MoveId = [MOVE_STUB_GROWL, MOVE_STUB_HIT, MOVE_STUB_STRUGGLE]);

/// The maps debug tests sample (Milestone 8 Task L), by decomp name: the 15 slice maps, the Act 1
/// maps the slice does not have (Petalburg, Route 104, Petalburg Woods, Rustboro), and one
/// interior per tileset family (`primary`/`secondary` pair) the rest leaves out. Whole-game
/// versions of the tests are `#[ignore]`d and run under `cargo xtask sweep --studio`.
#[cfg(not(pkr_stub))]
pub const MAPS: &[&str] = &[
    "MAP_LITTLEROOT_TOWN",
    "MAP_ROUTE101",
    "MAP_OLDALE_TOWN",
    "MAP_ROUTE102",
    "MAP_ROUTE103",
    "MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F",
    "MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F",
    "MAP_LITTLEROOT_TOWN_MAYS_HOUSE_1F",
    "MAP_LITTLEROOT_TOWN_MAYS_HOUSE_2F",
    "MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB",
    "MAP_OLDALE_TOWN_HOUSE1",
    "MAP_OLDALE_TOWN_HOUSE2",
    "MAP_OLDALE_TOWN_POKEMON_CENTER_1F",
    "MAP_OLDALE_TOWN_POKEMON_CENTER_2F",
    "MAP_OLDALE_TOWN_MART",
    "MAP_PETALBURG_CITY",
    "MAP_RUSTBORO_CITY",
    "MAP_ROUTE104",
    "MAP_PETALBURG_WOODS",
    // Interiors: Building + GenericBuilding, Gym, Shop, Facility, School; General + Cave,
    // Facility, InsideShip, Underwater; SecretBase.
    "MAP_PETALBURG_CITY_HOUSE1",
    "MAP_RUSTBORO_CITY_GYM",
    "MAP_RUSTBORO_CITY_MART",
    "MAP_RUSTBORO_CITY_DEVON_CORP_1F",
    "MAP_RUSTBORO_CITY_POKEMON_SCHOOL",
    "MAP_GRANITE_CAVE_1F",
    "MAP_AQUA_HIDEOUT_1F",
    "MAP_ABANDONED_SHIP_CORRIDORS_1F",
    "MAP_UNDERWATER_ROUTE105",
    "MAP_SECRET_BASE_BLUE_CAVE1",
];

#[cfg(not(pkr_stub))]
const MAP_SAMPLE: &[MapId] = &[
    crate::consts::MAP_LITTLEROOT_TOWN,
    crate::consts::MAP_ROUTE101,
    crate::consts::MAP_OLDALE_TOWN,
    crate::consts::MAP_ROUTE102,
    crate::consts::MAP_ROUTE103,
    crate::consts::MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F,
    crate::consts::MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F,
    crate::consts::MAP_LITTLEROOT_TOWN_MAYS_HOUSE_1F,
    crate::consts::MAP_LITTLEROOT_TOWN_MAYS_HOUSE_2F,
    crate::consts::MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB,
    crate::consts::MAP_OLDALE_TOWN_HOUSE1,
    crate::consts::MAP_OLDALE_TOWN_HOUSE2,
    crate::consts::MAP_OLDALE_TOWN_POKEMON_CENTER_1F,
    crate::consts::MAP_OLDALE_TOWN_POKEMON_CENTER_2F,
    crate::consts::MAP_OLDALE_TOWN_MART,
    crate::consts::MAP_PETALBURG_CITY,
    crate::consts::MAP_RUSTBORO_CITY,
    crate::consts::MAP_ROUTE104,
    crate::consts::MAP_PETALBURG_WOODS,
    crate::consts::MAP_PETALBURG_CITY_HOUSE1,
    crate::consts::MAP_RUSTBORO_CITY_GYM,
    crate::consts::MAP_RUSTBORO_CITY_MART,
    crate::consts::MAP_RUSTBORO_CITY_DEVON_CORP_1F,
    crate::consts::MAP_RUSTBORO_CITY_POKEMON_SCHOOL,
    crate::consts::MAP_GRANITE_CAVE_1F,
    crate::consts::MAP_AQUA_HIDEOUT_1F,
    crate::consts::MAP_ABANDONED_SHIP_CORRIDORS_1F,
    crate::consts::MAP_UNDERWATER_ROUTE105,
    crate::consts::MAP_SECRET_BASE_BLUE_CAVE1,
];

/// The stub world is small: every map.
#[cfg(pkr_stub)]
pub const MAPS: &[&str] = &[];

/// [`MAPS`] that this build has (every map of the stub world), in [`MAPS`] order.
pub fn map_ids() -> Vec<MapId> {
    #[cfg(not(pkr_stub))]
    return MAP_SAMPLE
        .iter()
        .copied()
        .filter(|&m| crate::has_map(m))
        .collect();
    #[cfg(pkr_stub)]
    return crate::data::MAP_IDS.to_vec();
}
