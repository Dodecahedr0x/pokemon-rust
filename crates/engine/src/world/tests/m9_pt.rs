//! Milestone 9 Task PT: the party menu (switching, the summary, field moves, the battle
//! actions) and Gen3's TM / HM teach flow (`ItemUseCB_TMHM`), on a game without pictures.

use super::m9_k::{HM, HM_MOVE, NO_TM, TM, TM_MOVE, UiGame, ui_run, ui_tap, ui_world};
use super::*;
use crate::battle::test_data::{GROWL, HARDEN, NORMAL, QUICK_ATTACK, TACKLE};
use crate::data::{FieldMove, Pocket};
use crate::ui::party::{Action, PartyMenu, Prompt};
use crate::ui::summary::{Page, Summary};
use crate::ui::{PartyMode, ScreenKind, ScreenState};

const FOUR: &[MoveId] = &[TACKLE, GROWL, QUICK_ATTACK, HARDEN];

fn top(w: &World) -> &ScreenState {
    &w.screens()
        .expect("a screen is open")
        .top()
        .expect("a top screen")
        .1
}

fn party(w: &World) -> &PartyMenu {
    match top(w) {
        ScreenState::Party(p) => p,
        _ => panic!("the party menu is not on top"),
    }
}

fn summary(w: &World) -> &Summary {
    match top(w) {
        ScreenState::Summary(s) => s,
        _ => panic!("the summary is not on top"),
    }
}

/// Lets the hosting's fades finish.
fn settle(w: &mut World) {
    ui_run(w, Buttons::NONE, 40);
}

/// Taps `keys` (2 frames each).
fn keys(w: &mut World, keys: &[Buttons]) {
    for &k in keys {
        ui_tap(w, k, 2);
    }
}

/// Runs until the screens closed (field back).
fn until_closed(w: &mut World) {
    for _ in 0..120 {
        if w.screens_open() == 0 {
            break;
        }
        ui_run(w, Buttons::NONE, 1);
    }
    assert_eq!(w.screens_open(), 0);
    assert!(w.field_active());
}

/// The world with `mons`, `item` in the bag and the party menu open to teach it.
fn teaching(mons: &[(SpeciesId, &[MoveId])], item: ItemId) -> World {
    let mut w = ui_world(mons);
    assert!(w.s.bag.add(Pocket::TmHm, item, 1));
    assert!(w.use_item(&UiGame, item));
    settle(&mut w);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: 0 });
    w
}

/// Opens the field party menu (START → POKéMON's screen) directly.
fn field_party(mons: &[(SpeciesId, &[MoveId])], mode: PartyMode) -> World {
    let mut w = ui_world(mons);
    let mut f = Frame::default();
    crate::world::screens::open(&mut w.s, &UiGame, ScreenKind::Party(mode), false, &mut f);
    settle(&mut w);
    w
}

#[test]
fn use_item_needs_the_item_in_the_bag_and_a_party_use() {
    let mut w = ui_world(&[(NORMAL, &[TACKLE])]);
    assert!(!w.use_item(&UiGame, TM), "not in the bag");
    w.s.bag.add(Pocket::TmHm, ItemId(302), 1);
    assert!(!w.use_item(&UiGame, ItemId(302)), "no party use");
    assert_eq!(w.screens_open(), 0);
}

/// A TM into a free move slot: "learned", the TM used up, the field back (the friendship
/// gain goes through `friendship::on_event`, Task E's).
#[test]
fn a_tm_teaches_into_a_free_slot_and_is_used_up() {
    let mut w = teaching(&[(NORMAL, &[TACKLE])], TM);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message);
    assert_eq!(w.s.party.get(0).unwrap().moves[1], Some(TM_MOVE));
    assert_eq!(w.s.bag.count(TM), 0);
    keys(&mut w, &[Buttons::A]);
    until_closed(&mut w);
}

/// An HM over a full move set: replace? yes → the summary → forget slot 1 → learned; the HM
/// stays in the bag.
#[test]
fn an_hm_replaces_a_move_through_the_summary_and_is_kept() {
    let mut w = teaching(&[(NORMAL, FOUR)], HM);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::YesNo { cursor: 0 });
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message);
    keys(&mut w, &[Buttons::A]);
    settle(&mut w);
    assert_eq!(summary(&w).page(), Page::Moves);
    assert_eq!(summary(&w).move_cursor, 0);
    keys(&mut w, &[Buttons::DOWN, Buttons::A]);
    settle(&mut w);
    assert_eq!(party(&w).prompt(), Prompt::Message, "forgot …");
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message, "learned …");
    assert_eq!(
        w.s.party.get(0).unwrap().moves,
        [
            Some(TACKLE),
            Some(HM_MOVE),
            Some(QUICK_ATTACK),
            Some(HARDEN)
        ]
    );
    assert_eq!(w.s.bag.count(HM), 1);
    keys(&mut w, &[Buttons::A]);
    until_closed(&mut w);
}

