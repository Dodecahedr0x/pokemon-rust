//! The Poké Mart (Gen3 `shop.c`, `CreatePokemartMenu`; Milestone 9 Task M, as built in the
//! parity drive's Wave F).
//!
//! - **The shop menu** (`CreateShopMenu(MART_TYPE_NORMAL)`): BUY / SELL / QUIT at (2, 1),
//!   `Menu_ProcessInputNoWrap` (no wrap); B quits (`Task_HandleShopMenuQuit`), and the screen
//!   answers `Done(Cancel)` (the script goes on: `ScriptContext_Enable`).
//! - **BUY** (`CB2_InitBuyMenu`): the money box at (1, 1), the item list at (14, 2), 15 x 16
//!   (`MAX_ITEMS_SHOWN` 8 rows, the price right-aligned on each row, CANCEL last), the
//!   description at (0, 13), 14 x 6 (`gText_QuitShopping` on CANCEL). A on an item
//!   (`Task_BuyMenu`): "You don't have enough money." when the money is short of one, else
//!   "{item}? Certainly. How many would you like?" and the quantity: "IN BAG" at (1, 11) and
//!   the count with the total at (18, 11) (`Task_BuyHowManyDialogueInit`: from 1, at most the
//!   money's worth, capped at `MAX_BAG_ITEM_CAPACITY` 99). A: "{item}, and you want {n}. That
//!   will be ¥{total}. OK?" and yes/no at (21, 9); yes (`BuyMenuTryMakePurchase`): the items go
//!   in the bag (`AddBagItem`) or "You have no more room for this.", then "Here you go! Thank
//!   you very much." and, once it is printed, the money is taken (`BuyMenuSubtractMoney`);
//!   after A or B (`Task_ReturnToItemListAfterItemPurchase`), 10 or more of the game's bonus
//!   item (Gen3: Poké Balls) add one of its gift (a Premier Ball) with "I'll throw in a
//!   PREMIER BALL, too." when it fits. B or CANCEL leaves the list: "Is there anything else I
//!   can help you with?" and the shop menu again (`MapPostLoadHook_ReturnToShopMenu`).
//! - **SELL** (`CB2_GoToSellMenu`) opens the bag in sell mode ([`BagMode::Sell`], Task B);
//!   back from it, "anything else" and the shop menu.
//!
//! Deviations: no scroll arrows and no shop map view (Gen3 redraws the field behind the BUY
//! windows, `BuyMenuDrawMapGraphics`). The selected item's icon (centre (24, 88)) and
//! `GameData::ui_gfx(ShopBg)` are drawn when the game has them. The whole mart is one
//! full screen, so the shop menu shows over the window background rather than the field, and
//! the fades between the shop menu and BUY (`FadeScreen`) are not played; the shop menu's
//! width is its template's 9 tiles (Gen3 fits it to the widest label); no purchase history
//! (`RecordItemPurchase`, the Smart Shopper TV show) nor `GAME_STAT_SHOPPED`; no Slateport
//! PokéNews half price (`IsPokeNewsActive`).

