//! [`ScriptHost`] for the world: every [`Request`], as the Gen3 script command it stands for
//! (`scrcmd.c`).
//!
//! Milestone 8 requests are routed to one file per owner: items and money ([`items`], Task H),
//! multichoice ([`menu`], H), string buffers ([`buffers`], H), the trainer approach
//! ([`trainer`], I) and warp slots ([`warps`], F).
//!
//! Milestone 9 requests go the same way: screens ([`screens`], Task K), effects and timed
//! waits ([`fx`], R), party, money and coins ([`party`], R) and the save ([`save`], S). After
//! Milestone 9 Wave 0 no task edits this file.

pub(in crate::world) use field::FieldEffects;
pub(in crate::world) use fx::whiteout_allowed;
pub(in crate::world) use rotating_tiles::RotatingTiles;
pub(in crate::world) use stash::Stash;

mod buffers;
mod field;
mod fx;
mod items;
mod menu;
mod party;
mod query;
pub(in crate::world) mod rotating_tiles;
mod save;
mod screens;
mod stash;
mod trainer;
mod warps;

use super::battle::{Origin, StarterRun};
use super::mode::Mode;
use super::{DOOR_FRAMES, Event, State};
use crate::Buttons;
use crate::battle::{BattleKind, BattleOutcome};
use crate::data::{BLOCK_COLLISION_SHIFT, BLOCK_METATILE_MASK, GameData, PLAYER};
use crate::mon::Party;
use crate::script::{Request, Response, ScriptHost};
use crate::starter::StarterChoice;
use crate::video::Frame;
use alloc::boxed::Box;

/// Block bits `setmetatile` keeps (Gen3 `MapGridSetMetatileIdAt` keeps the elevation).
const ELEVATION_MASK: u16 = 0xF000;
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

