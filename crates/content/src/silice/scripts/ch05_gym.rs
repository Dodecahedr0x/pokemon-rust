//! Chapter 5, the Owl's Sanctuary (`Owlys_Gym`) and its crypt (`Owlys_Gym_B1F`):
//! `contenu-jeu/chapitres/05-owlys.md` §4 (the darkness and the trapdoors, the Owls' Court and
//! its pivoting bridges, the Hall of Pillars and its Watchers, the hanging platform, the guide)
//! and §3 Owlys_Gym_Sc01 (Skycker, the badge, Sustentation, TM12, Mathieu, `LOG_05`;
//! `VAR_SIL_STORY` 53 → 54).
//!
//! **The bridges** ("SIL3 Ch5 maps as built"): five bridges between five islets, posed (planks,
//! SilOwlysGym's `pont`, walkable) or raised (its `vide`, solid). `FLAG_SIL_OWLYS_GYM_P1`–`_P5`
//! hold their state; every load resets them (p1 posed, the others raised) until islet N is
//! reached (`FLAG_SIL_OWLYS_GYM_COUR_OK`), then they all stay posed. The red, blue and black
//! books each flip their bridges. The walkway to the platform comes down once the four Watchers
//! are beaten; each beaten Watcher puts its owl statue's red eyes out (`pilier_eteint`).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Owlys_Gym", "Owlys_Gym_B1F"];

/// SilOwlysGym (games/silice/gfx/SilOwlysGym.ron): `pont`, `vide`, and `pilier_eteint`'s two
/// cells (the pillar's top row, the owl's eyes out).
const POSE: u16 = 542;
const RELEVE: u16 = 541;
const METATILE_STATUE_ETEINTE: [u16; 2] = [553, 554];
/// Each Watcher's pillar: its top-left cell (the owl statue's row), as `VEILLEURS`.
const YEUX: [(i16, i16); 4] = [(7, 14), (20, 14), (7, 18), (20, 18)];

/// The bridges' cells (the map's `02_events.ron` lists them).
const P1: &[(i16, i16)] = &[
    (4, 34),
    (4, 35),
    (4, 36),
    (4, 37),
    (4, 38),
    (4, 39),
    (4, 40),
    (4, 41),
    (4, 42),
    (4, 43),
    (5, 43),
    (6, 43),
    (7, 43),
    (8, 43),
    (9, 43),
    (10, 43),
];
const P2: &[(i16, i16)] = &[
    (24, 34),
    (24, 35),
    (24, 36),
    (24, 37),
    (24, 38),
    (24, 39),
    (24, 40),
    (24, 41),
    (24, 42),
    (24, 43),
    (18, 43),
    (19, 43),
    (20, 43),
    (21, 43),
    (22, 43),
    (23, 43),
];
const P3: &[(i16, i16)] = &[(7, 31), (8, 31), (9, 31), (10, 31), (11, 31)];
const P4: &[(i16, i16)] = &[(17, 31), (18, 31), (19, 31), (20, 31), (21, 31)];
const P5: &[(i16, i16)] = &[(14, 25), (14, 26), (14, 27), (14, 28)];
/// The walkway from the Hall of Pillars to the platform.
const PASSERELLE: &[(i16, i16)] = &[(14, 6), (14, 7), (14, 8), (14, 9), (14, 10), (14, 11)];

/// Each bridge: its flag and cells.
const BRIDGES: [(FlagId, &[(i16, i16)]); 5] = [
    (FLAG_SIL_OWLYS_GYM_P1, P1),
    (FLAG_SIL_OWLYS_GYM_P2, P2),
    (FLAG_SIL_OWLYS_GYM_P3, P3),
    (FLAG_SIL_OWLYS_GYM_P4, P4),
    (FLAG_SIL_OWLYS_GYM_P5, P5),
];

const VEILLEURS: [FlagId; 4] = [
    FLAG_SIL_OWLYS_VEILLEUR_1,
    FLAG_SIL_OWLYS_VEILLEUR_2,
    FLAG_SIL_OWLYS_VEILLEUR_3,
    FLAG_SIL_OWLYS_VEILLEUR_4,
];

async fn paint(ctx: &Ctx, cells: &[(i16, i16)], posed: bool) {
    let (m, blocked) = if posed { (POSE, false) } else { (RELEVE, true) };
    for &(x, y) in cells {
        ctx.set_metatile(x, y, m, blocked).await;
    }
}