use super::kit::Gfx;
use super::kit::list::{ListMenu, MenuInput, Paging};
use super::kit::pictures::{self, Bg, Load};
use super::kit::widgets::{
    CHAR_CURRENCY, CHAR_CURSOR, CURSOR_W, EOS, Message, Quantity, ROW_H, YesNo, draw_cursor,
    draw_menu, money_box, push_number,
};
use super::kit::window::{MapBuf, Panel, TileAlloc, colors, frame_kind};
use super::{BagMode, Screen, ScreenCx, ScreenInput, ScreenKind, ScreenOut, ScreenResult};
use crate::data::{DecorId, DecorShop, GameData, ItemId, UiGfx, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::str_vars::StrVars;
use crate::text::Window;
use crate::video::layout;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// `sShopMenuWindowTemplates[WIN_BUY_SELL_QUIT]`.
pub const MENU_RECT: (u8, u8, u8, u8) = (2, 1, 9, 6);
/// `sShopBuyMenuWindowTemplates[WIN_ITEM_LIST]`.
pub const LIST_RECT: (u8, u8, u8, u8) = (14, 2, 15, 16);
/// `WIN_ITEM_DESCRIPTION`.
pub const DESC_RECT: (u8, u8, u8, u8) = (0, 13, 14, 6);
/// `WIN_QUANTITY_IN_BAG`.
pub const IN_BAG_RECT: (u8, u8, u8, u8) = (1, 11, 12, 2);
/// `WIN_QUANTITY_PRICE`.
pub const PRICE_RECT: (u8, u8, u8, u8) = (18, 11, 10, 2);
/// `WIN_MESSAGE`.
pub const MSG_RECT: (u8, u8, u8, u8) = (2, 15, 27, 4);
/// Gen3 `MAX_ITEMS_SHOWN`.
pub const MAX_SHOWN: u16 = 8;
/// Gen3 `MAX_BAG_ITEM_CAPACITY`: the most one purchase buys.
pub const MAX_QUANTITY: u16 = 99;
/// The shop menu's entries (`sShopMenuActions_BuySellQuit`).
pub const BUY: u8 = 0;
pub const SELL: u8 = 1;
pub const QUIT: u8 = 2;

/// What the mart waits for (tools drive the mart from it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// BUY / SELL / QUIT, `cursor` on [`BUY`], [`SELL`] or [`QUIT`].
    Menu { cursor: u8 },
    /// The BUY list: `selected` the row's index, `item` under the cursor (`None` on CANCEL).
    List { selected: u16, item: Option<ItemId> },
    /// How many.
    Quantity { value: u16, max: u16 },
    /// A message printing or waiting for A or B.
    Message,
    /// "That will be ¥X. OK?" (`cursor` 0 yes).
    YesNo { cursor: u8 },
    /// The bag (SELL) has the screen.
    Child,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Next {
    /// Back to the shop menu.
    Menu,
    /// Back to the BUY list.
    List,
    /// The quantity box.
    Quantity,
    /// The purchase's yes/no.
    Confirm,
    /// The purchase is printed: take the money, then wait for A or B.
    Paid,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Menu(u8),
    List,
    Quantity(Quantity),
    Msg(Next),
    Ask(u8),
    /// "Thank you" printed, the money taken: A or B goes on.
    Press,
    Child,
}

struct Render {
    /// The shop menu (in [`St::Menu`] and its messages), else the BUY windows.
    menu: Option<Panel>,
    money: Option<Panel>,
    list: Option<Panel>,
    desc: Option<Panel>,
    popup: Vec<Panel>,
    popup_base: u16,
    item_icon: Load,
    bg: Bg,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mart {
    pub items: &'static [ItemId],
    /// Set for `pokemartdecoration` / `pokemartdecoration2` (buy / quit, no quantity, no sell).
    shop: Option<DecorShop>,
    decors: &'static [DecorId],
    decor_id: DecorId,
    st: St,
    /// In the BUY windows (else the shop menu).
    buying: bool,
    list: ListMenu,
    /// The item chosen, how many, and what they cost.
    item: ItemId,
    qty: u16,
    cost: u32,
    msg: Option<Box<Message>>,
    gfx: Gfx<Render>,
}

/// Gen3 `ItemId_GetPrice`.
fn price<G: GameData>(game: &G, item: ItemId) -> u32 {
    game.item(item).map_or(0, |i| u32::from(i.price))
}

fn item_name<G: GameData>(game: &G, item: ItemId) -> &'static [u8] {
    game.item(item).map_or(&[EOS][..], |i| i.name)
}

fn decor_name<G: GameData>(game: &G, id: DecorId) -> &'static [u8] {
    game.decoration(id).map_or(&[EOS][..], |d| d.name)
}

fn decor_price<G: GameData>(game: &G, id: DecorId) -> u32 {
    game.decoration(id).map_or(0, |d| u32::from(d.price))
}

fn number(n: u32, digits: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(8);
    push_number(&mut v, n, digits, false);
    v.push(EOS);
    v
}

/// `¥` and the amount right-aligned on `digits` (Gen3 `PrintMoneyAmount`; the currency sign
/// after the amount as the kit's money box prints it).
fn money_text(n: u32, digits: u8) -> Vec<u8> {
    let mut s = Vec::with_capacity(10);
    push_number(&mut s, n, digits, true);
    s.push(CHAR_CURRENCY);
    s.push(EOS);
    s
}

