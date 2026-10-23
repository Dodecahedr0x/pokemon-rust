//! Chapter 3, the Casino « Le Pixel d'Or » (`ViberCity_Casino`) and its basement
//! (`ViberCity_Casino_B1F`): `contenu-jeu/chapitres/03-viber-city.md` §2.9, §2.10, §3 Casino_Sc01
//! to Sc08, Casino_B1F_Sc01 and Sc02, §5 (their trainers) and §7 (the items, the prizes).
//! Optional: open from `VAR_SIL_STORY` 32. `VAR_SIL_CASINO_STATE` runs 0 (nothing) to 1 (Gogo &
//! Romain met), 2 (the basement open) and 3 (the Porygon freed).
//!
//! **Coins.** The engine has no coin counter (Gen3 `coins`): the source's fallback,
//! `VAR_SIL_COINS`, capped at 9 999 as Gen3's. Money is the engine's (`money_check`,
//! `money_remove`).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["ViberCity_Casino", "ViberCity_Casino_B1F"];

/// Gen3's coin cap (`MAX_COINS`).
const MAX_COINS: u16 = 9_999;

/// `METATILE_ESCALIER_DESCENDANT`: the hidden stairs as the game corner set's exit mat (518), a
/// south arrow warp ("SIL2 Ch2–Ch3 maps as built").
const METATILE_ESCALIER: u16 = 518;
const ESCALIER: (i16, i16) = (18, 1);

async fn coins(ctx: &Ctx) -> u16 {
    ctx.var(VAR_SIL_COINS).await
}

/// Adds `n` coins, up to the cap.
async fn add_coins(ctx: &Ctx, n: u16) {
    let c = coins(ctx).await.saturating_add(n).min(MAX_COINS);
    ctx.set_var(VAR_SIL_COINS, c).await;
}

/// Takes `n` coins when the case holds them; returns whether it did.
async fn take_coins(ctx: &Ctx, n: u16) -> bool {
    let c = coins(ctx).await;
    if c < n {
        return false;
    }
    ctx.set_var(VAR_SIL_COINS, c - n).await;
    true
}

async fn coin_case(ctx: &Ctx) -> bool {
    ctx.flag(FLAG_SIL_ITEM_VIBER_COIN_CASE).await
}

// ---- map scripts ----

/// The hidden stairs (18, 1) drawn as the exit mat once the switch was pressed
/// (`VAR_SIL_CASINO_STATE` ≥ 2, Casino_Sc08).
pub async fn ViberCity_Casino_OnLoad(ctx: Ctx) {
    if ctx.var(VAR_SIL_CASINO_STATE).await >= 2 {
        ctx.set_metatile(ESCALIER.0, ESCALIER.1, METATILE_ESCALIER, false)
            .await;
    }
}

/// Gogo and Romain play upstairs from `VAR_SIL_STORY` 32 while `VAR_SIL_CASINO_STATE` < 3 (at 3
/// Romain has gone down to the basement, B1F_Sc02).
pub async fn ViberCity_Casino_OnTransition(ctx: Ctx) {
    let here = story(&ctx).await >= 32 && ctx.var(VAR_SIL_CASINO_STATE).await < 3;
    ctx.set_flag(FLAG_SIL_HIDE_CASINO_GOGO, !here).await;
    ctx.set_flag(FLAG_SIL_HIDE_CASINO_ROMAIN, !here).await;
}

// ---- the coin case and the coins (Casino_Sc01, Sc02) ----

/// Casino_Sc01, the coin case seller (2, 9): 500 ₽, once (`FLAG_SIL_ITEM_VIBER_COIN_CASE`).
pub async fn ViberCity_Casino_Sc01(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if coin_case(&ctx).await {
        say(&ctx, Casino_Sc01_T4).await;
    } else if !ask(&ctx, Casino_Sc01_T1).await {
        say(&ctx, Casino_Sc01_T3).await;
    } else if !money_check(&ctx, 500).await {
        say(&ctx, Casino_Sc01_T5).await;
    } else {
        money_remove(&ctx, 500).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_VIBER_COIN_CASE,
            ITEM_COIN_CASE,
            1,
            Obtain::Message,
        )
        .await;
        say(&ctx, Casino_Sc01_T2).await;
    }
    ctx.release().await;
}

