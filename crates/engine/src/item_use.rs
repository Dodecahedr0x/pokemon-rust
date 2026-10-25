//! Item use out of battle (Gen3 `item_use.c`; Milestone 9 Task B).
//!
//! The field hooks `World::control` calls:
//! - [`on_step`] once per finished player step that started no coord or warp script, after the
//!   friendship counter and before the wild encounter (Gen3 `UpdateRepelCounter` in
//!   `TryStartStepBasedScript`): the repel counter; the wear-off script
//!   (`SpecialIds::script_repel_wore_off`) starts instead of the encounter check;
//! - [`select_pressed`] on SELECT while the field is idle (Gen3 `UseRegisteredKeyItemOnField`):
//!   the registered key item;
//! - [`use_on_field`] when the bag closes with `ScreenOut::UseOnField` (Gen3
//!   `SetUpItemUseOnFieldCallback`): the Escape Rope's warp.
//!
//! The bag's own uses run inside the screens: the bag (`ui/bag.rs`) decides what USE opens
//! ([`field_use`]) and the party menu applies an item to a mon with [`use_on_mon`] (Gen3
//! `ItemUseCB_Medicine`, `ItemUseCB_RareCandy`, `ItemUseCB_PPRecovery`,
//! `ItemUseCB_EvolutionStone`, `ExecuteTableBasedItemEffect`).
//!
//! Deviations: Ether and PP Up restore or raise the first move that can take it (Gen3 asks
//! which move: `Report::Degraded("item move choice")`); Rare Candy learns its level's moves into
//! free slots only (a full moveset reports `Degraded("rare candy move replace")`). A level-up,
//! a stone or a trade that would evolve returns [`EvolutionPlay`] and leaves the species for
//! the evolution scene. The Escape Rope has no spin effect and is allowed on
//! `map_type::UNDERGROUND` maps (Gen3 reads the header's `allowEscaping`, which `MapData` does
//! not carry) holding an escape warp.

use crate::data::{
    EvoMethod, GameData, ItemId, ItemUse, KeyUse, MedicineEffect, ScriptId, SpeciesId, UiText,
    map_type,
};
use crate::mon::{MAX_LEVEL, Pokemon, Status, exp_for_level};
use crate::rng::Rng;
use crate::script::Report;
use crate::world::State;
use crate::world::friendship::{self, FriendshipEvent};
use alloc::vec::Vec;

/// [`MedicineEffect::Cure`] bits (Gen3 `ITEM3_*`, one bit each in this order).
pub const CURE_SLEEP: u8 = 1 << 0;
pub const CURE_POISON: u8 = 1 << 1;
pub const CURE_BURN: u8 = 1 << 2;
pub const CURE_FREEZE: u8 = 1 << 3;
pub const CURE_PARALYSIS: u8 = 1 << 4;
pub const CURE_CONFUSION: u8 = 1 << 5;
pub const CURE_INFATUATION: u8 = 1 << 6;
/// Gen3 `ITEM3_STATUS_ALL` (Full Heal, Lava Cookie, Lum Berry; with infatuation for the
/// Full Restore): the five major statuses and confusion.
pub const CURE_ALL: u8 = CURE_SLEEP | CURE_POISON | CURE_BURN | CURE_FREEZE | CURE_PARALYSIS;

/// Gen3 `EV_ITEM_RAISE_LIMIT`: a vitamin works while the stat's EVs are below it.
pub const EV_ITEM_LIMIT: u8 = 100;
/// Gen3 `MAX_TOTAL_EVS`.
const MAX_TOTAL_EVS: u16 = 510;

/// What USE does for an item in the field bag (Gen3 `ItemId_GetFieldFunc`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldUse {
    /// Opens the party menu on the item (medicine, stones, TMs and HMs).
    Party,
    /// The Repel family (`ItemUseOutOfBattle_Repel`): this many steps.
    Repel(u16),
    /// Closes the bag and runs the item's field use (`ItemUseOutOfBattle_EscapeRope`).
    OnField,
    /// Closes the bag and runs the script (content-defined key items).
    Script(ScriptId),
    /// Reported, nothing happens (the bikes, rods and other key uses of Milestone 10).
    Unsupported(&'static str),
    /// `ItemUseOutOfBattle_CannotUse` ("not the time to use that").
    CannotUse,
}

