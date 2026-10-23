//! Task E: the softlock probe says `Trapped` for a walled-in player and `Free` otherwise.

use content::Emerald;
use engine::Buttons;
use engine::checkpoint::Checkpoint;
use engine::data::{GameData, MapId, block_collision};
use explore::softlock::{Trap, map_exits, probe};
use explore::testing::{NOWHERE, WallAround};
use game_native::Session;
use std::collections::VecDeque;

/// A new game settled to its first capturable frame (the policy answers an intro script, as
/// Silice's bar scene).
fn start<G: GameData>(game: G) -> Checkpoint {
    explore::expand::new_game(game)
        .checkpoint()
        .expect("a settled new game is capturable")
}

/// A fake `expand_all` that "walks the reach set": one `WalkTo` per cell reachable from the
/// player (collision only, the start cell included, row-major). A walk ending on a warp cell
/// leaves the map. The end state is the start (no session needed).
fn walk_reach(game: &impl GameData, cp: &Checkpoint) -> Vec<(u64, MapId, Checkpoint)> {
    let map = cp.world.field.map;
    let m = game.map(map);
    let (w, h) = (m.width as i16, m.height as i16);
    let idx = |x: i16, y: i16| y as usize * w as usize + x as usize;
    let mut seen = vec![false; m.blocks.len()];
    let p = cp.world.objects.objs[0].pos;
    seen[idx(p.0, p.1)] = true;
    let mut queue = VecDeque::from([p]);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w || ny >= h || seen[idx(nx, ny)] {
                continue;
            }
            if block_collision(m.blocks[idx(nx, ny)]) == 0 {
                seen[idx(nx, ny)] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| seen[idx(x, y)])
        .map(|(x, y)| {
            let to = m
                .warps
                .iter()
                .find(|wp| (wp.x, wp.y) == (x, y))
                .map_or(map, |wp| wp.dest_map);
            (0, to, cp.clone())
        })
        .collect()
}

#[test]
fn walled_in_start_is_trapped() {
    let inner = Emerald::for_shells();
    let ng = inner.new_game();
    let walled = WallAround {
        inner,
        map: ng.map,
        at: (ng.x, ng.y),
    };
    assert_eq!(map_exits(&walled, ng.map), map_exits(&inner, ng.map) + 1);
    assert!(
        walled
            .map(ng.map)
            .warps
            .iter()
            .any(|w| w.dest_map == NOWHERE)
    );
    let cp = start(walled);
    assert_eq!(cp.world.objects.objs[0].pos, (ng.x, ng.y));
    let exits = map_exits(&walled, ng.map);
    let trap = probe(&cp, 0, exits, 3, 64, &mut |c, _| walk_reach(&walled, c));
    assert_eq!(trap, Trap::Trapped { reached: 1 });

    // Without the wall around the player (the wall moved to the map's corner): free.
    let open = WallAround {
        inner,
        map: ng.map,
        at: (0, 0),
    };
    let cp = start(open);
    let trap = probe(&cp, 0, map_exits(&open, ng.map), 3, 64, &mut |c, _| {
        walk_reach(&open, c)
    });
    assert_eq!(trap, Trap::Free);

    if content::STUB {
        // The plain stub map has no exit: never a finding.
        assert_eq!(map_exits(&inner, ng.map), 0);
        let cp = start(inner);
        let trap = probe(&cp, 0, 0, 3, 64, &mut |c, _| walk_reach(&inner, c));
        assert_eq!(trap, Trap::NoExits);
    }
}

/// `cp` with another frame: a distinct state digest.
fn at_frame(cp: &Checkpoint, frame: u32) -> Checkpoint {
    let mut c = cp.clone();
    c.world.frame = frame;
    c.header.frame = frame;
    c
}

