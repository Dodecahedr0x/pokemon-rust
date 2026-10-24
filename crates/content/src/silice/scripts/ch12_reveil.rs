//! Chapter 12, the post-game, Acts I and II: the awakening and the hunt for the Vestiges
//! (`contenu-jeu/chapitres/12-post-game.md` §3 `Brico_Bourg_PG_Sc01` to `Brico_Bourg_PG_Sc09`,
//! `PG_VestigeFin`, `PG_Lib_Kimiko`, `PG_Lib_Silas`, "Après la Traque").
//!
//! The way (`VAR_SIL_STORY` 119 → 126): the first true morning at Brico-Bourg (`Sc01`, on-frame on
//! 119; Mathieu's call, `FLAG_SIL_POSTGAME`, 120), the Council of Three at Manga-Square's
//! library (`Sc02`, 121), Julien at the Tower's foot (`Sc03`, 122; his daily rematch `Sc03b` and
//! the liberation `Sc03c` live with him), Thomas at the top of the lighthouse (`Sc04`, 123), then
//! the three Vestiges in any order (`Sc05` Kimiko, `Sc06` Arthur, `Sc07` Silas), each ending on
//! `PG_VestigeFin` (+1; the third one's call opens Act III at 126). Silas's mail (`Sc09`) waits
//! on the bedroom PC (`ch00_houses.rs` calls [`pg_mail_silas`]).
//!
//! The maps are earlier chapters' (Ch0, Ch5, Ch6, Ch7, Ch9): the scripts the Ch12 edits named
//! on them were stubs (`ch12_stubs.rs`) until this file.
//!
//! **Convention: the trainers' defeat texts.** The source calls `battle_trainer(TRAINER)` and
//! says the trainer's next line after the battle; §5 names that line as the defeat text
//! (`TRAINER_SIL_JULIEN_10`: intro `Nebula_PG_Sc03b_T1`, defeat `Nebula_PG_Sc03b_T3`). So the line
//! after each battle is its defeat text, said once, in the battle.

use super::ch08_evalon::lost;
use crate::silice::prelude::*;

// ---- the legendaries (PG_Legendaire, shared by the Ch12 files) ----

/// `PG_Legendaire` (§3): the legendary's line, its cry (a fallback), a wild battle at `level`; a
/// capture hides it for good (`hide`, its map flag `FLAG_SIL_HIDE_<SPECIES>`), defeating it or
/// running leaves it standing (the plan's "The legendaries' clear points").
pub(super) async fn legendaire(ctx: &Ctx, text: TextId, species: SpeciesId, level: u8, obj: u8) {
    ctx.lock().await;
    msgbox(ctx, text, MsgBox::Default).await;
    play_cry(ctx).await;
    if battle_wild(ctx, species, level).await == BattleOutcome::Caught {
        hide(ctx, obj).await;
    }
    ctx.release().await;
}

/// The source's `battle_trainer(TRAINER)` followed by the trainer's own next line: that line is
/// the battle's defeat text (the module's convention). Whether the player lost (a whiteout; the
/// script must end).
pub(super) async fn battle_lost(ctx: &Ctx, trainer: TrainerId, defeat: TextId) -> bool {
    lost(battle_trainer(ctx, trainer, defeat).await)
}

// ---- Act I: Brico-Bourg, the first true morning ----

const LOOK_UP: &[Step] = &[face(Dir::Up), Step::Delay(40)];

/// `Brico_Bourg_PG_Sc01`, the first true morning (on-frame at `VAR_SIL_STORY` 119, the first step
/// out of the house): the village looking up, Mathieu's call (the Council at Manga-Square), the
/// post-game's flags (`FLAG_SIL_POSTGAME`; Gogo and Romain leave the Apex, Revenge's statue is
/// gone, Poussin is not back on his stage yet), 120.
pub async fn BricoBourg_PG_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    // fallback: MUS_SIL_BRICO_ACOUSTIQUE (play_music): the map's music plays.
    fallback(&ctx, "play_music").await;
    move_wait(&ctx, PLAYER, LOOK_UP).await;
    say(&ctx, Brico_Bourg_PG_Sc01_T1).await;
    ctx.delay(30).await;
    play_se(&ctx, "SE_MALLETTE_APPEL").await;
    say(&ctx, Brico_Bourg_PG_Sc01_T2).await;
    mallette_call(&ctx, "MATHIEU").await;
    say(&ctx, Brico_Bourg_PG_Sc01_T3).await;
    ctx.set_flag(FLAG_SIL_POSTGAME, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_APEX_GOGO_ROMAIN, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_REVENGE_STATUE, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_APEX_POUSSIN_PG, true).await;
    set_story(&ctx, 120).await;
    ctx.release_all().await;
}

