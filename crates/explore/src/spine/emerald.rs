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
    use super::super::{BattlePlan, Bypass, Step};
    use content::consts::*;
    use engine::data::Dir::{Down, Left, Right, Up};

    use engine::data::{Dir, MapId};

    const fn goto(map: MapId, x: i16, y: i16) -> Step {
        Step::Goto { map, x, y }
    }

    const fn enter(map: MapId, x: i16, y: i16, dir: Dir) -> Step {
        Step::Enter { map, x, y, dir }
    }

    const fn cross(map: MapId, x: i16, y: i16, dir: Dir) -> Step {
        Step::Cross { map, x, y, dir }
    }

    const fn use_(map: MapId, x: i16, y: i16, dir: Dir) -> Step {
        Step::Use { map, x, y, dir }
    }

    const fn talk(map: MapId, local_id: u8) -> Step {
        Step::Talk { map, local_id }
    }

    const fn train(map: MapId, x: i16, y: i16, level: u8) -> Step {
        Step::Train { map, x, y, level }
    }

    /// Mauville's gym: its floor switches run `special MauvilleGymSetDefaultBarriers` and
    /// `MauvilleGymPressSwitch` (unsupported), which toggle every beam barrier; without them
    /// the barriers never move and Wattson's dais is walled off. The bypass stands the player
    /// below Wattson (the gym's trainers are optional).
    static MAUVILLE_GYM_BARRIERS: Bypass = Bypass {
        issue: "mauville-gym-barriers",
        features: &[
            "SPECIAL_MauvilleGymSetDefaultBarriers",
            "SPECIAL_MauvilleGymPressSwitch",
        ],
        warp: Some((MAP_MAUVILLE_CITY_GYM, 5, 3)),
        ..Bypass::NONE
    };

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
        // Heal at Oldale's Center first: Route 102's trainers and grass can white out a lead
        // that comes back from the rival hurt.
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
        Step::Enter {
            map: MAP_OLDALE_TOWN_POKEMON_CENTER_1F,
            x: 7,
            y: 8,
            dir: Down,
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
        // Norman's script ends outside the gym; two frame scripts follow on idle frames: the
        // city's (Wally's catching tutorial, then the warp back into the gym), then the gym's
        // (`ReturnFromWallyTutorial`), which starts on the field's first frame after the warp,
        // one frame after the first scene's macro ended.
        Step::WaitScript,
        Step::WaitScript,
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
        // ================= Act 2: Devon Goods, Mr. Stone, Dewford, Brawly =================
        enter(MAP_RUSTBORO_CITY_GYM, 5, 19, Down),
        // West past the trigger column (23, 20..24): the grunt runs off with the Devon Goods.
        goto(MAP_RUSTBORO_CITY, 23, 21),
        Step::Expect("goods_stolen"),
        // The Devon employee's trigger (30, 9..12) asks for help.
        goto(MAP_RUSTBORO_CITY, 30, 11),
        // Heal before Route 116's trainers.
        enter(MAP_RUSTBORO_CITY, 16, 38, Up),
        use_(MAP_RUSTBORO_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_RUSTBORO_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_RUSTBORO_CITY, 39, 9, Right),
        // ---- Route 116: Mr. Briney's trigger, the Rusturf Tunnel grunt ----
        // Around the School Kid's sight at (22, 16) (her Shroomish's Stun Spore and Absorb can
        // outlast a Water Gun lead, depending on the RNG).
        Step::GotoAvoiding {
            map: MAP_ROUTE116,
            x: 47,
            y: 9,
            avoid: &[(22, 13), (22, 14), (22, 15)],
        },
        enter(MAP_ROUTE116, 47, 8, Up),
        goto(MAP_RUSTURF_TUNNEL, 9, 5),
        use_(MAP_RUSTURF_TUNNEL, 13, 5, Right),
        Step::Expect("devon_goods"),
        Step::Checkpoint("devon_goods"),
        enter(MAP_RUSTURF_TUNNEL, 4, 10, Down),
        cross(MAP_ROUTE116, 0, 9, Left),
        // ---- Back to Rustboro: the employee warps the player to Mr. Stone ----
        // Onto the employee's trigger (30, 9): its script warps to Devon's 3rd floor.
        goto(MAP_RUSTBORO_CITY, 31, 9),
        Step::Walk(Left),
        Step::WaitScript,
        Step::Expect("pokenav"),
        Step::Checkpoint("mr_stone"),
        enter(MAP_RUSTBORO_CITY_DEVON_CORP_3F, 2, 1, Up),
        enter(MAP_RUSTBORO_CITY_DEVON_CORP_2F, 14, 1, Up),
        enter(MAP_RUSTBORO_CITY_DEVON_CORP_1F, 5, 8, Down),
        // The scientist's frame script (Match Call).
        Step::WaitScript,
        // ---- May at Rustboro's south exit: she registers in the PokéNav; her battle is
        // optional (no: at L18 alone the party loses it) ----
        Step::Choose(1),
        goto(MAP_RUSTBORO_CITY, 19, 53),
        Step::Expect("rival_rustboro"),
        cross(MAP_RUSTBORO_CITY, 19, 59, Down),
        // ---- Through Petalburg Woods to Mr. Briney's cottage: sail to Dewford ----
        enter(MAP_ROUTE104, 11, 30, Down),
        enter(MAP_PETALBURG_WOODS, 17, 38, Down),
        enter(MAP_ROUTE104, 17, 50, Up),
        // Mr. Briney and Peeko walk the same loop round the room without a pause, Peeko three
        // cells ahead: step into it once Peeko is two cells past, and Mr. Briney stops in front.
        goto(MAP_ROUTE104_MR_BRINEYS_HOUSE, 10, 6),
        Step::WaitFor {
            local_id: 2,
            x: 7,
            y: 6,
        },
        Step::Walk(Left),
        Step::Face(Up),
        // Yes to Mr. Briney: his script warps to Route 104's dock, whose frame script sails
        // down Route 104, 105 and 106 to Dewford (the camera follows the player across the
        // connections).
        Step::Choose(0),
        Step::Interact,
        Step::WaitScript,
        Step::Expect("dewford"),
        Step::Checkpoint("dewford"),
        // ---- Dewford: heal, then north to Granite Cave ----
        enter(MAP_DEWFORD_TOWN, 2, 10, Up),
        use_(MAP_DEWFORD_TOWN_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_DEWFORD_TOWN_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_DEWFORD_TOWN, 9, 0, Up),
        enter(MAP_ROUTE106, 48, 16, Up),
        // ---- Granite Cave: 1F's ladder down, across B1F, up to 1F's north, Steven's room ----
        // (running from its wild battles: they interrupt every walk).
        Step::Battle(BattlePlan::RunWild),
        enter(MAP_GRANITE_CAVE_1F, 17, 11, Down),
        enter(MAP_GRANITE_CAVE_B1F, 28, 21, Down),
        enter(MAP_GRANITE_CAVE_B2F, 29, 13, Up),
        enter(MAP_GRANITE_CAVE_B1F, 25, 13, Up),
        enter(MAP_GRANITE_CAVE_1F, 5, 10, Down),
        talk(MAP_GRANITE_CAVE_STEVENS_ROOM, 1),
        Step::Expect("steven_letter"),
        Step::Checkpoint("steven"),
        // ---- Level 22 on Granite Cave's floor, back to Dewford, heal ----
        Step::Battle(BattlePlan::Best),
        enter(MAP_GRANITE_CAVE_STEVENS_ROOM, 7, 3, Up),
        train(MAP_GRANITE_CAVE_1F, 7, 9, 22),
        enter(MAP_GRANITE_CAVE_1F, 37, 12, Down),
        cross(MAP_ROUTE106, 69, 19, Down),
        enter(MAP_DEWFORD_TOWN, 2, 10, Up),
        use_(MAP_DEWFORD_TOWN_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_DEWFORD_TOWN_POKEMON_CENTER_1F, 7, 8, Down),
        // ---- Dewford Gym: Brawly at (4, 3) ----
        enter(MAP_DEWFORD_TOWN, 8, 17, Up),
        use_(MAP_DEWFORD_TOWN_GYM, 4, 4, Up),
        Step::Expect("brawly_defeated"),
        Step::Expect("badge2"),
        Step::Checkpoint("badge2"),
        // ================= Act 3: Slateport, the museum, Route 110, Mauville, Wattson =========
        enter(MAP_DEWFORD_TOWN_GYM, 5, 27, Down),
        enter(MAP_DEWFORD_TOWN, 2, 10, Up),
        use_(MAP_DEWFORD_TOWN_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_DEWFORD_TOWN_POKEMON_CENTER_1F, 7, 8, Down),
        // Mr. Briney on the dock (Talk gives NoPath on him, an explorer gap: stand west of
        // him). His multichoice's Slateport (1) sails to Route 109.
        goto(MAP_DEWFORD_TOWN, 11, 9),
        Step::Face(Right),
        Step::Choose(1),
        Step::Interact,
        cross(MAP_ROUTE109, 19, 0, Up),
        Step::Expect("slateport"),
        // ---- Slateport: heal, Dock at the shipyard (the grunts at the museum's door leave),
        // the Oceanic Museum (its fee: yes), Capt. Stern on 2F ----
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        enter(MAP_SLATEPORT_CITY, 26, 38, Up),
        talk(MAP_SLATEPORT_CITY_STERNS_SHIPYARD_1F, 1),
        enter(MAP_SLATEPORT_CITY_STERNS_SHIPYARD_1F, 2, 14, Down),
        enter(MAP_SLATEPORT_CITY, 30, 26, Up),
        goto(MAP_SLATEPORT_CITY_OCEANIC_MUSEUM_1F, 9, 6),
        Step::WaitScript,
        enter(MAP_SLATEPORT_CITY_OCEANIC_MUSEUM_1F, 6, 1, Up),
        talk(MAP_SLATEPORT_CITY_OCEANIC_MUSEUM_2F, 1),
        Step::Expect("devon_goods_delivered"),
        Step::Checkpoint("museum"),
        // ---- Route 110: the rival's trigger, then north to Mauville ----
        enter(MAP_SLATEPORT_CITY_OCEANIC_MUSEUM_2F, 6, 1, Up),
        enter(MAP_SLATEPORT_CITY_OCEANIC_MUSEUM_1F, 9, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        // The rival's Grovyle beats a level 30 Marshtomp that never learnt a move: from here
        // on, learn the stronger level-up moves. Level 31 (Take Down) on the south grass, a
        // level at a time (a level drains the lead's PP), healing in Slateport after each.
        Step::Battle(BattlePlan::BestLearn),
        train(MAP_ROUTE110, 14, 96, 24),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        train(MAP_ROUTE110, 14, 96, 25),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        train(MAP_ROUTE110, 14, 96, 26),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        train(MAP_ROUTE110, 14, 96, 27),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        train(MAP_ROUTE110, 14, 96, 28),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        train(MAP_ROUTE110, 14, 96, 29),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        train(MAP_ROUTE110, 14, 96, 30),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        train(MAP_ROUTE110, 14, 96, 31),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        // The trainers on the way drain the lead: beat them, heal, then the rival.
        goto(MAP_ROUTE110, 34, 57),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        goto(MAP_ROUTE110, 34, 56),
        Step::WaitScript,
        Step::Expect("rival_route110"),
        // Heal again before the trainers of the north half.
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        Step::Battle(BattlePlan::RunWild),
        cross(MAP_ROUTE110, 15, 0, Up),
        Step::Expect("mauville"),
        // ---- Mauville: heal, Wally's battle at the gym's door (yes), the gym ----
        Step::Battle(BattlePlan::BestLearn),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // Level 32 on Route 110's north grass, healed in Mauville after (level 33 comes on the
        // south grass below: the north grass whited out the lead at 32 on some RNG paths).
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train(MAP_ROUTE110, 5, 22, 32),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        talk(MAP_MAUVILLE_CITY, 6),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // One more level on Route 110's south grass, healed in Slateport after:
        // Wattson's Manectric can outlast a level 32 lead, depending on the RNG.
        Step::Battle(BattlePlan::RunWild),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        Step::Battle(BattlePlan::BestLearn),
        train(MAP_ROUTE110, 14, 96, 33),
        cross(MAP_ROUTE110, 17, 99, Down),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_SLATEPORT_CITY, 17, 0, Up),
        Step::Battle(BattlePlan::RunWild),
        cross(MAP_ROUTE110, 15, 0, Up),
        Step::Battle(BattlePlan::BestLearn),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        enter(MAP_MAUVILLE_CITY, 8, 5, Up),
        // The switches never open the barriers (`mauville-gym-barriers`).
        Step::Bypass(&MAUVILLE_GYM_BARRIERS),
        talk(MAP_MAUVILLE_CITY_GYM, 1),
        Step::Expect("wattson_defeated"),
        Step::Expect("badge3"),
        Step::Checkpoint("badge3"),
    ];
}