/// The source's `redraw_bridges()`: every bridge from its flag.
async fn draw_bridges(ctx: &Ctx) {
    for (f, cells) in BRIDGES {
        let posed = ctx.flag(f).await;
        paint(ctx, cells, posed).await;
    }
}

async fn all_watchers_beaten(ctx: &Ctx) -> bool {
    for f in VEILLEURS {
        if !ctx.flag(f).await {
            return false;
        }
    }
    true
}

// ---- map scripts ----

/// The bridges reset (p1 posed, the others raised) unless the court is solved (all posed);
/// the walkway posed once the four Watchers are beaten.
pub async fn Owlys_Gym_OnLoad(ctx: Ctx) {
    let solved = ctx.flag(FLAG_SIL_OWLYS_GYM_COUR_OK).await;
    for (i, (f, _)) in BRIDGES.into_iter().enumerate() {
        ctx.set_flag(f, solved || i == 0).await;
    }
    draw_bridges(&ctx).await;
    let watched = all_watchers_beaten(&ctx).await;
    paint(&ctx, PASSERELLE, watched).await;
    for (f, cell) in VEILLEURS.into_iter().zip(YEUX) {
        if ctx.flag(f).await {
            eyes_out(&ctx, cell).await;
        }
    }
}

/// The source's `set_metatile(X_YEUX_n, Y_YEUX_n, METATILE_STATUE_ETEINTE, true)`: the
/// pillar's top row (two cells) with the owl's eyes out.
async fn eyes_out(ctx: &Ctx, (x, y): (i16, i16)) {
    for (dx, m) in METATILE_STATUE_ETEINTE.into_iter().enumerate() {
        ctx.set_metatile(x + dx as i16, y, m, true).await;
    }
}

/// Skycker masked until the badge, then his face shown; Gym_Sc01's Mathieu only in the scene.
pub async fn Owlys_Gym_OnTransition(ctx: Ctx) {
    let badge = ctx.flag(FLAG_SIL_BADGE_05).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GYM_SKYCKER_MASQUE, badge)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GYM_SKYCKER_VISAGE, !badge)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GYM_MATHIEU, true).await;
}

// ---- the Owls' Court ----

/// A book read: its text, then (until the court is solved) its bridges flip and the court is
/// redrawn. The source's mechanism sound is its text's "CLAC".
async fn book(ctx: &Ctx, text: TextId, flips: &[FlagId]) {
    msgbox(ctx, text, MsgBox::Sign).await;
    if ctx.flag(FLAG_SIL_OWLYS_GYM_COUR_OK).await {
        return;
    }
    ctx.lock().await;
    for &f in flips {
        let on = ctx.flag(f).await;
        ctx.set_flag(f, !on).await;
    }
    draw_bridges(ctx).await;
    ctx.redraw_map().await;
    ctx.release().await;
}

/// The red book on islet S ("the day's lies"): p1 and p2. Read first, it strands the player
/// on E (the trap; E's trapdoor is the way out).
pub async fn Owlys_Gym_EventScript_LivreRouge(ctx: Ctx) {
    book(
        &ctx,
        Owlys_Gym_Livre_Rouge,
        &[FLAG_SIL_OWLYS_GYM_P1, FLAG_SIL_OWLYS_GYM_P2],
    )
    .await;
}

/// The blue book on islet O: p1, p3 and p4.
pub async fn Owlys_Gym_EventScript_LivreBleu(ctx: Ctx) {
    book(
        &ctx,
        Owlys_Gym_Livre_Bleu,
        &[
            FLAG_SIL_OWLYS_GYM_P1,
            FLAG_SIL_OWLYS_GYM_P3,
            FLAG_SIL_OWLYS_GYM_P4,
        ],
    )
    .await;
}

/// The black book on islet E: p2 and p5.
pub async fn Owlys_Gym_EventScript_LivreNoir(ctx: Ctx) {
    book(
        &ctx,
        Owlys_Gym_Livre_Noir,
        &[FLAG_SIL_OWLYS_GYM_P2, FLAG_SIL_OWLYS_GYM_P5],
    )
    .await;
}

