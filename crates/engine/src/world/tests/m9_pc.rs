//! The PC (`player_pc.c`, `pokemon_storage_system.c`): item storage and the Pokémon
//! storage list withdraw, deposit and move, including the party.

use super::m9_b::{G, POKE_BALL, POTION};
use super::*;
use crate::bag::Slot;
use crate::battle::test_data::{BUG, DARK, ELECTRIC, FIRE, GHOST, NORMAL, WATER};
use crate::data::SpecialIds;
use crate::input::Buttons;
use crate::mon::{Ivs, PARTY_SIZE, Pokemon};
use crate::pc::BOX_SLOTS;
use crate::rng::Rng;
use crate::ui::pc::{DEPOSIT, ListKind, MOVE, Prompt, RootEntry, WITHDRAW};
use crate::ui::{PcMode, ScreenKind, ScreenState};
use crate::video::Frame;

/// [`G`] with Pokémon boxes and item storage.
struct PcGame;

const PC: PcGame = PcGame;

impl GameData for PcGame {
    fn map(&self, id: MapId) -> &'static MapData {
        G.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            pc_boxes: 3,
            pc_item_slots: 4,
            ..G.special_ids()
        }
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        G.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        G.player_sprite()
    }
    fn font(&self) -> &'static Font {
        G.font()
    }
    fn window(&self) -> &'static WindowGfx {
        G.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        G.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        G.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        G.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        G.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        G.battle()
    }
    fn new_game(&self) -> NewGame {
        G.new_game()
    }
    fn start_menu(&self) -> &'static [StartItem] {
        G.start_menu()
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        G.item(id)
    }
    fn item_use(&self, id: ItemId) -> ItemUse {
        G.item_use(id)
    }
    fn mart_bonus(&self) -> Option<(ItemId, u16, ItemId)> {
        G.mart_bonus()
    }
    fn tm_move(&self, id: ItemId) -> Option<MoveId> {
        G.tm_move(id)
    }
    fn can_learn_tm(&self, species: SpeciesId, tm: ItemId) -> bool {
        G.can_learn_tm(species, tm)
    }
    fn is_hm_move(&self, m: MoveId) -> bool {
        G.is_hm_move(m)
    }
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        G.field_move_gate(m)
    }
}

fn run(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        let f = w.step(&PC, keys);
        f.validate().unwrap();
        assert!(
            f.is_black()
                || f.bulk
                || crate::ui::kit::frame_bytes(&f) <= crate::ui::kit::UPLOAD_BYTES_VISIBLE,
            "a visible frame over budget"
        );
    }
}

fn tap(w: &mut World, keys: Buttons) {
    run(w, keys, 1);
    run(w, Buttons::NONE, 1);
}

fn until(w: &mut World, done: impl Fn(&World) -> bool) {
    for _ in 0..300 {
        if done(w) {
            return;
        }
        run(w, Buttons::NONE, 1);
    }
    panic!("never reached: {:?}", top(w).map(|s| s.prompt_debug()));
}

fn top(w: &World) -> Option<&ScreenState> {
    w.screens().and_then(|s| s.top()).map(|(_, s)| s)
}

trait PromptDebug {
    fn prompt_debug(&self) -> String;
}

impl PromptDebug for ScreenState {
    fn prompt_debug(&self) -> String {
        match self {
            ScreenState::Pc(p) => format!("{:?}", p.prompt()),
            other => format!("{other:?}"),
        }
    }
}

fn prompt(w: &World) -> Prompt {
    match top(w) {
        Some(ScreenState::Pc(p)) => p.prompt(),
        t => panic!("the pc is not on top: {t:?}"),
    }
}

fn pc_on_top(w: &World) -> bool {
    matches!(top(w), Some(ScreenState::Pc(_)))
        && w.screens()
            .is_some_and(|s| s.host.phase == crate::ui::kit::Phase::Running)
}

fn boot() -> World {
    let mut w = World::new(&PC);
    run(&mut w, Buttons::NONE, 16);
    w
}

fn give(w: &mut World, species: SpeciesId, level: u8) -> Pokemon {
    let t = PC.battle();
    let mut rng = Rng::new(u32::from(species.0) << 8 | u32::from(level));
    let p = Pokemon::new(t, species, level, Ivs::Fixed(10), 0, 1, &mut rng);
    w.s.party.push(p).unwrap();
    p
}

