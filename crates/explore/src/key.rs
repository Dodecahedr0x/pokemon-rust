//! The abstract key (Task B). The fold is part of the db: changing it bumps [`KEY_VERSION`]
//! (`meta.key_version`; a db of another version is refused).
//!
//! A key is a pure function of a capturable [`WorldSnapshot`]: the map, the coarse cell of the
//! player, a digest of the story flags, a digest of the story vars, a party bucket
//! (decision 3), the respawn point, a digest of the object template deltas (Milestone 8:
//! scripts that move an object for good, `setobjectxyperm`/`setobjectmovementtype`) and a
//! digest of the key items held.
//! Temporary and special flags and vars, the flags cleared on every map change and the
//! content's counters, random vars and object graphics vars ([`EXCLUDED_VARS`]) are always left
//! out, so that walking around does not make new keys. `KeyConfig::ignore_*` only adds user
//! exclusions.
//!
//! The content lookups (cleared flags, [`EXCLUDED_VARS`]) read `content` directly, not the
//! `GameData` being simulated: a wrapper (the `testing` faults) never changes them.

use crate::config::KeyConfig;
use crate::model::{AbstractKey, KeyParts, PartyBucket};
use engine::checkpoint::{BagSnap, TemplateDelta, WorldSnapshot};
use engine::data::{Dir, FlagId, GameData, MovementType, Pocket, VarId};
use engine::flow::{FLAG_COUNT, TEMP_FLAGS_END, TEMP_VARS_END, VARS_END};

/// Domain tags, so that a key, a place and the digests never collide by construction.
const TAG_FLAGS: u64 = 0x4b45_5946_4c41_4753; // "KEYFLAGS"
const TAG_VARS: u64 = 0x4b45_5956_4152_5321; // "KEYVARS!"
const TAG_KEY: u64 = 0x4b45_594b_4559_2121; // "KEYKEY!!"

const TAG_PLACE: u64 = 0x4b45_5950_4c41_4345; // "KEYPLACE"
const TAG_TEMPLATES: u64 = 0x4b45_5954_454d_504c; // "KEYTEMPL"
const TAG_KEY_ITEMS: u64 = 0x4b45_594b_4559_4954; // "KEYKEYIT"

/// The version of the key fold, recorded as `meta.key_version`. 1: Milestone 7. 2 (Milestone 8
/// Task K): the respawn point and the template deltas join the key, `VAR_OBJ_GFX_ID_*` leave it.
/// 3 (Task K phase 2): the key items digest joins the key.
pub const KEY_VERSION: u32 = 3;

/// The splitmix64 finalizer (Steele, Lea and Flood): a bijective, platform-independent mix.
pub const fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Folds `v` into the running digest `h` (order-sensitive).
const fn fold(h: u64, v: u64) -> u64 {
    splitmix64(h ^ splitmix64(v))
}

/// The flags cleared on every map change (Gen3 `ClearTempFieldEventData`), from the content.
fn cleared_on_map_change() -> &'static [FlagId] {
    content::Emerald::default()
        .special_ids()
        .flags_cleared_on_map_change
}

/// Whether set flag `f` takes part in the key: a saved, non-temporary flag that the content
/// does not clear on map changes and `cfg` does not ignore.
pub fn story_flag(f: FlagId, cfg: &KeyConfig) -> bool {
    f.0 >= TEMP_FLAGS_END
        && f.0 < FLAG_COUNT
        && !cfg.ignore_flags.contains(&f.0)
        && !cleared_on_map_change().contains(&f)
}

