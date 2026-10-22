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
use crate::trace::TraceRecorder;
use engine::battle::scene::OpenMenu;
use engine::battle::{BattleKind, BattleOutcome, Str};
use engine::data::{Behavior, Dir, GameData, MapId, PLAYER, block_collision};
use engine::flow::block_behavior;
use engine::grid::{block_at, in_bounds};
use engine::{Buttons, World};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub enum Action {
    /// Walk to a cell of the current map.
    GoTo(i16, i16),
    /// Take one step that way and let what it triggers (a coord script) run on.
    Step(Dir),
    /// Hold a direction until a warp transition has run (doors, doormats, stairs).
    Enter(Dir),
    /// Hold a direction until the player crosses into the connected map, then settle.
    Cross(Dir),
    /// Turn to face a direction without moving (the cell ahead must be blocked).
    Face(Dir),
    /// Press A on what is ahead; with a name, let the first message print at its own speed and
    /// name that moment; then hold A, releasing it one frame in eight, until the script ends.
    Talk(Option<&'static str>),
    /// Hold nothing for this many frames.
    Wait(u32),
    /// Name the current frame (a screenshot and a parity picture frame).
    Shot(&'static str),
    /// Press A once on what is ahead and let the script run on (no waiting for its end).
    Interact,
    /// Alternate A and nothing until the field is idle again: scripts run on, battles fight
    /// with the first move.
    Mash,
    /// Walk back and forth between two cells (tall grass) until a battle starts.
    Grass((i16, i16), (i16, i16)),
    /// Wait for the starter screen to be up (a second in), then name that moment.
    StarterShot(&'static str),
    /// Wait for the starter screen, pick ball `i` (0..3) and confirm it.
    Starter(u8),
    /// Battle with move `slot` every turn, mashing through messages, until the field is back.
    Fight(u8),
    /// Battle: RUN every turn until the field is back; the battle must end `Ran`.
    Run,
    /// [`Action::Grass`] then [`Action::Fight`] with move `slot`, again and again, until a
    /// "grew to level" message shows; name that moment, then fight that battle out.
    LevelUp((i16, i16), (i16, i16), u8, &'static str),
    /// Battle with move `slot` until `When`, then name that moment.
    BattleShot(u8, When, &'static str),
    /// Press nothing until the world is capturable (`World::capturable`), then name that frame
    /// a checkpoint node ([`Recording::nodes`]).
    Checkpoint(&'static str),
}

/// A battle moment for [`Action::BattleShot`].
#[derive(Clone, Copy, Debug)]
pub enum When {
    /// The action menu is open.
    ActionMenu,
    /// The move menu is open.
    MoveMenu,
    /// An HP bar is draining (a few frames in).
    HpDrain,
    /// This message is printed (up to a second in, while it is still shown).
    Message(Str),
}

/// Most steps between a bulk step and the engine frame `run_rom` reads its overrun at: the
/// overrunning step `b`'s Frame is applied late, counted, and published after step `b + 1`;
/// the emulator frame boundary may fall mid-step, and the next step may be slow too (a
/// following chunk).
pub const DISPLAY_LAG: u32 = 3;

/// What [`record`] produced.
#[derive(Clone, Debug, Default)]
pub struct Recording {
    /// `keys[i]` is held during step `i + 1`.
    pub keys: Vec<Buttons>,
    /// Each [`Action::Shot`]; the picture is the output of step `frame`.
    pub moments: Vec<Moment>,
    /// `World::hash` after each step (`hashes[i]` after step `i + 1`).
    pub hashes: Vec<u64>,
    /// Each [`Action::Checkpoint`]: its name and the (capturable) frame it was taken at.
    pub nodes: Vec<(&'static str, u32)>,
    /// Warp transitions started.
    pub warps: u32,
    /// Battles started.
    pub battles: u32,
    /// Each battle started, in order, with how it ended.
    pub battle_log: Vec<BattleRecord>,
    /// Starter screens opened.
    pub starters: u32,
    /// The steps whose [`Frame::bulk`](engine::video::Frame::bulk) is set (ascending): the
    /// only steps the GBA may overrun, on a black screen (see [`Recording::bulk_events`]).
    pub bulk: Vec<u32>,
}

/// A battle of a [`Recording`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleRecord {
    pub kind: BattleKind,
    /// The scene's outcome once it has one (`None`: still running when the recording ended).
    pub outcome: Option<BattleOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub name: &'static str,
    pub frame: u32,
    pub map: MapId,
}

impl Recording {
    /// Runs of consecutive bulk steps: one per map load, whole-view redraw, battle load or
    /// restore, starter load. Each may overrun the GBA's frame once (`late_black`), however
    /// many chunks it takes, since every chunk fits a frame but the first may follow a full one.
    pub fn bulk_events(&self) -> u32 {
        let starts = self.bulk.windows(2).filter(|w| w[1] != w[0] + 1).count();
        if self.bulk.is_empty() {
            0
        } else {
            starts as u32 + 1
        }
    }

    /// The bulk step that explains a `late_black` the ROM reported at engine frame `late`
    /// (`run_rom` reads the counter after the step following the overrun, so the overrunning
    /// step is one to [`DISPLAY_LAG`] steps earlier).
    pub fn bulk_before(&self, late: u32) -> Option<u32> {
        self.bulk
            .iter()
            .copied()
            .rev()
            .find(|&b| b < late && late - b <= DISPLAY_LAG)
    }

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

    /// The recorded input of each [`Recording::nodes`] entry, chained from a new game: entry
    /// `k` starts where entry `k - 1` ends (the first at a new game), so its parent is the node
    /// of entry `k - 1`. A store gives that node its id, so the caller makes each trace
    /// ([`TraceRecorder::trace`]) once it knows the parent.
    pub fn node_inputs(&self) -> Vec<(&'static str, TraceRecorder)> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut start = 0u32;
        for &(name, frame) in &self.nodes {
            let mut r = TraceRecorder::new(start);
            for i in start..frame {
                r.push(self.keys_at(i), self.hashes[i as usize]);
            }
            out.push((name, r));
            start = frame;
        }
        out
    }

    /// Ascending frames to compare pictures at: every moment, the first frames and one every
    /// `every` frames.
    pub fn picture_frames(&self, every: u32) -> Vec<u32> {
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
        last: Buttons::NONE,
    };
    for (i, &a) in actions.iter().enumerate() {
        r.run(a)
            .map_err(|e| {
                let w = r.world();
                let log: Vec<&str> = w.log().lines().collect();
                format!(
                    "action {i} {a:?} at frame {}: {e} (map {:#x} at {:?}, script {}, battle {:?}; log {log:?})",
                    r.frame(),
                    w.map().0,
                    w.objects().player().pos(),
                    w.script_running(),
                    w.battle().map(|b| b.0),
                )
            })?;
    }
    Ok(r.rec)
}

struct Recorder<G: GameData> {
    s: Session<G>,
    rec: Recording,
    /// The buttons of the last step.
    last: Buttons,
}

/// Frames a battle may take before the run is declared stuck.
const BATTLE_STUCK: u32 = 12_000;

/// Idle frames in a row that end an [`Action::Mash`].
const MASH_CALM: u32 = 16;

impl<G: GameData> Recorder<G> {
    fn frame(&self) -> u32 {
        self.s.world().frame()
    }

    fn world(&self) -> &World {
        self.s.world()
    }

    fn press(&mut self, keys: Buttons) {
        let before = (
            self.world().transition_active(),
            self.world().in_battle(),
            self.world().starter().is_some(),
        );
        self.rec.keys.push(keys);
        self.s.step(keys);
        self.rec.hashes.push(self.world().hash());
        self.last = keys;
        if !before.0 && self.world().transition_active() {
            self.rec.warps += 1;
        }
        if let Some((_, battle, scene)) = self.world().battle() {
            let (kind, outcome) = (battle.setup().kind, scene.finished());
            if !before.1 {
                self.rec.battles += 1;
                self.rec.battle_log.push(BattleRecord { kind, outcome });
            } else if let Some(last) = self.rec.battle_log.last_mut() {
                last.outcome = outcome.or(last.outcome);
            }
        }
        if !before.2 && self.world().starter().is_some() {
            self.rec.starters += 1;
        }
        if self.s.frame().bulk {
            let f = self.frame();
            self.rec.bulk.push(f);
        }
    }

    /// Presses `keys` as a new press: a frame of nothing first if something is held. Nothing
    /// is pressed while a battle or a warp has the screen black: their bulk loads may overrun
    /// a frame there, and the ROM then reads one input for two steps (`late_black`), which
    /// must not change.
    fn tap(&mut self, keys: Buttons) {
        let w = self.world();
        let black = (w.in_battle() || w.transition_active()) && self.s.frame().is_black();
        if self.last != Buttons::NONE || black {
            self.press(Buttons::NONE);
        } else {
            self.press(keys);
        }
    }

    /// Nothing in progress: the field owns the frame, no transition, no script, player settled.
    fn idle(&self) -> bool {
        let w = self.world();
        w.field_active()
            && !w.transition_active()
            && !w.script_running()
            && w.objects().player().is_settled()
    }

    /// The next button for a battle: move `slot` (or RUN when `run`) in the menus, no to
    /// learning a move, A through messages.
    fn battle_key(&self, slot: u8, run: bool) -> Buttons {
        let toward = |c: u8, want: u8| {
            if c & 1 != want & 1 {
                if want & 1 == 0 {
                    Buttons::LEFT
                } else {
                    Buttons::RIGHT
                }
            } else if c & 2 != want & 2 {
                if want & 2 == 0 {
                    Buttons::UP
                } else {
                    Buttons::DOWN
                }
            } else {
                Buttons::A
            }
        };
        match self.world().battle().and_then(|b| b.2.open_menu()) {
            Some(OpenMenu::Action(c)) => toward(c, if run { 3 } else { 0 }),
            Some(OpenMenu::Move(_)) if run => Buttons::B,
            Some(OpenMenu::Move(c)) => toward(c, slot),
            // Keep the known moves (Gen3: "Stop learning X?" yes).
            Some(OpenMenu::LearnAsk(_) | OpenMenu::LearnPick(_)) => Buttons::B,
            Some(OpenMenu::StopAsk(0)) => Buttons::A,
            Some(OpenMenu::StopAsk(_)) => Buttons::UP,
            None => Buttons::A,
        }
    }

    /// Battles until the field is back; gives the battle's outcome.
    fn battle(&mut self, slot: u8, run: bool) -> Result<BattleOutcome, String> {
        let start = self.frame();
        let mut outcome = None;
        while self.world().in_battle() {
            if self.frame() - start > BATTLE_STUCK {
                return Err("battle never ended".into());
            }
            let k = self.battle_key(slot, run);
            self.tap(k);
            if let Some((_, _, scene)) = self.world().battle() {
                outcome = scene.finished().or(outcome);
            }
        }
        outcome.ok_or_else(|| "the battle ended without an outcome".into())
    }

    /// Battles with move `slot` (through a script's lead-in) until `when` or the battle's end:
    /// whether `when` came.
    fn battle_until(&mut self, slot: u8, when: When) -> Result<bool, String> {
        let start = self.frame();
        let mut started = false;
        while !self.battle_moment(when) {
            let w = self.world();
            started |= w.in_battle();
            let lead_in = !w.in_battle() && w.script_running() && !started;
            if !(w.in_battle() || lead_in) {
                return Ok(false);
            }
            if self.frame() - start > BATTLE_STUCK {
                return Err(format!("no {when:?} in the battle"));
            }
            // A script leading into a battle: A through its messages.
            let k = if lead_in {
                Buttons::A
            } else {
                self.battle_key(slot, false)
            };
            self.tap(k);
        }
        Ok(true)
    }

    /// Lets a battle moment show (a message prints, a bar drains a little), then names it.
    fn battle_shot(&mut self, when: When, name: &'static str) -> Result<(), String> {
        let wait = match when {
            When::Message(_) => 60,
            When::HpDrain => 4,
            // The menu's window and cursor are drawn.
            When::ActionMenu | When::MoveMenu => 8,
        };
        for _ in 0..wait {
            if !self.battle_moment(when) {
                break;
            }
            self.press(Buttons::NONE);
        }
        self.run(Action::Shot(name))
    }

    /// Whether a battle moment has come.
    fn battle_moment(&self, when: When) -> bool {
        let Some((_, _, scene)) = self.world().battle() else {
            return false;
        };
        match when {
            When::ActionMenu => matches!(scene.open_menu(), Some(OpenMenu::Action(_))),
            When::MoveMenu => matches!(scene.open_menu(), Some(OpenMenu::Move(_))),
            When::HpDrain => scene.draining_hp(),
            When::Message(m) => scene.message() == Some(m),
        }
    }

    fn settle(&mut self) -> Result<(), String> {
        let start = self.frame();
        while !self.idle() {
            if self.world().in_battle() {
                // A wild encounter on the way: fight it with the first move.
                self.battle(0, false)?;
                return self.settle();
            }
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
                    if self.world().in_battle() {
                        // An encounter in the grass on the way: fight it, then walk on.
                        self.settle()?;
                        return self.run(a);
                    }
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
                // A step-end script (a coord event) runs on: [`Action::Mash`] it.
                while !self.world().script_running() && !self.idle() {
                    if stuck(self) {
                        return Err("never settled after the crossing".into());
                    }
                    self.press(Buttons::NONE);
                }
                Ok(())
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
                while shot.is_some()
                    && self.world().script_running()
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
                    // A held for 7 frames in 8: each new press advances waiting text, and
                    // holding it prints one glyph per frame (Gen3 speed-up).
                    self.press(if i % 8 == 7 {
                        Buttons::NONE
                    } else {
                        Buttons::A
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
            Action::Checkpoint(name) => {
                while let Err(why) = self.world().capturable() {
                    if stuck(self) {
                        return Err(format!("never capturable ({why:?})"));
                    }
                    self.press(Buttons::NONE);
                }
                let frame = self.frame();
                self.rec.nodes.push((name, frame));
                Ok(())
            }
            Action::Step(d) => {
                self.settle()?;
                let at = self.world().objects().player().pos();
                while self.world().objects().player().pos() == at {
                    if stuck(self) {
                        return Err("never stepped".into());
                    }
                    self.press(dir_key(d));
                }
                self.press(Buttons::NONE);
                while !self.world().script_running() && !self.idle() {
                    if stuck(self) {
                        return Err("never settled after the step".into());
                    }
                    self.press(Buttons::NONE);
                }
                Ok(())
            }
            Action::Interact => {
                self.settle()?;
                self.press(Buttons::A);
                self.press(Buttons::NONE);
                if !self.world().script_running() {
                    return Err("nothing to interact with".into());
                }
                Ok(())
            }
            Action::Mash => {
                // Idle for a few frames in a row: a map's on-frame script starts a little
                // after its warp ends.
                let mut calm = 0;
                while calm < MASH_CALM {
                    if self.frame() - start > BATTLE_STUCK {
                        return Err("never idle".into());
                    }
                    if self.idle() {
                        calm += 1;
                        self.press(Buttons::NONE);
                    } else if self.world().in_battle() {
                        calm = 0;
                        self.battle(0, false).map(drop)?;
                    } else {
                        calm = 0;
                        self.tap(Buttons::A);
                    }
                }
                Ok(())
            }
            Action::Grass(a, b) => {
                self.settle()?;
                let mut to = a;
                while !self.world().in_battle() {
                    if self.frame() - start > 4 * STUCK {
                        return Err("no encounter".into());
                    }
                    let p = self.world().objects().player();
                    if p.pos() == to && p.is_settled() {
                        to = if to == a { b } else { a };
                    }
                    let keys = self.next_dir(to).map_or(Buttons::NONE, dir_key);
                    self.press(keys);
                }
                Ok(())
            }
            Action::StarterShot(name) => {
                while self.world().starter().is_none() {
                    if stuck(self) {
                        return Err("no starter screen".into());
                    }
                    self.press(Buttons::NONE);
                }
                for _ in 0..60 {
                    self.press(Buttons::NONE);
                }
                self.run(Action::Shot(name))
            }
            Action::Starter(i) => {
                while self.world().starter().is_none() {
                    if stuck(self) {
                        return Err("no starter screen".into());
                    }
                    self.press(Buttons::NONE);
                }
                while let Some(sel) = self.world().starter().map(|s| s.selection()) {
                    if self.frame() - start > 4 * STUCK {
                        return Err("starter never chosen".into());
                    }
                    let k = match sel.cmp(&i) {
                        std::cmp::Ordering::Less => Buttons::RIGHT,
                        std::cmp::Ordering::Greater => Buttons::LEFT,
                        std::cmp::Ordering::Equal => Buttons::A,
                    };
                    self.tap(k);
                }
                Ok(())
            }
            Action::Fight(slot) => {
                if !self.world().in_battle() {
                    return Err("not in a battle".into());
                }
                self.battle(slot, false).map(drop)
            }
            Action::Run => {
                if !self.world().in_battle() {
                    return Err("not in a battle".into());
                }
                match self.battle(0, true)? {
                    BattleOutcome::Ran => Ok(()),
                    o => Err(format!("ran from the battle but it ended {o:?}")),
                }
            }
            Action::BattleShot(slot, when, name) => {
                if !self.battle_until(slot, when)? {
                    return Err(format!("no {when:?} in the battle"));
                }
                self.battle_shot(when, name)
            }
            Action::LevelUp(a, b, slot, name) => {
                let when = When::Message(Str::PkmnGrewToLv);
                loop {
                    if self.frame() - start > 8 * BATTLE_STUCK {
                        return Err("never grew a level".into());
                    }
                    self.run(Action::Grass(a, b))?;
                    if self.battle_until(slot, when)? {
                        self.battle_shot(when, name)?;
                        return self.run(Action::Fight(slot));
                    }
                    self.settle()?;
                }
            }
        }
    }

    /// First direction of a shortest path from the player's cell to `to`.
    /// The first step toward `to`, through tall grass only when there is no other way.
    fn next_dir(&self, to: (i16, i16)) -> Option<Dir> {
        self.path_dir(to, true).or_else(|| self.path_dir(to, false))
    }

    fn path_dir(&self, to: (i16, i16), avoid_grass: bool) -> Option<Dir> {
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
            let behavior = block_behavior(game, map, b);
            block_collision(b) == 0
                && !matches!(
                    behavior,
                    Behavior::Blocked | Behavior::Door | Behavior::Warp
                )
                && !(avoid_grass && behavior == Behavior::TallGrass)
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

/// The parity route on the extracted maps, from a new game before Birch's rescue.
/// Coordinates are the maps' own (`content/extracted/maps/*.ron`).
pub const EMERALD_ROUTE: &[Action] = &[
    Action::Wait(24),
    Action::Shot("littleroot_start"),
    // North: the girl stops the player (`LittlerootTown_Coord2`: Birch is in trouble).
    Action::GoTo(11, 2),
    Action::Step(Dir::Up),
    Action::Mash,
    Action::GoTo(10, 0),
    Action::Cross(Dir::Up),
    // `Route101_Coord0`: Birch chased by the Zigzagoon.
    Action::Mash,
    Action::Shot("route101_rescue"),
    // Birch's bag (`Route101_Obj3`): `special ChooseStarter`, then the first battle.
    Action::GoTo(7, 15),
    Action::Face(Dir::Up),
    Action::Interact,
    Action::StarterShot("starter_screen"),
    Action::Starter(2),
    Action::Fight(0),
    // Birch's thanks, the heal and the warp to his lab, where he gives Mudkip.
    Action::Mash,
    Action::Shot("lab"),
    Action::Checkpoint("lab"),
    Action::GoTo(6, 11),
    Action::Enter(Dir::Down),
    // Back north to Route 101's tall grass.
    Action::GoTo(10, 0),
    Action::Cross(Dir::Up),
    Action::Grass((12, 11), (13, 11)),
    Action::BattleShot(0, When::Message(Str::Intro1), "wild_intro"),
    Action::BattleShot(0, When::ActionMenu, "action_menu"),
    Action::BattleShot(0, When::MoveMenu, "move_menu"),
    Action::BattleShot(0, When::HpDrain, "hp_drain"),
    Action::Fight(0),
    Action::Shot("after_wild"),
    Action::Checkpoint("after_wild"),
    Action::Grass((12, 11), (13, 11)),
    Action::Run,
    Action::LevelUp((12, 11), (13, 11), 0, "level_up"),
    // Oldale: the Pokémon Center nurse (`Common_EventScript_PkmnCenterNurse`, across the
    // counter) heals the party and the respawn moves here.
    Action::GoTo(10, 0),
    Action::Cross(Dir::Up),
    Action::GoTo(6, 17),
    Action::Enter(Dir::Up),
    Action::GoTo(7, 4),
    Action::Face(Dir::Up),
    Action::Interact,
    Action::Mash,
    Action::Checkpoint("oldale_center"),
    Action::GoTo(7, 7),
    Action::Enter(Dir::Down),
    // Route 103, where May waits at (10, 3) facing right.
    Action::GoTo(10, 0),
    Action::Cross(Dir::Up),
    Action::GoTo(11, 3),
    Action::Face(Dir::Left),
    Action::Interact,
    Action::BattleShot(0, When::Message(Str::IntroTrainer), "may_intro"),
    Action::Fight(0),
    Action::Mash,
    Action::Shot("route103_after_may"),
    Action::Checkpoint("route103_after_may"),
];

/// The route on the built-in stub world (one 16x16 map without events; the new game starts
/// with a Pokémon there, see `content::Start`).
pub const STUB_ROUTE: &[Action] = &[
    Action::Wait(24),
    Action::Shot("stub_start"),
    Action::Checkpoint("stub_start"),
    Action::GoTo(10, 9),
    Action::Shot("stub_east"),
    // The one tall-grass cell, (12, 3): back and forth until a wild battle starts.
    Action::Grass((12, 3), (11, 3)),
    Action::BattleShot(0, When::Message(Str::Intro1), "stub_wild_intro"),
    Action::BattleShot(0, When::ActionMenu, "stub_action_menu"),
    Action::Fight(0),
    Action::Shot("stub_after_battle"),
    Action::Checkpoint("stub_after_battle"),
    Action::GoTo(3, 3),
    Action::GoTo(5, 12),
    Action::Shot("stub_end"),
    Action::Checkpoint("stub_end"),
];

/// The route for the current content build.
pub fn route() -> &'static [Action] {
    if content::STUB {
        STUB_ROUTE
    } else {
        EMERALD_ROUTE
    }
}

/// Records [`route`] on `content::Emerald::for_shells()` (a new game before the rescue).
/// Fails on a stale extract ([`content::STALE_EXTRACT`]): its stub would pass for real data.
pub fn record_route() -> Result<Recording, String> {
    if content::STALE_EXTRACT {
        return Err(content::STALE_EXTRACT_MSG.into());
    }
    record(content::Emerald::for_shells(), route())
}

/// [`record_route`], recorded once per process and shared (tests use it; recording the route
/// takes most of a test's time). Panics if the route does not play.
pub fn cached_route() -> &'static Recording {
    static ROUTE: std::sync::OnceLock<Recording> = std::sync::OnceLock::new();
    ROUTE.get_or_init(|| {
        record_route().unwrap_or_else(|e| panic!("the route does not play natively: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_route_records() {
        let rec = cached_route();
        assert!(rec.frames() > 100);
        println!("{} frames; moments {:?}", rec.frames(), rec.moments);
        println!("battles {:?}", rec.battle_log);
        assert_eq!(rec.battle_log.len(), rec.battles as usize);
        let names: Vec<&str> = rec.moments.iter().map(|m| m.name).collect();
        let maps: Vec<u16> = rec.moments.iter().map(|m| m.map.0).collect();
        let mut s = Session::new(content::Emerald::for_shells());
        for f in 0..rec.frames() {
            s.step(rec.keys_at(f));
        }
        let w = s.world();
        assert!(!w.in_battle());
        if content::STUB {
            assert_eq!(
                names,
                [
                    "stub_start",
                    "stub_east",
                    "stub_wild_intro",
                    "stub_action_menu",
                    "stub_after_battle",
                    "stub_end"
                ]
            );
            assert_eq!(rec.battles, 1);
            assert_eq!(
                rec.battle_log[0],
                BattleRecord {
                    kind: BattleKind::Wild {
                        first_battle: false
                    },
                    outcome: Some(BattleOutcome::Won),
                }
            );
            assert!(w.party().get(0).unwrap().exp > 135);
        } else {
            use content::consts::*;
            assert_eq!(
                names,
                [
                    "littleroot_start",
                    "route101_rescue",
                    "starter_screen",
                    "lab",
                    "wild_intro",
                    "action_menu",
                    "move_menu",
                    "hp_drain",
                    "after_wild",
                    "level_up",
                    "may_intro",
                    "route103_after_may",
                ]
            );
            let (r101, r103) = (MAP_ROUTE101.0, MAP_ROUTE103.0);
            let lab = MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB.0;
            let want = [
                MAP_LITTLEROOT_TOWN.0,
                r101,
                r101,
                lab,
                r101,
                r101,
                r101,
                r101,
            ];
            assert_eq!(maps[..8], want);
            assert_eq!(maps[8..], [r101, r101, r103, r103]);
            // The first battle, the win, the run, the level-up battle and May at least.
            assert!(rec.battles >= 5, "{} battles", rec.battles);
            let log = &rec.battle_log;
            let outcome = |i: usize| log[i].outcome;
            // Birch's rescue: Gen3 `BATTLE_TYPE_FIRST_BATTLE`, which also accepts the
            // Zigzagoon fleeing.
            assert_eq!(log[0].kind, BattleKind::Wild { first_battle: true });
            assert!(
                matches!(
                    outcome(0),
                    Some(BattleOutcome::Won | BattleOutcome::MonFled)
                ),
                "{log:?}"
            );
            // The `wild_intro` battle is won, the next one (`Action::Run`) is run from.
            assert_eq!(
                log[1].kind,
                BattleKind::Wild {
                    first_battle: false
                }
            );
            assert_eq!(outcome(1), Some(BattleOutcome::Won));
            assert_eq!(
                log[2].kind,
                BattleKind::Wild {
                    first_battle: false
                }
            );
            assert_eq!(outcome(2), Some(BattleOutcome::Ran));
            // The level-up battles (and any wild battle on the way) are won.
            let (may, wild) = log[3..].split_last().unwrap();
            for b in wild {
                assert_eq!(
                    b.kind,
                    BattleKind::Wild {
                        first_battle: false
                    }
                );
                assert_eq!(b.outcome, Some(BattleOutcome::Won), "{log:?}");
            }
            // May is the one trainer battle, and it is won.
            assert!(matches!(may.kind, BattleKind::Trainer { .. }), "{log:?}");
            assert_eq!(may.outcome, Some(BattleOutcome::Won));
            let mon = w.party().get(0).unwrap();
            assert_eq!(
                (w.party().len(), mon.species),
                (1, content::battle::STARTERS[2])
            );
            assert!(mon.level >= 6);
            assert!(w.flag(FLAG_DEFEATED_RIVAL_ROUTE103));
            // The Oldale Pokémon Center moved the respawn.
            assert_eq!(w.respawn(), HEAL_LOCATION_OLDALE_TOWN);
            assert!((10_000..=20_000).contains(&rec.frames()));
        }
    }

    #[test]
    fn a_replay_matches_the_recording() {
        let rec = cached_route();
        let (mut a, mut b) = (
            Session::new(content::Emerald::for_shells()),
            Session::new(content::Emerald::for_shells()),
        );
        for f in 0..rec.frames() {
            a.step(rec.keys_at(f));
            b.step(rec.keys_at(f));
        }
        assert_eq!(a.world().hash(), b.world().hash());
    }

    #[test]
    fn picture_frames_ascend_within_the_run() {
        let rec = cached_route();
        let c = rec.picture_frames(150);
        assert!(c.windows(2).all(|w| w[0] < w[1]));
        assert!(c.iter().all(|&f| (1..=rec.frames()).contains(&f)));
    }
}