/// What USE does for `item` in the field (Gen3's field function), from `GameData::item_use`;
/// a TM or HM is recognised by `GameData::tm_move` too.
pub fn field_use(game: &impl GameData, item: ItemId) -> FieldUse {
    if game.tm_move(item).is_some() {
        return FieldUse::Party;
    }
    match game.item_use(item) {
        ItemUse::Medicine(_) | ItemUse::Evolution | ItemUse::TradeEvolution | ItemUse::Tm(_) => {
            FieldUse::Party
        }
        ItemUse::Repel(n) => FieldUse::Repel(n),
        ItemUse::EscapeRope => FieldUse::OnField,
        ItemUse::Key(k) => match k {
            KeyUse::Bike => FieldUse::Unsupported("bike"),
            KeyUse::Rod(_) => FieldUse::Unsupported("rod"),
            KeyUse::Script(id) => FieldUse::Script(id),
            KeyUse::None => FieldUse::CannotUse,
            KeyUse::Other(name) => FieldUse::Unsupported(name),
        },
        ItemUse::Other(name) => FieldUse::Unsupported(name),
        ItemUse::Ball(_) | ItemUse::StatStage(..) | ItemUse::Berry | ItemUse::None => {
            FieldUse::CannotUse
        }
    }
}

/// Whether the item has a battle use (Gen3 `GetItemBattleUsage`): medicine (but Rare Candy,
/// vitamins and PP Ups), balls, X items.
pub fn battle_usable(game: &impl GameData, item: ItemId) -> bool {
    match game.item_use(item) {
        ItemUse::Ball(_) | ItemUse::StatStage(..) => true,
        ItemUse::Medicine(m) => !matches!(
            m,
            MedicineEffect::LevelUp | MedicineEffect::Ev(..) | MedicineEffect::PpUp(_)
        ),
        _ => false,
    }
}

/// Whether Escape Rope works here (Gen3 `CanUseDigOrEscapeRopeOnCurMap`, see the module's
/// deviations) and where it leads.
pub(crate) fn escape_rope_dest(s: &State, game: &impl GameData) -> Option<crate::world::WarpDest> {
    let map = game.map(s.field.map());
    if map.map_type != map_type::UNDERGROUND {
        return None;
    }
    s.warp_slots.escape.filter(|d| game.has_map(d.map))
}

/// The evolution scene to open. The species has not changed yet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvolutionPlay {
    pub into: SpeciesId,
    /// Gen3 `canStopEvo`: true for a level-up (Rare Candy), false for a stone or a trade.
    pub can_cancel: bool,
}

/// The message after using an item on a mon, and what else happened.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MonUse {
    pub text: UiText,
    /// `{STR_VAR_2}` (a number, a level, a stat name), Gen3-encoded and 0xFF-terminated.
    pub var2: Vec<u8>,
    /// Open the evolution scene for this. `None` when the item did not evolve anything.
    pub scene: Option<EvolutionPlay>,
}

fn number(n: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(6);
    crate::ui::kit::widgets::push_number(&mut v, n, 3, false);
    v.push(0xFF);
    v
}

fn text_var(game: &impl GameData, t: UiText) -> Vec<u8> {
    let mut v = Vec::with_capacity(12);
    crate::ui::kit::widgets::push(&mut v, game.ui_text(t));
    v.push(0xFF);
    v
}

fn msg(text: UiText, var2: Vec<u8>) -> MonUse {
    MonUse {
        text,
        var2,
        scene: None,
    }
}

/// The status a [`MedicineEffect::Cure`]'s bits cure (out of battle: the major statuses).
fn cures(bits: u8, status: Status) -> bool {
    match status {
        Status::None => false,
        Status::Sleep(_) => bits & CURE_SLEEP != 0,
        Status::Poison | Status::Toxic(_) => bits & CURE_POISON != 0,
        Status::Burn => bits & CURE_BURN != 0,
        Status::Freeze => bits & CURE_FREEZE != 0,
        Status::Paralysis => bits & CURE_PARALYSIS != 0,
    }
}

/// Gen3 `GetMedicineItemEffectMessage` for a status cure.
fn cure_text(bits: u8, cured: Status) -> UiText {
    if bits & CURE_ALL == CURE_ALL {
        return UiText::ItemBecameHealthy;
    }
    match cured {
        Status::Sleep(_) => UiText::ItemWokeUp,
        Status::Poison | Status::Toxic(_) => UiText::ItemCuredPoison,
        Status::Burn => UiText::ItemBurnHealed,
        Status::Freeze => UiText::ItemThawed,
        _ => UiText::ItemCuredParalysis,
    }
}