// ---- Act I: the Council of Three (Manga-Square's library) ----

/// The Council by story step (§2 "PNJ fixes"): Adrien and Mathieu from 120; Gwada at the table
/// from 120 except 127–129 (he is at the Tower); the two queuing citizens from 120; Kimiko beside
/// Adrien once pardoned ("Après la Traque").
pub async fn MangaSquare_Bibliotheque_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    let council = s >= 120;
    for f in [
        FLAG_SIL_HIDE_BIBLIO_PG_ADRIEN,
        FLAG_SIL_HIDE_BIBLIO_PG_MATHIEU,
        FLAG_SIL_HIDE_BIBLIO_PG_CITOYEN_1,
        FLAG_SIL_HIDE_BIBLIO_PG_CITOYEN_2,
    ] {
        ctx.set_flag(f, !council).await;
    }
    let gwada = council && !(127..=129).contains(&s);
    ctx.set_flag(FLAG_SIL_HIDE_BIBLIO_PG_GWADA, !gwada).await;
    let kimiko = council && ctx.flag(FLAG_SIL_PARDON_KIMIKO).await;
    ctx.set_flag(FLAG_SIL_HIDE_BIBLIO_PG_KIMIKO, !kimiko).await;
}

const FACE_PLAYER: &[Step] = &[Step::FacePlayer];
const FACE_DOWN: &[Step] = &[face(Dir::Down)];

/// `Manga_Square_PG_Sc02`, the Council of Three (row 22, the first row past the mat, at 120):
/// Mathieu's cardboard badge, Gwada's science, Adrien's resources, the three missing Pillars,
/// Julien at Nébula, five Full Restores (`FLAG_SIL_ITEM_PG_CONSEIL_GUERISONS`), 121.
///
/// Deviation: Mathieu (14, 21) stands right above the row the player steps on, so he turns to
/// the player rather than walking two cells down (and back up at the end).
pub async fn MangaSquare_Bibliotheque_PG_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_BIBLIO_PG_MATHIEU, FACE_PLAYER).await;
    say(&ctx, Manga_Square_PG_Sc02_T1).await;
    move_wait(&ctx, LOCALID_BIBLIO_PG_GWADA, FACE_DOWN).await;
    say(&ctx, Manga_Square_PG_Sc02_T2).await;
    move_wait(&ctx, LOCALID_BIBLIO_PG_ADRIEN, FACE_DOWN).await;
    say(&ctx, Manga_Square_PG_Sc02_T3).await;
    say(&ctx, Manga_Square_PG_Sc02_T4).await;
    say(&ctx, Manga_Square_PG_Sc02_T5).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_PG_CONSEIL_GUERISONS,
        ITEM_FULL_RESTORE,
        5,
        Obtain::Message,
    )
    .await;
    say(&ctx, Manga_Square_PG_Sc02_T6).await;
    set_story(&ctx, 121).await;
    ctx.release_all().await;
}

/// Kimiko at the Council's table, pardoned ("Après la Traque", `Manga_Square_PG_Kimiko`).
pub async fn MangaSquare_Bibliotheque_PG_Kimiko(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_PG_Kimiko, MsgBox::Npc).await;
}

/// Mathieu at the door. The source gives him no line outside `Sc02`: his welcome again
/// (`Manga_Square_PG_Sc02_T1`, the badge speech).
pub async fn MangaSquare_Bibliotheque_PG_Mathieu(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_PG_Sc02_T1, MsgBox::Npc).await;
}

/// The two queuing citizens with their numbered tickets (§2): the source gives them no line,
/// so they only turn to the player.
pub async fn MangaSquare_Bibliotheque_PG_Citoyen(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    ctx.release().await;
}

// ---- Act I: Julien at the Tower's foot (Nébula) ----

/// Julien at his tent (local 15, shown from 120 by `Nebula_OnTransition`): `Sc03` at 121, then
/// `Sc03c` (the liberation, once, from 130 after a first rematch won) before his daily rematch
/// (`Sc03b`, from 122). At 120 (before the Council sends the player) he only turns round: the
/// source gives him no line there.
pub async fn Nebula_PG_Julien(ctx: Ctx) {
    let s = story(&ctx).await;
    if s == 121 {
        nebula_pg_sc03(&ctx).await;
    } else if s >= 130
        && ctx.flag(FLAG_SIL_ITEM_PG_SALAMENCITE).await
        && !ctx.flag(FLAG_SIL_LIBERATION_JULIEN).await
    {
        nebula_pg_sc03c(&ctx).await;
    } else if s >= 122 {
        nebula_pg_sc03b(&ctx).await;
    } else {
        ctx.lock().await;
        ctx.face_player().await;
        ctx.release().await;
    }
}

