//! Chapter 11, the Apex, the Zenith and the endings, from the Apex's arrival (`VAR_SIL_STORY`
//! 109) to the epilogue at home (119):
//! - the Apex: the riff in the wind (`Sc01`, 110), Gogo and Romain worn out on the catwalk
//!   (`Sc02`, 111), the Grand-Duc's solo (`Sc03`: Poussin's Primal Kyogre), his resignation
//!   (`Sc04`, 112), the white-and-gold glove at the console (`Sc05`, trigger A: the Mallette
//!   taken, 100 %, the alternate ending's first choice answered No, 113), the first-aid kit
//!   (`Sc06`: the heal, two Full Restores, 114), the crystal tube (`Sc07`, yes);
//! - the cabin's ascent (115); the Zenith: the call of the new day (`Sc08`: the last choice
//!   answered No, Cocorico's battle, no whiteout), the rupture (`Sc09`, 116), the night
//!   (`Sc10`–`Sc11`, 117), the descent (`Sc12`, 118), Nébula's waking and the credits
//!   (`Nebula_Sc13`), three days later at home (`Brico_Bourg_Sc14`: the scripted save; Maman,
//!   the cracked Mallette, 119).
//!
//! The alternate ending (Elysium, canonical, R-3) is its own lists: [`elysium_apex_start`] /
//! [`elysium_apex_steps`] (two yes at the console) and [`elysium_zenith_start`] /
//! [`elysium_zenith_steps`] (two yes on the throne): `Elysium_E1`–`E5`, the credits, then the
//! reload of the last save (`Event::ReloadLastSave`, `Run::reloaded`).
//!
//! **Start** ([`start`]): the Apex's arrival (11, 27), where Chapter 10's e2e ends, with
//! Chapter 10's end state ([`end_of_chapter_10`]).
//!
//! **The party** ([`PARTY`]): Chapter 10's end as `sil11_ch10` leaves it (ALAKAZAM 90,
//! TYPHLOSION 91, the four others 89); both TMs taught again first (`ch10::TEACH`). The Apex's
//! elites are avoided; the run heals at `Sc06` (Romain's kit) and mends with the bag's medicine
//! after Poussin.

use super::ch10::{TEACH, end_of_chapter_9_flags, give_party};
use super::{Run, Start, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const APEX: MapId = MAP_SOMMET_APEX;
const ZENITH: MapId = MAP_TRONE_ZENITH;
const HOUSE_2F: MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_2F;
const HOUSE_1F: MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_1F;

/// The Apex's arrival from the vault elevator (11, 27), facing north, Chapter 10's end.
pub fn start() -> Start {
    Start {
        map: APEX,
        x: 11,
        y: 27,
        facing: Dir::Up,
        setup: |ctx| {
            Box::pin(async move {
                end_of_chapter_10(ctx.clone()).await;
                give_party(&ctx, PARTY).await;
            })
        },
    }
}

/// The start state's party (see the module docs).
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_ALAKAZAM, 90),
    (SPECIES_TYPHLOSION, 91),
    (SPECIES_AMPHAROS, 89),
    (SPECIES_AGGRON, 89),
    (SPECIES_DUGTRIO, 89),
    (SPECIES_PORYGON2, 89),
];

/// Chapter 10's end without the party: Chapter 9's ([`end_of_chapter_9_flags`]), the three
/// access keys, the Badge Miroir restored, the Tower's scenes seen, the sync at 99, the story at
/// 109 (the Apex's `Sc01` plays on arrival).
pub async fn end_of_chapter_10(ctx: Ctx) {
    end_of_chapter_9_flags(ctx.clone()).await;
    for f in [
        FLAG_SIL_KEY_ACCES_ALPHA,
        FLAG_SIL_KEY_ACCES_BETA,
        FLAG_SIL_KEY_ACCES_GAMMA,
        FLAG_SIL_BADGE_MIROIR_RESTORED,
        FLAG_SIL_TOUR_SEUIL_VU,
        FLAG_SIL_HIDE_TOUR_REVENGE,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_REVENGE_STATUE, false).await;
    ctx.set_var(VAR_SIL_SYNC, 99).await;
    ctx.set_var(VAR_SIL_STORY, 109).await;
}

/// Kevin's (8, 13) line of sight east, Marc's (17, 22) west, Sophie's (3, 19) east.
const APEX_AVOID: &[(i16, i16)] = &[
    (9, 13),
    (10, 13),
    (11, 13),
    (12, 13),
    (13, 22),
    (14, 22),
    (15, 22),
    (16, 22),
    (4, 19),
    (5, 19),
    (6, 19),
];

