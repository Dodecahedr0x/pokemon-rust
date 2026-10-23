//! Chapter 4, Port-Ressac's buildings: the Taverne (`PortRessac_Taverne`, the Pokémon Center,
//! §2 and §3 Port_Ressac_Taverne_Sc01, §7 "Boutique") and the Cocket cargo
//! (`PortRessac_NavireCocket`, §2, §3 Port_Ressac_Navire_Sc01, §5, §7).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["PortRessac_Taverne", "PortRessac_NavireCocket"];

// ---- the Taverne ----

/// Gwada at the bar from the arrival (`VAR_SIL_STORY` 42) until he waits on the Zenith-Grog's
/// coupée (48); Adrien asleep at his table once he has left the tunnel
/// (`FLAG_SIL_ROUTE_S04_ADRIEN`) until the chapter's end (49).
pub async fn PortRessac_Taverne_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    ctx.set_flag(FLAG_SIL_HIDE_PORT_TAVERNE_GWADA, !(42..=47).contains(&s))
        .await;
    let adrien = ctx.flag(FLAG_SIL_ROUTE_S04_ADRIEN).await && s < 49;
    ctx.set_flag(FLAG_SIL_HIDE_PORT_TAVERNE_ADRIEN, !adrien)
        .await;
}

/// Port_Ressac_Taverne_Sc01, Gwada at the bar (obj 3): the Rhum Pirate ×3 once
/// (`FLAG_SIL_ITEM_PORT_RHUM_PIRATE`, its §7 flag, and the source's `FLAG_SIL_PORT_GWADA_RHUM`,
/// which the shop reads), with his advice on Samuel; afterwards, his line on the Cocket sand.
pub async fn PortRessac_Taverne_Sc01(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_PORT_GWADA_RHUM).await {
        say(&ctx, Port_Ressac_Taverne_Sc01_T3).await;
    } else {
        say(&ctx, Port_Ressac_Taverne_Sc01_T1).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_PORT_RHUM_PIRATE,
            ITEM_SIL_RHUM_PIRATE,
            3,
            Obtain::Message,
        )
        .await;
        say(&ctx, Port_Ressac_Taverne_Sc01_T2).await;
        ctx.set_flag(FLAG_SIL_PORT_GWADA_RHUM, true).await;
    }
    ctx.release().await;
}

/// The pirate nurse: her welcome, the respawn here (`HEAL_LOCATION_SIL_PORT_RESSAC`, the cell in
/// front of the Taverne's door), then Gen3's nurse.
pub async fn PortRessac_Taverne_EventScript_Infirmiere(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_PORT_RESSAC).await;
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Port_Ressac_Taverne_Infirmiere).await;
    ctx.release().await;
    pokecenter_nurse(&ctx).await;
}

/// The counter's stock (§7): Great Ball, Super Potion, Hyper Potion, Full Heal, Revive, Super
/// Repel, Escape Rope, Dive Ball; Ultra Ball with the badge, Rhum Pirate once Gwada gave it.
const COMPTOIR: &[ItemId] = &[
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_SUPER_REPEL),
    ItemId(ITEM_ESCAPE_ROPE),
    ItemId(ITEM_DIVE_BALL),
];
const COMPTOIR_RHUM: &[ItemId] = &[
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_SUPER_REPEL),
    ItemId(ITEM_ESCAPE_ROPE),
    ItemId(ITEM_SIL_RHUM_PIRATE),
    ItemId(ITEM_DIVE_BALL),
];
const COMPTOIR_BADGE: &[ItemId] = &[
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_SUPER_REPEL),
    ItemId(ITEM_ESCAPE_ROPE),
    ItemId(ITEM_DIVE_BALL),
];
const COMPTOIR_BADGE_RHUM: &[ItemId] = &[
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_SUPER_REPEL),
    ItemId(ITEM_ESCAPE_ROPE),
    ItemId(ITEM_SIL_RHUM_PIRATE),
    ItemId(ITEM_DIVE_BALL),
];

