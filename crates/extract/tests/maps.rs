use extract::maps::{self, Direction, LITTLEROOT, MapId, MapTable, OLDALE, ROUTE101, TilesetRef};
use extract::{Rom, fixtures};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

fn json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn u16s(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// `MAP_*` constant -> id, resolved through `map_groups.json` and each `map.json`.
fn map_ids(d: &Path) -> HashMap<String, MapId> {
    let groups = json(&d.join("data/maps/map_groups.json"));
    let mut ids = HashMap::new();
    ids.insert(
        "MAP_DYNAMIC".to_string(),
        MapId {
            group: 0x7F,
            num: 0x7F,
        },
    );
    for (g, group_name) in groups["group_order"].as_array().unwrap().iter().enumerate() {
        let names = groups[group_name.as_str().unwrap()].as_array().unwrap();
        for (n, name) in names.iter().enumerate() {
            let map = json(&d.join(format!("data/maps/{}/map.json", name.as_str().unwrap())));
            let id = MapId {
                group: g as u8,
                num: n as u8,
            };
            ids.insert(map["id"].as_str().unwrap().to_string(), id);
        }
    }
    ids
}

/// `#define NAME <integer>` lines from `include/constants/<file>`.
fn constants(d: &Path, files: &[&str]) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    for file in files {
        let path = d.join("include/constants").join(file);
        for line in std::fs::read_to_string(&path).unwrap().lines() {
            let mut words = line.split_whitespace();
            if words.next() != Some("#define") {
                continue;
            }
            let (Some(name), Some(value)) = (words.next(), words.next()) else {
                continue;
            };
            let parsed = match value.strip_prefix("0x") {
                Some(hex) => i64::from_str_radix(hex, 16).ok(),
                None => value.parse().ok(),
            };
            if let Some(v) = parsed {
                out.insert(name.to_string(), v);
            }
        }
    }
    out
}