/// `Nebula_PG_Sc03`, the Man of the Ruins (121): the gardens, the Dome to keep steady, Thomas's
/// letter (the lighthouse), the daily duel. 122.
async fn nebula_pg_sc03(ctx: &Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(ctx, Nebula_PG_Sc03_T1).await;
    say(ctx, Nebula_PG_Sc03_T2).await;
    say(ctx, Nebula_PG_Sc03_T3).await;
    set_story(ctx, 122).await;
    ctx.release().await;
}

/// `Nebula_PG_Sc03b`, Julien's daily rematch (`FLAG_SIL_DAILY_JULIEN`, cleared each day): the
/// legendaries' team (`TRAINER_SIL_JULIEN_9`) until `Sc03c`, then `TRAINER_SIL_JULIEN_10`; the
/// first win gives the Salamencite (`FLAG_SIL_ITEM_PG_SALAMENCITE`).
async fn nebula_pg_sc03b(ctx: &Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_DAILY_JULIEN).await {
        say(ctx, Nebula_PG_Sc03b_T4).await;
        ctx.release().await;
        return;
    }
    if !ask(ctx, Nebula_PG_Sc03b_T1).await {
        say(ctx, Nebula_PG_Sc03b_T2).await;
        ctx.release().await;
        return;
    }
    let team = if ctx.flag(FLAG_SIL_LIBERATION_JULIEN).await {
        TRAINER_SIL_JULIEN_10
    } else {
        TRAINER_SIL_JULIEN_9
    };
    if battle_lost(ctx, team, Nebula_PG_Sc03b_T3).await {
        return;
    }
    if gift(
        ctx,
        FLAG_SIL_ITEM_PG_SALAMENCITE,
        ITEM_SIL_SALAMENCITE,
        1,
        Obtain::Message,
    )
    .await
    {
        say(ctx, Nebula_PG_Sc03b_T5).await;
    }
    ctx.set_flag(FLAG_SIL_DAILY_JULIEN, true).await;
    ctx.release().await;
}

/// `Nebula_PG_Sc03c`, Julien gives the sun back (from 130, after a rematch won, once): Groudon's
/// distant cry, the Red Orb, `FLAG_SIL_LIBERATION_JULIEN`. This is the clear point of Entei,
/// Regirock and Groudon (the plan's table): their hide flags are cleared here, once.
async fn nebula_pg_sc03c(ctx: &Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(ctx, Nebula_PG_Sc03c_T1).await;
    ctx.fade(Fade::ToBlack).await;
    play_cry(ctx).await;
    ctx.delay(40).await;
    ctx.fade(Fade::FromBlack).await;
    say(ctx, Nebula_PG_Sc03c_T2).await;
    give_item(ctx, ITEM_SIL_ORBE_ROUGE, 1).await;
    say(ctx, Nebula_PG_Sc03c_T3).await;
    ctx.set_flag(FLAG_SIL_LIBERATION_JULIEN, true).await;
    for f in [
        FLAG_SIL_HIDE_ENTEI,
        FLAG_SIL_HIDE_REGIROCK,
        FLAG_SIL_HIDE_GROUDON,
    ] {
        ctx.set_flag(f, false).await;
    }
    ctx.release().await;
}

/// Julien's workbench (the lamp post (21, 8)). Silas, pardoned, works there ("Après la Traque",
/// `Nebula_PG_Npc_Silas`); Nébula has no Silas object, so the workbench says his line. Silent
/// otherwise (the source gives the workbench no text).
pub async fn Nebula_PG_Etabli(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_PARDON_SILAS).await {
        msgbox(&ctx, Nebula_PG_Npc_Silas, MsgBox::Sign).await;
    }
}

// ---- Act I: Thomas at the lighthouse ----

/// `Reverse_City_PG_Sc04`, the reflection that waits (the coord (12, 8) at 122, the only cell into
/// the top room): Thomas's white light, the three hiding places (the Mallette marks them),
/// Thomas gone (`FLAG_SIL_REVENGE_ALLIE`), 123.
pub async fn ReverseCity_Phare_PG_Sc04(ctx: Ctx) {
    if story(&ctx).await != 122 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Reverse_City_PG_Sc04_T1).await;
    ctx.turn(LOCALID_PHARE_THOMAS, Dir::Down).await;
    say(&ctx, Reverse_City_PG_Sc04_T2).await;
    say(&ctx, Reverse_City_PG_Sc04_T3).await;
    say(&ctx, Reverse_City_PG_Sc04_T4).await;
    say(&ctx, Reverse_City_PG_Sc04_T5).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_PHARE_THOMAS).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_REVENGE_ALLIE, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_THOMAS_PHARE, true).await;
    set_story(&ctx, 123).await;
    ctx.release_all().await;
}

