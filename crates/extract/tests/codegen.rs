//! Script codegen: golden output for a synthetic ROM, and a compile check of generated code
//! against the stub prelude in `tests/stub_prelude/` wrapped exactly as `content` wraps it.
use extract::codegen::{self, Generated, MapRoots};
use extract::schema::{LegacyScriptEntry, TextFile};
use extract::{ROM_BASE, Rom, export, maps, names, roots, text};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A tiny assembler: bytes from file offset `BASE`, with labels and 4-byte pointer fixups.
struct Asm {
    bytes: Vec<u8>,
    labels: BTreeMap<&'static str, u32>,
    fixups: Vec<(usize, &'static str)>,
}

const BASE: u32 = 0x100;

impl Asm {
    fn new() -> Asm {
        let mut bytes = vec![0u8; BASE as usize];
        bytes[0xAC..0xB0].copy_from_slice(b"BPEE");
        Asm {
            bytes,
            labels: BTreeMap::new(),
            fixups: Vec::new(),
        }
    }
    fn l(&mut self, name: &'static str) -> &mut Self {
        self.labels.insert(name, self.bytes.len() as u32);
        self
    }
    fn b(&mut self, b: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(b);
        self
    }
    fn h(&mut self, h: u16) -> &mut Self {
        self.b(&h.to_le_bytes())
    }
    fn p(&mut self, label: &'static str) -> &mut Self {
        self.fixups.push((self.bytes.len(), label));
        self.b(&[0; 4])
    }
    fn text(&mut self, label: &'static str, s: &str) -> &mut Self {
        self.l(label).b(&text::encode(s).unwrap())
    }
    fn at(&self, label: &str) -> u32 {
        self.labels[label]
    }
    fn rom(&self) -> Rom {
        let mut b = self.bytes.clone();
        for &(at, label) in &self.fixups {
            b[at..at + 4].copy_from_slice(&(ROM_BASE + self.labels[label]).to_le_bytes());
        }
        b.resize(b.len().next_multiple_of(0x100) + 0x100, 0xFF);
        Rom::from_bytes(b).unwrap()
    }
}

/// Map `A`: `A_Obj1` (flag test, msgbox, call, a counting loop), `A_Obj2` (movement, then a
/// `goto` into A_Obj1's code, which makes that block a function). Map `B`: `B_Bg0` calls
/// `A_Obj1` across files and a subroutine; `B_Bg1` hits an invalid opcode.
fn synthetic() -> (Rom, Vec<MapRoots>) {
    let mut a = Asm::new();
    a.l("a1")
        .b(&[0x6a, 0x5a]) // lock ; faceplayer
        .b(&[0x2b])
        .h(0x20) // checkflag FLAG_0x20
        .b(&[0x06, 1])
        .p("seen") // goto_if_eq seen
        .b(&[0x0f, 0])
        .p("t_hello")
        .b(&[0x09, 2]) // msgbox t_hello, MSGBOX_NPC
        .b(&[0x04])
        .p("sub") // call sub
        .b(&[0x16])
        .h(0x4050)
        .h(0) // setvar VAR_0x4050, 0
        .l("count")
        .b(&[0x17])
        .h(0x4050)
        .h(1) // addvar
        .b(&[0x21])
        .h(0x4050)
        .h(3) // compare_var_to_value
        .b(&[0x06, 0])
        .p("count") // goto_if_lt count
        .b(&[0x6c, 0x02]) // release ; end
        .l("seen")
        .b(&[0x67])
        .p("t_again") // message
        .b(&[0x66, 0x6d, 0x6c, 0x02]) // waitmessage ; waitbuttonpress ; release ; end
        .l("a2")
        .b(&[0x4f])
        .h(0xff)
        .p("mv") // applymovement LOCALID_PLAYER, mv
        .b(&[0x51])
        .h(0) // waitmovement 0
        .b(&[0x59])
        .h(0xff)
        .b(&[0, 16]) // hideobjectat LOCALID_PLAYER, MAP_ROUTE101 (another map)
        .b(&[0xac])
        .h(0x4051)
        .h(3) // opendoor VAR_0x4051, 3
        .b(&[0xae, 0xa0]) // waitdooranim ; checkplayergender
        .b(&[0x05])
        .p("seen") // goto seen
        .l("b0")
        .b(&[0x04])
        .p("sub")
        .b(&[0x04])
        .p("a1")
        .b(&[0x39])
        .b(&[0, 9, 0xff])
        .h(0x4051)
        .h(5) // warp MAP_LITTLEROOT_TOWN, x = VAR_0x4051, y = 5
        .b(&[0x02])
        .l("sub")
        .b(&[0x29])
        .h(0x3fff) // setflag (no name)
        .b(&[0x8f])
        .h(0x4051) // random VAR_0x4051
        .b(&[0x03]) // return
        .l("b1")
        .b(&[0xf0])
        .l("mv")
        .b(&[0x09, 0x51, 0x4f, 0xfe]) // walk_up ; disable_jump_landing_ground_effect ;
        // nurse_joy_bow_down ; step_end
        .text("t_hello", "Hello!")
        .text("t_again", "Again?");
    let maps = vec![
        MapRoots {
            map: "A".into(),
            roots: vec![
                ("A_Obj1".into(), a.at("a1")),
                ("A_Obj2".into(), a.at("a2")),
                ("A_Bg0".into(), a.at("a1")), // shares A_Obj1's script
            ],
        },
        MapRoots {
            map: "B".into(),
            roots: vec![("B_Bg0".into(), a.at("b0")), ("B_Bg1".into(), a.at("b1"))],
        },
    ];
    (a.rom(), maps)
}

const HEADER: &str = "// @generated test\n";

const GOLDEN_A: &str = r#"// @generated test

pub async fn A_Obj1(ctx: Ctx) {
    let mut cond = Ordering::Less;
    let mut pc: u16 = 0;
    loop {
        match pc {
            0 => {
                // A_Obj1
                ctx.lock().await;
                ctx.face_player().await;
                cond = if ctx.flag(FLAG_UNUSED_0x020).await { Ordering::Equal } else { Ordering::Less };
                if cond == Ordering::Equal {
                    Box::pin(A_Obj1_L0(ctx.clone())).await;
                    return;
                }
                msgbox(&ctx, A_Obj1_T0, MsgBox::Npc).await;
                Box::pin(A_Obj1_L1(ctx.clone())).await;
                ctx.set_var(VAR_LITTLEROOT_TOWN_STATE, 0).await;
                pc = 1;
                continue;
            }
            1 => {
                // A_Obj1_L2
                ctx.set_var(VAR_LITTLEROOT_TOWN_STATE, ctx.var(VAR_LITTLEROOT_TOWN_STATE).await.wrapping_add(1)).await;
                cond = ctx.var(VAR_LITTLEROOT_TOWN_STATE).await.cmp(&3);
                if cond == Ordering::Less {
                    pc = 1;
                    continue;
                }
                ctx.release().await;
                return;
            }
            _ => unreachable!(),
        }
    }
}

pub async fn A_Obj1_L0(ctx: Ctx) {
    ctx.message(A_Obj1_L0_T0).await;
    ctx.wait_message().await;
    ctx.wait_button().await;
    ctx.release().await;
}

pub async fn A_Obj1_L1(ctx: Ctx) {
    ctx.set_flag(FlagId(0x3fff), true).await;
    ctx.set_var(VAR_RESULT, ctx.random(ctx.var(VAR_OLDALE_TOWN_STATE).await).await).await;
}

pub async fn A_Obj2(ctx: Ctx) {
    ctx.move_obj(PLAYER, &A_Obj2_M0).await;
    ctx.wait_move(PLAYER).await;
    ctx.hide_object(PLAYER).await; // hideobjectat
    ctx.door(ctx.var(VAR_OLDALE_TOWN_STATE).await as i16, 3, true).await;
    ctx.wait_door().await;
    ctx.set_var(VAR_RESULT, check_player_gender(&ctx).await).await;
    Box::pin(A_Obj1_L0(ctx.clone())).await;
}

pub static A_Obj2_M0: [Step; 2] = [
    Step::Walk(Dir::Up, Speed::Normal), // WALK_NORMAL_UP
    // DISABLE_JUMP_LANDING_GROUND_EFFECT (no effect here)
    Step::Delay(0), // NURSE_JOY_BOW_DOWN (unsupported)
];
"#;

const GOLDEN_B: &str = r#"// @generated test

pub async fn B_Bg0(ctx: Ctx) {
    Box::pin(super::A::A_Obj1_L1(ctx.clone())).await;
    Box::pin(super::A::A_Obj1(ctx.clone())).await;
    // note: in Gen3, an `end` inside A_Obj1 also ends this script
    ctx.warp(MAP_LITTLEROOT_TOWN, None, ctx.var(VAR_OLDALE_TOWN_STATE).await as i16, 5).await;
}

pub async fn B_Bg1(ctx: Ctx) {
    unsupported(&ctx, "unknown opcode 0xf0").await; // at 0x16d
}
"#;

#[test]
fn synthetic_golden() {
    let (rom, maps) = synthetic();
    let g = codegen::generate(&rom, &maps, HEADER).unwrap();
    assert_eq!(g.files["A"], GOLDEN_A, "{}", g.files["A"]);
    assert_eq!(g.files["B"], GOLDEN_B, "{}", g.files["B"]);
    assert_eq!(
        g.texts,
        TextFile::from([
            ("A_Obj1_T0".into(), "Hello!".into()),
            ("A_Obj1_L0_T0".into(), "Again?".into())
        ])
    );
    let entry = |file: &str, root| LegacyScriptEntry {
        file: file.into(),
        root,
    };
    assert_eq!(
        g.index.scripts,
        BTreeMap::from([
            ("A_Obj1".into(), entry("A", true)),
            ("A_Obj2".into(), entry("A", true)),
            ("A_Obj1_L0".into(), entry("A", false)),
            ("A_Obj1_L1".into(), entry("A", false)),
            ("B_Bg0".into(), entry("B", true)),
            ("B_Bg1".into(), entry("B", true)),
        ])
    );
    assert_eq!(
        g.index.movements,
        BTreeMap::from([("A_Obj2_M0".into(), "A".into())])
    );
    let s = &g.stats;
    assert_eq!((s.fns, s.labels, s.texts, s.movements), (6, 7, 2, 1));
    assert_eq!(s.unknown_ops, BTreeMap::from([(0x16d, 0xf0)]));
    assert_eq!(s.end_in_callee, 1);
    assert_eq!(
        s.unknown_movement_actions,
        BTreeMap::from([("MOVEMENT_ACTION_NURSE_JOY_BOW_DOWN".into(), 1)])
    );
    assert_eq!(
        s.summary(),
        "script codegen: 6 fns, 2 texts, 1 movements; unsupported: unknown opcode 1; unknown \
         movement actions: MOVEMENT_ACTION_NURSE_JOY_BOW_DOWN 1; end in callee: 1; unresolved \
         waitmovement: 0"
    );
}

#[test]
fn synthetic_compiles() {
    let (rom, maps) = synthetic();
    let g = codegen::generate(&rom, &maps, HEADER).unwrap();
    let root = write_generated("synthetic", &g, "en");
    compile_check("synthetic", &root);
}

/// Map `C`, one root per `trainerbattle` shape (`include/constants/battle_setup.h`):
/// `C_Single` (type 0, then the already-beaten message), `C_Continue` (type 2 with a continue
/// script), `C_NoIntro` (type 3, then `givemon`, `getpartysize`, `setrespawn` and a type 4
/// double, which is unsupported).
fn synthetic_battle() -> (Rom, Vec<MapRoots>) {
    let mut a = Asm::new();
    a.l("single")
        .b(&[0x5c, 0])
        .h(1) // TRAINER_SAWYER_1
        .h(3)
        .p("t_intro")
        .p("t_defeat") // trainerbattle_single TRAINER_SAWYER_1, intro, defeat (local 3)
        .b(&[0x67])
        .p("t_after") // message
        .b(&[0x66, 0x6d, 0x6c, 0x02]) // waitmessage ; waitbuttonpress ; release ; end
        .l("continue")
        .b(&[0x5c, 2])
        .h(2)
        .h(4)
        .p("t_intro")
        .p("t_defeat")
        .p("cont") // trainerbattle_single TRAINER_GRUNT_AQUA_HIDEOUT_1, …, cont
        .b(&[0x6c, 0x02]) // release ; end
        .l("cont")
        .b(&[0x29])
        .h(0x20) // setflag FLAG_0x20
        .b(&[0x6c, 0x02])
        .l("nointro")
        .b(&[0x5c, 3])
        .h(1)
        .h(0)
        .p("t_defeat") // trainerbattle_no_intro TRAINER_SAWYER_1, defeat
        .b(&[0x79])
        .h(0x115)
        .b(&[5])
        .h(0)
        .b(&[0; 9]) // givemon SPECIES_TREECKO, 5
        .b(&[0x79])
        .h(0x4051)
        .b(&[7])
        .h(13)
        .b(&[0; 9]) // givemon VAR_0x4051, 7, ITEM_POTION
        .b(&[0x43]) // getpartysize
        .b(&[0x9f])
        .h(0xe) // setrespawn HEAL_LOCATION_OLDALE_TOWN
        .b(&[0x5c, 4])
        .h(1)
        .h(0)
        .p("t_intro")
        .p("t_defeat")
        .p("t_after") // trainerbattle_double (not enough mons text)
        .b(&[0x02])
        .text("t_intro", "Fight!")
        .text("t_defeat", "Lost!")
        .text("t_after", "Again?");
    let maps = vec![MapRoots {
        map: "C".into(),
        roots: vec![
            ("C_Single".into(), a.at("single")),
            ("C_Continue".into(), a.at("continue")),
            ("C_NoIntro".into(), a.at("nointro")),
        ],
    }];
    (a.rom(), maps)
}

const GOLDEN_C: &str = r#"// @generated test

pub async fn C_Single(ctx: Ctx) {
    if legacy_trainer_battle(&ctx, LegacyTrainerBattle::single(TRAINER_SAWYER_1, 3, C_Single_T0, C_Single_T1)).await {
        return;
    }
    ctx.message(C_Single_T2).await;
    ctx.wait_message().await;
    ctx.wait_button().await;
    ctx.release().await;
}

pub async fn C_Continue(ctx: Ctx) {
    if legacy_trainer_battle(&ctx, LegacyTrainerBattle::continue_script(TRAINER_GRUNT_AQUA_HIDEOUT_1, 4, C_Single_T0, C_Single_T1)).await {
        Box::pin(C_Continue_L0(ctx.clone())).await;
        return;
    }
    ctx.release().await;
}

pub async fn C_Continue_L0(ctx: Ctx) {
    ctx.set_flag(FLAG_UNUSED_0x020, true).await;
    ctx.release().await;
}

pub async fn C_NoIntro(ctx: Ctx) {
    legacy_trainer_battle(&ctx, LegacyTrainerBattle::no_intro(TRAINER_SAWYER_1, C_Single_T1)).await;
    ctx.set_var(VAR_RESULT, ctx.give_mon(SPECIES_TREECKO, 5).await).await;
    ctx.set_var(VAR_RESULT, ctx.give_mon(SpeciesId(ctx.var(VAR_OLDALE_TOWN_STATE).await), 7).await).await; // held item 13 not given
    ctx.set_var(VAR_RESULT, ctx.party_size().await).await;
    ctx.set_respawn(HEAL_LOCATION_OLDALE_TOWN).await;
    unsupported(&ctx, "trainerbattle").await; // 4, 0x1, 0x0, C_Single_T0, C_Single_T1, C_Single_T2
    // what follows is the already-beaten path
}
"#;

#[test]
fn synthetic_battle_golden_and_compiles() {
    let (rom, maps) = synthetic_battle();
    let g = codegen::generate(&rom, &maps, HEADER).unwrap();
    assert_eq!(g.files["C"], GOLDEN_C, "{}", g.files["C"]);
    assert_eq!(
        g.stats.unsupported,
        BTreeMap::from([("trainerbattle".into(), 1)])
    );
    let root = write_generated("battle", &g, "en");
    compile_check("battle", &root);
}

/// Writes `scripts/<Map>.rs` and `text/<lang>.ron` under a fresh temp dir.
fn write_generated(tag: &str, g: &Generated, lang: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("codegen-{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::create_dir_all(root.join("text")).unwrap();
    for (map, src) in &g.files {
        std::fs::write(root.join(format!("scripts/{map}.rs")), src).unwrap();
    }
    std::fs::write(
        root.join(format!("text/{lang}.ron")),
        export::to_ron(&g.texts).unwrap(),
    )
    .unwrap();
    root
}

/// The constants the prelude contract promises, as Rust items. A name used for several numbers
/// (`SPECIAL_ShowMapNamePopup` is two `gSpecials` entries for one function) is defined once,
/// with its first number; `STD_SCRIPTS`' `MSGBOX_*` entries are not `STD_*` constants. Text ids
/// follow the sorted key order of `texts`; map ids are Gen3 `num | group << 8`, as content's.
fn prelude_consts(texts: &TextFile) -> String {
    let mut s = String::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut table = |s: &mut String, t: &names::Table, ty: &str, wrap: &str| {
        for (v, n) in t.iter() {
            if n.starts_with(t.prefix) && seen.insert(n) {
                let v = if wrap.is_empty() {
                    format!("{v:#x}")
                } else {
                    format!("{wrap}({v:#x})")
                };
                *s += &format!("pub const {n}: {ty} = {v};\n");
            }
        }
    };
    table(&mut s, &names::FLAGS, "FlagId", "FlagId");
    table(&mut s, &names::VARS, "VarId", "VarId");
    table(&mut s, &names::SPECIALS, "u16", "");
    table(&mut s, &names::STD_SCRIPTS, "u8", "");
    table(&mut s, &names::OBJ_EVENT_GFX, "u16", "");
    table(&mut s, &names::ITEMS, "u16", "");
    table(&mut s, &names::SPECIES, "SpeciesId", "SpeciesId");
    table(&mut s, &names::TRAINERS, "TrainerId", "TrainerId");
    table(&mut s, &names::HEAL_LOCATIONS, "u8", "");
    for m in names::MAPS {
        let id = u16::from(m.num) | u16::from(m.group) << 8;
        s += &format!("pub const {}: MapId = MapId({id:#x});\n", m.constant);
    }
    for (i, k) in texts.keys().enumerate() {
        s += &format!("pub const {k}: TextId = TextId({i});\n");
    }
    s
}

/// Builds a scratch crate that wraps every `<root>/scripts/<Map>.rs` exactly as `content` does
/// and runs `cargo check` on it with warnings denied, against the real `engine` crate.
fn compile_check(tag: &str, root: &Path) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let dir = tmp.join(format!("stub-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let engine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../engine")
        .canonicalize()
        .unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"stub_scripts\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
             publish = false\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\n\
             engine = {{ path = {:?} }}\n\n[workspace]\n",
            engine.to_str().unwrap()
        ),
    )
    .unwrap();

