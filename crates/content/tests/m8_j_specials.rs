//! Milestone 8 Task J: `build/support/specials.ron` classifies every special of the decomp;
//! every `Native` one has an arm in `prelude::special` (run against a recording host: the
//! requests it issues, in order); `callstd` routes every `gStdScripts` entry; the `msgbox`
//! kinds follow `std_msgbox.inc`; the Act 1 specials (reachable from the Act 1 maps' scripts in
//! the decomp, `PKR_DECOMP`) are classified with a reason; the bedroom PC does not softlock.

use content::consts::*;
use content::prelude::{MsgBox, callstd, msgbox, special};
use engine::battle::BattleOutcome;
use engine::data::{FlagId, ItemId, TextId, VarId};
use engine::script::{Ctx, Fade, Report, Request, Response, Runner, Script, ScriptHost};
use extract::codegen::Class;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// ---- the support table ----

fn specials_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("build/support/specials.ron")
}

fn classes() -> BTreeMap<String, Class> {
    extract::export::read_ron(&specials_path()).unwrap()
}

/// The trailing `// reason` of each entry of `specials.ron`.
fn reasons() -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(specials_path()).unwrap();
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('"') else {
            continue;
        };
        let Some((name, tail)) = rest.split_once('"') else {
            continue;
        };
        if let Some((_, reason)) = tail.split_once("//") {
            out.insert(name.to_string(), reason.trim().to_string());
        }
    }
    out
}

fn id_of(name: &str) -> u16 {
    extract::names::SPECIALS
        .value(name)
        .unwrap_or_else(|| panic!("{name} is not a decomp special")) as u16
}

#[test]
fn every_decomp_special_is_classified_once() {
    let c = classes();
    let decomp: BTreeSet<&str> = extract::names::SPECIALS.iter().map(|(_, n)| n).collect();
    let ron: BTreeSet<&str> = c.keys().map(String::as_str).collect();
    assert_eq!(ron, decomp);
}

#[test]
fn ignore_and_default_classes_give_a_reason() {
    let r = reasons();
    for (name, class) in classes() {
        if matches!(class, Class::Ignore | Class::Default(_)) {
            assert!(
                r.get(&name).is_some_and(|s| !s.is_empty()),
                "{name}: no reason"
            );
        }
    }
}

// ---- a recording host ----

/// Answers every request at once with Gen3-neutral values, keeps vars and flags, and records
/// the requests.
#[derive(Default)]
struct Rec {
    log: Vec<Request>,
    vars: BTreeMap<u16, u16>,
    flags: BTreeSet<u16>,
    party: u16,
}

impl Rec {
    fn with_party(party: u16) -> Self {
        Rec {
            party,
            ..Rec::default()
        }
    }

    fn answer(&mut self, r: &Request) -> Response {
        match *r {
            Request::GetFlag(f) => Response::Bool(self.flags.contains(&f.0)),
            Request::SetFlag(f, on) => {
                if on {
                    self.flags.insert(f.0);
                } else {
                    self.flags.remove(&f.0);
                }
                Response::Unit
            }
            Request::GetVar(v) => Response::Value(self.vars.get(&v.0).copied().unwrap_or(0)),
            Request::SetVar(v, x) => {
                self.vars.insert(v.0, x);
                Response::Unit
            }
            Request::Random(_) | Request::PlayerFacing | Request::ChooseStarter(_) => {
                Response::Value(0)
            }
            Request::GiveMon { .. } => Response::Value(0),
            Request::PartySize => Response::Value(self.party),
            Request::PlayerPos => Response::Pos(0, 0),
            Request::YesNo => Response::Bool(false),
            Request::Multichoice { .. } => Response::Value(engine::script::MULTI_B_PRESSED),
            Request::BattleWild { .. } | Request::BattleTrainer { .. } => {
                Response::Value(BattleOutcome::Won as u16)
            }
            Request::ItemAdd { .. }
            | Request::ItemRemove { .. }
            | Request::ItemCheck { .. }
            | Request::ItemSpace { .. }
            | Request::MoneyCheck(_) => Response::Bool(true),
            _ => Response::Unit,
        }
    }
}

