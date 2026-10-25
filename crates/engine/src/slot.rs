//! Gen3's Game Corner slot machine (`playslotmachine`, `slot_machine.c`).
//!
//! Betting is `SlotTask_HandleBetInput`: Down inserts one coin (up to 3), R fills the bet to
//! 3 when the coins cover it, A with a bet spins, B asks to quit and A refunds the coins
//! still sitting in the bet. Lines and payouts are `CheckMatch` / `GetMatchFromSymbols` /
//! `sSlotPayouts` on the official reel strips. A payout is added up to [`MAX_COINS`].
//!
//! The reels do not animate. A spin rests each reel on `Random() % 21` (or a position a test
//! forced). Reel Time, the bias nudge, and the Pikachu power meter are not simulated: the
//! machine id is kept (it is what `playslotmachine` selects) and does not change a line's
//! payout. A replay match pays nothing and spins again at no extra cost.

use crate::input::Buttons;
use crate::rng::Rng;

/// Gen3 `MAX_COINS`.
pub const MAX_COINS: u16 = 9999;
/// Gen3 `SYMBOLS_PER_REEL`.
pub const SYMBOLS_PER_REEL: u8 = 21;
/// Gen3 `MAX_BET`.
pub const MAX_BET: u8 = 3;
/// Gen3 `SLOT_MACHINE_UNLUCKIEST`.
pub const UNLUCKIEST: u8 = 0;

/// A symbol on a reel (`SYMBOL_*`, value order).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Symbol {
    Red7,
    Blue7,
    Azurill,
    Lotad,
    Cherry,
    Power,
    Replay,
}

impl Symbol {
    /// A short label for the reels window.
    pub const fn label(self) -> &'static str {
        match self {
            Symbol::Red7 => "7RED",
            Symbol::Blue7 => "7BLU",
            Symbol::Azurill => "AZU",
            Symbol::Lotad => "LOT",
            Symbol::Cherry => "CHY",
            Symbol::Power => "PWR",
            Symbol::Replay => "RPY",
        }
    }
}

/// What the machine is waiting for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Phase {
    Bet,
    /// "You don't have three coins."
    NeedThree,
    /// "You've got 9999 coins."
    MaxCoins,
    /// "Quit the game?" A quits, B stays.
    Quit,
    /// "You've run out of coins." A or B closes.
    Broke,
}

/// `sReelSymbols`, left then middle then right.
const REELS: [[Symbol; SYMBOLS_PER_REEL as usize]; 3] = [
    [
        Symbol::Red7,
        Symbol::Cherry,
        Symbol::Azurill,
        Symbol::Replay,
        Symbol::Power,
        Symbol::Lotad,
        Symbol::Blue7,
        Symbol::Lotad,
        Symbol::Cherry,
        Symbol::Power,
        Symbol::Replay,
        Symbol::Azurill,
        Symbol::Red7,
        Symbol::Power,
        Symbol::Lotad,
        Symbol::Replay,
        Symbol::Azurill,
        Symbol::Blue7,
        Symbol::Power,
        Symbol::Lotad,
        Symbol::Replay,
    ],
    [
        Symbol::Red7,
        Symbol::Cherry,
        Symbol::Replay,
        Symbol::Lotad,
        Symbol::Azurill,
        Symbol::Cherry,
        Symbol::Replay,
        Symbol::Power,
        Symbol::Power,
        Symbol::Lotad,
        Symbol::Blue7,
        Symbol::Lotad,
        Symbol::Replay,
        Symbol::Cherry,
        Symbol::Azurill,
        Symbol::Lotad,
        Symbol::Replay,
        Symbol::Cherry,
        Symbol::Lotad,
        Symbol::Replay,
        Symbol::Cherry,
    ],
    [
        Symbol::Red7,
        Symbol::Power,
        Symbol::Blue7,
        Symbol::Replay,
        Symbol::Lotad,
        Symbol::Azurill,
        Symbol::Replay,
        Symbol::Lotad,
        Symbol::Power,
        Symbol::Azurill,
        Symbol::Replay,
        Symbol::Lotad,
        Symbol::Azurill,
        Symbol::Power,
        Symbol::Replay,
        Symbol::Lotad,
        Symbol::Azurill,
        Symbol::Power,
        Symbol::Replay,
        Symbol::Lotad,
        Symbol::Cherry,
    ],
];

