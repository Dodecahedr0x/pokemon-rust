//! Writes `content/extracted/` in the format of [`crate::schema`]: maps, layouts, tilesets,
//! sprites, field-effect emotes, font, window graphics and (with [`ExportOptions::previews`])
//! previews, all named by decomp identifiers. Texts and scripts are
//! written by [`scripts::export_scripts`] (Milestone 3 Task B; M8 Task B owns
//! `export/scripts.rs`), the item, multichoice, std-string and map-section tables by
//! [`tables::export_tables`] (M8 Task C owns `export/tables.rs`).
//!
//! Output is deterministic: everything is ordered (`BTreeMap`/`BTreeSet`, fixed map order) and
//! the RON style is fixed.

use crate::gfx::{self, IndexedImage, Palette, TILE_BYTES};
use crate::maps::{self, BgEventKind, Direction, MapHeader, MapId, TilesetHeaders};
use crate::render::{self, NUM_PALS_IN_PRIMARY, TilesetData};
use crate::roots::{self, MapScriptTable};
use crate::schema::{
    self, Bg, BgKind, Connection, ConnectionDir, Coord, FieldEffectSpriteFile, FontFile, Header,
    Hex, Layout, LayoutFile, Manifest, MapFile, MapScripts, Object, SpriteFile, TilesetFile,
    VarScript, Warp, WindowFile,
};
use crate::sprites::{self, ObjectGraphics, ObjectGraphicsTable};
use crate::{GameCode, Rom, battle_data, battle_gfx, font, names, window};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub mod scripts;
pub mod tables;

pub use scripts::ScriptInput;

/// The slice's 15 maps (decomp directory names), in the Milestone 3-7 export order: the named
/// fixed sample of tests and tools (`cargo xtask extract --maps slice`). This order decides
/// canonical script names (see `schema.rs`).
pub const SLICE_MAPS: [&str; 15] = [
    "LittlerootTown",
    "Route101",
    "OldaleTown",
    "Route102",
    "Route103",
    "LittlerootTown_BrendansHouse_1F",
    "LittlerootTown_BrendansHouse_2F",
    "LittlerootTown_MaysHouse_1F",
    "LittlerootTown_MaysHouse_2F",
    "LittlerootTown_ProfessorBirchsLab",
    "OldaleTown_House1",
    "OldaleTown_House2",
    "OldaleTown_PokemonCenter_1F",
    "OldaleTown_PokemonCenter_2F",
    "OldaleTown_Mart",
];

/// What [`export`] writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// Maps to export (decomp directory names), in export order.
    pub maps: Vec<&'static str>,
    /// Whether to write `previews/<Map>.png`.
    pub previews: bool,
}

impl ExportOptions {
    /// [`SLICE_MAPS`], with or without previews.
    pub fn slice(previews: bool) -> ExportOptions {
        ExportOptions {
            maps: SLICE_MAPS.to_vec(),
            previews,
        }
    }

    /// Every `names::MAPS` entry, in `gMapGroups` order.
    pub fn all(previews: bool) -> ExportOptions {
        ExportOptions {
            maps: names::MAPS.iter().map(|m| m.dir).collect(),
            previews,
        }
    }
}

impl Default for ExportOptions {
    /// [`SLICE_MAPS`], without previews. Every map exports and round-trips
    /// ([`ExportOptions::all`]), but the content build cannot consume all of them yet (Milestone 8
    /// Task E); Task L makes `all` the default once it does.
    fn default() -> ExportOptions {
        ExportOptions::slice(false)
    }
}

/// Sprites exported even if no exported map places them: the player characters. Emerald has no
/// separate running sprite: Brendan's and May's run frames are frames 9-17 of `*_NORMAL`.
pub const PLAYER_SPRITES: [&str; 2] = ["OBJ_EVENT_GFX_BRENDAN_NORMAL", "OBJ_EVENT_GFX_MAY_NORMAL"];

const GFX_PREFIX: &str = "OBJ_EVENT_GFX_";
/// First `VAR_OBJ_GFX_ID_*` var; the 16 of them select the `OBJ_EVENT_GFX_VAR_*` graphics.
const VAR_OBJ_GFX_ID_0: u16 = 0x4010;
/// `setvar` (`data/script_cmd_table.inc`).
const OP_SETVAR: u8 = 0x16;
/// `setmaplayoutindex` (`data/script_cmd_table.inc`).
const OP_SETMAPLAYOUTINDEX: u8 = 0xA7;
/// `VARS_START`: `VarGet` operands from here on name a var.
const FIRST_VAR: u16 = 0x4000;
/// gbagfx's font palette (`gFontPalette` in `tools/gbagfx/font.c`).
const FONT_PALETTE: [[u8; 3]; 4] = [
    [0x90, 0xC8, 0xFF],
    [0x38, 0x38, 0x38],
    [0xD8, 0xD8, 0xD8],
    [0xFF, 0xFF, 0xFF],
];
/// Glyphs per row of `latin_normal.png` (256 px wide like the decomp's).
pub(crate) const FONT_GLYPHS_PER_ROW: usize = 16;
const TILESET_TILES_PER_ROW: u32 = 16;

