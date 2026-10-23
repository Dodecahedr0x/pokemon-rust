//! Object sprites for the map view's NPC overlay (M6 Task E): sheets decoded to pixels.

use engine::data::{Dir, GameData, Gfx, MovementType, SpriteData};

/// Frame `frame` of a sprite sheet, decoded: `(width, height, pixels)`, row-major, with `None`
/// for color index 0 (transparent) and the palette color otherwise. The frame's tiles are
/// 4bpp in GBA 1D OBJ order (row-major tiles, `width / 8` per row; low nibble = left pixel).
/// A frame past the sheet's end is clamped to its last frame; a sheet without frames gives
/// all-transparent pixels.
pub fn decode_sprite(s: &SpriteData, frame: usize) -> (usize, usize, Vec<Option<u16>>) {
    let (w, h) = (s.width as usize, s.height as usize);
    let mut px = vec![None; w * h];
    let Some(tiles) = s.frames.get(frame.min(s.frames.len().saturating_sub(1))) else {
        return (w, h, px);
    };
    let per_row = w / 8;
    for (t, tile) in tiles.iter().enumerate().take(per_row * (h / 8)) {
        let (tx, ty) = (t % per_row * 8, t / per_row * 8);
        for (r, &row) in tile.iter().enumerate() {
            for c in 0..8 {
                let idx = (row >> (4 * c)) & 0xF;
                if idx != 0 {
                    px[(ty + r) * w + tx + c] = Some(s.palette[idx as usize]);
                }
            }
        }
    }
    (w, h, px)
}

/// An object's picture for the overlay.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ObjectSprite {
    pub width: usize,
    pub height: usize,
    /// Row-major; `None` is transparent.
    pub pixels: Vec<Option<u16>>,
    /// The graphics are not known statically (`Gfx::Var`) or the game has no such sprite:
    /// `pixels` is a placeholder box.
    pub placeholder: bool,
}

impl ObjectSprite {
    /// Map pixel of the sprite's top-left corner for an object standing on cell (x, y): the
    /// sprite is centered on the cell horizontally and its bottom is the cell's bottom (the
    /// engine's object placement, `objects/draw.rs`).
    pub fn top_left(&self, x: i16, y: i16) -> (i32, i32) {
        (
            x as i32 * 16 + 8 - self.width as i32 / 2,
            y as i32 * 16 + 16 - self.height as i32,
        )
    }
}

/// Placeholder color (magenta, RGB555).
const PLACEHOLDER: u16 = 0x7C1F;

/// A 16x16 box outline: what an object whose graphics come from a var looks like.
fn placeholder() -> ObjectSprite {
    let n = 16;
    let pixels = (0..n * n)
        .map(|i| {
            let (x, y) = (i % n, i / n);
            (x == 0 || y == 0 || x == n - 1 || y == n - 1 || x == y || x + y == n - 1)
                .then_some(PLACEHOLDER)
        })
        .collect();
    ObjectSprite {
        width: n,
        height: n,
        pixels,
        placeholder: true,
    }
}

/// The direction an object faces when it spawns, from its movement type (the engine's
/// `objects::initial_facing`, Gen3 `gInitialMovementTypeFacingDirections`).
pub fn initial_facing(m: MovementType) -> Dir {
    engine::objects::initial_facing(m)
}

/// An object's standing frame facing `facing`: `Gfx::Fixed` through `game.sprite`, with the
/// engine's facing frames (down 0, up 1, left 2, right = left mirrored; sheets with fewer
/// than 3 frames show frame 0). `Gfx::Var` and unknown graphics give a placeholder.
pub fn object_sprite(game: &impl GameData, gfx: Gfx, facing: Dir) -> ObjectSprite {
    let Gfx::Fixed(id) = gfx else {
        return placeholder();
    };
    let Some(s) = game.sprite(id).filter(|s| !s.frames.is_empty()) else {
        return placeholder();
    };
    let (frame, hflip) = match facing {
        _ if s.frames.len() < 3 => (0, false),
        Dir::Down => (0, false),
        Dir::Up => (1, false),
        Dir::Left => (2, false),
        Dir::Right => (2, true),
    };
    let (width, height, mut pixels) = decode_sprite(s, frame);
    if hflip {
        for row in pixels.chunks_exact_mut(width) {
            row.reverse();
        }
    }
    ObjectSprite {
        width,
        height,
        pixels,
        placeholder: false,
    }
}
