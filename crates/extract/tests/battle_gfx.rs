//! Battle graphics, the small font and the starter screen (Milestone 4 Task B), checked against
//! the decomp on the US ROM and located on the French ROM.

use extract::battle_gfx::{self, ENVIRONMENTS, Gfx, Located};
use extract::export::{self, Out};
use extract::schema::{FontFile, GfxFile, Hex, MonCoordsFile, TrainerPicsFile, from_rows};
use extract::{Rom, fixtures, font, gfx, names};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn us() -> &'static Rom {
    static R: OnceLock<Rom> = OnceLock::new();
    R.get_or_init(fixtures::us)
}

fn fr() -> &'static Rom {
    static R: OnceLock<Rom> = OnceLock::new();
    R.get_or_init(fixtures::fr)
}

fn us_located() -> &'static Located {
    static L: OnceLock<Located> = OnceLock::new();
    L.get_or_init(|| battle_gfx::locate(us()).unwrap())
}

fn fr_located() -> &'static Located {
    static L: OnceLock<Located> = OnceLock::new();
    L.get_or_init(|| battle_gfx::locate(fr()).unwrap())
}

fn decomp(rel: &str) -> PathBuf {
    fixtures::decomp().join(rel)
}

/// A decomp PNG converted to 4bpp as gbagfx does (8x8 tiles, row-major; an 8-bit PNG keeps the
/// low nibble of each index, as `ConvertToTiles4Bpp`).
fn png_4bpp(rel: &str) -> Vec<u8> {
    let mut img = gfx::read_png_indexed(&decomp(rel)).unwrap();
    img.pixels.iter_mut().for_each(|p| *p &= 0xF);
    gfx::image_to_tiles_4bpp(&img).unwrap()
}

/// A decomp PNG converted with gbagfx `-mwidth mw -mheight mh` (spritesheet_rules.mk): the image
/// is cut into metatiles of `mw` x `mh` tiles, taken row-major, each one's tiles row-major.
fn png_4bpp_meta(rel: &str, mw: usize, mh: usize) -> Vec<u8> {
    let rows = png_4bpp(rel);
    let img = gfx::read_png_indexed(&decomp(rel)).unwrap();
    let (tw, th) = (img.width as usize / 8, img.height as usize / 8);
    let mut out = Vec::new();
    for my in 0..th / mh {
        for mx in 0..tw / mw {
            for y in 0..mh {
                for x in 0..mw {
                    let t = (my * mh + y) * tw + mx * mw + x;
                    out.extend_from_slice(&rows[t * 32..t * 32 + 32]);
                }
            }
        }
    }
    out
}

/// Every colour of a JASC-PAL file, as BGR555 (8-bit channels divided by 8, as gbagfx).
fn jasc(rel: &str) -> Vec<u16> {
    let text = std::fs::read_to_string(decomp(rel)).unwrap();
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    assert_eq!(lines.next(), Some("JASC-PAL"));
    lines.next();
    let n: usize = lines.next().unwrap().parse().unwrap();
    lines
        .take(n)
        .map(|l| {
            let c: Vec<u16> = l.split_whitespace().map(|v| v.parse().unwrap()).collect();
            (c[0] / 8) | ((c[1] / 8) << 5) | ((c[2] / 8) << 10)
        })
        .collect()
}

/// The PLTE of a decomp PNG as BGR555.
fn png_palette(rel: &str) -> Vec<u16> {
    let reader = png::Decoder::new(std::fs::File::open(decomp(rel)).unwrap())
        .read_info()
        .unwrap();
    let plte = reader.info().palette.as_ref().unwrap().to_vec();
    plte.chunks(3)
        .map(|c| (c[0] / 8) as u16 | ((c[1] / 8) as u16) << 5 | ((c[2] / 8) as u16) << 10)
        .collect()
}

/// `got == want`, reporting the lengths and the first differing byte instead of both dumps.
#[track_caller]
fn same(got: &[u8], want: &[u8], what: &str) {
    let first = got.iter().zip(want).position(|(a, b)| a != b);
    assert!(
        got == want,
        "{what}: {} bytes vs {} expected, first difference at {first:?}",
        got.len(),
        want.len()
    );
}

fn flat(p: &[[u16; 16]]) -> Vec<u16> {
    p.iter().flatten().copied().collect()
}

