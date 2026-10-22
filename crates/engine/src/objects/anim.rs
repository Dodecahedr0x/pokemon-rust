//! Sprite animation, as Gen3's standard walker anim tables (`object_event_anims.h`): a looping
//! 4-command "go" animation (step, stand, other step, stand) whose position carries over from
//! one step to the next (`SetStepAnimHandleAlternation`, `animPos = {1, 3, 0, 2}`).

use super::ActionKind;
use crate::checkpoint::{AnimSnap, AnimTableSnap};
use crate::data::Dir;
use crate::hash::Fnv64;
use crate::script::Speed;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Table {
    /// `sAnim_Face*`: the facing frame.
    Face,
    /// `sAnim_Go*` with this many frames per command (8 normal, 4 fast, 2 faster; 16 for
    /// `walk_in_place_slow`, which delays the anim every other frame).
    Go(u8),
    /// `sAnim_Run*` (5, 3, 5, 3): the player's running frames 9..18.
    Run,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Anim {
    table: Table,
    cmd: u8,
    timer: u8,
}

/// Animation an action plays, `None` for actions that leave it alone.
pub(super) const fn table_for(kind: ActionKind) -> Option<Table> {
    Some(match kind {
        ActionKind::Walk(Speed::Slow | Speed::Normal) | ActionKind::Jump(_) => Table::Go(8),
        ActionKind::Walk(Speed::Fast) => Table::Go(4),
        ActionKind::Walk(Speed::Faster) => Table::Go(2),
        ActionKind::Run => Table::Run,
        ActionKind::InPlace(32) => Table::Go(16),
        ActionKind::InPlace(16) => Table::Go(8),
        ActionKind::InPlace(8) => Table::Go(4),
        ActionKind::InPlace(_) => Table::Go(2),
        ActionKind::Delay(_) => return None,
    })
}

/// Standard 9-frame layout: (go commands, face frame) for south, north, west; east is west
/// with hflip.
const GO: [[u8; 4]; 3] = [[3, 0, 4, 0], [5, 1, 6, 1], [7, 2, 8, 2]];
/// Brendan/May run frames (`sAnim_Run*`), in the second half of their 18-frame pic table.
const RUN: [[u8; 4]; 3] = [[12, 9, 13, 9], [14, 10, 15, 10], [16, 11, 17, 11]];
const RUN_DURATIONS: [u8; 4] = [5, 3, 5, 3];

impl Anim {
    pub(super) const fn new() -> Self {
        Self {
            table: Table::Face,
            cmd: 0,
            timer: 0,
        }
    }

    /// Starts a step animation, continuing the step alternation.
    pub(super) fn start(&mut self, table: Table) {
        self.table = table;
        self.cmd = match self.cmd {
            1 => 2,
            3 => 0,
            c => c,
        };
        self.timer = 0;
    }

    /// Gen3 `SetStepAnim` on the go table: show the stand frame nearest in the cycle.
    pub(super) fn stand(&mut self) {
        self.table = Table::Go(8);
        self.cmd = if self.cmd <= 1 { 1 } else { 3 };
        self.timer = 0;
    }

    /// Gen3 `ObjectEventTurn`: restart on the face animation.
    pub(super) fn turn(&mut self) {
        *self = Self::new();
    }

    fn duration(&self) -> u8 {
        match self.table {
            Table::Face => u8::MAX,
            Table::Go(d) => d,
            Table::Run => RUN_DURATIONS[self.cmd as usize],
        }
    }

    pub(super) fn tick(&mut self) {
        self.timer = self.timer.saturating_add(1);
        if self.timer > self.duration() {
            self.cmd = (self.cmd + 1) & 3;
            self.timer = 1;
        }
    }

    /// (frame index, hflip) for a sprite with `frames` frames. Sprites with fewer than 9
    /// frames show the facing frame (3+ frames) or frame 0; without the 18 run frames, running
    /// shows the walk frames.
    pub(super) fn frame(&self, facing: Dir, frames: usize) -> (usize, bool) {
        let (d, hflip) = match facing {
            Dir::Down => (0, false),
            Dir::Up => (1, false),
            Dir::Left => (2, false),
            Dir::Right => (2, true),
        };
        if frames < 3 {
            return (0, false);
        }
        if frames < 9 {
            return (d, hflip);
        }
        let c = self.cmd as usize;
        let f = match self.table {
            Table::Face => d as u8,
            Table::Run if frames >= 18 => RUN[d][c],
            Table::Go(_) | Table::Run => GO[d][c],
        };
        (f as usize, hflip)
    }

    pub(super) fn hash(&self, h: &mut Fnv64) {
        let (t, arg) = match self.table {
            Table::Face => (0, 0),
            Table::Go(d) => (1, d),
            Table::Run => (2, 0),
        };
        h.write(&[t, arg, self.cmd, self.timer]);
    }
}

// Milestone 5 Task B.
impl Anim {
    pub(super) const fn snapshot(&self) -> AnimSnap {
        AnimSnap {
            table: match self.table {
                Table::Face => AnimTableSnap::Face,
                Table::Go(d) => AnimTableSnap::Go(d),
                Table::Run => AnimTableSnap::Run,
            },
            cmd: self.cmd,
            timer: self.timer,
        }
    }

    pub(super) const fn from_snapshot(s: AnimSnap) -> Anim {
        Anim {
            table: match s.table {
                AnimTableSnap::Face => Table::Face,
                AnimTableSnap::Go(d) => Table::Go(d),
                AnimTableSnap::Run => Table::Run,
            },
            // `cmd` indexes 4-entry tables.
            cmd: s.cmd & 3,
            timer: s.timer,
        }
    }
}
