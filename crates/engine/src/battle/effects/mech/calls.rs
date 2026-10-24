//! The moves that use another move: Metronome, Mirror Move, Sleep Talk, Assist.

use super::super::{Canceler, abilities, later};
use super::{anim, say};
use crate::battle::core::{Battle, Env, HitResult, MoveUse};
use crate::battle::{BattleEvent, Str};
use crate::data::MoveId;
use crate::mon::{MoveEffect, Status};

/// `sMovesForbiddenToCopy` by effect (Metronome, Struggle, Sketch, Mimic, Counter, Mirror
/// Coat, Protect / Detect, Endure, Destiny Bond, Sleep Talk, Thief / Covet, Follow Me, Snatch,
/// Helping Hand, Trick, Focus Punch).
fn forbidden(env: &Env, m: MoveId) -> bool {
    use MoveEffect as E;
    m == env.t.struggle
        || matches!(
            env.t.move_info(m).effect,
            E::Metronome
                | E::Sketch
                | E::Mimic
                | E::Counter
                | E::MirrorCoat
                | E::Protect
                | E::Endure
                | E::DestinyBond
                | E::SleepTalk
                | E::Thief
                | E::FollowMe
                | E::Snatch
                | E::HelpingHand
                | E::Trick
                | E::FocusPunch
        )
}

/// `IsInvalidForSleepTalkOrAssist`.
fn invalid_for_call(env: &Env, m: MoveId) -> bool {
    matches!(
        env.t.move_info(m).effect,
        MoveEffect::SleepTalk | MoveEffect::Assist | MoveEffect::MirrorMove | MoveEffect::Metronome
    )
}

/// The called move's script from its `attackcanceler` (the target's Soundproof, Magic Bounce,
/// Lightning Rod … run again); `slot` is where its `ppreduce` takes the PP.
fn call(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    mov: MoveId,
    slot: Option<u8>,
) -> Option<HitResult> {
    let called = b.move_use(env, u.attacker, mov, slot);
    if let Some(c) = &mut b.fx.cur {
        c.target = called.target;
        c.called = Some(mov);
        c.physical = super::physical_of(&called);
    }
    let c = match later::before_move(b, env, &called) {
        Some(c) => c,
        None => abilities::moves_block(b, env, &called),
    };
    match c {
        Canceler::Use => b.execute(env, &called),
        Canceler::Done(_, hit) => hit,
        Canceler::Skip => None,
    }
}

/// `BattleScript_EffectMetronome`: the attack string (no PP: the called move's `ppreduce` pays
/// Metronome's), the animation, then `(Random() & 0x1FF) + 1` until a move that exists and may
/// be copied.
pub(super) fn metronome(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    env.out
        .push(BattleEvent::Msg(u.msg(Str::UsedMove, u.attacker)));
    anim(env, u);
    let count = env.t.moves.len() as u16;
    let mov = loop {
        let m = (env.rng.random() & 0x1FF) + 1;
        if m < count && !forbidden(env, MoveId(m)) {
            break MoveId(m);
        }
    };
    call(b, env, u, mov, u.slot)
}

/// `BattleScript_EffectMirrorMove` + `trymirrormove`: the last move that targeted the user
/// (`lastTakenMove`); else "The MIRROR MOVE failed!" (with PP).
pub(super) fn mirror_move(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    env.out
        .push(BattleEvent::Msg(u.msg(Str::UsedMove, u.attacker)));
    match b.battler(u.attacker).fx.rec.last_taken {
        Some(m) => call(b, env, u, m, u.slot),
        None => {
            super::hits::pay_pp(b, u);
            b.no_effect = true;
            say(env, u, Str::MirrorMoveFailed, u.attacker);
            None
        }
    }
}

/// `BattleScript_EffectSleepTalk` + `trychoosesleeptalkmove`: while asleep, a random other move
/// (not a two-turn move, Focus Punch, Uproar nor a calling move; with PP no matter, as
/// `CheckMoveLimitations(~MOVE_LIMITATION_PP)`), without PP.
pub(super) fn sleep_talk(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    let a = u.attacker;
    if !matches!(b.battler(a).mon.status, Status::Sleep(_)) {
        b.attack_string(env, u);
        b.fail(env, u);
        return None;
    }
    say(env, u, Str::PkmnFastAsleep, a);
    b.attack_string(env, u);
    let m = *b.battler(a);
    let usable = |i: usize| {
        let Some(mv) = m.mon.moves[i] else {
            return false;
        };
        let e = env.t.move_info(mv).effect;
        !(invalid_for_call(env, mv)
            || matches!(
                e,
                MoveEffect::FocusPunch
                    | MoveEffect::Uproar
                    | MoveEffect::TwoTurn(_)
                    | MoveEffect::SemiInvulnerable(_)
                    | MoveEffect::Bide
            )
            || super::turns::limited_except_pp(b, env.t, a, i))
    };
    let usable: [bool; 4] = core::array::from_fn(usable);
    if !usable.iter().any(|&x| x) {
        b.fail(env, u);
        return None;
    }
    let slot = loop {
        let i = (env.rng.random() % 4) as usize;
        if usable[i] {
            break i;
        }
    };
    anim(env, u);
    if let Some(c) = &mut b.fx.cur {
        c.sleep_talk = true;
    }
    let mov = m.mon.moves[slot]?;
    call(b, env, u, mov, None)
}

/// `BattleScript_EffectAssist` + `assistattackselect`: a random move of the other party mons
/// (`(Random() & 0xFF) * count >> 8`); the called move's `ppreduce` pays Assist's PP.
pub(super) fn assist(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    env.out
        .push(BattleEvent::Msg(u.msg(Str::UsedMove, u.attacker)));
    let a = u.attacker;
    let me = b.battler(a).mon.party as usize;
    let mut valid: alloc::vec::Vec<MoveId> = alloc::vec::Vec::new();
    for (i, p) in b.parties[a.s()].iter().enumerate() {
        if i == me {
            continue;
        }
        for m in p.moves.iter().flatten() {
            if !invalid_for_call(env, *m) && !forbidden(env, *m) {
                valid.push(*m);
            }
        }
    }
    if valid.is_empty() {
        super::hits::pay_pp(b, u);
        b.fail(env, u);
        return None;
    }
    let i = ((env.rng.random() & 0xFF) as usize * valid.len()) >> 8;
    anim(env, u);
    call(b, env, u, valid[i], u.slot)
}
