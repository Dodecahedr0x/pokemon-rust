//! Walks over water with currents (Gen3 `ForcedMovement_Pushed*ByCurrent`, Seafloor Cavern):
//! a step onto a current is carried on, cell by cell, until a still cell; a current pushing
//! onto land (elevation 3) ends the surf there (`DoForcedMovement`: `COLLISION_STOP_SURFING`).
//! Our planner does not model currents (our spine's routes are authored), and our engine
//! stops a current at land instead of landing (an engine gap): vanilla's walks plan here.

use engine::data::Dir;
use std::collections::{HashSet, VecDeque};

/// What a cell is to a surfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Blocked,
    Land,
    Water,
    Current(Dir),
}

/// The longest slide followed (a loop of currents ends it).
const MAX_SLIDE: usize = 128;

fn add((x, y): (i16, i16), d: Dir) -> (i16, i16) {
    let (dx, dy) = d.delta();
    (x + dx, y + dy)
}

/// Where a step onto `n` ends (surfing): carried by the currents, landed when one pushes
/// onto land. `(cell, surfing)`.
fn slide(mut n: (i16, i16), cell: &impl Fn((i16, i16)) -> Cell) -> ((i16, i16), bool) {
    for _ in 0..MAX_SLIDE {
        let Cell::Current(d) = cell(n) else {
            return (n, true);
        };
        let m = add(n, d);
        match cell(m) {
            Cell::Water | Cell::Current(_) => n = m,
            Cell::Land => return (m, false),
            Cell::Blocked => return (n, true),
        }
    }
    (n, true)
}

/// The first direction of a shortest walk from `from` to `to` (surfing or not), with the
/// currents; a step from land onto water is a Surf. `None` when there is no way.
pub fn first_dir(
    from: (i16, i16),
    to: (i16, i16),
    surfing: bool,
    cell: impl Fn((i16, i16)) -> Cell,
) -> Option<Dir> {
    if from == to {
        return None;
    }
    let mut seen: HashSet<((i16, i16), bool)> = HashSet::from([(from, surfing)]);
    let mut queue: VecDeque<((i16, i16), bool, Option<Dir>)> =
        VecDeque::from([(from, surfing, None)]);
    while let Some((c, s, first)) = queue.pop_front() {
        if c == to && first.is_some() {
            return first;
        }
        for d in Dir::ALL {
            let n = add(c, d);
            let next = match cell(n) {
                // The target itself (a door, an object's cell) is reached by facing it.
                Cell::Blocked if n == to => (n, s),
                Cell::Blocked => continue,
                Cell::Land => (n, false),
                Cell::Water | Cell::Current(_) => slide(n, &cell),
            };
            if seen.insert(next) {
                queue.push_back((next.0, next.1, first.or(Some(d))));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map from rows: `#` blocked, `.` land, `~` water, `<>^v` currents.
    fn map(rows: &'static [&'static str]) -> impl Fn((i16, i16)) -> Cell {
        move |(x, y)| {
            let Some(row) = usize::try_from(y).ok().and_then(|y| rows.get(y)) else {
                return Cell::Blocked;
            };
            match usize::try_from(x).ok().and_then(|x| row.as_bytes().get(x)) {
                Some(b'.') => Cell::Land,
                Some(b'~') => Cell::Water,
                Some(b'<') => Cell::Current(Dir::Left),
                Some(b'>') => Cell::Current(Dir::Right),
                Some(b'^') => Cell::Current(Dir::Up),
                Some(b'v') => Cell::Current(Dir::Down),
                _ => Cell::Blocked,
            }
        }
    }

    #[test]
    fn a_current_carries_onto_land() {
        let m = map(&["#~#", "#v#", "#.#"]);
        assert_eq!(slide((1, 1), &m), ((1, 2), false));
        assert_eq!(slide((1, 0), &m), ((1, 0), true));
    }

    #[test]
    fn the_way_rides_a_current_around_a_wall() {
        // Up the left column is a down current back to land: the way goes right, rides the
        // up current, then left along the still top row.
        let m = map(&[
            "~~~~", //
            "v#~#", //
            "v#^#", //
            "~~~#", //
        ]);
        assert_eq!(first_dir((0, 3), (0, 0), true, &m), Some(Dir::Right));
        // From land (0, 3) is not water here: start surfing from (1, 3).
        assert!(first_dir((1, 3), (0, 0), true, &m).is_some());
    }
}
