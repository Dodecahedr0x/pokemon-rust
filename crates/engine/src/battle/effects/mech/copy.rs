//! Wave D: Transform, Mimic, Sketch, Recycle, Conversion 2, Snatch and Beat Up, with
//! Snatch's steal in `Cmd_attackcanceler` and `PressurePPLose`.

use super::super::{abilities, items};
use super::{anim, no_acc_check, say, say_with};
use crate::battle::Effectiveness;
use crate::battle::calc;
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, msg};
use crate::battle::{BattleEvent, Buff, Msg, Side, Str};
use crate::data::MoveId;
use crate::mon::{Ability, MoveEffect, MoveFlags, Pokemon, Type};

/// `gChosenMove = MOVE_UNAVAILABLE`.
fn unavailable(b: &mut Battle) {
    if let Some(c) = &mut b.fx.cur {
        c.unavailable = true;
    }
}

/// `gSpecialStatuses.ppNotAffectedByPressure` for the current move.
pub(super) fn no_pressure(b: &mut Battle) {
    if let Some(c) = &mut b.fx.cur {
        c.no_pressure = true;
    }
}

/// `PressurePPLose(target, attacker, move)`: when `target` has Pressure, `attacker` loses one
/// more PP of its move with `effect` (Gen3 looks the move up by id; the effect names it).
pub(in crate::battle::effects) fn pressure_pp_lose(
    b: &mut Battle,
    env: &Env,
    target: Side,
    attacker: Side,
    effect: MoveEffect,
) {
    if b.battler(target).ability != Ability::Pressure {
        return;
    }
    let m = b.battler(attacker);
    let Some(i) =
        (0..4).find(|&i| m.mon.moves[i].is_some_and(|mv| env.t.move_info(mv).effect == effect))
    else {
        return;
    };
    let pp = &mut b.battler_mut(attacker).mon.pp[i];
    *pp = pp.saturating_sub(1);
    // MOVE_IS_PERMANENT: [`Battle::write_back`] skips transformed and mimicked slots.
    b.write_back(attacker);
}

/// `BattleScript_EffectTransform` + `Cmd_transformdataexecution`: fails on a transformed or
/// semi-invulnerable target; else the user takes the target's species, stats (not HP), moves
/// (PP 5 at most), stat stages, ability and types; its gender follows the species with its own
/// personality (`GetBattlerGender`). Disable ends, the PP are not written back.
pub(super) fn transform(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    unavailable(b);
    b.attack_string(env, u);
    let (a, t) = (u.attacker, u.target);
    let tm = *b.battler(t);
    if tm.fx.transformed || tm.fx.semi.is_some() {
        b.fail(env, u);
        return;
    }
    let party = b.battler(a).mon.party as usize;
    let gender = b.parties[a.s()]
        .get(party)
        .map(|p| {
            Pokemon {
                species: tm.mon.species,
                ..*p
            }
            .gender(env.t)
        })
        .unwrap_or(tm.mon.gender);
    let m = b.battler_mut(a);
    m.fx.transformed = true;
    m.fx.disabled = None;
    m.fx.disable_timer = 0;
    m.fx.mimicked = 0;
    m.mon.species = tm.mon.species;
    m.mon.gender = gender;
    m.stats[1..].copy_from_slice(&tm.stats[1..]);
    m.mon.moves = tm.mon.moves;
    for i in 0..4 {
        let pp = tm.mon.moves[i].map_or(0, |mv| env.t.move_info(mv).pp);
        m.mon.pp[i] = pp.min(5);
        m.mon.max_pp[i] = pp;
    }
    m.stages = tm.stages;
    m.ability = tm.ability;
    m.types = tm.types;
    anim(env, u);
    say_with(
        env,
        u,
        Str::PkmnTransformedInto,
        a,
        [Buff::Species(tm.mon.species), Buff::None, Buff::None],
    );
}

/// The slot `gCurrMovePos` (the chosen move's, also for a called move).
fn cur_slot(b: &Battle, u: &MoveUse) -> Option<u8> {
    b.fx.cur.and_then(|c| c.slot).or(u.slot)
}