/// The chapter's steps, from [`start`].
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut s = TEACH.to_vec();
    s.extend([
        // ---- the Apex ----
        Step::Settle,
        Step::Check("apex_sc01", |r| {
            assert_eq!(r.w.map(), APEX);
            assert_eq!(r.pos(), (11, 25));
            assert_eq!(r.w.var(VAR_SIL_STORY), 110);
            assert!(r.has_text(Sommet_Apex_Sc01_T1));
        }),
        goto_avoiding(APEX, 11, 19, APEX_AVOID),
        Step::Walk(Up),
        Step::Check("apex_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 111);
            r.printed_in_order(&[Sommet_Apex_Sc02_T1, Sommet_Apex_Sc02_T5]);
        }),
        // The stage's steps (10–13, 9), from the east column (Kevin's row is avoided).
        goto_avoiding(APEX, 13, 10, APEX_AVOID),
        Step::Walk(Up),
        Step::Check("poussin", |r| {
            assert!(r.beaten(TRAINER_SIL_POUSSIN_1));
            assert_eq!(r.w.var(VAR_SIL_STORY), 112);
            assert!(r.w.flag(FLAG_SIL_HIDE_APEX_POUSSIN));
            r.printed_in_order(&[
                Sommet_Apex_Sc03_T3,
                Sommet_Apex_Sc04_T8,
                Sommet_Apex_Sc04_T10,
            ]);
        }),
        Step::Mend(6),
        // The console (12, 4), trigger A: both choices of the alternate ending answer No (their
        // cursors start on No).
        goto(APEX, 12, 5),
        Step::Interact(Up),
        Step::Check("glove", |r| {
            assert!(r.has_text(Sommet_Apex_Sc05_T1a));
            assert!(r.has_text(Sommet_Apex_Sc05_C1n));
            assert!(!r.w.flag(FLAG_SIL_FIN_ELYSIUM_EN_COURS));
            assert!(r.w.flag(FLAG_SIL_MALLETTE_PRISE));
            assert_eq!(r.count(ITEM_SIL_MALLETTE), 0);
            assert_eq!(r.w.var(VAR_SIL_SYNC), 100);
            assert!(r.w.flag(FLAG_SIL_ELYSIUM_ACTIF));
        }),
        Step::Check("first_aid", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 114);
            assert!(r.w.flag(FLAG_SIL_ITEM_APEX_ROMAIN));
            assert!(r.healed());
            assert!(r.has_text(Sommet_Apex_Sc06_T4));
        }),
        // The crystal tube (12, 2), from beside the landing (11, 3).
        goto(APEX, 12, 3),
        Step::Interact(Up),
        Step::Check("ascent", |r| {
            assert_eq!(r.w.map(), ZENITH);
            assert_eq!(r.pos(), (7, 13));
            assert_eq!(r.w.var(VAR_SIL_STORY), 115);
            r.printed_in_order(&[Ascenseur_Sc07_T2, Ascenseur_Sc07_T3]);
        }),
        // ---- the Zenith ----
        goto(ZENITH, 7, 9),
        Step::Walk(Up),
        Step::Check("cocorico", |r| {
            assert!(r.beaten(TRAINER_SIL_COCORICO_1));
            assert!(r.has_text(Trone_Zenith_Sc08_C3));
            assert!(r.w.flag(FLAG_SIL_CIEL_VRAI));
            assert!(!r.w.flag(FLAG_SIL_ELYSIUM_ACTIF));
            r.printed_in_order(&[
                Trone_Zenith_Sc09_T5,
                Trone_Zenith_Sc10_T1,
                Trone_Zenith_Sc11_T12,
                Ascenseur_Sc12_T1,
                Nebula_Sc13_T4,
                Fin_Texte_T1,
                Fin_Texte_FIN,
                Brico_Bourg_Sc14_T1,
            ]);
            assert!(r.w.flag(FLAG_SIL_GAME_CLEAR));
            assert_eq!(r.w.var(VAR_SIL_STORY), 118);
            assert_eq!(r.w.var(VAR_SIL_FIN_SCENE), 0);
            assert_eq!(r.w.map(), HOUSE_2F);
            assert!(!r.w.flag(FLAG_SIL_HIDE_HOUSE_EPILOGUE));
        }),
        // ---- three days later: the stairs down, the coords off their foot ----
        goto(HOUSE_2F, 7, 2),
        Step::Enter {
            dir: Up,
            map: HOUSE_1F,
        },
        // The stairs land on their foot (8, 2); the scene's coords are the cells off it.
        Step::Check("stairs_foot", |r| assert_eq!(r.pos(), (8, 2))),
        Step::Walk(Down),
        Step::Settle,
    ]);
    s
}

