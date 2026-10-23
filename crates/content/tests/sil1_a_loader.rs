//! SIL1 Task A: the Silice authored-data loader (`build/silice/`).
//!
//! - On fixture games dirs over a fixture extract (the stub world written to disk), in any
//!   flavour: the world loads (maps, headers, `LOCALID_*`, texts, trainers, encounters, items,
//!   marts), a rebuild is byte-identical, and an unknown tileset or script, a repeated text id,
//!   an unencodable or over-long name without its exception, and a map over Gen3's limits fail.
//!   One case goes through a nested build with `PKR_SILICE_DIR`.
//! - On the committed `games/silice` with the real extract (skipped without one): it loads, with
//!   one constant per `LOCALID_*`, trainer, item and text.
//! - In the Silice flavour (`cfg(pkr_silice)`): the generated tables against the RON files
//!   (every trainer's party, the encounters, the items and their hold effects, the texts).
//!
//! No French literal: expected values are read back from the RON files or built from the
//! extract's tables.

#[allow(dead_code)]
#[path = "../build/battle.rs"]
mod battle;
#[allow(dead_code)]
#[path = "../build/battle_load.rs"]
mod battle_load;
#[allow(dead_code)]
#[path = "../build/blob_data.rs"]
mod blob_data;
#[allow(dead_code)]
#[path = "../build/build_id.rs"]
mod build_id;
#[allow(dead_code)]
#[path = "../../engine/build_support.rs"]
mod build_support;
#[allow(dead_code)]
#[path = "../build/classify/mod.rs"]
mod classify;
#[allow(dead_code)]
#[path = "../build/codegen.rs"]
mod codegen;
#[allow(dead_code)]
#[path = "../build/input.rs"]
mod input;
#[allow(dead_code)]
#[path = "../build/scripts.rs"]
mod scripts;
#[allow(dead_code)]
#[path = "../build/silice/mod.rs"]
mod silice;
#[allow(dead_code)]
#[path = "../build/stub.rs"]
mod stub;

use extract::export::to_ron;
use extract::games::{
    self as games, Currency, EncountersFile, Exception, ExceptionsFile, GameHeader, GameMap,
    GameObject, GameWarp, IdEntry, IdKind, IdsFile, ItemDef, ItemsFile, Mart, MartItem,
    TrainersFile,
};
use extract::schema::{ItemPocket, LandEncounters, Layout, MapScripts, Row, TrainerMon, WildMon};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A fresh scratch dir under the test target dir.
fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("sil1-a-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(root: &Path, rel: &str, s: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
}

/// `put` of a value as RON (a macro: the tests have no `serde` to name the bound).
macro_rules! put_ron {
    ($root:expr, $rel:expr, $v:expr $(,)?) => {
        put($root, $rel, &to_ron($v).unwrap())
    };
}

// ---- the fixture ----

const MAP: &str = "Fix";
const SCRIPT: &str = "Fix_EventScript_A";
const LOCALID: &str = "LOCALID_FIX_A";
const MUSIC: &str = "MUS_SIL_FIX";
const TRAINER_A: &str = "TRAINER_SIL_FIX_A";
const TRAINER_GONE: &str = "TRAINER_SIL_FIX_GONE";
const TRAINER_B: &str = "TRAINER_SIL_FIX_B";
const STONE: &str = "ITEM_SIL_FIX_STONE";
const KEY: &str = "ITEM_SIL_FIX_KEY";
const PIC: &str = "TRAINER_PIC_HIKER";
/// The stone's base item (its pocket and hold effect).
const BASE: &str = "ITEM_LEFTOVERS";

/// The French guillemets the FR charmap draws on English's curly quotes.
fn guillemets() -> (char, char) {
    let o = extract::text::FR_OVERLAY;
    (o[0].1, o[1].1)
}

fn ids(base: u32, names: &[(&str, u32, bool)]) -> IdsFile {
    IdsFile {
        base,
        ids: names
            .iter()
            .map(|&(n, num, removed)| IdEntry {
                name: n.into(),
                num,
                map: None,
                removed,
            })
            .collect(),
    }
}