impl ScriptHost for Rec {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        Some(self.answer(r))
    }

    fn resume(&mut self, r: &Request) -> Option<Response> {
        Some(self.answer(r))
    }
}

/// Runs `script` to its end (at most `frames` runner steps); whether it ended.
fn run_script(host: &mut Rec, make: impl FnOnce(Ctx) -> Script, frames: u32) -> bool {
    let mut runner = Runner::new();
    runner.start(make(runner.ctx()));
    for _ in 0..frames {
        if !runner.step(host) {
            return true;
        }
    }
    false
}

/// `special(id)` against `host`: its result and the requests it issued.
fn run_special(host: &mut Rec, name: &str) -> u16 {
    let id = id_of(name);
    let out = std::rc::Rc::new(std::cell::Cell::new(0));
    let o = out.clone();
    let ended = run_script(
        host,
        move |ctx| {
            Box::pin(async move {
                o.set(special(&ctx, id).await);
            })
        },
        100,
    );
    assert!(ended, "{name} did not end");
    // Every special first logs its name (as in Wave 0).
    assert_eq!(host.log.first(), Some(&Request::Log(id_name(id))), "{name}");
    host.log.remove(0);
    out.get()
}

fn id_name(id: u16) -> &'static str {
    content::data::SPECIAL_NAMES
        .iter()
        .find(|&&(v, _)| v == id)
        .unwrap()
        .1
}

fn unsupported(log: &[Request]) -> Vec<&'static str> {
    log.iter()
        .filter_map(|r| match r {
            Request::Report(Report::Unsupported(n)) => Some(*n),
            _ => None,
        })
        .collect()
}

// ---- specials ----

/// A special with no arm is only logged (the legacy generator's fallback); every `Native` one
/// does more.
#[test]
fn every_native_special_has_an_arm() {
    for (name, class) in classes() {
        if class != Class::Native {
            continue;
        }
        let mut h = Rec::with_party(1);
        run_special(&mut h, &name);
        assert!(
            !h.log.is_empty(),
            "{name} is Native but reaches the fallback"
        );
        assert!(unsupported(&h.log).is_empty(), "{name}: {:?}", h.log);
    }
}

#[test]
fn a_special_with_no_arm_is_only_logged() {
    let mut h = Rec::default();
    assert_eq!(run_special(&mut h, "SPECIAL_AccessHallOfFamePC"), 0);
    assert_eq!(h.log, []);
}

fn degraded(name: &'static str) -> Request {
    Request::Report(Report::Degraded(name))
}

fn set(v: VarId, x: u16) -> Request {
    Request::SetVar(v, x)
}

