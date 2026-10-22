//! Task B: the abstract key (both flavors).

use content::Emerald;
use engine::checkpoint::WorldSnapshot;
use engine::data::{FlagId, GameData};
use engine::input::Buttons;
use engine::world::World;
use explore::key::{default_key_config, key, key_parts, region};
use game_native::Session;

/// A new game of `game` settled to its first capturable frame.
fn settled<G: GameData>(game: G) -> Session<G> {
    let mut s = Session::new(game);
    for _ in 0..2000 {
        if s.world().capturable().is_ok() {
            return s;
        }
        s.step(Buttons::NONE);
    }
    panic!("a new game never settles: {:?}", s.world().capturable());
}

fn snap() -> WorldSnapshot {
    let s = settled(Emerald::for_shells());
    s.world().snapshot(s.game()).unwrap()
}

fn key_and_region(snap: &WorldSnapshot) -> (u64, u64) {
    let p = key_parts(snap, &default_key_config());
    (key(&p).0, region(&p))
}

#[test]
fn restore_keeps_the_key() {
    let s = settled(Emerald::for_shells());
    let cp = s.checkpoint().unwrap();
    let w = World::restore(s.game(), &cp).unwrap();
    let again = w.snapshot(s.game()).unwrap();
    assert_eq!(key_and_region(&cp.world), key_and_region(&again));
    // Deterministic across calls.
    assert_eq!(key_and_region(&again), key_and_region(&again));
}

#[test]
fn cells_inside_a_block_share_a_key_and_blocks_share_a_region() {
    let cfg = default_key_config();
    let side = 1i16 << cfg.cell_shift;
    let mut a = snap();
    // A cell at the corner of its block, so that +1 stays in the block.
    a.objects.objs[0].pos = (side * 2, side * 2);
    let (k, r) = key_and_region(&a);

    let mut b = a.clone();
    b.objects.objs[0].pos = (side * 2 + 1, side * 2 + 1);
    assert_eq!(key_and_region(&b), (k, r));

    let mut c = a.clone();
    c.objects.objs[0].pos = (side * 3, side * 2);
    let (k2, r2) = key_and_region(&c);
    assert_ne!(k2, k, "another block, another key");
    assert_eq!(r2, r, "the region ignores the cell");
    assert_eq!(key_parts(&c, &cfg).cell, (3, 2));
}

fn with_flag(snap: &WorldSnapshot, f: FlagId) -> WorldSnapshot {
    let mut s = snap.clone();
    if let Err(i) = s.vars.flags.binary_search(&f) {
        s.vars.flags.insert(i, f);
    }
    s
}

#[test]
fn temp_flags_do_not_count_and_story_flags_do() {
    let a = snap();
    let base = key_and_region(&a);
    assert_eq!(key_and_region(&with_flag(&a, FlagId(0x05))), base);
    // A flag cleared on every map change.
    let cleared = Emerald::for_shells()
        .special_ids()
        .flags_cleared_on_map_change;
    for &f in cleared {
        assert_eq!(key_and_region(&with_flag(&a, f)), base);
    }
    let story = [
        content::consts::FLAG_RESCUED_BIRCH,
        content::consts::FLAG_DEFEATED_RIVAL_ROUTE103,
    ]
    .into_iter()
    .find(|f| a.vars.flags.binary_search(f).is_err())
    .expect("an unset story flag");
    let (k, r) = key_and_region(&with_flag(&a, story));
    assert_ne!(k, base.0);
    assert_ne!(r, base.1);
}

#[test]
fn temp_special_and_ignored_vars_do_not_count() {
    let a = snap();
    let base = key_and_region(&a);
    let set = |var: u16, value: u16| {
        let mut s = a.clone();
        let v = engine::data::VarId(var);
        match s.vars.vars.binary_search_by_key(&v, |&(v, _)| v) {
            Ok(i) => s.vars.vars[i].1 = value,
            Err(i) => s.vars.vars.insert(i, (v, value)),
        }
        s
    };
    assert_eq!(key_and_region(&set(engine::flow::VARS_START, 7)), base);
    assert_eq!(key_and_region(&set(0x8004, 7)), base);
    for &v in &default_key_config().ignore_vars {
        assert_eq!(key_and_region(&set(v, 7)), base);
    }
    let story = content::consts::VAR_LITTLEROOT_TOWN_STATE.0;
    let (k, r) = key_and_region(&set(story, 0x77));
    assert_ne!((k, r), base);
}

#[test]
fn party_levels_within_a_bucket_keep_the_key() {
    // The after-starter new game has a party in both flavors.
    let s = settled(Emerald::after_starter());
    let mut a = s.world().snapshot(s.game()).unwrap();
    assert!(!a.party.is_empty());
    let cfg = default_key_config();
    let step = cfg.level_step as u8;
    assert!(step >= 2);
    a.party[0].level = step * 3;
    let (k, r) = key_and_region(&a);
    let mut b = a.clone();
    b.party[0].level = step * 3 + 1;
    assert_eq!(key_and_region(&b), (k, r));
    let mut c = a.clone();
    c.party[0].level = step * 4;
    assert_ne!(key_and_region(&c).0, k);
    assert_eq!(key_parts(&c, &cfg).party.level_bucket, 4);

    let mut fainted = a.clone();
    fainted.party[0].hp = 0;
    assert_eq!(key_parts(&fainted, &cfg).party.fainted, 1);
    assert_ne!(key_and_region(&fainted).0, k);

    let mut empty = a.clone();
    empty.party.clear();
    let p = key_parts(&empty, &cfg).party;
    assert_eq!((p.count, p.species, p.level_bucket), (0, [0; 6], 0));
}

#[test]
fn the_fold_is_stable() {
    // Pinned: the key fold is part of the db schema. A change here is a schema change.
    let parts = explore::model::KeyParts {
        map: engine::data::MapId(0x0009),
        cell: (1, -2),
        flags: 3,
        vars: 4,
        party: explore::model::PartyBucket {
            count: 1,
            species: [258, 0, 0, 0, 0, 0],
            level_bucket: 1,
            fainted: 0,
        },
    };
    assert_eq!(key(&parts).0, 0xbb5c_364e_29cb_f3b6);
    assert_eq!(region(&parts), 0x3042_127e_1a95_ce1c);
}
