//! Chapter 8, the gym: the Donjon de la Coordination (`Evalon_Citadelle`, `_2F`):
//! `contenu-jeu/chapitres/08-evalon.md` §3 Sc08 (the entry) and Sc09 (Loulpix and Doris: the
//! badge, the Sonde de Pression, the Cœur d'Évalon) and §4 (the five rooms: the trapped slabs,
//! the golems' bridge, the crypt, the Sphinge, the chests). `VAR_SIL_STORY` 84 → 85.
//!
//! **Temp vars:** `VAR_TEMP_1` is the entry's `on_frame` trigger (Sc08 sets it on every path);
//! `VAR_TEMP_3` the traps' and the crypt's coords (0: they fire; no script writes it, "SIL4 Ch8
//! maps as built"); `VAR_TEMP_5`, `_6` the golems' cells and `VAR_TEMP_4` the first push's line
//! (`push_object`).

use super::ch04_port::{Pushable, push_object};
use super::ch08_evalon::lost;
use super::ch08_galeries::step_back;
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Evalon_Citadelle", "Evalon_Citadelle_2F"];

/// SilEvalonCitadelle's lowered drawbridge `pont` (541, 542) over the chasm (5, 32–33), and the
/// Sphinge's open door `porte_ouverte` (545, 546) in the wall (5, 9–10); all walkable.
const PONT: [(i16, i16, u16); 2] = [(5, 32, 541), (5, 33, 542)];
const PORTE_SPHINGE: [(i16, i16, u16); 2] = [(5, 9, 545), (5, 10, 546)];

async fn paint(ctx: &Ctx, cells: [(i16, i16, u16); 2]) {
    for (x, y, m) in cells {
        ctx.set_metatile(x, y, m, false).await;
    }
}

// ---- map scripts ----

/// The bridge down from `FLAG_SIL_EVALON_GYM_PONT`, the Sphinge's door open from
/// `FLAG_SIL_EVALON_GYM_SPHINGE_PASSE`.
pub async fn Evalon_Citadelle_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_GYM_PONT).await {
        paint(&ctx, PONT).await;
    }
    if ctx.flag(FLAG_SIL_EVALON_GYM_SPHINGE_PASSE).await {
        paint(&ctx, PORTE_SPHINGE).await;
    }
}

/// The Sphinge one cell aside (6, 11) once passed; the golems on their plates once the bridge
/// is down (Gen3 `setobjectxyperm`).
pub async fn Evalon_Citadelle_OnTransition(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_GYM_SPHINGE_PASSE).await {
        ctx.set_object_template_pos(LOCALID_GYM8_SPHINGE, 6, 11)
            .await;
    }
    if ctx.flag(FLAG_SIL_EVALON_GYM_PONT).await {
        for (obj, _, plate) in GOLEMS {
            ctx.set_object_template_pos(obj, plate.0, plate.1).await;
        }
    }
}

/// Evalon_Sc08, the dungeon's innkeeper greets the player at the door: the `on_frame` entry
/// (`VAR_TEMP_1` 0) at `VAR_SIL_STORY` 84, when the player arrives on the door's mats (y 54),
/// not when a trap sends them back to (5, 52).
pub async fn Evalon_Citadelle_Sc08(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if story(&ctx).await != 84 || ctx.player_pos().await.1 != 54 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Evalon_Sc08_T1).await;
    ctx.release_all().await;
}

pub async fn Evalon_Citadelle_EventScript_Aubergiste(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc08_T1, MsgBox::Npc).await;
}

// ---- the rooms' signs (the MJ's voice) ----

pub async fn Evalon_Citadelle_EventScript_Salle1(ctx: Ctx) {
    msgbox(&ctx, Evalon_Gym_Salle1_Sign, MsgBox::Sign).await;
}

pub async fn Evalon_Citadelle_EventScript_Salle2(ctx: Ctx) {
    msgbox(&ctx, Evalon_Gym_Salle2_Sign, MsgBox::Sign).await;
}

pub async fn Evalon_Citadelle_EventScript_Salle3(ctx: Ctx) {
    msgbox(&ctx, Evalon_Gym_Salle3_Sign, MsgBox::Sign).await;
}

/// Room 4's sign: the source writes none; the Sphinge's challenge (author question).
pub async fn Evalon_Citadelle_EventScript_Salle4(ctx: Ctx) {
    msgbox(&ctx, Evalon_Gym_Sphinge_T1, MsgBox::Sign).await;
}

pub async fn Evalon_Citadelle_EventScript_Salle5(ctx: Ctx) {
    msgbox(&ctx, Evalon_Gym_Salle5_Sign, MsgBox::Sign).await;
}

// ---- room 1: the slabs ----

