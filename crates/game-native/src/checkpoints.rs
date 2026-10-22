//! The real replayer and materialisation over a [`Store`] (Milestone 5 Task G).

use crate::store::{Replayed, Store};
use crate::trace::{CheckpointId, Trace};
use engine::checkpoint::Checkpoint;
use engine::data::GameData;

/// Replays a trace from its parent (`None` = new game) on `game`, for [`Store::verify`].
pub fn replayer<'g, G: GameData>(
    game: &'g G,
) -> impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> + 'g {
    move |parent: Option<&Checkpoint>, trace: &Trace| {
        let _ = (game, parent, trace);
        todo!("Milestone 5 Task G")
    }
}

/// Node `id`'s checkpoint: the cached snapshot when its schema and build match, else replayed
/// from the root (caching every snapshot on the way).
pub fn materialize<G: GameData>(
    store: &Store,
    game: &G,
    id: CheckpointId,
) -> Result<Checkpoint, String> {
    let _ = (store, game, id);
    todo!("Milestone 5 Task G")
}
