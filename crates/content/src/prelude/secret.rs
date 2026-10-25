//! Secret-base specials (Gen3 `secret_base.c`, `decoration.c`) and `adddecoration`.
//!
//! The records live on the world. This module warps to the entrance's map, swaps the
//! entrance metatile, and writes the vars the scripts read. Decoration graphics are not
//! drawn: a placed decoration is stored and the cell is blocked.

use super::*;
use engine::grid::MAP_DYNAMIC;
use engine::script::{Query, SecretBaseCmd, WarpSlot};

/// `VAR_CURRENT_SECRET_BASE`.
const VAR_CURRENT_SECRET_BASE: VarId = VarId(0x4054);
/// `VAR_0x8007`, where the entrance type is written.
const VAR_ENTRANCE_TYPE: VarId = VarId(0x8007);
/// `VAR_OBJ_GFX_ID_F`.
const VAR_OBJ_GFX_ID_F: VarId = VarId(0x401F);
const VAR_STEP_COUNTER: VarId = VarId(0x40EC);
const VAR_LAST_ITEM: VarId = VarId(0x40ED);
const VAR_LOW_TV: VarId = VarId(0x40EE);
const VAR_HIGH_TV: VarId = VarId(0x40EF);
const VAR_IS_NOT_LOCAL: VarId = VarId(0x40F0);
/// `FLAG_DAILY_SECRET_BASE`.
const FLAG_DAILY_SECRET_BASE: FlagId = FlagId(0x922);

const TV_WON: u16 = 1 << 11;
const TV_LOST: u16 = 1 << 12;
const TV_DECLINED: u16 = 1 << 13;
const TV_LOW_MASK: u16 = TV_WON | TV_LOST | TV_DECLINED;
const TV_DRAW: u16 = 1;

/// `METATILE_SecretBase_PC` / `RegisterPC` / `PC_On`.
const PC: u16 = 0x220;
const REGISTER_PC: u16 = 0x221;
const PC_ON: u16 = 0x224;

/// Closed metatile, open metatile (`sSecretBaseEntranceMetatiles`).
const ENTRANCE_PAIRS: [(u16, u16); 7] = [
    (0x026, 0x036),
    (0x027, 0x037),
    (0x1A0, 0x1A1),
    (0x1A8, 0x1A9),
    (0x1B0, 0x1B1),
    (0x208, 0x210),
    (0x271, 0x278),
];

/// Owner graphics (`sSecretBaseOwnerGfxIds`): five male, five female.
const OWNER_GFX: [u16; 10] = [0x23, 0x24, 0x0F, 0x1F, 0x21, 0x2F, 0x0E, 0x14, 0x20, 0x22];

/// Map number in group 25, warp 0, and the computer cell, indexed by `id / 10`
/// (`sSecretBaseEntrancePositions`). `None` on Silice, which has no secret-base maps.
fn interior(id: u8) -> Option<(MapId, u8, i16, i16)> {
    #[cfg(pkr_silice)]
    {
        let _ = id;
        None
    }
    #[cfg(not(pkr_silice))]
    {
        if id == 0 {
            return None;
        }
        const POS: [(u16, i16, i16); 24] = [
            (0, 1, 3),
            (1, 5, 9),
            (2, 1, 3),
            (3, 7, 13),
            (4, 2, 3),
            (5, 9, 2),
            (6, 13, 4),
            (7, 1, 2),
            (8, 1, 3),
            (9, 1, 2),
            (10, 3, 15),
            (11, 3, 14),
            (12, 9, 3),
            (13, 8, 7),
            (14, 3, 6),
            (15, 5, 9),
            (16, 2, 3),
            (17, 5, 6),
            (18, 15, 3),
            (19, 4, 10),
            (20, 3, 3),
            (21, 1, 2),
            (22, 7, 8),
            (23, 9, 6),
        ];
        let (num, x, y) = POS.get((id / 10) as usize).copied()?;
        Some((MapId(num | 25 << 8), 0, x, y))
    }
}