/// The counter's clerk. fallback: `mart` (the shop screen, M9 Task J): a greeting, nothing sold.
pub async fn PortRessac_Taverne_EventScript_Vendeur(ctx: Ctx) {
    let badge = ctx.flag(FLAG_SIL_BADGE_04).await;
    let rhum = ctx.flag(FLAG_SIL_PORT_GWADA_RHUM).await;
    let stock = match (badge, rhum) {
        (false, false) => COMPTOIR,
        (false, true) => COMPTOIR_RHUM,
        (true, false) => COMPTOIR_BADGE,
        (true, true) => COMPTOIR_BADGE_RHUM,
    };
    mart(&ctx, stock).await;
}

pub async fn PortRessac_Taverne_EventScript_Adrien(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Taverne_Adrien, MsgBox::Npc).await;
}

pub async fn PortRessac_Taverne_EventScript_Pilier(ctx: Ctx) {
    msgbox(&ctx, Port_Ressac_Taverne_Pilier, MsgBox::Npc).await;
}

// ---- the Cocket cargo ----

/// Port_Ressac_Navire_Sc01, the terminal (19–20, 1): at `VAR_SIL_STORY` 44 (the Vieux Patch),
/// the Mallette decrypts `LOG_04` "SPECTRE-FRET" (`FLAG_SIL_LOG_04`). Sets 45. Before the patch,
/// the encryption (`_T1`; the source's "charabia chiffré" line has no text id); after, the log
/// again.
pub async fn PortRessac_NavireCocket_Sc01(ctx: Ctx) {
    let s = story(&ctx).await;
    if s < 44 {
        msgbox(&ctx, Port_Ressac_Navire_Sc01_T1, MsgBox::Sign).await;
        return;
    }
    if s > 44 {
        msgbox(&ctx, Port_Ressac_Navire_Log04, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    say(&ctx, Port_Ressac_Navire_Sc01_T1).await;
    say(&ctx, Port_Ressac_Navire_Sc01_T2).await;
    // The log itself, read once here. fallback: show_log(LOG_04), the Mallette's LOG menu
    // (SIL2 on M9's kit): the source's own fallback, the log as a message.
    say(&ctx, Port_Ressac_Navire_Log04).await;
    fallback(&ctx, "log_menu").await;
    say(&ctx, Port_Ressac_Navire_Sc01_T3).await;
    ctx.set_flag(FLAG_SIL_LOG_04, true).await;
    set_story(&ctx, 45).await;
    ctx.release().await;
}

/// The deck grunt by the gangway (2, 11), facing east, sight 4.
pub async fn PortRessac_NavireCocket_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NAVIRE_SBIRE_1,
        TRAINER_SIL_PORT_CARGO_1,
        Port_Ressac_Cargo_1_Intro,
        Port_Ressac_Cargo_1_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Cargo_1_Defeat).await;
    }
}

/// The grunt at the stairs' foot (9, 9), facing west, sight 2.
pub async fn PortRessac_NavireCocket_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NAVIRE_SBIRE_2,
        TRAINER_SIL_PORT_CARGO_2,
        Port_Ressac_Cargo_2_Intro,
        Port_Ressac_Cargo_2_Defeat,
    )
    .await
    {
        post_battle(&ctx, Port_Ressac_Cargo_2_Defeat).await;
    }
}

pub async fn PortRessac_NavireCocket_EventScript_PpPlus(ctx: Ctx) {
    find_item(&ctx, ITEM_PP_UP, 1).await;
}

/// Crow's crate in the hold (7, 4): its chalk crow, then the Lucky Punch under the straw, once
/// (`FLAG_SIL_ITEM_PORT_NAVIRE_POING_CHANCE`).
pub async fn PortRessac_NavireCocket_EventScript_CaisseCorbeau(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_ITEM_PORT_NAVIRE_POING_CHANCE).await {
        return;
    }
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`): a sign on the crate
    // (`06_ch4_scripts.ron`), which also lets the crate's own text show.
    fallback(&ctx, "hidden item").await;
    ctx.lock().await;
    say(&ctx, Port_Ressac_Navire_Caisse).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_PORT_NAVIRE_POING_CHANCE,
        ITEM_LUCKY_PUNCH,
        1,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}
