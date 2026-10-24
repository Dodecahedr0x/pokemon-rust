//! Battle message expansion. **Task F.**
//!
//! Gen3 `BattleStringExpandPlaceholders` (`battle_message.c`) for the `B_TXT_*` codes the
//! [`Str`]s use, with `B_BUFF1..3` from [`Msg::buffs`] (`ExpandBattleTextBuffPlaceholders`).
//!
//! Mapping of the Gen3 battlers onto a [`Msg`]:
//! - `B_ATK_*` (attacker): `msg.attacker`; `B_DEF_*` (target): `msg.target`;
//! - `B_EFF_NAME_WITH_PREFIX` (`gEffectBattler`): `msg.target` (the core puts the battler a
//!   secondary effect lands on there);
//! - `B_ACTIVE_NAME_WITH_PREFIX`, `B_SCR_ACTIVE_NAME_WITH_PREFIX` and `B_TXT_26`: `msg.active`;
//! - `B_CURRENT_MOVE` / `B_LAST_MOVE`: `msg.mov`;
//! - the ability codes (`B_ATK_ABILITY`, `B_DEF_ABILITY`, `B_SCR_ACTIVE_ABILITY`, …):
//!   `msg.buffs[2]`, set by the core to `Buff::Text(Str::AbilityName(..))` (Task E2).
//!
//! The battler name codes (`*_NAME_WITH_PREFIX`, `B_PLAYER/OPPONENT_MON1_NAME`) name the party
//! slot each side had out when the core emitted the message (`msg.slots`, falling back to
//! `names.mons` for a slot the party lacks), so a message queued before a `SendOut` names the
//! mon that was out, as Gen3 does. Messages about a given party slot (send-outs, faints, exp,
//! level ups) also carry a buffer:`Buff::Mon { side, party }` (`B_BUFF_MON_NICK_WITH_PREFIX`) or
//! `Buff::MonNick { side, party }` (`B_BUFF_MON_NICK`, no wild/foe tag, as
//! `sText_Trainer1SentOutPkmn2`). Tasks D/E2: send-out messages use `Buff::MonNick`, never the
//! prefixed battler codes.
//!
//! The wild/foe tag: Gen3 copies `sText_WildPkmnPrefix` / `sText_FoePkmnPrefix` before the
//! nickname. The French ROM has " sauvage" / " ennemi" there, written after the name, so a tag
//! that starts with a space is a suffix and any other tag a prefix ("Wild " in English).
//!
//! `B_LAST_ITEM` names the `Buff::Item` in `msg.buffs[2]` (the held-item messages).
//!
//! Codes outside the slice (link names, trainer lose/win texts, the
//! `*_PREFIX1..3` team tags, a second trainer) expand to nothing.

use super::{Buff, Msg, Names, Side, Str};
use crate::data::GameData;
use crate::text::unit_len;
use alloc::vec::Vec;

const EOS: u8 = 0xFF;
const PLACEHOLDER: u8 = 0xFD;
const SPACE: u8 = 0x00;
const CHAR_0: u8 = 0xA1;

// `B_TXT_*` (include/battle_message.h).
const B_TXT_BUFF1: u8 = 0x00;
const B_TXT_BUFF2: u8 = 0x01;
const B_TXT_PLAYER_MON1_NAME: u8 = 0x05;
const B_TXT_OPPONENT_MON1_NAME: u8 = 0x06;
const B_TXT_PLAYER_MON2_NAME: u8 = 0x07;
const B_TXT_OPPONENT_MON2_NAME: u8 = 0x08;
const B_TXT_ATK_NAME_WITH_PREFIX_MON1: u8 = 0x0D;
const B_TXT_ATK_NAME_WITH_PREFIX: u8 = 0x0F;
const B_TXT_DEF_NAME_WITH_PREFIX: u8 = 0x10;
const B_TXT_EFF_NAME_WITH_PREFIX: u8 = 0x11;
const B_TXT_ACTIVE_NAME_WITH_PREFIX: u8 = 0x12;
const B_TXT_SCR_ACTIVE_NAME_WITH_PREFIX: u8 = 0x13;
const B_TXT_CURRENT_MOVE: u8 = 0x14;
const B_TXT_LAST_MOVE: u8 = 0x15;
const B_TXT_LAST_ITEM: u8 = 0x16;
const B_TXT_LAST_ABILITY: u8 = 0x17;
const B_TXT_EFF_ABILITY: u8 = 0x1B;
const B_TXT_TRAINER1_CLASS: u8 = 0x1C;
const B_TXT_TRAINER1_NAME: u8 = 0x1D;
const B_TXT_PLAYER_NAME: u8 = 0x23;
const B_TXT_26: u8 = 0x26;
const B_TXT_TRAINER2_CLASS: u8 = 0x2E;
const B_TXT_TRAINER2_NAME: u8 = 0x2F;
const B_TXT_BUFF3: u8 = 0x34;

