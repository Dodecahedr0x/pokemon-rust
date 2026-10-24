//! Chapter 9, Nébula: `contenu-jeu/chapitres/09-nebula.md` §2.4–2.7 (the city, the last
//! Centre, the Comptoir Cocket, the Gargote), §3 Nebula_Sc01 (the arrival on the quai),
//! Nebula_Sc02 (the council of war), Nebula_Sc03 (the diversion) and §7 (the items, the shop).
//! `VAR_SIL_STORY` 92 → 93 (on the quai) → 94 (the plan) → 95 (the diversion). The Bassin's
//! dive (Nebula_Sc04) lands in `ch09_tour.rs`.
//!
//! Real engine features: the Comptoir and the Centre's vending machine open the mart screen
//! (`crate::prelude::mart`; Silice's prelude `mart` is still the pre-M9 fallback that the
//! earlier chapters' spines run through). The quai's and the Bassin's deep water dive by
//! `crate::silice::DIVE_WARPS` (Gen3 `setdivewarp`, which `Nebula_OnDiveWarp` would run).
//!
//! The Chapter 12 duties on this map ("SIL6 Ch12 maps: review", Ch9's row) are here too:
//! `Nebula_OnLoad` opens the MAINTENANCE door and draws the Breach's crack from 126, and
//! `Nebula_OnTransition` shows Julien from 120 and Nebula_PG_Sc10's cast at 126 only. The
//! Ch12 scripts themselves are in `ch12_reveil.rs` and `ch12_breche.rs`.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Nebula", "Nebula_PokemonCenter", "Nebula_Mart", "Nebula_Gargote"];

// ---- Nébula: map scripts ----

/// The MAINTENANCE door (11, 18), SilNebula's maintenance door (`MB_ANIMATED_DOOR`), drawn solid.
const METATILE_PORTE_MAINTENANCE: u16 = 606;
/// General's cave mouth (`MB_NON_ANIMATED_DOOR`): the Breach's crack in the north wall.
const METATILE_FISSURE: u16 = 167;

/// From `VAR_SIL_STORY` 126 (Chapter 12): the MAINTENANCE door opens (Benou's lab, warp 4) and
/// the north wall cracks (the Breach, warps 5–6).
///
/// fallback: the source's optional Gwada broadcast on the giant screen while
/// `FLAG_SIL_NEBULA_DIVERSION` is set: SilNebula has no such metatile; the screen is left as drawn.
pub async fn Nebula_OnLoad(ctx: Ctx) {
    if story(&ctx).await >= 126 {
        ctx.set_metatile(11, 18, METATILE_PORTE_MAINTENANCE, false)
            .await;
        ctx.set_metatile(18, 3, METATILE_FISSURE, false).await;
        ctx.set_metatile(19, 3, METATILE_FISSURE, false).await;
    }
}

/// The Grande Porte guards' cells once the diversion has called them to the door (Sc03,
/// Sc03_T6 "Toutes les unités à la GRANDE PORTE"): either side of the door (28–29, 10), on row 11,
/// so that `NEBULA_TOUR_EXIT` (28, 11) and (29, 11) stay free.
const GARDES_PORTE: [(u8, (i16, i16)); 4] = [
    (LOCALID_NEBULA_GARDE_PORTE_1, (26, 11)),
    (LOCALID_NEBULA_GARDE_PORTE_2, (27, 11)),
    (LOCALID_NEBULA_GARDE_PORTE_3, (30, 11)),
    (LOCALID_NEBULA_GARDE_PORTE_4, (31, 11)),
];

