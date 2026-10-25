//! The bag (Gen3 `item_menu.c`; Milestone 9 Task B).
//!
//! Gen3's windows (`sDefaultBagWindows`): the item list at (14, 2), 15 x 16 tiles, eight rows
//! (`MAX_ITEMS_SHOWN`), the description at (0, 13), 14 x 6, the pocket name at (4, 1), 8 x 2,
//! the message at (2, 15), 27 x 4; the context menu bottom-right, the quantity box at (24, 17)
//! (selling: (18, 11), 10 wide, with the price), the money box at (1, 1) while selling.
//!
//! - **Pockets** in Gen3's order ([`POCKETS`]: ITEMS, POKé BALLS, TMs & HMs, BERRIES, KEY
//!   ITEMS), switched with left/right (no wrap). Opening compacts every pocket and sorts the
//!   TM/HM and berries pockets by item id (`CompactItemsInBagPocket`, `SortBerriesOrTMHMs`).
//!   Each pocket keeps its cursor and scroll while the bag is open. The list ends with
//!   CLOSE BAG.
//! - **Field** (`ITEMMENULOCATION_FIELD`), the context menus of `OpenContextMenu`: ITEMS USE /
//!   GIVE / TOSS / CANCEL, KEY ITEMS USE / REGISTER (DESELECT on the registered item) / CANCEL,
//!   POKé BALLS GIVE / TOSS / CANCEL, TMs & HMs USE / GIVE / CANCEL, BERRIES CHECK TAG / USE /
//!   GIVE / TOSS / CANCEL.
//!   - USE (`item_use::field_use`): medicine, stones and TMs open the party menu
//!     (`PartyMode::UseItem`; a TM or HM first prints "Booted up a TM." and asks to teach it,
//!     `ItemUseOutOfBattle_TMHM`), the Repel family sets the repel steps or says the last one
//!     still lingers (`ItemUseOutOfBattle_Repel`), the Escape Rope closes the bag and warps
//!     (`ItemUseOutOfBattle_EscapeRope`), a content key item runs its script, the bikes and
//!     rods report `Unsupported` (Milestone 10), anything else prints Gen3's "not the time".
//!   - GIVE opens the party menu (`PartyMode::GiveItem`); a key item "can't be held".
//!   - TOSS asks how many (one: no question), confirms, throws away.
//!   - REGISTER sets `registered_item` (SELECT, `item_use::select_pressed`).
//! - **Battle**: USE (items with a battle use, `item_use::battle_usable`) answers
//!   `Value(item id)`; the battle uses it (Task BM).
//! - **Sell** (the mart): A on an item asks how many (the price shown at half), "I can pay
//!   ¥X. Would that be okay?", then sells (`Task_ItemContext_Sell`); a price of 0 "can't be
//!   bought".
//! - **ChooseBerry**: the BERRIES pocket only; A answers `Value(item id)`.
//!
//! Deviations: no scroll arrows or pocket slide. The bag sprite
//! (`GameData::ui_gfx(BagSprite(pocket))`, Gen3 centre (68, 66)) and the selected item's icon
//! (centre (24, 88)) are drawn when the game has them. The context menu is one column without Gen3's `ACTION_DUMMY` holes; the cursor and
//! pocket are not kept between openings (Gen3's `gBagPosition` lives in EWRAM, outside the
//! save: here it would be unhashed World state); no SELECT item swap; the TM/HM pocket prints
//! the item's name and its move instead of "No01 MOVE"; the description of CLOSE BAG is blank
//! (`gText_ReturnToVar1`).