/// Casino_Sc02, the coin counter (3, 7), talked to across the counter: 50 coins for 1 000 ₽.
/// fallback: multichoice (§9, "50 / 500 / cancel"): one yes/no on T1, for 50 coins.
pub async fn ViberCity_Casino_Sc02(ctx: Ctx) {
    if !coin_case(&ctx).await {
        msgbox(&ctx, Casino_Sc02_T4, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    fallback(&ctx, "multichoice").await;
    if ask(&ctx, Casino_Sc02_T1).await {
        if coins(&ctx).await > MAX_COINS - 50 {
            say(&ctx, Casino_Sc02_T3).await;
        } else if money_check(&ctx, 1_000).await {
            money_remove(&ctx, 1_000).await;
            add_coins(&ctx, 50).await;
            say(&ctx, Casino_Sc02_T2).await;
        }
    }
    ctx.release().await;
}

// ---- Gogo & Romain (Casino_Sc03) ----

const TAPE: &[Step] = &[
    walk_in_place(Dir::Left),
    walk_in_place(Dir::Left),
    walk_in_place(Dir::Left),
];
const FACE_DOWN: &[Step] = &[face(Dir::Down)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];

/// Casino_Sc03, Gogo & Romain at the violet machine: two cells in front of Gogo (3, 3) at
/// `VAR_SIL_CASINO_STATE` 0, or either of them talked to first (the row can be walked round).
/// They do not fight (on a break). Sets `VAR_SIL_CASINO_STATE` 1.
pub async fn ViberCity_Casino_Sc03(ctx: Ctx) {
    if story(&ctx).await < 32 || ctx.var(VAR_SIL_CASINO_STATE).await != 0 {
        return;
    }
    ctx.lock_all().await;
    // Gogo bangs on the machine on his left (the source's WalkInPlace(Up): the built machine is
    // at (2, 1)).
    move_wait(&ctx, LOCALID_CASINO_GOGO, TAPE).await;
    say(&ctx, Casino_Sc03_T1).await;
    say(&ctx, Casino_Sc03_T2).await;
    move_wait(&ctx, LOCALID_CASINO_GOGO, FACE_DOWN).await;
    say(&ctx, Casino_Sc03_T3).await;
    move_wait(&ctx, LOCALID_CASINO_ROMAIN, FACE_DOWN).await;
    say(&ctx, Casino_Sc03_T4).await;
    say(&ctx, Casino_Sc03_T5).await;
    say(&ctx, Casino_Sc03_T6).await;
    ctx.set_var(VAR_SIL_CASINO_STATE, 1).await;
    ctx.release_all().await;
}

/// Gogo: Casino_Sc03 the first time, then his repeatable line.
pub async fn ViberCity_Casino_EventScript_Gogo(ctx: Ctx) {
    if ctx.var(VAR_SIL_CASINO_STATE).await == 0 {
        ViberCity_Casino_Sc03(ctx).await;
        return;
    }
    msgbox(&ctx, Casino_Gogo_T1, MsgBox::Npc).await;
}

/// Romain: Casino_Sc03 the first time, then his hint.
pub async fn ViberCity_Casino_EventScript_Romain(ctx: Ctx) {
    if ctx.var(VAR_SIL_CASINO_STATE).await == 0 {
        ViberCity_Casino_Sc03(ctx).await;
        return;
    }
    msgbox(&ctx, Casino_Romain_T1, MsgBox::Npc).await;
}

// ---- Monsieur Dex's « Plus ou Moins » (Casino_Sc04) ----

/// The source's three fixed sequences (its fallback without a random draw of names): each
/// Pokémon and its national Pokédex number.
const SEQUENCES: [[(SpeciesId, u16); 6]; 3] = [
    [
        (SPECIES_PIKACHU, 25),
        (SPECIES_RAIKOU, 243),
        (SPECIES_VOLTORB, 100),
        (SPECIES_PORYGON, 137),
        (SPECIES_MEOWTH, 52),
        (SPECIES_BELDUM, 374),
    ],
    [
        (SPECIES_SKITTY, 300),
        (SPECIES_GASTLY, 92),
        (SPECIES_EEVEE, 133),
        (SPECIES_MAGNEMITE, 81),
        (SPECIES_MINUN, 312),
        (SPECIES_PLUSLE, 311),
    ],
    [
        (SPECIES_ELECTRODE, 101),
        (SPECIES_VOLTORB, 100),
        (SPECIES_LILEEP, 345),
        (SPECIES_RALTS, 280),
        (SPECIES_GENGAR, 94),
        (SPECIES_MAWILE, 303),
    ],
];

/// What stopping after `n` right answers pays back (the stake is 10).
const DEX_PAYOUT: [u16; 5] = [10, 15, 20, 30, 40];

/// Casino_Sc04, MONSIEUR DEX (9, 6): stake 10 coins; one of the three sequences (`random(3)`);
/// for each Pokémon, is the next one's number greater (yes) or smaller (no)? A wrong answer loses
/// the stake; five right ones pay 40; stopping after a right answer pays back 10, 15, 20 or 30.
/// The names come through `STR_VAR_1` and `STR_VAR_2` (Gen3 `bufferspeciesname`).
pub async fn ViberCity_Casino_Sc04(ctx: Ctx) {
    if !coin_case(&ctx).await {
        msgbox(&ctx, Casino_Sc02_T4, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    if !ask(&ctx, Casino_Sc04_T1).await {
        say(&ctx, Casino_Sc04_T7).await;
        ctx.release().await;
        return;
    }
    if !take_coins(&ctx, 10).await {
        say(&ctx, Casino_Sc07_T3).await;
        ctx.release().await;
        return;
    }
    let seq = &SEQUENCES[usize::from(ctx.random(3).await.min(2))];
    for i in 0..5 {
        let (cur, cur_num) = seq[i];
        let (next, next_num) = seq[i + 1];
        buffer(&ctx, 0, BufferSrc::Species(cur)).await;
        let plus = ask(&ctx, Casino_Sc04_T2).await;
        buffer(&ctx, 1, BufferSrc::Species(next)).await;
        if plus != (next_num > cur_num) {
            say(&ctx, Casino_Sc04_T4).await;
            break;
        }
        if i == 4 {
            say(&ctx, Casino_Sc04_T5).await;
            add_coins(&ctx, DEX_PAYOUT[4]).await;
            break;
        }
        if !ask(&ctx, Casino_Sc04_T3).await {
            say(&ctx, Casino_Sc04_T6).await;
            add_coins(&ctx, DEX_PAYOUT[i]).await;
            break;
        }
    }
    ctx.release().await;
}

// ---- the Electric Roulette (Casino_Sc05) ----

/// Casino_Sc05, the croupier (10, 1) or the roulette table (9–11, 2): stake 10 coins on a neon
/// colour; the strobe stops on blue, yellow or violet (30 % each) or black (10 %: everybody
/// loses); the right colour triples the stake. Once the basement is freed
/// (`FLAG_SIL_CASINO_LOYAL`) the black slot is gone. After the city's black-out, the croupier's
/// line first. fallback: multichoice (§9: the stake and the colour): a yes/no on T1, stake 10,
/// on blue.
pub async fn ViberCity_Casino_Sc05(ctx: Ctx) {
    if !coin_case(&ctx).await {
        msgbox(&ctx, Casino_Sc02_T4, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    if story(&ctx).await >= 34 {
        say(&ctx, Casino_Sc05_T6).await;
    }
    fallback(&ctx, "multichoice").await;
    if ask(&ctx, Casino_Sc05_T1).await {
        if !take_coins(&ctx, 10).await {
            say(&ctx, Casino_Sc07_T3).await;
        } else {
            say(&ctx, Casino_Sc05_T2).await;
            let roll = if ctx.flag(FLAG_SIL_CASINO_LOYAL).await {
                ctx.random(3).await * 3
            } else {
                ctx.random(10).await
            };
            if roll >= 9 {
                say(&ctx, Casino_Sc05_T5).await;
            } else if roll / 3 == 0 {
                say(&ctx, Casino_Sc05_T3).await;
                add_coins(&ctx, 30).await;
            } else {
                say(&ctx, Casino_Sc05_T4).await;
            }
        }
    }
    ctx.release().await;
}

// ---- the slot machines (Casino_Sc06) ----

/// Casino_Sc06, a slot machine (read from the row below). fallback: the slot machine screen
/// (§9, a port of `slot_machine.c`): the source's `random(16)` script, one coin a pull:
/// 0 → ×10, 1–2 → ×4, 3–5 → ×2, else lost. Until the basement is freed, a win that would take
/// this visit's winnings (`VAR_TEMP_1`, cleared on every load) past 500 is forced to lose. A win
/// has no text in the source (an author question): the coins are added silently.
pub async fn ViberCity_Casino_Sc06(ctx: Ctx) {
    if !coin_case(&ctx).await {
        msgbox(&ctx, Casino_Sc02_T4, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    fallback(&ctx, "slot machine").await;
    if ask(&ctx, Casino_Sc06_T1).await {
        if !take_coins(&ctx, 1).await {
            say(&ctx, Casino_Sc07_T3).await;
        } else {
            let win = match ctx.random(16).await {
                0 => 10,
                1..=2 => 4,
                3..=5 => 2,
                _ => 0,
            };
            let gained = ctx.var(VAR_TEMP_1).await;
            let forced = !ctx.flag(FLAG_SIL_CASINO_LOYAL).await && gained + win > 500;
            if win == 0 || forced {
                say(&ctx, Casino_Sc06_T2).await;
            } else {
                add_coins(&ctx, win).await;
                ctx.set_var(VAR_TEMP_1, gained + win).await;
            }
        }
    }
    ctx.release().await;
}

/// Gogo's violet machine (2, 1): its sticker.
pub async fn ViberCity_Casino_EventScript_MachineViolette(ctx: Ctx) {
    msgbox(&ctx, Casino_Sc06_T3, MsgBox::Sign).await;
}

/// The 20 coins under a machine (14, 3), once (`FLAG_SIL_ITEM_CASINO_COINS_FLOOR`), with the
/// coin case (Gen3's hidden coins need it). fallback: the hidden coins' message (Gen3
/// `gText_FoundXCoins`; no Silice text): the coins are added silently.
pub async fn ViberCity_Casino_EventScript_JetonsCaches(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_ITEM_CASINO_COINS_FLOOR).await || !coin_case(&ctx).await {
        return;
    }
    fallback(&ctx, "hidden coins message").await;
    add_coins(&ctx, 20).await;
    ctx.set_flag(FLAG_SIL_ITEM_CASINO_COINS_FLOOR, true).await;
}

// ---- the prize counter (Casino_Sc07) ----

/// Casino_Sc07, the prize counter (16, 7), talked to across the counter. fallback: multichoice
/// (§9: the TM, item and Pokémon menus, §7's prices): the clerk's welcome; nothing is exchanged
/// yet, as the `mart` fallback.
pub async fn ViberCity_Casino_Sc07(ctx: Ctx) {
    if !coin_case(&ctx).await {
        msgbox(&ctx, Casino_Sc02_T4, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Casino_Sc07_T1).await;
    fallback(&ctx, "multichoice").await;
    ctx.release().await;
}

// ---- the poster and the hidden switch (Casino_Sc08) ----

/// Casino_Sc08, the poster (17, 0) read from (17, 1), from `VAR_SIL_STORY` 32: the switch behind
/// it reveals the stairs (18, 1) (`VAR_SIL_CASINO_STATE` 2; `ViberCity_Casino_OnLoad` keeps them).
/// Afterwards, the crooked poster.
pub async fn ViberCity_Casino_Sc08(ctx: Ctx) {
    if story(&ctx).await < 32 {
        return;
    }
    if ctx.var(VAR_SIL_CASINO_STATE).await >= 2 {
        msgbox(&ctx, Casino_Sc08_T4, MsgBox::Sign).await;
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Casino_Sc08_T1).await;
    if ask(&ctx, Casino_Sc08_T2).await {
        say(&ctx, Casino_Sc08_T3).await;
        ctx.set_metatile(ESCALIER.0, ESCALIER.1, METATILE_ESCALIER, false)
            .await;
        ctx.set_var(VAR_SIL_CASINO_STATE, 2).await;
        move_wait(&ctx, LOCALID_CASINO_PNJ04, FACE_PLAYER).await;
        say(&ctx, Casino_Pnj04_T2).await;
    }
    ctx.release_all().await;
}

// ---- the casino's people ----

/// The player glued to his machine.
pub async fn ViberCity_Casino_EventScript_Pnj01(ctx: Ctx) {
    msgbox(&ctx, Casino_Pnj01_T1, MsgBox::Npc).await;
}

/// The old lady.
pub async fn ViberCity_Casino_EventScript_Pnj02(ctx: Ctx) {
    msgbox(&ctx, Casino_Pnj02_T1, MsgBox::Npc).await;
}

/// The waiter.
pub async fn ViberCity_Casino_EventScript_Pnj03(ctx: Ctx) {
    msgbox(&ctx, Casino_Pnj03_T1, MsgBox::Npc).await;
}

/// The guard by the poster: before the switch, "move along"; after it, his shrug.
pub async fn ViberCity_Casino_EventScript_Pnj04(ctx: Ctx) {
    let text = if ctx.var(VAR_SIL_CASINO_STATE).await >= 2 {
        Casino_Pnj04_T2
    } else {
        Casino_Pnj04_T1
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// PARIEUR FERNAND (9, 4), facing down.
pub async fn ViberCity_Casino_EventScript_Fernand(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_CASINO_FERNAND,
        TRAINER_SIL_CASINO_FERNAND_1,
        Casino_Fernand_Intro,
        Casino_Fernand_Defaite,
    )
    .await
    {
        post_battle(&ctx, Casino_Fernand_Apres).await;
    }
}

/// RICHARD GASTON (11, 7), facing left.
pub async fn ViberCity_Casino_EventScript_Gaston(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_CASINO_GASTON,
        TRAINER_SIL_CASINO_GASTON_1,
        Casino_Gaston_Intro,
        Casino_Gaston_Defaite,
    )
    .await
    {
        post_battle(&ctx, Casino_Gaston_Apres).await;
    }
}

/// MADEMOISELLE SOLÈNE (13, 9), facing left.
pub async fn ViberCity_Casino_EventScript_Solene(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_CASINO_SOLENE,
        TRAINER_SIL_CASINO_SOLENE_1,
        Casino_Solene_Intro,
        Casino_Solene_Defaite,
    )
    .await
    {
        post_battle(&ctx, Casino_Solene_Apres).await;
    }
}

// ---- the basement ----

/// Romain starts hidden (B1F_Sc02 shows him); the head croupier stands aside on (6, 10) once
/// beaten (`FLAG_SIL_CASINO_CHEF`).
pub async fn ViberCity_Casino_B1F_OnTransition(ctx: Ctx) {
    ctx.set_flag(FLAG_SIL_HIDE_CASINO_B1F_ROMAIN, true).await;
    if ctx.flag(FLAG_SIL_CASINO_CHEF).await {
        ctx.set_object_template_pos(LOCALID_CASINO_B1F_CROUPIER_CHEF, CHEF_SEAT.0, CHEF_SEAT.1)
            .await;
    }
}

/// Casino_B1F_Sc01, the evidence terminal (6, 7), read from (6, 8): repeatable; the first
/// reading copies the proof (`FLAG_SIL_CASINO_PREUVE`).
pub async fn ViberCity_Casino_B1F_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Casino_B1F_Sc01_T1).await;
    say(&ctx, Casino_B1F_Sc01_T2).await;
    if !ctx.flag(FLAG_SIL_CASINO_PREUVE).await {
        ctx.set_flag(FLAG_SIL_CASINO_PREUVE, true).await;
        say(&ctx, Casino_B1F_Sc01_T3).await;
    }
    ctx.release_all().await;
}

/// The head croupier steps aside from the passage (10, 9): one step down, then (the fade) behind
/// the capsule on (6, 10). The built room has the capsule (8, 10) solid and the cells west of it
/// walled in, so there is no walkable way there ("SIL2 Ch3 scripts as built").
const CHEF_DOWN: &[Step] = &[walk(Dir::Down)];
const CHEF_SEAT: (i16, i16) = (6, 10);

/// The head croupier (10, 9), blocking the Porygon's room: his battle, then he steps aside
/// (`FLAG_SIL_CASINO_CHEF`). Afterwards, his complaint.
pub async fn ViberCity_Casino_B1F_EventScript_CroupierChef(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_CASINO_CHEF).await {
        msgbox(&ctx, Casino_B1F_Chef_T2, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Casino_B1F_Chef_T1).await;
    battle_trainer(&ctx, TRAINER_SIL_CROUPIER_CHEF_1, Casino_B1F_Chef_Defaite).await;
    say(&ctx, Casino_B1F_Chef_T2).await;
    move_wait(&ctx, LOCALID_CASINO_B1F_CROUPIER_CHEF, CHEF_DOWN).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.set_object_pos(LOCALID_CASINO_B1F_CROUPIER_CHEF, CHEF_SEAT.0, CHEF_SEAT.1)
        .await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_CASINO_CHEF, true).await;
    ctx.release().await;
}

const ROMAIN_DOWN: &[Step] = &[walk(Dir::Down), walk(Dir::Down), Step::FacePlayer];
const ROMAIN_UP: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];

/// Casino_B1F_Sc02, the Porygon's capsule (8, 10), read from (9, 10), with the head croupier
/// beaten and `VAR_SIL_CASINO_STATE` 2: the Porygon (level 25), the machines fair from now on
/// (`FLAG_SIL_CASINO_LOYAL`), Romain comes down. Sets `VAR_SIL_CASINO_STATE` 3.
pub async fn ViberCity_Casino_B1F_Sc02(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_CASINO_CHEF).await || ctx.var(VAR_SIL_CASINO_STATE).await != 2 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Casino_B1F_Sc02_T1).await;
    if !ask(&ctx, Casino_B1F_Sc02_T2).await {
        ctx.release_all().await;
        return;
    }
    ctx.fade(Fade::ToBlack).await;
    // fallback: the open capsule's metatile (Facility has none): the capsule is not redrawn.
    fallback(&ctx, "capsule metatile").await;
    ctx.fade(Fade::FromBlack).await;
    ctx.give_mon(SPECIES_PORYGON, 25).await;
    // fallback: give_mon with custom moves, a held item and an OT (§9: CONVERSION 2, PSYBEAM,
    // RECOVER, AGILITY; the Up-Grade; "COCKET"): the level-25 moves, and the Up-Grade in the bag.
    fallback(&ctx, "give_mon custom").await;
    add_item(&ctx, ITEM_UP_GRADE, 1).await;
    say(&ctx, Casino_B1F_Sc02_T3).await;
    ctx.set_flag(FLAG_SIL_CASINO_LOYAL, true).await;
    ctx.set_flag(FLAG_SIL_PORYGON_LIBERE, true).await;
    ctx.set_var(VAR_SIL_CASINO_STATE, 3).await;
    show(&ctx, LOCALID_CASINO_B1F_ROMAIN, FLAG_SIL_HIDE_CASINO_B1F_ROMAIN).await;
    move_wait(&ctx, LOCALID_CASINO_B1F_ROMAIN, ROMAIN_DOWN).await;
    say(&ctx, Casino_B1F_Sc02_T4).await;
    move_wait(&ctx, LOCALID_CASINO_B1F_ROMAIN, ROMAIN_UP).await;
    hide(&ctx, LOCALID_CASINO_B1F_ROMAIN).await;
    ctx.release_all().await;
}

/// SBIRE 1 (3, 3), facing right.
pub async fn ViberCity_Casino_B1F_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_CASINO_B1F_SBIRE_1,
        TRAINER_SIL_GRUNT_CASINO_1,
        Casino_B1F_Sbire1_Intro,
        Casino_B1F_Sbire1_Defaite,
    )
    .await
    {
        post_battle(&ctx, Casino_B1F_Sbire1_Apres).await;
    }
}