/// A skull slab: the trap, and back to the room's start (5, 52).
///
/// fallback: Gen3's cracked floor (566 is `MB_CRACKED_FLOOR`, not in the engine): the skull
/// slabs are plain tiles with coord traps ("SIL4 Ch8 maps as built").
pub async fn Evalon_Citadelle_EventScript_Piege(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Evalon_Gym_Piege).await;
    warp(&ctx, MAP_EVALON_CITADELLE, 5, 52).await;
}

/// BARDE TRISTAN, on a safe slab (6, 45), sight 1 down.
pub async fn Evalon_Citadelle_EventScript_Tristan(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM8_TRISTAN,
        TRAINER_SIL_EVALON_GYM_TRISTAN,
        Evalon_Tristan_Intro,
        Evalon_Tristan_Defaite,
    )
    .await
    {
        post_battle(&ctx, Evalon_Tristan_Defaite).await;
    }
}

// ---- room 2: the golems' bridge ----

/// The two iron golems (Champ Magnétique), each pushed up twice from (3, 36) and (7, 36) onto
/// its pressure plate (3, 34) and (7, 34): the object, its temp var, its home, its plate.
const GOLEMS: [(u8, VarId, (i16, i16)); 2] = [
    (LOCALID_GYM8_GOLEM_GAUCHE, VAR_TEMP_5, (3, 34)),
    (LOCALID_GYM8_GOLEM_DROITE, VAR_TEMP_6, (7, 34)),
];
const GOLEMS_PUSH: &[Pushable] = &[
    (LOCALID_GYM8_GOLEM_GAUCHE, VAR_TEMP_5, (3, 36)),
    (LOCALID_GYM8_GOLEM_DROITE, VAR_TEMP_6, (7, 36)),
];

/// A golem moves along its own column (3 or 7) inside room 2's floor (y 34–37), so it can never
/// stand on the centre column (5) that leads to the bridge.
fn golem_free((x, y): (i16, i16)) -> bool {
    (x == 3 || x == 7) && (34..=37).contains(&y)
}

/// An iron golem: pushed (Chapter 4's `push_object`); when both stand on their plates, the
/// bridge comes down (`FLAG_SIL_EVALON_GYM_PONT`) with no line in the source (its sound).
pub async fn Evalon_Citadelle_EventScript_Golem(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_GYM_PONT).await {
        return;
    }
    push_object(&ctx, GOLEMS_PUSH, golem_free).await;
    for (_, var, (x, y)) in GOLEMS {
        if ctx.var(var).await != (y * 64 + x + 1) as u16 {
            return;
        }
    }
    ctx.lock_all().await;
    play_se(&ctx, "SE_M_STRENGTH").await;
    paint(&ctx, PONT).await;
    ctx.redraw_map().await;
    ctx.set_flag(FLAG_SIL_EVALON_GYM_PONT, true).await;
    ctx.release_all().await;
}

/// VOLEUR RENARD, behind the pillar (3, 30), sight 3 right.
pub async fn Evalon_Citadelle_EventScript_Renard(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM8_RENARD,
        TRAINER_SIL_EVALON_GYM_RENARD,
        Evalon_Renard_Intro,
        Evalon_Renard_Defaite,
    )
    .await
    {
        post_battle(&ctx, Evalon_Renard_Defaite).await;
    }
}

// ---- room 3: the crypt ----

/// The crypt's door (5, 28), its only way in.
///
/// fallback: per-area darkness (as the Galeries' dark gallery): without the Surcharge Optique
/// (`FLAG_SIL_MODULE_FLASH`), room 3's sign and one step back (the source's guard who advises
/// coming back); with it the way is open (reported `dark_map`).
pub async fn Evalon_Citadelle_EventScript_Crypte(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_MODULE_FLASH).await {
        fallback(&ctx, "dark_map").await;
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Evalon_Gym_Salle3_Sign).await;
    let back = step_back(ctx.player_facing().await);
    move_wait(&ctx, PLAYER, back).await;
    ctx.release_all().await;
}

/// The crypt's guard by its door (4, 29): room 3's sign (author question: no line of his own).
pub async fn Evalon_Citadelle_EventScript_GardeCrypte(ctx: Ctx) {
    msgbox(&ctx, Evalon_Gym_Salle3_Sign, MsgBox::Npc).await;
}

/// MAGE NOIRE MORGANE (5, 21), sight 1 down (she meets the player where the room widens, never
/// on the 1-wide (5, 23)): a `double: true` entry, a double battle.
pub async fn Evalon_Citadelle_EventScript_Morgane(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM8_MORGANE,
        TRAINER_SIL_EVALON_GYM_MORGANE,
        Evalon_Morgane_Intro,
        Evalon_Morgane_Defaite,
    )
    .await
    {
        post_battle(&ctx, Evalon_Morgane_Defaite).await;
    }
}

// ---- room 4: the Sphinge ----

