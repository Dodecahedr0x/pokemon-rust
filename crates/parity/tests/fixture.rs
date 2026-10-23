//! The ledger on a small synthetic decomp (written to a temp dir; no ROM data): one pinned
//! entry per category, with its status, uses, story act, reach and owner, and the ranking.

use parity::decomp::Decomp;
use parity::ledger::{self, Extra, Reached};
use parity::lowered::Lowering;
use parity::reach::Reach;
use parity::{Answer, Category, Engine, Status};
use std::collections::BTreeMap;
use std::path::Path;

fn write(root: &Path, rel: &str, body: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

/// The synthetic decomp: two maps (`Route101`, an Act 1 map, and `SideMap`, off the story),
/// a common nurse script, a trainer, a hidden item and a two-cell layout.
fn fixture(root: &Path) {
    let w = |rel: &str, s: &str| write(root, rel, s.as_bytes());
    w(
        "asm/macros/event.inc",
        "\t.macro end\n\t.byte SCR_OP_END\n\t.endm\n\
         \t.macro return\n\t.byte SCR_OP_RETURN\n\t.endm\n\
         \t.macro call dest:req\n\t.byte SCR_OP_CALL\n\t.4byte \\dest\n\t.endm\n\
         \t.macro special f:req\n\t.byte SCR_OP_SPECIAL\n\t.2byte SPECIAL_\\f\n\t.endm\n\
         \t.macro specialvar v:req, f:req\n\t.byte SCR_OP_SPECIALVAR\n\t.endm\n\
         \t.macro dofieldeffect e:req\n\t.byte SCR_OP_DOFIELDEFFECT\n\t.endm\n\
         \t.macro loadword d, v\n\t.byte SCR_OP_LOADWORD\n\t.endm\n\
         \t.macro callstd f\n\t.byte SCR_OP_CALLSTD\n\t.endm\n\
         \t.macro trainerbattle t, a\n\t.byte SCR_OP_TRAINERBATTLE\n\t.endm\n\
         \t@ A helper: no SCR_OP of its own.\n\
         \t.macro msgbox text:req, type=MSGBOX_DEFAULT\n\tloadword 0, \\text\n\tcallstd \\type\n\t.endm\n\
         \t.macro trainerbattle_single t, a\n\ttrainerbattle 0, \\t\n\t.endm\n",
    );
    // `addobjectat` has a handler but no macro.
    w(
        "data/script_cmd_table.inc",
        "\tscript_cmd_table_entry SCR_OP_END            ScrCmd_end            @ 0x02\n\
         \tscript_cmd_table_entry SCR_OP_ADDOBJECTAT    ScrCmd_addobjectat    @ 0x56\n",
    );
    w(
        "data/specials.inc",
        "gSpecials::\n\tdef_special HealPlayerParty\n\tdef_special ShowPokedexRatingMessage\n",
    );
    w(
        "data/event_scripts.s",
        "gStdScripts::\n\t.4byte Std_MsgboxNPC @ MSGBOX_NPC\ngStdScripts_End::\n\nStd_MsgboxNPC:\n\tloadword 0, 0\n\treturn\n",
    );
    w(
        "data/scripts/pkmn_center_nurse.inc",
        "Common_EventScript_Nurse::\n\tspecial HealPlayerParty\n\tdofieldeffect FLDEFF_POKECENTER_HEAL\n\tdofieldeffect FLDEFF_POKECENTER_HEAL\n\treturn\n",
    );
    w(
        "data/maps/map_groups.json",
        r#"{"group_order": ["gMapGroup_TownsAndRoutes"], "gMapGroup_TownsAndRoutes": ["Route101", "SideMap"]}"#,
    );
    w(
        "data/maps/Route101/map.json",
        r#"{"id": "MAP_ROUTE101", "name": "Route101", "layout": "LAYOUT_ROUTE101",
            "requires_flash": false, "weather": "WEATHER_SUNNY", "map_type": "MAP_TYPE_ROUTE",
            "connections": null,
            "object_events": [{"graphics_id": "OBJ_EVENT_GFX_CUTTABLE_TREE", "script": "Route101_EventScript_Tree"}],
            "coord_events": [],
            "bg_events": [{"type": "hidden_item", "item": "ITEM_POTION"}]}"#,
    );
    w(
        "data/maps/Route101/scripts.inc",
        "Route101_EventScript_Tree::\n\tmsgbox Route101_Text_Tree, MSGBOX_NPC\n\tcall Common_EventScript_Nurse\n\ttrainerbattle_single TRAINER_A, Route101_Text_Tree\n\tend\n\nRoute101_Text_Tree:\n\t.string \"A tree$\"\n",
    );
    w(
        "data/maps/SideMap/map.json",
        r#"{"id": "MAP_SIDE_MAP", "name": "SideMap", "layout": "LAYOUT_NONE",
            "requires_flash": false, "weather": "WEATHER_RAIN", "map_type": "MAP_TYPE_INDOOR",
            "object_events": [], "coord_events": [], "bg_events": []}"#,
    );
    w(
        "data/maps/SideMap/scripts.inc",
        "SideMap_EventScript_Dex::\n\tspecial ShowPokedexRatingMessage\n\tend\n",
    );
    w(
        "data/layouts/layouts.json",
        r#"{"layouts": [{"id": "LAYOUT_ROUTE101", "width": 2, "height": 1,
            "primary_tileset": "gTileset_General", "secondary_tileset": "gTileset_Route",
            "blockdata_filepath": "data/layouts/Route101/map.bin"}]}"#,
    );
    // Cell 0: primary metatile 0; cell 1: secondary metatile 0 (512).
    write(root, "data/layouts/Route101/map.bin", &[0, 0, 0, 2]);
    w(
        "src/data/tilesets/headers.h",
        "const struct Tileset gTileset_General =\n{\n    .metatileAttributes = gMetatileAttributes_General,\n};\nconst struct Tileset gTileset_Route =\n{\n    .metatileAttributes = gMetatileAttributes_Route,\n};\n",
    );
    w(
        "src/data/tilesets/metatiles.h",
        "const u16 gMetatileAttributes_General[] = INCBIN_U16(\"data/tilesets/primary/general/metatile_attributes.bin\");\nconst u16 gMetatileAttributes_Route[] = INCBIN_U16(\"data/tilesets/secondary/route/metatile_attributes.bin\");\n",
    );
    // MB_TALL_GRASS = 1, MB_POND_WATER = 2.
    write(
        root,
        "data/tilesets/primary/general/metatile_attributes.bin",
        &[1, 0x10],
    );
    write(
        root,
        "data/tilesets/secondary/route/metatile_attributes.bin",
        &[2, 0],
    );
    w(
        "include/constants/metatile_behaviors.h",
        "enum {\n    MB_NORMAL,\n    MB_TALL_GRASS,\n    MB_POND_WATER,\n    NUM_METATILE_BEHAVIORS\n};\n",
    );
    w(
        "include/constants/battle_move_effects.h",
        "#define EFFECT_HIT 0\n#define EFFECT_SLEEP 1\n",
    );
    w(
        "include/constants/abilities.h",
        "#define ABILITY_NONE 0\n#define ABILITY_OVERGROW 1\n#define ABILITY_DROUGHT 2\n#define ABILITIES_COUNT 3\n",
    );
    w(
        "include/constants/hold_effects.h",
        "#define HOLD_EFFECT_NONE 0\n#define HOLD_EFFECT_RESTORE_HP 1\n",
    );
    w(
        "include/constants/weather.h",
        "#define WEATHER_NONE 0\n#define WEATHER_SUNNY 2\n#define WEATHER_RAIN 3\n",
    );
    w(
        "include/constants/battle.h",
        "#define BATTLE_TYPE_DOUBLE (1 << 0)\n#define BATTLE_TYPE_TRAINER (1 << 3)\n#define BATTLE_TYPE_FRONTIER (BATTLE_TYPE_DOUBLE | BATTLE_TYPE_TRAINER)\n",
    );
    w(
        "include/constants/map_types.h",
        "#define MAP_TYPE_ROUTE 3\n#define MAP_TYPE_INDOOR 8\n",
    );
    w(
        "src/party_menu.c",
        "enum {\n    MENU_A,\n};\nenum {\n    FIELD_MOVE_CUT,         // FLAG_BADGE01_GET\n    FIELD_MOVE_SURF,\n    FIELD_MOVES_COUNT\n};\n",
    );
    w(
        "src/data/items.h",
        "    [ITEM_POTION] =\n    {\n        .fieldUseFunc = ItemUseOutOfBattle_Medicine,\n        .battleUseFunc = ItemUseInBattle_Medicine,\n    },\n    [ITEM_ORAN_BERRY] =\n    {\n        .holdEffect = HOLD_EFFECT_RESTORE_HP,\n        .fieldUseFunc = NULL,\n    },\n",
    );
    w(
        "src/data/battle_moves.h",
        "    [MOVE_TACKLE] =\n    {\n        .effect = EFFECT_HIT,\n    },\n    [MOVE_SING] =\n    {\n        .effect = EFFECT_SLEEP,\n    },\n",
    );
    w(
        "src/data/pokemon/species_info.h",
        "    [SPECIES_A] =\n    {\n        .abilities = {ABILITY_OVERGROW, ABILITY_NONE},\n    },\n",
    );
    w(
        "src/data/pokemon/level_up_learnsets.h",
        "static const u16 sALevelUpLearnset[] = {\n    LEVEL_UP_MOVE( 1, MOVE_TACKLE),\n    LEVEL_UP_MOVE(10, MOVE_SING),\n    LEVEL_UP_END\n};\n",
    );
    w(
        "src/data/pokemon/level_up_learnset_pointers.h",
        "    [SPECIES_A] = sALevelUpLearnset,\n",
    );
    w(
        "src/data/trainers.h",
        "    [TRAINER_A] =\n    {\n        .party = ITEM_DEFAULT_MOVES(sParty_A),\n    },\n",
    );
    w(
        "src/data/trainer_parties.h",
        "static const struct TrainerMonItemDefaultMoves sParty_A[] = {\n    {\n    .lvl = 5,\n    .species = SPECIES_A,\n    .heldItem = ITEM_ORAN_BERRY,\n    }\n};\n",
    );
    w(
        "src/data/wild_encounters.json",
        r#"{"wild_encounter_groups": [{"for_maps": true, "encounters": []}]}"#,
    );
}