/// Each `Native` special's requests, in order, and its result.
#[test]
fn native_specials_issue_their_requests_in_order() {
    let fade_in = vec![Request::Fade(Fade::FromBlack)];
    let cases: Vec<(&str, u16, Vec<Request>, u16)> = vec![
        ("SPECIAL_HealPlayerParty", 1, vec![Request::HealParty], 0),
        ("SPECIAL_ChangePokemonNickname", 1, fade_in.clone(), 0),
        ("SPECIAL_ChangeBoxPokemonNickname", 1, fade_in.clone(), 0),
        // M7 H: the clock's fade never cleared (two `CapturableStuck` findings).
        ("SPECIAL_StartWallClock", 1, fade_in.clone(), 0),
        ("SPECIAL_Special_ViewWallClock", 1, fade_in.clone(), 0),
        ("SPECIAL_DrawWholeMapView", 1, vec![Request::RedrawMap], 0),
        (
            "SPECIAL_InitBirchState",
            1,
            vec![set(VAR_BIRCH_STATE, 0)],
            0,
        ),
        (
            "SPECIAL_GetPCBoxToSendMon",
            1,
            vec![Request::GetVar(VAR_PC_BOX_TO_SEND_MON)],
            0,
        ),
        (
            "SPECIAL_CalculatePlayerPartyCount",
            2,
            vec![Request::PartySize],
            2,
        ),
        (
            "SPECIAL_CountPartyNonEggMons",
            3,
            vec![Request::PartySize],
            3,
        ),
        (
            "SPECIAL_HasEnoughMonsForDoubleBattle",
            1,
            vec![Request::PartySize, set(VAR_RESULT, 1)],
            0,
        ),
        (
            "SPECIAL_HasEnoughMonsForDoubleBattle",
            2,
            vec![
                Request::PartySize,
                degraded("SPECIAL_HasEnoughMonsForDoubleBattle"),
                set(VAR_RESULT, 0),
            ],
            0,
        ),
        (
            "SPECIAL_SetHiddenItemFlag",
            1,
            vec![
                Request::GetVar(VAR_0x8004),
                Request::SetFlag(FlagId(0), true),
            ],
            0,
        ),
        (
            "SPECIAL_GetPlayerBigGuyGirlString",
            1,
            vec![degraded("SPECIAL_GetPlayerBigGuyGirlString")],
            0,
        ),
        (
            "SPECIAL_GetRivalSonDaughterString",
            1,
            vec![degraded("SPECIAL_GetRivalSonDaughterString")],
            0,
        ),
        (
            "SPECIAL_GetMomOrDadStringForTVMessage",
            1,
            vec![degraded("SPECIAL_GetMomOrDadStringForTVMessage")],
            0,
        ),
        // No battle: the script goes on as after Wally's catch.
        (
            "SPECIAL_StartWallyTutorialBattle",
            1,
            vec![degraded("SPECIAL_StartWallyTutorialBattle")],
            0,
        ),
        (
            "SPECIAL_ChoosePartyMon",
            1,
            vec![degraded("SPECIAL_ChoosePartyMon"), set(VAR_0x8004, 0xFF)],
            0,
        ),
        (
            "SPECIAL_DoWaldaNamingScreen",
            1,
            vec![degraded("SPECIAL_DoWaldaNamingScreen"), set(VAR_0x8004, 2)],
            0,
        ),
        (
            "SPECIAL_ShowEasyChatScreen",
            1,
            vec![degraded("SPECIAL_ShowEasyChatScreen"), set(VAR_RESULT, 0)],
            0,
        ),
        (
            "SPECIAL_Bag_ChooseBerry",
            1,
            vec![
                degraded("SPECIAL_Bag_ChooseBerry"),
                set(VAR_ITEM_ID, 0),
                Request::Fade(Fade::FromBlack),
            ],
            0,
        ),
        (
            "SPECIAL_ObjectEventInteractionGetBerryTreeData",
            1,
            vec![
                degraded("SPECIAL_ObjectEventInteractionGetBerryTreeData"),
                set(VAR_0x8004, 0),
                set(VAR_0x8005, 0),
                set(VAR_0x8006, 0),
            ],
            0,
        ),
        (
            "SPECIAL_ScriptMenu_CreatePCMultichoice",
            1,
            vec![
                degraded("SPECIAL_ScriptMenu_CreatePCMultichoice"),
                set(VAR_RESULT, engine::script::MULTI_B_PRESSED),
            ],
            0,
        ),
        (
            "SPECIAL_ScriptMenu_CreateStartMenuForPokenavTutorial",
            1,
            vec![
                degraded("SPECIAL_ScriptMenu_CreateStartMenuForPokenavTutorial"),
                set(VAR_RESULT, 3),
            ],
            0,
        ),
        (
            "SPECIAL_FieldShowRegionMap",
            1,
            vec![
                degraded("SPECIAL_FieldShowRegionMap"),
                Request::Fade(Fade::FromBlack),
            ],
            0,
        ),
    ];
    let mut covered = BTreeSet::new();
    for (name, party, want, result) in cases {
        let mut h = Rec::with_party(party);
        assert_eq!(run_special(&mut h, name), result, "{name} result");
        assert_eq!(h.log, want, "{name} (party {party})");
        covered.insert(name);
    }
    // `ChooseStarter`: the starter screen, the choice, the starter, the first battle.
    let mut h = Rec::with_party(0);
    run_special(&mut h, "SPECIAL_ChooseStarter");
    assert!(matches!(h.log[0], Request::ChooseStarter(_)), "{:?}", h.log);
    assert_eq!(h.log[1], set(VAR_STARTER_MON, 0));
    assert!(matches!(h.log[2], Request::GiveMon { level: 5, .. }));
    assert!(matches!(
        h.log[3],
        Request::BattleWild {
            first_battle: true,
            ..
        }
    ));
    assert_eq!(h.log.len(), 4);
    covered.insert("SPECIAL_ChooseStarter");
    // Every Native special has a case above.
    for (name, class) in classes() {
        if class == Class::Native {
            assert!(covered.contains(name.as_str()), "{name}: no request case");
        }
    }
}

