//! Milestone 9 Wave 0: the route-neutral behaviours it implements (the yes/no cursor start,
//! the text speed, Pokédex recording) and the hash rule of the new state.

use super::battles::{mash, walk_grass_until_battle};
use super::*;
use crate::battle::test_data::WATER;
use crate::options::TextSpeed;
use crate::pokedex::DexFlag;

#[test]
fn yes_no_default_no_starts_on_no() {
    for (default_no, want) in [(false, 11), (true, 10)] {
        let mut w = started();
        start(&mut w, move |ctx| {
            Box::pin(async move {
                ctx.message(TextId(0)).await;
                let yes = if default_no {
                    ctx.yes_no_default_no().await
                } else {
                    ctx.yes_no().await
                };
                ctx.set_var(VAR_RESULT, yes as u16 + 10).await;
            })
        });
        run(&mut w, Buttons::NONE, 5);
        assert!(w.text().is_yes_no_active());
        tap(&mut w, Buttons::A, 3);
        until_done(&mut w, 50);
        assert_eq!(w.var(VAR_RESULT), want, "default_no {default_no}");
    }
}

/// Frames the player-name message (no page break) takes to print at `speed`.
fn print_frames(speed: TextSpeed) -> u32 {
    let mut w = started();
    w.s.options.text_speed = speed;
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.message(PLAYER_TEXT).await;
        })
    });
    w.set_player_name(&[1, 2, 3, 1, 2, 3, 0xFF]);
    let mut started = None;
    for n in 1..2000 {
        run(&mut w, Buttons::NONE, 1);
        match (started, w.text().is_printing()) {
            (None, true) => started = Some(n),
            (Some(s), false) => return n - s,
            _ => {}
        }
    }
    panic!("never printed");
}

/// Gen3 `sTextSpeedFrameDelays`: SLOW 8, MID 4 (the default, today's speed), FAST 1 frame
/// per character.
#[test]
fn text_speed_option_sets_the_print_rate() {
    assert_eq!(Options::default().text_speed, TextSpeed::Mid);
    let (slow, mid, fast) = (
        print_frames(TextSpeed::Slow),
        print_frames(TextSpeed::Mid),
        print_frames(TextSpeed::Fast),
    );
    assert!(fast < mid && mid < slow, "{fast} {mid} {slow}");
}

#[test]
fn givemon_records_seen_and_caught() {
    let mut w = started();
    assert!(w.pokedex().is_empty());
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 5).await;
        })
    });
    until_done(&mut w, 3);
    assert!(w.pokedex().get(DexFlag::Seen, WATER));
    assert!(w.pokedex().get(DexFlag::Caught, WATER));
    assert_eq!(w.pokedex().count(DexFlag::Caught), 1);
}

#[test]
fn battles_record_the_opponent_as_seen() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_trainer(TRAINER, TextId(0)).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(w.in_battle());
    let foe = w
        .battle()
        .unwrap()
        .1
        .party(crate::battle::Side::Opponent)
        .get(0)
        .unwrap()
        .species;
    run(&mut w, Buttons::NONE, 2);
    assert!(w.pokedex().get(DexFlag::Seen, foe));
    assert!(!w.pokedex().get(DexFlag::Caught, foe) || foe == WATER);
    mash(&mut w, 20_000, |w| !w.script_running());

    // A wild encounter on the route.
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    walk(&mut w, Dir::Up, 6);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
        })
    });
    until_done(&mut w, 3);
    walk_grass_until_battle(&mut w);
    let foe = w
        .battle()
        .unwrap()
        .1
        .party(crate::battle::Side::Opponent)
        .get(0)
        .unwrap()
        .species;
    run(&mut w, Buttons::NONE, 2);
    assert!(w.pokedex().get(DexFlag::Seen, foe));
}

/// The new state is hashed only away from a new game's values (so routes that never touch it
/// keep their hashes).
#[test]
fn new_state_is_hashed_only_when_not_default() {
    let mut w = started();
    let h = w.hash();
    w.s.coins = 5;
    assert_ne!(w.hash(), h);
    w.s.coins = 0;
    w.s.options.text_speed = TextSpeed::Fast;
    assert_ne!(w.hash(), h);
    w.s.options = Options::default();
    w.s.registered_item = Some(crate::data::ItemId(1));
    assert_ne!(w.hash(), h);
    w.s.registered_item = None;
    assert_eq!(w.hash(), h);
}

/// A stub screen opened over the field reports itself and closes on its first running frame
/// (after the hosting's fade in), handing the field back; the stack hashes while open and the
/// world is not capturable then.
#[test]
fn a_stub_screen_closes_back_to_the_field() {
    use crate::ui::{NamingKind, ScreenKind};
    let mut w = started();
    let idle = w.hash();
    let mut f = Frame::default();
    crate::world::screens::open(
        &mut w.s,
        &Game,
        ScreenKind::Naming(NamingKind::Player),
        false,
        &mut f,
    );
    assert_eq!(w.screens_open(), 1);
    assert_ne!(w.hash(), idle, "an open screen is hashed");
    assert_eq!(
        w.capturable(),
        Err(crate::checkpoint::NotCapturable::Screen)
    );
    let (mut reported, mut closed) = (false, false);
    for _ in 0..200 {
        w.step(&Game, Buttons::NONE);
        let events = w.events();
        reported |= events.contains(&Event::Report(Report::Unsupported("naming screen")));
        closed |= events.contains(&Event::ScreenClosed);
        if w.screens_open() == 0 {
            break;
        }
    }
    assert!(w.field_active());
    assert!(reported && closed);
    for _ in 0..40 {
        w.step(&Game, Buttons::NONE);
    }
    assert_eq!(w.capturable(), Ok(()));
}
