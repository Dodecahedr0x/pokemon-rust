//! A delivered Wonder Card runs the script it points at (`trywondercardscript`).

use super::*;
use crate::battle::test_data::WATER;
use crate::script::NO_WONDER_CARD_SCRIPT;
use crate::world::{MEVENT_STATUS_SUCCESS, WonderCard};

const CARD_SCRIPT: u16 = 40;
const CONTINUED: VarId = VarId(0x4050);

fn card() -> WonderCard {
    WonderCard {
        flag_id: 1000,
        icon_species: 25,
        id_number: 1,
        card_type: 0,
        bg_type: 0,
        send_type: 0,
        max_stamps: 0,
    }
}

/// The content dispatch (`data::script`): script `CARD_SCRIPT` is the card's gift script.
fn dispatch(id: u16, ctx: Ctx) -> Script {
    match id {
        CARD_SCRIPT => Box::pin(card_script(ctx)),
        _ => Box::pin(async {}),
    }
}

async fn card_script(ctx: Ctx) {
    ctx.set_var(VAR_RESULT, 1).await;
    ctx.set_mystery_event_status(MEVENT_STATUS_SUCCESS).await;
    ctx.set_modern_fateful_encounter(0).await;
}

/// What `trywondercardscript` lowers to, aside from `Flow::End` (the card script returns).
async fn try_wonder_card(ctx: Ctx) {
    if let Some(id) = ctx.wonder_card_script().await {
        dispatch(id.0, ctx.clone()).await;
    }
    ctx.set_var(CONTINUED, 9).await;
}

fn run_script(w: &mut World, script: impl FnOnce(Ctx) -> Script) {
    let mut f = Frame::default();
    let mut runner = Runner::new();
    runner.start(script(runner.ctx()));
    let mut host = Host {
        game: &Game,
        s: &mut w.s,
        f: &mut f,
        immediate: false,
    };
    assert!(!runner.step(&mut host), "the script finished");
}

#[test]
fn a_delivered_wonder_card_runs_its_script() {
    let mut w = started();
    assert!(w.deliver_wonder_card(card(), ScriptId(CARD_SCRIPT)));
    assert_eq!(w.mystery_event_status(), 0);
    run_script(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 5).await;
            try_wonder_card(ctx).await;
        })
    });
    assert_eq!(w.var(VAR_RESULT), 1, "the card's script ran");
    assert_eq!(
        w.var(CONTINUED),
        9,
        "the caller continued after the card script"
    );
    assert_eq!(w.mystery_event_status(), MEVENT_STATUS_SUCCESS);
    let mon = w.party().get(0).expect("the gift mon");
    assert!(mon.modern_fateful, "setmodernfatefulencounter set the bit");
}

#[test]
fn no_card_and_an_invalid_card_do_not_run_a_script() {
    let mut w = started();
    run_script(&mut w, |ctx| Box::pin(try_wonder_card(ctx)));
    assert_eq!(w.var(VAR_RESULT), 0);
    assert_eq!(w.var(CONTINUED), 9);
    assert_eq!(w.mystery_event_status(), 0);
    assert!(w.party().is_empty());

    let mut bad = card();
    bad.flag_id = 0;
    assert!(!w.deliver_wonder_card(bad, ScriptId(CARD_SCRIPT)));
    assert!(!w.deliver_wonder_card(card(), ScriptId(NO_WONDER_CARD_SCRIPT)));
    bad = card();
    bad.card_type = 9;
    assert!(!w.deliver_wonder_card(bad, ScriptId(CARD_SCRIPT)));

    assert!(w.deliver_wonder_card(card(), ScriptId(CARD_SCRIPT)));
    bad.flag_id = 0;
    assert!(
        !w.deliver_wonder_card(bad, ScriptId(1)),
        "a bad card leaves the saved one"
    );
    run_script(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 5).await;
            ctx.set_modern_fateful_encounter(9).await;
            try_wonder_card(ctx).await;
        })
    });
    assert_eq!(w.var(VAR_RESULT), 1);
    assert!(w.party().get(0).unwrap().modern_fateful);
}
