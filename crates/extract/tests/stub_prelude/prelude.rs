// Stub of `content::prelude` for compiling generated scripts in tests (the codegen contract,
// see `crates/extract/src/codegen.rs`). The constants are appended by `tests/codegen.rs`.

pub use core::cmp::Ordering;
pub use engine::data::{Dir, FlagId, MapId, MovementType, PLAYER, TextId, VarId};
pub use engine::script::{Ctx, Emote, Fade, Speed, Step};
pub use std::boxed::Box;

/// `MSGBOX_*` from `asm/macros/event.inc`.
pub enum MsgBox {
    Npc = 2,
    Sign = 3,
    Default = 4,
    YesNo = 5,
    Autoclose = 6,
    GetPoints = 9,
    Pokenav = 10,
}

pub async fn msgbox(ctx: &Ctx, text: TextId, kind: MsgBox) {
    let _ = kind;
    ctx.say(text).await;
}

pub async fn callstd(ctx: &Ctx, std: u8) {
    let _ = std;
    ctx.log("callstd").await;
}

pub async fn special(ctx: &Ctx, id: u16) -> u16 {
    let _ = id;
    ctx.log("special").await;
    0
}

/// Gen3 `checkplayergender`: 0 male, 1 female.
pub async fn check_player_gender(ctx: &Ctx) -> u16 {
    ctx.log("check_player_gender").await;
    0
}

pub async fn unsupported(ctx: &Ctx, what: &'static str) {
    ctx.log(what).await;
}