fn bin_u16(rel: &str) -> Vec<u16> {
    std::fs::read(decomp(rel))
        .unwrap()
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn species(name: &str) -> u16 {
    names::SPECIES.value(name).unwrap() as u16
}

fn by_name<'a>(all: &'a [(String, Gfx)], name: &str) -> &'a Gfx {
    &all.iter().find(|(n, _)| n == name).unwrap().1
}

#[test]
fn us_tables_are_where_the_decomp_puts_them() {
    let l = us_located();
    let m = &l.mons;
    assert_eq!(m.count, 440);
    assert_eq!(
        (m.front, m.still_front, m.back),
        (0x30a18c, 0x301418, 0x3028b8)
    );
    assert_eq!((m.palettes, m.shiny), (0x303678, 0x304438));
    assert_eq!((m.front_coords, m.back_coords), (0x300d38, 0x3021d8));
    assert_eq!(m.elevation, 0x305dcc);
    let t = &l.trainers;
    assert_eq!((t.front_count, t.back_count), (93, 8));
    assert_eq!(
        (t.front, t.front_palettes, t.front_coords),
        (0x305654, 0x30593c, 0x3054e0)
    );
    assert_eq!(t.back_palettes, 0x305d8c);
    assert_eq!(l.environments, 0x31aba8);
    assert_eq!(l.textbox, [0xc00000, 0xc004e0, 0xc00524]);
    assert_eq!(l.text_palettes, [0xd85600, 0xd85620]);
    assert_eq!(l.healthbox_sheets, 0x32c0d0);
    assert_eq!(
        (l.healthbox_palette, l.healthbar_palette),
        (0xc11b9c, 0xc11bbc)
    );
    assert_eq!(l.healthbox_elements, 0xc11bdc);
    assert_eq!((l.ball_sheets, l.ball_palettes), (0x32c400, 0x32c460));
    assert_eq!(l.starter_sheets, 0x5b1ed8);
}

#[test]
fn us_treecko_and_mudkip_pics_match_decomp() {
    let l = us_located();
    for (name, dir) in [("SPECIES_TREECKO", "treecko"), ("SPECIES_MUDKIP", "mudkip")] {
        let p = l.mons.get(us(), species(name)).unwrap();
        let anim = png_4bpp(&format!("graphics/pokemon/{dir}/anim_front.png"));
        assert_eq!(anim.len(), 0x1000, "{name}: two 64x64 frames");
        assert_eq!(p.front, anim[..0x800], "{name}: front frame 0");
        assert_eq!(
            p.back,
            png_4bpp(&format!("graphics/pokemon/{dir}/back.png")),
            "{name}"
        );
        assert_eq!(
            p.palette.to_vec(),
            jasc(&format!("graphics/pokemon/{dir}/normal.pal"))
        );
        assert_eq!(
            p.shiny.to_vec(),
            jasc(&format!("graphics/pokemon/{dir}/shiny.pal"))
        );
    }
    // front_pic_coordinates.h / back_pic_coordinates.h: Treecko y_offset 8 / 6.
    let t = l.mons.get(us(), species("SPECIES_TREECKO")).unwrap();
    assert_eq!(
        (t.coords.front_y, t.coords.back_y, t.coords.elevation),
        (8, 6, 0)
    );
    // enemy_mon_elevation.h: Butterfree 8, Pidgey 16, Beautifly 8.
    for (name, e) in [
        ("SPECIES_BUTTERFREE", 8),
        ("SPECIES_PIDGEY", 16),
        ("SPECIES_BEAUTIFLY", 8),
    ] {
        assert_eq!(
            l.mons.get(us(), species(name)).unwrap().coords.elevation,
            e,
            "{name}"
        );
    }
}

#[test]
fn us_trainer_pics_match_decomp() {
    let l = us_located();
    let may = names::TRAINER_PICS.value("TRAINER_PIC_MAY").unwrap() as u8;
    let p = l.trainers.front(us(), may).unwrap();
    same(
        &p.tiles,
        &png_4bpp("graphics/trainers/front_pics/may.png"),
        "p",
    );
    assert_eq!(p.frames, 1);
    // front_pic_tables.h: May {.size = 8, .y_offset = 1}.
    assert_eq!((p.size, p.y_offset), (8, 1));
    assert_eq!(
        p.palette.to_vec(),
        jasc("graphics/trainers/palettes/may.pal")
    );
    let b = l.trainers.back(us(), 0).unwrap();
    assert_eq!(b.frames, 4);
    same(
        &b.tiles,
        &png_4bpp("graphics/trainers/back_pics/brendan.png"),
        "b",
    );
    assert_eq!(
        b.palette.to_vec(),
        jasc("graphics/trainers/palettes/brendan.pal")
    );
    // back_pic_tables.h: Brendan {.size = 8, .y_offset = 4}.
    assert_eq!((b.size, b.y_offset), (8, 4));
}

