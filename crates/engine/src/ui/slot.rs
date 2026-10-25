//! The Game Corner slot machine (`playslotmachine`, `slot_machine.c`).
//!
//! One framed window: the coin count, the bet, the three visible rows, and a prompt. Input is
//! [`crate::slot::Machine`]; this screen only draws it and closes when the machine does.
//! Reels are labels (no reel graphics, no stop timing).

use super::kit::widgets::{EOS, push_number};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::kit::{Gfx, frame_budget};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::slot::{self, Phase, Symbol};

struct Render {
    panel: Panel,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotMachine {
    machine: slot::Machine,
    gfx: Gfx<Render>,
}

impl SlotMachine {
    pub fn new(machine: u8) -> Self {
        SlotMachine {
            machine: slot::Machine::new(u16::from(machine)),
            gfx: Gfx::none(),
        }
    }

    fn paint<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(r) = self.gfx.0.as_deref_mut() else {
            return;
        };
        let game = cx.game;
        r.panel.clear(game);
        let mut coins = glyphs("COINS ");
        coins.pop();
        push_number(&mut coins, u32::from(*cx.coins), 4, true);
        coins.push(EOS);
        r.panel.text(game, &coins, 4, 1);

        let mut bet = glyphs("BET ");
        bet.pop();
        push_number(&mut bet, u32::from(self.machine.bet), 1, false);
        bet.push(EOS);
        r.panel.text(game, &bet, 4, 17);

        let rows = self.machine.grid();
        for (i, row) in rows.iter().enumerate() {
            r.panel.text(game, &symbol_row(row), 4, 33 + i as i16 * 16);
        }
        r.panel.text(game, &prompt(&self.machine), 4, 81);

        let mut map = MapBuf::full(frame_kind(game, cx.options.frame));
        map.put(&r.panel);
        map.flush(cx.f);
    }
}

fn glyphs(s: &str) -> alloc::vec::Vec<u8> {
    let mut v = alloc::vec::Vec::with_capacity(s.len() + 1);
    for c in s.bytes() {
        v.push(match c {
            b'A'..=b'Z' => 0xBB + (c - b'A'),
            b'0'..=b'9' => 0xA1 + (c - b'0'),
            _ => 0x00,
        });
    }
    v.push(EOS);
    v
}

fn symbol_row(row: &[Symbol; 3]) -> alloc::vec::Vec<u8> {
    let mut v = alloc::vec::Vec::new();
    for (i, symbol) in row.iter().enumerate() {
        if i != 0 {
            v.push(0x00);
            v.push(0x00);
        }
        let label = glyphs(symbol.label());
        v.extend(label.iter().take_while(|&&b| b != EOS).copied());
    }
    v.push(EOS);
    v
}

fn prompt(m: &slot::Machine) -> alloc::vec::Vec<u8> {
    match m.phase {
        Phase::NeedThree => glyphs("NEED 3 COINS"),
        Phase::MaxCoins => glyphs("9999 COINS"),
        Phase::Quit => glyphs("QUIT? A YES B NO"),
        Phase::Broke => glyphs("NO COINS"),
        Phase::Bet if m.spun && m.bet == 0 && m.last_payout == 0 => glyphs("LOST"),
        Phase::Bet if m.spun && m.bet == 0 => {
            let mut v = glyphs("WON ");
            v.pop();
            push_number(&mut v, u32::from(m.last_payout), 1, false);
            v.push(EOS);
            v
        }
        Phase::Bet => glyphs("DOWN COIN A SPIN B QUIT"),
    }
}

impl Screen for SlotMachine {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let close = self.machine.input(
            cx.coins,
            cx.game.special_ids().max_coins,
            cx.rng,
            input.pressed,
        );
        if self.gfx.0.is_none() {
            let mut tiles = TileAlloc::full();
            let panel = tiles.panel(cx.game, 1, 1, 28, 14, true);
            self.gfx.0 = Some(alloc::boxed::Box::new(Render { panel }));
        }
        self.paint(cx);
        let mut budget = frame_budget(cx.f);
        if let Some(r) = self.gfx.0.as_deref_mut() {
            r.panel.flush_within(cx.f, &mut budget);
        }
        if close {
            ScreenOut::Done(ScreenResult::Cancel)
        } else {
            ScreenOut::Running
        }
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.machine.machine,
            self.machine.bet,
            self.machine.phase as u8,
        ]);
        h.write(&self.machine.positions);
        h.write_u16(self.machine.last_payout);
        h.write(&[u8::from(self.machine.spun)]);
    }
}
