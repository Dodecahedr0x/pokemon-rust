//! The shared UI kit (Milestone 9 Task K): list menus (Gen3 `list_menu.c`), windows and frames
//! ([`window`]), the money and coin boxes, the quantity selector, yes/no and messages in
//! screens ([`widgets`]), with the per-frame upload budget and the hosting state
//! `world/screens.rs` keeps between frames.
//!
//! Deviations (Task K):
//! - Screens draw on BG0 only (frames and text; pictures from `GameData::ui_gfx` are not drawn
//!   yet, and every game answers `None` until Task X1). Full-screen screens turn BG1–BG3 off
//!   and fill BG0 with the window background; the field's VRAM is never written, so closing a
//!   screen re-enables it and reloads it spread (`Field::restore_spread`) as after a battle.
//! - Item and mon icons are not drawn (no icon is extracted yet); the slots keep their place.
//! - The d-pad does not repeat while held ([`list`]).

pub mod list;
pub mod widgets;
pub mod window;

use super::{ScreenKind, ScreenResult};
use crate::data::{ItemId, ScriptId};
use crate::hash::Fnv64;

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

/// VRAM bytes `f` copies so far (tiles, tilemaps, palettes); a map fill is a `CpuFastFill`
/// with no source data and is not counted.
pub fn frame_bytes(f: &crate::video::Frame) -> usize {
    use crate::video::Cmd;
    f.cmds
        .iter()
        .map(|c| match c {
            Cmd::BgPalette { .. } | Cmd::ObjPalette { .. } => 32,
            Cmd::BgTiles { tiles, .. } | Cmd::ObjTiles { tiles, .. } => tiles.len() * 32,
            Cmd::BgTile { .. } | Cmd::ObjTile { .. } => 32,
            Cmd::MapEntries { entries, .. } => entries.len() * 2,
            Cmd::MapFill { .. } | Cmd::Bg { .. } => 0,
        })
        .sum()
}

/// What is left of `f`'s upload budget ([`UPLOAD_BYTES_PER_FRAME`] on a black or bulk frame,
/// else [`UPLOAD_BYTES_VISIBLE`]): screens flush their changed tiles within it and the rest
/// on the next frames (a full redraw while visible shows over a few frames).
pub fn frame_budget(f: &crate::video::Frame) -> usize {
    let cap = if f.is_black() || f.bulk {
        UPLOAD_BYTES_PER_FRAME
    } else {
        UPLOAD_BYTES_VISIBLE
    };
    cap.saturating_sub(frame_bytes(f))
}

/// Frames of a screen fade (Gen3 `BeginNormalPaletteFade(PALETTES_ALL, 0, 0, 16, RGB_BLACK)`
/// as the engine's `Fader` runs it: one level a frame).
pub const FADE_FRAMES: u8 = 16;

/// Whether `kind` is drawn over the field (Gen3's start menu and save dialog windows) rather
/// than owning the whole screen.
pub const fn is_overlay(kind: ScreenKind) -> bool {
    matches!(kind, ScreenKind::StartMenu | ScreenKind::SaveDialog)
}

/// A screen's drawing state (canvases, the composed map), rebuilt from the screen's own state
/// whenever it is missing: on the first frame, after a child screen took the screen (the
/// screen drops it when it pushes), and in a clone. Not state: never hashed, always equal.
pub struct Gfx<T>(pub Option<alloc::boxed::Box<T>>);

impl<T> Gfx<T> {
    pub const fn none() -> Self {
        Gfx(None)
    }
}

impl<T> Default for Gfx<T> {
    fn default() -> Self {
        Gfx(None)
    }
}

impl<T> Clone for Gfx<T> {
    fn clone(&self) -> Self {
        Gfx(None)
    }
}

impl<T> PartialEq for Gfx<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for Gfx<T> {}

impl<T> core::fmt::Debug for Gfx<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Gfx(drawn)"
        } else {
            "Gfx(none)"
        })
    }
}

/// Where the hosting is between two screens (`world/screens.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Phase {
    /// The top screen runs with the player's input.
    #[default]
    Running,
    /// Fading to black before [`Hosting::pending`] happens; nothing steps.
    Out,
    /// Black while the field reloads (`Field::uploading`).
    Restoring,
    /// Fading in from black; the top screen steps without input.
    In,
}

/// What happens once the screen is black ([`Phase::Out`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Pending {
    #[default]
    None,
    /// The top screen (just pushed) takes the screen.
    Enter,
    /// The top screen closes with this result.
    Pop(ScreenResult),
    /// Every screen closes, then the script runs (`ScreenOut::RunScript`).
    Script(ScriptId),
    /// Every screen closes, then the item's field use (`ScreenOut::UseOnField`).
    Item(ItemId),
    /// Every screen closes, then Fly (`ScreenOut::Fly`).
    Fly(u8),
}

/// What `world/screens.rs` keeps between frames while screens are open: the fade, what waits
/// for black, and whether a script request waits for the bottom screen's result. Lives in the
/// hashed `ui::ScreenStack`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Hosting {
    pub phase: Phase,
    /// Fade level, 0 (shown) ..= [`FADE_FRAMES`] (black).
    pub level: u8,
    pub pending: Pending,
    /// BG0 holds a full-screen screen (the field's layers are off).
    pub full: bool,
    /// A script request (`ChoosePartyMon`, `SaveDialog`, …) waits for the bottom screen's
    /// result (`host/screens.rs`).
    pub waiting: bool,
}

impl Hosting {
    /// Feeds the hosting state into `h`.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.phase as u8,
            self.level,
            self.full as u8,
            self.waiting as u8,
        ]);
        match self.pending {
            Pending::None => h.write(&[0]),
            Pending::Enter => h.write(&[1]),
            Pending::Pop(r) => {
                let (t, v) = match r {
                    ScreenResult::Cancel => (0, 0),
                    ScreenResult::Value(v) => (1, v),
                    ScreenResult::Bool(b) => (2, b as u16),
                };
                h.write(&[2, t]);
                h.write_u16(v);
            }
            Pending::Script(id) => {
                h.write(&[3]);
                h.write_u16(id.0);
            }
            Pending::Item(id) => {
                h.write(&[4]);
                h.write_u16(id.0);
            }
            Pending::Fly(i) => h.write(&[5, i]),
        }
    }
}