#[test]
fn us_interface_matches_decomp() {
    let all = battle_gfx::graphics(us(), us_located()).unwrap();
    let bi = "graphics/battle_interface";
    let tb = by_name(&all, "battle/gfx/textbox");
    same(&tb.tiles, &png_4bpp(&format!("{bi}/textbox.png")), "tb");
    let mut pal = jasc(&format!("{bi}/textbox_0.pal"));
    pal.extend(jasc(&format!("{bi}/textbox_1.pal")));
    // gBattleWindowTextPalette and gPPTextPalette, appended as palettes 2 and 3.
    pal.extend(jasc(&format!("{bi}/text.pal")));
    pal.extend(jasc(&format!("{bi}/text_pp.pal")));
    assert_eq!(flat(&tb.palettes), pal);
    assert_eq!(tb.tilemap, bin_u16(&format!("{bi}/textbox_map.bin")));
    assert_eq!(tb.palette_bank, 0);

    let hb = by_name(&all, "battle/gfx/healthbox_player");
    let hb_png = png_4bpp_meta(&format!("{bi}/healthbox_singles_player.png"), 8, 8);
    same(&hb.tiles, &hb_png, "healthbox_player");
    let box_pal = png_palette(&format!("{bi}/ball_status_bar.png"));
    let bar_pal = png_palette(&format!("{bi}/ball_display.png"));
    assert_eq!(flat(&hb.palettes), box_pal[..16]);
    let ho = by_name(&all, "battle/gfx/healthbox_opponent");
    let ho_png = png_4bpp_meta(&format!("{bi}/healthbox_singles_opponent.png"), 8, 4);
    same(&ho.tiles, &ho_png, "healthbox_opponent");
    for name in ["healthbox_opponent", "expbar", "status", "misc"] {
        let g = by_name(&all, &format!("battle/gfx/{name}"));
        assert_eq!(flat(&g.palettes), box_pal[..16], "{name}");
    }

    let cat = |files: &[&str]| -> Vec<u8> {
        files
            .iter()
            .flat_map(|f| png_4bpp(&format!("{bi}/{f}.png")))
            .collect()
    };
    let hp = by_name(&all, "battle/gfx/hpbar");
    same(&hp.tiles, &cat(&["hpbar", "hpbar_anim"]), "hp");
    assert_eq!(flat(&hp.palettes), bar_pal[..16]);
    same(
        &by_name(&all, "battle/gfx/expbar").tiles,
        &cat(&["expbar"]),
        "battle/gfx/expbar",
    );
    same(
        &by_name(&all, "battle/gfx/status").tiles,
        &cat(&["status", "status2"]),
        "battle/gfx/status",
    );
    same(
        &by_name(&all, "battle/gfx/misc").tiles,
        &cat(&["misc", "misc_frameend"]),
        "battle/gfx/misc",
    );

    let ball = by_name(&all, "battle/gfx/ball");
    same(&ball.tiles, &png_4bpp("graphics/balls/poke.png"), "ball");
    assert_eq!(
        flat(&ball.palettes),
        png_palette("graphics/balls/poke.png")[..16]
    );
}

#[test]
fn us_environments_match_decomp() {
    let all = battle_gfx::graphics(us(), us_located()).unwrap();
    assert_eq!(ENVIRONMENTS.len(), 10);
    let grass = by_name(&all, "battle/env/Grass");
    let dir = "graphics/battle_environment";
    same(
        &grass.tiles,
        &png_4bpp(&format!("{dir}/tall_grass/tiles.png")),
        "grass",
    );
    assert_eq!(grass.tilemap, bin_u16(&format!("{dir}/tall_grass/map.bin")));
    assert_eq!(
        flat(&grass.palettes),
        jasc(&format!("{dir}/tall_grass/palette.pal"))
    );
    assert_eq!(grass.palette_bank, 2);
    // sBattleEnvironmentTable: Plain uses the Building tiles and map with its own palette.
    let plain = by_name(&all, "battle/env/Plain");
    same(
        &plain.tiles,
        &png_4bpp(&format!("{dir}/building/tiles.png")),
        "plain",
    );
    assert_eq!(plain.tilemap, bin_u16(&format!("{dir}/building/map.bin")));
    assert_eq!(
        flat(&plain.palettes),
        jasc(&format!("{dir}/plain/palette.pal"))
    );
}

