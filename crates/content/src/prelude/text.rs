//! String buffers (owned by Milestone 8 Task H).

use super::*;

/// Gen3 `buffer*` into string var `slot` (0..3, `STR_VAR_1`..`STR_VAR_3`): species, item and
/// move names, `buffernumberstring` (left-aligned digits), std strings, party nicknames, the lead
/// mon's species and `bufferstring` texts. `{STR_VAR_n}` in later messages prints it.
pub async fn buffer(ctx: &Ctx, slot: u8, src: BufferSrc) {
    ctx.buffer(slot, src).await
}