/// A match (`MATCH_*`) and what `sSlotPayouts` pays for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Line {
    Cherry,
    TopBotCherry,
    Replay,
    Lotad,
    Azurill,
    Power,
    Mixed7,
    Red7,
    Blue7,
}

impl Line {
    const fn payout(self) -> u16 {
        match self {
            Line::Cherry => 2,
            Line::TopBotCherry => 4,
            Line::Replay => 0,
            Line::Lotad => 6,
            Line::Azurill => 12,
            Line::Power => 3,
            Line::Mixed7 => 90,
            Line::Red7 | Line::Blue7 => 300,
        }
    }
}

/// The cap `playslotmachine` uses. A game that leaves `max_coins` at 0 still pays up to
/// [`MAX_COINS`].
pub const fn coin_cap(game_max: u16) -> u16 {
    if game_max == 0 { MAX_COINS } else { game_max }
}

/// `GetSymbolAtRest`: `offset` 1 is the top row, 2 the center, 3 the bottom.
fn symbol_at(reel: usize, pos: u8, offset: i16) -> Symbol {
    let i = (i16::from(pos) + offset).rem_euclid(i16::from(SYMBOLS_PER_REEL)) as usize;
    REELS[reel][i]
}

/// The three visible rows, top first.
pub fn grid(stops: [u8; 3]) -> [[Symbol; 3]; 3] {
    let mut rows = [[Symbol::Replay; 3]; 3];
    for row in 0..3 {
        let offset = row as i16 + 1;
        for reel in 0..3 {
            rows[row][reel] = symbol_at(reel, stops[reel], offset);
        }
    }
    rows
}

/// `GetMatchFromSymbols`.
fn line_of(a: Symbol, b: Symbol, c: Symbol) -> Option<Line> {
    if a == b && a == c {
        return Some(match a {
            Symbol::Red7 => Line::Red7,
            Symbol::Blue7 => Line::Blue7,
            Symbol::Azurill => Line::Azurill,
            Symbol::Lotad => Line::Lotad,
            Symbol::Cherry => Line::Cherry,
            Symbol::Power => Line::Power,
            Symbol::Replay => Line::Replay,
        });
    }
    if (a == Symbol::Red7 && b == Symbol::Red7 && c == Symbol::Blue7)
        || (a == Symbol::Blue7 && b == Symbol::Blue7 && c == Symbol::Red7)
    {
        return Some(Line::Mixed7);
    }
    if a == Symbol::Cherry {
        return Some(Line::Cherry);
    }
    None
}

fn add_line(
    payout: &mut u16,
    replay: &mut bool,
    line: Option<Line>,
    top_or_bottom: bool,
    diagonal: bool,
) {
    let Some(line) = line else {
        return;
    };
    let line = if top_or_bottom && line == Line::Cherry {
        Line::TopBotCherry
    } else {
        line
    };
    if line == Line::Replay {
        *replay = true;
    }
    // A diagonal cherry was already paid on the top or bottom row.
    if diagonal && line == Line::Cherry {
        return;
    }
    *payout = payout.saturating_add(line.payout());
}

/// `CheckMatch` for `bet` (1, 2, or 3) with each reel resting at `stops`.
pub fn settle(bet: u8, stops: [u8; 3]) -> (u16, bool) {
    let rows = grid(stops);
    let mut payout = 0;
    let mut replay = false;
    let at = |row: usize| line_of(rows[row][0], rows[row][1], rows[row][2]);
    add_line(&mut payout, &mut replay, at(1), false, false);
    if bet > 1 {
        add_line(&mut payout, &mut replay, at(0), true, false);
        add_line(&mut payout, &mut replay, at(2), true, false);
    }
    if bet > 2 {
        add_line(
            &mut payout,
            &mut replay,
            line_of(rows[0][0], rows[1][1], rows[2][2]),
            false,
            true,
        );
        add_line(
            &mut payout,
            &mut replay,
            line_of(rows[2][0], rows[1][1], rows[0][2]),
            false,
            true,
        );
    }
    (payout, replay)
}