    let text_file = std::fs::read_dir(root.join("text"))
        .unwrap()
        .next()
        .expect("a text file")
        .unwrap()
        .path();
    let texts: TextFile = export::read_ron(&text_file).unwrap();
    let mut prelude = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stub_prelude/prelude.rs"),
    )
    .unwrap();
    prelude += &prelude_consts(&texts);
    std::fs::write(dir.join("prelude.rs"), prelude).unwrap();

    let mut maps: Vec<String> = std::fs::read_dir(root.join("scripts"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    maps.sort();
    let mut lib = String::from(
        "#[allow(non_upper_case_globals)]\npub mod prelude {\n    include!(\"prelude.rs\");\n}\n\npub mod scripts {\n",
    );
    for m in &maps {
        lib += &format!(
            "    #[allow(non_snake_case, non_upper_case_globals, unused_mut, unused_variables, \
             unused_assignments, unreachable_code, clippy::all)] pub mod {m} {{ use crate::prelude::*; \
             include!(concat!(env!(\"PKR_EXTRACTED\"), \"/scripts/{m}.rs\")); }}\n"
        );
    }
    lib += "}\n";
    std::fs::write(dir.join("lib.rs"), lib).unwrap();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["check", "--offline", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", tmp.join("stub-target"))
        .env("PKR_EXTRACTED", root)
        .env("RUSTFLAGS", "-D warnings")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "{tag}: generated code does not compile:\n{stderr}"
    );
}

/// The roots of every exported map of `rom`, in export order.
fn rom_roots(rom: &Rom) -> Vec<MapRoots> {
    let table = maps::locate_map_table(rom).unwrap();
    export::SLICE_MAPS
        .iter()
        .map(|&name| {
            let m = names::map_by_dir(name).unwrap();
            let id = maps::MapId {
                group: m.group,
                num: m.num,
            };
            let header = table.header(rom, id).unwrap();
            MapRoots {
                map: name.into(),
                roots: roots::named_roots(rom, &header, name).unwrap(),
            }
        })
        .collect()
}

fn rom_from_env(var: &str) -> Option<Rom> {
    match std::env::var_os(var) {
        Some(p) => Some(Rom::open(Path::new(&p)).unwrap()),
        None => {
            eprintln!("skipped: set {var} to run this test");
            None
        }
    }
}

#[test]
fn fr_scripts_are_deterministic_round_trip_and_compile() {
    let Some(rom) = rom_from_env("PKR_ROM_FR") else {
        return;
    };
    let maps = rom_roots(&rom);
    let g = codegen::generate(&rom, &maps, HEADER).unwrap();
    let again = codegen::generate(&rom, &maps, HEADER).unwrap();
    assert_eq!(g.files, again.files);
    assert_eq!(g.texts, again.texts);
    assert_eq!(g.index, again.index);

    // Every text re-encodes to its exact ROM bytes.
    for (name, s) in &g.texts {
        let bytes = text::encode_lang(s, rom.lang()).unwrap();
        let off = g.text_offsets[name] as usize;
        assert_eq!(&rom.bytes()[off..off + bytes.len()], &bytes[..], "{name}");
    }

    let s = &g.stats;
    assert!(s.fns > 100 && s.texts > 100 && s.movements > 50, "{s:#?}");
    assert!(s.unknown_ops.is_empty(), "{:x?}", s.unknown_ops);
    let mut by_count: Vec<(&String, &usize)> = s.unsupported.iter().collect();
    by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    eprintln!(
        "FR: {} fns, {} labels, {} texts, {} movements; end in callee {}, foreign cond {}, \
         unresolved waitmovement {}",
        s.fns,
        s.labels,
        s.texts,
        s.movements,
        s.end_in_callee,
        s.foreign_cond,
        s.unresolved_waitmovement
    );
    eprintln!("unsupported: {by_count:?}");
    eprintln!("unknown movement actions: {:?}", s.unknown_movement_actions);
    eprintln!("specials: {:?}", s.specials);
    eprintln!("stds: {:?}", s.stds);

    let root = write_generated("fr", &g, "fr");
    compile_check("fr", &root);
}

/// `LittlerootTown_Obj1` (`LittlerootTown_EventScript_Twin`) as the decomp writes it:
/// `lock ; faceplayer ; goto_if_set FLAG_ADVENTURE_STARTED, …GoodLuck ; goto_if_set
/// FLAG_RESCUED_BIRCH, …YouSavedBirch ; goto_if_ne VAR_LITTLEROOT_TOWN_STATE, 0, …GoSaveBirch ;
/// msgbox …CanUsePokemon, MSGBOX_DEFAULT ; release ; end`, one block per branch.
#[test]
fn us_littleroot_twin_matches_the_decomp() {
    let Some(rom) = rom_from_env("PKR_ROM_US") else {
        return;
    };
    let g = codegen::generate(&rom, &rom_roots(&rom), HEADER).unwrap();
    let src = &g.files["LittlerootTown"];
    let start = src.find("pub async fn LittlerootTown_Obj1(").unwrap();
    let end = start + src[start..].find("\n}\n").unwrap() + 3;
    assert_eq!(&src[start..end], US_TWIN, "{}", &src[start..end]);
}

const US_TWIN: &str = r#"pub async fn LittlerootTown_Obj1(ctx: Ctx) {
    let mut cond = Ordering::Less;
    let mut pc: u16 = 0;
    loop {
        match pc {
            0 => {
                // LittlerootTown_Obj1
                ctx.lock().await;
                ctx.face_player().await;
                cond = if ctx.flag(FLAG_ADVENTURE_STARTED).await { Ordering::Equal } else { Ordering::Less };
                if cond == Ordering::Equal {
                    pc = 1;
                    continue;
                }
                cond = if ctx.flag(FLAG_RESCUED_BIRCH).await { Ordering::Equal } else { Ordering::Less };
                if cond == Ordering::Equal {
                    pc = 2;
                    continue;
                }
                cond = ctx.var(VAR_LITTLEROOT_TOWN_STATE).await.cmp(&0);
                if cond != Ordering::Equal {
                    pc = 3;
                    continue;
                }
                msgbox(&ctx, LittlerootTown_Obj1_T0, MsgBox::Default).await;
                ctx.release().await;
                return;
            }
            1 => {
                // LittlerootTown_Obj1_L0
                msgbox(&ctx, LittlerootTown_Obj1_L0_T0, MsgBox::Default).await;
                ctx.release().await;
                return;
            }
            2 => {
                // LittlerootTown_Obj1_L1
                special(&ctx, SPECIAL_GetPlayerBigGuyGirlString).await;
                msgbox(&ctx, LittlerootTown_Obj1_L1_T0, MsgBox::Default).await;
                ctx.release().await;
                return;
            }
            3 => {
                // LittlerootTown_Obj1_L2
                special(&ctx, SPECIAL_GetPlayerBigGuyGirlString).await;
                msgbox(&ctx, LittlerootTown_Obj1_L2_T0, MsgBox::Default).await;
                ctx.close_message().await;
                ctx.move_obj(1, &LittlerootTown_Obj1_L2_M0).await;
                ctx.wait_move(1).await;
                ctx.set_var(VAR_LITTLEROOT_TOWN_STATE, 2).await;
                ctx.release().await;
                return;
            }
            _ => unreachable!(),
        }
    }
}
"#;

