//! Wave C: the move effects of `data/battle_scripts_1.s` that Task D / E2 / G left as
//! `MoveEffect::Unsupported`, with the battle state they read (`gDisableStructs`,
//! `gProtectStructs`, `gSideTimers`, `gWishFutureKnock`, the `STATUS2` / `STATUS3` bits).
//!
//! - [`hits`]: the damaging scripts (`*_HIT` variants, fixed damage, power formulas, two-turn
//!   and semi-invulnerable moves, Rollout, Rampage, Recharge, the trapping moves, OHKO,
//!   Counter / Mirror Coat, Explosion, Dream Eater, Brick Break, Future Sight, Spit Up …).
//! - [`cond`]: the status and stat scripts (Curse, Swagger, Rest, healing, Haze, Belly Drum,
//!   Disable, Encore, Taunt, Torment, Spite, Destiny Bond, Lock-On, Perish Song …).
//! - [`field`]: protect-likes, screens, Mist, Safeguard, Spikes, the weather moves, Ingrain,
//!   Wish, Roar, Helping Hand / Follow Me, and `DoFieldEndTurnEffects`' side timers.
//! - [`calls`]: the moves that use another move (Metronome, Mirror Move, Sleep Talk, Assist).
//! - [`turns`]: the hooks into the shared flow (`attackcanceler`, `moveend`,
//!   `DoBattlerEndTurnEffects`, `HandleWishPerishSongOnTurnEnd`, `TurnValuesCleanUp`, the
//!   selection limits and the switch-in).
//!
//! # Deviations (singles, the engine's state)
//! - Helping Hand always fails (no partner in singles, as Gen3's `trysethelpinghand`); Follow
//!   Me only prints its message.
//! - Surf's double damage on a diving target is keyed on `MOVE_SURF` in Gen3 (not an effect):
//!   not modelled. Whirlpool's is (its trapping kind).
//! - A battler locked into a move it reached through another move (Metronome → Fly) uses it
//!   without the canceler's Soundproof / primal-weather checks being run again.
//! - `protectUses` past 23 (beyond `sMovesForbiddenToCopy`, which Gen3 reads after
//!   `sProtectSuccessRates`) gets a rate of 0.
//! - Counter / Mirror Coat / Revenge read the damage records of [`Records`], which are not
//!   hashed (as `multi_last`): they are reset before the next action choice. Sketch and
//!   Conversion 2 read `gLastPrintedMoves` / `gLastLandedMoves` there too, and Recycle
//!   [`FieldFx::used_item`].
//!
//! Wave D ([`sub`], [`copy`]): Substitute, Baton Pass, Transform, Mimic, Sketch, Recycle,
//! Conversion 2, Snatch, Beat Up. Their deviations: the substitute doll and Transform's sprite
//! have no event; the opponent's Baton Pass sends its first usable mon (as a faint); the
//! player's `moveend` for Baton Pass runs before the switch-in (nothing in it reads the
//! battler); Conversion 2's draw one entry past `gTypeEffectiveness` matches nothing.

mod calls;
mod cond;
mod copy;
mod field;
mod hits;
mod more;
mod sub;
mod turns;

use super::status;
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, Secondary, msg};
use crate::battle::{BattleEvent, Buff, Msg, Side, Str};
use crate::data::MoveId;
use crate::hash::Fnv64;
use crate::mon::{MoveEffect, SemiKind, Stat};
use alloc::vec;
use alloc::vec::Vec;

pub(super) use copy::{record_used_item, steal};
pub(super) use field::mist_blocks;
#[cfg(test)]
pub(super) use field::protect_rate;
pub(super) use more::{bounce, grudge, imprisoned};
pub(super) use sub::sub_end;
pub(crate) use sub::{baton_pass_in, sub_absorbed, sub_blocks_secondary, sub_takes, subbed};
pub(crate) use turns::{
    all_moves_unusable, before_turn, clear_bonds, destiny_bond, encored_slot, locked_action,
    reads_first_turn, record_damage, selection_block, slot_limited, spikes, wish_perish,
    wish_perish_pending,
};
pub(super) use turns::{
    called_use, canceler_flags, canceler_limits, canceler_recharge, canceler_thaw,
    end_turn_after_status, end_turn_ingrain, end_turn_taunt, end_turn_timers, move_end_first,
    move_end_last, physical_of, reroute, safeguarded, skipped, sleep_usable, start_move, trapped,
    turn_clean_up, turn_passed, via_sleep_talk,
};

