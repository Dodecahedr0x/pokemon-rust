//! The party menu (Gen3 `party_menu.c`; Milestone 9 Task PT).
//!
//! Six slots in Gen3's single layout (`sSinglePartyMenuWindowTemplate`: the lead at (1, 3),
//! 10 x 7 tiles, the others at (12, 1 + 3k), 18 x 3), the CANCEL button (24, 17), the hint
//! window (1, 17), the message window (1, 15, 28 x 4) and the action list at
//! (19, 19 - 2n), 10 tiles wide (`DisplaySelectionWindow`). Each slot shows the nickname, the
//! level, the gender symbol and the HP (or, teaching a move, ABLE / NOT ABLE / LEARNED).
//!
//! Modes ([`PartyMode`]):
//! - `Field` (START → POKéMON): SUMMARY, the field moves the mon knows whose
//!   `GameData::field_move_gate` lists them (Gen3 `SetPartyMonFieldSelectionActions`: move
//!   slot order, `sFieldMoves` order inside), SWITCH (when the party has two mons), ITEM
//!   (GIVE / TAKE), CANCEL. SWITCH swaps two slots; TAKE puts the held item in the bag.
//! - `Choose` (`ChoosePartyMon`, `PARTY_ACTION_CHOOSE_AND_CLOSE`): A answers the slot.
//! - `UseItem(tm)` (the bag's USE on a TM or HM, `ItemUseCB_TMHM`) and `MoveTutor(move)`
//!   (`ChooseMonForMoveTutor`): Gen3's teach flow: not compatible / already knows, learned
//!   into a free slot, else "delete a move?" → the summary's move pick (HM moves refused
//!   there) → "1, 2, and… Poof!" → learned, or "stop trying to teach?" → not learned. A TM is
//!   used up, an HM is not (`item < ITEM_HM01`: here `GameData::is_hm_move` of its move). The
//!   result is the slot that learned the move, else `Cancel`.
//! - `GiveItem(item)`: the slot answers; the item goes to a mon holding nothing.
//! - `Battle { forced }`: SHIFT / SUMMARY / CANCEL; fainted and active mons refused with Gen3's
//!   messages; forced has no CANCEL and B does nothing.
//!
//! Deviations: no slot pictures, mon or item icons, HP bar colours or the switch slide
//! animation (Gen3 `SlidePartyMenuBoxOneStep`): frames and text, the HP bar a plain line; a
//! field move reports `Unsupported("field move")` (Milestone 10) and GIVE
//! `Unsupported("bag")`; the move tutor accepts every mon (no `sTutorLearnsets` yet); no mail.

use super::kit::Gfx;
use super::kit::list::{MenuCursor, MenuInput};
use super::kit::widgets::{
    CHAR_FEMALE, CHAR_MALE, CHAR_SLASH, EOS, EXTRA_SYMBOL, Message, SYMBOL_LV, YesNo, draw_menu,
    menu_step, push, push_number,
};
use super::kit::window::{MapBuf, Panel, TileAlloc, colors, frame_kind};
use super::{
    PartyMode, Screen, ScreenCx, ScreenInput, ScreenKind, ScreenOut, ScreenResult, SummaryMode,
};
use crate::data::{FieldMove, FieldMoveGate, GameData, ItemId, MoveId, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::mon::{Gender, Learn, PARTY_SIZE, Pokemon};
use crate::script::Report;
use crate::str_vars::StrVars;
use crate::text::{Colors, Window};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// The CANCEL button's cursor index.
pub const CANCEL: u8 = PARTY_SIZE as u8;
/// Gen3 `sSinglePartyMenuWindowTemplate`: (left, top, width, height) per slot.
pub const SLOT_RECTS: [(u8, u8, u8, u8); PARTY_SIZE] = [
    (1, 3, 10, 7),
    (12, 1, 18, 3),
    (12, 4, 18, 3),
    (12, 7, 18, 3),
    (12, 10, 18, 3),
    (12, 13, 18, 3),
];
/// `sCancelButtonWindowTemplate`.
pub const CANCEL_RECT: (u8, u8, u8, u8) = (24, 17, 6, 2);
/// `sDefaultPartyMsgWindowTemplate`.
pub const HINT_RECT: (u8, u8, u8, u8) = (1, 17, 21, 2);
/// The message window (`sSinglePartyMenuWindowTemplate`'s last entry).
pub const MSG_RECT: (u8, u8, u8, u8) = (1, 15, 28, 4);
/// `DisplaySelectionWindow(SELECTWINDOW_ACTIONS)`: x, bottom row and width.
pub const ACTIONS_X: u8 = 19;
pub const ACTIONS_BOTTOM: u8 = 19;
pub const ACTIONS_W: u8 = 10;
/// `sItemGiveTakeWindowTemplate`.
pub const ITEM_RECT: (u8, u8, u8, u8) = (23, 13, 6, 6);
/// Actions at most (SUMMARY, four field moves, SWITCH, ITEM, CANCEL).
const MAX_ACTIONS: u8 = 8;

/// Gen3 `sFieldMoves` order.
pub const FIELD_MOVES: [FieldMove; 14] = [
    FieldMove::Cut,
    FieldMove::Flash,
    FieldMove::RockSmash,
    FieldMove::Strength,
    FieldMove::Surf,
    FieldMove::Fly,
    FieldMove::Dive,
    FieldMove::Waterfall,
    FieldMove::Teleport,
    FieldMove::Dig,
    FieldMove::SecretPower,
    FieldMove::MilkDrink,
    FieldMove::SoftBoiled,
    FieldMove::SweetScent,
];

/// A party action (Gen3 `MENU_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Summary,
    Switch,
    Item,
    Cancel,
    Shift,
    FieldMove(FieldMove),
}

