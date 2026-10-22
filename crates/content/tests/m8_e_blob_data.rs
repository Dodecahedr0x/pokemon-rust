//! Milestone 8 Task E: the generator's blobs, tables and digests, on in-memory fixture worlds
//! (the stub world plus what each test adds).

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
#[path = "../../engine/build_support.rs"]
mod build_support;
#[allow(dead_code)]
#[path = "../build/classify/mod.rs"]
mod classify;
#[allow(dead_code)]
#[path = "../build/codegen.rs"]
mod codegen;
#[allow(dead_code)]
#[path = "../build/input.rs"]
mod input;
#[allow(dead_code)]
#[path = "../build/scripts.rs"]
mod scripts;
#[allow(dead_code)]
#[path = "../build/stub.rs"]
mod stub;

use extract::schema::{
    EncounterHeader, Item, ItemPocket, LandEncounters, MapFile, WildMon, to_rows,
};
use std::collections::BTreeMap;

/// The stub world with a second map, `StubRoute` (`MAP_ROUTE101`'s number), on the same
/// tilesets.
fn two_maps(route_blocks: impl Fn(&mut Vec<u16>)) -> input::World {
    let mut w = stub::world();
    let mut m: MapFile = w.maps[0].clone();
    m.name = "StubRoute".into();
    m.num = 16;
    let mut blocks = extract::schema::from_rows(&m.layout.blocks);
    route_blocks(&mut blocks);
    m.layout.blocks = to_rows(&blocks, m.layout.width as usize);
    w.maps.push(m);
    w
}

#[test]
fn maps_sharing_a_tileset_share_it_in_the_blob() {
    let (one, bin_one, _) = codegen::data(&stub::world(), 0).unwrap();
    // Same blocks: the second map adds nothing to the blob.
    let (two, bin_two, _) = codegen::data(&two_maps(|_| {}), 0).unwrap();
    assert_eq!(bin_one.len(), bin_two.len());
    assert_eq!(two.matches("pub static TS_").count(), 2);
    assert_eq!(two.matches("primary: &TS_StubPrimary").count(), 2);
    assert_eq!(one.matches("primary: &TS_StubPrimary").count(), 1);
    // Other blocks: only those are added (4-aligned).
    let (_, bin_other, report) = codegen::data(&two_maps(|b| b[17] ^= 1), 0).unwrap();
    let blocks = stub::WIDTH as usize * stub::HEIGHT as usize * 2;
    assert_eq!(bin_other.len(), bin_one.len() + blocks.next_multiple_of(4));
    assert!(report.contains("data.bin:"));
    // No bulk data as Rust literals.
    for array in [": [Tile;", ": [u16;", ": [[u16;", ": [[u8; 64];", ": [u8;"] {
        assert!(!two.contains(array), "a {array} literal in data.rs");
    }
}

#[test]
fn the_data_blob_is_reproducible() {
    let a = codegen::data(&two_maps(|b| b[3] ^= 2), 7).unwrap();
    let b = codegen::data(&two_maps(|b| b[3] ^= 2), 7).unwrap();
    assert_eq!(a, b);
}

#[test]
fn map_digests_follow_the_map_and_its_neighbors_only() {
    let base = codegen::map_digests(&two_maps(|_| {})).unwrap();
    assert_eq!(base.len(), 2);
    assert_eq!(base[0].0, stub::MAP_NAME);
    // Export order does not matter.
    let mut w = two_maps(|_| {});
    w.maps.reverse();
    let mut rev = codegen::map_digests(&w).unwrap();
    rev.sort();
    let mut sorted = base.clone();
    sorted.sort();
    assert_eq!(rev, sorted);
    // Editing StubRoute changes its digest, not StubTown's (they are not connected).
    let edited = codegen::map_digests(&two_maps(|b| b[5] ^= 1)).unwrap();
    assert_eq!(edited[0], base[0]);
    assert_ne!(edited[1], base[1]);
    // Connected, StubTown's digest covers StubRoute's layout too.
    let connect = |w: &mut input::World| {
        w.maps[0].connections.push(extract::schema::Connection {
            dir: extract::schema::ConnectionDir::North,
            offset: 0,
            map_id: "0.16".into(),
            map: Some("StubRoute".into()),
        });
    };
    let mut a = two_maps(|_| {});
    connect(&mut a);
    let mut b = two_maps(|b| b[5] ^= 1);
    connect(&mut b);
    assert_ne!(
        codegen::map_digests(&a).unwrap()[0],
        codegen::map_digests(&b).unwrap()[0]
    );
}