fn object(local_id: u8, id: Option<&str>, x: i16) -> GameObject {
    GameObject {
        local_id,
        id: id.map(String::from),
        gfx: "OBJ_EVENT_GFX_BRENDAN_NORMAL".into(),
        x,
        y: 1,
        elevation: 3,
        movement: "MOVEMENT_TYPE_FACE_DOWN".into(),
        range_x: 0,
        range_y: 0,
        trainer_type: "TRAINER_TYPE_NONE".into(),
        trainer_sight_or_berry_tree: 0,
        script: Some(SCRIPT.into()),
        flag: None,
    }
}

fn fixture_map() -> GameMap {
    GameMap {
        name: MAP.into(),
        layout: MAP.into(),
        header: GameHeader {
            music: MUSIC.into(),
            weather: "WEATHER_SUNNY".into(),
            map_type: "MAP_TYPE_ROUTE".into(),
            ..GameHeader::new_map()
        },
        objects: vec![object(1, Some(LOCALID), 1), object(2, None, 2)],
        warps: vec![GameWarp {
            x: 0,
            y: 0,
            elevation: 3,
            dest_map: MAP.into(),
            dest_warp: 0,
        }],
        coords: vec![],
        bgs: vec![],
        connections: vec![],
        scripts: MapScripts::default(),
    }
}

fn mon(level: u8, item: Option<&str>, moves: Option<&[&str]>) -> TrainerMon {
    TrainerMon {
        iv: 100,
        level,
        species: "SPECIES_STUB_A".into(),
        item: item.map(String::from),
        moves: moves.map(|m| m.iter().map(|s| s.to_string()).collect()),
    }
}

fn trainer(id: &str, name: &str, party: Vec<TrainerMon>) -> games::Trainer {
    games::Trainer {
        id: id.into(),
        class: "TRAINER_CLASS_HIKER".into(),
        class_name: "CLASS".into(),
        name: name.into(),
        pic: PIC.into(),
        female: false,
        ai: vec!["AI_SCRIPT_CHECK_BAD_MOVE".into()],
        double: false,
        party,
        items: vec![],
        notes: vec![],
    }
}

/// The texts: (id, text), the second with the guillemets.
fn fixture_texts() -> Vec<(String, String)> {
    let (l, r) = guillemets();
    vec![
        ("Fix_Text_A".into(), "HELLO {PLAYER}!\\nOK.".into()),
        ("Fix_Text_B".into(), format!("{l} QUOTE {r}\\pBYE.")),
    ]
}

fn write_texts(games: &Path, texts: &[(String, String)]) {
    let mut s = String::from("{\n");
    for (k, v) in texts {
        s.push_str(&format!("    {k:?}: {v:?},\n"));
    }
    s.push_str("}\n");
    put(games, "text/fr.ron", &s);
}

fn write_exceptions(games: &Path, list: &[(&str, &str)]) {
    let f = ExceptionsFile {
        exceptions: list
            .iter()
            .map(|&(rule, subject)| Exception {
                rule: rule.into(),
                subject: subject.into(),
                reason: "fixture".into(),
                ruling_needed: true,
            })
            .collect(),
    };
    put_ron!(games, "check-exceptions.ron", &f);
}

