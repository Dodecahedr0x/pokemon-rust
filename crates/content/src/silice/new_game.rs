//! Silice's new-game script (`NewGame::script`), run once by `World::new` before the first map
//! loads. Wave 0 sets nothing: Task E sets the Prologue's starting flags and vars (the hidden
//! sbires, the Mallette and the friends, from `games/silice/game.ron`).

use super::prelude::*;

/// The new-game script.
pub async fn new_game(_ctx: Ctx) {}
