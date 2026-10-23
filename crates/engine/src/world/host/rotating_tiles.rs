//! The rotating tile puzzle (Wave F; Gen3 `rotating_tile_puzzle.c`, Mossdeep's gym and the
//! Trick House's seventh room): the floor switches' scripts move every object standing on an
//! arrow of one colour a cell along it, then turn the moved ones a quarter.
//!
//! The game passes the puzzle's metatiles ([`Request::RotatingTilesInit`]): each colour is a
//! metatile row of `METATILE_ROW_WIDTH` (8), its first four the arrows right, down, left and
//! up. The state ([`RotatingTiles`]) is Gen3's `sRotatingTilePuzzle`, script RAM between the
//! script's `initrotatingtilepuzzle` and `freerotatingtilepuzzle`: never captured (a script
//! runs) nor hashed, as the field effects.

use super::{Host, UNIT};
use crate::data::{Dir, GameData, MovementType, block_metatile};
use crate::script::{Request, Response, Speed, Step};
use alloc::vec::Vec;

/// Gen3 `METATILE_ROW_WIDTH`: metatiles per row of a tileset image.
const METATILE_ROW_WIDTH: u16 = 8;
/// Colours of a puzzle (Gen3's `>= 5` check).
const COLOURS: u16 = 5;

/// Gen3 `struct RotatingTilePuzzle`.
pub(in crate::world) struct RotatingTiles {
    /// The yellow right arrow (`METATILE_MossdeepGym_YellowArrow_Right` or the Trick House's).
    start: u16,
    /// No puzzle tile below this metatile (Gen3 checks Mossdeep's first arrow for both).
    floor: u16,
    /// The moved templates: (template index, the arrow it left: 0 right to 3 up).
    moved: Vec<(usize, u8)>,
}

/// Gen3 `sMovement_Shift*`: `MOVEMENT_ACTION_LOCK_ANIM` (the facing locked, the walk does not
/// turn the object), a normal walk, `MOVEMENT_ACTION_UNLOCK_ANIM`; right, down, left, up.
static SHIFT: [[Step; 3]; 4] = [
    shift(Dir::Right),
    shift(Dir::Down),
    shift(Dir::Left),
    shift(Dir::Up),
];

const fn shift(d: Dir) -> [Step; 3] {
    [Step::LockFacing, Step::Walk(d, Speed::Normal), Step::UnlockFacing]
}

/// Gen3 `sMovement_Face*`, by direction.
static FACE: [[Step; 1]; 4] = [
    [Step::Face(Dir::Right)],
    [Step::Face(Dir::Down)],
    [Step::Face(Dir::Left)],
    [Step::Face(Dir::Up)],
];

/// Arrow `n` (0 right, 1 down, 2 left, 3 up) as a direction.
const fn arrow_dir(n: u8) -> Dir {
    match n {
        0 => Dir::Right,
        1 => Dir::Down,
        2 => Dir::Left,
        _ => Dir::Up,
    }
}

/// The index of `d` in [`FACE`].
const fn face_index(d: Dir) -> usize {
    match d {
        Dir::Right => 0,
        Dir::Down => 1,
        Dir::Left => 2,
        Dir::Up => 3,
    }
}

/// A quarter turn counterclockwise (Gen3 `ROTATE_COUNTERCLOCKWISE`).
pub(in crate::world) const fn counterclockwise(d: Dir) -> Dir {
    match d {
        Dir::Right => Dir::Up,
        Dir::Down => Dir::Right,
        Dir::Left => Dir::Down,
        Dir::Up => Dir::Left,
    }
}

/// A quarter turn clockwise (Gen3 `ROTATE_CLOCKWISE`).
pub(in crate::world) const fn clockwise(d: Dir) -> Dir {
    match d {
        Dir::Right => Dir::Down,
        Dir::Down => Dir::Left,
        Dir::Left => Dir::Up,
        Dir::Up => Dir::Right,
    }
}