/// The quantity a purchase may reach: the money's worth, at most [`MAX_QUANTITY`]
/// (`Task_BuyHowManyDialogueInit`; a free item: the cap).
pub fn max_quantity(money: u32, price: u32) -> u16 {
    money
        .checked_div(price)
        .unwrap_or(u32::MAX)
        .min(u32::from(MAX_QUANTITY)) as u16
}

impl Mart {
    pub fn new(items: &'static [ItemId]) -> Self {
        Mart {
            items,
            shop: None,
            decors: &[],
            decor_id: DecorId(0),
            st: St::Menu(0),
            buying: false,
            list: ListMenu::new(items.len() as u16 + 1, MAX_SHOWN, Paging::None),
            item: ItemId(0),
            qty: 0,
            cost: 0,
            msg: None,
            gfx: Gfx::none(),
        }
    }

    /// A decoration shop (`CreateDecorationShop1Menu` / `CreateDecorationShop2Menu`).
    pub fn decorations(shop: DecorShop, decors: &'static [DecorId]) -> Self {
        let mut m = Mart::new(&[]);
        m.shop = Some(shop);
        m.decors = decors;
        m.list = ListMenu::new(decors.len() as u16 + 1, MAX_SHOWN, Paging::None);
        m
    }

    /// What the mart waits for.
    pub fn prompt(&self) -> Prompt {
        match self.st {
            St::Menu(cursor) => Prompt::Menu { cursor },
            St::List => Prompt::List {
                selected: self.list.selected(),
                item: self.items.get(self.list.selected() as usize).copied(),
            },
            St::Quantity(q) => Prompt::Quantity {
                value: q.value,
                max: q.max,
            },
            St::Msg(_) | St::Press => Prompt::Message,
            St::Ask(cursor) => Prompt::YesNo { cursor },
            St::Child => Prompt::Child,
        }
    }

    fn layout<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Render {
        let game = cx.game;
        let mut tiles = TileAlloc::full();
        let mut r = Render {
            menu: None,
            money: None,
            list: None,
            desc: None,
            popup: Vec::new(),
            popup_base: 0,
            item_icon: Load::new(),
            bg: Bg::new(),
        };
        if self.buying {
            r.money = Some(money_box(game, &mut tiles, *cx.money, 0, 0));
            let (l, t, w, h) = LIST_RECT;
            r.list = Some(tiles.panel(game, l, t, w, h, false));
            let (l, t, w, h) = DESC_RECT;
            r.desc = Some(tiles.panel(game, l, t, w, h, false));
        } else {
            let (l, t, w, h) = if self.shop.is_some() {
                (2, 1, 9, 4)
            } else {
                MENU_RECT
            };
            r.menu = Some(tiles.panel(game, l, t, w, h, true));
        }
        r.popup_base = msg_base() + MSG_RECT.2 as u16 * MSG_RECT.3 as u16;
        r
    }

