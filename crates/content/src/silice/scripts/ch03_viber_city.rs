//! Chapter 3, Viber-City (`ViberCity`) and its small buildings (`ViberCity_PokemonCenter`,
//! `ViberCity_Mart`, `ViberCity_House1`): `contenu-jeu/chapitres/03-viber-city.md` §2.3 to §2.6
//! (signs, ambient NPCs before and after the black-out, the east barrier's guard, the Center, the
//! Volt-Market, the street dancer), §3 Viber_City_Sc01 to Sc06 and §7 (the items).
//! `VAR_SIL_STORY` runs 31 (in sight of the city, from the depot) to 36 (Tauber's goodbye and
//! Julien's call: the barrier to Route S4 is open, Chapter 4 takes over). The Station Lumière is
//! `ch03_station.rs`, the Nocturne `ch03_gym.rs`, the Casino `ch03_casino.rs`.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "ViberCity",
    "ViberCity_PokemonCenter",
    "ViberCity_Mart",
    "ViberCity_House1",
];

/// The sector barrier to Route S4, two cells at the east street's end.
const BARRIERE: [(i16, i16); 2] = [(37, 15), (37, 16)];

/// `METATILE_BARRIERE_OUVERTE`: the open sector barrier, the city's street (General 94), walkable.
pub const METATILE_VIBER_BARRIERE_OUVERTE: u16 = 94;

// ---- map scripts ----

/// The east barrier drawn open once its guard left (`FLAG_SIL_VIBER_BARRIERE_EST`, Viber_City_Sc06),
/// and, after the black-out, the white neons.
pub async fn ViberCity_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_VIBER_BARRIERE_EST).await {
        for (x, y) in BARRIERE {
            ctx.set_metatile(x, y, METATILE_VIBER_BARRIERE_OUVERTE, false)
                .await;
        }
    }
    if ctx.flag(FLAG_SIL_VIBER_NEONS_BLANCS).await {
        // fallback: the white neons (§9: a "néons secours" tileset, or `set_metatile` on the
        // twelve main signs): the neons are static already ("SIL2 Ch2–Ch3 maps as built"), and
        // the set has no white variant. Nothing is redrawn; the texts say it.
        fallback(&ctx, "neon tiles").await;
    }
}

/// Mathieu's heal is once per visit (the source's temporary `FLAG_SIL_TMP_MATHIEU_HEAL`, cleared
/// on every entry). During the black-out, the silence. Viber_City_Sc05 is armed (`VAR_TEMP_1` 1)
/// when Station_Lumiere_Sc03 has just warped the player here (`FLAG_SIL_VIBER_BLACKOUT` at
/// `VAR_SIL_STORY` 33).
pub async fn ViberCity_OnTransition(ctx: Ctx) {
    ctx.set_flag(FLAG_SIL_TMP_MATHIEU_HEAL, false).await;
    let s = story(&ctx).await;
    if ctx.flag(FLAG_SIL_VIBER_BLACKOUT).await && s < 35 {
        // fallback: stop_music (audio, M12): the map's own music.
        fallback(&ctx, "stop_music").await;
    }
    if ctx.flag(FLAG_SIL_VIBER_BLACKOUT).await && s == 33 {
        ctx.set_var(VAR_TEMP_1, 1).await;
    }
}

// ---- scenes ----

/// The player's way from the south ramp (20, 29) to Mathieu's stand (20, 17), re-derived for the
/// built city (the source's `Walk(Up) x12` crosses the Casino): up the avenue, round the Casino's
/// west side, east along the square.
const RAMPE_STAND: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    face(Dir::Up),
];
const FACE_UP: &[Step] = &[face(Dir::Up)];
const FACE_LEFT: &[Step] = &[face(Dir::Left)];
const FACE_RIGHT: &[Step] = &[face(Dir::Right)];
const FACE_DOWN: &[Step] = &[face(Dir::Down)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];
/// Adrien's way from his bench (21, 17) to the Center's door front (6, 15).
const ADRIEN_CENTRE: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
];