/// Runs `id` when it is a secret-base special. `None` leaves it to [`super::special`].
pub async fn run(ctx: &Ctx, id: u16) -> Option<u16> {
    Some(match id {
        id if id == SPECIAL_GetSecretBaseTypeInFrontOfPlayer => {
            let t = ctx.secret_base(SecretBaseCmd::TypeInFront).await;
            ctx.set_var(VAR_ENTRANCE_TYPE, t).await;
            0
        }
        id if id == SPECIAL_CheckPlayerHasSecretBase => {
            let has = ctx.secret_base(SecretBaseCmd::HasPlayer).await;
            ctx.set_var(VAR_RESULT, has).await;
            has
        }
        id if id == SPECIAL_SetPlayerSecretBase => {
            ctx.secret_base(SecretBaseCmd::Claim).await;
            ctx.set_var(VAR_CURRENT_SECRET_BASE, 0).await;
            ctx.secret_base(SecretBaseCmd::SnapshotParty).await;
            0
        }
        id if id == SPECIAL_EnterSecretBase => {
            enter(ctx).await;
            0
        }
        id if id == SPECIAL_EnterNewlyCreatedSecretBase => {
            enter_newly_created(ctx).await;
            0
        }
        id if id == SPECIAL_ClearAndLeaveSecretBase || id == SPECIAL_MoveOutOfSecretBase => {
            ctx.secret_base(SecretBaseCmd::ClearPlayer).await;
            ctx.warp(MAP_DYNAMIC, Some(126), 0, 0).await;
            0
        }
        id if id == SPECIAL_IsCurSecretBaseOwnedByAnotherPlayer => {
            let v = ctx.secret_base(SecretBaseCmd::OwnedByOther).await;
            ctx.set_var(VAR_RESULT, v).await;
            v
        }
        id if id == SPECIAL_InitSecretBaseVars => {
            init_vars(ctx).await;
            0
        }
        id if id == SPECIAL_SetSecretBaseOwnerGfxId => {
            let idx = ctx.var(VAR_CURRENT_SECRET_BASE).await;
            let owner = ctx.secret_base(SecretBaseCmd::OwnerType(idx)).await as usize;
            let gfx = OWNER_GFX.get(owner).copied().unwrap_or(OWNER_GFX[0]);
            ctx.set_var(VAR_OBJ_GFX_ID_F, gfx).await;
            0
        }
        id if id == SPECIAL_ShowSecretBaseDecorationMenu || id == SPECIAL_SetDecoration => {
            // The menu edits the player's own base. `SetDecoration` places on the one being
            // visited (a sprite decoration the menu already chose; the first owned one here).
            let index = if id == SPECIAL_ShowSecretBaseDecorationMenu {
                0
            } else {
                ctx.var(VAR_CURRENT_SECRET_BASE).await
            };
            ctx.secret_base(SecretBaseCmd::PlaceFront(index)).await;
            0
        }
        id if id == SPECIAL_DoSecretBasePCTurnOffEffect => {
            pc_off(ctx).await;
            0
        }
        id if id == SPECIAL_PrepSecretBaseBattleFlags => {
            // The battle party is the record's. Refresh it from the party the player has now.
            ctx.secret_base(SecretBaseCmd::SnapshotParty).await;
            0
        }
        id if id == SPECIAL_GetSecretBaseOwnerAndState => owner_and_state(ctx).await,
        id if id == SPECIAL_SetBattledOwnerFromResult => {
            let idx = ctx.var(VAR_CURRENT_SECRET_BASE).await;
            let v = ctx.var(VAR_RESULT).await;
            ctx.secret_base(SecretBaseCmd::Battled { index: idx, set: v })
                .await;
            0
        }
        id if id == SPECIAL_DeclinedSecretBaseBattle => {
            tv(ctx, TV_DECLINED, 0).await;
            0
        }
        id if id == SPECIAL_DrewSecretBaseBattle => {
            tv(ctx, 0, TV_DRAW).await;
            0
        }
        id if id == SPECIAL_WonSecretBaseBattle => {
            tv(ctx, TV_WON, 0).await;
            0
        }
        id if id == SPECIAL_LostSecretBaseBattle => {
            tv(ctx, TV_LOST, 0).await;
            0
        }
        _ => return None,
    })
}

async fn enter(ctx: &Ctx) {
    let idx = ctx.var(VAR_CURRENT_SECRET_BASE).await;
    let id = ctx.secret_base(SecretBaseCmd::Entered(idx)).await as u8;
    let Some((map, warp, _, _)) = interior(id) else {
        return;
    };
    let (x, y) = ctx.player_pos().await;
    let here = ctx.current_map().await;
    ctx.set_warp(WarpSlot::Dynamic, here, None, x, y).await;
    ctx.warp(map, Some(warp), 0, 0).await;
}