/// Wave C's per-battler state (`gDisableStructs`, `gProtectStructs` and the `STATUS2` /
/// `STATUS3` bits the new effects use). Cleared on switch-in and faint, as
/// `SwitchInClearSetData` / `FaintClearSetData`. Hashed (without [`Records`]) only when set.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct BattlerFx {
    /// `STATUS2_MULTIPLETURNS` + `gLockedMoves`: the two-turn, semi-invulnerable, Rollout and
    /// Rampage moves.
    pub lock: Option<MoveId>,
    /// `gChosenMovePos` of a locked or recharging battler: the action it is given.
    pub slot: u8,
    /// `gDisableStructs.rolloutTimer`.
    pub rollout: u8,
    /// `STATUS2_DEFENSE_CURL`.
    pub defense_curl: bool,
    /// `STATUS2_LOCK_CONFUSE` turns (Thrash, Petal Dance, Outrage).
    pub rampage: u8,
    /// `STATUS3_ON_AIR` / `_UNDERGROUND` / `_UNDERWATER`.
    pub semi: Option<SemiKind>,
    /// `gDisableStructs.rechargeTimer` (`STATUS2_RECHARGE` while non-zero).
    pub recharge: u8,
    /// `STATUS2_WRAPPED` turns, and the move (`gBattleStruct->wrappedMove`); by the foe.
    pub wrapped: u8,
    pub wrap_move: Option<MoveId>,
    /// `STATUS2_CURSED`.
    pub cursed: bool,
    /// `STATUS2_NIGHTMARE`.
    pub nightmare: bool,
    /// `gProtectStructs.protected` / `.endured` (this turn).
    pub protected: bool,
    pub endured: bool,
    /// `gDisableStructs.protectUses`.
    pub protect_uses: u8,
    /// `gDisableStructs.disabledMove` / `disableTimer`.
    pub disabled: Option<MoveId>,
    pub disable_timer: u8,
    /// `gDisableStructs.encoredMove` / `encoredMovePos` / `encoreTimer`.
    pub encore: Option<(MoveId, u8)>,
    pub encore_timer: u8,
    /// `gDisableStructs.tauntTimer`.
    pub taunt: u8,
    /// `STATUS2_TORMENT`.
    pub tormented: bool,
    /// `STATUS3_ALWAYS_HITS` turns (Lock-On / Mind Reader by the foe, `battlerWithSureHit`).
    pub lock_on: u8,
    /// `STATUS3_PERISH_SONG` + `perishSongTimer`.
    pub perish: Option<u8>,
    /// `gDisableStructs.stockpileCounter`.
    pub stockpile: u8,
    /// `STATUS2_RAGE`.
    pub rage: bool,
    /// `STATUS2_DESTINY_BOND`.
    pub destiny_bond: bool,
    /// `STATUS3_ROOTED`.
    pub rooted: bool,
    /// `STATUS3_MINIMIZED`.
    pub minimized: bool,
    /// `STATUS3_GRUDGE`.
    pub grudge: bool,
    /// `STATUS3_IMPRISONED_OTHERS`.
    pub imprison: bool,
    /// `gProtectStructs.bounceMove` (Magic Coat, this turn).
    pub bounce: bool,
    /// `STATUS2_SUBSTITUTE` and `gDisableStructs.substituteHP` (Wave D).
    pub substitute: bool,
    pub sub_hp: u8,
    /// `STATUS2_TRANSFORMED`.
    pub transformed: bool,
    /// `gDisableStructs.mimickedMoves`: a bit per move slot (its PP is not written back).
    pub mimicked: u8,
    /// `gProtectStructs.stealMove` (Snatch, this turn).
    pub snatch: bool,
    /// Baton Pass waits for the player's choice of the mon it passes to (`openpartyscreen`).
    pub baton: bool,
    /// Bookkeeping later moves read (not hashed).
    pub rec: Records,
}

