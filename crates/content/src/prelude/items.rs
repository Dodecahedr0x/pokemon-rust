//! Items and money (owned by Milestone 8 Task H): the item script commands and Gen3's
//! `data/scripts/obtain_item.inc` (`Std_ObtainItem`, `Std_FindItem`).
//!
//! Every command sets `VAR_RESULT` as Gen3's does and also returns it. Quantities pass through
//! Gen3's `(u8)` cast of `ScrCmd_additem` and friends.
//!
//! The std scripts print `gText_*` texts looked up by their decomp label ([`text_named`]); a
//! build without them (the stub, an extract before the script IR) skips the message and raises
//! `Report::Degraded`. Not modelled (no sound, no bag menu yet): Gen3's plural item names
//! (`bufferitemnameplural` prints the singular; reported for the Poké Balls and berries that
//! have a plural), and `BufferTMHMMoveName` (a found TM reads as an item; reported).
//!
//! Visible difference: `waitfanfare` becomes `waitmessage`. Gen3 holds the "Obtained the …" /
//! "… found one …" message until the fanfare ends (`sFanfares`: 160 frames for an item, 220 for
//! a TM/HM, counted from `playfanfare`, before the message prints); here the next message
//! follows as soon as the first is printed, so these scenes run shorter than Gen3's. Padding with
//! `delay` would need the message's print time, which the script does not see; it waits for a
//! sound model.
//!
//! Scratch vars: the std scripts' working copies (`VAR_0x8007` = the add or space result,
//! `Std_FindItem`'s `VAR_0x8004`/`VAR_0x8005` = item and amount, `VAR_0x8008` = is a TM/HM) and
//! `checkitemtype`'s intermediate `VAR_RESULT` (the pocket) are not written; only the final
//! `VAR_RESULT` is. `POCKET_NONE` (`ITEM_NONE` or an item the table lacks): Gen3's
//! `EventScript_BufferPocketNameAndTryFanfare` `switch` falls through to `end` and ends the
//! whole script; here the pocket name is left as it was and the script goes on (the add has
//! already failed; the host reported an unknown item).

//! This module reads the build's tables only through `text_named` and `item_pocket` (in
//! `prelude/mod.rs`), so the lowering's compile check can build it against stubs of those two.

use super::*;
use engine::data::Pocket;

/// Gen3 `POCKET_*` (`include/constants/item.h`): 0 for none.
pub const fn pocket_number(p: Option<Pocket>) -> u16 {
    match p {
        None => 0,
        Some(Pocket::Items) => 1,
        Some(Pocket::PokeBalls) => 2,
        Some(Pocket::TmHm) => 3,
        Some(Pocket::Berries) => 4,
        Some(Pocket::KeyItems) => 5,
    }
}

/// `STDSTRING_*` of each pocket's name (`EventScript_BufferPocketNameAndTryFanfare`).
pub const fn pocket_std_string(p: Pocket) -> u8 {
    match p {
        Pocket::Items => STDSTRING_ITEMS,
        Pocket::KeyItems => STDSTRING_KEYITEMS,
        Pocket::PokeBalls => STDSTRING_POKEBALLS,
        Pocket::TmHm => STDSTRING_TMHMS,
        Pocket::Berries => STDSTRING_BERRIES,
    }
}

/// Gen3 `STDSTRING_ITEMS` .. `STDSTRING_BERRIES` (`include/constants/script_menu.h`).
pub const STDSTRING_ITEMS: u8 = 0xE;
pub const STDSTRING_KEYITEMS: u8 = 0xF;
pub const STDSTRING_POKEBALLS: u8 = 0x10;
pub const STDSTRING_TMHMS: u8 = 0x11;
pub const STDSTRING_BERRIES: u8 = 0x12;

/// Gen3's `(u8)` cast of item quantities in `ScrCmd_additem` and friends.
const fn script_qty(qty: u16) -> u16 {
    qty as u8 as u16
}

async fn result(ctx: &Ctx, ok: bool) -> bool {
    ctx.set_var(VAR_RESULT, ok as u16).await;
    ok
}

/// Gen3 `additem`.
pub async fn add_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    let ok = ctx.item_add(ItemId(item), script_qty(qty)).await;
    result(ctx, ok).await
}

/// Gen3 `removeitem`.
pub async fn remove_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    let ok = ctx.item_remove(ItemId(item), script_qty(qty)).await;
    result(ctx, ok).await
}

/// Gen3 `checkitem`.
pub async fn check_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    let ok = ctx.item_check(ItemId(item), script_qty(qty)).await;
    result(ctx, ok).await
}

/// Gen3 `checkitemspace`.
pub async fn check_item_space(ctx: &Ctx, item: u16, qty: u16) -> bool {
    let ok = ctx.item_space(ItemId(item), script_qty(qty)).await;
    result(ctx, ok).await
}

/// Gen3 `checkitemtype`: `VAR_RESULT` = the item's `POCKET_*`.
pub async fn check_item_type(ctx: &Ctx, item: u16) -> u16 {
    let p = pocket_number(item_pocket(item));
    ctx.set_var(VAR_RESULT, p).await;
    p
}

/// Gen3 `bufferitemnameplural` (`CopyItemNameHandlePlural`) into string var `slot`.
async fn buffer_item_plural(ctx: &Ctx, slot: u8, item: u16, qty: u16) {
    let berry = (ITEM_CHERI_BERRY..=ITEM_ENIGMA_BERRY).contains(&item);
    if qty >= 2 && (item == ITEM_POKE_BALL || berry) {
        ctx.report(Report::Degraded("bufferitemnameplural")).await;
    }
    ctx.buffer(slot, BufferSrc::Item(ItemId(item))).await;
}