#[test]
fn spec_named_classes() {
    let c = classes();
    let class = |n: &str| c[&format!("SPECIAL_{n}")];
    for n in [
        "StartWallClock",
        "GetPlayerBigGuyGirlString",
        "GetRivalSonDaughterString",
        "DrawWholeMapView",
        "StartWallyTutorialBattle",
        "ChooseStarter",
        "HealPlayerParty",
        "ChangePokemonNickname",
    ] {
        assert_eq!(class(n), Class::Native, "{n}");
    }
    for n in [
        "SetUnlockedPokedexFlags",
        "ShowMapNamePopup",
        "DoPCTurnOnEffect",
        "DoPCTurnOffEffect",
        "TurnOnTVScreen",
        "TurnOffTVScreen",
    ] {
        assert_eq!(class(n), Class::Ignore, "{n}");
    }
    for n in ["ShouldTryRematchBattle", "IsTrainerRegistered"] {
        assert_eq!(class(n), Class::Default(0), "{n}");
    }
    // Every caller of `Common_EventScript_SaveGame` treats 0 as "not saved" and backs out.
    assert_eq!(class("SaveGame"), Class::Default(0));
    for n in ["PlayerPC", "BedroomPC"] {
        assert_eq!(class(n), Class::Unsupported, "{n}");
    }
}

// ---- callstd and msgbox ----

fn run_std(host: &mut Rec, std: u8) {
    let ended = run_script(
        host,
        move |ctx| Box::pin(async move { callstd(&ctx, std).await }),
        100,
    );
    assert!(ended, "std {std} did not end");
    // `callstd` first logs the std's name (as in Wave 0).
    assert!(
        matches!(host.log.first(), Some(Request::Log(_))),
        "{:?}",
        host.log
    );
    host.log.remove(0);
}

#[test]
fn callstd_routes_every_std() {
    for (n, name) in extract::names::STD_SCRIPTS.iter() {
        let n = n as u8;
        let mut h = Rec::default();
        h.vars.insert(VAR_0x8000.0, 13);
        h.vars.insert(VAR_0x8001.0, 2);
        run_std(&mut h, n);
        match name {
            "STD_OBTAIN_ITEM" | "STD_FIND_ITEM" => {
                // Task H's helpers on the `giveitem`/`finditem` operands (`VAR_0x8000`,
                // `VAR_0x8001`); they write `VAR_RESULT`.
                assert_eq!(
                    h.log[..2],
                    [Request::GetVar(VAR_0x8000), Request::GetVar(VAR_0x8001)],
                    "{name}"
                );
                assert!(
                    h.log.contains(&Request::ItemAdd {
                        item: ItemId(13),
                        qty: 2
                    }),
                    "{name}: {:?}",
                    h.log
                );
                assert!(unsupported(&h.log).is_empty(), "{name}");
            }
            "STD_REGISTER_MATCH_CALL" => assert!(h.log.is_empty(), "{name}: {:?}", h.log),
            _ => assert_eq!(
                h.log,
                [Request::Report(Report::Unsupported(name))],
                "{name}"
            ),
        }
    }
}