/// `gProtectStructs.physicalDmg` / `specialDmg` (+ the battler that dealt it) of this turn,
/// and `gBattleStruct->lastTakenMove` (Mirror Move). Not hashed: the damage records are
/// cleared at the end of each turn (`TurnValuesCleanUp`), and keeping `last_taken` out keeps
/// the checkpoint hashes of battles without these moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Records {
    pub physical: Option<(u16, Side)>,
    pub special: Option<(u16, Side)>,
    pub last_taken: Option<MoveId>,
    /// `gLastPrintedMoves`: the chosen move whose attack string this battler printed (Sketch).
    pub printed: Option<MoveId>,
    /// `gLastLandedMoves` / `gLastHitByType`: the last move that landed on this battler, any
    /// power, and its type (Conversion 2).
    pub landed: Option<MoveId>,
    pub hit_type: Option<crate::mon::Type>,
}

impl BattlerFx {
    /// Whether the hashed part is set.
    pub fn is_set(&self) -> bool {
        BattlerFx {
            rec: Records::default(),
            ..*self
        } != BattlerFx::default()
    }

    pub fn hash(&self, h: &mut Fnv64) {
        let mv = |m: Option<MoveId>| m.map_or(0xFFFF, |m| m.0);
        h.write_u16(mv(self.lock));
        h.write_u16(mv(self.wrap_move));
        h.write_u16(mv(self.disabled));
        h.write_u16(mv(self.encore.map(|e| e.0)));
        h.write(&[
            self.slot,
            self.rollout,
            self.defense_curl as u8,
            self.rampage,
            self.semi.map_or(0xFF, |s| s as u8),
            self.recharge,
            self.wrapped,
            self.cursed as u8,
            self.nightmare as u8,
            self.protected as u8,
            self.endured as u8,
            self.protect_uses,
            self.disable_timer,
            self.encore.map_or(0xFF, |e| e.1),
            self.encore_timer,
            self.taunt,
            self.tormented as u8,
            self.lock_on,
            self.perish.unwrap_or(0xFF),
            self.stockpile,
            self.rage as u8,
            self.destiny_bond as u8,
            self.rooted as u8,
            self.minimized as u8,
            self.grudge as u8 | (self.imprison as u8) << 1 | (self.bounce as u8) << 2,
        ]);
        // Wave D: a tagged block only while any is set, so other battles hash as before.
        let wd = [
            self.substitute as u8
                | (self.transformed as u8) << 1
                | (self.snatch as u8) << 2
                | (self.baton as u8) << 3,
            self.sub_hp,
            self.mimicked,
        ];
        if wd != [0; 3] {
            h.write(&[0xD1]);
            h.write(&wd);
        }
    }
}

/// A pending Future Sight / Doom Desire on a battler slot (`gWishFutureKnock.futureSight*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Future {
    pub turns: u8,
    pub damage: i32,
    pub mov: MoveId,
    pub attacker: Side,
}

/// A side timer with the move that set it (its `PkmnSXWoreOff` names it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Timer {
    pub turns: u8,
    pub mov: MoveId,
}

