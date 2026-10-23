//! The real flavours' spines, debug: Emerald's from a new game to Roxanne's badge, Silice's
//! to the end of Chapter 0 (`VAR_SIL_STORY` 9), with their frame counts; then a few branches
//! off each, whose findings reproduce. Each test returns at once on the other flavours.

use content::Emerald;
use explore::branch;
use explore::config::Config;
use explore::spine::{self, SpineRun};

/// The spine's frame count when it was recorded: a change means the spine plays differently
/// (an engine or content change on its path), worth a look before updating it.
const EMERALD_BADGE1_FRAMES: u64 = 109_851;

/// The Silice spine's frame count to the end of Chapter 0.
const SILICE_CH0_FRAMES: u64 = 21_821;

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
    assert_eq!(
        s.labels(),
        [
            "lab",
            "oldale_center",
            "may",
            "pokedex",
            "petalburg",
            "woods",
            "rustboro",
            "badge1"
        ]
    );
    assert_eq!(s.frames, EMERALD_BADGE1_FRAMES);
    assert!(s.coverage.maps.len() >= 15, "{:?}", s.coverage.maps);
    branch_from(&s, "lab", 6);
    trainer_approach_is_not_progress(&s);
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
