//! The real flavours' spines, debug: Emerald's from a new game through Act 1 (Roxanne's badge)
//! and on (Task S: its bypassed engine gaps listed in [`SPINE_BLOCKERS`]), Silice's
//! to the end of Chapter 0 (`VAR_SIL_STORY` 9), with their frame counts; then a few branches
//! off each, whose findings reproduce. Each test returns at once on the other flavours.

use content::Emerald;
use explore::branch;
use explore::config::Config;
use explore::spine::{self, SpineRun};

/// The spine's frame count when it was recorded: a change means the spine plays differently
/// (an engine or content change on its path), worth a look before updating it.
const EMERALD_BADGE1_FRAMES: u64 = 104_879;

/// The whole Emerald spine's frame count (to its last step, to its last checkpoint).
const EMERALD_FRAMES: u64 = 2_992_885;

/// The Emerald spine's checkpoints.
const EMERALD_LABELS: &[&str] = &[
    "lab",
    "oldale_center",
    "may",
    "pokedex",
    "petalburg",
    "woods",
    "rustboro",
    "badge1",
    "devon_goods",
    "mr_stone",
    "dewford",
    "steven",
    "badge2",
    "museum",
    "badge3",
    "badge4",
    "badge5",
    "route118_east",
    "route119",
    "castform",
    "hm_fly",
    "fortree",
    "devon_scope",
    "badge6",
    "lilycove",
    "magma_emblem",
    "magma_hideout",
    "submarine_stolen",
    "aqua_hideout",
    "mossdeep",
    "badge7",
    "space_center",
    "hm_dive",
    "seafloor_cavern",
    "cave_of_origin",
    "rayquaza_awakened",
    "hm_waterfall",
    "badge8",
    "ever_grande",
    "victory_road",
    "hall_of_fame",
];

/// The engine gaps the Emerald spine steps over with `Step::Bypass`, in spine order: the
/// "Spine blockers" table of `docs/plans/2026-10-23-emerald-parity.md`. Fixing a gap means
/// deleting its bypass (and its line here).
const SPINE_BLOCKERS: &[&str] = &[
    "fly-mauville",
    "fly-slateport",
    "fly-lilycove",
    "water-door-in",
    "water-door-out",
    "mossdeep-gym-puzzle",
    "steven-multi",
    "dive-seafloor",
    "legendaries-scene",
    "dive-sootopolis",
    "dive-out-sootopolis",
    "sky-pillar-layouts",
    "rayquaza-scene",
    "dive-sootopolis",
    "dive-out-sootopolis",
    "waterfall-ever-grande",
    "league-mart",
    "game-clear",
];

/// The Silice spine's frame count to the end of Chapter 0 (21,821 until `2fc4c16` redrew
/// Brico-Bourg on its own tileset: the walks are 8 frames shorter).
const SILICE_CH0_FRAMES: u64 = 21_813;

fn play() -> SpineRun {
    let s = spine::run(&Emerald::for_shells(), spine::steps(), &Config::default());
    assert!(
        s.complete(),
        "{}",
        s.error.as_ref().map_or(String::new(), |e| e.to_string())
    );
    s
}

/// A few branches from `label`, none a bug or an unexpected divergence.
fn branch_from(s: &SpineRun, label: &str, n: usize) {
    let game = Emerald::for_shells();
    let cfg = Config {
        jobs: 4,
        ..Config::default()
    };
    let origins = branch::origins(s, &cfg.branch, Some(label));
    let mut targets = branch::all_targets(&game, s, &origins, &cfg);
    targets.truncate(n);
    assert!(!targets.is_empty(), "targets from {label}");
    for r in branch::run_all(&game, s, &targets, &cfg) {
        assert!(r.findings.is_empty(), "{:?}: {}", r.target, r.message);
    }
}

#[test]
fn spine_reaches_badge1() {
    if content::FLAVOR != content::Flavor::Emerald {
        return;
    }
    let s = play();
    assert_eq!(s.labels()[..8], EMERALD_LABELS[..8]);
    let badge1 = s.labelled("badge1").expect("badge1");
    assert_eq!(badge1.frame, EMERALD_BADGE1_FRAMES);
    // Act 1 plays without a bypass: its inputs reproduce from a new game.
    assert!(s.first_bypass_frame().is_none_or(|f| f >= badge1.frame));
    assert!(s.coverage.maps.len() >= 15, "{:?}", s.coverage.maps);
    branch_from(&s, "lab", 6);
    trainer_approach_is_not_progress(&s);
}

#[test]
fn spine_reaches_hall_of_fame() {
    if content::FLAVOR != content::Flavor::Emerald {
        return;
    }
    let s = play();
    assert_eq!(s.labels(), EMERALD_LABELS);
    assert_eq!(s.frames, EMERALD_FRAMES);
    let issues: Vec<&str> = s.bypasses().into_iter().map(|(_, i)| i).collect();
    assert_eq!(
        issues, SPINE_BLOCKERS,
        "the bypasses are the recorded blockers"
    );
}

/// A trainer that spotted the player stays where it stopped for the rest of the visit (its
/// template moved, Gen3 `OverrideTemplateCoordsForObjectEvent`): which ones did depends on the
/// path a walk took, so it is not progress (it made the 7 unexpected divergences on Route 104
/// after `petalburg`, whose windows walked Petalburg Woods past other sight lines). A script's
/// `setobjectxyperm` on any other object is.
fn trainer_approach_is_not_progress(s: &SpineRun) {
    use engine::checkpoint::TemplateDelta;
    use engine::data::GameData;
    let game = Emerald::for_shells();
    let key = Config::default().key;
    let woods = s.labelled("woods").expect("woods");
    let base = spine::Progress::of(&game, &woods.end.world, &s.spine_flags, &key);
    let objects = game.map(woods.map()).objects;
    let moved = |o: &engine::data::ObjectEvent| {
        let mut w = woods.end.world.clone();
        w.templates.push(TemplateDelta {
            local_id: o.local_id,
            pos: Some((o.x + 1, o.y)),
            movement: None,
        });
        spine::Progress::of(&game, &w, &s.spine_flags, &key)
    };
    let trainer = (objects.iter().find(|o| o.trainer_type == 1)).expect("a woods trainer");
    let other = (objects.iter().find(|o| o.trainer_type == 0)).expect("a woods object");
    assert_eq!(moved(trainer), base);
    assert_ne!(moved(other), base);
}

#[test]
fn spine_reaches_ch0_end() {
    if content::FLAVOR != content::Flavor::Silice {
        return;
    }
    let s = play();
    assert_eq!(
        s.labels(),
        [
            "bar",
            "explosion",
            "mallette",
            "chase",
            "night",
            "starter",
            "route_s1",
            "first_battle",
            "teo",
            "ch0_end"
        ]
    );
    assert_eq!(s.frames, SILICE_CH0_FRAMES);
    branch_from(&s, "starter", 6);
}