/// The move being used, as the move-effect hooks see it (not hashed: set from the canceler
/// to the end of the move).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Current {
    pub attacker: Side,
    pub target: Side,
    /// `IS_TYPE_PHYSICAL` of the move's base type (`datahpupdate`'s damage records).
    pub physical: bool,
    /// `gSpecialStatuses[].physicalDmg || specialDmg`: damaged during this move.
    pub hit: [bool; 4],
    /// `gChosenMove == MOVE_SLEEP_TALK`: the move was called by Sleep Talk.
    pub sleep_talk: bool,
    /// A drop on that side was already refused with a message (Mist / Clear Body once per
    /// move, `STAT_CHANGE_ALLOW_PTR`).
    pub lowered: [bool; 4],
    /// The move a calling move (Metronome, Mirror Move, Sleep Talk, Assist) used.
    pub called: Option<MoveId>,
    /// `gCurrentMove` and `gChosenMovePos` (Grudge).
    pub mov: MoveId,
    pub slot: Option<u8>,
    /// `HITMARKER_ATTACKSTRING_PRINTED` (`gLastPrintedMoves`, Wave D).
    pub printed: bool,
    /// `gChosenMove = MOVE_UNAVAILABLE` (Transform, Mimic, Sketch): no last move, nothing
    /// printed nor landed.
    pub unavailable: bool,
    /// `gSpecialStatuses.ppNotAffectedByPressure`.
    pub no_pressure: bool,
}

/// Wave C's side and field state (`gSideTimers`, `gWishFutureKnock`): the side timers and
/// `knocked_off` by [`Side::s`], the battler counters (wish, future attacks, used items) by
/// [`Side::i`].
/// Hashed (without `cur` and `used_item`) only when set.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct FieldFx {
    pub reflect: [Option<Timer>; 2],
    pub light_screen: [Option<Timer>; 2],
    pub mist: [Option<Timer>; 2],
    pub safeguard: [u8; 2],
    pub spikes: [u8; 2],
    /// `wishCounter` / `wishMonId` (the party slot that wished).
    pub wish: [u8; 4],
    pub wish_party: [u8; 4],
    /// `knockedOffMons`: a bit per party slot.
    pub knocked_off: [u8; 2],
    /// Future attacks aimed at each side.
    pub future: [Option<Future>; 4],
    /// `weatherDuration` of a move's weather (0: none, or an ability's permanent weather).
    pub weather_turns: u8,
    /// `gPaydayMoney` (Wave D).
    pub pay_day: u16,
    /// `gBattleStruct->usedHeldItems` by battler position, with the item's hold effect
    /// (Recycle). Not hashed.
    pub used_item: [Option<(crate::data::ItemId, Option<crate::mon::HoldEffect>)>; 4],
    /// `gSideTimers[side].followmeTarget` while `followmeTimer` is set (doubles), by side.
    pub follow_me: [Option<Side>; 2],
    /// `gProtectStructs[battler].helpingHand`, a bit per battler (doubles).
    pub helping_hand: u8,
    /// `gSpecialStatuses[target].lightningRodRedirected`, for this move's `attackcanceler`
    /// (set and read within one move: not hashed).
    pub rod: bool,
    /// The script ran `moveendto MOVEEND_NEXT_TARGET` for each of its targets (the
    /// `MOVE_TARGET_FOES_AND_ALLY` loops), so the core skips its `moveend` (not hashed).
    pub ended: bool,
    pub cur: Option<Current>,
}

impl FieldFx {
    /// Whether the hashed part is set.
    pub fn is_set(&self) -> bool {
        FieldFx {
            cur: None,
            used_item: [None; 4],
            rod: false,
            ended: false,
            ..*self
        } != FieldFx::default()
    }

    pub fn hash(&self, h: &mut Fnv64) {
        let future = |h: &mut Fnv64, f: Option<Future>| match f {
            None => h.write(&[0xFF]),
            Some(f) => {
                h.write(&[f.turns, f.attacker.code()]);
                h.write_u32(f.damage as u32);
                h.write_u16(f.mov.0);
            }
        };
        let timer = |h: &mut Fnv64, t: Option<Timer>| match t {
            None => h.write(&[0xFF]),
            Some(t) => {
                h.write(&[t.turns]);
                h.write_u16(t.mov.0);
            }
        };
        for s in 0..2 {
            timer(h, self.reflect[s]);
            timer(h, self.light_screen[s]);
            timer(h, self.mist[s]);
            h.write(&[
                self.safeguard[s],
                self.spikes[s],
                self.wish[s],
                self.wish_party[s],
                self.knocked_off[s],
            ]);
            future(h, self.future[s]);
        }
        // Doubles: the right battlers' counters, only while set (singles hash unchanged).
        for s in 2..4 {
            if self.wish[s] != 0 || self.future[s].is_some() {
                h.write(&[0xDB, s as u8, self.wish[s], self.wish_party[s]]);
                future(h, self.future[s]);
            }
        }
        h.write(&[self.weather_turns]);
        // Wave D: a tagged block only while set. `used_item` is not hashed (as [`Records`]):
        // every consumed berry sets it, and only Recycle reads it.
        if self.pay_day != 0 {
            h.write(&[0xD2]);
            h.write_u16(self.pay_day);
        }
        // Doubles: Follow Me and Helping Hand, only while set (cleared every turn).
        if self.follow_me != [None; 2] || self.helping_hand != 0 {
            let f = |s: Option<Side>| s.map_or(0xFF, |s| s.code());
            h.write(&[
                0xDC,
                f(self.follow_me[0]),
                f(self.follow_me[1]),
                self.helping_hand,
            ]);
        }
    }
}