/// SBIRE 2 (11, 6), facing left.
pub async fn ViberCity_Casino_B1F_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_CASINO_B1F_SBIRE_2,
        TRAINER_SIL_GRUNT_CASINO_2,
        Casino_B1F_Sbire2_Intro,
        Casino_B1F_Sbire2_Defaite,
    )
    .await
    {
        post_battle(&ctx, Casino_B1F_Sbire2_Apres).await;
    }
}

/// SBIRE 3 (10, 2), facing down.
pub async fn ViberCity_Casino_B1F_EventScript_Sbire3(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_CASINO_B1F_SBIRE_3,
        TRAINER_SIL_GRUNT_CASINO_3,
        Casino_B1F_Sbire3_Intro,
        Casino_B1F_Sbire3_Defaite,
    )
    .await
    {
        post_battle(&ctx, Casino_B1F_Sbire3_Apres).await;
    }
}

/// The Silk Scarf in the top right room (13, 1).
pub async fn ViberCity_Casino_B1F_EventScript_ItemSilk(ctx: Ctx) {
    find_item(&ctx, ITEM_SILK_SCARF, 1).await;
}

/// The Ultra Ball in the corridor (1, 6).
pub async fn ViberCity_Casino_B1F_EventScript_ItemUltraBall(ctx: Ctx) {
    find_item(&ctx, ITEM_ULTRA_BALL, 1).await;
}

/// The Rare Candy in the Porygon's room (11, 10).
pub async fn ViberCity_Casino_B1F_EventScript_ItemRareCandy(ctx: Ctx) {
    find_item(&ctx, ITEM_RARE_CANDY, 1).await;
}
