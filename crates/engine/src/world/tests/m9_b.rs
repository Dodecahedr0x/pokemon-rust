//! Milestone 9 Task B: the bag (`item_menu.c`) and item use out of battle (`item_use.c`) on a
//! game without pictures: START → BAG, the pockets, USE (medicine, Repel, TMs, key items),
//! GIVE, TOSS, REGISTER, the sell and battle modes, and the repel's field hooks.

use super::m9_k::{HM, TM, TM_MOVE, UiGame};
use super::*;
use crate::battle::test_data::{GROWL, HARDEN, NORMAL, QUICK_ATTACK, TACKLE};
use crate::data::{
    DecorId, DecorationInfo, FieldMove, FieldMoveGate, ItemInfo, ItemUse, KeyUse, MedicineEffect,
    Pocket, StartAction, StartItem,
};
use crate::item_use::{CURE_ALL, CURE_POISON};
use crate::mon::{Ivs, Pokemon, Status};
use crate::rng::Rng;
use crate::script::Report;
use crate::ui::bag::{Action, BagMenu, POCKETS, Prompt};
use crate::ui::party::{PartyMenu, Prompt as PartyPrompt};
use crate::ui::{BagMode, ScreenKind, ScreenResult, ScreenState};

pub(super) const POTION: ItemId = ItemId(13);
const ANTIDOTE: ItemId = ItemId(14);
const FULL_RESTORE: ItemId = ItemId(19);
const REVIVE: ItemId = ItemId(24);
const PROTEIN: ItemId = ItemId(64);
const RARE_CANDY: ItemId = ItemId(68);
const ESCAPE_ROPE: ItemId = ItemId(85);
const REPEL: ItemId = ItemId(86);
pub(super) const POKE_BALL: ItemId = ItemId(4);
/// The mart bonus (Gen3 `ITEM_PREMIER_BALL`).
pub(super) const PREMIER_BALL: ItemId = ItemId(12);
const BIKE: ItemId = ItemId(259);
const SUPER_POTION: ItemId = ItemId(22);
const VAR_REPEL: VarId = VarId(0x4021);
const REPEL_WORE_OFF: ScriptId = ScriptId(999);

static DESK: DecorationInfo = DecorationInfo {
    name: b"SMALL DESK\xFF",
    price: 3000,
    category: 0,
    description: b"\xFF",
};

static START: [StartItem; 3] = [
    StartItem {
        label: TextId(0),
        action: StartAction::Bag,
        shown_if: None,
        description: None,
    },
    StartItem {
        label: TextId(0),
        action: StartAction::Party,
        shown_if: None,
        description: None,
    },
    StartItem {
        label: TextId(0),
        action: StartAction::Exit,
        shown_if: None,
        description: None,
    },
];

const fn info(pocket: Pocket, price: u16, key: bool) -> ItemInfo {
    ItemInfo {
        name: &[1, 0xFF],
        pocket,
        price,
        key,
    }
}
static ITEMS: ItemInfo = info(Pocket::Items, 300, false);
static BALLS: ItemInfo = info(Pocket::PokeBalls, 200, false);
static KEYS: ItemInfo = info(Pocket::KeyItems, 0, true);

/// [`UiGame`] with a bag start menu, medicine, a repel, an Escape Rope, a ball and a bike.
pub(super) struct BagGame;