/// `selectfirstvalidtarget` / `jumpifnexttargetvalid`: every battler on the field but the
/// attacker, in battler order (singles: the target).
pub(in crate::battle::effects) fn all_targets(b: &Battle, u: &MoveUse) -> Vec<Side> {
    if !b.is_double() {
        return vec![u.target];
    }
    b.sides()
        .filter(|&s| s != u.attacker && b.is_present(s))
        .collect()
}

/// A `MOVE_TARGET_FOES_AND_ALLY` loop: `each` on every target ([`all_targets`]), each
/// followed by `moveendto MOVEEND_NEXT_TARGET` in doubles (the core then skips its own).
/// The last target's hit.
pub(in crate::battle::effects) fn each_target(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    mut each: impl FnMut(&mut Battle, &mut Env, &MoveUse) -> Option<HitResult>,
) -> Option<HitResult> {
    if !b.is_double() {
        return each(b, env, u);
    }
    let mut last = None;
    for t in all_targets(b, u) {
        let ut = MoveUse { target: t, ..*u };
        last = each(b, env, &ut);
        super::after_move(b, env, &ut, last);
    }
    b.fx.ended = true;
    last
}

// ---- the hooks the core reads (`core/moves.rs`) ----

/// `DEFENDER_IS_PROTECTED`: the target protected itself this turn and the move is
/// `FLAG_PROTECT_AFFECTED`.
pub(crate) fn protected(b: &Battle, u: &MoveUse) -> bool {
    u.target != u.attacker
        && b.battler(u.target).fx.protected
        && u.info.flags.has(crate::mon::MoveFlags::PROTECT_AFFECTED)
}

/// `Cmd_accuracycheck`'s draws-nothing outcomes: `JumpIfMoveAffectedByProtect`, then
/// `AccuracyCalcHelper` (Lock-On by the attacker; a target in the air, underground or
/// underwater unless the move reaches it; Thunder in rain; `EFFECT_ALWAYS_HIT` /
/// `EFFECT_VITAL_THROW`). `None`: roll.
pub(crate) fn sure_accuracy(b: &Battle, u: &MoveUse) -> Option<bool> {
    if protected(b, u) {
        return Some(false);
    }
    let t = b.battler(u.target);
    if u.target != u.attacker && t.fx.lock_on > 0 {
        return Some(true);
    }
    if u.target != u.attacker
        && let Some(semi) = t.fx.semi
    {
        let reaches = match semi {
            SemiKind::Fly | SemiKind::Bounce => matches!(
                u.info.effect,
                MoveEffect::Gust
                    | MoveEffect::Twister
                    | MoveEffect::SkyUppercut
                    | MoveEffect::Thunder
            ),
            SemiKind::Dig => matches!(
                u.info.effect,
                MoveEffect::Earthquake | MoveEffect::Magnitude
            ),
            SemiKind::Dive => u.info.effect == MoveEffect::Trap(crate::mon::TrapKind::Whirlpool),
        };
        if !reaches {
            return Some(false);
        }
    }
    match u.info.effect {
        MoveEffect::Thunder if b.weather().rain() => Some(true),
        MoveEffect::AlwaysHit | MoveEffect::VitalThrow => Some(true),
        _ => None,
    }
}

