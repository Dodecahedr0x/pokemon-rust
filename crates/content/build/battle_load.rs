//! Reading the battle files of an extracted dir ([`load`]) into a [`BattleIn`].

use crate::battle::{BattleIn, FIRST_FOE, GfxIn, MonGfxIn, PLAYER_BACK, PicIn, STARTERS, UiGfx};
use crate::input::{self, Tile};
use anyhow::{Context, Result, bail, ensure};
use extract::battle_gfx::ENVIRONMENTS;
use extract::export::read_ron;
use extract::gfx;
use extract::schema::{
    BattleStringsFile, EncountersFile, GfxFile, HealLocationsFile, MonCoordsFile, MovesFile,
    SpeciesFile, TrainerPicsFile, TrainersFile, TypesFile, from_rows,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The palette of an indexed PNG (`PLTE`, 8-bit RGB -> BGR555 as gbagfx: `x / 8`).
pub fn png_palette(path: &Path) -> Result<[u16; 16]> {
    let b = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    ensure!(
        b.starts_with(b"\x89PNG\r\n\x1a\n"),
        "{}: not a PNG",
        path.display()
    );
    let mut i = 8;
    while i + 8 <= b.len() {
        let len = u32::from_be_bytes(b[i..i + 4].try_into().unwrap()) as usize;
        let data = b
            .get(i + 8..i + 8 + len)
            .with_context(|| format!("{}: truncated chunk", path.display()))?;
        if &b[i + 4..i + 8] == b"PLTE" {
            let mut p = [0u16; 16];
            for (k, c) in data.chunks_exact(3).take(16).enumerate() {
                p[k] =
                    u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10;
            }
            return Ok(p);
        }
        i += 12 + len;
    }
    bail!("{}: no PLTE chunk", path.display())
}

fn tiles_png(path: &Path) -> Result<Vec<Tile>> {
    input::tiles_of(&gfx::read_png_indexed(path)?).with_context(|| path.display().to_string())
}

/// Frames of `w`-pixel width side by side in a png: each frame's tiles, one after the other.
fn frames_png(path: &Path, frames: u32) -> Result<Vec<Tile>> {
    let img = gfx::read_png_indexed(path)?;
    ensure!(
        img.width == frames * 64 && img.height == 64,
        "{}: {}x{}, expected {} frames of 64x64",
        path.display(),
        img.width,
        img.height,
        frames
    );
    let mut out = Vec::new();
    for k in 0..frames {
        out.extend(input::tiles_of(&input::crop(&img, k * 64, 0, 64, 64)?)?);
    }
    Ok(out)
}

fn gfx_file(dir: &Path, rel: &str) -> Result<GfxIn> {
    let f: GfxFile = read_ron(&dir.join(format!("{rel}.ron")))?;
    let mut tiles = if f.tile_count > 0 {
        tiles_png(&dir.join(format!("{rel}.png")))?
    } else {
        vec![]
    };
    ensure!(
        tiles.len() >= usize::from(f.tile_count),
        "{rel}: {} tiles in png, {} in ron",
        tiles.len(),
        f.tile_count
    );
    tiles.truncate(f.tile_count.into());
    Ok(GfxIn {
        tiles,
        palettes: f.palettes,
        tilemap: from_rows(&f.tilemap),
    })
}

/// The front pic `pic` (`TRAINER_PIC_*`) of an extracted dir, `None` when it was not exported.
pub fn trainer_pic(dir: &Path, pics: &TrainerPicsFile, pic: &str) -> Result<Option<PicIn>> {
    let path = dir.join(format!("trainers/front/{pic}.png"));
    if !path.exists() {
        return Ok(None);
    }
    let info = pics
        .front
        .get(pic)
        .with_context(|| format!("trainers/pics.ron has no {pic}"))?;
    Ok(Some(PicIn {
        tiles: frames_png(&path, 1)?,
        palette: png_palette(&path)?,
        y: info.size,
    }))
}

/// Reads the battle files of an extracted dir: every table, and every species and trainer pic
/// the extract has.
pub fn load(dir: &Path) -> Result<BattleIn> {
    let b = dir.join("battle");
    let species: SpeciesFile = read_ron(&b.join("species.ron"))?;
    let moves: MovesFile = read_ron(&b.join("moves.ron"))?;
    let types: TypesFile = read_ron(&b.join("types.ron"))?;
    let trainers: TrainersFile = read_ron(&b.join("trainers.ron"))?;
    let encounters: EncountersFile = read_ron(&b.join("encounters.ron"))?;
    let heal: HealLocationsFile = read_ron(&b.join("heal_locations.ron"))?;
    let strings: BattleStringsFile = read_ron(&b.join("strings.ron"))?;
    let mut out = BattleIn {
        species: species.species,
        moves,
        types,
        trainers: trainers.trainers,
        encounters: encounters.maps,
        heal,
        strings,
        mon_gfx: BTreeMap::new(),
        trainer_pics: BTreeMap::new(),
        // Format 5.1 (Task A); empty in older extracts.
        evolutions: crate::read_opt!(b.join("evolutions.ron")),
        trainer_money: crate::read_opt!(b.join("trainer_money.ron")),
        player_back: PicIn {
            tiles: vec![],
            palette: [0; 16],
            y: 0,
        },
        ui: UiGfx::default(),
        small_glyphs: vec![],
        small_widths: vec![],
        starters: STARTERS.map(String::from),
        first_foe: (FIRST_FOE.0.into(), FIRST_FOE.1),
        stub: false,
        struggle: "MOVE_STRUGGLE".into(),
    };

    let coords: MonCoordsFile = read_ron(&dir.join("pokemon/coords.ron"))?;
    for s in out.species.iter().map(|s| &s.id) {
        let p = dir.join("pokemon").join(s);
        // A species without pics battles without them (`gfx: None`; listed in the report).
        let Some(&c) = coords.get(s) else { continue };
        if !p.join("front.png").exists() {
            continue;
        }
        let front = tiles_png(&p.join("front.png"))?;
        let back = tiles_png(&p.join("back.png"))?;
        ensure!(
            front.len() == 64 && back.len() == 64,
            "{s}: pics are not 64x64"
        );
        let g = MonGfxIn {
            front,
            back,
            palette: png_palette(&p.join("front.png"))?,
            shiny: gfx::read_jasc_pal(&p.join("shiny.pal"))?.0,
            coords: c,
        };
        out.mon_gfx.insert(s.clone(), g);
    }

    let pics: TrainerPicsFile = read_ron(&dir.join("trainers/pics.ron"))?;
    let used_pics: BTreeSet<String> = out.trainers.iter().map(|t| t.pic.clone()).collect();
    for pic in used_pics {
        // A trainer whose pic was not exported is left out (`trainer()` is `None`; listed in
        // the report).
        if let Some(p) = trainer_pic(dir, &pics, &pic)? {
            out.trainer_pics.insert(pic, p);
        }
    }
    let frames = *pics
        .back
        .get(PLAYER_BACK)
        .with_context(|| format!("trainers/pics.ron has no {PLAYER_BACK}"))?;
    let path = dir.join(format!("trainers/back/{PLAYER_BACK}.png"));
    out.player_back = PicIn {
        tiles: frames_png(&path, frames.into())?,
        palette: png_palette(&path)?,
        y: pics
            .back_coords
            .get(PLAYER_BACK)
            .with_context(|| format!("trainers/pics.ron has no back_coords {PLAYER_BACK}"))?
            .size,
    };

    let g = |n: &str| gfx_file(dir, &format!("battle/gfx/{n}"));
    let mut bag = gfx_file(dir, "starter/bag")?;
    ensure!(bag.tilemap.len() <= 1024, "starter/bag: tilemap > 1024");
    bag.tilemap.resize(1024, 0);
    let grass = gfx_file(dir, "starter/grass")?;
    ensure!(grass.tilemap.len() == 1024, "starter/grass: not 32x32");
    bag.tilemap.extend(grass.tilemap);
    out.ui = UiGfx {
        textbox: g("textbox")?,
        environments: ENVIRONMENTS
            .iter()
            .map(|e| gfx_file(dir, &format!("battle/env/{e}")))
            .collect::<Result<_>>()?,
        healthbox_player: g("healthbox_player")?,
        healthbox_opponent: g("healthbox_opponent")?,
        hpbar: g("hpbar")?,
        expbar: g("expbar")?,
        status: g("status")?,
        misc: g("misc")?,
        ball: g("ball")?,
        starter_bag: bag,
        starter_ball: gfx_file(dir, "starter/ball")?,
        starter_circle: gfx_file(dir, "starter/circle")?,
    };
    let (glyphs, widths) = input::read_font(dir, "latin_small")?;
    out.small_glyphs = glyphs;
    out.small_widths = widths;
    Ok(out)
}