/// `BattleScript_EffectMimic` + `Cmd_mimicattackcopy`: not behind a substitute, the
/// `NO_ACC_CALC_CHECK_LOCK_ON` check, then the target's last move (not Metronome, Struggle,
/// Sketch nor Mimic: `IsMoveUncopyableByMimic`; not one the user knows; not while
/// transformed) replaces Mimic for the battle, with 5 PP at most.
pub(super) fn mimic(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if super::sub::subbed(b, u) || !no_acc_check(b, u) {
        b.fail(env, u);
        return;
    }
    unavailable(b);
    let me = *b.battler(u.attacker);
    let last = b.battler(u.target).last_move.filter(|&m| {
        m != env.t.struggle
            && !matches!(
                env.t.move_info(m).effect,
                MoveEffect::Metronome | MoveEffect::Sketch | MoveEffect::Mimic
            )
    });
    let (Some(mov), Some(slot)) = (last, cur_slot(b, u)) else {
        b.fail(env, u);
        return;
    };
    if me.fx.transformed || me.mon.moves.contains(&Some(mov)) {
        b.fail(env, u);
        return;
    }
    let pp = env.t.move_info(mov).pp;
    let m = b.battler_mut(u.attacker);
    m.mon.moves[slot as usize] = Some(mov);
    m.mon.pp[slot as usize] = pp.min(5);
    m.mon.max_pp[slot as usize] = pp.min(5);
    m.fx.mimicked |= 1 << slot;
    anim(env, u);
    say_with(
        env,
        u,
        Str::PkmnLearnedMove2,
        u.attacker,
        super::move_buff(mov),
    );
}

/// `BattleScript_EffectSketch` + `Cmd_copymovepermanently`: not behind a substitute nor while
/// transformed; the target's last printed move (`gLastPrintedMoves`: not Struggle nor Sketch,
/// not one the user knows outside its Sketch slots) replaces Sketch for good, with its full
/// base PP. Gen3 writes all four battle moves and PP to the party mon (a Mimic'd move too).
pub(super) fn sketch(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if super::sub::subbed(b, u) {
        b.fail(env, u);
        return;
    }
    unavailable(b);
    let me = *b.battler(u.attacker);
    let printed = b
        .battler(u.target)
        .fx
        .rec
        .printed
        .filter(|&m| m != env.t.struggle && env.t.move_info(m).effect != MoveEffect::Sketch);
    let known = |m: MoveId| {
        me.mon
            .moves
            .iter()
            .flatten()
            .any(|&k| k == m && env.t.move_info(k).effect != MoveEffect::Sketch)
    };
    let (Some(mov), Some(slot), false) = (printed, cur_slot(b, u), me.fx.transformed) else {
        b.fail(env, u);
        return;
    };
    if known(mov) {
        b.fail(env, u);
        return;
    }
    let slot = slot as usize;
    let a = u.attacker;
    let party = me.mon.party as usize;
    let m = b.battler_mut(a);
    m.mon.moves[slot] = Some(mov);
    m.mon.pp[slot] = env.t.move_info(mov).pp;
    let (moves, pp) = (m.mon.moves, m.mon.pp);
    if let Some(p) = b.parties[a.s()].get_mut(party) {
        p.moves = moves;
        p.pp = pp;
        let max = p.max_pp(env.t, slot as u8);
        b.battler_mut(a).mon.max_pp[slot] = max;
    }
    anim(env, u);
    say_with(env, u, Str::PkmnSketchedMove, a, super::move_buff(mov));
}

/// `BattleScript_EffectRecycle` + `Cmd_tryrecycleitem`: the item this battler position used
/// last (`usedHeldItems`) comes back when it holds none.
pub(super) fn recycle(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let used = b.fx.used_item[a.i()];
    let (Some((item, hold)), None) = (used, b.battler(a).x.item) else {
        b.fail(env, u);
        return;
    };
    b.fx.used_item[a.i()] = None;
    let party = b.battler(a).mon.party as usize;
    let m = b.battler_mut(a);
    m.x.item = Some(item);
    m.x.hold = hold;
    b.x.holds[a.s()][party] = hold;
    if let Some(p) = b.parties[a.s()].get_mut(party) {
        p.held = Some(item);
    }
    anim(env, u);
    say_with(
        env,
        u,
        Str::XFoundOneY,
        a,
        [Buff::None, Buff::None, Buff::Item(item)],
    );
}