/// `NO_ACC_CALC_CHECK_LOCK_ON`: Lock-On passes; a hidden target or protection fails; no roll.
pub(super) fn no_acc_check(b: &Battle, u: &MoveUse) -> bool {
    let t = b.battler(u.target);
    if u.target != u.attacker && t.fx.lock_on > 0 {
        return true;
    }
    if u.target != u.attacker && t.fx.semi.is_some() {
        return false;
    }
    !protected(b, u)
}

/// The move's accuracy: Thunder's is 50 in the sun.
pub(crate) fn move_accuracy(b: &Battle, u: &MoveUse) -> u8 {
    if u.info.effect == MoveEffect::Thunder && b.weather().sun() {
        50
    } else {
        u.info.accuracy
    }
}

/// `Cmd_critcalc`'s move terms: `EFFECT_HIGH_CRITICAL`, `SKY_ATTACK`, `BLAZE_KICK`,
/// `POISON_TAIL` add 1.
pub(crate) fn crit_bonus(effect: MoveEffect) -> u8 {
    matches!(
        effect,
        MoveEffect::HighCritical(_) | MoveEffect::TwoTurn(crate::mon::TwoTurnKind::SkyAttack)
    ) as u8
}

/// What `CalculateBaseDamage` reads beyond the fighters: the weather, the defender's Reflect /
/// Light Screen (not on a crit), Explosion's halved defense, Solar Beam in rain, sand or hail.
pub(crate) fn damage_extra(
    b: &Battle,
    u: &MoveUse,
    crit: bool,
    weather: crate::battle::calc::DamageWeather,
) -> crate::battle::calc::Extra {
    let side = u.target.s();
    let screen = if u.ty.is_physical() {
        b.fx.reflect[side].is_some()
    } else {
        b.fx.light_screen[side].is_some()
    };
    let w = b.weather();
    crate::battle::calc::Extra {
        weather,
        screen: screen && !crit && u.target != u.attacker,
        halve_defense: u.info.effect == MoveEffect::Explosion,
        double_def: b.is_double() && b.is_present(u.target) && b.is_present(u.target.partner()),
        spread: u.info.target == crate::mon::MoveTarget::BothFoes,
        weak_solar: u.info.effect == MoveEffect::TwoTurn(crate::mon::TwoTurnKind::SolarBeam)
            && (w.rain()
                || matches!(
                    w,
                    crate::battle::core::Weather::Sand | crate::battle::core::Weather::Hail
                )),
        ..Default::default()
    }
}

/// `adjustnormaldamage`: the Focus Band roll (drawn whenever the target holds one), then the
/// target keeps 1 HP when it endured, its band worked, or the move is False Swipe. The damage
/// and whether a message follows (`MOVE_RESULT_FOE_ENDURED` / `_FOE_HUNG_ON`; False Swipe is
/// silent).
pub(crate) fn hold_on(
    b: &Battle,
    env: &mut Env,
    u: &MoveUse,
    damage: i32,
    eff: crate::battle::Effectiveness,
) -> (i32, bool) {
    let d = b.battler(u.target);
    let banded = match d.x.hold {
        Some(crate::mon::HoldEffect::FocusBand(p)) => env.rng.random() % 100 < p as u16,
        _ => false,
    };
    let hp = d.mon.hp;
    let holds = banded || d.fx.endured || u.info.effect == MoveEffect::FalseSwipe;
    // Not behind a substitute (`STATUS2_SUBSTITUTE`, read before the HP check).
    if holds
        && !d.fx.substitute
        && eff != crate::battle::Effectiveness::NoEffect
        && hp > 0
        && hp as i32 <= damage
    {
        (hp as i32 - 1, banded || d.fx.endured)
    } else {
        (damage, false)
    }
}

/// `MOVE_RESULT_FOE_ENDURED`: `side` endured this turn.
pub(crate) fn endured(b: &Battle, side: Side) -> bool {
    b.battler(side).fx.endured
}

