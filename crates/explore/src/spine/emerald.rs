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
    use super::super::{BattlePlan, Bypass, Step, UseItem};
    use content::consts::*;
    use engine::data::Dir::{Down, Left, Right, Up};

    use engine::data::{Dir, MapId};

    /// Gen3's `ITEM_HM03` (`include/constants/items.h`), named here because the stub build's
    /// `content::consts` has no Emerald items (this local item shadows the glob import).
    const ITEM_HM03: engine::data::ItemId = engine::data::ItemId(341);
    /// Gen3's `ITEM_HM04`, for the same reason.
    const ITEM_HM04: engine::data::ItemId = engine::data::ItemId(342);
    /// Gen3's `ITEM_HM06`, for the same reason.
    const ITEM_HM06: engine::data::ItemId = engine::data::ItemId(344);
    /// Gen3's `SPECIES_TENTACOOL` (`include/constants/species.h`), for the same reason.
    const SPECIES_TENTACOOL: u16 = 72;
    /// Gen3's `SPECIES_WINGULL`, for the same reason.
    const SPECIES_WINGULL: u16 = 309;
    /// Gen3's `ITEM_HM07`, for the same reason.
    const ITEM_HM07: engine::data::ItemId = engine::data::ItemId(345);
    /// Gen3's `ITEM_HM08`, for the same reason.
    const ITEM_HM08: engine::data::ItemId = engine::data::ItemId(346);
    /// Gen3's `ITEM_FULL_RESTORE`, for the same reason.
    const ITEM_FULL_RESTORE: engine::data::ItemId = engine::data::ItemId(19);
    /// Gen3's `ITEM_MAX_ELIXIR`, for the same reason.
    const ITEM_MAX_ELIXIR: engine::data::ItemId = engine::data::ItemId(37);

    /// The League's Poké Mart (`EverGrandeCity_PokemonLeague_1F_EventScript_Clerk`'s
    /// `pokemart`): the mart screen is a stub (Milestone 9 Task M), so the Full Restores the
    /// lead drinks between the Elite Four's rooms (the policy uses no items in battle, and the
    /// rooms have no nurse) are bought here, at the mart's price.
    static LEAGUE_MART: Bypass = Bypass {
        issue: "league-mart",
        features: &["pokemart"],
        buy: &[(ITEM_FULL_RESTORE, 10)],
        ..Bypass::NONE
    };

    /// The Hall of Fame (`EverGrandeCity_HallOfFame_EventScript_EnterHallOfFame`): its walk and
    /// `FLDEFF_HALL_OF_FAME_RECORD` play, but `special GameClear` (the Hall of Fame screen, the
    /// credits, the save and the reset; `post_battle_event_funcs.c`) does not, which leaves the
    /// screen faded out and the world never capturable again. So the whole room is bypassed on
    /// arrival: `SetGameClearFlags`' `FLAG_IS_CHAMPION`, `GameClear`'s `FLAG_SYS_GAME_CLEAR`,
    /// and the continue warp (`HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F`).
    static HALL_OF_FAME: Bypass = Bypass {
        issue: "game-clear",
        features: &["SPECIAL_GameClear"],
        set: &[FLAG_IS_CHAMPION, FLAG_SYS_GAME_CLEAR],
        warp: Some((MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F, 4, 2)),
        ..Bypass::NONE
    };

    /// A Full Restore from the bag on the lead, between two Elite Four rooms.
    const FULL_RESTORE: Step = Step::UseItem(UseItem {
        item: ITEM_FULL_RESTORE,
        mon: 0,
        forget: 0,
    });

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

    /// Fly from the party menu (`FIELD_MOVE_FLY`, `FLDEFF_USE_FLY`, the region map): the
    /// engine has neither the field move from the party menu (Milestone 10) nor a party that
    /// could carry it (nothing in Swampert's or Castform's learnsets; catching is the bag in
    /// battle, Milestone 9). The bypass lands where Gen3's Fly does, on the city's heal
    /// location (`heal_locations.json`).
    const fn fly(issue: &'static str, map: MapId, x: i16, y: i16) -> Bypass {
        Bypass {
            issue,
            features: &[
                "FIELD_MOVE_FLY",
                "FLDEFF_USE_FLY",
                "party menu: Fly",
                "catching (a mon that learns Fly)",
            ],
            warp: Some((map, x, y)),
            ..Bypass::NONE
        }
    }

    /// HM04 from the bag on the lead, as [`USE_HM06`], for the Magma Hideout's boulders:
    /// forgetting Muddy Water (slot 0), which Surf outdoes, over Earthquake (slot 1, learned at
    /// 52 over Take Down), the lead's one non-Water attack for the League.
    const USE_HM04: Step = Step::UseItem(UseItem {
        item: ITEM_HM04,
        mon: 0,
        forget: 0,
    });

    /// Route 120's ledges are one-way south: back from Mt. Pyre to the Jagged Pass by Fly.
    static FLY_MAUVILLE: Bypass = fly("fly-mauville", MAP_MAUVILLE_CITY, 22, 6);
    /// From the Jagged Pass (Groudon awake) to Slateport's harbor, where Archie takes the
    /// submarine.
    static FLY_SLATEPORT: Bypass = fly("fly-slateport", MAP_SLATEPORT_CITY, 19, 20);
    /// From Slateport to Lilycove's Aqua Hideout.
    static FLY_LILYCOVE: Bypass = fly("fly-lilycove", MAP_LILYCOVE_CITY, 24, 15);

    /// Mossdeep's gym: the trick-house statues block every way to Tate and Liza until the
    /// floor switches turn them (`MossdeepCity_Gym_EventScript_*FloorSwitch`: the rotating
    /// tile puzzle's commands are unsupported). The bypass lands in front of the twins, where
    /// the gym's warp pads end.
    static MOSSDEEP_GYM_PUZZLE: Bypass = Bypass {
        issue: "mossdeep-gym-puzzle",
        features: &[
            "initrotatingtilepuzzle",
            "moverotatingtileobjects",
            "turnrotatingtileobjects",
            "freerotatingtilepuzzle",
        ],
        warp: Some((MAP_MOSSDEEP_CITY_GYM, 23, 8)),
        ..Bypass::NONE
    };

    /// One training round on Route 120's grass by Fortree, healed at Fortree's Center.
    const fn fortree_train(level: u8) -> [Step; 6] {
        [
            cross(MAP_FORTREE_CITY, 39, 8, Right),
            train_fresh(MAP_ROUTE120, 24, 8, level),
            cross(MAP_ROUTE120, 0, 8, Left),
            enter(MAP_FORTREE_CITY, 5, 6, Up),
            use_(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
            enter(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        ]
    }
    const FT47: [Step; 6] = fortree_train(47);
    const FT48: [Step; 6] = fortree_train(48);
    const FT49: [Step; 6] = fortree_train(49);

    /// One training round on the water west of Mossdeep: Surf from (10, 10), pace on (8, 10)
    /// to `level` (stopping at half HP or low PP), back ashore, healed at the Center.
    const fn mossdeep_train(level: u8) -> [Step; 9] {
        [
            goto(MAP_MOSSDEEP_CITY, 10, 10),
            Step::Choose(0),
            use_(MAP_MOSSDEEP_CITY, 10, 10, Left),
            train_fresh(MAP_MOSSDEEP_CITY, 8, 10, level),
            goto(MAP_MOSSDEEP_CITY, 9, 10),
            Step::Walk(Right),
            enter(MAP_MOSSDEEP_CITY, 28, 16, Up),
            use_(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 4, Up),
            enter(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        ]
    }
    // Rounds to these levels (a round stops short at half HP; one past its level is cheap).
    const MT53: [Step; 9] = mossdeep_train(53);
    const MT55: [Step; 9] = mossdeep_train(55);
    const MT57: [Step; 9] = mossdeep_train(57);
    const MT58: [Step; 9] = mossdeep_train(58);

    /// HM08 from the bag on the Route 122 Tentacool (party slot 2, after Castform), forgetting
    /// Poison Sting (slot 0): the lead's four slots are taken (three HMs and Earthquake).
    const USE_HM08: Step = Step::UseItem(UseItem {
        item: ITEM_HM08,
        mon: 2,
        forget: 0,
    });
    /// HM07 on the Tentacool too, forgetting Constrict (slot 2).
    const USE_HM07: Step = Step::UseItem(UseItem {
        item: ITEM_HM07,
        mon: 2,
        forget: 2,
    });

    /// B (Gen3 `TrySetupDiveEmergeScript`: on an underwater map, surface).
    const PRESS_B: Step = Step::Press(engine::Buttons::B.0, 1);

    /// Sootopolis after the dive in: its `ON_TRANSITION` (`SootopolisCity_EventScript_SetLayout`)
    /// put `LAYOUT_SOOTOPOLIS_CITY_LEGENDS_BATTLE` in, which the checkpoint does not keep (as
    /// [`SKY_PILLAR`]): the explorer's round trip diverges on the lake (a restored world surfs
    /// the city's own layout, pond water for ocean) and the planner reads the city's own
    /// layout. The bypass restores the position as the checkpoint has it (the own layout).
    static SOOTOPOLIS_LAYOUT: Bypass = Bypass {
        issue: "sootopolis-layout",
        features: &["setmaplayoutindex"],
        ..Bypass::NONE
    };

    /// Sootopolis Gym 1F's thin ice (`MB_THIN_ICE`, `STEP_CB_SOOTOPOLIS_ICE`): a path over
    /// every cell of a room once, from the cell above its entrance to the one below its stairs.
    /// Room 1: (8, 19) to (8, 17).
    const GYM_ICE_1: &[Dir] = &[Left, Up, Right, Right, Up, Left];
    /// Room 2: (8, 14) to (8, 12).
    const GYM_ICE_2: &[Dir] = &[
        Right, Right, Right, Up, Up, Left, Left, Down, Left, Left, Down, Left, Left, Up, Up, Right,
        Right, Right,
    ];
    /// Room 3: (8, 9) to (8, 6).
    const GYM_ICE_3: &[Dir] = &[
        Left, Left, Up, Left, Down, Left, Left, Up, Up, Up, Right, Down, Right, Right, Up, Right,
        Down, Right, Down, Right, Down, Right, Up, Up, Right, Down, Right, Down, Right, Up, Up, Up,
        Left, Left, Left, Left, Left,
    ];

    /// Route 131 and the Sky Pillar: Route 131 (`LAYOUT_ROUTE131_SKY_PILLAR`) and every floor
    /// (`LAYOUT_SKY_PILLAR_*_CLEAN`) are under a `setmaplayoutindex` layout, which the checkpoint
    /// does not keep (`Field::from_snapshot`'s TODO(schema 5); `World::hash` leaves it out
    /// too): the explorer's round trip diverges there (a restored world walks the map's own
    /// layout), and the planner reads the map's own layout. The bypass writes what Wallace's
    /// door scene (`SkyPillar_Outside_EventScript_WallaceScene`) and Rayquaza's waking
    /// (`SkyPillar_Top_EventScript_AwakenRayquaza`) leave, and lands on Route 126's dive spot
    /// (surfing), where the way back into Sootopolis starts.
    static SKY_PILLAR: Bypass = Bypass {
        issue: "sky-pillar-layouts",
        features: &["setmaplayoutindex"],
        set: &[
            FLAG_LANDMARK_SKY_PILLAR,
            FLAG_SYS_WEATHER_CTRL,
            FLAG_HIDE_SKY_PILLAR_WALLACE,
            FLAG_HIDE_SKY_PILLAR_TOP_RAYQUAZA,
        ],
        clear: &[FLAG_HIDE_SOOTOPOLIS_CITY_WALLACE],
        vars: &[
            (VAR_SOOTOPOLIS_CITY_STATE, 5),
            (VAR_SKY_PILLAR_STATE, 1),
            (VAR_SKY_PILLAR_RAYQUAZA_CRY_DONE, 1),
        ],
        warp: Some((MAP_ROUTE126, 45, 66)),
        ..Bypass::NONE
    };

    /// Rayquaza parting Groudon and Kyogre on the dive back in
    /// (`SootopolisCity_EventScript_RayquazaSceneFromDive`): `SPECIAL_Script_DoRayquazaScene`
    /// again, as [`LEGENDARIES_SCENE`]. The bypass writes the scene's end.
    static RAYQUAZA_SCENE: Bypass = Bypass {
        issue: "rayquaza-scene",
        features: &["SPECIAL_Script_DoRayquazaScene"],
        set: &[
            FLAG_HIDE_SOOTOPOLIS_CITY_GROUDON,
            FLAG_HIDE_SOOTOPOLIS_CITY_KYOGRE,
            FLAG_HIDE_SOOTOPOLIS_CITY_RAYQUAZA,
        ],
        clear: &[FLAG_SYS_WEATHER_CTRL, FLAG_LEGENDARIES_IN_SOOTOPOLIS],
        vars: &[(VAR_SKY_PILLAR_STATE, 2)],
        ..Bypass::NONE
    };

    /// Sootopolis's Groudon/Kyogre scene: `SPECIAL_Script_DoRayquazaScene` (the fight's own
    /// screen, which fades back in) is unsupported, so the field stays faded out. The bypass
    /// writes the scene's end (`VAR_SOOTOPOLIS_CITY_STATE` 2).
    static LEGENDARIES_SCENE: Bypass = Bypass {
        issue: "legendaries-scene",
        features: &["SPECIAL_Script_DoRayquazaScene"],
        vars: &[(VAR_SOOTOPOLIS_CITY_STATE, 2)],
        ..Bypass::NONE
    };

    /// Steven's multi battle against Maxie and Tabitha
    /// (`MossdeepCity_SpaceCenter_2F_EventScript_ChoosePartyForMultiBattle`): its specials are
    /// unsupported, the script goes on as if won, and the objects stay frozen (Gen3 reloads the
    /// field after the battle). The bypass warps onto the player's cell, which reloads it.
    static STEVEN_MULTI: Bypass = Bypass {
        issue: "steven-multi",
        features: &[
            "SPECIAL_ChooseHalfPartyForBattle",
            "SPECIAL_ReducePlayerPartyToSelectedMons",
            "SPECIAL_DoSpecialTrainerBattle",
            "FRONTIER_UTIL_FUNC_SAVE_PARTY",
        ],
        warp: Some((MAP_MOSSDEEP_CITY_SPACE_CENTER_2F, 1, 7)),
        ..Bypass::NONE
    };

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
        // Route 111's west trainers leave the lead hurt and paralysed (since field poison
        // drains HP, `DoPoisonFieldEffect`, the battles before differ), short for Route 113:
        // back to the rest stop (its trainers beaten; via (21, 10): the direct walk sticks at
        // (21, 11)), then west again.
        cross(MAP_ROUTE113, 99, 8, Right),
        goto(MAP_ROUTE111, 21, 10),
        goto(MAP_ROUTE111, 28, 11),
        goto(MAP_ROUTE111, 29, 17),
        enter(MAP_ROUTE111, 26, 18, Up),
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
        // The lead fainted to the rival's Grovyle (Castform won): back to the Institute's bed,
        // so that branches from `hm_fly` do not white out at their first wild battle.
        Step::Battle(BattlePlan::RunWild),
        Step::GotoAvoiding {
            map: MAP_ROUTE119,
            x: 6,
            y: 33,
            avoid: ROUTE119_SIGHT,
        },
        enter(MAP_ROUTE119, 6, 32, Up),
        goto(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 2, 3),
        use_(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 2, 3, Left),
        Step::WaitScript,
        enter(MAP_ROUTE119_WEATHER_INSTITUTE_1F, 9, 12, Down),
        Step::Checkpoint("hm_fly"),
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
        // ================= Act 6: Steven's Devon Scope on Route 120, Winona =================
        enter(MAP_FORTREE_CITY, 5, 6, Up),
        use_(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_FORTREE_CITY, 39, 8, Right),
        // Steven on the bridge (`Route120_EventScript_Steven`: ready, yes), the Kecleon.
        Step::Choose(0),
        talk(MAP_ROUTE120, 31),
        Step::Expect("devon_scope"),
        Step::Checkpoint("devon_scope"),
        // ---- Back to Fortree: the Devon Scope shows the Kecleon before the gym (yes) ----
        cross(MAP_ROUTE120, 0, 8, Left),
        // `Talk` finds no path to the invisible Kecleon at the foot of the
        // walkway ladder that leads down to the gym: `Use` from the ladder.
        Step::Choose(0),
        use_(MAP_FORTREE_CITY, 25, 7, Down),
        Step::Expect("kecleon_fled"),
        enter(MAP_FORTREE_CITY, 5, 6, Up),
        use_(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // Winona's Altaria beat the lead at 46: rounds on Route 120's grass east of the city.
        FT47[0],
        FT47[1],
        FT47[2],
        FT47[3],
        FT47[4],
        FT47[5],
        FT48[0],
        FT48[1],
        FT48[2],
        FT48[3],
        FT48[4],
        FT48[5],
        FT49[0],
        FT49[1],
        FT49[2],
        FT49[3],
        FT49[4],
        FT49[5],
        // ---- Winona ----
        enter(MAP_FORTREE_CITY, 22, 11, Up),
        talk(MAP_FORTREE_CITY_GYM, 1),
        Step::Expect("winona_defeated"),
        Step::Expect("badge6"),
        Step::Checkpoint("badge6"),
        // ================= Act 7: Route 120 south, Route 121, Lilycove; Mt. Pyre =================
        enter(MAP_FORTREE_CITY_GYM, 15, 24, Down),
        enter(MAP_FORTREE_CITY, 5, 6, Up),
        use_(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_FORTREE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_FORTREE_CITY, 39, 8, Right),
        cross(MAP_ROUTE120, 39, 89, Right),
        // Aqua's grunts block the way east until the player steps on their trigger
        // (`Route121_EventScript_AquaGruntsMoveOut`, x 25).
        goto(MAP_ROUTE121, 25, 6),
        Step::WaitScript,
        cross(MAP_ROUTE121, 79, 6, Right),
        Step::Expect("lilycove"),
        Step::Checkpoint("lilycove"),
        enter(MAP_LILYCOVE_CITY, 24, 14, Up),
        use_(MAP_LILYCOVE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_LILYCOVE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // ---- Back west to Route 121's pier, Surf south over Route 122 to Mt. Pyre ----
        cross(MAP_LILYCOVE_CITY, 0, 16, Left),
        // Round the twins Kate and Joy's sight (a double battle, below).
        Step::GotoAvoiding {
            map: MAP_ROUTE121,
            x: 30,
            y: 18,
            avoid: &[(39, 10), (40, 10)],
        },
        Step::Choose(0),
        use_(MAP_ROUTE121, 30, 18, Down),
        cross(MAP_ROUTE121, 30, 19, Down),
        // Off the water onto Mt. Pyre's island (the planner surfs water only).
        goto(MAP_ROUTE122, 22, 32),
        Step::Walk(Up),
        enter(MAP_ROUTE122, 22, 29, Up),
        enter(MAP_MT_PYRE_1F, 3, 6, Down),
        enter(MAP_MT_PYRE_EXTERIOR, 19, 10, Up),
        // The summit: Aqua's grunts by sight, then Archie takes the orb
        // (`MtPyre_Summit_EventScript_TeamAquaTrigger*`) and the old lady gives the Magma Emblem.
        goto(MAP_MT_PYRE_SUMMIT, 23, 7),
        Step::WaitScript,
        Step::Expect("magma_emblem"),
        Step::Checkpoint("magma_emblem"),
        // ---- The Magma Hideout, on the Jagged Pass: back by Surf to Route 121 and
        // Lilycove (a heal), Fly to Mauville, Route 111, the cable car to Mt. Chimney, down
        // the pass ----
        enter(MAP_MT_PYRE_SUMMIT, 23, 31, Down),
        enter(MAP_MT_PYRE_EXTERIOR, 10, 42, Up),
        enter(MAP_MT_PYRE_1F, 17, 18, Down),
        Step::Choose(0),
        use_(MAP_ROUTE122, 22, 31, Down),
        // Two catches on Route 122 with the bag's balls (the battle's degraded BAG): a
        // Tentacool for Dive and Waterfall, a Wingull for Fly (the lead's four slots end up
        // Strength, Earthquake, Rock Smash and Surf; Castform learns none of them).
        Step::Catch {
            map: MAP_ROUTE122,
            x: 22,
            y: 33,
            species: SPECIES_TENTACOOL,
        },
        Step::Catch {
            map: MAP_ROUTE122,
            x: 22,
            y: 33,
            species: SPECIES_WINGULL,
        },
        cross(MAP_ROUTE122, 10, 0, Up),
        Step::Walk(Up),
        // The summit's grunts leave the lead weak: Lilycove's Center first.
        Step::GotoAvoiding {
            map: MAP_ROUTE121,
            x: 78,
            y: 6,
            avoid: &[(39, 10), (40, 10)],
        },
        cross(MAP_ROUTE121, 79, 6, Right),
        enter(MAP_LILYCOVE_CITY, 24, 14, Up),
        use_(MAP_LILYCOVE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_LILYCOVE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // Route 120 is one-way south (its ledges): the way back is Fly (`fly-mauville`).
        Step::Bypass(&FLY_MAUVILLE),
        enter(MAP_MAUVILLE_CITY, 22, 5, Up),
        use_(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MAUVILLE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        cross(MAP_MAUVILLE_CITY, 16, 0, Up),
        Step::Choose(0),
        talk(MAP_ROUTE111, 15),
        cross(MAP_ROUTE111, 0, 68, Left),
        enter(MAP_ROUTE112, 28, 27, Up),
        talk(MAP_ROUTE112_CABLE_CAR_STATION, 1),
        Step::WaitScript,
        enter(MAP_MT_CHIMNEY_CABLE_CAR_STATION, 6, 11, Down),
        enter(MAP_MT_CHIMNEY, 20, 41, Down),
        // The emblem opens the hideout (`JaggedPass_EventScript_OpenMagmaHideout`).
        goto(MAP_JAGGED_PASS, 14, 15),
        Step::WaitScript,
        enter(MAP_JAGGED_PASS, 16, 18, Up),
        // ---- The hideout: 1F, 2F 1R, 3F 1R, 4F down its arrow warps; Maxie wakes Groudon ----
        // Three boulders close the way north (`EventScript_StrengthBoulder`): Strength from
        // the bag, then (6, 23) west, up, and (5, 22) west.
        USE_HM04,
        Step::Choose(0),
        use_(MAP_MAGMA_HIDEOUT_1F, 7, 23, Left),
        // (A `Walk` into a boulder pushes it, then steps after it.)
        Step::Walk(Left),
        Step::Walk(Up),
        Step::Walk(Left),
        Step::Walk(Up),
        enter(MAP_MAGMA_HIDEOUT_1F, 25, 34, Down),
        enter(MAP_MAGMA_HIDEOUT_2F_1R, 17, 33, Down),
        enter(MAP_MAGMA_HIDEOUT_3F_1R, 7, 21, Down),
        talk(MAP_MAGMA_HIDEOUT_4F, 6),
        Step::WaitScript,
        Step::Expect("groudon_awakened"),
        Step::Checkpoint("magma_hideout"),
        // ---- Out the way in: 4F, 3F, 2F, 1F (the boulders again, from the north: (5, 22)
        // down, (7, 22) east), the Jagged Pass; the wild battles run (a Geodude's
        // Self-Destruct finished the hurt lead) ----
        Step::Battle(BattlePlan::RunWild),
        enter(MAP_MAGMA_HIDEOUT_4F, 46, 7, Up),
        enter(MAP_MAGMA_HIDEOUT_3F_1R, 23, 3, Up),
        enter(MAP_MAGMA_HIDEOUT_2F_1R, 8, 2, Up),
        Step::Choose(0),
        use_(MAP_MAGMA_HIDEOUT_1F, 5, 21, Down),
        Step::Walk(Down),
        Step::Walk(Right),
        Step::Walk(Right),
        Step::Walk(Down),
        enter(MAP_MAGMA_HIDEOUT_1F, 10, 34, Down),
        // ---- Slateport's harbor: Archie takes Stern's submarine ----
        Step::Bypass(&FLY_SLATEPORT),
        enter(MAP_SLATEPORT_CITY, 19, 19, Up),
        use_(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SLATEPORT_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        // Capt. Stern's interview in front of the harbor (`SlateportCity_EventScript_CaptStern`)
        // ends with a warp into it.
        use_(MAP_SLATEPORT_CITY, 27, 13, Right),
        goto(MAP_SLATEPORT_CITY_HARBOR, 8, 13),
        Step::WaitScript,
        Step::Checkpoint("submarine_stolen"),
        // ---- Lilycove: Surf east to the Aqua Hideout's water door ----
        enter(MAP_SLATEPORT_CITY_HARBOR, 11, 14, Down),
        Step::Bypass(&FLY_LILYCOVE),
        enter(MAP_LILYCOVE_CITY, 24, 14, Up),
        use_(MAP_LILYCOVE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_LILYCOVE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        Step::Choose(0),
        goto(MAP_LILYCOVE_CITY, 50, 8),
        Step::Walk(Down),
        Step::Walk(Down),
        use_(MAP_LILYCOVE_CITY, 63, 14, Right),
        // The water door (`MB_WATER_DOOR`, (70, 5)) warps the surfer onto the hideout pool's
        // south arrow warp (13, 27), still surfing; north across the pool to its shore.
        enter(MAP_LILYCOVE_CITY, 70, 5, Up),
        goto(MAP_AQUA_HIDEOUT_1F, 13, 13),
        Step::Walk(Up),
        // ---- The Aqua Hideout: 1F, B1F's and B2F's warp pads to Matt ----
        enter(MAP_AQUA_HIDEOUT_1F, 22, 1, Up),
        enter(MAP_AQUA_HIDEOUT_B1F, 27, 4, Up),
        enter(MAP_AQUA_HIDEOUT_B1F, 32, 19, Up),
        enter(MAP_AQUA_HIDEOUT_B1F, 18, 1, Up),
        enter(MAP_AQUA_HIDEOUT_B2F, 31, 8, Up),
        enter(MAP_AQUA_HIDEOUT_B2F, 3, 3, Up),
        enter(MAP_AQUA_HIDEOUT_B1F, 12, 1, Up),
        enter(MAP_AQUA_HIDEOUT_B2F, 8, 8, Up),
        // Matt notices the player on (28, 17) (`AquaHideout_B2F_EventScript_MattNoticePlayer`).
        goto(MAP_AQUA_HIDEOUT_B2F, 28, 17),
        Step::WaitScript,
        // (`Talk` finds no path to him: an explorer gap, the same as Stern's.)
        use_(MAP_AQUA_HIDEOUT_B2F, 24, 19, Left),
        Step::WaitScript,
        Step::Expect("aqua_submarine"),
        Step::Checkpoint("aqua_hideout"),
        // ---- Out by B2F's east pad, back across the pool: its arrow warp (13, 27) south
        // to Lilycove's water door, surfing ----
        enter(MAP_AQUA_HIDEOUT_B2F, 32, 20, Up),
        enter(MAP_AQUA_HIDEOUT_B1F, 29, 1, Up),
        Step::Choose(0),
        use_(MAP_AQUA_HIDEOUT_1F, 13, 12, Down),
        enter(MAP_AQUA_HIDEOUT_1F, 13, 27, Down),
        // ---- Route 124 east to Mossdeep ----
        cross(MAP_LILYCOVE_CITY, 79, 12, Right),
        cross(MAP_ROUTE124, 79, 50, Right),
        goto(MAP_MOSSDEEP_CITY, 9, 10),
        Step::Walk(Right),
        // `MossdeepCity_EventScript_VisitedMossdeep` (coord events) sets the visited flag.
        goto(MAP_MOSSDEEP_CITY, 25, 25),
        Step::WaitScript,
        Step::Expect("mossdeep"),
        enter(MAP_MOSSDEEP_CITY, 28, 16, Up),
        use_(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        Step::Checkpoint("mossdeep"),
        // Tate and Liza's four (the double degrades to singles: `Degraded("double battle")`)
        // beat the lead alone at 52 (Solrock's SolarBeam in the sun): it trains on the water
        // west of the city, in rounds healed at the Center (fighting the wild battles again).
        Step::Battle(BattlePlan::BestLearn),
        MT53[0],
        MT53[1],
        MT53[2],
        MT53[3],
        MT53[4],
        MT53[5],
        MT53[6],
        MT53[7],
        MT53[8],
        MT53[0],
        MT53[1],
        MT53[2],
        MT53[3],
        MT53[4],
        MT53[5],
        MT53[6],
        MT53[7],
        MT53[8],
        MT55[0],
        MT55[1],
        MT55[2],
        MT55[3],
        MT55[4],
        MT55[5],
        MT55[6],
        MT55[7],
        MT55[8],
        MT55[0],
        MT55[1],
        MT55[2],
        MT55[3],
        MT55[4],
        MT55[5],
        MT55[6],
        MT55[7],
        MT55[8],
        MT57[0],
        MT57[1],
        MT57[2],
        MT57[3],
        MT57[4],
        MT57[5],
        MT57[6],
        MT57[7],
        MT57[8],
        MT57[0],
        MT57[1],
        MT57[2],
        MT57[3],
        MT57[4],
        MT57[5],
        MT57[6],
        MT57[7],
        MT57[8],
        MT58[0],
        MT58[1],
        MT58[2],
        MT58[3],
        MT58[4],
        MT58[5],
        MT58[6],
        MT58[7],
        MT58[8],
        MT58[0],
        MT58[1],
        MT58[2],
        MT58[3],
        MT58[4],
        MT58[5],
        MT58[6],
        MT58[7],
        MT58[8],
        enter(MAP_MOSSDEEP_CITY, 38, 9, Up),
        // The gym's statues move with its floor switches (`initrotatingtilepuzzle`, ...):
        // `mossdeep-gym-puzzle` lands in front of the twins.
        Step::Bypass(&MOSSDEEP_GYM_PUZZLE),
        talk(MAP_MOSSDEEP_CITY_GYM, 1),
        Step::WaitScript,
        Step::Expect("tate_liza_defeated"),
        Step::Expect("badge7"),
        Step::Checkpoint("badge7"),
        // ---- The Space Center: out of the gym by its exit pad (21, 6), Maxie's men go in ----
        goto(MAP_MOSSDEEP_CITY_GYM, 21, 7),
        Step::Walk(Up),
        Step::WaitScript,
        enter(MAP_MOSSDEEP_CITY_GYM, 7, 35, Down),
        enter(MAP_MOSSDEEP_CITY, 28, 16, Up),
        use_(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        goto(MAP_MOSSDEEP_CITY, 41, 22),
        Step::WaitScript,
        enter(MAP_MOSSDEEP_CITY, 64, 15, Up),
        // The stair grunt; a grunt's sight interrupts the first walk to him.
        talk(MAP_MOSSDEEP_CITY_SPACE_CENTER_1F, 9),
        Step::WaitScript,
        talk(MAP_MOSSDEEP_CITY_SPACE_CENTER_1F, 9),
        Step::WaitScript,
        // 2F: three grunts in a row ("take us on?" yes), then Steven.
        Step::Choose(0),
        enter(MAP_MOSSDEEP_CITY_SPACE_CENTER_1F, 13, 1, Up),
        Step::WaitScript,
        talk(MAP_MOSSDEEP_CITY_SPACE_CENTER_2F, 4),
        Step::WaitScript,
        Step::Choose(0),
        talk(MAP_MOSSDEEP_CITY_SPACE_CENTER_2F, 4),
        Step::WaitScript,
        Step::Expect("space_center"),
        // The multi battle with Steven is unsupported (reported, skipped as won); the scene's
        // `lockall` then never ends (Gen3's battle return reloads the field): `steven-multi`
        // reloads the room.
        Step::Bypass(&STEVEN_MULTI),
        Step::Checkpoint("space_center"),
        // ---- Steven's house: HM08 (`MossdeepCity_StevensHouse_EventScript_StevenGivesDive`)
        enter(MAP_MOSSDEEP_CITY_SPACE_CENTER_2F, 13, 1, Up),
        enter(MAP_MOSSDEEP_CITY_SPACE_CENTER_1F, 7, 9, Down),
        enter(MAP_MOSSDEEP_CITY, 19, 10, Up),
        Step::WaitScript,
        Step::Expect("hm_dive"),
        Step::Checkpoint("hm_dive"),
        USE_HM08,
        // ---- Act 8: Route 127, 128 south; Dive to the Seafloor Cavern: down from Route 128's
        // deep water (Gen3 `TrySetupDiveDownScript`, the dive connection keeps the cell),
        // Underwater Route 128's tunnel (38, 26), up in the cavern's pool (its fixed dive warp,
        // (10, 17)) ----
        enter(MAP_MOSSDEEP_CITY_STEVENS_HOUSE, 3, 7, Down),
        enter(MAP_MOSSDEEP_CITY, 28, 16, Up),
        use_(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_MOSSDEEP_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        Step::Battle(BattlePlan::RunWild),
        goto(MAP_MOSSDEEP_CITY, 23, 30),
        Step::Choose(0),
        use_(MAP_MOSSDEEP_CITY, 23, 30, Down),
        cross(MAP_MOSSDEEP_CITY, 30, 39, Down),
        cross(MAP_ROUTE127, 30, 79, Down),
        goto(MAP_ROUTE128, 38, 27),
        Step::Choose(0),
        Step::Interact,
        enter(MAP_UNDERWATER_ROUTE128, 38, 26, Up),
        Step::Choose(0),
        PRESS_B,
        goto(MAP_SEAFLOOR_CAVERN_ENTRANCE, 10, 11),
        Step::Walk(Up),
        enter(MAP_SEAFLOOR_CAVERN_ENTRANCE, 10, 1, Up),
        // ---- The cavern: Room 1, 2, 7, 3 (Shelly), 8, 9 (Archie wakes Kyogre) ----
        // Room 1: smash (5, 10), push (5, 11) down and (12, 11) east.
        Step::Choose(0),
        use_(MAP_SEAFLOOR_CAVERN_ROOM1, 4, 10, Right),
        Step::Choose(0),
        use_(MAP_SEAFLOOR_CAVERN_ROOM1, 5, 10, Down),
        Step::Walk(Down),
        goto(MAP_SEAFLOOR_CAVERN_ROOM1, 11, 11),
        Step::Walk(Right),
        enter(MAP_SEAFLOOR_CAVERN_ROOM1, 6, 2, Up),
        // Room 2: smash (8, 7), push (4, 5) up thrice, smash (9, 3), push (10, 3) east twice.
        Step::Choose(0),
        use_(MAP_SEAFLOOR_CAVERN_ROOM2, 9, 7, Left),
        Step::Choose(0),
        use_(MAP_SEAFLOOR_CAVERN_ROOM2, 4, 6, Up),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Choose(0),
        use_(MAP_SEAFLOOR_CAVERN_ROOM2, 8, 3, Right),
        Step::Walk(Right),
        Step::Walk(Right),
        Step::Walk(Right),
        enter(MAP_SEAFLOOR_CAVERN_ROOM2, 11, 1, Up),
        // Room 7: Surf across its pool of currents (the planner does not model them: a step
        // onto (4, 18) slides west to (3, 18), then the still column 2 north).
        Step::Choose(0),
        use_(MAP_SEAFLOOR_CAVERN_ROOM7, 3, 19, Right),
        Step::Walk(Up),
        Step::Wait(60),
        Step::Walk(Left),
        goto(MAP_SEAFLOOR_CAVERN_ROOM7, 2, 4),
        goto(MAP_SEAFLOOR_CAVERN_ROOM7, 4, 4),
        Step::Walk(Up),
        enter(MAP_SEAFLOOR_CAVERN_ROOM7, 5, 1, Up),
        enter(MAP_SEAFLOOR_CAVERN_ROOM3, 8, 1, Up),
        // Room 8's boulder block (a search's solution).
        goto(MAP_SEAFLOOR_CAVERN_ROOM8, 5, 8),
        Step::Choose(0),
        use_(MAP_SEAFLOOR_CAVERN_ROOM8, 5, 8, Up),
        Step::Walk(Up),
        Step::Walk(Left),
        goto(MAP_SEAFLOOR_CAVERN_ROOM8, 4, 6),
        Step::Walk(Right),
        Step::Walk(Up),
        Step::Walk(Left),
        Step::Walk(Up),
        goto(MAP_SEAFLOOR_CAVERN_ROOM8, 5, 5),
        Step::Walk(Right),
        Step::Walk(Up),
        Step::Walk(Left),
        enter(MAP_SEAFLOOR_CAVERN_ROOM8, 5, 2, Up),
        // Archie's scene (coord (17, 42)) ends with a warp up to Route 128.
        goto(MAP_SEAFLOOR_CAVERN_ROOM9, 16, 42),
        Step::Walk(Right),
        Step::WaitScript,
        Step::Expect("kyogre_awakened"),
        Step::Checkpoint("seafloor_cavern"),
        // ---- Sootopolis: Surf off Route 128's islet, Route 127 north, Route 126 west; Dive
        // under the crater's wall (Underwater Route 126's tunnel (45, 65)) and up in its lake
        // (Underwater Sootopolis's fixed dive warp, (29, 53)) ----
        Step::Choose(0),
        use_(MAP_ROUTE128, 42, 22, Right),
        cross(MAP_ROUTE128, 30, 0, Up),
        cross(MAP_ROUTE127, 0, 40, Left),
        goto(MAP_ROUTE126, 45, 66),
        // Groudon and Kyogre on arrival (`SootopolisCity_EventScript_LegendariesSceneFromDive`):
        // its `Script_DoRayquazaScene` is unsupported and leaves the screen black, so
        // `legendaries-scene` writes the scene's end before the dive lands.
        Step::Bypass(&LEGENDARIES_SCENE),
        Step::Choose(0),
        Step::Interact,
        enter(MAP_UNDERWATER_ROUTE126, 45, 65, Up),
        Step::Choose(0),
        PRESS_B,
        Step::Expect("sootopolis"),
        // The city is `LAYOUT_SOOTOPOLIS_CITY_LEGENDS_BATTLE` now (`setmaplayoutindex`), which
        // the checkpoint does not keep: `sootopolis-layout`.
        Step::Bypass(&SOOTOPOLIS_LAYOUT),
        goto(MAP_SOOTOPOLIS_CITY, 23, 37),
        Step::Walk(Left),
        // Steven leads to the Cave of Origin; Wallace in B1F asks where Rayquaza is: Sky
        // Pillar (the multichoice's third answer).
        talk(MAP_SOOTOPOLIS_CITY, 7),
        Step::WaitScript,
        enter(MAP_CAVE_OF_ORIGIN_ENTRANCE, 9, 5, Up),
        enter(MAP_CAVE_OF_ORIGIN_1F, 14, 5, Up),
        Step::Choose(2),
        talk(MAP_CAVE_OF_ORIGIN_B1F, 1),
        Step::WaitScript,
        Step::Checkpoint("cave_of_origin"),
        // ---- Sky Pillar: back out of the cave, Surf off Sootopolis's shore and Dive out
        // (the lake's deep water, Underwater Sootopolis's tunnel), Routes 126-127-128-129 to Route 130; Route 131 and the
        // pillar are `sky-pillar-layouts` ----
        enter(MAP_CAVE_OF_ORIGIN_B1F, 9, 3, Up),
        enter(MAP_CAVE_OF_ORIGIN_1F, 11, 17, Down),
        enter(MAP_CAVE_OF_ORIGIN_ENTRANCE, 9, 20, Down),
        Step::Bypass(&SOOTOPOLIS_LAYOUT),
        goto(MAP_SOOTOPOLIS_CITY, 22, 37),
        Step::Choose(0),
        use_(MAP_SOOTOPOLIS_CITY, 22, 37, Right),
        goto(MAP_SOOTOPOLIS_CITY, 29, 54),
        Step::Choose(0),
        Step::Interact,
        enter(MAP_UNDERWATER_SOOTOPOLIS_CITY, 9, 8, Down),
        Step::Choose(0),
        PRESS_B,
        cross(MAP_ROUTE126, 79, 40, Right),
        cross(MAP_ROUTE127, 30, 79, Down),
        // East of Route 128's reef (its west water is cut off from 127 and 129's south).
        cross(MAP_ROUTE128, 40, 39, Down),
        cross(MAP_ROUTE129, 0, 23, Left),
        Step::Bypass(&SKY_PILLAR),
        Step::Expect("rayquaza_awakened"),
        Step::Bypass(&RAYQUAZA_SCENE),
        Step::Choose(0),
        Step::Interact,
        enter(MAP_UNDERWATER_ROUTE126, 45, 65, Up),
        Step::Choose(0),
        PRESS_B,
        Step::Checkpoint("rayquaza_awakened"),
        // ---- HM07: Maxie and Archie leave, Wallace gives Waterfall ----
        goto(MAP_SOOTOPOLIS_CITY, 32, 37),
        Step::Walk(Up),
        talk(MAP_SOOTOPOLIS_CITY, 16),
        Step::WaitScript,
        talk(MAP_SOOTOPOLIS_CITY, 17),
        Step::WaitScript,
        talk(MAP_SOOTOPOLIS_CITY, 18),
        Step::WaitScript,
        Step::Expect("hm_waterfall"),
        Step::Checkpoint("hm_waterfall"),
        USE_HM07,
        // ---- Juan: the gym's three thin-ice rooms, each crossed once per cell (a second
        // step falls through to B1F); each room's last cell opens its stairs ----
        // Healed first (the Center across the lake, by Surf).
        goto(MAP_SOOTOPOLIS_CITY, 34, 35),
        Step::Choose(0),
        use_(MAP_SOOTOPOLIS_CITY, 34, 35, Right),
        goto(MAP_SOOTOPOLIS_CITY, 39, 37),
        Step::Walk(Right),
        enter(MAP_SOOTOPOLIS_CITY, 43, 31, Up),
        use_(MAP_SOOTOPOLIS_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_SOOTOPOLIS_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        goto(MAP_SOOTOPOLIS_CITY, 40, 37),
        Step::Choose(0),
        use_(MAP_SOOTOPOLIS_CITY, 40, 37, Left),
        goto(MAP_SOOTOPOLIS_CITY, 35, 35),
        Step::Walk(Left),
        enter(MAP_SOOTOPOLIS_CITY, 31, 32, Up),
        goto(MAP_SOOTOPOLIS_CITY_GYM_1F, 8, 20),
        Step::Walk(Up),
        Step::Path {
            map: MAP_SOOTOPOLIS_CITY_GYM_1F,
            dirs: GYM_ICE_1,
        },
        Step::WaitScript,
        Step::Path {
            map: MAP_SOOTOPOLIS_CITY_GYM_1F,
            dirs: &[Up, Up, Up],
        },
        Step::Path {
            map: MAP_SOOTOPOLIS_CITY_GYM_1F,
            dirs: GYM_ICE_2,
        },
        Step::WaitScript,
        Step::Path {
            map: MAP_SOOTOPOLIS_CITY_GYM_1F,
            dirs: &[Up, Up, Up],
        },
        Step::Path {
            map: MAP_SOOTOPOLIS_CITY_GYM_1F,
            dirs: GYM_ICE_3,
        },
        Step::WaitScript,
        Step::Path {
            map: MAP_SOOTOPOLIS_CITY_GYM_1F,
            dirs: &[Up, Up, Up],
        },
        talk(MAP_SOOTOPOLIS_CITY_GYM_1F, 1),
        Step::WaitScript,
        Step::Expect("juan_defeated"),
        Step::Expect("badge8"),
        Step::Checkpoint("badge8"),
        // ---- Ever Grande: out of the gym, Surf and Dive out of the crater, Routes 126-127-128
        // east; up the waterfall ----
        // Back down onto the broken ice: a fall to B1F, whose ladder leads to the entrance.
        Step::Path {
            map: MAP_SOOTOPOLIS_CITY_GYM_1F,
            dirs: &[Down, Down, Down],
        },
        enter(MAP_SOOTOPOLIS_CITY_GYM_B1F, 11, 22, Right),
        enter(MAP_SOOTOPOLIS_CITY_GYM_1F, 8, 25, Down),
        goto(MAP_SOOTOPOLIS_CITY, 34, 35),
        Step::Choose(0),
        use_(MAP_SOOTOPOLIS_CITY, 34, 35, Right),
        goto(MAP_SOOTOPOLIS_CITY, 29, 54),
        Step::Choose(0),
        Step::Interact,
        enter(MAP_UNDERWATER_SOOTOPOLIS_CITY, 9, 8, Down),
        Step::Choose(0),
        PRESS_B,
        cross(MAP_ROUTE126, 79, 40, Right),
        cross(MAP_ROUTE127, 30, 79, Down),
        cross(MAP_ROUTE128, 119, 30, Right),
        // Up the falls (Gen3 `GetInteractedWaterScript`: `MB_WATERFALL` ahead, surfing north,
        // the eighth badge; `EventScript_UseWaterfall` yes, `FLDEFF_USE_WATERFALL`) to (20, 59).
        Step::Choose(0),
        use_(MAP_EVER_GRANDE_CITY, 20, 68, Up),
        Step::Walk(Up),
        Step::Expect("ever_grande"),
        goto(MAP_EVER_GRANDE_CITY, 20, 56),
        Step::Walk(Up),
        Step::Checkpoint("ever_grande"),
        // ---- Victory Road (healed first: the PP the routes since Juan spent) ----
        enter(MAP_EVER_GRANDE_CITY, 27, 48, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 41, Up),
        // The Max Elixir east of the entrance: the lead's PP for Wallace.
        talk(MAP_VICTORY_ROAD_1F, 5),
        Step::WaitScript,
        // A level on the cave floor, healed, before Wally: at 60 his Altaria's paralysis and
        // Roselia's Giga Drain can win (they did once field poison moved the RNG).
        Step::Battle(BattlePlan::BestLearn),
        train_fresh(MAP_VICTORY_ROAD_1F, 39, 26, 62),
        Step::Battle(BattlePlan::RunWild),
        enter(MAP_VICTORY_ROAD_1F, 15, 40, Down),
        enter(MAP_EVER_GRANDE_CITY, 27, 48, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_CENTER_1F, 7, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_CENTER_1F, 7, 8, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 41, Up),
        enter(MAP_VICTORY_ROAD_1F, 9, 14, Up),
        // B1F: Strength the boulder (4, 7) west and (9, 10) east, Rock Smash (18, 12), down
        // to B2F.
        goto(MAP_VICTORY_ROAD_B1F, 5, 7),
        Step::Choose(0),
        use_(MAP_VICTORY_ROAD_B1F, 5, 7, Left),
        Step::Walk(Left),
        goto(MAP_VICTORY_ROAD_B1F, 8, 10),
        Step::Walk(Right),
        goto(MAP_VICTORY_ROAD_B1F, 17, 12),
        Step::Choose(0),
        use_(MAP_VICTORY_ROAD_B1F, 17, 12, Right),
        enter(MAP_VICTORY_ROAD_B1F, 30, 25, Up),
        // B2F: Surf the east pool north, round the west, Surf down the falls (9, 11) and east
        // to the ladder (19, 12).
        goto(MAP_VICTORY_ROAD_B2F, 34, 17),
        Step::Choose(0),
        use_(MAP_VICTORY_ROAD_B2F, 34, 17, Up),
        goto(MAP_VICTORY_ROAD_B2F, 34, 14),
        Step::Walk(Up),
        goto(MAP_VICTORY_ROAD_B2F, 6, 10),
        Step::Choose(0),
        use_(MAP_VICTORY_ROAD_B2F, 6, 10, Right),
        Step::Path {
            map: MAP_VICTORY_ROAD_B2F,
            dirs: &[Right, Right, Down, Down, Down, Down],
        },
        goto(MAP_VICTORY_ROAD_B2F, 19, 14),
        Step::Walk(Up),
        enter(MAP_VICTORY_ROAD_B2F, 19, 12, Up),
        // B1F's south room: smash (20, 25), push (20, 26) down, smash (21, 26).
        goto(MAP_VICTORY_ROAD_B1F, 19, 25),
        Step::Choose(0),
        use_(MAP_VICTORY_ROAD_B1F, 19, 25, Right),
        goto(MAP_VICTORY_ROAD_B1F, 20, 25),
        Step::Choose(0),
        use_(MAP_VICTORY_ROAD_B1F, 20, 25, Down),
        Step::Walk(Down),
        Step::Choose(0),
        use_(MAP_VICTORY_ROAD_B1F, 20, 26, Right),
        enter(MAP_VICTORY_ROAD_B1F, 20, 21, Left),
        goto(MAP_VICTORY_ROAD_1F, 33, 28),
        goto(MAP_VICTORY_ROAD_1F, 36, 25),
        // North over the bridge's east cell (Quincy stands on (32, 17)).
        goto(MAP_VICTORY_ROAD_1F, 33, 19),
        goto(MAP_VICTORY_ROAD_1F, 33, 16),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        Step::Checkpoint("victory_road"),
        // ---- Training for the League on Victory Road's last cave (level-40 wild mons), healed
        // at the League's counter: the lead fights the Elite Four and Wallace alone, with its
        // one non-HM attack (Earthquake) and no items; to 98 (the League is a coin toss on PP:
        // since field poison moved the RNG, 96 and 97 lose to Wallace or Glacia) ----
        Step::Battle(BattlePlan::BestLearn),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 11, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 27, Up),
        train_fresh(MAP_VICTORY_ROAD_1F, 35, 4, 98),
        enter(MAP_VICTORY_ROAD_1F, 39, 5, Down),
        enter(MAP_EVER_GRANDE_CITY, 18, 5, Up),
        use_(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3, 4, Up),
        // ---- The League: past the badge guards; each Elite Four room walks the player in,
        // then the trainer's talk is the battle ----
        Step::Bypass(&LEAGUE_MART),
        talk(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 3),
        Step::WaitScript,
        enter(MAP_EVER_GRANDE_CITY_POKEMON_LEAGUE_1F, 9, 1, Up),
        enter(MAP_EVER_GRANDE_CITY_HALL5, 5, 2, Up),
        Step::WaitScript,
        talk(MAP_EVER_GRANDE_CITY_SIDNEYS_ROOM, 1),
        Step::WaitScript,
        Step::Expect("sidney_defeated"),
        FULL_RESTORE,
        enter(MAP_EVER_GRANDE_CITY_SIDNEYS_ROOM, 6, 2, Up),
        enter(MAP_EVER_GRANDE_CITY_HALL1, 5, 2, Up),
        Step::WaitScript,
        talk(MAP_EVER_GRANDE_CITY_PHOEBES_ROOM, 1),
        Step::WaitScript,
        Step::Expect("phoebe_defeated"),
        FULL_RESTORE,
        enter(MAP_EVER_GRANDE_CITY_PHOEBES_ROOM, 6, 2, Up),
        enter(MAP_EVER_GRANDE_CITY_HALL2, 5, 2, Up),
        Step::WaitScript,
        talk(MAP_EVER_GRANDE_CITY_GLACIAS_ROOM, 1),
        Step::WaitScript,
        Step::Expect("glacia_defeated"),
        FULL_RESTORE,
        enter(MAP_EVER_GRANDE_CITY_GLACIAS_ROOM, 6, 2, Up),
        enter(MAP_EVER_GRANDE_CITY_HALL3, 5, 2, Up),
        Step::WaitScript,
        talk(MAP_EVER_GRANDE_CITY_DRAKES_ROOM, 1),
        Step::WaitScript,
        Step::Expect("drake_defeated"),
        FULL_RESTORE,
        // The Max Elixir from Victory Road: four rooms leave the lead's PP near empty.
        Step::UseItem(UseItem {
            item: ITEM_MAX_ELIXIR,
            mon: 0,
            forget: 0,
        }),
        enter(MAP_EVER_GRANDE_CITY_DRAKES_ROOM, 6, 2, Up),
        // Wallace's room plays itself: the walk in, the battle, the walk to the Hall of Fame.
        enter(MAP_EVER_GRANDE_CITY_HALL4, 5, 2, Up),
        Step::WaitScript,
        Step::Expect("wallace_defeated"),
        Step::Bypass(&HALL_OF_FAME),
        Step::Expect("champion"),
        Step::Expect("hall_of_fame"),
        Step::Checkpoint("hall_of_fame"),
    ];
}