/// The fixed RON style of every exported file, with a trailing newline.
pub fn to_ron<T: Serialize>(value: &T) -> Result<String> {
    let config = ron::ser::PrettyConfig::new()
        .new_line("\n".to_string())
        .indentor("    ".to_string())
        .struct_names(false);
    let mut s = ron::ser::to_string_pretty(value, config)?;
    s.push('\n');
    Ok(s)
}

/// Creates files under `root` and records their sha1.
pub struct Out {
    root: PathBuf,
    files: BTreeMap<String, String>,
}

impl Out {
    /// Writes under `root` (created as needed) and records nothing yet.
    pub fn new(root: &Path) -> Out {
        Out {
            root: root.to_path_buf(),
            files: BTreeMap::new(),
        }
    }

    /// Relative path -> sha1 of every file written so far.
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    fn path(&self, rel: &str) -> Result<PathBuf> {
        let p = self.root.join(rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        Ok(p)
    }

    fn record(&mut self, rel: &str) -> Result<()> {
        let p = self.root.join(rel);
        let bytes = std::fs::read(&p).with_context(|| format!("reading back {}", p.display()))?;
        self.files.insert(
            rel.to_string(),
            sha1_smol::Sha1::from(&bytes).digest().to_string(),
        );
        Ok(())
    }

    /// Writes `data` to `rel` (relative, `/`-separated) and records its sha1 in the manifest.
    pub fn bytes(&mut self, rel: &str, data: &[u8]) -> Result<()> {
        let p = self.path(rel)?;
        std::fs::write(&p, data).with_context(|| format!("writing {}", p.display()))?;
        self.record(rel)
    }

    /// Writes `value` in the fixed RON style.
    pub fn ron<T: Serialize>(&mut self, rel: &str, value: &T) -> Result<()> {
        self.bytes(rel, to_ron(value)?.as_bytes())
    }

    /// Writes a 4-bit indexed PNG with `pal`.
    pub fn png_indexed(&mut self, rel: &str, img: &IndexedImage, pal: &Palette) -> Result<()> {
        gfx::write_png_indexed(&self.path(rel)?, img, pal)?;
        self.record(rel)
    }

    fn png_rgba(&mut self, rel: &str, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
        gfx::write_png_rgba(&self.path(rel)?, w, h, rgba)?;
        self.record(rel)
    }
}

/// One exported map as read from the ROM.
pub struct ExportedMap {
    /// Decomp directory name.
    pub name: &'static str,
    pub id: MapId,
    pub header: MapHeader,
    /// [`roots::named_roots`] of the map.
    pub roots: Vec<(String, u32)>,
}

/// Exports `opts.maps`, their layouts (and those their scripts switch to), the tilesets of those
/// layouts, the sprites they use, the field-effect emotes, the font, the window graphics, the
/// battle data, the scripts and the tables from `rom` into `out`, which is created if needed
/// (existing files are overwritten, others left alone).
pub fn export(rom: &Rom, out: &Path, opts: &ExportOptions) -> Result<Manifest> {
    let mut o = Out {
        root: out.to_path_buf(),
        files: BTreeMap::new(),
    };
    let mut provenance = BTreeMap::new();
    let table = maps::locate_map_table(rom)?;
    provenance.insert("map_table".to_string(), Hex(table.offset));
    let ts_headers = TilesetHeaders::locate(rom, &table)?;
    provenance.insert("tileset_headers".to_string(), Hex(ts_headers.offset));
    let layout_table = maps::locate_layout_table(rom, &table)?;
    provenance.insert("layout_table".to_string(), Hex(layout_table.offset));

    let mut exported = Vec::new();
    for &name in &opts.maps {
        let m = names::map_by_dir(name).with_context(|| format!("map {name} is not in names"))?;
        let id = MapId {
            group: m.group,
            num: m.num,
        };
        let header = table.header(rom, id).with_context(|| name.to_string())?;
        let roots = roots::named_roots(rom, &header, name)?;
        exported.push(ExportedMap {
            name,
            id,
            header,
            roots,
        });
    }
    let canonical = roots::canonical(exported.iter().flat_map(|m| &m.roots));
    for (off, script) in &canonical {
        provenance.insert(format!("script/{script}"), Hex(*off));
    }

    // Script walk: unknown opcodes per root, `setvar VAR_OBJ_GFX_ID_*` graphics and
    // `setmaplayoutindex` layouts.
    let mut unknown_script_ops = BTreeMap::new();
    let mut gfx_ids: BTreeSet<u8> = BTreeSet::new();
    let mut layout_ids: BTreeSet<u16> = exported.iter().map(|m| m.header.layout_id).collect();
    for (&off, script) in &canonical {
        let scan = crate::scripts::walk(rom, &[off]).with_context(|| format!("script {script}"))?;
        if !scan.unknown.is_empty() {
            let ops = scan.unknown.iter().map(|(&k, &v)| (Hex(k), v)).collect();
            unknown_script_ops.insert(script.clone(), ops);
        }
        gfx_ids.extend(var_gfx_ids(rom, &scan).with_context(|| format!("script {script}"))?);
        layout_ids.extend(set_layout_ids(rom, &scan).with_context(|| format!("script {script}"))?);
    }

    // Layouts by id, and the tilesets they use, keyed by header index (headers.h order).
    let mut layouts: BTreeMap<u16, maps::MapLayout> = BTreeMap::new();
    for &id in &layout_ids {
        layouts.insert(id, layout_table.layout(rom, id)?);
    }
    let mut tilesets: BTreeMap<u32, (&'static str, maps::TilesetRef, TilesetData)> =
        BTreeMap::new();
    for (&id, l) in &layouts {
        for off in [l.primary_tileset, l.secondary_tileset] {
            let name = ts_headers.name(off)?;
            let index = names::TILESETS.value(name).expect("named tileset");
            if let std::collections::btree_map::Entry::Vacant(e) = tilesets.entry(index) {
                let r = maps::read_tileset(rom, off, &table)
                    .with_context(|| format!("layout {id}: tileset {name}"))?;
                let data = render::load_tileset(rom, &r)?;
                e.insert((name, r, data));
            }
        }
    }
    let tileset_by_offset: BTreeMap<u32, (&str, &TilesetData)> = tilesets
        .values()
        .map(|(n, r, d)| (r.offset, (*n, d)))
        .collect();
    for (&id, l) in &layouts {
        let name = maps::LayoutTable::name(id)?;
        let file = LayoutFile {
            id: names::LAYOUTS.name_or_hex(id.into()),
            layout: layout(
                l,
                tileset_by_offset[&l.primary_tileset].0,
                tileset_by_offset[&l.secondary_tileset].0,
            )
            .with_context(|| format!("layout {name}"))?,
        };
        o.ron(&format!("layouts/{name}.ron"), &file)?;
        provenance.insert(format!("layout/{name}"), Hex(l.offset));
    }

    let exported_names: BTreeMap<MapId, &str> = exported.iter().map(|m| (m.id, m.name)).collect();
    for m in &exported {
        let l = &m.header.layout;
        let (primary, p_data) = tileset_by_offset[&l.primary_tileset];
        let (secondary, s_data) = tileset_by_offset[&l.secondary_tileset];
        if opts.previews {
            let rgba = render::render_map(l, p_data, s_data)?;
            o.png_rgba(
                &format!("previews/{}.png", m.name),
                u32::from(l.width) * 16,
                u32::from(l.height) * 16,
                &rgba,
            )?;
        }
        let map_scripts = match m.header.map_scripts {
            Some(t) => {
                provenance.insert(format!("map/{}/map_scripts", m.name), Hex(t));
                roots::read_map_scripts(rom, t)?
            }
            None => MapScriptTable::default(),
        };
        let file = map_file(
            m,
            primary,
            secondary,
            &map_scripts,
            &canonical,
            &exported_names,
        )?;
        o.ron(&format!("maps/{}.ron", m.name), &file)?;
        provenance.insert(format!("map/{}/header", m.name), Hex(m.header.offset));
        provenance.insert(format!("map/{}/layout", m.name), Hex(l.offset));
    }

    for (name, r, data) in tilesets.values() {
        write_tileset(&mut o, name, r.secondary, data)?;
        for (part, off) in [
            ("", r.offset),
            ("/tiles", r.tiles),
            ("/palettes", r.palettes),
            ("/metatiles", r.metatiles),
            ("/attributes", r.metatile_attributes),
        ] {
            provenance.insert(format!("tileset/{name}{part}"), Hex(off));
        }
    }

    for p in PLAYER_SPRITES {
        gfx_ids.insert(names::OBJ_EVENT_GFX.value(p).expect("player gfx") as u8);
    }
    for m in &exported {
        gfx_ids.extend(m.header.events.objects.iter().map(|ob| ob.graphics_id));
    }

    let gfx_table = sprites::locate_object_graphics(rom)?;
    provenance.insert("object_graphics_table".to_string(), Hex(gfx_table.offset));
    provenance.insert(
        "object_graphics_palettes".to_string(),
        Hex(gfx_table.palettes_offset),
    );
    let mut sprite_names = Vec::new();
    let mut skipped_graphics = BTreeSet::new();
    for id in gfx_ids {
        if u16::from(id) >= gfx_table.count {
            skipped_graphics.insert(names::gfx_name(id));
            continue;
        }
        let g = gfx_table.get(rom, id)?;
        let (img, file) = sprite(&g)?;
        o.png_indexed(
            &format!("sprites/{}.png", file.name),
            &img,
            &Palette(g.palette),
        )?;
        o.ron(&format!("sprites/{}.ron", file.name), &file)?;
        provenance.insert(format!("sprite/{}", file.name), Hex(g.offset));
        sprite_names.push(file.name);
    }
    write_emotes(rom, &gfx_table, &mut o, &mut provenance)?;

    let (glyphs_off, widths_off) = font::locate_latin_normal(rom)?;
    let f = font::read_latin_normal_at(rom, glyphs_off, widths_off)?;
    o.bytes("font/latin_normal.png", &font_png(&f)?)?;
    o.ron(
        "font/latin_normal.ron",
        &FontFile {
            glyphs: f.glyphs.len() as u16,
            glyphs_per_row: FONT_GLYPHS_PER_ROW as u16,
            widths: f.widths.clone(),
        },
    )?;
    provenance.insert("font/glyphs".to_string(), Hex(glyphs_off));
    provenance.insert("font/widths".to_string(), Hex(widths_off));

    let mb = window::locate_message_box(rom)?;
    let img = gfx::tiles_4bpp_to_image(&mb.tiles, window::TILE_COUNT as u32)?;
    o.png_indexed("window/message_box.png", &img, &Palette(mb.palette))?;
    o.ron(
        "window/message_box.ron",
        &WindowFile {
            tile_count: window::TILE_COUNT as u16,
            palette: mb.palette,
            text_fg: window::TEXT_FG,
            text_shadow: window::TEXT_SHADOW,
            text_bg: window::TEXT_BG,
        },
    )?;
    provenance.insert("window/tiles".to_string(), Hex(mb.tiles_offset));
    provenance.insert("window/palette".to_string(), Hex(mb.palette_offset));

    let battle = battle_data::extract(rom, &opts.maps).context("battle data")?;
    battle.write(&mut o)?;
    provenance.extend(battle.provenance);
    battle_gfx::export(rom, &mut o, &mut provenance)?;

    scripts::export_scripts(
        rom,
        &ScriptInput {
            maps: &exported,
            canonical: &canonical,
        },
        &mut o,
    )?;
    tables::export_tables(rom, &mut o, &mut provenance)?;

    let manifest = Manifest {
        format: schema::FORMAT_VERSION,
        format_minor: schema::FORMAT_MINOR,
        game_code: match rom.code() {
            GameCode::Bpee => "BPEE",
            GameCode::Bpef => "BPEF",
        }
        .to_string(),
        sha1: rom.sha1_hex(),
        language: rom.lang().code().to_string(),
        maps: opts.maps.iter().map(|m| m.to_string()).collect(),
        tilesets: tilesets.values().map(|(n, _, _)| n.to_string()).collect(),
        sprites: sprite_names,
        provenance,
        skipped_graphics,
        unknown_script_ops,
        files: o.files.clone(),
    };
    let path = o.path("manifest.ron")?;
    std::fs::write(&path, to_ron(&manifest)?)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(manifest)
}

/// `sprites/field_effects/<Name>.{png,ron}` for each of [`sprites::EMOTES`].
fn write_emotes(
    rom: &Rom,
    objects: &ObjectGraphicsTable,
    o: &mut Out,
    provenance: &mut BTreeMap<String, Hex>,
) -> Result<()> {
    let emotes = sprites::locate_emotes(rom)?;
    let palette = emotes.palette(rom, objects)?;
    provenance.insert("emotes".to_string(), Hex(emotes.offset));
    for p in &emotes.pics {
        let img = gfx::tiles_4bpp_to_image(&p.tiles, u32::from(p.width) / 8)
            .with_context(|| format!("emote {}", p.name))?;
        o.png_indexed(
            &format!("sprites/field_effects/{}.png", p.name),
            &img,
            &Palette(palette),
        )?;
        o.ron(
            &format!("sprites/field_effects/{}.ron", p.name),
            &FieldEffectSpriteFile {
                name: p.name.to_string(),
                width: p.width,
                height: p.height,
                frames: 1,
                palette,
            },
        )?;
        provenance.insert(format!("sprite/field_effects/{}", p.name), Hex(p.offset));
    }
    Ok(())
}

/// Layout ids a walk switches to through `setmaplayoutindex <LAYOUT_*>`. The operand goes through
/// `VarGet`, so a var (0x4000 and up) is possible; Emerald never uses one, and one is skipped.
pub fn set_layout_ids(rom: &Rom, scan: &crate::scripts::ScriptScan) -> Result<BTreeSet<u16>> {
    let mut ids = BTreeSet::new();
    for &pc in &scan.visited {
        if rom.u8(pc)? == OP_SETMAPLAYOUTINDEX {
            let id = rom.u16(pc + 1)?;
            if id < FIRST_VAR {
                ids.insert(id);
            }
        }
    }
    Ok(ids)
}

/// The canonical root name of the script at `off`, `None` for a null script.
/// Graphics ids a walk sets through `setvar VAR_OBJ_GFX_ID_<n>, <gfx>` (the values that
/// `OBJ_EVENT_GFX_VAR_<n>` objects resolve to).
pub fn var_gfx_ids(rom: &Rom, scan: &crate::scripts::ScriptScan) -> Result<BTreeSet<u8>> {
    let mut ids = BTreeSet::new();
    for &pc in &scan.visited {
        if rom.u8(pc)? == OP_SETVAR
            && (VAR_OBJ_GFX_ID_0..VAR_OBJ_GFX_ID_0 + 16).contains(&rom.u16(pc + 1)?)
        {
            let value = rom.u16(pc + 3)?;
            ids.insert(
                u8::try_from(value)
                    .with_context(|| format!("setvar of graphics id {value} at {pc:#x}"))?,
            );
        }
    }
    Ok(ids)
}

fn script_name(canonical: &BTreeMap<u32, String>, off: Option<u32>) -> Result<Option<String>> {
    off.map(|o| {
        canonical
            .get(&o)
            .cloned()
            .with_context(|| format!("script {o:#x} is not a named root"))
    })
    .transpose()
}

/// `"<group>.<num>"`.
fn map_id_string(id: MapId) -> String {
    format!("{}.{}", id.group, id.num)
}

/// The schema `MapFile` of an exported map. `exported` names the maps that are exported, the
/// only ones warps and connections refer to by name.
pub fn map_file(
    m: &ExportedMap,
    primary: &str,
    secondary: &str,
    map_scripts: &MapScriptTable,
    canonical: &BTreeMap<u32, String>,
    exported: &BTreeMap<MapId, &str>,
) -> Result<MapFile> {
    let h = &m.header;
    let l = &h.layout;
    let ev = &h.events;
    let target = |id: MapId| exported.get(&id).map(|n| n.to_string());
    let hidden_items_start = names::FLAGS
        .value("FLAG_HIDDEN_ITEMS_START")
        .expect("FLAG_HIDDEN_ITEMS_START") as u16;
    let bgs = ev
        .bgs
        .iter()
        .map(|b| {
            let kind = match b.kind {
                BgEventKind::Script(s) => BgKind::Script {
                    facing: b.kind_raw,
                    script: script_name(canonical, s)?,
                },
                BgEventKind::HiddenItem {
                    item,
                    hidden_item_id,
                } => BgKind::HiddenItem {
                    item: names::item_name(item),
                    flag: names::flag_name(
                        hidden_items_start
                            .checked_add(hidden_item_id)
                            .with_context(|| {
                                format!(
                                    "{}: hidden item id {hidden_item_id} at ({}, {}) overflows the \
                                     flag range",
                                    m.name, b.x, b.y
                                )
                            })?,
                    ),
                },
                BgEventKind::SecretBase(id) => BgKind::SecretBase { id },
            };
            Ok(Bg {
                x: b.x as i16,
                y: b.y as i16,
                elevation: b.elevation,
                kind,
            })
        })
        .collect::<Result<_>>()?;
    let var_scripts = |list: &[roots::VarScript]| -> Result<Vec<VarScript>> {
        list.iter()
            .map(|v| {
                Ok(VarScript {
                    var: names::var_name(v.var),
                    value: v.value,
                    script: script_name(canonical, Some(v.script))?.expect("non-null"),
                })
            })
            .collect()
    };
    Ok(MapFile {
        name: m.name.to_string(),
        group: m.id.group,
        num: m.id.num,
        layout: layout(l, primary, secondary).with_context(|| m.name.to_string())?,
        header: Header {
            music: h.music,
            region_map_section: h.region_map_section,
            cave: h.cave,
            weather: h.weather,
            map_type: h.map_type,
            allow_cycling: h.allow_cycling,
            allow_escaping: h.allow_escaping,
            allow_running: h.allow_running,
            show_map_name: h.show_map_name,
            battle_type: h.battle_type,
        },
        objects: ev
            .objects
            .iter()
            .map(|ob| {
                Ok(Object {
                    local_id: ob.local_id,
                    gfx: names::gfx_name(ob.graphics_id),
                    x: ob.x,
                    y: ob.y,
                    elevation: ob.elevation,
                    movement: names::movement_type_name(ob.movement_type),
                    range_x: ob.movement_range_x,
                    range_y: ob.movement_range_y,
                    trainer_type: names::trainer_type_name(ob.trainer_type),
                    trainer_sight_or_berry_tree: ob.trainer_range_berry_tree_id,
                    script: script_name(canonical, ob.script)?,
                    flag: (ob.flag_id != 0).then(|| names::flag_name(ob.flag_id)),
                })
            })
            .collect::<Result<_>>()?,
        warps: ev
            .warps
            .iter()
            .map(|w| Warp {
                x: w.x,
                y: w.y,
                elevation: w.elevation,
                dest_map: target(w.dest),
                dest_map_id: map_id_string(w.dest),
                dest_warp: w.warp_id,
            })
            .collect(),
        coords: ev
            .coords
            .iter()
            .map(|c| {
                Ok(Coord {
                    x: c.x,
                    y: c.y,
                    elevation: c.elevation,
                    var: (c.trigger_var != 0).then(|| names::var_name(c.trigger_var)),
                    value: c.index,
                    script: script_name(canonical, c.script)?,
                })
            })
            .collect::<Result<_>>()?,
        bgs,
        connections: h
            .connections
            .iter()
            .map(|c| Connection {
                dir: match c.direction {
                    Direction::South => ConnectionDir::South,
                    Direction::North => ConnectionDir::North,
                    Direction::West => ConnectionDir::West,
                    Direction::East => ConnectionDir::East,
                    Direction::Dive => ConnectionDir::Dive,
                    Direction::Emerge => ConnectionDir::Emerge,
                },
                offset: c.offset,
                map: target(c.map),
                map_id: map_id_string(c.map),
            })
            .collect(),
        scripts: MapScripts {
            on_load: script_name(canonical, map_scripts.on_load)?,
            on_transition: script_name(canonical, map_scripts.on_transition)?,
            on_resume: script_name(canonical, map_scripts.on_resume)?,
            on_return_to_field: script_name(canonical, map_scripts.on_return_to_field)?,
            on_dive_warp: script_name(canonical, map_scripts.on_dive_warp)?,
            on_frame: var_scripts(&map_scripts.on_frame)?,
            on_warp_into: var_scripts(&map_scripts.on_warp_into)?,
        },
    })
}

/// The schema `Layout` of `l`, with its tilesets' names.
fn layout(l: &maps::MapLayout, primary: &str, secondary: &str) -> Result<Layout> {
    ensure!(
        l.border.len() == 4 && l.blocks.len() == usize::from(l.width) * usize::from(l.height),
        "bad layout size"
    );
    Ok(Layout {
        width: l.width,
        height: l.height,
        primary: primary.to_string(),
        secondary: secondary.to_string(),
        border: schema::to_rows(&l.border, 2),
        blocks: schema::to_rows(&l.blocks, l.width.into()),
    })
}

/// `tilesets/<name>.{png,ron}`. The PNG holds the tileset's tiles with its first used palette:
/// 0 for a primary tileset, [`NUM_PALS_IN_PRIMARY`] (6) for a secondary one.
fn write_tileset(o: &mut Out, name: &str, secondary: bool, data: &TilesetData) -> Result<()> {
    let img = gfx::tiles_4bpp_to_image(&data.tiles, TILESET_TILES_PER_ROW)
        .with_context(|| format!("tileset {name}"))?;
    let first = if secondary { NUM_PALS_IN_PRIMARY } else { 0 };
    let pal = data
        .palettes
        .get(first)
        .with_context(|| format!("tileset {name} has no palette {first}"))?;
    o.png_indexed(&format!("tilesets/{name}.png"), &img, &Palette(*pal))?;
    o.ron(
        &format!("tilesets/{name}.ron"),
        &TilesetFile {
            name: name.to_string(),
            secondary,
            tile_count: (data.tiles.len() / TILE_BYTES) as u16,
            palettes: data.palettes.clone(),
            metatiles: data.metatiles.clone(),
            attributes: data.attributes.clone(),
        },
    )
}

/// The sprite sheet and its `.ron` for one graphics id.
///
/// Frames sit side by side in cells of the info's width and the tallest frame's height, 8x8
/// tiles row-major. A frame keeps the info's width and is `2 * bytes / width` pixels tall, so a
/// pic table mixing frame sizes (berry trees: 16x16 sprouts, 16x32 trees) draws each frame at
/// the top-left of its cell, the rest of the cell index 0. `SpriteFile::frame_sizes` records
/// each frame's size.
pub fn sprite(g: &ObjectGraphics) -> Result<(IndexedImage, SpriteFile)> {
    let w = u32::from(g.width);
    let ctx = || format!("object graphics id {} at {:#x}", g.id, g.offset);
    ensure!(
        w > 0 && w.is_multiple_of(8) && g.height > 0 && g.height.is_multiple_of(8),
        "{}: bad size {w}x{}",
        ctx(),
        g.height
    );
    let mut frame_sizes = Vec::with_capacity(g.frames.len());
    for (k, frame) in g.frames.iter().enumerate() {
        let h = 2 * frame.len() as u32 / w;
        ensure!(
            h > 0 && h.is_multiple_of(8) && (w * h / 2) as usize == frame.len(),
            "{}: frame {k} is {} bytes, not whole {w}-pixel-wide tile rows",
            ctx(),
            frame.len()
        );
        frame_sizes.push((g.width, h as u16));
    }
    let cell_h = frame_sizes
        .iter()
        .map(|&(_, h)| u32::from(h))
        .max()
        .unwrap_or(0);
    let sheet_w = w * g.frames.len() as u32;
    let mut pixels = vec![0u8; (sheet_w * cell_h) as usize];
    for (k, frame) in g.frames.iter().enumerate() {
        let img = gfx::tiles_4bpp_to_image(frame, w / 8).with_context(ctx)?;
        for y in 0..img.height {
            let src = (y * w) as usize;
            let dst = (y * sheet_w + k as u32 * w) as usize;
            pixels[dst..dst + w as usize].copy_from_slice(&img.pixels[src..src + w as usize]);
        }
    }
    let img = IndexedImage {
        width: sheet_w,
        height: cell_h,
        pixels,
    };
    let gfx_name = names::gfx_name(g.id);
    let file = SpriteFile {
        name: gfx_name
            .strip_prefix(GFX_PREFIX)
            .unwrap_or(&gfx_name)
            .to_string(),
        gfx_id: g.id.into(),
        width: g.width,
        height: g.height,
        cell_width: g.width,
        cell_height: cell_h as u16,
        frame_sizes,
        palette_tag: g.palette_tag,
        palette: g.palette,
    };
    Ok((img, file))
}

/// 2-bit indexed PNG in `latin_normal.png`'s layout: 16 glyphs per row, gbagfx palette.
pub(crate) fn font_png(f: &font::Font) -> Result<Vec<u8>> {
    let (gw, gh) = (usize::from(f.glyph_width_max), usize::from(f.glyph_height));
    let rows = f.glyphs.len().div_ceil(FONT_GLYPHS_PER_ROW);
    let (w, h) = (FONT_GLYPHS_PER_ROW * gw, rows * gh);
    let stride = w.div_ceil(4);
    let mut data = vec![0u8; stride * h];
    for (i, glyph) in f.glyphs.iter().enumerate() {
        ensure!(glyph.len() == gw * gh, "glyph {i}: {} pixels", glyph.len());
        let (ox, oy) = (i % FONT_GLYPHS_PER_ROW * gw, i / FONT_GLYPHS_PER_ROW * gh);
        for (p, &px) in glyph.iter().enumerate() {
            ensure!(px < 4, "glyph {i}: pixel value {px}");
            let (x, y) = (ox + p % gw, oy + p / gw);
            data[y * stride + x / 4] |= px << (6 - 2 * (x % 4));
        }
    }
    let mut buf = Vec::new();
    let mut enc = png::Encoder::new(&mut buf, w as u32, h as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Two);
    enc.set_palette(FONT_PALETTE.concat());
    let mut writer = enc.write_header()?;
    writer.write_image_data(&data)?;
    writer.finish()?;
    Ok(buf)
}

/// Reads back an exported map: `maps/<name>.ron` under `root`.
pub fn read_map(root: &Path, name: &str) -> Result<MapFile> {
    read_ron(&root.join(format!("maps/{name}.ron")))
}

/// Reads back an exported tileset: `tilesets/<name>.ron` under `root`.
pub fn read_tileset(root: &Path, name: &str) -> Result<TilesetFile> {
    read_ron(&root.join(format!("tilesets/{name}.ron")))
}

/// Parses any exported `.ron` file into its schema type.
pub fn read_ron<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graphics(w: u16, h: u16, frames: Vec<Vec<u8>>) -> ObjectGraphics {
        ObjectGraphics {
            id: 89,
            offset: 0x100,
            width: w,
            height: h,
            palette_tag: 0,
            frames,
            palette: [0; 16],
        }
    }

    #[test]
    fn sprite_sheet_places_frames_side_by_side() {
        // Two 8x16 frames: frame 0 is all 1s, frame 1 has 2s in its bottom tile only.
        let f0 = vec![0x11; 64];
        let mut f1 = vec![0; 64];
        f1[32..].fill(0x22);
        let (img, file) = sprite(&graphics(8, 16, vec![f0, f1])).unwrap();
        assert_eq!(file.name, "MAY_NORMAL");
        assert_eq!(file.gfx_id, 89);
        assert_eq!((img.width, img.height), (16, 16));
        assert_eq!((file.cell_width, file.cell_height), (8, 16));
        assert_eq!(file.frame_sizes, vec![(8, 16), (8, 16)]);
        assert_eq!(img.pixels[0], 1);
        assert_eq!(img.pixels[8], 0);
        assert_eq!(img.pixels[8 * 16 + 8], 2);
        assert_eq!(img.pixels[15 * 16 + 7], 1);
    }

    #[test]
    fn sprite_sheet_handles_mixed_frame_sizes() {
        // An 8x8 info whose pic table mixes an 8x8 frame (all 1s) and an 8x16 frame (all 2s),
        // like the berry trees' 16x16 sprouts and 16x32 trees.
        let (img, file) = sprite(&graphics(8, 8, vec![vec![0x11; 32], vec![0x22; 64]])).unwrap();
        assert_eq!((img.width, img.height), (16, 16));
        assert_eq!(file.frame_sizes, vec![(8, 8), (8, 16)]);
        assert_eq!(file.cell_height, 16);
        assert_eq!(img.pixels[0], 1);
        assert_eq!(img.pixels[7 * 16 + 7], 1);
        assert_eq!(img.pixels[8 * 16], 0, "below the short frame is blank");
        assert_eq!(img.pixels[8], 2);
        assert_eq!(img.pixels[15 * 16 + 15], 2);
    }

    #[test]
    fn sprite_sheet_rejects_odd_frame_size() {
        // 32 bytes at width 16 would be 4 rows: not whole tiles.
        let err = sprite(&graphics(16, 16, vec![vec![0; 32]])).unwrap_err();
        assert!(format!("{err}").contains("0x100"), "{err}");
    }

    #[test]
    fn setvar_of_a_gfx_var_adds_the_graphics_id() {
        // setvar VAR_OBJ_GFX_ID_0, 105 ; setvar VAR_TEMP_0, 7 ; setvar VAR_OBJ_GFX_ID_F, 3 ; end
        let mut b = vec![0u8; 0x100];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b.extend([0x16, 0x10, 0x40, 105, 0, 0x16, 0x00, 0x40, 7, 0]);
        b.extend([0x16, 0x1F, 0x40, 3, 0, 0x02]);
        b.resize(0x200, 0);
        let rom = Rom::from_bytes(b).unwrap();
        let scan = crate::scripts::walk(&rom, &[0x100]).unwrap();
        assert_eq!(var_gfx_ids(&rom, &scan).unwrap(), BTreeSet::from([3, 105]));
        // A graphics id that does not fit a byte is an error.
        let mut b = rom.bytes().to_vec();
        b[0x104] = 1;
        let rom = Rom::from_bytes(b).unwrap();
        let scan = crate::scripts::walk(&rom, &[0x100]).unwrap();
        assert!(var_gfx_ids(&rom, &scan).is_err());
    }

    #[test]
    fn font_png_packs_2bpp_msb_first() {
        let mut glyphs = vec![vec![0u8; 256]; 17];
        glyphs[0][1] = 3; // x=1, y=0
        glyphs[16][0] = 2; // second row, x=0, y=16
        let f = font::Font {
            glyph_width_max: 16,
            glyph_height: 16,
            glyphs,
            widths: vec![0; 17],
        };
        let png = font_png(&f).unwrap();
        let mut reader = png::Decoder::new(png.as_slice()).read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (256, 32));
        assert_eq!(info.bit_depth, png::BitDepth::Two);
        assert_eq!(buf[0], 0b0011_0000);
        assert_eq!(buf[16 * info.line_size], 0b1000_0000);
    }

    #[test]
    fn to_ron_is_stable_and_round_trips() {
        let v: BTreeMap<String, Vec<u16>> =
            BTreeMap::from([("b".into(), vec![1, 2]), ("a".into(), vec![])]);
        let s = to_ron(&v).unwrap();
        assert!(s.ends_with('\n') && !s.contains('\r'));
        assert!(s.find("\"a\"") < s.find("\"b\""));
        let back: BTreeMap<String, Vec<u16>> = ron::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn layout_rows_are_one_line_each_and_round_trip() {
        let layout = Layout {
            width: 3,
            height: 2,
            primary: "General".into(),
            secondary: "Petalburg".into(),
            border: schema::to_rows(&[1, 2, 3, 4], 2),
            blocks: schema::to_rows(&[10, 11, 12, 20, 21, 22], 3),
        };
        let s = to_ron(&layout).unwrap();
        assert!(
            s.contains("        (10, 11, 12),\n        (20, 21, 22),\n"),
            "{s}"
        );
        let back: Layout = ron::from_str(&s).unwrap();
        assert_eq!(back, layout);
    }
}