fn both() -> [(&'static str, Rom); 2] {
    [("US", fixtures::us()), ("FR", fixtures::fr())]
}

#[test]
fn map_table_located_at_ground_truth() {
    for (name, rom, header) in [
        ("US", fixtures::us(), 0x4825b4),
        ("FR", fixtures::fr(), 0x4874a0),
    ] {
        let table = maps::locate_map_table(&rom).unwrap();
        assert_eq!(table.groups.len(), 34, "{name}");
        assert_eq!(table.groups[0][9], header, "{name}");
        let h = table.header(&rom, LITTLEROOT).unwrap();
        assert_eq!(h.offset, header, "{name}");
        assert_eq!((h.music, h.layout_id), (405, 10), "{name}");
        eprintln!("{name}: gMapGroups at {:#x}", table.offset);
    }
}

#[test]
fn three_maps_match_decomp() {
    let d = fixtures::decomp();
    let ids = map_ids(&d);
    let layouts = json(&d.join("data/layouts/layouts.json"));
    let consts = constants(&d, &["songs.h", "weather.h", "map_types.h"]);
    let xy = |j: &Value| (j["x"].as_i64().unwrap(), j["y"].as_i64().unwrap());
    for (rom_name, rom) in both() {
        let table = maps::locate_map_table(&rom).unwrap();
        for (folder, id) in [
            ("LittlerootTown", LITTLEROOT),
            ("OldaleTown", OLDALE),
            ("Route101", ROUTE101),
        ] {
            let ctx = format!("{rom_name} {folder}");
            let map = json(&d.join(format!("data/maps/{folder}/map.json")));
            assert_eq!(ids[map["id"].as_str().unwrap()], id, "{ctx}");
            let h = table.header(&rom, id).unwrap();

            let layout = layouts["layouts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|l| l["id"] == map["layout"])
                .unwrap();
            assert_eq!(
                h.layout.width as u64,
                layout["width"].as_u64().unwrap(),
                "{ctx}"
            );
            assert_eq!(
                h.layout.height as u64,
                layout["height"].as_u64().unwrap(),
                "{ctx}"
            );
            let blocks = std::fs::read(d.join(layout["blockdata_filepath"].as_str().unwrap()));
            assert_eq!(h.layout.blocks, u16s(&blocks.unwrap()), "{ctx} blocks");
            let border = std::fs::read(d.join(layout["border_filepath"].as_str().unwrap()));
            assert_eq!(h.layout.border, u16s(&border.unwrap()), "{ctx} border");

            let objects = map["object_events"].as_array().unwrap();
            assert_eq!(h.events.objects.len(), objects.len(), "{ctx}");
            for (o, j) in h.events.objects.iter().zip(objects) {
                assert_eq!(o.x as i64, j["x"].as_i64().unwrap(), "{ctx}");
                assert_eq!(o.y as i64, j["y"].as_i64().unwrap(), "{ctx}");
            }
            let warps = map["warp_events"].as_array().unwrap();
            assert_eq!(h.events.warps.len(), warps.len(), "{ctx}");
            for (w, j) in h.events.warps.iter().zip(warps) {
                assert_eq!(w.dest, ids[j["dest_map"].as_str().unwrap()], "{ctx}");
                assert_eq!(w.x as i64, j["x"].as_i64().unwrap(), "{ctx}");
                assert_eq!(w.y as i64, j["y"].as_i64().unwrap(), "{ctx}");
                assert_eq!(
                    w.elevation as i64,
                    j["elevation"].as_i64().unwrap(),
                    "{ctx}"
                );
                let dest_warp: u8 = j["dest_warp_id"].as_str().unwrap().parse().unwrap();
                assert_eq!(w.warp_id, dest_warp, "{ctx}");
            }
            let list = |k: &str| map[k].as_array().cloned().unwrap_or_default();
            let coords = list("coord_events");
            assert_eq!(h.events.coords.len(), coords.len(), "{ctx}");
            for (c, j) in h.events.coords.iter().zip(&coords) {
                assert_eq!((c.x as i64, c.y as i64), xy(j), "{ctx} coord");
            }
            let bgs = list("bg_events");
            assert_eq!(h.events.bgs.len(), bgs.len(), "{ctx}");
            for (b, j) in h.events.bgs.iter().zip(&bgs) {
                assert_eq!((b.x as i64, b.y as i64), xy(j), "{ctx} bg");
            }

            let flag = |k: &str| map[k].as_bool().unwrap();
            assert_eq!(h.allow_cycling, flag("allow_cycling"), "{ctx}");
            assert_eq!(h.allow_escaping, flag("allow_escaping"), "{ctx}");
            assert_eq!(h.allow_running, flag("allow_running"), "{ctx}");
            assert_eq!(h.show_map_name, flag("show_map_name"), "{ctx}");
            let constant = |k: &str| consts[map[k].as_str().unwrap()];
            assert_eq!(h.music as i64, constant("music"), "{ctx} music");
            assert_eq!(h.weather as i64, constant("weather"), "{ctx} weather");
            assert_eq!(h.map_type as i64, constant("map_type"), "{ctx} map_type");

            let conns: Vec<(Direction, i32, MapId)> = map["connections"]
                .as_array()
                .map(|a| a.as_slice())
                .unwrap_or_default()
                .iter()
                .map(|c| {
                    let dir = match c["direction"].as_str().unwrap() {
                        "down" => Direction::South,
                        "up" => Direction::North,
                        "left" => Direction::West,
                        "right" => Direction::East,
                        "dive" => Direction::Dive,
                        "emerge" => Direction::Emerge,
                        other => panic!("{other}"),
                    };
                    let offset = c["offset"].as_i64().unwrap() as i32;
                    (dir, offset, ids[c["map"].as_str().unwrap()])
                })
                .collect();
            let got: Vec<(Direction, i32, MapId)> = h
                .connections
                .iter()
                .map(|c| (c.direction, c.offset, c.map))
                .collect();
            assert_eq!(got, conns, "{ctx} connections");
        }
    }
}

#[test]
fn every_map_in_table_parses() {
    for (rom_name, rom) in both() {
        let table = maps::locate_map_table(&rom).unwrap();
        let mut count = 0;
        for (g, group) in table.groups.iter().enumerate() {
            for n in 0..group.len() {
                let id = MapId {
                    group: g as u8,
                    num: n as u8,
                };
                table
                    .header(&rom, id)
                    .unwrap_or_else(|e| panic!("{rom_name} {id:?}: {e:#}"));
                count += 1;
            }
        }
        assert_eq!(count, 518, "{rom_name}");
    }
}

#[test]
fn tileset_metatile_counts_match_decomp() {
    let d = fixtures::decomp();
    let mut folders = Vec::new();
    for kind in ["primary", "secondary"] {
        for entry in std::fs::read_dir(d.join("data/tilesets").join(kind)).unwrap() {
            let dir = entry.unwrap().path();
            if let Ok(bytes) = std::fs::read(dir.join("metatiles.bin")) {
                folders.push((kind == "secondary", dir, bytes));
            }
        }
    }

    // US: match each tileset to a decomp folder by its exact metatile bytes.
    let us = fixtures::us();
    let us_sets = reachable_tilesets(&us, &maps::locate_map_table(&us).unwrap());
    assert!(us_sets.len() > 60, "US: {} tilesets", us_sets.len());
    let mut folder_of = BTreeMap::new();
    for (key, ts) in &us_sets {
        let ctx = format!("US tileset {:#x}", ts.offset);
        let matches: Vec<_> = folders
            .iter()
            .filter(|(secondary, _, bytes)| {
                *secondary == ts.secondary
                    && us.slice(ts.metatiles, bytes.len()).ok() == Some(bytes.as_slice())
            })
            .collect();
        let hit = matches
            .iter()
            .find(|(_, _, b)| b.len() == ts.metatile_count as usize * 16)
            .unwrap_or_else(|| {
                let sizes: Vec<_> = matches
                    .iter()
                    .map(|(_, p, b)| (p.file_name().unwrap().to_owned(), b.len() / 16))
                    .collect();
                panic!("{ctx}: count {} vs decomp {sizes:?}", ts.metatile_count)
            });
        folder_of.insert(*key, *hit);
    }

    // FR tilesets are paired with the US ones through the same (map, slot). Metatiles and
    // attributes are byte-identical, except 5 localized metatiles in `shop`.
    let fr = fixtures::fr();
    let fr_sets = reachable_tilesets(&fr, &maps::locate_map_table(&fr).unwrap());
    assert_eq!(
        fr_sets.keys().collect::<Vec<_>>(),
        us_sets.keys().collect::<Vec<_>>()
    );
    let mut shop_seen = false;
    for (key, ts) in &fr_sets {
        let (_, dir, bytes) = folder_of[key];
        let folder = dir.file_name().unwrap().to_str().unwrap();
        let ctx = format!("FR tileset {:#x} ({folder})", ts.offset);
        let us_ts = &us_sets[key];
        assert_eq!(ts.metatile_count, us_ts.metatile_count, "{ctx}");
        assert_eq!(ts.metatile_count as usize * 16, bytes.len(), "{ctx}");
        let attrs = |rom: &Rom, t: &TilesetRef| {
            rom.slice(t.metatile_attributes, t.metatile_count as usize * 2)
                .unwrap()
                .to_vec()
        };
        assert_eq!(attrs(&fr, ts), attrs(&us, us_ts), "{ctx} attributes");
        let fr_meta = fr.slice(ts.metatiles, bytes.len()).unwrap();
        let us_meta = us.slice(us_ts.metatiles, bytes.len()).unwrap();
        let differing: Vec<usize> = fr_meta
            .chunks(16)
            .zip(us_meta.chunks(16))
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        if folder == "shop" {
            shop_seen = true;
            assert_eq!(differing, [159, 167, 175, 183, 191], "{ctx}");
        } else {
            assert_eq!(differing, Vec::<usize>::new(), "{ctx}");
        }
    }
    assert!(shop_seen);
}

/// Every tileset reachable from the map table, keyed by the first (map, slot) that uses it.
fn reachable_tilesets(rom: &Rom, table: &MapTable) -> BTreeMap<(MapId, u8), TilesetRef> {
    let mut seen = std::collections::HashSet::new();
    let mut out = BTreeMap::new();
    for (g, group) in table.groups.iter().enumerate() {
        for n in 0..group.len() {
            let id = MapId {
                group: g as u8,
                num: n as u8,
            };
            let layout = table.header(rom, id).unwrap().layout;
            for (slot, off) in [(0, layout.primary_tileset), (1, layout.secondary_tileset)] {
                if seen.insert(off) {
                    let ts = maps::read_tileset(rom, off, table).unwrap();
                    assert_eq!(ts.secondary, slot == 1, "tileset {off:#x}");
                    rom.slice(ts.metatile_attributes, ts.metatile_count as usize * 2)
                        .unwrap();
                    out.insert((id, slot), ts);
                }
            }
        }
    }
    out
}

#[test]
fn table_header_rejects_out_of_range_ids() {
    let rom = fixtures::us();
    let table: MapTable = maps::locate_map_table(&rom).unwrap();
    assert!(table.header(&rom, MapId { group: 34, num: 0 }).is_err());
    assert!(table.header(&rom, MapId { group: 0, num: 57 }).is_err());
}
