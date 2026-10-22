//! Task G smoke run on the extracted (real) flavor, debug: ten generations from an empty temp db
//! reach the first goal (five, the plan's budget, end just before it), and every finding they
//! raise reproduces from a new game. Returns at once on the stub.

use content::Emerald;
use explore::config::{Budget, Config};
use explore::wire::{self, Explore};

#[test]
fn ten_generations_reach_the_first_goal() {
    if content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut ex = Explore::open(
        Emerald::for_shells(),
        &dir.path().join("explore.db"),
        Config::default(),
    )
    .unwrap();
    let sum = ex.run(Budget::Generations(10), |_| {}).unwrap();
    assert_eq!(sum.generations, 10);
    let stats = ex.stats().unwrap();
    assert!(stats.max_progress >= 1, "{stats:?}");
    assert!(stats.maps >= 2, "{stats:?}");
    for f in ex.db.findings().unwrap() {
        wire::reproduce(&ex.game, &ex.db, f.id, &ex.cfg)
            .unwrap()
            .unwrap_or_else(|e| panic!("finding {} ({:?}): {e}", f.id, f.kind));
    }
}
