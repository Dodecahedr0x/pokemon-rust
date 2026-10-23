//! Emerald's spine: from a new game (before Birch's rescue) to Roxanne's badge, along the route
//! the evolutionary explorer found for M8 Task P (`act1:*`), rewritten as steps. It grows with
//! each milestone; the goal is the whole game.

use super::Step;

#[cfg(not(pkr_silice))]
pub static STEPS: &[Step] = list::STEPS;

/// No Emerald spine in a Silice build.
#[cfg(pkr_silice)]
pub static STEPS: &[Step] = &[];

#[cfg(not(pkr_silice))]
mod list {
    use super::super::{BattlePlan, Step};
    use content::consts::*;
    use engine::data::Dir::{Down, Left, Right, Up};

    const fn goto(map: engine::data::MapId, x: i16, y: i16) -> Step {
        Step::Goto { map, x, y }
    }

    pub static STEPS: &[Step] = &[
        Step::Battle(BattlePlan::Best),
        // ---- Littleroot: the girl at the north exit sends the player to save Birch ----
        goto(MAP_LITTLEROOT_TOWN, 11, 1),
        Step::Cross {
            map: MAP_LITTLEROOT_TOWN,
            x: 10,
            y: 0,
            dir: Up,
        },
        Step::Expect("rescue_started"),
        // ---- Route 101: Birch's bag, Mudkip, the first battle, the lab ----
        Step::Choose(2),
        Step::Use {
            map: MAP_ROUTE101,
            x: 7,
            y: 15,
            dir: Up,
        },
        Step::Expect("birch_rescued"),
        // In the lab: Birch gives the starter.
        Step::Talk {
            map: MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB,
            local_id: 1,
        },
        Step::Expect("lab"),
        Step::Checkpoint("lab"),
        Step::Enter {
            map: MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB,
            x: 6,
            y: 12,
            dir: Down,
        },
        Step::Cross {
            map: MAP_LITTLEROOT_TOWN,
            x: 10,
            y: 0,
            dir: Up,
        },
        Step::Cross {
            map: MAP_ROUTE101,
            x: 10,
            y: 0,
            dir: Up,
        },
        Step::Expect("oldale"),
        // ---- Oldale: the Pokémon Center (heal, respawn) ----
        Step::Enter {
            map: MAP_OLDALE_TOWN,
            x: 6,
            y: 16,
            dir: Up,
        },
        Step::Use {
            map: MAP_OLDALE_TOWN_POKEMON_CENTER_1F,
            x: 7,
            y: 4,
            dir: Up,
        },
        Step::Checkpoint("oldale_center"),
        Step::Enter {
            map: MAP_OLDALE_TOWN_POKEMON_CENTER_1F,
            x: 7,
            y: 8,
            dir: Down,
        },
        Step::Cross {
            map: MAP_OLDALE_TOWN,
            x: 10,
            y: 0,
            dir: Up,
        },
        Step::Expect("route103"),
        // ---- Route 103: May ----
        Step::Use {
            map: MAP_ROUTE103,
            x: 11,
            y: 3,
            dir: Left,
        },
        Step::Expect("may_defeated"),
        Step::Checkpoint("may"),
        // ---- Back to the lab: the Pokédex ----
        Step::Cross {
            map: MAP_ROUTE103,
            x: 9,
            y: 21,
            dir: Down,
        },
        Step::Cross {
            map: MAP_OLDALE_TOWN,
            x: 9,
            y: 19,
            dir: Down,
        },
        Step::Cross {
            map: MAP_ROUTE101,
            x: 10,
            y: 19,
            dir: Down,
        },
        Step::Enter {
            map: MAP_LITTLEROOT_TOWN,
            x: 7,
            y: 16,
            dir: Up,
        },
        // The lab's frame script: May and Birch, the Pokédex.
        Step::WaitScript,
        Step::Expect("pokedex"),
        Step::Checkpoint("pokedex"),
        // ---- West to Petalburg (Route 102's trainers on the way) ----
        Step::Enter {
            map: MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB,
            x: 6,
            y: 12,
            dir: Down,
        },
        Step::Cross {
            map: MAP_LITTLEROOT_TOWN,
            x: 10,
            y: 0,
            dir: Up,
        },
        Step::Cross {
            map: MAP_ROUTE101,
            x: 10,
            y: 0,
            dir: Up,
        },
        Step::Cross {
            map: MAP_OLDALE_TOWN,
            x: 0,
            y: 10,
            dir: Left,
        },
        Step::Cross {
            map: MAP_ROUTE102,
            x: 0,
            y: 9,
            dir: Left,
        },
        Step::Expect("petalburg"),
        Step::Checkpoint("petalburg"),
        // The Pokémon Center: heal (and the respawn moves here).
        Step::Enter {
            map: MAP_PETALBURG_CITY,
            x: 20,
            y: 16,
            dir: Up,
        },
        Step::Use {
            map: MAP_PETALBURG_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 4,
            dir: Up,
        },
        Step::Enter {
            map: MAP_PETALBURG_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 8,
            dir: Down,
        },
        // ---- Petalburg Gym: Norman, then Wally's catching tutorial ----
        Step::Enter {
            map: MAP_PETALBURG_CITY,
            x: 15,
            y: 8,
            dir: Up,
        },
        Step::Talk {
            map: MAP_PETALBURG_CITY_GYM,
            local_id: 1,
        },
        // The tutorial ends outside, at the gym door: back in (the return scene is a frame script
        // of the gym that starts once the player walks).
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Expect("wally_tutorial"),
        // ---- West to Route 104 and Petalburg Woods: the Aqua grunt ----
        Step::Enter {
            map: MAP_PETALBURG_CITY_GYM,
            x: 4,
            y: 111,
            dir: Down,
        },
        Step::Cross {
            map: MAP_PETALBURG_CITY,
            x: 0,
            y: 13,
            dir: Left,
        },
        Step::Expect("route104"),
        Step::Enter {
            map: MAP_ROUTE104,
            x: 11,
            y: 38,
            dir: Up,
        },
        // Level 10 (Water Gun) on the woods' grass, then back to Petalburg's Center to heal.
        Step::Train {
            map: MAP_PETALBURG_WOODS,
            x: 21,
            y: 35,
            level: 10,
        },
        Step::Enter {
            map: MAP_PETALBURG_WOODS,
            x: 17,
            y: 38,
            dir: Down,
        },
        Step::Cross {
            map: MAP_ROUTE104,
            x: 39,
            y: 63,
            dir: Right,
        },
        Step::Enter {
            map: MAP_PETALBURG_CITY,
            x: 20,
            y: 16,
            dir: Up,
        },
        Step::Use {
            map: MAP_PETALBURG_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 4,
            dir: Up,
        },
        Step::Enter {
            map: MAP_PETALBURG_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 8,
            dir: Down,
        },
        Step::Cross {
            map: MAP_PETALBURG_CITY,
            x: 0,
            y: 13,
            dir: Left,
        },
        Step::Enter {
            map: MAP_ROUTE104,
            x: 11,
            y: 38,
            dir: Up,
        },
        goto(MAP_PETALBURG_WOODS, 13, 7),
        Step::Expect("woods_grunt_defeated"),
        Step::Checkpoint("woods"),
        // ---- North through Route 104 to Rustboro ----
        Step::Enter {
            map: MAP_PETALBURG_WOODS,
            x: 15,
            y: 5,
            dir: Up,
        },
        Step::Cross {
            map: MAP_ROUTE104,
            x: 19,
            y: 0,
            dir: Up,
        },
        Step::Expect("rustboro"),
        Step::Checkpoint("rustboro"),
        // Heal, train on Route 104's north grass, heal again.
        Step::Enter {
            map: MAP_RUSTBORO_CITY,
            x: 16,
            y: 38,
            dir: Up,
        },
        Step::Use {
            map: MAP_RUSTBORO_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 4,
            dir: Up,
        },
        Step::Enter {
            map: MAP_RUSTBORO_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 8,
            dir: Down,
        },
        Step::Cross {
            map: MAP_RUSTBORO_CITY,
            x: 19,
            y: 59,
            dir: Down,
        },
        Step::Train {
            map: MAP_ROUTE104,
            x: 2,
            y: 11,
            level: 14,
        },
        Step::Cross {
            map: MAP_ROUTE104,
            x: 19,
            y: 0,
            dir: Up,
        },
        Step::Enter {
            map: MAP_RUSTBORO_CITY,
            x: 16,
            y: 38,
            dir: Up,
        },
        Step::Use {
            map: MAP_RUSTBORO_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 4,
            dir: Up,
        },
        Step::Enter {
            map: MAP_RUSTBORO_CITY_POKEMON_CENTER_1F,
            x: 7,
            y: 8,
            dir: Down,
        },
        // ---- Rustboro Gym: the trainers, then Roxanne ----
        Step::Enter {
            map: MAP_RUSTBORO_CITY,
            x: 27,
            y: 19,
            dir: Up,
        },
        // Past the gym's trainers (they battle on sight) to Roxanne at (5, 2).
        Step::Use {
            map: MAP_RUSTBORO_CITY_GYM,
            x: 5,
            y: 3,
            dir: Up,
        },
        Step::Expect("roxanne_defeated"),
        Step::Expect("badge1"),
        Step::Checkpoint("badge1"),
    ];
}