fn open(w: &mut World, mode: PcMode) {
    let mut f = Frame::default();
    crate::world::screens::open(&mut w.s, &PC, ScreenKind::Pc(mode), false, &mut f);
    until(w, pc_on_top);
}

/// Presses A through messages until `done`.
fn through(w: &mut World, done: impl Fn(&World) -> bool) {
    for _ in 0..600 {
        if done(w) {
            return;
        }
        let msg = matches!(prompt(w), Prompt::Message);
        tap(w, if msg { Buttons::A } else { Buttons::NONE });
    }
    panic!("never reached: {:?}", top(w).map(|s| s.prompt_debug()));
}

fn mons(cursor: u8, box_no_expected: u8) -> impl Fn(&World) -> bool {
    move |w| {
        pc_on_top(w)
            && matches!(
                prompt(w),
                Prompt::Mons {
                    cursor: c,
                    box_no
                } if c == cursor && box_no == box_no_expected
            )
    }
}

fn items(cursor: u8) -> impl Fn(&World) -> bool {
    move |w| pc_on_top(w) && matches!(prompt(w), Prompt::Items { cursor: c } if c == cursor)
}

fn list(kind: ListKind) -> impl Fn(&World) -> bool {
    move |w| {
        pc_on_top(w)
            && matches!(prompt(w), Prompt::List { kind: k, cancel: false, .. } if k == kind)
    }
}

fn add_item(w: &mut World, item: ItemId, n: u16) {
    let pocket = PC.item(item).unwrap().pocket;
    assert!(w.s.bag.add(pocket, item, n));
}

fn action_cursor(p: Prompt) -> Option<u8> {
    match p {
        Prompt::Mons { cursor, .. } | Prompt::Items { cursor } => Some(cursor),
        _ => None,
    }
}

/// WITHDRAW / DEPOSIT / MOVE, then the first row of that list.
fn choose_action(w: &mut World, cursor: u8, kind: ListKind) {
    for _ in 0..6 {
        if action_cursor(prompt(w)) == Some(cursor) {
            break;
        }
        tap(w, Buttons::DOWN);
    }
    assert_eq!(action_cursor(prompt(w)), Some(cursor));
    tap(w, Buttons::A);
    until(w, list(kind));
}

fn pick_row(w: &mut World, row: u16) {
    for _ in 0..row {
        tap(w, Buttons::DOWN);
    }
    match prompt(w) {
        Prompt::List {
            row: got, cancel, ..
        } => {
            assert_eq!((got, cancel), (row, false));
        }
        other => panic!("not a list: {other:?}"),
    }
    tap(w, Buttons::A);
}

#[test]
fn depositing_a_mon_and_withdrawing_it_round_trips_the_party() {
    let mut w = boot();
    let normal = give(&mut w, NORMAL, 10);
    give(&mut w, FIRE, 12);
    w.s.party.get_mut(0).unwrap().hp = 1;
    open(&mut w, PcMode::Storage);
    assert!(mons(WITHDRAW, 0)(&w));

    choose_action(&mut w, DEPOSIT, ListKind::DepositMon);
    pick_row(&mut w, 0);
    through(&mut w, mons(WITHDRAW, 0));

    assert_eq!(w.s.party.len(), 1);
    assert_eq!(w.s.party.get(0).unwrap().species, FIRE);
    let stored = w.s.pc.mon(0, 0).expect("deposited into box 1 slot 1");
    assert_eq!(stored.species, NORMAL);
    assert_eq!(stored.personality, normal.personality);
    assert_eq!(stored.nickname, normal.nickname);
    assert!(w.s.pc.mon(0, 1).is_none());

    choose_action(&mut w, WITHDRAW, ListKind::WithdrawMon);
    pick_row(&mut w, 0);
    through(&mut w, mons(WITHDRAW, 0));

    assert!(w.s.pc.mon(0, 0).is_none());
    assert_eq!(w.s.party.len(), 2);
    let back = w.s.party.get(1).unwrap();
    assert_eq!(back.species, NORMAL);
    assert_eq!(back.personality, normal.personality);
    assert_eq!(back.nickname, normal.nickname);
    assert_eq!(back.hp, back.stats[0]);
    assert!(back.hp > 1);
    assert_eq!(w.s.party.get(0).unwrap().species, FIRE);
}

