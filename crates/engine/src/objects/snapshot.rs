//! [`Objects`] to and from its checkpoint DTO (Milestone 5 Task B).

use super::{Action, ActionKind, Idle, Object, Objects, RunState, anim::Anim};
use crate::checkpoint::{
    ActionKindSnap, ActionSnap, IdleSnap, ObjectSnap, ObjectsSnap, RunStateSnap,
};

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
        }
    }

    /// First object (local id) with a live movement script.
    pub fn any_moves(&self) -> Option<u8> {
        self.objs
            .iter()
            .find(|o| o.moves.is_some())
            .map(|o| o.local_id)
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
                IdleSnap::Wait(n) => Idle::Wait(n),
                IdleSnap::Done => Idle::Done,
            },
            anim: Anim::from_snapshot(s.anim),
            vram: None,
            uploaded: None,
        }
    }
}
