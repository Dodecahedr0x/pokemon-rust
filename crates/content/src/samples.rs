//! Named fixed samples (Milestone 8): species and moves tests and tools pick from, by decomp
//! name (resolved through [`crate::consts`]), instead of the slice tables the full import
//! removed. On the extracted world they are the Milestone 7 slice's species and moves (once
//! `SLICE_SPECIES` and `SLICE_MOVES`); on the stub battle data, the stub's.

use engine::data::{MoveId, SpeciesId};

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