/// `std_msgbox.inc` (and `Std_MsgboxAutoclose`).
#[test]
fn msgbox_kinds_follow_std_msgbox() {
    let t = TextId(7);
    let say = [
        Request::Message(t),
        Request::WaitMessage,
        Request::WaitButton,
    ];
    let cases: Vec<(MsgBox, Vec<Request>)> = vec![
        (
            MsgBox::Npc,
            [
                vec![Request::Lock, Request::FacePlayer],
                say.to_vec(),
                vec![Request::Release],
            ]
            .concat(),
        ),
        (
            MsgBox::Sign,
            [
                vec![Request::LockAll],
                say.to_vec(),
                vec![Request::ReleaseAll],
            ]
            .concat(),
        ),
        (MsgBox::Default, say.to_vec()),
        (
            MsgBox::YesNo,
            vec![
                Request::Message(t),
                Request::WaitMessage,
                Request::YesNo,
                set(VAR_RESULT, 0),
            ],
        ),
        (
            MsgBox::Autoclose,
            [say.to_vec(), vec![Request::Release]].concat(),
        ),
        (
            MsgBox::GetPoints,
            vec![Request::Message(t), Request::WaitMessage],
        ),
    ];
    for (kind, want) in cases {
        let mut h = Rec::default();
        assert!(run_script(
            &mut h,
            move |ctx| Box::pin(async move { msgbox(&ctx, t, kind).await }),
            100
        ));
        assert_eq!(h.log, want, "{kind:?}");
    }
}

// ---- generated scripts ----

/// Whether `r` is the `BedroomPC` special: its `Unsupported` report (the lowering) or its log
/// line (the legacy generator, through `special`'s fallback).
fn is_bedroom_pc(r: &Request) -> bool {
    match r {
        Request::Report(Report::Unsupported(n)) | Request::Log(n) => n.contains("BedroomPC"),
        _ => false,
    }
}

/// The bedroom PC (`special BedroomPC`, `Unsupported` until Milestone 9): the script still
/// ends and releases the player (it does not softlock).
#[test]
fn bedroom_pc_does_not_softlock() {
    if content::STUB {
        return;
    }
    let mut found = 0;
    for (id, name) in content::data::SCRIPT_NAMES.iter().enumerate() {
        if !name.starts_with("LittlerootTown_BrendansHouse_2F") {
            continue;
        }
        let mut h = Rec::with_party(1);
        let ended = run_script(
            &mut h,
            move |ctx| content::data::script(id as u16, ctx),
            2_000,
        );
        let Some(pc) = h.log.iter().position(is_bedroom_pc) else {
            continue;
        };
        found += 1;
        assert!(ended, "{name} does not end: {:?}", h.log);
        assert!(
            h.log[pc..].contains(&Request::ReleaseAll),
            "{name}: the player is not released: {:?}",
            h.log
        );
    }
    assert!(found > 0, "no script of Brendan's room reaches BedroomPC");
}

// ---- Act 1 ----

/// The Act 1 maps (new game to badge 1), decomp folder names.
const ACT1_MAPS: &[&str] = &[
    "LittlerootTown",
    "LittlerootTown_BrendansHouse_1F",
    "LittlerootTown_BrendansHouse_2F",
    "LittlerootTown_MaysHouse_1F",
    "LittlerootTown_MaysHouse_2F",
    "LittlerootTown_ProfessorBirchsLab",
    "Route101",
    "OldaleTown",
    "OldaleTown_House1",
    "OldaleTown_House2",
    "OldaleTown_Mart",
    "OldaleTown_PokemonCenter_1F",
    "OldaleTown_PokemonCenter_2F",
    "Route103",
    "Route102",
    "PetalburgCity",
    "PetalburgCity_Gym",
    "PetalburgCity_House1",
    "PetalburgCity_House2",
    "PetalburgCity_Mart",
    "PetalburgCity_PokemonCenter_1F",
    "PetalburgCity_PokemonCenter_2F",
    "PetalburgCity_WallysHouse",
    "Route104",
    "Route104_MrBrineysHouse",
    "Route104_PrettyPetalFlowerShop",
    "PetalburgWoods",
    "RustboroCity",
    "RustboroCity_CuttersHouse",
    "RustboroCity_DevonCorp_1F",
    "RustboroCity_DevonCorp_2F",
    "RustboroCity_DevonCorp_3F",
    "RustboroCity_Flat1_1F",
    "RustboroCity_Flat1_2F",
    "RustboroCity_Flat2_1F",
    "RustboroCity_Flat2_2F",
    "RustboroCity_Flat2_3F",
    "RustboroCity_Gym",
    "RustboroCity_House1",
    "RustboroCity_House2",
    "RustboroCity_House3",
    "RustboroCity_Mart",
    "RustboroCity_PokemonCenter_1F",
    "RustboroCity_PokemonCenter_2F",
    "RustboroCity_PokemonSchool",
];