/// A fixture extract (the stub world, plus one trainer pic) and games dir and scripts dir:
/// (extracted, games, scripts).
fn fixture(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = scratch(name);
    let (x, g, s) = (
        root.join("extracted"),
        root.join("games"),
        root.join("scripts"),
    );
    // The stub world, with Gen3's item table up to the base item of the fixture's stone.
    let mut w = stub::world();
    let base = extract::names::ITEMS.value(BASE).unwrap() as usize;
    let items = &mut w.tables.items.items;
    while items.len() <= base {
        let n = items.len();
        items.push(extract::schema::Item {
            id: extract::names::ITEMS.name_or_hex(n as u32),
            name: "?".into(),
            pocket: Some(ItemPocket::Items),
            price: 0,
            importance: 0,
        });
    }
    input::write(&w, &x).unwrap();
    // `input::write` leaves the tables out: the items table the stone reads.
    put_ron!(&x, "tables/items.ron", &w.tables.items);
    let mut pics = extract::schema::TrainerPicsFile::default();
    pics.front.insert(
        PIC.into(),
        extract::schema::TrainerPicInfo { y: 0, size: 8 },
    );
    put_ron!(&x, "trainers/pics.ron", &pics);
    let img = extract::gfx::IndexedImage {
        width: 64,
        height: 64,
        pixels: (0..64 * 64).map(|i| (i % 3) as u8).collect(),
    };
    std::fs::create_dir_all(x.join("trainers/front")).unwrap();
    extract::gfx::write_png_indexed(
        &x.join(format!("trainers/front/{PIC}.png")),
        &img,
        &extract::gfx::Palette([0x7FFF; 16]),
    )
    .unwrap();

    put(
        &s,
        "fix.rs",
        &format!("pub async fn {SCRIPT}(ctx: Ctx) {{}}\n"),
    );

    put_ron!(&g, &format!("maps/{MAP}.ron"), &fixture_map());
    let blocks = vec![Row(vec![1 | 3 << 12; 4]); 4];
    put_ron!(
        &g,
        &format!("layouts/{MAP}.ron"),
        &Layout {
            width: 4,
            height: 4,
            primary: "StubPrimary".into(),
            secondary: "StubSecondary".into(),
            border: vec![Row(vec![1; 2]); 2],
            blocks,
        },
    );
    let mut maps = ids(0, &[("MAP_FIX", 0, false)]);
    maps.ids[0].map = Some(MAP.into());
    put_ron!(&g, "ids/maps.ron", &maps);
    let song = IdKind::Songs.range().0;
    put_ron!(&g, "ids/songs.ron", &ids(song, &[(MUSIC, song, false)]));
    put_ron!(
        &g,
        "ids/trainers.ron",
        &ids(
            1,
            &[
                (TRAINER_A, 1, false),
                (TRAINER_GONE, 2, true),
                (TRAINER_B, 3, false)
            ],
        ),
    );
    let item = IdKind::Items.range().0;
    put_ron!(
        &g,
        "ids/items.ron",
        &ids(item, &[(STONE, item, false), (KEY, item + 1, false)]),
    );
    put_ron!(
        &g,
        "battle/trainers.ron",
        &TrainersFile {
            trainers: vec![
                trainer(
                    TRAINER_B,
                    "BEE",
                    vec![
                        mon(7, Some(STONE), Some(&["MOVE_STUB_HIT"])),
                        mon(9, Some("ITEM_ORAN_BERRY"), None),
                    ],
                ),
                trainer(TRAINER_A, "AYE", vec![mon(5, Some("ITEM_NONE"), None)]),
            ],
        },
    );
    let slot = WildMon {
        species: "SPECIES_STUB_B".into(),
        min_level: 2,
        max_level: 4,
    };
    put_ron!(
        &g,
        "battle/encounters.ron",
        &EncountersFile {
            maps: BTreeMap::from([(
                MAP.to_string(),
                vec![games::EncounterHeader {
                    land: Some(LandEncounters {
                        rate: 20,
                        mons: vec![slot; 12],
                    }),
                    ..Default::default()
                }],
            )]),
        },
    );
    put_ron!(
        &g,
        "items.ron",
        &ItemsFile {
            items: vec![
                ItemDef {
                    id: KEY.into(),
                    name: Some("KEY".into()),
                    pocket: Some(ItemPocket::KeyItems),
                    price: None,
                    clone_of: None,
                    effect: None,
                    chapters: vec![],
                },
                ItemDef {
                    id: STONE.into(),
                    name: Some("STONE".into()),
                    pocket: None,
                    price: Some(1200),
                    clone_of: Some(BASE.into()),
                    effect: None,
                    chapters: vec![],
                },
            ],
            placements: vec![],
            marts: vec![Mart {
                chapter: "00".into(),
                label: "Fix".into(),
                map: Some("MAP_FIX".into()),
                condition: None,
                note: None,
                currency: Currency::Money,
                stock: vec![
                    MartItem {
                        item: "ITEM_POTION".into(),
                        price: 300,
                        condition: None,
                        note: None,
                    },
                    MartItem {
                        item: STONE.into(),
                        price: 1200,
                        condition: None,
                        note: None,
                    },
                ],
            }],
        },
    );
    write_texts(&g, &fixture_texts());
    write_exceptions(&g, &[]);
    (x, g, s)
}

