//! The scene's hash encoding of its playback state and queued events.

use super::{Menu, Play, Send};
use crate::battle::{BattleEvent, Buff, MsgWait, Str};
use crate::data::MoveId;
use crate::hash::Fnv64;

pub(super) fn hash_play(p: Play, h: &mut Fnv64) {
    let opt = |o: Option<u8>| o.unwrap_or(0xFF);
    let bytes: [u8; 4] = match p {
        Play::Idle => [0, 0, 0, 0],
        Play::Msg => [1, 0, 0, 0],
        Play::Anims => [2, 0, 0, 0],
        Play::WildAppear(b) => [3, b as u8, 0, 0],
        Play::SendOut(side, s) => {
            let (k, n) = match s {
                Send::Wait(n) => (0, n),
                Send::Ball => (1, 0),
                Send::Box => (2, 0),
                Send::Delay(n) => (3, n),
            };
            [4, side.code(), k, n]
        }
        Play::Faint(side, n) => [5, side.code(), opt(n), 0],
        Play::Hp(side) => [6, side.code(), 0, 0],
        Play::Exp => [7, 0, 0, 0],
        Play::Bars => [8, 0, 0, 0],
        Play::Defeated(b, second) => [9, b as u8, second as u8, 0],
        Play::Ending => [10, 0, 0, 0],
        Play::Form(side) => [11, side.code(), 0, 0],
    };
    h.write(&bytes);
}

pub(super) fn hash_menu(m: Menu, h: &mut Fnv64) {
    let (tag, party, mov, n) = match m {
        Menu::None => (0, 0, MoveId(0), 0),
        Menu::Action => (1, 0, MoveId(0), 0),
        Menu::Move => (2, 0, MoveId(0), 0),
        Menu::LearnAsk { party, mov } => (3, party, mov, 0),
        Menu::LearnPick { party, mov } => (4, party, mov, 0),
        Menu::StopMsg { party, mov } => (5, party, mov, 0),
        Menu::StopAsk { party, mov } => (6, party, mov, 0),
        Menu::Retry { party, mov, n } => (7, party, mov, n),
        Menu::HmMsg { party, mov } => (8, party, mov, 0),
        Menu::Target { slot, mega } => (9, slot, MoveId(0), mega as u8),
    };
    h.write(&[tag, party, n]);
    h.write_u16(mov.0);
}

/// [`Str::code`].
fn str_code(s: Str, h: &mut Fnv64) {
    h.write_u16(s.code());
    if let Str::Authored(t) = s {
        h.write_u16(t.0);
    }
}

fn hash_buff(b: &Buff, h: &mut Fnv64) {
    match *b {
        Buff::None => h.write(&[0]),
        Buff::Stat(s) => h.write(&[1, s as u8]),
        Buff::Move(m) => {
            h.write(&[2]);
            h.write_u16(m.0);
        }
        Buff::Number(n) => {
            h.write(&[3]);
            h.write_u32(n);
        }
        Buff::Species(s) => {
            h.write(&[4]);
            h.write_u16(s.0);
        }
        Buff::Type(t) => h.write(&[5, t as u8]),
        Buff::Mon { side, party } => h.write(&[6, side.code(), party]),
        Buff::MonNick { side, party } => h.write(&[7, side.code(), party]),
        Buff::Text(s) => {
            h.write(&[8]);
            str_code(s, h);
        }
        Buff::Item(i) => {
            h.write(&[9]);
            h.write_u16(i.0);
        }
        Buff::TrainerName => h.write(&[10]),
    }
}

pub(super) fn hash_event(e: &BattleEvent, h: &mut Fnv64) {
    match *e {
        BattleEvent::Msg(m) => {
            h.write(&[0]);
            str_code(m.id, h);
            h.write(&[
                m.attacker.code(),
                m.target.code(),
                m.active.code(),
                m.slots[0],
                m.slots[1],
            ]);
            if m.slots[2] | m.slots[3] != 0 {
                h.write(&[0xDB, m.slots[2], m.slots[3]]);
            }
            h.write_u16(m.mov.map_or(0xFFFF, |m| m.0));
            for b in &m.buffs {
                hash_buff(b, h);
            }
            h.write(&match m.wait {
                MsgWait::Frames(n) => [0, n],
                MsgWait::Button => [1, 0],
            });
        }
        BattleEvent::WildAppear => h.write(&[1]),
        BattleEvent::TrainerAppear => h.write(&[2]),
        BattleEvent::TrainerSlideOut => h.write(&[3]),
        BattleEvent::TrainerDefeated => h.write(&[4]),
        BattleEvent::TrainerDefeated2 => h.write(&[0x44]),
        BattleEvent::SendOut { side, party } => h.write(&[5, side.code(), party]),
        BattleEvent::Recall { side } => h.write(&[6, side.code()]),
        BattleEvent::MoveAnim { attacker, mov } => {
            h.write(&[7, attacker.code()]);
            h.write_u16(mov.0);
        }
        BattleEvent::Hit { target, eff } => h.write(&[8, target.code(), eff as u8]),
        BattleEvent::Hp { side, from, to } => {
            h.write(&[9, side.code()]);
            h.write_u16(from);
            h.write_u16(to);
        }
        BattleEvent::StatusIcon { side, status } => {
            let (a, b) = status.code();
            h.write(&[10, side.code(), a, b]);
        }
        BattleEvent::StatAnim { side, up } => h.write(&[11, side.code(), up as u8]),
        BattleEvent::Faint { side } => h.write(&[12, side.code()]),
        BattleEvent::Exp { party, from, to } => {
            h.write(&[13, party]);
            h.write_u32(from);
            h.write_u32(to);
        }
        BattleEvent::LevelUp { party, level } => h.write(&[14, party, level]),
        BattleEvent::Ball { shakes, caught } => h.write(&[15, shakes, caught as u8]),
        BattleEvent::End(o) => h.write(&[16, o as u8]),
        BattleEvent::FormChange {
            side,
            party,
            species,
        } => {
            h.write(&[17, side.code(), party]);
            h.write_u16(species.0);
        }
        // Never queued (`BattleScene::push` sets reports aside).
        BattleEvent::Report(_) => {}
    }
}
