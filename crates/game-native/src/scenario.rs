//! Scripted runs for parity and screenshots.
//!
//! A scenario is a list of high-level [`Action`]s ("walk to (10, 9)", "go through the door
//! above", "talk"). [`record`] plays it on a native [`Session`] and records the buttons held
//! on every frame, plus the frame of each named moment. The recorded buttons are the input
//! script: the ROM replays them exactly (`cargo xtask parity`), so the scenario only needs to
//! be right on native; any ROM divergence is a parity failure, as it should be.
//!
//! Paths are found with a breadth-first search over the map's collision bits, ledges and the
//! current object positions, recomputed every frame (wandering NPCs). It is a test driver, not
//! game logic: elevation is ignored and the run stops with an error when stuck.

use crate::Session;
use engine::data::{Behavior, Dir, GameData, MapId, PLAYER, block_collision};
use engine::flow::block_behavior;
use engine::grid::{block_at, in_bounds};
use engine::{Buttons, World};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub enum Action {
    /// Walk to a cell of the current map.
    GoTo(i16, i16),
    /// Hold a direction until a warp transition has run (doors, doormats, stairs).
    Enter(Dir),
    /// Hold a direction until the player crosses into the connected map, then settle.
    Cross(Dir),
    /// Turn to face a direction without moving (the cell ahead must be blocked).
    Face(Dir),
    /// Press A on what is ahead, let the first message print (naming that moment if given),
    /// then press A now and then until the script ends.
    Talk(Option<&'static str>),
    /// Hold nothing for this many frames.
    Wait(u32),
    /// Name the current frame (a screenshot and a parity checkpoint).
    Shot(&'static str),
}

/// What [`record`] produced.
#[derive(Clone, Debug, Default)]
pub struct Recording {
    /// `keys[i]` is held during step `i + 1`.
    pub keys: Vec<Buttons>,
    /// Each [`Action::Shot`]; the picture is the output of step `frame`.
    pub moments: Vec<Moment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub name: &'static str,
    pub frame: u32,
    pub map: MapId,
}

impl Recording {
    /// The buttons held during step `frame + 1` (nothing past the end).
    pub fn keys_at(&self, frame: u32) -> Buttons {
        self.keys
            .get(frame as usize)
            .copied()
            .unwrap_or(Buttons::NONE)
    }

    pub fn frames(&self) -> u32 {
        self.keys.len() as u32
    }

    /// Ascending frames to compare pictures at: every moment, the first frames and one every
    /// `every` frames.
    pub fn checkpoints(&self, every: u32) -> Vec<u32> {
        let mut c: Vec<u32> = self.moments.iter().map(|m| m.frame).collect();
        c.extend([1, 2]);
        c.extend((1..=self.frames() / every).map(|i| i * every));
        c.retain(|&f| (1..=self.frames()).contains(&f));
        c.sort_unstable();
        c.dedup();
        c
    }
}

/// Frames an action may take before the run is declared stuck.
const STUCK: u32 = 600;
/// Frames a first message may take to print before [`Action::Talk`] starts pressing A.
const PRINT_WAIT: u32 = 400;

fn dir_key(d: Dir) -> Buttons {
    match d {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    }
}

const DIRS: [Dir; 4] = [Dir::Down, Dir::Up, Dir::Left, Dir::Right];

/// Plays `actions` from a new game and records the input.
pub fn record<G: GameData>(game: G, actions: &[Action]) -> Result<Recording, String> {
    let mut r = Recorder {
        s: Session::new(game),
        rec: Recording::default(),
    };
    for (i, &a) in actions.iter().enumerate() {
        r.run(a)
            .map_err(|e| format!("action {i} {a:?} at frame {}: {e}", r.frame()))?;
    }
    Ok(r.rec)
}

struct Recorder<G: GameData> {
    s: Session<G>,
    rec: Recording,
}

impl<G: GameData> Recorder<G> {
    fn frame(&self) -> u32 {
        self.s.world().frame()
    }

    fn world(&self) -> &World {
        self.s.world()
    }

    fn press(&mut self, keys: Buttons) {
        self.rec.keys.push(keys);
        self.s.step(keys);
    }

    /// Nothing in progress: no transition, no script, player settled.
    fn idle(&self) -> bool {
        let w = self.world();
        !w.transition_active() && !w.script_running() && w.objects().player().is_settled()
    }

    fn settle(&mut self) -> Result<(), String> {
        let start = self.frame();
        while !self.idle() {
            if self.frame() - start > STUCK {
                return Err("never settled".into());
            }
            self.press(Buttons::NONE);
        }
        Ok(())
    }

    fn run(&mut self, a: Action) -> Result<(), String> {
        let start = self.frame();
        let stuck = |r: &Self| r.frame() - start > STUCK;
        match a {
            Action::GoTo(x, y) => {
                self.settle()?;
                loop {
                    if stuck(self) {
                        let p = self.world().objects().player().pos();
                        return Err(format!("stuck at {p:?}"));
                    }
                    let p = self.world().objects().player();
                    if p.pos() == (x, y) {
                        // Stop holding; finish the last step.
                        self.press(Buttons::NONE);
                        self.settle()?;
                        if self.world().objects().player().pos() == (x, y) {
                            return Ok(());
                        }
                        continue;
                    }
                    let keys = match self.next_dir((x, y)) {
                        Some(d) => dir_key(d),
                        None if self.world().objects().player().is_settled() => {
                            return Err(format!("no path from {:?}", p.pos()));
                        }
                        None => Buttons::NONE,
                    };
                    self.press(keys);
                }
            }
            Action::Enter(d) => {
                self.settle()?;
                while !self.world().transition_active() {
                    if stuck(self) {
                        return Err("no warp started".into());
                    }
                    self.press(dir_key(d));
                }
                self.settle()
            }
            Action::Cross(d) => {
                self.settle()?;
                let map = self.world().map();
                while self.world().map() == map {
                    if stuck(self) {
                        return Err("no crossing".into());
                    }
                    self.press(dir_key(d));
                }
                self.press(Buttons::NONE);
                self.settle()
            }
            Action::Face(d) => {
                self.settle()?;
                let at = self.world().objects().player().pos();
                while self.world().objects().player().facing != d {
                    if stuck(self) {
                        return Err("never turned".into());
                    }
                    self.press(dir_key(d));
                }
                self.press(Buttons::NONE);
                self.settle()?;
                if self.world().objects().player().pos() != at {
                    return Err("moved while turning".into());
                }
                Ok(())
            }
            Action::Talk(shot) => {
                self.settle()?;
                self.press(Buttons::A);
                if !self.world().script_running() {
                    self.press(Buttons::NONE);
                }
                if !self.world().script_running() {
                    return Err("nothing to talk to".into());
                }
                // Let the first message print at its own speed and name that moment.
                let printing = start + PRINT_WAIT;
                while self.world().script_running()
                    && (!self.world().text().is_open() || self.world().text().is_printing())
                    && self.frame() < printing
                {
                    self.press(Buttons::NONE);
                }
                if let Some(name) = shot {
                    self.run(Action::Shot(name))?;
                }
                let mut i = 0u32;
                while self.world().script_running() {
                    if self.frame() - start > 4 * STUCK {
                        return Err("script never ended".into());
                    }
                    // A for one frame in eight: advances waiting text, never held.
                    self.press(if i.is_multiple_of(8) {
                        Buttons::A
                    } else {
                        Buttons::NONE
                    });
                    i += 1;
                }
                self.settle()
            }
            Action::Wait(n) => {
                for _ in 0..n {
                    self.press(Buttons::NONE);
                }
                Ok(())
            }
            Action::Shot(name) => {
                let (frame, map) = (self.frame(), self.world().map());
                self.rec.moments.push(Moment { name, frame, map });
                Ok(())
            }
        }
    }

    /// First direction of a shortest path from the player's cell to `to`.
    fn next_dir(&self, to: (i16, i16)) -> Option<Dir> {
        let game = self.s.game();
        let w = self.world();
        let map = game.map(w.map());
        let from = w.objects().player().pos();
        let (mw, mh) = (map.width as i16, map.height as i16);
        let idx = |(x, y): (i16, i16)| y as usize * mw as usize + x as usize;
        let occupied: Vec<(i16, i16)> = w
            .objects()
            .iter()
            .filter(|o| o.local_id != PLAYER)
            .flat_map(|o| [o.pos(), o.prev])
            .collect();
        let walkable = |c: (i16, i16)| {
            if !in_bounds(map, c.0, c.1) || occupied.contains(&c) {
                return false;
            }
            let b = block_at(game, map, c.0, c.1);
            block_collision(b) == 0
                && !matches!(
                    block_behavior(game, map, b),
                    Behavior::Blocked | Behavior::Door | Behavior::Warp
                )
        };
        let mut queue = VecDeque::new();
        if !in_bounds(map, from.0, from.1) || !in_bounds(map, to.0, to.1) {
            return None;
        }
        queue.push_back((from, None::<Dir>));
        let mut seen = vec![false; (mw * mh) as usize];
        seen[idx(from)] = true;
        while let Some((c, d0)) = queue.pop_front() {
            if c == to {
                return d0;
            }
            for d in DIRS {
                let (dx, dy) = d.delta();
                let mut n = (c.0 + dx, c.1 + dy);
                if !in_bounds(map, n.0, n.1) {
                    continue;
                }
                if let Behavior::Ledge(ld) =
                    block_behavior(game, map, block_at(game, map, n.0, n.1))
                {
                    if ld != d {
                        continue;
                    }
                    n = (n.0 + dx, n.1 + dy);
                } else if !walkable(n) && n != to {
                    continue;
                }
                if !in_bounds(map, n.0, n.1) || seen[idx(n)] || (n != to && !walkable(n)) {
                    continue;
                }
                seen[idx(n)] = true;
                queue.push_back((n, Some(d0.unwrap_or(d))));
            }
        }
        None
    }
}

/// The parity route on the extracted maps (Littleroot → Brendan's house → Route 101 → Oldale →
/// an NPC → the Pokémon Center). See `docs` for the map coordinates.
pub const EMERALD_ROUTE: &[Action] = &[
    Action::Wait(24),
    Action::Shot("littleroot_start"),
    // Brendan's house: the door is right above the start cell.
    Action::Enter(Dir::Up),
    Action::Shot("brendans_house"),
    // Back out over the doormat.
    Action::GoTo(8, 8),
    Action::Enter(Dir::Down),
    Action::Shot("littleroot_exit"),
    // North exit to Route 101.
    Action::GoTo(10, 0),
    Action::Cross(Dir::Up),
    Action::Shot("route101"),
    Action::GoTo(10, 0),
    Action::Cross(Dir::Up),
    Action::Shot("oldale"),
    // The man by the east exit (`OldaleTown_Obj1`, a plain `msgbox MSGBOX_NPC`).
    Action::GoTo(15, 11),
    Action::Face(Dir::Right),
    Action::Talk(Some("oldale_talk")),
    Action::Shot("oldale_after_talk"),
    // The Pokémon Center.
    Action::GoTo(6, 17),
    Action::Enter(Dir::Up),
    Action::Wait(8),
    Action::Shot("pokemon_center"),
    Action::Wait(60),
];

/// The route on the built-in stub world (one 16x16 map without events).
pub const STUB_ROUTE: &[Action] = &[
    Action::Wait(24),
    Action::Shot("stub_start"),
    Action::GoTo(10, 9),
    Action::Shot("stub_east"),
    Action::GoTo(3, 3),
    Action::GoTo(5, 12),
    Action::Shot("stub_end"),
];

/// The route for the current content build.
pub fn route() -> &'static [Action] {
    if content::STUB {
        STUB_ROUTE
    } else {
        EMERALD_ROUTE
    }
}

/// Records [`route`] on [`content::Emerald`].
pub fn record_route() -> Result<Recording, String> {
    record(content::Emerald, route())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_route_records() {
        let rec = record_route().unwrap_or_else(|e| panic!("{e}"));
        assert!(rec.frames() > 100);
        println!("{} frames; moments {:?}", rec.frames(), rec.moments);
        let maps: Vec<u16> = rec.moments.iter().map(|m| m.map.0).collect();
        if content::STUB {
            assert_eq!(rec.moments.len(), 3);
        } else {
            use content::consts::*;
            let want = [
                MAP_LITTLEROOT_TOWN,
                MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F,
                MAP_LITTLEROOT_TOWN,
                MAP_ROUTE101,
                MAP_OLDALE_TOWN,
                MAP_OLDALE_TOWN,
                MAP_OLDALE_TOWN,
                MAP_OLDALE_TOWN_POKEMON_CENTER_1F,
            ];
            assert_eq!(maps, want.map(|m| m.0));
            assert!((1500..=3000).contains(&rec.frames()));
        }
    }

    #[test]
    fn a_replay_matches_the_recording() {
        let rec = record_route().unwrap();
        let (mut a, mut b) = (
            Session::new(content::Emerald),
            Session::new(content::Emerald),
        );
        for f in 0..rec.frames() {
            a.step(rec.keys_at(f));
            b.step(rec.keys_at(f));
        }
        assert_eq!(a.world().hash(), b.world().hash());
    }

    #[test]
    fn checkpoints_ascend_within_the_run() {
        let rec = record_route().unwrap();
        let c = rec.checkpoints(150);
        assert!(c.windows(2).all(|w| w[0] < w[1]));
        assert!(c.iter().all(|&f| (1..=rec.frames()).contains(&f)));
    }
}
