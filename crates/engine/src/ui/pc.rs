//! The PC menus (Gen3 `player_pc.c`, `pokemon_storage_system.c`).
//!
//! - **Someone's PC** (`PcMode::Someone`, `PlayerPC`): item storage, Pokémon storage, turn off.
//! - **Bedroom PC** (`PcMode::Bedroom`): item storage and turn off.
//! - **Pokémon storage** (`PcMode::Storage`, `ShowPokemonStorageSystemPC`): the box list directly.
//!
//! Item storage: WITHDRAW (PC → bag), DEPOSIT (bag → PC) and MOVE (swap two stored items).
//! Pokémon storage: WITHDRAW (box → party, healed, Gen3 `BoxMonToMon`), DEPOSIT (party → the
//! first free slot of the current box) and MOVE (a party or box slot onto another, including
//! another box). Left/right change the current box. The last party Pokémon cannot be deposited
//! or moved onto an empty slot; a full party or box refuses the transfer.
//!
//! Deviations: lists rather than the box grid, no wallpapers or box names, no release, toss,
//! mail or decorations, and no yes/no confirm on a quantity (A commits).

use super::kit::Gfx;
use super::kit::list::{ListMenu, MenuInput, Paging};
use super::kit::widgets::{
    CHAR_SPACE, CHAR_TIMES, CURSOR_W, EOS, Message, ROW_H, draw_cursor, draw_menu, push,
    push_number,
};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{PcMode, Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::bag::Slot;
use crate::data::{GameData, ItemId, Pocket, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::mon::{PARTY_SIZE, Pokemon};
use crate::pc::{BOX_SLOTS, MAX_PC_ITEM_CAPACITY, PcStorage};
use crate::text::Window;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Item and Pokémon action cursors, and the action menu's CANCEL.
pub const WITHDRAW: u8 = 0;
pub const DEPOSIT: u8 = 1;
pub const MOVE: u8 = 2;
pub const CANCEL: u8 = 3;
/// Rows of a storage list (`ListMenu`, no paging: left/right change boxes).
pub const MAX_SHOWN: u16 = 6;

const MAIN_RECT: (u8, u8, u8, u8) = (1, 1, 14, 12);
const BOX_RECT: (u8, u8, u8, u8) = (17, 1, 12, 2);
const QTY_RECT: (u8, u8, u8, u8) = (17, 5, 12, 2);
const MSG_RECT: (u8, u8, u8, u8) = (1, 15, 28, 4);
const ACTIONS: u8 = 4;

const SOMEONE_ROOT: [RootEntry; 3] = [RootEntry::Items, RootEntry::Mons, RootEntry::Off];
const BEDROOM_ROOT: [RootEntry; 2] = [RootEntry::Items, RootEntry::Off];

/// A top-menu entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RootEntry {
    Items,
    Mons,
    Off,
}

/// Which list is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListKind {
    WithdrawItem,
    DepositItem,
    MoveItem,
    MoveItemTo,
    WithdrawMon,
    DepositMon,
    MoveMon,
    MoveMonTo,
}

impl ListKind {
    const fn is_mon(self) -> bool {
        matches!(
            self,
            ListKind::WithdrawMon | ListKind::DepositMon | ListKind::MoveMon | ListKind::MoveMonTo
        )
    }

    const fn tag(self) -> u8 {
        match self {
            ListKind::WithdrawItem => 0,
            ListKind::DepositItem => 1,
            ListKind::MoveItem => 2,
            ListKind::MoveItemTo => 3,
            ListKind::WithdrawMon => 4,
            ListKind::DepositMon => 5,
            ListKind::MoveMon => 6,
            ListKind::MoveMonTo => 7,
        }
    }
}

