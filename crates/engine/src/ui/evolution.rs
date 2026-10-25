//! The evolution scene (Gen3 `evolution_scene.c`, simplified).
//!
//! "What? {NAME} is evolving!", the front pic alternating between the two species (no
//! sparkles), then "Congratulations!" or "stopped evolving". B cancels while the pics
//! alternate when `can_cancel` (Gen3 `canStopEvo`: level-up and friendship, not a stone or
//! a trade). The cry is [`ScreenNote::Cry`]; playing the sample is the audio gap.
//!
//! Several party mons (a battle where more than one leveled up) play one after another
//! before the screen closes.

use super::kit::widgets::Message;
use super::kit::window::{self, MapBuf};
use super::{Screen, ScreenCx, ScreenInput, ScreenNote, ScreenOut, ScreenResult};
use crate::data::{GameData, SpeciesId, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::mon::NAME_LEN;
use crate::text::Window;
use crate::video::{Cmd, Obj, ObjShape};
use alloc::boxed::Box;

/// Gen3 `FRIENDSHIP_EVO_THRESHOLD` lives with the check; the scene only plays it.
/// Frames the two pics alternate. B can cancel during these when `can_cancel`.
const FLASH_FRAMES: u16 = 48;
/// Frames each pic stays up before the swap.
const SWAP_FRAMES: u16 = 8;
const PIC_TILES: usize = 64;
/// OBJ VRAM. A full-screen screen is not showing field objects.
const PIC_TILE: u16 = 0;
const PIC_PAL: u8 = 0;
/// Centred above the message window (tile row 16).
const PIC_X: i16 = 88;
const PIC_Y: i16 = 40;
/// Message along the bottom (`gText_PkmnIsEvolving` and the two results).
const MSG: (u8, u8, u8, u8) = (1, 16, 28, 4);

/// One mon the scene will offer to evolve.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct EvoJob {
    pub slot: u8,
    pub into: SpeciesId,
    pub can_cancel: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Intro,
    Flash {
        t: u16,
    },
    /// `armed` is false on the frame the result message starts, so the button that ended
    /// the flash does not also dismiss it.
    Result {
        evolved: bool,
        armed: bool,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EvolutionScene {
    jobs: [EvoJob; 6],
    n: u8,
    i: u8,
    phase: Phase,
    /// Species before this job's change.
    pre: SpeciesId,
    pre_valid: bool,
    shown: SpeciesId,
    cried: bool,
    uploaded: Option<SpeciesId>,
    map_dirty: bool,
    msg: Option<Box<Message>>,
}

impl EvolutionScene {
    pub fn new(slot: u8, into: SpeciesId, can_cancel: bool) -> Self {
        let mut scene = Self::empty();
        scene.jobs[0] = EvoJob {
            slot,
            into,
            can_cancel,
        };
        scene.n = 1;
        scene
    }

    /// Replace the queue (a battle with more than one level-up). Called before the first step.
    pub(crate) fn set_jobs(&mut self, jobs: &[EvoJob]) {
        *self = Self::empty();
        self.n = jobs.len().min(6) as u8;
        for (i, job) in jobs.iter().take(6).enumerate() {
            self.jobs[i] = *job;
        }
    }

    fn empty() -> Self {
        EvolutionScene {
            jobs: [EvoJob {
                slot: 0,
                into: SpeciesId(0),
                can_cancel: false,
            }; 6],
            n: 0,
            i: 0,
            phase: Phase::Intro,
            pre: SpeciesId(0),
            pre_valid: false,
            shown: SpeciesId(0),
            cried: false,
            uploaded: None,
            map_dirty: true,
            msg: None,
        }
    }

    fn job(&self) -> EvoJob {
        self.jobs[self.i as usize]
    }

    fn load_pre<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let job = self.job();
        self.pre = cx
            .party
            .get(job.slot as usize)
            .map(|m| m.species)
            .unwrap_or(job.into);
        self.pre_valid = true;
        self.shown = self.pre;
        self.cried = false;
        self.say(cx, UiText::EvolutionWhat, false);
        self.cry(cx, self.pre);
    }

    fn nick_and_target<G: GameData>(
        &self,
        cx: &ScreenCx<'_, G>,
    ) -> ([u8; NAME_LEN], &'static [u8]) {
        let job = self.job();
        let nick = cx
            .party
            .get(job.slot as usize)
            .map(|m| m.nickname)
            .unwrap_or([0xFF; NAME_LEN]);
        let name = cx.game.battle().species_info(job.into).name;
        (nick, name)
    }

    fn say<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, text: UiText, wait: bool) {
        let (nick, name) = self.nick_and_target(cx);
        let mut vars = crate::str_vars::StrVars::new();
        vars.set(0, &nick);
        vars.set(1, name);
        let (l, t, w, h) = MSG;
        let speed = cx.options.text_speed;
        let m = self.msg.get_or_insert_with(|| {
            Box::new(Message::new(
                Window::new(l, t, w, h, window::FULL_BASE),
                speed,
            ))
        });
        m.start(
            cx.game,
            cx.game.ui_text(text),
            &cx.player_name[..],
            Some(&vars),
            wait,
        );
    }

    fn cry<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, species: SpeciesId) {
        if self.cried {
            return;
        }
        cx.notes.push(ScreenNote::Cry(species));
        self.cried = true;
    }

    fn commit<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) -> bool {
        let job = self.job();
        let Some(mon) = cx.party.get_mut(job.slot as usize) else {
            return false;
        };
        mon.evolve(cx.game.battle(), job.into);
        cx.pokedex.set_caught(job.into);
        cx.notes.push(ScreenNote::Evolved(job.into));
        true
    }

    fn enter_result<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, evolved: bool) {
        self.phase = Phase::Result {
            evolved,
            armed: false,
        };
        self.cried = false;
        let species = if evolved { self.job().into } else { self.pre };
        self.shown = species;
        self.cry(cx, species);
        let text = if evolved {
            UiText::EvolutionCongrats
        } else {
            UiText::EvolutionStopped
        };
        self.say(cx, text, true);
    }

    /// The result was confirmed. The next mon, or close with whether this one evolved.
    fn finish_job(&mut self, evolved: bool) -> ScreenOut {
        let next = self.i + 1;
        if next < self.n {
            self.i = next;
            self.pre_valid = false;
            self.phase = Phase::Intro;
            self.cried = false;
            self.msg = None;
            ScreenOut::Running
        } else {
            ScreenOut::Done(ScreenResult::Bool(evolved))
        }
    }

    fn message_done<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> bool {
        let Some(m) = self.msg.as_mut() else {
            return true;
        };
        m.update(cx.game, input.pressed, input.held)
    }

    fn blit<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let species = self.shown;
        let Some(gfx) = cx
            .game
            .battle()
            .species
            .get(species.0 as usize)
            .and_then(|s| s.gfx)
        else {
            return;
        };
        if self.uploaded != Some(species) {
            let n = gfx.front.len().min(PIC_TILES);
            if n > 0 {
                cx.f.cmds.push(Cmd::ObjTiles {
                    first: PIC_TILE,
                    tiles: &gfx.front[..n],
                });
            }
            cx.f.cmds.push(Cmd::ObjPalette {
                bank: PIC_PAL,
                colors: *gfx.palette,
            });
            self.uploaded = Some(species);
        }
        cx.f.objs.push(Obj {
            x: PIC_X,
            y: PIC_Y + i16::from(gfx.front_y),
            tile: PIC_TILE,
            palette: PIC_PAL,
            shape: ObjShape::Square,
            size: 3,
            priority: 0,
            ..Obj::default()
        });
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.blit(cx);
        if self.map_dirty {
            let mut map = MapBuf::full(window::frame_kind(cx.game, cx.options.frame));
            if let Some(m) = &self.msg
                && m.shown
            {
                map.put_window(m.window(), true);
            }
            map.flush(cx.f);
            self.map_dirty = false;
        }
        let mut budget = super::kit::frame_budget(cx.f);
        if let Some(m) = &mut self.msg {
            m.flush_within(cx.f, &mut budget);
        }
    }
}

