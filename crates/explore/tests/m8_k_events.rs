//! Milestone 8 Task K phase 2: the explorer reads the engine's events (`World::events`), not a
//! `GameData` wrapper. `Unsupported` findings come from script reports, the stuck and
//! round-trip signatures carry the last script, and every test wrapper forwards the Milestone
//! 8 `GameData` methods.

use content::Emerald;
use engine::battle;
use engine::data::{
    Behavior, Emote, EvoMethod, Evolution, FlagId, Font, GameData, GfxId, ItemId, ItemInfo,
    MapData, MapId, NewGame, Pocket, ScriptId, SpeciesId, SpriteData, TextId, TrainerId, UiText,
    WindowGfx,
};
use engine::mon::{LandEncounters, WildMon};
use engine::script::{Ctx, Script};
use explore::FindingKind;
use explore::config::{Budget, Config};
use explore::findings::{signature, signature_of};
use explore::model::FindingDraft;
use explore::testing::{
    DRIFT_CALLS, Drifting, FadeTrap, GrassPocket, INJECTED_REPORT, PanicOnScript, Rebuilt,
    Reporting, WallAround, same_answers,
};
use explore::wire::{self, Explore};
use std::collections::BTreeSet;

static LAND: LandEncounters = LandEncounters {
    rate: 7,
    mons: [WildMon {
        species: SpeciesId(1),
        min_level: 2,
        max_level: 3,
    }; 12],
};
static ITEM: ItemInfo = ItemInfo {
    name: &[0xBB, 0xFF],
    pocket: Pocket::KeyItems,
    price: 5,
    key: true,
};
static CHOICES: [&[u8]; 2] = [&[0xBB, 0xFF], &[0xBC, 0xFF]];
static EVOS: [Evolution; 1] = [Evolution {
    method: EvoMethod::Level,
    param: 16,
    target: SpeciesId(2),
}];

/// Emerald with an answer of its own for every Milestone 8 method.
#[derive(Clone, Copy)]
struct Answers(Emerald);

impl GameData for Answers {
    fn map(&self, id: MapId) -> &'static MapData {
        self.0.map(id)
    }
    fn has_map(&self, id: MapId) -> bool {
        self.0.has_map(id)
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        self.0.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        self.0.player_sprite()
    }
    fn font(&self) -> &'static Font {
        self.0.font()
    }
    fn window(&self) -> &'static WindowGfx {
        self.0.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        self.0.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        self.0.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        self.0.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        self.0.script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        self.0.new_game()
    }
    fn battle_text(&self, s: battle::Str) -> &'static [u8] {
        self.0.battle_text(s)
    }
    fn wild_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters> {
        if wild_set == 1 {
            Some(&LAND)
        } else {
            self.0.wild_encounters(map, wild_set)
        }
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        (id == ItemId(0x105)).then_some(&ITEM)
    }
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
        (list == 3).then_some(&CHOICES[..])
    }
    fn std_string(&self, id: u8) -> Option<&'static [u8]> {
        (id == 4).then_some(&[0xBD, 0xFF][..])
    }
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        (script == ScriptId(6)).then_some(FlagId(0x500))
    }
    fn emote(&self, e: Emote) -> Option<&'static SpriteData> {
        (e == Emote::Heart).then(|| self.0.player_sprite())
    }
    fn trainer_money(&self, id: TrainerId) -> Option<u8> {
        (id == TrainerId(7)).then_some(9)
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        if species == SpeciesId(8) { &EVOS } else { &[] }
    }
}