// ---- shared building blocks ----

pub(super) fn anim(env: &mut Env, u: &MoveUse) {
    env.out.push(BattleEvent::MoveAnim {
        attacker: u.attacker,
        mov: u.mov,
    });
}

/// `u.msg_long(id, active)`.
pub(super) fn say(env: &mut Env, u: &MoveUse, id: Str, active: Side) {
    env.out.push(BattleEvent::Msg(u.msg_long(id, active)));
}

/// [`say`] with buffers.
pub(super) fn say_with(env: &mut Env, u: &MoveUse, id: Str, active: Side, buffs: [Buff; 3]) {
    env.out.push(BattleEvent::Msg(Msg {
        buffs,
        ..u.msg_long(id, active)
    }));
}

/// A message about `side` outside a move (`B_ATK` = `side`, `B_DEF` = the other battler).
pub(super) fn tell(env: &mut Env, id: Str, side: Side, buffs: [Buff; 3]) {
    env.out.push(BattleEvent::Msg(Msg {
        buffs,
        wait: LONG,
        ..msg(id, side, side.other(), side)
    }));
}

/// `Buff::Move` in `B_BUFF1`.
pub(super) fn move_buff(m: MoveId) -> [Buff; 3] {
    [Buff::Move(m), Buff::None, Buff::None]
}

/// A secondary effect drawn by `seteffectwithchance`.
pub(super) fn chance(apply: fn(&mut Battle, &mut Env, &MoveUse, &HitResult)) -> Option<Secondary> {
    Some(Secondary {
        certain: false,
        apply,
    })
}

/// A `MOVE_EFFECT_CERTAIN` secondary effect.
pub(super) fn certain(apply: fn(&mut Battle, &mut Env, &MoveUse, &HitResult)) -> Option<Secondary> {
    Some(Secondary {
        certain: true,
        apply,
    })
}

/// `statbuffchange` of `delta` stages of `stat` on `side` with its stat animation and
/// `gStatUpStringIds` / `gStatDownStringIds` message. At the limit: "won't go higher / lower"
/// when `loud` (scripts without the `B_MSG_STAT_WONT_*` skip), else nothing. True when it
/// changed.
pub(super) fn stage(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    side: Side,
    stat: Stat,
    delta: i8,
    loud: bool,
) -> bool {
    let up = delta > 0;
    if b.change_stage(side, stat, delta) {
        env.out.push(BattleEvent::StatAnim { side, up });
        env.out.push(BattleEvent::Msg(b.stat_msg(
            u,
            side,
            stat,
            delta.unsigned_abs(),
            up,
        )));
        true
    } else {
        if loud {
            let id = if up {
                Str::StatWontIncrease
            } else {
                Str::StatWontDecrease
            };
            say_with(env, u, id, side, [Buff::Stat(stat), Buff::None, Buff::None]);
        }
        false
    }
}

