//! Battle graphics (Milestone 4 Task B): Pokémon and trainer pics, the battle interface, the
//! battle environments and the starter-choice screen, plus the small font (in [`crate::font`]).
//!
//! Every table is found by a structural signature built from decomp source constants (sprite
//! sheet sizes and tags, `struct` layouts, the order of definitions in one source file), never
//! by a per-version offset. Code-only symbols are reached through the data placed next to them.
//!
//! # `engine::battle::BattleGfx` field list (fixed by Task B; Task I fills it)
//!
//! Every `GfxImage` below is one exported
//! [`GfxFile`]: `tiles` = the png's tiles in row-major order (`tile_count` of them), `palettes`,
//! `tilemap` (empty when the file has none).
//!
//! | field | file | contents |
//! |---|---|---|
//! | `textbox` | `battle/gfx/textbox` | `gBattleTextbox{Tiles,Palette,Tilemap}`: 256 tiles, 32x64 map (Gen3's 64-row BG0), 4 palettes: `[textbox_0, textbox_1]` (BG 0-1), then `gBattleWindowTextPalette` (`text.pal`, BG 5) and `gPPTextPalette` (`text_pp.pal`: 4 PP colour levels x 2 colours, for BG 5 entries 12 and 11) |
//! | `environments[e]` | `battle/env/<Environment>` | `sBattleEnvironmentTable[e]`: tiles, 3 palettes (BG 2-4), a 64x32 map stored as two 32x32 screen blocks side by side, the left one first (2048 entries) |
//! | `healthbox_player` | `battle/gfx/healthbox_player` | `gHealthboxSinglesPlayerGfx`: 128 tiles (gbagfx `-mwidth 8 -mheight 8`: the png's left 64x64 half, then its right half; the export png is 8 tiles wide). `CreateBattlerHealthboxSprites`: two square 64x64 OBJs start at tiles 0 and 64; Gen3 writes the level, HP numbers and exp bar into the other tiles |
//! | `healthbox_opponent` | `battle/gfx/healthbox_opponent` | `gHealthboxSinglesOpponentGfx`: 64 tiles (`-mwidth 8 -mheight 4`); OBJs at tiles 0 and 32 |
//! | `hpbar` | `battle/gfx/hpbar` | 30 tiles: `HEALTHBOX_GFX_0..=11` (0 black end, drawn with the HP bar palette, 1-2 the "HP" label, "PV" in BPEF, 3-11 green 0..8 px) then `HEALTHBOX_GFX_47..=64` (yellow 0..8 px, red 0..8 px); palette = the HP bar OBJ palette (`TAG_HEALTHBAR_PAL`) |
//! | `expbar` | `battle/gfx/expbar` | 9 tiles: `HEALTHBOX_GFX_12..=20` (0..8 px); healthbox palette |
//! | `status` | `battle/gfx/status` | 30 tiles: 15 for the player (battler 0, `status.png`), then 15 for the opponent (battler 1, `status2.png`); 3 tiles per icon in `HEALTHBOX_GFX_STATUS_*` order PSN, PAR, SLP, FRZ, BRN. The two sets differ only in the palette index of the fill. Localized in BPEF. Healthbox palette |
//! | `misc` | `battle/gfx/misc` | 12 tiles: `HEALTHBOX_GFX_36..=46` (11 `misc.png` tiles; 39 is the blank "no status" tile) then `HEALTHBOX_GFX_65` (HP bar frame end, tile 11, drawn with the HP bar palette); `palettes` = the healthbox palette, which serves tiles 0-10 |
//! | (no `numbers`) | none | `numbers1/2.png` only feed the unused `gBattleInterfaceGfx_BattleBar`, so there is no digit image. Gen3 prints the level ("Lv" is font glyph `0x100 + CHAR_LV_2`), the HP numbers and the gender symbol with `FONT_SMALL` (`AddTextPrinterAndCreateWindowOnHealthbox`) |
//! | `ball` | `battle/gfx/ball` | `gBallGfx_Poke` (12 tiles: three 16x16 frames) + `gBallPal_Poke` |
//! | `small_font` | `font/latin_small` | `gFontSmallLatinGlyphs` / widths (same layout as `latin_normal`) |
//! | `player_back` | `trainers/back/TRAINER_BACK_PIC_BRENDAN` | 4 frames of 64 tiles, side by side in the png |
//! | `starter_bag` | `starter/bag` | `gBirchBagGrass_Gfx` + `gBirchBagGrass_Pal` (2 palettes, BG 0-1). `tilemap` = `gBirchBagTilemap` (`starter/bag`, 32x20, BG3) padded with 0 to 1024 entries, then `gBirchGrassTilemap` (`starter/grass.ron`, a tilemap-only file: 32x32, BG2, the bag's tiles and palettes) |
//! | `starter_ball` | `starter/ball` | `gPokeballSelection_Gfx` (64 tiles: 32x32 frames, ball and hand) + its palette |
//! | `starter_circle` | `starter/circle` | `sStarterCircle_Gfx` (64 tiles, 64x64) + its palette |
//!
//! OBJ palettes: the healthbox palette (`TAG_HEALTHBOX_PAL`, `gBattleInterface_BallStatusBarPal`)
//! serves both healthboxes, the exp bar, the status icons and `misc`; the HP bar palette
//! (`TAG_HEALTHBAR_PAL`, `gBattleInterface_BallDisplayPal`) serves `hpbar`.
//!
//! Pokémon pics (`MonGfx`): `pokemon/<SPECIES>/front.png` is frame 0 of `anim_front` (64x64),
//! `back.png` 64x64, both with the normal palette; `shiny.pal`; `pokemon/coords.ron`.
//! Trainer front pics (`TrainerPic`): `trainers/front/<TRAINER_PIC>.png`, frame 0 (64x64).
//! `TrainerPic.y` is the Gen3 `struct MonCoords` `.size` of the pic (`TrainerPicInfo::size`,
//! front in `pics.ron` `front`, back in `back_coords`), not `y_offset`: Gen3 places the
//! opponent's front pic at (176, 40 + 4 * (8 - size)) and the player's back pic at
//! (80, 80 + 4 * (8 - size)).

use crate::export::{self, Out};
use crate::gfx::{self, IndexedImage, Palette, TILE_BYTES};
use crate::schema::{
    FontFile, GfxFile, Hex, MonCoords, MonCoordsFile, TrainerPicInfo, TrainerPicsFile, to_rows,
};
use crate::{Rom, font, names};
use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// `Environment` names (engine order = `BATTLE_ENVIRONMENT_*`), used as `battle/env/<name>`.
pub const ENVIRONMENTS: [&str; 10] = [
    "Grass",
    "LongGrass",
    "Sand",
    "Underwater",
    "Water",
    "Pond",
    "Mountain",
    "Cave",
    "Building",
    "Plain",
];

