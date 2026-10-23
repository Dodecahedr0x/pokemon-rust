//! Stamps: small reusable cell grids (a house, a fence corner, a pond), saved as
//! `games/<game>/stamps/<name>.ron` and painted with the `Stamp` op.
//!
//! ```ron
//! (
//!     primary: "General",
//!     secondary: "Petalburg",
//!     width: 2,
//!     height: 1,
//!     elevation: true,
//!     blocks: [
//!         (13313, 13314),
//!     ],
//! )
//! ```
//!
//! `blocks` are raw blocks like a layout's (`metatile | collision << 10 | elevation << 12`).
//! With `elevation: false` a stamp keeps the map's elevations (its blocks' elevation bits are
//! 0). A stamp is keyed to its tilesets: stamping checks that the map's primary tileset is the
//! stamp's, and so is its secondary when the stamp uses secondary metatiles (512 and up).
//! `stamp-from` captures a region of an authored map; stamps never hold extracted map regions
//! (`games::copies` checks them as it checks layouts).

use super::model::{Doc, block};
use anyhow::{Result, ensure};
use engine::data::{METATILES_IN_PRIMARY, block_collision, block_metatile};
use extract::schema::{Row, from_rows, to_rows};
use serde::{Deserialize, Serialize};

/// `stamps/<name>.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    pub primary: String,
    pub secondary: String,
    pub width: u16,
    pub height: u16,
    /// Whether stamping writes the elevations (false: the map's stay).
    pub elevation: bool,
    pub blocks: Vec<Row>,
}

impl Stamp {
    /// The `w x h` cells of `d` from (x, y) (which must fit).
    pub fn capture(d: &Doc, x: i32, y: i32, w: i32, h: i32, elevation: bool) -> Result<Stamp> {
        ensure!(
            w > 0
                && h > 0
                && x >= 0
                && y >= 0
                && x as i64 + w as i64 <= d.width as i64
                && y as i64 + h as i64 <= d.height as i64,
            "the {w}x{h} region at ({x}, {y}) is not inside the {}x{} map",
            d.width,
            d.height
        );
        let mut cells = Vec::with_capacity(w as usize * h as usize);
        for cy in y..y + h {
            for cx in x..x + w {
                let b = d.block(cx, cy).unwrap_or_default();
                cells.push(match elevation {
                    true => b,
                    false => block(block_metatile(b), block_collision(b), 0),
                });
            }
        }
        Ok(Stamp {
            primary: d.primary.clone(),
            secondary: d.secondary.clone(),
            width: w as u16,
            height: h as u16,
            elevation,
            blocks: to_rows(&cells, w as usize),
        })
    }

    /// The blocks, row-major, checked against the size.
    pub fn cells(&self) -> Result<Vec<u16>> {
        let cells = from_rows(&self.blocks);
        ensure!(
            self.width > 0
                && self.height > 0
                && self.blocks.len() == self.height as usize
                && self.blocks.iter().all(|r| r.0.len() == self.width as usize),
            "the stamp's blocks are not {}x{}",
            self.width,
            self.height
        );
        Ok(cells)
    }

    /// Whether some cell uses the secondary tileset.
    pub fn uses_secondary(&self) -> bool {
        (from_rows(&self.blocks).iter()).any(|&b| block_metatile(b) >= METATILES_IN_PRIMARY)
    }

    /// The stamp can paint `d`: same primary tileset, and same secondary when it uses it.
    pub fn check_tilesets(&self, d: &Doc) -> Result<()> {
        ensure!(
            self.primary == d.primary,
            "the stamp is for primary tileset {:?}, the map uses {:?}",
            self.primary,
            d.primary
        );
        ensure!(
            self.secondary == d.secondary || !self.uses_secondary(),
            "the stamp uses secondary tileset {:?}'s metatiles, the map uses {:?}",
            self.secondary,
            d.secondary
        );
        Ok(())
    }
}