use super::kit::Gfx;
use super::kit::list::{ListMenu, MenuCursor, MenuInput, Paging};
use super::kit::pictures::{self, Bg, Load};
use super::kit::widgets::{
    CHAR_CURRENCY, CHAR_CURSOR, CURSOR_W, EOS, Message, Quantity, ROW_H, YesNo, draw_menu,
    menu_step, money_box, push, push_number,
};
use super::kit::window::{MapBuf, Panel, TileAlloc, colors, frame_kind};
use super::{
    BagMode, PartyMode, Screen, ScreenCx, ScreenInput, ScreenKind, ScreenOut, ScreenResult,
};
use crate::bag::Slot;
use crate::data::{GameData, ItemId, Pocket, UiGfx, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::item_use::{self, FieldUse};
use crate::script::Report;
use crate::str_vars::StrVars;
use crate::text::Window;
use crate::video::layout;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Gen3's pocket order (`ITEMS_POCKET` .. `KEYITEMS_POCKET`).
pub const POCKETS: [Pocket; 5] = [
    Pocket::Items,
    Pocket::PokeBalls,
    Pocket::TmHm,
    Pocket::Berries,
    Pocket::KeyItems,
];
/// `sDefaultBagWindows[WIN_ITEM_LIST]`.
pub const LIST_RECT: (u8, u8, u8, u8) = (14, 2, 15, 16);
/// `WIN_DESCRIPTION`.
pub const DESC_RECT: (u8, u8, u8, u8) = (0, 13, 14, 6);
/// `WIN_POCKET_NAME`.
pub const POCKET_RECT: (u8, u8, u8, u8) = (4, 1, 8, 2);
/// `ITEMWIN_MESSAGE`.
pub const MSG_RECT: (u8, u8, u8, u8) = (2, 15, 27, 4);
/// `ITEMWIN_QUANTITY`.
pub const QTY_RECT: (u8, u8, u8, u8) = (24, 17, 5, 2);
/// `ITEMWIN_QUANTITY_WIDE` (selling).
pub const QTY_WIDE_RECT: (u8, u8, u8, u8) = (18, 11, 10, 2);
/// The context menu: x, bottom row and width (`ITEMWIN_1x2`'s column).
pub const CONTEXT_X: u8 = 22;
pub const CONTEXT_BOTTOM: u8 = 19;
pub const CONTEXT_W: u8 = 7;
/// Gen3 `MAX_ITEMS_SHOWN`.
pub const MAX_SHOWN: u16 = 8;
/// The popup's tiles (the largest: five context entries, 7 x 10).
const POPUP_TILES: (u8, u8) = (CONTEXT_W, 12);

/// A context-menu action (Gen3 `ACTION_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Use,
    Give,
    Toss,
    Register,
    Deselect,
    CheckTag,
    BattleUse,
    Cancel,
}

/// What the bag waits for (tools drive the bag from it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// The item list: `pocket` shown, `selected` the list index, `item` under the cursor
    /// (`None` on CLOSE BAG).
    List {
        pocket: Pocket,
        selected: u16,
        item: Option<ItemId>,
    },
    /// The context menu ([`BagMenu::actions`]).
    Context { cursor: u8 },
    /// How many (toss, sell).
    Quantity { value: u16, max: u16 },
    /// A message printing or waiting for A.
    Message,
    /// A yes/no question (`cursor` 0 yes).
    YesNo { cursor: u8 },
    /// A child screen (the party menu) has the screen.
    Child,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Question {
    Toss,
    Sell,
    Teach,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Next {
    List,
    TmContains,
    Ask(Question),
    Quantity,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    List,
    Context(MenuCursor),
    Quantity(Quantity),
    Msg(Next),
    Ask { q: Question, cursor: u8 },
    Child,
}

struct Render {
    pocket: Panel,
    list: Panel,
    desc: Panel,
    money: Option<Panel>,
    popup: Option<Panel>,
    popup_base: u16,
    item_icon: Load,
    bag: Load,
    bg: Bg,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BagMenu {
    pub mode: BagMode,
    /// Index into [`POCKETS`].
    pocket: u8,
    lists: [ListMenu; 5],
    st: St,
    /// The item the context menu, quantity or question is about.
    item: ItemId,
    /// The quantity chosen (toss, sell).
    qty: u16,
    actions: Vec<Action>,
    msg: Option<Box<Message>>,
    /// The pockets were compacted (once, on the first frame).
    opened: bool,
    gfx: Gfx<Render>,
}

impl BagMenu {
    pub fn new(mode: BagMode) -> Self {
        let pocket = match mode {
            BagMode::ChooseBerry => 3,
            _ => 0,
        };
        BagMenu {
            mode,
            pocket,
            lists: [ListMenu::new(1, MAX_SHOWN, Paging::None); 5],
            st: St::List,
            item: ItemId(0),
            qty: 0,
            actions: Vec::new(),
            msg: None,
            opened: false,
            gfx: Gfx::none(),
        }
    }

    /// The pocket shown.
    pub fn pocket(&self) -> Pocket {
        POCKETS[self.pocket as usize % 5]
    }

    /// The context menu's actions (while [`Prompt::Context`]).
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// What the bag waits for. `item` needs the bag: see [`BagMenu::prompt_in`].
    pub fn prompt_in(&self, bag: &crate::bag::Bag) -> Prompt {
        match self.st {
            St::List => {
                let l = &self.lists[self.pocket as usize];
                let items = entries(bag, self.pocket());
                Prompt::List {
                    pocket: self.pocket(),
                    selected: l.selected(),
                    item: items.get(l.selected() as usize).map(|s| s.item),
                }
            }
            St::Context(c) => Prompt::Context { cursor: c.pos },
            St::Quantity(q) => Prompt::Quantity {
                value: q.value,
                max: q.max,
            },
            St::Msg(_) => Prompt::Message,
            St::Ask { cursor, .. } => Prompt::YesNo { cursor },
            St::Child => Prompt::Child,
        }
    }
}

/// The non-empty slots of `pocket`, in order (the list's items).
fn entries(bag: &crate::bag::Bag, pocket: Pocket) -> Vec<Slot> {
    (bag.pocket(pocket).iter())
        .filter(|s| s.qty > 0)
        .copied()
        .collect()
}

fn item_name<G: GameData>(game: &G, item: ItemId) -> &'static [u8] {
    game.item(item).map_or(&[EOS][..], |i| i.name)
}

fn pocket_label(p: Pocket) -> UiText {
    match p {
        Pocket::Items => UiText::PocketItems,
        Pocket::KeyItems => UiText::PocketKeyItems,
        Pocket::PokeBalls => UiText::PocketPokeBalls,
        Pocket::TmHm => UiText::PocketTmHm,
        Pocket::Berries => UiText::PocketBerries,
    }
}

fn action_label<G: GameData>(game: &G, a: Action) -> &'static [u8] {
    game.ui_text(match a {
        Action::Use | Action::BattleUse => UiText::BagUse,
        Action::Give => UiText::BagGive,
        Action::Toss => UiText::BagToss,
        Action::Register => UiText::BagRegister,
        Action::Deselect => UiText::BagDeselect,
        // Gen3 `gMenuText_CheckTag` is not extracted: the cancel slot's text stands in.
        Action::CheckTag | Action::Cancel => UiText::BagCancel,
    })
}