/// `MON_PIC_SIZE`: one 64x64 4bpp frame.
const PIC_SIZE: usize = 0x800;
/// `sizeof(struct MonCoords)` in the ROM (two u8 padded to 4 by agbcc).
const COORDS_SIZE: u32 = 4;
/// `SPECIES_SHINY_TAG` (include/data.h).
const SHINY_TAG: u32 = 500;
/// `MON_COORDS_SIZE(64, 64)`, the coords of `SPECIES_NONE`.
const COORDS_NONE: [u8; 4] = [0x88, 0, 0, 0];

/// `TAG_HEALTHBOX_PLAYER1_TILE` .. (include/battle_interface.h); also `TAG_HEALTHBOX_PAL`.
const TAG_HEALTHBOX_PLAYER1: u16 = 0xD6FF;
const TAG_HEALTHBOX_PLAYER2: u16 = 0xD700;
const TAG_HEALTHBOX_OPPONENT1: u16 = 0xD701;
const TAG_HEALTHBOX_OPPONENT2: u16 = 0xD702;
/// `TAG_HEALTHBAR_PLAYER1_TILE`, also `TAG_HEALTHBAR_PAL`; the 4 bars are 0xD704..=0xD707.
const TAG_HEALTHBAR_PLAYER1: u16 = 0xD704;
const TAG_HEALTHBOX_SAFARI: u16 = 0xD70B;
/// `GFX_TAG_POKE_BALL` (pokeball.c), with `gBallSpriteSheets[BALL_POKE].size` 384.
const TAG_POKE_BALL: u16 = 55000;
const BALL_GFX_SIZE: usize = 384;
/// `TAG_POKEBALL_SELECT` / `TAG_STARTER_CIRCLE` (starter_choose.c).
const TAG_POKEBALL_SELECT: u16 = 0x1000;
const TAG_STARTER_CIRCLE: u16 = 0x1001;

/// `gHealthboxElementsGfxTable` (graphics.c): (first tile, tile count) of each source file, from
/// the png sizes; 118 tiles in all.
const ELEM_HPBAR: (usize, usize) = (0, 12);
const ELEM_EXPBAR: (usize, usize) = (12, 9);
const ELEM_STATUS: (usize, usize) = (21, 15);
const ELEM_MISC: (usize, usize) = (36, 11);
const ELEM_HPBAR_ANIM: (usize, usize) = (47, 18);
const ELEM_MISC_FRAMEEND: (usize, usize) = (65, 1);
const ELEM_STATUS2: (usize, usize) = (71, 15);
const ELEM_TILES: usize = 118;

/// Where every table is, as file offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct Located {
    pub mons: MonTables,
    pub trainers: TrainerTables,
    /// `sBattleEnvironmentTable`.
    pub environments: u32,
    /// `gBattleTextboxTiles`, `gBattleTextboxPalette`, `gBattleTextboxTilemap`.
    pub textbox: [u32; 3],
    /// `gBattleWindowTextPalette` (LZ77) and `gPPTextPalette` (raw, right after it).
    pub text_palettes: [u32; 2],
    /// `sSpriteSheet_SinglesPlayerHealthbox` (battle_gfx_sfx_util.c), first of the healthbox
    /// sheets, healthbar sheets and `sSpritePalettes_HealthBoxHealthBar`.
    pub healthbox_sheets: u32,
    /// `gBattleInterface_BallStatusBarPal` (`TAG_HEALTHBOX_PAL`).
    pub healthbox_palette: u32,
    /// `gBattleInterface_BallDisplayPal` (`TAG_HEALTHBAR_PAL`).
    pub healthbar_palette: u32,
    /// `gHealthboxElementsGfxTable`.
    pub healthbox_elements: u32,
    /// `gBallSpriteSheets` / `gBallSpritePalettes`.
    pub ball_sheets: u32,
    pub ball_palettes: u32,
    /// `sSpriteSheet_PokeballSelect` (starter_choose.c), followed by the circle sheet and
    /// `sSpritePalettes_StarterChoose`.
    pub starter_sheets: u32,
}

/// The `data.c` Pokémon pic tables.
#[derive(Clone, Debug, PartialEq)]
pub struct MonTables {
    /// Entries per table (`NUM_SPECIES` + the Unown forms).
    pub count: usize,
    /// `gMonFrontPicTable` (2-frame `anim_front`), `gMonStillFrontPicTable`, `gMonBackPicTable`.
    pub front: u32,
    pub still_front: u32,
    pub back: u32,
    /// `gMonPaletteTable` / `gMonShinyPaletteTable`.
    pub palettes: u32,
    pub shiny: u32,
    /// `gMonFrontPicCoords` / `gMonBackPicCoords`.
    pub front_coords: u32,
    pub back_coords: u32,
    /// `gEnemyMonElevation[NUM_SPECIES]`.
    pub elevation: u32,
    pub elevation_count: usize,
}

/// One species' battle pics.
#[derive(Clone, Debug, PartialEq)]
pub struct MonPics {
    /// Frame 0 of the front pic, 64 tiles.
    pub front: Vec<u8>,
    pub back: Vec<u8>,
    pub palette: [u16; 16],
    pub shiny: [u16; 16],
    pub coords: MonCoords,
}

/// The `data.c` trainer pic tables.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainerTables {
    pub front_count: usize,
    /// `gTrainerFrontPicTable`, `gTrainerFrontPicPaletteTable`, `gTrainerFrontPicCoords`.
    pub front: u32,
    pub front_palettes: u32,
    pub front_coords: u32,
    pub back_count: usize,
    /// `gTrainerBackPicTable` (uncompressed pics), `gTrainerBackPicPaletteTable`,
    /// `gTrainerBackPicCoords`.
    pub back: u32,
    pub back_palettes: u32,
    pub back_coords: u32,
}

/// A trainer pic: `frames` 64x64 frames of 64 tiles each, one after the other.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainerPic {
    pub tiles: Vec<u8>,
    pub frames: u8,
    pub palette: [u16; 16],
    /// `struct MonCoords` fields.
    pub size: u8,
    pub y_offset: u8,
}

/// A decoded graphic, written as one [`GfxFile`] (+ png when it has tiles).
#[derive(Clone, Debug, PartialEq)]
pub struct Gfx {
    /// Raw 4bpp tiles.
    pub tiles: Vec<u8>,
    /// Tiles per png row.
    pub width: u16,
    pub palettes: Vec<[u16; 16]>,
    pub palette_bank: u8,
    pub tilemap: Vec<u16>,
}

// ---- LZ77 helpers ----