/// Uses `item` on `mon` out of battle (Gen3 `ExecuteTableBasedItemEffect` and the party
/// menu's item callbacks): `None` when it would have no effect (Gen3 "It won't have any
/// effect."; the item is kept), else the message to show (the caller removes one item).
/// Reports go to `reports`.
pub fn use_on_mon(
    game: &impl GameData,
    mon: &mut Pokemon,
    item: ItemId,
    rng: &mut Rng,
    reports: &mut Vec<Report>,
) -> Option<MonUse> {
    let t = game.battle();
    let hold = mon.held.and_then(|i| game.hold_effect(i));
    let max = mon.stats[0];
    match game.item_use(item) {
        ItemUse::Medicine(m) => match m {
            MedicineEffect::Heal(n) => {
                if mon.hp == 0 || mon.hp >= max {
                    return None;
                }
                let healed = n.min(max - mon.hp);
                mon.hp += healed;
                Some(msg(UiText::ItemHpRestored, number(healed as u32)))
            }
            MedicineEffect::Cure(bits) => {
                let old = mon.status;
                if !cures(bits, old) {
                    return None;
                }
                mon.status = Status::None;
                Some(msg(cure_text(bits, old), Vec::new()))
            }
            MedicineEffect::HealCure(n, bits) => {
                if mon.hp == 0 {
                    return None;
                }
                let old = mon.status;
                let cure = cures(bits, old);
                if mon.hp >= max && !cure {
                    return None;
                }
                if cure {
                    mon.status = Status::None;
                }
                let healed = n.min(max - mon.hp);
                mon.hp += healed;
                Some(if healed > 0 {
                    msg(UiText::ItemHpRestored, number(healed as u32))
                } else {
                    msg(cure_text(bits, old), Vec::new())
                })
            }
            MedicineEffect::Revive(frac) => {
                if mon.hp != 0 {
                    return None;
                }
                let hp = ((max as u32 * frac as u32) / 256).clamp(1, max as u32) as u16;
                mon.hp = hp;
                mon.status = Status::None;
                Some(msg(UiText::ItemHpRestored, number(hp as u32)))
            }
            MedicineEffect::Pp(amount, all) => {
                let mut changed = false;
                for slot in 0..4u8 {
                    let cap = mon.max_pp(t, slot);
                    let pp = &mut mon.pp[slot as usize];
                    if mon.moves[slot as usize].is_none() || *pp >= cap {
                        continue;
                    }
                    *pp = if amount == u8::MAX {
                        cap
                    } else {
                        pp.saturating_add(amount).min(cap)
                    };
                    changed = true;
                    if !all {
                        reports.push(Report::Degraded("item move choice"));
                        break;
                    }
                }
                changed.then(|| msg(UiText::ItemPpRestored, Vec::new()))
            }
            MedicineEffect::PpUp(full) => {
                let slot = (0..4u8).find(|&s| {
                    let bonus = (mon.pp_bonuses >> (2 * s)) & 3;
                    // Gen3 `ITEM4_PP_UP`: a move whose base PP is under 5 takes none.
                    mon.moves[s as usize].is_some_and(|m| t.move_info(m).pp >= 5) && bonus < 3
                })?;
                reports.push(Report::Degraded("item move choice"));
                let old_max = mon.max_pp(t, slot);
                let bonus = if full {
                    3
                } else {
                    ((mon.pp_bonuses >> (2 * slot)) & 3) + 1
                };
                mon.pp_bonuses = (mon.pp_bonuses & !(3 << (2 * slot))) | (bonus << (2 * slot));
                let new_max = mon.max_pp(t, slot);
                let pp = &mut mon.pp[slot as usize];
                *pp = pp.saturating_add(new_max - old_max);
                Some(msg(UiText::ItemPpIncreased, Vec::new()))
            }
            MedicineEffect::LevelUp => {
                if mon.level >= MAX_LEVEL {
                    return None;
                }
                let growth = t.species_info(mon.species).growth;
                mon.exp = exp_for_level(growth, mon.level + 1);
                mon.recalc_stats(t);
                friendship::on_event(mon, FriendshipEvent::Vitamin, hold, t, rng);
                let level = mon.level;
                let new: Vec<_> = mon.moves_at_level(t, level).collect();
                for m in new {
                    if mon.try_learn(t, m) == crate::mon::Learn::Full {
                        reports.push(Report::Degraded("rare candy move replace"));
                    }
                }
                let mut out = msg(UiText::ItemElevatedToLevel, number(level as u32));
                if mon.held.and_then(|i| game.hold_effect(i))
                    != Some(crate::mon::HoldEffect::PreventEvolve)
                    && let Some((into, _)) = mon.evolution_target(game.evolutions(mon.species))
                {
                    // The scene changes the species. B can cancel a level-up (`canStopEvo`).
                    out.scene = Some(EvolutionPlay {
                        into,
                        can_cancel: true,
                    });
                }
                Some(out)
            }
            MedicineEffect::Ev(stat, amount) => {
                let i = stat as usize % 6;
                let total: u16 = mon.evs.iter().map(|&e| e as u16).sum();
                if mon.evs[i] >= EV_ITEM_LIMIT || total >= MAX_TOTAL_EVS {
                    return None;
                }
                let gain = (amount as u16)
                    .min((EV_ITEM_LIMIT - mon.evs[i]) as u16)
                    .min(MAX_TOTAL_EVS - total);
                mon.evs[i] += gain as u8;
                mon.recalc_stats(t);
                friendship::on_event(mon, FriendshipEvent::Vitamin, hold, t, rng);
                let name = [
                    UiText::SummaryHp,
                    UiText::SummaryAttack,
                    UiText::SummaryDefense,
                    UiText::SummarySpeed,
                    UiText::SummarySpAtk,
                    UiText::SummarySpDef,
                ][i];
                Some(msg(UiText::ItemStatIncreased, text_var(game, name)))
            }
        },
        ItemUse::Evolution => {
            let into = game
                .evolutions(mon.species)
                .iter()
                .find(|e| e.method == EvoMethod::Item && e.param == item.0)?
                .target;
            // A stone cannot be cancelled (`EVO_MODE_ITEM_USE`).
            Some(MonUse {
                text: UiText::ItemUsed,
                var2: Vec::new(),
                scene: Some(EvolutionPlay {
                    into,
                    can_cancel: false,
                }),
            })
        }
        ItemUse::TradeEvolution => {
            // `EVO_TRADE` (param 0) or `EVO_TRADE_ITEM` (param: the item the mon must hold);
            // an Everstone stops it as it stops a trade. A trade cannot be cancelled.
            if hold == Some(crate::mon::HoldEffect::PreventEvolve) {
                return None;
            }
            let e = game.evolutions(mon.species).iter().find(|e| {
                matches!(e.method, EvoMethod::Trade | EvoMethod::TradeItem)
                    && (e.param == 0 || mon.held.is_some_and(|h| h.0 == e.param))
            })?;
            if e.param != 0 {
                mon.held = None;
            }
            Some(MonUse {
                text: UiText::ItemUsed,
                var2: Vec::new(),
                scene: Some(EvolutionPlay {
                    into: e.target,
                    can_cancel: false,
                }),
            })
        }
        _ => None,
    }
}