/// What the menu waits for (for tools: the explorer drives the menu from it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// Picking a slot (`cursor`: the slot under the cursor, [`CANCEL`] on CANCEL).
    Slot { cursor: u8 },
    /// The action list of `slot`.
    Actions { slot: u8 },
    /// The GIVE / TAKE list.
    Item { slot: u8 },
    /// Picking where to move `from`.
    Switch { from: u8 },
    /// A message printing or waiting for A.
    Message,
    /// A yes/no question (`cursor` 0 yes).
    YesNo { cursor: u8 },
    /// A child screen (the summary) has the screen.
    Child,
}

/// After a message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Next {
    Close(ScreenResult),
    Slots,
    AskReplace,
    AskStop,
    ForgetSummary,
    /// Replace move slot `n` with the move being taught.
    Learn(u8),
}

/// After a yes/no.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Question {
    Replace,
    Stop,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Slots,
    Actions { slot: u8, cursor: MenuCursor },
    Item { slot: u8, cursor: MenuCursor },
    Switch { from: u8 },
    Msg(Next),
    Ask { q: Question, cursor: u8 },
    Summary,
}

/// The teach in progress (`gPartyMenu.data1`, `slotId`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Teaching {
    slot: u8,
    mv: MoveId,
    /// The TM / HM item (None for a tutor).
    item: Option<ItemId>,
}

struct Render {
    slots: Vec<Panel>,
    cancel: Panel,
    hint: Panel,
    popup: Option<Panel>,
    popup_base: u16,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartyMenu {
    pub mode: PartyMode,
    cursor: u8,
    st: St,
    teaching: Option<Teaching>,
    msg: Option<Box<Message>>,
    actions: Vec<Action>,
    /// The hint text shown when no message is.
    hint: UiText,
    gfx: Gfx<Render>,
}

impl PartyMenu {
    pub fn new(mode: PartyMode) -> Self {
        let hint = match mode {
            PartyMode::UseItem(_) | PartyMode::MoveTutor(_) => UiText::PartyTeachWhichMon,
            _ => UiText::PartyChoosePokemon,
        };
        PartyMenu {
            mode,
            cursor: 0,
            st: St::Slots,
            teaching: None,
            msg: None,
            actions: Vec::new(),
            hint,
            gfx: Gfx::none(),
        }
    }

    /// What the menu waits for.
    pub fn prompt(&self) -> Prompt {
        match self.st {
            St::Slots => Prompt::Slot {
                cursor: self.cursor,
            },
            St::Actions { slot, .. } => Prompt::Actions { slot },
            St::Item { slot, .. } => Prompt::Item { slot },
            St::Switch { from } => Prompt::Switch { from },
            St::Msg(_) => Prompt::Message,
            St::Ask { cursor, .. } => Prompt::YesNo { cursor },
            St::Summary => Prompt::Child,
        }
    }

    /// The action list shown (empty unless [`Prompt::Actions`]).
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// The action cursor while [`Prompt::Actions`] or [`Prompt::Item`].
    pub fn action_cursor(&self) -> Option<u8> {
        match self.st {
            St::Actions { cursor, .. } | St::Item { cursor, .. } => Some(cursor.pos),
            _ => None,
        }
    }
}

/// The move a [`PartyMode`] teaches.
fn taught<G: GameData>(game: &G, mode: PartyMode) -> Option<(MoveId, Option<ItemId>)> {
    match mode {
        PartyMode::UseItem(item) => game.tm_move(item).map(|m| (m, Some(item))),
        PartyMode::MoveTutor(m) => Some((m, None)),
        _ => None,
    }
}

/// Gen3 `CanMonLearnTMTutor`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Teachable {
    Able,
    NotAble,
    Learned,
}

pub fn teachable<G: GameData>(
    game: &G,
    mon: &Pokemon,
    mv: MoveId,
    item: Option<ItemId>,
) -> Teachable {
    if mon.moves.contains(&Some(mv)) {
        Teachable::Learned
    } else if item.is_none_or(|i| game.can_learn_tm(mon.species, i)) {
        Teachable::Able
    } else {
        Teachable::NotAble
    }
}