fn vars(v1: &[u8], v2: &[u8]) -> StrVars {
    let mut v = StrVars::new();
    v.set(0, v1);
    v.set(1, v2);
    v
}

fn number(n: u32, digits: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(8);
    push_number(&mut v, n, digits, false);
    v.push(EOS);
    v
}

/// The message canvas' first tile (after the pocket name, the list and the description).
const fn msg_tiles_base() -> u16 {
    super::kit::window::FULL_BASE
        + POCKET_RECT.2 as u16 * POCKET_RECT.3 as u16
        + LIST_RECT.2 as u16 * LIST_RECT.3 as u16
        + DESC_RECT.2 as u16 * DESC_RECT.3 as u16
}

impl BagMenu {
    fn list(&self) -> &ListMenu {
        &self.lists[self.pocket as usize]
    }

    /// Gen3 `UpdatePocketListPosition` after the pocket changed (an item used up or tossed):
    /// the list's length again, the cursor kept inside it.
    fn refit<G: GameData>(&mut self, cx: &ScreenCx<'_, G>) {
        for (i, &p) in POCKETS.iter().enumerate() {
            let total = entries(cx.bag, p).len() as u16 + 1;
            let l = &mut self.lists[i];
            if l.total == total {
                continue;
            }
            let selected = l.selected().min(total - 1);
            let mut n = ListMenu::new(total, MAX_SHOWN, Paging::None);
            // Gen3 keeps the scroll unless the window would show past the end.
            n.scroll = l.scroll.min(total.saturating_sub(n.max_showed));
            n.row = selected - n.scroll;
            *l = n;
        }
    }

    fn layout<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Render {
        let game = cx.game;
        let mut tiles = TileAlloc::full();
        let (l, t, w, h) = POCKET_RECT;
        let pocket = tiles.panel(game, l, t, w, h, true);
        let (l, t, w, h) = LIST_RECT;
        let list = tiles.panel(game, l, t, w, h, true);
        let (l, t, w, h) = DESC_RECT;
        let desc = tiles.panel(game, l, t, w, h, false);
        let (_, _, w, h) = MSG_RECT;
        let msg_base = tiles.take(w, h);
        debug_assert_eq!(msg_base, msg_tiles_base());
        let popup_base = tiles.take(POPUP_TILES.0, POPUP_TILES.1);
        let money =
            (self.mode == BagMode::Sell).then(|| money_box(game, &mut tiles, *cx.money, 0, 0));
        Render {
            pocket,
            list,
            desc,
            money,
            popup: None,
            popup_base,
            item_icon: Load::new(),
            bag: Load::new(),
            bg: Bg::new(),
        }
    }