impl GameData for BagGame {
    fn map(&self, id: MapId) -> &'static MapData {
        UiGame.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            var_repel_steps: Some(VAR_REPEL),
            script_repel_wore_off: Some(REPEL_WORE_OFF),
            ..UiGame.special_ids()
        }
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        UiGame.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        UiGame.player_sprite()
    }
    fn font(&self) -> &'static Font {
        UiGame.font()
    }
    fn window(&self) -> &'static WindowGfx {
        UiGame.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        UiGame.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        UiGame.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        UiGame.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        UiGame.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        UiGame.battle()
    }
    fn new_game(&self) -> NewGame {
        UiGame.new_game()
    }
    fn start_menu(&self) -> &'static [StartItem] {
        &START
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        match id {
            TM | HM => UiGame.item(id),
            POKE_BALL | PREMIER_BALL => Some(&BALLS),
            BIKE => Some(&KEYS),
            POTION | ANTIDOTE | FULL_RESTORE | REVIVE | PROTEIN | RARE_CANDY | ESCAPE_ROPE
            | REPEL | SUPER_POTION => Some(&ITEMS),
            _ => None,
        }
    }
    fn item_use(&self, id: ItemId) -> ItemUse {
        match id {
            POTION => ItemUse::Medicine(MedicineEffect::Heal(20)),
            SUPER_POTION => ItemUse::Medicine(MedicineEffect::Heal(50)),
            ANTIDOTE => ItemUse::Medicine(MedicineEffect::Cure(CURE_POISON)),
            FULL_RESTORE => ItemUse::Medicine(MedicineEffect::HealCure(u16::MAX, CURE_ALL)),
            REVIVE => ItemUse::Medicine(MedicineEffect::Revive(128)),
            PROTEIN => ItemUse::Medicine(MedicineEffect::Ev(1, 10)),
            RARE_CANDY => ItemUse::Medicine(MedicineEffect::LevelUp),
            REPEL => ItemUse::Repel(100),
            ESCAPE_ROPE => ItemUse::EscapeRope,
            POKE_BALL => ItemUse::Ball(10),
            BIKE => ItemUse::Key(KeyUse::Bike),
            _ => ItemUse::None,
        }
    }
    fn mart_bonus(&self) -> Option<(ItemId, u16, ItemId)> {
        Some((POKE_BALL, 10, PREMIER_BALL))
    }
    fn decoration(&self, id: DecorId) -> Option<&'static DecorationInfo> {
        (id == DecorId(1)).then_some(&DESK)
    }
    fn tm_move(&self, id: ItemId) -> Option<MoveId> {
        UiGame.tm_move(id)
    }
    fn can_learn_tm(&self, species: SpeciesId, tm: ItemId) -> bool {
        UiGame.can_learn_tm(species, tm)
    }
    fn is_hm_move(&self, m: MoveId) -> bool {
        UiGame.is_hm_move(m)
    }
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        UiGame.field_move_gate(m)
    }
}

pub(super) const G: BagGame = BagGame;

pub(super) fn run(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        let f = w.step(&G, keys);
        f.validate().unwrap();
        assert!(
            f.is_black()
                || f.bulk
                || crate::ui::kit::frame_bytes(&f) <= crate::ui::kit::UPLOAD_BYTES_VISIBLE,
            "a visible frame over budget"
        );
    }
}

pub(super) fn tap(w: &mut World, keys: Buttons) {
    run(w, keys, 1);
    run(w, Buttons::NONE, 1);
}

/// The world after the new-game fade with one Normal mon (level 10, four moves).
pub(super) fn world() -> World {
    let mut w = World::new(&G);
    run(&mut w, Buttons::NONE, 16);
    let t = G.battle();
    let mut rng = Rng::new(7);
    let mut p = Pokemon::new(t, NORMAL, 10, Ivs::Fixed(10), 0, 1, &mut rng);
    p.moves = [Some(TACKLE), Some(GROWL), Some(QUICK_ATTACK), Some(HARDEN)];
    for i in 0..4 {
        p.pp[i] = t.move_info(p.moves[i].unwrap()).pp;
    }
    w.s.party.push(p).unwrap();
    w
}

pub(super) fn add(w: &mut World, item: ItemId, n: u16) {
    let pocket = G.item(item).unwrap().pocket;
    assert!(w.s.bag.add(pocket, item, n));
}

pub(super) fn top(w: &World) -> Option<&ScreenState> {
    w.screens().and_then(|s| s.top()).map(|(_, s)| s)
}