/// The field moves `mon` lists (Gen3 `SetPartyMonFieldSelectionActions`).
pub fn field_moves<G: GameData>(
    game: &G,
    mon: &Pokemon,
    flag: impl Fn(crate::data::FlagId) -> bool,
) -> Vec<FieldMove> {
    let mut out = Vec::new();
    for m in mon.moves.iter().flatten() {
        for fm in FIELD_MOVES {
            let listed = match game.field_move_gate(fm) {
                FieldMoveGate::PartyMove { move_id, .. } => move_id == *m,
                FieldMoveGate::Flag(_) | FieldMoveGate::Never => false,
            };
            if listed {
                out.push(fm);
                break;
            }
        }
    }
    // A game-flag gate (Silice's modules) lists the move on every mon once its flag is set.
    for fm in FIELD_MOVES {
        if let FieldMoveGate::Flag(f) = game.field_move_gate(fm)
            && flag(f)
            && !out.contains(&fm)
        {
            out.push(fm);
        }
    }
    out
}

fn move_name<G: GameData>(game: &G, m: MoveId) -> &'static [u8] {
    game.battle()
        .moves
        .get(m.0 as usize)
        .map_or(&[EOS][..], |i| i.name)
}

fn vars(mon: &[u8], other: &[u8]) -> StrVars {
    let mut v = StrVars::new();
    v.set(0, mon);
    v.set(1, other);
    v
}

impl PartyMenu {
    fn layout<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Render {
        let game = cx.game;
        let mut tiles = TileAlloc::full();
        let slots = SLOT_RECTS
            .iter()
            .map(|&(l, t, w, h)| tiles.panel(game, l, t, w, h, true))
            .collect();
        let (l, t, w, h) = CANCEL_RECT;
        let cancel = tiles.panel(game, l, t, w, h, true);
        let (l, t, w, h) = HINT_RECT;
        let hint = tiles.panel(game, l, t, w, h, true);
        let (_, _, w, h) = MSG_RECT;
        // The message canvas lives in the state ([`Message`]): its tiles are reserved here.
        let msg_base = tiles.take(w, h);
        debug_assert_eq!(msg_base, msg_tiles_base());
        let popup_base = tiles.take(ACTIONS_W, MAX_ACTIONS * 2);
        Render {
            slots,
            cancel,
            hint,
            popup: None,
            popup_base,
        }
    }

