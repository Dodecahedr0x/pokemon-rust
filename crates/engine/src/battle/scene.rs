//! Battle scene: plays the event stream, runs the menus, returns the player's choice.
//! **Task K.**
//!
//! [`BattleScene`] composes Task E's [`Background`], [`MonSprite`]s, [`TrainerSprite`]s,
//! [`Healthbox`]es, [`HpBar`]s and [`ExpBar`] with Task F's [`BattleWindows`], as Gen3's
//! battle controllers do (`battle_controller_player.c`, `battle_controller_opponent.c`).
//!
//! # Driving it (Task J)
//! 1. `Battle::new`, `Battle::start(.., &mut events)`, [`BattleScene::new`] and
//!    [`BattleScene::push`] the start events.
//! 2. While the screen is black (after the transition): [`BattleScene::load`] each frame until
//!    it returns true. It uploads in chunks (environment, textbox, pics, healthboxes), so no
//!    frame is a big bulk copy.
//! 3. Then each frame [`BattleScene::step`] (or [`BattleScene::update`], answering a returned
//!    [`Choice`] with `Battle::choose` and pushing its events). The scene fades in, plays the
//!    events and opens the menus when the core needs the player.
//! 4. Once [`BattleScene::finished`] returns the outcome, the screen is black (the scene faded
//!    out after `End`). See "Restore" below.
//!
//! # Playback
//! Events play one at a time, each until its animation is over:
//! - `Msg`: expanded with [`text::expand`] from [`Names`] built from the `Battle` (both parties,
//!   `Msg::slots` name the right mons), printed, then `MsgWait`: `Frames(n)` keeps it `n`
//!   frames (`Cmd_waitmessage`, `B_WAIT_TIME_LONG` = 64), `Button` waits for A/B. A text that
//!   ends with a `\p` prompt already waits for a press, so no `Button` wait is added to it.
//! - `WildAppear`: the wild mon slides in (`SpriteCB_MoveWildMonToRight`) with the player's
//!   back pic (`BattleIntroSlide*`); its healthbox slides in when it arrives
//!   (`SpriteCB_WildMonShowHealthbox`).
//! - `TrainerAppear`: both trainers slide in. `TrainerSlideOut` does not block: the opponent
//!   trainer leaves while its ball is thrown (`OpponentHandleIntroTrainerBallThrow`).
//! - `SendOut`: the pic and healthbox of a new mon are uploaded within the frame budget
//!   (`MonSprite::load_step`, `Healthbox::set_mon`, then the bars). At the intro the player's
//!   back pic slides out and the ball is thrown 31 frames later (`Task_StartSendOutAnim`); then
//!   the ball pops, the healthbox slides in (`Intro_TryShinyAnimShowHealthbox`,
//!   `StartHealthboxSlideIn`: 5 px/frame) and the scene waits 3 more frames
//!   (`Intro_DelayAndEnd`, `introEndDelay = 3`).
//! - `Recall`, `MoveAnim` (lunge), `Hit` (blink + healthbox shake, `PlayerHandleHitAnimation`,
//!   `DoHitAnimHealthboxEffect`), `StatAnim` (tint): run to completion.
//! - `Hp`: the bar drains (`MoveBattleBar`) and the numbers follow its value.
//! - `Faint`: `BattleScript_FaintTarget`'s `pause B_WAIT_TIME_LONG` (64 frames), the faint
//!   animation, then the healthbox hides (`HideHealthboxAfterMonFaint`). The pic is reloaded at
//!   the next send-out of that side.
//! - `Exp`: the exp bar fills (the active mon only, `Task_GiveExpWithExpBar`), the box level
//!   follows a wrap; `LevelUp` redraws the box and HP bar with the new stats.
//! - `StatusIcon`: the healthbox icon (no status animation).
//! - `Ball`: no throw animation (catching has no UI in the slice); a catch hides the wild mon.
//! - `TrainerDefeated`: the trainer slides back in, then the `BattleKind::Trainer::defeat`
//!   field text prints (`{PLAYER}` expanded) and waits for a button.
//! - `End`: the screen fades to black in 16 frames (`HandleEndTurn_FinishBattle`'s
//!   `BeginFastPaletteFade`), then `finished` returns the outcome.
//!
//! The scene keeps its own copy of both parties and replays the events onto it (HP, status,
//! exp, level), so a healthbox shows each mon as it was at that point of the playback even
//! though the `Battle` already ran to its next `Need`.
//!
//! # Input
//! When everything is idle and the core needs the player:
//! - `Need::Action`: the action menu; FIGHT → the move menu → `Choice::Move`, B goes back;
//!   RUN → `Choice::Run`. BAG and POKéMON do nothing yet (no bag or party screen in the
//!   slice): the menu stays open.
//! - `Need::Switch`: the first usable mon at once (no party screen).
//! - `Need::LearnMove`: the core printed `TryToLearnMove1..3`; yes/no "learn it?"
//!   (`Cmd_yesnoboxlearnmove`). Yes opens the move menu with the new move as a fifth entry
//!   (Gen3's summary screen); a slot → `LearnMove { forget: Some(slot) }`. No, B, or the new
//!   move → `StopLearningMove` and yes/no (`Cmd_yesnoboxstoplearningmove`): yes →
//!   `LearnMove { forget: None }`, no → `TryToLearnMove1..3` again and the first question.
//!
//! # Frame budget (GBA)
//! `load` spreads its uploads over about 10 black frames (at most ~14 KiB each, the textbox
//! frame). Once the picture shows, a frame uploads at most [`gfx::OBJ_TILES_PER_FRAME`] OBJ
//! tiles: components update as Task E requires (bars and sprites, then the scene logic, whose
//! bar redraws wait for 8 free tiles, then `Healthbox::update`, then `MonSprite::load_step`),
//! plus at most one window redraw (Task F) on the BGs.
//!
//! # Restore (Task J)
//! While it runs the scene owns the screen with `video::layout::battle`: BG0..3 controls (BG2
//! disabled), BG char blocks 0..2 and screen blocks 24/26/28, BG palettes 0..5, OBJ tiles
//! 0..468 and OBJ palettes 0..7; it sets `brightness`, `lighten` (false), every scroll (0) and
//! the whole OAM list on each frame. After `finished`, J re-uploads the field (tilesets, maps,
//! palettes, BG controls), rebuilds the field `TextBox` and object sprites, and fades in.
//!
//! # Simplifications
//! No intro window reveal (the scene fades in from black during the intro slides), no
//! environment slide, no status or level-up animations, no "Come back X!" text, no ball throw
//! for catches. Everything else follows the components' own documented approximations.