/// Scripts the C code starts by metatile behaviour (`field_control_avatar.c`
/// `GetInteractedMetatileScript`) or on whiteout, found on the Act 1 maps. The trainer-battle
/// scripts are run natively by `trainer_battle` (Task I) and the item std scripts by Task H's
/// helpers, so they are not roots.
const C_ROOTS: &[&str] = &[
    "EventScript_PC",
    "EventScript_TV",
    "EventScript_HiddenItemScript",
    "EventScript_RegionMap",
    "EventScript_RunningShoesManual",
    "EventScript_PictureBookShelf",
    "EventScript_BookShelf",
    "EventScript_PokemonCenterBookShelf",
    "EventScript_Vase",
    "EventScript_EmptyTrashCan",
    "EventScript_ShopShelf",
    "EventScript_Blueprint",
    "EventScript_CableBoxResults",
    "EventScript_WirelessBoxResults",
    "EventScript_Questionnaire",
    "EventScript_WhiteOut",
];

struct Label {
    lines: Vec<String>,
    next: Option<String>,
    file: PathBuf,
}

fn ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn idents(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| t.starts_with(ident_start))
}

/// Every label of the decomp's script sources, in source order, with its body.
fn labels(decomp: &Path) -> BTreeMap<String, Label> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut maps: Vec<PathBuf> = std::fs::read_dir(decomp.join("data/maps"))
        .unwrap()
        .map(|e| e.unwrap().path().join("scripts.inc"))
        .filter(|p| p.exists())
        .collect();
    maps.sort();
    files.extend(maps);
    let mut scripts: Vec<PathBuf> = std::fs::read_dir(decomp.join("data/scripts"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "inc"))
        .collect();
    scripts.sort();
    files.extend(scripts);
    files.push(decomp.join("data/event_scripts.s"));
    let mut out: BTreeMap<String, Label> = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let mut cur: Option<String> = None;
        for raw in text.lines() {
            let line = raw.split('@').next().unwrap().trim_end();
            let label = line
                .strip_suffix("::")
                .or_else(|| line.strip_suffix(':'))
                .filter(|l| l.starts_with(ident_start) && idents(l).count() == 1);
            if let Some(l) = label.filter(|l| idents(l).next() == Some(*l)) {
                if let Some(c) = &cur {
                    out.get_mut(c).unwrap().next = Some(l.to_string());
                }
                cur = Some(l.to_string());
                out.insert(
                    l.to_string(),
                    Label {
                        lines: Vec::new(),
                        next: None,
                        file: file.clone(),
                    },
                );
                continue;
            }
            if let Some(c) = &cur
                && !line.trim().is_empty()
            {
                out.get_mut(c).unwrap().lines.push(line.trim().to_string());
            }
        }
    }
    out
}

/// The script labels of `map.json`'s object, coord and bg events.
fn map_json_scripts(decomp: &Path, map: &str) -> Vec<String> {
    let text =
        std::fs::read_to_string(decomp.join("data/maps").join(map).join("map.json")).unwrap();
    let mut out = Vec::new();
    for part in text.split("\"script\"").skip(1) {
        let v = part.split('"').nth(1).unwrap_or_default();
        out.push(v.to_string());
    }
    out
}

/// A `special`/`specialvar` site: the special and the lines around it.
struct Site {
    special: String,
    label: String,
    before: Vec<String>,
    after: Vec<String>,
}