    fn draw_pocket<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        r.pocket.clear(cx.game);
        r.pocket
            .text(cx.game, cx.game.ui_text(pocket_label(self.pocket())), 0, 1);
    }

    /// The list rows and the cursor (Gen3 `BagMenu_ItemPrintCallback`).
    fn draw_list<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        let p = &mut r.list;
        p.clear(game);
        let items = entries(cx.bag, self.pocket());
        let l = self.list();
        for (row, i) in l.shown().enumerate() {
            let y = row as i16 * ROW_H + 1;
            match items.get(i as usize) {
                Some(s) => {
                    let mut name = Vec::with_capacity(24);
                    push(&mut name, item_name(game, s.item));
                    let mv = (self.pocket() == Pocket::TmHm)
                        .then(|| game.tm_move(s.item))
                        .flatten();
                    if let Some(m) = mv {
                        name.push(0x00);
                        push(
                            &mut name,
                            game.battle()
                                .moves
                                .get(m.0 as usize)
                                .map_or(&[EOS][..], |i| i.name),
                        );
                    }
                    name.push(EOS);
                    p.text(game, &name, CURSOR_W, y);
                    let hm = mv.is_some_and(|m| game.is_hm_move(m));
                    if self.pocket() != Pocket::KeyItems && !hm && mv.is_none() {
                        let digits = if self.pocket() == Pocket::Berries {
                            3
                        } else {
                            2
                        };
                        let mut q = Vec::with_capacity(6);
                        q.push(super::kit::widgets::CHAR_TIMES);
                        push_number(&mut q, s.qty as u32, digits, true);
                        q.push(EOS);
                        p.text(game, &q, 96, y);
                    }
                    if *cx.registered_item == Some(s.item) && self.pocket() == Pocket::KeyItems {
                        // Gen3 draws the SELECT icon (`BagMenu_PrintCursor`'s neighbour); a
                        // cursor glyph marks it here.
                        p.text(game, &[CHAR_CURSOR, EOS], 104, y);
                    }
                }
                None => p.text(game, cx.game.ui_text(UiText::BagCloseBag), CURSOR_W, y),
            }
        }
        self.draw_list_cursor(cx, r);
    }

    fn draw_list_cursor<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        let c = colors(game);
        let p = &mut r.list;
        let l = self.list();
        for row in 0..l.max_showed {
            let y = row as i16 * ROW_H + 1;
            p.canvas.fill_rect(0, y, CURSOR_W, ROW_H - 1, c.bg);
            if row == l.row && matches!(self.st, St::List) {
                p.text(game, &[CHAR_CURSOR, EOS], 0, y);
            }
        }
    }

    /// Gen3 `PrintItemDescription`.
    fn draw_desc<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        r.desc.clear(game);
        let items = entries(cx.bag, self.pocket());
        if let Some(s) = items.get(self.list().selected() as usize)
            && let Some(d) = game.item_description(s.item)
        {
            r.desc.text(game, d, 0, 1);
        }
    }

    fn compose<G: GameData>(&self, cx: &mut ScreenCx<'_, G>, r: &Render) {
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        map.put(&r.pocket);
        map.put(&r.list);
        map.put(&r.desc);
        if let Some(m) = &r.money {
            map.put(m);
        }
        if let Some(m) = &self.msg
            && m.shown
        {
            map.put_window(m.window(), true);
        }
        if let Some(p) = &r.popup {
            map.put(p);
        }
        map.flush(cx.f);
    }

    fn popup_panel<G: GameData>(&self, cx: &ScreenCx<'_, G>, base: u16) -> Option<Panel> {
        let game = cx.game;
        let mut tiles = TileAlloc::at(base);
        match self.st {
            St::Context(c) => {
                let n = self.actions.len() as u8;
                let mut p = tiles.panel(
                    game,
                    CONTEXT_X,
                    CONTEXT_BOTTOM - n * 2,
                    CONTEXT_W,
                    n * 2,
                    true,
                );
                let labels: Vec<&[u8]> = self
                    .actions
                    .iter()
                    .map(|a| action_label(game, *a))
                    .collect();
                draw_menu(game, &mut p, &labels, c.pos);
                Some(p)
            }
            St::Quantity(q) => {
                let sell = self.mode == BagMode::Sell;
                let (l, t, w, h) = if sell { QTY_WIDE_RECT } else { QTY_RECT };
                let mut p = tiles.panel(game, l, t, w, h, true);
                p.text(game, &q.text(2), 0, 1);
                if sell {
                    let price = sell_price(game, self.item) * q.value as u32;
                    let mut s = Vec::with_capacity(10);
                    push_number(&mut s, price, 6, true);
                    s.push(CHAR_CURRENCY);
                    s.push(EOS);
                    p.text(game, &s, 32, 1);
                }
                Some(p)
            }
            St::Ask { cursor, .. } => Some(YesNo::new(game, &mut tiles, cursor == 1).panel),
            _ => None,
        }
    }

    fn redraw_all<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let mut r = self.layout(cx);
        self.draw_pocket(cx, &mut r);
        self.draw_list(cx, &mut r);
        self.draw_desc(cx, &mut r);
        r.popup = self.popup_panel(cx, r.popup_base);
        if let Some(m) = &mut self.msg {
            m.touch();
        }
        self.compose(cx, &r);
        self.gfx.0 = Some(Box::new(r));
    }

    /// The popup and the map again after a state change.
    fn relayout<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(mut r) = self.gfx.0.take() else {
            return;
        };
        r.popup = self.popup_panel(cx, r.popup_base);
        self.draw_list_cursor(cx, &mut r);
        self.compose(cx, &r);
        self.gfx.0 = Some(r);
    }

    /// The list (full, or the cursor only) and the description again.
    fn redraw_list<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, full: bool) {
        let Some(mut r) = self.gfx.0.take() else {
            return;
        };
        if full {
            self.draw_list(cx, &mut r);
        } else {
            self.draw_list_cursor(cx, &mut r);
        }
        self.draw_desc(cx, &mut r);
        self.gfx.0 = Some(r);
    }

    fn show_msg<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        text: UiText,
        v: Option<&StrVars>,
        next: Next,
    ) {
        let (l, t, w, h) = MSG_RECT;
        let m = self.msg.get_or_insert_with(|| {
            Box::new(Message::new(
                Window::new(l, t, w, h, msg_tiles_base()),
                cx.options.text_speed,
            ))
        });
        // A message before a question or a quantity stays up without a press (Gen3
        // `DisplayItemMessage` with a callback that opens the yes/no or the quantity box).
        let wait = matches!(next, Next::List | Next::TmContains);
        m.start(cx.game, cx.game.ui_text(text), &cx.player_name[..], v, wait);
        self.st = St::Msg(next);
        self.relayout(cx);
    }

    fn back_to_list<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.st = St::List;
        self.actions.clear();
        if let Some(m) = &mut self.msg {
            m.hide();
        }
        self.refit(cx);
        self.redraw_list(cx, true);
        self.relayout(cx);
    }

    /// The context menu of `item` (Gen3 `OpenContextMenu`).
    fn context_for<G: GameData>(&self, cx: &ScreenCx<'_, G>, item: ItemId) -> Vec<Action> {
        if self.mode == BagMode::Battle {
            return if item_use::battle_usable(cx.game, item) {
                alloc::vec![Action::BattleUse, Action::Cancel]
            } else {
                alloc::vec![Action::Cancel]
            };
        }
        match self.pocket() {
            Pocket::Items => alloc::vec![Action::Use, Action::Give, Action::Toss, Action::Cancel],
            Pocket::KeyItems => {
                let reg = if *cx.registered_item == Some(item) {
                    Action::Deselect
                } else {
                    Action::Register
                };
                alloc::vec![Action::Use, reg, Action::Cancel]
            }
            Pocket::PokeBalls => alloc::vec![Action::Give, Action::Toss, Action::Cancel],
            Pocket::TmHm => alloc::vec![Action::Use, Action::Give, Action::Cancel],
            Pocket::Berries => alloc::vec![
                Action::CheckTag,
                Action::Use,
                Action::Give,
                Action::Toss,
                Action::Cancel
            ],
        }
    }

    /// A on an item of the list.
    fn choose<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, item: ItemId) -> ScreenOut {
        self.item = item;
        match self.mode {
            BagMode::ChooseBerry => ScreenOut::Done(ScreenResult::Value(item.0)),
            BagMode::Sell => {
                // Gen3 `Task_ItemContext_Sell`.
                let name = item_name(cx.game, item);
                if sell_price(cx.game, item) == 0 {
                    let v = vars(&[EOS], name);
                    self.show_msg(cx, UiText::BagCantBuy, Some(&v), Next::List);
                } else {
                    let v = vars(&[EOS], name);
                    self.show_msg(cx, UiText::BagSellHowMany, Some(&v), Next::Quantity);
                }
                ScreenOut::Running
            }
            BagMode::Field | BagMode::Battle => {
                self.actions = self.context_for(cx, item);
                self.st = St::Context(MenuCursor::new(self.actions.len() as u8, 0));
                self.relayout(cx);
                ScreenOut::Running
            }
        }
    }

    fn action<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, a: Action) -> ScreenOut {
        let game = cx.game;
        let item = self.item;
        let name = item_name(game, item);
        match a {
            Action::Cancel => {
                self.back_to_list(cx);
                ScreenOut::Running
            }
            Action::BattleUse => ScreenOut::Done(ScreenResult::Value(item.0)),
            Action::Register | Action::Deselect => {
                // Gen3 `ItemMenu_Register`.
                *cx.registered_item = (a == Action::Register).then_some(item);
                self.back_to_list(cx);
                ScreenOut::Running
            }
            Action::CheckTag => {
                cx.report(Report::Unsupported("berry tag"));
                self.back_to_list(cx);
                ScreenOut::Running
            }
            Action::Give => {
                // Gen3 `ItemMenu_Give`.
                if game.item(item).is_some_and(|i| i.key) {
                    let v = vars(name, &[EOS]);
                    self.show_msg(cx, UiText::BagCantBeHeld, Some(&v), Next::List);
                    ScreenOut::Running
                } else if cx.party.is_empty() {
                    self.show_msg(cx, UiText::BagCantUseHere, None, Next::List);
                    ScreenOut::Running
                } else {
                    self.st = St::Child;
                    self.gfx.0 = None;
                    ScreenOut::Push(ScreenKind::Party(PartyMode::GiveItem(item)))
                }
            }
            Action::Toss => {
                let have = cx.bag.count(item).min(u16::MAX as u32) as u16;
                self.qty = 1;
                if have <= 1 {
                    self.ask_toss(cx);
                } else {
                    let v = vars(name, &[EOS]);
                    self.show_msg(cx, UiText::BagTossHowMany, Some(&v), Next::Quantity);
                }
                ScreenOut::Running
            }
            Action::Use => self.use_item(cx),
        }
    }

    /// USE in the field (Gen3 `ItemMenu_UseOutOfBattle` → the item's field function).
    fn use_item<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) -> ScreenOut {
        let game = cx.game;
        let item = self.item;
        match item_use::field_use(game, item) {
            FieldUse::Party => {
                if cx.party.is_empty() {
                    // Gen3 `PrintThereIsNoPokemon` (`gText_NoPokemon`, not extracted).
                    self.show_msg(cx, UiText::BagCantUseHere, None, Next::List);
                } else if let Some(m) = game.tm_move(item) {
                    let text = if game.is_hm_move(m) {
                        UiText::BagBootedUpHm
                    } else {
                        UiText::BagBootedUpTm
                    };
                    self.show_msg(cx, text, None, Next::TmContains);
                } else {
                    self.st = St::Child;
                    self.gfx.0 = None;
                    return ScreenOut::Push(ScreenKind::Party(PartyMode::UseItem(item)));
                }
                ScreenOut::Running
            }
            FieldUse::Repel(steps) => {
                // Gen3 `ItemUseOutOfBattle_Repel` → `Task_UseRepel`.
                match game.special_ids().var_repel_steps {
                    Some(var) if cx.vars.var(var) == 0 => {
                        cx.vars.set_var(var, steps);
                        if let Some(p) = game.item(item).map(|i| i.pocket) {
                            cx.bag.remove(p, item, 1);
                        }
                        let v = vars(&[EOS], item_name(game, item));
                        self.show_msg(cx, UiText::ItemUsed, Some(&v), Next::List);
                    }
                    Some(_) => self.show_msg(cx, UiText::ItemRepelLingers, None, Next::List),
                    None => {
                        cx.report(Report::Unsupported("repel"));
                        self.back_to_list(cx);
                    }
                }
                ScreenOut::Running
            }
            FieldUse::OnField => {
                let underground = game.map(cx.map).map_type == crate::data::map_type::UNDERGROUND;
                if underground {
                    ScreenOut::UseOnField(item)
                } else {
                    self.show_msg(cx, UiText::BagCantUseHere, None, Next::List);
                    ScreenOut::Running
                }
            }
            FieldUse::Script(id) => ScreenOut::RunScript(id),
            FieldUse::Unsupported(what) => {
                cx.report(Report::Unsupported(what));
                self.back_to_list(cx);
                ScreenOut::Running
            }
            FieldUse::CannotUse => {
                // Gen3 `DisplayDadsAdviceCannotUseItemMessage`.
                self.show_msg(cx, UiText::BagCantUseHere, None, Next::List);
                ScreenOut::Running
            }
        }
    }

    fn ask_toss<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let v = vars(item_name(cx.game, self.item), &number(self.qty as u32, 2));
        self.show_msg(
            cx,
            UiText::BagConfirmToss,
            Some(&v),
            Next::Ask(Question::Toss),
        );
    }

    fn after_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, next: Next) -> ScreenOut {
        match next {
            Next::List => self.back_to_list(cx),
            Next::TmContains => {
                let m = cx.game.tm_move(self.item);
                let name = m.map_or(&[EOS][..], |m| {
                    cx.game
                        .battle()
                        .moves
                        .get(m.0 as usize)
                        .map_or(&[EOS][..], |i| i.name)
                });
                let v = vars(name, &[EOS]);
                self.show_msg(
                    cx,
                    UiText::BagTmContains,
                    Some(&v),
                    Next::Ask(Question::Teach),
                );
            }
            Next::Ask(q) => {
                self.st = St::Ask { q, cursor: 0 };
                self.relayout(cx);
            }
            Next::Quantity => {
                let have = cx.bag.count(self.item).min(99);
                self.st = St::Quantity(Quantity::new(have as u16));
                self.relayout(cx);
            }
        }
        ScreenOut::Running
    }

    fn answer<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        q: Question,
        yes: bool,
    ) -> ScreenOut {
        let game = cx.game;
        let item = self.item;
        let name = item_name(game, item);
        if !yes {
            self.back_to_list(cx);
            return ScreenOut::Running;
        }
        match q {
            Question::Teach => {
                if let Some(m) = &mut self.msg {
                    m.hide();
                }
                self.st = St::Child;
                self.gfx.0 = None;
                ScreenOut::Push(ScreenKind::Party(PartyMode::UseItem(item)))
            }
            Question::Toss => {
                // Gen3 `ConfirmToss` → `Task_RemoveItemFromBag`.
                if let Some(p) = game.item(item).map(|i| i.pocket) {
                    cx.bag.remove(p, item, self.qty);
                }
                let v = vars(name, &number(self.qty as u32, 2));
                self.show_msg(cx, UiText::BagThrewAway, Some(&v), Next::List);
                ScreenOut::Running
            }
            Question::Sell => {
                // Gen3 `SellItem`: the money (capped), the items, the message.
                let price = sell_price(game, item) * self.qty as u32;
                if let Some(p) = game.item(item).map(|i| i.pocket) {
                    cx.bag.remove(p, item, self.qty);
                }
                let max = game.special_ids().max_money;
                *cx.money = cx.money.saturating_add(price).min(max);
                let v = vars(&number(price, 6), name);
                self.show_msg(cx, UiText::BagSold, Some(&v), Next::List);
                if let Some(r) = self.gfx.0.as_deref_mut()
                    && r.money.is_some()
                {
                    let mut tiles =
                        TileAlloc::at(r.money.as_ref().map_or(0, |p| p.window().base()));
                    r.money = Some(money_box(game, &mut tiles, *cx.money, 0, 0));
                }
                ScreenOut::Running
            }
        }
    }

    /// The quantity chosen (A on the quantity box).
    fn quantity_chosen<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, n: u16) {
        self.qty = n;
        match self.mode {
            BagMode::Sell => {
                let price = sell_price(cx.game, self.item) * n as u32;
                let v = vars(&number(price, 6), &[EOS]);
                self.show_msg(
                    cx,
                    UiText::BagSellOffer,
                    Some(&v),
                    Next::Ask(Question::Sell),
                );
            }
            _ => self.ask_toss(cx),
        }
    }

    /// A child screen (the party menu) closed: back to the list.
    fn child_closed<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.st = St::List;
        self.actions.clear();
        if let Some(m) = &mut self.msg {
            m.hide();
        }
        self.refit(cx);
    }
}