/// `EventScript_BufferPocketNameAndTryFanfare`: `STR_VAR_3` = the pocket's name.
async fn buffer_pocket(ctx: &Ctx, item: u16) {
    if let Some(p) = item_pocket(item) {
        ctx.buffer(2, BufferSrc::Std(pocket_std_string(p))).await;
    }
}

/// Prints the `gText_*` text `name`; whether the build has it (else reported).
async fn std_message(ctx: &Ctx, name: &'static str) -> bool {
    match text_named(name) {
        Some(t) => {
            ctx.message(t).await;
            true
        }
        None => {
            ctx.report(Report::Degraded("obtain_item.inc text")).await;
            false
        }
    }
}

/// `msgbox <name>, MSGBOX_DEFAULT`.
async fn std_msgbox(ctx: &Ctx, name: &'static str) {
    if std_message(ctx, name).await {
        ctx.wait_message().await;
        ctx.wait_button().await;
    }
}

/// Gen3 `Std_ObtainItem` (`giveitem`): adds the item, then "Obtained the {STR_VAR_2}!" and
/// "{PLAYER} put away the {STR_VAR_2} in the {STR_VAR_3} POCKET." when it fit (nothing when it
/// did not: the calling script says so). `VAR_RESULT` and the return value: whether it fit.
pub async fn give_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    let added = add_item(ctx, item, qty).await;
    buffer_item_plural(ctx, 1, item, qty).await;
    buffer_pocket(ctx, item).await;
    if added {
        if std_message(ctx, "gText_ObtainedTheItem").await {
            ctx.wait_message().await; // waitfanfare
        }
        std_msgbox(ctx, "gText_PutItemInPocket").await;
    }
    result(ctx, added).await
}

/// Gen3 `Std_FindItem` (`finditem`, item balls): when the item fits, the ball is removed
/// (`removeobject VAR_LAST_TALKED`, which sets its hide flag), the item added and "{PLAYER} found
/// one {STR_VAR_2}!" then the pocket message print; when it does not, "Obtained the …" and
/// "Too bad! The BAG is full…". Returns whether it fit; `VAR_RESULT` ends as Gen3 leaves it
/// (`PYRAMID_LOCATION_NONE`, 0, after a pick-up; `FALSE` when full).
pub async fn find_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    ctx.lock().await;
    ctx.face_player().await;
    let space = check_item_space(ctx, item, qty).await;
    buffer_item_plural(ctx, 1, item, qty).await;
    buffer_pocket(ctx, item).await;
    if space {
        let obj = ctx.var(VAR_LAST_TALKED).await;
        ctx.remove_object(obj as u8).await;
        add_item(ctx, item, qty).await;
        if item_pocket(item) == Some(Pocket::TmHm) {
            ctx.report(Report::Degraded("BufferTMHMMoveName")).await;
        }
        if std_message(ctx, "gText_PlayerFoundOneItem").await {
            ctx.wait_message().await; // waitfanfare, waitmessage
        }
        buffer_item_plural(ctx, 1, item, qty).await;
        ctx.set_var(VAR_RESULT, 0).await; // pyramid_getlocation
        std_msgbox(ctx, "gText_PutItemInPocket").await;
    } else {
        std_msgbox(ctx, "gText_ObtainedTheItem").await;
        std_msgbox(ctx, "gText_TooBadBagIsFull").await;
        ctx.set_var(VAR_RESULT, 0).await;
    }
    ctx.release().await;
    space
}

/// Gen3 `EventScript_HiddenItemScript` (a hidden item's bg event, `GameData::hidden_item_script`;
/// the engine sets `VAR_0x8004` to the item's flag and `VAR_0x8005` to the item): the item
/// added, "{PLAYER} found one {STR_VAR_2}!", the pocket message and the flag set
/// (`special SetHiddenItemFlag`) when it fit; else "found one" and "Too bad! The BAG is full…"
/// with the flag clear. Returns whether it fit.
pub async fn hidden_item_script(ctx: &Ctx) -> bool {
    ctx.lock_all().await;
    let flag = ctx.var(VAR_0x8004).await;
    let item = ctx.var(VAR_0x8005).await;
    let added = add_item(ctx, item, 1).await;
    buffer_item_plural(ctx, 1, item, 1).await;
    buffer_pocket(ctx, item).await;
    if added {
        if item_pocket(item) == Some(Pocket::TmHm) {
            ctx.report(Report::Degraded("BufferTMHMMoveName")).await;
        }
        if std_message(ctx, "gText_PlayerFoundOneItem").await {
            ctx.wait_message().await; // waitmessage, waitfanfare
        }
        buffer_item_plural(ctx, 1, item, 1).await;
        std_msgbox(ctx, "gText_PutItemInPocket").await;
        ctx.set_flag(FlagId(flag), true).await;
    } else {
        std_msgbox(ctx, "gText_PlayerFoundOneItem").await;
        std_msgbox(ctx, "gText_TooBadBagIsFull").await;
        ctx.set_var(VAR_RESULT, 0).await;
    }
    ctx.release_all().await;
    added
}

/// Gen3 `addmoney`.
pub async fn money_add(ctx: &Ctx, n: u32) {
    ctx.money_add(n).await
}

/// Gen3 `removemoney`.
pub async fn money_remove(ctx: &Ctx, n: u32) {
    ctx.money_remove(n).await
}

/// Gen3 `checkmoney`.
pub async fn money_check(ctx: &Ctx, n: u32) -> bool {
    let ok = ctx.money_check(n).await;
    result(ctx, ok).await
}