#[test]
fn every_wrapper_answers_the_milestone_8_methods_like_its_inner_game() {
    let inner = Answers(Emerald::for_shells());
    assert!(
        same_answers(&Emerald::for_shells(), &inner).is_err(),
        "the fixture answers on its own"
    );
    let map = inner.new_game().map;
    let drifting = Drifting::new(inner);
    // Past the drift: then it answers like the inner game.
    for _ in 0..DRIFT_CALLS {
        assert_eq!(drifting.wild_encounters(map, 1), None);
    }
    let checks: [(&str, Result<(), String>); 8] = [
        (
            "PanicOnScript",
            same_answers(
                &PanicOnScript {
                    inner,
                    script: ScriptId(u16::MAX),
                },
                &inner,
            ),
        ),
        (
            "WallAround",
            same_answers(
                &WallAround {
                    inner,
                    map,
                    at: (1, 1),
                },
                &inner,
            ),
        ),
        (
            "GrassPocket",
            same_answers(
                &GrassPocket {
                    inner,
                    map,
                    at: (1, 1),
                },
                &inner,
            ),
        ),
        ("Drifting", same_answers(&drifting, &inner)),
        (
            "FadeTrap",
            same_answers(
                &FadeTrap {
                    inner,
                    map,
                    at: (1, 1),
                },
                &inner,
            ),
        ),
        (
            "Rebuilt",
            same_answers(&Rebuilt { inner, build_id: 5 }, &inner),
        ),
        ("Reporting", same_answers(&Reporting { inner }, &inner)),
        ("itself", same_answers(&inner, &inner)),
    ];
    for (name, r) in checks {
        assert_eq!(r, Ok(()), "{name}");
    }
}

fn draft(kind: FindingKind, script: Option<ScriptId>) -> FindingDraft {
    FindingDraft {
        kind,
        message: "idle but not capturable for 41 frames: Fade".into(),
        location: None,
        macro_: None,
        tail: Vec::new(),
        map: MapId(0x0101),
        pos: (3, 4),
        script,
        log: Vec::new(),
    }
}

#[test]
fn stuck_and_round_trip_signatures_carry_the_last_script() {
    for kind in [FindingKind::CapturableStuck, FindingKind::RoundTrip] {
        let a = signature(&draft(kind, Some(ScriptId(5))));
        assert_ne!(a, signature(&draft(kind, Some(ScriptId(6)))), "{kind:?}");
        assert_ne!(a, signature(&draft(kind, None)), "{kind:?}");
    }
    for kind in [
        FindingKind::Panic,
        FindingKind::PlannerDivergence,
        FindingKind::Unsupported,
    ] {
        assert_eq!(
            signature(&draft(kind, Some(ScriptId(5)))),
            signature(&draft(kind, None)),
            "{kind:?}"
        );
    }
    let f = draft(FindingKind::Unsupported, None);
    assert_eq!(
        signature(&f),
        signature_of(f.kind, &f.message, None, f.map, Some(ScriptId(9)))
    );
}

#[test]
fn unsupported_findings_come_from_script_reports_and_reproduce() {
    let dir = tempfile::tempdir().unwrap();
    let game = Reporting {
        inner: Emerald::for_shells(),
    };
    let cfg = Config {
        threads: 2,
        batch: 0,
        ..Config::default()
    };
    let mut ex = Explore::open(game, &dir.path().join("explore.db"), cfg).unwrap();
    ex.run(Budget::Generations(2), |_| {}).unwrap();
    let fs: Vec<_> = ex
        .db
        .findings()
        .unwrap()
        .into_iter()
        .filter(|f| f.kind == FindingKind::Unsupported)
        .collect();
    assert!(!fs.is_empty(), "the new game's script reports");
    let mut maps = BTreeSet::new();
    for f in &fs {
        assert_eq!(f.message, format!("unsupported: {INJECTED_REPORT}"));
        assert!(maps.insert(f.map), "one finding per map: {fs:?}");
        wire::reproduce(&game, &ex.db, f.id, &ex.cfg)
            .unwrap()
            .unwrap_or_else(|e| panic!("{f:?}: {e}"));
        // Not on the inner game, which reports nothing.
        assert!(
            wire::reproduce(&game.inner, &ex.db, f.id, &ex.cfg)
                .unwrap()
                .is_err()
        );
    }
    // The run goes on: report-only findings never stop a job.
    assert!(ex.stats().unwrap().nodes > 1);
}