/// Saved vars that never take part in a key: the Emerald step and day counters, the random
/// vars and the object graphics vars `VAR_OBJ_GFX_ID_0..F` (scripts set them to pick a sprite,
/// for a scene or a rival; not story progress) (from `content::consts`). On the stub world,
/// only the counters its walks and wild battles tick (`UpdatePoisonStepCounter` every step,
/// `VAR_DAILY_WILDS`; its story vars are the ids above them); Silice has its own list.
#[cfg(not(pkr_silice))]
pub static EXCLUDED_VARS: &[VarId] = if content::STUB {
    use content::consts::*;
    &[VAR_POISON_STEP_COUNTER, VAR_DAILY_WILDS]
} else {
    use content::consts::*;
    &[
        VAR_REPEL_STEP_COUNT,
        VAR_ICE_STEP_COUNT,
        VAR_MIRAGE_RND_H,
        VAR_MIRAGE_RND_L,
        VAR_FRIENDSHIP_STEP_COUNTER,
        VAR_POISON_STEP_COUNTER,
        VAR_WONDER_NEWS_STEP_COUNTER,
        VAR_DEOXYS_ROCK_STEP_COUNT,
        VAR_ABNORMAL_WEATHER_STEP_COUNTER,
        VAR_FARAWAY_ISLAND_STEP_COUNTER,
        VAR_DAYS,
        VAR_DAILY_WILDS,
        VAR_FANCLUB_LOSE_FAN_TIMER,
        VAR_CRUISE_STEP_COUNT,
        VAR_POKELOT_RND1,
        VAR_POKELOT_RND2,
        VAR_SECRET_BASE_STEP_COUNTER,
        VAR_WALLY_CALL_STEP_COUNTER,
        VAR_SCOTT_FORTREE_CALL_STEP_COUNTER,
        VAR_ROXANNE_CALL_STEP_COUNTER,
        VAR_SCOTT_BF_CALL_STEP_COUNTER,
        VAR_RIVAL_RAYQUAZA_CALL_STEP_COUNTER,
        VAR_OBJ_GFX_ID_0,
        VAR_OBJ_GFX_ID_1,
        VAR_OBJ_GFX_ID_2,
        VAR_OBJ_GFX_ID_3,
        VAR_OBJ_GFX_ID_4,
        VAR_OBJ_GFX_ID_5,
        VAR_OBJ_GFX_ID_6,
        VAR_OBJ_GFX_ID_7,
        VAR_OBJ_GFX_ID_8,
        VAR_OBJ_GFX_ID_9,
        VAR_OBJ_GFX_ID_A,
        VAR_OBJ_GFX_ID_B,
        VAR_OBJ_GFX_ID_C,
        VAR_OBJ_GFX_ID_D,
        VAR_OBJ_GFX_ID_E,
        VAR_OBJ_GFX_ID_F,
    ]
};

/// Silice's [`EXCLUDED_VARS`]: the Gen3 vars it shares that are not story progress (the day
/// counter and the object graphics vars). Silice's own vars are story vars; the Emerald
/// counters it does not have (their numbers may be Silice ids).
#[cfg(pkr_silice)]
pub static EXCLUDED_VARS: &[VarId] = {
    use content::consts::*;
    &[
        VAR_DAYS,
        VAR_OBJ_GFX_ID_0,
        VAR_OBJ_GFX_ID_1,
        VAR_OBJ_GFX_ID_2,
        VAR_OBJ_GFX_ID_3,
        VAR_OBJ_GFX_ID_4,
        VAR_OBJ_GFX_ID_5,
        VAR_OBJ_GFX_ID_6,
        VAR_OBJ_GFX_ID_7,
        VAR_OBJ_GFX_ID_8,
        VAR_OBJ_GFX_ID_9,
        VAR_OBJ_GFX_ID_A,
        VAR_OBJ_GFX_ID_B,
        VAR_OBJ_GFX_ID_C,
        VAR_OBJ_GFX_ID_D,
        VAR_OBJ_GFX_ID_E,
        VAR_OBJ_GFX_ID_F,
    ]
};

/// Whether var `v` takes part in the key: a saved, non-temporary var (`TEMP_VARS_END..VARS_END`;
/// the special vars `0x8000..` are out), not in [`EXCLUDED_VARS`], that `cfg` does not ignore.
pub fn story_var(v: u16, cfg: &KeyConfig) -> bool {
    (TEMP_VARS_END..VARS_END).contains(&v)
        && !EXCLUDED_VARS.contains(&VarId(v))
        && !cfg.ignore_vars.contains(&v)
}

