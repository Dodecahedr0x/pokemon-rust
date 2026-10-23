//! Which milestone owns a gap, hand-kept from the plans: Milestone 9's tasks
//! (`2026-10-23-milestone-9-menus-mart-save.md`: menus, mart, PC, save, evolution, the naming
//! screen) as `M9 <task>`, M10 (field moves, bikes, surfing and water, weather, the RTC and
//! berry trees, roamers; `2026-10-22-milestone-8-scale-import.md`'s roadmap), M11 (battle
//! completeness: double battles, the remaining effects, abilities and held items), M12 (audio:
//! the sound commands the content build ignores), M13 (side
//! content: contests, the Game Corner, secret bases, the Battle Frontier, link), else `none`.
//!
//! **Not M10 here**, although the M8 roadmap lists them under M10: field effects
//! (`dofieldeffect`) and virtual objects (`createvobject`, `turnvobject`). The parity plan names
//! them as Wave C's likely clusters, so the ledger leaves them unowned.

use crate::Category;

/// The Gen3 screens: (decomp source, what it is, the engine module under
/// `crates/engine/src/` or `""` for none, owner, the commands and specials that open it).
pub const SCREENS: &[(&str, &str, &str, &str, &[&str])] = &[
    (
        "main_menu.c",
        "main menu (CONTINUE / NEW GAME / OPTION)",
        "ui/main_menu.rs",
        "M9 S",
        &[],
    ),
    (
        "start_menu.c",
        "field start menu",
        "ui/start_menu.rs",
        "M9 SM",
        &["SPECIAL_ScriptMenu_CreateStartMenuForPokenavTutorial"],
    ),
    ("option_menu.c", "options", "ui/options.rs", "M9 SM", &[]),
    (
        "party_menu.c",
        "party menu",
        "ui/party.rs",
        "M9 PT",
        &[
            "SPECIAL_ChoosePartyMon",
            "SPECIAL_ChooseMonForMoveRelearner",
            "SPECIAL_ChooseHalfPartyForBattle",
        ],
    ),
    (
        "pokemon_summary_screen.c",
        "summary",
        "ui/summary.rs",
        "M9 PT",
        &[],
    ),
    (
        "item_menu.c",
        "bag",
        "ui/bag.rs",
        "M9 B",
        &["SPECIAL_Bag_ChooseBerry"],
    ),
    (
        "shop.c",
        "mart (buy and sell)",
        "ui/mart.rs",
        "M9 M",
        &["pokemart", "pokemartdecoration", "pokemartdecoration2"],
    ),
    (
        "player_pc.c",
        "PC (items, mailbox) and Pokémon storage",
        "ui/pc.rs",
        "M9 M",
        &[
            "SPECIAL_PlayerPC",
            "SPECIAL_BedroomPC",
            "SPECIAL_ShowPokemonStorageSystemPC",
            "SPECIAL_ScriptMenu_CreatePCMultichoice",
        ],
    ),
    (
        "start_menu.c (save)",
        "save dialog",
        "ui/save.rs",
        "M9 S",
        &["SPECIAL_SaveGame"],
    ),
    (
        "evolution_scene.c",
        "evolution scene",
        "ui/evolution.rs",
        "M9 E",
        &[],
    ),
    (
        "naming_screen.c",
        "naming screen",
        "ui/naming.rs",
        "M9 W",
        &[
            "SPECIAL_ChangePokemonNickname",
            "SPECIAL_ChangeBoxPokemonNickname",
            "SPECIAL_DoWaldaNamingScreen",
        ],
    ),
    (
        "starter_choose.c",
        "starter choice",
        "starter.rs",
        "none",
        &["SPECIAL_ChooseStarter"],
    ),
    (
        "pokedex.c",
        "Pokédex",
        "",
        "M10",
        &["SPECIAL_ShowPokedexRatingMessage"],
    ),
    (
        "region_map.c",
        "region map (and Fly's map)",
        "",
        "M10",
        &["SPECIAL_FieldShowRegionMap"],
    ),
    (
        "wallclock.c",
        "wall clock",
        "",
        "M10",
        &["SPECIAL_StartWallClock", "SPECIAL_Special_ViewWallClock"],
    ),
    ("trainer_card.c", "trainer card", "", "none", &[]),
    ("pokenav.c", "PokéNav", "", "none", &[]),
    (
        "easy_chat.c",
        "Easy Chat",
        "",
        "none",
        &["SPECIAL_ShowEasyChatScreen"],
    ),
    (
        "hall_of_fame.c",
        "Hall of Fame and credits",
        "",
        "none",
        &["SPECIAL_GameClear"],
    ),
    (
        "trade.c",
        "in-game trade scene",
        "",
        "M9 R",
        &["SPECIAL_DoInGameTradeScene"],
    ),
    (
        "egg_hatch.c",
        "egg hatching",
        "",
        "none",
        &["SPECIAL_EggHatch"],
    ),
    (
        "move_relearner.c",
        "move relearner",
        "",
        "none",
        &["SPECIAL_TeachMoveRelearnerMove"],
    ),
    (
        "contest.c",
        "contests",
        "",
        "M13",
        &["SPECIAL_TryEnterContestMon"],
    ),
    (
        "slot_machine.c",
        "slot machines",
        "",
        "M13",
        &["SPECIAL_GetSlotMachineId"],
    ),
    (
        "roulette.c",
        "roulette",
        "",
        "M13",
        &["SPECIAL_PlayRoulette"],
    ),
    (
        "berry_blender.c",
        "berry blender",
        "",
        "M13",
        &["SPECIAL_DoBerryBlending"],
    ),
];