/// The `Report::Degraded` of money added (`addmoney`, prize money) in a game whose
/// `SpecialIds::max_money` is 0: the money stays 0.
pub(super) const NO_MONEY: &str = "money added without max_money";

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
            Request::WaitMoveAt(obj, map) => {
                let id = self.mover(obj);
                self.s.objects.moves_done_on(id, map)
            }
            Request::Delay(_) => {
                s.delay = s.delay.saturating_sub(1);
                s.delay == 0
            }
            Request::Fade(_) => s.fader.is_done(),
            Request::WaitDoor => s.door == 0,
            Request::WaitState => {
                !s.transition.is_active()
                    && s.fader.is_done()
                    && s.door == 0
                    && !s.field_move_busy()
            }
            // Never answered: the warp's map load ends the script (Gen3 `warp` returns TRUE
            // and `CB2_LoadMap` resets the script context).
            Request::Warp { .. } | Request::WarpHole(_) => false,
            // Never answered: `World::step_script` whites out when the count reaches 0.
            Request::FieldWhiteOut => {
                s.delay = s.delay.saturating_sub(1);
                false
            }
            Request::Multichoice { .. } => menu::done(self, r),
            Request::TrainerApproach { .. } => trainer::done(self, r),
            Request::WaitFieldEffect(_) => field::done(self, r),
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
                // Gen3 `GetPlayerTextSpeedDelay`: the option, read when the message starts.
                s.text.set_speed(s.options.text_speed);
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
            Request::YesNo { .. } if self.immediate => Some(Response::Bool(false)),
            Request::YesNo { default_no } => {
                s.text.start_yes_no_from(game, default_no, self.f);
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
            Request::RockSmashEncounter if self.immediate => {
                s.log("rock smash encounter in a map script answered false");
                Some(Response::Bool(false))
            }
            Request::RockSmashEncounter => {
                Some(Response::Bool(s.rock_smash_encounter(game, self.f)))
            }
            Request::FieldWhiteOut if self.immediate => {
                s.log("whiteout in a map script ignored");
                UNIT
            }
            Request::FieldWhiteOut => {
                s.delay = u16::from(super::battle::WHITEOUT_FRAMES);
                self.wait(r)
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
            Request::WarpHole(_) if self.immediate => {
                s.log("warp in a map script ignored");
                UNIT
            }
            Request::WarpHole(map) => {
                // Gen3 `ScrCmd_warphole`: the player's cell on the map below
                // (`SetWarpDestinationToFixedHoleWarp`), then `DoFallWarp`. The fall itself
                // (`FieldCB_FallWarpExit`) is a plain arrival here, facing south as Gen3's
                // `ResetInitialPlayerAvatarState` leaves it.
                match map.or_else(|| game.hole_warp(s.field.map())) {
                    Some(dest) if game.has_map(dest) => {
                        let (x, y) = s.objects.player().pos();
                        s.transition.start(game, dest, None, x, y);
                        s.objects.freeze_all();
                        None
                    }
                    _ => {
                        s.log("hole warp to a missing map ignored");
                        UNIT
                    }
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
            Request::WaitMove(obj) | Request::WaitMoveAt(obj, _) => {
                if obj != 0 {
                    s.last_mover = obj;
                }
                self.wait(r)
            }
            // Gen3 `applymovementat` / `showobjectat` / `hideobjectat`: the object of the named
            // map, which a crossing may have made the previous one.
            Request::MoveAt { obj, map, steps } => {
                s.objects.start_moves_on(obj, map, steps);
                s.last_mover = obj;
                UNIT
            }
            Request::ShowObjectAt(obj, map) | Request::HideObjectAt(obj, map) => {
                let visible = matches!(r, Request::ShowObjectAt(..));
                s.objects.set_visible_on(obj, map, visible);
                UNIT
            }
            // ---- party and battle (Milestone 4) ----
            Request::GiveMon {
                species,
                level,
                extra,
            } => Some(Response::Value(party::give_mon(
                self, species, level, extra,
            ))),
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
            Request::BattleWild { .. }
            | Request::BattleTrainer { .. }
            | Request::BattleMulti { .. }
                if self.immediate =>
            {
                s.log("battle in a map script answered Ran");
                Some(Response::Value(BattleOutcome::Ran as u16))
            }
            Request::BattleWild { .. }
            | Request::BattleTrainer { .. }
            | Request::BattleMulti { .. }
                if !s.can_battle() =>
            {
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
            Request::BattleTrainer {
                trainer, defeat, ..
            } => {
                // Wave G: a second trainer saw the player too (`BATTLE_TYPE_TWO_OPPONENTS`).
                let second = s.pair.b.map(|(b, d, _)| (b, d));
                let Some((opponents, double)) =
                    super::battle::trainer_party(game, &mut s.rng, trainer, second)
                else {
                    s.log("battle against an unknown or empty trainer answered Won");
                    return Some(Response::Value(BattleOutcome::Won as u16));
                };
                let kind = BattleKind::Trainer { trainer, defeat };
                s.start_battle_with(game, kind, opponents, double, Origin::Script, self.f);
                None
            }
            Request::BattleMulti {
                a,
                a_defeat,
                b,
                b_defeat,
            } => {
                let Some(partner) = game.partner() else {
                    s.log("multi battle without an in-game partner answered Won");
                    return Some(Response::Value(BattleOutcome::Won as u16));
                };
                // Gen3 `FillPartnerParty` (`gPlayerParty[3..]`), then the opponents.
                let own = s.party.len().min(crate::battle::MULTI_PARTY_SIZE as usize);
                let mons = crate::trainer::create_partner_party(game.battle(), partner, &mut s.rng);
                let Some((opponents, double)) =
                    super::battle::trainer_party(game, &mut s.rng, a, Some((b, b_defeat)))
                else {
                    s.log("multi battle against an unknown or empty trainer answered Won");
                    return Some(Response::Value(BattleOutcome::Won as u16));
                };
                let mut party = crate::mon::Party::default();
                for m in s.party.iter().take(own).chain(mons.iter()) {
                    let _ = party.push(*m);
                }
                s.party = party;
                let double = double.map(|d| crate::battle::Double {
                    partner: Some(partner.trainer),
                    split: [own as u8, d.split[1]],
                    ..d
                });
                let kind = BattleKind::Trainer {
                    trainer: a,
                    defeat: a_defeat,
                };
                s.start_battle_with(game, kind, opponents, double, Origin::Script, self.f);
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
            | Request::MoneyCheck(_)
            | Request::PcItemAdd { .. } => items::begin(self, r),
            Request::Buffer { .. } => buffers::begin(self, r),
            Request::Multichoice { .. } => menu::begin(self, r),
            Request::SetWarp { .. } => warps::begin(self, r),
            Request::TrainerApproach { .. } => trainer::begin(self, r),
            Request::TrainerPair { .. } => trainer::pair(self, r),
            // ---- Milestone 9 ----
            Request::Mart { .. }
            | Request::ChoosePartyMon(_)
            | Request::PlayerPc(_)
            | Request::SaveDialog
            | Request::HallOfFame
            | Request::Evolve { .. } => screens::begin(self, r),
            Request::ScreenFlash { .. }
            | Request::ScreenShake { .. }
            | Request::SetTint { .. }
            | Request::WaitInputOrTimeout(_) => fx::begin(self, r),
            Request::MoneyGet
            | Request::CoinsGet
            | Request::CoinsAdd(_)
            | Request::CoinsRemove(_)
            | Request::AddFriendship { .. }
            | Request::PartyMonSpecies(_)
            | Request::InGameTrade { .. } => party::begin(self, r),
            Request::PartyStash(op) => stash::begin(self, op),
            Request::FieldEffectArg { .. }
            | Request::FieldEffect { .. }
            | Request::WaitFieldEffect(_)
            | Request::CopyObjectPosToTemplate(_)
            | Request::SetMapLayout(_)
            | Request::CreateVirtualObject { .. }
            | Request::TurnVirtualObject { .. } => field::begin(self, r),
            Request::RotatingTilesInit { .. }
            | Request::RotatingTilesMove(_)
            | Request::RotatingTilesTurn
            | Request::RotatingTilesFree => rotating_tiles::begin(self, r),
            Request::FieldMoveEffect(m) => {
                s.start_field_move(game, m);
                UNIT
            }
            Request::Query(q) => Some(Response::Value(query::answer(self, q))),
            Request::BerryTree(id) => Some(Response::Berry(self.s.berry_trees.get(id))),
            Request::SetBerryTree(id, tree) => {
                self.s.berry_trees.set(id, tree);
                UNIT
            }
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
        | Request::BattleTrainer { .. }
        | Request::BattleMulti { .. } = r
        {
            // Set once the starter screen or the battle has handed the screen back.
            return self.s.answer.take().map(Response::Value);
        }
        if let Request::Multichoice { .. } = r {
            return menu::resume(self, r);
        }
        if let Request::TrainerApproach { .. } = r {
            return if self.done(r) {
                trainer::resume(self)
            } else {
                None
            };
        }
        if let Request::Mart { .. }
        | Request::ChoosePartyMon(_)
        | Request::PlayerPc(_)
        | Request::SaveDialog
        | Request::HallOfFame
        | Request::Evolve { .. } = r
        {
            return screens::resume(self, r);
        }
        if let Request::ScreenFlash { .. }
        | Request::ScreenShake { .. }
        | Request::SetTint { .. }
        | Request::WaitInputOrTimeout(_) = r
        {
            return fx::resume(self, r);
        }
        if let Request::MoneyGet
        | Request::CoinsGet
        | Request::CoinsAdd(_)
        | Request::CoinsRemove(_)
        | Request::AddFriendship { .. }
        | Request::PartyMonSpecies(_)
        | Request::InGameTrade { .. } = r
        {
            return party::resume(self, r);
        }
        if let Request::YesNo { .. } = r {
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
