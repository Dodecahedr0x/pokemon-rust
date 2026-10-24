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
        slots: [0; 4],
    }
}

fn names<'a>([p, o]: [&'a BattleMon; 2], wild: bool) -> Names<'a> {
    Names {
        player: b"\xC3\xD5\xFF", // "Ia"
        trainer_class: b"\xBF\xC5\xFF",
        trainer_name: b"\xC7\xD5\xED\xFF",
        trainer2_class: &[0xFF],
        trainer2_name: &[0xFF],
        wild,
        mons: [p, o, p, o],
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
        slots: [1; 4],
        ..msg([Buff::None; 3])
    };
    let out = expand_src(&g, src, &m, &names([&p, &o], true));
    assert_eq!(out, enc("SLUG wildZAP"));
    // A slot the party lacks falls back to the battler.
    let m = Msg {
        slots: [5; 4],
        ..msg([Buff::None; 3])
    };
    let out = expand_src(&g, src, &m, &names([&p, &o], true));
    assert_eq!(out, enc("POOCHY wildTORCHY"));
}