const SPHINGE_ASIDE: &[Step] = &[walk(Dir::Right), face(Dir::Left)];

/// The Sphinge (5, 11) on the door's threshold: three riddles; each right answer is a step
/// nearer the door, the third (`Evalon_Gym_Sphinge_Ciel`) and she steps aside. A wrong answer is
/// the battle (once); after it she lets the player pass too. The door opens
/// (`FLAG_SIL_EVALON_GYM_SPHINGE_PASSE`) with no line in the source.
pub async fn Evalon_Citadelle_EventScript_Sphinge(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_EVALON_GYM_SPHINGE_PASSE).await {
        let text = if defeated(&ctx, TRAINER_SIL_EVALON_GYM_SPHINGE).await {
            Evalon_Sphinge_Defaite
        } else {
            Evalon_Gym_Sphinge_Ciel
        };
        say(&ctx, text).await;
        ctx.release().await;
        return;
    }
    say(&ctx, Evalon_Gym_Sphinge_T1).await;
    // fallback: multichoice (§9): the source's own fallback, a yes/no on the right answer
    // ("Est-ce TROU?").
    fallback(&ctx, "multichoice").await;
    let mut right = true;
    for (q, ok) in [
        (Evalon_Gym_Sphinge_Q1, Evalon_Gym_Sphinge_Bon),
        (Evalon_Gym_Sphinge_Q2, Evalon_Gym_Sphinge_Bon),
        (Evalon_Gym_Sphinge_Q3, Evalon_Gym_Sphinge_Ciel),
    ] {
        if !ask(&ctx, q).await {
            right = false;
            break;
        }
        say(&ctx, ok).await;
    }
    if !right {
        say(&ctx, Evalon_Gym_Sphinge_Faux).await;
        if !defeated(&ctx, TRAINER_SIL_EVALON_GYM_SPHINGE).await
            && lost(
                battle_trainer(&ctx, TRAINER_SIL_EVALON_GYM_SPHINGE, Evalon_Sphinge_Defaite).await,
            )
        {
            return;
        }
    }
    move_wait(&ctx, LOCALID_GYM8_SPHINGE, SPHINGE_ASIDE).await;
    ctx.set_object_template_pos(LOCALID_GYM8_SPHINGE, 6, 11)
        .await;
    play_se(&ctx, "SE_DOOR").await;
    paint(&ctx, PORTE_SPHINGE).await;
    ctx.redraw_map().await;
    ctx.set_flag(FLAG_SIL_EVALON_GYM_SPHINGE_PASSE, true).await;
    ctx.release().await;
}

// ---- room 5: the chests ----

/// The left chest (2, 5), which purrs: an Electrode (`battle_wild(SPECIES_ELECTRODE, 47)`);
/// then the chest is gone (`FLAG_SIL_HIDE_GYM8_COFFRE_GAUCHE`).
pub async fn Evalon_Citadelle_EventScript_CoffreGauche(ctx: Ctx) {
    ctx.lock().await;
    if ask(&ctx, Evalon_Gym_Coffre_Gauche).await {
        say(&ctx, Evalon_Gym_Coffre_Piege).await;
        if lost(battle_wild(&ctx, SPECIES_ELECTRODE, 47).await) {
            return;
        }
        hide(&ctx, LOCALID_GYM8_COFFRE_GAUCHE).await;
    }
    ctx.release().await;
}

/// The right chest (8, 5): the Orbe d'Éveil (`FLAG_SIL_ITEM_EVALON_ORBE`, its hide flag).
pub async fn Evalon_Citadelle_EventScript_CoffreDroite(ctx: Ctx) {
    msgbox(&ctx, Evalon_Gym_Coffre_Droite, MsgBox::Default).await;
    find_item(&ctx, ITEM_SIL_ORBE_EVEIL, 1).await;
}

/// CLERC ÉLISE (6, 3), sight 1 left, beside the stairs' only approach (5, 3), met there (never
/// drawn onto it): a `double: true` entry (a double battle).
pub async fn Evalon_Citadelle_EventScript_Elise(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM8_ELISE,
        TRAINER_SIL_EVALON_GYM_ELISE,
        Evalon_Elise_Intro,
        Evalon_Elise_Defaite,
    )
    .await
    {
        post_battle(&ctx, Evalon_Elise_Defaite).await;
    }
}

// ---- 2F: the throne room (Sc09) ----

