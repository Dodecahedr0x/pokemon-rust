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
const EMERALD_BADGE1_FRAMES: u64 = 104_523;

/// The whole Emerald spine's frame count (to its last step, to its last checkpoint); 3,032,538
/// before Wave G's doubles (Fortree's and Mossdeep's gym pairs, Tate and Liza, Steven);
/// 3,172,864 before a macro released an A the last one ended on (Gen3 `newKeys`: +1 to +4
/// frames from Route 118, and Victory Road B1F's `Wait(2)`s gone).
const EMERALD_FRAMES: u64 = 3_166_366;

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
const SPINE_BLOCKERS: &[&str] = &[];

/// The Silice spine's frame count to the end of Chapter 0 (21,821 until `2fc4c16` redrew
/// Brico-Bourg on its own tileset: the walks are 8 frames shorter; 21,813 until the bar's and
/// the town's shakes became `Request::ScreenShake` in place of a fade out and in: 34 shorter).
const SILICE_CH0_FRAMES: u64 = 21_779;

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

/// A spine segment from a saved position, for working on a stretch of a long spine without
/// replaying it from a new game (`--ignored`, Emerald). `PKR_SPINE_SAVE` names the save file:
/// when it is missing, the spine plays from a new game up to step `PKR_SPINE_FROM` and saves
/// that position there (the encoded checkpoint, then the step options); then the steps from
/// `PKR_SPINE_FROM` to `PKR_SPINE_TO` (default: the end) play from it, and the test prints
/// where they stopped.
/// `spine::run_from` at step `at` from `cp` with `opts`.
fn run_at(
    game: &Emerald,
    steps: &'static [spine::Step],
    cfg: &Config,
    at: usize,
    cp: std::sync::Arc<engine::checkpoint::Checkpoint>,
    opts: spine::StepOpts,
) -> SpineRun {
    let start = spine::Start {
        end: cp,
        index: at,
        opts,
        frame: 0,
    };
    spine::run_from(game, steps, cfg, Some(start))
}