/// Declared size of the LZ77 stream at file offset `off`, if one starts there.
fn lz_size(rom: &Rom, off: u32) -> Option<usize> {
    let h = rom.slice(off, 4).ok()?;
    (h[0] == 0x10).then(|| u32::from_le_bytes([h[1], h[2], h[3], 0]) as usize)
}

/// The ROM pointer stored at `word` if it targets an LZ77 stream: (target, declared size).
fn lz_ptr(rom: &Rom, word: u32) -> Option<(u32, usize)> {
    let target = rom.ptr(word).ok()??;
    Some((target, lz_size(rom, target)?))
}

/// End of the LZ77 stream at `off`, rounded up to 4 like the next `INCBIN_U32` in the same file.
fn lz_end(rom: &Rom, off: u32) -> Result<u32> {
    let size = lz_size(rom, off).with_context(|| format!("no lz77 stream at {off:#x}"))?;
    let (mut pos, mut out) = (off + 4, 0usize);
    while out < size {
        let flags = rom.u8(pos)?;
        pos += 1;
        for bit in 0..8 {
            if out >= size {
                break;
            }
            if flags & (0x80 >> bit) != 0 {
                out += (rom.u8(pos)? >> 4) as usize + 3;
                pos += 2;
            } else {
                out += 1;
                pos += 1;
            }
        }
    }
    Ok(pos.next_multiple_of(4))
}

/// Decompresses the LZ77 stream at `off` and checks its size.
fn lz_exact(rom: &Rom, off: u32, size: usize, what: &str) -> Result<Vec<u8>> {
    let data = rom.lz77_at(off).with_context(|| what.to_string())?;
    ensure!(
        data.len() == size,
        "{what} at {off:#x}: {} bytes, expected {size:#x}",
        data.len()
    );
    Ok(data)
}

fn palettes_from(bytes: &[u8], what: &str) -> Result<Vec<[u16; 16]>> {
    ensure!(
        !bytes.is_empty() && bytes.len().is_multiple_of(32),
        "{what}: {} palette bytes",
        bytes.len()
    );
    bytes
        .chunks_exact(32)
        .map(|c| Ok(Palette::from_bytes(c)?.0))
        .collect()
}

fn palette_at(rom: &Rom, off: u32) -> Result<[u16; 16]> {
    Ok(Palette::from_bytes(rom.slice(off, 32)?)?.0)
}

/// The first palette of a compressed palette (Castform's holds one per form).
fn lz_palette(rom: &Rom, off: u32, what: &str) -> Result<[u16; 16]> {
    let data = rom.lz77_at(off).with_context(|| what.to_string())?;
    Ok(palettes_from(&data, what)?[0])
}