/// The party part of a key.
pub fn party_bucket(snap: &WorldSnapshot, cfg: &KeyConfig) -> PartyBucket {
    let mut b = PartyBucket::default();
    let mut levels: u16 = 0;
    for (i, mon) in snap.party.iter().take(6).enumerate() {
        b.count += 1;
        b.species[i] = mon.species.0;
        levels += u16::from(mon.level);
        if mon.hp == 0 {
            b.fainted += 1;
        }
    }
    b.level_bucket = levels / cfg.level_step.max(1);
    b
}

/// A stable number per movement type (the template digest; never renumber).
fn movement_code(m: MovementType) -> u64 {
    let dir = |d: Dir| Dir::ALL.iter().position(|&x| x == d).unwrap_or(0) as u64;
    match m {
        MovementType::Face(d) => 0x10 | dir(d),
        MovementType::LookAround => 1,
        MovementType::Wander => 2,
        MovementType::WanderVertical => 3,
        MovementType::WanderHorizontal => 4,
        MovementType::WalkInPlace(d) => 0x20 | dir(d),
        MovementType::Invisible => 5,
        // Milestone 8 Task I: the engine's two hash bytes (tag 7..), above the numbers above.
        m => {
            let [tag, arg] = engine::objects::movement_code(m);
            0x1000 | u64::from(tag) << 8 | u64::from(arg)
        }
    }
}

/// Digest of the object template deltas (the snapshot's order: by local id).
pub fn templates_digest(templates: &[TemplateDelta]) -> u64 {
    templates.iter().fold(TAG_TEMPLATES, |h, t| {
        let mut h = fold(h, u64::from(t.local_id));
        h = match t.pos {
            None => fold(h, 0),
            Some((x, y)) => fold(h, 1 << 32 | u64::from(x as u16) << 16 | u64::from(y as u16)),
        };
        fold(h, t.movement.map_or(0, |m| 0x100 | movement_code(m)))
    })
}

/// Digest of the key items held (the `KeyItems` pocket's item ids, sorted: the order they were
/// obtained in does not count).
pub fn key_items_digest(bag: &BagSnap) -> u64 {
    let mut items: Vec<u16> = bag
        .slots
        .iter()
        .filter(|s| s.pocket == Pocket::KeyItems && s.qty > 0)
        .map(|s| s.item.0)
        .collect();
    items.sort_unstable();
    items.dedup();
    items
        .into_iter()
        .fold(TAG_KEY_ITEMS, |h, i| fold(h, u64::from(i)))
}

pub fn key_parts(snap: &WorldSnapshot, cfg: &KeyConfig) -> KeyParts {
    let pos = snap.objects.objs.first().map_or((0, 0), |p| p.pos);
    let shift = u32::from(cfg.cell_shift.min(15));
    let flags = snap
        .vars
        .flags
        .iter()
        .filter(|&&f| story_flag(f, cfg))
        .fold(TAG_FLAGS, |h, f| fold(h, u64::from(f.0)));
    let vars = snap
        .vars
        .vars
        .iter()
        .filter(|(v, value)| *value != 0 && story_var(v.0, cfg))
        .fold(TAG_VARS, |h, (v, value)| {
            fold(h, u64::from(v.0) << 16 | u64::from(*value))
        });
    KeyParts {
        map: snap.field.map,
        cell: (pos.0 >> shift, pos.1 >> shift),
        flags,
        vars,
        party: party_bucket(snap, cfg),
        respawn: snap.respawn,
        templates: templates_digest(&snap.templates),
        key_items: key_items_digest(&snap.bag),
    }
}