async fn enter_newly_created(ctx: &Ctx) {
    let id = ctx.secret_base(SecretBaseCmd::Cur).await as u8;
    let Some((_, _, x, y)) = interior(id) else {
        return;
    };
    ctx.set_metatile(x, y, PC, true).await;
    let map = ctx.current_map().await;
    ctx.warp(map, None, x, y).await;
}

async fn init_vars(ctx: &Ctx) {
    ctx.set_var(VAR_STEP_COUNTER, 0).await;
    ctx.set_var(VAR_LAST_ITEM, 0).await;
    ctx.set_var(VAR_LOW_TV, 0).await;
    ctx.set_var(VAR_HIGH_TV, 0).await;
    let cur = ctx.var(VAR_CURRENT_SECRET_BASE).await;
    ctx.set_var(VAR_IS_NOT_LOCAL, u16::from(cur != 0)).await;
}

async fn pc_off(ctx: &Ctx) {
    let tile = if ctx.var(VAR_CURRENT_SECRET_BASE).await == 0 {
        PC
    } else {
        REGISTER_PC
    };
    let (x, y) = front(ctx).await;
    ctx.set_metatile(x, y, tile, true).await;
}

async fn owner_and_state(ctx: &Ctx) -> u16 {
    if !ctx.flag(FLAG_DAILY_SECRET_BASE).await {
        ctx.secret_base(SecretBaseCmd::ClearBattled).await;
        ctx.set_flag(FLAG_DAILY_SECRET_BASE, true).await;
    }
    let idx = ctx.var(VAR_CURRENT_SECRET_BASE).await;
    let owner = ctx.secret_base(SecretBaseCmd::OwnerType(idx)).await;
    let battled = ctx
        .secret_base(SecretBaseCmd::Battled {
            index: idx,
            set: 0xFFFF,
        })
        .await;
    ctx.set_var(VAR_0x8004, owner).await;
    ctx.set_var(VAR_RESULT, battled).await;
    battled
}

/// TV show flags, only while visiting someone else's base (`VAR_CURRENT_SECRET_BASE != 0`).
async fn tv(ctx: &Ctx, low_bit: u16, high_bit: u16) {
    if ctx.var(VAR_CURRENT_SECRET_BASE).await == 0 {
        return;
    }
    if low_bit != 0 {
        let v = (ctx.var(VAR_LOW_TV).await & !TV_LOW_MASK) | low_bit;
        ctx.set_var(VAR_LOW_TV, v).await;
    }
    if high_bit != 0 {
        let v = (ctx.var(VAR_HIGH_TV).await & !TV_DRAW) | high_bit;
        ctx.set_var(VAR_HIGH_TV, v).await;
    }
}

async fn front(ctx: &Ctx) -> (i16, i16) {
    let (x, y) = ctx.player_pos().await;
    let (dx, dy) = ctx.player_facing().await.delta();
    (x + dx, y + dy)
}

/// Gen3 `adddecoration`: the id goes into the inventory. `VAR_RESULT` is whether it fit.
pub async fn add_decoration(ctx: &Ctx, id: u16) -> bool {
    let ok =
        (1..256).contains(&id) && ctx.secret_base(SecretBaseCmd::GiveDecor(id as u8)).await != 0;
    ctx.set_var(VAR_RESULT, u16::from(ok)).await;
    ok
}

/// The secret-power effect opens or closes the entrance in front of the player.
pub async fn toggle_entrance(ctx: &Ctx) {
    let (x, y) = front(ctx).await;
    let id = ctx.query(Query::Metatile { x, y }).await;
    let Some(next) = ENTRANCE_PAIRS
        .iter()
        .find_map(|&(a, b)| (a == id).then_some(b).or_else(|| (b == id).then_some(a)))
    else {
        return;
    };
    ctx.set_metatile(x, y, next, true).await;
}

/// The PC in front of the player ends on the lit metatile.
pub async fn pc_on(ctx: &Ctx) {
    let (x, y) = front(ctx).await;
    ctx.set_metatile(x, y, PC_ON, true).await;
}
