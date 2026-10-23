//! Chapter 1, Bois-Éther's buildings: the Herboriste (`BoisEther_Herboriste`, §3
//! Bois_Ether_Sc05), the Pokémon Center and the Mart (§2, §7 "Boutiques").

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "BoisEther_Herboriste",
    "BoisEther_PokemonCenter",
    "BoisEther_Mart",
];

// ---- the Herboriste ----

/// `MART_HERBORISTE` (§7): Heal Powder, Energy Powder, Energy Root; with the badge, Revival Herb.
const HERBORISTE: &[ItemId] = &[
    ItemId(ITEM_HEAL_POWDER),
    ItemId(ITEM_ENERGY_POWDER),
    ItemId(ITEM_ENERGY_ROOT),
];
const HERBORISTE_BADGE: &[ItemId] = &[
    ItemId(ITEM_HEAL_POWDER),
    ItemId(ITEM_ENERGY_POWDER),
    ItemId(ITEM_ENERGY_ROOT),
    ItemId(ITEM_REVIVAL_HERB),
];

/// Bois_Ether_Sc05, the herbalist: his welcome, the Heal Powder sample on the first visit
/// (`FLAG_SIL_ITEM_HERBORISTE_SAMPLE`; Herboriste_Cadeau says it is obtained), Loïc's "special
/// herbs" after the badge, then his shop.
pub async fn BoisEther_Herboriste_Sc05(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Herboriste_T1).await;
    if gift(
        &ctx,
        FLAG_SIL_ITEM_HERBORISTE_SAMPLE,
        ITEM_HEAL_POWDER,
        1,
        Obtain::Quiet,
    )
    .await
    {
        say(&ctx, Herboriste_Cadeau).await;
    }
    let badge = ctx.flag(FLAG_SIL_BADGE_01).await;
    if badge && ctx.flag(FLAG_SIL_ITEM_HERBORISTE_SAMPLE).await {
        say(&ctx, Herboriste_T2).await;
    }
    ctx.release().await;
    // fallback: mart(MART_HERBORISTE) (the shop screen, M9 Task J): a greeting, nothing sold.
    mart(&ctx, if badge { HERBORISTE_BADGE } else { HERBORISTE }).await;
}

// ---- the Pokémon Center ----

/// The Center is Bois-Éther's heal point: the respawn, as the player enters ("set_respawn à
/// l'entrée"; `HEAL_LOCATION_SIL_BOIS_ETHER`, in front of its door).
pub async fn BoisEther_PokemonCenter_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_BOIS_ETHER).await;
}

/// The nurse: Gen3's.
pub async fn BoisEther_PokemonCenter_EventScript_Nurse(ctx: Ctx) {
    pokecenter_nurse(&ctx).await;
}

pub async fn BoisEther_PokemonCenter_EventScript_Dresseur(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_PC_Npc, MsgBox::Npc).await;
}

// ---- the Mart ----

/// `MAP_BOIS_ETHER_MART` (§7): Poké Ball, Potion, Antidote, Parlyz Heal, Awakening, Repel; with
/// the badge, Super Potion.
const BOUTIQUE: &[ItemId] = &[
    ItemId(ITEM_POKE_BALL),
    ItemId(ITEM_POTION),
    ItemId(ITEM_ANTIDOTE),
    ItemId(ITEM_PARALYZE_HEAL),
    ItemId(ITEM_AWAKENING),
    ItemId(ITEM_REPEL),
];
const BOUTIQUE_BADGE: &[ItemId] = &[
    ItemId(ITEM_POKE_BALL),
    ItemId(ITEM_POTION),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_ANTIDOTE),
    ItemId(ITEM_PARALYZE_HEAL),
    ItemId(ITEM_AWAKENING),
    ItemId(ITEM_REPEL),
];

/// The clerk. fallback: `mart` (the shop screen, M9 Task J): a greeting, nothing sold.
pub async fn BoisEther_Mart_EventScript_Clerk(ctx: Ctx) {
    let badge = ctx.flag(FLAG_SIL_BADGE_01).await;
    mart(&ctx, if badge { BOUTIQUE_BADGE } else { BOUTIQUE }).await;
}

pub async fn BoisEther_Mart_EventScript_Client(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Mart_Npc, MsgBox::Npc).await;
}