#[test]
fn us_starter_screen_matches_decomp() {
    let all = battle_gfx::graphics(us(), us_located()).unwrap();
    let dir = "graphics/starter_choose";
    let bag = by_name(&all, "starter/bag");
    same(&bag.tiles, &png_4bpp(&format!("{dir}/tiles.png")), "bag");
    assert_eq!(
        flat(&bag.palettes),
        png_palette(&format!("{dir}/tiles.png"))
    );
    assert_eq!(bag.tilemap, bin_u16(&format!("{dir}/birch_bag.bin")));
    let grass = by_name(&all, "starter/grass");
    assert!(grass.tiles.is_empty());
    assert_eq!(grass.tilemap, bin_u16(&format!("{dir}/birch_grass.bin")));
    let ball = by_name(&all, "starter/ball");
    same(
        &ball.tiles,
        &png_4bpp(&format!("{dir}/pokeball_selection.png")),
        "ball",
    );
    assert_eq!(
        flat(&ball.palettes),
        png_palette(&format!("{dir}/pokeball_selection.png"))
    );
    let circle = by_name(&all, "starter/circle");
    same(
        &circle.tiles,
        &png_4bpp(&format!("{dir}/starter_circle.png")),
        "circle",
    );
    assert_eq!(
        flat(&circle.palettes),
        png_palette(&format!("{dir}/starter_circle.png"))
    );
}

