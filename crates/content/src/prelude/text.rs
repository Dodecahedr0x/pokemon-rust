//! String buffers (owned by Milestone 8 Task H).

use super::*;

/// Gen3 `buffer*` into string var `slot` (0..3).
pub async fn buffer(ctx: &Ctx, slot: u8, src: BufferSrc) {
    ctx.buffer(slot, src).await
}