// ---- Milestone 8 Task D1: `codegen::lower` on hand-written IR ----

mod lowering {
    use IrArg::*;
    use extract::codegen::{self, Class, Lowered, Support};
    use extract::names;
    use extract::schema::{
        Hex, IrArg, IrInsn, IrLabel, IrMovement, NameOrigin, RootKind, ScriptEntry, ScriptFile,
        ScriptIndex,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;
    use std::process::Command;

    fn i(op: &str, args: Vec<IrArg>) -> IrInsn {
        IrInsn {
            op: op.to_string(),
            args,
        }
    }

    fn label(
        name: &str,
        root: Option<RootKind>,
        insns: Vec<IrInsn>,
        next: Option<&str>,
    ) -> IrLabel {
        IrLabel {
            name: name.to_string(),
            origin: NameOrigin::Decomp,
            offset: Hex(0),
            root,
            insns,
            next: next.map(str::to_string),
        }
    }

    fn file(module: &str, labels: Vec<IrLabel>, movements: Vec<IrMovement>) -> ScriptFile {
        ScriptFile {
            module: module.to_string(),
            source: None,
            labels,
            movements,
        }
    }

    fn s(name: &str) -> IrArg {
        Script(name.to_string())
    }
    fn t(name: &str) -> IrArg {
        Text(name.to_string())
    }
    fn v(name: &str) -> IrArg {
        Var(name.to_string())
    }
    fn f(name: &str) -> IrArg {
        Flag(name.to_string())
    }
    fn c(name: &str) -> IrArg {
        let table = name.rsplit_once('_').map_or(name, |p| p.0).to_string();
        Const {
            table,
            name: name.to_string(),
        }
    }

    /// The index a Task B export would write for `files` (texts from the IR's text operands).
    fn index(files: &[ScriptFile]) -> ScriptIndex {
        let mut ix = ScriptIndex::default();
        for f in files {
            for l in &f.labels {
                ix.scripts.insert(
                    l.name.clone(),
                    ScriptEntry {
                        module: f.module.clone(),
                        root: l.root.is_some(),
                        origin: l.origin,
                        offset: l.offset,
                    },
                );
                for insn in &l.insns {
                    for a in &insn.args {
                        if let Text(t) = a {
                            ix.texts.insert(t.clone(), f.module.clone());
                        }
                    }
                }
            }
        }
        ix
    }

    fn commands_ron() -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/build/support/commands.ron"),
        )
        .unwrap()
    }

    /// The committed `commands.ron` plus a few specials of each class.
    fn support() -> Support {
        let mut s = Support::parse(&commands_ron(), None).unwrap();
        s.specials = BTreeMap::from([
            ("SPECIAL_HealPlayerParty".to_string(), Class::Native),
            ("SPECIAL_ShowMapNamePopup".to_string(), Class::Ignore),
            (
                "SPECIAL_ShouldTryRematchBattle".to_string(),
                Class::Default(0),
            ),
            ("SPECIAL_DoPCTurnOnEffect".to_string(), Class::Unsupported),
        ]);
        s
    }

    fn lower(files: &[ScriptFile]) -> Lowered {
        lower_with(files, &BTreeMap::new())
    }

    fn lower_with(files: &[ScriptFile], overrides: &BTreeMap<String, String>) -> Lowered {
        codegen::lower(files, &index(files), &support(), overrides).unwrap()
    }

    /// The source of `pub async fn <name>` in `src`.
    fn fn_src<'a>(src: &'a str, name: &str) -> &'a str {
        let start = src
            .find(&format!("pub async fn {name}("))
            .unwrap_or_else(|| panic!("no fn {name} in:\n{src}"));
        let end = start + src[start..].find("\n}\n").unwrap() + 3;
        &src[start..end]
    }

    fn kid() -> Vec<ScriptFile> {
        vec![file(
            "Town",
            vec![
                label(
                    "Town_EventScript_Kid",
                    Some(RootKind::Object(1)),
                    vec![
                        i("lock", vec![]),
                        i("faceplayer", vec![]),
                        i("checkflag", vec![f("FLAG_SYS_POKEMON_GET")]),
                        i("goto_if", vec![Byte(1), s("Town_EventScript_KidHasMon")]),
                        i("setvar", vec![v("VAR_0x8004"), Half(0)]),
                    ],
                    Some("Town_EventScript_KidLoop"),
                ),
                label(
                    "Town_EventScript_KidLoop",
                    None,
                    vec![
                        i("addvar", vec![v("VAR_0x8004"), Half(1)]),
                        i("compare_var_to_value", vec![v("VAR_0x8004"), Half(3)]),
                        i("goto_if", vec![Byte(0), s("Town_EventScript_KidLoop")]),
                        i("loadword", vec![Byte(0), t("Town_Text_Kid")]),
                        i("callstd", vec![c("MSGBOX_NPC")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "Town_EventScript_KidHasMon",
                    None,
                    vec![
                        i("loadword", vec![Byte(0), t("Town_Text_HasMon")]),
                        i("callstd", vec![Byte(4)]),
                        i("release", vec![]),
                        i("end", vec![]),
                    ],
                    None,
                ),
            ],
            vec![],
        )]
    }

    const KID: &str = r#"pub async fn Town_EventScript_Kid(ctx: Ctx) -> Flow {
    let mut pc: u16 = 0;
    loop {
        match pc {
            0 => {
                // Town_EventScript_Kid
                ctx.lock().await;
                ctx.face_player().await;
                ctx.set_cond(if ctx.flag(FLAG_SYS_POKEMON_GET).await { Ordering::Equal } else { Ordering::Less });
                if ctx.cond() == Ordering::Equal {
                    pc = 2;
                    continue;
                }
                ctx.set_var(VAR_0x8004, 0).await;
                pc = 1;
                continue;
            }
            1 => {
                // Town_EventScript_KidLoop
                ctx.set_var(VAR_0x8004, ctx.var(VAR_0x8004).await.wrapping_add(1)).await;
                ctx.set_cond(ctx.var(VAR_0x8004).await.cmp(&3));
                if ctx.cond() == Ordering::Less {
                    pc = 1;
                    continue;
                }
                msgbox(&ctx, Town_Text_Kid, MsgBox::Npc).await;
                return Flow::End;
            }
            2 => {
                // Town_EventScript_KidHasMon
                msgbox(&ctx, Town_Text_HasMon, MsgBox::Default).await;
                ctx.release().await;
                return Flow::End;
            }
            _ => return Flow::End,
        }
    }
}
"#;

    #[test]
    fn lower_structured_flow_golden() {
        let out = lower(&kid());
        let src = &out.modules["Town"];
        assert!(src.starts_with("use super::Flow;\n"), "{src}");
        assert_eq!(fn_src(src, "Town_EventScript_Kid"), KID, "{src}");
        // Blocks owned by one function are arms, not functions.
        assert_eq!(out.stats.fns, 1);
        assert_eq!(out.stats.labels, 3);
        assert_eq!(out.stats.texts, 2);
        assert!(out.stats.reports.is_empty(), "{:?}", out.stats.reports);
        assert_eq!(out.stats.named_decomp_pct["functions"], 100);
    }

    #[test]
    fn lower_is_deterministic() {
        assert_eq!(lower(&kid()), lower(&kid()));
    }

    /// Two roots jump into one block: it becomes a function, entered by tail calls.
    #[test]
    fn lower_tail_calls_shared_blocks() {
        let files = vec![
            file(
                "A",
                vec![
                    label(
                        "A_One",
                        Some(RootKind::Bg(0)),
                        vec![i("lock", vec![])],
                        Some("A_Shared"),
                    ),
                    label(
                        "A_Two",
                        Some(RootKind::Bg(1)),
                        vec![i("goto", vec![s("A_Shared")])],
                        None,
                    ),
                    label(
                        "A_Shared",
                        None,
                        vec![i("release", vec![]), i("return", vec![])],
                        None,
                    ),
                ],
                vec![],
            ),
            file(
                "B",
                vec![label(
                    "B_Root",
                    Some(RootKind::Coord(0)),
                    vec![i("goto", vec![s("A_Two")])],
                    None,
                )],
                vec![],
            ),
        ];
        let out = lower(&files);
        let a = &out.modules["A"];
        assert_eq!(
            fn_src(a, "A_One"),
            "pub async fn A_One(ctx: Ctx) -> Flow {\n    ctx.lock().await;\n    \
             return Box::pin(A_Shared(ctx.clone())).await;\n}\n"
        );
        assert_eq!(
            fn_src(a, "A_Shared"),
            "pub async fn A_Shared(ctx: Ctx) -> Flow {\n    ctx.release().await;\n    \
             return Flow::Continue;\n}\n"
        );
        assert!(
            fn_src(&out.modules["B"], "B_Root")
                .contains("return Box::pin(super::A::A_Two(ctx.clone())).await;")
        );
        assert_eq!(out.stats.fns, 4);
    }

    /// Gen3: an `end` in a callee ends the whole script, so its callers stop too; a callee that
    /// only `return`s is a plain call.
    #[test]
    fn lower_end_in_callee_stops_the_caller() {
        let files = vec![file(
            "M",
            vec![
                label(
                    "M_Root",
                    Some(RootKind::Object(1)),
                    vec![
                        i("call", vec![s("M_Ends")]),
                        i("call", vec![s("M_Returns")]),
                        i("call", vec![s("M_EndsViaTail")]),
                        i("release", vec![]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Ends",
                    None,
                    vec![i("lock", vec![]), i("end", vec![])],
                    None,
                ),
                label(
                    "M_Returns",
                    None,
                    vec![i("lock", vec![]), i("return", vec![])],
                    None,
                ),
                label(
                    "M_EndsViaTail",
                    None,
                    vec![i("goto", vec![s("M_Ends")])],
                    None,
                ),
            ],
            vec![],
        )];
        let out = lower(&files);
        assert_eq!(
            fn_src(&out.modules["M"], "M_Root"),
            "pub async fn M_Root(ctx: Ctx) -> Flow {\n    \
             if Box::pin(M_Ends(ctx.clone())).await == Flow::End {\n        return Flow::End;\n    }\n    \
             Box::pin(M_Returns(ctx.clone())).await;\n    \
             if Box::pin(M_EndsViaTail(ctx.clone())).await == Flow::End {\n        return Flow::End;\n    }\n    \
             ctx.release().await;\n    return Flow::End;\n}\n"
        );
        assert_eq!(out.stats.end_in_callee, 2);
    }

    /// The comparison register is the script's: a callee's `compare` is read by its caller.
    #[test]
    fn lower_cond_flows_across_calls() {
        let files = vec![file(
            "M",
            vec![
                label(
                    "M_Root",
                    Some(RootKind::Object(1)),
                    vec![
                        i("call", vec![s("M_Check")]),
                        i("goto_if", vec![Byte(1), s("M_Yes")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Check",
                    None,
                    vec![
                        i("compare_var_to_value", vec![v("VAR_RESULT"), Half(1)]),
                        i("return", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Yes",
                    None,
                    vec![i("release", vec![]), i("end", vec![])],
                    None,
                ),
            ],
            vec![],
        )];
        let out = lower(&files);
        let m = &out.modules["M"];
        assert!(!m.contains("let mut cond"), "{m}");
        assert!(fn_src(m, "M_Root").contains("if ctx.cond() == Ordering::Equal {"));
        assert!(fn_src(m, "M_Check").contains("ctx.set_cond(ctx.var(VAR_RESULT).await.cmp(&1));"));
    }

    #[test]
    fn lower_waitmovement_zero_is_the_last_mover() {
        let mv = IrMovement {
            name: "M_Movement_Walk".into(),
            origin: NameOrigin::Decomp,
            actions: vec!["MOVEMENT_ACTION_WALK_NORMAL_UP".into()],
        };
        let files = vec![file(
            "M",
            vec![label(
                "M_Root",
                Some(RootKind::Object(1)),
                vec![
                    i("waitmovement", vec![Half(0)]),
                    i(
                        "applymovement",
                        vec![Half(2), Movement("M_Movement_Walk".into())],
                    ),
                    i("waitmovement", vec![Half(0)]),
                    i("waitmovement", vec![Half(3)]),
                    i("end", vec![]),
                ],
                None,
            )],
            vec![mv],
        )];
        let out = lower(&files);
        let src = fn_src(&out.modules["M"], "M_Root");
        assert_eq!(
            src.matches("ctx.wait_move(0).await; // the last mover")
                .count(),
            2
        );
        assert!(src.contains("ctx.move_obj(2, &M_Movement_Walk).await;"));
        assert!(src.contains("ctx.wait_move(3).await;"));
        assert_eq!(out.stats.unresolved_waitmovement, 1);
    }

    /// Unknown movement actions follow `commands.ron`'s `movements`.
    #[test]
    fn lower_movements_use_the_fallback_table() {
        let mv = IrMovement {
            name: "M_Movement_Odd".into(),
            origin: NameOrigin::Positional,
            actions: [
                "MOVEMENT_ACTION_SLIDE_UP",
                "MOVEMENT_ACTION_WALK_NORMAL_DIAGONAL_UP_LEFT",
                "MOVEMENT_ACTION_FIGURE_8",
                "MOVEMENT_ACTION_DISABLE_ANIMATION",
                "MOVEMENT_ACTION_0xA0",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        };
        let out = lower(&[file("M", vec![], vec![mv])]);
        let m = &out.modules["M"];
        assert_eq!(
            m,
            "\npub static M_Movement_Odd: [Step; 5] = [\n    \
             Step::Walk(Dir::Up, Speed::Fast), // SLIDE_UP (as WALK_FAST_UP)\n    \
             Step::Walk(Dir::Up, Speed::Fast), // WALK_NORMAL_DIAGONAL_UP_LEFT (as WALK_FAST_UP)\n    \
             Step::Walk(Dir::Left, Speed::Fast), // WALK_NORMAL_DIAGONAL_UP_LEFT (as WALK_FAST_LEFT)\n    \
             Step::Delay(64), // FIGURE_8 (as a delay)\n    \
             // DISABLE_ANIMATION (no effect here)\n    \
             Step::Delay(0), // 0xA0 (unsupported)\n];\n",
            "no fns, so no Flow import"
        );
        assert_eq!(
            out.stats.unknown_movement_actions,
            BTreeMap::from([("MOVEMENT_ACTION_0xA0".to_string(), 1)])
        );
        assert_eq!(out.stats.named_decomp_pct["movements"], 0);
    }

    /// Every `movements` fallback of `commands.ron` lowers to actions that have a `Step`.
    #[test]
    fn lower_every_movement_fallback() {
        let mv = IrMovement {
            name: "M_All".into(),
            origin: NameOrigin::Decomp,
            actions: support().movements.keys().cloned().collect(),
        };
        let out = lower(&[file("M", vec![], vec![mv])]);
        assert!(
            out.stats.unknown_movement_actions.is_empty(),
            "{:?}",
            out.stats
        );
    }

    #[test]
    fn lower_classes_ignore_default_unsupported() {
        let files = vec![file(
            "M",
            vec![label(
                "M_Root",
                Some(RootKind::Object(1)),
                vec![
                    i("playse", vec![c("SE_PIN")]),
                    i("checkpartymove", vec![c("MOVE_CUT")]),
                    i("checkcoins", vec![v("VAR_0x8005")]),
                    i("pokemart", vec![Raw(0x1234)]),
                    i("special", vec![c("SPECIAL_HealPlayerParty")]),
                    i("special", vec![c("SPECIAL_ShowMapNamePopup")]),
                    i(
                        "specialvar",
                        vec![v("VAR_RESULT"), c("SPECIAL_ShouldTryRematchBattle")],
                    ),
                    i("special", vec![c("SPECIAL_DoPCTurnOnEffect")]),
                    i("gotobeatenscript", vec![]),
                ],
                None,
            )],
            vec![],
        )];
        let out = lower(&files);
        assert_eq!(
            fn_src(&out.modules["M"], "M_Root"),
            "pub async fn M_Root(ctx: Ctx) -> Flow {\n    \
             // playse SE_PIN\n    \
             ctx.report(Report::Unsupported(\"checkpartymove\")).await; // MOVE_CUT\n    \
             ctx.set_var(VAR_RESULT, 6).await;\n    \
             ctx.report(Report::Unsupported(\"checkcoins\")).await; // VAR_0x8005\n    \
             ctx.set_var(VAR_0x8005, 0).await;\n    \
             ctx.report(Report::Unsupported(\"pokemart\")).await; // 0x1234\n    \
             special(&ctx, SPECIAL_HealPlayerParty).await;\n    \
             // special SPECIAL_ShowMapNamePopup\n    \
             ctx.report(Report::Unsupported(\"SPECIAL_ShouldTryRematchBattle\")).await;\n    \
             ctx.set_var(VAR_RESULT, 0).await;\n    \
             ctx.report(Report::Unsupported(\"SPECIAL_DoPCTurnOnEffect\")).await;\n    \
             ctx.report(Report::Unsupported(\"gotobeatenscript\")).await;\n    \
             return Flow::End;\n}\n"
        );
        let st = &out.stats;
        assert_eq!(st.by_class["Ignore"], 2, "{:?}", st.by_class);
        assert_eq!(st.by_class["Default"], 3, "{:?}", st.by_class);
        assert_eq!(st.by_class["Unsupported"], 3, "{:?}", st.by_class);
        assert_eq!(st.reports.len(), 6, "{:?}", st.reports);
    }

    #[test]
    fn lower_fails_on_unclassified_commands_and_specials() {
        let files = vec![file(
            "M",
            vec![label(
                "M_Root",
                Some(RootKind::Object(1)),
                vec![
                    i("frobnicate", vec![]),
                    i("special", vec![c("SPECIAL_ChooseStarter")]),
                    i("end", vec![]),
                ],
                None,
            )],
            vec![],
        )];
        let err = codegen::lower(&files, &index(&files), &support(), &BTreeMap::new())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("frobnicate") && err.contains("SPECIAL_ChooseStarter"),
            "{err}"
        );
    }

    #[test]
    fn lower_dispatch_is_a_thunk_table() {
        let out = lower(&kid());
        let d = &out.dispatch;
        assert!(!d.contains("match id"), "{d}");
        assert!(d.contains("pub static SCRIPT_NAMES: [&str; 1] = [\"Town_EventScript_Kid\",];"));
        assert!(d.contains("pub const Town_EventScript_Kid: ScriptId = ScriptId(0);"));
        assert!(d.contains(
            "pub static SCRIPTS: [fn(engine::script::Ctx) -> engine::script::Script; 1] = [\n    \
             |ctx| Box::pin(run(Town::Town_EventScript_Kid(ctx))),\n];"
        ));
        assert!(d.contains("pub fn script(id: u16, ctx: engine::script::Ctx)"));
    }

    fn graph(callee_flag: &str) -> Vec<ScriptFile> {
        vec![
            file(
                "M",
                vec![
                    label(
                        "M_Root",
                        Some(RootKind::Object(1)),
                        vec![i("call", vec![s("M_Callee")]), i("end", vec![])],
                        None,
                    ),
                    label(
                        "M_Callee",
                        None,
                        vec![i("setflag", vec![f(callee_flag)]), i("return", vec![])],
                        None,
                    ),
                    label(
                        "M_Other",
                        Some(RootKind::Object(2)),
                        vec![i("callstd", vec![c("STD_OBTAIN_ITEM")]), i("end", vec![])],
                        None,
                    ),
                ],
                vec![],
            ),
            file(
                "Common_obtain_item",
                vec![label(
                    "Std_ObtainItem",
                    None,
                    vec![i("setflag", vec![f("FLAG_TEMP_1")]), i("return", vec![])],
                    None,
                )],
                vec![],
            ),
        ]
    }

    #[test]
    fn lower_digests_cover_the_call_graph_and_overrides() {
        let a = lower(&graph("FLAG_TEMP_2")).script_digests;
        let b = lower(&graph("FLAG_TEMP_3")).script_digests;
        assert_eq!(
            a.keys().collect::<Vec<_>>(),
            ["M_Callee", "M_Other", "M_Root", "Std_ObtainItem"]
        );
        assert_ne!(a["M_Callee"], b["M_Callee"]);
        assert_ne!(
            a["M_Root"], b["M_Root"],
            "a callee's change changes its callers"
        );
        assert_eq!(a["M_Other"], b["M_Other"]);
        // `callstd STD_OBTAIN_ITEM` depends on `Std_ObtainItem` when the IR has it.
        let mut g = graph("FLAG_TEMP_2");
        g[1].labels[0].insns[0] = i("setflag", vec![f("FLAG_TEMP_4")]);
        let c = lower(&g).script_digests;
        assert_ne!(a["M_Other"], c["M_Other"]);
        assert_eq!(a["M_Root"], c["M_Root"]);
        // An override's source is digested, and so is what it names.
        let ov = |src: &str| BTreeMap::from([("M_Other".to_string(), src.to_string())]);
        let d1 = lower_with(&graph("FLAG_TEMP_2"), &ov("M_Callee(ctx).await; // 1")).script_digests;
        let d2 = lower_with(&graph("FLAG_TEMP_2"), &ov("M_Callee(ctx).await; // 2")).script_digests;
        assert_ne!(d1["M_Other"], d2["M_Other"]);
        assert_ne!(d1["M_Other"], a["M_Other"]);
        let d3 = lower_with(&graph("FLAG_TEMP_3"), &ov("M_Callee(ctx).await; // 1")).script_digests;
        assert_ne!(d1["M_Other"], d3["M_Other"], "the override names M_Callee");
    }

    #[test]
    fn lower_overrides_replace_a_script() {
        let o = BTreeMap::from([(
            "M_Callee".to_string(),
            "pub async fn M_Callee(ctx: Ctx) {}".to_string(),
        )]);
        let out = lower_with(&graph("FLAG_TEMP_2"), &o);
        assert_eq!(
            fn_src(&out.modules["M"], "M_Callee"),
            "pub async fn M_Callee(ctx: Ctx) -> Flow {\n    \
             crate::overrides::M_Callee(ctx).await;\n    Flow::Continue\n}\n"
        );
        let bad = BTreeMap::from([("M_Nope".to_string(), String::new())]);
        let g = graph("FLAG_TEMP_2");
        let err = codegen::lower(&g, &index(&g), &support(), &bad).unwrap_err();
        assert!(err.to_string().contains("M_Nope"), "{err}");
    }

    #[test]
    fn lower_trainerbattle_kinds() {
        let tb = |kind: u8, rest: Vec<IrArg>| {
            let mut a = vec![Byte(kind), c("TRAINER_CALVIN_1"), Half(1)];
            a.extend(rest);
            i("trainerbattle", a)
        };
        let files = vec![file(
            "M",
            vec![
                label(
                    "M_Single",
                    Some(RootKind::Object(1)),
                    vec![
                        tb(0, vec![t("M_Text_Intro"), t("M_Text_Lose")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Continue",
                    Some(RootKind::Object(2)),
                    vec![
                        tb(1, vec![t("M_Text_Intro"), t("M_Text_Lose"), s("M_After")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_After",
                    None,
                    vec![i("release", vec![]), i("end", vec![])],
                    None,
                ),
                label(
                    "M_NoIntro",
                    Some(RootKind::Object(3)),
                    vec![tb(3, vec![t("M_Text_Lose")]), i("end", vec![])],
                    None,
                ),
                label(
                    "M_Double",
                    Some(RootKind::Object(4)),
                    vec![
                        tb(
                            4,
                            vec![t("M_Text_Intro"), t("M_Text_Lose"), t("M_Text_NotEnough")],
                        ),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Rematch",
                    Some(RootKind::Object(5)),
                    vec![
                        tb(5, vec![t("M_Text_Intro"), t("M_Text_Lose")]),
                        i("end", vec![]),
                    ],
                    None,
                ),
                label(
                    "M_Hill",
                    Some(RootKind::Object(6)),
                    vec![tb(12, vec![Raw(0), Raw(0)])],
                    Some("M_After"),
                ),
            ],
            vec![],
        )];
        let out = lower(&files);
        let m = &out.modules["M"];
        assert_eq!(
            fn_src(m, "M_Single"),
            "pub async fn M_Single(ctx: Ctx) -> Flow {\n    \
             let beaten = ctx.flag(trainer_flag(TRAINER_CALVIN_1)).await;\n    \
             trainer_battle(&ctx, TrainerBattle { kind: 0, trainer: TRAINER_CALVIN_1.0, local_id: 1, \
             intro: Some(M_Text_Intro), lose: M_Text_Lose, not_enough: None, event_script: None, \
             music: true }).await;\n    \
             if !beaten {\n        return Flow::End;\n    }\n    return Flow::End;\n}\n"
        );
        assert!(
            fn_src(m, "M_Continue")
                .contains("event_script: Some(super::script_ids::M_After), music: false }).await;")
        );
        let no_intro = fn_src(m, "M_NoIntro");
        assert!(no_intro.contains("kind: 3,") && no_intro.contains("intro: None"));
        assert!(!no_intro.contains("beaten"));
        assert!(fn_src(m, "M_Double").contains("not_enough: Some(M_Text_NotEnough)"));
        let rematch = fn_src(m, "M_Rematch");
        assert!(!rematch.contains("beaten"));
        assert!(
            rematch.contains(").await;\n    return Flow::End;\n"),
            "{rematch}"
        );
        // A battle with no lose text cannot be described: reported.
        assert!(fn_src(m, "M_Hill").contains("Report::Unsupported(\"trainerbattle\")"));
        assert_eq!(
            out.trainer_scripts,
            [
                "M_Continue",
                "M_Double",
                "M_NoIntro",
                "M_Rematch",
                "M_Single"
            ]
            .iter()
            .map(|s| (s.to_string(), "TRAINER_CALVIN_1".to_string()))
            .collect::<Vec<_>>()
        );
    }

    /// One site of every command `commands.ron` marks `Native`, in Task B's operand forms.
    fn every_native() -> Vec<ScriptFile> {
        let mv = |name: &str| Movement(name.to_string());
        let map = || c("MAP_LITTLEROOT_TOWN");
        let body = vec![
            i("nop", vec![]),
            i("nop1", vec![]),
            i("loadword", vec![Byte(0), t("N_Text_Hello")]),
            i("callstd", vec![c("MSGBOX_SIGN")]),
            i("loadword", vec![Byte(0), t("N_Text_Hello")]),
            i("callstd_if", vec![Byte(1), Byte(5)]),
            i("callstd", vec![c("STD_OBTAIN_ITEM")]),
            i("callstd", vec![c("STD_FIND_ITEM")]),
            i("callstd", vec![c("STD_REGISTER_MATCH_CALL")]),
            i(
                "setvar",
                vec![v("VAR_OBJ_GFX_ID_0"), c("OBJ_EVENT_GFX_RIVAL_MAY_NORMAL")],
            ),
            i("addvar", vec![v("VAR_0x8004"), Half(1)]),
            i("subvar", vec![v("VAR_0x8004"), v("VAR_0x8005")]),
            i("copyvar", vec![v("VAR_0x8004"), v("VAR_RESULT")]),
            i("setorcopyvar", vec![v("VAR_0x8000"), c("ITEM_POTION")]),
            i("setorcopyvar", vec![v("VAR_0x8001"), v("VAR_0x8006")]),
            i(
                "compare_var_to_value",
                vec![v("VAR_RESULT"), c("SPECIES_TORCHIC")],
            ),
            i("compare_var_to_var", vec![v("VAR_0x8004"), v("VAR_0x8005")]),
            i("call_if", vec![Byte(1), s("N_Sub")]),
            i("special", vec![c("SPECIAL_HealPlayerParty")]),
            i(
                "specialvar",
                vec![v("VAR_RESULT"), c("SPECIAL_HealPlayerParty")],
            ),
            i("waitstate", vec![]),
            i("delay", vec![Half(30)]),
            i("setflag", vec![f("FLAG_SYS_POKEMON_GET")]),
            i("clearflag", vec![f("FLAG_0x500")]),
            i("checkflag", vec![Half(0x20)]),
            i("warp", vec![map(), Byte(1), Half(0xFFFF), Half(0xFFFF)]),
            i(
                "warpsilent",
                vec![map(), Byte(0xFF), Half(5), v("VAR_0x8004")],
            ),
            i("warpdoor", vec![Half(0x0100), Byte(1), Half(0), Half(0)]),
            i("warpteleport", vec![map(), Byte(0), Half(1), Half(2)]),
            i("warpspinenter", vec![map(), Byte(0), Half(1), Half(2)]),
            i("warpmossdeepgym", vec![map(), Byte(0), Half(1), Half(2)]),
            i("warpwhitefade", vec![map(), Byte(0), Half(1), Half(2)]),
            i("setdynamicwarp", vec![map(), Byte(0xFF), Half(3), Half(4)]),
            i(
                "setescapewarp",
                vec![Half(0x7F7F), Byte(1), Half(3), Half(4)],
            ),
            i("getplayerxy", vec![v("VAR_0x8004"), v("VAR_0x8005")]),
            i("getpartysize", vec![]),
            i("additem", vec![c("ITEM_POTION"), Half(1)]),
            i("removeitem", vec![v("VAR_0x8004"), Half(1)]),
            i("checkitemspace", vec![c("ITEM_POTION"), Half(2)]),
            i("checkitem", vec![c("ITEM_POTION"), v("VAR_0x8005")]),
            i(
                "applymovement",
                vec![c("LOCALID_PLAYER"), mv("N_Movement_Walk")],
            ),
            i(
                "applymovementat",
                vec![Half(2), mv("N_Movement_Walk"), map()],
            ),
            i("waitmovement", vec![Half(0)]),
            i("waitmovementat", vec![Half(2), map()]),
            i("removeobject", vec![v("VAR_LAST_TALKED")]),
            i("removeobjectat", vec![Half(2), map()]),
            i("addobject", vec![Half(2)]),
            i("addobjectat", vec![Half(2), map()]),
            i("showobjectat", vec![Half(0xFF), c("MAP_ROUTE101")]),
            i("hideobjectat", vec![Half(2), map()]),
            i("setobjectxy", vec![Half(2), Half(3), Half(4)]),
            i("setobjectxyperm", vec![Half(2), Half(3), Half(4)]),
            i("faceplayer", vec![]),
            i("turnobject", vec![Half(2), Byte(3)]),
            i(
                "setobjectmovementtype",
                vec![Half(2), c("MOVEMENT_TYPE_FACE_LEFT")],
            ),
            i(
                "setobjectmovementtype",
                vec![Half(2), c("MOVEMENT_TYPE_WALK_SEQUENCE_UP_RIGHT_LEFT_DOWN")],
            ),
            i("setobjectmovementtype", vec![Half(2), Byte(0x4c)]),
            i(
                "trainerbattle",
                vec![
                    Byte(0),
                    c("TRAINER_CALVIN_1"),
                    Half(1),
                    t("N_Text_Hello"),
                    t("N_Text_Hello"),
                ],
            ),
            i("checktrainerflag", vec![c("TRAINER_CALVIN_1")]),
            i("settrainerflag", vec![v("VAR_0x8004")]),
            i("cleartrainerflag", vec![Half(5)]),
            i("message", vec![t("N_Text_Hello")]),
            i("messageautoscroll", vec![t("N_Text_Hello")]),
            i("messageinstant", vec![t("N_Text_Hello")]),
            i("waitmessage", vec![]),
            i("closemessage", vec![]),
            i("lockall", vec![]),
            i("lock", vec![]),
            i("releaseall", vec![]),
            i("release", vec![]),
            i("waitbuttonpress", vec![]),
            i("yesnobox", vec![Byte(20), Byte(8)]),
            i(
                "multichoice",
                vec![Byte(0), Byte(0), c("MULTI_YESNOINFO"), Byte(0)],
            ),
            i(
                "multichoicedefault",
                vec![Byte(20), Byte(6), Byte(1), Byte(1), Byte(1)],
            ),
            i(
                "givemon",
                vec![
                    c("SPECIES_TORCHIC"),
                    Byte(5),
                    c("ITEM_NONE"),
                    Word(0),
                    Word(0),
                    Byte(0),
                ],
            ),
            i("bufferspeciesname", vec![Byte(0), v("VAR_0x8004")]),
            i("bufferleadmonspeciesname", vec![Byte(0)]),
            i("bufferpartymonnick", vec![Byte(1), v("VAR_0x8004")]),
            i("bufferitemname", vec![Byte(0), c("ITEM_POTION")]),
            i(
                "bufferitemnameplural",
                vec![Byte(0), c("ITEM_POTION"), Half(2)],
            ),
            i("buffermovename", vec![Byte(1), c("MOVE_CUT")]),
            i("buffernumberstring", vec![Byte(2), v("VAR_0x8005")]),
            i("bufferstdstring", vec![Byte(0), c("STDSTRING_COOL")]),
            i("bufferstring", vec![Byte(0), t("N_Text_Hello")]),
            i("random", vec![Half(3)]),
            i("addmoney", vec![Word(300), Byte(0)]),
            i("removemoney", vec![Word(300), Byte(0)]),
            i("checkmoney", vec![Raw(300), Byte(0)]),
            i("fadescreen", vec![Byte(1)]),
            i("fadescreenspeed", vec![Byte(0), Byte(4)]),
            i("fadescreenswapbuffers", vec![Byte(3)]),
            i(
                "setrespawn",
                vec![c("HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F")],
            ),
            i("checkplayergender", vec![]),
            i("setmetatile", vec![Half(3), Half(4), Half(0x201), Half(1)]),
            i("opendoor", vec![Half(3), Half(4)]),
            i("waitdooranim", vec![]),
            i("closedoor", vec![v("VAR_0x8004"), Half(4)]),
            i(
                "setwildbattle",
                vec![c("SPECIES_ZIGZAGOON"), Byte(2), c("ITEM_NONE")],
            ),
            i("dowildbattle", vec![]),
            i("goto_if", vec![Byte(5), s("N_Tail")]),
            i("gotostd_if", vec![Byte(0), c("STD_OBTAIN_ITEM")]),
            i("call", vec![s("N_Sub")]),
            i("goto", vec![s("N_Tail")]),
        ];
        vec![file(
            "LittlerootTown",
            vec![
                label("N_Root", Some(RootKind::Object(1)), body, None),
                label("N_Sub", None, vec![i("returnram", vec![])], None),
                label(
                    "N_Tail",
                    None,
                    vec![i("callstd", vec![Byte(2)])],
                    Some("N_Tail2"),
                ),
                label("N_Tail2", None, vec![i("gotostd", vec![Byte(0)])], None),
                label(
                    "N_Ends",
                    Some(RootKind::Bg(0)),
                    vec![i("return", vec![])],
                    None,
                ),
                label(
                    "N_EndRam",
                    Some(RootKind::Bg(1)),
                    vec![i("lock", vec![])],
                    Some("N_EndRam2"),
                ),
                label("N_EndRam2", None, vec![i("endram", vec![])], None),
                label("N_End", Some(RootKind::Bg(2)), vec![i("end", vec![])], None),
            ],
            vec![IrMovement {
                name: "N_Movement_Walk".into(),
                origin: NameOrigin::Decomp,
                actions: vec!["MOVEMENT_ACTION_WALK_NORMAL_UP".into()],
            }],
        )]
    }

    #[test]
    fn lower_every_native_command_without_reports() {
        let files = every_native();
        let used: BTreeSet<&str> = files
            .iter()
            .flat_map(|f| &f.labels)
            .flat_map(|l| &l.insns)
            .map(|i| i.op.as_str())
            .collect();
        let sup = support();
        let native: BTreeSet<&str> = sup
            .commands
            .iter()
            .filter(|(_, c)| **c == Class::Native)
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(
            used, native,
            "the sample must cover exactly the Native commands"
        );
        let out = lower(&files);
        assert!(out.stats.reports.is_empty(), "{:?}", out.stats.reports);
        assert_eq!(out.stats.by_class.get("Unsupported"), None);
        let src = &out.modules["LittlerootTown"];
        assert!(!src.contains("Report::Unsupported"), "{src}");
        for needle in [
            "msgbox(&ctx, N_Text_Hello, MsgBox::Sign).await;",
            "    msgbox(&ctx, N_Text_Hello, MsgBox::YesNo).await;",
            "ctx.set_var(VAR_RESULT, give_item(&ctx, ctx.var(VAR_0x8000).await, ctx.var(VAR_0x8001).await).await as u16).await;",
            "ctx.set_var(VAR_RESULT, find_item(&ctx, ctx.var(VAR_0x8000).await, ctx.var(VAR_0x8001).await).await as u16).await;",
            "callstd(&ctx, STD_REGISTER_MATCH_CALL).await;",
            "callstd(&ctx, STD_MSGBOX_NPC).await;",
            "ctx.set_var(VAR_OBJ_GFX_ID_0, OBJ_EVENT_GFX_RIVAL_MAY_NORMAL).await;",
            "ctx.set_var(VAR_0x8004, ctx.var(VAR_0x8004).await.wrapping_sub(ctx.var(VAR_0x8005).await)).await;",
            "ctx.set_cond(ctx.var(VAR_RESULT).await.cmp(&SPECIES_TORCHIC.0));",
            "ctx.set_flag(FlagId(0x500), false).await;",
            "ctx.warp(MAP_LITTLEROOT_TOWN, Some(1), -1, -1).await;",
            "ctx.warp(MAP_LITTLEROOT_TOWN, None, 5, ctx.var(VAR_0x8004).await as i16).await; // warpsilent",
            "set_warp(&ctx, WarpSlot::Dynamic, MAP_LITTLEROOT_TOWN.0, None, 3, 4).await;",
            "set_warp(&ctx, WarpSlot::Escape, MAP_DYNAMIC.0, Some(1), 3, 4).await;",
            "ctx.set_var(VAR_RESULT, ctx.item_add(ItemId(ITEM_POTION), 1).await as u16).await;",
            "ctx.set_var(VAR_RESULT, check_item(&ctx, ITEM_POTION, ctx.var(VAR_0x8005).await).await as u16).await;",
            "ctx.move_obj(PLAYER, &N_Movement_Walk).await;",
            "ctx.move_obj(2, &N_Movement_Walk).await; // applymovementat",
            "ctx.remove_object(ctx.var(VAR_LAST_TALKED).await as u8).await;",
            "ctx.show_object(PLAYER).await; // showobjectat",
            "ctx.set_movement_type(2, MovementType::Face(Dir::Left)).await;",
            "ctx.set_movement_type(2, MovementType::Wander).await;",
            "ctx.set_cond(if ctx.flag(trainer_flag(TRAINER_CALVIN_1)).await { Ordering::Equal } else { Ordering::Less });",
            "ctx.set_flag(trainer_flag(TrainerId(ctx.var(VAR_0x8004).await)), true).await;",
            "ctx.message(N_Text_Hello).await; // messageinstant",
            "multichoice(&ctx, 17, 0, 0, 0, true).await",
            "multichoice(&ctx, 1, 20, 6, 1, false).await",
            "ctx.set_var(VAR_RESULT, ctx.give_mon(SPECIES_TORCHIC, 5).await).await;",
            "buffer(&ctx, 0, BufferSrc::Species(SpeciesId(ctx.var(VAR_0x8004).await))).await;",
            "buffer(&ctx, 1, BufferSrc::PartyNick(ctx.var(VAR_0x8004).await as u8)).await;",
            "buffer(&ctx, 1, BufferSrc::Move(MOVE_CUT)).await;",
            "buffer(&ctx, 2, BufferSrc::Number(u32::from(ctx.var(VAR_0x8005).await))).await;",
            "buffer(&ctx, 0, BufferSrc::Std(0)).await;",
            "buffer(&ctx, 0, BufferSrc::Text(N_Text_Hello)).await;",
            "money_add(&ctx, 300).await;",
            "ctx.set_var(VAR_RESULT, money_check(&ctx, 300).await as u16).await;",
            "ctx.report(Report::Degraded(\"fadescreenswapbuffers\")).await;",
            "ctx.set_respawn(HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F).await;",
            "ctx.set_metatile(3, 4, 513, true).await;",
            "ctx.door(ctx.var(VAR_0x8004).await as i16, 4, false).await;",
            "wild = Some((SPECIES_ZIGZAGOON, 2));",
            "if let Some((s, l)) = wild { ctx.battle_wild(s, l, false).await; }",
            "return Flow::Continue;",
        ] {
            assert!(src.contains(needle), "missing {needle:?} in:\n{src}");
        }
    }

    #[test]
    fn lower_output_compiles() {
        let mut files = every_native();
        files.extend(kid());
        files.extend(graph("FLAG_TEMP_2"));
        let mut tb = vec![];
        for (k, rest) in [
            (1u8, vec![t("N_Text_Hello"), t("N_Text_Hello"), s("N_Sub")]),
            (3, vec![t("N_Text_Hello")]),
            (
                4,
                vec![t("N_Text_Hello"), t("N_Text_Hello"), t("N_Text_Hello")],
            ),
            (5, vec![t("N_Text_Hello"), t("N_Text_Hello")]),
            (
                8,
                vec![
                    t("N_Text_Hello"),
                    t("N_Text_Hello"),
                    t("N_Text_Hello"),
                    s("N_Sub"),
                ],
            ),
            (10, vec![Raw(0), t("N_Text_Hello")]),
        ] {
            let mut a = vec![Byte(k), Half(0x10), Half(1)];
            a.extend(rest);
            tb.push(i("trainerbattle", a));
        }
        tb.push(i("pokemart", vec![Raw(0x1234)]));
        tb.push(i("checkcoins", vec![v("VAR_0x8004")]));
        tb.push(i("playse", vec![Half(5)]));
        tb.push(i("special", vec![c("SPECIAL_ShouldTryRematchBattle")]));
        tb.push(i("dowildbattle", vec![]));
        files.push(file(
            "Other",
            vec![
                label("O_Root", Some(RootKind::Coord(0)), tb, Some("O_Unknown")),
                label("O_Unknown", None, vec![i("lock", vec![])], None),
                label(
                    "O_Cross",
                    Some(RootKind::Coord(1)),
                    vec![i("call", vec![s("N_Sub")]), i("goto", vec![s("M_Callee")])],
                    None,
                ),
            ],
            vec![],
        ));
        let o = BTreeMap::from([("M_Other".to_string(), String::new())]);
        let out = lower_with(&files, &o);
        assert!(out.stats.reports.contains_key("unknown opcode"));
        compile_lowered("m8", &out, &index(&files));
    }

    /// Builds a scratch crate laid out as `content` is after Task D2 (`crate::prelude`,
    /// `crate::overrides`, and `crate::scripts` holding the dispatch and one module per IR
    /// module) against the real `engine`, and runs `cargo check` with warnings denied.
    fn compile_lowered(tag: &str, out: &Lowered, ix: &ScriptIndex) {
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let dir = tmp.join(format!("lowered-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let engine = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../engine")
            .canonicalize()
            .unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"stub_lowered\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
                 publish = false\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\n\
                 engine = {{ path = {:?} }}\n\n[workspace]\n",
                engine.to_str().unwrap()
            ),
        )
        .unwrap();
        let mut prelude = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stub_prelude/prelude.rs"),
        )
        .unwrap();
        prelude += STUB_PRELUDE_M8;
        prelude += &consts(ix.texts.keys());
        std::fs::write(dir.join("prelude.rs"), prelude).unwrap();
        std::fs::write(dir.join("dispatch.rs"), &out.dispatch).unwrap();
        let mut overrides = String::new();
        for (m, src) in &out.modules {
            std::fs::write(dir.join(format!("{m}.rs")), src).unwrap();
            for line in src.lines() {
                if let Some(rest) = line.trim().strip_prefix("crate::overrides::") {
                    let name = rest.split('(').next().unwrap();
                    overrides += &format!(
                        "pub async fn {name}(ctx: crate::prelude::Ctx) {{ ctx.log(\"override\").await; }}\n"
                    );
                }
            }
        }
        let mut lib = String::from(
            "#[allow(non_upper_case_globals)]\npub mod prelude {\n    include!(\"prelude.rs\");\n}\n\
             #[allow(non_snake_case)]\npub mod overrides {\n",
        );
        lib += &overrides;
        lib += "}\npub mod scripts {\n    include!(\"dispatch.rs\");\n";
        for m in out.modules.keys() {
            lib += &format!(
                "    #[allow(non_snake_case, non_upper_case_globals, unused_mut, unused_variables, \
                 unused_assignments, unreachable_code, clippy::all)] pub mod {m} {{ use crate::prelude::*; \
                 include!(\"{m}.rs\"); }}\n"
            );
        }
        lib += "}\n";
        std::fs::write(dir.join("lib.rs"), lib).unwrap();
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let res = Command::new(cargo)
            .args(["check", "--offline", "--quiet", "--manifest-path"])
            .arg(dir.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", tmp.join("stub-target"))
            .env("RUSTFLAGS", "-D warnings")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&res.stderr);
        assert!(
            res.status.success(),
            "{tag}: lowered code does not compile:\n{stderr}"
        );
    }

    /// The prelude's constants (as `content`'s `consts.rs` types them) and the text ids.
    fn consts<'a>(texts: impl Iterator<Item = &'a String>) -> String {
        let mut s = String::new();
        let mut seen = BTreeSet::new();
        let mut table = |s: &mut String, t: &names::Table, ty: &str, wrap: bool| {
            for (v, n) in t.iter() {
                if !codegen::is_ident(n) {
                    continue;
                }
                let n = if n.starts_with(t.prefix) {
                    n.to_string()
                } else {
                    format!("{}{n}", t.prefix)
                };
                if seen.insert(n.clone()) {
                    let v = if wrap {
                        format!("{ty}({v:#x})")
                    } else {
                        format!("{v:#x}")
                    };
                    *s += &format!("pub const {n}: {ty} = {v};\n");
                }
            }
        };
        table(&mut s, &names::FLAGS, "FlagId", true);
        table(&mut s, &names::VARS, "VarId", true);
        table(&mut s, &names::SPECIALS, "u16", false);
        table(&mut s, &names::STD_SCRIPTS, "u8", false);
        table(&mut s, &names::OBJ_EVENT_GFX, "u16", false);
        table(&mut s, &names::ITEMS, "u16", false);
        table(&mut s, &names::SPECIES, "SpeciesId", true);
        table(&mut s, &names::MOVES, "MoveId", true);
        table(&mut s, &names::TRAINERS, "TrainerId", true);
        table(&mut s, &names::HEAL_LOCATIONS, "u8", false);
        for m in names::MAPS {
            let id = u16::from(m.num) | u16::from(m.group) << 8;
            s += &format!("pub const {}: MapId = MapId({id:#x});\n", m.constant);
        }
        s += "pub const MAP_DYNAMIC: MapId = MapId(0x7f7f);\n";
        for (i, k) in texts.enumerate() {
            s += &format!("pub const {k}: TextId = TextId({i});\n");
        }
        s
    }

    /// The Milestone 8 prelude contract (Wave 0 names and signatures) over today's engine. The
    /// `Ctx` methods Wave 0 adds come from `CtxM8` until the engine has them; then this stub
    /// re-exports `engine::script::{Report, BufferSrc, WarpSlot}` and `engine::data::ItemId`.
    const STUB_PRELUDE_M8: &str = r#"
pub use engine::data::{MoveId, ScriptId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ItemId(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Report {
    Unsupported(&'static str),
    Degraded(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BufferSrc {
    Species(SpeciesId),
    Item(ItemId),
    Move(MoveId),
    Number(u32),
    Std(u8),
    PartyNick(u8),
    LeadSpecies,
    Text(TextId),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WarpSlot {
    Dynamic,
    Escape,
}

#[allow(async_fn_in_trait)]
pub trait CtxM8 {
    fn cond(&self) -> Ordering;
    fn set_cond(&self, o: Ordering);
    async fn report(&self, r: Report);
    async fn item_add(&self, item: ItemId, qty: u16) -> bool;
    async fn item_remove(&self, item: ItemId, qty: u16) -> bool;
    async fn item_space(&self, item: ItemId, qty: u16) -> bool;
}

impl CtxM8 for Ctx {
    fn cond(&self) -> Ordering {
        Ordering::Equal
    }
    fn set_cond(&self, o: Ordering) {
        let _ = o;
    }
    async fn report(&self, r: Report) {
        let _ = r;
        self.log("report").await;
    }
    async fn item_add(&self, item: ItemId, qty: u16) -> bool {
        let _ = (item, qty);
        false
    }
    async fn item_remove(&self, item: ItemId, qty: u16) -> bool {
        let _ = (item, qty);
        false
    }
    async fn item_space(&self, item: ItemId, qty: u16) -> bool {
        let _ = (item, qty);
        false
    }
}

pub const TRAINER_FLAGS_START: u16 = 0x500;
pub const fn trainer_flag(trainer: TrainerId) -> FlagId {
    FlagId(TRAINER_FLAGS_START + trainer.0)
}

pub async fn give_item(ctx: &Ctx, item: u16, qty: u16) -> bool { let _ = (ctx, item, qty); false }
pub async fn find_item(ctx: &Ctx, item: u16, qty: u16) -> bool { let _ = (ctx, item, qty); false }
pub async fn check_item(ctx: &Ctx, item: u16, qty: u16) -> bool { let _ = (ctx, item, qty); false }
pub async fn money_add(ctx: &Ctx, n: u32) { let _ = (ctx, n); }
pub async fn money_remove(ctx: &Ctx, n: u32) { let _ = (ctx, n); }
pub async fn money_check(ctx: &Ctx, n: u32) -> bool { let _ = (ctx, n); false }
pub async fn buffer(ctx: &Ctx, slot: u8, src: BufferSrc) { let _ = (ctx, slot, src); }
pub async fn multichoice(ctx: &Ctx, list: u8, x: u8, y: u8, default: u8, cancel: bool) -> u16 {
    let _ = (ctx, list, x, y, default, cancel);
    127
}
pub async fn set_warp(ctx: &Ctx, slot: WarpSlot, map: u16, warp: Option<u8>, x: i16, y: i16) {
    let _ = (ctx, slot, map, warp, x, y);
}

pub struct TrainerBattle {
    pub kind: u8,
    pub trainer: u16,
    pub local_id: u8,
    pub intro: Option<TextId>,
    pub lose: TextId,
    pub not_enough: Option<TextId>,
    pub event_script: Option<ScriptId>,
    pub music: bool,
}

pub async fn trainer_battle(ctx: &Ctx, b: TrainerBattle) {
    let _ = (ctx, b.kind, b.trainer, b.local_id, b.intro, b.lose, b.not_enough, b.event_script, b.music);
}
"#;
}