#[test]
fn probe_bfs_depth_breadth_and_regions() {
    let root = start(Emerald::for_shells());
    let map = root.world.field.map;
    // A ternary tree of states numbered by frame (children of n: 3n+1..=3n+3); region 7,
    // except state `odd` (region 8). `calls` records (state, remaining).
    let tree = |odd: u32| {
        // State n > 0 sits at frame BASE + n; the root is state 0.
        const BASE: u32 = 1_000_000;
        let f = move |c: &Checkpoint, remaining: u32, calls: &mut Vec<(u32, u32)>| {
            let n = c.world.frame.saturating_sub(BASE);
            calls.push((n, remaining));
            (1..=3)
                .map(|i| {
                    let child = 3 * n + i;
                    (
                        if child == odd { 8 } else { 7 },
                        map,
                        at_frame(c, BASE + child),
                    )
                })
                .collect::<Vec<_>>()
        };
        (f, Vec::new())
    };

    let (f, calls) = tree(0);
    let calls = std::cell::RefCell::new(calls);
    let mut ex = |c: &Checkpoint, r: u32| f(c, r, &mut calls.borrow_mut());
    assert_eq!(
        probe(&root, 7, 1, 2, 64, &mut ex),
        Trap::Trapped { reached: 12 }
    );
    // BFS order: the root, then depth 1 in order; `remaining` = breadth - reached.
    assert_eq!(&calls.borrow()[..4], &[(0, 64), (1, 61), (2, 58), (3, 55)]);
    assert_eq!(
        probe(&root, 7, 1, 1, 64, &mut ex),
        Trap::Trapped { reached: 3 }
    );
    assert_eq!(
        probe(&root, 7, 1, 3, 5, &mut ex),
        Trap::Trapped { reached: 5 }
    );
    // Nothing probed: nothing claimed.
    assert_eq!(probe(&root, 7, 1, 0, 64, &mut ex), Trap::Free);
    assert_eq!(probe(&root, 7, 1, 3, 0, &mut ex), Trap::Free);
    assert_eq!(probe(&root, 7, 0, 3, 64, &mut ex), Trap::NoExits);
    // Another start region: the first child already differs.
    assert_eq!(probe(&root, 9, 1, 2, 64, &mut ex), Trap::Free);

    // A region change at depth 2 (state 5 = the second child of 1).
    let (f, calls) = tree(5);
    let calls = std::cell::RefCell::new(calls);
    let mut ex = |c: &Checkpoint, r: u32| f(c, r, &mut calls.borrow_mut());
    assert_eq!(
        probe(&root, 7, 1, 1, 64, &mut ex),
        Trap::Trapped { reached: 3 }
    );
    assert_eq!(probe(&root, 7, 1, 2, 64, &mut ex), Trap::Free);
    assert_eq!(
        probe(&root, 7, 1, 2, 4, &mut ex),
        Trap::Trapped { reached: 4 }
    );

    // Another map is free; identical states are expanded once.
    let mut other = |c: &Checkpoint, _| vec![(7, MapId(map.0 ^ 1), c.clone())];
    assert_eq!(probe(&root, 7, 1, 3, 64, &mut other), Trap::Free);
    let mut n = 0;
    let mut same = |c: &Checkpoint, _| {
        n += 1;
        vec![(7, map, c.clone()), (7, map, c.clone())]
    };
    assert_eq!(
        probe(&root, 7, 1, 5, 64, &mut same),
        Trap::Trapped { reached: 2 }
    );
    assert_eq!(n, 1);
}

#[test]
fn end_state_replays_a_trace_to_its_checkpoint() {
    use explore::findings::{end_state, repro_trace};
    let game = Emerald::for_shells();
    // A new game through its opening scene (Silice's bar: A on its messages), then 150 idle
    // frames.
    let mut s = Session::new(game);
    let mut keys = game_native::scenario::to_field(&mut s).unwrap_or_else(|e| panic!("{e}"));
    keys.extend([Buttons::NONE; 150]);
    for _ in 0..150 {
        s.step(Buttons::NONE);
    }
    let runs = game_native::trace::rle(keys.iter().copied());
    let trace = repro_trace(&game, &[&runs[..]], &[]).unwrap();
    assert_eq!(end_state(&game, &trace), Ok(s.checkpoint().unwrap()));
    let mut bad = trace.clone();
    bad.hashes[0].1 ^= 1;
    assert!(end_state(&game, &bad).is_err());
}