/// Gen3 `BattleStringExpandPlaceholders` of `game.battle_text(msg.id)`. Returns Gen3-encoded
/// text, 0xFF-terminated.
pub fn expand(game: &impl GameData, msg: &Msg, names: &Names) -> Vec<u8> {
    if let Str::Authored(t) = msg.id {
        return expand_authored(game, game.text(t), msg, names);
    }
    expand_src(game, game.battle_text(msg.id), msg, names)
}

/// The field text codes of an authored text ([`Str::Authored`]): `{PLAYER}` (`0xFD 0x01`) is
/// the player's name and `{STR_VAR_1..3}` (`0xFD 0x02..=0x04`) expand `msg.buffs[0..3]`; the
/// other codes are kept as they are.
fn expand_authored(game: &impl GameData, src: &[u8], msg: &Msg, names: &Names) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() + 24);
    let mut i = 0;
    while i < src.len() && src[i] != EOS {
        let n = unit_len(src, i);
        match (src[i], src.get(i + 1).copied()) {
            (PLACEHOLDER, Some(1)) => push(&mut out, names.player),
            (PLACEHOLDER, Some(k @ 2..=4)) => {
                buff(game, &msg.buffs[k as usize - 2], names, &mut out)
            }
            _ => out.extend_from_slice(&src[i..(i + n).min(src.len())]),
        }
        i += n;
    }
    out.push(EOS);
    out
}

/// [`expand`] of an explicit source string (Gen3-encoded, 0xFF-terminated).
pub fn expand_src(game: &impl GameData, src: &[u8], msg: &Msg, names: &Names) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() + 24);
    let mut i = 0;
    while i < src.len() && src[i] != EOS {
        let n = unit_len(src, i);
        if src[i] == PLACEHOLDER {
            let code = src.get(i + 1).copied().unwrap_or(EOS);
            placeholder(game, code, msg, names, &mut out);
        } else {
            out.extend_from_slice(&src[i..(i + n).min(src.len())]);
        }
        i += n;
    }
    out.push(EOS);
    out
}

/// Appends `s` up to its 0xFF.
fn push(out: &mut Vec<u8>, s: &[u8]) {
    out.extend(s.iter().take_while(|&&b| b != EOS));
}

fn placeholder(game: &impl GameData, code: u8, msg: &Msg, names: &Names, out: &mut Vec<u8>) {
    let nick = |side: Side| party_nick(side, msg.slots[side.i()], names);
    match code {
        B_TXT_BUFF1 => buff(game, &msg.buffs[0], names, out),
        B_TXT_BUFF2 => buff(game, &msg.buffs[1], names, out),
        B_TXT_BUFF3 => buff(game, &msg.buffs[2], names, out),
        B_TXT_PLAYER_MON1_NAME => push(out, nick(Side::Player)),
        B_TXT_OPPONENT_MON1_NAME => push(out, nick(Side::Opponent)),
        B_TXT_PLAYER_MON2_NAME => push(out, nick(Side::PlayerRight)),
        B_TXT_OPPONENT_MON2_NAME => push(out, nick(Side::OpponentRight)),
        B_TXT_ATK_NAME_WITH_PREFIX_MON1 | B_TXT_ATK_NAME_WITH_PREFIX => {
            name_with_prefix(game, msg.attacker, msg, names, out)
        }
        B_TXT_DEF_NAME_WITH_PREFIX | B_TXT_EFF_NAME_WITH_PREFIX => {
            name_with_prefix(game, msg.target, msg, names, out)
        }
        B_TXT_ACTIVE_NAME_WITH_PREFIX | B_TXT_SCR_ACTIVE_NAME_WITH_PREFIX | B_TXT_26 => {
            name_with_prefix(game, msg.active, msg, names, out)
        }
        B_TXT_CURRENT_MOVE | B_TXT_LAST_MOVE => {
            if let Some(m) = msg.mov {
                move_name(game, m, out);
            }
        }
        // B_LAST/ATK/DEF/SCR_ACTIVE/EFF_ABILITY: the ability the message names, which the core
        // puts in `buffs[2]` (`Buff::Text(Str::AbilityName(..))`; B_BUFF3 is unused by these
        // strings).
        B_TXT_LAST_ABILITY..=B_TXT_EFF_ABILITY => buff(game, &msg.buffs[2], names, out),
        // `gLastUsedItem`: the item the core puts in `buffs[2]` (`Buff::Item`).
        B_TXT_LAST_ITEM => buff(game, &msg.buffs[2], names, out),
        B_TXT_TRAINER1_CLASS => push(out, names.trainer_class),
        B_TXT_TRAINER1_NAME => push(out, names.trainer_name),
        B_TXT_TRAINER2_CLASS => push(out, names.trainer2_class),
        B_TXT_TRAINER2_NAME => push(out, names.trainer2_name),
        B_TXT_PLAYER_NAME => push(out, names.player),
        _ => {}
    }
}

