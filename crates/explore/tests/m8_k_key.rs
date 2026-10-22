//! Milestone 8 Task K: the key gaps (key version 2). The respawn point and the object
//! template deltas are in the key; the object graphics vars are not.

use content::Emerald;
use engine::checkpoint::{TemplateDelta, WorldSnapshot};
use engine::data::{Dir, MovementType, VarId};
use explore::key::{
    KEY_VERSION, default_key_config, key, key_parts, place_region, templates_digest,
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
fn the_key_version_is_2() {
    assert_eq!(KEY_VERSION, 2);
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
