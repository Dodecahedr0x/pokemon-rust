//! [`ScriptHost`] for the world: every [`Request`], as the Gen3 script command it stands for
//! (`scrcmd.c`).
//!
//! Milestone 8 requests are routed to one file per owner: items and money ([`items`], Task H),
//! multichoice ([`menu`], H), string buffers ([`buffers`], H), the trainer approach
//! ([`trainer`], I) and warp slots ([`warps`], F). Their Wave 0 bodies answer safe defaults.

mod buffers;
mod items;
mod menu;
mod trainer;
mod warps;

use super::battle::{Mode, Origin, StarterRun};
use super::{DOOR_FRAMES, Event, State};
use crate::Buttons;
use crate::battle::{BattleKind, BattleOutcome};
use crate::data::{BLOCK_COLLISION_SHIFT, BLOCK_METATILE_MASK, GameData, PLAYER};
use crate::mon::Party;
use crate::script::{Request, Response, ScriptHost};
use crate::starter::StarterChoice;
use crate::trainer::create_trainer_party;
use crate::video::Frame;
use alloc::boxed::Box;

/// Block bits `setmetatile` keeps (Gen3 `MapGridSetMetatileIdAt` keeps the elevation).
const ELEVATION_MASK: u16 = 0xF000;
/// Gen3 `MON_GIVEN_TO_PARTY` / `MON_CANT_GIVE` (`givemon` results).
const MON_GIVEN_TO_PARTY: u16 = 0;
const MON_CANT_GIVE: u16 = 2;
/// Gen3 `MAPGRID_COLLISION_MASK`.
const COLLISION_MASK: u16 = 3 << BLOCK_COLLISION_SHIFT;

pub(super) struct Host<'a, G: GameData> {
    pub game: &'a G,
    pub s: &'a mut State,
    pub f: &'a mut Frame,
    /// Map scripts (Gen3 `RunScriptImmediately`): waiting requests are logged and answered at
    /// once, text and warps are ignored.
    pub immediate: bool,
}

const UNIT: Option<Response> = Some(Response::Unit);

impl<G: GameData> Host<'_, G> {
    /// The object a `WaitMove` / `Move` id names (`0` = the last mover, Gen3 `sMovingNpcId`).
    fn mover(&self, obj: u8) -> u8 {
        if obj == 0 { self.s.last_mover } else { obj }
    }

    /// Whether a waiting request is done.
    fn done(&mut self, r: &Request) -> bool {
        let s = &mut *self.s;
        match *r {
            Request::WaitMessage => !s.text.is_printing(),
            Request::WaitButton => s.input.pressed.0 & (Buttons::A.0 | Buttons::B.0) != 0,
            Request::Lock | Request::LockAll => {
                let selected_settled = s
                    .selected
                    .is_none_or(|(id, map)| s.objects.is_settled_on(id, map));
                s.objects.is_settled(PLAYER) && selected_settled
            }
            Request::WaitMove(obj) => {
                let id = self.mover(obj);
                self.s.objects.moves_done(id)
            }
            Request::Delay(_) => {
                s.delay = s.delay.saturating_sub(1);
                s.delay == 0
            }
            Request::Fade(_) => s.fader.is_done(),
            Request::WaitDoor => s.door == 0,
            Request::WaitState => !s.transition.is_active() && s.fader.is_done() && s.door == 0,
            // Never answered: the warp's map load ends the script (Gen3 `warp` returns TRUE
            // and `CB2_LoadMap` resets the script context).
            Request::Warp { .. } => false,
            Request::Multichoice { .. } => menu::done(self, r),
            Request::TrainerApproach { .. } => trainer::done(self, r),
            _ => true,
        }
    }
}