#[cfg(test)]
mod test_game;
#[cfg(test)]
mod tests;

use super::gfx::{
    self, Background, ExpBar, Healthbox, HpBar, MonSprite, TINT_STAT_DOWN, TINT_STAT_UP,
    TrainerSprite,
};
use super::ui::{Action, BattleWindows, MoveChoice};
use super::{
    Battle, BattleEvent, BattleGfx, BattleKind, BattleMon, BattleOutcome, BattleSetup, Buff,
    Choice, EMPTY_GFX, Environment, Msg, MsgWait, Names, Need, Side, Str, text,
};
use crate::data::{GameData, MoveId, SpeciesId};
use crate::flow::{FADE_MAX, Fader, Input};
use crate::hash::Fnv64;
use crate::mon::{BattleTables, Party, Pokemon, exp_for_level};
use crate::rng::Rng;
use crate::script::Fade;
use crate::video::{BgControl, Cmd, Frame, layout::battle::ANIM};
use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// `Task_StartSendOutAnim`: the player's ball flies 31 frames after its trainer starts sliding.
const PLAYER_BALL_DELAY: u8 = 31;
/// `Intro_WaitForShinyAnimAndHealthbox`: `introEndDelay = 3`.
const INTRO_END_DELAY: u8 = 3;
/// `BattleScript_FaintTarget` / `_FaintAttacker`: `pause B_WAIT_TIME_LONG` before the faint.
const FAINT_PAUSE: u8 = super::calc::WAIT_LONG;
/// OBJ tiles a bar redraw may take (`HpBar::set`: 6, `ExpBar::set`: 8).
const BAR_SET_TILES: usize = 8;
/// `\p`.
const PROMPT: u8 = 0xFB;
const EOS: u8 = 0xFF;

/// Where a send-out is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Send {
    /// Frames left before the ball; also waits for the pic and the bars.
    Wait(u8),
    Ball,
    Box,
    /// `Intro_DelayAndEnd`.
    Delay(u8),
}

/// The event being played.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Play {
    Idle,
    /// A message prints or waits.
    Msg,
    /// Until every sprite, box and bar is idle.
    Anims,
    /// The wild mon slides in (`false`), then its healthbox (`true`).
    WildAppear(bool),
    SendOut(Side, Send),
    /// Pause frames left (`Some`), then the animation (`None`).
    Faint(Side, Option<u8>),
    Hp(Side),
    Exp,
    /// Until the pending bar redraws are done (a level-up).
    Bars,
    /// The trainer slides in (`false`), then its defeat text prints (`true`).
    Defeated(bool),
    /// Fading out after `End`.
    Ending,
}

/// The open menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Menu {
    None,
    Action,
    Move,
    /// "Learn `mov`?" yes/no (`Cmd_yesnoboxlearnmove`).
    LearnAsk {
        party: u8,
        mov: MoveId,
    },
    /// The move menu with `mov` as a fifth entry.
    LearnPick {
        party: u8,
        mov: MoveId,
    },
    /// `StopLearningMove` prints.
    StopMsg {
        party: u8,
        mov: MoveId,
    },
    /// "Stop learning?" yes/no (`Cmd_yesnoboxstoplearningmove`).
    StopAsk {
        party: u8,
        mov: MoveId,
    },
    /// `TryToLearnMove1..3` again (`n` = the one printing).
    Retry {
        party: u8,
        mov: MoveId,
        n: u8,
    },
}

/// The visual components of one battle.
struct Parts {
    bg: Background,
    windows: BattleWindows,
    /// Index = `Side`.
    mons: [MonSprite; 2],
    /// `[player back pic, opponent trainer]`.
    trainers: [TrainerSprite; 2],
    boxes: [Healthbox; 2],
    bars: [HpBar; 2],
    exp: ExpBar,
}

impl Parts {
    fn new(gfx: &'static BattleGfx, env: Environment) -> Self {
        Parts {
            bg: Background::new(env),
            windows: BattleWindows::new(),
            mons: [
                MonSprite::new(Side::Player, gfx),
                MonSprite::new(Side::Opponent, gfx),
            ],
            trainers: [
                TrainerSprite::new(Side::Player),
                TrainerSprite::new(Side::Opponent),
            ],
            boxes: [
                Healthbox::new(Side::Player, gfx),
                Healthbox::new(Side::Opponent, gfx),
            ],
            bars: [
                HpBar::new(Side::Player, gfx),
                HpBar::new(Side::Opponent, gfx),
            ],
            exp: ExpBar::new(gfx),
        }
    }
}