/// Evalon_Sc09, Loulpix and Doris (talk to Loulpix, (5, 5), from (5, 6)) at `VAR_SIL_STORY` 84:
/// the battle (one `double: true` entry, a double battle), the Résonance Badge
/// (`FLAG_SIL_BADGE_08`), the Sonde de Pression (`FLAG_SIL_MODULE_PLONGEE`), the Cœur d'Évalon,
/// TM19 and TM23, the Épée de Légende from the Paladin rank (reputation 8), sync 80,
/// `LOG_08`, the friends gone (`FLAG_SIL_HIDE_EVALON_AMIS` and the Taverne's). Sets 85. Before
/// 84 the challenge waits; after, the defeat line.
pub async fn Evalon_Sc09(ctx: Ctx) {
    if super::ch12_revanches::ouvertes(&ctx).await {
        super::ch12_revanches::revanche_loulpix_doris(&ctx).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    let s = story(&ctx).await;
    if s != 84 {
        let text = if s > 84 {
            Evalon_LoulpixDoris_Defaite
        } else {
            Evalon_Sc09_T1
        };
        say(&ctx, text).await;
        ctx.release().await;
        return;
    }
    say(&ctx, Evalon_Sc09_T1).await;
    say(&ctx, Evalon_Sc09_T2).await;
    // fallback: play_music(MUS_VS_GYM_LEADER) (audio, M12).
    fallback(&ctx, "play_music").await;
    // Mawile Mega Evolves (the Mawilite, D-02).
    if lost(battle_trainer(&ctx, TRAINER_SIL_LOULPIX_DORIS, Evalon_LoulpixDoris_Defaite).await) {
        return;
    }
    say(&ctx, Evalon_Sc09_T3).await;
    say(&ctx, Evalon_Sc09_T4).await;
    // fallback: give_badge(8)'s fanfare (audio, M12): the flag and the badge line.
    fallback(&ctx, "play_fanfare").await;
    ctx.set_flag(FLAG_SIL_BADGE_08, true).await;
    say(&ctx, Evalon_Sc09_T5).await;
    say(&ctx, Evalon_Sc09_T6).await;
    ctx.set_flag(FLAG_SIL_MODULE_PLONGEE, true).await;
    say(&ctx, Evalon_Sc09_T7).await;
    give_item(&ctx, ITEM_SIL_COEUR_EVALON, 1).await;
    ctx.set_flag(FLAG_SIL_COEUR_EVALON, true).await;
    // fallback: the Cœur d'Évalon's confusion immunity (§9, the battle engine): the source's
    // own fallback, three Persim Berries more, and confusion as usual.
    fallback(&ctx, "coeur_evalon").await;
    give_item(&ctx, ITEM_PERSIM_BERRY, 3).await;
    give_item(&ctx, ITEM_TM19, 1).await;
    give_item(&ctx, ITEM_TM23, 1).await;
    epee(&ctx).await;
    ctx.set_var(VAR_SIL_SYNC, 80).await;
    play_se(&ctx, "SE_M_THUNDER_WAVE").await;
    say(&ctx, Evalon_Sc09_T8).await;
    ctx.set_flag(FLAG_SIL_LOG_08, true).await;
    say(&ctx, Evalon_Sc09_T9).await;
    for f in [
        FLAG_SIL_HIDE_EVALON_AMIS,
        FLAG_SIL_HIDE_EVALON_ADRIEN,
        FLAG_SIL_HIDE_EVALON_GWADA,
        FLAG_SIL_HIDE_EVALON_TAVERNE_MATHIEU,
        FLAG_SIL_HIDE_EVALON_TAVERNE_ADRIEN,
        FLAG_SIL_HIDE_EVALON_TAVERNE_GWADA,
    ] {
        ctx.set_flag(f, true).await;
    }
    set_story(&ctx, 85).await;
    ctx.release().await;
}

/// Doris's Épée de Légende: from the Paladin rank (reputation 8), once
/// (`FLAG_SIL_ITEM_EVALON_EPEE`); else come back later.
async fn epee(ctx: &Ctx) {
    if ctx.flag(FLAG_SIL_ITEM_EVALON_EPEE).await {
        return;
    }
    if ctx.var(VAR_SIL_EVALON_RENOM).await >= 8 {
        say(ctx, Evalon_Sc09_Epee_T1).await;
        gift(
            ctx,
            FLAG_SIL_ITEM_EVALON_EPEE,
            ITEM_SIL_EPEE_LEGENDE,
            1,
            Obtain::Message,
        )
        .await;
    } else {
        say(ctx, Evalon_Sc09_Epee_T0).await;
    }
}

/// Doris (6, 5): before the battle, Loulpix's table (Sc09); after it, the Épée de Légende test
/// again ("parler à DORIS rejoue ce test"), then her post-battle line.
pub async fn Evalon_Citadelle_2F_EventScript_Doris(ctx: Ctx) {
    if story(&ctx).await <= 84 {
        Evalon_Sc09(ctx).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_ITEM_EVALON_EPEE).await {
        say(&ctx, Evalon_Sc09_T3).await;
    } else {
        epee(&ctx).await;
    }
    ctx.release().await;
}
