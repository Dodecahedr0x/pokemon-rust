//! Chapter 8, Évalon's buildings: the Taverne du Dé à Vingt Faces and its Pigeonnier
//! (`Evalon_Taverne`: the heal, the respawn, the pigeon "PC", Q3's keeper, Evalon_Sc02's
//! friends), the Échoppe du Troc (`Evalon_Echoppe`), the Forge (`Evalon_Forge`, Q2) and the
//! Maison des Anciens (`Evalon_Conseil`, Evalon_Sc05, the Épreuve de l'Esprit):
//! `contenu-jeu/chapitres/08-evalon.md` §2.3–§2.7, §3 Sc02, Sc05 and Sc06, §7.

use super::ch08_evalon::{add_renom, epreuves_finies, fiche_lire};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "Evalon_Taverne",
    "Evalon_Echoppe",
    "Evalon_Forge",
    "Evalon_Conseil",
];

// ---- the Taverne ----

/// The Taverne is Évalon's Centre: entering it sets the respawn (`HEAL_LOCATION_SIL_EVALON`,
/// the cell before its door). The friends sit at their table from the arrival (82) to the
/// invasion's end (84); after the badge they are gone to Nébula.
pub async fn Evalon_Taverne_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_EVALON).await;
    let here = (82..=84).contains(&story(&ctx).await);
    for f in [
        FLAG_SIL_HIDE_EVALON_TAVERNE_MATHIEU,
        FLAG_SIL_HIDE_EVALON_TAVERNE_ADRIEN,
        FLAG_SIL_HIDE_EVALON_TAVERNE_GWADA,
    ] {
        ctx.set_flag(f, !here).await;
    }
}

/// Evalon_Sc02, the friends at the taverne (free talk): each one's line.
pub async fn Evalon_Sc02(ctx: Ctx) {
    let text = match ctx.var(VAR_LAST_TALKED).await as u8 {
        LOCALID_EVALON_TAVERNE_MATHIEU => Evalon_Taverne_Mathieu_T1,
        LOCALID_EVALON_TAVERNE_ADRIEN => Evalon_Taverne_Adrien_T1,
        _ => Evalon_Taverne_Gwada_T1,
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// The innkeeper heals: a bowl of stew and a bed (the source asks no question).
pub async fn Evalon_Taverne_EventScript_Aubergiste(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Evalon_Taverne_Soin_T1).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.heal_party().await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Evalon_Taverne_Soin_T2).await;
    ctx.release().await;
}

pub async fn Evalon_Taverne_EventScript_Serveuse(ctx: Ctx) {
    msgbox(&ctx, Evalon_Taverne_Npc_2, MsgBox::Npc).await;
}

pub async fn Evalon_Taverne_EventScript_Roliste(ctx: Ctx) {
    msgbox(&ctx, Evalon_Taverne_Npc_1, MsgBox::Npc).await;
}

/// The bard: the player's ballad from the Paladin rank (reputation 8), else the town
/// role-player's bard line (`Evalon_Npc_3`; author question: no line of his own below Paladin).
pub async fn Evalon_Taverne_EventScript_Barde(ctx: Ctx) {
    let text = if ctx.var(VAR_SIL_EVALON_RENOM).await >= 8 {
        Evalon_Barde_Ballade
    } else {
        Evalon_Npc_3
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// The pigeon keeper (Pigeonnier): Q3 first (the lost Plume-d'Argent; once it is home, the
/// Poudre Soin ×3, `FLAG_SIL_EVALON_Q3`, +1 reputation), then the pigeon "PC" ([`pigeon_pc`]).
pub async fn Evalon_Taverne_EventScript_Colombophile(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_Q3).await {
        pigeon_pc(&ctx).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_HIDE_EVALON_PIGEON).await {
        say(&ctx, Evalon_Q3_T2).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_EVALON_POUDRE,
            ITEM_HEAL_POWDER,
            3,
            Obtain::Message,
        )
        .await;
        ctx.set_flag(FLAG_SIL_EVALON_Q3, true).await;
        add_renom(&ctx, 1).await;
    } else {
        say(&ctx, Evalon_Q3_T1).await;
    }
    ctx.release().await;
}

