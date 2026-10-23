//! Wave C field effects and object commands: `dofieldeffect` / `waitfieldeffect` timing,
//! `setfieldeffectargument` and `copyobjectxytoperm`, on the synthetic game.

use super::*;
use crate::hash::Fnv64;
use crate::script::Request;

#[test]
fn wait_field_effect_waits_for_its_frames() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.field_effect(25, 10).await;
            ctx.wait_field_effect(25).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    // Frame 1 starts it; the countdown runs on frames 1..=10, the wait sees it gone on 11.
    run(&mut w, Buttons::NONE, 9);
    assert!(!w.flag(FLAG_DONE));
    assert!(w.s.field_effects.contains(25));
    until_done(&mut w, 5);
    assert!(w.flag(FLAG_DONE));
    assert!(!w.s.field_effects.contains(25));
}

#[test]
fn waiting_on_an_effect_not_running_answers_at_once() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.field_effect(30, 50).await;
            // Another id, and a zero-length start: neither waits.
            ctx.field_effect(31, 0).await;
            ctx.wait_field_effect(31).await;
            ctx.wait_field_effect(7).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    // Each wait checks from the next frame on (Gen3 `ScrCmd_waitfieldeffect` returns TRUE).
    run(&mut w, Buttons::NONE, 3);
    assert!(w.flag(FLAG_DONE));
    // The effect keeps running after the script (Gen3: its sprite outlives the script).
    assert!(w.s.field_effects.contains(30));
    run(&mut w, Buttons::NONE, 48);
    assert!(!w.s.field_effects.contains(30));
}

#[test]
fn field_effect_arguments_are_stored_and_out_of_range_ones_ignored() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.field_effect_arg(0, 12).await;
            ctx.field_effect_arg(2, -1).await;
            ctx.field_effect_arg(8, 99).await;
        })
    });
    until_done(&mut w, 5);
    assert_eq!(w.s.field_effects.args[..3], [12, 0, -1]);
}

#[test]
fn copy_object_pos_to_template_keeps_the_live_cell() {
    let mut w = started();
    assert_eq!(w.objects().get(2).unwrap().pos(), (7, 7));
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.set_object_pos(2, 4, 9).await;
            ctx.copy_object_pos_to_template(2).await;
            // No object 0x30 and the player have no template: nothing happens.
            ctx.copy_object_pos_to_template(0x30).await;
            ctx.copy_object_pos_to_template(PLAYER).await;
        })
    });
    until_done(&mut w, 5);
    let t = w.s.template(2).unwrap();
    assert_eq!((t.x, t.y), (4, 9));
}

#[test]
fn field_requests_hash_apart() {
    let h = |r: Request| {
        let mut h = Fnv64::new();
        r.hash_into(&mut h);
        h.finish()
    };
    let all = [
        h(Request::FieldEffectArg { n: 0, value: 1 }),
        h(Request::FieldEffect { id: 1, frames: 0 }),
        h(Request::WaitFieldEffect(1)),
        h(Request::CopyObjectPosToTemplate(1)),
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