/// Thomas at the top (shown at 122 only): `Sc04`'s coord is the only way in, so he is talked to
/// only if the scene has not played.
pub async fn ReverseCity_Phare_PG_Thomas(ctx: Ctx) {
    ReverseCity_Phare_PG_Sc04(ctx).await;
}

// ---- Act II: the end of a Vestige ----

/// `PG_VestigeFin`: one more Vestige found (123 → 124 → 125 → 126); the third one's alarm (the
/// Dome cracks over Nébula, a distant cry, Mathieu's call, `FLAG_SIL_BRECHE_ALERTE`) opens Act
/// III.
pub(super) async fn vestige_fin(ctx: &Ctx) {
    let s = story(ctx).await;
    if s >= 126 {
        return;
    }
    set_story(ctx, s + 1).await;
    if s + 1 == 126 {
        ctx.delay(60).await;
        screen_shake(ctx, 4, 90).await;
        play_cry(ctx).await;
        play_se(ctx, "SE_MALLETTE_APPEL").await;
        mallette_call(ctx, "MATHIEU").await;
        say(ctx, PG_VestigeFin_T1).await;
        ctx.set_flag(FLAG_SIL_BRECHE_ALERTE, true).await;
    }
}

/// One more pardon (`VAR_SIL_PARDONS`, 0–3, read by `Sc20`).
async fn pardon(ctx: &Ctx, flag: FlagId) {
    ctx.set_flag(flag, true).await;
    let n = ctx.var(VAR_SIL_PARDONS).await;
    ctx.set_var(VAR_SIL_PARDONS, n + 1).await;
}

// ---- Act II: Kimiko in the Bois des Soupirs ----

/// The clearing's actors: Kimiko and her broken drone from 123 until her Vestige is done; Adrien
/// only inside `Sc05`.
pub async fn RouteS06_BoisSoupirs_OnTransition(ctx: Ctx) {
    let here = story(&ctx).await >= 123 && !ctx.flag(FLAG_SIL_VESTIGE_KIMIKO).await;
    ctx.set_flag(FLAG_SIL_HIDE_KIMIKO_BOIS, !here).await;
    ctx.set_flag(FLAG_SIL_HIDE_BOIS_SOUPIRS_DRONE, !here).await;
    ctx.set_flag(FLAG_SIL_HIDE_BOIS_SOUPIRS_ADRIEN, true).await;
}

const ADRIEN_COMES: &[Step] = &[walk(Dir::Up), walk(Dir::Up), walk(Dir::Up), face(Dir::Up)];

/// `Route_S06_PG_Sc05`, Kimiko in the Bois des Soupirs (the secret path's end (12, 19), from 123
/// until `FLAG_SIL_VESTIGE_KIMIKO`): the live without viewers, the last show
/// (`TRAINER_SIL_KIMIKO_9`), Adrien, the pardon or the exile (the Soothe Bell either way), the
/// Steelixite, `PG_VestigeFin`.
///
/// Deviation: the player comes from the north, so Kimiko (12, 21) turns up rather than down.
pub async fn RouteS06_BoisSoupirs_PG_Sc05(ctx: Ctx) {
    if story(&ctx).await < 123 || ctx.flag(FLAG_SIL_VESTIGE_KIMIKO).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Route_S06_PG_Sc05_T1).await;
    ctx.turn(LOCALID_BOIS_SOUPIRS_KIMIKO, Dir::Up).await;
    say(&ctx, Route_S06_PG_Sc05_T2).await;
    if battle_lost(&ctx, TRAINER_SIL_KIMIKO_9, Route_S06_PG_Sc05_T3).await {
        return;
    }
    show(&ctx, LOCALID_BOIS_SOUPIRS_ADRIEN, FLAG_SIL_HIDE_BOIS_SOUPIRS_ADRIEN).await;
    move_wait(&ctx, LOCALID_BOIS_SOUPIRS_ADRIEN, ADRIEN_COMES).await;
    say(&ctx, Route_S06_PG_Sc05_T4).await;
    say(&ctx, Route_S06_PG_Sc05_T5).await;
    if ask(&ctx, Route_S06_PG_Sc05_T6).await {
        say(&ctx, Route_S06_PG_Sc05_T7).await;
        give_item(&ctx, ITEM_SOOTHE_BELL, 1).await;
        pardon(&ctx, FLAG_SIL_PARDON_KIMIKO).await;
    } else {
        say(&ctx, Route_S06_PG_Sc05_T8).await;
        ctx.fade(Fade::ToBlack).await;
        hide(&ctx, LOCALID_BOIS_SOUPIRS_KIMIKO).await;
        ctx.fade(Fade::FromBlack).await;
        say(&ctx, Route_S06_PG_Sc05_T9).await;
        give_item(&ctx, ITEM_SOOTHE_BELL, 1).await;
    }
    give_item(&ctx, ITEM_SIL_STEELIXITE, 1).await;
    say(&ctx, Route_S06_PG_Sc05_T10).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_BOIS_SOUPIRS_KIMIKO).await;
    hide(&ctx, LOCALID_BOIS_SOUPIRS_ADRIEN).await;
    // The drone goes with her (she repairs it; exiled, it floats at the No Man's Land's Oasis).
    hide(&ctx, LOCALID_BOIS_SOUPIRS_DRONE).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_VESTIGE_KIMIKO, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_KIMIKO_BOIS, true).await;
    vestige_fin(&ctx).await;
    ctx.release_all().await;
}