/// The actors by story step: the gate guards at the door from 95 (Gen3 `setobjectxyperm`),
/// Julien at his tent from 120, Nebula_PG_Sc10's cast (Mathieu, the two guards, the wild
/// Tyranitar and Crobat) at 126 only (Thomas is shown by the scene itself).
pub async fn Nebula_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    if s >= 95 {
        for (obj, (x, y)) in GARDES_PORTE {
            ctx.set_object_template_pos(obj, x, y).await;
        }
    }
    ctx.set_flag(FLAG_SIL_HIDE_NEBULA_JULIEN, s < 120).await;
    for f in [
        FLAG_SIL_HIDE_NEBULA_PG_MATHIEU,
        FLAG_SIL_HIDE_NEBULA_PG_GARDE_1,
        FLAG_SIL_HIDE_NEBULA_PG_GARDE_2,
        FLAG_SIL_HIDE_NEBULA_PG_TYRANITAR,
        FLAG_SIL_HIDE_NEBULA_PG_CROBAT,
    ] {
        ctx.set_flag(f, s != 126).await;
    }
    // Chapter 12's destroyed sky (127–129).
    super::ch12_breche::destroyed_sky(&ctx).await;
}

/// Nothing: the map has no Dive connection, so its two dive spots (the Bassin (50–52, 10–13)
/// to the Tower's B1F, the quai (29–30, 54–55) back into the conduit) are
/// `crate::silice::DIVE_WARPS`, which the engine asks with the player's cell (the Gen3
/// `setdivewarp` this script would run).
pub async fn Nebula_OnDiveWarp(_ctx: Ctx) {}

// ---- Nébula: scenes ----

const DRONE_PASSE: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
];

/// Nebula_Sc01, the arrival: the on-frame script at `VAR_SIL_STORY` 92, as the player surfaces
/// on the quai's canal (the conduit's south exit). The drone flies 6 west; Gwada's message
/// names the Gargote. Sets 93.
pub async fn Nebula_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    // fallback: play_music(MUS_RG_SILPH) (the map's own music).
    fallback(&ctx, "play_music").await;
    say(&ctx, Nebula_Sc01_T1).await;
    move_wait(&ctx, LOCALID_NEBULA_DRONE, DRONE_PASSE).await;
    say(&ctx, Nebula_Sc01_T2).await;
    say(&ctx, Nebula_Annonce_T1).await;
    say(&ctx, Nebula_Sc01_T3).await;
    set_story(&ctx, 93).await;
    ctx.release_all().await;
}

/// The Grande Porte's door (its left cell, `Nebula_EventScript_GrandePorte`), Sc03's camera target.
const GRANDE_PORTE: (i16, i16) = (28, 10);

/// Nebula_Sc03, the diversion: the Bassin's fence either side of its gate (44, 11) and (44, 13),
/// at `VAR_SIL_STORY` 94. The signal (yes/no); Gwada on the giant screen, Mathieu on the bus
/// shelter, Adrien at the loudspeaker; the crowd looks up; the gate guards run to the Grande
/// Porte; the Bassin's guard leaves. Sets `FLAG_SIL_NEBULA_DIVERSION`, the friends gone from the
/// Gargote, and 95. Before 94 and after, the fence is silent (the source gives it no line).
///
/// `camera_pan(GRANDE_PORTE)` pans from the player to the Grande Porte's door ([`GRANDE_PORTE`])
/// and `camera_pan(PLAYER)` back after the guards' chief speaks. The guards are moved under a
/// fade (`set_object_pos`), which stands in for their run (the source's `Walk(Left) x4` would
/// take them away from the door). The giant
/// screen's `set_metatile(ECRAN_GEANT, ECRAN_GWADA)` has no metatile in SilNebula; Mathieu's jump
/// is left out; the crowd already faces up (towards the screen and the sky).
pub async fn Nebula_Sc03(ctx: Ctx) {
    if story(&ctx).await != 94 {
        return;
    }
    ctx.lock_all().await;
    if !ask(&ctx, Nebula_Sc03_T0).await {
        ctx.release_all().await;
        return;
    }
    say(&ctx, Nebula_Sc03_T1).await;
    let (px, py) = ctx.player_pos().await;
    camera_pan(&ctx, GRANDE_PORTE.0 - px, GRANDE_PORTE.1 - py).await;
    // fallback: play_music(MUS_EVER_GRANDE).
    fallback(&ctx, "play_music").await;
    say(&ctx, Nebula_Sc03_T2).await;
    show(&ctx, LOCALID_NEBULA_MATHIEU, FLAG_SIL_HIDE_NEBULA_MATHIEU).await;
    say(&ctx, Nebula_Sc03_T3).await;
    show(&ctx, LOCALID_NEBULA_ADRIEN, FLAG_SIL_HIDE_NEBULA_ADRIEN).await;
    say(&ctx, Nebula_Sc03_T4).await;
    say(&ctx, Nebula_Sc03_T5).await;
    ctx.fade(Fade::ToBlack).await;
    for (obj, (x, y)) in GARDES_PORTE {
        ctx.set_object_pos(obj, x, y).await;
        ctx.set_object_template_pos(obj, x, y).await;
    }
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Nebula_Sc03_T6).await;
    camera_pan(&ctx, 0, 0).await;
    // The Bassin guard's run north (`Walk(Up) x8`) is closed by the fence: he leaves under a fade.
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_NEBULA_GARDE_BASSIN).await;
    ctx.set_flag(FLAG_SIL_HIDE_NEBULA_GARDE_BASSIN, true).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_NEBULA_DIVERSION, true).await;
    for f in [
        FLAG_SIL_HIDE_NEBULA_AMIS_GARGOTE,
        FLAG_SIL_HIDE_NEBULA_GARGOTE_ADRIEN,
        FLAG_SIL_HIDE_NEBULA_GARGOTE_GWADA,
    ] {
        ctx.set_flag(f, true).await;
    }
    say(&ctx, Nebula_Sc03_T7).await;
    set_story(&ctx, 95).await;
    ctx.release_all().await;
}