/// The fixture's world, its new game on the fixture map (`silice::START` is Task E's).
fn world(f: &(PathBuf, PathBuf, PathBuf), strict: bool) -> anyhow::Result<silice::Silice> {
    let mut s = silice::world(&f.0, &f.1, &f.2, strict)?;
    s.world.authored.as_mut().unwrap().start = (MAP.into(), 1, 1);
    Ok(s)
}

fn err(f: &(PathBuf, PathBuf, PathBuf)) -> String {
    match world(f, true) {
        Ok(_) => panic!("the fixture built"),
        Err(e) => format!("{e:#}"),
    }
}

/// Every generated file of a world.
fn generated(s: &silice::Silice) -> Vec<Vec<u8>> {
    let g = codegen::generate(&s.world, 1).unwrap();
    let d = silice::dispatch::generate(&s.registry, &s.unwritten, &s.report).unwrap();
    vec![
        g.consts.into_bytes(),
        g.data.into_bytes(),
        g.data_bin,
        g.battle.into_bytes(),
        g.battle_bin,
        g.report.into_bytes(),
        d.dispatch.into_bytes(),
        d.rs.into_bytes(),
        d.report.into_bytes(),
    ]
}

fn has_const(consts: &str, name: &str, ty: &str, value: &str) -> bool {
    consts.contains(&format!("pub const {name}: {ty} = {value};"))
}

// ---- fixture: what loads ----

#[test]
fn the_fixture_loads_every_kind_of_data() {
    let f = fixture("loads");
    let s = world(&f, true).unwrap();
    let w = &s.world;
    let a = w.authored.as_ref().unwrap();

    // The map: its header names resolved, its LOCALID_* kept.
    let m = &w.maps[0];
    let song = IdKind::Songs.range().0;
    assert_eq!(u32::from(m.header.music), song);
    assert_eq!(
        u32::from(m.header.weather),
        games::WEATHER.value("WEATHER_SUNNY").unwrap()
    );
    assert_eq!(
        u32::from(m.header.map_type),
        games::MAP_TYPES.value("MAP_TYPE_ROUTE").unwrap()
    );
    assert!(a.consts.contains(&(LOCALID.into(), "u8", "1".into())));
    assert_eq!(m.objects[0].script.as_deref(), Some(SCRIPT));

    // The texts: by id, encoded with the FR charmap and its guillemets.
    for (k, v) in fixture_texts() {
        let b = extract::text::encode_lang(&v, extract::text::Lang::Fr).unwrap();
        assert_eq!(w.texts.get(&k), Some(&b), "{k}");
    }
    let (l, _) = guillemets();
    let overlay = extract::text::FR_OVERLAY[0].0;
    assert!(fixture_texts()[1].1.starts_with(l));
    assert_eq!(w.texts["Fix_Text_B"][0], overlay);

    // The trainers: by number, the tombstone an empty entry, held items by number.
    let t = &w.battle.trainers;
    assert_eq!(t.len(), 4);
    assert_eq!(t[0].id, "TRAINER_NONE");
    assert_eq!(t[1].id, TRAINER_A);
    assert!(t[2].party.is_empty() && t[2].id != TRAINER_GONE);
    assert_eq!(t[3].id, TRAINER_B);
    let item = IdKind::Items.range().0;
    assert_eq!(
        t[3].party[0].item.as_deref(),
        Some(format!("ITEM_0x{item:X}").as_str())
    );
    assert_eq!(t[3].party[0].moves.as_ref().unwrap().len(), 4, "padded");
    assert_eq!(t[3].party[1].item.as_deref(), Some("ITEM_ORAN_BERRY"));
    assert_eq!(t[1].party[0].item, None);
    assert_eq!(a.rules, classify::Rules::SILICE);
    assert!(w.battle.trainer_pics.contains_key(PIC));

    // Encounters by Silice map name, numbered by ids/maps.ron.
    assert_eq!(a.map_numbers.get(MAP), Some(&0));
    assert_eq!(w.battle.encounters[MAP].len(), 1);

    // Items after Gen3's, the base item's pocket and hold effect.
    let items = &w.tables.items.items;
    assert_eq!(items.len(), item as usize + 2);
    assert_eq!(items[item as usize].id, STONE);
    assert_eq!(items[item as usize].pocket, Some(ItemPocket::Items));
    assert_eq!(items[item as usize + 1].pocket, Some(ItemPocket::KeyItems));
    assert_eq!(a.item_bases, [(item as u16, BASE.to_string())]);

    // The generated sources.
    let g = codegen::generate(w, 1).unwrap();
    for (n, ty, v) in [
        (TRAINER_A, "TrainerId", "TrainerId(1)"),
        (TRAINER_B, "TrainerId", "TrainerId(3)"),
        (STONE, "u16", &format!("{item:#X}")),
        ("MAP_FIX", "MapId", "MapId(0x0)"),
        (LOCALID, "u8", "1"),
    ] {
        assert!(has_const(&g.consts, n, ty, v), "{n}");
    }
    assert!(!g.consts.contains(TRAINER_GONE));
    assert!(g.consts.contains("pub const Fix_Text_A: TextId"));
    assert!(
        g.battle
            .contains(&format!("{item} => Some(HoldEffect::Leftovers)"))
    );
    assert!(g.battle.contains("ENC_0_0"), "the land table of map 0");
    assert!(g.data.contains("pub static MARTS: [Mart; 1]"));
    assert!(g.data.contains(&format!("({item:#X}, 1200)")));
    assert!(s.report[0].starts_with("silice: 1 maps, 2 texts, 2 trainers, 1 encounter maps"));
}