fn bag(w: &World) -> &BagMenu {
    match top(w) {
        Some(ScreenState::Bag(b)) => b,
        t => panic!("the bag is not on top: {t:?}"),
    }
}

fn prompt(w: &World) -> Prompt {
    bag(w).prompt_in(&w.s.bag)
}

fn party(w: &World) -> &PartyMenu {
    match top(w) {
        Some(ScreenState::Party(p)) => p,
        t => panic!("the party menu is not on top: {t:?}"),
    }
}

/// Runs idle frames until `done` (fades, prints), at most 200.
pub(super) fn until(w: &mut World, done: impl Fn(&World) -> bool) {
    for _ in 0..200 {
        if done(w) {
            return;
        }
        run(w, Buttons::NONE, 1);
    }
    panic!("never reached");
}

fn bag_on_top(w: &World) -> bool {
    matches!(top(w), Some(ScreenState::Bag(_)))
        && w.screens()
            .is_some_and(|s| s.host.phase == crate::ui::kit::Phase::Running)
}

fn party_on_top(w: &World) -> bool {
    matches!(top(w), Some(ScreenState::Party(_)))
        && w.screens()
            .is_some_and(|s| s.host.phase == crate::ui::kit::Phase::Running)
}

/// Opens the bag in `mode` directly and lets it fade in.
fn open_bag(w: &mut World, mode: BagMode) {
    let mut f = Frame::default();
    crate::world::screens::open(&mut w.s, &G, ScreenKind::Bag(mode), false, &mut f);
    until(w, bag_on_top);
}

/// Moves to `pocket` and the list entry of `item`, then presses A.
fn pick(w: &mut World, item: ItemId) {
    let pocket = G.item(item).unwrap().pocket;
    let want = POCKETS.iter().position(|&p| p == pocket).unwrap();
    let index = (w.s.bag.pocket(pocket).iter())
        .filter(|s| s.qty > 0)
        .position(|s| s.item == item)
        .expect("in the bag") as u16;
    loop {
        let Prompt::List {
            pocket: p,
            selected,
            ..
        } = prompt(w)
        else {
            panic!("not on the list: {:?}", prompt(w));
        };
        let at = POCKETS.iter().position(|&q| q == p).unwrap();
        if at < want {
            tap(w, Buttons::RIGHT);
        } else if at > want {
            tap(w, Buttons::LEFT);
        } else if selected < index {
            tap(w, Buttons::DOWN);
        } else if selected > index {
            tap(w, Buttons::UP);
        } else {
            tap(w, Buttons::A);
            return;
        }
    }
}

/// From the context menu, chooses `a`.
fn act(w: &mut World, a: Action) {
    let i = bag(w)
        .actions()
        .iter()
        .position(|&x| x == a)
        .expect("action listed");
    loop {
        let Prompt::Context { cursor } = prompt(w) else {
            panic!("no context menu: {:?}", prompt(w));
        };
        if cursor as usize == i {
            tap(w, Buttons::A);
            return;
        }
        tap(w, Buttons::DOWN);
    }
}

/// Presses A through messages until `done`.
fn through(w: &mut World, done: impl Fn(&World) -> bool) {
    for _ in 0..400 {
        if done(w) {
            return;
        }
        let msg = match top(w) {
            Some(ScreenState::Bag(b)) => b.prompt_in(&w.s.bag) == Prompt::Message,
            Some(ScreenState::Party(p)) => p.prompt() == PartyPrompt::Message,
            _ => false,
        };
        tap(w, if msg { Buttons::A } else { Buttons::NONE });
    }
    panic!("never reached");
}

fn list_prompt(w: &World) -> bool {
    bag_on_top(w) && matches!(prompt(w), Prompt::List { .. })
}