/// The bag sprite for the open pocket and the selected item's icon.
fn draw_pictures<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    pocket: u8,
    selected: u16,
    r: &mut Render,
    budget: &mut usize,
) {
    let game = cx.game;
    let pocket_i = pocket as usize % 5;
    if let Some(pic) = game.ui_gfx(UiGfx::BagSprite(pocket_i as u8)) {
        r.bag.step(
            cx.f,
            budget,
            &pictures::bytes(
                pic.tiles,
                pic.palette,
                layout::menu::OBJ_BAG,
                layout::menu::OBJ_PAL_BAG,
                36,
                34,
                64,
                64,
                u32::from(pocket_i as u8) + 1,
            ),
        );
    }
    let items = entries(cx.bag, POCKETS[pocket_i]);
    if let Some(slot) = items.get(selected as usize)
        && let Some(icon) = game.item_icon(slot.item)
    {
        r.item_icon.step(
            cx.f,
            budget,
            &pictures::icon(
                icon,
                layout::menu::OBJ_ITEM_ICON,
                layout::menu::OBJ_PAL_ITEM_ICON,
                8,
                72,
                u32::from(slot.item.0) + 1,
            ),
        );
    }
    r.bg.step(
        cx.f,
        budget,
        game.ui_gfx(UiGfx::BagBg),
        layout::menu::BACKGROUND,
        8,
    );
}