#[test]
fn a_rebuild_is_byte_identical() {
    let f = fixture("rebuild");
    let a = generated(&world(&f, true).unwrap());
    let b = generated(&world(&f, true).unwrap());
    assert_eq!(a, b);
}

// ---- fixture: what fails ----

#[test]
fn an_unknown_tileset_fails() {
    let f = fixture("tileset");
    let p = f.1.join(format!("layouts/{MAP}.ron"));
    let s = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, s.replace("StubSecondary", "NoSuchTileset")).unwrap();
    let e = err(&f);
    assert!(
        e.contains("tileset NoSuchTileset is not in the extract"),
        "{e}"
    );
}

#[test]
fn an_unknown_script_fails_when_strict_and_is_dropped_otherwise() {
    let f = fixture("script");
    std::fs::remove_file(f.2.join("fix.rs")).unwrap();
    let e = err(&f);
    assert!(e.contains(&format!("{SCRIPT} (map {MAP})")), "{e}");
    let s = world(&f, false).unwrap();
    assert!(s.unwritten.contains(SCRIPT));
    assert_eq!(s.world.maps[0].objects[0].script, None);
}

#[test]
fn a_repeated_text_id_fails() {
    let f = fixture("dup");
    let mut t = fixture_texts();
    t.push(t[0].clone());
    write_texts(&f.1, &t);
    let e = err(&f);
    assert!(e.contains("text id Fix_Text_A is defined twice"), "{e}");
}

#[test]
fn an_unencodable_character_fails_unless_excepted_then_is_a_placeholder() {
    let f = fixture("charmap");
    let mut t = fixture_texts();
    let bad = '\u{266A}';
    t[0].1.push(bad);
    write_texts(&f.1, &t);
    let e = err(&f);
    assert!(e.contains("Fix_Text_A") && e.contains(bad), "{e}");
    write_exceptions(&f.1, &[("text-charmap", "Fix_Text_*")]);
    let s = world(&f, true).unwrap();
    let placeholder = t[0].1.replace(bad, &silice::text::PLACEHOLDER.to_string());
    let b = extract::text::encode_lang(&placeholder, extract::text::Lang::Fr).unwrap();
    assert_eq!(s.world.texts["Fix_Text_A"], b);
    assert!(
        s.report
            .iter()
            .any(|l| l.starts_with("placeholder: Fix_Text_A")),
        "{:?}",
        s.report
    );
}