/// Gen3's rotation from the arrow left (`prev`) to the arrow now under the object (`now`), as
/// `tileDifference = (u8)now - prev` (an `s8`): counterclockwise for a negative difference or 3,
/// clockwise for a positive one (or -3, never reached), none for 0.
pub(in crate::world) fn rotation(now: u8, prev: u8) -> Option<bool> {
    let d = now as i8 - prev as i8;
    if d < 0 || d == 3 {
        Some(d != -3)
    } else if d > 0 {
        Some(false)
    } else {
        None
    }
}

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let game = h.game;
    let s = &mut *h.s;
    match *r {
        Request::RotatingTilesInit { start, floor } => {
            // Gen3 allocates once and keeps an earlier list (the scripts always free it).
            let moved = s.rotating_tiles.take().map_or_else(Vec::new, |p| p.moved);
            s.rotating_tiles = Some(RotatingTiles {
                start,
                floor,
                moved,
            });
        }
        Request::RotatingTilesMove(puzzle) => {
            let Some(p) = s.rotating_tiles.as_mut() else {
                // Gen3 dereferences NULL: not reached by the scripts.
                return UNIT;
            };
            // Gen3 `sMovingNpcId` = LOCALID_NONE (no object of that id) unless one moved.
            let mut mover = 0;
            for i in 0..s.templates.len() {
                let t = s.templates[i];
                let metatile = block_metatile(s.field.block(game, t.x, t.y));
                if metatile < p.floor {
                    continue;
                }
                let off = metatile.wrapping_sub(p.start);
                let row = (off / METATILE_ROW_WIDTH) as u8 as u16;
                if row >= COLOURS || row != puzzle {
                    continue;
                }
                let tile = (off % METATILE_ROW_WIDTH) as u8;
                if tile >= 4 {
                    continue;
                }
                let d = arrow_dir(tile);
                let (dx, dy) = d.delta();
                s.templates.edit(t.local_id, |t| (t.x, t.y) = (t.x + dx, t.y + dy));
                if s.objects.get(t.local_id).is_some() {
                    p.moved.push((i, tile));
                    mover = t.local_id;
                    s.objects.start_moves(t.local_id, &SHIFT[tile as usize]);
                } else {
                    // Gen3 `TurnUnsavedRotatingTileObject` ("never reached in normal gameplay"):
                    // the template turns, by the arrow it now stands on.
                    let now = (s.templates[i].x, s.templates[i].y);
                    let off = block_metatile(s.field.block(game, now.0, now.1))
                        .wrapping_sub(p.start);
                    let turn = rotation((off % METATILE_ROW_WIDTH) as u8, tile);
                    s.templates.edit(t.local_id, |t| {
                        if let (MovementType::Face(f), Some(ccw)) = (t.movement, turn) {
                            let f = if ccw { counterclockwise(f) } else { clockwise(f) };
                            t.movement = MovementType::Face(f);
                        }
                    });
                }
            }
            s.last_mover = mover;
        }
        Request::RotatingTilesTurn => {
            let Some(p) = s.rotating_tiles.as_ref() else {
                return UNIT;
            };
            for &(i, prev) in &p.moved {
                let t = s.templates[i];
                let off = block_metatile(s.field.block(game, t.x, t.y)).wrapping_sub(p.start);
                let Some(ccw) = rotation((off % METATILE_ROW_WIDTH) as u8, prev) else {
                    continue;
                };
                let Some(facing) = s.objects.get(t.local_id).map(|o| o.facing) else {
                    continue;
                };
                let f = if ccw {
                    counterclockwise(facing)
                } else {
                    clockwise(facing)
                };
                s.templates
                    .edit(t.local_id, |t| t.movement = MovementType::Face(f));
                s.objects.start_moves(t.local_id, &FACE[face_index(f)]);
            }
        }
        Request::RotatingTilesFree => {
            // Gen3 also unfreezes the objects (`ScriptMovement_UnfreezeObjectEvents`): the
            // engine's movement scripts leave none frozen.
            s.rotating_tiles = None;
        }
        _ => {}
    }
    UNIT
}