fn any(name: &str, words: &[&str]) -> bool {
    words.iter().any(|w| name.contains(w))
}

/// The owner of entry `name` of `cat`.
pub fn owner(cat: Category, name: &str) -> &'static str {
    const SIDE: &[&str] = &[
        "Frontier",
        "Apprentice",
        "Pike",
        "Pyramid",
        "Dome",
        "Palace",
        "Arena",
        "Factory",
        "BattleTower",
        "TrainerHill",
        "Contest",
        "Slot",
        "Roulette",
        "SecretBase",
        "Decoration",
        "Link",
        "Union",
        "Record",
        "Berry",
        "Blend",
        "Pokeblock",
        "Lottery",
        "Mystery",
        "EReader",
        "Trader",
        "Cable",
        "Wireless",
        "Dodrio",
        "PokemonJump",
        "Quiz",
        "Lady",
        "Interview",
        "Gabby",
        "TV",
        "Storyteller",
        "Bard",
        "Fanclub",
        "FanClub",
        "Mauville",
        "Trick",
        "Safari",
        "Coins",
        "Fossil",
    ];
    match cat {
        Category::Screen => SCREENS.iter().find(|s| s.0 == name).map_or("none", |s| s.3),
        Category::FieldMove | Category::Weather => "M10",
        Category::Behavior => {
            if any(name, &["SECRET_BASE"]) {
                "M13"
            } else if any(
                name,
                &[
                    "WATER",
                    "CURRENT",
                    "SEAWEED",
                    "SURF",
                    "BIKE",
                    "RAIL",
                    "BUMPY",
                    "PUDDLE",
                    "HOT_SPRINGS",
                    "DIVE",
                    "WATERFALL",
                    "MUDDY_SLOPE",
                    "BERRY",
                    "SPRINGS",
                ],
            ) {
                "M10"
            } else {
                "none"
            }
        }
        Category::MoveEffect | Category::Ability | Category::HoldEffect => "M11",
        Category::BattleType => {
            if any(
                name,
                &[
                    "TOWER",
                    "DOME",
                    "PALACE",
                    "ARENA",
                    "FACTORY",
                    "PIKE",
                    "PYRAMID",
                    "LINK",
                    "RECORDED",
                    "EREADER",
                    "TRAINER_HILL",
                    "SECRET_BASE",
                    "SAFARI",
                    "MULTI",
                ],
            ) {
                "M13"
            } else if any(name, &["ROAMER"]) {
                "M10"
            } else {
                "M11"
            }
        }
        Category::ItemFieldUse => {
            if any(
                name,
                &["Bike", "Rod", "Itemfinder", "PokeblockCase", "CoinCase"],
            ) {
                "M10"
            } else {
                "M9 B"
            }
        }
        Category::ItemBattleUse => "M9 BM",
        Category::MapType => "none",
        Category::Command => match name {
            "pokemart" | "pokemartdecoration" | "pokemartdecoration2" => "M9 M",
            "fadescreenswapbuffers" | "setflashlevel" | "animateflash" => "M10",
            "addcoins"
            | "removecoins"
            | "checkcoins"
            | "showcoinsbox"
            | "hidecoinsbox"
            | "updatecoinsbox"
            | "showcontestpainting"
            | "showcontestresults"
            | "contestlinktransfer"
            | "choosecontestmon"
            | "startcontest"
            | "dotrainerhillbattle"
            | "checkdecor"
            | "checkdecorspace"
            | "adddecoration"
            | "removedecoration"
            | "bufferdecorationname"
            | "setberrytree"
            | "getpokenewsactive"
            | "setmysteryeventstatus" => "M13",
            "showmoneybox" | "hidemoneybox" | "updatemoneybox" => "M9 K",
            "setwildbattle" | "dowildbattle" => "none",
            "playse" | "waitse" | "checkse" | "playmoncry" | "waitmoncry" => "M12",
            _ if any(name, &["bgm", "fanfare", "sound"]) => "M12",
            _ if any(name, &["weather"]) => "M10",
            _ => "none",
        },
        Category::Special => {
            let n = name.trim_start_matches("SPECIAL_");
            if n == "SaveGame" || n.contains("SaveWarp") {
                "M9 S"
            } else if any(
                n,
                &["PC", "StorageSystem", "BoxToSendMon", "BoxWasFull", "Mart"],
            ) {
                "M9 M"
            } else if any(n, &["Nickname", "NamingScreen"]) {
                "M9 W"
            } else if any(n, &["Evolution", "Evolve"]) {
                "M9 E"
            } else if any(
                n,
                &["ChoosePartyMon", "ScriptGetPartyMonSpecies", "ChooseMonFor"],
            ) {
                "M9 PT"
            } else if any(n, &["Bag_", "ItemUse"]) {
                "M9 B"
            } else if any(n, &["InGameTrade"]) {
                "M9 R"
            } else if any(
                n,
                &[
                    "Clock",
                    "Cycling",
                    "Bike",
                    "Roamer",
                    "Dive",
                    "Surf",
                    "Waterfall",
                    "Strength",
                    "Weather",
                    "Tide",
                    "Fly",
                    "Flash",
                    "Steps",
                    "TimeBased",
                ],
            ) {
                "M10"
            } else if any(
                n,
                &["Legendary", "Lati", "Regi", "Battle", "Double", "Multi"],
            ) && !any(n, SIDE)
            {
                "M11"
            } else if any(n, SIDE) {
                "M13"
            } else {
                "none"
            }
        }
        Category::EngineReport => {
            if let Some(s) = SCREENS.iter().find(|s| {
                let stem = s.2.trim_start_matches("ui/").trim_end_matches(".rs");
                !stem.is_empty() && name.replace(' ', "_").contains(stem)
            }) {
                return s.3;
            }
            match name {
                "evolution scene" => "M9 E",
                "double battle" | "double sight" => "M11",
                "field move" | "bike" => "M10",
                "pc storage ui" => "M9 M",
                "pokedex" | "trainer card" => "none",
                _ => "none",
            }
        }
    }
}

/// Whether `owner` is Milestone 9's or 10's (Wave C takes the others).
pub fn is_m9_m10(owner: &str) -> bool {
    owner.starts_with("M9") || owner == "M10"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owners_follow_the_plans() {
        assert_eq!(owner(Category::FieldMove, "FIELD_MOVE_CUT"), "M10");
        assert_eq!(owner(Category::Special, "SPECIAL_PlayerPC"), "M9 M");
        assert_eq!(owner(Category::Special, "SPECIAL_SaveGame"), "M9 S");
        assert_eq!(
            owner(Category::Special, "SPECIAL_CallFrontierUtilFunc"),
            "M13"
        );
        assert_eq!(owner(Category::Command, "dofieldeffect"), "none");
        assert_eq!(owner(Category::Command, "createvobject"), "none");
        assert_eq!(owner(Category::Behavior, "MB_POND_WATER"), "M10");
        assert_eq!(owner(Category::Screen, "item_menu.c"), "M9 B");
        assert_eq!(owner(Category::EngineReport, "evolution scene"), "M9 E");
        assert_eq!(owner(Category::EngineReport, "bag"), "M9 B");
    }
}
