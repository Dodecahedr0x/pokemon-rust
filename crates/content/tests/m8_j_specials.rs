//! Milestone 8 Task J: `build/support/specials.ron` classifies every special of the decomp;
//! every `Native` one has an arm in `prelude::special` (run against a recording host: the
//! requests it issues, in order); `callstd` routes every `gStdScripts` entry; the `msgbox`
//! kinds follow `std_msgbox.inc`; the Act 1 specials (reachable from the Act 1 maps' scripts in
//! the decomp, `PKR_DECOMP`) are classified with a reason; the bedroom PC does not softlock.

use content::consts::*;
use content::prelude::{MsgBox, callstd, msgbox, special};
use engine::battle::BattleOutcome;
use engine::berry::BerryTree;
use engine::data::{FlagId, ItemId, TextId, VarId};
use engine::script::{
    BufferSrc, Ctx, Fade, Query, Report, Request, Response, Runner, Script, ScriptHost,
};
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
fn native_ignore_and_default_classes_give_a_reason() {
    let r = reasons();
    for (name, class) in classes() {
        if matches!(class, Class::Native | Class::Ignore | Class::Default(_)) {
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
    /// The answer to `Random`.
    rand: u16,
    /// The answer to `CurrentMap`.
    map: u16,
    /// The answers to `Query` (0 for the others).
    queries: Vec<(Query, u16)>,
    /// The map's metatiles (`Query::Metatile`; 0 elsewhere), which `SetMetatile` changes.
    tiles: BTreeMap<(i16, i16), (u16, bool)>,
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
            Request::Random(_) => Response::Value(self.rand),
            Request::Query(Query::Metatile { x, y }) => {
                Response::Value(self.tiles.get(&(x, y)).map_or(0, |t| t.0))
            }
            Request::SetMetatile {
                x,
                y,
                metatile,
                blocked,
            } => {
                self.tiles.insert((x, y), (metatile, blocked));
                Response::Unit
            }
            Request::Query(q) => Response::Value(
                (self.queries.iter())
                    .find(|&&(k, _)| k == q)
                    .map_or(0, |&(_, v)| v),
            ),
            Request::PlayerFacing | Request::ChooseStarter(_) => Response::Value(0),
            Request::BerryTree(_) => Response::Berry(Default::default()),
            Request::GiveMon { .. } => Response::Value(0),
            Request::PartySize => Response::Value(self.party),
            Request::PlayerPos => Response::Pos(0, 0),
            Request::CurrentMap => Response::Value(self.map),
            Request::YesNo { .. } | Request::RockSmashEncounter => Response::Bool(false),
            Request::Multichoice { .. } => Response::Value(content::MULTI_B_PRESSED),
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

/// A special with no arm is only logged (`special`'s fallback, for an id the lowering never
/// passes); every `Native` one does more.
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

/// `STR_VAR_1` = field string `i` (`extract::tables::FIELD_STRINGS` order).
fn field_string(i: u8) -> Request {
    Request::Buffer {
        slot: 0,
        src: BufferSrc::Std(content::data::FIELD_STRING_BASE + i),
    }
}

/// `extract::tables::FIELD_STRINGS` indices.
const BIG_GUY: u8 = 0;
const DAUGHTER: u8 = 3;
const MOM: u8 = 4;
const DAD: u8 = 5;

/// Each `Native` special's requests, in order, and its result.
#[test]
fn native_specials_issue_their_requests_in_order() {
    let fade_in = vec![Request::Fade(Fade::FromBlack)];
    // The first berry (Cheri): its name, and its stage duration in minutes (`gBerries`).
    let cheri = Request::Buffer {
        slot: 0,
        src: BufferSrc::Item(ItemId(ITEM_CHERI_BERRY)),
    };
    let cheri_minutes = content::data::BERRIES
        .first()
        .map_or(0, |b| u16::from(b.4) * 60);
    let cases: Vec<(&str, u16, Vec<Request>, u16)> = vec![
        ("SPECIAL_HealPlayerParty", 1, vec![Request::HealParty], 0),
        ("SPECIAL_ChangePokemonNickname", 1, fade_in.clone(), 0),
        ("SPECIAL_ChangeBoxPokemonNickname", 1, fade_in.clone(), 0),
        // M7 H: the clock's fade never cleared (two `CapturableStuck` findings).
        // Gen3 `InitTimeBasedEvents` (`clock.c`): the clock is set, `VAR_DAYS` = the RTC's
        // days (0: no RTC until Milestone 10).
        (
            "SPECIAL_StartWallClock",
            1,
            vec![
                Request::SetFlag(FLAG_SYS_CLOCK_SET, true),
                set(VAR_DAYS, 0),
                Request::Fade(Fade::FromBlack),
            ],
            0,
        ),
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
            vec![field_string(BIG_GUY)],
            0,
        ),
        // The rival of a male player is May: a daughter.
        (
            "SPECIAL_GetRivalSonDaughterString",
            1,
            vec![field_string(DAUGHTER)],
            0,
        ),
        // `tv.c`: not in the player's house and `VAR_TEMP_3` unset, so Mom or Dad at random
        // (the host answers 0: Dad).
        (
            "SPECIAL_GetMomOrDadStringForTVMessage",
            1,
            vec![
                Request::CurrentMap,
                Request::GetVar(VAR_TEMP_3),
                Request::Random(2),
                set(VAR_TEMP_3, 2),
                field_string(DAD),
            ],
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
            // Wave E: the talked object's tree (here tree 0, blank; its berry falls back to the
            // first, `GetBerryInfo`).
            vec![
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::BerryTree(0),
                set(VAR_0x8004, 0),
                set(VAR_0x8005, 0),
                set(VAR_0x8006, 0),
                Request::Buffer {
                    slot: 0,
                    src: BufferSrc::Item(ItemId(ITEM_CHERI_BERRY)),
                },
            ],
            0,
        ),
        (
            "SPECIAL_ObjectEventInteractionRemoveBerryTree",
            1,
            vec![
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::SetBerryTree(0, Default::default()),
            ],
            0,
        ),
        (
            "SPECIAL_DoWateringBerryTreeAnim",
            1,
            vec![degraded("SPECIAL_DoWateringBerryTreeAnim")],
            0,
        ),
        (
            "SPECIAL_ObjectEventInteractionGetBerryName",
            1,
            vec![
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::BerryTree(0),
                cheri.clone(),
            ],
            0,
        ),
        (
            "SPECIAL_ObjectEventInteractionGetBerryCountString",
            1,
            vec![
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::BerryTree(0),
                cheri.clone(),
            ],
            0,
        ),
        (
            "SPECIAL_ObjectEventInteractionPickBerryTree",
            1,
            // A blank tree yields nothing (`AddBagItem` of 0 succeeds).
            vec![
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::BerryTree(0),
                Request::ItemAdd {
                    item: ItemId(ITEM_CHERI_BERRY),
                    qty: 0,
                },
                set(VAR_0x8004, 1),
            ],
            0,
        ),
        (
            "SPECIAL_ObjectEventInteractionWaterBerryTree",
            1,
            // A blank tree has no stage to water.
            vec![
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::BerryTree(0),
            ],
            0,
        ),
        (
            "SPECIAL_ObjectEventInteractionPlantBerryTree",
            1,
            // `VAR_ITEM_ID` 0 plants the first berry (`ItemIdToBerryType`), growing.
            vec![
                Request::GetVar(VAR_ITEM_ID),
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::SetBerryTree(
                    0,
                    BerryTree {
                        berry: 1,
                        stage: 1,
                        minutes_until_next_stage: cheri_minutes,
                        ..BerryTree::default()
                    },
                ),
                Request::GetVar(VAR_LAST_TALKED),
                Request::Query(Query::ObjectBerryTree(0)),
                Request::BerryTree(0),
                set(VAR_0x8004, 0),
                set(VAR_0x8005, 0),
                set(VAR_0x8006, 0),
                cheri.clone(),
            ],
            0,
        ),
        (
            "SPECIAL_ScriptMenu_CreatePCMultichoice",
            1,
            vec![
                degraded("SPECIAL_ScriptMenu_CreatePCMultichoice"),
                set(VAR_RESULT, content::MULTI_B_PRESSED),
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
    let cases = [cases, wave_c_cases(&mut covered)].concat();
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
    // The cable car (`CableCarWarp`, `CableCar`): `VAR_0x8004` picks the other station; the
    // ride fades to black, waits 602 frames (`Task_CableCar`'s 570 and its fade out), then
    // warps to (6, 4) with no warp id.
    #[cfg(not(pkr_silice))]
    for (down, map) in [
        (0, MAP_MT_CHIMNEY_CABLE_CAR_STATION),
        (1, MAP_ROUTE112_CABLE_CAR_STATION),
    ] {
        let mut h = Rec::default();
        h.vars.insert(VAR_0x8004.0, down);
        run_special(&mut h, "SPECIAL_CableCarWarp");
        assert_eq!(h.log, [Request::GetVar(VAR_0x8004)]);
        let mut h = Rec::default();
        h.vars.insert(VAR_0x8004.0, down);
        run_special(&mut h, "SPECIAL_CableCar");
        assert_eq!(
            h.log,
            [
                degraded("SPECIAL_CableCar"),
                Request::GetVar(VAR_0x8004),
                Request::Fade(Fade::ToBlack),
                Request::Delay(602),
                Request::Warp {
                    map,
                    warp: None,
                    x: 6,
                    y: 4
                },
            ]
        );
    }
    // `CallFrontierUtilFunc` with no frontier data: `GET_STATUS` answers no challenge (0xFF:
    // a Battle Tent lobby's `ON_FRAME` table then stops), `GET_DATA` 0, the others report.
    for (func, want) in [
        (0, vec![set(VAR_TEMP_CHALLENGE_STATUS, 0xFF)]),
        (
            1,
            vec![degraded("SPECIAL_CallFrontierUtilFunc"), set(VAR_RESULT, 0)],
        ),
        (2, vec![degraded("SPECIAL_CallFrontierUtilFunc")]),
        (
            7,
            vec![Request::Report(Report::Unsupported(
                "FRONTIER_UTIL_FUNC_RESULTS_WINDOW",
            ))],
        ),
    ] {
        let mut h = Rec::default();
        h.vars.insert(VAR_0x8004.0, func);
        run_special(&mut h, "SPECIAL_CallFrontierUtilFunc");
        let mut log = vec![Request::GetVar(VAR_0x8004)];
        log.extend(want);
        assert_eq!(h.log, log, "func {func}");
    }
    covered.insert("SPECIAL_CallFrontierUtilFunc");
    // The Petalburg gym's doors (`PetalburgGymSetDoorMetatiles`): room 2's two doors, both
    // halves impassable, the open frame (0x21C) at once, or the five frames 2 frames apart.
    let doors = |frame: u16| {
        let mut v = Vec::new();
        for x in [1, 7] {
            v.push(Request::SetMetatile {
                x,
                y: 78,
                metatile: frame,
                blocked: true,
            });
            v.push(Request::SetMetatile {
                x,
                y: 79,
                metatile: frame + 8,
                blocked: true,
            });
        }
        v.push(Request::RedrawMap);
        v
    };
    let mut h = Rec::default();
    h.vars.insert(VAR_0x8004.0, 2);
    run_special(&mut h, "SPECIAL_PetalburgGymUnlockRoomDoors");
    let mut want = vec![Request::GetVar(VAR_0x8004)];
    want.extend(doors(0x21C));
    assert_eq!(h.log, want);
    let mut h = Rec::default();
    h.vars.insert(VAR_0x8004.0, 2);
    run_special(&mut h, "SPECIAL_PetalburgGymSlideOpenRoomDoors");
    let mut want = vec![Request::GetVar(VAR_0x8004)];
    for frame in 0x218..=0x21C {
        if frame > 0x218 {
            want.push(Request::Delay(2));
        }
        want.extend(doors(frame));
    }
    assert_eq!(h.log, want);
    covered.insert("SPECIAL_PetalburgGymUnlockRoomDoors");
    covered.insert("SPECIAL_PetalburgGymSlideOpenRoomDoors");
    // `SetSootopolisGymCrackedIceMetatiles`: VAR_TEMP_1 (row 6) bits 0 and 10 are columns 3
    // and 13; the other row vars (all read, in row order) are clear.
    let mut h = Rec::default();
    h.vars.insert(0x4001, 1 | 1 << 10);
    run_special(&mut h, "SPECIAL_SetSootopolisGymCrackedIceMetatiles");
    let mut want = vec![Request::GetVar(VarId(0x4001))];
    for x in [3, 13] {
        want.push(Request::SetMetatile {
            x,
            y: 6,
            metatile: 0x20E,
            blocked: false,
        });
    }
    for v in [
        0x4002, 0x4003, 0x4004, 0x4005, 0x4006, 0x4007, 0x4008, 0x4009, 0x400A,
    ] {
        want.push(Request::GetVar(VarId(v)));
    }
    assert_eq!(h.log, want);
    covered.insert("SPECIAL_SetSootopolisGymCrackedIceMetatiles");
    covered.insert("SPECIAL_CableCarWarp");
    covered.insert("SPECIAL_CableCar");
    // `GetMomOrDadStringForTVMessage` with `VAR_TEMP_3` already set (a second look at the TV):
    // no random draw, as in Gen3; above 2 by parity.
    for (t3, s) in [(1, MOM), (2, DAD), (5, DAD), (6, MOM)] {
        let mut h = Rec::default();
        h.vars.insert(VAR_TEMP_3.0, t3);
        run_special(&mut h, "SPECIAL_GetMomOrDadStringForTVMessage");
        assert_eq!(
            h.log,
            [
                Request::CurrentMap,
                Request::GetVar(VAR_TEMP_3),
                field_string(s)
            ],
            "VAR_TEMP_3 = {t3}"
        );
    }
    // An odd draw: Mom (`VAR_TEMP_3` = 1).
    let mut h = Rec {
        rand: 1,
        ..Rec::default()
    };
    run_special(&mut h, "SPECIAL_GetMomOrDadStringForTVMessage");
    assert!(h.log.contains(&set(VAR_TEMP_3, 1)));
    assert_eq!(h.log.last(), Some(&field_string(MOM)));
    // In the player's own house (Brendan's 1F: the player is male): Mom, with no draw.
    #[cfg(not(pkr_silice))]
    let house = MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F;
    #[cfg(pkr_silice)]
    let house = MAP_BRICO_BOURG_PLAYER_HOUSE_1F;
    let mut h = Rec {
        map: house.0,
        ..Rec::default()
    };
    run_special(&mut h, "SPECIAL_GetMomOrDadStringForTVMessage");
    assert_eq!(
        h.log,
        [
            Request::CurrentMap,
            set(VAR_TEMP_3, 1),
            Request::GetVar(VAR_TEMP_3),
            field_string(MOM)
        ]
    );
    // Every Native special has a case above.
    for (name, class) in classes() {
        if class == Class::Native {
            assert!(covered.contains(name.as_str()), "{name}: no request case");
        }
    }
}

type Case = (&'static str, u16, Vec<Request>, u16);

fn q(query: Query) -> Request {
    Request::Query(query)
}

/// Wave C's `Native` specials on a one-mon party whose queries answer 0; the specials naming
/// an Emerald-only flag or var are covered by `every_native_special_has_an_arm` in Silice.
fn wave_c_cases(covered: &mut BTreeSet<&'static str>) -> Vec<Case> {
    let hoenn: &'static [_] = &content::battle::HOENN_DEX;
    let caught_hoenn = q(Query::DexCount {
        caught: true,
        among: &hoenn[..hoenn.len().saturating_sub(2)],
    });
    let rating = match content::prelude::text_named("gBirchDexRatingText_LessThan10") {
        Some(t) => Request::Message(t),
        None => degraded("SPECIAL_ShowPokedexRatingMessage"),
    };
    let species = |s| q(Query::PartySpecies(s));
    let mut cases: Vec<Case> = vec![
        // Every slot empty (the recording host answers 0): every mon fainted, a whiteout.
        (
            "SPECIAL_TryFieldPoisonWhiteOut",
            1,
            [
                vec![degraded("SPECIAL_TryFieldPoisonWhiteOut")],
                (0..6).map(species).collect(),
                vec![set(VAR_RESULT, 1)],
            ]
            .concat(),
            0,
        ),
        ("SPECIAL_SetCB2WhiteOut", 1, vec![Request::FieldWhiteOut], 0),
        (
            "SPECIAL_ShakeCamera",
            1,
            vec![
                degraded("SPECIAL_ShakeCamera"),
                Request::GetVar(VAR_0x8004),
                Request::GetVar(VAR_0x8005),
                Request::GetVar(VAR_0x8006),
                Request::GetVar(VAR_0x8007),
                Request::ScreenShake {
                    magnitude: 0,
                    frames: 0,
                },
                Request::Delay(0),
            ],
            0,
        ),
        // The recording host answers `false`: no battle.
        (
            "SPECIAL_RockSmashWildEncounter",
            1,
            vec![Request::RockSmashEncounter, set(VAR_RESULT, 0)],
            0,
        ),
        (
            "SPECIAL_ScriptGetPokedexInfo",
            1,
            vec![
                Request::GetVar(VAR_0x8004),
                q(Query::DexCount {
                    caught: false,
                    among: hoenn,
                }),
                q(Query::DexCount {
                    caught: true,
                    among: hoenn,
                }),
                set(VAR_0x8005, 0),
                set(VAR_0x8006, 0),
                Request::GetVar(VAR_NATIONAL_DEX),
            ],
            0,
        ),
        ("SPECIAL_HasAllHoennMons", 1, vec![caught_hoenn.clone()], 0),
        (
            "SPECIAL_ShowPokedexRatingMessage",
            1,
            vec![Request::GetVar(VAR_0x8004), rating],
            0,
        ),
        (
            "SPECIAL_EnableNationalPokedex",
            1,
            vec![
                set(VAR_NATIONAL_DEX, 0x302),
                Request::SetFlag(FLAG_SYS_NATIONAL_DEX, true),
            ],
            0,
        ),
        (
            "SPECIAL_CountPlayerTrainerStars",
            1,
            vec![
                Request::GetFlag(FLAG_SYS_GAME_CLEAR),
                caught_hoenn,
                Request::GetFlag(FLAG_SYS_TOWER_SILVER),
            ],
            0,
        ),
        (
            "SPECIAL_ShouldTryRematchBattle",
            1,
            vec![degraded("SPECIAL_ShouldTryRematchBattle")],
            0,
        ),
        (
            "SPECIAL_IsTrainerReadyForRematch",
            1,
            vec![degraded("SPECIAL_IsTrainerReadyForRematch")],
            0,
        ),
        (
            "SPECIAL_IsTrainerRegistered",
            1,
            vec![Request::GetVar(VAR_0x8004)],
            0,
        ),
        // `Dir::Down`: Gen3 `DIR_SOUTH`.
        (
            "SPECIAL_GetPlayerFacingDirection",
            1,
            vec![Request::PlayerFacing],
            1,
        ),
        (
            "SPECIAL_StorePlayerCoordsInVars",
            1,
            vec![Request::PlayerPos, set(VAR_0x8004, 0), set(VAR_0x8005, 0)],
            0,
        ),
        (
            "SPECIAL_GetPlayerTrainerIdOnesDigit",
            1,
            vec![q(Query::PlayerIdLow)],
            0,
        ),
        (
            "SPECIAL_IsStarterInParty",
            1,
            vec![
                Request::GetVar(VAR_STARTER_MON),
                Request::PartySize,
                species(0),
            ],
            0,
        ),
        (
            "SPECIAL_PlayerHasBerries",
            1,
            vec![q(Query::PocketNonEmpty(engine::data::Pocket::Berries))],
            0,
        ),
        (
            "SPECIAL_Special_AreLeadMonEVsMaxedOut",
            1,
            vec![q(Query::PartyEvTotal(0))],
            0,
        ),
        (
            "SPECIAL_GetLeadMonFriendshipScore",
            1,
            vec![q(Query::PartyFriendship(0))],
            0,
        ),
        (
            "SPECIAL_GetBattleOutcome",
            1,
            vec![degraded("SPECIAL_GetBattleOutcome")],
            0,
        ),
        (
            "SPECIAL_ScriptGetPartyMonSpecies",
            1,
            vec![Request::GetVar(VAR_0x8004), species(0)],
            0,
        ),
        ("SPECIAL_CheckRelicanthWailord", 1, vec![species(0)], 0),
        (
            "SPECIAL_CountPartyAliveNonEggMons",
            1,
            [
                vec![degraded("SPECIAL_CountPartyAliveNonEggMons")],
                (0..6).map(species).collect(),
            ]
            .concat(),
            0,
        ),
        (
            "SPECIAL_CountPartyAliveNonEggMons_IgnoreVar0x8004Slot",
            1,
            [
                vec![Request::GetVar(VAR_0x8004)],
                (1..6).map(species).collect(),
            ]
            .concat(),
            0,
        ),
        (
            "SPECIAL_GetSelectedMonNicknameAndSpecies",
            1,
            vec![
                Request::GetVar(VAR_0x8004),
                Request::Buffer {
                    slot: 0,
                    src: BufferSrc::PartyNick(0),
                },
                species(0),
            ],
            0,
        ),
        (
            "SPECIAL_IsEnoughForCostInVar0x8005",
            1,
            vec![Request::GetVar(VAR_0x8005), Request::MoneyCheck(0)],
            1,
        ),
        (
            "SPECIAL_GiveLeadMonEffortRibbon",
            1,
            vec![
                degraded("SPECIAL_GiveLeadMonEffortRibbon"),
                Request::SetFlag(FLAG_SYS_RIBBON_GET, true),
            ],
            0,
        ),
    ];
    // Mauville's gym on an empty map: the switches, then the beam area's 9x12 cells read.
    let switch = |x, y, metatile| Request::SetMetatile {
        x,
        y,
        metatile,
        blocked: false,
    };
    let beams: Vec<Request> = (5..17)
        .flat_map(|y| (0..9).map(move |x| q(Query::Metatile { x, y })))
        .collect();
    let pressed = [(0, 15), (4, 12), (3, 9), (8, 9)].map(|(x, y)| switch(x, y, 0x206));
    cases.extend([
        (
            "SPECIAL_MauvilleGymPressSwitch",
            1,
            vec![
                Request::GetVar(VAR_0x8004),
                switch(0, 15, 0x206),
                switch(4, 12, 0x205),
                switch(3, 9, 0x205),
                switch(8, 9, 0x205),
            ],
            0,
        ),
        ("SPECIAL_MauvilleGymSetDefaultBarriers", 1, beams.clone(), 0),
        (
            "SPECIAL_MauvilleGymDeactivatePuzzle",
            1,
            [pressed.to_vec(), beams].concat(),
            0,
        ),
    ]);
    let story = [
        "SPECIAL_TryUpdateRusturfTunnelState",
        "SPECIAL_FoundBlackGlasses",
        "SPECIAL_FoundAbandonedShipRoom1Key",
        "SPECIAL_FoundAbandonedShipRoom2Key",
        "SPECIAL_FoundAbandonedShipRoom4Key",
        "SPECIAL_FoundAbandonedShipRoom6Key",
        "SPECIAL_IsMirageIslandPresent",
        "SPECIAL_GetDaysUntilPacifidlogTMAvailable",
        "SPECIAL_SetMirageTowerVisibility",
        "SPECIAL_IncrementDailyPickedBerries",
        "SPECIAL_IncrementDailyPlantedBerries",
        "SPECIAL_OpenPokenavForTutorial",
        "SPECIAL_SetMauvilleOldManObjEventGfx",
    ];
    if cfg!(pkr_silice) {
        covered.extend(story);
    }
    #[cfg(not(pkr_silice))]
    {
        cases.extend([
            // The tutorial's forced call to Mr. Stone sets his PokéNav flag.
            (
                "SPECIAL_OpenPokenavForTutorial",
                1,
                vec![
                    degraded("SPECIAL_OpenPokenavForTutorial"),
                    Request::SetFlag(FLAG_ENABLE_MR_STONE_POKENAV, true),
                ],
                0,
            ),
            (
                "SPECIAL_SetMauvilleOldManObjEventGfx",
                1,
                vec![set(VAR_OBJ_GFX_ID_0, OBJ_EVENT_GFX_BARD)],
                0,
            ),
            // Received on day 0, today is day 0: a week to wait.
            (
                "SPECIAL_GetDaysUntilPacifidlogTMAvailable",
                1,
                vec![Request::GetVar(VAR_PACIFIDLOG_TM_RECEIVED_DAY)],
                7,
            ),
            (
                "SPECIAL_TryUpdateRusturfTunnelState",
                1,
                vec![
                    Request::GetFlag(FLAG_RUSTURF_TUNNEL_OPENED),
                    Request::CurrentMap,
                ],
                0,
            ),
            (
                "SPECIAL_FoundBlackGlasses",
                1,
                vec![Request::GetFlag(FLAG_HIDDEN_ITEM_ROUTE_116_BLACK_GLASSES)],
                0,
            ),
            (
                "SPECIAL_IsMirageIslandPresent",
                1,
                [
                    vec![Request::GetVar(VAR_MIRAGE_RND_H)],
                    (0..6).map(species).collect(),
                ]
                .concat(),
                0,
            ),
            (
                "SPECIAL_SetMirageTowerVisibility",
                1,
                vec![
                    Request::GetVar(VAR_MIRAGE_TOWER_STATE),
                    Request::Random(2),
                    Request::GetFlag(FLAG_FORCE_MIRAGE_TOWER_VISIBLE),
                    Request::SetFlag(FLAG_MIRAGE_TOWER_VISIBLE, false),
                ],
                0,
            ),
            (
                "SPECIAL_IncrementDailyPickedBerries",
                1,
                vec![
                    Request::GetVar(VAR_DAILY_PICKED_BERRIES),
                    set(VAR_DAILY_PICKED_BERRIES, 1),
                ],
                0,
            ),
            (
                "SPECIAL_IncrementDailyPlantedBerries",
                1,
                vec![
                    Request::GetVar(VAR_DAILY_PLANTED_BERRIES),
                    set(VAR_DAILY_PLANTED_BERRIES, 1),
                ],
                0,
            ),
        ]);
        for (name, flag) in [
            (
                "SPECIAL_FoundAbandonedShipRoom1Key",
                FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_1_KEY,
            ),
            (
                "SPECIAL_FoundAbandonedShipRoom2Key",
                FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_2_KEY,
            ),
            (
                "SPECIAL_FoundAbandonedShipRoom4Key",
                FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_4_KEY,
            ),
            (
                "SPECIAL_FoundAbandonedShipRoom6Key",
                FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_6_KEY,
            ),
        ] {
            cases.push((
                name,
                1,
                vec![set(VAR_0x8004, flag.0), Request::GetFlag(flag)],
                0,
            ));
        }
    }
    cases
}

/// Wave C's specials beyond the all-zero answers.
#[test]
fn wave_c_specials_follow_their_answers() {
    // `GetLeadMonFriendshipScore`'s bands (`FRIENDSHIP_NONE` … `FRIENDSHIP_MAX`).
    for (f, band) in [
        (0, 0),
        (1, 1),
        (49, 1),
        (50, 2),
        (100, 3),
        (150, 4),
        (199, 4),
        (200, 5),
        (254, 5),
        (255, 6),
    ] {
        let mut h = Rec::with_party(1);
        h.queries.push((Query::PartyFriendship(0), f));
        let got = run_special(&mut h, "SPECIAL_GetLeadMonFriendshipScore");
        assert_eq!(got, band, "{f}");
    }
    // `MAX_TOTAL_EVS` is 510.
    for (evs, maxed) in [(509, 0), (510, 1)] {
        let mut h = Rec::with_party(1);
        h.queries.push((Query::PartyEvTotal(0), evs));
        let got = run_special(&mut h, "SPECIAL_Special_AreLeadMonEVsMaxedOut");
        assert_eq!(got, maxed, "{evs}");
    }
    let mut h = Rec::with_party(1);
    h.queries.push((Query::PlayerIdLow, 54321));
    assert_eq!(
        run_special(&mut h, "SPECIAL_GetPlayerTrainerIdOnesDigit"),
        1
    );
    // Three alive mons; slot 1 is ignored.
    let mut h = Rec::with_party(3);
    h.vars.insert(VAR_0x8004.0, 1);
    for s in 0..3 {
        h.queries.push((Query::PartySpecies(s), 1));
        h.queries.push((Query::PartyHp(s), 10));
    }
    assert_eq!(
        run_special(
            &mut h,
            "SPECIAL_CountPartyAliveNonEggMons_IgnoreVar0x8004Slot"
        ),
        2
    );
    // The National counts, and the National dex on (`VAR_NATIONAL_DEX` 0x302 and its flag).
    let mut h = Rec::with_party(1);
    h.vars.insert(VAR_0x8004.0, 1);
    h.vars.insert(VAR_NATIONAL_DEX.0, 0x302);
    h.flags.insert(FLAG_SYS_NATIONAL_DEX.0);
    let national = |caught| Query::DexCount { caught, among: &[] };
    h.queries.push((national(false), 9));
    h.queries.push((national(true), 4));
    assert_eq!(run_special(&mut h, "SPECIAL_ScriptGetPokedexInfo"), 1);
    assert_eq!((h.vars[&VAR_0x8005.0], h.vars[&VAR_0x8006.0]), (9, 4));
}

/// `MauvilleGymSetDefaultBarriers` (`field_specials.c`) on the gym's alternate barriers at
/// (3..=8, 10..=13): every beam flips, the poles' beams follow the cell above as changed.
#[test]
fn mauville_gym_barriers_toggle() {
    let mut h = Rec::with_party(1);
    for (cell, m) in [
        ((3, 11), 0x241), // RedBeamV1_On
        ((3, 12), 0x249), // RedBeamV2_On
        ((3, 13), 0x250), // PoleTop_On
        ((4, 10), 0x222), // RedBeamH1_On
        ((4, 11), 0x22A), // RedBeamH3_On
        ((4, 13), 0x230), // GreenBeamH1_Off
        ((4, 14), 0x238), // GreenBeamH3_Off
        ((6, 8), 0x242),  // PoleBottom_On
        ((6, 9), 0x21A),  // FloorTile
        ((6, 10), 0x251), // PoleTop_Off
        ((7, 9), 0x21A),  // FloorTile under nothing: the red beam
    ] {
        h.tiles.insert(cell, (m, false));
    }
    run_special(&mut h, "SPECIAL_MauvilleGymSetDefaultBarriers");
    let want = [
        ((3, 11), (0x243, true)), // PoleBottom_Off
        ((3, 12), (0x21A, false)),
        ((3, 13), (0x251, false)),
        ((4, 10), (0x232, false)),
        ((4, 11), (0x23A, false)),
        ((4, 13), (0x220, false)),
        ((4, 14), (0x228, true)),
        ((6, 8), (0x240, true)),  // GreenBeamV1_On
        ((6, 9), (0x248, true)),  // GreenBeamV2_On: under the green beam just set
        ((6, 10), (0x250, true)), // PoleTop_On
        ((7, 9), (0x249, true)),  // RedBeamV2_On
    ];
    for (cell, t) in want {
        assert_eq!(h.tiles[&cell], t, "{cell:?}");
    }
    // Pressed twice, the switch's barriers are back.
    h.log.clear();
    run_special(&mut h, "SPECIAL_MauvilleGymSetDefaultBarriers");
    assert_eq!(h.tiles[&(4, 10)].0, 0x222);
    assert_eq!(h.tiles[&(6, 8)].0, 0x242);
    // Deactivated: every beam off, every switch pressed.
    h.log.clear();
    run_special(&mut h, "SPECIAL_MauvilleGymDeactivatePuzzle");
    assert_eq!(h.tiles[&(4, 10)], (0x232, false));
    assert_eq!(h.tiles[&(3, 9)], (0x206, false));
}

/// `GetPokedexRatingText` (`birch_pc.c`) with no mythical caught: by tens below 200, then the
/// completed dex.
#[test]
fn pokedex_rating_texts_by_count() {
    let rate = |count: u16| {
        let mut h = Rec::with_party(1);
        let out = std::rc::Rc::new(std::cell::Cell::new(""));
        let o = out.clone();
        run_script(
            &mut h,
            move |ctx| {
                Box::pin(async move {
                    o.set(content::prelude::pokedex_rating_text(&ctx, count).await);
                })
            },
            100,
        );
        out.get()
    };
    assert_eq!(rate(0), "gBirchDexRatingText_LessThan10");
    assert_eq!(rate(9), "gBirchDexRatingText_LessThan10");
    assert_eq!(rate(10), "gBirchDexRatingText_LessThan20");
    assert_eq!(rate(199), "gBirchDexRatingText_LessThan200");
    assert_eq!(rate(200), "gBirchDexRatingText_DexCompleted");
    assert_eq!(rate(201), "gBirchDexRatingText_DexCompleted");
    assert_eq!(rate(202), "gBirchDexRatingText_DexCompleted");
    // Out of range: Gen3's fallback.
    assert_eq!(rate(203), "gBirchDexRatingText_LessThan10");
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
    // Wave C: lowered to `special_vs` after a `trainerbattle`; `gRematchTable`'s flags.
    for n in ["ShouldTryRematchBattle", "IsTrainerRegistered"] {
        assert_eq!(class(n), Class::Native, "{n}");
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
                // `VAR_0x8001`); they write `VAR_RESULT`, and `finditem` removes the ball
                // (`removeobject VAR_LAST_TALKED`).
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
                assert!(
                    h.log
                        .iter()
                        .any(|r| matches!(r, Request::SetVar(v, _) if *v == VAR_RESULT)),
                    "{name}: VAR_RESULT not written: {:?}",
                    h.log
                );
                let removes = h.log.iter().any(|r| matches!(r, Request::RemoveObject(_)));
                assert_eq!(removes, name == "STD_FIND_ITEM", "{name}: {:?}", h.log);
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
                Request::YesNo { default_no: false },
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

/// Whether `r` is the `BedroomPC` special: its `Unsupported` report (the lowering).
fn is_bedroom_pc(r: &Request) -> bool {
    *r == Request::Report(Report::Unsupported("SPECIAL_BedroomPC"))
}

/// The bedroom PC (`special BedroomPC`, `Unsupported` until Milestone 9): the script still
/// ends and releases the player (it does not softlock).
///
/// `Rec` completes `waitstate` at once; the `World` host completes it once the fader, the
/// transition and the doors are idle (`world/host/mod.rs`). Nothing on this path starts any of
/// them (no fade, warp or door before `waitstate`), so the real host completes it at once too.
#[test]
fn bedroom_pc_does_not_softlock() {
    if content::FLAVOR != content::Flavor::Emerald {
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

/// The Act 1 maps (new game to badge 1), decomp folder names (`build/support/act1_maps.ron`,
/// which the content build also reads).
fn act1_maps() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/support/act1_maps.ron");
    extract::export::read_ron(&path).unwrap()
}

/// Scripts the C code starts by metatile behaviour (`field_control_avatar.c`
/// `GetInteractedMetatileScript`), on whiteout or on field poison (`field_poison.c`), found on the
/// Act 1 maps. The trainer-battle
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
    "EventScript_FieldPoison",
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
    /// A `specialvar`: the script reads the result.
    dest: bool,
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
    for map in act1_maps() {
        let map = map.as_str();
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
                    dest: line.starts_with("specialvar "),
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

/// Whether `line` is a `fadescreen*` command (`fadescreen`, `fadescreenswapbuffers`,
/// `fadescreenspeed`) whose mode starts with `mode`.
fn fades(line: &str, mode: &str) -> bool {
    let mut t = idents(line);
    t.next().is_some_and(|c| c.starts_with("fadescreen")) && t.any(|m| m.starts_with(mode))
}

/// The non-`Native` specials reached after a fade to black, each with the `Default` special
/// whose answer keeps the script away from it.
const FADE_GUARDS: &[(&str, &str)] = &[
    // `EventScript_WirelessBoxResults`: FALSE goes to `CableClub_EventScript_AdapterNotConnected`.
    (
        "SPECIAL_ShowWirelessCommunicationScreen",
        "SPECIAL_IsWirelessAdapterConnected",
    ),
];

/// Every special reachable on the Act 1 maps is classified with a reason (no softlock: the
/// script goes on, tested where it matters). A special after a fade to black whose script does
/// not fade back itself is `Native` and fades the field back in (run against `Rec`), or is
/// listed in [`FADE_GUARDS`] with the `Default` special that keeps it unreachable.
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
    let no_reason: Vec<String> = act1
        .iter()
        .filter(|s| !r.contains_key(**s))
        .map(|s| format!("{s} ({:?})", c.get(*s).expect("not in specials.ron")))
        .collect();
    assert!(
        no_reason.is_empty(),
        "reachable in Act 1 with no reason: {no_reason:?}"
    );
    // No stale `specialvar` (D2's `Stats::stale_specialvars`): a script that reads a special's
    // result never gets an `Unsupported` one, whose destination would keep its old value.
    let stale: BTreeSet<(&str, &str)> = sites
        .iter()
        .filter(|s| s.dest && c[&s.special] == Class::Unsupported)
        .map(|s| (s.special.as_str(), s.label.as_str()))
        .collect();
    assert!(
        stale.is_empty(),
        "stale specialvar sites in Act 1: {stale:?}"
    );
    let mut black: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut fade_back: BTreeSet<&str> = BTreeSet::new();
    for site in &sites {
        let faded = site.before.iter().any(|l| fades(l, "FADE_TO_BLACK"));
        let fades_back = site.after.iter().any(|l| fades(l, "FADE_FROM"));
        if !faded || fades_back {
            continue;
        }
        if c[&site.special] == Class::Native {
            // Behaviour, not the reason text: the special fades the field back in itself.
            let mut h = Rec::with_party(1);
            run_special(&mut h, &site.special);
            // A whiteout's map load fades in by itself.
            assert!(
                h.log.contains(&Request::Fade(Fade::FromBlack))
                    || h.log.contains(&Request::FieldWhiteOut),
                "{} in {} leaves the screen black: {:?}",
                site.special,
                site.label,
                h.log
            );
            fade_back.insert(site.special.as_str());
            continue;
        }
        black.insert((site.special.as_str(), site.label.as_str()));
    }
    // The others are behind a `Default` special whose answer never takes their branch.
    let unguarded: Vec<_> = black
        .iter()
        .filter(|(s, _)| !FADE_GUARDS.iter().any(|(g, _)| g == s))
        .collect();
    assert!(
        unguarded.is_empty(),
        "these leave the screen black: {unguarded:?}"
    );
    for (special, _) in &black {
        let guard = FADE_GUARDS.iter().find(|(s, _)| s == special).unwrap().1;
        assert!(
            matches!(c[guard], Class::Default(_)),
            "{special}: its guard {guard} is {:?}",
            c[guard]
        );
    }
    // Not vacuous: the clock and the nickname screens are reached after a fade.
    for s in ["SPECIAL_StartWallClock", "SPECIAL_ChangePokemonNickname"] {
        assert!(fade_back.contains(s), "{s}: not checked after a fade");
    }
    let listed: BTreeSet<&str> = black.iter().map(|&(s, _)| s).collect();
    for (s, _) in FADE_GUARDS {
        assert!(
            listed.contains(s),
            "{s}: in FADE_GUARDS but never after a fade"
        );
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for s in &act1 {
        *counts.entry(format!("{:?}", c[*s])).or_default() += 1;
    }
    eprintln!("Act 1 specials: {} {counts:?}", act1.len());
}

/// Task L: the extract's field strings (`tables/field_strings.ron`) are `std_string`s past
/// `FIELD_STRING_BASE`; the stub world has none.
#[test]
fn field_strings_follow_the_std_strings() {
    use engine::data::GameData;
    let game = content::Emerald::for_shells();
    for i in 0..extract::tables::FIELD_STRINGS.len() as u8 {
        let s = game.std_string(content::data::FIELD_STRING_BASE + i);
        assert_eq!(s.is_some(), !content::STUB, "field string {i}");
    }
    assert!(
        game.std_string(content::data::FIELD_STRING_BASE - 1)
            .is_none()
    );
}