/// What the PC waits for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    Root {
        cursor: u8,
        entry: RootEntry,
    },
    Items {
        cursor: u8,
    },
    Mons {
        cursor: u8,
        box_no: u8,
    },
    List {
        kind: ListKind,
        row: u16,
        cancel: bool,
        box_no: u8,
    },
    Quantity {
        value: u16,
        max: u16,
    },
    Message,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Cancel,
    PcItem(u8),
    Bag { pocket: Pocket, index: u8 },
    Party(u8),
    PartyNew,
    BoxSlot(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MonRef {
    Party(u8),
    Box { box_no: u8, slot: u8 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum QtyTarget {
    Withdraw { slot: u8 },
    Deposit { pocket: Pocket, index: u8 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Next {
    Items,
    Mons,
    Stay,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Root(u8),
    Items(u8),
    Mons(u8),
    List(ListKind),
    MoveItem { from: u8 },
    MoveMon { from: MonRef },
    Qty(QtyTarget, QuantitySnap),
    Msg(Next),
}

/// [`super::kit::widgets::Quantity`] copied so [`St`] stays `Copy` without leaking the widget
/// into the message/restore state. The live value is this snapshot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct QuantitySnap {
    value: u16,
    max: u16,
}

impl From<super::kit::widgets::Quantity> for QuantitySnap {
    fn from(q: super::kit::widgets::Quantity) -> Self {
        QuantitySnap {
            value: q.value,
            max: q.max,
        }
    }
}

impl QuantitySnap {
    fn widget(self) -> super::kit::widgets::Quantity {
        super::kit::widgets::Quantity {
            value: self.value,
            max: self.max,
        }
    }
}

enum Area {
    Items,
    Mons,
}

enum Fail {
    Full,
}

struct Render {
    main: Panel,
    box_label: Option<Panel>,
    popup: Vec<Panel>,
}

fn root_entries(mode: PcMode) -> &'static [RootEntry] {
    match mode {
        PcMode::Someone => &SOMEONE_ROOT,
        PcMode::Bedroom => &BEDROOM_ROOT,
        PcMode::Storage => &[],
    }
}

const fn main_tiles() -> u16 {
    MAIN_RECT.2 as u16 * MAIN_RECT.3 as u16
}

const fn msg_base() -> u16 {
    super::kit::window::FULL_BASE + main_tiles() + BOX_RECT.2 as u16 * BOX_RECT.3 as u16
}

const fn qty_base() -> u16 {
    msg_base() + MSG_RECT.2 as u16 * MSG_RECT.3 as u16
}

fn nudge(cursor: u8, count: u8, pressed: Buttons) -> u8 {
    if count == 0 {
        return 0;
    }
    if pressed.contains(Buttons::UP) {
        cursor.saturating_sub(1)
    } else if pressed.contains(Buttons::DOWN) {
        (cursor + 1).min(count - 1)
    } else {
        cursor
    }
}

fn item_name<G: GameData>(game: &G, item: ItemId) -> &'static [u8] {
    game.item(item).map_or(&[EOS], |i| i.name)
}

fn species_name<G: GameData>(game: &G, species: crate::data::SpeciesId) -> Vec<u8> {
    let mut v = Vec::new();
    push(&mut v, game.battle().species_info(species).name);
    v.push(EOS);
    v
}

fn with_qty(name: &[u8], qty: u16) -> Vec<u8> {
    let mut v = Vec::new();
    push(&mut v, name);
    v.push(CHAR_SPACE);
    v.push(CHAR_TIMES);
    push_number(&mut v, u32::from(qty), 1, false);
    v.push(EOS);
    v
}

fn box_label<G: GameData>(game: &G, box_no: u8) -> Vec<u8> {
    let mut v = Vec::new();
    push(&mut v, game.ui_text(UiText::PcBox));
    v.push(CHAR_SPACE);
    push_number(&mut v, u32::from(box_no) + 1, 2, false);
    v.push(EOS);
    v
}

fn pc_room(pc: &PcStorage, item: ItemId) -> u32 {
    let mut room = 0u32;
    let mut empty = 0u32;
    for s in pc.items() {
        if s.qty == 0 {
            empty += 1;
        } else if s.item == item {
            room += u32::from(MAX_PC_ITEM_CAPACITY.saturating_sub(s.qty));
        }
    }
    room + empty * u32::from(MAX_PC_ITEM_CAPACITY)
}

fn bag_fits(bag: &crate::bag::Bag, pocket: Pocket, item: ItemId, qty: u16) -> bool {
    let mut trial = bag.clone();
    trial.add(pocket, item, qty)
}

fn max_qty(have: u16, mut fits: impl FnMut(u16) -> bool) -> u16 {
    if have == 0 || !fits(1) {
        return 0;
    }
    let mut lo = 1u16;
    let mut hi = have;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

fn take_pc_item(pc: &mut PcStorage, slot: usize, qty: u16) -> ItemId {
    let item = pc.items()[slot].item;
    let mut next = pc.items().to_vec();
    if next[slot].qty == qty {
        next.remove(slot);
        next.push(Slot::default());
    } else {
        next[slot].qty -= qty;
    }
    for (i, s) in next.into_iter().enumerate() {
        if pc.items()[i] != s {
            pc.set_item(i, s);
        }
    }
    item
}

fn swap_pc_items(pc: &mut PcStorage, a: u8, b: u8) {
    if a == b {
        return;
    }
    let sa = pc.items()[a as usize];
    let sb = pc.items()[b as usize];
    pc.set_item(a as usize, sb);
    pc.set_item(b as usize, sa);
}

fn free_box_slot(pc: &PcStorage, box_no: u8) -> Option<usize> {
    let b = box_no as usize;
    if b >= pc.box_count() as usize {
        return None;
    }
    match pc.pc_box(b) {
        None => Some(0),
        Some(slots) => slots.iter().position(|m| m.is_none()),
    }
}

fn withdraw_item<G: GameData>(cx: &mut ScreenCx<'_, G>, slot: u8, qty: u16) -> Result<(), Fail> {
    let Some(s) = cx.pc.items().get(slot as usize).copied() else {
        return Err(Fail::Full);
    };
    if s.qty < qty || qty == 0 {
        return Err(Fail::Full);
    }
    let Some(info) = cx.game.item(s.item) else {
        return Err(Fail::Full);
    };
    if !bag_fits(cx.bag, info.pocket, s.item, qty) {
        return Err(Fail::Full);
    }
    let item = take_pc_item(cx.pc, slot as usize, qty);
    if !cx.bag.add(info.pocket, item, qty) {
        cx.pc.add_item(item, qty);
        return Err(Fail::Full);
    }
    Ok(())
}

fn deposit_item<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    pocket: Pocket,
    index: u8,
    qty: u16,
) -> Result<(), Fail> {
    let Some(slot) = cx.bag.pocket(pocket).get(index as usize).copied() else {
        return Err(Fail::Full);
    };
    if slot.qty < qty || qty == 0 || pc_room(cx.pc, slot.item) < u32::from(qty) {
        return Err(Fail::Full);
    }
    let left = slot.qty - qty;
    cx.bag.set_slot(
        pocket,
        index as usize,
        if left == 0 {
            Slot::default()
        } else {
            Slot {
                item: slot.item,
                qty: left,
            }
        },
    );
    if !cx.pc.add_item(slot.item, qty) {
        cx.bag.add(pocket, slot.item, qty);
        return Err(Fail::Full);
    }
    Ok(())
}

fn withdraw_mon<G: GameData>(cx: &mut ScreenCx<'_, G>, box_no: u8, slot: u8) -> Result<(), Fail> {
    if cx.party.len() >= PARTY_SIZE {
        return Err(Fail::Full);
    }
    let Some(boxed) = cx.pc.mon(box_no as usize, slot as usize).copied() else {
        return Err(Fail::Full);
    };
    let mon = Pokemon::from_boxed(cx.game.battle(), &boxed);
    cx.pc.set_mon(box_no as usize, slot as usize, None);
    cx.party.push(mon).map_err(|_| Fail::Full)?;
    Ok(())
}

fn deposit_mon<G: GameData>(cx: &mut ScreenCx<'_, G>, party: u8, box_no: u8) -> Result<(), Fail> {
    if cx.party.len() <= 1 {
        return Err(Fail::Full);
    }
    let Some(free) = free_box_slot(cx.pc, box_no) else {
        return Err(Fail::Full);
    };
    let Some(mon) = cx.party.remove(party as usize) else {
        return Err(Fail::Full);
    };
    cx.pc.set_mon(box_no as usize, free, Some(mon.to_boxed()));
    Ok(())
}

fn place_mon<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    from: MonRef,
    row: Row,
    view_box: u8,
) -> Result<(), Fail> {
    match (from, row) {
        (MonRef::Party(a), Row::Party(b)) => {
            cx.party.swap(a as usize, b as usize);
            Ok(())
        }
        (MonRef::Party(_), Row::PartyNew) => Ok(()),
        (MonRef::Party(p), Row::BoxSlot(slot)) => move_party_to_box(cx, p, view_box, slot),
        (MonRef::Box { box_no, slot }, Row::Party(p)) => swap_box_party(cx, box_no, slot, p),
        (MonRef::Box { box_no, slot }, Row::PartyNew) => withdraw_mon(cx, box_no, slot),
        (MonRef::Box { box_no, slot }, Row::BoxSlot(dest)) => {
            move_box_to_box(cx, box_no, slot, view_box, dest)
        }
        _ => Err(Fail::Full),
    }
}

fn move_party_to_box<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    party: u8,
    box_no: u8,
    slot: u8,
) -> Result<(), Fail> {
    let Some(party_mon) = cx.party.get(party as usize).copied() else {
        return Err(Fail::Full);
    };
    let dest = cx.pc.mon(box_no as usize, slot as usize).copied();
    if dest.is_none() && cx.party.len() <= 1 {
        return Err(Fail::Full);
    }
    if let Some(boxed) = dest {
        let withdrawn = Pokemon::from_boxed(cx.game.battle(), &boxed);
        *cx.party.get_mut(party as usize).unwrap() = withdrawn;
        cx.pc
            .set_mon(box_no as usize, slot as usize, Some(party_mon.to_boxed()));
    } else {
        cx.party.remove(party as usize);
        cx.pc
            .set_mon(box_no as usize, slot as usize, Some(party_mon.to_boxed()));
    }
    Ok(())
}

