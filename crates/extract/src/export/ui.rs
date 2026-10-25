//! Writes the menu pictures: `icons/mons/<SPECIES>.png`, `icons/mons/palettes.ron`,
//! `icons/items/<ITEM>.{png,ron}` and `ui/BagSprite{0..4}.{png,ron}`.

use super::Out;
use crate::Rom;
use crate::gfx::{self, Palette};
use crate::schema::{Hex, ItemIconFile, MonIconPalettesFile, UiPictureFile};
use crate::ui_gfx::{self, item_name, species_name};
use anyhow::Result;
use std::collections::BTreeMap;

/// Exports mon icons, item icons and the bag sprite, and records where each table was found.
/// A ROM without those tables writes nothing (the screens stay frames and text).
pub fn export_ui(rom: &Rom, out: &mut Out, provenance: &mut BTreeMap<String, Hex>) -> Result<()> {
    if let Some(mons) = ui_gfx::locate_mon_icons(rom) {
        let mut species = BTreeMap::new();
        for (id, (tiles, pal)) in mons.icons.iter().enumerate() {
            let Some(name) = species_name(id as u32) else {
                continue;
            };
            species.insert(name.to_string(), *pal);
            let img = gfx::tiles_4bpp_to_image(tiles, 4)?;
            let palette = mons.palettes.get(*pal as usize).copied().unwrap_or([0; 16]);
            out.png_indexed(&format!("icons/mons/{name}.png"), &img, &Palette(palette))?;
        }
        out.ron(
            "icons/mons/palettes.ron",
            &MonIconPalettesFile {
                species,
                palettes: mons.palettes,
            },
        )?;
        provenance.insert("icons/mons".into(), Hex(mons.table));
        provenance.insert("icons/mons/palettes".into(), Hex(mons.palettes_at));
    }
    if let Some(items) = ui_gfx::locate_item_icons(rom) {
        for (id, (tiles, pal)) in items.icons.iter().enumerate() {
            let Some(name) = item_name(id as u32) else {
                continue;
            };
            let img = gfx::tiles_4bpp_to_image(tiles, 4)?;
            out.png_indexed(&format!("icons/items/{name}.png"), &img, &Palette(*pal))?;
            out.ron(
                &format!("icons/items/{name}.ron"),
                &ItemIconFile { palette: *pal },
            )?;
        }
        provenance.insert("icons/items".into(), Hex(items.table));
    }
    if let Some(bag) = ui_gfx::locate_bag_sprites(rom) {
        for (i, frame) in bag.frames.iter().enumerate() {
            let img = gfx::tiles_4bpp_to_image(frame, 8)?;
            let rel = format!("ui/BagSprite{i}");
            out.png_indexed(&format!("{rel}.png"), &img, &Palette(bag.palette))?;
            out.ron(
                &format!("{rel}.ron"),
                &UiPictureFile {
                    tile_count: (frame.len() / 32) as u16,
                    map: Vec::new(),
                    map_width: 0,
                    palettes: vec![bag.palette],
                },
            )?;
        }
        provenance.insert("ui/bag_sprite".into(), Hex(bag.sheet));
    }
    Ok(())
}
