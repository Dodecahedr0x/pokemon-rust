//! The naming screen (Gen3 `naming_screen.c`): a typed name is stored.

use super::*;
use crate::battle::test_data::WATER;
use crate::mon::NAME_LEN;
use crate::ui::{NamingKind, ScreenKind};
use alloc::vec::Vec;

/// Gen3 charmap `'A'`.
const CHAR_A: u8 = 0xBB;
const EOS: u8 = 0xFF;

fn nickname_a() -> [u8; NAME_LEN] {
    let mut name = [EOS; NAME_LEN];
    name[0] = CHAR_A;
    name
}

fn give_one(w: &mut World) {
    start(w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 5).await;
        })
    });
    until_done(w, 8);
}

fn open_naming(w: &mut World, kind: NamingKind) {
    let mut f = Frame::default();
    crate::world::screens::open(&mut w.s, &Game, ScreenKind::Naming(kind), false, &mut f);
}

/// Steps until the naming screen has faded in and is taking input.
fn until_naming_accepts_input(w: &mut World) {
    for _ in 0..80 {
        w.step(&Game, Buttons::NONE);
        if w.screens_open() == 0 {
            panic!("naming screen closed before it accepted a name");
        }
    }
}

fn confirm_letter_a(w: &mut World) {
    until_naming_accepts_input(w);
    // Upper-case page opens with the cursor on A. A types it, START moves to OK, A confirms.
    tap(w, Buttons::A, 2);
    tap(w, Buttons::START, 2);
    tap(w, Buttons::A, 2);
    for _ in 0..80 {
        w.step(&Game, Buttons::NONE);
        if w.screens_open() == 0 {
            return;
        }
    }
    panic!("naming screen did not close");
}

fn name_of(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().copied().take_while(|&c| c != EOS).collect()
}

#[test]
fn naming_a_pokemon_stores_the_nickname_on_the_party() {
    let mut w = started();
    give_one(&mut w);
    let before = w.party().get(0).unwrap().nickname;
    open_naming(&mut w, NamingKind::Mon(0));
    confirm_letter_a(&mut w);
    let nick = w.party().get(0).unwrap().nickname;
    assert_ne!(nick, before, "the species name is replaced");
    assert_eq!(nick, nickname_a());
    assert!(
        !w.events()
            .iter()
            .any(|e| e == &Event::Report(Report::Unsupported("naming screen")))
    );
}

#[test]
fn the_player_the_rival_and_a_box_take_gen3_length_limits() {
    let mut player = started();
    open_naming(&mut player, NamingKind::Player);
    // Seven letters fill the player name and move to OK; the eighth press confirms.
    type_a(&mut player, 8);
    assert_eq!(name_of(player.player_name()), vec![CHAR_A; 7]);

    let mut rival = started();
    open_naming(&mut rival, NamingKind::Rival);
    confirm_letter_a(&mut rival);
    assert_eq!(rival.rival_name().map(|n| name_of(n)), Some(vec![CHAR_A]));

    let mut pc = started();
    open_naming(&mut pc, NamingKind::Box(0));
    // Eight letters fill a box name; the ninth confirms.
    type_a(&mut pc, 9);
    assert_eq!(name_of(&pc.pc().box_name(0)), vec![CHAR_A; 8]);
}

#[test]
fn the_symbols_page_stores_a_gen3_digit() {
    let mut w = started();
    give_one(&mut w);
    open_naming(&mut w, NamingKind::Mon(0));
    until_naming_accepts_input(&mut w);
    // Upper, then lower, then the symbols page. The cursor stays on '0' (0xA1).
    tap(&mut w, Buttons::SELECT, 2);
    tap(&mut w, Buttons::SELECT, 2);
    tap(&mut w, Buttons::A, 2);
    tap(&mut w, Buttons::START, 2);
    tap(&mut w, Buttons::A, 2);
    for _ in 0..80 {
        w.step(&Game, Buttons::NONE);
        if w.screens_open() == 0 {
            break;
        }
    }
    assert_eq!(w.party().get(0).unwrap().nickname[0], 0xA1);
}

/// Types `n` A's. A full name confirms on the press that follows the last character.
fn type_a(w: &mut World, n: u32) {
    until_naming_accepts_input(w);
    for _ in 0..n {
        tap(w, Buttons::A, 2);
    }
    for _ in 0..80 {
        if w.screens_open() == 0 {
            return;
        }
        w.step(&Game, Buttons::NONE);
    }
    panic!("naming screen did not close");
}