    fn draw_menu<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        if let Some(p) = &mut r.menu {
            let cursor = match self.st {
                St::Menu(c) => c,
                _ => u8::MAX,
            };
            if self.shop.is_some() {
                let labels = [
                    game.ui_text(UiText::ShopBuy),
                    game.ui_text(UiText::ShopQuit),
                ];
                draw_menu(game, p, &labels, cursor);
            } else {
                let labels = [
                    game.ui_text(UiText::ShopBuy),
                    game.ui_text(UiText::ShopSell),
                    game.ui_text(UiText::ShopQuit),
                ];
                draw_menu(game, p, &labels, cursor);
            }
        }
    }

    /// The rows with their prices (`BuyMenuPrintPriceInList`) and the cursor.
    fn draw_list<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        let Some(p) = &mut r.list else {
            return;
        };
        p.clear(game);
        for (row, i) in self.list.shown().enumerate() {
            let y = row as i16 * ROW_H + 1;
            if self.shop.is_some() {
                match self.decors.get(i as usize) {
                    Some(&id) => {
                        p.text(game, decor_name(game, id), CURSOR_W, y);
                        p.text(game, &money_text(decor_price(game, id), 5), 80, y);
                    }
                    None => p.text(game, game.ui_text(UiText::BagCancel), CURSOR_W, y),
                }
                continue;
            }
            match self.items.get(i as usize) {
                Some(&item) => {
                    p.text(game, item_name(game, item), CURSOR_W, y);
                    p.text(game, &money_text(price(game, item), 5), 80, y);
                }
                None => p.text(game, game.ui_text(UiText::BagCancel), CURSOR_W, y),
            }
        }
        self.draw_list_cursor(cx, r);
    }

    fn draw_list_cursor<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        let c = colors(game);
        let Some(p) = &mut r.list else {
            return;
        };
        for row in 0..self.list.max_showed {
            let y = row as i16 * ROW_H + 1;
            p.canvas.fill_rect(0, y, CURSOR_W, ROW_H - 1, c.bg);
            // Gen3 greys the cursor while an item is being bought (`COLORID_GRAY_CURSOR`).
            if row == self.list.row {
                p.text(game, &[CHAR_CURSOR, EOS], 0, y);
            }
        }
    }

    /// `BuyMenuPrintItemDescriptionAndShowItemIcon`.
    fn draw_desc<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        let Some(p) = &mut r.desc else {
            return;
        };
        p.clear(game);
        if self.shop.is_some() {
            if let Some(&id) = self.decors.get(self.list.selected() as usize)
                && let Some(d) = game.decoration(id)
            {
                p.text(game, d.description, 3, 1);
            }
            return;
        }
        if let Some(&item) = self.items.get(self.list.selected() as usize)
            && let Some(d) = game.item_description(item)
        {
            p.text(game, d, 3, 1);
        }
    }

    /// The quantity windows or the yes/no, as the state asks.
    fn popups<G: GameData>(&self, cx: &ScreenCx<'_, G>, base: u16) -> Vec<Panel> {
        let game = cx.game;
        let mut tiles = TileAlloc::at(base);
        match self.st {
            St::Quantity(q) => {
                // `Task_BuyHowManyDialogueInit` / `BuyMenuPrintItemQuantityAndPrice`.
                let (l, t, w, h) = IN_BAG_RECT;
                let mut bag = tiles.panel(game, l, t, w, h, true);
                let have = cx.bag.count(self.item).min(9999);
                let mut s = Vec::with_capacity(8);
                push_number(&mut s, have, 4, true);
                s.push(EOS);
                bag.text(game, &s, 0, 1);
                let (l, t, w, h) = PRICE_RECT;
                let mut p = tiles.panel(game, l, t, w, h, true);
                p.text(game, &q.text(2), 0, 1);
                let total = price(game, self.item) * u32::from(q.value);
                p.text(game, &money_text(total, 6), 38, 1);
                alloc::vec![bag, p]
            }
            St::Ask(cursor) => alloc::vec![YesNo::new(game, &mut tiles, cursor == 1).panel],
            _ => Vec::new(),
        }
    }

    fn compose<G: GameData>(&self, cx: &mut ScreenCx<'_, G>, r: &Render) {
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        for p in [&r.menu, &r.money, &r.list, &r.desc].into_iter().flatten() {
            map.put(p);
        }
        if let Some(m) = &self.msg
            && m.shown
        {
            map.put_window(m.window(), true);
        }
        for p in &r.popup {
            map.put(p);
        }
        map.flush(cx.f);
    }

    fn redraw_all<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let mut r = self.layout(cx);
        self.draw_menu(cx, &mut r);
        self.draw_list(cx, &mut r);
        self.draw_desc(cx, &mut r);
        r.popup = self.popups(cx, r.popup_base);
        if let Some(m) = &mut self.msg {
            m.touch();
        }
        self.compose(cx, &r);
        self.gfx.0 = Some(Box::new(r));
    }

    /// The popups, the menu's cursor and the map again after a state change.
    fn relayout<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(mut r) = self.gfx.0.take() else {
            return;
        };
        r.popup = self.popups(cx, r.popup_base);
        if let (Some(p), St::Menu(c)) = (&mut r.menu, self.st) {
            let rows = if self.shop.is_some() { 2 } else { 3 };
            draw_cursor(cx.game, p, rows, c);
        }
        self.compose(cx, &r);
        self.gfx.0 = Some(r);
    }

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

    /// The money box again (`PrintMoneyAmountInMoneyBox`).
    fn redraw_money<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        if let Some(r) = self.gfx.0.as_deref_mut()
            && let Some(old) = &r.money
        {
            let mut tiles = TileAlloc::at(old.window().base());
            r.money = Some(money_box(cx.game, &mut tiles, *cx.money, 0, 0));
        }
    }

    /// `BuyMenuDisplayMessage` / `DisplayItemMessageOnField`: `next` once printed.
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
                Window::new(l, t, w, h, msg_base()),
                cx.options.text_speed,
            ))
        });
        m.start(
            cx.game,
            cx.game.ui_text(text),
            &cx.player_name[..],
            v,
            false,
        );
        self.st = St::Msg(next);
        self.relayout(cx);
    }

    fn hide_msg(&mut self) {
        if let Some(m) = &mut self.msg {
            m.hide();
        }
    }

    /// `BuyMenuReturnToItemList`.
    fn back_to_list<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.hide_msg();
        self.st = St::List;
        self.redraw_list(cx, false);
        self.relayout(cx);
    }

    /// Back to the shop menu with "Is there anything else I can help you with?"
    /// (`Task_ReturnToShopMenu`).
    fn anything_else<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.buying = false;
        self.hide_msg();
        self.st = St::Msg(Next::Menu);
        self.redraw_all(cx);
        let text = if self.shop == Some(DecorShop::Shop2) {
            UiText::ShopDecor2AnythingElse
        } else {
            UiText::ShopAnythingElse
        };
        self.show_msg(cx, text, None, Next::Menu);
    }

    fn open_list<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.buying = true;
        let n = if self.shop.is_some() {
            self.decors.len()
        } else {
            self.items.len()
        };
        self.list = ListMenu::new(n as u16 + 1, MAX_SHOWN, Paging::None);
        self.st = St::List;
        self.redraw_all(cx);
    }

    /// A on the shop menu.
    fn menu_chosen<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, c: u8) -> ScreenOut {
        if self.shop.is_some() {
            return if c == BUY {
                self.open_list(cx);
                ScreenOut::Running
            } else {
                ScreenOut::Done(ScreenResult::Cancel)
            };
        }
        match c {
            BUY => {
                self.open_list(cx);
                ScreenOut::Running
            }
            SELL => {
                self.st = St::Child;
                self.gfx.0 = None;
                ScreenOut::Push(ScreenKind::Bag(BagMode::Sell))
            }
            _ => ScreenOut::Done(ScreenResult::Cancel),
        }
    }

    /// A on a decoration (`Task_BuyMenu`, one piece, no quantity box).
    fn decor_chosen<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, id: DecorId) {
        let game = cx.game;
        let info = game.decoration(id);
        self.decor_id = id;
        self.item = ItemId(0);
        self.qty = 1;
        self.cost = info.map_or(0, |d| u32::from(d.price));
        if *cx.money < self.cost {
            self.show_msg(cx, UiText::ShopNotEnoughMoney, None, Next::List);
            return;
        }
        let name = info.map_or(&[EOS][..], |d| d.name);
        cx.str_vars.set(0, name);
        let mut v = StrVars::new();
        v.set(0, name);
        v.set(1, &number(self.cost, 6));
        let text = if self.shop == Some(DecorShop::Shop2) {
            UiText::ShopDecor2Confirm
        } else {
            UiText::ShopDecorConfirm
        };
        self.show_msg(cx, text, Some(&v), Next::Confirm);
    }

    /// A on an item of the list (`Task_BuyMenu`).
    fn item_chosen<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, item: ItemId) {
        let game = cx.game;
        self.item = item;
        self.cost = price(game, item);
        if *cx.money < self.cost {
            self.show_msg(cx, UiText::ShopNotEnoughMoney, None, Next::List);
            return;
        }
        let mut v = StrVars::new();
        v.set(0, item_name(game, item));
        if let Some(m) = game.tm_move(item) {
            let name = game
                .battle()
                .moves
                .get(m.0 as usize)
                .map_or(&[EOS][..], |i| i.name);
            v.set(1, name);
        }
        self.show_msg(cx, UiText::ShopHowMany, Some(&v), Next::Quantity);
    }

    fn after_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, next: Next) {
        match next {
            Next::Menu => {
                self.hide_msg();
                self.st = St::Menu(0);
                self.relayout(cx);
            }
            Next::List => self.back_to_list(cx),
            Next::Quantity => {
                let max = max_quantity(*cx.money, self.cost);
                self.st = St::Quantity(Quantity::new(max));
                self.relayout(cx);
            }
            Next::Confirm => {
                self.st = St::Ask(0);
                self.relayout(cx);
            }
            Next::Paid => {
                // `BuyMenuSubtractMoney`.
                *cx.money = cx.money.saturating_sub(self.cost);
                self.redraw_money(cx);
                self.st = St::Press;
                self.relayout(cx);
            }
        }
    }

    /// A on the quantity (`Task_BuyHowManyDialogueHandleInput`).
    fn quantity_chosen<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, n: u16) {
        let game = cx.game;
        self.qty = n;
        self.cost = price(game, self.item) * u32::from(n);
        let mut v = StrVars::new();
        v.set(0, item_name(game, self.item));
        v.set(1, &number(u32::from(n), 2));
        v.set(2, &number(self.cost, 6));
        self.show_msg(cx, UiText::ShopIsThatOkay, Some(&v), Next::Confirm);
    }

    /// Yes to the purchase (`BuyMenuTryMakePurchase`).
    fn buy<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        if self.shop.is_some() {
            let added = cx
                .game
                .decoration(self.decor_id)
                .is_some_and(|d| cx.decor.add(self.decor_id, d.category));
            if added {
                let text = if self.shop == Some(DecorShop::Shop2) {
                    UiText::ShopDecor2ThankYou
                } else {
                    UiText::ShopDecorThankYou
                };
                self.show_msg(cx, text, None, Next::Paid);
            } else {
                let name = decor_name(cx.game, self.decor_id);
                let mut v = StrVars::new();
                v.set(0, name);
                self.show_msg(cx, UiText::ShopDecorFull, Some(&v), Next::List);
            }
            return;
        }
        let pocket = cx.game.item(self.item).map(|i| i.pocket);
        let added = pocket.is_some_and(|p| cx.bag.add(p, self.item, self.qty));
        if added {
            self.show_msg(cx, UiText::ShopThankYou, None, Next::Paid);
        } else {
            self.show_msg(cx, UiText::ShopNoRoom, None, Next::List);
        }
    }

    /// A or B after a purchase (`Task_ReturnToItemListAfterItemPurchase`): the bonus item.
    fn after_purchase<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        if self.shop.is_some() {
            self.back_to_list(cx);
            return;
        }
        if let Some((item, qty, gift)) = cx.game.mart_bonus()
            && self.item == item
            && self.qty >= qty
            && let Some(p) = cx.game.item(gift).map(|i| i.pocket)
            && cx.bag.add(p, gift, 1)
        {
            self.show_msg(cx, UiText::ShopPremierBonus, None, Next::List);
        } else {
            self.back_to_list(cx);
        }
    }
}