#[test]
#[ignore]
fn spine_segment() {
    if content::FLAVOR != content::Flavor::Emerald {
        return;
    }
    let var = |k: &str| std::env::var(k).ok();
    let from: usize = var("PKR_SPINE_FROM")
        .expect("PKR_SPINE_FROM")
        .parse()
        .unwrap();
    let save = var("PKR_SPINE_SAVE").expect("PKR_SPINE_SAVE");
    let steps = spine::steps();
    // `PKR_SPINE_LIST=1`: the steps with their indices, and nothing else.
    if var("PKR_SPINE_LIST").is_some() {
        for (i, s) in steps.iter().enumerate() {
            println!("step {i} {s:?}");
        }
        return;
    }
    let to = var("PKR_SPINE_TO").map_or(steps.len(), |t| {
        t.parse::<usize>().unwrap().min(steps.len())
    });
    let game = Emerald::for_shells();
    let mut cfg = Config::default();
    // `PKR_SPINE_ROUNDTRIP=0`: no round-trip detector (a stretch across a known checkpoint gap).
    if var("PKR_SPINE_ROUNDTRIP").is_some_and(|v| v == "0") {
        cfg.detect.roundtrip_every = 0;
    }
    let load = |path: &str| {
        let bytes = std::fs::read(path).unwrap();
        let (cp, o) = bytes.split_at(bytes.len() - 4);
        let cp = engine::checkpoint::decode(cp).unwrap();
        let opts = spine::StepOpts {
            run_wild: o[0] != 0,
            best_move: o[1] != 0,
            learn: o[2] != 0,
            choice: (o[3] != 0xFF).then_some(o[3]),
        };
        (std::sync::Arc::new(cp), opts)
    };
    // The party healed (HP, status, PP), outside the game's rules: a red spine's workaround.
    let heal = |cp: &engine::checkpoint::Checkpoint| {
        let mut cp = cp.clone();
        for m in &mut cp.world.party {
            use engine::data::GameData;
            m.hp = m.stats[0];
            m.status = Default::default();
            for (pp, mv) in m.pp.iter_mut().zip(m.moves) {
                *pp = mv.map_or(0, |mv| game.battle().move_info(mv).pp);
            }
        }
        cp.header.build = engine::checkpoint::BuildHash {
            engine: 0,
            content: 0,
        };
        let w = engine::world::World::restore(&game, &cp).unwrap();
        w.checkpoint(&game).unwrap()
    };
    let dump = |s: &SpineRun| {
        if !s.complete() {
            for p in s.positions.iter().rev().take(12) {
                let hp: Vec<_> = (p.end.world.party.iter())
                    .map(|m| (m.hp, m.level, m.status, m.pp, m.moves))
                    .collect();
                println!("{} {:?} {:?} {hp:?}", p.index, p.map(), p.pos());
            }
            for m in s.played.iter().rev().take(20) {
                println!("played {m:?}");
            }
            for n in s.notes.iter().rev().take(10) {
                println!("note {n:?}");
            }
        }
    };
    let write = |s: &SpineRun| {
        dump(s);
        assert!(s.complete(), "{:?}", s.error);
        let p = s.positions.last().unwrap();
        let mut bytes = engine::checkpoint::encode(&p.end);
        let o = p.opts;
        bytes.extend([
            o.run_wild as u8,
            o.best_move as u8,
            o.learn as u8,
            o.choice.unwrap_or(0xFF),
        ]);
        std::fs::write(&save, bytes).unwrap();
        println!("saved step {from} at frame {}", p.frame);
    };
    // `PKR_SPINE_BASE=<step>:<save>`: the save is made from that other save instead.
    if !std::path::Path::new(&save).exists()
        && let Some(base) = var("PKR_SPINE_BASE").filter(|b| !b.is_empty())
    {
        let (at, path) = base.split_once(':').expect("<step>:<save>");
        let (cp, opts) = load(path);
        write(&run_at(
            &game,
            &steps[..from],
            &cfg,
            at.parse().unwrap(),
            cp,
            opts,
        ));
    }
    if !std::path::Path::new(&save).exists() {
        // `PKR_SPINE_HEAL_AT=<step>,<step>,...`: a red spine's workaround (outside the game's
        // rules): the party is healed (HP, status, PP) at those steps on the way.
        let heals: Vec<usize> = var("PKR_SPINE_HEAL_AT")
            .map(|h| h.split(',').map(|n| n.parse().unwrap()).collect())
            .unwrap_or_default();
        let mut s = spine::run(
            &game,
            &steps[..heals.first().copied().unwrap_or(from)],
            &cfg,
        );
        for (k, &h) in heals.iter().enumerate() {
            assert!(s.complete(), "{:?}", s.error);
            let p = s.positions.last().unwrap();
            let cp = heal(&p.end);
            let next = heals.get(k + 1).copied().unwrap_or(from);
            s = run_at(
                &game,
                &steps[..next],
                &cfg,
                h,
                std::sync::Arc::new(cp),
                p.opts,
            );
        }
        write(&s);
    }
    let (mut cp, mut opts) = load(&save);
    // `PKR_SPINE_SEG_HEAL=<step>,...`: heals (as `PKR_SPINE_HEAL_AT`) inside the segment.
    let mut at = from;
    for h in var("PKR_SPINE_SEG_HEAL")
        .filter(|h| !h.is_empty())
        .map(|h| {
            h.split(',')
                .map(|n| n.parse::<usize>().unwrap())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    {
        let s = run_at(&game, &steps[..h], &cfg, at, cp, opts);
        dump(&s);
        assert!(s.complete(), "{:?}", s.error);
        let p = s.positions.last().unwrap();
        cp = std::sync::Arc::new(heal(&p.end));
        opts = p.opts;
        at = h;
    }
    let s = run_at(&game, &steps[..to], &cfg, at, cp, opts);
    let last = s.positions.last().unwrap();
    println!(
        "segment {from}..{to}: reached step {} ({:?} at {:?}), {} frames, labels {:?}",
        last.index,
        last.map(),
        last.pos(),
        s.frames,
        s.labels(),
    );
    for m in &last.end.world.party {
        println!(
            "party {:?} lv {} hp {} moves {:?}",
            m.species, m.level, m.hp, m.moves
        );
    }
    for r in s.reports.iter().take(40) {
        println!("report: {r:?}");
    }
    dump(&s);
    assert!(s.complete(), "{:?}", s.error);
}
