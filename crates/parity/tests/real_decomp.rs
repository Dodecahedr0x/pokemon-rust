//! The universe on the real decomp (`PKR_DECOMP`, default `../pokeemerald` next to the
//! checkout); skipped with a message when it is absent.

use parity::decomp::Decomp;
use parity::{Category, owners, story};
use std::path::PathBuf;

fn decomp() -> Option<Decomp> {
    let root = std::env::var_os("PKR_DECOMP").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../pokeemerald"),
        PathBuf::from,
    );
    if !root.join("data/specials.inc").exists() {
        eprintln!("no decomp at {}: skipped", root.display());
        return None;
    }
    Some(Decomp::load(&root).expect("loading the decomp"))
}

#[test]
fn the_real_universe_and_its_uses() {
    let Some(d) = decomp() else { return };
    assert!(d.issues.is_empty(), "{:#?}", d.issues);
    for (cat, at_least) in [
        (Category::Command, 200),
        (Category::Special, 500),
        (Category::Behavior, 200),
        (Category::MoveEffect, 200),
        (Category::Ability, 70),
        (Category::HoldEffect, 60),
        (Category::ItemFieldUse, 15),
        (Category::ItemBattleUse, 5),
        (Category::FieldMove, 14),
        (Category::Weather, 15),
        (Category::BattleType, 30),
        (Category::MapType, 10),
    ] {
        let n = d.names(cat).len();
        assert!(n >= at_least, "{cat:?}: {n} entries");
    }
    assert_eq!(d.maps.len(), 518);
    let map = |n: &str| d.maps.iter().find(|m| m.name == n).unwrap();
    let r101 = map("Route101");
    assert_eq!(r101.id, 0x0010);
    assert!(r101.behaviors.contains_key("MB_TALL_GRASS"));
    let pc = map("OldaleTown_PokemonCenter_1F");
    assert!(
        pc.script_sites.contains_key("SPECIAL_HealPlayerParty"),
        "the nurse's common script"
    );
    assert!(pc.script_sites.contains_key("dofieldeffect"));
    let gym = map("RustboroCity_Gym");
    assert!(gym.trainers.contains("TRAINER_ROXANNE_1"));
    assert!(
        gym.battle
            .contains(&(Category::MoveEffect, "EFFECT_DEFENSE_CURL".into()))
    );
    assert!(map("PetalburgWoods").items.contains("ITEM_GREAT_BALL"));
    assert!(map("GraniteCave_B1F").requires_flash);
    // The closure stays per map: Route 101 does not reach the Frontier.
    assert!(
        !r101
            .script_sites
            .contains_key("SPECIAL_CallFrontierUtilFunc")
    );
}

#[test]
fn story_maps_and_screen_openers_are_decomp_names() {
    let Some(d) = decomp() else { return };
    for (_, maps) in story::ACTS {
        for m in *maps {
            assert!(d.maps.iter().any(|x| x.name == *m), "story map {m}");
        }
    }
    for s in owners::SCREENS {
        for o in s.4 {
            assert!(
                d.names(Category::Special).iter().any(|x| x == o)
                    || d.names(Category::Command).iter().any(|x| x == o),
                "{}: opener {o}",
                s.0
            );
        }
    }
}
