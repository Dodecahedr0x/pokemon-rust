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
            }
            let n = |k: &str| map[k].as_array().map_or(0, |a| a.len());
            assert_eq!(h.events.coords.len(), n("coord_events"), "{ctx}");
            assert_eq!(h.events.bgs.len(), n("bg_events"), "{ctx}");

            let conns: Vec<(Direction, MapId)> = map["connections"]
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
                    (dir, ids[c["map"].as_str().unwrap()])
                })
                .collect();
            let got: Vec<(Direction, MapId)> =
                h.connections.iter().map(|c| (c.direction, c.map)).collect();
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

    // FR localizes a few metatiles (e.g. 5 shop signs), so its tilesets are paired with the US
    // ones through the same (map, slot); the bytes must still mostly agree with that folder.
    let fr = fixtures::fr();
    let fr_sets = reachable_tilesets(&fr, &maps::locate_map_table(&fr).unwrap());
    assert_eq!(
        fr_sets.keys().collect::<Vec<_>>(),
        us_sets.keys().collect::<Vec<_>>()
    );
    for (key, ts) in &fr_sets {
        let (_, dir, bytes) = folder_of[key];
        let ctx = format!("FR tileset {:#x} ({})", ts.offset, dir.display());
        assert_eq!(ts.metatile_count as usize * 16, bytes.len(), "{ctx}");
        let rom_bytes = fr.slice(ts.metatiles, bytes.len()).unwrap();
        let differing = rom_bytes
            .chunks(16)
            .zip(bytes.chunks(16))
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differing * 20 <= ts.metatile_count as usize,
            "{ctx}: {differing} metatiles differ"
        );
    }
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
