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

    /// Blocker `new-game-map-flags`: the new game (`content::new_game`) sets 25 of the 159
    /// flags of `EventScript_ResetAllMapFlags` (`data/scripts/new_game.inc`), so objects hidden
    /// from the start show (Route 104's rival stands on Mr. Briney's door). The bypass sets the
    /// others that Act 1 left untouched, once, at the start of Act 2.
    static RESET_ALL_MAP_FLAGS: Bypass = Bypass {
        issue: "new-game-map-flags",
        set: &[
            FLAG_HIDE_CONTEST_POKE_BALL,
            FLAG_HIDE_ROUTE_111_VICTORIA_WINSTRATE,
            FLAG_HIDE_ROUTE_111_VIVI_WINSTRATE,
            FLAG_HIDE_ROUTE_111_VICKY_WINSTRATE,
            FLAG_UNKNOWN_0x363,
            FLAG_HIDE_RUSTBORO_CITY_AQUA_GRUNT,
            FLAG_HIDE_RUSTBORO_CITY_DEVON_EMPLOYEE_1,
            FLAG_HIDE_RUSTBORO_CITY_RIVAL,
            FLAG_HIDE_RUSTBORO_CITY_SCIENTIST,
            FLAG_HIDE_BRINEYS_HOUSE_MR_BRINEY,
            FLAG_HIDE_BRINEYS_HOUSE_PEEKO,
            FLAG_HIDE_ROUTE_104_MR_BRINEY,
            FLAG_HIDE_MR_BRINEY_DEWFORD_TOWN,
            FLAG_HIDE_ROUTE_109_MR_BRINEY,
            FLAG_HIDE_MR_BRINEY_BOAT_DEWFORD_TOWN,
            FLAG_HIDE_ROUTE_109_MR_BRINEY_BOAT,
            FLAG_HIDE_ROUTE_110_BIRCH,
            FLAG_HIDE_LILYCOVE_CONTEST_HALL_CONTEST_ATTENDANT_1,
            FLAG_HIDE_LILYCOVE_CONTEST_HALL_CONTEST_ATTENDANT_2,
            FLAG_HIDE_LILYCOVE_MUSEUM_PATRON_1,
            FLAG_HIDE_LILYCOVE_MUSEUM_PATRON_2,
            FLAG_HIDE_LILYCOVE_MUSEUM_PATRON_3,
            FLAG_HIDE_LILYCOVE_MUSEUM_PATRON_4,
            FLAG_HIDE_LILYCOVE_MUSEUM_TOURISTS,
            FLAG_HIDE_PETALBURG_GYM_GREETER,
            FLAG_HIDE_PETALBURG_GYM_WALLYS_DAD,
            FLAG_HIDE_FANCLUB_OLD_LADY,
            FLAG_HIDE_FANCLUB_BOY,
            FLAG_HIDE_FANCLUB_LITTLE_BOY,
            FLAG_HIDE_FANCLUB_LADY,
            FLAG_HIDE_LILYCOVE_FAN_CLUB_INTERVIEWER,
            FLAG_HIDE_ROUTE_118_GABBY_AND_TY_1,
            FLAG_HIDE_ROUTE_120_GABBY_AND_TY_1,
            FLAG_HIDE_ROUTE_111_GABBY_AND_TY_2,
            FLAG_HIDE_ROUTE_118_GABBY_AND_TY_2,
            FLAG_HIDE_ROUTE_120_GABBY_AND_TY_2,
            FLAG_HIDE_ROUTE_111_GABBY_AND_TY_3,
            FLAG_HIDE_ROUTE_118_GABBY_AND_TY_3,
            FLAG_HIDE_SLATEPORT_CITY_CONTEST_REPORTER,
            FLAG_HIDE_LILYCOVE_CONTEST_HALL_REPORTER,
            FLAG_HIDE_VERDANTURF_TOWN_WANDAS_HOUSE_WALLY,
            FLAG_HIDE_VERDANTURF_TOWN_WANDAS_HOUSE_WANDAS_BOYFRIEND,
            FLAG_HIDE_VERDANTURF_TOWN_WANDAS_HOUSE_WALLYS_UNCLE,
            FLAG_HIDE_VERDANTURF_TOWN_WANDAS_HOUSE_WANDA,
            FLAG_HIDE_VERDANTURF_TOWN_SCOTT,
            FLAG_HIDE_PETALBURG_CITY_WALLYS_DAD,
            FLAG_HIDE_SLATEPORT_CITY_STERNS_SHIPYARD_MR_BRINEY,
            FLAG_HIDE_SEAFLOOR_CAVERN_ROOM_9_ARCHIE,
            FLAG_HIDE_SEAFLOOR_CAVERN_ROOM_9_MAXIE,
            FLAG_HIDE_SEAFLOOR_CAVERN_ROOM_9_MAGMA_GRUNTS,
            FLAG_HIDE_SEAFLOOR_CAVERN_ROOM_9_KYOGRE,
            FLAG_HIDE_MAGMA_HIDEOUT_4F_GROUDON,
            FLAG_HIDE_SLATEPORT_CITY_HARBOR_CAPTAIN_STERN,
            FLAG_HIDE_SLATEPORT_CITY_HARBOR_AQUA_GRUNT,
            FLAG_HIDE_SLATEPORT_CITY_HARBOR_ARCHIE,
            FLAG_HIDE_SLATEPORT_CITY_HARBOR_SS_TIDAL,
            FLAG_HIDE_LILYCOVE_HARBOR_SSTIDAL,
            FLAG_HIDE_SLATEPORT_CITY_GABBY_AND_TY,
            FLAG_HIDE_SLATEPORT_CITY_CAPTAIN_STERN,
            FLAG_HIDE_SLATEPORT_CITY_HARBOR_SUBMARINE_SHADOW,
            FLAG_HIDE_ROUTE_119_RIVAL,
            FLAG_HIDE_ROUTE_119_SCOTT,
            FLAG_HIDE_SOOTOPOLIS_CITY_STEVEN,
            FLAG_HIDE_SOOTOPOLIS_CITY_WALLACE,
            FLAG_HIDE_LANETTES_HOUSE_LANETTE,
            FLAG_HIDE_TRICK_HOUSE_ENTRANCE_MAN,
            FLAG_HIDE_MT_CHIMNEY_TRAINERS,
            FLAG_HIDE_MT_CHIMNEY_LAVA_COOKIE_LADY,
            FLAG_HIDE_RUSTURF_TUNNEL_BRINEY,
            FLAG_HIDE_ROUTE_116_MR_BRINEY,
            FLAG_HIDE_RUSTURF_TUNNEL_PEEKO,
            FLAG_HIDE_RUSTURF_TUNNEL_AQUA_GRUNT,
            FLAG_HIDE_RUSTURF_TUNNEL_WANDAS_BOYFRIEND,
            FLAG_HIDE_RUSTURF_TUNNEL_WANDA,
            FLAG_HIDE_SLATEPORT_CITY_OCEANIC_MUSEUM_2F_ARCHIE,
            FLAG_HIDE_SLATEPORT_CITY_OCEANIC_MUSEUM_2F_AQUA_GRUNT_1,
            FLAG_HIDE_SLATEPORT_CITY_OCEANIC_MUSEUM_2F_AQUA_GRUNT_2,
            FLAG_HIDE_SLATEPORT_MUSEUM_POPULATION,
            FLAG_HIDE_BATTLE_TOWER_OPPONENT,
            FLAG_HIDE_WEATHER_INSTITUTE_1F_WORKERS,
            FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_UNKNOWN_0x380,
            FLAG_HIDE_LILYCOVE_HARBOR_FERRY_SAILOR,
            FLAG_HIDE_LILYCOVE_HARBOR_EVENT_TICKET_TAKER,
            FLAG_HIDE_SOUTHERN_ISLAND_EON_STONE,
            FLAG_HIDE_SOUTHERN_ISLAND_UNCHOSEN_EON_DUO_MON,
            FLAG_UNKNOWN_0x393,
            FLAG_HIDE_MT_PYRE_SUMMIT_MAXIE,
            FLAG_HIDE_MAUVILLE_CITY_WATTSON,
            FLAG_HIDE_MAUVILLE_CITY_SCOTT,
            FLAG_HIDE_CHAMPIONS_ROOM_RIVAL,
            FLAG_HIDE_CHAMPIONS_ROOM_BIRCH,
            FLAG_HIDE_ROUTE_110_RIVAL_ON_BIKE,
            FLAG_HIDE_ROUTE_119_RIVAL_ON_BIKE,
            FLAG_HIDE_ROUTE_104_RIVAL,
            FLAG_HIDE_LILYCOVE_MOTEL_GAME_DESIGNERS,
            FLAG_HIDE_LAVARIDGE_TOWN_RIVAL,
            FLAG_HIDE_LAVARIDGE_TOWN_RIVAL_ON_BIKE,
            FLAG_HIDE_MOSSDEEP_CITY_HOUSE_2_WINGULL,
            FLAG_HIDE_METEOR_FALLS_TEAM_AQUA,
            FLAG_HIDE_DEWFORD_HALL_SLUDGE_BOMB_MAN,
            FLAG_HIDE_FALLARBOR_HOUSE_PROF_COZMO,
            FLAG_HIDE_WEATHER_INSTITUTE_2F_AQUA_GRUNT_M,
            FLAG_HIDE_ROUTE_128_STEVEN,
            FLAG_HIDE_ROUTE_128_ARCHIE,
            FLAG_HIDE_ROUTE_128_MAXIE,
            FLAG_HIDE_ROUTE_116_DEVON_EMPLOYEE,
            FLAG_HIDE_SLATEPORT_CITY_TM_SALESMAN,
            FLAG_HIDE_SLATEPORT_CITY_SCOTT,
            FLAG_HIDE_VICTORY_ROAD_ENTRANCE_WALLY,
            FLAG_HIDE_VICTORY_ROAD_EXIT_WALLY,
            FLAG_HIDE_SS_TIDAL_CORRIDOR_MR_BRINEY,
            FLAG_HIDE_MOSSDEEP_CITY_STEVENS_HOUSE_STEVEN,
            FLAG_HIDE_MOSSDEEP_CITY_STEVENS_HOUSE_BELDUM_POKEBALL,
            FLAG_HIDE_MOSSDEEP_CITY_STEVENS_HOUSE_INVISIBLE_NINJA_BOY,
            FLAG_HIDE_SOOTOPOLIS_CITY_RAYQUAZA,
            FLAG_HIDE_SOOTOPOLIS_CITY_KYOGRE,
            FLAG_HIDE_SOOTOPOLIS_CITY_GROUDON,
            FLAG_HIDE_SOOTOPOLIS_CITY_RESIDENTS,
            FLAG_HIDE_SOOTOPOLIS_CITY_ARCHIE,
            FLAG_HIDE_SOOTOPOLIS_CITY_MAXIE,
            FLAG_HIDE_ROUTE_111_DESERT_FOSSIL,
            FLAG_HIDE_ROUTE_111_PLAYER_DESCENT,
            FLAG_HIDE_DESERT_UNDERPASS_FOSSIL,
            FLAG_HIDE_MOSSDEEP_CITY_TEAM_MAGMA,
            FLAG_HIDE_MOSSDEEP_CITY_SPACE_CENTER_1F_TEAM_MAGMA,
            FLAG_HIDE_MOSSDEEP_CITY_SPACE_CENTER_2F_TEAM_MAGMA,
            FLAG_HIDE_MOSSDEEP_CITY_SPACE_CENTER_2F_STEVEN,
            FLAG_HIDE_LILYCOVE_CONTEST_HALL_BLEND_MASTER,
            FLAG_HIDE_DEOXYS,
            FLAG_HIDE_SAFARI_ZONE_SOUTH_EAST_EXPANSION,
            FLAG_HIDE_FALLARBOR_TOWN_BATTLE_TENT_SCOTT,
            FLAG_HIDE_EVER_GRANDE_POKEMON_CENTER_1F_SCOTT,
            FLAG_HIDE_SKY_PILLAR_WALLACE,
            FLAG_HIDE_SKY_PILLAR_TOP_RAYQUAZA_STILL,
        ],
        ..Bypass::NONE
    };

    /// Blocker `briney-sail-camera`: Mr. Briney's yes, then `Route104_EventScript_SailToDewford`
    /// (the boat and the player `applymovement` down Route 104 off the map's edge, through Route
    /// 105 and 106, to Dewford) stop on a `RoundTrip` finding: the checkpoint taken mid-sail does
    /// not restore ("camera far outside the map"). The bypass writes what the talk and the sail
    /// leave and lands in Dewford.
    static BRINEY_SAIL_TO_DEWFORD: Bypass = Bypass {
        issue: "briney-sail-camera",
        set: &[
            FLAG_MR_BRINEY_SAILING_INTRO,
            FLAG_HIDE_BRINEYS_HOUSE_MR_BRINEY,
            FLAG_HIDE_BRINEYS_HOUSE_PEEKO,
            FLAG_HIDE_RUSTBORO_CITY_RIVAL,
            FLAG_HIDE_ROUTE_104_RIVAL,
            FLAG_HIDE_ROUTE_104_MR_BRINEY,
            FLAG_HIDE_ROUTE_104_MR_BRINEY_BOAT,
            FLAG_ENABLE_NORMAN_MATCH_CALL,
        ],
        clear: &[
            FLAG_HIDE_MR_BRINEY_DEWFORD_TOWN,
            FLAG_HIDE_MR_BRINEY_BOAT_DEWFORD_TOWN,
        ],
        vars: &[
            (VAR_BOARD_BRINEY_BOAT_STATE, 0),
            (VAR_BRINEY_LOCATION, 1),
            (VAR_RUSTBORO_CITY_STATE, 8),
            (VAR_ROUTE104_STATE, 2),
        ],
        warp: Some((MAP_DEWFORD_TOWN, 11, 11)),
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
        Step::Bypass(&RESET_ALL_MAP_FLAGS),
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
        goto(MAP_ROUTE116, 47, 9),
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
        // Yes to Mr. Briney (`Choose(0)`, `Interact`): his script warps to Route 104's dock,
        // whose frame script sails to Dewford. Blocked (`briney-sail-camera`).
        Step::Bypass(&BRINEY_SAIL_TO_DEWFORD),
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
    ];
}