impl<G: GameData> ScriptHost for Host<'_, G> {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        let game = self.game;
        let s = &mut *self.s;
        match *r {
            Request::GetFlag(fl) => Some(Response::Bool(s.vars.flag(fl))),
            Request::SetFlag(fl, on) => {
                s.vars.set_flag(fl, on);
                UNIT
            }
            Request::GetVar(v) => Some(Response::Value(s.vars.var(v))),
            Request::SetVar(v, x) => {
                s.vars.set_var(v, x);
                UNIT
            }
            Request::Random(n) => {
                let r = s.rng.random();
                Some(Response::Value(if n == 0 { 0 } else { r % n }))
            }
            Request::PlayerPos => {
                let (x, y) = s.objects.player().pos();
                Some(Response::Pos(x, y))
            }
            Request::PlayerFacing => Some(Response::Value(s.objects.player().facing as u16)),
            Request::CurrentMap => Some(Response::Value(s.field.map().0)),
            Request::Log(line) => {
                s.log(line);
                UNIT
            }
            Request::Message(t) if !self.immediate => {
                let vars = Some(&s.str_vars);
                (s.text).print_vars(game, game.text(t), &s.player_name, vars, self.f);
                UNIT
            }
            Request::CloseMessage => {
                if s.text.is_open() {
                    s.text.close(self.f);
                }
                UNIT
            }
            Request::YesNo if self.immediate => Some(Response::Bool(false)),
            Request::YesNo => {
                s.text.start_yes_no(game, self.f);
                None
            }
            Request::Lock | Request::LockAll => {
                s.objects.freeze_all();
                self.wait(r)
            }
            Request::Release | Request::ReleaseAll => {
                if s.text.is_open() {
                    s.text.close(self.f);
                }
                s.objects.unfreeze_all();
                UNIT
            }
            Request::FacePlayer => {
                // Gen3 `faceplayer`: the opposite of the player's facing.
                if let Some((id, map)) = s.selected {
                    let d = s.objects.player().facing.opposite();
                    s.objects.face_on(id, map, d);
                }
                UNIT
            }
            Request::Move { obj, steps } => {
                s.objects.start_moves(obj, steps);
                s.last_mover = obj;
                UNIT
            }
            Request::SetObjectPos { obj, x, y } => {
                s.objects.set_pos(obj, x, y);
                UNIT
            }
            Request::ShowObject(obj) => {
                s.objects.show(obj);
                UNIT
            }
            Request::HideObject(obj) => {
                s.objects.hide(obj);
                UNIT
            }
            Request::AddObject(obj) => {
                // Gen3 `TrySpawnObjectEvent`: nothing if it already exists.
                if s.objects.get(obj).is_none()
                    && let Some(ev) = s.template(obj).copied()
                {
                    let vars = &s.vars;
                    s.objects.add(&ev, |v| vars.var(v));
                }
                UNIT
            }
            Request::RemoveObject(obj) => {
                s.objects.remove(obj);
                if let Some(fl) = s.template(obj).and_then(|t| t.hide_flag) {
                    s.vars.set_flag(fl, true);
                }
                UNIT
            }
            Request::Turn { obj, dir } => {
                s.objects.face(obj, dir);
                UNIT
            }
            Request::SetMovementType { obj, movement } => {
                s.edit_template(obj, |t| t.movement = movement);
                s.objects.set_movement_type(obj, movement);
                UNIT
            }
            Request::SetObjectTemplatePos { obj, x, y } => {
                s.edit_template(obj, |t| (t.x, t.y) = (x, y));
                UNIT
            }
            Request::Delay(0) => UNIT,
            Request::Delay(n) => {
                s.delay = n;
                self.wait(r)
            }
            Request::Fade(_) if self.immediate => {
                s.log("fade in a map script ignored");
                UNIT
            }
            Request::Fade(fade) => {
                s.fader.start(fade);
                self.wait(r)
            }
            Request::Warp { .. } if self.immediate => {
                s.log("warp in a map script ignored");
                UNIT
            }
            Request::Warp { map, warp, x, y } => {
                if game.has_map(map) {
                    s.transition.start(game, map, warp, x, y);
                    s.objects.freeze_all();
                    None
                } else {
                    s.log("warp to a missing map ignored");
                    UNIT
                }
            }
            Request::SetMetatile {
                x,
                y,
                metatile,
                blocked,
            } => {
                let old = s.field.block(game, x, y);
                let collision = if blocked { COLLISION_MASK } else { 0 };
                let block = (old & ELEVATION_MASK) | (metatile & BLOCK_METATILE_MASK) | collision;
                s.field.set_block(game, x, y, block, self.f);
                UNIT
            }
            Request::RedrawMap => {
                s.field.redraw(game, self.f);
                UNIT
            }
            Request::Door { .. } => {
                // TODO: door graphics (Gen3 `field_door.c`); only the timing is modelled.
                s.door = DOOR_FRAMES;
                UNIT
            }
            Request::WaitMove(obj) => {
                if obj != 0 {
                    s.last_mover = obj;
                }
                self.wait(r)
            }
            // ---- party and battle (Milestone 4) ----
            Request::GiveMon { species, level } => {
                // Gen3 `ScriptGiveMon`: the party, else the PC (none yet).
                let mon = s.create_mon(game, species, level);
                Some(Response::Value(match s.party.push(mon) {
                    Ok(_) => MON_GIVEN_TO_PARTY,
                    Err(_) => {
                        s.log("givemon: party full and no PC");
                        MON_CANT_GIVE
                    }
                }))
            }
            Request::PartySize => Some(Response::Value(s.party.len() as u16)),
            Request::HealParty => {
                s.party.heal(game.battle());
                UNIT
            }
            Request::SetRespawn(id) => {
                s.respawn = id;
                UNIT
            }
            Request::ChooseStarter(_) if self.immediate => {
                s.log("starter choice in a map script answered 0");
                Some(Response::Value(0))
            }
            Request::ChooseStarter(choices) => {
                // The starter screen owns the screen; the field text box is rebuilt with the
                // field afterwards.
                s.text = crate::text::TextBox::new();
                s.mode = Mode::Starter(Box::new(StarterRun {
                    choice: StarterChoice::new(choices, s.player_name),
                    leaving: None,
                }));
                None
            }
            Request::BattleWild { .. } | Request::BattleTrainer { .. } if self.immediate => {
                s.log("battle in a map script answered Ran");
                Some(Response::Value(BattleOutcome::Ran as u16))
            }
            Request::BattleWild { .. } | Request::BattleTrainer { .. } if !s.can_battle() => {
                s.log("battle without a mon that can fight answered Won");
                Some(Response::Value(BattleOutcome::Won as u16))
            }
            Request::BattleWild {
                species,
                level,
                first_battle,
            } => {
                // Gen3 `CreateScriptedWildMon` / the first battle's `CreateMon`.
                let mon = s.create_mon(game, species, level);
                let mut opponents = Party::default();
                let _ = opponents.push(mon);
                let kind = BattleKind::Wild { first_battle };
                s.start_battle(game, kind, opponents, Origin::Script, self.f);
                None
            }
            Request::BattleTrainer { trainer, defeat } => {
                let opponents = game
                    .trainer(trainer)
                    .map(|t| create_trainer_party(game.battle(), t, &mut s.rng))
                    .filter(|p| p.first_usable().is_some());
                let Some(opponents) = opponents else {
                    s.log("battle against an unknown or empty trainer answered Won");
                    return Some(Response::Value(BattleOutcome::Won as u16));
                };
                let kind = BattleKind::Trainer { trainer, defeat };
                s.start_battle(game, kind, opponents, Origin::Script, self.f);
                None
            }
            Request::Message(_)
            | Request::WaitMessage
            | Request::WaitButton
            | Request::WaitDoor
            | Request::WaitState => self.wait(r),
            // ---- Milestone 8 ----
            Request::ItemAdd { .. }
            | Request::ItemRemove { .. }
            | Request::ItemCheck { .. }
            | Request::ItemSpace { .. }
            | Request::MoneyAdd(_)
            | Request::MoneyRemove(_)
            | Request::MoneyCheck(_) => items::begin(self, r),
            Request::Buffer { .. } => buffers::begin(self, r),
            Request::Multichoice { .. } => menu::begin(self, r),
            Request::SetWarp { .. } => warps::begin(self, r),
            Request::TrainerApproach { .. } => trainer::begin(self, r),
            Request::Report(rep) => {
                s.log(match rep {
                    crate::script::Report::Unsupported(what)
                    | crate::script::Report::Degraded(what) => what,
                });
                s.event(Event::Report(rep));
                UNIT
            }
        }
    }

    fn resume(&mut self, r: &Request) -> Option<Response> {
        if let Request::ChooseStarter(_)
        | Request::BattleWild { .. }
        | Request::BattleTrainer { .. } = r
        {
            // Set once the starter screen or the battle has handed the screen back.
            return self.s.answer.take().map(Response::Value);
        }
        if let Request::Multichoice { .. } = r {
            return menu::resume(self, r);
        }
        if let Request::YesNo = r {
            let t = &self.s.text;
            return match t.yes_no_result() {
                Some(yes) if !t.is_yes_no_active() => Some(Response::Bool(yes)),
                _ => None,
            };
        }
        self.done(r).then_some(Response::Unit)
    }
}

impl<G: GameData> Host<'_, G> {
    /// A request that waits: starts waiting (resumed from the next frame on), or in an
    /// immediate map script, is logged and answered at once.
    fn wait(&mut self, r: &Request) -> Option<Response> {
        if self.immediate {
            if !matches!(r, Request::Lock | Request::LockAll) {
                self.s
                    .log("waiting request in a map script answered at once");
            }
            return UNIT;
        }
        let _ = r;
        None
    }
}
