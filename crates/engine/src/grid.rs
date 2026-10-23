//! Block lookup around a map: its own blocks, then connected neighbors, then the border.

use crate::data::{BLOCK_COLLISION_SHIFT, Connection, Dir, GameData, MapData, MapId};

/// Gen3 `MAP_DYNAMIC` (group and number 0x7F): the warp target that is not a map but the
/// destination `setdynamicwarp` stored (the content schema exports it as this id).
pub const MAP_DYNAMIC: MapId = MapId(0x7F7F);

/// Where a cell outside `map` lands in a connected neighbor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Neighbor {
    pub map: MapId,
    pub x: i16,
    pub y: i16,
}

pub fn in_bounds(map: &MapData, x: i16, y: i16) -> bool {
    x >= 0 && y >= 0 && (x as u16) < map.width && (y as u16) < map.height
}

/// The neighbor cell for (x, y) outside `map`, if a connection covers it. Corners (outside on
/// both axes) are never connected.
pub fn neighbor(game: &impl GameData, map: &MapData, x: i16, y: i16) -> Option<Neighbor> {
    let (w, h) = (map.width as i16, map.height as i16);
    let dir = match (x < 0, x >= w, y < 0, y >= h) {
        (false, false, false, true) => Dir::Down,
        (false, false, true, false) => Dir::Up,
        (true, false, false, false) => Dir::Left,
        (false, true, false, false) => Dir::Right,
        _ => return None,
    };
    map.connections
        .iter()
        .filter(|c| c.dir == dir)
        .find_map(|c| {
            let n = game.map(c.map);
            let (nw, nh) = (n.width as i16, n.height as i16);
            let (nx, ny) = match dir {
                Dir::Down => (x - c.offset, y - h),
                Dir::Up => (x - c.offset, y + nh),
                Dir::Left => (x + nw, y - c.offset),
                Dir::Right => (x - w, y - c.offset),
            };
            in_bounds(n, nx, ny).then_some(Neighbor {
                map: c.map,
                x: nx,
                y: ny,
            })
        })
}

/// The cell shift (`new = old + shift`) of crossing from `map` through its connection `c`.
fn connection_shift(game: &impl GameData, map: &MapData, c: &Connection) -> (i16, i16) {
    let n = game.map(c.map);
    match c.dir {
        Dir::Down => (-c.offset, -(map.height as i16)),
        Dir::Up => (-c.offset, n.height as i16),
        Dir::Left => (n.width as i16, -c.offset),
        Dir::Right => (-(map.width as i16), -c.offset),
    }
}

/// Where both maps of a connection crossing see the same blocks, in the old map's coordinates
/// (half-open ranges): the cells whose coordinate along the connection is in `along` and across
/// it in `across`. On those cells each map's own blocks, seen from the other through the
/// connection, are the blocks the other draws there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Seam {
    /// The connection is north or south (along = x, across = y), else east or west.
    pub vertical: bool,
    pub along: (i16, i16),
    pub across: (i16, i16),
}

/// The [`Seam`] of crossing from `from` into `to` by `shift` ([`neighbor`] coordinates), if the
/// two connections mirror each other exactly: `from`'s only connection in that direction leads
/// to `to`, and `to`'s only connection in the opposite direction leads back at the opposite
/// offset. Without a mirror, `None` (compare every cell).
pub fn seam(game: &impl GameData, from: MapId, to: MapId, shift: (i16, i16)) -> Option<Seam> {
    let (o, n) = (game.map(from), game.map(to));
    let only = |m: &'static MapData, dir: Dir| {
        let mut it = m.connections.iter().filter(move |c| c.dir == dir);
        it.next().filter(|_| it.next().is_none())
    };
    let c =
        (o.connections.iter()).find(|c| c.map == to && connection_shift(game, o, c) == shift)?;
    let there = only(o, c.dir)?;
    let back = only(n, c.dir.opposite())?;
    if !core::ptr::eq(there, c) || back.map != from || back.offset != -c.offset {
        return None;
    }
    let (ow, oh, nw, nh) = (
        o.width as i16,
        o.height as i16,
        n.width as i16,
        n.height as i16,
    );
    let (sx, sy) = shift;
    Some(match c.dir {
        Dir::Up | Dir::Down => Seam {
            vertical: true,
            along: ((-sx).max(0), ow.min(nw - sx)),
            across: ((-sy).min(0), oh.max(nh - sy)),
        },
        Dir::Left | Dir::Right => Seam {
            vertical: false,
            along: ((-sy).max(0), oh.min(nh - sy)),
            across: ((-sx).min(0), ow.max(nw - sx)),
        },
    })
}

/// The block at (x, y), which may lie outside `map`. Border blocks come back with full collision,
/// as in Gen3 (`GetBorderBlockAt`).
pub fn block_at(game: &impl GameData, map: &MapData, x: i16, y: i16) -> u16 {
    if in_bounds(map, x, y) {
        return map.blocks[y as usize * map.width as usize + x as usize];
    }
    if let Some(n) = neighbor(game, map, x, y) {
        let m = game.map(n.map);
        return m.blocks[n.y as usize * m.width as usize + n.x as usize];
    }
    map.border[(x & 1) as usize + 2 * (y & 1) as usize] | (3 << BLOCK_COLLISION_SHIFT)
}

#[cfg(test)]
pub(crate) mod tests;