/// Gen3 `UpdateRepelCounter`: one step off the repel; at zero, the wear-off script.
pub(crate) fn on_step(s: &mut State, game: &impl GameData) -> Option<ScriptId> {
    let ids = game.special_ids();
    let var = ids.var_repel_steps?;
    let steps = s.vars.var(var);
    if steps == 0 {
        return None;
    }
    s.vars.set_var(var, steps - 1);
    if steps == 1 {
        ids.script_repel_wore_off
    } else {
        None
    }
}

/// Gen3 `UseRegisteredKeyItemOnField`: the registered item's field use, if the bag still holds
/// it (else the registration is cleared, as Gen3). A script use starts; the bikes and rods are
/// Milestone 10 (`Report::Unsupported`). Nothing registered: nothing (Gen3's
/// `EventScript_SelectWithoutRegisteredItem` message has no `SpecialIds` role yet).
pub(crate) fn select_pressed(s: &mut State, game: &impl GameData) -> Option<ScriptId> {
    let item = s.registered_item?;
    if s.bag.count(item) == 0 {
        s.registered_item = None;
        return None;
    }
    match field_use(game, item) {
        FieldUse::Script(id) => Some(id),
        FieldUse::Unsupported(what) => {
            s.report(Report::Unsupported(what));
            None
        }
        _ => None,
    }
}

/// The field use of `item` once the bag has closed (Gen3 `ItemUseOnFieldCB_EscapeRope`): the
/// item is used up and the warp to the escape warp starts. A content script use runs.
pub(crate) fn use_on_field(s: &mut State, game: &impl GameData, item: ItemId) -> Option<ScriptId> {
    match field_use(game, item) {
        FieldUse::Script(id) => Some(id),
        FieldUse::OnField => {
            let dest = escape_rope_dest(s, game)?;
            let pocket = game.item(item)?.pocket;
            if !s.bag.remove(pocket, item, 1) {
                return None;
            }
            s.transition
                .start(game, dest.map, dest.warp, dest.x, dest.y);
            s.objects.freeze_all();
            None
        }
        _ => None,
    }
}