/// Islet N reached (its row y 24, on `VAR_TEMP_1` 0): the court is solved, every bridge stays
/// posed (`FLAG_SIL_OWLYS_GYM_COUR_OK`). `VAR_TEMP_1` 1 silences the row for this visit.
pub async fn Owlys_Gym_EventScript_CourOk(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if ctx.flag(FLAG_SIL_OWLYS_GYM_COUR_OK).await {
        return;
    }
    ctx.set_flag(FLAG_SIL_OWLYS_GYM_COUR_OK, true).await;
    for (f, _) in BRIDGES {
        ctx.set_flag(f, true).await;
    }
    draw_bridges(&ctx).await;
    ctx.redraw_map().await;
}

// ---- the Hall of Pillars ----

/// A Watcher of §4, beaten: its flag, its statue's eyes out, and once all four are, the
/// walkway comes down (`Owlys_Gym_Passerelle`, a rumble, the six cells posed).
async fn watcher_beaten(ctx: &Ctx, flag: FlagId) {
    if ctx.flag(flag).await {
        return;
    }
    ctx.set_flag(flag, true).await;
    if let Some(i) = VEILLEURS.iter().position(|&f| f == flag) {
        eyes_out(ctx, YEUX[i]).await;
        ctx.redraw_map().await;
    }
    if all_watchers_beaten(ctx).await {
        ctx.lock_all().await;
        say(ctx, Owlys_Gym_Passerelle).await;
        screen_shake(ctx, 2, 30).await;
        paint(ctx, PASSERELLE, true).await;
        ctx.redraw_map().await;
        ctx.release_all().await;
    }
}

/// ARIANE, pillar 1 (9, 14). The source hides each Watcher inside its pillar until the player
/// crosses its sight (Emerald's buried trainers): a fallback, they stand beside the pillars.
pub async fn Owlys_Gym_EventScript_Ariane(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM_VEILLEUR_1,
        TRAINER_SIL_OWLYS_GYM_ARIANE,
        Owlys_Gym_Ariane_Intro,
        Owlys_Gym_Ariane_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Gym_Ariane_Defeat).await;
    }
    watcher_beaten(&ctx, FLAG_SIL_OWLYS_VEILLEUR_1).await;
}

/// MORGANE, pillar 2 (19, 14).
pub async fn Owlys_Gym_EventScript_Morgane(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM_VEILLEUR_2,
        TRAINER_SIL_OWLYS_GYM_MORGANE,
        Owlys_Gym_Morgane_Intro,
        Owlys_Gym_Morgane_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Gym_Morgane_Defeat).await;
    }
    watcher_beaten(&ctx, FLAG_SIL_OWLYS_VEILLEUR_2).await;
}

/// CORVUS, pillar 3 (9, 19).
pub async fn Owlys_Gym_EventScript_Corvus(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM_VEILLEUR_3,
        TRAINER_SIL_OWLYS_GYM_CORVUS,
        Owlys_Gym_Corvus_Intro,
        Owlys_Gym_Corvus_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Gym_Corvus_Defeat).await;
    }
    watcher_beaten(&ctx, FLAG_SIL_OWLYS_VEILLEUR_3).await;
}

/// NYX, pillar 4 (19, 19).
pub async fn Owlys_Gym_EventScript_Nyx(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM_VEILLEUR_4,
        TRAINER_SIL_OWLYS_GYM_NYX,
        Owlys_Gym_Nyx_Intro,
        Owlys_Gym_Nyx_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Gym_Nyx_Defeat).await;
    }
    watcher_beaten(&ctx, FLAG_SIL_OWLYS_VEILLEUR_4).await;
}

/// The gym guide by the entrance (whispering is the rule).
pub async fn Owlys_Gym_EventScript_Guide(ctx: Ctx) {
    msgbox(&ctx, Owlys_Gym_Guide, MsgBox::Npc).await;
}

/// The Sharp Beak on islet E's perch (the source wants it seen only under the light: the engine
/// draws no darkness, so it is always drawn).
pub async fn Owlys_Gym_EventScript_ItemBecPointu(ctx: Ctx) {
    find_item(&ctx, ITEM_SHARP_BEAK, 1).await;
}

// ---- the platform ----

/// Mathieu comes up the walkway (`Walk(Up) x4`, from (14, 9) to below the player at (14, 4)).
const MATHIEU_UP: &[Step] = &[walk(Dir::Up), walk(Dir::Up), walk(Dir::Up), walk(Dir::Up)];