/// Plays a battle's events and turns input into [`Choice`]s. See the module docs.
pub struct BattleScene {
    kind: BattleKind,
    parts: Parts,
    /// `Some(step)` while loading.
    loading: Option<u8>,
    /// The scene's copy of both parties (index = `Side`), as of the playback.
    parties: [Party; 2],
    /// The party slot out on each side.
    out: [u8; 2],
    /// The pic in OBJ VRAM, intact (`None` after a faint).
    pics: [Option<(SpeciesId, bool)>; 2],
    /// A mid-battle pic upload in progress.
    pic_load: [Option<(SpeciesId, bool)>; 2],
    /// The party slot the healthbox and bars show.
    box_slot: [Option<u8>; 2],
    /// HP bar redraws waiting for budget (index = `Side`), then the exp bar's.
    hp_pending: [bool; 2],
    exp_pending: bool,
    queue: VecDeque<BattleEvent>,
    play: Play,
    menu: Menu,
    fader: Fader,
    outcome: Option<BattleOutcome>,
    finished: bool,
}

/// What presentation shows of party mon `p` (slot `party`), as `CopyMonToBattleMon`.
fn view(t: &BattleTables, p: &Pokemon, party: u8) -> BattleMon {
    BattleMon {
        party,
        species: p.species,
        nickname: p.nickname,
        level: p.level,
        hp: p.hp,
        max_hp: p.stats[0],
        status: p.status,
        gender: p.gender(t),
        moves: p.moves,
        pp: p.pp,
        max_pp: core::array::from_fn(|s| p.max_pp(t, s as u8)),
    }
}

/// Whether Gen3 text ends with a `\p` prompt (it waits for a press by itself).
fn ends_with_prompt(text: &[u8]) -> bool {
    let end = text.iter().position(|&b| b == EOS).unwrap_or(text.len());
    end > 0 && text[end - 1] == PROMPT
}

impl BattleScene {
    /// A scene for `setup`. Nothing is drawn until [`BattleScene::load`].
    pub fn new(setup: &BattleSetup) -> Self {
        let mut fader = Fader::new();
        fader.start(Fade::FromBlack);
        BattleScene {
            kind: setup.kind,
            parts: Parts::new(&EMPTY_GFX, setup.environment),
            loading: Some(0),
            parties: [Party::default(), setup.opponents.clone()],
            out: [0; 2],
            pics: [None; 2],
            pic_load: [None; 2],
            box_slot: [None; 2],
            hp_pending: [false; 2],
            exp_pending: false,
            queue: VecDeque::new(),
            play: Play::Idle,
            menu: Menu::None,
            fader,
            outcome: None,
            finished: false,
        }
    }

    /// Uploads one chunk while the screen is black (the frame is set black); true once
    /// everything is loaded (then a no-op). Call it before `push`ed events can play, right
    /// after `Battle::start`: the battle's state then is the one the intro shows.
    pub fn load(&mut self, game: &impl GameData, battle: &Battle, f: &mut Frame) -> bool {
        let Some(step) = self.loading else {
            return true;
        };
        f.brightness = FADE_MAX;
        f.lighten = false;
        f.scroll = Default::default();
        f.objs.clear();
        let gfx = game.battle_gfx();
        let t = game.battle();
        let p = &mut self.parts;
        let next = match step {
            0 => {
                *p = Parts::new(gfx, battle.setup().environment);
                self.parties = [
                    battle.party(Side::Player).clone(),
                    battle.party(Side::Opponent).clone(),
                ];
                self.out = [Side::Player, Side::Opponent].map(|s| battle.mon(s).party);
                f.cmds.push(Cmd::Bg {
                    layer: ANIM.0,
                    control: BgControl::default(),
                });
                p.bg.load_step(game, f);
                1
            }
            1 => {
                if !p.bg.load_step(game, f) {
                    return false;
                }
                2
            }
            2 => {
                p.windows.load(game, f);
                3
            }
            3 => {
                for side in [Side::Opponent, Side::Player] {
                    let mon = self.mon(side);
                    let pic = (mon.species, mon.is_shiny());
                    self.parts.mons[side as usize].load(game, pic.0, pic.1, f);
                    self.pics[side as usize] = Some(pic);
                }
                4
            }
            4 => {
                if let BattleKind::Trainer { trainer, .. } = self.kind
                    && let Some(data) = game.trainer(trainer)
                {
                    p.trainers[1].load(data.pic, f);
                }
                p.trainers[0].load(&gfx.player_back, f);
                5
            }
            5 | 6 => {
                let side = if step == 5 {
                    Side::Player
                } else {
                    Side::Opponent
                };
                let s = side as usize;
                let mon = view(t, self.mon(side), self.out[s]);
                self.parts.boxes[s].load(&mon, f);
                self.parts.bars[s].set(mon.hp, mon.max_hp, f);
                if side == Side::Player {
                    self.parts.exp.invalidate();
                    self.set_exp(t, f);
                }
                self.box_slot[s] = Some(self.out[s]);
                step + 1
            }
            _ => {
                self.loading = None;
                return true;
            }
        };
        self.loading = Some(next);
        false
    }

    /// Queues events (drains `events`).
    pub fn push(&mut self, events: &mut Vec<BattleEvent>) {
        self.queue.extend(events.drain(..));
    }

