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
//! Codes outside the slice (items, link names, trainer lose/win texts, the
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
const B_TXT_ATK_NAME_WITH_PREFIX_MON1: u8 = 0x0D;
const B_TXT_ATK_NAME_WITH_PREFIX: u8 = 0x0F;
const B_TXT_DEF_NAME_WITH_PREFIX: u8 = 0x10;
const B_TXT_EFF_NAME_WITH_PREFIX: u8 = 0x11;
const B_TXT_ACTIVE_NAME_WITH_PREFIX: u8 = 0x12;
const B_TXT_SCR_ACTIVE_NAME_WITH_PREFIX: u8 = 0x13;
const B_TXT_CURRENT_MOVE: u8 = 0x14;
const B_TXT_LAST_MOVE: u8 = 0x15;
const B_TXT_LAST_ABILITY: u8 = 0x17;
const B_TXT_EFF_ABILITY: u8 = 0x1B;
const B_TXT_TRAINER1_CLASS: u8 = 0x1C;
const B_TXT_TRAINER1_NAME: u8 = 0x1D;
const B_TXT_PLAYER_NAME: u8 = 0x23;
const B_TXT_26: u8 = 0x26;
const B_TXT_BUFF3: u8 = 0x34;

/// Gen3 `BattleStringExpandPlaceholders` of `game.battle_text(msg.id)`. Returns Gen3-encoded
/// text, 0xFF-terminated.
pub fn expand(game: &impl GameData, msg: &Msg, names: &Names) -> Vec<u8> {
    expand_src(game, game.battle_text(msg.id), msg, names)
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
    let nick = |side: Side| party_nick(side, msg.slots[side as usize], names);
    match code {
        B_TXT_BUFF1 => buff(game, &msg.buffs[0], names, out),
        B_TXT_BUFF2 => buff(game, &msg.buffs[1], names, out),
        B_TXT_BUFF3 => buff(game, &msg.buffs[2], names, out),
        B_TXT_PLAYER_MON1_NAME => push(out, nick(Side::Player)),
        B_TXT_OPPONENT_MON1_NAME => push(out, nick(Side::Opponent)),
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
        B_TXT_TRAINER1_CLASS => push(out, names.trainer_class),
        B_TXT_TRAINER1_NAME => push(out, names.trainer_name),
        B_TXT_PLAYER_NAME => push(out, names.player),
        _ => {}
    }
}

/// `HANDLE_NICKNAME_STRING_CASE` for the battler on `side`: the party slot it had when the
/// message was emitted (`msg.slots`).
fn name_with_prefix(game: &impl GameData, side: Side, msg: &Msg, names: &Names, out: &mut Vec<u8>) {
    let nick = party_nick(side, msg.slots[side as usize], names);
    tagged(game, side, nick, names, out);
}

/// The nickname of party slot `party` of `side` (the battler's if there is no such slot).
fn party_nick<'a>(side: Side, party: u8, names: &'a Names) -> &'a [u8] {
    names.parties[side as usize]
        .get(party as usize)
        .map_or(&names.mons[side as usize].nickname[..], |p| &p.nickname[..])
}