fn latin_small_png_glyphs() -> Vec<Vec<u8>> {
    let decoder =
        png::Decoder::new(std::fs::File::open(decomp("graphics/fonts/latin_small.png")).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let (w, h) = (info.width as usize, info.height as usize);
    let px = |x: usize, y: usize| (buf[y * info.line_size + x / 4] >> (6 - 2 * (x % 4))) & 3;
    let mut glyphs = Vec::new();
    for gy in 0..h / 16 {
        for gx in 0..w / 16 {
            glyphs.push(
                (0..256)
                    .map(|i| px(gx * 16 + i % 16, gy * 16 + i / 16))
                    .collect(),
            );
        }
    }
    glyphs
}

fn fonts_c_small_widths() -> Vec<u8> {
    let src = std::fs::read_to_string(decomp("src/fonts.c")).unwrap();
    let body = &src[src.find("gFontSmallLatinGlyphWidths[] = {").unwrap()..];
    let body = &body[body.find('{').unwrap() + 1..body.find("};").unwrap()];
    body.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

#[test]
fn us_latin_small_matches_decomp() {
    let (g, w) = font::locate_latin_small(us()).unwrap();
    assert_eq!((g, w), (0x633ce4, 0x63bce4));
    let f = font::read_latin_at(us(), g, w).unwrap();
    assert_eq!(f.glyphs, latin_small_png_glyphs());
    assert_eq!(f.widths, fonts_c_small_widths());
}

#[test]
fn fr_tables_are_found_with_the_same_counts() {
    let l = fr_located();
    assert_eq!(l.mons.count, us_located().mons.count);
    assert_eq!(l.trainers.front_count, us_located().trainers.front_count);
    assert_eq!(l.trainers.back_count, us_located().trainers.back_count);
    let (fr_all, us_all) = (
        battle_gfx::graphics(fr(), l).unwrap(),
        battle_gfx::graphics(us(), us_located()).unwrap(),
    );
    let names = |a: &[(String, Gfx)]| a.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    assert_eq!(names(&fr_all), names(&us_all));
    let mut localized = Vec::new();
    for ((name, f), (_, u)) in fr_all.iter().zip(&us_all) {
        assert_eq!(f.tiles.len(), u.tiles.len(), "{name}: tile count");
        assert_eq!(f.palettes.len(), u.palettes.len(), "{name}: palettes");
        assert_eq!(f.tilemap.len(), u.tilemap.len(), "{name}: tilemap");
        if f != u {
            let tiles: Vec<usize> = (0..f.tiles.len() / 32)
                .filter(|&t| f.tiles[t * 32..t * 32 + 32] != u.tiles[t * 32..t * 32 + 32])
                .collect();
            localized.push((
                name.as_str(),
                tiles,
                f.palettes != u.palettes,
                f.tilemap != u.tilemap,
            ));
        }
    }
    // BPEF redraws: one healthbox tile (35), the "HP" label (hpbar 1-2, "PV" in French), and the
    // SLP, FRZ and BRN icons of both status sets (PSN and PAR keep their letters).
    let mut status: Vec<usize> = [6, 7, 8, 9, 10, 11, 13, 14]
        .into_iter()
        .flat_map(|t| [t, t + 15])
        .collect();
    status.sort_unstable();
    assert_eq!(
        localized,
        vec![
            ("battle/gfx/healthbox_player", vec![35], false, false),
            ("battle/gfx/hpbar", vec![1, 2], false, false),
            ("battle/gfx/status", status, false, false),
        ]
    );
    // Pokémon pics are not localized.
    let id = species("SPECIES_TORCHIC");
    let (a, b) = (
        l.mons.get(fr(), id).unwrap(),
        us_located().mons.get(us(), id).unwrap(),
    );
    assert_eq!((a.front, a.back, a.palette), (b.front, b.back, b.palette));
    let (g, w) = font::locate_latin_small(fr()).unwrap();
    let f = font::read_latin_at(fr(), g, w).unwrap();
    assert_eq!(f.glyphs.len(), 512);
    let (ug, uw) = font::locate_latin_small(us()).unwrap();
    let u = font::read_latin_at(us(), ug, uw).unwrap();
    let redrawn: Vec<usize> = (0..512).filter(|&i| f.glyphs[i] != u.glyphs[i]).collect();
    let widths: Vec<usize> = (0..512).filter(|&i| f.widths[i] != u.widths[i]).collect();
    // BPEF redraws `LV` (0x34), the unused 0x58/0x59/0x64, the guillemets 0xB1/0xB2 and three
    // symbols (0x105, 0x107, 0x108); only 0x59 and 0x64 change width.
    assert_eq!(
        redrawn,
        [0x34, 0x58, 0x59, 0x64, 0xB1, 0xB2, 0x105, 0x107, 0x108]
    );
    assert_eq!(widths, [0x59, 0x64]);
    assert!(widths.iter().all(|i| redrawn.contains(i)));
    assert!(
        f.glyphs[0x105].iter().any(|&p| p != 0),
        "glyph 0x105 is drawn"
    );

    // Data tables that are not localized: the located FR tables hold the US bytes.
    let (fl, ul) = (fr_located(), us_located());
    let table = |rom: &Rom, off: u32, len: usize| rom.slice(off, len).unwrap().to_vec();
    let mons = ul.mons.count * 4;
    let (tf, tb) = (ul.trainers.front_count * 4, ul.trainers.back_count * 4);
    for (what, f_off, u_off, len) in [
        (
            "elevation",
            fl.mons.elevation,
            ul.mons.elevation,
            ul.mons.elevation_count,
        ),
        (
            "mon front coords",
            fl.mons.front_coords,
            ul.mons.front_coords,
            mons,
        ),
        (
            "mon back coords",
            fl.mons.back_coords,
            ul.mons.back_coords,
            mons,
        ),
        (
            "trainer front coords",
            fl.trainers.front_coords,
            ul.trainers.front_coords,
            tf,
        ),
        (
            "trainer back coords",
            fl.trainers.back_coords,
            ul.trainers.back_coords,
            tb,
        ),
    ] {
        assert_eq!(table(fr(), f_off, len), table(us(), u_off, len), "{what}");
    }
}

/// Exports the battle graphics of `rom` into a fresh directory.
fn export_into(tag: &str, rom: &Rom) -> (PathBuf, BTreeMap<String, String>) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("battle-gfx-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    let mut o = Out::new(&dir);
    let mut provenance = BTreeMap::new();
    battle_gfx::export(rom, &mut o, &mut provenance).unwrap();
    assert!(provenance.contains_key("battle_gfx/environments"));
    assert!(provenance.values().all(|&Hex(v)| v > 0));
    (dir, o.files().clone())
}

#[test]
fn export_round_trips_and_is_deterministic() {
    let (dir, files) = export_into("a", fr());
    let (_, again) = export_into("b", fr());
    assert_eq!(files, again);

    let gfx_file: GfxFile = export::read_ron(&dir.join("battle/env/Grass.ron")).unwrap();
    let img = gfx::read_png_indexed(&dir.join("battle/env/Grass.png")).unwrap();
    assert_eq!(gfx_file.tile_count as usize * 32, {
        let t = gfx::image_to_tiles_4bpp(&img).unwrap();
        t.len()
    });
    assert_eq!(gfx_file.palettes.len(), 3);
    assert_eq!(gfx_file.palette_bank, 2);
    assert_eq!(from_rows(&gfx_file.tilemap).len(), 32 * 64);
    for name in [
        "battle/gfx/textbox",
        "battle/gfx/healthbox_player",
        "battle/gfx/healthbox_opponent",
        "battle/gfx/hpbar",
        "battle/gfx/expbar",
        "battle/gfx/status",
        "battle/gfx/misc",
        "battle/gfx/ball",
        "starter/bag",
        "starter/ball",
        "starter/circle",
    ] {
        let f: GfxFile = export::read_ron(&dir.join(format!("{name}.ron"))).unwrap();
        let img = gfx::read_png_indexed(&dir.join(format!("{name}.png"))).unwrap();
        assert_eq!(img.width, u32::from(f.width) * 8, "{name}");
        let tiles = (img.width / 8 * img.height / 8) as u16;
        assert!(
            tiles >= f.tile_count && tiles - f.tile_count < f.width,
            "{name}"
        );
    }
    let grass: GfxFile = export::read_ron(&dir.join("starter/grass.ron")).unwrap();
    assert_eq!(grass.tile_count, 0);
    assert!(!dir.join("starter/grass.png").exists());

    let coords: MonCoordsFile = export::read_ron(&dir.join("pokemon/coords.ron")).unwrap();
    assert_eq!(coords.len(), 440);
    assert_eq!(coords["SPECIES_TREECKO"].front_y, 8);
    for s in ["SPECIES_TREECKO", "SPECIES_ZIGZAGOON"] {
        for f in ["front.png", "back.png", "shiny.pal"] {
            assert!(dir.join(format!("pokemon/{s}/{f}")).exists(), "{s}/{f}");
        }
        let img = gfx::read_png_indexed(&dir.join(format!("pokemon/{s}/front.png"))).unwrap();
        assert_eq!((img.width, img.height), (64, 64));
    }
    let shiny = gfx::read_jasc_pal(&dir.join("pokemon/SPECIES_TREECKO/shiny.pal")).unwrap();
    let id = species("SPECIES_TREECKO");
    assert_eq!(shiny.0, fr_located().mons.get(fr(), id).unwrap().shiny);

    // Every species with ROM graphics (Unown's entry is form A; forms B-? are their own
    // `SPECIES_UNOWN_*` entries), not a slice set.
    for (_, s) in extract::names::SPECIES.iter() {
        for f in ["front.png", "back.png", "shiny.pal"] {
            assert!(dir.join(format!("pokemon/{s}/{f}")).exists(), "{s}/{f}");
        }
    }

    let pics: TrainerPicsFile = export::read_ron(&dir.join("trainers/pics.ron")).unwrap();
    assert_eq!(pics.front.len(), 93);
    // Every trainer's pic.
    let trainers = extract::battle_data::extract(fr(), &[]).unwrap().trainers;
    for t in &trainers.trainers {
        assert!(pics.front.contains_key(&t.pic), "{}: {}", t.id, t.pic);
        assert!(
            dir.join(format!("trainers/front/{}.png", t.pic)).exists(),
            "{}",
            t.pic
        );
    }
    assert_eq!(pics.back["TRAINER_BACK_PIC_BRENDAN"], 4);
    assert_eq!(pics.back_coords["TRAINER_BACK_PIC_BRENDAN"].size, 8);
    let back = gfx::read_png_indexed(&dir.join("trainers/back/TRAINER_BACK_PIC_BRENDAN.png"));
    let back = back.unwrap();
    assert_eq!((back.width, back.height), (256, 64), "frames side by side");
    let may = gfx::read_png_indexed(&dir.join("trainers/front/TRAINER_PIC_MAY.png")).unwrap();
    assert_eq!((may.width, may.height), (64, 64));

    let small: FontFile = export::read_ron(&dir.join("font/latin_small.ron")).unwrap();
    assert_eq!((small.glyphs, small.glyphs_per_row), (512, 16));
    assert!(dir.join("font/latin_small.png").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