/// Viber_City_Sc01, arrival: Mathieu's street food. The south ramp (20, 29) at `VAR_SIL_STORY`
/// 31: the player is walked to the stand, gets the Neon Skewers and the Lemonades, Adrien leaves
/// for the Center. Sets 32.
pub async fn ViberCity_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Viber_City_Sc01_T1).await;
    say(&ctx, Viber_City_Sc01_T2).await;
    move_wait(&ctx, PLAYER, RAMPE_STAND).await;
    say(&ctx, Viber_City_Sc01_T3).await;
    // §7 "Donné", no flag: the scene runs once.
    give_item(&ctx, ITEM_SIL_BROCHETTE_NEON, 3).await;
    give_item(&ctx, ITEM_LEMONADE, 2).await;
    ctx.move_obj(PLAYER, FACE_RIGHT).await;
    move_wait(&ctx, LOCALID_VIBER_ADRIEN, FACE_LEFT).await;
    say(&ctx, Viber_City_Sc01_T4).await;
    say(&ctx, Viber_City_Sc01_T5).await;
    move_wait(&ctx, LOCALID_VIBER_ADRIEN, ADRIEN_CENTRE).await;
    hide(&ctx, LOCALID_VIBER_ADRIEN).await;
    move_wait(&ctx, PLAYER, FACE_UP).await;
    say(&ctx, Viber_City_Sc01_T6).await;
    set_story(&ctx, 32).await;
    ctx.release_all().await;
}

/// Viber_City_Sc02, the giant neon sign (20, 12): the Mallette's analysis, once
/// (`FLAG_SIL_VIBER_NEON_SCANNED`). Afterwards the source's `Viber_City_Neon_T2` is not in
/// `text/fr.ron`: the sign says nothing more.
pub async fn ViberCity_Sc02(ctx: Ctx) {
    if story(&ctx).await < 32 || ctx.flag(FLAG_SIL_VIBER_NEON_SCANNED).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Viber_City_Sc02_T1).await;
    ctx.set_flag(FLAG_SIL_VIBER_NEON_SCANNED, true).await;
    ctx.release_all().await;
}

/// Viber_City_Sc03, the Nocturne's door and Tauber: the row in front of the dancers (27–30, 6)
/// at `VAR_SIL_STORY` 32. Tauber comes out onto the door cell (28, 4) and stays there (the
/// source's `Walk(Down) x2` runs into dancer 1 and the player). Sets 33.
pub async fn ViberCity_Sc03(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Viber_City_Sc03_T1).await;
    ctx.door(28, 4, true).await;
    ctx.wait_door().await;
    show(&ctx, LOCALID_VIBER_TAUBER, FLAG_SIL_HIDE_VIBER_TAUBER).await;
    move_wait(&ctx, LOCALID_VIBER_TAUBER, FACE_DOWN).await;
    ctx.door(28, 4, false).await;
    ctx.wait_door().await;
    say(&ctx, Viber_City_Sc03_T2).await;
    say(&ctx, Viber_City_Sc03_T3).await;
    say(&ctx, Viber_City_Sc03_T4).await;
    say(&ctx, Viber_City_Sc03_T5).await;
    ctx.door(28, 4, true).await;
    ctx.wait_door().await;
    hide(&ctx, LOCALID_VIBER_TAUBER).await;
    ctx.door(28, 4, false).await;
    ctx.wait_door().await;
    say(&ctx, Viber_City_Sc03_T6).await;
    set_story(&ctx, 33).await;
    ctx.release_all().await;
}

/// Mathieu (with his cart) comes from (5, 7) to below the Station's front, facing the guard.
const PRAUGECT_IN: &[Step] = &[walk(Dir::Right), walk(Dir::Right), face(Dir::Up)];
const PRAUGECT_LEAVE: &[Step] = &[walk(Dir::Left), walk(Dir::Left), walk(Dir::Left)];
const PRAUGECT_OUT: &[Step] = &[walk(Dir::Left), walk(Dir::Left)];
/// The guard follows, from (8, 5): down first, or left first when the player stands on (8, 6).
const GARDE_FOLLOW: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
];
const GARDE_FOLLOW_LEFT: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Down),
    walk(Dir::Left),
    walk(Dir::Left),
];
const GARDE_OUT: &[Step] = &[walk(Dir::Left), walk(Dir::Left)];