#[test]
fn a_long_trainer_name_fails_unless_excepted_then_is_cut() {
    let f = fixture("name");
    let p = f.1.join("battle/trainers.ron");
    let long = "ABCDEFGHIJKL";
    let s = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, s.replace("\"AYE\"", &format!("{long:?}"))).unwrap();
    let e = err(&f);
    assert!(e.contains(TRAINER_A) && e.contains("over 10"), "{e}");
    write_exceptions(&f.1, &[("trainer-name", TRAINER_A)]);
    let s = world(&f, true).unwrap();
    assert_eq!(s.world.battle.trainers[1].name, long[..10]);
    assert!(
        s.report
            .iter()
            .any(|l| l.starts_with(&format!("truncated: {TRAINER_A}")))
    );
}

#[test]
fn an_incomplete_item_fails_unless_excepted() {
    let f = fixture("item");
    let p = f.1.join("items.ron");
    let s = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, s.replace("name: Some(\"STONE\")", "name: None")).unwrap();
    let e = err(&f);
    assert!(e.contains(STONE) && e.contains("no name"), "{e}");
    write_exceptions(&f.1, &[("item-incomplete", STONE)]);
    let s = world(&f, true).unwrap();
    let item = IdKind::Items.range().0 as usize;
    let words = STONE.trim_start_matches("ITEM_SIL_").replace('_', " ");
    assert_eq!(s.world.tables.items.items[item].name, words);
}

#[test]
fn maps_over_the_gen3_limits_fail() {
    /// (case, the edit, the expected message).
    type LimitCase = (&'static str, fn(&mut GameMap), &'static str);
    let cases: [LimitCase; 4] = [
        (
            "objects",
            |m| {
                m.objects = (0..65).map(|i| object(i % 63 + 1, None, 0)).collect();
            },
            "65 objects",
        ),
        (
            "localid",
            |m| m.objects[1].local_id = 1,
            "local id used twice",
        ),
        ("outside", |m| m.objects[0].x = 9, "outside the 4x4 map"),
        ("warp", |m| m.warps[0].dest_warp = 3, "leads to warp 3"),
    ];
    for (name, edit, want) in cases {
        let f = fixture(&format!("limit-{name}"));
        let mut m = fixture_map();
        edit(&mut m);
        put_ron!(&f.1, &format!("maps/{MAP}.ron"), &m);
        let e = err(&f);
        assert!(e.contains(want), "{name}: {e}");
    }
}

#[test]
fn an_unknown_header_name_fails() {
    let f = fixture("header");
    let mut m = fixture_map();
    m.header.music = "MUS_SIL_NOT_ALLOCATED".into();
    put_ron!(&f.1, &format!("maps/{MAP}.ron"), &m);
    let e = err(&f);
    assert!(e.contains("unknown song MUS_SIL_NOT_ALLOCATED"), "{e}");
}

/// The build script itself, with `PKR_SILICE_DIR` on a fixture games dir: an unknown tileset
/// fails the build (a nested `cargo check`, which stops in the build script).
#[test]
fn the_build_reads_pkr_silice_dir() {
    let f = fixture("env");
    let p = f.1.join(format!("layouts/{MAP}.ron"));
    let s = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, s.replace("StubPrimary", "NoSuchTileset")).unwrap();
    let target = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sil1-a-target");
    let out = std::process::Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR")
        .env("PKR_GAME", "silice")
        .env("PKR_EXTRACTED_DIR", &f.0)
        .env("PKR_SILICE_DIR", &f.1)
        .args(["check", "-p", "content", "--target-dir"])
        .arg(&target)
        .output()
        .expect("spawning cargo");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "{stderr}");
    assert!(
        stdout.contains("tileset NoSuchTileset is not in the extract")
            || stderr.contains("tileset NoSuchTileset is not in the extract"),
        "{stdout}\n{stderr}"
    );
}

