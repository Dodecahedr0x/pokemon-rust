//! Milestone 8 Task N: a `--from` db's root jobs start from the root state's progress. From
//! 0, `goals::progress` stopped at the first place goal the root no longer satisfied ("on
//! Route 103"), so a db rooted in Petalburg Woods counted 5 goals, and only a walk back to
//! Route 103 restored them (Act 1 tuning, segment 2).

use content::Emerald;
use engine::Buttons;
use explore::config::{Budget, Config};
use explore::goals::{self, Act};
use explore::wire::Explore;
use game_native::Session;
use game_native::checkpoints::seed;
use game_native::store::Store;
use std::path::Path;

fn stores(dir: &Path) -> Store {
    let base = dir.join("committed");
    let mut rec = game_native::scenario::cached_route().clone();
    rec.nodes.truncate(2);
    seed(&Store::open(&base).unwrap(), &Emerald::for_shells(), &rec).unwrap();
    Store::layered(&base, dir.join("local")).unwrap()
}

#[test]
fn state_progress_counts_up_to_the_last_goal_reached() {
    let mut s = Session::new(Emerald::for_shells());
    let snap = loop {
        if let Ok(cp) = s.checkpoint() {
            break cp.world;
        }
        s.step(Buttons::NONE);
    };
    for act in Act::ALL {
        let all = goals::goals_of(act);
        // A new game: as many goals as `progress` from 0 (none of the later ones hold).
        assert_eq!(
            goals::state_progress(act, &snap),
            goals::progress(act, 0, &snap)
        );
        // A state whose last goal is a flag, past place goals it does not satisfy.
        if content::FLAVOR != content::Flavor::Emerald || act != Act::Act1 {
            continue;
        }
        let mut late = snap.clone();
        let f = content::consts::FLAG_VISITED_PETALBURG_CITY;
        if let Err(i) = late.vars.flags.binary_search(&f) {
            late.vars.flags.insert(i, f);
        }
        let petalburg = all.iter().position(|g| g.name == "petalburg").unwrap() as u32;
        assert_eq!(goals::state_progress(act, &late), petalburg + 1);
        assert!(goals::progress(act, 0, &late) < petalburg);
    }
}

#[test]
fn the_root_job_starts_from_the_root_states_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = stores(dir.path());
    let cfg = Config {
        threads: 2,
        batch: 0,
        ..Config::default()
    };
    let path = dir.path().join("explore.db");
    let mut ex = Explore::open_from(Emerald::for_shells(), &path, cfg, &store, "c00002").unwrap();
    let root = ex.root.clone().unwrap();
    let want = goals::state_progress(ex.cfg.act, &root.checkpoint.world);
    assert_eq!(ex.eval.root_progress, want);
    ex.run(Budget::Generations(2), |_| {}).unwrap();
    let first = ex.db.node(1).unwrap().unwrap();
    assert!(first.parent.is_none());
    assert!(first.progress >= want, "{} < {want}", first.progress);
}
