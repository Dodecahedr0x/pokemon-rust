//! [`Objects`] to and from its checkpoint DTO (Milestone 5 Task B).

use super::{
    Action, ActionKind, Idle, JUMP_Y_HIGH, JUMP_Y_NORMAL, Object, Objects, RunState, anim::Anim,
};
use crate::checkpoint::{
    ActionKindSnap, ActionSnap, IdleSnap, ObjectSnap, ObjectsSnap, RunStateSnap,
};
use crate::script::Speed;

/// `IdleSnap` has no variant for a walk sequence's index (`Idle::Seq`, Milestone 8), so it is
/// stored as `IdleSnap::Wait(SEQ_SNAP | n)`: no movement delay comes near (Gen3's longest is 192).
const SEQ_SNAP: u16 = 0xFF00;

/// Checks an action from an untrusted snapshot against what `Object::begin` and `tick` can
/// leave between frames, so stepping it cannot panic: `t` frames played (at least the first,
/// and fewer than the action lasts), an offset back along `dir` that the action can have, and
/// the jump height its table gives for `t`.
fn check_action(a: &ActionSnap) -> Result<(), &'static str> {
    // (frames, cells of offset at most, jump table and its index shift)
    let (frames, cells, jump): (u8, i16, Option<(&[i8; 16], u8)>) = match a.kind {
        ActionKindSnap::Walk(Speed::Slow) => (31, 1, None),
        ActionKindSnap::Walk(Speed::Normal) => (16, 1, None),
        ActionKindSnap::Walk(Speed::Fast) | ActionKindSnap::Run => (8, 1, None),
        ActionKindSnap::Walk(Speed::Faster) => (4, 1, None),
        ActionKindSnap::InPlace(n) => (n, 0, None),
        ActionKindSnap::Jump(0) => (16, 0, Some((&JUMP_Y_HIGH, 0))),
        ActionKindSnap::Jump(1) => (16, 1, Some((&JUMP_Y_NORMAL, 0))),
        ActionKindSnap::Jump(2) => (32, 2, Some((&JUMP_Y_HIGH, 1))),
        ActionKindSnap::Jump(_) => return Err("jump of more than 2 cells"),
        ActionKindSnap::Delay(n) => (n.max(1), 0, None),
    };
    if !(1..frames).contains(&a.t) {
        return Err("action frame out of range");
    }
    let (dx, dy) = a.dir.delta();
    let k = -(a.off.0 * dx + a.off.1 * dy);
    if a.off != (-k * dx, -k * dy) || !(0..=16 * cells).contains(&k) {
        return Err("action offset inconsistent with its direction");
    }
    let want = jump.map_or(0, |(table, shift)| table[((a.t - 1) >> shift) as usize]);
    if a.jump_y != want {
        return Err("jump height off its table");
    }
    Ok(())
}

impl Objects {
    /// Every hashed field. Live movement scripts are not kept: capture requires
    /// [`Objects::any_moves`] to be `None`.
    pub(crate) fn snapshot(&self) -> ObjectsSnap {
        debug_assert!(
            self.any_moves().is_none(),
            "movement scripts are not captured"
        );
        ObjectsSnap {
            map: self.map,
            run_state: match self.run_state {
                RunState::NotMoving => RunStateSnap::NotMoving,
                RunState::TurnDirection => RunStateSnap::TurnDirection,
                RunState::Moving => RunStateSnap::Moving,
            },
            objs: self.objs.iter().map(Object::snapshot).collect(),
        }
    }