fn special_of(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix("specialvar ") {
        return rest.split(',').nth(1).map(str::trim);
    }
    line.strip_prefix("special ").map(str::trim)
}

/// The special sites reachable from the Act 1 maps' scripts (a label reachability over-
/// approximation: every label named in a reachable body, and fallthrough).
fn act1_sites(decomp: &Path) -> Vec<Site> {
    let all = labels(decomp);
    let mut stack: Vec<String> = C_ROOTS.iter().map(|s| s.to_string()).collect();
    for map in ACT1_MAPS {
        let dir = decomp.join("data/maps").join(map);
        assert!(dir.is_dir(), "{map}: not a decomp map");
        stack.extend(
            all.iter()
                .filter(|(_, l)| l.file.parent() == Some(dir.as_path()))
                .map(|(n, _)| n.clone()),
        );
        stack.extend(map_json_scripts(decomp, map));
    }
    let mut seen = BTreeSet::new();
    while let Some(l) = stack.pop() {
        let Some(label) = all.get(&l) else { continue };
        if !seen.insert(l) {
            continue;
        }
        for line in &label.lines {
            stack.extend(
                idents(line)
                    .filter(|t| all.contains_key(*t))
                    .map(String::from),
            );
        }
        let last = label.lines.last().map(String::as_str).unwrap_or("");
        let ends = ["end", "return", "goto ", "gotostd", "step_end", "."]
            .iter()
            .any(|p| last.starts_with(p));
        if !ends && let Some(n) = &label.next {
            stack.push(n.clone());
        }
    }
    let mut sites = Vec::new();
    for l in &seen {
        let lines = &all[l].lines;
        for (i, line) in lines.iter().enumerate() {
            if let Some(s) = special_of(line) {
                sites.push(Site {
                    special: format!("SPECIAL_{s}"),
                    label: l.clone(),
                    before: lines[i.saturating_sub(2)..i].to_vec(),
                    after: lines[i + 1..(i + 4).min(lines.len())].to_vec(),
                });
            }
        }
    }
    sites
}

fn decomp() -> Option<PathBuf> {
    std::env::var_os("PKR_DECOMP").map(PathBuf::from)
}

/// Every special reachable on the Act 1 maps is `Native`, `Ignore` or `Default` with a reason,
/// or `Unsupported` with a reason (no softlock: the script goes on, tested where it matters).
/// A special after `fadescreen FADE_TO_BLACK` whose script does not fade back itself is
/// `Native` (it fades the field back in), unless its reason says it is unreachable.
#[test]
fn act1_specials_are_classified_with_a_reason() {
    let Some(decomp) = decomp() else {
        eprintln!("PKR_DECOMP not set: skipped");
        return;
    };
    let c = classes();
    let r = reasons();
    let sites = act1_sites(&decomp);
    let act1: BTreeSet<&str> = sites.iter().map(|s| s.special.as_str()).collect();
    assert!(act1.len() > 50, "only {} Act 1 specials", act1.len());
    for s in &act1 {
        let class = c
            .get(*s)
            .unwrap_or_else(|| panic!("{s}: not in specials.ron"));
        if *class != Class::Native {
            assert!(
                r.contains_key(*s),
                "{s} ({class:?}) is reachable in Act 1 with no reason"
            );
        }
    }
    for site in &sites {
        let faded = site.before.iter().any(|l| l == "fadescreen FADE_TO_BLACK");
        let fades_back = site
            .after
            .iter()
            .any(|l| l.starts_with("fadescreen FADE_FROM"));
        if !faded || fades_back || c[&site.special] == Class::Native {
            continue;
        }
        assert!(
            r.get(&site.special)
                .is_some_and(|x| x.contains("unreachable")),
            "{} in {} leaves the screen black",
            site.special,
            site.label
        );
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for s in &act1 {
        *counts.entry(format!("{:?}", c[*s])).or_default() += 1;
    }
    eprintln!("Act 1 specials: {} {counts:?}", act1.len());
}