// ---- Nébula: people ----

/// Mathieu on the bus shelter, from the diversion: his speech (the source writes no other line
/// for him there).
pub async fn Nebula_EventScript_Mathieu(ctx: Ctx) {
    msgbox(&ctx, Nebula_Sc03_T3, MsgBox::Npc).await;
}

/// Adrien at the loudspeaker, from the diversion: his poem.
pub async fn Nebula_EventScript_Adrien(ctx: Ctx) {
    msgbox(&ctx, Nebula_Sc03_T4, MsgBox::Npc).await;
}

/// The man before the screen; after the diversion, `Nebula_Npc_1b`.
pub async fn Nebula_EventScript_Npc1(ctx: Ctx) {
    let t = if story(&ctx).await >= 95 {
        Nebula_Npc_1b
    } else {
        Nebula_Npc_1
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

/// The woman at the wanted poster.
pub async fn Nebula_EventScript_Npc2(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_2, MsgBox::Npc).await;
}

/// The old man on his bench; after the diversion, `Nebula_Npc_5b`.
pub async fn Nebula_EventScript_Npc5(ctx: Ctx) {
    let t = if story(&ctx).await >= 95 {
        Nebula_Npc_5b
    } else {
        Nebula_Npc_5
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

/// The teenager with headphones.
pub async fn Nebula_EventScript_Npc8(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_8, MsgBox::Npc).await;
}

/// The child looking at the sky.
pub async fn Nebula_EventScript_Enfant(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_3, MsgBox::Npc).await;
}

/// The office worker.
pub async fn Nebula_EventScript_Employe(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_4, MsgBox::Npc).await;
}

/// The delivery boy on his bike.
pub async fn Nebula_EventScript_Livreur(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_7, MsgBox::Npc).await;
}

/// The rest of the crowd (34–39): the source gives them no line of their own (an author
/// question); the man before the screen's (`Nebula_Npc_1`, `_1b` after the diversion).
pub async fn Nebula_EventScript_Foule(ctx: Ctx) {
    Nebula_EventScript_Npc1(ctx).await;
}

/// The sbire at the east barricade.
pub async fn Nebula_EventScript_SbireBarricade(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_6, MsgBox::Npc).await;
}

/// The Bassin's guard on its gate (until the diversion) and the Grande Porte's four: the
/// source gives them no line (an author question); the barricade sbire's (`Nebula_Npc_6`).
pub async fn Nebula_EventScript_GardeBassin(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_6, MsgBox::Npc).await;
}

