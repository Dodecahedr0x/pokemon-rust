use extract::sprites::{ObjectGraphicsTable, locate_object_graphics};
use extract::{Rom, fixtures};
use std::collections::HashMap;
use std::path::Path;

const DIR: &str = "src/data/object_events";

/// Converts an indexed 4-bit PNG to GBA 4bpp, laid out as consecutive `fw`×`fh` frames
/// (left to right, top to bottom), 8×8 tiles row-major within each frame, low nibble = left pixel.
fn png_to_4bpp(path: &Path, fw: usize, fh: usize) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Four, "{}", path.display());
    let (w, h) = (info.width as usize, info.height as usize);
    let px = |x: usize, y: usize| {
        let b = buf[y * info.line_size + x / 2];
        if x.is_multiple_of(2) { b >> 4 } else { b & 0xF }
    };
    let mut out = Vec::with_capacity(w * h / 2);
    for fy in (0..h).step_by(fh) {
        for fx in (0..w).step_by(fw) {
            for ty in (0..fh).step_by(8) {
                for tx in (0..fw).step_by(8) {
                    for y in 0..8 {
                        for x in (0..8).step_by(2) {
                            let (ax, ay) = (fx + tx + x, fy + ty + y);
                            out.push(px(ax, ay) | (px(ax + 1, ay) << 4));
                        }
                    }
                }
            }
        }
    }
    out
}

/// JASC-PAL file to BGR555.
fn jasc_to_bgr555(path: &Path) -> Vec<u16> {
    let text = std::fs::read_to_string(path).unwrap();
    text.lines()
        .skip(3)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<u16> = l.split_whitespace().map(|v| v.parse().unwrap()).collect();
            (c[0] >> 3) | ((c[1] >> 3) << 5) | ((c[2] >> 3) << 10)
        })
        .collect()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(fixtures::decomp().join(rel)).unwrap()
}

/// Graphics-info names in id order, from `gObjectEventGraphicsInfoPointers`.
fn decomp_info_names() -> Vec<String> {
    let text = read(&format!("{DIR}/object_event_graphics_info_pointers.h"));
    let body = text
        .split("gObjectEventGraphicsInfoPointers[NUM_OBJ_EVENT_GFX] = {")
        .nth(1)
        .unwrap();
    let body = body.split("};").next().unwrap();
    body.lines()
        .filter_map(|l| l.split("&gObjectEventGraphicsInfo_").nth(1))
        .map(|n| n.trim_end_matches(',').trim().to_string())
        .collect()
}

/// Expected frame count per id, from the info's `.images` pic table.
fn decomp_frame_counts() -> Vec<usize> {
    let mut frames: HashMap<String, usize> = HashMap::new();
    for f in std::fs::read_dir(fixtures::decomp().join(DIR)).unwrap() {
        let text = std::fs::read_to_string(f.unwrap().path()).unwrap();
        let mut current = None;
        for l in text.lines() {
            if let Some(rest) = l.split("struct SpriteFrameImage ").nth(1) {
                current = Some(rest.split('[').next().unwrap().to_string());
                frames.insert(current.clone().unwrap(), 0);
            } else if l.starts_with("};") {
                current = None;
            } else if let Some(name) = &current
                && (l.contains("overworld_frame(") || l.contains("obj_frame_tiles("))
            {
                *frames.get_mut(name).unwrap() += 1;
            }
        }
    }
    let infos = read(&format!("{DIR}/object_event_graphics_info.h"));
    let mut images = HashMap::new();
    let mut current = String::new();
    for l in infos.lines() {
        if let Some(rest) = l.split("gObjectEventGraphicsInfo_").nth(1) {
            current = rest.split(' ').next().unwrap().to_string();
        } else if let Some(rest) = l.split(".images = ").nth(1) {
            images.insert(current.clone(), rest.trim_end_matches(',').to_string());
        }
    }
    decomp_info_names()
        .iter()
        .map(|n| frames[&images[n]])
        .collect()
}