    /// One frame of playback and menus. `Some` is the player's answer to `battle.need()`:
    /// give it to `Battle::choose` and [`BattleScene::push`] the events before the next
    /// frame. Loads first if [`BattleScene::load`] is not done.
    pub fn update(
        &mut self,
        game: &impl GameData,
        battle: &Battle,
        input: &Input,
        f: &mut Frame,
    ) -> Option<Choice> {
        if self.loading.is_some() {
            self.load(game, battle, f);
            return None;
        }
        f.lighten = false;
        f.scroll = Default::default();
        f.objs.clear();
        if self.finished {
            f.brightness = FADE_MAX;
            return None;
        }
        // Input and the BG windows, then the components with fixed OBJ tile costs (Task E's
        // order), the scene logic (budgeted bar redraws), and the budgeted uploads last.
        let p = &mut self.parts;
        p.windows.update(game, input, f);
        for b in &mut p.bars {
            b.update(f);
        }
        p.exp.update(f);
        for m in &mut p.mons {
            m.update(f);
        }
        for t in &mut p.trainers {
            t.update(f);
        }
        let choice = self.advance(game, battle, f);
        let p = &mut self.parts;
        for b in &mut p.boxes {
            b.update(f);
        }
        for side in [Side::Player, Side::Opponent] {
            let s = side as usize;
            if let Some((species, shiny)) = self.pic_load[s]
                && p.mons[s].load_step(game, species, shiny, f)
            {
                self.pic_load[s] = None;
                self.pics[s] = Some((species, shiny));
            }
        }
        f.brightness = self.fader.update();
        if self.play == Play::Ending && self.fader.is_done() {
            self.finished = true;
        }
        choice
    }

    /// [`BattleScene::update`] plus the core: a choice goes to `battle.choose` at once and its
    /// events are queued. The usual way to drive a battle (Task J).
    pub fn step(
        &mut self,
        game: &impl GameData,
        battle: &mut Battle,
        rng: &mut Rng,
        input: &Input,
        f: &mut Frame,
    ) {
        if let Some(c) = self.update(game, battle, input, f) {
            let mut events = Vec::new();
            battle.choose(game, rng, c, &mut events);
            self.push(&mut events);
        }
    }

    /// The outcome once `End` played, its last message was dismissed and the screen faded to
    /// black; `None` before.
    pub fn finished(&self) -> Option<BattleOutcome> {
        if self.finished { self.outcome } else { None }
    }