/// `PG_Lib_Kimiko`, the feather's stump (15, 21): Ho-Oh's line while Ho-Oh stands in the clearing
/// (shown by `Sommet_Apex_PG_Sc19` when Kimiko's Vestige is done); silent otherwise.
pub async fn RouteS06_BoisSoupirs_PG_Lib(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_HIDE_HO_OH).await {
        msgbox(&ctx, Route_S06_PG_Lib_T1, MsgBox::Sign).await;
    }
}

/// Kimiko's broken drone: shown only with her, inside the clearing that `Sc05`'s coord opens, so
/// the scene always plays first and takes it away. Nothing to say (the source gives it no line).
pub async fn RouteS06_BoisSoupirs_EventScript_Drone(_ctx: Ctx) {}

/// Ho-Oh in the clearing (`PG_Legendaire`, level 78, its line `Route_S06_PG_Lib_T1`).
pub async fn RouteS06_BoisSoupirs_EventScript_HoOh(ctx: Ctx) {
    legendaire(
        &ctx,
        Route_S06_PG_Lib_T1,
        SPECIES_HO_OH,
        78,
        LOCALID_BOIS_SOUPIRS_HO_OH,
    )
    .await;
}

// ---- Act II: Arthur on the wreck ----

/// Arthur on the upper deck from 123 until his Vestige is done.
pub async fn RouteS05_Epave_OnTransition(ctx: Ctx) {
    let here = story(&ctx).await >= 123 && !ctx.flag(FLAG_SIL_VESTIGE_ARTHUR).await;
    ctx.set_flag(FLAG_SIL_HIDE_ARTHUR_EPAVE, !here).await;
}

/// `Route_S05_PG_Sc06`, Arthur and the wreck (talk to him, from 123 until
/// `FLAG_SIL_VESTIGE_ARTHUR`): the sails of Cocket reports, a last manoeuvre
/// (`TRAINER_SIL_ARTHUR_9`), the pardon or the exile, the compass (given if he got it back in
/// Ch10, else left with the player), `PG_VestigeFin`.
pub async fn RouteS05_Epave_PG_Sc06(ctx: Ctx) {
    if story(&ctx).await < 123 || ctx.flag(FLAG_SIL_VESTIGE_ARTHUR).await {
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Route_S05_PG_Sc06_T1).await;
    say(&ctx, Route_S05_PG_Sc06_T2).await;
    if battle_lost(&ctx, TRAINER_SIL_ARTHUR_9, Route_S05_PG_Sc06_T3).await {
        return;
    }
    let boussole = ctx.flag(FLAG_SIL_ARTHUR_BOUSSOLE_RENDUE).await;
    if ask(&ctx, Route_S05_PG_Sc06_T4).await {
        say(&ctx, Route_S05_PG_Sc06_T5).await;
        pardon(&ctx, FLAG_SIL_PARDON_ARTHUR).await;
    } else {
        say(&ctx, Route_S05_PG_Sc06_T6).await;
        if boussole {
            say(&ctx, Route_S05_PG_Sc06_T8).await;
            remove_item(&ctx, ITEM_SIL_BATEAU_ORIGAMI, 1).await;
        }
    }
    if boussole {
        give_item(&ctx, ITEM_SIL_BOUSSOLE_ARTHUR, 1).await;
        say(&ctx, Route_S05_PG_Sc06_T7).await;
    } else {
        say(&ctx, Route_S05_PG_Sc06_T9).await;
    }
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_EPAVE_ARTHUR).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_VESTIGE_ARTHUR, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_ARTHUR_EPAVE, true).await;
    vestige_fin(&ctx).await;
    ctx.release().await;
}

