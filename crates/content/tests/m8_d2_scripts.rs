//! Milestone 8 Task D2: the content build lowers the script IR (`build/scripts.rs`).
//! - Fixture dirs (an IR written to disk, read and lowered by the build's own code, with the
//!   committed support tables): an unclassified op fails, an override replaces its script, an
//!   override naming no script fails, positional names apply no override, and trainer scripts
//!   carry their flag and `trainerbattle` kind.
//! - The built crate (on an extract): the worked-example override runs in place of its script,
//!   the trainer table answers `trainer_flag_of`/`trainer_battle_kind`, the std scripts' texts are
//!   built, and the digests cover every script.

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
#[path = "../build/stub.rs"]
mod stub;

use engine::data::{FlagId, GameData, ScriptId};
use engine::script::{Request, Response, Runner, ScriptHost};
use extract::schema::{
    Hex, IrArg, IrInsn, IrLabel, Manifest, NameOrigin, RootKind, ScriptEntry, ScriptFile,
    ScriptIndex,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn insn(op: &str, args: Vec<IrArg>) -> IrInsn {
    IrInsn {
        op: op.into(),
        args,
    }
}

fn root(name: &str, insns: Vec<IrInsn>) -> IrLabel {
    IrLabel {
        name: name.into(),
        origin: NameOrigin::Decomp,
        offset: Hex(0),
        root: Some(RootKind::Object(1)),
        insns,
        next: None,
    }
}

/// Writes a fixture extract's IR (module `Town`, `labels`) and manifest under `name`; returns
/// the dir.
fn fixture(name: &str, labels: Vec<IrLabel>, decomp_names: bool) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("m8-d2")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let file = ScriptFile {
        module: "Town".into(),
        source: None,
        labels,
        movements: vec![],
    };
    let mut index = ScriptIndex::default();
    for l in &file.labels {
        index.scripts.insert(
            l.name.clone(),
            ScriptEntry {
                module: "Town".into(),
                root: l.root.is_some(),
                origin: l.origin,
                offset: l.offset,
            },
        );
    }
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    let put = |rel: &str, s: String| std::fs::write(dir.join(rel), s).unwrap();
    put(
        "scripts/index.ron",
        extract::export::to_ron(&index).unwrap(),
    );
    put("scripts/Town.ron", extract::export::to_ron(&file).unwrap());
    let manifest = Manifest {
        format: extract::schema::FORMAT_VERSION,
        format_minor: extract::schema::FORMAT_MINOR,
        game_code: "BPEF".into(),
        sha1: String::new(),
        language: "fr".into(),
        maps: vec![],
        tilesets: vec![],
        sprites: vec![],
        provenance: BTreeMap::from([(
            extract::export::scripts::DECOMP_NAMES_KEY.to_string(),
            Hex(decomp_names.into()),
        )]),
        skipped_graphics: Default::default(),
        unknown_script_ops: Default::default(),
        files: Default::default(),
    };
    put("manifest.ron", extract::export::to_ron(&manifest).unwrap());
    dir
}

/// Reads `dir`'s IR as the build does and lowers it with the committed support tables and the
/// override files of `overrides` (name -> source).
fn lower(dir: &Path, overrides: &[(&str, &str)]) -> anyhow::Result<scripts::Scripted> {
    let manifest: Manifest = extract::export::read_ron(&dir.join("manifest.ron"))?;
    let ir = scripts::read_ir(dir, &manifest)?;
    let content = Path::new(env!("CARGO_MANIFEST_DIR"));
    let support = scripts::support(&content.join("build"))?;
    let odir = dir.join("overrides");
    std::fs::create_dir_all(&odir)?;
    for (name, src) in overrides {
        std::fs::write(odir.join(format!("{name}.rs")), src)?;
    }
    let sources = scripts::override_sources(&odir)?;
    scripts::lower_ir(ir.as_ref(), &support, &sources)
}

const OVERRIDE_SRC: &str = "pub async fn Town_EventScript_Kid(ctx: Ctx) { ctx.release().await; }\n";