    /// Feeds every piece of state into `h` (cheap enough for every frame).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        let p = &self.parts;
        p.bg.hash(h);
        p.windows.hash(h);
        for m in &p.mons {
            m.hash(h);
        }
        for t in &p.trainers {
            t.hash(h);
        }
        for b in &p.boxes {
            b.hash(h);
        }
        for b in &p.bars {
            b.hash(h);
        }
        p.exp.hash(h);
        h.write(&[
            self.loading.unwrap_or(0xFF),
            self.out[0],
            self.out[1],
            self.box_slot[0].unwrap_or(0xFF),
            self.box_slot[1].unwrap_or(0xFF),
            self.hp_pending[0] as u8,
            self.hp_pending[1] as u8,
            self.exp_pending as u8,
            self.outcome.map_or(0, |o| o as u8),
            self.finished as u8,
        ]);
        for pic in self.pics.iter().chain(&self.pic_load) {
            match pic {
                None => h.write(&[0xFF]),
                Some((s, shiny)) => {
                    h.write_u16(s.0);
                    h.write(&[*shiny as u8]);
                }
            }
        }
        for party in &self.parties {
            party.hash(h);
        }
        hash_play(self.play, h);
        hash_menu(self.menu, h);
        self.fader.hash(h);
        h.write_u16(self.queue.len() as u16);
        for e in &self.queue {
            hash_event(e, h);
        }
    }

    // ---- playback ----

    /// The scene's copy of the mon out on `side`.
    fn mon(&self, side: Side) -> &Pokemon {
        let s = side as usize;
        self.parties[s]
            .get(self.out[s] as usize)
            .expect("the battler's party slot")
    }

    /// Everything idle: no sprite, box or bar animation, no pending upload.
    fn anims_busy(&self) -> bool {
        let p = &self.parts;
        p.mons.iter().any(MonSprite::busy)
            || p.trainers.iter().any(TrainerSprite::busy)
            || p.boxes.iter().any(Healthbox::busy)
            || p.bars.iter().any(|b| !b.done())
            || !p.exp.done()
            || self.pic_load.iter().any(Option::is_some)
            || self.hp_pending.iter().any(|&b| b)
            || self.exp_pending
    }

    /// Runs the current event, starts the next ones, and opens the menus when idle.
    fn advance(&mut self, game: &impl GameData, battle: &Battle, f: &mut Frame) -> Option<Choice> {
        self.sync_bars(game.battle(), f);
        while self.tick(game, f) {
            self.play = Play::Idle;
            let Some(e) = self.queue.pop_front() else {
                break;
            };
            self.start(game, battle, e, f);
        }
        let idle = self.play == Play::Idle
            && self.queue.is_empty()
            && !self.parts.windows.is_printing()
            && !self.anims_busy()
            && self.fader.is_done();
        if idle {
            self.input(game, battle, f)
        } else {
            None
        }
    }

    /// The bar redraws waiting for OBJ tile budget (`HpBar::set` after `Healthbox::set_mon`,
    /// then `ExpBar::set`).
    fn sync_bars(&mut self, t: &BattleTables, f: &mut Frame) {
        for side in [Side::Player, Side::Opponent] {
            let s = side as usize;
            if self.hp_pending[s] && gfx::obj_tiles_left(f) >= BAR_SET_TILES {
                let mon = self.mon(side);
                let (hp, max) = (mon.hp, mon.stats[0]);
                self.parts.bars[s].set(hp, max, f);
                self.hp_pending[s] = false;
            }
        }
        if self.exp_pending && gfx::obj_tiles_left(f) >= BAR_SET_TILES {
            self.set_exp(t, f);
            self.exp_pending = false;
        }
    }

    /// `ExpBar::set` for the player's mon out.
    fn set_exp(&mut self, t: &BattleTables, f: &mut Frame) {
        let mon = self.mon(Side::Player);
        let growth = t.species_info(mon.species).growth;
        let (level, exp) = (mon.level, mon.exp);
        let next = exp_for_level(growth, level.saturating_add(1).min(crate::mon::MAX_LEVEL));
        self.parts
            .exp
            .set(level, exp_for_level(growth, level), next, exp, f);
    }

    /// One frame of the current event; true when it is over.
    fn tick(&mut self, game: &impl GameData, f: &mut Frame) -> bool {
        let p = &mut self.parts;
        match self.play {
            Play::Idle => true,
            Play::Msg => !p.windows.is_printing(),
            Play::Anims | Play::Bars => !self.anims_busy(),
            Play::WildAppear(false) => {
                if !p.mons[1].busy() {
                    p.boxes[1].slide_in();
                    self.play = Play::WildAppear(true);
                }
                false
            }
            Play::WildAppear(true) => !self.anims_busy(),
            Play::SendOut(side, stage) => {
                let s = side as usize;
                let next = match stage {
                    Send::Wait(n) if n > 0 => Send::Wait(n - 1),
                    Send::Wait(_) => {
                        if self.pic_load[s].is_some() || self.hp_pending[s] || self.exp_pending {
                            return false;
                        }
                        p.mons[s].send_out();
                        Send::Ball
                    }
                    Send::Ball if p.mons[s].busy() => Send::Ball,
                    Send::Ball => {
                        p.boxes[s].slide_in();
                        Send::Box
                    }
                    Send::Box if p.boxes[s].busy() => Send::Box,
                    Send::Box => Send::Delay(INTRO_END_DELAY),
                    Send::Delay(0) => return true,
                    Send::Delay(n) => Send::Delay(n - 1),
                };
                self.play = Play::SendOut(side, next);
                false
            }
            Play::Faint(side, Some(0)) => {
                p.mons[side as usize].faint();
                self.play = Play::Faint(side, None);
                false
            }
            Play::Faint(side, Some(n)) => {
                self.play = Play::Faint(side, Some(n - 1));
                false
            }
            Play::Faint(side, None) => {
                let s = side as usize;
                if p.mons[s].busy() {
                    return false;
                }
                p.boxes[s].hide();
                // The faint erased the pic: reload it before it shows again.
                self.pics[s] = None;
                true
            }
            Play::Hp(side) => {
                let s = side as usize;
                p.boxes[s].set_hp(p.bars[s].value());
                p.bars[s].done()
            }
            Play::Exp => {
                if !p.exp.done() {
                    return false;
                }
                if p.exp.leveled_up() {
                    p.boxes[0].set_level(p.exp.level());
                }
                true
            }
            Play::Defeated(false) => {
                if p.trainers[1].busy() {
                    return false;
                }
                if let BattleKind::Trainer { defeat, .. } = self.kind {
                    let text = crate::text::expand(game, game.text(defeat));
                    let wait = if ends_with_prompt(&text) {
                        MsgWait::Frames(0)
                    } else {
                        MsgWait::Button
                    };
                    p.windows.print(game, text, wait, f);
                }
                self.play = Play::Defeated(true);
                false
            }
            Play::Defeated(true) => !p.windows.is_printing(),
            Play::Ending => false,
        }
    }

    /// Starts playing `e`.
    fn start(&mut self, game: &impl GameData, battle: &Battle, e: BattleEvent, f: &mut Frame) {
        let t = game.battle();
        let p = &mut self.parts;
        self.play = match e {
            BattleEvent::Msg(m) => {
                self.print(game, battle, &m, f);
                Play::Msg
            }
            BattleEvent::WildAppear => {
                p.mons[1].slide_in();
                p.trainers[0].slide_in();
                Play::WildAppear(false)
            }
            BattleEvent::TrainerAppear => {
                p.trainers[0].slide_in();
                p.trainers[1].slide_in();
                Play::Anims
            }
            BattleEvent::TrainerSlideOut => {
                p.trainers[1].slide_out();
                Play::Idle
            }
            BattleEvent::TrainerDefeated => {
                p.trainers[1].slide_in_defeated();
                Play::Defeated(false)
            }
            BattleEvent::SendOut { side, party } => self.send_out(t, side, party),
            BattleEvent::Recall { side } => {
                p.mons[side as usize].recall();
                p.boxes[side as usize].hide();
                Play::Anims
            }
            BattleEvent::MoveAnim { attacker, .. } => {
                p.mons[attacker as usize].lunge();
                Play::Anims
            }
            BattleEvent::Hit { target, .. } => {
                p.mons[target as usize].hit();
                p.boxes[target as usize].shake();
                Play::Anims
            }
            BattleEvent::Hp { side, from, to } => {
                let s = side as usize;
                let slot = self.out[s] as usize;
                let max = match self.parties[s].get_mut(slot) {
                    Some(m) => {
                        m.hp = to;
                        m.stats[0]
                    }
                    None => from.max(to),
                };
                p.bars[s].start(max, from, to);
                Play::Hp(side)
            }
            BattleEvent::StatusIcon { side, status } => {
                let s = side as usize;
                if let Some(m) = self.parties[s].get_mut(self.out[s] as usize) {
                    m.status = status;
                }
                p.boxes[s].set_status(status);
                Play::Idle
            }
            BattleEvent::StatAnim { side, up } => {
                p.mons[side as usize].tint(if up { TINT_STAT_UP } else { TINT_STAT_DOWN });
                Play::Anims
            }
            BattleEvent::Faint { side } => Play::Faint(side, Some(FAINT_PAUSE)),
            BattleEvent::Exp { party, from, to } => {
                let Some(m) = self.parties[0].get_mut(party as usize) else {
                    return;
                };
                let (level, growth) = (m.level, t.species_info(m.species).growth);
                // The core gave the EVs of the knock-out before its exp (`MonGainEVs`); at
                // most one knock-out per `choose`, so the battle's EVs are the ones to use.
                if let Some(b) = battle.party(Side::Player).get(party as usize) {
                    m.evs = b.evs;
                }
                m.exp = to;
                m.recalc_stats(t);
                if party == self.out[0] {
                    p.exp.start(level, from, to, |l| exp_for_level(growth, l));
                    Play::Exp
                } else {
                    Play::Idle
                }
            }
            BattleEvent::LevelUp { party, .. } => {
                if party == self.out[0] && self.box_slot[0] == Some(party) {
                    let mon = view(t, self.mon(Side::Player), party);
                    self.parts.boxes[0].set_mon(&mon);
                    self.hp_pending[0] = true;
                    Play::Bars
                } else {
                    Play::Idle
                }
            }
            BattleEvent::Ball { caught, .. } => {
                if caught {
                    p.mons[1].hide();
                    p.boxes[1].hide();
                }
                Play::Idle
            }
            BattleEvent::End(o) => {
                self.outcome = Some(o);
                self.fader.start(Fade::ToBlack);
                Play::Ending
            }
        };
    }

    /// `SendOut`: the pic, healthbox and bars of the new mon, then the ball.
    fn send_out(&mut self, t: &BattleTables, side: Side, party: u8) -> Play {
        let s = side as usize;
        self.out[s] = party;
        let mon = *self.mon(side);
        let pic = (mon.species, mon.is_shiny());
        if self.pics[s] != Some(pic) {
            self.pics[s] = None;
            self.pic_load[s] = Some(pic);
        }
        if self.box_slot[s] != Some(party) {
            self.parts.boxes[s].set_mon(&view(t, &mon, party));
            self.box_slot[s] = Some(party);
            self.hp_pending[s] = true;
            if side == Side::Player {
                self.exp_pending = true;
            }
        }
        let back = &mut self.parts.trainers[0];
        let wait = if side == Side::Player && back.visible() {
            // PlayerHandleIntroTrainerBallThrow + Task_StartSendOutAnim.
            back.slide_out();
            PLAYER_BALL_DELAY
        } else {
            0
        };
        Play::SendOut(side, Send::Wait(wait))
    }

    /// Prints `m` (expanded with the battle's names).
    fn print(&mut self, game: &impl GameData, battle: &Battle, m: &Msg, f: &mut Frame) {
        let text = text::expand(game, m, &names(game, battle));
        let wait = match m.wait {
            MsgWait::Button if ends_with_prompt(&text) => MsgWait::Frames(0),
            w => w,
        };
        self.parts.windows.print(game, text, wait, f);
    }

    /// A learn-move message about party slot `party` (`BattleScript_AskToLearnMove`'s buffers).
    fn learn_msg(&self, id: Str, party: u8, mov: MoveId) -> Msg {
        let side = Side::Player;
        Msg {
            id,
            attacker: side,
            target: side,
            active: side,
            mov: None,
            buffs: [Buff::Mon { side, party }, Buff::Move(mov), Buff::None],
            wait: MsgWait::Frames(0),
            slots: self.out,
        }
    }

    // ---- menus ----

    /// Idle: opens the menu the core's `Need` asks for, or reads the open one.
    fn input(&mut self, game: &impl GameData, battle: &Battle, f: &mut Frame) -> Option<Choice> {
        let w = &mut self.parts.windows;
        match self.menu {
            Menu::None => match battle.need() {
                Need::Action => {
                    w.action_menu(game, &names(game, battle), f);
                    self.menu = Menu::Action;
                }
                // No party screen: the first mon that can fight.
                Need::Switch => {
                    return battle
                        .party(Side::Player)
                        .first_usable()
                        .map(Choice::Switch);
                }
                Need::LearnMove { party, mov } => {
                    w.yes_no(game, f);
                    self.menu = Menu::LearnAsk { party, mov };
                }
                Need::Busy | Need::Ended(_) => {}
            },
            Menu::Action => match w.action_result() {
                Some(Action::Fight) => {
                    w.move_menu(game, battle.mon(Side::Player), f);
                    self.menu = Menu::Move;
                }
                Some(Action::Run) => {
                    self.menu = Menu::None;
                    return Some(Choice::Run);
                }
                // No bag nor party screen in the slice: back to the menu.
                Some(Action::Bag | Action::Pokemon) => {
                    w.action_menu(game, &names(game, battle), f);
                }
                None => {}
            },
            Menu::Move => match w.move_result() {
                Some(MoveChoice::Move(slot)) => {
                    self.menu = Menu::None;
                    return Some(Choice::Move(slot));
                }
                Some(MoveChoice::Back) => {
                    w.action_menu(game, &names(game, battle), f);
                    self.menu = Menu::Action;
                }
                None => {}
            },
            Menu::LearnAsk { party, mov } => match w.yes_no_result() {
                Some(true) => {
                    let Some(p) = battle.party(Side::Player).get(party as usize) else {
                        return Some(Choice::LearnMove { forget: None });
                    };
                    let mon = view(game.battle(), p, party);
                    w.learn_move_menu(game, &mon, mov, f);
                    self.menu = Menu::LearnPick { party, mov };
                }
                Some(false) => self.stop_learning(game, battle, party, mov, f),
                None => {}
            },
            Menu::LearnPick { party, mov } => match w.move_result() {
                Some(MoveChoice::Move(slot)) => {
                    self.menu = Menu::None;
                    return Some(Choice::LearnMove { forget: Some(slot) });
                }
                Some(MoveChoice::Back) => self.stop_learning(game, battle, party, mov, f),
                None => {}
            },
            Menu::StopMsg { party, mov } => {
                w.yes_no(game, f);
                self.menu = Menu::StopAsk { party, mov };
            }
            Menu::StopAsk { party, mov } => match w.yes_no_result() {
                Some(true) => {
                    self.menu = Menu::None;
                    return Some(Choice::LearnMove { forget: None });
                }
                // BattleScript_AskToLearnMove again.
                Some(false) => {
                    let m = self.learn_msg(Str::TryToLearnMove1, party, mov);
                    self.print(game, battle, &m, f);
                    self.menu = Menu::Retry { party, mov, n: 0 };
                }
                None => {}
            },
            Menu::Retry { party, mov, n } => {
                if n < 2 {
                    let id = [Str::TryToLearnMove2, Str::TryToLearnMove3][n as usize];
                    let m = self.learn_msg(id, party, mov);
                    self.print(game, battle, &m, f);
                    self.menu = Menu::Retry {
                        party,
                        mov,
                        n: n + 1,
                    };
                } else {
                    self.parts.windows.yes_no(game, f);
                    self.menu = Menu::LearnAsk { party, mov };
                }
            }
        }
        None
    }

    /// "Stop learning `mov`?" (`STRINGID_STOPLEARNINGMOVE`), then its yes/no.
    fn stop_learning(
        &mut self,
        game: &impl GameData,
        battle: &Battle,
        party: u8,
        mov: MoveId,
        f: &mut Frame,
    ) {
        let m = self.learn_msg(Str::StopLearningMove, party, mov);
        self.print(game, battle, &m, f);
        self.menu = Menu::StopMsg { party, mov };
    }
}