/// Owlys_Gym_Sc01, Skycker, Governor of the Shadow (masked, on the platform, `VAR_SIL_STORY`
/// 53, the walkway down; before the badge). His battle, the mask off (two objects on one
/// cell), the Owl Badge (`FLAG_SIL_BADGE_05`), Sustentation (`FLAG_SIL_MODULE_VOL`), TM12 Taunt
/// (`FLAG_SIL_BADGE_05` is its §7 flag: given once, inside this scene), Mathieu stays
/// (`FLAG_SIL_OWLYS_MATHIEU_RESTE`), sync 50, `LOG_05`; the elevator's Watchers leave. Sets 54.
pub async fn Owlys_Gym_Sc01(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_BADGE_05).await {
        msgbox(&ctx, Owlys_Gym_Skycker_Apres, MsgBox::Npc).await;
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    say(&ctx, Owlys_Gym_Sc01_T1).await;
    // fallback: Mega Evolution (no form change in the data yet, "Silice engine: held items,
    // Fairy, Mega/Primal, abilities (as built)"): Absol battles unevolved and
    // Owlys_Gym_Sc01_Mega is not shown. The source's custom night battle background is the
    // engine's plain one (`World::environment` knows tall grass and plain only).
    fallback(&ctx, "mega_evolution").await;
    if battle_trainer(&ctx, TRAINER_SIL_SKYCKER_1, Owlys_Gym_Sc01_Defeat).await
        != BattleOutcome::Won
    {
        ctx.release_all().await;
        return;
    }
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_GYM_SKYCKER_MASQUE).await;
    show(
        &ctx,
        LOCALID_GYM_SKYCKER_VISAGE,
        FLAG_SIL_HIDE_OWLYS_GYM_SKYCKER_VISAGE,
    )
    .await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Owlys_Gym_Sc01_T2).await;
    // fallback: play_fanfare(MUS_OBTAIN_BADGE) (audio, M12): the badge line alone.
    fallback(&ctx, "play_fanfare").await;
    say(&ctx, Owlys_Gym_Sc01_Badge).await;
    ctx.set_flag(FLAG_SIL_BADGE_05, true).await;
    say(&ctx, Owlys_Gym_Sc01_T3).await;
    ctx.set_flag(FLAG_SIL_MODULE_VOL, true).await;
    say(&ctx, Owlys_Gym_Sc01_Module).await;
    give_item(&ctx, ITEM_TM12, 1).await;
    say(&ctx, Owlys_Gym_Sc01_T4).await;
    show(&ctx, LOCALID_GYM_MATHIEU, FLAG_SIL_HIDE_OWLYS_GYM_MATHIEU).await;
    move_wait(&ctx, LOCALID_GYM_MATHIEU, MATHIEU_UP).await;
    say(&ctx, Owlys_Gym_Sc01_T5).await;
    say(&ctx, Owlys_Gym_Sc01_T6).await;
    say(&ctx, Owlys_Gym_Sc01_T7).await;
    ctx.set_var(VAR_SIL_SYNC, 50).await;
    // fallback: show_sync (the sync gauge): nothing drawn; Sync's text gives the value.
    show_sync(&ctx, 50).await;
    say(&ctx, Owlys_Gym_Sc01_Sync).await;
    // show_log(LOG_05): the log as a message, as the Mallette's RAPPORTS reads it again.
    say(&ctx, Owlys_Gym_Log05).await;
    ctx.set_flag(FLAG_SIL_LOG_05, true).await;
    ctx.set_flag(FLAG_SIL_OWLYS_MATHIEU_RESTE, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GARDES_ASCENSEUR, true)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GARDE_ASCENSEUR_B, true)
        .await;
    set_story(&ctx, 54).await;
    // Not in the source: Mathieu leaves for the square (where `FLAG_SIL_OWLYS_MATHIEU_RESTE`
    // shows him). Left on the walkway (14, 5), the platform's only way down, he would wall the
    // player in until the map reloads.
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_GYM_MATHIEU).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.release_all().await;
}

/// Skycker, his face shown, after the badge.
pub async fn Owlys_Gym_EventScript_Skycker(ctx: Ctx) {
    msgbox(&ctx, Owlys_Gym_Skycker_Apres, MsgBox::Npc).await;
}

// ---- the crypt ----

/// The Hyper Potion in the crypt, a consolation after a fall.
pub async fn Owlys_Gym_B1F_EventScript_ItemHyperPotion(ctx: Ctx) {
    find_item(&ctx, ITEM_HYPER_POTION, 1).await;
}