// ---- Act II: Silas at the Point Zéro ----

/// Silas walks away (towards the player, who stands on the room's only way in).
const SILAS_LEAVES: &[Step] = &[walk(Dir::Down), walk(Dir::Down), walk(Dir::Down)];

/// `Owlys_PG_Sc07`, Silas at the Point Zéro (the coord (33, 9), from 123 until
/// `FLAG_SIL_VESTIGE_SILAS`): 4 degrees and nothing changes, neutral violence
/// (`TRAINER_SIL_SILAS_9`), perfection was an error, the pardon or the exile, `PG_VestigeFin`.
///
/// Deviation: the player stands on (33, 9), the room's only entrance, so Silas walks three cells
/// down (not four) and vanishes in front of the player.
pub async fn Owlys_GrotteEst_PG_Sc07(ctx: Ctx) {
    if story(&ctx).await < 123 || ctx.flag(FLAG_SIL_VESTIGE_SILAS).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Owlys_PG_Sc07_T1).await;
    ctx.turn(LOCALID_GROTTE_EST_SILAS, Dir::Down).await;
    say(&ctx, Owlys_PG_Sc07_T2).await;
    if battle_lost(&ctx, TRAINER_SIL_SILAS_9, Owlys_PG_Sc07_T3).await {
        return;
    }
    if ask(&ctx, Owlys_PG_Sc07_T4).await {
        say(&ctx, Owlys_PG_Sc07_T5).await;
        pardon(&ctx, FLAG_SIL_PARDON_SILAS).await;
    } else {
        say(&ctx, Owlys_PG_Sc07_T6).await;
    }
    move_wait(&ctx, LOCALID_GROTTE_EST_SILAS, SILAS_LEAVES).await;
    hide(&ctx, LOCALID_GROTTE_EST_SILAS).await;
    ctx.set_flag(FLAG_SIL_VESTIGE_SILAS, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_SILAS_GROTTE, true).await;
    vestige_fin(&ctx).await;
    ctx.release_all().await;
}

/// Silas's machine (33, 4): nothing before his Vestige; then, once, the Debug Key and the
/// Kangaskhanite (`FLAG_SIL_CLE_DEBUG`); from 130, `PG_Lib_Silas` (text only: Mew is in the room).
pub async fn Owlys_GrotteEst_EventScript_Machine(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_VESTIGE_SILAS).await {
        return;
    }
    if !ctx.flag(FLAG_SIL_CLE_DEBUG).await {
        ctx.lock().await;
        say(&ctx, Owlys_PG_Sc07_T7).await;
        add_item(&ctx, ITEM_SIL_CLE_DEBUG, 1).await;
        add_item(&ctx, ITEM_SIL_KANGASKHANITE, 1).await;
        ctx.set_flag(FLAG_SIL_CLE_DEBUG, true).await;
        ctx.release().await;
    } else if story(&ctx).await >= 130 {
        msgbox(&ctx, Owlys_PG_Lib_T1, MsgBox::Sign).await;
    }
}

/// Mew at the Point Zéro (`PG_Legendaire`, level 75, its line `Owlys_PG_Lib_T1`).
pub async fn Owlys_GrotteEst_EventScript_Mew(ctx: Ctx) {
    legendaire(&ctx, Owlys_PG_Lib_T1, SPECIES_MEW, 75, LOCALID_GROTTE_EST_MEW).await;
}

// ---- Act II: Silas's mail (the bedroom PC) ----

/// `Brico_Bourg_PG_Sc09`, Silas's mail: the bedroom PC opens on it once Silas's Vestige is done
/// (`FLAG_SIL_MAIL_SILAS` not set), before its usual line. The egg is the source's fallback
/// (`give_egg`: a level-5 Porygon). Whether the mail played.
pub(super) async fn pg_mail_silas(ctx: &Ctx) -> bool {
    if !ctx.flag(FLAG_SIL_VESTIGE_SILAS).await || ctx.flag(FLAG_SIL_MAIL_SILAS).await {
        return false;
    }
    ctx.lock().await;
    say(ctx, Brico_Bourg_PG_Sc09_T1).await;
    if ctx.flag(FLAG_SIL_PARDON_SILAS).await {
        say(ctx, Brico_Bourg_PG_Sc09_T2).await;
    } else {
        say(ctx, Brico_Bourg_PG_Sc09_T3).await;
    }
    give_egg(ctx, SPECIES_PORYGON).await;
    say(ctx, Brico_Bourg_PG_Sc09_T4).await;
    if ctx.flag(FLAG_SIL_PORYGON_LIBERE).await {
        say(ctx, Brico_Bourg_PG_Sc09_T5).await;
    }
    ctx.set_flag(FLAG_SIL_MAIL_SILAS, true).await;
    ctx.release().await;
    true
}

