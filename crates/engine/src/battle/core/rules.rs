//! Silice's per-battle rules in the core ([`crate::battle::rules`]): the field rule's effects,
//! the trainer's lines and the trainer's items (Gen3 `AI_TrySwitchOrUseItem`'s
//! `ShouldUseItem`, `HandleAction_UseItem`'s `BattleScript_OpponentUses*`). Nothing here runs
//! in a battle without rules, lines or items: Gen3 battles draw and hash as before.

use super::{Battle, Env, LONG, Side, about, msg};
use crate::battle::rules::{LineWhen, RuleEffect, Var, What, When, Who};
use crate::battle::{BattleEvent, Buff, Msg, Str};
use crate::data::{GameData, ItemId, ItemUse, MedicineEffect, TextId};
use crate::item_use::{
    CURE_BURN, CURE_CONFUSION, CURE_FREEZE, CURE_PARALYSIS, CURE_POISON, CURE_SLEEP,
};
use crate::mon::{Stat, Status, StatusKind};
use alloc::vec::Vec;

/// Gen3 `MAX_TRAINER_ITEMS`.
pub(crate) const MAX_TRAINER_ITEMS: usize = 4;

/// The rule state a battle starts with: the field rule, the trainer's lines and items.
pub(crate) fn setup(b: &mut Battle, game: &impl GameData) {
    b.x.rule = b.setup.rules.field.and_then(|id| game.battle_rule(id));
    if let crate::battle::BattleKind::Trainer { trainer, .. } = b.setup.kind {
        b.x.lines = game.trainer_lines(trainer);
        for (slot, &item) in game
            .trainer_items(trainer)
            .iter()
            .take(MAX_TRAINER_ITEMS)
            .enumerate()
        {
            let usage = game.item_use(item);
            if ai_kind(usage).is_some() {
                b.x.items[slot] = Some((item, usage));
            }
        }
    }
}

/// Gen3 `GetAI_ItemType`: the kinds the trainer AI uses (`AI_ITEM_NOT_RECOGNIZABLE`: `None`;
/// Guard Spec is not classified by content, so it is never used).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AiItem {
    FullRestore,
    HealHp(u16),
    Cure(u8),
    XStat(u8, i8),
}

fn ai_kind(u: ItemUse) -> Option<AiItem> {
    Some(match u {
        ItemUse::Medicine(MedicineEffect::HealCure(..)) => AiItem::FullRestore,
        ItemUse::Medicine(MedicineEffect::Heal(n)) => AiItem::HealHp(n),
        ItemUse::Medicine(MedicineEffect::Cure(bits)) => AiItem::Cure(bits),
        ItemUse::StatStage(stat, n) => AiItem::XStat(stat, n),
        _ => return None,
    })
}

impl Battle {
    /// The battle's turn, from 1 (`gBattleResults.battleTurnCounter + 1`).
    fn turn1(&self) -> u8 {
        self.turn.saturating_add(1)
    }

    /// The battlers on the field `who` names, in battler order.
    fn rule_targets(&self, who: Who) -> Vec<Side> {
        self.sides()
            .filter(|&s| self.is_present(s) && self.battler(s).mon.hp > 0)
            .filter(|&s| who.sides.has(s.is_player()))
            .filter(|&s| who.types.matches(self.battler(s).types))
            .filter(|&s| !who.last || self.is_last(s))
            .collect()
    }

    /// Whether `side`'s battler is its trainer's last mon that can fight.
    fn is_last(&self, side: Side) -> bool {
        let me = self.battler(side).mon.party;
        let party = &self.parties[side.s()];
        !self
            .party_range(side)
            .any(|i| i != me && party.get(i as usize).is_some_and(|m| m.hp > 0))
    }

    /// The instant effects of the field rule that `pick` selects, on their battlers.
    fn run_rule(&mut self, env: &mut Env, pick: impl Fn(When) -> bool, only: Option<Side>) {
        let Some(rule) = self.x.rule else { return };
        for e in rule.effects.iter().filter(|e| pick(e.when)) {
            let mut targets = self.rule_targets(e.who);
            if let Some(o) = only {
                targets.retain(|&s| s == o);
                if targets.is_empty() {
                    continue;
                }
            }
            if e.what == What::Say {
                let side = targets.first().copied().unwrap_or(Side::Player);
                if let Some(t) = e.text {
                    self.say(env, t, e.var, side);
                }
                continue;
            }
            for side in targets {
                self.apply_effect(env, e, side);
            }
        }
    }