/// The selected item's icon (Gen3 `BuyMenuAddItemIcon`, centre (24, 88)) and the shop picture.
fn draw_pictures<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    item: Option<ItemId>,
    r: &mut Render,
    budget: &mut usize,
) {
    let game = cx.game;
    if let Some(item) = item
        && let Some(icon) = game.item_icon(item)
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
                u32::from(item.0) + 1,
            ),
        );
    }
    r.bg.step(
        cx.f,
        budget,
        game.ui_gfx(UiGfx::ShopBg),
        layout::menu::BACKGROUND,
        1,
    );
}

/// The message canvas' first tile (after the BUY windows, the larger layout).
const fn msg_base() -> u16 {
    super::kit::window::FULL_BASE
        + 10 * 2
        + LIST_RECT.2 as u16 * LIST_RECT.3 as u16
        + DESC_RECT.2 as u16 * DESC_RECT.3 as u16
}

impl Screen for Mart {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if cx.child.take().is_some() {
            // Back from SELL (`CB2_ExitSellMenu`).
            self.anything_else(cx);
        }
        if self.gfx.0.is_none() {
            self.redraw_all(cx);
        }
        let pressed = input.pressed;
        let out = match self.st {
            St::Menu(c) => {
                // `Menu_ProcessInputNoWrap`.
                if pressed.contains(Buttons::A) {
                    self.menu_chosen(cx, c)
                } else if pressed.contains(Buttons::B) {
                    ScreenOut::Done(ScreenResult::Cancel)
                } else {
                    let to = if pressed.contains(Buttons::UP) {
                        c.saturating_sub(1)
                    } else if pressed.contains(Buttons::DOWN) {
                        let max = if self.shop.is_some() { 1 } else { QUIT };
                        (c + 1).min(max)
                    } else {
                        c
                    };
                    if to != c {
                        self.st = St::Menu(to);
                        self.relayout(cx);
                    }
                    ScreenOut::Running
                }
            }
            St::List => {
                let before = self.list;
                match self.list.input(pressed) {
                    MenuInput::Moved => {
                        let full = self.list.scroll != before.scroll;
                        self.redraw_list(cx, full);
                    }
                    MenuInput::Chosen(i) => {
                        if self.shop.is_some() {
                            match self.decors.get(i as usize) {
                                Some(&id) => self.decor_chosen(cx, id),
                                None => self.anything_else(cx),
                            }
                        } else {
                            match self.items.get(i as usize) {
                                Some(&item) => self.item_chosen(cx, item),
                                None => self.anything_else(cx),
                            }
                        }
                    }
                    MenuInput::Cancel => self.anything_else(cx),
                    MenuInput::None => {}
                }
                ScreenOut::Running
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
                let done =
                    (self.msg.as_mut()).is_none_or(|m| m.update(cx.game, pressed, input.held));
                if done {
                    self.after_msg(cx, next);
                }
                ScreenOut::Running
            }
            St::Press => {
                if pressed.contains(Buttons::A) || pressed.contains(Buttons::B) {
                    self.after_purchase(cx);
                }
                ScreenOut::Running
            }
            St::Ask(cursor) => {
                let mut yes_no = YesNo {
                    panel: (self.gfx.0.as_deref_mut())
                        .and_then(|r| r.popup.pop())
                        .unwrap_or_else(|| {
                            YesNo::new(cx.game, &mut TileAlloc::full(), cursor == 1).panel
                        }),
                    cursor,
                };
                let r = yes_no.input(cx.game, pressed);
                if let Some(render) = self.gfx.0.as_deref_mut() {
                    render.popup.push(yes_no.panel);
                }
                self.st = St::Ask(yes_no.cursor);
                match r {
                    Some(true) => self.buy(cx),
                    Some(false) => self.back_to_list(cx),
                    None => {}
                }
                ScreenOut::Running
            }
            St::Child => ScreenOut::Running,
        };
        let mut budget = super::kit::frame_budget(cx.f);
        let shown = self
            .buying
            .then(|| self.items.get(self.list.selected() as usize).copied());
        if let Some(m) = &mut self.msg {
            m.flush_within(cx.f, &mut budget);
        }
        if let Some(r) = self.gfx.0.as_deref_mut() {
            for p in &mut r.popup {
                p.flush_within(cx.f, &mut budget);
            }
            for p in [&mut r.menu, &mut r.list, &mut r.desc, &mut r.money]
                .into_iter()
                .flatten()
            {
                p.flush_within(cx.f, &mut budget);
            }
            draw_pictures(cx, shown.flatten(), r, &mut budget);
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.items.len() as u16);
        self.list.hash(h);
        let (tag, a) = match self.st {
            St::Menu(c) => (0, c),
            St::List => (1, 0),
            St::Quantity(q) => {
                q.hash(h);
                (2, 0)
            }
            St::Msg(n) => (3, n as u8),
            St::Ask(c) => (4, c),
            St::Press => (5, 0),
            St::Child => (6, 0),
        };
        h.write(&[tag, a, self.buying as u8]);
        h.write_u16(self.item.0);
        h.write_u16(self.qty);
        h.write(&self.cost.to_le_bytes());
        if let Some(m) = &self.msg {
            m.hash(h);
        }
        if let Some(shop) = self.shop {
            h.write(&[0xD0, shop as u8]);
            h.write_u16(self.decor_id.0);
            h.write_u16(self.decors.len() as u16);
        }
    }
}
