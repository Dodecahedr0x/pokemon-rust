//! The shared UI kit (Milestone 9 Task K): list menus (Gen3 `list_menu.c`), the 20 user window
//! frames, the money and coin boxes, the quantity selector, yes/no and messages in screens,
//! item and mon icons, scroll arrows and screen fades, with the per-frame upload budget.
//!
//! Wave 0: the upload budget only.

/// VRAM bytes (tiles, tilemaps, palettes, OBJ tiles) a screen uploads per frame while the
/// screen is black (opening, closing). Milestone 9 Step 0: the field's measured spread budget
/// (`field::SPREAD_TILES_BLACK`, 512 tiles = 16 KiB per black frame, `late_black` 0 on the
/// route) goes through the same `Frame` upload path, so a party-menu-sized screen (about 8 KiB
/// of tiles, three 2 KiB tilemaps, two palette banks, six mon icons: about 15.5 KiB) opens in
/// one black frame.
pub const UPLOAD_BYTES_PER_FRAME: usize = crate::field::SPREAD_TILES_BLACK as usize * 32;

/// VRAM bytes a screen uploads per frame while it shows (scrolling, a page change, a redraw):
/// the field's `SPREAD_TILES_VISIBLE` (128 tiles, 4 KiB), a few lines of vblank.
pub const UPLOAD_BYTES_VISIBLE: usize = crate::field::SPREAD_TILES_VISIBLE as usize * 32;