/// USE `item` on party slot 0 from the field bag, back to the list.
fn use_on_lead(w: &mut World, item: ItemId) {
    pick(w, item);
    act(w, Action::Use);
    until(w, party_on_top);
    tap(w, Buttons::A);
    through(w, list_prompt);
}

#[test]
fn start_bag_opens_the_bag_and_closing_it_shows_the_start_menu_again() {
    let mut w = world();
    tap(&mut w, Buttons::START);
    until(&mut w, |w| {
        matches!(top(w), Some(ScreenState::StartMenu(_)))
    });
    tap(&mut w, Buttons::A);
    until(&mut w, bag_on_top);
    assert_eq!(bag(&w).mode, BagMode::Field);
    assert_eq!(
        prompt(&w),
        Prompt::List {
            pocket: Pocket::Items,
            selected: 0,
            item: None
        }
    );
    tap(&mut w, Buttons::B);
    until(&mut w, |w| {
        matches!(top(w), Some(ScreenState::StartMenu(_)))
            && w.screens()
                .is_some_and(|s| s.host.phase == crate::ui::kit::Phase::Running)
    });
    tap(&mut w, Buttons::B);
    until(&mut w, |w| w.screens_open() == 0);
    assert!(w.field_active());
}

#[test]
fn opening_compacts_pockets_sorts_tms_and_pockets_switch_in_gen3_order() {
    let mut w = world();
    add(&mut w, POTION, 1);
    add(&mut w, ANTIDOTE, 2);
    assert!(w.s.bag.remove(Pocket::Items, POTION, 1));
    add(&mut w, HM, 1);
    add(&mut w, TM, 1);
    assert_eq!(w.s.bag.pocket(Pocket::Items)[0].qty, 0);
    open_bag(&mut w, BagMode::Field);
    assert_eq!(w.s.bag.pocket(Pocket::Items)[0].item, ANTIDOTE);
    let tms: Vec<_> = w.s.bag.pocket(Pocket::TmHm)[..2]
        .iter()
        .map(|s| s.item)
        .collect();
    assert_eq!(tms, [TM, HM]);
    let mut seen = Vec::new();
    for _ in 0..6 {
        if let Prompt::List { pocket, .. } = prompt(&w) {
            seen.push(pocket);
        }
        tap(&mut w, Buttons::RIGHT);
    }
    // No wrap past KEY ITEMS.
    assert_eq!(
        seen,
        [
            Pocket::Items,
            Pocket::PokeBalls,
            Pocket::TmHm,
            Pocket::Berries,
            Pocket::KeyItems,
            Pocket::KeyItems
        ]
    );
}

#[test]
fn pocket_context_menus_follow_gen3() {
    let mut w = world();
    add(&mut w, POTION, 1);
    add(&mut w, POKE_BALL, 1);
    add(&mut w, BIKE, 1);
    add(&mut w, TM, 1);
    open_bag(&mut w, BagMode::Field);
    for (item, want) in [
        (
            POTION,
            &[Action::Use, Action::Give, Action::Toss, Action::Cancel][..],
        ),
        (POKE_BALL, &[Action::Give, Action::Toss, Action::Cancel]),
        (TM, &[Action::Use, Action::Give, Action::Cancel]),
        (BIKE, &[Action::Use, Action::Register, Action::Cancel]),
    ] {
        pick(&mut w, item);
        assert_eq!(bag(&w).actions(), want, "{item:?}");
        tap(&mut w, Buttons::B);
        assert!(matches!(prompt(&w), Prompt::List { .. }));
    }
}

#[test]
fn a_potion_heals_twenty_hp_and_is_used_up_on_a_hurt_mon_only() {
    let mut w = world();
    add(&mut w, POTION, 2);
    let max = w.s.party.get(0).unwrap().stats[0];
    w.s.party.get_mut(0).unwrap().hp = max;
    open_bag(&mut w, BagMode::Field);
    // Full HP: "It won't have any effect.", the potion kept.
    use_on_lead(&mut w, POTION);
    assert_eq!(w.s.bag.count(POTION), 2);
    w.s.party.get_mut(0).unwrap().hp = 5;
    use_on_lead(&mut w, POTION);
    assert_eq!(w.s.party.get(0).unwrap().hp, (5 + 20).min(max));
    assert_eq!(w.s.bag.count(POTION), 1);
}