/// The summary refuses an HM move (`CanReplaceMove`); B there gives up: "stop learning?" yes
/// → nothing learned, the TM kept.
#[test]
fn the_summary_refuses_forgetting_an_hm_and_b_stops_learning() {
    let moves: &[MoveId] = &[HM_MOVE, GROWL, QUICK_ATTACK, HARDEN];
    let mut w = teaching(&[(NORMAL, moves)], TM);
    keys(&mut w, &[Buttons::A, Buttons::A, Buttons::A, Buttons::A]);
    settle(&mut w);
    keys(&mut w, &[Buttons::A]);
    // Still in the summary, the HM refused until the next press.
    assert_eq!(summary(&w).move_cursor, 0);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(summary(&w).move_cursor, 0);
    keys(&mut w, &[Buttons::B]);
    settle(&mut w);
    // "Stop learning?" prints (no press: a question follows), then the yes/no.
    assert_eq!(party(&w).prompt(), Prompt::YesNo { cursor: 0 });
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message, "did not learn");
    keys(&mut w, &[Buttons::A]);
    until_closed(&mut w);
    let want: Vec<_> = moves.iter().map(|&m| Some(m)).collect();
    assert_eq!(&w.s.party.get(0).unwrap().moves[..], &want[..]);
    assert_eq!(w.s.bag.count(TM), 1);
}

/// "Replace?" no → "stop learning?" no → back to "replace?".
#[test]
fn declining_to_stop_asks_to_replace_again() {
    let mut w = teaching(&[(NORMAL, FOUR)], TM);
    keys(&mut w, &[Buttons::A, Buttons::A, Buttons::DOWN, Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message);
    keys(&mut w, &[Buttons::A, Buttons::DOWN, Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::YesNo { cursor: 0 });
}

/// A mon that can't learn the TM, and one that already knows it: a message, then the field;
/// the TM kept.
#[test]
fn an_incompatible_or_knowing_mon_is_refused() {
    for mons in [
        &[(NO_TM, &[TACKLE][..])][..],
        &[(NORMAL, &[TM_MOVE][..])][..],
    ] {
        let mut w = teaching(mons, TM);
        let before = w.s.party.get(0).unwrap().moves;
        keys(&mut w, &[Buttons::A]);
        assert_eq!(party(&w).prompt(), Prompt::Message);
        keys(&mut w, &[Buttons::A]);
        until_closed(&mut w);
        assert_eq!(w.s.party.get(0).unwrap().moves, before);
        assert_eq!(w.s.bag.count(TM), 1);
    }
}

/// B while picking cancels the teach.
#[test]
fn b_cancels_the_teach() {
    let mut w = teaching(&[(NORMAL, &[TACKLE])], TM);
    keys(&mut w, &[Buttons::B]);
    until_closed(&mut w);
    assert_eq!(w.s.party.get(0).unwrap().moves[1], None);
    assert_eq!(w.s.bag.count(TM), 1);
}

/// SWITCH (Gen3 `CursorCb_Switch` → `SwitchSelectedMons`) reorders the party.
#[test]
fn switch_reorders_the_party() {
    let mut w = field_party(&[(NORMAL, &[TACKLE]), (NO_TM, &[GROWL])], PartyMode::Field);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Actions { slot: 0 });
    assert_eq!(
        party(&w).actions(),
        &[
            Action::Summary,
            Action::Switch,
            Action::Item,
            Action::Cancel
        ]
    );
    keys(&mut w, &[Buttons::DOWN, Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Switch { from: 0 });
    keys(&mut w, &[Buttons::DOWN, Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: 1 });
    assert_eq!(w.s.party.get(0).unwrap().species, NO_TM);
    assert_eq!(w.s.party.get(1).unwrap().species, NORMAL);
    keys(&mut w, &[Buttons::B]);
    until_closed(&mut w);
}

/// A known field move whose gate passes is listed after SUMMARY (Gen3
/// `SetPartyMonFieldSelectionActions`); choosing it reports (Milestone 10 runs it).
#[test]
fn field_moves_are_listed_after_summary() {
    let mut w = field_party(&[(NORMAL, &[HM_MOVE])], PartyMode::Field);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(
        party(&w).actions(),
        &[
            Action::Summary,
            Action::FieldMove(FieldMove::RockSmash),
            Action::Item,
            Action::Cancel
        ]
    );
    keys(&mut w, &[Buttons::DOWN, Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: 0 });
    assert!(w.log().lines().any(|l| l.contains("field move")));
}

/// SUMMARY: the info page, LEFT/RIGHT pages, DOWN the next mon, B back with the cursor there.
#[test]
fn the_summary_pages_and_returns_to_the_mon_shown() {
    let mut w = field_party(&[(NORMAL, &[TACKLE]), (NO_TM, &[GROWL])], PartyMode::Field);
    keys(&mut w, &[Buttons::A, Buttons::A]);
    settle(&mut w);
    assert_eq!(summary(&w).page(), Page::Info);
    keys(&mut w, &[Buttons::RIGHT]);
    assert_eq!(summary(&w).page(), Page::Skills);
    keys(&mut w, &[Buttons::RIGHT]);
    assert_eq!(summary(&w).page(), Page::Moves);
    keys(&mut w, &[Buttons::LEFT, Buttons::DOWN]);
    assert_eq!(summary(&w).slot, 1);
    keys(&mut w, &[Buttons::B]);
    settle(&mut w);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: 1 });
}

/// In battle: SHIFT / SUMMARY / CANCEL; SHIFT on the active mon is refused, on another it
/// answers its slot.
#[test]
fn battle_mode_shifts_only_a_benched_mon() {
    let mons: &[(SpeciesId, &[MoveId])] = &[(NORMAL, &[TACKLE]), (NO_TM, &[GROWL])];
    let mut w = field_party(mons, PartyMode::Battle { forced: false });
    keys(&mut w, &[Buttons::A]);
    assert_eq!(
        party(&w).actions(),
        &[Action::Shift, Action::Summary, Action::Cancel]
    );
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message, "already in battle");
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: 0 });
    keys(&mut w, &[Buttons::DOWN, Buttons::A, Buttons::A]);
    until_closed(&mut w);
}
