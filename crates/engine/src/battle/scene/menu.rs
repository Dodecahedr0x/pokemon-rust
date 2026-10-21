//! The menus of [`BattleScene`]: the action and move menus and the learn-move flow
//! (`Cmd_yesnoboxlearnmove`, `Cmd_yesnoboxstoplearningmove`, `BattleScript_AskToLearnMove`).

use super::{BattleScene, names, view};
use crate::battle::ui::{Action, MoveChoice};
use crate::battle::{Battle, Buff, Choice, Msg, MsgWait, Need, Side, Str};
use crate::data::{GameData, MoveId};
use crate::video::Frame;

/// The open menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Menu {
    None,
    Action,
    Move,
    /// "Learn `mov`?" yes/no (`Cmd_yesnoboxlearnmove`).
    LearnAsk {
        party: u8,
        mov: MoveId,
    },
    /// The move menu with `mov` as a fifth entry.
    LearnPick {
        party: u8,
        mov: MoveId,
    },
    /// `StopLearningMove` prints.
    StopMsg {
        party: u8,
        mov: MoveId,
    },
    /// "Stop learning?" yes/no (`Cmd_yesnoboxstoplearningmove`).
    StopAsk {
        party: u8,
        mov: MoveId,
    },
    /// `TryToLearnMove1..3` again (`n` = the one printing).
    Retry {
        party: u8,
        mov: MoveId,
        n: u8,
    },
}

impl BattleScene {
    /// A learn-move message about party slot `party` (`BattleScript_AskToLearnMove`'s buffers).
    fn learn_msg(&self, id: Str, party: u8, mov: MoveId) -> Msg {
        let side = Side::Player;
        Msg {
            id,
            attacker: side,
            target: side,
            active: side,
            mov: None,
            buffs: [Buff::Mon { side, party }, Buff::Move(mov), Buff::None],
            wait: MsgWait::Frames(0),
            slots: self.out,
        }
    }

    // ---- menus ----

    /// Idle: opens the menu the core's `Need` asks for, or reads the open one.
    pub(super) fn input(
        &mut self,
        game: &impl GameData,
        battle: &Battle,
        f: &mut Frame,
    ) -> Option<Choice> {
        let w = &mut self.parts.windows;
        match self.menu {
            Menu::None => match battle.need() {
                Need::Action => {
                    w.action_menu(game, &names(game, battle), f);
                    self.menu = Menu::Action;
                }
                // No party screen: the first mon that can fight.
                Need::Switch => {
                    debug_assert!(
                        battle.party(Side::Player).first_usable().is_some(),
                        "Need::Switch without a usable mon"
                    );
                    return battle
                        .party(Side::Player)
                        .first_usable()
                        .map(Choice::Switch);
                }
                Need::LearnMove { party, mov } => {
                    w.yes_no(game, f);
                    self.menu = Menu::LearnAsk { party, mov };
                }
                Need::Busy | Need::Ended(_) => {}
            },
            Menu::Action => match w.action_result() {
                Some(Action::Fight) => {
                    w.move_menu(game, battle.mon(Side::Player), f);
                    self.menu = Menu::Move;
                }
                Some(Action::Run) => {
                    self.menu = Menu::None;
                    return Some(Choice::Run);
                }
                // No bag nor party screen in the slice: back to the menu.
                Some(Action::Bag | Action::Pokemon) => {
                    w.action_menu(game, &names(game, battle), f);
                }
                None => {}
            },
            Menu::Move => match w.move_result() {
                Some(MoveChoice::Move(slot)) => {
                    self.menu = Menu::None;
                    return Some(Choice::Move(slot));
                }
                Some(MoveChoice::Back) => {
                    w.action_menu(game, &names(game, battle), f);
                    self.menu = Menu::Action;
                }
                None => {}
            },
            Menu::LearnAsk { party, mov } => match w.yes_no_result() {
                Some(true) => {
                    let Some(p) = battle.party(Side::Player).get(party as usize) else {
                        return Some(Choice::LearnMove { forget: None });
                    };
                    let mon = view(game.battle(), p, party);
                    w.learn_move_menu(game, &mon, mov, f);
                    self.menu = Menu::LearnPick { party, mov };
                }
                Some(false) => self.stop_learning(game, battle, party, mov, f),
                None => {}
            },
            Menu::LearnPick { party, mov } => match w.move_result() {
                Some(MoveChoice::Move(slot)) => {
                    self.menu = Menu::None;
                    return Some(Choice::LearnMove { forget: Some(slot) });
                }
                Some(MoveChoice::Back) => self.stop_learning(game, battle, party, mov, f),
                None => {}
            },
            Menu::StopMsg { party, mov } => {
                w.yes_no(game, f);
                self.menu = Menu::StopAsk { party, mov };
            }
            Menu::StopAsk { party, mov } => match w.yes_no_result() {
                Some(true) => {
                    self.menu = Menu::None;
                    return Some(Choice::LearnMove { forget: None });
                }
                // BattleScript_AskToLearnMove again.
                Some(false) => {
                    let m = self.learn_msg(Str::TryToLearnMove1, party, mov);
                    self.print(game, battle, &m, f);
                    self.menu = Menu::Retry { party, mov, n: 0 };
                }
                None => {}
            },
            Menu::Retry { party, mov, n } => {
                if n < 2 {
                    let id = [Str::TryToLearnMove2, Str::TryToLearnMove3][n as usize];
                    let m = self.learn_msg(id, party, mov);
                    self.print(game, battle, &m, f);
                    self.menu = Menu::Retry {
                        party,
                        mov,
                        n: n + 1,
                    };
                } else {
                    self.parts.windows.yes_no(game, f);
                    self.menu = Menu::LearnAsk { party, mov };
                }
            }
        }
        None
    }

    /// "Stop learning `mov`?" (`STRINGID_STOPLEARNINGMOVE`), then its yes/no.
    fn stop_learning(
        &mut self,
        game: &impl GameData,
        battle: &Battle,
        party: u8,
        mov: MoveId,
        f: &mut Frame,
    ) {
        let m = self.learn_msg(Str::StopLearningMove, party, mov);
        self.print(game, battle, &m, f);
        self.menu = Menu::StopMsg { party, mov };
    }
}