/// `World::use_item` (the bag's USE for tools and the explorer) on medicine: the party menu
/// opens on it, as the bag's USE does, and the chosen mon is healed.
#[test]
fn use_item_opens_the_party_menu_on_medicine_too() {
    let mut w = world();
    add(&mut w, POTION, 1);
    w.s.party.get_mut(0).unwrap().hp = 5;
    assert!(w.use_item(&G, POTION));
    until(&mut w, party_on_top);
    tap(&mut w, Buttons::A);
    through(&mut w, |w| w.screens_open() == 0);
    assert_eq!(w.s.party.get(0).unwrap().hp, 25);
    assert_eq!(w.s.bag.count(POTION), 0);
    assert!(!w.use_item(&G, ESCAPE_ROPE), "no party use");
}

#[test]
fn status_heals_revive_full_restore_rare_candy_and_vitamins() {
    let t = G.battle();
    let mut rng = Rng::new(1);
    let mut reports = Vec::new();
    let mut mon = Pokemon::new(t, NORMAL, 10, Ivs::Fixed(10), 0, 1, &mut rng);
    let max = mon.stats[0];
    // Antidote: poison only.
    assert!(crate::item_use::use_on_mon(&G, &mut mon, ANTIDOTE, &mut rng, &mut reports).is_none());
    mon.status = Status::Toxic(2);
    let u = crate::item_use::use_on_mon(&G, &mut mon, ANTIDOTE, &mut rng, &mut reports).unwrap();
    assert_eq!(
        (mon.status, u.text),
        (Status::None, UiText::ItemCuredPoison)
    );
    // Full Restore: HP and status.
    mon.hp = 1;
    mon.status = Status::Burn;
    crate::item_use::use_on_mon(&G, &mut mon, FULL_RESTORE, &mut rng, &mut reports).unwrap();
    assert_eq!((mon.hp, mon.status), (max, Status::None));
    // Revive: fainted only, half the max HP.
    assert!(crate::item_use::use_on_mon(&G, &mut mon, REVIVE, &mut rng, &mut reports).is_none());
    mon.hp = 0;
    crate::item_use::use_on_mon(&G, &mut mon, REVIVE, &mut rng, &mut reports).unwrap();
    assert_eq!(mon.hp, max / 2);
    // A potion does nothing to a fainted mon.
    mon.hp = 0;
    assert!(crate::item_use::use_on_mon(&G, &mut mon, POTION, &mut rng, &mut reports).is_none());
    // Rare Candy: one level, the exp at its threshold.
    let u = crate::item_use::use_on_mon(&G, &mut mon, RARE_CANDY, &mut rng, &mut reports).unwrap();
    assert_eq!((mon.level, u.text), (11, UiText::ItemElevatedToLevel));
    let growth = t.species_info(NORMAL).growth;
    assert_eq!(mon.exp, crate::mon::exp_for_level(growth, 11));
    // Protein: +10 attack EVs up to 100, then no effect.
    for n in 1..=10 {
        crate::item_use::use_on_mon(&G, &mut mon, PROTEIN, &mut rng, &mut reports).unwrap();
        assert_eq!(mon.evs[1], 10 * n);
    }
    assert!(crate::item_use::use_on_mon(&G, &mut mon, PROTEIN, &mut rng, &mut reports).is_none());
}