/// Viber_City_Sc04, PRAUGECT's diversion: the cells beside the Station's guard (7, 5), (9, 5)
/// and (8, 6) at `VAR_SIL_STORY` 33. Mathieu (as PRAUGECT) lures the guard away with skewers;
/// both leave (the stand stays empty until Sc05). The door is free. Once only: the guard's
/// hide flag says it played (leaving the Station puts the player on (8, 5), past these cells).
pub async fn ViberCity_Sc04(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_HIDE_STATION_GARDE).await {
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_VIBER_GARDE_STATION, FACE_PLAYER).await;
    say(&ctx, Viber_City_Sc04_T1).await;
    show(
        &ctx,
        LOCALID_VIBER_MATHIEU_CHARIOT,
        FLAG_SIL_HIDE_VIBER_MATHIEU_CHARIOT,
    )
    .await;
    move_wait(&ctx, LOCALID_VIBER_MATHIEU_CHARIOT, PRAUGECT_IN).await;
    say(&ctx, Viber_City_Sc04_T2).await;
    say(&ctx, Viber_City_Sc04_T3).await;
    move_wait(&ctx, LOCALID_VIBER_MATHIEU_CHARIOT, PRAUGECT_LEAVE).await;
    let follow = if ctx.player_pos().await == (8, 6) {
        GARDE_FOLLOW_LEFT
    } else {
        GARDE_FOLLOW
    };
    move_wait(&ctx, LOCALID_VIBER_GARDE_STATION, follow).await;
    say(&ctx, Viber_City_Sc04_T4).await;
    ctx.move_obj(LOCALID_VIBER_MATHIEU_CHARIOT, PRAUGECT_OUT).await;
    move_wait(&ctx, LOCALID_VIBER_GARDE_STATION, GARDE_OUT).await;
    ctx.wait_move(LOCALID_VIBER_MATHIEU_CHARIOT).await;
    hide(&ctx, LOCALID_VIBER_MATHIEU_CHARIOT).await;
    hide(&ctx, LOCALID_VIBER_GARDE_STATION).await;
    // Mathieu is away from his stand until the black-out (Sc05).
    hide(&ctx, LOCALID_VIBER_MATHIEU).await;
    say(&ctx, Viber_City_Sc04_T5).await;
    ctx.release_all().await;
}

const DANSEURS: [u8; 4] = [
    LOCALID_VIBER_DANSEUR_1,
    LOCALID_VIBER_DANSEUR_2,
    LOCALID_VIBER_DANSEUR_3,
    LOCALID_VIBER_DANSEUR_4,
];
const RIGHT_3: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    Step::FacePlayer,
];
const UP_2: &[Step] = &[walk(Dir::Up), walk(Dir::Up), Step::FacePlayer];

/// Viber_City_Sc05, the city in the dark: the map's on-frame entry on `VAR_TEMP_1` 1, which
/// `ViberCity_OnTransition` arms right after Station_Lumiere_Sc03's warp to (8, 5)
/// (`FLAG_SIL_VIBER_BLACKOUT`, `VAR_SIL_STORY` 33). The dancers sit, Adrien comes from (4, 5)
/// and Mathieu with his cart from (8, 8) (both placed before they are shown), the white neons.
/// Sets 34: the Nocturne is open.
pub async fn ViberCity_Sc05(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 0).await;
    ctx.lock_all().await;
    // fallback: set_flash_level(2) on an outdoor map (the circle of light, §9): the black-out's
    // texts on the map as it is.
    fallback(&ctx, "set_flash_level").await;
    say(&ctx, Viber_City_Sc05_T1).await;
    say(&ctx, Viber_City_Sc05_T2).await;
    say(&ctx, Viber_City_Sc05_T3).await;
    say(&ctx, Viber_City_Sc05_T4).await;
    for d in DANSEURS {
        ctx.move_obj(d, FACE_DOWN).await;
    }
    ctx.wait_move(LOCALID_VIBER_DANSEUR_4).await;
    ctx.delay(30).await;
    ctx.set_object_template_pos(LOCALID_VIBER_ADRIEN, 4, 5).await;
    show(&ctx, LOCALID_VIBER_ADRIEN, FLAG_SIL_HIDE_VIBER_ADRIEN).await;
    move_wait(&ctx, LOCALID_VIBER_ADRIEN, RIGHT_3).await;
    say(&ctx, Viber_City_Sc05_T5).await;
    ctx.set_object_template_pos(LOCALID_VIBER_MATHIEU_CHARIOT, 8, 8)
        .await;
    show(
        &ctx,
        LOCALID_VIBER_MATHIEU_CHARIOT,
        FLAG_SIL_HIDE_VIBER_MATHIEU_CHARIOT,
    )
    .await;
    move_wait(&ctx, LOCALID_VIBER_MATHIEU_CHARIOT, UP_2).await;
    say(&ctx, Viber_City_Sc05_T6).await;
    ctx.fade(Fade::ToBlack).await;
    for d in DANSEURS {
        hide(&ctx, d).await;
    }
    hide(&ctx, LOCALID_VIBER_ADRIEN).await;
    hide(&ctx, LOCALID_VIBER_MATHIEU_CHARIOT).await;
    // Mathieu is back at his stand.
    show(&ctx, LOCALID_VIBER_MATHIEU, FLAG_SIL_HIDE_VIBER_MATHIEU).await;
    ctx.set_flag(FLAG_SIL_VIBER_NEONS_BLANCS, true).await;
    // fallback: the white neons (as `ViberCity_OnLoad`).
    fallback(&ctx, "neon tiles").await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Viber_City_Sc05_T7).await;
    set_story(&ctx, 34).await;
    ctx.release_all().await;
}

