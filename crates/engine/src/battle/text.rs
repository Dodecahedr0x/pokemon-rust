//! Battle message expansion. **Task F.**

use super::{Msg, Names};
use crate::data::GameData;
use alloc::vec::Vec;

/// Gen3 `BattleStringExpandPlaceholders` (`battle_message.c`): the `B_TXT_*` codes the `Str`s
/// use, and `B_BUFF1..3` from `Msg::buffs` (`ExpandBattleTextBuffPlaceholders`). Returns
/// Gen3-encoded text, 0xFF-terminated. Task F.
#[allow(unused_variables)]
pub fn expand(game: &impl GameData, msg: &Msg, names: &Names) -> Vec<u8> {
    todo!("Task F: BattleStringExpandPlaceholders")
}
