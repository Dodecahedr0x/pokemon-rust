//! Wave F: the choose-half party menu (`PARTY_MENU_TYPE_CHOOSE_HALF`, `party_menu.c`) and the
//! party's backup around a battle fought with part of the party (`SavePlayerParty`,
//! `ReducePlayerPartyToSelectedMons`, `SaveSelectedParty`, `LoadPlayerParty`), as
//! `MossdeepCity_SpaceCenter_2F_EventScript_ChoosePartyForMultiBattle` runs them.

use super::m9_k::{ui_run, ui_tap, ui_world};
use super::*;
use crate::battle::test_data::{NORMAL, TACKLE};
use crate::script::PartyStash;
use crate::ui::party::{Action, CANCEL, CONFIRM, PartyMenu, Prompt, pack_order};
use crate::ui::{PartyMode, ScreenState};

const RESULT: VarId = VarId(0x800D);
const HALF: PartyMode = PartyMode::ChooseHalf { max: 3, min: 1 };

fn party(w: &World) -> &PartyMenu {
    match w.screens().and_then(|s| s.top()).map(|(_, s)| s) {
        Some(ScreenState::Party(p)) => p,
        _ => panic!("the party menu is not on top"),
    }
}

fn keys(w: &mut World, keys: &[Buttons]) {
    for &k in keys {
        ui_tap(w, k, 2);
    }
}

/// Five mons (levels 10..14 to tell them apart), slot 1 fainted; a script that saves the party,
/// opens the choose-half menu, and on a choice reduces the party, waits 30 frames (the battle),
/// saves the chosen mons back and loads the party.
fn world() -> World {
    let one: &[MoveId] = &[TACKLE];
    let mut w = ui_world(&[(NORMAL, one); 5]);
    for i in 0..5 {
        w.s.party.get_mut(i).unwrap().level = 10 + i as u8;
    }
    w.s.party.get_mut(1).unwrap().hp = 0;
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.party_stash(PartyStash::Save).await;
        let chosen = ctx.choose_party_mon(HALF).await;
        ctx.set_var(RESULT, 1 + chosen).await;
        if chosen != 0 {
            ctx.party_stash(PartyStash::Reduce).await;
            ctx.delay(30).await;
            ctx.party_stash(PartyStash::SaveSelected).await;
        }
        ctx.party_stash(PartyStash::Load).await;
    }));
    ui_run(&mut w, Buttons::NONE, 40);
    assert_eq!(party(&w).mode, HALF);
    w
}

fn levels(w: &World) -> Vec<u8> {
    w.s.party.iter().map(|m| m.level).collect()
}

/// A on the slot under the cursor, then `a` from its actions.
fn act(w: &mut World, a: Action) {
    keys(w, &[Buttons::A]);
    let i = party(w)
        .actions()
        .iter()
        .position(|&x| x == a)
        .expect("listed");
    for _ in 0..i {
        keys(w, &[Buttons::DOWN]);
    }
    keys(w, &[Buttons::A]);
}

#[test]
fn the_order_packs_a_nibble_per_entry() {
    assert_eq!(pack_order(&[1, 3, 4, 0]), 0x431);
}

#[test]
fn entering_three_moves_to_confirm_and_the_battle_party_goes_back_in_place() {
    let mut w = world();
    assert_eq!(party(&w).actions(), [] as [Action; 0]);
    keys(&mut w, &[Buttons::A]);
    assert_eq!(
        party(&w).actions(),
        [Action::Enter, Action::Summary, Action::Cancel]
    );
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).order(), [1, 0, 0, 0]);
    // The fainted mon: SUMMARY only.
    keys(&mut w, &[Buttons::DOWN, Buttons::A]);
    assert_eq!(party(&w).actions(), [Action::Summary, Action::Cancel]);
    keys(&mut w, &[Buttons::B]);
    // Slot 3, then slot 2: the order is the entry order.
    keys(&mut w, &[Buttons::DOWN, Buttons::DOWN]);
    act(&mut w, Action::Enter);
    keys(&mut w, &[Buttons::UP]);
    act(&mut w, Action::Enter);
    assert_eq!(party(&w).order(), [1, 4, 3, 0]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: CONFIRM });
    keys(&mut w, &[Buttons::A]);
    ui_run(&mut w, Buttons::NONE, 40);
    assert_eq!(w.var(RESULT), 2);
    // Reduced: the chosen three in order.
    assert_eq!(levels(&w), [10, 13, 12]);
    // The battle: the third chosen (slot 2) levels up.
    w.s.party.get_mut(2).unwrap().level = 50;
    for _ in 0..200 {
        if !w.script_running() {
            break;
        }
        ui_run(&mut w, Buttons::NONE, 1);
    }
    assert!(!w.script_running());
    assert_eq!(levels(&w), [10, 11, 50, 13, 14]);
    assert!(w.s.stash.is_empty());
}

#[test]
fn no_entry_takes_a_mon_out_and_the_later_ones_move_up() {
    let mut w = world();
    act(&mut w, Action::Enter);
    keys(&mut w, &[Buttons::DOWN, Buttons::DOWN]);
    act(&mut w, Action::Enter);
    keys(&mut w, &[Buttons::DOWN]);
    act(&mut w, Action::Enter);
    assert_eq!(party(&w).order(), [1, 3, 4, 0]);
    keys(&mut w, &[Buttons::UP; 5]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: 0 });
    act(&mut w, Action::NoEntry);
    assert_eq!(party(&w).order(), [3, 4, 0, 0]);
}

#[test]
fn confirm_with_nobody_says_no_mon_and_start_jumps_to_confirm() {
    let mut w = world();
    keys(&mut w, &[Buttons::START]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: CONFIRM });
    keys(&mut w, &[Buttons::A]);
    assert_eq!(party(&w).prompt(), Prompt::Message);
    for _ in 0..60 {
        if party(&w).prompt() != Prompt::Message {
            break;
        }
        keys(&mut w, &[Buttons::A]);
    }
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: CONFIRM });
    // DOWN from CONFIRM is CANCEL, then the lead.
    keys(&mut w, &[Buttons::DOWN]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: CANCEL });
    keys(&mut w, &[Buttons::DOWN]);
    assert_eq!(party(&w).prompt(), Prompt::Slot { cursor: 0 });
}

#[test]
fn b_asks_to_cancel_the_battle_and_yes_answers_nothing_chosen() {
    let mut w = world();
    act(&mut w, Action::Enter);
    keys(&mut w, &[Buttons::B]);
    for _ in 0..60 {
        if matches!(party(&w).prompt(), Prompt::YesNo { .. }) {
            break;
        }
        ui_run(&mut w, Buttons::NONE, 1);
    }
    assert_eq!(party(&w).prompt(), Prompt::YesNo { cursor: 0 });
    keys(&mut w, &[Buttons::A]);
    for _ in 0..200 {
        if !w.script_running() {
            break;
        }
        ui_run(&mut w, Buttons::NONE, 1);
    }
    assert_eq!(w.var(RESULT), 1);
    assert_eq!(levels(&w), [10, 11, 12, 13, 14]);
}
