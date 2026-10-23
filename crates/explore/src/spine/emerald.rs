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
    use super::super::{BattlePlan, Step, UseItem};
    use content::consts::*;
    use engine::data::Dir::{Down, Left, Right, Up};

    use engine::data::{Dir, MapId};

    /// Gen3's `ITEM_HM03` (`include/constants/items.h`), named here because the stub build's
    /// `content::consts` has no Emerald items (this local item shadows the glob import).
    const ITEM_HM03: engine::data::ItemId = engine::data::ItemId(341);
    /// Gen3's `ITEM_HM06`, for the same reason.
    const ITEM_HM06: engine::data::ItemId = engine::data::ItemId(344);

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

    /// A walk in Mauville's gym to (x, y) around the floor switches (`sMauvilleGymSwitchCoords`)
    /// in `avoid`.
    const fn gym_switch(x: i16, y: i16, avoid: &'static [(i16, i16)]) -> Step {
        Step::GotoAvoiding {
            map: MAP_MAUVILLE_CITY_GYM,
            x,
            y,
            avoid,
        }
    }

    const fn train_fresh(map: MapId, x: i16, y: i16, level: u8) -> Step {
        Step::TrainFresh { map, x, y, level }
    }

    /// HM06 from the bag on the lead (START → BAG → HM06 → USE): the party menu teaches it,
    /// forgetting Mud-Slap (slot 2), so `EventScript_RockSmash`'s `checkpartymove` finds it.
    const USE_HM06: Step = Step::UseItem(UseItem {
        item: ITEM_HM06,
        mon: 0,
        forget: 2,
    });

    /// HM03 from the bag on the lead, as [`USE_HM06`], forgetting Water Gun (slot 3), so
    /// `EventScript_UseSurf`'s `checkpartymove` finds it.
    const USE_HM03: Step = Step::UseItem(UseItem {
        item: ITEM_HM03,
        mon: 0,
        forget: 3,
    });

    /// The Route 119 trainers' sight lines (`map.json` facing and range): the grass trainers
    /// that copy the player (range 1, every side), Catherine, Jackson, Rachel, Phil, Hugh,
    /// Takashi, Dayton, Yasu, Hideo and Fabian.
    const ROUTE119_SIGHT: &[(i16, i16)] = &[
        // Greg (12,123), Taylor (26,123), Donald (5,125), Brent (28,116), Doug (34,122),
        // Kent (17,128).
        (11, 123),
        (13, 123),
        (12, 122),
        (12, 124),
        (25, 123),
        (27, 123),
        (26, 122),
        (26, 124),
        (4, 125),
        (6, 125),
        (5, 124),
        (5, 126),
        (27, 116),
        (29, 116),
        (28, 115),
        (28, 117),
        (33, 122),
        (35, 122),
        (34, 121),
        (34, 123),
        (16, 128),
        (18, 128),
        (17, 127),
        (17, 129),
        // Catherine (35,83) down 2.
        (35, 84),
        (35, 85),
        // Jackson (7,74) down and right 3.
        (7, 75),
        (7, 76),
        (7, 77),
        (8, 74),
        (9, 74),
        (10, 74),
        // Rachel (8,68) up 5, Phil (8,63) down 5.
        (8, 63),
        (8, 64),
        (8, 65),
        (8, 66),
        (8, 67),
        (8, 68),
        // Hugh (10,50) right 3, Takashi (19,49) down 3, Dayton (16,52) right 3.
        (11, 50),
        (12, 50),
        (13, 50),
        (19, 50),
        (19, 51),
        (19, 52),
        (17, 52),
        (18, 52),
        // Yasu (28,14) down 1, Hideo (29,6) down 3, Fabian (32,15) left 4.
        (28, 15),
        (29, 7),
        (29, 8),
        (29, 9),
        (29, 15),
        (30, 15),
        (31, 15),
    ];

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
        // The floor switches (`MauvilleGymSetDefaultBarriers` flips every beam), stepping on
        // no other and round Kirk's and Vivian's sight lines (1,14), (1,15). The first (0,15)
        // opens the way north, but Ben (5,10) spots the player at (3,10) and walks to (4,10),
        // the only way on, where he stays (`OverrideTemplateCoordsForObjectEvent`). Out to
        // heal: Mauville resets the puzzle (`MauvilleCity_OnTransition`), and from the default
        // barriers the third switch (3,9) alone opens the way past him.
        gym_switch(0, 15, &[(4, 12), (3, 9), (8, 9), (1, 14), (1, 15)]),
        gym_switch(3, 10, &[(4, 12), (3, 9), (8, 9), (1, 14), (1, 15)]),
        gym_switch(5, 19, &[(0, 15), (4, 12), (3, 9), (8, 9), (1, 14), (1, 15)]),
        enter(MAP_MAUVILLE_CITY_GYM, 5, 20, Down),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        enter(MAP_MAUVILLE_CITY, 8, 5, Up),
        gym_switch(3, 9, &[(0, 15), (4, 12), (8, 9), (1, 14), (1, 15)]),
        gym_switch(5, 3, &[(0, 15), (4, 12), (3, 9), (8, 9)]),
        talk(MAP_MAUVILLE_CITY_GYM, 1),
        Step::Expect("wattson_defeated"),
        Step::Expect("badge3"),
        Step::Checkpoint("badge3"),
        // ================= Act 4: Route 111 and 112, Fallarbor, Meteor Falls, Mt. Chimney,
        // Lavaridge, Flannery =================
        // Wattson's script deactivated the puzzle (`MauvilleGymDeactivatePuzzle`).
        enter(MAP_MAUVILLE_CITY_GYM, 4, 20, Down),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // Act 4 has long stretches without a Center (Route 111 to Route 113, the cable car to
        // Mt. Chimney's admins): to level 36 (Swampert) on Route 110's north grass first, in
        // rounds that stop at half HP or low PP, healed in Mauville after each (a round past
        // 36 trains no more).
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 15, 19, Down),
        train_fresh(MAP_ROUTE110, 5, 22, 36),
        cross(MAP_ROUTE110, 15, 0, Up),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // HM06 from the Rock Smash dude; teach it from the bag.
        enter(MAP_MAUVILLE_CITY, 32, 14, Up),
        talk(MAP_MAUVILLE_CITY_HOUSE1, 1),
        enter(MAP_MAUVILLE_CITY_HOUSE1, 3, 7, Down),
        USE_HM06,
        // ---- Route 111: smash the rock north of the Winstrates', on to Route 113 ----
        cross(MAP_MAUVILLE_CITY, 16, 0, Up),
        // The rock: `EventScript_RockSmash` (yes).
        Step::Choose(0),
        talk(MAP_ROUTE111, 15),
        cross(MAP_ROUTE111, 0, 68, Left),
        // ---- Route 112: through the Fiery Path to its north part, back east onto Route 111
        // north of the desert, west to Route 113 and Fallarbor ----
        enter(MAP_ROUTE112, 11, 36, Up),
        enter(MAP_FIERY_PATH, 26, 4, Down),
        cross(MAP_ROUTE112, 39, 9, Right),
        // The old lady's rest stop heals before Route 113's trainers.
        goto(MAP_ROUTE111, 28, 11),
        goto(MAP_ROUTE111, 29, 17),
        enter(MAP_ROUTE111, 26, 18, Up),
        // Yes to "rest up here", no to "another rest" (she offers again after every yes).
        Step::Choose(crate::policy::YES_ONCE),
        talk(MAP_ROUTE111_OLD_LADYS_REST_STOP, 1),
        enter(MAP_ROUTE111_OLD_LADYS_REST_STOP, 3, 7, Down),
        cross(MAP_ROUTE111, 0, 8, Left),
        cross(MAP_ROUTE113, 0, 9, Left),
        Step::Expect("fallarbor"),
        // ---- Fallarbor: heal; Route 114 to Meteor Falls, Magma steals the meteorite ----
        enter(MAP_FALLARBOR_TOWN, 14, 7, Up),
        use_(MAP_FALLARBOR_TOWN_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_FALLARBOR_TOWN_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_FALLARBOR_TOWN, 0, 9, Left),
        enter(MAP_ROUTE114, 8, 63, Up),
        goto(MAP_METEOR_FALLS_1F_1R, 14, 18),
        Step::WaitScript,
        Step::Expect("meteor_falls"),
        // ---- Back to Route 112's cable car (the Magma grunts are gone): Fallarbor, Route
        // 113, the rest stop, Route 112 north, the Fiery Path ----
        enter(MAP_METEOR_FALLS_1F_1R, 27, 18, Down),
        cross(MAP_ROUTE114, 39, 9, Right),
        enter(MAP_FALLARBOR_TOWN, 14, 7, Up),
        use_(MAP_FALLARBOR_TOWN_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_FALLARBOR_TOWN_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_FALLARBOR_TOWN, 19, 9, Right),
        goto(MAP_ROUTE113, 14, 13),
        cross(MAP_ROUTE113, 99, 8, Right),
        goto(MAP_ROUTE111, 28, 11),
        goto(MAP_ROUTE111, 29, 17),
        enter(MAP_ROUTE111, 26, 18, Up),
        Step::Choose(crate::policy::YES_ONCE),
        talk(MAP_ROUTE111_OLD_LADYS_REST_STOP, 1),
        enter(MAP_ROUTE111_OLD_LADYS_REST_STOP, 3, 7, Down),
        Step::Battle(BattlePlan::RunWild),
        cross(MAP_ROUTE111, 0, 29, Left),
        enter(MAP_ROUTE112, 22, 10, Up),
        enter(MAP_FIERY_PATH, 26, 36, Down),
        // Route 111's and 112's trainers leave the lead hurt (a Static Electrike's paralysis,
        // Muddy Water's PP spent), short for Maxie's Camerupt and its Magnitude: back through
        // the Fiery Path to the rest stop (Route 111's south has its rocks back, Mauville is
        // out of reach), then down again.
        enter(MAP_ROUTE112, 11, 36, Up),
        enter(MAP_FIERY_PATH, 26, 4, Down),
        cross(MAP_ROUTE112, 39, 9, Right),
        goto(MAP_ROUTE111, 28, 11),
        goto(MAP_ROUTE111, 29, 17),
        enter(MAP_ROUTE111, 26, 18, Up),
        Step::Choose(crate::policy::YES_ONCE),
        talk(MAP_ROUTE111_OLD_LADYS_REST_STOP, 1),
        enter(MAP_ROUTE111_OLD_LADYS_REST_STOP, 3, 7, Down),
        cross(MAP_ROUTE111, 0, 29, Left),
        enter(MAP_ROUTE112, 22, 10, Up),
        enter(MAP_FIERY_PATH, 26, 36, Down),
        enter(MAP_ROUTE112, 28, 27, Up),
        Step::Battle(BattlePlan::BestLearn),
        // The attendant's ride (`CableCarWarp`, `CableCar`) lands in Mt. Chimney's station,
        // whose `ON_FRAME` script walks the player out.
        talk(MAP_ROUTE112_CABLE_CAR_STATION, 1),
        Step::WaitScript,
        // ---- Mt. Chimney: Maxie at the meteorite machine ----
        enter(MAP_MT_CHIMNEY_CABLE_CAR_STATION, 6, 11, Down),
        talk(MAP_MT_CHIMNEY, 2),
        Step::Expect("mt_chimney"),
        // ---- Down the Jagged Pass to Route 112's west part, Lavaridge ----
        enter(MAP_MT_CHIMNEY, 20, 41, Down),
        enter(MAP_JAGGED_PASS, 14, 40, Down),
        cross(MAP_ROUTE112, 0, 50, Left),
        Step::Expect("lavaridge"),
        enter(MAP_LAVARIDGE_TOWN, 9, 6, Up),
        use_(MAP_LAVARIDGE_TOWN_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_LAVARIDGE_TOWN_POKEMON_CENTER_1F, 7, 8, Down),
        // ---- Flannery ----
        // The gym's floors chain through 1F's hidden holes (`MB_LAVARIDGE_GYM_1F_WARP`) and
        // B1F's geysers (`MB_LAVARIDGE_GYM_B1F_WARP`) to Flannery's platform, clear of the
        // buried trainers' cells: 1F (8, 9), B1F (1, 14), 1F (0, 10), B1F (0, 6), 1F (2, 3),
        // B1F (7, 2), 1F (10, 6), then B1F (12, 12) past the ledges.
        enter(MAP_LAVARIDGE_TOWN, 5, 15, Up),
        enter(MAP_LAVARIDGE_TOWN_GYM_1F, 8, 9, Up),
        enter(MAP_LAVARIDGE_TOWN_GYM_B1F, 1, 14, Left),
        enter(MAP_LAVARIDGE_TOWN_GYM_1F, 0, 10, Left),
        enter(MAP_LAVARIDGE_TOWN_GYM_B1F, 0, 6, Up),
        enter(MAP_LAVARIDGE_TOWN_GYM_1F, 2, 3, Right),
        enter(MAP_LAVARIDGE_TOWN_GYM_B1F, 7, 2, Right),
        enter(MAP_LAVARIDGE_TOWN_GYM_1F, 10, 6, Right),
        enter(MAP_LAVARIDGE_TOWN_GYM_B1F, 12, 12, Right),
        talk(MAP_LAVARIDGE_TOWN_GYM_1F, 1),
        Step::Expect("flannery_defeated"),
        Step::Expect("badge4"),
        Step::Checkpoint("badge4"),
        // ================= Act 5: the Go-Goggles, back to Mauville, Verdanturf, Rusturf
        // Tunnel, Rustboro, Petalburg Woods, Norman =================
        enter(MAP_LAVARIDGE_TOWN_GYM_1F, 13, 18, Down),
        Step::WaitScript,
        Step::Expect("go_goggles"),
        enter(MAP_LAVARIDGE_TOWN, 9, 6, Up),
        use_(MAP_LAVARIDGE_TOWN_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_LAVARIDGE_TOWN_POKEMON_CENTER_1F, 7, 8, Down),
        // Route 112's ledges down to Route 111, south past the rock to Mauville.
        cross(MAP_LAVARIDGE_TOWN, 19, 10, Right),
        cross(MAP_ROUTE112, 39, 48, Right),
        // The same rock from the north (it comes back on every map load): smashed again, the
        // lead keeping Rock Smash (HM moves are never forgotten, `IsHMMove2`).
        goto(MAP_ROUTE111, 18, 100),
        Step::Choose(0),
        talk(MAP_ROUTE111, 15),
        cross(MAP_ROUTE111, 16, 139, Down),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // Route 117 west to Verdanturf, the tunnel to Wanda.
        cross(MAP_MAUVILLE_CITY, 0, 9, Left),
        // A stop at (12, 9) before Lydia (8, 10): walked straight in, her team's paralysis
        // and Take Down's recoil drew the battle (a whiteout), depending on the RNG.
        goto(MAP_ROUTE117, 12, 9),
        cross(MAP_ROUTE117, 0, 10, Left),
        Step::Expect("verdanturf"),
        enter(MAP_VERDANTURF_TOWN, 8, 1, Up),
        // The boulder between Wanda and her boyfriend: smashed (`removeobject`,
        // `SPECIAL_TryUpdateRusturfTunnelState`), then the tunnel's `ON_FRAME` scene (HM04).
        goto(MAP_RUSTURF_TUNNEL, 25, 5),
        Step::Choose(0),
        talk(MAP_RUSTURF_TUNNEL, 2),
        Step::WaitScript,
        Step::Expect("hm_strength"),
        // ---- West out of the tunnel, Route 116, Rustboro (heal), Route 104, the woods,
        // Petalburg ----
        enter(MAP_RUSTURF_TUNNEL, 4, 10, Down),
        cross(MAP_ROUTE116, 0, 9, Left),
        enter(MAP_RUSTBORO_CITY, 16, 38, Up),
        use_(MAP_RUSTBORO_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_RUSTBORO_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_RUSTBORO_CITY, 19, 59, Down),
        enter(MAP_ROUTE104, 11, 30, Down),
        enter(MAP_PETALBURG_WOODS, 17, 38, Down),
        cross(MAP_ROUTE104, 39, 63, Right),
        enter(MAP_PETALBURG_CITY, 20, 16, Up),
        use_(MAP_PETALBURG_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_PETALBURG_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // ---- Norman ----
        // The rooms chain through doors that are signs (`PetalburgCity_Gym_EventScript_*Door`:
        // yes, then `warpdoor`): the entrance room's (1, 105) (opened on load,
        // `PetalburgGymUnlockRoomDoors`), then (1, 79), (7, 40) and (7, 14) to Norman's, each
        // opened by beating its room's trainer (Randall, Parker, Jody), whose script slides the
        // doors open (`PetalburgGymSlideOpenRoomDoors`).
        enter(MAP_PETALBURG_CITY, 15, 8, Up),
        Step::Choose(0),
        use_(MAP_PETALBURG_CITY_GYM, 1, 106, Up),
        talk(MAP_PETALBURG_CITY_GYM, 3),
        use_(MAP_PETALBURG_CITY_GYM, 1, 80, Up),
        talk(MAP_PETALBURG_CITY_GYM, 4),
        use_(MAP_PETALBURG_CITY_GYM, 7, 41, Up),
        talk(MAP_PETALBURG_CITY_GYM, 7),
        use_(MAP_PETALBURG_CITY_GYM, 7, 15, Up),
        talk(MAP_PETALBURG_CITY_GYM, 1),
        Step::Expect("norman_defeated"),
        Step::Expect("badge5"),
        Step::Checkpoint("badge5"),
        // Wally's father gives HM03 in his house (`VAR_PETALBURG_CITY_STATE` 4).
        enter(MAP_PETALBURG_CITY, 7, 5, Up),
        Step::WaitScript,
        Step::Expect("hm_surf"),
        // Teach it from the bag.
        USE_HM03,
        // ================= Act 6 (Wave C, M10's field moves): back east to Mauville by the
        // woods, Rustboro and the tunnel, then Surf across Route 118 to Route 119 =================
        enter(MAP_PETALBURG_CITY_WALLYS_HOUSE, 3, 7, Down),
        cross(MAP_PETALBURG_CITY, 0, 13, Left),
        enter(MAP_ROUTE104, 11, 38, Up),
        enter(MAP_PETALBURG_WOODS, 15, 5, Up),
        cross(MAP_ROUTE104, 19, 0, Up),
        enter(MAP_RUSTBORO_CITY, 16, 38, Up),
        use_(MAP_RUSTBORO_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_RUSTBORO_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_RUSTBORO_CITY, 39, 9, Right),
        Step::GotoAvoiding {
            map: MAP_ROUTE116,
            x: 47,
            y: 9,
            avoid: &[(22, 13), (22, 14), (22, 15)],
        },
        enter(MAP_ROUTE116, 47, 8, Up),
        // Through the tunnel, its rock smashed, to Verdanturf.
        enter(MAP_RUSTURF_TUNNEL, 29, 16, Down),
        cross(MAP_VERDANTURF_TOWN, 19, 9, Right),
        cross(MAP_ROUTE117, 59, 8, Right),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // ---- Route 118: Surf across the river (`GetInteractedWaterScript`,
        // `EventScript_UseSurf` yes, `FLDEFF_USE_SURF`), then off the water on its east bank ----
        cross(MAP_MAUVILLE_CITY, 39, 8, Right),
        Step::Choose(0),
        use_(MAP_ROUTE118, 17, 9, Right),
        Step::Expect("surfing"),
        goto(MAP_ROUTE118, 27, 9),
        Step::Walk(Right),
        Step::Checkpoint("route118_east"),
        // North onto Route 119.
        cross(MAP_ROUTE118, 58, 0, Up),
        Step::Expect("route119"),
        Step::Checkpoint("route119"),
        // ---- Route 119: up the east bank, west over the long bridge, up the west bank to the
        // Weather Institute, round every trainer's sight (alone at L44 the lead cannot battle
        // them all without a heal); the wild battles run ----
        Step::Battle(BattlePlan::RunWild),
        Step::GotoAvoiding {
            map: MAP_ROUTE119,
            x: 6,
            y: 33,
            avoid: ROUTE119_SIGHT,
        },
        enter(MAP_ROUTE119, 6, 32, Up),
        // The bed first (`Route119_WeatherInstitute_1F_EventScript_Bed`), west of Grunt 4's
        // sight, then up past him.
        Step::Battle(BattlePlan::Best),
        goto(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 2, 3),
        use_(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 2, 3, Left),
        Step::WaitScript,
        enter(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 17, 1, Up),
        // 2F: the grunts by sight, then Shelly; Castform not nicknamed.
        Step::Choose(1),
        talk(MAP_ROUTE119_WEATHER_INSTITUTE_2F, 3),
        Step::WaitScript,
        Step::Expect("castform"),
        Step::Checkpoint("castform"),
        Step::Choose(0),
        enter(MAP_ROUTE119_WEATHER_INSTITUTE_2F, 17, 1, Up),
        goto(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 2, 3),
        use_(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 2, 3, Left),
        Step::WaitScript,
        enter(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 9, 12, Down),
        // ---- Over the river bridge, the rival at (25, 31) (HM02 Fly), north to Fortree ----
        // Muddy Water, the first move: Take Down's 85% accuracy misses Grovyle twice here
        // and its Leaf Blade (4x) wins.
        Step::Battle(BattlePlan::Fight),
        goto(MAP_ROUTE119, 26, 30),
        Step::WaitScript,
        Step::Expect("hm_fly"),
        Step::Checkpoint("hm_fly"),
        Step::Battle(BattlePlan::RunWild),
        Step::GotoAvoiding {
            map: MAP_ROUTE119,
            x: 38,
            y: 8,
            avoid: ROUTE119_SIGHT,
        },
        cross(MAP_ROUTE119, 39, 8, Right),
        Step::Expect("fortree"),
        Step::Checkpoint("fortree"),
        Step::Battle(BattlePlan::BestLearn),
    ];
}
