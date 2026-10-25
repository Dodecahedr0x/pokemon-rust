//! An e-Reader card (`mystery_event_script.c`).
//!
//! `setenigmaberry` copies the berry. When it is valid, Emerald's berry-available
//! var (0x402D) is set so Petalburg Gym gives the item. The berry is not put in
//! the bag (`gText_MysteryEventBerry`: Dad has it). The number is written here
//! rather than the Gen3 name: Silice's story var already has that number, and
//! naming the Gen3 var in this prelude would reserve it.

use super::*;

/// Emerald's berry-available var, 0x402D. See the module note.
const BERRY_AVAILABLE: VarId = VarId(0x402D);

/// Runs one mystery-event payload. The status is `MEVENT_STATUS_*`.
pub async fn apply_ereader(ctx: &Ctx, payload: &[u8]) -> u16 {
    let outcome = engine::ereader::run(payload);
    if let Some(raw) = outcome.berry {
        ctx.set_enigma_berry(raw).await;
        if engine::ereader::is_valid(&raw) {
            ctx.set_var(BERRY_AVAILABLE, 1).await;
        }
    }
    outcome.status
}
