//! Screen windows on BG0 (Gen3 `window.c`, `DrawStdWindowFrame`, `ClearStdWindowAndFrame`).
//!
//! Every screen draws on the text BG (`layout::TEXT_BG`, char block `TEXT_CHAR_BLOCK`, screen
//! block 31), the layer the field text box uses: a full-screen screen turns BG1–BG3 off and
//! covers BG0 with a solid tile, an overlay (the start menu, the save dialog) draws its windows
//! over the field. The field's tiles and maps (char blocks 0–1, screen blocks 28–30) are never
//! touched, so the field comes back by re-enabling its layers (`world/screens.rs`).
//!
//! A screen keeps one [`Panel`] (a [`Canvas`] in its own tile range) per window and composes
//! the visible ones into a [`MapBuf`], uploaded whole (640 entries, 1.25 KiB) when the layout
//! changes; canvases upload only their dirty tiles.
//!
//! Frames (Gen3 `WindowFunc_DrawStandardFrame`): the user frame `GameData::window_frame(n)` of
//! the frame option when the game has one (9 tiles: corners, edges), else a one-tile frame made
//! of the dialogue frame's tiles (`GameData::window`: corner 1, top 4, corner 6, sides 7 and
//! 10, the bottom row flipped), so a game without UI pictures still gets framed windows.

use crate::Tile;
use crate::data::GameData;
use crate::text::{Canvas, Colors, FRAME_BASE, MAX_FRAME_TILES, Window};
use crate::video::{BgControl, Cmd, Frame, MAP_SIZE, layout};
use alloc::vec;
use alloc::vec::Vec;

/// The char block of every screen canvas (the text BG's).
pub const CHAR_BLOCK: u8 = layout::TEXT_CHAR_BLOCK;
/// BG0's screen block.
pub const SCREEN_BLOCK: u8 = layout::SCREEN_BLOCKS[layout::TEXT_BG as usize];
/// Visible tile rows of a 160-pixel screen.
pub const ROWS: usize = 20;
/// The solid background tile of full-screen screens (all pixels the window's text background).
pub const SOLID_TILE: u16 = 32;
/// The user frame's 9 tiles (`gUserFrames`), when the game has them.
pub const USER_FRAME_TILE: u16 = 33;
/// First canvas tile of a full-screen screen, and the end of the range (screen block 28, the
/// field's top layer map, starts at tile 768 of char block 2).
pub const FULL_BASE: u16 = 48;
pub const FULL_END: u16 = 768;
/// The canvas range of overlays: past the field text box's canvases and multichoice
/// (`menu::CANVAS_BASE` and up), below the field maps.
pub const OVERLAY_BASE: u16 = 512;
pub const OVERLAY_END: u16 = 768;
/// Text and dialogue-frame palette (the field's window palette).
pub const TEXT_PALETTE: u8 = layout::WINDOW_PALETTE;
/// The user frame's palette (Gen3 `STD_WINDOW_PALETTE_NUM`).
pub const USER_FRAME_PALETTE: u8 = 14;

/// The dialogue frame tiles (indices into `WindowGfx::tiles`) a one-tile frame is made of.
const DLG_CORNER: u16 = 1;
const DLG_TOP: u16 = 4;
const DLG_CORNER_R: u16 = 6;
const DLG_LEFT: u16 = 7;
const DLG_RIGHT: u16 = 10;

/// Which frame tiles are loaded (decided when the screen layer is set up).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FrameKind {
    /// The dialogue frame's tiles (no user frame in the game).
    #[default]
    Dialogue,
    /// `gUserFrames[n]` at [`USER_FRAME_TILE`].
    User,
}

/// The frame kind [`load`] loads for frame option `n`.
pub fn frame_kind(game: &impl GameData, n: u8) -> FrameKind {
    match game.window_frame(n) {
        Some(u) if u.tiles.len() >= 9 => FrameKind::User,
        _ => FrameKind::Dialogue,
    }
}

/// The text colors of every screen window (the dialogue window's).
pub fn colors(game: &impl GameData) -> Colors {
    Colors::of(game.window())
}