fn u16s(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

// ---- sprite sheet / palette tables ----

/// `struct CompressedSpriteSheet { const u32 *data; u16 size; u16 tag; }` at `off`.
fn sheet(rom: &Rom, off: u32) -> Option<(u32, u16, u16)> {
    let data = rom.ptr(off).ok()??;
    Some((data, rom.u16(off + 4).ok()?, rom.u16(off + 6).ok()?))
}

/// `struct CompressedSpritePalette { const u32 *data; u16 tag; }` (padded to 8) at `off`.
fn pal_entry(rom: &Rom, off: u32) -> Option<(u32, u16)> {
    let data = rom.ptr(off).ok()??;
    (rom.u16(off + 6).ok()? == 0).then_some(())?;
    Some((data, rom.u16(off + 4).ok()?))
}

/// Number of consecutive sheets from `off` whose tag is their index (from `base`) and whose
/// size passes `size_ok`.
fn sheet_run(rom: &Rom, off: u32, size_ok: &impl Fn(u16) -> bool) -> usize {
    (0..)
        .take_while(|&i: &u32| {
            matches!(sheet(rom, off + 8 * i), Some((_, s, t)) if size_ok(s) && u32::from(t) == i)
        })
        .count()
}

fn pal_run(rom: &Rom, off: u32, base: u32) -> usize {
    (0..)
        .take_while(|&i: &u32| {
            matches!(pal_entry(rom, off + 8 * i), Some((_, t)) if u32::from(t) == base + i)
        })
        .count()
}

/// Offsets of every sheet table of exactly `count` entries (tag = index, sizes passing `size_ok`).
fn sheet_tables(rom: &Rom, count: usize, size_ok: impl Fn(u16) -> bool) -> Vec<u32> {
    let len = rom.bytes().len() as u32;
    let mut out = Vec::new();
    let mut off = 0;
    while off + 16 <= len {
        if matches!(sheet(rom, off), Some((_, s, 0)) if size_ok(s))
            && matches!(sheet(rom, off + 8), Some((_, s, 1)) if size_ok(s))
        {
            let n = sheet_run(rom, off, &size_ok);
            if n == count {
                out.push(off);
            }
            off += 8 * n as u32;
            continue;
        }
        off += 4;
    }
    out
}

/// Coords tables are `count` 4-byte `struct MonCoords` with zero padding.
fn check_coords(rom: &Rom, off: u32, count: usize, what: &str) -> Result<()> {
    let bytes = rom
        .slice(off, count * COORDS_SIZE as usize)
        .with_context(|| format!("{what} at {off:#x}"))?;
    for (i, c) in bytes.chunks_exact(4).enumerate() {
        ensure!(
            c[2] == 0 && c[3] == 0 && c[1] < 64,
            "{what} at {off:#x}: entry {i} {c:?} is not a MonCoords"
        );
    }
    Ok(())
}

fn table_count(t: &names::Table) -> usize {
    t.entries.last().map_or(0, |&(v, _)| v as usize + 1)
}

/// Finds the Pokémon pic tables. `data.c` defines, in order: front coords, still front pics,
/// back coords, back pics, palettes, shiny palettes, ... trainer tables ..., front pics.
fn locate_mons(rom: &Rom, elevation: u32) -> Result<MonTables> {
    let count = table_count(&names::SPECIES);
    let tables = sheet_tables(rom, count, |s| usize::from(s) == PIC_SIZE);
    ensure!(
        tables.len() == 3,
        "Pokémon pic tables ({count} sheets of {PIC_SIZE:#x}, tag = species): expected 3 \
         (front, still front, back), found {tables:x?}"
    );
    let n = count as u32;
    let is_back = |&t: &u32| {
        pal_run(rom, t + 8 * n, 0) == count && pal_run(rom, t + 16 * n, SHINY_TAG) == count
    };
    let backs: Vec<u32> = tables.iter().copied().filter(is_back).collect();
    let [back] = backs[..] else {
        bail!("Pokémon back pic table (followed by both palette tables): found {backs:x?}");
    };
    let anim_size = |t: u32| -> Result<usize> {
        let (data, _, _) = sheet(rom, t + 8).context("sheet")?;
        lz_size(rom, data).with_context(|| format!("species 1 front pic at {data:#x}"))
    };
    let mut front = None;
    let mut still = None;
    for &t in tables.iter().filter(|&&t| t != back) {
        match anim_size(t)? {
            s if s == 2 * PIC_SIZE => front = front.or(Some(t)),
            s if s == PIC_SIZE => still = still.or(Some(t)),
            s => bail!("pic table at {t:#x}: species 1 is {s:#x} bytes"),
        }
    }
    let (Some(front), Some(still)) = (front, still) else {
        bail!("Pokémon front pic tables: need one 2-frame and one still table in {tables:x?}");
    };
    let coords_len = n * COORDS_SIZE;
    let (front_coords, back_coords) = (still - coords_len, back - coords_len);
    for (off, what) in [
        (front_coords, "gMonFrontPicCoords"),
        (back_coords, "gMonBackPicCoords"),
    ] {
        check_coords(rom, off, count, what)?;
        ensure!(
            rom.slice(off, 4)? == COORDS_NONE,
            "{what} at {off:#x}: SPECIES_NONE is not 64x64"
        );
    }
    let elevation_count = names::SPECIES
        .value("SPECIES_EGG")
        .context("SPECIES_EGG (NUM_SPECIES)")? as usize;
    let elev = rom.slice(elevation, elevation_count)?;
    ensure!(
        elev[0] == 0 && elev.iter().all(|&e| e < 64),
        "gEnemyMonElevation at {elevation:#x}: not elevations"
    );
    Ok(MonTables {
        count,
        front,
        still_front: still,
        back,
        palettes: back + 8 * n,
        shiny: back + 16 * n,
        front_coords,
        back_coords,
        elevation,
        elevation_count,
    })
}

impl MonTables {
    pub fn get(&self, rom: &Rom, species: u16) -> Result<MonPics> {
        let i = u32::from(species);
        ensure!(
            (species as usize) < self.count,
            "species {species} out of range"
        );
        let what = |t: &str| format!("species {species} {t}");
        let ptr = |table: u32, t: &str| rom.req_ptr(table + 8 * i, &what(t));
        let front = rom
            .lz77_at(ptr(self.front, "front pic")?)
            .with_context(|| what("front pic"))?;
        ensure!(
            front.len() >= PIC_SIZE,
            "{}: {} bytes",
            what("front pic"),
            front.len()
        );
        // Deoxys' back pic (and front) holds a second form after frame 0: keep frame 0.
        let back = rom
            .lz77_at(ptr(self.back, "back pic")?)
            .with_context(|| what("back pic"))?;
        ensure!(
            back.len() >= PIC_SIZE,
            "{}: {} bytes",
            what("back pic"),
            back.len()
        );
        let coord = |table: u32| rom.u8(table + COORDS_SIZE * i + 1);
        Ok(MonPics {
            front: front[..PIC_SIZE].to_vec(),
            back: back[..PIC_SIZE].to_vec(),
            palette: lz_palette(rom, ptr(self.palettes, "palette")?, &what("palette"))?,
            shiny: lz_palette(rom, ptr(self.shiny, "shiny")?, &what("shiny palette"))?,
            coords: MonCoords {
                front_y: coord(self.front_coords)?,
                back_y: coord(self.back_coords)?,
                elevation: if (species as usize) < self.elevation_count {
                    rom.u8(self.elevation + i)?
                } else {
                    0
                },
            },
        })
    }
}

/// Finds the trainer pic tables (`front_pic_tables.h`, `back_pic_tables.h`): each coords table
/// is followed by its pic table and then its palette table.
fn locate_trainers(rom: &Rom) -> Result<TrainerTables> {
    let find = |count: usize, size_ok: fn(u16) -> bool, what: &str| -> Result<u32> {
        let hits: Vec<u32> = sheet_tables(rom, count, size_ok)
            .into_iter()
            .filter(|&t| pal_run(rom, t + 8 * count as u32, 0) == count)
            .collect();
        let [t] = hits[..] else {
            bail!("{what} ({count} sheets + palettes): expected 1, found {hits:x?}");
        };
        let coords = t - COORDS_SIZE * count as u32;
        check_coords(rom, coords, count, &format!("{what} coords"))?;
        Ok(t)
    };
    let front_count = table_count(&names::TRAINER_PICS);
    let back_count = table_count(&names::TRAINER_BACK_PICS);
    // Front pics are 1 or 2 frames (`TRAINER_PIC_SIZE` or twice that); back pics are
    // uncompressed strips of 4-5 frames.
    let front = find(
        front_count,
        |s| s == 0x800 || s == 0x1000,
        "gTrainerFrontPicTable",
    )?;
    let back = find(
        back_count,
        |s| s >= 0x800 && s % 0x800 == 0 && s <= 0x4000,
        "gTrainerBackPicTable",
    )?;
    Ok(TrainerTables {
        front_count,
        front,
        front_palettes: front + 8 * front_count as u32,
        front_coords: front - COORDS_SIZE * front_count as u32,
        back_count,
        back,
        back_palettes: back + 8 * back_count as u32,
        back_coords: back - COORDS_SIZE * back_count as u32,
    })
}

impl TrainerTables {
    /// Frame 0 of front pic `pic` (`TRAINER_PIC_*`).
    pub fn front(&self, rom: &Rom, pic: u8) -> Result<TrainerPic> {
        ensure!(usize::from(pic) < self.front_count, "trainer pic {pic}");
        let i = u32::from(pic);
        let what = format!("trainer front pic {pic}");
        let data = rom.lz77_at(rom.req_ptr(self.front + 8 * i, &what)?)?;
        ensure!(data.len() >= PIC_SIZE, "{what}: {} bytes", data.len());
        let pal = rom.req_ptr(self.front_palettes + 8 * i, &what)?;
        let c = rom.slice(self.front_coords + COORDS_SIZE * i, 2)?;
        Ok(TrainerPic {
            tiles: data[..PIC_SIZE].to_vec(),
            frames: 1,
            palette: lz_palette(rom, pal, &what)?,
            size: c[0],
            y_offset: c[1],
        })
    }

    /// Every frame of back pic `pic` (`TRAINER_BACK_PIC_*`).
    pub fn back(&self, rom: &Rom, pic: u8) -> Result<TrainerPic> {
        ensure!(usize::from(pic) < self.back_count, "trainer back pic {pic}");
        let i = u32::from(pic);
        let what = format!("trainer back pic {pic}");
        let (data, size, _) = sheet(rom, self.back + 8 * i).with_context(|| what.clone())?;
        let pal = rom.req_ptr(self.back_palettes + 8 * i, &what)?;
        let c = rom.slice(self.back_coords + COORDS_SIZE * i, 2)?;
        Ok(TrainerPic {
            tiles: rom.slice(data, size.into())?.to_vec(),
            frames: (usize::from(size) / PIC_SIZE) as u8,
            palette: lz_palette(rom, pal, &what)?,
            size: c[0],
            y_offset: c[1],
        })
    }
}

/// `sBattleEnvironmentTable` (battle_bg.c): 10 `struct BattleBackground { tileset, tilemap,
/// entryTileset, entryTilemap, palette }`, every one an LZ77 stream.
fn locate_environments(rom: &Rom) -> Result<u32> {
    let entry_ok = |e: u32| -> bool {
        let p = |k: u32| lz_ptr(rom, e + 4 * k).map(|(_, s)| s);
        matches!(
            (p(0), p(1), p(2), p(3), p(4)),
            (Some(t), Some(0x1000), Some(a), Some(0x800), Some(0x60))
                if t > 0 && t % TILE_BYTES == 0 && a % TILE_BYTES == 0
        )
    };
    let len = rom.bytes().len() as u32;
    let hits: Vec<u32> = (0..len.saturating_sub(200))
        .step_by(4)
        .filter(|&o| (0..ENVIRONMENTS.len() as u32).all(|e| entry_ok(o + 20 * e)))
        .collect();
    let [t] = hits[..] else {
        bail!("sBattleEnvironmentTable: expected 1 match, found {hits:x?}");
    };
    Ok(t)
}

/// `gBattleTextbox{Tiles,Palette,Tilemap}` open graphics.c, back to back, and are referenced by
/// one literal pool (`LoadBattleTextboxAndBackground`): a 2-palette stream directly followed by
/// a 32x64 tilemap stream and preceded by the tiles, all three within a few pool words.
fn locate_textbox(rom: &Rom) -> Result<[u32; 3]> {
    const POOL: u32 = 24;
    let len = rom.bytes().len() as u32;
    let mut found = BTreeSet::new();
    for o in (0..len.saturating_sub(4)).step_by(4) {
        let Some((pal, 0x40)) = lz_ptr(rom, o) else {
            continue;
        };
        let Ok(map) = lz_end(rom, pal) else { continue };
        if lz_size(rom, map) != Some(0x1000) {
            continue;
        }
        let refs: Vec<(u32, usize)> = (o.saturating_sub(POOL)..=o + POOL)
            .step_by(4)
            .filter_map(|k| lz_ptr(rom, k))
            .collect();
        if !refs.iter().any(|&(t, _)| t == map) {
            continue;
        }
        for &(t, size) in &refs {
            if t < pal && size > 0 && size % TILE_BYTES == 0 && lz_end(rom, t).ok() == Some(pal) {
                found.insert([t, pal, map]);
            }
        }
    }
    let [t] = found.iter().copied().collect::<Vec<_>>()[..] else {
        bail!("battle textbox (tiles, palette, tilemap): expected 1, found {found:x?}");
    };
    Ok(t)
}

/// `gBattleWindowTextPalette` (`text.gbapal.lz`) and `gPPTextPalette` (`text_pp.gbapal`), back to
/// back in graphics.c: a one-palette LZ77 stream ending (4-aligned) where a raw palette starts,
/// both referenced by ROM pointer words. That shape alone matches hundreds of places (a
/// compressed palette followed by raw tiles), so the text palette must also be referenced from
/// battle_bg.c: `LoadBattleMenuWindowGfx` loads it, a few functions before
/// `LoadBattleTextboxAndBackground`, whose literal pool references `textbox_palette`.
fn locate_text_palettes(rom: &Rom, textbox_palette: u32) -> Result<[u32; 2]> {
    /// How far before the textbox pool battle_bg.c's earlier literal pools may sit.
    const NEAR: u32 = 0x1000;
    let len = rom.bytes().len() as u32;
    let words: Vec<(u32, u32)> = (0..len.saturating_sub(4))
        .step_by(4)
        .filter_map(|o| Some((o, rom.ptr(o).ok()??)))
        .collect();
    let targets: BTreeSet<u32> = words.iter().map(|&(_, t)| t).collect();
    let tb_refs: Vec<u32> = words
        .iter()
        .filter(|&&(_, t)| t == textbox_palette)
        .map(|&(o, _)| o)
        .collect();
    let near_textbox = |t: u32| {
        words
            .iter()
            .any(|&(o, w)| w == t && tb_refs.iter().any(|&r| r > o && r - o <= NEAR))
    };
    let hits: Vec<[u32; 2]> = targets
        .iter()
        .filter(|&&t| lz_size(rom, t) == Some(32))
        .filter_map(|&t| {
            let pp = lz_end(rom, t).ok()?;
            (targets.contains(&pp) && lz_size(rom, pp).is_none()).then_some([t, pp])
        })
        .filter(|&[t, _]| near_textbox(t))
        .collect();
    let [hit] = hits[..] else {
        bail!("battle text palettes (lz77 text, raw PP): expected 1, found {hits:x?}");
    };
    Ok(hit)
}

/// The healthbox sheets, healthbar sheets and their two palettes (battle_gfx_sfx_util.c), as
/// laid out from `sSpriteSheet_SinglesPlayerHealthbox`.
fn locate_healthbox(rom: &Rom) -> Result<(u32, u32, u32)> {
    let sheets: [(u16, u16); 11] = [
        (0x1000, TAG_HEALTHBOX_PLAYER1),
        (0x1000, TAG_HEALTHBOX_OPPONENT1),
        (0x800, TAG_HEALTHBOX_PLAYER1),
        (0x800, TAG_HEALTHBOX_PLAYER2),
        (0x800, TAG_HEALTHBOX_OPPONENT1),
        (0x800, TAG_HEALTHBOX_OPPONENT2),
        (0x1000, TAG_HEALTHBOX_SAFARI),
        (0x100, TAG_HEALTHBAR_PLAYER1),
        (0x120, TAG_HEALTHBAR_PLAYER1 + 1),
        (0x100, TAG_HEALTHBAR_PLAYER1 + 2),
        (0x120, TAG_HEALTHBAR_PLAYER1 + 3),
    ];
    let len = rom.bytes().len() as u32;
    let hits: Vec<u32> = (0..len.saturating_sub(13 * 8))
        .step_by(4)
        .filter(|&o| {
            sheets.iter().enumerate().all(|(i, &(size, tag))| {
                matches!(sheet(rom, o + 8 * i as u32), Some((_, s, t)) if s == size && t == tag)
            }) && pal_entry(rom, o + 88).map(|p| p.1) == Some(TAG_HEALTHBOX_PLAYER1)
                && pal_entry(rom, o + 96).map(|p| p.1) == Some(TAG_HEALTHBAR_PLAYER1)
        })
        .collect();
    let [o] = hits[..] else {
        bail!("healthbox sprite sheets: expected 1 match, found {hits:x?}");
    };
    let (box_pal, bar_pal) = (rom.req_ptr(o + 88, "pal")?, rom.req_ptr(o + 96, "pal")?);
    // graphics.c: BallStatusBarPal, BallDisplayPal, then gHealthboxElementsGfxTable.
    ensure!(
        bar_pal == box_pal + 32,
        "healthbox palettes {box_pal:#x} / {bar_pal:#x} are not adjacent"
    );
    Ok((o, box_pal, bar_pal))
}

/// `gBallSpriteSheets` / `gBallSpritePalettes` (pokeball.c), from their first two entries.
fn locate_ball(rom: &Rom) -> Result<(u32, u32)> {
    let len = rom.bytes().len() as u32;
    let size = BALL_GFX_SIZE as u16;
    let sheets: Vec<u32> = (0..len.saturating_sub(16))
        .step_by(4)
        .filter(|&o| {
            sheet(rom, o).is_some_and(|(_, s, t)| s == size && t == TAG_POKE_BALL)
                && sheet(rom, o + 8).is_some_and(|(_, s, t)| s == size && t == TAG_POKE_BALL + 1)
        })
        .collect();
    let pals: Vec<u32> = (0..len.saturating_sub(16))
        .step_by(4)
        .filter(|&o| {
            pal_entry(rom, o).is_some_and(|p| p.1 == TAG_POKE_BALL)
                && pal_entry(rom, o + 8).is_some_and(|p| p.1 == TAG_POKE_BALL + 1)
        })
        .collect();
    let ([s], [p]) = (&sheets[..], &pals[..]) else {
        bail!("ball sheets / palettes: expected 1 each, found {sheets:x?} / {pals:x?}");
    };
    Ok((*s, *p))
}

/// `sSpriteSheet_PokeballSelect`, `sSpriteSheet_StarterCircle` (each followed by an empty
/// terminator) and `sSpritePalettes_StarterChoose`.
fn locate_starter(rom: &Rom) -> Result<u32> {
    let len = rom.bytes().len() as u32;
    let zero = |o: u32| rom.slice(o, 8).is_ok_and(|b| b.iter().all(|&x| x == 0));
    let hits: Vec<u32> = (0..len.saturating_sub(48))
        .step_by(4)
        .filter(|&o| {
            sheet(rom, o).is_some_and(|(_, s, t)| s == 0x800 && t == TAG_POKEBALL_SELECT)
                && zero(o + 8)
                && sheet(rom, o + 16).is_some_and(|(_, s, t)| s == 0x800 && t == TAG_STARTER_CIRCLE)
                && zero(o + 24)
                && pal_entry(rom, o + 32).is_some_and(|p| p.1 == TAG_POKEBALL_SELECT)
                && pal_entry(rom, o + 40).is_some_and(|p| p.1 == TAG_STARTER_CIRCLE)
        })
        .collect();
    let [o] = hits[..] else {
        bail!("starter choose sprite sheets: expected 1 match, found {hits:x?}");
    };
    Ok(o)
}

/// Finds every table.
pub fn locate(rom: &Rom) -> Result<Located> {
    let trainers = locate_trainers(rom)?;
    // back_pic_tables.h ends with gTrainerBackPicPaletteTable; data.c includes
    // enemy_mon_elevation.h right after it.
    let elevation = trainers.back_palettes + 8 * trainers.back_count as u32;
    let mons = locate_mons(rom, elevation)?;
    let (healthbox_sheets, healthbox_palette, healthbar_palette) = locate_healthbox(rom)?;
    let (ball_sheets, ball_palettes) = locate_ball(rom)?;
    let textbox = locate_textbox(rom)?;
    Ok(Located {
        mons,
        trainers,
        environments: locate_environments(rom)?,
        textbox,
        text_palettes: locate_text_palettes(rom, textbox[1])?,
        healthbox_sheets,
        healthbox_palette,
        healthbar_palette,
        healthbox_elements: healthbar_palette + 32,
        ball_sheets,
        ball_palettes,
        starter_sheets: locate_starter(rom)?,
    })
}

fn obj(tiles: Vec<u8>, width: u16, palettes: Vec<[u16; 16]>) -> Gfx {
    Gfx {
        tiles,
        width,
        palettes,
        palette_bank: 0,
        tilemap: Vec::new(),
    }
}

/// Every battle-interface, environment and starter-screen graphic, by export path (without
/// extension), in a fixed order.
pub fn graphics(rom: &Rom, l: &Located) -> Result<Vec<(String, Gfx)>> {
    let mut out = Vec::new();
    let [tb_tiles, tb_pal, tb_map] = l.textbox;
    out.push((
        "battle/gfx/textbox".to_string(),
        Gfx {
            tiles: rom.lz77_at(tb_tiles)?,
            width: 16,
            palettes: {
                let mut p =
                    palettes_from(&lz_exact(rom, tb_pal, 0x40, "textbox palette")?, "textbox")?;
                let [text, pp] = l.text_palettes;
                p.push(lz_palette(rom, text, "gBattleWindowTextPalette")?);
                p.push(palette_at(rom, pp)?);
                p
            },
            palette_bank: 0,
            tilemap: u16s(&lz_exact(rom, tb_map, 0x1000, "textbox tilemap")?),
        },
    ));

    let box_pal = palette_at(rom, l.healthbox_palette)?;
    let bar_pal = palette_at(rom, l.healthbar_palette)?;
    let hb = |i: u32, size: usize, what: &str| -> Result<Vec<u8>> {
        lz_exact(
            rom,
            rom.req_ptr(l.healthbox_sheets + 8 * i, what)?,
            size,
            what,
        )
    };
    let player = hb(0, 0x1000, "healthbox_singles_player")?;
    let opponent = hb(1, 0x800, "healthbox_singles_opponent")?;
    out.push((
        "battle/gfx/healthbox_player".into(),
        obj(player, 8, vec![box_pal]),
    ));
    out.push((
        "battle/gfx/healthbox_opponent".into(),
        obj(opponent, 8, vec![box_pal]),
    ));
    let elems = rom.slice(l.healthbox_elements, ELEM_TILES * TILE_BYTES)?;
    let el = |parts: &[(usize, usize)]| -> Vec<u8> {
        parts
            .iter()
            .flat_map(|&(first, n)| &elems[first * TILE_BYTES..(first + n) * TILE_BYTES])
            .copied()
            .collect()
    };
    out.push((
        "battle/gfx/hpbar".into(),
        obj(el(&[ELEM_HPBAR, ELEM_HPBAR_ANIM]), 30, vec![bar_pal]),
    ));
    out.push((
        "battle/gfx/expbar".into(),
        obj(el(&[ELEM_EXPBAR]), 9, vec![box_pal]),
    ));
    out.push((
        "battle/gfx/status".into(),
        obj(el(&[ELEM_STATUS, ELEM_STATUS2]), 3, vec![box_pal]),
    ));
    out.push((
        "battle/gfx/misc".into(),
        obj(el(&[ELEM_MISC, ELEM_MISC_FRAMEEND]), 12, vec![box_pal]),
    ));
    let ball = lz_exact(
        rom,
        rom.req_ptr(l.ball_sheets, "ball gfx")?,
        BALL_GFX_SIZE,
        "gBallGfx_Poke",
    )?;
    let ball_pal = lz_palette(
        rom,
        rom.req_ptr(l.ball_palettes, "ball pal")?,
        "gBallPal_Poke",
    )?;
    out.push(("battle/gfx/ball".into(), obj(ball, 2, vec![ball_pal])));

    for (e, name) in ENVIRONMENTS.iter().enumerate() {
        let base = l.environments + 20 * e as u32;
        let what = format!("environment {name}");
        let p = |k: u32| rom.req_ptr(base + 4 * k, &what);
        out.push((
            format!("battle/env/{name}"),
            Gfx {
                tiles: rom.lz77_at(p(0)?).with_context(|| what.clone())?,
                width: 16,
                palettes: palettes_from(&lz_exact(rom, p(4)?, 0x60, &what)?, &what)?,
                palette_bank: 2,
                tilemap: u16s(&lz_exact(rom, p(1)?, 0x1000, &what)?),
            },
        ));
    }

    out.extend(starter(rom, l.starter_sheets)?);
    Ok(out)
}

/// starter_choose.c defines, in order: `gBirchBagGrass_Pal` (2 palettes, as `credits.c` loads
/// `2 * 16` colours), `sPokeballSelection_Pal`, `sStarterCircle_Pal`, then the LZ77 streams
/// `gBirchBagTilemap`, `gBirchGrassTilemap`, `gBirchBagGrass_Gfx`, `gPokeballSelection_Gfx`.
fn starter(rom: &Rom, o: u32) -> Result<Vec<(String, Gfx)>> {
    let ball_gfx = rom.req_ptr(o, "pokeball select gfx")?;
    let circle_gfx = rom.req_ptr(o + 16, "starter circle gfx")?;
    let ball_pal = rom.req_ptr(o + 32, "pokeball select palette")?;
    let circle_pal = rom.req_ptr(o + 40, "starter circle palette")?;
    ensure!(
        circle_pal == ball_pal + 32,
        "starter palettes {ball_pal:#x} / {circle_pal:#x} are not adjacent"
    );
    let bag_map = circle_pal + 32;
    let grass_map = lz_end(rom, bag_map).context("gBirchBagTilemap")?;
    let tiles = lz_end(rom, grass_map).context("gBirchGrassTilemap")?;
    ensure!(
        lz_end(rom, tiles).context("gBirchBagGrass_Gfx")? == ball_gfx,
        "starter tiles at {tiles:#x} are not followed by gPokeballSelection_Gfx {ball_gfx:#x}"
    );
    let bag_pal = rom.slice(ball_pal - 64, 64)?;
    Ok(vec![
        (
            "starter/bag".into(),
            Gfx {
                tiles: rom.lz77_at(tiles)?,
                width: 16,
                palettes: palettes_from(bag_pal, "gBirchBagGrass_Pal")?,
                palette_bank: 0,
                tilemap: u16s(&rom.lz77_at(bag_map)?),
            },
        ),
        (
            "starter/grass".into(),
            Gfx {
                tiles: Vec::new(),
                width: 16,
                palettes: Vec::new(),
                palette_bank: 0,
                tilemap: u16s(&rom.lz77_at(grass_map)?),
            },
        ),
        (
            "starter/ball".into(),
            obj(
                lz_exact(rom, ball_gfx, 0x800, "gPokeballSelection_Gfx")?,
                4,
                vec![palette_at(rom, ball_pal)?],
            ),
        ),
        (
            "starter/circle".into(),
            obj(
                lz_exact(rom, circle_gfx, 0x800, "sStarterCircle_Gfx")?,
                8,
                vec![palette_at(rom, circle_pal)?],
            ),
        ),
    ])
}

/// JASC-PAL text of one palette (8-bit channels `x * 255 / 31`, as `gfx`).
pub fn jasc_pal(p: &[u16; 16]) -> String {
    let mut s = String::from("JASC-PAL\r\n0100\r\n16\r\n");
    for &c in p {
        let [r, g, b, _] = gfx::rgb555_to_rgba(c);
        s.push_str(&format!("{r} {g} {b}\r\n"));
    }
    s
}

fn write_gfx(o: &mut Out, name: &str, g: &Gfx) -> Result<()> {
    let tile_count = g.tiles.len() / TILE_BYTES;
    if tile_count > 0 {
        let img = gfx::tiles_4bpp_to_image(&g.tiles, g.width.into())?;
        let pal = Palette(g.palettes.first().copied().unwrap_or_default());
        o.png_indexed(&format!("{name}.png"), &img, &pal)?;
    }
    o.ron(
        &format!("{name}.ron"),
        &GfxFile {
            tile_count: tile_count as u16,
            width: g.width,
            palettes: g.palettes.clone(),
            palette_bank: g.palette_bank,
            tilemap: to_rows(&g.tilemap, 32),
        },
    )
}

/// `frames` 64x64 frames of 64 tiles each, side by side.
fn frames_side_by_side(tiles: &[u8], frames: usize) -> Result<IndexedImage> {
    ensure!(
        tiles.len() == frames * PIC_SIZE,
        "{} pic bytes",
        tiles.len()
    );
    let w = 64 * frames;
    let mut pixels = vec![0u8; w * 64];
    for (k, frame) in tiles.chunks_exact(PIC_SIZE).enumerate() {
        let img = gfx::tiles_4bpp_to_image(frame, 8)?;
        for y in 0..64 {
            pixels[y * w + 64 * k..y * w + 64 * k + 64]
                .copy_from_slice(&img.pixels[y * 64..y * 64 + 64]);
        }
    }
    Ok(IndexedImage {
        width: w as u32,
        height: 64,
        pixels,
    })
}

/// Writes every battle graphic, the Pokémon and trainer pics and the small font under `o`.
pub fn export(rom: &Rom, o: &mut Out, provenance: &mut BTreeMap<String, Hex>) -> Result<()> {
    let l = locate(rom)?;
    for (name, g) in graphics(rom, &l)? {
        write_gfx(o, &name, &g)?;
    }
    let m = &l.mons;
    let t = &l.trainers;
    for (key, off) in [
        ("mon_front_pics", m.front),
        ("mon_back_pics", m.back),
        ("mon_palettes", m.palettes),
        ("mon_shiny_palettes", m.shiny),
        ("mon_front_coords", m.front_coords),
        ("mon_back_coords", m.back_coords),
        ("mon_elevation", m.elevation),
        ("trainer_front_pics", t.front),
        ("trainer_front_palettes", t.front_palettes),
        ("trainer_front_coords", t.front_coords),
        ("trainer_back_pics", t.back),
        ("trainer_back_palettes", t.back_palettes),
        ("trainer_back_coords", t.back_coords),
        ("environments", l.environments),
        ("textbox/tiles", l.textbox[0]),
        ("textbox/palette", l.textbox[1]),
        ("textbox/tilemap", l.textbox[2]),
        ("text_palette", l.text_palettes[0]),
        ("pp_text_palette", l.text_palettes[1]),
        ("healthbox_sheets", l.healthbox_sheets),
        ("healthbox_elements", l.healthbox_elements),
        ("ball_sheets", l.ball_sheets),
        ("ball_palettes", l.ball_palettes),
        ("starter_sheets", l.starter_sheets),
    ] {
        provenance.insert(format!("battle_gfx/{key}"), Hex(off));
    }

    let mut coords = MonCoordsFile::new();
    for id in 0..m.count as u16 {
        let name = names::SPECIES
            .name(id.into())
            .with_context(|| format!("species {id} has no name"))?;
        let p = m.get(rom, id)?;
        let pal = Palette(p.palette);
        let dir = format!("pokemon/{name}");
        o.png_indexed(
            &format!("{dir}/front.png"),
            &gfx::tiles_4bpp_to_image(&p.front, 8)?,
            &pal,
        )?;
        o.png_indexed(
            &format!("{dir}/back.png"),
            &gfx::tiles_4bpp_to_image(&p.back, 8)?,
            &pal,
        )?;
        o.bytes(&format!("{dir}/shiny.pal"), jasc_pal(&p.shiny).as_bytes())?;
        coords.insert(name.to_string(), p.coords);
    }
    o.ron("pokemon/coords.ron", &coords)?;

    let mut pics = TrainerPicsFile::default();
    for id in 0..t.front_count as u8 {
        let name = names::TRAINER_PICS
            .name(id.into())
            .context("trainer pic name")?;
        let p = t.front(rom, id)?;
        o.png_indexed(
            &format!("trainers/front/{name}.png"),
            &gfx::tiles_4bpp_to_image(&p.tiles, 8)?,
            &Palette(p.palette),
        )?;
        pics.front.insert(
            name.to_string(),
            TrainerPicInfo {
                y: p.y_offset,
                size: p.size,
            },
        );
    }
    for id in 0..t.back_count as u8 {
        let name = names::TRAINER_BACK_PICS
            .name(id.into())
            .context("trainer back pic name")?;
        let p = t.back(rom, id)?;
        o.png_indexed(
            &format!("trainers/back/{name}.png"),
            &frames_side_by_side(&p.tiles, p.frames.into())?,
            &Palette(p.palette),
        )?;
        pics.back.insert(name.to_string(), p.frames);
        pics.back_coords.insert(
            name.to_string(),
            TrainerPicInfo {
                y: p.y_offset,
                size: p.size,
            },
        );
    }
    o.ron("trainers/pics.ron", &pics)?;

    let (glyphs, widths) = font::locate_latin_small(rom)?;
    let f = font::read_latin_at(rom, glyphs, widths)?;
    o.bytes("font/latin_small.png", &export::font_png(&f)?)?;
    o.ron(
        "font/latin_small.ron",
        &FontFile {
            glyphs: f.glyphs.len() as u16,
            glyphs_per_row: export::FONT_GLYPHS_PER_ROW as u16,
            widths: f.widths.clone(),
        },
    )?;
    provenance.insert("font_small/glyphs".to_string(), Hex(glyphs));
    provenance.insert("font_small/widths".to_string(), Hex(widths));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_with(tail: &[u8]) -> Rom {
        let mut b = vec![0u8; 0x100];
        b[0xAC..0xB0].copy_from_slice(b"BPEF");
        b.extend_from_slice(tail);
        Rom::from_bytes(b).unwrap()
    }

    #[test]
    fn lz_end_counts_literals_and_references_and_aligns() {
        // "ab" then a 3-byte back-reference: 4 header + 1 flag + 2 literals + 2 ref = 9 -> 12.
        let rom = rom_with(&[0x10, 5, 0, 0, 0x20, b'a', b'b', 0x00, 0x01, 0xEE]);
        assert_eq!(lz_end(&rom, 0x100).unwrap(), 0x10C);
        assert_eq!(rom.lz77_at(0x100).unwrap(), b"ababa");
        assert!(lz_end(&rom, 0x101).is_err());
    }

    #[test]
    fn sheet_tables_need_the_exact_count() {
        // Three sheets tagged 0, 1, 2 (size 0x800), then a palette-like entry.
        let mut tail = Vec::new();
        for i in 0..3u16 {
            tail.extend_from_slice(&(crate::ROM_BASE + 0x100).to_le_bytes());
            tail.extend_from_slice(&0x800u16.to_le_bytes());
            tail.extend_from_slice(&i.to_le_bytes());
        }
        let rom = rom_with(&tail);
        assert_eq!(sheet_tables(&rom, 3, |s| s == 0x800), vec![0x100]);
        assert!(sheet_tables(&rom, 2, |s| s == 0x800).is_empty());
        assert_eq!(sheet_run(&rom, 0x100, &|s| s == 0x800), 3);
    }

    #[test]
    fn jasc_round_trips_through_the_reader() {
        let mut p = [0u16; 16];
        for (i, c) in p.iter_mut().enumerate() {
            *c = (i as u16 * 2) | (31 - i as u16) << 5 | (i as u16) << 10;
        }
        let path = std::env::temp_dir().join(format!("battle-gfx-{}.pal", std::process::id()));
        std::fs::write(&path, jasc_pal(&p)).unwrap();
        assert_eq!(gfx::read_jasc_pal(&path).unwrap().0, p);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn frames_are_placed_side_by_side() {
        let mut tiles = vec![0u8; 2 * PIC_SIZE];
        tiles[PIC_SIZE] = 0x0F; // frame 1, pixel (0, 0) = 15
        let img = frames_side_by_side(&tiles, 2).unwrap();
        assert_eq!((img.width, img.height), (128, 64));
        assert_eq!(img.pixels[64], 15);
        assert!(frames_side_by_side(&tiles, 1).is_err());
    }
}
