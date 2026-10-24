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
    /// The doubles target cursor for move `slot` (`mega`: armed).
    Target {
        slot: u8,
        mega: bool,
    },
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
    /// `HmMovesCantBeForgotten` prints, then the pick again.
    HmMsg {
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
        let w = &mut self.parts.as_mut().expect(super::LOADED).windows;
        match self.menu {
            Menu::None => match battle.need() {
                Need::Action => {
                    let me = battle.choosing();
                    // B_ACTION_CANCEL_PARTNER: the right battler, asked after the left one.
                    let cancel = me == Side::PlayerRight && battle.player_chooses(Side::Player);
                    w.action_menu_for(game, &names(game, battle, &self.player), me, cancel, f);
                    self.menu = Menu::Action;
                }
                // No party screen: the first mon that can fight and is not out (Baton Pass
                // asks with the user still out).
                Need::Switch => {
                    let me = battle.choosing();
                    let next = battle
                        .party_range(me)
                        .find(|&i| battle.can_switch_to(me, i));
                    debug_assert!(next.is_some(), "Need::Switch without a usable mon");
                    return next.map(Choice::Switch);
                }
                Need::LearnMove { party, mov } => {
                    w.yes_no(game, f);
                    self.menu = Menu::LearnAsk { party, mov };
                }
                Need::Busy | Need::Ended(_) => {}
            },
            Menu::Action => match w.action_result() {
                Some(Action::Fight) => {
                    let me = battle.choosing();
                    let mega = battle.can_mega(me);
                    w.move_menu_mega(game, battle.mon(me), mega, f);
                    self.menu = Menu::Move;
                }
                Some(Action::Cancel) => {
                    self.menu = Menu::None;
                    return Some(Choice::Cancel);
                }
                Some(Action::Run) => {
                    self.menu = Menu::None;
                    return Some(Choice::Run);
                }
                // No bag nor party screen in the slice: back to the menu.
                Some(Action::Bag | Action::Pokemon) => {
                    let me = battle.choosing();
                    let cancel = me == Side::PlayerRight && battle.player_chooses(Side::Player);
                    w.action_menu_for(game, &names(game, battle, &self.player), me, cancel, f);
                }
                None => {}
            },
            Menu::Move => match w.move_result() {
                Some(MoveChoice::Move(slot)) => {
                    let me = battle.choosing();
                    let mega = w.mega_armed();
                    if let Some(targets) = battle.target_choices(game.battle(), me, slot) {
                        w.target_menu(targets.len() as u8, 0, f);
                        self.menu = Menu::Target { slot, mega };
                        return None;
                    }
                    self.menu = Menu::None;
                    if mega {
                        return Some(Choice::Mega(slot));
                    }
                    return Some(Choice::Move(slot));
                }
                Some(MoveChoice::Back) => {
                    let me = battle.choosing();
                    let cancel = me == Side::PlayerRight && battle.player_chooses(Side::Player);
                    w.action_menu_for(game, &names(game, battle, &self.player), me, cancel, f);
                    self.menu = Menu::Action;
                }
                None => {}
            },
            Menu::Target { slot, mega } => match w.target_result() {
                Some(Some(k)) => {
                    self.menu = Menu::None;
                    let me = battle.choosing();
                    let targets = battle
                        .target_choices(game.battle(), me, slot)
                        .unwrap_or_default();
                    let target = targets.get(k as usize).copied().unwrap_or(me.other());
                    // No Mega Evolution with a picked target (Silice only; not in doubles).
                    let _ = mega;
                    return Some(Choice::MoveAt { slot, target });
                }
                Some(None) => {
                    let me = battle.choosing();
                    let can = battle.can_mega(me);
                    w.move_menu_mega(game, battle.mon(me), can, f);
                    self.menu = Menu::Move;
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
                // `Cmd_yesnoboxlearnmove` case 4: an HM move is never forgotten
                // (`IsHMMove2`): the message, then the pick again (case 6, then 2).
                Some(MoveChoice::Move(slot))
                    if battle
                        .party(Side::Player)
                        .get(party as usize)
                        .and_then(|p| p.moves.get(slot as usize).copied().flatten())
                        .is_some_and(|m| game.is_hm_move(m)) =>
                {
                    let m = self.learn_msg(Str::HmMovesCantBeForgotten, party, mov);
                    self.print(game, battle, &m, f);
                    self.menu = Menu::HmMsg { party, mov };
                }
                Some(MoveChoice::Move(slot)) => {
                    self.menu = Menu::None;
                    return Some(Choice::LearnMove { forget: Some(slot) });
                }
                Some(MoveChoice::Back) => self.stop_learning(game, battle, party, mov, f),
                None => {}
            },
            Menu::HmMsg { party, mov } => {
                if let Some(p) = battle.party(Side::Player).get(party as usize) {
                    let mon = view(game.battle(), p, party);
                    w.learn_move_menu(game, &mon, mov, f);
                }
                self.menu = Menu::LearnPick { party, mov };
            }
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
                    self.parts
                        .as_mut()
                        .expect(super::LOADED)
                        .windows
                        .yes_no(game, f);
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