    /// Checks a snapshot from an untrusted source (forged, or another engine build) so that
    /// restoring and stepping it cannot panic: every action and animation is one the live
    /// objects can be in between frames.
    pub(crate) fn check_snapshot(s: &ObjectsSnap) -> Result<(), &'static str> {
        for o in &s.objs {
            if let Some(a) = &o.action {
                check_action(a)?;
            }
            Anim::check_snapshot(o.anim)?;
        }
        Ok(())
    }

    /// `vram`, `uploaded` and `banks` start empty; the boot frame's `draw` refills them.
    pub(crate) fn from_snapshot(s: &ObjectsSnap) -> Objects {
        Objects {
            objs: s.objs.iter().map(Object::from_snapshot).collect(),
            map: s.map,
            run_state: match s.run_state {
                RunStateSnap::NotMoving => RunState::NotMoving,
                RunStateSnap::TurnDirection => RunState::TurnDirection,
                RunStateSnap::Moving => RunState::Moving,
            },
            banks: [None; 16],
            emotes: alloc::vec::Vec::new(),
            approach: None,
        }
    }

    /// First object (local id) with a live movement script, else the one a trainer approach or
    /// an emote bubble is about (neither is captured either).
    pub fn any_moves(&self) -> Option<u8> {
        self.objs
            .iter()
            .find(|o| o.moves.is_some())
            .map(|o| o.local_id)
            .or_else(|| self.approaching())
            .or_else(|| self.emotes.first().map(|e| e.local_id))
    }
}

impl Object {
    fn snapshot(&self) -> ObjectSnap {
        ObjectSnap {
            local_id: self.local_id,
            map: self.map,
            gfx: self.gfx,
            pos: (self.x, self.y),
            prev: self.prev,
            elevation: self.elevation,
            facing: self.facing,
            movement: self.movement,
            home: self.home,
            range: self.range,
            visible: self.visible,
            frozen: self.frozen,
            facing_locked: self.facing_locked,
            action: self.action.map(|a| ActionSnap {
                kind: match a.kind {
                    ActionKind::Walk(s) => ActionKindSnap::Walk(s),
                    ActionKind::Run => ActionKindSnap::Run,
                    ActionKind::InPlace(n) => ActionKindSnap::InPlace(n),
                    ActionKind::Jump(n) => ActionKindSnap::Jump(n),
                    ActionKind::Delay(n) => ActionKindSnap::Delay(n),
                },
                dir: a.dir,
                t: a.t,
                off: a.off,
                jump_y: a.jump_y,
            }),
            idle: match self.idle {
                Idle::Start => IdleSnap::Start,
                Idle::Wait(n) => IdleSnap::Wait(n),
                Idle::Done => IdleSnap::Done,
                Idle::Seq(n) => IdleSnap::Wait(SEQ_SNAP | n as u16),
            },
            anim: self.anim.snapshot(),
        }
    }

    fn from_snapshot(s: &ObjectSnap) -> Object {
        Object {
            local_id: s.local_id,
            map: s.map,
            gfx: s.gfx,
            x: s.pos.0,
            y: s.pos.1,
            prev: s.prev,
            elevation: s.elevation,
            facing: s.facing,
            movement: s.movement,
            home: s.home,
            range: s.range,
            visible: s.visible,
            frozen: s.frozen,
            facing_locked: s.facing_locked,
            action: s.action.map(|a| Action {
                kind: match a.kind {
                    ActionKindSnap::Walk(s) => ActionKind::Walk(s),
                    ActionKindSnap::Run => ActionKind::Run,
                    ActionKindSnap::InPlace(n) => ActionKind::InPlace(n),
                    ActionKindSnap::Jump(n) => ActionKind::Jump(n),
                    ActionKindSnap::Delay(n) => ActionKind::Delay(n),
                },
                dir: a.dir,
                t: a.t,
                off: a.off,
                jump_y: a.jump_y,
            }),
            moves: None,
            idle: match s.idle {
                IdleSnap::Start => Idle::Start,
                IdleSnap::Wait(n) if n & SEQ_SNAP == SEQ_SNAP => Idle::Seq(n as u8),
                IdleSnap::Wait(n) => Idle::Wait(n),
                IdleSnap::Done => Idle::Done,
            },
            anim: Anim::from_snapshot(s.anim),
            vram: None,
            uploaded: None,
        }
    }
}