/// The names `text::expand` reads, from the battle and the game.
fn names<'a>(game: &impl GameData, battle: &'a Battle) -> Names<'a> {
    let (trainer_class, trainer_name): (&[u8], &[u8]) = match battle.setup().kind {
        BattleKind::Trainer { trainer, .. } => game
            .trainer(trainer)
            .map_or((&[EOS], &[EOS]), |t| (t.class_name, t.name)),
        BattleKind::Wild { .. } => (&[EOS], &[EOS]),
    };
    Names {
        player: game.new_game().player_name,
        trainer_class,
        trainer_name,
        wild: matches!(battle.setup().kind, BattleKind::Wild { .. }),
        mons: [battle.mon(Side::Player), battle.mon(Side::Opponent)],
        parties: [battle.party(Side::Player), battle.party(Side::Opponent)],
    }
}

fn hash_play(p: Play, h: &mut Fnv64) {
    let opt = |o: Option<u8>| o.unwrap_or(0xFF);
    let bytes: [u8; 4] = match p {
        Play::Idle => [0, 0, 0, 0],
        Play::Msg => [1, 0, 0, 0],
        Play::Anims => [2, 0, 0, 0],
        Play::WildAppear(b) => [3, b as u8, 0, 0],
        Play::SendOut(side, s) => {
            let (k, n) = match s {
                Send::Wait(n) => (0, n),
                Send::Ball => (1, 0),
                Send::Box => (2, 0),
                Send::Delay(n) => (3, n),
            };
            [4, side as u8, k, n]
        }
        Play::Faint(side, n) => [5, side as u8, opt(n), 0],
        Play::Hp(side) => [6, side as u8, 0, 0],
        Play::Exp => [7, 0, 0, 0],
        Play::Bars => [8, 0, 0, 0],
        Play::Defeated(b) => [9, b as u8, 0, 0],
        Play::Ending => [10, 0, 0, 0],
    };
    h.write(&bytes);
}