    fn draw_slot<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render, i: usize) {
        let game = cx.game;
        let p = &mut r.slots[i];
        let Some(mon) = cx.party.get(i) else {
            return;
        };
        let c = colors(game);
        let selected = self.cursor as usize == i
            || matches!(self.st, St::Switch { from } if from as usize == i);
        let cc = if selected {
            Colors::new(c.fg, c.bg, c.shadow)
        } else {
            c
        };
        p.canvas.fill(cc.bg);
        let mut name = Vec::with_capacity(12);
        push(&mut name, &mon.nickname);
        name.push(EOS);
        let mut lv = Vec::with_capacity(6);
        lv.extend([EXTRA_SYMBOL, SYMBOL_LV]);
        push_number(&mut lv, mon.level as u32, 3, false);
        lv.push(EOS);
        let gender = match mon.gender(game.battle()) {
            Gender::Male => Some(CHAR_MALE),
            Gender::Female => Some(CHAR_FEMALE),
            Gender::Genderless => None,
        };
        let mut hp = Vec::with_capacity(8);
        push_number(&mut hp, mon.hp as u32, 3, true);
        hp.push(CHAR_SLASH);
        push_number(&mut hp, mon.stats[0] as u32, 3, true);
        hp.push(EOS);
        let desc = taught(game, self.mode).map(|(mv, item)| match teachable(game, mon, mv, item) {
            Teachable::Able => UiText::PartyAble,
            Teachable::NotAble => UiText::PartyNotAble,
            Teachable::Learned => UiText::PartyLearned,
        });
        let bar = |p: &mut Panel, x: i16, y: i16, w: i16| {
            let max = mon.stats[0].max(1) as i32;
            let fill = (mon.hp as i32 * w as i32 / max) as i16;
            p.canvas.fill_rect(x, y, w, 3, cc.shadow);
            p.canvas
                .fill_rect(x, y, fill.max(if mon.hp > 0 { 1 } else { 0 }), 3, cc.fg);
        };
        if i == 0 {
            p.text_colored(game, &name, 2, 1, cc);
            p.text_colored(game, &lv, 2, 17, cc);
            if let Some(g) = gender {
                p.text_colored(game, &[g, EOS], 56, 17, cc);
            }
            match desc {
                Some(d) => p.text_colored(game, game.ui_text(d), 2, 33, cc),
                None => {
                    p.text_colored(game, &hp, 16, 33, cc);
                    bar(p, 8, 50, 64);
                }
            }
        } else {
            p.text_colored(game, &name, 2, 1, cc);
            p.text_colored(game, &lv, 64, 1, cc);
            if let Some(g) = gender {
                p.text_colored(game, &[g, EOS], 56, 1, cc);
            }
            match desc {
                Some(d) => p.text_colored(game, game.ui_text(d), 92, 1, cc),
                None => {
                    p.text_colored(game, &hp, 92, 1, cc);
                    bar(p, 96, 18, 48);
                }
            }
        }
    }

    fn draw_cancel<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        let c = colors(game);
        let cc = if self.cursor == CANCEL {
            Colors::new(c.fg, c.bg, c.shadow)
        } else {
            c
        };
        r.cancel.canvas.fill(cc.bg);
        r.cancel
            .text_colored(game, game.ui_text(UiText::PartyCancel), 4, 1, cc);
    }

    fn draw_hint<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        r.hint.clear(cx.game);
        r.hint.text(cx.game, cx.game.ui_text(self.hint), 0, 1);
    }

    /// Composes and uploads the map.
    fn compose<G: GameData>(&self, cx: &mut ScreenCx<'_, G>, r: &Render) {
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        for (i, p) in r.slots.iter().enumerate() {
            if cx.party.get(i).is_some() {
                map.put(p);
            }
        }
        let forced = matches!(self.mode, PartyMode::Battle { forced: true });
        if !forced {
            map.put(&r.cancel);
        }
        match &self.msg {
            Some(m) if m.shown => map.put_window(m.window(), true),
            _ => map.put(&r.hint),
        }
        if let Some(p) = &r.popup {
            map.put(p);
        }
        map.flush(cx.f);
    }

    fn redraw_all<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let mut r = self.layout(cx);
        for i in 0..PARTY_SIZE {
            self.draw_slot(cx, &mut r, i);
        }
        self.draw_cancel(cx, &mut r);
        self.draw_hint(cx, &mut r);
        r.popup = self.popup_panel(cx, r.popup_base);
        if let Some(m) = &mut self.msg {
            m.touch();
        }
        self.compose(cx, &r);
        self.gfx.0 = Some(Box::new(r));
    }

    /// The popup the state shows (actions, item list, yes/no).
    fn popup_panel<G: GameData>(&self, cx: &ScreenCx<'_, G>, base: u16) -> Option<Panel> {
        let game = cx.game;
        let mut tiles = TileAlloc::at(base);
        match self.st {
            St::Actions { cursor, .. } => {
                let n = self.actions.len() as u8;
                let mut p = tiles.panel(
                    game,
                    ACTIONS_X,
                    ACTIONS_BOTTOM - n * 2,
                    ACTIONS_W,
                    n * 2,
                    true,
                );
                let labels: Vec<&[u8]> = self
                    .actions
                    .iter()
                    .map(|a| action_label(game, *a))
                    .collect();
                draw_menu(game, &mut p, &labels, cursor.pos);
                Some(p)
            }
            St::Item { cursor, .. } => {
                let (l, t, w, h) = ITEM_RECT;
                let mut p = tiles.panel(game, l, t, w, h, true);
                let labels = [
                    game.ui_text(UiText::PartyGive),
                    game.ui_text(UiText::PartyTake),
                    game.ui_text(UiText::PartyCancel),
                ];
                draw_menu(game, &mut p, &labels, cursor.pos);
                Some(p)
            }
            St::Ask { cursor, .. } => {
                let y = YesNo::new(game, &mut tiles, cursor == 1);
                Some(y.panel)
            }
            _ => None,
        }
    }

    /// Re-makes the popup and the map after a state change.
    fn relayout<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(mut r) = self.gfx.0.take() else {
            return;
        };
        r.popup = self.popup_panel(cx, r.popup_base);
        self.compose(cx, &r);
        self.gfx.0 = Some(r);
    }

    fn redraw_slots<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(mut r) = self.gfx.0.take() else {
            return;
        };
        for i in 0..PARTY_SIZE {
            self.draw_slot(cx, &mut r, i);
        }
        self.draw_cancel(cx, &mut r);
        self.gfx.0 = Some(r);
    }

    /// Redraws the slots (or CANCEL) the cursor left and entered: a cursor move stays
    /// within the visible upload budget.
    fn redraw_two<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, a: u8, b: u8) {
        let Some(mut r) = self.gfx.0.take() else {
            return;
        };
        for i in [a, b] {
            if i == CANCEL {
                self.draw_cancel(cx, &mut r);
            } else if (i as usize) < PARTY_SIZE {
                self.draw_slot(cx, &mut r, i as usize);
            }
        }
        self.gfx.0 = Some(r);
    }

    fn set_hint<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, hint: UiText) {
        self.hint = hint;
        if let Some(mut r) = self.gfx.0.take() {
            self.draw_hint(cx, &mut r);
            self.gfx.0 = Some(r);
        }
    }

    fn show_msg<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        text: UiText,
        v: Option<&StrVars>,
        wait_press: bool,
        next: Next,
    ) {
        let (l, t, w, h) = MSG_RECT;
        let m = self.msg.get_or_insert_with(|| {
            Box::new(Message::new(
                Window::new(l, t, w, h, msg_tiles_base()),
                cx.options.text_speed,
            ))
        });
        m.start(
            cx.game,
            cx.game.ui_text(text),
            &cx.player_name[..],
            v,
            wait_press,
        );
        self.st = St::Msg(next);
        self.relayout(cx);
    }

    fn hide_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        if let Some(m) = &mut self.msg {
            m.hide();
        }
        self.relayout(cx);
    }

    /// The menu's input while picking a slot (Gen3 `HandleChooseMonInput`'s d-pad): up/down
    /// through the slots and CANCEL with wrap, left to the lead, right from the lead to slot 1.
    fn move_cursor(&mut self, len: u8, pressed: Buttons, cancel: bool) -> bool {
        let order: Vec<u8> = (0..len).chain(cancel.then_some(CANCEL)).collect();
        let at = order.iter().position(|&c| c == self.cursor).unwrap_or(0);
        let to = if pressed.contains(Buttons::UP) {
            order[(at + order.len() - 1) % order.len()]
        } else if pressed.contains(Buttons::DOWN) {
            order[(at + 1) % order.len()]
        } else if pressed.contains(Buttons::LEFT) && self.cursor != 0 && self.cursor != CANCEL {
            0
        } else if pressed.contains(Buttons::RIGHT) && self.cursor == 0 && len > 1 {
            1
        } else {
            return false;
        };
        self.cursor = to;
        true
    }

    fn actions_for<G: GameData>(&self, cx: &ScreenCx<'_, G>, slot: u8) -> Vec<Action> {
        let mut a = Vec::with_capacity(MAX_ACTIONS as usize);
        match self.mode {
            PartyMode::Battle { forced } => {
                a.extend([Action::Shift, Action::Summary]);
                if !forced {
                    a.push(Action::Cancel);
                }
            }
            _ => {
                a.push(Action::Summary);
                if let Some(mon) = cx.party.get(slot as usize) {
                    let vars = &*cx.vars;
                    for fm in field_moves(cx.game, mon, |f| vars.flag(f))
                        .into_iter()
                        .take(4)
                    {
                        a.push(Action::FieldMove(fm));
                    }
                }
                if cx.party.len() > 1 {
                    a.push(Action::Switch);
                }
                a.push(Action::Item);
                a.push(Action::Cancel);
            }
        }
        a
    }

    /// A on slot `slot` while picking.
    fn choose<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, slot: u8) -> ScreenOut {
        match self.mode {
            PartyMode::Choose => ScreenOut::Done(ScreenResult::Value(slot as u16)),
            PartyMode::GiveItem(item) => {
                let pocket = cx.game.item(item).map(|i| i.pocket);
                match (cx.party.get_mut(slot as usize), pocket) {
                    (Some(mon), Some(p)) if mon.held.is_none() && cx.bag.remove(p, item, 1) => {
                        mon.held = Some(item);
                        ScreenOut::Done(ScreenResult::Value(slot as u16))
                    }
                    _ => {
                        cx.report(Report::Degraded("party give: mon already holds an item"));
                        ScreenOut::Done(ScreenResult::Cancel)
                    }
                }
            }
            PartyMode::UseItem(_) | PartyMode::MoveTutor(_) => match taught(cx.game, self.mode) {
                Some((mv, item)) => {
                    self.teach(cx, slot, mv, item);
                    ScreenOut::Running
                }
                None => {
                    cx.report(Report::Unsupported("item use"));
                    ScreenOut::Done(ScreenResult::Cancel)
                }
            },
            PartyMode::Field | PartyMode::Battle { .. } => {
                self.actions = self.actions_for(cx, slot);
                let cursor = MenuCursor::new(self.actions.len() as u8, 0);
                self.st = St::Actions { slot, cursor };
                self.set_hint(cx, UiText::PartyDoWhatWithMon);
                self.relayout(cx);
                ScreenOut::Running
            }
        }
    }

    /// Gen3 `ItemUseCB_TMHM` / `TryTutorSelectedMon`.
    fn teach<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        slot: u8,
        mv: MoveId,
        item: Option<ItemId>,
    ) {
        let game = cx.game;
        let Some(mon) = cx.party.get_mut(slot as usize) else {
            return;
        };
        self.teaching = Some(Teaching { slot, mv, item });
        let v = vars(&mon.nickname, move_name(game, mv));
        match teachable(game, mon, mv, item) {
            Teachable::NotAble => self.show_msg(
                cx,
                UiText::PartyCantLearnMove,
                Some(&v),
                false,
                Next::Close(ScreenResult::Cancel),
            ),
            Teachable::Learned => self.show_msg(
                cx,
                UiText::PartyAlreadyKnows,
                Some(&v),
                false,
                Next::Close(ScreenResult::Cancel),
            ),
            Teachable::Able => match mon.try_learn(game.battle(), mv) {
                Learn::Learned(_) => self.learned(cx),
                Learn::AlreadyKnown => self.show_msg(
                    cx,
                    UiText::PartyAlreadyKnows,
                    Some(&v),
                    false,
                    Next::Close(ScreenResult::Cancel),
                ),
                Learn::Full => self.show_msg(
                    cx,
                    UiText::PartyNeedsToReplaceMove,
                    Some(&v),
                    false,
                    Next::AskReplace,
                ),
            },
        }
    }

    /// Gen3 `Task_LearnedMove`: friendship, the TM used up, "learned" (waits for A or B).
    fn learned<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let game = cx.game;
        let Some(t) = self.teaching else { return };
        let Some(mon) = cx.party.get_mut(t.slot as usize) else {
            return;
        };
        if let Some(item) = t.item {
            let hold = mon.held.and_then(|i| game.hold_effect(i));
            crate::world::friendship::on_event(
                mon,
                crate::world::friendship::FriendshipEvent::LearnTmHm,
                hold,
                game.battle(),
                cx.rng,
            );
            if !game.is_hm_move(t.mv)
                && let Some(p) = game.item(item).map(|i| i.pocket)
            {
                cx.bag.remove(p, item, 1);
            }
        }
        let v = vars(&mon.nickname, move_name(game, t.mv));
        self.redraw_slots(cx);
        self.show_msg(
            cx,
            UiText::PartyLearnedMove,
            Some(&v),
            true,
            Next::Close(ScreenResult::Value(t.slot as u16)),
        );
    }

    fn teach_vars<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Option<StrVars> {
        let t = self.teaching?;
        let mon = cx.party.get(t.slot as usize)?;
        Some(vars(&mon.nickname, move_name(cx.game, t.mv)))
    }

    fn after_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, next: Next) -> ScreenOut {
        match next {
            Next::Close(r) => return ScreenOut::Done(r),
            Next::Slots => {
                self.st = St::Slots;
                self.hide_msg(cx);
            }
            Next::AskReplace => {
                self.st = St::Ask {
                    q: Question::Replace,
                    cursor: 0,
                };
                self.relayout(cx);
            }
            Next::AskStop => {
                self.st = St::Ask {
                    q: Question::Stop,
                    cursor: 0,
                };
                self.relayout(cx);
            }
            Next::ForgetSummary => {
                if let Some(t) = self.teaching {
                    self.st = St::Summary;
                    self.gfx.0 = None;
                    return ScreenOut::Push(ScreenKind::Summary {
                        slot: t.slot,
                        mode: SummaryMode::ReplaceMove(t.mv),
                    });
                }
                return ScreenOut::Done(ScreenResult::Cancel);
            }
            Next::Learn(n) => {
                if let Some(t) = self.teaching
                    && let Some(mon) = cx.party.get_mut(t.slot as usize)
                {
                    mon.replace_move(cx.game.battle(), n, t.mv);
                }
                self.learned(cx);
            }
        }
        ScreenOut::Running
    }

    fn answer<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, q: Question, yes: bool) {
        let v = self.teach_vars(cx);
        match (q, yes) {
            (Question::Replace, true) => self.show_msg(
                cx,
                UiText::PartyWhichMoveToForget,
                v.as_ref(),
                false,
                Next::ForgetSummary,
            ),
            (Question::Replace, false) => self.show_msg(
                cx,
                UiText::PartyStopLearningMove,
                v.as_ref(),
                false,
                Next::AskStop,
            ),
            (Question::Stop, true) => self.show_msg(
                cx,
                UiText::PartyMoveNotLearned,
                v.as_ref(),
                false,
                Next::Close(ScreenResult::Cancel),
            ),
            (Question::Stop, false) => self.show_msg(
                cx,
                UiText::PartyNeedsToReplaceMove,
                v.as_ref(),
                false,
                Next::AskReplace,
            ),
        }
    }

    /// The summary closed (`Value(slot)` viewing, `Value(move slot)` / 4 picking a move).
    fn child_closed<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, r: ScreenResult) {
        match (self.teaching, r) {
            (Some(t), ScreenResult::Value(n)) if n < 4 && self.st == St::Summary => {
                let old = cx
                    .party
                    .get(t.slot as usize)
                    .and_then(|m| m.moves[n as usize])
                    .map_or(&[EOS][..], |m| move_name(cx.game, m));
                let name = cx
                    .party
                    .get(t.slot as usize)
                    .map(|m| m.nickname)
                    .unwrap_or([EOS; crate::mon::NAME_LEN]);
                let v = vars(&name, old);
                self.redraw_all(cx);
                self.show_msg(
                    cx,
                    UiText::PartyForgotMove,
                    Some(&v),
                    false,
                    Next::Learn(n as u8),
                );
            }
            (Some(_), _) if self.st == St::Summary => {
                self.redraw_all(cx);
                let v = self.teach_vars(cx);
                self.show_msg(
                    cx,
                    UiText::PartyStopLearningMove,
                    v.as_ref(),
                    false,
                    Next::AskStop,
                );
            }
            (_, r) => {
                if let ScreenResult::Value(s) = r
                    && (s as usize) < cx.party.len()
                {
                    self.cursor = s as u8;
                }
                self.st = St::Slots;
                self.hint = taught_hint(self.mode);
                self.redraw_all(cx);
            }
        }
    }

    fn action<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, slot: u8, a: Action) -> ScreenOut {
        match a {
            Action::Summary => {
                self.st = St::Summary;
                self.gfx.0 = None;
                ScreenOut::Push(ScreenKind::Summary {
                    slot,
                    mode: SummaryMode::View,
                })
            }
            Action::Switch => {
                self.st = St::Switch { from: slot };
                self.set_hint(cx, UiText::PartyMoveToWhere);
                self.redraw_slots(cx);
                self.relayout(cx);
                ScreenOut::Running
            }
            Action::Item => {
                self.st = St::Item {
                    slot,
                    cursor: MenuCursor::new(3, 0),
                };
                self.relayout(cx);
                ScreenOut::Running
            }
            Action::Cancel => {
                self.back_to_slots(cx);
                ScreenOut::Running
            }
            Action::Shift => {
                let mon = cx.party.get(slot as usize);
                let text = if mon.is_some_and(|m| m.hp == 0) {
                    Some(UiText::PartyCantSwitchFainted)
                } else if slot == 0 {
                    Some(UiText::PartyAlreadyInBattle)
                } else {
                    None
                };
                match text {
                    Some(t) => {
                        let name = mon
                            .map(|m| m.nickname)
                            .unwrap_or([EOS; crate::mon::NAME_LEN]);
                        let v = vars(&name, &[EOS]);
                        self.show_msg(cx, t, Some(&v), true, Next::Slots);
                        ScreenOut::Running
                    }
                    None => ScreenOut::Done(ScreenResult::Value(slot as u16)),
                }
            }
            Action::FieldMove(_) => {
                // Milestone 10 runs the field moves from the menu.
                cx.report(Report::Unsupported("field move"));
                self.back_to_slots(cx);
                ScreenOut::Running
            }
        }
    }

    fn back_to_slots<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.st = St::Slots;
        self.actions.clear();
        self.set_hint(cx, taught_hint(self.mode));
        self.redraw_slots(cx);
        self.relayout(cx);
    }

    /// TAKE (Gen3 `CursorCb_TakeItem`).
    fn take_item<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, slot: u8) {
        let game = cx.game;
        let Some(mon) = cx.party.get_mut(slot as usize) else {
            return;
        };
        let name = mon.nickname;
        match mon.held {
            None => {
                let v = vars(&name, &[EOS]);
                self.show_msg(cx, UiText::PartyNotHolding, Some(&v), false, Next::Slots);
            }
            Some(item) => {
                let info = game.item(item);
                if info.is_some_and(|i| cx.bag.add(i.pocket, item, 1)) {
                    mon.held = None;
                    let v = vars(&name, info.map_or(&[EOS][..], |i| i.name));
                    self.redraw_slots(cx);
                    self.show_msg(
                        cx,
                        UiText::PartyReceivedItemFromMon,
                        Some(&v),
                        false,
                        Next::Slots,
                    );
                } else {
                    // Gen3 `gText_BagFullCouldNotRemoveItem`.
                    cx.report(Report::Degraded("party take: bag full"));
                    self.back_to_slots(cx);
                }
            }
        }
    }
}