// ---- the committed games dir ----

fn real_extract() -> Option<PathBuf> {
    let d = workspace().join("content/extracted");
    let ok = d.join("manifest.ron").exists()
        && input::format_of(&d.join("manifest.ron")) == Some(extract::schema::FORMAT_VERSION);
    if !ok {
        eprintln!("no current extract: skipped");
    }
    ok.then_some(d)
}

fn committed() -> PathBuf {
    workspace().join("games/silice")
}

#[test]
fn the_committed_games_dir_loads_with_a_constant_per_id() {
    let Some(x) = real_extract() else { return };
    let g = committed();
    let scripts = workspace().join("crates/content/src/silice/scripts");
    let s = silice::world(&x, &g, &scripts, false).unwrap();
    let consts = &s.world.authored.as_ref().unwrap().consts;
    let has = |n: &str| consts.iter().any(|c| c.0 == n);
    for m in silice::load::load_maps(&g).unwrap() {
        for o in &m.map.objects {
            if let Some(id) = &o.id {
                assert!(
                    consts.contains(&(id.clone(), "u8", o.local_id.to_string())),
                    "{id}"
                );
            }
        }
    }
    let trainers: TrainersFile = extract::export::read_ron(&g.join("battle/trainers.ron")).unwrap();
    for t in &trainers.trainers {
        assert!(has(&t.id), "{}", t.id);
    }
    let items: ItemsFile = extract::export::read_ron(&g.join("items.ron")).unwrap();
    for i in &items.items {
        assert!(has(&i.id), "{}", i.id);
    }
    for (k, _) in silice::text::read(&g.join("text/fr.ron")).unwrap() {
        assert!(s.world.texts.contains_key(&k), "{k}");
    }
    // Every placeholder and cut has its exception (the build enforces it) and a report line.
    assert!(s.report[0].starts_with("silice: "));
    let a = generated(&s);
    assert_eq!(
        a,
        generated(&silice::world(&x, &g, &scripts, false).unwrap())
    );
}

// ---- the Silice flavour's tables ----

#[cfg(pkr_silice)]
mod flavour {
    use super::*;
    use content::Game;
    use engine::data::{GameData, ItemId, MapId, TextId, TrainerId};

    fn ids_of(kind: IdKind) -> BTreeMap<String, u32> {
        let f: IdsFile =
            extract::export::read_ron(&committed().join(format!("ids/{}.ron", kind.file())))
                .unwrap();
        f.ids
            .into_iter()
            .filter(|e| !e.removed)
            .map(|e| (e.name, e.num))
            .collect()
    }

    fn item_number(items: &BTreeMap<String, u32>, name: &str) -> u16 {
        match items.get(name) {
            Some(&n) => n as u16,
            None => extract::names::ITEMS.value(name).unwrap() as u16,
        }
    }

    #[test]
    fn the_first_trainer_has_its_party() {
        let file: TrainersFile =
            extract::export::read_ron(&committed().join("battle/trainers.ron")).unwrap();
        let first = &file.trainers[0];
        let num = ids_of(IdKind::Trainers)[&first.id];
        assert_eq!(
            content::consts::TRAINER_SIL_GAMIN_TEO,
            TrainerId(num as u16)
        );
        assert_eq!(first.id, "TRAINER_SIL_GAMIN_TEO");
        let t = Game.trainer(TrainerId(num as u16)).unwrap();
        assert_eq!(t.party.len(), first.party.len());
        for (m, r) in t.party.iter().zip(&first.party) {
            assert_eq!(
                u32::from(m.species.0),
                extract::names::SPECIES.value(&r.species).unwrap()
            );
            assert_eq!(m.level, r.level);
            assert_eq!(u16::from(m.iv), r.iv);
        }
    }

