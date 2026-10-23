//! Silice's spine: Chapter 0, from a new game to `VAR_SIL_STORY` 9 at the Bois-Éther edge.
//!
//! The main path of `crates/content/tests/sil1_f_ch0.rs` (and of the content tests' shared
//! `silice_spine::ch00` once it lands), step for step in the explorer's vocabulary: the bar
//! (Julien's explosion, the Mallette), the night chase, home (the desk, the bed, the running
//! shoes), the morning (the starter and the gifts), the hidden Potion, the north exit, then
//! Route S1 (the first wild battle, the walker's berries, a heal at home, TÉO by sight, the
//! items) around the trainers' sight lines. Left out, because the branches explore them: the
//! exit guard while the Mallette is left behind, the north guard, the bed too early, the
//! "only once" re-checks, and the Labo's brambles (cut with Chapter 1's module, a test-only
//! grant). Each chapter's steps are appended as its scripts land.

use super::Step;

#[cfg(pkr_silice)]
pub static STEPS: &[Step] = list::STEPS;

/// No Silice spine outside a Silice build.
#[cfg(not(pkr_silice))]
pub static STEPS: &[Step] = &[];

#[cfg(pkr_silice)]
mod list {
    use super::super::{BattlePlan, Step};
    use content::consts::*;
    use engine::data::Dir::{Down, Left, Up};
    use engine::data::{Dir, MapId};

    const BAR: MapId = MAP_BRICO_BOURG_BAR;
    const TOWN: MapId = MAP_BRICO_BOURG;
    const HOUSE_1F: MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_1F;
    const HOUSE_2F: MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_2F;
    const ROUTE: MapId = MAP_ROUTE_S01;

    /// The sight lines of the Route S1 trainers, never walked through (as `ch00`'s
    /// `ROUTE_S01_SIGHT`): TÉO is met on purpose, from (6, 25).
    const SIGHT: &[(i16, i16)] = &[
        (4, 24),
        (5, 24),
        (6, 24),
        (12, 19),
        (12, 20),
        (12, 21),
        (4, 12),
        (5, 12),
        (6, 12),
        (8, 4),
        (9, 4),
        (10, 4),
        (11, 4),
    ];

    const fn goto(map: MapId, x: i16, y: i16) -> Step {
        Step::Goto { map, x, y }
    }

    const fn route(x: i16, y: i16) -> Step {
        Step::GotoAvoiding {
            map: ROUTE,
            x,
            y,
            avoid: SIGHT,
        }
    }

    const fn exit(dir: Dir, to: MapId) -> Step {
        Step::Exit { dir, to }
    }

    const fn use_(map: MapId, x: i16, y: i16, dir: Dir) -> Step {
        Step::Use { map, x, y, dir }
    }

    pub static STEPS: &[Step] = &[
        Step::Battle(BattlePlan::Best),
        // ---- the Shaker, the evening ----
        Step::Expect("intro_done"),
        Step::Checkpoint("bar"),
        // A friend's line (Bar_Sc02), then Julien (Bar_Sc03): the explosion.
        use_(BAR, 6, 5, Up),
        use_(BAR, 12, 4, Up),
        Step::Expect("explosion"),
        Step::Checkpoint("explosion"),
        // The Mallette on his table (Bar_Sc04), yes.
        use_(BAR, 13, 4, Up),
        Step::Expect("mallette"),
        Step::Checkpoint("mallette"),
        goto(BAR, 7, 9),
        exit(Down, TOWN),
        // ---- the night chase (Brico_Bourg_Sc01) ----
        goto(TOWN, 14, 15),
        Step::Expect("chase"),
        Step::Checkpoint("chase"),
        // ---- home (Player_House_Sc01 to Sc03) ----
        goto(TOWN, 5, 15),
        exit(Up, HOUSE_1F),
        Step::WaitScript,
        Step::Expect("home"),
        goto(HOUSE_1F, 8, 2),
        exit(Up, HOUSE_2F),
        // The desk, then the bed.
        use_(HOUSE_2F, 5, 3, Up),
        use_(HOUSE_2F, 2, 2, Left),
        Step::Expect("night"),
        Step::Checkpoint("night"),
        goto(HOUSE_2F, 7, 2),
        exit(Up, HOUSE_1F),
        // Maman: the running shoes.
        use_(HOUSE_1F, 3, 2, Left),
        // ---- the morning: the call, the starter (Brico_Bourg_Sc02, Sc03), ball 2 ----
        goto(HOUSE_1F, 4, 7),
        exit(Down, TOWN),
        Step::Choose(2),
        Step::WaitScript,
        Step::Expect("starter"),
        Step::Checkpoint("starter"),
        // The hidden Potion behind the fountain.
        use_(TOWN, 10, 7, Down),
        // The north exit: the signal (Brico_Bourg_Sc04), then Route S1.
        goto(TOWN, 11, 1),
        exit(Up, ROUTE),
        Step::Expect("route_s1"),
        Step::Checkpoint("route_s1"),
        // ---- Route S1 ----
        // The first wild battle (Route_S01_Sc01).
        route(9, 30),
        Step::Expect("first_battle"),
        Step::Checkpoint("first_battle"),
        // The walker's Oran Berries.
        route(7, 32),
        Step::Face(Left),
        Step::Interact,
        // Home to heal before the trainers, back by the south edge.
        route(8, 39),
        exit(Down, TOWN),
        goto(TOWN, 5, 15),
        exit(Up, HOUSE_1F),
        use_(HOUSE_1F, 3, 2, Left),
        goto(HOUSE_1F, 4, 7),
        exit(Down, TOWN),
        goto(TOWN, 11, 0),
        exit(Up, ROUTE),
        // TÉO sees the player on (6, 24).
        route(6, 25),
        goto(ROUTE, 6, 24),
        Step::Checkpoint("teo"),
        // The hidden Antidote, then the item balls.
        route(5, 22),
        Step::Face(Left),
        Step::Interact,
        route(14, 23),
        Step::Face(Up),
        Step::Interact,
        route(14, 7),
        Step::Face(Up),
        Step::Interact,
        // In sight of Bois-Éther (Route_S01_Sc02): story 9.
        route(10, 2),
        Step::Expect("bois_ether_edge"),
        Step::Checkpoint("ch0_end"),
    ];
}