fn taught_hint(mode: PartyMode) -> UiText {
    match mode {
        PartyMode::UseItem(_) | PartyMode::MoveTutor(_) => UiText::PartyTeachWhichMon,
        _ => UiText::PartyChoosePokemon,
    }
}

fn action_label<G: GameData>(game: &G, a: Action) -> &'static [u8] {
    match a {
        Action::Summary => game.ui_text(UiText::PartySummary),
        Action::Switch => game.ui_text(UiText::PartySwitch),
        Action::Item => game.ui_text(UiText::PartyItem),
        Action::Cancel => game.ui_text(UiText::PartyCancel),
        Action::Shift => game.ui_text(UiText::PartyShift),
        // Gen3 prints the move's name (`gMoveNames[sFieldMoves[i]]`).
        Action::FieldMove(fm) => match game.field_move_gate(fm) {
            FieldMoveGate::PartyMove { move_id, .. } => move_name(game, move_id),
            _ => &[EOS],
        },
    }
}

/// The message canvas' first tile: after the slots, CANCEL and the hint ([`PartyMenu::layout`]).
const fn msg_tiles_base() -> u16 {
    let mut n = super::kit::window::FULL_BASE;
    let mut i = 0;
    while i < PARTY_SIZE {
        n += SLOT_RECTS[i].2 as u16 * SLOT_RECTS[i].3 as u16;
        i += 1;
    }
    n + CANCEL_RECT.2 as u16 * CANCEL_RECT.3 as u16 + HINT_RECT.2 as u16 * HINT_RECT.3 as u16
}