pub async fn Nebula_EventScript_GardePorte(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_6, MsgBox::Npc).await;
}

/// The drone that films the screens: Sc01's line about it (no line of its own).
pub async fn Nebula_EventScript_Drone(ctx: Ctx) {
    msgbox(&ctx, Nebula_Sc01_T2, MsgBox::Sign).await;
}

// ---- Nébula: signs and screens ----

/// The giant screen (Kimiko-Live).
pub async fn Nebula_EventScript_EcranGeant(ctx: Ctx) {
    msgbox(&ctx, Nebula_Ecran_1, MsgBox::Sign).await;
}

pub async fn Nebula_EventScript_AvisRecherche(ctx: Ctx) {
    msgbox(&ctx, Nebula_Ecran_2, MsgBox::Sign).await;
}

pub async fn Nebula_EventScript_EcranSol(ctx: Ctx) {
    msgbox(&ctx, Nebula_Ecran_3, MsgBox::Sign).await;
}

/// The loudspeaker: the periodic announcement's fallback (§9 "Annonce périodique": a
/// loudspeaker to read).
pub async fn Nebula_EventScript_HautParleur(ctx: Ctx) {
    msgbox(&ctx, Nebula_Annonce_T1, MsgBox::Sign).await;
}

/// The barricades (no module opens a watched wall): the source quotes no sign for them (an
/// author question); the barricade sbire's line.
pub async fn Nebula_EventScript_Barricade(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_6, MsgBox::Sign).await;
}

/// The Grande Porte, locked (laser barrier): the source gives it no text (an author question);
/// silent.
pub async fn Nebula_EventScript_GrandePorte(_ctx: Ctx) {}

/// The MAINTENANCE door ("ACCÈS RÉSERVÉ · B."): the source names the panel but writes no text
/// key for it (an author question); silent. From 126 it is Benou's lab's door (warp 4).
pub async fn Nebula_EventScript_PorteMaintenance(_ctx: Ctx) {}

// ---- Nébula: items ----

/// The broken projector crate at the screen's foot: the Élecsprintite, with the source's own
/// line (`Nebula_Manectite` says "obtient").
pub async fn Nebula_EventScript_ItemManectite(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Nebula_Manectite).await;
    if add_item(&ctx, ITEM_SIL_MANECTITE, 1).await {
        let obj = ctx.var(VAR_LAST_TALKED).await;
        ctx.remove_object(obj as u8).await;
    }
    ctx.release().await;
}

// ---- the Centre ----

/// The last Centre before the Tower: entering it sets the respawn (`HEAL_LOCATION_SIL_NEBULA`,
/// the cell before its door), the respawn for all of Chapter 10.
pub async fn Nebula_PokemonCenter_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_NEBULA).await;
}

/// UNITÉ-SOIN 7, the robot nurse: no question (the source's two lines around the heal).
pub async fn Nebula_PokemonCenter_EventScript_Nurse(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Nebula_Centre_Soin_T1).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.heal_party().await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Nebula_Centre_Soin_T2).await;
    ctx.release().await;
}

pub async fn Nebula_PokemonCenter_EventScript_Dresseuse(ctx: Ctx) {
    msgbox(&ctx, Nebula_Centre_Npc_1, MsgBox::Npc).await;
}

pub async fn Nebula_PokemonCenter_EventScript_Homme(ctx: Ctx) {
    msgbox(&ctx, Nebula_Centre_Npc_2, MsgBox::Npc).await;
}

/// The vending machine's stock ("saveurs certifiées").
const DISTRIBUTEUR: &[ItemId] = &[
    ItemId(ITEM_FRESH_WATER),
    ItemId(ITEM_SODA_POP),
    ItemId(ITEM_LEMONADE),
];

/// The vending machine: its panel, then the mart screen (the real one, `crate::prelude::mart`).
pub async fn Nebula_PokemonCenter_EventScript_Distributeur(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Nebula_Distributeur).await;
    crate::prelude::mart(&ctx, DISTRIBUTEUR).await;
    ctx.release().await;
}

