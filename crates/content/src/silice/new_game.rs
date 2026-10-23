//! Silice's new-game script (`NewGame::script`), run once by `World::new` before the first map
//! loads: it sets `game.ron`'s `new_game_flags` and `new_game_vars` (`SIL_NEW_GAME_FLAGS`,
//! `SIL_NEW_GAME_VARS`). At the Prologue's start (`VAR_SIL_STORY` 0, night, in the bar) those
//! are the hide flags of the objects a scene shows later (the sbires, the Mallette), of the
//! friends waiting in front of the house (story 3 and 4) and of the day NPCs (story 3 and up).
//! Every other flag and var starts clear, as in Gen3: no party, no running shoes.

use super::prelude::*;

/// The new-game script.
pub async fn new_game(ctx: Ctx) {
    for &f in SIL_NEW_GAME_FLAGS {
        ctx.set_flag(f, true).await;
    }
    for &(v, x) in SIL_NEW_GAME_VARS {
        ctx.set_var(v, x).await;
    }
}