/// Loads what every screen window needs on BG0: the window palette, the blank tile 0, the
/// dialogue frame tiles and, if the game has it, user frame `frame` with its palette. Returns
/// the frame kind loaded.
pub fn load(game: &impl GameData, frame: u8, f: &mut Frame) -> FrameKind {
    static BLANK: [Tile; 1] = [[0; 8]];
    let w = game.window();
    f.cmds.push(Cmd::BgPalette {
        bank: TEXT_PALETTE,
        colors: *w.palette,
    });
    f.cmds.push(Cmd::BgTiles {
        char_block: CHAR_BLOCK,
        first: 0,
        tiles: &BLANK,
    });
    f.cmds.push(Cmd::BgTiles {
        char_block: CHAR_BLOCK,
        first: FRAME_BASE,
        tiles: &w.tiles[..w.tiles.len().min(MAX_FRAME_TILES)],
    });
    let bg = Colors::of(w).bg & 0xF;
    f.cmds.push(Cmd::BgTile {
        char_block: CHAR_BLOCK,
        index: SOLID_TILE,
        tile: [bg as u32 * 0x1111_1111; 8],
    });
    match game.window_frame(frame) {
        Some(u) if u.tiles.len() >= 9 => {
            f.cmds.push(Cmd::BgPalette {
                bank: USER_FRAME_PALETTE,
                colors: *u.palette,
            });
            f.cmds.push(Cmd::BgTiles {
                char_block: CHAR_BLOCK,
                first: USER_FRAME_TILE,
                tiles: &u.tiles[..9],
            });
            FrameKind::User
        }
        _ => FrameKind::Dialogue,
    }
}

/// BG0 on, at the text BG's priority, char and screen blocks.
pub fn enable_bg0(f: &mut Frame, enabled: bool) {
    f.cmds.push(Cmd::Bg {
        layer: layout::TEXT_BG,
        control: BgControl {
            enabled,
            priority: layout::BG_PRIORITY[layout::TEXT_BG as usize],
            char_block: CHAR_BLOCK,
            screen_block: SCREEN_BLOCK,
        },
    });
}

/// Sets up a full-screen screen (on a black frame): [`load`], BG1–BG3 off, BG0 on and covered
/// with the solid tile. Objects are not drawn while it shows (the frame's OAM is empty).
pub fn enter_full(game: &impl GameData, frame: u8, f: &mut Frame) -> FrameKind {
    let kind = load(game, frame, f);
    for bg in [
        layout::FIELD_TOP_BG,
        layout::FIELD_MIDDLE_BG,
        layout::FIELD_BOTTOM_BG,
    ] {
        f.cmds.push(Cmd::Bg {
            layer: bg,
            control: BgControl {
                enabled: false,
                priority: layout::BG_PRIORITY[bg as usize],
                char_block: layout::FIELD_CHAR_BLOCK,
                screen_block: layout::SCREEN_BLOCKS[bg as usize],
            },
        });
    }
    f.cmds.push(Cmd::MapFill {
        screen_block: SCREEN_BLOCK,
        entry: SOLID_TILE | (TEXT_PALETTE as u16) << 12,
    });
    enable_bg0(f, true);
    f.bulk = true;
    kind
}

/// Sets up an overlay over the field: [`load`], BG0 cleared and on.
pub fn enter_overlay(game: &impl GameData, frame: u8, f: &mut Frame) -> FrameKind {
    let kind = load(game, frame, f);
    f.cmds.push(Cmd::MapFill {
        screen_block: SCREEN_BLOCK,
        entry: 0,
    });
    enable_bg0(f, true);
    kind
}

/// One window: its rectangle, canvas and whether it has a frame.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Panel {
    pub canvas: Canvas,
    pub framed: bool,
}

impl Panel {
    /// A window at tile (`left`, `top`), `width` x `height` tiles, canvas from tile `base`
    /// (Gen3 `struct WindowTemplate`), cleared to `bg`.
    pub fn new(left: u8, top: u8, width: u8, height: u8, base: u16, framed: bool, bg: u8) -> Self {
        let mut canvas = Canvas::new(Window::new(left, top, width, height, base), CHAR_BLOCK);
        canvas.fill(bg);
        Panel { canvas, framed }
    }

    pub fn window(&self) -> Window {
        self.canvas.window()
    }

    /// Tiles the canvas takes (the next window's base is `base + tiles`).
    pub fn tiles(&self) -> u16 {
        let w = self.window();
        w.width() as u16 * w.height() as u16
    }

    /// Clears the canvas to the window background.
    pub fn clear(&mut self, game: &impl GameData) {
        self.canvas.fill(colors(game).bg);
    }

    /// Draws `text` (Gen3-encoded) at pixel (x, y) with the window colors.
    pub fn text(&mut self, game: &impl GameData, text: &[u8], x: i16, y: i16) {
        self.canvas.draw_text(game.font(), text, x, y, colors(game));
    }

    /// Draws `text` at (x, y) with `colors`.
    pub fn text_colored(&mut self, game: &impl GameData, text: &[u8], x: i16, y: i16, c: Colors) {
        self.canvas.draw_text(game.font(), text, x, y, c);
    }

    /// Uploads the changed tiles.
    pub fn flush(&mut self, f: &mut Frame) {
        self.canvas.flush(f);
    }
}