const TAUBER_OUT: &[Step] = &[walk(Dir::Down), Step::FacePlayer];
const TAUBER_IN: &[Step] = &[walk(Dir::Up)];

/// Viber_City_Sc06, Tauber's fear and Julien's call: the cells around the Nocturne's door front
/// ((27, 5), (29, 5), (28, 6)) at `VAR_SIL_STORY` 35. The barrier's guard leaves
/// (`FLAG_SIL_VIBER_BARRIERE_EST`) and the barrier opens. Sets 36: Chapter 3 ends.
pub async fn ViberCity_Sc06(ctx: Ctx) {
    ctx.lock_all().await;
    show(&ctx, LOCALID_VIBER_TAUBER, FLAG_SIL_HIDE_VIBER_TAUBER).await;
    move_wait(&ctx, LOCALID_VIBER_TAUBER, TAUBER_OUT).await;
    say(&ctx, Viber_City_Sc06_T1).await;
    say(&ctx, Viber_City_Sc06_T2).await;
    move_wait(&ctx, LOCALID_VIBER_TAUBER, TAUBER_IN).await;
    hide(&ctx, LOCALID_VIBER_TAUBER).await;
    // fallback: play_se (audio, M12) and the Match Call window (mallette_call, plan §1.1 P1):
    // T3 announces the call, T4 is Julien's line as a plain message.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    say(&ctx, Viber_City_Sc06_T3).await;
    mallette_call(&ctx, "JULIEN").await;
    say(&ctx, Viber_City_Sc06_T4).await;
    end_call(&ctx).await;
    open_barriere(&ctx).await;
    set_story(&ctx, 36).await;
    ctx.release_all().await;
}

/// The guard leaves (`FLAG_SIL_VIBER_BARRIERE_EST`, his hide flag) and the barrier is drawn open.
async fn open_barriere(ctx: &Ctx) {
    hide(ctx, LOCALID_VIBER_GARDE_BARRIERE).await;
    ctx.set_flag(FLAG_SIL_VIBER_BARRIERE_EST, true).await;
    for (x, y) in BARRIERE {
        ctx.set_metatile(x, y, METATILE_VIBER_BARRIERE_OUVERTE, false)
            .await;
    }
}

// ---- the cast ----

/// Mathieu at his stand (repeatable): `Viber_City_Mathieu_T1`, from the black-out on `_T2`
/// (the source's "après le black-out"); he feeds the party once per visit
/// (`FLAG_SIL_TMP_MATHIEU_HEAL`, cleared on every entry).
pub async fn ViberCity_EventScript_Mathieu(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    let line = if story(&ctx).await >= 34 {
        Viber_City_Mathieu_T2
    } else {
        Viber_City_Mathieu_T1
    };
    say(&ctx, line).await;
    if !ctx.flag(FLAG_SIL_TMP_MATHIEU_HEAL).await {
        ctx.heal_party().await;
        ctx.set_flag(FLAG_SIL_TMP_MATHIEU_HEAL, true).await;
    }
    ctx.release().await;
}