/// `removeitem`'s `usedHeldItems` (by battler position, kept across switches).
pub(crate) fn record_used_item(b: &mut Battle, side: Side) {
    let m = b.battler(side);
    if let Some(item) = m.x.item {
        b.fx.used_item[side.i()] = Some((item, m.x.hold));
    }
}

/// `IsTwoTurnsMove`.
fn two_turns(e: MoveEffect) -> bool {
    matches!(
        e,
        MoveEffect::TwoTurn(_) | MoveEffect::SemiInvulnerable(_) | MoveEffect::Bide
    )
}

/// `STATUS2_MULTIPLETURNS` (a lock, Bide, Uproar).
fn multiple_turns(b: &Battle, side: Side) -> bool {
    let m = b.battler(side);
    m.fx.lock.is_some() || m.bide_turns > 0 || m.v.uproar > 0
}

/// `BattleScript_EffectConversion2` + `Cmd_settypetorandomresistance`: the last move that
/// landed on the user (`gLastLandedMoves`; not a two-turn move whose user is still in it), and
/// a type resisting its type (`gLastHitByType`) the user is not: up to 1000 draws of
/// `Random() % 128` over `gTypeEffectiveness` (rerolled above its entry count; the Foresight
/// marker and the end entry match nothing; Gen3 lets the draw reach one entry past the end,
/// read as no match), then a scan of the chart (with Gen3's bug: the user's type check reads
/// the last drawn entry).
pub(super) fn conversion_2(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let rec = b.battler(a).fx.rec;
    let (Some(landed), Some(hit)) = (rec.landed, rec.hit_type) else {
        b.fail(env, u);
        return;
    };
    if two_turns(env.t.move_info(landed).effect) && multiple_turns(b, a.other()) {
        b.fail(env, u);
        return;
    }
    let chart = env.t.type_chart;
    let split = env.t.foresight_split;
    // The Gen3 table: chart[..split], TYPE_FORESIGHT, chart[split..], TYPE_ENDTABLE.
    let entries = chart.len() + 2;
    let entry = |i: usize| match i {
        _ if i < split => chart.get(i),
        _ if i == split => None,
        _ => chart.get(i - 1),
    };
    let types = b.battler(a).types;
    let has = |t: Type| types.contains(&t);
    let mut last = 0usize;
    for _ in 0..1000 {
        let i = loop {
            let i = (env.rng.random() % 128) as usize;
            if i <= entries {
                break i;
            }
        };
        last = i;
        if let Some(e) = entry(i)
            && e.attack == hit
            && e.multiplier <= 5
            && !has(e.defend)
        {
            set_types(b, env, u, e.defend);
            return;
        }
    }
    let last_defend = entry(last).map(|e| e.defend);
    for j in 0..entries {
        if let Some(e) = entry(j)
            && e.attack == hit
            && e.multiplier <= 5
            && !last_defend.is_some_and(has)
        {
            set_types(b, env, u, e.defend);
            return;
        }
    }
    b.fail(env, u);
}

fn set_types(b: &mut Battle, env: &mut Env, u: &MoveUse, ty: Type) {
    b.battler_mut(u.attacker).types = [ty, ty];
    anim(env, u);
    say_with(
        env,
        u,
        Str::PkmnChangedType,
        u.attacker,
        [Buff::Type(ty), Buff::None, Buff::None],
    );
}

/// `BattleScript_EffectSnatch` + `Cmd_trysetsnatch` (no Pressure on its PP): fails as the
/// turn's last action; else `gProtectStructs.stealMove` until the end of the turn.
pub(super) fn snatch(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    no_pressure(b);
    b.attack_string(env, u);
    if b.acts_last(u.attacker) {
        b.fail(env, u);
        return;
    }
    b.battler_mut(u.attacker).fx.snatch = true;
    anim(env, u);
    say(env, u, Str::PkmnWaitsForTarget, u.attacker);
}