#[test]
fn repel_sets_the_steps_lingers_blocks_weaker_mons_and_wears_off() {
    let mut w = world();
    add(&mut w, REPEL, 2);
    open_bag(&mut w, BagMode::Field);
    pick(&mut w, REPEL);
    act(&mut w, Action::Use);
    through(&mut w, list_prompt);
    assert_eq!((w.s.vars.var(VAR_REPEL), w.s.bag.count(REPEL)), (100, 1));
    // A second one while it lasts: "still linger", kept.
    pick(&mut w, REPEL);
    act(&mut w, Action::Use);
    through(&mut w, list_prompt);
    assert_eq!((w.s.vars.var(VAR_REPEL), w.s.bag.count(REPEL)), (100, 1));
    let ids = G.special_ids();
    assert!(crate::wild::repel_blocks(9, &w.s.vars, &w.s.party, &ids));
    assert!(!crate::wild::repel_blocks(10, &w.s.vars, &w.s.party, &ids));
    w.s.vars.set_var(VAR_REPEL, 2);
    assert_eq!(crate::item_use::on_step(&mut w.s, &G), None);
    assert_eq!(crate::item_use::on_step(&mut w.s, &G), Some(REPEL_WORE_OFF));
    assert_eq!(crate::item_use::on_step(&mut w.s, &G), None);
    assert!(!crate::wild::repel_blocks(1, &w.s.vars, &w.s.party, &ids));
}

#[test]
fn toss_asks_how_many_confirms_and_throws_away() {
    let mut w = world();
    add(&mut w, POTION, 5);
    open_bag(&mut w, BagMode::Field);
    pick(&mut w, POTION);
    act(&mut w, Action::Toss);
    until(&mut w, |w| matches!(prompt(w), Prompt::Quantity { .. }));
    assert_eq!(prompt(&w), Prompt::Quantity { value: 1, max: 5 });
    tap(&mut w, Buttons::UP);
    tap(&mut w, Buttons::UP);
    tap(&mut w, Buttons::A);
    until(&mut w, |w| matches!(prompt(w), Prompt::YesNo { .. }));
    tap(&mut w, Buttons::A);
    through(&mut w, list_prompt);
    assert_eq!(w.s.bag.count(POTION), 2);
    // No answers keep them.
    pick(&mut w, POTION);
    act(&mut w, Action::Toss);
    until(&mut w, |w| matches!(prompt(w), Prompt::Quantity { .. }));
    tap(&mut w, Buttons::A);
    until(&mut w, |w| matches!(prompt(w), Prompt::YesNo { .. }));
    tap(&mut w, Buttons::B);
    assert!(list_prompt(&w));
    assert_eq!(w.s.bag.count(POTION), 2);
}

fn bike_lines(w: &World) -> usize {
    w.log().lines.iter().filter(|l| **l == "bike").count()
}

#[test]
fn register_sets_the_select_item_and_select_reports_the_bike() {
    let mut w = world();
    add(&mut w, BIKE, 1);
    open_bag(&mut w, BagMode::Field);
    pick(&mut w, BIKE);
    act(&mut w, Action::Register);
    assert_eq!(w.registered_item(), Some(BIKE));
    pick(&mut w, BIKE);
    assert_eq!(bag(&w).actions()[1], Action::Deselect);
    act(&mut w, Action::Deselect);
    assert_eq!(w.registered_item(), None);
    pick(&mut w, BIKE);
    act(&mut w, Action::Register);
    // USE on the bike: Milestone 10.
    pick(&mut w, BIKE);
    let before = bike_lines(&w);
    act(&mut w, Action::Use);
    assert_eq!(bike_lines(&w), before + 1);
    tap(&mut w, Buttons::B);
    until(&mut w, |w| w.screens_open() == 0 && w.field_active());
    run(&mut w, Buttons::NONE, 4);
    run(&mut w, Buttons::SELECT, 1);
    assert!(
        w.s.events
            .contains(&Event::Report(Report::Unsupported("bike")))
    );
}