impl Screen for EvolutionScene {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if self.n == 0 || self.i >= self.n {
            return ScreenOut::Done(ScreenResult::Bool(false));
        }
        if !self.pre_valid {
            self.load_pre(cx);
        }
        let mut out = ScreenOut::Running;
        match self.phase {
            Phase::Intro => {
                self.shown = self.pre;
                if self.message_done(cx, input) {
                    self.phase = Phase::Flash { t: 0 };
                }
            }
            Phase::Flash { t } => {
                let job = self.job();
                let swap = (t / SWAP_FRAMES) % 2 == 1;
                self.shown = if swap { job.into } else { self.pre };
                let cancel = job.can_cancel
                    && (input.pressed.contains(Buttons::B) || input.held.contains(Buttons::B));
                if cancel {
                    self.enter_result(cx, false);
                } else if t + 1 >= FLASH_FRAMES {
                    let evolved = self.commit(cx);
                    self.enter_result(cx, evolved);
                } else {
                    self.phase = Phase::Flash { t: t + 1 };
                }
            }
            Phase::Result { evolved, armed } => {
                self.shown = if evolved { self.job().into } else { self.pre };
                if !armed {
                    // Drop this frame's button: it is the one that ended the flash.
                    let _ = self.message_done(cx, ScreenInput::default());
                    self.phase = Phase::Result {
                        evolved,
                        armed: true,
                    };
                } else if self.message_done(cx, input) {
                    out = self.finish_job(evolved);
                }
            }
        }
        self.draw(cx);
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.n,
            self.i,
            u8::from(self.pre_valid),
            u8::from(self.cried),
        ]);
        h.write_u16(self.pre.0);
        h.write_u16(self.shown.0);
        let (tag, extra) = match self.phase {
            Phase::Intro => (0, 0),
            Phase::Flash { t } => (1, t),
            Phase::Result { evolved, armed } => (2, u16::from(evolved) | (u16::from(armed) << 1)),
        };
        h.write(&[tag]);
        h.write_u16(extra);
        if self.n > 0 && self.i < self.n {
            let job = self.job();
            h.write(&[job.slot, u8::from(job.can_cancel)]);
            h.write_u16(job.into.0);
        }
        if let Some(m) = &self.msg {
            m.hash(h);
        }
    }
}
