//! The Silice flavour's generated tables and game root (SIL1 Wave 0).

use super::*;
use crate::consts;
use engine::data::block_collision;
use engine::input::Buttons;
use extract::games::{IdKind, IdsFile};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::vec::Vec;

fn games() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../games/silice")
}

fn ids(kind: IdKind) -> IdsFile {
    extract::export::read_ron(&games().join(format!("ids/{}.ron", kind.file()))).unwrap()
}

#[test]
fn the_maps_are_the_games_dir_maps_at_their_allocated_numbers() {
    let maps: BTreeMap<String, u32> = ids(IdKind::Maps)
        .ids
        .into_iter()
        .filter(|e| !e.removed)
        .map(|e| (e.map.unwrap(), e.num))
        .collect();
    let files = std::fs::read_dir(games().join("maps")).unwrap().count();
    assert_eq!(data::MAP_COUNT, files);
    for id in data::MAP_IDS {
        let m = Game.map(id);
        assert_eq!(maps.get(m.name), Some(&u32::from(id.0)), "{}", m.name);
        assert!(m.width > 0 && m.height > 0);
    }
}

#[test]
fn the_new_game_starts_on_a_walkable_cell_of_the_bedroom() {
    let g = Game.new_game();
    assert_eq!(g.map, consts::MAP_BRICO_BOURG_PLAYER_HOUSE_2F);
    let m = Game.map(g.map);
    assert!(g.x >= 0 && g.y >= 0 && (g.x as u16) < m.width && (g.y as u16) < m.height);
    let block = m.blocks[g.y as usize * m.width as usize + g.x as usize];
    assert_eq!(block_collision(block), 0);
    let (map, x, y) = Game
        .heal_location(Game.special_ids().respawn_default)
        .unwrap();
    assert_eq!((map, x, y), (g.map, g.x, g.y));
}

#[test]
fn the_world_boots_and_idles() {
    let mut w = engine::World::new(&Game);
    assert_eq!(w.map(), consts::MAP_BRICO_BOURG_PLAYER_HOUSE_2F);
    for _ in 0..120 {
        w.step(&Game, Buttons::NONE);
    }
    assert_eq!(w.map(), consts::MAP_BRICO_BOURG_PLAYER_HOUSE_2F);
}

#[test]
fn the_dispatch_has_the_registry_shape() {
    assert_eq!(data::SCRIPT_NAMES.len(), data::SCRIPT_COUNT);
    assert_eq!(data::SCRIPTS.len(), data::SCRIPT_COUNT);
    assert!(data::SCRIPT_NAMES.windows(2).all(|w| w[0] < w[1]));
    let digests: Vec<&str> = data::SCRIPT_DIGESTS.iter().map(|d| d.0).collect();
    assert_eq!(digests, data::SCRIPT_NAMES);
    const { assert!(data::SCRIPT_COUNT >= 1, "the Wave 0 placeholder script") };
    const { assert!(!crate::scripts::DECOMP_NAMES) };
    assert!(crate::overrides::OVERRIDES.is_empty());
    assert!(crate::REPORT.contains("== scripts (build/silice/dispatch.rs) =="));
}

#[test]
fn map_script_references_are_registered_scripts() {
    let mut refs = 0;
    for id in data::MAP_IDS {
        let m = Game.map(id);
        let s = &m.scripts;
        let ids = (m.objects.iter().filter_map(|o| o.script))
            .chain(m.coords.iter().map(|c| c.script))
            .chain(m.bgs.iter().filter_map(|b| match b.kind {
                engine::data::BgKind::Script { script, .. } => Some(script),
                _ => None,
            }))
            .chain(
                [
                    s.on_load,
                    s.on_transition,
                    s.on_resume,
                    s.on_return_to_field,
                ]
                .into_iter()
                .flatten(),
            )
            .chain(s.on_frame.iter().chain(s.on_warp_into).map(|v| v.script));
        for sid in ids {
            assert!(
                usize::from(sid.0) < data::SCRIPT_COUNT,
                "{}: {sid:?}",
                m.name
            );
            refs += 1;
        }
    }
    // The placeholder script is the bedroom window's.
    assert!(refs >= 1);
}

#[test]
fn ids_constants_follow_the_ids_files() {
    // The maps are checked by number above; the flag and var blocks stay in their ranges.
    let first_live = |k: IdKind| ids(k).ids.into_iter().find(|e| !e.removed).unwrap().num;
    let flags = ids(IdKind::Flags);
    assert!(
        flags.base >= 0x113,
        "above every Gen3 story flag Silice reuses"
    );
    assert!(first_live(IdKind::Flags) >= flags.base);
    assert!(first_live(IdKind::Vars) >= 0x4020);
}

#[test]
fn the_build_is_the_silice_flavour() {
    // `PKR_GAME=silice` never falls back to the stub silently (`build/main.rs`).
    assert_eq!(crate::FLAVOR, crate::Flavor::Silice);
    const { assert!(!crate::STUB) };
}

/// The shared `game_data_tables!` methods reach the Silice root: held items have their battle
/// effects (the trait default would answer `None`).
#[test]
fn held_items_have_their_effects() {
    use engine::mon::HoldEffect;
    assert_eq!(
        Game.hold_effect(engine::data::ItemId(consts::ITEM_LEFTOVERS)),
        Some(HoldEffect::Leftovers)
    );
    assert_eq!(
        Game.hold_effect(engine::data::ItemId(consts::ITEM_ORAN_BERRY)),
        Some(HoldEffect::RestoreHp(10))
    );
    assert!(Game.form_changes().is_empty());
}

#[test]
fn the_new_game_script_runs() {
    let w = engine::World::new(&Game);
    assert!(!w.flag(consts::FLAG_SYS_B_DASH));
}
