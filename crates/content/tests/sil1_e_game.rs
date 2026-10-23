//! SIL1 Task E: `games/silice/game.ron` (`build/silice/game.rs`), on the build's own code, in any
//! flavour and without the extract: the committed file checks clean and emits its constants, and
//! each rule rejects a broken copy (unknown, removed or unreserved flags, an unwalkable or missing
//! cell, a player name over Gen3's 7 characters or empty, an unknown respawn, a heal location without
//! the Silice prefix or listed twice, a wrong starter count, species or level).

#[allow(dead_code)]
#[path = "../build/battle.rs"]
mod battle;
#[allow(dead_code)]
#[path = "../build/battle_load.rs"]
mod battle_load;
#[allow(dead_code)]
#[path = "../build/blob_data.rs"]
mod blob_data;
#[allow(dead_code)]
#[path = "../build/build_id.rs"]
mod build_id;
#[allow(dead_code)]
#[path = "../../engine/build_support.rs"]
mod build_support;
#[allow(dead_code)]
#[path = "../build/classify/mod.rs"]
mod classify;
#[allow(dead_code)]
#[path = "../build/codegen.rs"]
mod codegen;
#[allow(dead_code)]
#[path = "../build/layouts.rs"]
mod layouts;
#[allow(dead_code)]
#[path = "../build/input.rs"]
mod input;
#[allow(dead_code)]
#[path = "../build/scripts.rs"]
mod scripts;
#[allow(dead_code)]
#[path = "../build/silice/mod.rs"]
mod silice;
#[allow(dead_code)]
#[path = "../build/stub.rs"]
mod stub;

use silice::schema::GameFile;
use silice::{game, ids, load};
use std::path::{Path, PathBuf};

fn games() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/silice")
}

fn committed() -> GameFile {
    game::load(&games()).unwrap()
}

/// `check` of `g` against the committed ids and maps.
fn check(g: &GameFile) -> anyhow::Result<()> {
    let ids = ids::load(&games()).unwrap();
    let maps = load::load_maps(&games()).unwrap();
    game::check(g, &ids, &maps)
}

/// `check` of a copy of the committed file changed by `edit` fails, saying `what`.
fn fails(what: &str, edit: impl FnOnce(&mut GameFile)) {
    let mut g = committed();
    edit(&mut g);
    let e = format!("{:#}", check(&g).unwrap_err());
    assert!(e.contains(what), "{what}: {e}");
}

#[test]
fn the_committed_game_ron_checks_and_emits_its_constants() {
    let g = committed();
    check(&g).unwrap();
    let ids = ids::load(&games()).unwrap();
    let consts = game::consts(&g, &ids).unwrap();
    let get = |n: &str| {
        let c = consts.iter().find(|(name, _, _)| name == n);
        c.unwrap_or_else(|| panic!("{n}")).2.clone()
    };
    // Heal locations are numbered from 1, in file order.
    for (i, h) in g.heal_locations.iter().enumerate() {
        assert_eq!(get(&h.name), (i + 1).to_string());
        assert_eq!(g.heal_location(&h.name), Some(i as u8 + 1));
    }
    assert_eq!(get("SIL_RESPAWN"), g.respawn);
    assert_eq!(get("SIL_RUNNING_FLAG"), g.running_flag);
    assert_eq!(get("SIL_STARTERS"), format!("[{}]", g.starters.join(", ")));
    assert_eq!(get("SIL_START_FACING"), game::facing(g.start.facing));
    assert_eq!(
        get("SIL_NEW_GAME_FLAGS"),
        format!("&[{}]", g.new_game_flags.join(", "))
    );
}

#[test]
fn each_game_ron_rule_rejects_a_broken_copy() {
    fails("neither a Silice id", |g| {
        g.new_game_flags.push("FLAG_SIL_NO_SUCH_FLAG".into())
    });
    fails("is removed", |g| {
        g.new_game_flags.push("FLAG_SIL_HIDE_BAR_SBIRES".into())
    });
    fails("does not reserve", |g| {
        g.cleared_on_map_change
            .push("FLAG_RECEIVED_RUNNING_SHOES".into())
    });
    fails("neither a Silice id", |g| {
        g.new_game_vars.push(("VAR_SIL_NO_SUCH_VAR".into(), 1))
    });
    fails("running_flag", |g| g.running_flag = "FLAG_SIL_NOPE".into());
    fails("not walkable", |g| (g.start.x, g.start.y) = (0, 0));
    fails("outside", |g| g.start.y = 99);
    fails("not in maps/", |g| g.start.map = "NoSuchMap".into());
    fails("player_name", |g| g.player_name = "N".repeat(8));
    fails("player_name", |g| g.player_name.clear());
    fails("not a heal location", |g| {
        g.respawn = "HEAL_LOCATION_SIL_NOPE".into()
    });
    fails("HEAL_LOCATION_SIL_ prefix", |g| {
        g.heal_locations[0].name = "HEAL_LOCATION_HOUSE".into();
        g.respawn = "HEAL_LOCATION_HOUSE".into();
    });
    fails("listed twice", |g| {
        let h = g.heal_locations[0].clone();
        g.heal_locations.push(h)
    });
    fails("not walkable", |g| {
        (g.heal_locations[0].x, g.heal_locations[0].y) = (0, 0)
    });
    fails("three", |g| {
        g.starters.pop();
    });
    fails("not a Gen3 species", |g| {
        g.starters[0] = "SPECIES_NOPE".into()
    });
    fails("starter_level", |g| g.starter_level = 0);
}