fn roms() -> [(&'static str, Rom); 2] {
    [("US", fixtures::us()), ("FR", fixtures::fr())]
}

fn check_brendan(rom: &Rom, table: &ObjectGraphicsTable, name: &str) {
    let d = fixtures::decomp();
    let pics = d.join("graphics/object_events/pics/people/brendan");
    let mut expected = png_to_4bpp(&pics.join("walking.png"), 16, 32);
    expected.extend(png_to_4bpp(&pics.join("running.png"), 16, 32));
    let g = table.get(rom, 0).unwrap();
    assert_eq!((g.id, g.width, g.height), (0, 16, 32), "{name}");
    assert_eq!(g.palette_tag, 0x1100, "{name}");
    assert_eq!(g.frames.len(), decomp_frame_counts()[0], "{name}");
    assert!(g.frames.iter().all(|f| f.len() == 16 * 32 / 2), "{name}");
    assert_eq!(
        g.frames.concat(),
        expected,
        "{name}: frames differ from walking+running.png"
    );
    let pal = jasc_to_bgr555(&d.join("graphics/object_events/palettes/brendan.pal"));
    assert_eq!(g.palette.to_vec(), pal, "{name}: palette");
}

#[test]
fn walking_png_anchor_is_unique_and_in_brendan_frames() {
    let walking = png_to_4bpp(
        &fixtures::decomp().join("graphics/object_events/pics/people/brendan/walking.png"),
        16,
        32,
    );
    for (name, rom) in roms() {
        let hits = rom.find_all(&walking[..128]);
        assert_eq!(hits.len(), 1, "{name}: {hits:x?}");
        let table = locate_object_graphics(&rom).unwrap();
        let first = table.get(&rom, 0).unwrap().offset;
        assert_eq!(
            rom.ptr(first + 0x1C).unwrap().map(|p| rom.ptr(p).unwrap()),
            Some(Some(hits[0]))
        );
    }
}

#[test]
fn object_graphics_table_matches_decomp() {
    let counts = decomp_frame_counts();
    let names = decomp_info_names();
    assert_eq!(counts.len(), 239);
    for (name, rom) in roms() {
        let table = locate_object_graphics(&rom).unwrap();
        eprintln!(
            "{name}: table {:#x}, count {}, palettes {:#x}",
            table.offset, table.count, table.palettes_offset
        );
        assert_eq!(table.count as usize, counts.len(), "{name}");
        check_brendan(&rom, &table, name);
        for id in 0..table.count {
            let g = table
                .get(&rom, id as u8)
                .unwrap_or_else(|e| panic!("{name}: id {id}: {e:#}"));
            assert_eq!(
                g.frames.len(),
                counts[id as usize],
                "{name}: id {id} frames"
            );
            let size = g.width as usize * g.height as usize / 2;
            if names[id as usize].contains("BerryTree") {
                // Dirt pile and sprouts are 16×16, the grown tree frames 16×32.
                assert!(g.frames.iter().all(|f| [128, 256].contains(&f.len())));
            } else {
                assert!(g.frames.iter().all(|f| f.len() == size), "{name}: id {id}");
            }
        }
        assert!(table.get(&rom, table.count as u8).is_err(), "{name}");
    }
}

#[test]
fn berry_tree_sprites_export_with_mixed_frame_sizes() {
    let text = read("include/constants/event_objects.h");
    let id = |name: &str| -> u8 {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("#define {name} ")))
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    };
    let dirt = png_to_4bpp(
        &fixtures::decomp().join("graphics/object_events/pics/berry_trees/dirt_pile.png"),
        16,
        16,
    );
    let dirt = extract::gfx::tiles_4bpp_to_image(&dirt, 2).unwrap();
    for (name, rom) in roms() {
        let table = locate_object_graphics(&rom).unwrap();
        for gfx in [
            "OBJ_EVENT_GFX_BERRY_TREE",
            "OBJ_EVENT_GFX_BERRY_TREE_EARLY_STAGES",
            "OBJ_EVENT_GFX_BERRY_TREE_LATE_STAGES",
        ] {
            let g = table.get(&rom, id(gfx)).unwrap();
            let (img, file) =
                extract::export::sprite(&g).unwrap_or_else(|e| panic!("{name}: {gfx}: {e:#}"));
            assert_eq!(file.frame_sizes.len(), g.frames.len(), "{name}: {gfx}");
            assert!(file.frame_sizes.contains(&(16, 32)), "{name}: {gfx}");
            assert_eq!(
                (img.width, img.height),
                (16 * g.frames.len() as u32, 32),
                "{name}: {gfx}"
            );
            // Frame 0 is the 16x16 dirt pile at the top of its cell, blank below.
            assert_eq!(file.frame_sizes[0], (16, 16), "{name}: {gfx}");
            for y in 0..32 {
                let row = &img.pixels[(y * img.width) as usize..][..16];
                if y < 16 {
                    assert_eq!(
                        row,
                        &dirt.pixels[(y * 16) as usize..][..16],
                        "{name}: {gfx}"
                    );
                } else {
                    assert!(row.iter().all(|&p| p == 0), "{name}: {gfx}: row {y}");
                }
            }
        }
    }
}

#[test]
fn may_normal_uses_may_palette() {
    let text = read("include/constants/event_objects.h");
    let may: u8 = text
        .lines()
        .find_map(|l| l.strip_prefix("#define OBJ_EVENT_GFX_MAY_NORMAL "))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let pal = jasc_to_bgr555(&fixtures::decomp().join("graphics/object_events/palettes/may.pal"));
    for (name, rom) in roms() {
        let table = locate_object_graphics(&rom).unwrap();
        let g = table.get(&rom, may).unwrap();
        assert_eq!(
            (g.width, g.height, g.palette_tag),
            (16, 32, 0x1110),
            "{name}"
        );
        assert_eq!(g.palette.to_vec(), pal, "{name}");
    }
}

#[test]
fn emote_pics_match_decomp() {
    let d = fixtures::decomp();
    let npc1 = jasc_to_bgr555(&d.join("graphics/object_events/palettes/npc_1.pal"));
    let files = ["emotion_exclamation", "emotion_question", "emotion_heart"];
    let mut us_tiles = Vec::new();
    for (name, rom) in roms() {
        let e = extract::sprites::locate_emotes(&rom).unwrap();
        let names: Vec<&str> = e.pics.iter().map(|p| p.name).collect();
        assert_eq!(names, extract::sprites::EMOTES, "{name}");
        for (p, file) in e.pics.iter().zip(files) {
            assert_eq!((p.width, p.height), (16, 16), "{name} {file}");
            if name == "US" {
                let png = d.join(format!("graphics/field_effects/pics/{file}.png"));
                assert_eq!(p.tiles, png_to_4bpp(&png, 16, 16), "{file}");
            }
        }
        let table = locate_object_graphics(&rom).unwrap();
        assert_eq!(e.palette(&rom, &table).unwrap().to_vec(), npc1, "{name}");
        let tiles: Vec<Vec<u8>> = e.pics.iter().map(|p| p.tiles.clone()).collect();
        if us_tiles.is_empty() {
            us_tiles = tiles;
        } else {
            assert_eq!(tiles, us_tiles, "{name}: same pics as US");
        }
    }
}