impl Screen for PartyMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if let Some(r) = cx.child.take() {
            self.child_closed(cx, r);
        }
        if self.gfx.0.is_none() {
            if self.st == St::Summary {
                // Back from a child without a result (never: the host gives one).
                self.st = St::Slots;
            }
            self.redraw_all(cx);
        }
        let len = cx.party.len() as u8;
        if self.cursor != CANCEL && self.cursor >= len {
            self.cursor = 0;
        }
        let pressed = input.pressed;
        let forced = matches!(self.mode, PartyMode::Battle { forced: true });
        let out = match self.st {
            St::Slots | St::Switch { .. } => {
                let switching = match self.st {
                    St::Switch { from } => Some(from),
                    _ => None,
                };
                let before = self.cursor;
                if self.move_cursor(len, pressed, !forced) {
                    self.redraw_two(cx, before, self.cursor);
                    ScreenOut::Running
                } else if pressed.contains(Buttons::A) {
                    match (switching, self.cursor) {
                        (Some(_), CANCEL) => {
                            self.back_to_slots(cx);
                            ScreenOut::Running
                        }
                        (None, CANCEL) => ScreenOut::Done(ScreenResult::Cancel),
                        (Some(from), to) => {
                            // Gen3 `SwitchSelectedMons` (without the slide).
                            cx.party.swap(from as usize, to as usize);
                            self.cursor = to;
                            self.back_to_slots(cx);
                            ScreenOut::Running
                        }
                        (None, slot) => self.choose(cx, slot),
                    }
                } else if pressed.contains(Buttons::B) {
                    if switching.is_some() {
                        self.back_to_slots(cx);
                        ScreenOut::Running
                    } else if forced {
                        ScreenOut::Running
                    } else {
                        ScreenOut::Done(ScreenResult::Cancel)
                    }
                } else {
                    ScreenOut::Running
                }
            }
            St::Actions { slot, mut cursor } => {
                let r = {
                    let render = self.gfx.0.as_deref_mut().expect("drawn");
                    match render.popup.as_mut() {
                        Some(p) => menu_step(cx.game, p, &mut cursor, pressed),
                        None => cursor.input(pressed),
                    }
                };
                self.st = St::Actions { slot, cursor };
                match r {
                    MenuInput::Chosen(i) => {
                        let a = self
                            .actions
                            .get(i as usize)
                            .copied()
                            .unwrap_or(Action::Cancel);
                        self.action(cx, slot, a)
                    }
                    MenuInput::Cancel => {
                        if !forced || self.actions.contains(&Action::Cancel) {
                            self.back_to_slots(cx);
                        }
                        ScreenOut::Running
                    }
                    _ => ScreenOut::Running,
                }
            }
            St::Item { slot, mut cursor } => {
                let r = {
                    let render = self.gfx.0.as_deref_mut().expect("drawn");
                    match render.popup.as_mut() {
                        Some(p) => menu_step(cx.game, p, &mut cursor, pressed),
                        None => cursor.input(pressed),
                    }
                };
                self.st = St::Item { slot, cursor };
                match r {
                    MenuInput::Chosen(0) => {
                        // GIVE opens the bag (Task B).
                        cx.report(Report::Unsupported("bag"));
                        self.back_to_slots(cx);
                        ScreenOut::Running
                    }
                    MenuInput::Chosen(1) => {
                        self.st = St::Slots;
                        self.take_item(cx, slot);
                        ScreenOut::Running
                    }
                    MenuInput::Chosen(_) | MenuInput::Cancel => {
                        self.back_to_slots(cx);
                        ScreenOut::Running
                    }
                    _ => ScreenOut::Running,
                }
            }
            St::Msg(next) => {
                let done = self
                    .msg
                    .as_mut()
                    .is_none_or(|m| m.update(cx.game, pressed, input.held));
                if done {
                    self.after_msg(cx, next)
                } else {
                    ScreenOut::Running
                }
            }
            St::Ask { q, cursor } => {
                let mut yes_no = YesNo {
                    panel: self
                        .gfx
                        .0
                        .as_deref_mut()
                        .and_then(|r| r.popup.take())
                        .unwrap_or_else(|| {
                            YesNo::new(cx.game, &mut TileAlloc::full(), cursor == 1).panel
                        }),
                    cursor,
                };
                let r = yes_no.input(cx.game, pressed);
                if let Some(render) = self.gfx.0.as_deref_mut() {
                    render.popup = Some(yes_no.panel);
                }
                match r {
                    Some(yes) => {
                        self.st = St::Ask { q, cursor };
                        self.answer(cx, q, yes);
                    }
                    None => {
                        self.st = St::Ask {
                            q,
                            cursor: yes_no.cursor,
                        };
                    }
                }
                ScreenOut::Running
            }
            St::Summary => ScreenOut::Running,
        };
        // The message and the popup first: they answer the last press.
        let mut budget = super::kit::frame_budget(cx.f);
        if let Some(m) = &mut self.msg {
            m.flush_within(cx.f, &mut budget);
        }
        if let Some(r) = self.gfx.0.as_deref_mut() {
            if let Some(p) = &mut r.popup {
                p.flush_within(cx.f, &mut budget);
            }
            for p in &mut r.slots {
                p.flush_within(cx.f, &mut budget);
            }
            r.cancel.flush_within(cx.f, &mut budget);
            r.hint.flush_within(cx.f, &mut budget);
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        let (tag, a, b) = match self.st {
            St::Slots => (0, 0, 0),
            St::Actions { slot, cursor } => (1, slot, cursor.pos),
            St::Item { slot, cursor } => (2, slot, cursor.pos),
            St::Switch { from } => (3, from, 0),
            St::Msg(n) => (
                4,
                match n {
                    Next::Close(_) => 0,
                    Next::Slots => 1,
                    Next::AskReplace => 2,
                    Next::AskStop => 3,
                    Next::ForgetSummary => 4,
                    Next::Learn(k) => 5 + k,
                },
                0,
            ),
            St::Ask { q, cursor } => (5, q as u8, cursor),
            St::Summary => (6, 0, 0),
        };
        h.write(&[self.cursor, tag, a, b, self.actions.len() as u8]);
        if let Some(t) = self.teaching {
            h.write(&[t.slot]);
            h.write_u16(t.mv.0);
        }
        if let Some(m) = &self.msg {
            m.hash(h);
        }
    }
}