/// Adrien on his bench (shown at `VAR_SIL_STORY` 31 only; Sc01 takes him away at once).
pub async fn ViberCity_EventScript_Adrien(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Sc01_T5, MsgBox::Npc).await;
}

/// The dancers in front of the Nocturne (until the black-out).
pub async fn ViberCity_EventScript_Danseur(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Danseurs_T1, MsgBox::Npc).await;
}

/// The Station's Cocket guard (until Sc04).
pub async fn ViberCity_EventScript_GardeStation(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Sc04_T1, MsgBox::Npc).await;
}

/// PNJ 08, the east barrier's guard: without the Tempo badge, `Viber_City_Garde_T1`; with it,
/// `_T2` and he leaves (as Sc06 does; Sc06 runs first on the way out of the Nocturne).
pub async fn ViberCity_EventScript_GardeBarriere(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_BADGE_03).await {
        msgbox(&ctx, Viber_City_Garde_T1, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Viber_City_Garde_T2).await;
    open_barriere(&ctx).await;
    ctx.release().await;
}

/// An ambient line: `before` until the black-out (`VAR_SIL_STORY` < 34), `after` from it.
async fn avant_apres(ctx: &Ctx, before: TextId, after: TextId) {
    let text = if story(ctx).await >= 34 { after } else { before };
    msgbox(ctx, text, MsgBox::Npc).await;
}

/// The sweeper.
pub async fn ViberCity_EventScript_Pnj01(ctx: Ctx) {
    avant_apres(&ctx, Viber_City_Pnj01_T1, Viber_City_Pnj01_T2).await;
}

/// The balloon seller.
pub async fn ViberCity_EventScript_Pnj02(ctx: Ctx) {
    avant_apres(&ctx, Viber_City_Pnj02_T1, Viber_City_Pnj02_T2).await;
}

/// The grandpa.
pub async fn ViberCity_EventScript_Pnj03(ctx: Ctx) {
    avant_apres(&ctx, Viber_City_Pnj03_T1, Viber_City_Pnj03_T2).await;
}

/// The factory worker.
pub async fn ViberCity_EventScript_Pnj04(ctx: Ctx) {
    avant_apres(&ctx, Viber_City_Pnj04_T1, Viber_City_Pnj04_T2).await;
}

/// The kid in front of the Casino.
pub async fn ViberCity_EventScript_Pnj05(ctx: Ctx) {
    avant_apres(&ctx, Viber_City_Pnj05_T1, Viber_City_Pnj05_T2).await;
}

/// The tourist from Gris-Échec (one line: the source has no second).
pub async fn ViberCity_EventScript_Pnj06(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Pnj06_T1, MsgBox::Npc).await;
}

/// The street DJ.
pub async fn ViberCity_EventScript_Pnj07(ctx: Ctx) {
    avant_apres(&ctx, Viber_City_Pnj07_T1, Viber_City_Pnj07_T2).await;
}

// ---- items (§7) ----

/// The Revive at the canal's end in the Jardin des Néons (37, 26).
pub async fn ViberCity_EventScript_ItemRevive(ctx: Ctx) {
    find_item(&ctx, ITEM_REVIVE, 1).await;
}

/// The hidden Hyper Potion under a bench of the Jardin des Néons (29, 26), once.
pub async fn ViberCity_EventScript_HiddenHyperPotion(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of SIL1
    // Task F): a sign (`05_ch3_scripts.ron`) gives the item once, behind its flag.
    hidden_item(&ctx, FLAG_SIL_ITEM_VIBER_HYPER_POTION, ITEM_HYPER_POTION).await;
}

/// The hidden PP Up behind the Station (13, 3), there only after the black-out (§7), once.
pub async fn ViberCity_EventScript_HiddenPpUp(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_VIBER_BLACKOUT).await {
        return;
    }
    // fallback: hidden items (as the Hyper Potion's).
    hidden_item(&ctx, FLAG_SIL_ITEM_VIBER_PP_UP, ITEM_PP_UP).await;
}

// ---- signs ----

pub async fn ViberCity_EventScript_Panneau1(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Panneau_T1, MsgBox::Sign).await;
}