/// BG0's map for the visible rows, composed from panels and uploaded whole.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapBuf {
    entries: Vec<u16>,
    frame: FrameKind,
}

impl MapBuf {
    /// A full-screen map (the solid tile everywhere).
    pub fn full(frame: FrameKind) -> Self {
        MapBuf {
            entries: vec![SOLID_TILE | (TEXT_PALETTE as u16) << 12; MAP_SIZE * ROWS],
            frame,
        }
    }

    /// An overlay map (transparent everywhere).
    pub fn overlay(frame: FrameKind) -> Self {
        MapBuf {
            entries: vec![0; MAP_SIZE * ROWS],
            frame,
        }
    }

    pub fn entry(&self, x: usize, y: usize) -> u16 {
        self.entries[y * MAP_SIZE + x]
    }

    fn set(&mut self, x: i16, y: i16, e: u16) {
        if (0..MAP_SIZE as i16).contains(&x) && (0..ROWS as i16).contains(&y) {
            self.entries[y as usize * MAP_SIZE + x as usize] = e;
        }
    }

    /// The frame map entry for logical frame position `pos` (0 TL, 1 top, 2 TR, 3 left, 5
    /// right, 6 BL, 7 bottom, 8 BR; Gen3's `gUserFrames` tile order).
    fn frame_entry(&self, pos: u16) -> u16 {
        match self.frame {
            FrameKind::User => USER_FRAME_TILE + pos | (USER_FRAME_PALETTE as u16) << 12,
            FrameKind::Dialogue => {
                let (tile, vflip) = match pos {
                    0 => (DLG_CORNER, false),
                    1 => (DLG_TOP, false),
                    2 => (DLG_CORNER_R, false),
                    3 => (DLG_LEFT, false),
                    5 => (DLG_RIGHT, false),
                    6 => (DLG_CORNER, true),
                    7 => (DLG_TOP, true),
                    _ => (DLG_CORNER_R, true),
                };
                (FRAME_BASE + tile) | (vflip as u16) << 11 | (TEXT_PALETTE as u16) << 12
            }
        }
    }

    /// Puts `p`'s canvas (and frame, one tile around it) on the map.
    pub fn put(&mut self, p: &Panel) {
        let w = p.window();
        let (l, t, ww, hh) = (
            w.left() as i16,
            w.top() as i16,
            w.width() as i16,
            w.height() as i16,
        );
        if p.framed {
            self.set(l - 1, t - 1, self.frame_entry(0));
            self.set(l + ww, t - 1, self.frame_entry(2));
            self.set(l - 1, t + hh, self.frame_entry(6));
            self.set(l + ww, t + hh, self.frame_entry(8));
            for x in l..l + ww {
                self.set(x, t - 1, self.frame_entry(1));
                self.set(x, t + hh, self.frame_entry(7));
            }
            for y in t..t + hh {
                self.set(l - 1, y, self.frame_entry(3));
                self.set(l + ww, y, self.frame_entry(5));
            }
        }
        let pal = (TEXT_PALETTE as u16) << 12;
        for y in 0..hh {
            for x in 0..ww {
                let tile = w.base() + (y * ww + x) as u16;
                self.set(l + x, t + y, tile | pal);
            }
        }
    }

    /// Uploads the visible rows.
    pub fn flush(&self, f: &mut Frame) {
        f.cmds.push(Cmd::MapEntries {
            screen_block: SCREEN_BLOCK,
            start: 0,
            entries: self.entries.clone(),
        });
    }
}

/// Hands out consecutive canvas tile ranges.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileAlloc {
    next: u16,
    end: u16,
}

impl TileAlloc {
    pub const fn full() -> Self {
        TileAlloc {
            next: FULL_BASE,
            end: FULL_END,
        }
    }

    pub const fn overlay() -> Self {
        TileAlloc {
            next: OVERLAY_BASE,
            end: OVERLAY_END,
        }
    }

    /// The base of a `width` x `height` canvas. Past the range, the last tiles are shared (a
    /// garbled window, never a panic or a write into the field's maps).
    pub fn take(&mut self, width: u8, height: u8) -> u16 {
        let n = width as u16 * height as u16;
        if self.next + n > self.end {
            return self.end.saturating_sub(n);
        }
        let base = self.next;
        self.next += n;
        base
    }

    /// A [`Panel`] in the next tiles.
    #[allow(clippy::too_many_arguments)]
    pub fn panel(
        &mut self,
        game: &impl GameData,
        left: u8,
        top: u8,
        width: u8,
        height: u8,
        framed: bool,
    ) -> Panel {
        let base = self.take(width, height);
        Panel::new(left, top, width, height, base, framed, colors(game).bg)
    }
}
