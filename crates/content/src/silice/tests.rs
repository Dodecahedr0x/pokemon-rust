//! The Silice flavour's generated tables and game root (SIL1 Wave 0; `game.ron` and the
//! prelude: Task E, `e_tests`).

use super::*;
use crate::consts;
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
fn the_world_boots_and_idles() {
    let mut w = engine::World::new(&Game);
    let start = Game.new_game().map;
    assert_eq!(w.map(), start);
    for _ in 0..120 {
        w.step(&Game, Buttons::NONE);
    }
    assert_eq!(w.map(), start);
}

#[test]
fn the_dispatch_has_the_registry_shape() {
    assert_eq!(data::SCRIPT_NAMES.len(), data::SCRIPT_COUNT);
    assert_eq!(data::SCRIPTS.len(), data::SCRIPT_COUNT);
    assert!(data::SCRIPT_NAMES.windows(2).all(|w| w[0] < w[1]));
    let digests: Vec<&str> = data::SCRIPT_DIGESTS.iter().map(|d| d.0).collect();
    assert_eq!(digests, data::SCRIPT_NAMES);
    const { assert!(data::SCRIPT_COUNT >= 1, "the Ch0 scripts") };
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
    // The Ch0 maps are strict (claimed by the `ch00_*.rs` files, SIL1 Task F): an unwritten
    // name there fails the build; only the unclaimed maps' names are dropped and listed.
    assert!(refs >= 1);
    for (map, module) in [
        ("BricoBourg", "ch00_town"),
        ("BricoBourg_Bar", "ch00_bar"),
        ("BricoBourg_FriendsHouse", "ch00_houses"),
        ("BricoBourg_PlayerHouse_1F", "ch00_houses"),
        ("BricoBourg_PlayerHouse_2F", "ch00_houses"),
        ("RouteS01", "ch00_route_s01"),
    ] {
        let line = format!("  strict: {map} (scripts/{module}.rs)");
        assert!(crate::REPORT.contains(&line), "{line}");
    }
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
}

/// SIL3: the forms of `battle/forms.ron` (the stone on its base species, the Mega Ring's flag
/// for the player), their stats and abilities, the Fairy type, and the battle rules' tables.
#[test]
fn the_mega_forms_and_battle_rules_reach_the_game() {
    use engine::data::{FormKind, ItemId, TrainerId};
    use engine::mon::{Ability, Type};
    let f = Game.form_changes();
    let alakazam = f
        .iter()
        .find(|c| c.item == ItemId(consts::ITEM_SIL_ALAKAZITE))
        .expect("the Alakazite");
    assert_eq!(alakazam.form, consts::SPECIES_ALAKAZAM_MEGA);
    assert_eq!(alakazam.kind, FormKind::Mega);
    assert_eq!(alakazam.enable_flag, Some(consts::FLAG_SIL_MODULE_MEGA));
    let t = Game.battle();
    let info = &t.species[usize::from(consts::SPECIES_ALAKAZAM_MEGA.0)];
    assert_eq!(info.base, [55, 50, 65, 150, 175, 95]);
    assert_eq!(info.name, t.species[usize::from(alakazam.base.0)].name);
    let mawile = &t.species[usize::from(consts::SPECIES_MAWILE_MEGA.0)];
    assert_eq!(mawile.types, [Type::Steel, Type::Fairy]);
    assert_eq!(mawile.abilities[0], Ability::HugePower);
    let kyogre = f
        .iter()
        .find(|c| c.form == consts::SPECIES_KYOGRE_PRIMAL)
        .unwrap();
    assert_eq!((kyogre.kind, kyogre.enable_flag), (FormKind::Primal, None));
    assert_eq!(
        t.species[usize::from(kyogre.form.0)].abilities[0],
        Ability::PrimordialSea
    );
    assert!(t.type_chart.iter().any(|m| m.attack == Type::Fairy));
    assert!(Game.trainer_rules(TrainerId(0)).is_none());
    assert!(
        Game.trainer_rules(consts::TRAINER_SIL_REVERSE_1)
            .unwrap()
            .inverse
    );
    let field = Game
        .trainer_rules(consts::TRAINER_SIL_SAMUEL_1)
        .unwrap()
        .field;
    assert!(Game.battle_rule(field.unwrap()).is_some());
    assert!(!Game.trainer_lines(consts::TRAINER_SIL_YOHAN_1).is_empty());
}