#[test]
fn deposit_follows_the_box_on_screen() {
    let mut w = boot();
    let normal = give(&mut w, NORMAL, 8);
    give(&mut w, FIRE, 8);
    open(&mut w, PcMode::Storage);
    tap(&mut w, Buttons::RIGHT);
    assert!(mons(WITHDRAW, 1)(&w));
    assert_eq!(w.s.pc.current_box(), 1);
    choose_action(&mut w, DEPOSIT, ListKind::DepositMon);
    assert!(matches!(prompt(&w), Prompt::List { box_no: 1, .. }));
    pick_row(&mut w, 0);
    through(&mut w, mons(WITHDRAW, 1));
    assert!(w.s.pc.mon(0, 0).is_none());
    assert_eq!(
        w.s.pc.mon(1, 0).map(|m| m.personality),
        Some(normal.personality)
    );
}

#[test]
fn move_puts_a_party_mon_in_a_chosen_box_slot() {
    let mut w = boot();
    let normal = give(&mut w, NORMAL, 9);
    give(&mut w, FIRE, 9);
    open(&mut w, PcMode::Storage);
    choose_action(&mut w, MOVE, ListKind::MoveMon);
    pick_row(&mut w, 0);
    until(&mut w, list(ListKind::MoveMonTo));
    // Party, the other party mon, the empty party slot, then box slots.
    pick_row(&mut w, 3 + 5);
    through(&mut w, mons(WITHDRAW, 0));
    assert_eq!(w.s.party.len(), 1);
    assert_eq!(w.s.party.get(0).unwrap().species, FIRE);
    assert!(w.s.pc.mon(0, 0).is_none());
    assert_eq!(
        w.s.pc.mon(0, 5).map(|m| (m.species, m.personality)),
        Some((NORMAL, normal.personality))
    );
}

#[test]
fn move_swaps_a_boxed_mon_with_the_party() {
    let mut w = boot();
    let normal = give(&mut w, NORMAL, 11);
    let fire = give(&mut w, FIRE, 11);
    open(&mut w, PcMode::Storage);
    choose_action(&mut w, DEPOSIT, ListKind::DepositMon);
    pick_row(&mut w, 0);
    through(&mut w, mons(WITHDRAW, 0));

    choose_action(&mut w, MOVE, ListKind::MoveMon);
    pick_row(&mut w, 1);
    until(&mut w, list(ListKind::MoveMonTo));
    pick_row(&mut w, 0);
    through(&mut w, mons(WITHDRAW, 0));

    let lead = w.s.party.get(0).unwrap();
    assert_eq!(w.s.party.len(), 1);
    assert_eq!(lead.species, NORMAL);
    assert_eq!(lead.personality, normal.personality);
    assert_eq!(lead.hp, lead.stats[0]);
    assert_eq!(
        w.s.pc.mon(0, 0).map(|m| (m.species, m.personality)),
        Some((FIRE, fire.personality))
    );
}

#[test]
fn move_carries_a_mon_into_another_box() {
    let mut w = boot();
    let normal = give(&mut w, NORMAL, 7);
    give(&mut w, FIRE, 7);
    open(&mut w, PcMode::Storage);
    choose_action(&mut w, DEPOSIT, ListKind::DepositMon);
    pick_row(&mut w, 0);
    through(&mut w, mons(WITHDRAW, 0));

    choose_action(&mut w, MOVE, ListKind::MoveMon);
    pick_row(&mut w, 1);
    until(&mut w, list(ListKind::MoveMonTo));
    tap(&mut w, Buttons::RIGHT);
    assert!(matches!(
        prompt(&w),
        Prompt::List {
            kind: ListKind::MoveMonTo,
            box_no: 1,
            row: 0,
            cancel: false
        }
    ));
    // Party mon, empty party slot, then box slot 0.
    pick_row(&mut w, 2);
    through(&mut w, mons(WITHDRAW, 1));
    assert!(w.s.pc.mon(0, 0).is_none());
    assert_eq!(
        w.s.pc.mon(1, 0).map(|m| m.personality),
        Some(normal.personality)
    );
    assert_eq!(w.s.party.get(0).unwrap().species, FIRE);
    assert_eq!(w.s.pc.current_box(), 1);
}