/// Synthetic engine answers.
struct Fake;

impl Engine for Fake {
    fn answer(&self, cat: Category, name: &str, _: &Decomp) -> Answer {
        use Status::*;
        let s = match (cat, name) {
            (Category::Command, "dofieldeffect") => Unsupported,
            (Category::Special, "SPECIAL_ShowPokedexRatingMessage") => Unsupported,
            (Category::Behavior, "MB_POND_WATER") => Unsupported,
            (Category::MoveEffect, "EFFECT_SLEEP") => Unsupported,
            (Category::Ability, "ABILITY_DROUGHT") => Missing,
            (Category::ItemFieldUse | Category::ItemBattleUse | Category::FieldMove, _) => Missing,
            (Category::Weather, w) if w != "WEATHER_NONE" => Ignored,
            (Category::BattleType, "BATTLE_TYPE_DOUBLE") => Degraded,
            (Category::Screen, "pokedex.c") => {
                return Answer {
                    status: None,
                    note: "no engine module".into(),
                };
            }
            (Category::Screen, _) => Missing,
            _ => Native,
        };
        Answer::new(s, "fake")
    }
}

fn reach_db(path: &Path) {
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO meta VALUES ('schema', '4');
         CREATE TABLE spine (id INTEGER PRIMARY KEY, steps INTEGER, done INTEGER, error TEXT);
         INSERT INTO spine VALUES (1, 10, 10, NULL);
         CREATE TABLE position (idx INTEGER PRIMARY KEY, map INTEGER NOT NULL);
         INSERT INTO position VALUES (0, 0);
         CREATE TABLE coverage (map INTEGER, x INTEGER, y INTEGER, spine INTEGER, hits INTEGER);
         INSERT INTO coverage VALUES (0, 1, 1, 1, 0), (1, 2, 2, 0, 3);
         CREATE TABLE seen (kind INTEGER, id INTEGER, first_branch INTEGER, hits INTEGER);
         CREATE TABLE branch (id INTEGER PRIMARY KEY, outcome TEXT);
         INSERT INTO branch VALUES (1, 'rejoined'), (2, 'rejoined');
         CREATE TABLE finding (id INTEGER PRIMARY KEY, kind TEXT, branch INTEGER, message TEXT,
                               map INTEGER, hits INTEGER);
         INSERT INTO finding VALUES
           (1, 'unsupported', 2, 'unsupported: SPECIAL_ShowPokedexRatingMessage', 1, 1),
           (2, 'unsupported', NULL, 'degraded: evolution scene', 0, 2),
           (3, 'panic', 1, 'boom', 0, 1);",
    )
    .unwrap();
}