#[test]
fn a_tm_boots_up_asks_and_teaches_through_the_party_menu() {
    let mut w = world();
    add(&mut w, TM, 1);
    open_bag(&mut w, BagMode::Field);
    pick(&mut w, TM);
    act(&mut w, Action::Use);
    through(&mut w, |w| {
        bag_on_top(w) && matches!(prompt(w), Prompt::YesNo { .. })
    });
    tap(&mut w, Buttons::A);
    until(&mut w, party_on_top);
    assert_eq!(party(&w).mode, crate::ui::PartyMode::UseItem(TM));
    // Four moves: forget one (yes), the summary's first move.
    tap(&mut w, Buttons::A);
    for _ in 0..400 {
        match top(&w) {
            Some(ScreenState::Bag(_)) if list_prompt(&w) => break,
            Some(ScreenState::Party(p)) => match p.prompt() {
                PartyPrompt::Message | PartyPrompt::YesNo { cursor: 0 } => tap(&mut w, Buttons::A),
                _ => run(&mut w, Buttons::NONE, 1),
            },
            Some(ScreenState::Summary(_)) => tap(&mut w, Buttons::A),
            _ => run(&mut w, Buttons::NONE, 1),
        }
    }
    assert!(list_prompt(&w));
    assert_eq!(w.s.party.get(0).unwrap().moves[0], Some(TM_MOVE));
    assert_eq!(w.s.bag.count(TM), 0);
}

#[test]
fn sell_mode_pays_half_the_price() {
    let mut w = world();
    add(&mut w, POTION, 3);
    w.s.money = 100;
    open_bag(&mut w, BagMode::Sell);
    pick(&mut w, POTION);
    through(&mut w, |w| matches!(prompt(w), Prompt::Quantity { .. }));
    tap(&mut w, Buttons::UP);
    tap(&mut w, Buttons::A);
    through(&mut w, |w| matches!(prompt(w), Prompt::YesNo { .. }));
    tap(&mut w, Buttons::A);
    through(&mut w, list_prompt);
    assert_eq!((w.s.money, w.s.bag.count(POTION)), (100 + 2 * 150, 1));
}

#[test]
fn battle_mode_answers_the_item_and_refuses_field_only_items() {
    let mut w = world();
    add(&mut w, POTION, 1);
    add(&mut w, REPEL, 1);
    let mut f = Frame::default();
    crate::world::screens::open(&mut w.s, &G, ScreenKind::Bag(BagMode::Battle), true, &mut f);
    until(&mut w, bag_on_top);
    pick(&mut w, REPEL);
    assert_eq!(bag(&w).actions(), [Action::Cancel]);
    tap(&mut w, Buttons::A);
    pick(&mut w, POTION);
    assert_eq!(bag(&w).actions(), [Action::BattleUse, Action::Cancel]);
    tap(&mut w, Buttons::A);
    until(&mut w, |w| w.screens_open() == 0);
    assert_eq!(w.s.answer, Some(POTION.0));
    let _ = ScreenResult::Cancel;
}

#[test]
fn escape_rope_is_refused_outside_caves() {
    let mut w = world();
    add(&mut w, ESCAPE_ROPE, 1);
    open_bag(&mut w, BagMode::Field);
    pick(&mut w, ESCAPE_ROPE);
    act(&mut w, Action::Use);
    through(&mut w, list_prompt);
    assert_eq!(w.s.bag.count(ESCAPE_ROPE), 1);
}

#[test]
fn give_from_the_bag_hands_the_item_to_the_chosen_mon() {
    let mut w = world();
    add(&mut w, POTION, 2);
    open_bag(&mut w, BagMode::Field);
    pick(&mut w, POTION);
    act(&mut w, Action::Give);
    until(&mut w, party_on_top);
    tap(&mut w, Buttons::A);
    until(&mut w, list_prompt);
    assert_eq!(w.s.party.get(0).unwrap().held, Some(POTION));
    assert_eq!(w.s.bag.count(POTION), 1);
}