fn swap_box_party<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    box_no: u8,
    slot: u8,
    party: u8,
) -> Result<(), Fail> {
    let Some(boxed) = cx.pc.mon(box_no as usize, slot as usize).copied() else {
        return Err(Fail::Full);
    };
    let Some(party_mon) = cx.party.get(party as usize).copied() else {
        return Err(Fail::Full);
    };
    *cx.party.get_mut(party as usize).unwrap() = Pokemon::from_boxed(cx.game.battle(), &boxed);
    cx.pc
        .set_mon(box_no as usize, slot as usize, Some(party_mon.to_boxed()));
    Ok(())
}

fn move_box_to_box<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    from_box: u8,
    from_slot: u8,
    to_box: u8,
    to_slot: u8,
) -> Result<(), Fail> {
    if from_box == to_box && from_slot == to_slot {
        return Ok(());
    }
    let Some(mon) = cx.pc.mon(from_box as usize, from_slot as usize).copied() else {
        return Err(Fail::Full);
    };
    let other = cx.pc.mon(to_box as usize, to_slot as usize).copied();
    cx.pc.set_mon(from_box as usize, from_slot as usize, other);
    cx.pc.set_mon(to_box as usize, to_slot as usize, Some(mon));
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PcMenu {
    pub mode: PcMode,
    /// Pokémon storage (the box label) rather than item storage.
    mon_area: bool,
    st: St,
    list: ListMenu,
    view_box: u8,
    /// List to restore when a refusal message closes.
    back: Option<St>,
    msg: Option<Box<Message>>,
    gfx: Gfx<Render>,
}

impl PcMenu {
    pub fn new(mode: PcMode) -> Self {
        let storage = mode == PcMode::Storage;
        PcMenu {
            mode,
            mon_area: storage,
            st: if storage {
                St::Mons(WITHDRAW)
            } else {
                St::Root(0)
            },
            list: ListMenu::new(1, MAX_SHOWN, Paging::None),
            view_box: 0,
            back: None,
            msg: None,
            gfx: Gfx::none(),
        }
    }

    /// What the PC waits for.
    pub fn prompt(&self) -> Prompt {
        match self.st {
            St::Root(cursor) => Prompt::Root {
                cursor,
                entry: root_entries(self.mode)
                    .get(cursor as usize)
                    .copied()
                    .unwrap_or(RootEntry::Off),
            },
            St::Items(cursor) => Prompt::Items { cursor },
            St::Mons(cursor) => Prompt::Mons {
                cursor,
                box_no: self.view_box,
            },
            St::Qty(_, q) => Prompt::Quantity {
                value: q.value,
                max: q.max,
            },
            St::Msg(_) => Prompt::Message,
            St::List(_) | St::MoveItem { .. } | St::MoveMon { .. } => Prompt::List {
                kind: self.list_kind().unwrap_or(ListKind::WithdrawItem),
                row: self.list.selected(),
                cancel: self.list.total > 0 && self.list.selected() + 1 == self.list.total,
                box_no: self.view_box,
            },
        }
    }

    fn list_kind(&self) -> Option<ListKind> {
        match self.st {
            St::List(k) => Some(k),
            St::MoveItem { .. } => Some(ListKind::MoveItemTo),
            St::MoveMon { .. } => Some(ListKind::MoveMonTo),
            _ => None,
        }
    }

    fn in_mon_list(&self) -> bool {
        self.list_kind().is_some_and(ListKind::is_mon)
    }

    fn wants_box_shift(&self) -> bool {
        matches!(self.st, St::Mons(_)) || self.in_mon_list()
    }

    fn rows<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Vec<Row> {
        let Some(kind) = self.list_kind() else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        match kind {
            ListKind::WithdrawItem | ListKind::MoveItem | ListKind::MoveItemTo => {
                for (i, s) in cx.pc.items().iter().enumerate() {
                    if s.qty > 0 {
                        rows.push(Row::PcItem(i as u8));
                    }
                }
            }
            ListKind::DepositItem => {
                for pocket in Pocket::ALL {
                    for (i, s) in cx.bag.pocket(pocket).iter().enumerate() {
                        if s.qty > 0 {
                            rows.push(Row::Bag {
                                pocket,
                                index: i as u8,
                            });
                        }
                    }
                }
            }
            ListKind::WithdrawMon => push_occupied(&mut rows, cx.pc, self.view_box),
            ListKind::DepositMon => push_party(&mut rows, cx.party.len()),
            ListKind::MoveMon => {
                push_party(&mut rows, cx.party.len());
                push_occupied(&mut rows, cx.pc, self.view_box);
            }
            ListKind::MoveMonTo => {
                push_party(&mut rows, cx.party.len());
                if cx.party.len() < PARTY_SIZE {
                    rows.push(Row::PartyNew);
                }
                for s in 0..BOX_SLOTS {
                    rows.push(Row::BoxSlot(s as u8));
                }
            }
        }
        rows.push(Row::Cancel);
        rows
    }

    fn row_text<G: GameData>(&self, cx: &ScreenCx<'_, G>, row: Row) -> Vec<u8> {
        let game = cx.game;
        match row {
            Row::Cancel => game.ui_text(UiText::BagCancel).to_vec(),
            Row::PcItem(i) => {
                let s = cx.pc.items()[i as usize];
                with_qty(item_name(game, s.item), s.qty)
            }
            Row::Bag { pocket, index } => {
                let s = cx.bag.pocket(pocket)[index as usize];
                with_qty(item_name(game, s.item), s.qty)
            }
            Row::Party(i) => species_name(game, cx.party.get(i as usize).unwrap().species),
            Row::PartyNew => game.ui_text(UiText::PcDepositMon).to_vec(),
            Row::BoxSlot(s) => match cx.pc.mon(self.view_box as usize, s as usize) {
                Some(m) => species_name(game, m.species),
                None => alloc::vec![CHAR_SPACE, EOS],
            },
        }
    }

    fn refocus(&mut self, total: u16, reset: bool) {
        let total = total.max(1);
        let sel = if reset {
            0
        } else {
            self.list.selected().min(total - 1)
        };
        self.list = ListMenu::new(total, MAX_SHOWN, Paging::None);
        let index = sel.min(self.list.total - 1);
        let max = self.list.max_showed.max(1);
        if index < max {
            self.list.scroll = 0;
            self.list.row = index;
        } else {
            self.list.scroll = index + 1 - max;
            self.list.row = max - 1;
        }
    }

    fn enter<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, st: St) {
        self.st = st;
        if self.list_kind().is_some() {
            let n = self.rows(cx).len() as u16;
            self.refocus(n, true);
        }
        self.redraw_all(cx);
    }

    fn shift_box<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, dir: i16) {
        let n = i16::from(cx.pc.box_count());
        if n <= 0 {
            return;
        }
        let next = (i16::from(self.view_box) + dir).rem_euclid(n) as u8;
        self.view_box = next;
        cx.pc.set_current_box(next);
        if self.list_kind().is_some() {
            let n = self.rows(cx).len() as u16;
            self.refocus(n, false);
            self.paint_list(cx);
        } else {
            self.paint_box(cx);
        }
    }

    fn close_menu<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) -> ScreenOut {
        if self.mode == PcMode::Storage {
            ScreenOut::Done(ScreenResult::Cancel)
        } else {
            self.mon_area = false;
            self.st = St::Root(0);
            self.redraw_all(cx);
            ScreenOut::Running
        }
    }

    fn back_to_menu<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) -> ScreenOut {
        if self.in_mon_list() {
            self.mon_area = true;
            self.st = St::Mons(WITHDRAW);
        } else {
            self.mon_area = false;
            self.st = St::Items(WITHDRAW);
        }
        self.redraw_all(cx);
        ScreenOut::Running
    }

    fn show_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, text: UiText, next: Next) {
        let (l, t, w, h) = MSG_RECT;
        let speed = cx.options.text_speed;
        let m = self.msg.get_or_insert_with(|| {
            Box::new(Message::new(Window::new(l, t, w, h, msg_base()), speed))
        });
        m.start(
            cx.game,
            cx.game.ui_text(text),
            &cx.player_name[..],
            None,
            false,
        );
        m.touch();
        self.st = St::Msg(next);
        self.compose(cx);
    }

    fn hide_msg(&mut self) {
        if let Some(m) = &mut self.msg {
            m.hide();
        }
    }

    fn succeed<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, area: Area) {
        self.back = None;
        let next = match area {
            Area::Items => Next::Items,
            Area::Mons => Next::Mons,
        };
        self.show_msg(cx, UiText::PcSentToBox, next);
    }

    fn fail<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, text: UiText) {
        self.back = Some(self.st);
        self.show_msg(cx, text, Next::Stay);
    }

    fn after_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, next: Next) {
        self.hide_msg();
        match next {
            Next::Items => {
                self.mon_area = false;
                self.st = St::Items(WITHDRAW);
            }
            Next::Mons => {
                self.mon_area = true;
                self.st = St::Mons(WITHDRAW);
            }
            Next::Stay => {
                self.st = self.back.take().unwrap_or(St::Items(WITHDRAW));
            }
        }
        self.redraw_all(cx);
    }

    fn root_chosen<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        entry: RootEntry,
    ) -> ScreenOut {
        match entry {
            RootEntry::Items => {
                self.mon_area = false;
                self.enter(cx, St::Items(WITHDRAW));
                ScreenOut::Running
            }
            RootEntry::Mons => {
                self.mon_area = true;
                self.enter(cx, St::Mons(WITHDRAW));
                ScreenOut::Running
            }
            RootEntry::Off => ScreenOut::Done(ScreenResult::Cancel),
        }
    }

    fn action_chosen<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        cursor: u8,
        mons: bool,
    ) -> ScreenOut {
        match cursor {
            WITHDRAW if mons => self.enter(cx, St::List(ListKind::WithdrawMon)),
            DEPOSIT if mons => self.enter(cx, St::List(ListKind::DepositMon)),
            MOVE if mons => self.enter(cx, St::List(ListKind::MoveMon)),
            WITHDRAW => self.enter(cx, St::List(ListKind::WithdrawItem)),
            DEPOSIT => self.enter(cx, St::List(ListKind::DepositItem)),
            MOVE => self.enter(cx, St::List(ListKind::MoveItem)),
            _ => return self.close_menu(cx),
        }
        ScreenOut::Running
    }

    fn on_row<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, row: Row) -> ScreenOut {
        if matches!(row, Row::Cancel) {
            return self.back_to_menu(cx);
        }
        match self.st {
            St::List(ListKind::WithdrawItem) => self.begin_withdraw(cx, row),
            St::List(ListKind::DepositItem) => self.begin_deposit(cx, row),
            St::List(ListKind::MoveItem) => self.hold_item(cx, row),
            St::MoveItem { from } => self.place_item(cx, from, row),
            St::List(ListKind::WithdrawMon) => self.withdraw_mon_row(cx, row),
            St::List(ListKind::DepositMon) => self.deposit_mon_row(cx, row),
            St::List(ListKind::MoveMon) => self.hold_mon(cx, row),
            St::MoveMon { from } => self.place_mon_row(cx, from, row),
            _ => ScreenOut::Running,
        }
    }

    fn begin_withdraw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, row: Row) -> ScreenOut {
        let Row::PcItem(slot) = row else {
            return self.back_to_menu(cx);
        };
        let have = cx.pc.items().get(slot as usize).map(|s| s.qty).unwrap_or(0);
        let item = cx
            .pc
            .items()
            .get(slot as usize)
            .map(|s| s.item)
            .unwrap_or(ItemId(0));
        let max = cx.game.item(item).map_or(0, |info| {
            max_qty(have, |n| bag_fits(cx.bag, info.pocket, item, n))
        });
        if max == 0 {
            self.fail(cx, UiText::ShopNoRoom);
        } else {
            let q = super::kit::widgets::Quantity::new(max);
            self.st = St::Qty(QtyTarget::Withdraw { slot }, q.into());
            self.redraw_all(cx);
        }
        ScreenOut::Running
    }

    fn begin_deposit<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, row: Row) -> ScreenOut {
        let Row::Bag { pocket, index } = row else {
            return self.back_to_menu(cx);
        };
        let slot = cx
            .bag
            .pocket(pocket)
            .get(index as usize)
            .copied()
            .unwrap_or_default();
        let room = pc_room(cx.pc, slot.item).min(u32::from(slot.qty));
        let max = u16::try_from(room).unwrap_or(u16::MAX);
        if max == 0 {
            self.fail(cx, UiText::ShopNoRoom);
        } else {
            let q = super::kit::widgets::Quantity::new(max);
            self.st = St::Qty(QtyTarget::Deposit { pocket, index }, q.into());
            self.redraw_all(cx);
        }
        ScreenOut::Running
    }

    fn hold_item<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, row: Row) -> ScreenOut {
        if let Row::PcItem(slot) = row {
            self.enter(cx, St::MoveItem { from: slot });
        }
        ScreenOut::Running
    }

    fn place_item<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        from: u8,
        row: Row,
    ) -> ScreenOut {
        if let Row::PcItem(slot) = row {
            swap_pc_items(cx.pc, from, slot);
            self.succeed(cx, Area::Items);
        } else {
            return self.back_to_menu(cx);
        }
        ScreenOut::Running
    }

    fn withdraw_mon_row<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, row: Row) -> ScreenOut {
        if let Row::BoxSlot(slot) = row {
            let r = withdraw_mon(cx, self.view_box, slot);
            self.finish_mon(cx, r)
        } else {
            self.back_to_menu(cx)
        }
    }

    fn deposit_mon_row<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, row: Row) -> ScreenOut {
        if let Row::Party(slot) = row {
            let r = deposit_mon(cx, slot, self.view_box);
            self.finish_mon(cx, r)
        } else {
            self.back_to_menu(cx)
        }
    }

    fn hold_mon<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, row: Row) -> ScreenOut {
        let from = match row {
            Row::Party(i) => Some(MonRef::Party(i)),
            Row::BoxSlot(slot) => Some(MonRef::Box {
                box_no: self.view_box,
                slot,
            }),
            _ => None,
        };
        if let Some(from) = from {
            self.enter(cx, St::MoveMon { from });
        }
        ScreenOut::Running
    }

    fn place_mon_row<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        from: MonRef,
        row: Row,
    ) -> ScreenOut {
        let r = place_mon(cx, from, row, self.view_box);
        self.finish_mon(cx, r)
    }

    fn finish_mon<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        r: Result<(), Fail>,
    ) -> ScreenOut {
        match r {
            Ok(()) => self.succeed(cx, Area::Mons),
            Err(Fail::Full) => self.fail(cx, UiText::PcBoxFull),
        }
        ScreenOut::Running
    }

    fn confirm_qty<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, target: QtyTarget, n: u16) {
        let r = match target {
            QtyTarget::Withdraw { slot } => withdraw_item(cx, slot, n),
            QtyTarget::Deposit { pocket, index } => deposit_item(cx, pocket, index, n),
        };
        match r {
            Ok(()) => self.succeed(cx, Area::Items),
            Err(Fail::Full) => {
                self.back = Some(match target {
                    QtyTarget::Withdraw { .. } => St::List(ListKind::WithdrawItem),
                    QtyTarget::Deposit { .. } => St::List(ListKind::DepositItem),
                });
                self.show_msg(cx, UiText::ShopNoRoom, Next::Stay);
            }
        }
    }

    fn layout<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Render {
        let game = cx.game;
        let mut tiles = TileAlloc::full();
        let (l, t, w, h) = MAIN_RECT;
        let main = tiles.panel(game, l, t, w, h, true);
        let box_label = if self.mon_area {
            let (l, t, w, h) = BOX_RECT;
            Some(tiles.panel(game, l, t, w, h, true))
        } else {
            None
        };
        Render {
            main,
            box_label,
            popup: Vec::new(),
        }
    }

    fn menu_labels<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Vec<&'static [u8]> {
        let game = cx.game;
        match self.st {
            St::Root(_) => root_entries(self.mode)
                .iter()
                .map(|e| match e {
                    RootEntry::Items => game.ui_text(UiText::PcItemStorage),
                    RootEntry::Mons => game.ui_text(UiText::PcBox),
                    RootEntry::Off => game.ui_text(UiText::PcTurnOff),
                })
                .collect(),
            St::Items(_) => alloc::vec![
                game.ui_text(UiText::PcWithdrawItem),
                game.ui_text(UiText::PcDepositItem),
                game.ui_text(UiText::PcMoveMon),
                game.ui_text(UiText::BagCancel),
            ],
            St::Mons(_) => alloc::vec![
                game.ui_text(UiText::PcWithdrawMon),
                game.ui_text(UiText::PcDepositMon),
                game.ui_text(UiText::PcMoveMon),
                game.ui_text(UiText::BagCancel),
            ],
            _ => Vec::new(),
        }
    }

    fn menu_cursor(&self) -> Option<u8> {
        match self.st {
            St::Root(c) | St::Items(c) | St::Mons(c) => Some(c),
            _ => None,
        }
    }

    fn draw_main<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        if self.list_kind().is_some() {
            self.draw_list_into(cx, r);
            return;
        }
        let labels = self.menu_labels(cx);
        let cursor = self.menu_cursor().unwrap_or(u8::MAX);
        draw_menu(cx.game, &mut r.main, &labels, cursor);
    }

    fn draw_list_into<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        r.main.clear(game);
        let rows = self.rows(cx);
        for (row, i) in self.list.shown().enumerate() {
            let y = row as i16 * ROW_H + 1;
            let text = rows
                .get(i as usize)
                .map(|row| self.row_text(cx, *row))
                .unwrap_or_default();
            r.main.text(game, &text, CURSOR_W, y);
        }
        draw_cursor(
            game,
            &mut r.main,
            self.list.max_showed as u8,
            self.list.row as u8,
        );
    }

    fn draw_box_into<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let Some(p) = &mut r.box_label else {
            return;
        };
        p.clear(cx.game);
        let text = box_label(cx.game, self.view_box);
        p.text(cx.game, &text, 0, 1);
    }

    fn popups<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Vec<Panel> {
        let St::Qty(_, q) = self.st else {
            return Vec::new();
        };
        let game = cx.game;
        let mut tiles = TileAlloc::at(qty_base());
        let (l, t, w, h) = QTY_RECT;
        let mut p = tiles.panel(game, l, t, w, h, true);
        p.text(game, &q.widget().text(3), 0, 1);
        alloc::vec![p]
    }

    fn compose<G: GameData>(&self, cx: &mut ScreenCx<'_, G>) {
        let Some(r) = self.gfx.0.as_deref() else {
            return;
        };
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        map.put(&r.main);
        if let Some(p) = &r.box_label {
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
        self.draw_main(cx, &mut r);
        self.draw_box_into(cx, &mut r);
        r.popup = self.popups(cx);
        if let Some(m) = &mut self.msg {
            m.touch();
        }
        self.gfx.0 = Some(Box::new(r));
        self.compose(cx);
    }

    fn paint_list<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(mut r) = self.gfx.0.take() else {
            self.redraw_all(cx);
            return;
        };
        self.draw_list_into(cx, &mut r);
        self.draw_box_into(cx, &mut r);
        self.gfx.0 = Some(r);
    }

    fn paint_box<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(mut r) = self.gfx.0.take() else {
            self.redraw_all(cx);
            return;
        };
        self.draw_box_into(cx, &mut r);
        self.gfx.0 = Some(r);
    }

    fn paint_menu_cursor<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(cursor) = self.menu_cursor() else {
            return;
        };
        let count = self.menu_labels(cx).len() as u8;
        let Some(r) = self.gfx.0.as_deref_mut() else {
            return;
        };
        draw_cursor(cx.game, &mut r.main, count, cursor);
    }

    fn paint_qty<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let popup = self.popups(cx);
        if let Some(r) = self.gfx.0.as_deref_mut() {
            r.popup = popup;
        }
    }

    fn list_input<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, pressed: Buttons) -> ScreenOut {
        let before = self.list.scroll;
        match self.list.input(pressed) {
            MenuInput::Moved => {
                if self.list.scroll != before {
                    self.paint_list(cx);
                } else if let Some(r) = self.gfx.0.as_deref_mut() {
                    draw_cursor(
                        cx.game,
                        &mut r.main,
                        self.list.max_showed as u8,
                        self.list.row as u8,
                    );
                }
            }
            MenuInput::Chosen(i) => {
                let row = self
                    .rows(cx)
                    .get(i as usize)
                    .copied()
                    .unwrap_or(Row::Cancel);
                return self.on_row(cx, row);
            }
            MenuInput::Cancel => return self.back_to_menu(cx),
            MenuInput::None => {}
        }
        ScreenOut::Running
    }

    fn finish<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, out: ScreenOut) -> ScreenOut {
        let mut budget = super::kit::frame_budget(cx.f);
        if let Some(m) = &mut self.msg {
            m.flush_within(cx.f, &mut budget);
        }
        if let Some(r) = self.gfx.0.as_deref_mut() {
            r.main.flush_within(cx.f, &mut budget);
            if let Some(p) = &mut r.box_label {
                p.flush_within(cx.f, &mut budget);
            }
            for p in &mut r.popup {
                p.flush_within(cx.f, &mut budget);
            }
        }
        out
    }
}

