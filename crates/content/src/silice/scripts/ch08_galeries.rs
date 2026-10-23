//! Chapter 8, the Galeries (`Evalon_Galeries`), the old mines under Évalon:
//! `contenu-jeu/chapitres/08-evalon.md` §2.8, §3 Sc06 (Q1's rock in the fountain's canal, Q2's
//! ore in the dark gallery) and §7 (the items). After the invasion, the breach where Gogo and
//! Romain broke in.
//!
//! **Temp vars:** `VAR_TEMP_3` is the dark gallery's coord trigger (0: it fires; no script
//! writes it, "SIL4 Ch8 maps as built").

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Evalon_Galeries"];

/// The breach (3, 5) into the collapsed cul-de-sac, cave floor (776) once open.
const BRECHE: (i16, i16) = (3, 5);
const METATILE_SOL: u16 = 776;

/// The breach open from `FLAG_SIL_EVALON_INVASION` (Sc07).
pub async fn Evalon_Galeries_OnLoad(ctx: Ctx) {
    repaint_if(&ctx, FLAG_SIL_EVALON_INVASION, BRECHE, METATILE_SOL).await;
}

/// Q1, the compressed-silica rock (25, 15) in the canal: its label, then, with the Scanner de
/// Faille (`FLAG_SIL_MODULE_ECLATE_ROC`), Chapter 7's scan (yes/no) and break. The rock's hide
/// flag is `FLAG_SIL_EVALON_Q1_ROCHER`, so it stays broken and the fairy's water runs.
pub async fn Evalon_Galeries_EventScript_Rocher(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Evalon_Q1_Rocher).await;
    if ctx.flag(FLAG_SIL_MODULE_ECLATE_ROC).await && ask(&ctx, Route_S07_Bloc_Scan).await {
        say(&ctx, Route_S07_Bloc_Casse).await;
        hide(&ctx, LOCALID_GALERIES_ROCHER).await;
        ctx.set_flag(FLAG_SIL_EVALON_Q1_ROCHER, true).await;
    }
    ctx.release().await;
}

/// The canal's mouth (26, 15), beside the rock: the rock's label while it stands (author
/// question: no line once the water runs).
pub async fn Evalon_Galeries_EventScript_Canal(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_EVALON_Q1_ROCHER).await {
        msgbox(&ctx, Evalon_Q1_Rocher, MsgBox::Sign).await;
    }
}

const BACK_DOWN: &[Step] = &[walk(Dir::Down)];
const BACK_UP: &[Step] = &[walk(Dir::Up)];
const BACK_LEFT: &[Step] = &[walk(Dir::Left)];
const BACK_RIGHT: &[Step] = &[walk(Dir::Right)];

/// One step back the way the player came (the opposite of `facing`).
pub(super) fn step_back(facing: Dir) -> &'static [Step] {
    match facing {
        Dir::Up => BACK_DOWN,
        Dir::Down => BACK_UP,
        Dir::Left => BACK_RIGHT,
        Dir::Right => BACK_LEFT,
    }
}

/// The dark gallery's neck (15, 5), its only way in.
///
/// fallback: per-area darkness (the header's `cave` byte darkens a whole map): the map is lit,
/// and without the Surcharge Optique (`FLAG_SIL_MODULE_FLASH`) the player is turned back with
/// Chapter 4's dark-glass line (`Route_S04_Mur_Sombre`; author question: no line of its own).
/// With the module the way is open (reported `dark_map`).
pub async fn Evalon_Galeries_EventScript_Obscure(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_MODULE_FLASH).await {
        fallback(&ctx, "dark_map").await;
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Route_S04_Mur_Sombre).await;
    let back = step_back(ctx.player_facing().await);
    move_wait(&ctx, PLAYER, back).await;
    ctx.release_all().await;
}

// ---- items ----

/// The Moon Stone by the minecart.
pub async fn Evalon_Galeries_EventScript_ItemPierreLune(ctx: Ctx) {
    find_item(&ctx, ITEM_MOON_STONE, 1).await;
}

/// Q2's Minerai Ferreux, at the dark gallery's end (a key item the Forge takes back).
pub async fn Evalon_Galeries_EventScript_ItemMinerai(ctx: Ctx) {
    find_item(&ctx, ITEM_SIL_MINERAI_FERREUX, 1).await;
}

/// The Nugget under the collapsed prop (9, 10).
pub async fn Evalon_Galeries_EventScript_HiddenNugget(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of
    // "SIL1 Task F as built"): the map has a sign here instead (`06_ch8_scripts.ron`).
    hidden_item(&ctx, FLAG_SIL_ITEM_EVALON_GALERIES_2, ITEM_NUGGET).await;
}

/// The Link Stone near the ore (18, 1).
pub async fn Evalon_Galeries_EventScript_HiddenPierreLien(ctx: Ctx) {
    // fallback: hidden items (as above).
    hidden_item(&ctx, FLAG_SIL_ITEM_EVALON_PIERRE_LIEN, ITEM_SIL_PIERRE_LIEN).await;
}