/// `Cmd_attackcanceler`'s Snatch (after Magic Coat): a `FLAG_SNATCH_AFFECTED` move used while
/// the foe waits with Snatch is taken (`PressurePPLose` on its Snatch, then
/// `BattleScript_SnatchedMove`: the user's attack string and PP, "X SNATCHED Y's move!", and
/// the move runs for the snatcher, `snatchsetbattlers` + `swapattackerwithtarget`).
pub(in crate::battle::effects) fn steal(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
) -> Option<(MoveUse, Option<HitResult>)> {
    let s = u.attacker.other();
    if !b.battler(s).fx.snatch || !u.info.flags.has(MoveFlags::SNATCH_AFFECTED) {
        return None;
    }
    pressure_pp_lose(b, env, u.attacker, s, MoveEffect::Snatch);
    b.battler_mut(s).fx.snatch = false;
    b.attack_string(env, u);
    env.out.push(BattleEvent::Msg(Msg {
        mov: Some(u.mov),
        wait: LONG,
        ..msg(Str::PkmnSnatchedMove, u.attacker, s, u.attacker)
    }));
    let target = if u.target == u.attacker {
        s
    } else {
        u.attacker
    };
    let back = MoveUse {
        attacker: s,
        target,
        slot: None,
        printed: true,
        ..*u
    };
    super::start_move(b, &back);
    let hit = b.execute(env, &back);
    Some((back, hit))
}

/// `BattleScript_EffectBeatUp` + `Cmd_trydobeatup`: one accuracy check, then a hit per party
/// mon with HP and no status, in party order ("X's attack!"): its species' base Attack x power
/// x (level x 2 / 5 + 2) / the target species' base Defense / 50 + 2, doubled on a crit, the
/// random roll and the Focus Band (`adjustnormaldamage`); no type, STAB nor Wonder Guard.
/// Each hit ends with `moveendto MOVEEND_NEXT_TARGET`; the loop stops when the target faints
/// or the party runs out ("But it failed!" with no hit at all).
pub(super) fn beat_up(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let (a, t) = (u.attacker, u.target);
    let n = b.parties[a.s()].len();
    let mut next = 0usize;
    let mut last = None;
    loop {
        if b.battler(t).mon.hp == 0 {
            break;
        }
        let party = &b.parties[a.s()];
        let found = (next..n).find(|&i| {
            party
                .get(i)
                .is_some_and(|p| p.hp > 0 && p.status == crate::mon::Status::None)
        });
        let Some(i) = found else {
            if last.is_none() {
                b.fail(env, u);
            }
            break;
        };
        next = i + 1;
        let p = *party.get(i).expect("a party slot");
        say_with(
            env,
            u,
            Str::PkmnAttack,
            a,
            [
                Buff::Mon {
                    side: a,
                    party: i as u8,
                },
                Buff::None,
                Buff::None,
            ],
        );
        let atk = env.t.species_info(p.species).base[1] as i32;
        let def = (env.t.species_info(b.battler(t).mon.species).base[2] as i32).max(1);
        let mut damage = atk * u.power as i32 * (p.level as i32 * 2 / 5 + 2) / def / 50 + 2;
        let crit = b.crit_roll(env, u);
        if crit {
            damage *= 2;
        }
        damage = calc::random_damage(damage, env.rng.random());
        let (damage, hung) = super::hold_on(b, env, u, damage, Effectiveness::Normal);
        let mut r = HitResult {
            damage,
            hp_dealt: 0,
            crit,
            eff: Effectiveness::Normal,
            hung,
        };
        anim(env, u);
        env.out.push(BattleEvent::Hit {
            target: t,
            eff: Effectiveness::Normal,
        });
        r.hp_dealt = b.change_hp(env, t, damage);
        if crit {
            say(env, u, Str::CriticalHit, a);
        }
        b.hung_on(env, u, &r);
        b.try_faint(env, t);
        // moveendto MOVEEND_NEXT_TARGET.
        super::move_end_first(b, env, u);
        abilities::move_end(b, env, u, Some(r));
        items::move_end(b, env, u, Some(r));
        super::sub::sub_end(b);
        last = Some(r);
    }
    // `BattleScript_BeatUpEnd` is `end`: the last hit's `moveend` ran already, so the shared
    // one finds nothing damaged (Rage once per hit).
    if let Some(c) = &mut b.fx.cur {
        c.hit = [false; 4];
    }
    None
}