#[test]
fn deposit_refuses_the_last_mon_and_a_full_box() {
    let mut w = boot();
    let only = give(&mut w, NORMAL, 5);
    open(&mut w, PcMode::Storage);
    choose_action(&mut w, DEPOSIT, ListKind::DepositMon);
    pick_row(&mut w, 0);
    through(&mut w, list(ListKind::DepositMon));
    assert_eq!(w.s.party.len(), 1);
    assert_eq!(w.s.party.get(0).unwrap().personality, only.personality);
    assert!(w.s.pc.mon(0, 0).is_none());
    tap(&mut w, Buttons::B);
    through(&mut w, mons(WITHDRAW, 0));

    let mut w = boot();
    give(&mut w, NORMAL, 5);
    give(&mut w, FIRE, 5);
    let filler = {
        let t = PC.battle();
        let mut rng = Rng::new(99);
        Pokemon::new(t, WATER, 4, Ivs::Fixed(10), 0, 1, &mut rng).to_boxed()
    };
    for slot in 0..BOX_SLOTS {
        w.s.pc.set_mon(0, slot, Some(filler));
    }
    open(&mut w, PcMode::Storage);
    choose_action(&mut w, DEPOSIT, ListKind::DepositMon);
    pick_row(&mut w, 0);
    through(&mut w, list(ListKind::DepositMon));
    assert_eq!(w.s.party.len(), 2);
    assert_eq!(w.s.party.get(0).unwrap().species, NORMAL);
    assert_eq!(w.s.pc.mon(0, 0).map(|m| m.species), Some(WATER));
}

#[test]
fn withdraw_refuses_a_full_party() {
    let mut w = boot();
    for species in [NORMAL, FIRE, WATER, BUG, DARK, GHOST] {
        give(&mut w, species, 10);
    }
    assert_eq!(w.s.party.len(), PARTY_SIZE);
    let extra = {
        let t = PC.battle();
        let mut rng = Rng::new(4);
        Pokemon::new(t, ELECTRIC, 15, Ivs::Fixed(10), 0, 1, &mut rng)
    };
    w.s.pc.set_mon(0, 0, Some(extra.to_boxed()));
    open(&mut w, PcMode::Storage);
    choose_action(&mut w, WITHDRAW, ListKind::WithdrawMon);
    pick_row(&mut w, 0);
    through(&mut w, list(ListKind::WithdrawMon));
    assert_eq!(w.s.party.len(), PARTY_SIZE);
    assert!(w.s.party.iter().all(|m| m.species != ELECTRIC));
    assert_eq!(
        w.s.pc.mon(0, 0).map(|m| m.personality),
        Some(extra.personality)
    );
}

#[test]
fn item_storage_withdraws_deposits_and_swaps() {
    let mut w = boot();
    add_item(&mut w, POTION, 5);
    add_item(&mut w, POKE_BALL, 2);
    open(&mut w, PcMode::Bedroom);
    assert_eq!(
        prompt(&w),
        Prompt::Root {
            cursor: 0,
            entry: RootEntry::Items
        }
    );
    tap(&mut w, Buttons::A);
    assert!(items(WITHDRAW)(&w));

    choose_action(&mut w, DEPOSIT, ListKind::DepositItem);
    pick_row(&mut w, 0);
    until(&mut w, |w| {
        matches!(prompt(w), Prompt::Quantity { value: 1, max: 5 })
    });
    tap(&mut w, Buttons::RIGHT);
    assert!(matches!(prompt(&w), Prompt::Quantity { value: 5, max: 5 }));
    tap(&mut w, Buttons::A);
    through(&mut w, items(WITHDRAW));
    assert_eq!(w.s.bag.count(POTION), 0);
    assert_eq!(
        w.s.pc.item_entries().collect::<Vec<_>>(),
        vec![(0, POTION, 5)]
    );

    choose_action(&mut w, DEPOSIT, ListKind::DepositItem);
    pick_row(&mut w, 0);
    until(&mut w, |w| {
        matches!(prompt(w), Prompt::Quantity { value: 1, max: 2 })
    });
    tap(&mut w, Buttons::UP);
    assert!(matches!(prompt(&w), Prompt::Quantity { value: 2, max: 2 }));
    tap(&mut w, Buttons::A);
    through(&mut w, items(WITHDRAW));
    assert_eq!(w.s.bag.count(POKE_BALL), 0);
    assert_eq!(
        w.s.pc.item_entries().collect::<Vec<_>>(),
        vec![(0, POTION, 5), (1, POKE_BALL, 2)]
    );

    choose_action(&mut w, MOVE, ListKind::MoveItem);
    pick_row(&mut w, 0);
    until(&mut w, list(ListKind::MoveItemTo));
    pick_row(&mut w, 1);
    through(&mut w, items(WITHDRAW));
    assert_eq!(
        w.s.pc.item_entries().collect::<Vec<_>>(),
        vec![(0, POKE_BALL, 2), (1, POTION, 5)]
    );

    choose_action(&mut w, WITHDRAW, ListKind::WithdrawItem);
    pick_row(&mut w, 0);
    until(&mut w, |w| {
        matches!(prompt(w), Prompt::Quantity { value: 1, max: 2 })
    });
    tap(&mut w, Buttons::UP);
    tap(&mut w, Buttons::A);
    through(&mut w, items(WITHDRAW));
    assert_eq!(w.s.bag.count(POKE_BALL), 2);
    assert_eq!(w.s.bag.count(POTION), 0);
    assert_eq!(
        w.s.pc.item_entries().collect::<Vec<_>>(),
        vec![(0, POTION, 5)]
    );
}

