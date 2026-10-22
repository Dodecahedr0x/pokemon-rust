//! Battle and menu policy (Task F). Its keys are intents: the driver applies the tap rule
//! (a release frame between two presses of the same keys, nothing while a battle or a warp has
//! the screen black).
//!
//! The battle cursor walk is the parity scenario's (`game_native::scenario`, `battle_key`),
//! adapted to [`Opts`]: v0 has a one-mon party, no bag and no switching (decision 18).

use crate::model::Opts;
use engine::battle::scene::OpenMenu;
use engine::battle::{BattleKind, BattleMon, Side};
use engine::mon::BattleTables;
use engine::{Buttons, World};

pub struct Policy {
    /// The last intent given (diagnostics; the tap rule is the driver's).
    last: Buttons,
}

impl Default for Policy {
    fn default() -> Self {
        Policy::new()
    }
}

/// RUN in the 2x2 action menu (FIGHT 0, BAG 1, POKéMON 2, RUN 3).
const RUN: u8 = 3;
const FIGHT: u8 = 0;

/// The key that moves a 2x2 menu cursor from `c` toward `want` (column first, then row), or A
/// once it is there.
fn toward(c: u8, want: u8) -> Buttons {
    if c & 1 != want & 1 {
        if want & 1 == 0 {
            Buttons::LEFT
        } else {
            Buttons::RIGHT
        }
    } else if c & 2 != want & 2 {
        if want & 2 == 0 {
            Buttons::UP
        } else {
            Buttons::DOWN
        }
    } else {
        Buttons::A
    }
}

impl Policy {
    pub fn new() -> Policy {
        Policy {
            last: Buttons::NONE,
        }
    }

    /// The last intent returned by [`Policy::battle`] or [`Policy::script`].
    pub fn last(&self) -> Buttons {
        self.last
    }

    /// Next intent while a battle owns the frame: first damaging usable move (power > 0, pp > 0;
    /// slot 0 if none), RUN from wild battles when `opts.run_wild` (never the first battle or a
    /// trainer), keep known moves (B on learn prompts), A through messages.
    pub fn battle(&mut self, w: &World, t: &BattleTables, opts: Opts) -> Buttons {
        let k = match w.battle() {
            None => Buttons::A,
            Some((_, battle, scene)) => {
                let run = opts.run_wild
                    && matches!(
                        battle.setup().kind,
                        BattleKind::Wild {
                            first_battle: false
                        }
                    );
                match scene.open_menu() {
                    Some(OpenMenu::Action(c)) => toward(c, if run { RUN } else { FIGHT }),
                    // Never fight when running: back out to the action menu.
                    Some(OpenMenu::Move(_)) if run => Buttons::B,
                    Some(OpenMenu::Move(c)) => {
                        toward(c, first_damaging_slot(battle.mon(Side::Player), t))
                    }
                    // Keep the known moves (Gen3: "Stop learning X?" yes).
                    Some(OpenMenu::LearnAsk(_) | OpenMenu::LearnPick(_)) => Buttons::B,
                    Some(OpenMenu::StopAsk(0)) => Buttons::A,
                    Some(OpenMenu::StopAsk(_)) => Buttons::UP,
                    None => Buttons::A,
                }
            }
        };
        self.last = k;
        k
    }

    /// Next intent while a script or the starter screen runs: A through text; yes/no -> A for
    /// choice 0, B otherwise; starter -> move to ball `opts.choice % 3`, then A.
    pub fn script(&mut self, w: &World, opts: Opts) -> Buttons {
        let k = if let Some(starter) = w.starter() {
            // Its own "Do you choose this?" yes/no starts on yes: A confirms the ball.
            let want = opts.choice % 3;
            match starter.selection().cmp(&want) {
                std::cmp::Ordering::Less => Buttons::RIGHT,
                std::cmp::Ordering::Greater => Buttons::LEFT,
                std::cmp::Ordering::Equal => Buttons::A,
            }
        } else if w.text().is_yes_no_active() {
            // The cursor starts on yes; B answers no wherever it is.
            if opts.choice == 0 {
                Buttons::A
            } else {
                Buttons::B
            }
        } else {
            Buttons::A
        };
        self.last = k;
        k
    }
}

/// The first move slot of `m` with power > 0 and pp > 0 (slot 0 if none).
pub fn first_damaging_slot(m: &BattleMon, t: &BattleTables) -> u8 {
    (0..4u8)
        .find(|&i| {
            let i = i as usize;
            m.moves[i].is_some_and(|id| {
                m.pp[i] > 0 && t.moves.get(id.0 as usize).is_some_and(|mi| mi.power > 0)
            })
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toward_walks_the_2x2_grid() {
        assert_eq!(toward(0, 0), Buttons::A);
        assert_eq!(toward(0, 3), Buttons::RIGHT);
        assert_eq!(toward(1, 3), Buttons::DOWN);
        assert_eq!(toward(3, 3), Buttons::A);
        assert_eq!(toward(3, 0), Buttons::LEFT);
        assert_eq!(toward(2, 0), Buttons::UP);
        assert_eq!(toward(0, 2), Buttons::DOWN);
    }
}