/// The waiting room's Kimiko screen.
pub async fn Nebula_PokemonCenter_EventScript_Ecran(ctx: Ctx) {
    msgbox(&ctx, Nebula_Ecran_1, MsgBox::Sign).await;
}

// ---- the Comptoir Cocket ----

/// The end-game shop (§7), the Pierre Lien first (D-30).
const COMPTOIR: &[ItemId] = &[
    ItemId(ITEM_SIL_PIERRE_LIEN),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_TIMER_BALL),
    ItemId(ITEM_FULL_RESTORE),
    ItemId(ITEM_MAX_POTION),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_MAX_REPEL),
    ItemId(ITEM_X_ATTACK),
    ItemId(ITEM_X_DEFEND),
    ItemId(ITEM_X_SPEED),
    ItemId(ITEM_X_SPECIAL),
    ItemId(ITEM_GUARD_SPEC),
    ItemId(ITEM_DIRE_HIT),
];

/// The clerk: his line, the mart screen (`crate::prelude::mart`), Gen3's goodbye.
pub async fn Nebula_Mart_EventScript_Clerk(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Nebula_Mart_T1).await;
    crate::prelude::mart(&ctx, COMPTOIR).await;
    say(&ctx, gText_PleaseComeAgain).await;
    ctx.release().await;
}

/// The customer: no line in the source (an author question); the office worker's
/// (`Nebula_Npc_4`, "J'espère qu'il y aura des récompenses en points").
pub async fn Nebula_Mart_EventScript_Client(ctx: Ctx) {
    msgbox(&ctx, Nebula_Npc_4, MsgBox::Npc).await;
}

// ---- the Gargote ----

const MATHIEU_VIENT: &[Step] = &[walk(Dir::Down), walk(Dir::Down), Step::FacePlayer];

/// Nebula_Sc02, the council of war: the on-frame script at `VAR_SIL_STORY` 93, on entering.
/// Mathieu comes down two cells; the plan; two Full Restores (`give_item`); sets 94.
pub async fn Nebula_Gargote_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_GARGOTE_MATHIEU, MATHIEU_VIENT).await;
    say(&ctx, Nebula_Sc02_T1).await;
    say(&ctx, Nebula_Sc02_T2).await;
    say(&ctx, Nebula_Sc02_T3).await;
    say(&ctx, Nebula_Sc02_T4).await;
    say(&ctx, Nebula_Sc02_T5).await;
    say(&ctx, Nebula_Sc02_T6).await;
    say(&ctx, Nebula_Sc02_T7).await;
    say(&ctx, Nebula_Sc02_T8).await;
    give_item(&ctx, ITEM_FULL_RESTORE, 2).await;
    say(&ctx, Nebula_Sc02_T9).await;
    set_story(&ctx, 94).await;
    ctx.release_all().await;
}

pub async fn Nebula_Gargote_EventScript_Mathieu(ctx: Ctx) {
    msgbox(&ctx, Nebula_Gargote_Mathieu_T1, MsgBox::Npc).await;
}

pub async fn Nebula_Gargote_EventScript_Adrien(ctx: Ctx) {
    msgbox(&ctx, Nebula_Gargote_Adrien_T1, MsgBox::Npc).await;
}

pub async fn Nebula_Gargote_EventScript_Gwada(ctx: Ctx) {
    msgbox(&ctx, Nebula_Gargote_Gwada_T1, MsgBox::Npc).await;
}

/// Mme Fenouil: after the council (94), the Leftovers once (`FLAG_SIL_ITEM_NEBULA_RESTES`);
/// otherwise her soup.
pub async fn Nebula_Gargote_EventScript_Fenouil(ctx: Ctx) {
    if story(&ctx).await < 94 || ctx.flag(FLAG_SIL_ITEM_NEBULA_RESTES).await {
        msgbox(&ctx, Nebula_Gargote_Fenouil_T1, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Nebula_Gargote_Fenouil_T2).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_NEBULA_RESTES,
        ITEM_LEFTOVERS,
        1,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}