#[test]
fn deposit_refuses_an_item_the_pc_cannot_hold() {
    let mut w = boot();
    for i in 0..4 {
        w.s.pc.set_item(
            i,
            Slot {
                item: ItemId(50 + i as u16),
                qty: 999,
            },
        );
    }
    add_item(&mut w, POTION, 1);
    open(&mut w, PcMode::Bedroom);
    tap(&mut w, Buttons::A);
    choose_action(&mut w, DEPOSIT, ListKind::DepositItem);
    pick_row(&mut w, 0);
    through(&mut w, list(ListKind::DepositItem));
    assert_eq!(w.s.bag.count(POTION), 1);
    assert_eq!(w.s.pc.items()[0].item, ItemId(50));
    assert_eq!(w.s.pc.items()[0].qty, 999);
}

#[test]
fn withdraw_refuses_an_item_the_bag_cannot_hold() {
    let mut w = boot();
    let pocket = PC.item(POTION).unwrap().pocket;
    let slots = PC.special_ids().pocket_slots[pocket as usize] as u16;
    assert!(w.s.bag.add(pocket, POTION, slots * 99));
    assert!(w.s.pc.add_item(POTION, 1));
    let held = w.s.bag.count(POTION);
    open(&mut w, PcMode::Bedroom);
    tap(&mut w, Buttons::A);
    choose_action(&mut w, WITHDRAW, ListKind::WithdrawItem);
    pick_row(&mut w, 0);
    through(&mut w, list(ListKind::WithdrawItem));
    assert_eq!(w.s.bag.count(POTION), held);
    assert_eq!(
        w.s.pc.item_entries().collect::<Vec<_>>(),
        vec![(0, POTION, 1)]
    );
}

#[test]
fn someones_pc_opens_items_and_pokemon_storage() {
    let mut w = boot();
    give(&mut w, NORMAL, 10);
    open(&mut w, PcMode::Someone);
    assert_eq!(
        prompt(&w),
        Prompt::Root {
            cursor: 0,
            entry: RootEntry::Items
        }
    );
    tap(&mut w, Buttons::DOWN);
    assert_eq!(
        prompt(&w),
        Prompt::Root {
            cursor: 1,
            entry: RootEntry::Mons
        }
    );
    tap(&mut w, Buttons::A);
    assert!(mons(WITHDRAW, 0)(&w));
    tap(&mut w, Buttons::B);
    through(&mut w, |w| {
        matches!(
            prompt(w),
            Prompt::Root {
                cursor: 0,
                entry: RootEntry::Items
            }
        )
    });
    tap(&mut w, Buttons::DOWN);
    tap(&mut w, Buttons::DOWN);
    assert_eq!(
        prompt(&w),
        Prompt::Root {
            cursor: 2,
            entry: RootEntry::Off
        }
    );
    tap(&mut w, Buttons::A);
    until(&mut w, |w| w.screens_open() == 0);
}

#[test]
fn the_storage_pc_closes_on_b() {
    let mut w = boot();
    give(&mut w, NORMAL, 10);
    open(&mut w, PcMode::Storage);
    tap(&mut w, Buttons::B);
    until(&mut w, |w| w.screens_open() == 0);
    assert_eq!(w.s.party.len(), 1);
}