fn add_coins(coins: &mut u16, max: u16, payout: u16) {
    let room = max.saturating_sub(*coins);
    *coins += payout.min(room);
}

/// The machine the screen steps. `input` returns whether it closed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Machine {
    pub machine: u8,
    pub bet: u8,
    pub phase: Phase,
    /// Rest positions (`reelPositions`). Normal-game `sInitialReelPositions` is 0.
    pub positions: [u8; 3],
    /// Coins the last spin added (0 on a loss).
    pub last_payout: u16,
    /// A spin has resolved, so the window can say won or lost.
    pub spun: bool,
    forced: Option<[u8; 3]>,
    shown_max: bool,
}

impl Machine {
    pub fn new(machine: u16) -> Self {
        Machine {
            machine: machine as u8,
            bet: 0,
            phase: Phase::Bet,
            positions: [0; 3],
            last_payout: 0,
            spun: false,
            forced: None,
            shown_max: false,
        }
    }

    /// The next spin rests here instead of drawing `Random()`.
    pub fn force_stops(&mut self, stops: [u8; 3]) {
        self.forced = Some(stops);
    }

    pub fn grid(&self) -> [[Symbol; 3]; 3] {
        grid(self.positions)
    }

    /// One press (`SlotTask_HandleBetInput` and the messages around it).
    pub fn input(&mut self, coins: &mut u16, max: u16, rng: &mut Rng, pressed: Buttons) -> bool {
        let max = coin_cap(max);
        match self.phase {
            Phase::NeedThree | Phase::MaxCoins => {
                if pressed.contains(Buttons::A) || pressed.contains(Buttons::B) {
                    self.phase = Phase::Bet;
                }
                return false;
            }
            Phase::Broke => {
                return pressed.contains(Buttons::A) || pressed.contains(Buttons::B);
            }
            Phase::Quit => {
                if pressed.contains(Buttons::A) {
                    add_coins(coins, max, u16::from(self.bet));
                    self.bet = 0;
                    return true;
                }
                if pressed.contains(Buttons::B) {
                    self.phase = Phase::Bet;
                }
                return false;
            }
            Phase::Bet => {}
        }
        if self.bet == 0 && *coins == 0 {
            self.phase = Phase::Broke;
            return false;
        }
        if *coins < max {
            self.shown_max = false;
        } else if self.bet == 0 && !self.shown_max {
            self.shown_max = true;
            self.phase = Phase::MaxCoins;
            return false;
        }
        if pressed.contains(Buttons::R) {
            let need = u16::from(MAX_BET - self.bet);
            if *coins >= need {
                *coins -= need;
                self.bet = MAX_BET;
                self.spin(coins, max, rng);
            } else {
                self.phase = Phase::NeedThree;
            }
            return false;
        }
        if pressed.contains(Buttons::DOWN) && *coins != 0 && self.bet < MAX_BET {
            *coins -= 1;
            self.bet += 1;
        }
        if self.bet >= MAX_BET || (self.bet != 0 && pressed.contains(Buttons::A)) {
            self.spin(coins, max, rng);
        }
        // B is checked after the spin, as in `SlotTask_HandleBetInput`, and replaces it.
        if pressed.contains(Buttons::B) {
            self.phase = Phase::Quit;
        }
        false
    }

    fn spin(&mut self, coins: &mut u16, max: u16, rng: &mut Rng) {
        let bet = self.bet;
        let mut total = 0u16;
        for _ in 0..8 {
            let stops = self.forced.unwrap_or_else(|| {
                [
                    (rng.random() % u16::from(SYMBOLS_PER_REEL)) as u8,
                    (rng.random() % u16::from(SYMBOLS_PER_REEL)) as u8,
                    (rng.random() % u16::from(SYMBOLS_PER_REEL)) as u8,
                ]
            });
            self.positions = stops;
            let (payout, replay) = settle(bet, stops);
            total = total.saturating_add(payout);
            if !replay || self.forced.is_some() {
                break;
            }
        }
        add_coins(coins, max, total);
        self.last_payout = total;
        self.spun = true;
        self.bet = 0;
        self.phase = Phase::Bet;
    }
}

#[cfg(test)]
#[path = "slot_tests.rs"]
mod tests;