fn hash_menu(m: Menu, h: &mut Fnv64) {
    let (tag, party, mov, n) = match m {
        Menu::None => (0, 0, MoveId(0), 0),
        Menu::Action => (1, 0, MoveId(0), 0),
        Menu::Move => (2, 0, MoveId(0), 0),
        Menu::LearnAsk { party, mov } => (3, party, mov, 0),
        Menu::LearnPick { party, mov } => (4, party, mov, 0),
        Menu::StopMsg { party, mov } => (5, party, mov, 0),
        Menu::StopAsk { party, mov } => (6, party, mov, 0),
        Menu::Retry { party, mov, n } => (7, party, mov, n),
    };
    h.write(&[tag, party, n]);
    h.write_u16(mov.0);
}

/// A stable code for `s` (its declaration order; indexed strings carry their index).
fn str_code(s: Str, h: &mut Fnv64) {
    use Str::*;
    let code: u8 = match s {
        Intro1 => 0,
        IntroTrainer => 1,
        IntroMsg => 2,
        Intro2 => 3,
        GoPkmn => 4,
        PkmnSentOut => 5,
        WhatWillPkmnDo => 6,
        BattleMenu => 7,
        MoveInterfacePP => 8,
        MoveInterfaceType => 9,
        UsedMove => 10,
        AttackMissed => 11,
        CriticalHit => 12,
        SuperEffective => 13,
        NotVeryEffective => 14,
        ItDoesntAffect => 15,
        PkmnFainted => 16,
        GotAwaySafely => 17,
        CantEscape => 18,
        DontLeaveBirch => 19,
        WildPkmnFled => 20,
        PlayerDefeatedTrainer => 21,
        PlayerWhiteout => 22,
        PlayerWhiteout2 => 23,
        PkmnGainedExp => 24,
        PkmnGrewToLv => 25,
        PkmnLearnedMove => 26,
        TryToLearnMove1 => 27,
        TryToLearnMove2 => 28,
        TryToLearnMove3 => 29,
        PkmnForgotMove => 30,
        StopLearningMove => 31,
        DidNotLearnMove => 32,
        AttackerStatChanged => 33,
        DefenderStatChanged => 34,
        StatRose => 35,
        StatSharplyRose => 36,
        StatFell => 37,
        StatHarshlyFell => 38,
        StatWontIncrease => 39,
        StatWontDecrease => 40,
        ButItFailed => 41,
        PkmnWasPoisoned => 42,
        PkmnWasBurned => 43,
        PkmnWasParalyzed => 44,
        PkmnFellAsleep => 45,
        PkmnBecameConfused => 46,
        PkmnFlinched => 47,
        PkmnIsConfused => 48,
        PkmnHurtItself => 49,
        PkmnIsParalyzed => 50,
        PkmnFastAsleep => 51,
        PkmnWokeUp => 52,
        PkmnHurtByPoison => 53,
        PkmnHurtByBurn => 54,
        PkmnSeeded => 55,
        LeechSeedDrain => 56,
        PkmnStoringEnergy => 57,
        PkmnUnleashedEnergy => 58,
        GettingPumped => 59,
        AbsorbDrain => 60,
        PkmnHitWithRecoil => 61,
        NoPPLeft => 62,
        Struggle => 63,
        NoRunningFromTrainers => 64,
        Gotcha => 65,
        WildPrefix => 66,
        FoePrefix => 67,
        BattleYesNoChoice => 68,
        ConfirmStarterChoice => 69,
        TypeName(t) => {
            h.write(&[70]);
            t as u8
        }
        StatName(s) => {
            h.write(&[71]);
            s as u8
        }
        // Strings added after this list (Tasks E2/I) hash alike: the core state still tells
        // runs apart.
        #[allow(unreachable_patterns)]
        _ => 0xFE,
    };
    h.write(&[code]);
}