    /// An authored text about `side`'s battler (`{STR_VAR_1}`: `var`, `{STR_VAR_2}`: the
    /// trainer).
    pub(crate) fn rule_say(&mut self, env: &mut Env, text: TextId, var: Var, side: Side) {
        self.say(env, text, var, side);
    }

    fn say(&mut self, env: &mut Env, text: TextId, var: Var, side: Side) {
        let party = self.battler(side).mon.party;
        let first = match var {
            Var::Mon => Buff::Mon { side, party },
            Var::Trainer => Buff::TrainerName,
        };
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [first, Buff::TrainerName, Buff::None],
            wait: LONG,
            ..about(Str::Authored(text), side)
        }));
    }

    fn apply_effect(&mut self, env: &mut Env, e: &RuleEffect, side: Side) {
        match e.what {
            What::Stage { stat, delta, limit } => {
                let cur = self.battler(side).stages[stat as usize];
                let to = if delta < 0 {
                    (cur + delta).max(if limit < 0 { limit } else { -6 })
                } else {
                    (cur + delta).min(if limit > 0 { limit } else { 6 })
                };
                if to == cur || (delta < 0 && to > cur) || (delta > 0 && to < cur) {
                    return;
                }
                self.battler_mut(side).stages[stat as usize] = to;
                env.out.push(BattleEvent::StatAnim {
                    side,
                    up: delta > 0,
                });
                match e.text {
                    Some(t) => self.say(env, t, e.var, side),
                    None => {
                        let m = self.stat_msg_sides(
                            side,
                            side,
                            side,
                            stat,
                            delta.unsigned_abs(),
                            delta > 0,
                        );
                        env.out.push(BattleEvent::Msg(m));
                    }
                }
            }
            What::Status(kind) => {
                if self.battler(side).mon.status != Status::None {
                    return;
                }
                let status = match kind {
                    StatusKind::Sleep => Status::Sleep((env.rng.random() & 3) as u8 + 2),
                    StatusKind::Poison => Status::Poison,
                    StatusKind::Burn => Status::Burn,
                    StatusKind::Freeze => Status::Freeze,
                    StatusKind::Paralysis => Status::Paralysis,
                    StatusKind::Toxic => Status::Toxic(0),
                };
                self.battler_mut(side).mon.status = status;
                self.write_back(side);
                env.out.push(BattleEvent::StatusIcon { side, status });
                let id = match kind {
                    StatusKind::Sleep => Str::PkmnFellAsleep,
                    StatusKind::Poison => Str::PkmnWasPoisoned,
                    StatusKind::Burn => Str::PkmnWasBurned,
                    StatusKind::Freeze => Str::PkmnWasFrozen,
                    StatusKind::Paralysis => Str::PkmnWasParalyzed,
                    StatusKind::Toxic => Str::PkmnBadlyPoisoned,
                };
                self.effect_text(env, e, side, id);
            }
            What::Confuse => {
                if self.battler(side).confusion > 0 {
                    return;
                }
                self.battler_mut(side).confusion = (env.rng.random() % 4) as u8 + 2;
                self.effect_text(env, e, side, Str::PkmnBecameConfused);
            }
            What::Heal(n) | What::Hurt(n) => {
                let b = self.battler(side);
                if matches!(e.what, What::Heal(_)) && b.mon.hp >= b.mon.max_hp {
                    return;
                }
                let amount = (b.mon.max_hp / n.max(1) as u16).max(1) as i32;
                let damage = if matches!(e.what, What::Heal(_)) {
                    -amount
                } else {
                    amount
                };
                if let Some(t) = e.text {
                    self.say(env, t, e.var, side);
                }
                self.change_hp_passive(env, side, damage);
                self.try_faint(env, side);
            }
            _ => {}
        }
    }

    /// The effect's text, or the Gen3 message `id` about `side`.
    fn effect_text(&mut self, env: &mut Env, e: &RuleEffect, side: Side, id: Str) {
        match e.text {
            Some(t) => self.say(env, t, e.var, side),
            None => env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..msg(id, side, side, side)
            })),
        }
    }

    /// The rules' intro message and `When::Start` effects (before the first turn's switch-in
    /// effects), then the leads that are their side's last mon.
    pub(crate) fn rules_start(&mut self, env: &mut Env) {
        if let Some(t) = self.setup.rules.intro {
            self.say(env, t, Var::Mon, Side::Player);
        }
        self.run_rule(env, |w| w == When::Start, None);
        for side in self.sides().collect::<Vec<_>>() {
            if self.is_present(side) {
                self.last_in(env, side);
            }
        }
    }

    /// `When::TurnStart` effects of the turn about to be chosen.
    pub(crate) fn rules_turn_start(&mut self, env: &mut Env) {
        let turn = self.turn1();
        self.run_rule(
            env,
            |w| matches!(w, When::TurnStart(t) if t.matches(turn)),
            None,
        );
    }

    /// `When::TurnEnd` effects of the turn ending (after the end-of-turn effects).
    pub(crate) fn rules_turn_end(&mut self, env: &mut Env) {
        let turn = self.turn1();
        self.run_rule(
            env,
            |w| matches!(w, When::TurnEnd(t) if t.matches(turn)),
            None,
        );
    }

    /// `side` sent out a mon: its `When::LastIn` effects, once, when it is the side's last.
    pub(crate) fn last_in(&mut self, env: &mut Env, side: Side) {
        let s = side.s();
        if self.x.rule.is_none() || self.x.last_in[s] || !self.is_last(side) {
            return;
        }
        self.x.last_in[s] = true;
        self.run_rule(env, |w| w == When::LastIn, Some(side));
    }

    /// The product of the `When::During` ratios `f` reads for `side`'s battler this turn
    /// (`(1, 1)` without one).
    pub(crate) fn rule_ratio(
        &self,
        side: Side,
        f: impl Fn(What) -> Option<(u8, u8)>,
    ) -> (u32, u32) {
        let Some(rule) = self.x.rule else {
            return (1, 1);
        };
        let turn = self.turn1();
        let b = self.battler(side);
        let (mut num, mut den) = (1u32, 1u32);
        for e in rule.effects {
            let When::During(t) = e.when else { continue };
            if !t.matches(turn)
                || !e.who.sides.has(side.is_player())
                || !e.who.types.matches(b.types)
                || (e.who.last && !self.is_last(side))
            {
                continue;
            }
            if let Some((n, d)) = f(e.what) {
                num *= n as u32;
                den *= d.max(1) as u32;
            }
        }
        (num, den)
    }

    /// The first `When::During` effect `f` selects for `side`'s battler this turn.
    pub(crate) fn rule_during(&self, side: Side, f: impl Fn(What) -> bool) -> Option<RuleEffect> {
        let rule = self.x.rule?;
        let turn = self.turn1();
        let b = self.battler(side);
        rule.effects
            .iter()
            .find(|e| {
                matches!(e.when, When::During(t) if t.matches(turn))
                    && e.who.sides.has(side.is_player())
                    && e.who.types.matches(b.types)
                    && (!e.who.last || self.is_last(side))
                    && f(e.what)
            })
            .copied()
    }

    /// `What::DrainDealt` on `attacker` after it dealt `dealt` HP with a move.
    pub(crate) fn rule_drain(&mut self, env: &mut Env, attacker: Side, dealt: u16) {
        if dealt == 0 || self.x.rule.is_none() || self.battler(attacker).mon.hp == 0 {
            return;
        }
        let Some(e) = self.rule_during(attacker, |w| matches!(w, What::DrainDealt { .. })) else {
            return;
        };
        let What::DrainDealt { num, den } = e.what else {
            return;
        };
        let loss = (dealt as u32 * num as u32 / den.max(1) as u32).max(1) as i32;
        if let Some(t) = e.text {
            self.say(env, t, e.var, attacker);
        }
        self.change_hp_passive(env, attacker, loss);
        self.try_faint(env, attacker);
    }

    /// `What::SoundFails` for `attacker`'s move.
    pub(crate) fn rule_sound_fails(&self, attacker: Side) -> Option<RuleEffect> {
        self.x.rule?;
        self.rule_during(attacker, |w| w == What::SoundFails)
    }

    /// `What::ConfusionWard` on `side`, just confused by a move: the ward's roll, then either
    /// the confusion is gone (with the text) or it lasts at most `max_turns`.
    pub(crate) fn rule_confusion_ward(&mut self, env: &mut Env, side: Side) {
        if self.x.rule.is_none() {
            return;
        }
        let Some(e) = self.rule_during(side, |w| matches!(w, What::ConfusionWard { .. })) else {
            return;
        };
        let What::ConfusionWard { chance, max_turns } = e.what else {
            return;
        };
        if chance >= 100 || (chance > 0 && (env.rng.random() % 100) < chance as u16) {
            self.battler_mut(side).confusion = 0;
            if let Some(t) = e.text {
                self.say(env, t, e.var, side);
            }
        } else if max_turns > 0 {
            let b = self.battler_mut(side);
            b.confusion = b.confusion.min(max_turns);
        }
    }

    // ---- trainer lines ----

    /// Says the trainer's first unsaid line that `pick` selects (each line once).
    fn line(&mut self, env: &mut Env, side: Side, pick: impl Fn(LineWhen) -> bool) {
        if side.is_player() || !self.is_trainer() {
            return;
        }
        let lines = self.x.lines;
        for (k, l) in lines.iter().enumerate().take(16) {
            if self.x.said & (1 << k) == 0 && pick(l.when) {
                self.x.said |= 1 << k;
                self.say(env, l.text, Var::Mon, side);
                return;
            }
        }
    }

    /// The trainer's lines before it sends out party slot `party` on `side`.
    pub(crate) fn line_send_out(&mut self, env: &mut Env, side: Side, party: u8) {
        if self.x.lines.is_empty() {
            return;
        }
        self.line(env, side, |w| w == LineWhen::SendOut(party));
        let others = self.party_range(side).filter(|&i| i != party).any(|i| {
            self.parties[side.s()]
                .get(i as usize)
                .is_some_and(|m| m.hp > 0)
        });
        let len = self.party_range(side).len();
        if !others && len > 1 {
            self.line(env, side, |w| w == LineWhen::LastSendOut);
        }
    }

    /// The trainer's line before its mon Mega Evolves.
    pub(crate) fn line_mega(&mut self, env: &mut Env, side: Side) {
        if !self.x.lines.is_empty() {
            self.line(env, side, |w| w == LineWhen::Mega);
        }
    }

    /// The trainer's line once one of its mons on the field is under half its max HP.
    pub(crate) fn line_low_hp(&mut self, env: &mut Env) {
        if self.x.lines.is_empty() {
            return;
        }
        for side in self.sides().filter(|s| !s.is_player()).collect::<Vec<_>>() {
            let b = self.battler(side);
            if self.is_present(side) && b.mon.hp > 0 && (b.mon.hp as u32) * 2 < b.mon.max_hp as u32
            {
                self.line(env, side, |w| w == LineWhen::LowHp);
            }
        }
    }

    // ---- trainer items ----

    /// Gen3 `ShouldUseItem` for `side` (a trainer's battler): the index of the trainer item
    /// to use now, taken from the list.
    pub(crate) fn ai_item(&mut self, side: Side) -> Option<u8> {
        if self.x.items == [None; MAX_TRAINER_ITEMS] || !self.is_trainer() || side.is_player() {
            return None;
        }
        let valid = self
            .party_range(side)
            .filter(|&i| {
                self.parties[side.s()]
                    .get(i as usize)
                    .is_some_and(|m| m.hp > 0)
            })
            .count();
        let b = *self.battler(side);
        for i in 0..MAX_TRAINER_ITEMS {
            if i != 0 && valid > (MAX_TRAINER_ITEMS - i) + 1 {
                continue;
            }
            let Some((_, usage)) = self.x.items[i] else {
                continue;
            };
            let kind = ai_kind(usage)?;
            let (hp, max) = (b.mon.hp, b.mon.max_hp);
            let use_it = match kind {
                AiItem::FullRestore => hp != 0 && hp < max / 4,
                AiItem::HealHp(n) => {
                    hp != 0 && (hp < max / 4 || (max - hp) as u32 > n.min(255) as u32)
                }
                AiItem::Cure(bits) => {
                    let s = b.mon.status;
                    (bits & CURE_SLEEP != 0 && matches!(s, Status::Sleep(_)))
                        || (bits & CURE_POISON != 0
                            && matches!(s, Status::Poison | Status::Toxic(_)))
                        || (bits & CURE_BURN != 0 && s == Status::Burn)
                        || (bits & CURE_FREEZE != 0 && s == Status::Freeze)
                        || (bits & CURE_PARALYSIS != 0 && s == Status::Paralysis)
                        || (bits & CURE_CONFUSION != 0 && b.confusion > 0)
                }
                AiItem::XStat(..) => b.x.first_turn != 0,
            };
            if use_it {
                return Some(i as u8);
            }
        }
        None
    }

    /// Whether the trainer's items need `isFirstTurn` (an X item or Guard Spec).
    pub(crate) fn items_read_first_turn(&self) -> bool {
        self.x
            .items
            .iter()
            .flatten()
            .any(|&(_, u)| matches!(ai_kind(u), Some(AiItem::XStat(..))))
    }

    /// `HandleAction_UseItem` for a trainer: `BattleScript_OpponentUses*` with item `k`.
    pub(crate) fn use_trainer_item(&mut self, env: &mut Env, side: Side, k: u8) {
        let Some((item, usage)) = self.x.items[k as usize].take() else {
            return;
        };
        // ClearFuryCutterDestinyBondGrudge.
        self.battler_mut(side).v.fury_cutter = 0;
        let item_msg = |id: Str| Msg {
            buffs: [Buff::None, Buff::None, Buff::Item(item)],
            wait: LONG,
            ..about(id, side)
        };
        env.out
            .push(BattleEvent::Msg(item_msg(Str::Trainer1UsedItem)));
        let b = *self.battler(side);
        match ai_kind(usage) {
            Some(AiItem::FullRestore) | Some(AiItem::HealHp(_)) => {
                let (heal, cure) = match usage {
                    ItemUse::Medicine(MedicineEffect::HealCure(n, c)) => (n, c),
                    ItemUse::Medicine(MedicineEffect::Heal(n)) => (n, 0),
                    _ => (0, 0),
                };
                if cure != 0 {
                    self.cure(side, cure);
                }
                let to = (b.mon.hp as u32 + heal as u32).min(b.mon.max_hp as u32);
                self.change_hp_passive(env, side, -((to - b.mon.hp as u32) as i32));
                env.out
                    .push(BattleEvent::Msg(item_msg(Str::ItemRestoredHealth)));
                let status = self.battler(side).mon.status;
                env.out.push(BattleEvent::StatusIcon { side, status });
            }
            Some(AiItem::Cure(bits)) => {
                // gTrainerItemCuredStatusStringIds by `AI_itemFlags`' first bit.
                let s = b.mon.status;
                let id = if bits & CURE_CONFUSION != 0 && b.confusion > 0 {
                    Str::ItemSnappedOut
                } else if s == Status::Paralysis {
                    Str::ItemCuredParalysis
                } else if s == Status::Freeze {
                    Str::ItemDefrostedIt
                } else if s == Status::Burn {
                    Str::ItemHealedBurn
                } else if matches!(s, Status::Poison | Status::Toxic(_)) {
                    Str::ItemCuredPoison
                } else {
                    Str::ItemWokeIt
                };
                self.cure(side, bits);
                env.out.push(BattleEvent::Msg(item_msg(id)));
                let status = self.battler(side).mon.status;
                env.out.push(BattleEvent::StatusIcon { side, status });
            }
            Some(AiItem::XStat(stat, n)) => {
                let stat = Stat::ALL[(stat as usize).min(7)];
                if self.change_stage(side, stat, n) {
                    env.out.push(BattleEvent::StatAnim { side, up: true });
                }
                let m = self.stat_msg_sides(side, side, side, stat, n.unsigned_abs(), true);
                env.out.push(BattleEvent::Msg(m));
            }
            None => {}
        }
        self.write_back(side);
    }

    /// Cures the statuses `bits` names (`ITEM3_*`) on `side`'s battler.
    fn cure(&mut self, side: Side, bits: u8) {
        let b = self.battler_mut(side);
        let s = b.mon.status;
        let gone = (bits & CURE_SLEEP != 0 && matches!(s, Status::Sleep(_)))
            || (bits & CURE_POISON != 0 && matches!(s, Status::Poison | Status::Toxic(_)))
            || (bits & CURE_BURN != 0 && s == Status::Burn)
            || (bits & CURE_FREEZE != 0 && s == Status::Freeze)
            || (bits & CURE_PARALYSIS != 0 && s == Status::Paralysis);
        if gone {
            b.mon.status = Status::None;
        }
        if bits & CURE_CONFUSION != 0 {
            b.confusion = 0;
        }
    }
}

/// For the hash: the trainer items left.
pub(crate) fn items_code(items: &[Option<(ItemId, ItemUse)>; MAX_TRAINER_ITEMS]) -> [u16; 4] {
    items.map(|i| i.map_or(0, |(id, _)| id.0))
}