fn kid() -> IrLabel {
    root(
        "Town_EventScript_Kid",
        vec![insn("lock", vec![]), insn("end", vec![])],
    )
}

#[test]
fn m8_d2_an_unclassified_op_fails_the_build_with_the_list() {
    let dir = fixture(
        "unclassified",
        vec![root(
            "Town_EventScript_Odd",
            vec![
                insn("frobnicate", vec![]),
                insn(
                    "special",
                    // A number no `SPECIAL_*` has: `SPECIAL_0x3ff`.
                    vec![IrArg::Half(0x3FF)],
                ),
                insn("end", vec![]),
            ],
        )],
        true,
    );
    let e = format!("{:#}", lower(&dir, &[]).err().expect("must fail"));
    assert!(e.contains("frobnicate"), "{e}");
    assert!(e.contains("SPECIAL_0x3ff"), "{e}");
}

#[test]
fn m8_d2_an_override_replaces_its_script() {
    let dir = fixture("override", vec![kid()], true);
    let plain = lower(&dir, &[]).unwrap();
    let s = lower(&dir, &[("Town_EventScript_Kid", OVERRIDE_SRC)]).unwrap();
    let m = &s.lowered.modules["Town"];
    assert!(
        m.contains("crate::overrides::Town_EventScript_Kid(ctx).await;"),
        "{m}"
    );
    assert!(!m.contains("ctx.lock()"), "{m}");
    assert!(s.overrides.contains_key("Town_EventScript_Kid"));
    // The override's source is digested into the script's entry.
    assert_ne!(
        s.lowered.script_digests["Town_EventScript_Kid"],
        plain.lowered.script_digests["Town_EventScript_Kid"]
    );
}

#[test]
fn m8_d2_an_unknown_override_fails_the_build() {
    let dir = fixture("unknown-override", vec![kid()], true);
    let e = format!(
        "{:#}",
        lower(&dir, &[("Town_EventScript_Nobody", OVERRIDE_SRC)])
            .err()
            .expect("must fail")
    );
    assert!(e.contains("Town_EventScript_Nobody"), "{e}");
}

/// Override names are decomp labels: an extract named positionally (no `PKR_DECOMP`) applies
/// none, and says so in the report.
#[test]
fn m8_d2_positional_names_apply_no_override() {
    let dir = fixture("positional", vec![kid()], false);
    let s = lower(&dir, &[("Town_EventScript_Nobody", OVERRIDE_SRC)]).unwrap();
    assert!(s.overrides.is_empty());
    assert_eq!(s.skipped_overrides.len(), 1);
}

/// Trainer scripts carry their trainer and their `trainerbattle` type in decomp numbering
/// (`include/constants/battle_setup.h`: 1 `CONTINUE_SCRIPT_NO_MUSIC`, 2 `CONTINUE_SCRIPT`).
#[test]
fn m8_d2_trainer_scripts_carry_flag_and_kind() {
    use IrArg::*;
    let battle = |name: &str, kind: u8, trainer: u16| {
        root(
            name,
            vec![
                insn(
                    "trainerbattle",
                    vec![Byte(kind), Half(trainer), Half(0)]
                        .into_iter()
                        // `SINGLE_NO_INTRO_TEXT` has the defeat text only.
                        .chain((kind != 3).then(|| Text("Town_Text_Intro".into())))
                        .chain([Text("Town_Text_Defeat".into())])
                        .collect(),
                ),
                insn("end", vec![]),
            ],
        )
    };
    let dir = fixture(
        "trainers",
        vec![
            battle("Town_EventScript_Single", 0, 333),
            battle("Town_EventScript_NoIntro", 3, 36),
            kid(),
        ],
        true,
    );
    let s = lower(&dir, &[]).unwrap();
    let mut t = s.trainers.clone();
    t.sort();
    assert_eq!(
        t,
        [
            ("Town_EventScript_NoIntro".to_string(), 36, 3),
            ("Town_EventScript_Single".to_string(), 333, 0),
        ]
    );
}