fn push_party(rows: &mut Vec<Row>, len: usize) {
    for i in 0..len {
        rows.push(Row::Party(i as u8));
    }
}

fn push_occupied(rows: &mut Vec<Row>, pc: &PcStorage, box_no: u8) {
    for s in 0..BOX_SLOTS {
        if pc.mon(box_no as usize, s).is_some() {
            rows.push(Row::BoxSlot(s as u8));
        }
    }
}

impl Screen for PcMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if self.gfx.0.is_none() {
            self.view_box = cx.pc.current_box();
            self.redraw_all(cx);
        }
        let pressed = input.pressed;
        if self.wants_box_shift() {
            if pressed.contains(Buttons::LEFT) {
                self.shift_box(cx, -1);
                return self.finish(cx, ScreenOut::Running);
            }
            if pressed.contains(Buttons::RIGHT) {
                self.shift_box(cx, 1);
                return self.finish(cx, ScreenOut::Running);
            }
        }
        let out = match self.st {
            St::Root(c) => {
                let n = root_entries(self.mode).len() as u8;
                if pressed.contains(Buttons::A) {
                    let entry = root_entries(self.mode)
                        .get(c as usize)
                        .copied()
                        .unwrap_or(RootEntry::Off);
                    self.root_chosen(cx, entry)
                } else if pressed.contains(Buttons::B) {
                    ScreenOut::Done(ScreenResult::Cancel)
                } else {
                    let to = nudge(c, n, pressed);
                    if to != c {
                        self.st = St::Root(to);
                        self.paint_menu_cursor(cx);
                    }
                    ScreenOut::Running
                }
            }
            St::Items(c) => self.menu_step(cx, pressed, c, false),
            St::Mons(c) => self.menu_step(cx, pressed, c, true),
            St::List(_) | St::MoveItem { .. } | St::MoveMon { .. } => self.list_input(cx, pressed),
            St::Qty(target, mut q) => {
                if pressed.contains(Buttons::A) {
                    self.confirm_qty(cx, target, q.value);
                } else if pressed.contains(Buttons::B) {
                    let back = match target {
                        QtyTarget::Withdraw { .. } => St::List(ListKind::WithdrawItem),
                        QtyTarget::Deposit { .. } => St::List(ListKind::DepositItem),
                    };
                    self.st = back;
                    self.redraw_all(cx);
                } else {
                    let mut widget = q.widget();
                    if widget.input(pressed) {
                        q.value = widget.value;
                        q.max = widget.max;
                        self.st = St::Qty(target, q);
                        self.paint_qty(cx);
                    }
                }
                ScreenOut::Running
            }
            St::Msg(next) => {
                let done = self
                    .msg
                    .as_mut()
                    .is_none_or(|m| m.update(cx.game, pressed, input.held));
                if done {
                    self.after_msg(cx, next);
                }
                ScreenOut::Running
            }
        };
        self.finish(cx, out)
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.mode as u8, u8::from(self.mon_area), self.view_box]);
        self.list.hash(h);
        match self.st {
            St::Root(c) => h.write(&[0, c]),
            St::Items(c) => h.write(&[1, c]),
            St::Mons(c) => h.write(&[2, c]),
            St::List(k) => h.write(&[3, k.tag()]),
            St::MoveItem { from } => h.write(&[4, from]),
            St::MoveMon { from } => match from {
                MonRef::Party(i) => h.write(&[5, 0, i]),
                MonRef::Box { box_no, slot } => h.write(&[5, 1, box_no, slot]),
            },
            St::Qty(target, q) => {
                let (tag, a, b) = match target {
                    QtyTarget::Withdraw { slot } => (0, slot, 0),
                    QtyTarget::Deposit { pocket, index } => (1, pocket as u8, index),
                };
                h.write(&[6, tag, a, b]);
                h.write_u16(q.value);
                h.write_u16(q.max);
            }
            St::Msg(n) => h.write(&[7, n as u8]),
        }
        match self.back {
            None => h.write(&[0xFF]),
            Some(St::List(k)) => h.write(&[1, k.tag()]),
            Some(St::MoveItem { from }) => h.write(&[2, from]),
            Some(St::MoveMon {
                from: MonRef::Party(i),
            }) => h.write(&[3, i]),
            Some(St::MoveMon {
                from: MonRef::Box { box_no, slot },
            }) => h.write(&[4, box_no, slot]),
            Some(_) => h.write(&[5]),
        }
        if let Some(m) = &self.msg {
            m.hash(h);
        }
    }
}

impl PcMenu {
    fn menu_step<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        pressed: Buttons,
        cursor: u8,
        mons: bool,
    ) -> ScreenOut {
        if pressed.contains(Buttons::A) {
            self.action_chosen(cx, cursor, mons)
        } else if pressed.contains(Buttons::B) {
            self.close_menu(cx)
        } else {
            let to = nudge(cursor, ACTIONS, pressed);
            if to != cursor {
                self.st = if mons { St::Mons(to) } else { St::Items(to) };
                self.paint_menu_cursor(cx);
            }
            ScreenOut::Running
        }
    }
}
