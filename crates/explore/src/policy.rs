//! Battle and menu policy (Task F). Its keys are intents: the driver applies the tap rule
//! (a release frame between two presses of the same keys, nothing while a battle or a warp has
//! the screen black).
//!
//! The battle cursor walk is the parity scenario's (`game_native::scenario`, `battle_key`),
//! adapted to [`Opts`]: v0 has a one-mon party, no bag and no switching (decision 18).

use crate::model::Opts;
use engine::battle::scene::OpenMenu;
use engine::battle::{BattleKind, BattleMon, Side};
use engine::data::MoveId;
use engine::mon::BattleTables;
use engine::{Buttons, World};

pub struct Policy {
    /// The last intent given (diagnostics; the tap rule is the driver's).
    last: Buttons,
    /// The yes/no prompts seen so far (counted on the frame each opens).
    yes_nos: u32,
    /// Whether a yes/no was open on the previous script frame.
    yes_no_open: bool,
}

/// [`Opts::choice`] value: yes to the first yes/no of the macro, no to every later one (a
/// script that offers again after a yes, the Route 111 rest stop's old lady).
pub const YES_ONCE: u8 = 3;

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
            yes_nos: 0,
            yes_no_open: false,
        }
    }

    /// The last intent returned by [`Policy::battle`] or [`Policy::script`].
    pub fn last(&self) -> Buttons {
        self.last
    }

    /// Next intent while a battle owns the frame: first damaging usable move (power > 0, pp > 0;
    /// else the first move with pp > 0; else slot 0, where the engine uses Struggle), RUN from wild battles when `opts.run_wild` (never the first battle or a
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
                    Some(OpenMenu::Move(c)) if opts.best_move => toward(
                        c,
                        best_damaging_slot(battle.mon(Side::Player), battle.mon(Side::Opponent), t),
                    ),
                    Some(OpenMenu::Move(c)) => {
                        toward(c, first_damaging_slot(battle.mon(Side::Player), t))
                    }
                    // Learn a stronger level-up move over the weakest known one.
                    Some(OpenMenu::LearnAsk(c))
                        if opts.learn && forget_slot(battle.mon(Side::Player), t).is_some() =>
                    {
                        if c == 0 { Buttons::A } else { Buttons::UP }
                    }
                    Some(OpenMenu::LearnPick(c)) if opts.learn => {
                        match forget_slot(battle.mon(Side::Player), t) {
                            Some(_) if c > 3 => Buttons::LEFT,
                            Some(s) => toward(c, s),
                            None => Buttons::B,
                        }
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
    /// choice 0 (and the first one for [`YES_ONCE`]), B otherwise; a multichoice -> move to choice `opts.choice`, then A; starter ->
    /// move to ball `opts.choice % 3`, then A.
    ///
    /// B on every yes/no when `choice != 0` can cycle on a script that re-asks until it gets a
    /// yes: the driver's `settle_frames`/`macro_frames` limits end such a macro (a `Timeout`).
    pub fn script(&mut self, w: &World, opts: Opts) -> Buttons {
        let yes_no = w.text().is_yes_no_active();
        if yes_no && !self.yes_no_open {
            self.yes_nos += 1;
        }
        self.yes_no_open = yes_no;
        let k = if let Some(starter) = w.starter() {
            // Its own "Do you choose this?" yes/no starts on yes: A confirms the ball.
            let want = opts.choice % 3;
            match starter.selection().cmp(&want) {
                std::cmp::Ordering::Less => Buttons::RIGHT,
                std::cmp::Ordering::Greater => Buttons::LEFT,
                std::cmp::Ordering::Equal => Buttons::A,
            }
        } else if let Some(m) = w.text().multichoice() {
            // A script multichoice: move to choice `opts.choice`, then A.
            match m.cursor().cmp(&opts.choice) {
                std::cmp::Ordering::Less => Buttons::DOWN,
                std::cmp::Ordering::Greater => Buttons::UP,
                std::cmp::Ordering::Equal => Buttons::A,
            }
        } else if yes_no {
            // The cursor starts on yes; B answers no wherever it is.
            if opts.choice == 0 || (opts.choice == YES_ONCE && self.yes_nos == 1) {
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

/// The move slot to use: the first with power > 0 and pp > 0, else the first with a move and
/// pp > 0 (a status move beats selecting a slot without PP, which the engine refuses forever),
/// else slot 0 (no PP anywhere: the engine uses Struggle).
pub fn first_damaging_slot(m: &BattleMon, t: &BattleTables) -> u8 {
    let usable = |i: usize| m.moves[i].is_some() && m.pp[i] > 0;
    let damaging = |i: usize| {
        usable(i)
            && m.moves[i]
                .and_then(|id| t.moves.get(id.0 as usize))
                .is_some_and(|mi| mi.power > 0)
    };
    (0..4)
        .find(|&i| damaging(i))
        .or_else(|| (0..4).find(|&i| usable(i)))
        .unwrap_or(0) as u8
}

/// The move slot of the best expected damage on `foe`: power x STAB (1.5) x the type chart's
/// multipliers on the foe's types, over the usable moves with power; ties go to the lower slot.
/// Without a damaging move, [`first_damaging_slot`].
/// A move's worth for [`forget_slot`]: power, x1.5 with STAB; 0 for a status move.
fn move_worth(me: &BattleMon, mov: MoveId, t: &BattleTables) -> u32 {
    let Some(mi) = t.moves.get(mov.0 as usize) else {
        return 0;
    };
    let stab = t.species_info(me.species).types.contains(&mi.ty);
    u32::from(mi.power) * if stab { 15 } else { 10 }
}

/// The slot to forget for the level-up move `me` is learning (its learnset's first move at its
/// level it does not know): the weakest known move ([`move_worth`]), when the new one is worth
/// more; `None` keeps the known moves.
pub fn forget_slot(me: &BattleMon, t: &BattleTables) -> Option<u8> {
    let new = (t.species_info(me.species).learnset.iter())
        .find(|l| l.level == me.level && !me.moves.contains(&Some(l.mov)))?
        .mov;
    let (worth, slot) = (0..4)
        .filter_map(|i| me.moves[i].map(|m| (move_worth(me, m, t), i as u8)))
        .min()?;
    (move_worth(me, new, t) > worth).then_some(slot)
}

pub fn best_damaging_slot(me: &BattleMon, foe: &BattleMon, t: &BattleTables) -> u8 {
    let mine = t.species_info(me.species).types;
    let theirs = t.species_info(foe.species).types;
    let score = |i: usize| -> Option<u32> {
        let id = me.moves[i]?;
        if me.pp[i] == 0 {
            return None;
        }
        let mi = t.moves.get(id.0 as usize)?;
        if mi.power == 0 {
            return None;
        }
        let mut s = u32::from(mi.power) * if mine.contains(&mi.ty) { 15 } else { 10 };
        let mut seen = None;
        for d in theirs {
            if seen == Some(d) {
                continue;
            }
            seen = Some(d);
            let m = (t.type_chart.iter())
                .find(|e| e.attack == mi.ty && e.defend == d)
                .map_or(10, |e| u32::from(e.multiplier));
            s = s * m / 10;
        }
        Some(s)
    };
    let mut best: Option<(u32, usize)> = None;
    for i in 0..4 {
        if let Some(s) = score(i)
            && best.is_none_or(|(b, _)| s > b)
        {
            best = Some((s, i));
        }
    }
    best.map_or_else(|| first_damaging_slot(me, t), |(_, i)| i as u8)
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