/// Answers every request with a default of its kind; records them.
#[derive(Default)]
struct Host {
    log: Vec<Request>,
}

impl ScriptHost for Host {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        Some(match r {
            Request::GetFlag(_) | Request::YesNo => Response::Bool(false),
            Request::GetVar(_) | Request::Random(_) | Request::PlayerFacing => Response::Value(0),
            Request::PartySize => Response::Value(1),
            Request::PlayerPos => Response::Pos(0, 0),
            _ => Response::Unit,
        })
    }

    fn resume(&mut self, _: &Request) -> Option<Response> {
        Some(Response::Unit)
    }
}

fn script_id(name: &str) -> Option<ScriptId> {
    content::data::SCRIPT_NAMES
        .iter()
        .position(|&n| n == name)
        .map(|i| ScriptId(i as u16))
}

const NURSE: &str = "OldaleTown_PokemonCenter_1F_EventScript_Nurse";

/// The worked example (`src/overrides/OldaleTown_PokemonCenter_1F_EventScript_Nurse.rs`) runs in
/// place of its script: the nurse's object id first, then `Common_EventScript_PkmnCenterNurse`.
#[test]
fn m8_d2_the_worked_example_override_runs() {
    let Some(id) = script_id(NURSE) else {
        assert!(content::overrides::OVERRIDES.is_empty());
        return;
    };
    assert_eq!(content::overrides::OVERRIDES, [NURSE]);
    assert!(content::REPORT.contains(&format!("overrides: {NURSE}")));
    let game = content::Emerald::for_shells();
    let mut r = Runner::new();
    r.start(game.script(id, r.ctx()));
    let mut h = Host::default();
    for _ in 0..100 {
        if !r.step(&mut h) {
            break;
        }
    }
    assert!(!r.is_running());
    assert_eq!(
        h.log[..3],
        [
            Request::SetVar(content::consts::VAR_0x800B, 1),
            Request::Lock,
            Request::FacePlayer
        ]
    );
    // "No" to resting: the goodbye message, left open for the caller, then the caller's tail.
    assert_eq!(
        h.log[h.log.len() - 3..],
        [Request::WaitMessage, Request::WaitButton, Request::Release]
    );
}

#[test]
fn m8_d2_trainer_table_answers_trainer_flag_of() {
    let game = content::Emerald::for_shells();
    let rows = &content::scripts::TRAINER_SCRIPTS;
    if content::STUB {
        assert!(rows.is_empty());
        return;
    }
    assert!(!rows.is_empty());
    assert!(rows.windows(2).all(|w| w[0].0 < w[1].0));
    for &(id, trainer, kind) in rows.iter() {
        assert_eq!(
            game.trainer_flag_of(ScriptId(id)),
            Some(FlagId(0x500 + trainer))
        );
        assert_eq!(game.trainer_battle_kind(ScriptId(id)), Some(kind));
        assert!(kind <= 12);
    }
    let kid = script_id("LittlerootTown_EventScript_Twin").unwrap();
    assert_eq!(game.trainer_flag_of(kid), None);
    assert_eq!(game.trainer_battle_kind(kid), None);
}

/// The std scripts' texts (`data/text/obtain_item.inc`), which the prelude looks up by name.
#[test]
fn m8_d2_std_script_texts_are_built() {
    if content::STUB {
        return;
    }
    for t in [
        "gText_ObtainedTheItem",
        "gText_PutItemInPocket",
        "gText_PlayerFoundOneItem",
        "gText_TooBadBagIsFull",
    ] {
        assert!(content::data::TEXT_NAMES.binary_search(&t).is_ok(), "{t}");
    }
}

/// One digest per script, in `SCRIPT_NAMES` order.
#[test]
fn m8_d2_every_script_has_a_digest() {
    let names: Vec<&str> = content::data::SCRIPT_DIGESTS.iter().map(|d| d.0).collect();
    assert_eq!(names, content::data::SCRIPT_NAMES);
    assert_eq!(names.len(), content::data::SCRIPT_COUNT);
}
