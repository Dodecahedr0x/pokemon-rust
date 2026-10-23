//! Items used from the battle bag (Gen3 `B_ACTION_USE_ITEM`, `HandleAction_UseItem`;
//! Milestone 9 Task BM owns the file).
//!
//! Wave 0: [`choose`] refuses every item, so `Choice::Item` is a no-op choice (the menu asks
//! again) until Task BM implements it.

use super::{Action, Battle, Env};
use crate::data::ItemId;

/// The action for `Choice::Item` (Task BM). Wave 0: refused.
pub(super) fn choose(b: &mut Battle, env: &mut Env, item: ItemId, party: u8) -> Option<Action> {
    let _ = (b, env, item, party);
    None
}