// ---- the final scene (Brico-Bourg) ----

const MAMAN_OUT: &[Step] = &[walk(Dir::Down), Step::FacePlayer];
/// Where `Sc20` wakes the player the next morning (the source's (4, 3)).
const MATIN_2F: (i16, i16) = (4, 3);

/// `Brico_Bourg_PG_Sc20`, the last ceiling (the coords (4, 15), (6, 15), (5, 16) before the house
/// at 130): Maman on the doorstep, her bonus line after three pardons, the black screen, the
/// National Dex, 131, the morning after in the bedroom.
///
/// Maman stands on the house's door warp (5, 14): she is hidden again at the end (the plan's
/// review), or the house would stay shut.
pub async fn BricoBourg_PG_Sc20(ctx: Ctx) {
    if story(&ctx).await != 130 {
        return;
    }
    ctx.lock_all().await;
    // fallback: set_time_of_day(DUSK) (§9): no time-of-day palette.
    fallback(&ctx, "set_time_of_day").await;
    show(&ctx, LOCALID_BRICO_MAMAN, FLAG_SIL_HIDE_BRICO_MAMAN).await;
    move_wait(&ctx, LOCALID_BRICO_MAMAN, MAMAN_OUT).await;
    say(&ctx, Brico_Bourg_PG_Sc20_T1).await;
    if ctx.var(VAR_SIL_PARDONS).await == 3 {
        say(&ctx, Brico_Bourg_PG_Sc20_T2).await;
    }
    // fallback: MUS_SIL_FIN (play_music).
    fallback(&ctx, "play_music").await;
    // The two last lines on black (the source's fade(Out) then its messages).
    ctx.text_on_black(true).await;
    say(&ctx, Brico_Bourg_PG_Sc20_T3).await;
    enable_national_dex(&ctx).await;
    say(&ctx, Brico_Bourg_PG_Sc20_T4).await;
    ctx.close_message().await;
    hide(&ctx, LOCALID_BRICO_MAMAN).await;
    ctx.text_on_black(false).await;
    ctx.fade(Fade::ToBlack).await;
    set_story(&ctx, 131).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_BRICO_BOURG_PLAYER_HOUSE).await;
    warp(&ctx, MAP_BRICO_BOURG_PLAYER_HOUSE_2F, MATIN_2F.0, MATIN_2F.1).await;
}

// ---- the Council's games (Act IV) ----

/// `Manga_Square_PG_Sc21`, the Friends' Tournament (talking to Adrien; 122–126 or from 130; daily,
/// `FLAG_SIL_DAILY_TOURNOI`): Mathieu, Gwada, Adrien in a row, no heal between; the Golden Feather
/// every win, the Altarianite the first (`FLAG_SIL_TOURNOI_AMIS`); the party healed after.
///
/// Adrien asks here, then the source's `warp(MAP_MANGA_SQUARE_DOJO, 6, 10)`: a map load ends a
/// script (as Gen3's `warp`), so the tournament is marked under way (`FLAG_SIL_TOURNOI_EN_COURS`)
/// and the Dojo's on-frame script ([`MangaSquare_Dojo_PG_Sc21`]) fights the three battles on the
/// central tatami (G-50, the author's ruling: the tournament happens in the Dojo).
pub async fn MangaSquare_Bibliotheque_PG_Sc21(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    let s = story(&ctx).await;
    if (127..=129).contains(&s) {
        say(&ctx, Manga_Square_PG_Sc21_T0).await;
        ctx.release().await;
        return;
    }
    if s < 122 {
        // Before Julien's return (122) the tournament is not open: Adrien only turns.
        ctx.release().await;
        return;
    }
    if ctx.flag(FLAG_SIL_DAILY_TOURNOI).await {
        say(&ctx, Manga_Square_PG_Sc21_T9).await;
        ctx.release().await;
        return;
    }
    if !ask(&ctx, Manga_Square_PG_Sc21_T1).await {
        ctx.release().await;
        return;
    }
    ctx.set_flag(FLAG_SIL_TOURNOI_EN_COURS, true).await;
    ctx.fade(Fade::ToBlack).await;
    let (x, y) = TATAMI;
    warp(&ctx, MAP_MANGA_SQUARE_DOJO, x, y).await;
}

