//! The stub world's spine (one 16x16 map without events; the new game starts with a
//! Pokémon): the parity stub route (`game_native::scenario::STUB_ROUTE`). The fixture of the
//! branch tests.

use super::{NEW_GAME_MAP as M, Step};

pub static STEPS: &[Step] = &[
    Step::Checkpoint("start"),
    Step::Goto {
        map: M,
        x: 10,
        y: 9,
    },
    Step::Expect("east"),
    // The one tall-grass cell: a wild battle, fought.
    Step::Grass {
        map: M,
        x: 12,
        y: 3,
    },
    Step::Expect("wild_won"),
    Step::Checkpoint("after_battle"),
    Step::Goto { map: M, x: 3, y: 3 },
    Step::Goto {
        map: M,
        x: 5,
        y: 12,
    },
    Step::Expect("far_corner"),
    Step::Checkpoint("end"),
];
