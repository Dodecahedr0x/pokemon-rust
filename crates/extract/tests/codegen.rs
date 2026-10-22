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