fn hash_buff(b: &Buff, h: &mut Fnv64) {
    match *b {
        Buff::None => h.write(&[0]),
        Buff::Stat(s) => h.write(&[1, s as u8]),
        Buff::Move(m) => {
            h.write(&[2]);
            h.write_u16(m.0);
        }
        Buff::Number(n) => {
            h.write(&[3]);
            h.write_u32(n);
        }
        Buff::Species(s) => {
            h.write(&[4]);
            h.write_u16(s.0);
        }
        Buff::Type(t) => h.write(&[5, t as u8]),
        Buff::Mon { side, party } => h.write(&[6, side as u8, party]),
        Buff::MonNick { side, party } => h.write(&[7, side as u8, party]),
        Buff::Text(s) => {
            h.write(&[8]);
            str_code(s, h);
        }
    }
}

fn hash_event(e: &BattleEvent, h: &mut Fnv64) {
    match *e {
        BattleEvent::Msg(m) => {
            h.write(&[0]);
            str_code(m.id, h);
            h.write(&[
                m.attacker as u8,
                m.target as u8,
                m.active as u8,
                m.slots[0],
                m.slots[1],
            ]);
            h.write_u16(m.mov.map_or(0xFFFF, |m| m.0));
            for b in &m.buffs {
                hash_buff(b, h);
            }
            h.write(&match m.wait {
                MsgWait::Frames(n) => [0, n],
                MsgWait::Button => [1, 0],
            });
        }
        BattleEvent::WildAppear => h.write(&[1]),
        BattleEvent::TrainerAppear => h.write(&[2]),
        BattleEvent::TrainerSlideOut => h.write(&[3]),
        BattleEvent::TrainerDefeated => h.write(&[4]),
        BattleEvent::SendOut { side, party } => h.write(&[5, side as u8, party]),
        BattleEvent::Recall { side } => h.write(&[6, side as u8]),
        BattleEvent::MoveAnim { attacker, mov } => {
            h.write(&[7, attacker as u8]);
            h.write_u16(mov.0);
        }
        BattleEvent::Hit { target, eff } => h.write(&[8, target as u8, eff as u8]),
        BattleEvent::Hp { side, from, to } => {
            h.write(&[9, side as u8]);
            h.write_u16(from);
            h.write_u16(to);
        }
        BattleEvent::StatusIcon { side, status } => {
            let (a, b) = status.code();
            h.write(&[10, side as u8, a, b]);
        }
        BattleEvent::StatAnim { side, up } => h.write(&[11, side as u8, up as u8]),
        BattleEvent::Faint { side } => h.write(&[12, side as u8]),
        BattleEvent::Exp { party, from, to } => {
            h.write(&[13, party]);
            h.write_u32(from);
            h.write_u32(to);
        }
        BattleEvent::LevelUp { party, level } => h.write(&[14, party, level]),
        BattleEvent::Ball { shakes, caught } => h.write(&[15, shakes, caught as u8]),
        BattleEvent::End(o) => h.write(&[16, o as u8]),
    }
}