/// What the end of Chapter 11 asserts beyond its checkpoints: the epilogue's second half, the
/// cracked Mallette back in the bag, and the fallbacks reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.map(), HOUSE_1F);
    assert_eq!(r.w.var(VAR_SIL_STORY), 119);
    r.printed_in_order(&[Brico_Bourg_Sc14_T2, Brico_Bourg_Sc14_T11]);
    assert!(r.w.flag(FLAG_SIL_MALLETTE_FELEE));
    assert!(!r.w.flag(FLAG_SIL_MALLETTE_PRISE));
    assert_eq!(r.count(ITEM_SIL_MALLETTE), 1);
    assert!(r.w.flag(FLAG_SIL_HIDE_HOUSE_EPILOGUE));
    assert!(!r.reloaded);
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "play_music",
        "battle_lines",
        "weather",
        "palette",
        "cabin_background",
        "sky_crack",
        "credits_montage",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}

/// The alternate ending from the console: the Apex at the console's foot (12, 5), facing north,
/// after Poussin's resignation (112).
pub fn elysium_apex_start() -> Start {
    Start {
        map: APEX,
        x: 12,
        y: 5,
        facing: Dir::Up,
        setup: |ctx| {
            Box::pin(async move {
                end_of_chapter_10(ctx.clone()).await;
                ctx.set_var(VAR_SIL_STORY, 112).await;
                ctx.set_flag(FLAG_SIL_HIDE_APEX_POUSSIN, true).await;
                ctx.set_flag(content::prelude::trainer_flag(TRAINER_SIL_POUSSIN_1), true)
                    .await;
                give_party(&ctx, PARTY).await;
            })
        },
    }
}

/// The console, three yes: Julien's question (`Sc05_T6`, on yes), then both choices
/// (`Sc05_C1`, `Sc05_C2`, cursors on No): `Elysium_E1`–`E5`, the credits, the reload.
pub fn elysium_apex_steps() -> Vec<Step> {
    vec![
        Step::Yes(3),
        Step::Interact(Dir::Up),
        Step::Check("elysium_apex", |r| check_elysium(r, Sommet_Apex_Sc05_C1)),
    ]
}

/// The alternate ending on the throne: the Zenith one cell south of the call's coords (7, 9),
/// facing north, after the ascent (115).
pub fn elysium_zenith_start() -> Start {
    Start {
        map: ZENITH,
        x: 7,
        y: 9,
        facing: Dir::Up,
        setup: |ctx| {
            Box::pin(async move {
                end_of_chapter_10(ctx.clone()).await;
                ctx.set_var(VAR_SIL_STORY, 115).await;
                ctx.set_var(VAR_SIL_SYNC, 100).await;
                for f in [
                    FLAG_SIL_HIDE_APEX_POUSSIN,
                    FLAG_SIL_MALLETTE_PRISE,
                    FLAG_SIL_ELYSIUM_ACTIF,
                    FLAG_SIL_ITEM_APEX_ROMAIN,
                ] {
                    ctx.set_flag(f, true).await;
                }
                ctx.item_remove(ItemId(ITEM_SIL_MALLETTE), 1).await;
                give_party(&ctx, PARTY).await;
            })
        },
    }
}

/// The throne, two yes (`Sc08_C1`, `Sc05_C2`): the same ending from the Zenith.
pub fn elysium_zenith_steps() -> Vec<Step> {
    vec![
        Step::Yes(2),
        Step::Walk(Dir::Up),
        Step::Check("elysium_zenith", |r| check_elysium(r, Trone_Zenith_Sc08_C1)),
    ]
}

/// The alternate ending played to its end: the five scenes in order, the credits, `FIN`, the
/// scene var and flag cleared, the reload asked for, and nothing of the canonical end.
fn check_elysium(r: &Run, question: engine::data::TextId) {
    r.printed_in_order(&[
        question,
        Sommet_Apex_Sc05_C2,
        Elysium_E1_T1,
        Elysium_E1_T3,
        Elysium_E2_T0,
        Elysium_E3_T1,
        Elysium_E4_T1,
        Elysium_E4_T5,
        Elysium_E5_T1,
        Elysium_Fin_T1,
        Elysium_Fin_FIN,
    ]);
    assert!(r.reloaded, "the last save reloaded");
    assert!(!r.w.flag(FLAG_SIL_FIN_ELYSIUM_EN_COURS));
    assert_eq!(r.w.var(VAR_SIL_FIN_SCENE), 0);
    assert!(!r.w.flag(FLAG_SIL_GAME_CLEAR));
    assert!(!r.has_text(Nebula_Sc13_T1));
    for name in [
        "elysium_free_roam",
        "hide_player",
        "show_title",
        "credits_montage",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