/// The keeper's register (19–20, 1): the pigeon "PC".
pub async fn Evalon_Taverne_EventScript_Registre(ctx: Ctx) {
    pigeon_pc(&ctx).await;
}

/// The pigeon "PC" (§9 "PC par pigeon"): the keeper's line.
///
/// fallback: the PC's storage screen (M9's menus have no Pokémon storage yet): the line alone,
/// nothing opens. Reported (`pc`).
async fn pigeon_pc(ctx: &Ctx) {
    msgbox(ctx, Evalon_Taverne_PC_T1, MsgBox::Npc).await;
    fallback(ctx, "pc").await;
}

/// The house rules on the wall (2–3, 1): the role-players' first rule (`Evalon_Npc_8`; author
/// question: no line of their own).
pub async fn Evalon_Taverne_EventScript_Regles(ctx: Ctx) {
    msgbox(&ctx, Evalon_Npc_8, MsgBox::Sign).await;
}

// ---- the Échoppe ----

/// The Échoppe's stock (§7): always, and from the Chevalier rank (reputation 5) the reserve.
const STOCK: &[ItemId] = &[
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_HEAL_POWDER),
    ItemId(ITEM_ENERGY_POWDER),
    ItemId(ITEM_ENERGY_ROOT),
    ItemId(ITEM_REVIVAL_HERB),
    ItemId(ITEM_BERRY_JUICE),
    ItemId(ITEM_FRESH_WATER),
    ItemId(ITEM_MOOMOO_MILK),
];
const STOCK_CHEVALIER: &[ItemId] = &[
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_HEAL_POWDER),
    ItemId(ITEM_ENERGY_POWDER),
    ItemId(ITEM_ENERGY_ROOT),
    ItemId(ITEM_REVIVAL_HERB),
    ItemId(ITEM_BERRY_JUICE),
    ItemId(ITEM_FRESH_WATER),
    ItemId(ITEM_MOOMOO_MILK),
    ItemId(ITEM_LAVA_COOKIE),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_MAX_REPEL),
    ItemId(ITEM_HYPER_POTION),
];

/// The merchant: her line (the reserve's from the Chevalier rank), then the shop.
pub async fn Evalon_Echoppe_EventScript_Marchande(ctx: Ctx) {
    let chevalier = ctx.var(VAR_SIL_EVALON_RENOM).await >= 5;
    let (text, stock) = if chevalier {
        (Evalon_Echoppe_T2, STOCK_CHEVALIER)
    } else {
        (Evalon_Echoppe_T1, STOCK)
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
    mart(&ctx, stock).await;
}

/// A customer (author question: no line of her own; the taverne waitress's, on paying in
/// stories).
pub async fn Evalon_Echoppe_EventScript_Cliente(ctx: Ctx) {
    msgbox(&ctx, Evalon_Taverne_Npc_2, MsgBox::Npc).await;
}

// ---- the Forge (Q2) ----

/// Q2, the smith Hilde: the Minerai Ferreux from the dark gallery (`Evalon_Q2_T1`); brought
/// back, it is taken, the Macho Brace, `FLAG_SIL_EVALON_Q2`, +1 reputation. Afterwards, from the
/// Chevalier rank (reputation 5), the Galekingite once (`_T3`), else `_T4`; once given, her
/// thanks again (`_T2`; author question: no later line).
pub async fn Evalon_Forge_EventScript_Hilde(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_EVALON_Q2).await {
        if check_item(&ctx, ITEM_SIL_MINERAI_FERREUX, 1).await {
            say(&ctx, Evalon_Q2_T2).await;
            remove_item(&ctx, ITEM_SIL_MINERAI_FERREUX, 1).await;
            gift(
                &ctx,
                FLAG_SIL_ITEM_EVALON_BRACELET,
                ITEM_MACHO_BRACE,
                1,
                Obtain::Message,
            )
            .await;
            ctx.set_flag(FLAG_SIL_EVALON_Q2, true).await;
            add_renom(&ctx, 1).await;
        } else {
            say(&ctx, Evalon_Q2_T1).await;
        }
    } else if ctx.flag(FLAG_SIL_ITEM_EVALON_AGGRONITE).await {
        say(&ctx, Evalon_Q2_T2).await;
    } else if ctx.var(VAR_SIL_EVALON_RENOM).await >= 5 {
        say(&ctx, Evalon_Q2_T3).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_EVALON_AGGRONITE,
            ITEM_SIL_AGGRONITE,
            1,
            Obtain::Message,
        )
        .await;
    } else {
        say(&ctx, Evalon_Q2_T4).await;
    }
    ctx.release().await;
}