#[test]
fn one_entry_per_category_on_a_synthetic_decomp() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let d = Decomp::load(dir.path()).unwrap();
    // Only the optional files the fixture leaves out.
    assert!(d.issues.is_empty(), "{:#?}", d.issues);
    let db = dir.path().join("explore.db");
    reach_db(&db);
    let mut reach = Reach::default();
    reach.read_db(&db).unwrap();
    reach.read_sweep(
        "-- reports raised while idling (1 kinds): maps, events --\nUnsupported(\"dofieldeffect\"): 2 maps, 5 events\n",
    );
    let lowering = Lowering::parse(
        "site\tdofieldeffect\tUnsupported\tCommon_pkmn_center_nurse\t2\nreport\tdofieldeffect\t2\n",
    );
    let extra = Extra {
        engine_reports: BTreeMap::from([
            ("double battle".to_string(), Status::Degraded),
            // A native command the engine reports simplified.
            ("loadword".to_string(), Status::Degraded),
        ]),
        bypassed: vec!["SPECIAL_ShowPokedexRatingMessage".into()],
    };
    let l = ledger::build(&d, &Fake, &lowering, &reach, &extra);
    let e = |c: Category, n: &str| {
        l.entry(c, n)
            .unwrap_or_else(|| panic!("no {c:?} {n}: {:#?}", l.entries.get(&c)))
    };
    let maps = |c: Category, n: &str| e(c, n).maps.clone();

    // Commands: through the helper macro and the common script the map calls.
    let c = e(Category::Command, "dofieldeffect");
    assert_eq!(
        (c.status, c.story_act, c.reached, c.sites),
        (Status::Unsupported, Some(0), Reached::Spine, 2)
    );
    assert_eq!(c.maps, ["Route101"]);
    assert_eq!(
        (c.lowered, c.reports, c.sweep_maps),
        (Some(2), Some(2), Some(2))
    );
    assert_eq!(c.owner, "none");
    assert_eq!(maps(Category::Command, "callstd"), ["Route101"]);
    assert_eq!(
        e(Category::Command, "loadword").sites,
        2,
        "msgbox and the std"
    );
    assert_eq!(e(Category::Command, "loadword").status, Status::Degraded);
    assert!(l.entry(Category::EngineReport, "loadword").is_none());
    // Specials: reported by a branch, bypassed by the spine.
    let s = e(Category::Special, "SPECIAL_ShowPokedexRatingMessage");
    assert_eq!(
        (s.reached, s.story_act, s.story_blocking),
        (Reached::Reported, None, true)
    );
    assert_eq!(s.maps, ["SideMap"]);
    assert_eq!(
        maps(Category::Special, "SPECIAL_HealPlayerParty"),
        ["Route101"]
    );
    // Behaviours, from the layout's two tilesets.
    assert_eq!(maps(Category::Behavior, "MB_POND_WATER"), ["Route101"]);
    assert_eq!(e(Category::Behavior, "MB_POND_WATER").owner, "M10");
    assert!(maps(Category::Behavior, "MB_NORMAL").is_empty());
    // Battle: the trainer's default moves at level 5, its ability and held item.
    assert_eq!(maps(Category::MoveEffect, "EFFECT_HIT"), ["Route101"]);
    assert!(maps(Category::MoveEffect, "EFFECT_SLEEP").is_empty());
    assert_eq!(maps(Category::Ability, "ABILITY_OVERGROW"), ["Route101"]);
    assert_eq!(
        e(Category::Ability, "ABILITY_DROUGHT").status,
        Status::Missing
    );
    assert_eq!(
        maps(Category::HoldEffect, "HOLD_EFFECT_RESTORE_HP"),
        ["Route101"]
    );
    // Items: the hidden Potion.
    let i = e(Category::ItemFieldUse, "ItemUseOutOfBattle_Medicine");
    assert_eq!(
        (i.maps.clone(), i.sites, i.owner.as_str()),
        (vec!["Route101".to_string()], 1, "M9 B")
    );
    assert_eq!(
        e(Category::ItemBattleUse, "ItemUseInBattle_Medicine").owner,
        "M9 BM"
    );
    // Field moves: the cuttable tree and the pond.
    assert_eq!(maps(Category::FieldMove, "FIELD_MOVE_CUT"), ["Route101"]);
    assert_eq!(maps(Category::FieldMove, "FIELD_MOVE_SURF"), ["Route101"]);
    // Weathers, battle types, map types.
    let w = e(Category::Weather, "WEATHER_RAIN");
    assert_eq!((w.status, w.reached), (Status::Ignored, Reached::Branch));
    assert_eq!(w.maps, ["SideMap"]);
    assert_eq!(
        maps(Category::BattleType, "BATTLE_TYPE_TRAINER"),
        ["Route101"]
    );
    assert!(
        l.entry(Category::BattleType, "BATTLE_TYPE_FRONTIER")
            .is_none()
    );
    assert_eq!(maps(Category::MapType, "MAP_TYPE_INDOOR"), ["SideMap"]);
    // Screens: opened by their specials; an unknown status goes to the issues.
    let p = e(Category::Screen, "pokedex.c");
    assert_eq!(
        (p.status, p.maps.clone()),
        (Status::Unknown, vec!["SideMap".to_string()])
    );
    assert!(l.issues.iter().any(|i| i.contains("pokedex.c")));
    assert_eq!(e(Category::Screen, "item_menu.c").owner, "M9 B");
    // Engine reports: the engine's own names and what the explorer saw.
    let r = e(Category::EngineReport, "evolution scene");
    assert_eq!(
        (r.status, r.reached, r.owner.as_str()),
        (Status::Degraded, Reached::Reported, "M9 E")
    );
    assert_eq!(e(Category::EngineReport, "double battle").owner, "M11");

    // Coverage and ranking.
    let cov = &l.coverage[&Category::Command];
    assert_eq!((cov.total, cov.count(Status::Unsupported)), (10, 1));
    assert_eq!(l.entries[&Category::Command][0].name, "dofieldeffect");
    let ranked = l.ranked_gaps();
    assert_eq!(
        ranked[0].name, "SPECIAL_ShowPokedexRatingMessage",
        "story-blocking first"
    );
    // Then the Act 1 gaps, reached before unreached.
    assert_eq!(ranked[1].story_act, Some(0));
    let wave_c = l.wave_c(100);
    assert!(wave_c.iter().all(|e| !parity::owners::is_m9_m10(&e.owner)));
    assert!(wave_c.iter().any(|e| e.name == "dofieldeffect"));
    assert!(!wave_c.iter().any(|e| e.name == "MB_POND_WATER"));
    let md = l.markdown();
    assert!(md.contains("| script commands | 10 |"), "{md}");
    assert!(md.contains("`dofieldeffect`"));
    let ron = l.ron();
    assert!(ron.contains("dofieldeffect"));
}