/// Runs a Wave C effect.
pub(super) fn apply(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    use MoveEffect as E;
    match u.info.effect {
        E::AlwaysHit
        | E::HighCritical(_)
        | E::TriAttack
        | E::ThawHit
        | E::PoisonFang
        | E::Gust
        | E::Twister
        | E::SkyUppercut
        | E::Thunder
        | E::StatUpHit(_)
        | E::AllStatsUpHit
        | E::FakeOut
        | E::FalseSwipe
        | E::PayDay
        | E::RapidSpin
        | E::SecretPower
        | E::Superpower
        | E::Overheat
        | E::KnockOff
        | E::Thief
        | E::SmellingSalt
        | E::Facade
        | E::Revenge
        | E::DoubleEdge
        | E::Recharge
        | E::Trap(_)
        | E::Flail
        | E::Eruption
        | E::Return
        | E::Frustration
        | E::HiddenPower
        | E::WeatherBall
        | E::Rage => hits::plain(b, env, u),
        E::Earthquake => hits::earthquake(b, env, u, false),
        E::Magnitude => hits::earthquake(b, env, u, true),
        E::BrickBreak => hits::brick_break(b, env, u),
        E::RecoilIfMiss => hits::recoil_if_miss(b, env, u),
        E::Rampage => hits::rampage(b, env, u),
        E::Twineedle => hits::twineedle(b, env, u),
        E::TripleKick => hits::triple_kick(b, env, u),
        E::DreamEater => hits::dream_eater(b, env, u),
        E::Explosion => hits::explosion(b, env, u),
        E::Ohko => hits::ohko(b, env, u),
        E::SuperFang | E::DragonRage | E::LevelDamage | E::Psywave => hits::fixed(b, env, u),
        E::Counter | E::MirrorCoat => hits::counter(b, env, u),
        E::Endeavor => hits::endeavor(b, env, u),
        E::Present => hits::present(b, env, u),
        E::Rollout => hits::rollout(b, env, u),
        E::Snore => hits::snore(b, env, u),
        E::FocusPunch => hits::focus_punch(b, env, u),
        E::TwoTurn(kind) => hits::two_turn(b, env, u, kind),
        E::SemiInvulnerable(kind) => hits::semi_invulnerable(b, env, u, kind),
        E::SpitUp => hits::spit_up(b, env, u),
        E::FutureSight(doom) => {
            hits::future_sight(b, env, u, doom);
            None
        }
        E::RolePlay | E::SkillSwap | E::Grudge | E::Imprison | E::MagicCoat | E::Trick => {
            more::apply(b, env, u);
            None
        }
        E::Substitute => {
            sub::substitute(b, env, u);
            None
        }
        E::BatonPass => {
            sub::baton_pass(b, env, u);
            None
        }
        E::Transform => {
            copy::transform(b, env, u);
            None
        }
        E::Mimic => {
            copy::mimic(b, env, u);
            None
        }
        E::Sketch => {
            copy::sketch(b, env, u);
            None
        }
        E::Recycle => {
            copy::recycle(b, env, u);
            None
        }
        E::Conversion2 => {
            copy::conversion_2(b, env, u);
            None
        }
        E::Snatch => {
            copy::snatch(b, env, u);
            None
        }
        E::BeatUp => copy::beat_up(b, env, u),
        E::Metronome => calls::metronome(b, env, u),
        E::MirrorMove => calls::mirror_move(b, env, u),
        E::SleepTalk => calls::sleep_talk(b, env, u),
        E::Assist => calls::assist(b, env, u),
        E::Roar => {
            field::roar(b, env, u);
            None
        }
        E::Protect
        | E::Endure
        | E::Spikes
        | E::LightScreen
        | E::Reflect
        | E::Mist
        | E::Safeguard
        | E::SetWeather(_)
        | E::Ingrain
        | E::Wish
        | E::HelpingHand
        | E::FollowMe => {
            field::apply(b, env, u);
            None
        }
        _ => {
            if cond::apply(b, env, u) {
                None
            } else {
                b.unsupported(env, u)
            }
        }
    }
}

/// `DoFieldEndTurnEffects` up to the weather (the side timers, Wish).
pub(super) fn field_end_turn(b: &mut Battle, env: &mut Env) {
    field::end_turn(b, env);
}

/// Gen3 `CancelMultiTurnMoves`' Wave C part, shared with [`status::cancel_multi_turn`].
pub(super) fn cancel_locks(b: &mut Battle, side: Side) {
    let fx = &mut b.battler_mut(side).fx;
    fx.lock = None;
    fx.rampage = 0;
    fx.semi = None;
    fx.rollout = 0;
}

/// `CancelMultiTurnMoves` in full (Bide, Uproar, Fury Cutter, and the locks).
pub(super) fn cancel_all(b: &mut Battle, side: Side) {
    status::cancel_multi_turn(b, side);
}

/// `BattleScript_StickyHoldActivates` / `BattleScript_AbilityPreventsPhasingOut`: the
/// target's ability names itself.
pub(super) fn ability_blocks(env: &mut Env, u: &MoveUse, id: Str, ability: crate::mon::Ability) {
    say_with(
        env,
        u,
        id,
        u.target,
        [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(ability)),
        ],
    );
}
