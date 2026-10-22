//! Milestone 8 Task K: the key gaps (key versions 2 and 3). The respawn point, the object
//! template deltas and the key items are in the key; the object graphics vars are not.

use content::Emerald;
use engine::checkpoint::{BagSlotSnap, TemplateDelta, WorldSnapshot};
use engine::data::{Dir, ItemId, MovementType, Pocket, VarId};
use explore::key::{
    KEY_VERSION, default_key_config, key, key_items_digest, key_parts, place_region,
    templates_digest,
};
use game_native::Session;

fn snap() -> WorldSnapshot {
    let mut s = Session::new(Emerald::for_shells());
    loop {
        if let Ok(snap) = s.world().snapshot(s.game()) {
            return snap;
        }
        s.step(engine::Buttons::NONE);
    }
}

fn k(s: &WorldSnapshot) -> u64 {
    key(&key_parts(s, &default_key_config())).0
}

fn place(s: &WorldSnapshot) -> u64 {
    place_region(&key_parts(s, &default_key_config()))
}

#[test]
fn the_key_version_is_3() {
    assert_eq!(KEY_VERSION, 3);
}

#[test]
fn key_items_are_in_the_key_in_any_order_other_pockets_are_not() {
    let a = snap();
    let slot = |pocket, index, item| BagSlotSnap {
        pocket,
        index,
        item: ItemId(item),
        qty: 1,
    };
    let with = |slots: Vec<BagSlotSnap>| {
        let mut b = a.clone();
        b.bag.slots = slots;
        b
    };
    let devon = with(vec![slot(Pocket::KeyItems, 0, 0x0105)]);
    assert_ne!(k(&a), k(&devon), "the Devon Goods is progress");
    assert_eq!(place(&a), place(&devon), "the place has no items");
    let both = with(vec![
        slot(Pocket::KeyItems, 0, 0x0105),
        slot(Pocket::KeyItems, 1, 0x0106),
    ]);
    let swapped = with(vec![
        slot(Pocket::KeyItems, 0, 0x0106),
        slot(Pocket::KeyItems, 1, 0x0105),
    ]);
    assert_ne!(k(&devon), k(&both));
    assert_eq!(k(&both), k(&swapped), "the order obtained does not count");
    let potion = with(vec![slot(Pocket::Items, 0, 0x000D)]);
    assert_eq!(k(&a), k(&potion), "other pockets are not in the key");
    assert_eq!(
        key_items_digest(&devon.bag),
        key_items_digest(
            &with(vec![
                slot(Pocket::Items, 0, 0x000D),
                slot(Pocket::KeyItems, 0, 0x0105),
            ])
            .bag
        )
    );
}

#[test]
fn the_respawn_point_is_in_the_key_not_the_place() {
    let a = snap();
    let mut b = a.clone();
    b.respawn = a.respawn.wrapping_add(1);
    assert_ne!(k(&a), k(&b), "a healed-at Pokémon Center is progress");
    assert_eq!(place(&a), place(&b));
}

#[test]
fn template_deltas_are_in_the_key() {
    let a = snap();
    let delta = |pos, movement| TemplateDelta {
        local_id: 3,
        pos,
        movement,
    };
    let mut seen = vec![k(&a)];
    for t in [
        delta(Some((4, 5)), None),
        delta(Some((5, 4)), None),
        delta(None, Some(MovementType::Face(Dir::Up))),
        delta(None, Some(MovementType::Face(Dir::Down))),
        delta(None, Some(MovementType::WalkInPlace(Dir::Up))),
        delta(Some((4, 5)), Some(MovementType::Wander)),
    ] {
        let mut b = a.clone();
        b.templates = vec![t];
        let kb = k(&b);
        assert!(!seen.contains(&kb), "{t:?}");
        seen.push(kb);
        assert_eq!(place(&a), place(&b), "the place has no templates");
    }
    // The digest is over the list: order and local ids count.
    let t1 = delta(Some((1, 1)), None);
    let t2 = TemplateDelta { local_id: 4, ..t1 };
    assert_ne!(templates_digest(&[t1, t2]), templates_digest(&[t2, t1]));
    assert_ne!(templates_digest(&[t1]), templates_digest(&[t2]));
    assert_ne!(templates_digest(&[]), templates_digest(&[t1]));
}

#[test]
fn object_graphics_vars_are_not_in_the_key() {
    if content::STUB {
        return;
    }
    let a = snap();
    let first = content::consts::VAR_OBJ_GFX_ID_0.0;
    for v in first..first + 16 {
        let mut b = a.clone();
        b.vars.vars.retain(|&(x, _)| x != VarId(v));
        let i = b.vars.vars.partition_point(|&(x, _)| x < VarId(v));
        b.vars.vars.insert(i, (VarId(v), 77));
        assert_eq!(k(&a), k(&b), "VAR_OBJ_GFX_ID_{:X}", v - first);
    }
    // The var after them is a story var.
    let mut b = a.clone();
    let v = VarId(first + 16);
    b.vars.vars.retain(|&(x, _)| x != v);
    let i = b.vars.vars.partition_point(|&(x, _)| x < v);
    b.vars.vars.insert(i, (v, 77));
    assert_ne!(k(&a), k(&b));
}