#[test]
fn the_global_digest_follows_the_sources_and_tables() {
    let w = stub::world();
    let d = |w: &input::World, src| codegen::global_digest(w, src, "rs", b"bin").unwrap();
    assert_eq!(d(&w, 1), d(&w, 1));
    assert_ne!(d(&w, 1), d(&w, 2));
    let mut t = stub::world();
    t.tables
        .std_strings
        .strings
        .insert("STDSTRING_COOL".into(), "COOL".into());
    assert_ne!(d(&w, 1), d(&t, 1));
    // Everything compiled that is not one map's: sprites, font, window.
    let mut f = stub::world();
    f.widths[0] ^= 1;
    assert_ne!(d(&w, 1), d(&f, 1));
    let mut s = stub::world();
    s.sprites[0].frames[0][0][0] ^= 1;
    assert_ne!(d(&w, 1), d(&s, 1));
    let mut n = stub::world();
    n.window.text_fg ^= 1;
    assert_ne!(d(&w, 1), d(&n, 1));
}

/// The sources digest covers the generator and the content sources, never their tests.
#[test]
fn the_sources_digest_skips_tests() {
    let tmp = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m8e-sources");
    let _ = std::fs::remove_dir_all(&tmp);
    let put = |rel: &str, s: &str| {
        let p = tmp.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    };
    put("src/lib.rs", "a");
    put("build/main.rs", "b");
    put("build/support/commands.ron", "c");
    let base = codegen::sources_digest(&tmp).unwrap();
    put("src/data_tests.rs", "t");
    put("src/tests/x.rs", "t");
    assert_eq!(codegen::sources_digest(&tmp).unwrap(), base);
    put("build/codegen.rs", "g");
    let generator = codegen::sources_digest(&tmp).unwrap();
    assert_ne!(generator, base);
    put("build/support/commands.ron", "c2");
    let support = codegen::sources_digest(&tmp).unwrap();
    assert_ne!(support, generator);
    put("src/prelude.rs", "p");
    assert_ne!(codegen::sources_digest(&tmp).unwrap(), support);
}

#[test]
fn tables_are_generated() {
    let mut w = stub::world();
    w.tables.items.items = vec![
        Item {
            id: "ITEM_NONE".into(),
            name: "????????".into(),
            pocket: Some(ItemPocket::Items), // as the ROM has it
            price: 0,
            importance: 0,
        },
        Item {
            id: "ITEM_MASTER_BALL".into(),
            name: "MASTER BALL".into(),
            pocket: Some(ItemPocket::PokeBalls),
            price: 0,
            importance: 0,
        },
    ];
    w.tables
        .multichoice
        .lists
        .insert("MULTI_YESNO".into(), vec!["YES A".into(), "NO B".into()]);
    w.tables
        .std_strings
        .strings
        .insert("STDSTRING_COOL".into(), "COOL".into());
    w.tables
        .map_sections
        .sections
        .insert("MAPSEC_LITTLEROOT_TOWN".into(), "TOWN A".into());
    let (rs, _, _) = codegen::data(&w, 0).unwrap();
    assert!(rs.contains("pub static ITEMS: [Option<ItemInfo>; 2] = [None,Some(ItemInfo {"));
    assert!(rs.contains("pocket: Pocket::PokeBalls"));
    let multi = extract::names::MULTICHOICE.value("MULTI_YESNO").unwrap();
    // A static per list: `Some(&[..])` of non-const items would borrow a temporary.
    assert!(rs.contains(&format!("static MULTI_{multi}: [&[u8]; 2] = [DATA.bytes(")));
    assert!(rs.contains(&format!("        {multi} => Some(&MULTI_{multi}),")));
    let cool = extract::names::STD_STRINGS.value("STDSTRING_COOL").unwrap();
    assert!(rs.contains(&format!("        {cool} => Some(DATA.bytes(")));
    assert!(rs.contains("pub fn map_section(id: u8)"));
    // A wrong item number fails the build.
    w.tables.items.items.remove(0);
    assert!(codegen::data(&w, 0).is_err());
}

#[test]
fn emotes_are_sprites() {
    let mut w = stub::world();
    w.emotes.push(input::EmoteIn {
        name: "QuestionMark".into(),
        width: 16,
        height: 16,
        frames: vec![vec![[0x1111_1111; 8]; 4]],
        palette: [0; 16],
    });
    let (rs, _, _) = codegen::data(&w, 0).unwrap();
    assert!(rs.contains("static EMOTE_Question: SpriteData = SpriteData { width: 16, height: 16"));
    assert!(rs.contains("Emote::Question => Some(&EMOTE_Question),"));
    assert!(!rs.contains("Emote::Heart =>"));
}