/// Everything but the cell, folded from `tag`.
fn fold_parts(tag: u64, parts: &KeyParts) -> u64 {
    let p = &parts.party;
    let mut h = fold(tag, u64::from(parts.map.0));
    h = fold(h, parts.flags);
    h = fold(h, parts.vars);
    h = fold(h, u64::from(p.count));
    for s in p.species {
        h = fold(h, u64::from(s));
    }
    h = fold(h, u64::from(p.level_bucket));
    h = fold(h, u64::from(p.fainted));
    h = fold(h, u64::from(parts.respawn));
    h = fold(h, parts.templates);
    fold(h, parts.key_items)
}

/// Stable splitmix fold, platform-independent.
pub fn key(parts: &KeyParts) -> AbstractKey {
    let h = fold_parts(TAG_KEY, parts);
    // Cell coordinates as their u16 bit patterns (negative coordinates stay distinct).
    let h = fold(h, u64::from(parts.cell.0 as u16));
    AbstractKey(fold(h, u64::from(parts.cell.1 as u16)))
}

/// The place: map, story flags and story vars, folded (no cell, no party). The softlock probe
/// compares places, so a level-up or a faint (the party bucket) inside a sealed pocket does not
/// count as getting out.
pub fn place_region(parts: &KeyParts) -> u64 {
    let h = fold(TAG_PLACE, u64::from(parts.map.0));
    let h = fold(h, parts.flags);
    fold(h, parts.vars)
}

/// The key config. The content's exclusions are built in ([`story_flag`], [`story_var`]), so
/// this equals `KeyConfig::default()`; kept for callers of the Wave 0 contract.
pub fn default_key_config() -> KeyConfig {
    KeyConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_is_the_reference() {
        // splitmix64 with state 0: the first output of the reference generator.
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn exclusions_cover_temp_and_special_ranges() {
        let cfg = KeyConfig::default();
        assert!(!story_flag(FlagId(TEMP_FLAGS_END - 1), &cfg));
        assert!(story_flag(FlagId(TEMP_FLAGS_END), &cfg));
        assert!(!story_flag(FlagId(FLAG_COUNT), &cfg));
        assert!(!story_var(TEMP_VARS_END - 1, &cfg));
        // The first vars after the temp ones are the object graphics vars (excluded on the
        // extracted build); the first story var follows them.
        assert_eq!(story_var(TEMP_VARS_END, &cfg), content::STUB);
        assert!(story_var(TEMP_VARS_END + 0x10, &cfg));
        assert!(!story_var(engine::flow::SPECIAL_VARS_START, &cfg));
        for &f in cleared_on_map_change() {
            assert!(!story_flag(f, &cfg));
        }
    }

    #[test]
    fn content_counters_are_always_excluded() {
        assert_eq!(default_key_config(), KeyConfig::default());
        assert_eq!(EXCLUDED_VARS.len() == 2, content::STUB);
        let cfg = KeyConfig::default();
        for v in EXCLUDED_VARS {
            assert!((TEMP_VARS_END..VARS_END).contains(&v.0));
            assert!(!story_var(v.0, &cfg));
        }
    }

    #[test]
    fn the_place_ignores_the_party_and_the_cell() {
        let a = KeyParts {
            map: engine::data::MapId(7),
            cell: (1, 2),
            flags: 11,
            vars: 13,
            party: Default::default(),
            respawn: 0,
            templates: 0,
            key_items: 0,
        };
        let mut b = a.clone();
        b.cell = (3, 3);
        b.party.level_bucket = 4;
        b.party.fainted = 1;
        assert_ne!(key(&a), key(&b));
        assert_eq!(place_region(&a), place_region(&b));
        for c in [
            KeyParts {
                flags: 12,
                ..a.clone()
            },
            KeyParts {
                vars: 14,
                ..a.clone()
            },
            KeyParts {
                map: engine::data::MapId(8),
                ..a.clone()
            },
        ] {
            assert_ne!(place_region(&a), place_region(&c));
        }
        assert_ne!(place_region(&a), key(&a).0, "its own tag");
    }
}