/// The player's cell on the Dojo's central tatami (the source's warp target).
const TATAMI: (i16, i16) = (6, 10);
/// The beaten friends go to sit on the bench to the left (row 9): Mathieu to (4, 9), Gwada to
/// (5, 9), each from the challenger's cell (7, 10).
const BANC_MATHIEU: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    face(Dir::Down),
];
const BANC_GWADA: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Left),
    walk(Dir::Left),
    face(Dir::Down),
];
/// The next friend steps up to the challenger's cell (7, 10).
const AU_TATAMI_1: &[Step] = &[walk(Dir::Left)];
const AU_TATAMI_2: &[Step] = &[walk(Dir::Left), walk(Dir::Left)];
const FACE_EST: &[Step] = &[face(Dir::Right)];

/// The Friends' Tournament in the Dojo (on-frame, `VAR_TEMP_1` 0, every load): nothing unless
/// Adrien sent the player here (`FLAG_SIL_TOURNOI_EN_COURS`, cleared at once, so a loss's whiteout
/// does not replay it). Tak referees (his one line); Mathieu, Gwada and Adrien wait one behind the
/// other and fight in turn, no heal between; each beaten friend sits on the bench to the left.
/// The friends stay on the tatami until the player leaves (their hide flag is set again on load).
pub async fn MangaSquare_Dojo_PG_Sc21(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if !ctx.flag(FLAG_SIL_TOURNOI_EN_COURS).await {
        return;
    }
    ctx.set_flag(FLAG_SIL_TOURNOI_EN_COURS, false).await;
    ctx.lock_all().await;
    for obj in [
        LOCALID_DOJO_PG_TAK,
        LOCALID_DOJO_PG_MATHIEU,
        LOCALID_DOJO_PG_GWADA,
        LOCALID_DOJO_PG_ADRIEN,
    ] {
        ctx.add_object(obj).await;
    }
    move_wait(&ctx, PLAYER, FACE_EST).await;
    say(&ctx, Manga_Square_PG_Tak_Arbitre).await;
    say(&ctx, Manga_Square_PG_Sc21_T2).await;
    if battle_lost(&ctx, TRAINER_SIL_MATHIEU_9, Manga_Square_PG_Sc21_T3).await {
        return;
    }
    move_wait(&ctx, LOCALID_DOJO_PG_MATHIEU, BANC_MATHIEU).await;
    move_wait(&ctx, LOCALID_DOJO_PG_GWADA, AU_TATAMI_1).await;
    say(&ctx, Manga_Square_PG_Sc21_T4).await;
    if battle_lost(&ctx, TRAINER_SIL_GWADA_9, Manga_Square_PG_Sc21_T5).await {
        return;
    }
    move_wait(&ctx, LOCALID_DOJO_PG_GWADA, BANC_GWADA).await;
    move_wait(&ctx, LOCALID_DOJO_PG_ADRIEN, AU_TATAMI_2).await;
    say(&ctx, Manga_Square_PG_Sc21_T6).await;
    if battle_lost(&ctx, TRAINER_SIL_ADRIEN_9, Manga_Square_PG_Sc21_T7).await {
        return;
    }
    add_item(&ctx, ITEM_SIL_PLUME_OR, 1).await;
    msgbox(&ctx, Manga_Square_PG_Sc21_T8, MsgBox::Default).await;
    if !ctx.flag(FLAG_SIL_TOURNOI_AMIS).await {
        give_item(&ctx, ITEM_SIL_ALTARIANITE, 1).await;
        say(&ctx, Manga_Square_PG_Sc21_T10).await;
    }
    ctx.heal_party().await;
    ctx.set_flag(FLAG_SIL_TOURNOI_AMIS, true).await;
    ctx.set_flag(FLAG_SIL_DAILY_TOURNOI, true).await;
    ctx.release_all().await;
}

/// `Manga_Square_PG_Sc30`, the Space Ticket (talking to Gwada with the eight fragments, once:
/// `FLAG_SIL_TICKET_ESPACE`): the elevator's ROOF button. Otherwise Gwada only turns (the source
/// gives him no other line here).
pub async fn MangaSquare_Bibliotheque_PG_Sc30(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.var(VAR_SIL_FRAGMENTS).await >= 8 && !ctx.flag(FLAG_SIL_TICKET_ESPACE).await {
        say(&ctx, Manga_Square_PG_Sc30_T1).await;
        give_item(&ctx, ITEM_SIL_TICKET_ESPACE, 1).await;
        ctx.set_flag(FLAG_SIL_TICKET_ESPACE, true).await;
        say(&ctx, Manga_Square_PG_Sc30_T2).await;
    }
    ctx.release().await;
}