    #[test]
    fn every_trainer_has_its_party_and_held_items() {
        let file: TrainersFile =
            extract::export::read_ron(&committed().join("battle/trainers.ron")).unwrap();
        let (tids, items) = (ids_of(IdKind::Trainers), ids_of(IdKind::Items));
        for r in &file.trainers {
            let t = Game.trainer(TrainerId(tids[&r.id] as u16)).unwrap();
            assert_eq!(t.party.len(), r.party.len(), "{}", r.id);
            assert_eq!(t.double, r.double, "{}", r.id);
            for (m, p) in t.party.iter().zip(&r.party) {
                let want = match p.item.as_deref() {
                    None | Some("ITEM_NONE") => None,
                    Some(i) => Some(ItemId(item_number(&items, i))),
                };
                assert_eq!(m.item, want, "{}", r.id);
                if let Some(ms) = &p.moves {
                    let got = m.moves.unwrap();
                    for (k, mv) in ms.iter().enumerate() {
                        let v = extract::names::MOVES.value(mv).unwrap() as u16;
                        assert_eq!(got[k].map_or(0, |x| x.0), v, "{}", r.id);
                    }
                }
            }
        }
    }

    #[test]
    fn the_encounters_are_keyed_by_silice_map() {
        let file: EncountersFile =
            extract::export::read_ron(&committed().join("battle/encounters.ron")).unwrap();
        let maps: IdsFile = extract::export::read_ron(&committed().join("ids/maps.ron")).unwrap();
        for (name, headers) in &file.maps {
            let num = maps
                .ids
                .iter()
                .find(|e| e.map.as_deref() == Some(name))
                .unwrap()
                .num;
            let got = Game.wild_encounters(MapId(num as u16), 0);
            match &headers[0].land {
                Some(l) => {
                    let got = got.unwrap();
                    assert_eq!(got.rate, l.rate, "{name}");
                    for (g, w) in got.mons.iter().zip(&l.mons) {
                        assert_eq!(
                            u32::from(g.species.0),
                            extract::names::SPECIES.value(&w.species).unwrap()
                        );
                        assert_eq!((g.min_level, g.max_level), (w.min_level, w.max_level));
                    }
                }
                None => assert!(got.is_none(), "{name}"),
            }
        }
    }

    #[test]
    fn the_items_and_their_hold_effects() {
        let file: ItemsFile = extract::export::read_ron(&committed().join("items.ron")).unwrap();
        let items = ids_of(IdKind::Items);
        for d in &file.items {
            let id = ItemId(items[&d.id] as u16);
            let info = Game.item(id).unwrap();
            if let Some(p) = d.price {
                assert_eq!(info.price, p, "{}", d.id);
            }
            if let Some(base) = &d.clone_of {
                let b = ItemId(extract::names::ITEMS.value(base).unwrap() as u16);
                assert_eq!(Game.hold_effect(id), Game.hold_effect(b), "{}", d.id);
            }
        }
        assert_eq!(content::data::MARTS.len(), file.marts.len());
        assert!(Game.form_changes().is_empty());
    }

    #[test]
    fn the_texts_are_the_encoded_ron_texts() {
        let texts = silice::text::read(&committed().join("text/fr.ron")).unwrap();
        let names = &content::data::TEXT_NAMES;
        let mut checked = 0;
        for (k, v) in &texts {
            let i = names.iter().position(|n| n == k).unwrap();
            // Texts with an excepted character hold a placeholder instead.
            if let Ok(b) = extract::text::encode_lang(v, extract::text::Lang::Fr) {
                assert_eq!(Game.text(TextId(i as u16)), &b[..], "{k}");
                checked += 1;
            }
        }
        assert!(checked > texts.len() / 2);
    }
}

/// The glyph hook's characters encode as their bytes, one or several, and count as encodable.
#[test]
fn hook_glyphs_encode_as_their_bytes() {
    let (one, two) = ('\u{2192}', '\u{2665}');
    let enc = silice::text::Encoder::new(
        extract::text::Lang::Fr,
        vec![(one, vec![0x37]), (two, vec![0xF9, 0xE7])],
    );
    assert_eq!(
        enc.encode(&format!("{one}{two}")).unwrap(),
        [0x37, 0xF9, 0xE7, 0xFF]
    );
    assert!(enc.bad_chars(&format!("{one}{two}")).is_empty());
}