/// Gen3 `ItemId_GetPrice(item) / 2`.
fn sell_price<G: GameData>(game: &G, item: ItemId) -> u32 {
    game.item(item).map_or(0, |i| i.price as u32 / 2)
}

impl Screen for BagMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if !self.opened {
            self.opened = true;
            for p in POCKETS {
                cx.bag
                    .compact(p, matches!(p, Pocket::TmHm | Pocket::Berries));
            }
            self.refit(cx);
        }
        if cx.child.take().is_some() {
            self.child_closed(cx);
        }
        if self.gfx.0.is_none() {
            self.redraw_all(cx);
        }
        let pressed = input.pressed;
        let out = match self.st {
            St::List => {
                let before = *self.list();
                let berry_only = self.mode == BagMode::ChooseBerry;
                if pressed.contains(Buttons::LEFT) && !berry_only && self.pocket > 0 {
                    self.pocket -= 1;
                    self.redraw_all(cx);
                    ScreenOut::Running
                } else if pressed.contains(Buttons::RIGHT) && !berry_only && self.pocket < 4 {
                    self.pocket += 1;
                    self.redraw_all(cx);
                    ScreenOut::Running
                } else {
                    let r = self.lists[self.pocket as usize].input(pressed);
                    match r {
                        MenuInput::Moved => {
                            let full = self.list().scroll != before.scroll;
                            self.redraw_list(cx, full);
                            ScreenOut::Running
                        }
                        MenuInput::Chosen(i) => {
                            match entries(cx.bag, self.pocket()).get(i as usize) {
                                Some(s) => self.choose(cx, s.item),
                                None => ScreenOut::Done(ScreenResult::Cancel),
                            }
                        }
                        MenuInput::Cancel => ScreenOut::Done(ScreenResult::Cancel),
                        MenuInput::None => ScreenOut::Running,
                    }
                }
            }
            St::Context(mut c) => {
                let r = {
                    let render = self.gfx.0.as_deref_mut().expect("drawn");
                    match render.popup.as_mut() {
                        Some(p) => menu_step(cx.game, p, &mut c, pressed),
                        None => c.input(pressed),
                    }
                };
                self.st = St::Context(c);
                match r {
                    MenuInput::Chosen(i) => {
                        let a = self
                            .actions
                            .get(i as usize)
                            .copied()
                            .unwrap_or(Action::Cancel);
                        self.action(cx, a)
                    }
                    MenuInput::Cancel => {
                        self.back_to_list(cx);
                        ScreenOut::Running
                    }
                    _ => ScreenOut::Running,
                }
            }
            St::Quantity(mut q) => {
                if pressed.contains(Buttons::A) {
                    self.quantity_chosen(cx, q.value);
                } else if pressed.contains(Buttons::B) {
                    self.back_to_list(cx);
                } else if q.input(pressed) {
                    self.st = St::Quantity(q);
                    self.relayout(cx);
                }
                ScreenOut::Running
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
                self.st = St::Ask {
                    q,
                    cursor: yes_no.cursor,
                };
                match r {
                    Some(yes) => self.answer(cx, q, yes),
                    None => ScreenOut::Running,
                }
            }
            St::Child => ScreenOut::Running,
        };
        // The message and the popup first: they answer the last press.
        let mut budget = super::kit::frame_budget(cx.f);
        let pocket = self.pocket;
        let selected = self.list().selected();
        if let Some(m) = &mut self.msg {
            m.flush_within(cx.f, &mut budget);
        }
        if let Some(r) = self.gfx.0.as_deref_mut() {
            if let Some(p) = &mut r.popup {
                p.flush_within(cx.f, &mut budget);
            }
            r.list.flush_within(cx.f, &mut budget);
            r.desc.flush_within(cx.f, &mut budget);
            r.pocket.flush_within(cx.f, &mut budget);
            if let Some(p) = &mut r.money {
                p.flush_within(cx.f, &mut budget);
            }
            draw_pictures(cx, pocket, selected, r, &mut budget);
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.mode as u8, self.pocket, self.opened as u8]);
        for l in &self.lists {
            l.hash(h);
        }
        let (tag, a, b) = match self.st {
            St::List => (0, 0, 0),
            St::Context(c) => (1, c.pos, c.count),
            St::Quantity(q) => {
                q.hash(h);
                (2, 0, 0)
            }
            St::Msg(n) => (
                3,
                match n {
                    Next::List => 0,
                    Next::TmContains => 1,
                    Next::Ask(q) => 2 + q as u8,
                    Next::Quantity => 5,
                },
                0,
            ),
            St::Ask { q, cursor } => (4, q as u8, cursor),
            St::Child => (5, 0, 0),
        };
        h.write(&[tag, a, b, self.actions.len() as u8]);
        h.write_u16(self.item.0);
        h.write_u16(self.qty);
        if let Some(m) = &self.msg {
            m.hash(h);
        }
    }
}