pub async fn Evalon_Forge_EventScript_Apprenti(ctx: Ctx) {
    msgbox(&ctx, Evalon_Forge_Npc_1, MsgBox::Npc).await;
}

/// The swords on the wall (3–4, 1): the apprentice's line on them (author question).
pub async fn Evalon_Forge_EventScript_Epees(ctx: Ctx) {
    msgbox(&ctx, Evalon_Forge_Npc_1, MsgBox::Sign).await;
}

// ---- the Maison des Anciens (Sc05) ----

/// The three questions: the asker's question, the honest answer's line, the boastful one's.
const QUESTIONS: [(TextId, TextId, TextId); 3] = [
    (Evalon_Sc05_Q1, Evalon_Sc05_R1_Bon, Evalon_Sc05_R1_Vantard),
    (Evalon_Sc05_Q2, Evalon_Sc05_R2_Bon, Evalon_Sc05_R2_Vantard),
    (Evalon_Sc05_Q3, Evalon_Sc05_R3_Bon, Evalon_Sc05_R3_Vantard),
];

/// Evalon_Sc05, the Épreuve de l'Esprit: the Doyenne Aldegonde (from (2, 5)). Three questions,
/// each honest answer +1 reputation; the Fiche de Personnage, `FLAG_SIL_EVALON_EPREUVE_ESPRIT`,
/// the sheet read, the trials' end. Afterwards she reads the sheet again, and at the Légende
/// rank (reputation 10) the Anciens give a PP Max once (`FLAG_SIL_EVALON_PP_MAX`, a new flag:
/// the source's §7 row has none).
pub async fn Evalon_Sc05(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_EVALON_EPREUVE_ESPRIT).await {
        fiche_lire(&ctx).await;
        if ctx.var(VAR_SIL_EVALON_RENOM).await >= 10 {
            gift(
                &ctx,
                FLAG_SIL_EVALON_PP_MAX,
                ITEM_PP_MAX,
                1,
                Obtain::Message,
            )
            .await;
        }
        ctx.release().await;
        return;
    }
    say(&ctx, Evalon_Sc05_T1).await;
    // fallback: multichoice (§9): the source's own fallback, a yes/no whose yes is the honest
    // answer.
    fallback(&ctx, "multichoice").await;
    for (q, bon, vantard) in QUESTIONS {
        if ask(&ctx, q).await {
            add_renom(&ctx, 1).await;
            say(&ctx, bon).await;
        } else {
            say(&ctx, vantard).await;
        }
    }
    say(&ctx, Evalon_Sc05_T2).await;
    give_item(&ctx, ITEM_SIL_FICHE_PERSONNAGE, 1).await;
    ctx.set_flag(FLAG_SIL_EVALON_EPREUVE_ESPRIT, true).await;
    fiche_lire(&ctx).await;
    epreuves_finies(&ctx).await;
    ctx.release().await;
}

/// Old Bohort (Q2's asker): his line on the honest answer (author question: no line of his own
/// outside the trial).
pub async fn Evalon_Conseil_EventScript_Bohort(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc05_R2_Bon, MsgBox::Npc).await;
}

/// Sister Mélisende (Q3's asker): as Bohort.
pub async fn Evalon_Conseil_EventScript_Melisende(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc05_R3_Bon, MsgBox::Npc).await;
}

/// The hand-drawn wall map, without the Dome.
pub async fn Evalon_Conseil_EventScript_Carte(ctx: Ctx) {
    msgbox(&ctx, Evalon_Conseil_Carte, MsgBox::Sign).await;
}
