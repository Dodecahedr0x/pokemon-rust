//! Battle-only forms (Silice): Mega Evolution and Primal Reversion, after Gen6/7 (no Gen3
//! counterpart).
//!
//! - The forms come from `GameData::form_changes`: a party mon whose species is a change's
//!   `base` and that holds its `item` may become `form` ([`Battle::find_forms`]). The player's
//!   also need the change's `enable_flag` (the World's flags, [`Battle::enable_player_forms`]);
//!   trainers ignore it. Wild mons never Mega Evolve (they may Primal Revert).
//! - Mega Evolution is chosen with the move (`Choice::Mega`, the AI at its first chance) and
//!   happens after the switches, items and running and before the first move action
//!   (expansion `TryDoGimmicksBeforeMoves`), faster battler first, so the new speed orders
//!   the moves (Gen7). One per side per battle. Simplified: a Pursuit on a switching foe
//!   strikes before the Mega.
//! - Primal Reversion happens when the holder enters the field (and at the battle's start, in
//!   speed order), before its switch-in abilities, and does not use the side's Mega.
//! - The change recomputes the battler's stats from the form's base stats (HP kept), takes the
//!   form's types and ability, and runs the new ability's switch-in effect. The party keeps its
//!   species: the form lasts until the battle ends (switching out keeps it).

use super::{Battle, Env, LONG, Side, about};
use crate::battle::{BattleEvent, Buff, Msg, Str};
use crate::data::{FlagId, FormKind, GameData};

impl Battle {
    /// Fills `side`'s `x.forms` from `game.form_changes()`: a mon holding a change's item; a
    /// player's change also needs `is_set` of its flag.
    pub(crate) fn find_forms(
        &mut self,
        game: &impl GameData,
        side: Side,
        is_set: impl Fn(FlagId) -> bool,
    ) {
        let changes = game.form_changes();
        if changes.is_empty() {
            return;
        }
        let s = side as usize;
        for (i, p) in self.parties[s].iter().enumerate() {
            let Some(item) = p.held else { continue };
            self.x.forms[s][i] = changes
                .iter()
                .find(|f| {
                    f.item == item
                        && f.base == p.species
                        && (side == Side::Opponent || f.enable_flag.is_none_or(&is_set))
                })
                .copied();
        }
    }

    /// Whether `side`'s battler may Mega Evolve now: a Mega form for its slot, not changed
    /// yet, the side's Mega unused, and not a wild mon.
    pub fn can_mega(&self, side: Side) -> bool {
        let s = side as usize;
        let b = self.battler(side);
        let slot = b.mon.party;
        b.mon.hp > 0
            && !self.x.mega_used[s]
            && self.x.formed[s] & (1 << slot) == 0
            && (side == Side::Player || self.is_trainer())
            && self.x.forms[s][slot as usize].is_some_and(|f| f.kind == FormKind::Mega)
    }

    /// The pending Mega Evolutions, faster battler first (full speed: Swift Swim and
    /// Chlorophyll count; a tie: the player first).
    pub(crate) fn mega_evolutions(&mut self, env: &mut Env) {
        if self.x.mega_pending == [false; 2] {
            return;
        }
        let order = if self.full_speed(Side::Opponent) > self.full_speed(Side::Player) {
            [Side::Opponent, Side::Player]
        } else {
            [Side::Player, Side::Opponent]
        };
        for side in order {
            if self.x.mega_pending[side as usize] && self.can_mega(side) {
                self.change_form(env, side);
                crate::battle::effects::on_switch_in(self, env, side);
                crate::battle::effects::switch_in(self, env, side);
            }
        }
        self.x.mega_pending = [false; 2];
    }

    /// Primal Reversion of `side`'s battler if its slot has a Primal form not taken yet. The
    /// caller runs the switch-in abilities next.
    pub(crate) fn primal_reversion(&mut self, env: &mut Env, side: Side) {
        let s = side as usize;
        let b = self.battler(side);
        let slot = b.mon.party;
        if b.mon.hp > 0
            && self.x.formed[s] & (1 << slot) == 0
            && self.x.forms[s][slot as usize].is_some_and(|f| f.kind == FormKind::Primal)
        {
            self.change_form(env, side);
        }
    }

    /// Changes `side`'s battler into its slot's form: messages, the new stats, types and
    /// ability, and the `FormChange` event (the caller runs the new ability's switch-in
    /// effect).
    fn change_form(&mut self, env: &mut Env, side: Side) {
        let s = side as usize;
        let slot = self.battler(side).mon.party;
        let Some(f) = self.x.forms[s][slot as usize] else {
            return;
        };
        let buffs = [
            Buff::Mon { side, party: slot },
            Buff::Species(f.form),
            Buff::Item(f.item),
        ];
        if f.kind == FormKind::Mega {
            env.out.push(BattleEvent::Msg(Msg {
                buffs,
                wait: LONG,
                ..about(Str::MegaReacting, side)
            }));
            self.x.mega_used[s] = true;
        }
        self.x.formed[s] |= 1 << slot;
        let old = *self.battler(side);
        let mut b = self.battler_for(env.t, side, slot);
        // A form change is not a switch: the battle state stays.
        b.mon.hp = old.mon.hp;
        b.mon.status = old.mon.status;
        b.mon.pp = old.mon.pp;
        b.x.item = old.x.item;
        b.x.hold = old.x.hold;
        b = super::Battler {
            stages: old.stages,
            confusion: old.confusion,
            flinched: old.flinched,
            focus_energy: old.focus_energy,
            seeded: old.seeded,
            foresight: old.foresight,
            bide_turns: old.bide_turns,
            charge: old.charge,
            bide_damage: old.bide_damage,
            last_move: old.last_move,
            last_landed: old.last_landed,
            fainted: old.fainted,
            v: old.v,
            ..b
        };
        b.x.first_turn = if b.ability == crate::mon::Ability::SpeedBoost {
            old.x.first_turn
        } else {
            0
        };
        self.battlers[s] = b;
        env.out.push(BattleEvent::FormChange {
            side,
            party: slot,
            species: f.form,
        });
        let id = match f.kind {
            FormKind::Mega => Str::MegaEvolved,
            FormKind::Primal => Str::PrimalReversion,
        };
        env.out.push(BattleEvent::Msg(Msg {
            buffs,
            wait: LONG,
            ..about(id, side)
        }));
    }
}