pub async fn ViberCity_EventScript_Panneau2(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Panneau_T2, MsgBox::Sign).await;
}

pub async fn ViberCity_EventScript_Panneau3(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Panneau_T3, MsgBox::Sign).await;
}

pub async fn ViberCity_EventScript_Panneau4(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Panneau_T4, MsgBox::Sign).await;
}

// ---- the Pokémon Center (§2.4) ----

/// The Center is Viber-City's heal point (`HEAL_LOCATION_SIL_VIBER_CITY`, in front of its door);
/// from the black-out on it is full: the six people under blankets show.
pub async fn ViberCity_PokemonCenter_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_VIBER_CITY).await;
    let full = ctx.flag(FLAG_SIL_VIBER_BLACKOUT).await;
    for f in [
        FLAG_SIL_HIDE_VIBER_PC_PNJ01,
        FLAG_SIL_HIDE_VIBER_PC_PNJ02,
        FLAG_SIL_HIDE_VIBER_PC_PNJ03,
        FLAG_SIL_HIDE_VIBER_PC_PNJ04,
        FLAG_SIL_HIDE_VIBER_PC_PNJ05,
        FLAG_SIL_HIDE_VIBER_PC_PNJ06,
    ] {
        ctx.set_flag(f, !full).await;
    }
}

/// The nurse: Gen3's; after the black-out, her apology first.
pub async fn ViberCity_PokemonCenter_EventScript_Nurse(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_VIBER_BLACKOUT).await {
        ctx.lock().await;
        ctx.face_player().await;
        say(&ctx, Viber_City_Centre_Infirmiere_T1).await;
        ctx.release().await;
    }
    pokecenter_nurse(&ctx).await;
}

/// The man under a blanket who sold his guitar.
pub async fn ViberCity_PokemonCenter_EventScript_Pnj01(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Centre_Pnj01_T1, MsgBox::Npc).await;
}

/// The young woman and her Skitty.
pub async fn ViberCity_PokemonCenter_EventScript_Pnj02(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Centre_Pnj02_T1, MsgBox::Npc).await;
}

/// The four other people under blankets: the source gives them no line; the sweeper's
/// after-black-out line stands in (an author question).
pub async fn ViberCity_PokemonCenter_EventScript_Couverture(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Pnj01_T2, MsgBox::Npc).await;
}

// ---- the Volt-Market (§2.5, §7 "Boutiques") ----

/// `MAP_VIBER_CITY_MART` (§7): Poké Ball, Great Ball, Super Potion, Hyper Potion, Antidote,
/// Parlyz Heal, Awakening, Burn Heal, Revive, Super Repel, Escape Rope, X Speed.
const VOLT_MARKET: &[ItemId] = &[
    ItemId(ITEM_POKE_BALL),
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_ANTIDOTE),
    ItemId(ITEM_PARALYZE_HEAL),
    ItemId(ITEM_AWAKENING),
    ItemId(ITEM_BURN_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_SUPER_REPEL),
    ItemId(ITEM_ESCAPE_ROPE),
    ItemId(ITEM_X_SPEED),
];

/// The clerk. fallback: `mart` (the shop screen, M9 Task J): a greeting, nothing sold.
pub async fn ViberCity_Mart_EventScript_Clerk(ctx: Ctx) {
    mart(&ctx, VOLT_MARKET).await;
}

pub async fn ViberCity_Mart_EventScript_Pnj01(ctx: Ctx) {
    msgbox(&ctx, Viber_City_Mart_Pnj01_T1, MsgBox::Npc).await;
}

// ---- the street dancer's house (§2.6) ----

/// The street dancer: the Quick Claw once (`FLAG_SIL_ITEM_VIBER_QUICK_CLAW`, with Gen3's obtain
/// messages: his line offers it), then his advice.
pub async fn ViberCity_House1_EventScript_Danseur(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_ITEM_VIBER_QUICK_CLAW).await {
        say(&ctx, Viber_City_Danseur_T2).await;
    } else {
        say(&ctx, Viber_City_Danseur_T1).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_VIBER_QUICK_CLAW,
            ITEM_QUICK_CLAW,
            1,
            Obtain::Message,
        )
        .await;
    }
    ctx.release().await;
}
