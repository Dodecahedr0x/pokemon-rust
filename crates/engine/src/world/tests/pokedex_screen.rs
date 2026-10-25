//! START → POKéDEX: the list, the detail page and the rating, in Hoenn and National mode.

use super::m9_k::{
    FLAG_NATIONAL_DEX, FLAG_POKEDEX_MENU, UiGame, VAR_NATIONAL_DEX, ui_run, ui_tap, ui_world,
};
use super::*;
use crate::pokedex::DexFlag;
use crate::ui::kit::Phase;
use crate::ui::pokedex::{DexMode, Page, PokedexScreen, Rating};
use crate::ui::{ScreenKind, ScreenState};

const FIRE: SpeciesId = crate::battle::test_data::FIRE;
const WATER: SpeciesId = crate::battle::test_data::WATER;
const BUG: SpeciesId = crate::battle::test_data::BUG;

fn dex(w: &World) -> &PokedexScreen {
    match w.screens().and_then(|s| s.top()) {
        Some((ScreenKind::Pokedex, ScreenState::Pokedex(p))) => p,
        other => panic!("expected the Pokédex, found {other:?}"),
    }
}

fn running(w: &World) -> bool {
    w.screens().is_some_and(|s| s.host.phase == Phase::Running)
}

/// START, then A on POKéDEX (the first shown entry), through the fade.
fn open_pokedex(w: &mut World) {
    ui_tap(w, Buttons::START, 2);
    ui_run(w, Buttons::A, 1);
    for _ in 0..48 {
        ui_run(w, Buttons::NONE, 1);
        if running(w)
            && matches!(
                w.screens().and_then(|s| s.top()),
                Some((ScreenKind::Pokedex, _))
            )
        {
            return;
        }
    }
    panic!("the Pokédex never opened");
}

fn species(w: &World) -> Option<SpeciesId> {
    dex(w).species(&UiGame)
}

fn caught(w: &World) -> bool {
    species(w).is_some_and(|s| w.pokedex().get(DexFlag::Caught, s))
}

fn counts(w: &World) -> (u16, u16) {
    let mode = dex(w).mode();
    crate::ui::pokedex::counts(&UiGame, w.pokedex(), mode)
}

fn rating(w: &World) -> Rating {
    let mode = dex(w).mode();
    crate::ui::pokedex::rating(&UiGame, w.pokedex(), mode)
}

/// Hoenn order is FIRE, WATER, NORMAL. National order is WATER, NORMAL, FIRE, BUG.
#[test]
fn start_pokedex_finds_a_caught_species_and_opens_detail_and_rating() {
    let mut w = ui_world(&[]);
    w.s.vars.set_flag(FLAG_POKEDEX_MENU, true);
    w.s.vars.set_flag(FLAG_NATIONAL_DEX, true);
    w.s.vars
        .set_var(VAR_NATIONAL_DEX, crate::ui::pokedex::NATIONAL_DEX_MAGIC);
    w.s.pokedex.set_caught(WATER);
    w.s.pokedex
        .set(DexFlag::Seen, crate::battle::test_data::NORMAL);

    open_pokedex(&mut w);
    assert_eq!(dex(&w).page(), Page::List);
    assert_eq!(dex(&w).mode(), DexMode::Hoenn);
    // Unseen first entry: A does not open the detail page.
    assert_eq!(species(&w), Some(FIRE));
    assert!(!caught(&w));
    ui_tap(&mut w, Buttons::A, 2);
    assert_eq!(dex(&w).page(), Page::List);

    ui_tap(&mut w, Buttons::DOWN, 2);
    assert_eq!(species(&w), Some(WATER));
    assert!(caught(&w));

    ui_tap(&mut w, Buttons::A, 2);
    assert_eq!(dex(&w).page(), Page::Detail);
    assert_eq!(species(&w), Some(WATER));
    assert!(caught(&w));

    ui_tap(&mut w, Buttons::B, 2);
    assert_eq!(dex(&w).page(), Page::List);
    assert_eq!(species(&w), Some(WATER));

    ui_tap(&mut w, Buttons::START, 2);
    assert_eq!(dex(&w).page(), Page::Rating);
    assert_eq!(dex(&w).mode(), DexMode::Hoenn);
    let (seen, n) = counts(&w);
    assert_eq!(n, 1);
    assert!(seen >= n);
    assert_eq!(rating(&w), Rating::Band(0));

    ui_tap(&mut w, Buttons::B, 2);
    ui_tap(&mut w, Buttons::SELECT, 2);
    assert_eq!(dex(&w).page(), Page::List);
    assert_eq!(dex(&w).mode(), DexMode::National);
    assert_eq!(species(&w), Some(WATER));
    assert!(caught(&w));
    assert_eq!(counts(&w).1, 1);

    ui_tap(&mut w, Buttons::START, 2);
    assert_eq!(dex(&w).page(), Page::Rating);
    assert_eq!(dex(&w).mode(), DexMode::National);
    assert_eq!(counts(&w).1, 1);
    assert_eq!(rating(&w), Rating::Band(0));

    // BUG is on the National list only.
    ui_tap(&mut w, Buttons::B, 2);
    for _ in 0..3 {
        ui_tap(&mut w, Buttons::DOWN, 2);
    }
    assert_eq!(species(&w), Some(BUG));
    assert!(!caught(&w));

    assert!(
        !w.log().lines().any(|l| l == "pokedex"),
        "the Pokédex is a screen, not an unsupported report"
    );
}

#[test]
fn select_stays_on_hoenn_until_the_national_dex_is_enabled() {
    let mut w = ui_world(&[]);
    w.s.vars.set_flag(FLAG_POKEDEX_MENU, true);
    w.s.pokedex.set_caught(WATER);
    open_pokedex(&mut w);
    ui_tap(&mut w, Buttons::SELECT, 2);
    assert_eq!(dex(&w).mode(), DexMode::Hoenn);

    w.s.vars.set_flag(FLAG_NATIONAL_DEX, true);
    ui_tap(&mut w, Buttons::SELECT, 2);
    assert_eq!(dex(&w).mode(), DexMode::Hoenn, "the var is still unset");

    w.s.vars
        .set_var(VAR_NATIONAL_DEX, crate::ui::pokedex::NATIONAL_DEX_MAGIC);
    ui_tap(&mut w, Buttons::SELECT, 2);
    assert_eq!(dex(&w).mode(), DexMode::National);
    assert!(caught(&w));
}
