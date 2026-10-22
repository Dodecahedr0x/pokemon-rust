//! Block lookup around a map: its own blocks, then connected neighbors, then the border.

use crate::data::{BLOCK_COLLISION_SHIFT, Dir, GameData, MapData, MapId};

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
