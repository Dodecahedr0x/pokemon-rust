//! `playslotmachine` (the Game Corner).

use super::*;

/// Gen3 `playslotmachine`: machine `machine` (`SLOT_MACHINE_*`, `VarGet` of the operand).
pub async fn play_slot_machine(ctx: &Ctx, machine: u16) {
    ctx.slot_machine(machine).await;
}