fn slot(species: &str) -> WildMon {
    WildMon {
        species: species.into(),
        min_level: 2,
        max_level: 3,
    }
}

#[test]
fn every_wild_header_is_compiled() {
    let mut w = stub::world();
    let land = |s: &str| LandEncounters {
        rate: 10,
        mons: vec![slot(s); 12],
    };
    w.battle.encounters.insert(
        stub::MAP_NAME.into(),
        vec![
            EncounterHeader {
                land: Some(land("SPECIES_STUB_A")),
                water: Some(LandEncounters {
                    rate: 4,
                    mons: vec![slot("SPECIES_STUB_B"); 5],
                }),
                ..Default::default()
            },
            EncounterHeader {
                land: Some(land("SPECIES_STUB_B")),
                fishing: Some(LandEncounters {
                    rate: 30,
                    mons: vec![slot("SPECIES_STUB_A"); 10],
                }),
                ..Default::default()
            },
        ],
    );
    let out = battle::generate(&w.battle, &w).unwrap();
    let id = u16::from(stub::MAP_GROUP) << 8 | u16::from(stub::MAP_NUM);
    assert!(
        out.rs
            .contains(&format!("({id:#X}, 0) => Some(&ENC_{id:X}_0)"))
    );
    assert!(
        out.rs
            .contains(&format!("({id:#X}, 1) => Some(&ENC_{id:X}_1)"))
    );
    assert!(out.rs.contains(&format!("        {id:#X} => 2,")));
    assert!(
        out.rs
            .contains(&format!("        {id:#X} => Some(&ENC_{id:X}_0),"))
    );
    assert!(out.rs.contains(&format!("({id:#X}, 0) => Some((4, &[")));
    assert!(out.rs.contains(&format!("({id:#X}, 1) => Some((30, &[")));
    // A land table without 12 slots is an error.
    w.battle.encounters.get_mut(stub::MAP_NAME).unwrap()[1]
        .land
        .as_mut()
        .unwrap()
        .mons
        .pop();
    assert!(battle::generate(&w.battle, &w).is_err());
}

#[test]
fn evolutions_and_trainer_money() {
    let mut w = stub::world();
    w.battle.evolutions.species = BTreeMap::from([(
        "SPECIES_STUB_A".to_string(),
        vec![("EVO_LEVEL".to_string(), 16, "SPECIES_STUB_B".to_string())],
    )]);
    w.battle.trainer_money.classes = BTreeMap::from([("TRAINER_CLASS_0xFF".to_string(), 5)]);
    let out = battle::generate(&w.battle, &w).unwrap();
    assert!(out.rs.contains(
        "=> &[Evolution { method: EvoMethod::Level, param: 16, target: SpeciesId(2) },], // SPECIES_STUB_A"
    ));
    // The stub's one trainer (`TRAINER_NONE`) gets the default.
    assert!(out.rs.contains("pub static TRAINER_MONEY: [u8; 1] = [5];"));
    w.battle.trainer_money.classes.clear();
    let out = battle::generate(&w.battle, &w).unwrap();
    assert!(out.rs.contains("pub static TRAINER_MONEY: [u8; 0] = [];"));
}

#[test]
fn unimplemented_stat_effects_are_plain_unsupported() {
    let mut w = stub::world();
    let m = &mut w.battle.moves.moves[1];
    m.effect = "EFFECT_SWAGGER".into();
    let id = m.id.clone();
    let out = battle::generate(&w.battle, &w).unwrap();
    let n = extract::names::MOVE_EFFECTS
        .value("EFFECT_SWAGGER")
        .unwrap();
    assert!(
        out.rs
            .contains(&format!("effect: MoveEffect::Unsupported({n})"))
    );
    // Every stat change has words now (G1): Swagger is unsupported only as unimplemented.
    assert!(out.report.contains(&format!(
        "unsupported effect EFFECT_SWAGGER (MoveEffect::Unsupported): {id}"
    )));
    assert!(!out.report.contains("unworded"));
}

#[test]
fn species_and_trainers_without_pics_degrade() {
    let mut w = stub::world();
    w.battle.mon_gfx.remove("SPECIES_STUB_B");
    let out = battle::generate(&w.battle, &w).unwrap();
    assert!(
        out.report
            .contains("species without pics (gfx: None): SPECIES_STUB_B")
    );
    assert!(out.rs.contains("gfx: None }, // SPECIES_STUB_B"));
}