/// `nick`, with the wild or foe tag for an opponent.
fn tagged(game: &impl GameData, side: Side, nick: &[u8], names: &Names, out: &mut Vec<u8>) {
    if side == Side::Player {
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
mod tests {
    use super::*;
    use crate::battle::ui::test_game::*;
    use crate::battle::{BattleMon, MsgWait};
    use crate::mon::{Stat, Type};

    fn msg(buffs: [Buff; 3]) -> Msg {
        Msg {
            id: Str::UsedMove,
            attacker: Side::Player,
            target: Side::Opponent,
            active: Side::Player,
            mov: Some(EMBER),
            buffs,
            wait: MsgWait::Button,
            slots: [0, 0],
        }
    }

    fn names<'a>(mons: [&'a BattleMon; 2], wild: bool) -> Names<'a> {
        Names {
            player: b"\xC3\xD5\xFF", // "Ia"
            trainer_class: b"\xBF\xC5\xFF",
            trainer_name: b"\xC7\xD5\xED\xFF",
            wild,
            mons,
            parties: [party(&["TORCHY", "ZAP"]), party(&["POOCHY", "SLUG"])],
        }
    }

    #[test]
    fn party_buffers_name_the_slot_not_the_battler() {
        let g = game();
        let (p, o) = mons();
        // STRINGID_TARGETFAINTED-style message queued for opponent slot 0 ...
        let src = msg_text(&[&[FD, 0x00], &t(" fainted "), &[FD, 0x01]]);
        let m = msg([
            Buff::Mon {
                side: Side::Opponent,
                party: 0,
            },
            Buff::MonNick {
                side: Side::Opponent,
                party: 0,
            },
            Buff::None,
        ]);
        // ... played after `Names` already shows slot 1 (SLUG) as the opponent battler.
        let slug = mon("SLUG", POOCHY, [None; 4], [0; 4]);
        let out = expand_src(&g, src, &m, &names([&p, &slug], true));
        assert_eq!(out, enc("POOCHY wild fainted POOCHY"));
        // A send-out (`B_BUFF_MON_NICK`): no tag; player slots likewise.
        let src = msg_text(&[&t("Go "), &[FD, 0x00], &t(" "), &[FD, 0x01]]);
        let m = msg([
            Buff::MonNick {
                side: Side::Opponent,
                party: 1,
            },
            Buff::MonNick {
                side: Side::Player,
                party: 1,
            },
            Buff::None,
        ]);
        let out = expand_src(&g, src, &m, &names([&p, &o], false));
        assert_eq!(out, enc("Go SLUG ZAP"));
        // A slot past the party falls back to the battler.
        let m = msg([
            Buff::MonNick {
                side: Side::Player,
                party: 5,
            },
            Buff::None,
            Buff::None,
        ]);
        let src = msg_text(&[&[FD, 0x00]]);
        assert_eq!(
            expand_src(&g, src, &m, &names([&p, &o], true)),
            enc("TORCHY")
        );
    }

    fn mons() -> (BattleMon, BattleMon) {
        (
            mon(
                "TORCHY",
                TORCH,
                [Some(EMBER), None, None, None],
                [25, 0, 0, 0],
            ),
            mon(
                "POOCHY",
                POOCHY,
                [Some(TACKLE), None, None, None],
                [35, 0, 0, 0],
            ),
        )
    }

    fn cat(parts: &[&[u8]]) -> Vec<u8> {
        let mut v: Vec<u8> = parts.iter().flat_map(|p| p.iter().copied()).collect();
        v.push(0xFF);
        v
    }

    #[test]
    fn names_with_wild_and_foe_suffix() {
        let g = game();
        let (p, o) = mons();
        // "{B_ATK_NAME_WITH_PREFIX} used\n{B_CURRENT_MOVE}! {B_DEF_NAME_WITH_PREFIX}"
        let src = msg_text(&[
            &[FD, 0x0F],
            &t(" used"),
            &[0xFE, FD, 0x14],
            &t("! "),
            &[FD, 0x10],
        ]);
        let m = msg([Buff::None; 3]);
        let wild = expand_src(&g, src, &m, &names([&p, &o], true));
        let expect = cat(&[&t("TORCHY used"), &[0xFE], &t("EMBER! POOCHY wild")]);
        assert_eq!(wild, expect);
        let trainer = expand_src(&g, src, &m, &names([&p, &o], false));
        assert_eq!(trainer.len(), expect.len() - 1);
        assert!(trainer.ends_with(&cat(&[&t("POOCHY foe")])));
    }

    #[test]
    fn english_style_prefix_tag() {
        let g = Game {
            wild_tag: leak(enc("Wild ")),
            ..game()
        };
        let (p, o) = mons();
        let src = msg_text(&[&[FD, 0x10]]);
        let out = expand_src(&g, src, &msg([Buff::None; 3]), &names([&p, &o], true));
        assert_eq!(out, enc("Wild POOCHY"));
    }

    #[test]
    fn active_effect_and_mon1_names() {
        let g = game();
        let (p, o) = mons();
        let src = msg_text(&[&[FD, 0x12, FD, 0x11, FD, 0x05, FD, 0x06, FD, 0x13, FD, 0x0D]]);
        let m = Msg {
            active: Side::Opponent,
            attacker: Side::Opponent,
            target: Side::Player,
            ..msg([Buff::None; 3])
        };
        let out = expand_src(&g, src, &m, &names([&p, &o], true));
        assert_eq!(
            out,
            enc("POOCHY wildTORCHYTORCHYPOOCHYPOOCHY wildPOOCHY wild")
        );
    }

    #[test]
    fn trainer_and_player_names() {
        let g = game();
        let (p, o) = mons();
        // "{B_TRAINER1_CLASS} {B_TRAINER1_NAME}\nwants to battle! {B_PLAYER_NAME}"
        let src = msg_text(&[&[FD, 0x1C, 0x00, FD, 0x1D, 0xFE], &t("vs "), &[FD, 0x23]]);
        let out = expand_src(&g, src, &msg([Buff::None; 3]), &names([&p, &o], false));
        assert_eq!(out, cat(&[&t("EK May"), &[0xFE], &t("vs Ia")]));
    }

    #[test]
    fn stat_message_with_two_buffs() {
        let g = game();
        let (p, o) = mons();
        // STRINGID_ATTACKERSSTATROSE: "{B_ATK_NAME_WITH_PREFIX}'s {B_BUFF1}\n{B_BUFF2}"
        // with BUFF1 = PREPARE_STAT_BUFFER(ATK), BUFF2 = PREPARE_STRING_BUFFER(STRINGID_STATROSE).
        let src = msg_text(&[&[FD, 0x0F], &t(" "), &[FD, 0x00, 0xFE, FD, 0x01]]);
        let m = Msg {
            attacker: Side::Opponent,
            ..msg([Buff::Stat(Stat::Atk), Buff::Text(Str::StatRose), Buff::None])
        };
        let out = expand_src(&g, src, &m, &names([&p, &o], true));
        assert_eq!(out, cat(&[&t("POOCHY wild ATTACK"), &[0xFE], &t("rose!")]));
    }

    #[test]
    fn buffers_numbers_moves_species_types_mons() {
        let g = game();
        let (p, o) = mons();
        let src = msg_text(&[&[FD, 0x00], &t(" "), &[FD, 0x01], &t(" "), &[FD, 0x34]]);
        let names = names([&p, &o], true);
        let cases: [([Buff; 3], &str); 5] = [
            (
                [
                    Buff::Number(0),
                    Buff::Number(7),
                    Buff::Number(4_294_967_295),
                ],
                "0 7 4294967295",
            ),
            (
                [
                    Buff::Move(TACKLE),
                    Buff::Species(POOCHY),
                    Buff::Type(Type::Fire),
                ],
                "TACKLE POOCHY FIRE",
            ),
            (
                [
                    Buff::Mon {
                        side: Side::Player,
                        party: 0,
                    },
                    Buff::Mon {
                        side: Side::Opponent,
                        party: 0,
                    },
                    Buff::Number(105),
                ],
                "TORCHY POOCHY wild 105",
            ),
            ([Buff::None, Buff::None, Buff::None], "  "),
            (
                [
                    Buff::Stat(Stat::Atk),
                    Buff::Text(Str::FoePrefix),
                    Buff::None,
                ],
                "ATTACK  foe ",
            ),
        ];
        for (buffs, want) in cases {
            assert_eq!(
                expand_src(&g, src, &msg(buffs), &names),
                enc(want),
                "{want}"
            );
        }
    }

    #[test]
    fn codes_are_copied_whole_and_unknown_placeholders_vanish() {
        let g = game();
        let (p, o) = mons();
        // {COLOR 2} keeps its argument even when it looks like a placeholder byte; FD 0x18
        // (attacker ability) expands to nothing; {PAUSE 0xFD} is copied as is.
        let src = msg_text(&[&[0xFC, 0x01, 0xFD, FD, 0x18, 0xFC, 0x08, 0xFD, 0xBB]]);
        let out = expand_src(&g, src, &msg([Buff::None; 3]), &names([&p, &o], true));
        assert_eq!(out, [0xFC, 0x01, 0xFD, 0xFC, 0x08, 0xFD, 0xBB, 0xFF]);
    }

    #[test]
    fn expand_reads_the_string_table() {
        let g = game();
        let (p, o) = mons();
        let m = Msg {
            id: Str::WhatWillPkmnDo,
            ..msg([Buff::None; 3])
        };
        let out = expand(&g, &m, &names([&p, &o], true));
        assert_eq!(out, cat(&[&t("What will"), &[0xFE], &t("TORCHY do?")]));
        // No move: B_CURRENT_MOVE is empty.
        let src = msg_text(&[&[FD, 0x14]]);
        let none = Msg {
            mov: None,
            ..msg([Buff::None; 3])
        };
        assert_eq!(expand_src(&g, src, &none, &names([&p, &o], true)), [0xFF]);
    }

    #[test]
    fn battler_names_read_the_emitted_slots() {
        let g = game();
        let (p, o) = mons();
        // "{B_DEF_NAME_WITH_PREFIX}{B_PLAYER_MON1_NAME}": emitted while slot 1 of each side was
        // out; POOCHY / TORCHY (slot 0) are the battlers now.
        let src = msg_text(&[&[FD, 0x10, FD, 0x05]]);
        let m = Msg {
            slots: [1, 1],
            ..msg([Buff::None; 3])
        };
        let out = expand_src(&g, src, &m, &names([&p, &o], true));
        assert_eq!(out, enc("SLUG wildZAP"));
        // A slot the party lacks falls back to the battler.
        let m = Msg {
            slots: [5, 5],
            ..msg([Buff::None; 3])
        };
        let out = expand_src(&g, src, &m, &names([&p, &o], true));
        assert_eq!(out, enc("POOCHY wildTORCHY"));
    }
}