/// `HANDLE_NICKNAME_STRING_CASE` for the battler on `side`: the party slot it had when the
/// message was emitted (`msg.slots`).
fn name_with_prefix(game: &impl GameData, side: Side, msg: &Msg, names: &Names, out: &mut Vec<u8>) {
    let nick = party_nick(side, msg.slots[side.i()], names);
    tagged(game, side, nick, names, out);
}

/// The nickname of party slot `party` of `side` (the battler's if there is no such slot).
fn party_nick<'a>(side: Side, party: u8, names: &'a Names) -> &'a [u8] {
    names.parties[side.s()]
        .get(party as usize)
        .map_or(&names.mons[side.i()].nickname[..], |p| &p.nickname[..])
}

/// `nick`, with the wild or foe tag for an opponent.
fn tagged(game: &impl GameData, side: Side, nick: &[u8], names: &Names, out: &mut Vec<u8>) {
    if side.is_player() {
        push(out, nick);
        return;
    }
    let tag = game.battle_text(if names.wild {
        Str::WildPrefix
    } else {
        Str::FoePrefix
    });
    if tag.first() == Some(&SPACE) {
        push(out, nick);
        push(out, tag);
    } else {
        push(out, tag);
        push(out, nick);
    }
}

fn move_name(game: &impl GameData, m: crate::data::MoveId, out: &mut Vec<u8>) {
    if let Some(info) = game.battle().moves.get(m.0 as usize) {
        push(out, info.name);
    }
}

/// `ExpandBattleTextBuffPlaceholders` for one buffer.
fn buff(game: &impl GameData, b: &Buff, names: &Names, out: &mut Vec<u8>) {
    match *b {
        Buff::None => {}
        Buff::Stat(s) => push(out, game.battle_text(Str::StatName(s))),
        Buff::Type(t) => push(out, game.battle_text(Str::TypeName(t))),
        Buff::Move(m) => move_name(game, m, out),
        Buff::Number(n) => number(n, out),
        Buff::Species(s) => {
            if let Some(info) = game.battle().species.get(s.0 as usize) {
                push(out, info.name);
            }
        }
        // `B_BUFF_MON_NICK_WITH_PREFIX` / `B_BUFF_MON_NICK`.
        Buff::Mon { side, party } => tagged(game, side, party_nick(side, party, names), names, out),
        Buff::MonNick { side, party } => push(out, party_nick(side, party, names)),
        Buff::Text(s) => push(out, game.battle_text(s)),
        Buff::TrainerName => push(out, names.trainer_name),
        Buff::Item(i) => {
            if let Some(info) = game.item(i) {
                push(out, info.name);
            }
        }
    }
}

/// `ConvertIntToDecimalStringN(.., STR_CONV_MODE_LEFT_ALIGN, ..)`: no leading zeros.
fn number(n: u32, out: &mut Vec<u8>) {
    let mut digits = [0u8; 10];
    let mut len = 0;
    let mut v = n;
    loop {
        digits[len] = CHAR_0 + (v % 10) as u8;
        len += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    out.extend(digits[..len].iter().rev());
}

#[cfg(test)]
mod tests;
