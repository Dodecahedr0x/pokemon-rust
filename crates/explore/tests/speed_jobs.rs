//! Explorer speed work: the result never depends on `--jobs`, and `run` (each generation's db
//! write overlapped with the next generation's jobs) writes what one committed generation at a
//! time writes. Runs on every flavour, on a small batch.

use content::Emerald;
use explore::config::{Budget, Config};
use explore::db::Db;
use explore::wire::{self, Explore};
use std::path::Path;

const GENERATIONS: u32 = 8;

fn config(threads: usize) -> Config {
    Config {
        threads,
        batch: 16,
        ..Config::default()
    }
}

fn digest(path: &Path) -> u64 {
    Db::open_read_only(path).unwrap().content_digest().unwrap()
}

#[test]
fn the_cli_gives_the_same_db_on_one_and_four_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("explore.ron");
    std::fs::write(&cfg, "(batch: 16)").unwrap();
    let run = |jobs: &str| {
        let db = dir.path().join(format!("jobs{jobs}.db"));
        let args = [
            "run",
            "--generations",
            &GENERATIONS.to_string(),
            "--jobs",
            jobs,
            "--db",
            db.to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
        ];
        explore::cli::main(args.iter().map(|a| a.to_string()).collect()).unwrap();
        digest(&db)
    };
    assert_eq!(run("1"), run("4"));
}

#[test]
fn run_writes_what_committed_generations_write() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("run.db"), dir.path().join("steps.db"));
    let mut ex = Explore::open(Emerald::for_shells(), &a, config(4)).unwrap();
    let mut seen = Vec::new();
    let sum = ex
        .run(Budget::Generations(GENERATIONS), |p| {
            // The progress db has every generation so far committed.
            let s = p.db().unwrap().stats().unwrap();
            seen.push((p.last.generation, s.generation));
        })
        .unwrap();
    assert_eq!(sum.generations, GENERATIONS);
    assert!(seen.iter().all(|(g, db)| g == db), "{seen:?}");
    drop(ex);

    let mut ex = Explore::open(Emerald::for_shells(), &b, config(2)).unwrap();
    for _ in 0..GENERATIONS {
        ex.generation().unwrap().unwrap();
        // Committed on return.
        assert_eq!(ex.db.stats().unwrap().generation, ex.runner.generation - 1);
    }
    drop(ex);
    assert_eq!(digest(&a), digest(&b));
}

#[test]
fn the_default_jobs_leave_a_margin() {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    let d = wire::default_jobs();
    assert!(d >= 1 && d <= n);
    if n >= 4 {
        assert!(d < n);
    }
    assert_eq!(wire::jobs(3), 3);
    assert_eq!(wire::jobs(0), d);
}
