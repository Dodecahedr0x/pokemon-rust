//! `Cmd_handleballthrow` for a Poké Ball in a wild battle.

use crate::battle::calc;
use crate::battle::core::{Battle, Env, LONG, msg};
use crate::battle::{BattleEvent, BattleOutcome, Msg, Side, Str};

/// The odds (`calc::catch_odds`) and shakes (`calc::catch_shakes`, one `Random()` per shake
/// below 255 odds), the `Ball` event, then:
/// - caught (`BattleScript_SuccessBallThrow`): "Gotcha!", the mon joins the party as it is
///   (HP, status, PP); with a full party it is dropped into `caught_without_room` (no PC);
/// - escaped (`BattleScript_ShakeBallThrow`): `gBallEscapeStringIds[shakes]`.
pub(super) fn throw(b: &mut Battle, env: &mut Env) {
    let (p, o) = (Side::Player, Side::Opponent);
    let foe = b.battler(o).mon;
    let rate = env.t.species_info(foe.species).catch_rate;
    let odds = calc::catch_odds(rate, foe.hp, foe.max_hp, foe.status);
    let shakes = calc::catch_shakes(odds, || env.rng.random());
    let caught = shakes >= 4;
    env.out.push(BattleEvent::Ball { shakes, caught });
    if !caught {
        let id = match shakes {
            0 => Str::PkmnBrokeFree,
            1 => Str::ItAppearedCaught,
            2 => Str::AarghAlmostHadIt,
            _ => Str::ShootSoClose,
        };
        env.out.push(BattleEvent::Msg(Msg {
            wait: LONG,
            ..msg(id, p, o, o)
        }));
        return;
    }
    // STRINGID_GOTCHAPKMNCAUGHT names the target (`B_DEF_NAME`); its own codes wait.
    env.out.push(BattleEvent::Msg(msg(Str::Gotcha, p, o, o)));
    b.write_back(o);
    if let Some(mon) = b.parties[o as usize].get(foe.party as usize).copied()
        && let Err(mon) = b.parties[p as usize].push(mon)
    {
        b.caught_without_room = Some(mon);
    }
    b.finish(env, BattleOutcome::Caught);
}
