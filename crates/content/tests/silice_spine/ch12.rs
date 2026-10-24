//! Chapter 12, the post-game, from Chapter 11's canonical end (`VAR_SIL_STORY` 119, at home) to
//! the last ceiling (`Brico_Bourg_PG_Sc20`, 131), then its optional goals. Each segment starts
//! from its documented state ([`Start`]), the state the segments before it leave (set directly:
//! the region's travel between them is not the chapter's), and plays the scenes by buttons:
//! - [`reveil`]: the first true morning (`Sc01`, 120);
//! - [`conseil`]: the Council of Three (`Sc02`, 121; five Full Restores);
//! - [`julien`]: Julien at the Tower's foot (`Sc03`, 122), his daily rematch (`Sc03b`: the
//!   Salamencite), refused the same day, played again the next day ([`Step::Days`]);
//! - [`phare`]: Thomas's reflection at the lighthouse (`Sc04`, 123);
//! - [`vestiges`]: Kimiko in the Bois des Soupirs (`Sc05`), Arthur on the wreck (`Sc06`), Silas
//!   at the Point Zéro (`Sc07`): each battled and pardoned (124, 125, 126, the Breach's alarm);
//!   Silas's mail at home (`Sc09`, the Porygon, the egg's fallback);
//! - [`horde`]: the sky splits at Nébula (`Sc10`, two wild battles, 127), Thomas's camp in the
//!   Breach (`Sc10b`: the heal, the Heracronite);
//! - [`tour`]: the elevator hub, Gwada's Filter (`Sc11`), the three components on the ruined
//!   floors (gas sheets, Mathias's barrels, the mirrors), Filter v2 (`Sc15`, 128);
//! - [`dehors`]: the Breach's v2 gas, the outside (`Sc16`), Poussin at the Relay (`Sc16b`: the
//!   Blue Orb, Lugia and Kyogre freed), the three soil samples (`Sc17`), the Centrale's
//!   switches (Zapdos), the Ice Vault's door (Regice);
//! - [`balise`]: Gwada's « life can start again » (`Sc18`, 129), the Purity Beacon on the Apex
//!   (`Sc19`, 130: Suicune, Mew and Ho-Oh), the last ceiling at home (`Sc20`: the National Dex,
//!   131);
//! - [`options`]: Benou (his battle, the Debug Key installed, the Zone 404 opened), the Clone of
//!   Noobody (a copy of the party) and Mewtwo, Crow's Lounge, a gym leader's rematch, the Stele's
//!   « Dominant » (the boss aura) and its tutor move, an Antenna (its rule, three trainers), the
//!   first Code Fragment.
//!
//! **The party** ([`PARTY`]): Chapter 11's six at level 100, a SWAMPERT for the DUGTRIO. The starter is the Cyndaquil
//! (`VAR_SIL_STARTER` 1).

use super::ch10::give_party;
use super::ch11::end_of_chapter_10;
use super::{Run, Start, Step, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, MapId};
use engine::script::Ctx;

/// The post-game's party: Chapter 11's six at level 100, the DUGTRIO swapped for a SWAMPERT (a
/// player who finished the story trains before the post-game's level 75–80 teams; Chapter 11's
/// levels lose to the Vestiges' Mega Evolutions, and the DUGTRIO's frailty to Julien's Entei,
/// under the spine's battle policy).
pub const PARTY: &[(engine::data::SpeciesId, u8)] = &[
    (SPECIES_ALAKAZAM, 100),
    (SPECIES_TYPHLOSION, 100),
    (SPECIES_AMPHAROS, 100),
    (SPECIES_AGGRON, 100),
    (SPECIES_SWAMPERT, 100),
    (SPECIES_PORYGON2, 100),
];

const HOUSE_1F: MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_1F;
const HOUSE_2F: MapId = MAP_BRICO_BOURG_PLAYER_HOUSE_2F;
const BIBLIO: MapId = MAP_MANGA_SQUARE_BIBLIOTHEQUE;
const NML: MapId = MAP_NO_MANS_LAND;
const APEX: MapId = MAP_SOMMET_APEX;
const HALL: MapId = MAP_NEBULA_TOUR_HALL;
const STUDIO: MapId = MAP_TOUR_NEBULA_2F;

/// Chapter 11's canonical end without the party: Chapter 10's, the true sky, the game cleared,
/// the cracked Mallette, Poussin and Julien gone from the Apex, the story at 119.
pub async fn end_of_chapter_11(ctx: Ctx) {
    end_of_chapter_10(ctx.clone()).await;
    for f in [
        FLAG_SIL_CIEL_VRAI,
        FLAG_SIL_GAME_CLEAR,
        FLAG_SIL_MALLETTE_FELEE,
        FLAG_SIL_HIDE_APEX_POUSSIN,
        FLAG_SIL_HIDE_APEX_JULIEN,
        FLAG_SIL_HIDE_HOUSE_EPILOGUE,
        FLAG_SIL_HIDE_HOUSE_EPILOGUE_2,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_APEX_SOCLE, false).await;
    ctx.set_var(VAR_SIL_SYNC, 100).await;
    ctx.set_var(VAR_SIL_STORY, 119).await;
}

/// Chapter 11's end, the party, and the post-game up to `story` (the flags the scenes before it
/// set: `Sc01`'s; the Vestiges' from 126; the Horde's from 127).
async fn post_game_at(ctx: &Ctx, story: u16) {
    post_game_with(ctx, story, PARTY).await;
}

/// [`post_game_at`] with another party.
async fn post_game_with(ctx: &Ctx, story: u16, party: &[(engine::data::SpeciesId, u8)]) {
    end_of_chapter_11(ctx.clone()).await;
    give_party(ctx, party).await;
    if story >= 120 {
        for f in [
            FLAG_SIL_POSTGAME,
            FLAG_SIL_HIDE_APEX_GOGO_ROMAIN,
            FLAG_SIL_HIDE_REVENGE_STATUE,
            FLAG_SIL_HIDE_APEX_POUSSIN_PG,
        ] {
            ctx.set_flag(f, true).await;
        }
    }
    if story >= 126 {
        for f in [
            FLAG_SIL_VESTIGE_KIMIKO,
            FLAG_SIL_VESTIGE_ARTHUR,
            FLAG_SIL_VESTIGE_SILAS,
            FLAG_SIL_PARDON_KIMIKO,
            FLAG_SIL_PARDON_ARTHUR,
            FLAG_SIL_PARDON_SILAS,
            FLAG_SIL_CLE_DEBUG,
            FLAG_SIL_BRECHE_ALERTE,
        ] {
            ctx.set_flag(f, true).await;
        }
        ctx.set_var(VAR_SIL_PARDONS, 3).await;
    }
    ctx.set_var(VAR_SIL_STORY, story).await;
}

fn start_at(
    map: MapId,
    x: i16,
    y: i16,
    facing: Dir,
    setup: fn(Ctx) -> engine::script::Script,
) -> Start {
    Start {
        map,
        x,
        y,
        facing,
        setup,
    }
}

// ---- Act I ----

/// Chapter 11's end: at home (8, 3), below the stairs, at 119.
pub fn reveil_start() -> Start {
    start_at(HOUSE_1F, 8, 3, Dir::Down, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 119).await })
    })
}

/// Out of the house: the first true morning.
pub fn reveil() -> Vec<Step> {
    vec![
        goto(HOUSE_1F, 4, 6),
        Step::Enter {
            dir: Dir::Down,
            map: MAP_BRICO_BOURG,
        },
        Step::Check("pg_sc01", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 120);
            assert!(r.w.flag(FLAG_SIL_POSTGAME));
            r.printed_in_order(&[
                Brico_Bourg_PG_Sc01_T1,
                Brico_Bourg_PG_Sc01_T2,
                Brico_Bourg_PG_Sc01_T3,
            ]);
        }),
    ]
}

/// The library's mat (12, 23), at 120.
pub fn conseil_start() -> Start {
    start_at(BIBLIO, 12, 23, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 120).await })
    })
}

/// One step in: the Council of Three.
pub fn conseil() -> Vec<Step> {
    vec![
        Step::Walk(Dir::Up),
        Step::Check("pg_sc02", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 121);
            assert_eq!(r.count(ITEM_FULL_RESTORE), 5);
            r.printed_in_order(&[Manga_Square_PG_Sc02_T1, Manga_Square_PG_Sc02_T6]);
        }),
    ]
}

/// Before Julien's tent (22, 10), at 121.
pub fn julien_start() -> Start {
    start_at(MAP_NEBULA, 22, 10, Dir::Up, |ctx| {
        Box::pin(async move { post_game_with(&ctx, 121, JULIEN_PARTY).await })
    })
}

/// Julien's six end on Primal Groudon (its Desolate Land voids Water moves, its sun fires
/// Solar Beam at once) and Entei: [`PARTY`] with a KYOGRE leading in the AGGRON's place and a
/// RAYQUAZA (Air Lock) in the PORYGON2's (the spine's battle policy never switches; Chapter 12's
/// Kyogre and Rayquaza are post-game catches).
const JULIEN_PARTY: &[(engine::data::SpeciesId, u8)] = &[
    (SPECIES_KYOGRE, 100),
    (SPECIES_ALAKAZAM, 100),
    (SPECIES_TYPHLOSION, 100),
    (SPECIES_AMPHAROS, 100),
    (SPECIES_RAYQUAZA, 100),
    (SPECIES_SWAMPERT, 100),
];

/// Julien: his scene, his rematch, the same day again (no), a new day (yes).
pub fn julien() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Interact(Up),
        Step::Check("pg_sc03", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 122);
            assert!(r.has_text(Nebula_PG_Sc03_T3));
        }),
        Step::Interact(Up),
        Step::Check("pg_sc03b", |r| {
            assert!(r.beaten(TRAINER_SIL_JULIEN_9));
            assert!(r.w.flag(FLAG_SIL_ITEM_PG_SALAMENCITE));
            assert_eq!(r.count(ITEM_SIL_SALAMENCITE), 1);
            assert!(r.w.flag(FLAG_SIL_DAILY_JULIEN));
            assert_eq!(r.battles.len(), 1);
        }),
        Step::Interact(Up),
        Step::Check("pg_sc03b_deja", |r| {
            assert!(r.has_text(Nebula_PG_Sc03b_T4));
            assert_eq!(r.battles.len(), 1);
        }),
        // A new day: the daily flags reset on the next map load (Benou's lab and back).
        Step::Days(1),
        goto(MAP_NEBULA, 11, 19),
        Step::Enter {
            dir: Up,
            map: MAP_BENOU_LAB,
        },
        Step::Enter {
            dir: Down,
            map: MAP_NEBULA,
        },
        Step::Check("new_day", |r| assert!(!r.w.flag(FLAG_SIL_DAILY_JULIEN))),
        goto(MAP_NEBULA, 22, 10),
        Step::Interact(Up),
        Step::Check("pg_sc03b_again", |r| {
            assert_eq!(r.battles.len(), 2);
            assert!(r.w.flag(FLAG_SIL_DAILY_JULIEN));
            assert_eq!(r.count(ITEM_SIL_SALAMENCITE), 1);
        }),
    ]
}

/// Below the lighthouse's stairs (12, 9), at 122.
pub fn phare_start() -> Start {
    start_at(MAP_REVERSE_CITY_PHARE, 12, 9, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 122).await })
    })
}

pub fn phare() -> Vec<Step> {
    vec![
        Step::Walk(Dir::Up),
        Step::Check("pg_sc04", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 123);
            assert!(r.has_text(Reverse_City_PG_Sc04_T1));
        }),
    ]
}

// ---- Act II: the Vestiges ----

/// The Bois des Soupirs' secret path's end (12, 20), at 123.
pub fn kimiko_start() -> Start {
    start_at(MAP_ROUTE_S06_BOIS_SOUPIRS, 12, 20, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 123).await })
    })
}

pub fn kimiko() -> Vec<Step> {
    vec![
        Step::Walk(Dir::Up),
        Step::Check("pg_sc05", |r| {
            assert!(r.beaten(TRAINER_SIL_KIMIKO_9));
            assert!(r.w.flag(FLAG_SIL_VESTIGE_KIMIKO));
            assert!(r.w.flag(FLAG_SIL_PARDON_KIMIKO));
            assert_eq!(r.w.var(VAR_SIL_STORY), 124);
        }),
    ]
}

/// On the wreck, west of Arthur (25, 7), at 124 (Kimiko's Vestige done).
pub fn arthur_start() -> Start {
    start_at(MAP_ROUTE_S05_EPAVE, 25, 7, Dir::Right, |ctx| {
        Box::pin(async move {
            post_game_at(&ctx, 124).await;
            ctx.set_flag(FLAG_SIL_VESTIGE_KIMIKO, true).await;
            ctx.set_flag(FLAG_SIL_PARDON_KIMIKO, true).await;
            ctx.set_var(VAR_SIL_PARDONS, 1).await;
        })
    })
}

pub fn arthur() -> Vec<Step> {
    vec![
        Step::Interact(Dir::Right),
        Step::Check("pg_sc06", |r| {
            assert!(r.beaten(TRAINER_SIL_ARTHUR_9));
            assert!(r.w.flag(FLAG_SIL_VESTIGE_ARTHUR));
            assert!(r.w.flag(FLAG_SIL_PARDON_ARTHUR));
            assert_eq!(r.w.var(VAR_SIL_STORY), 125);
        }),
    ]
}

/// Below the Point Zéro (33, 10), at 125 (two Vestiges done).
pub fn silas_start() -> Start {
    start_at(MAP_OWLYS_GROTTE_EST, 33, 10, Dir::Up, |ctx| {
        Box::pin(async move {
            post_game_at(&ctx, 125).await;
            for f in [
                FLAG_SIL_VESTIGE_KIMIKO,
                FLAG_SIL_PARDON_KIMIKO,
                FLAG_SIL_VESTIGE_ARTHUR,
                FLAG_SIL_PARDON_ARTHUR,
            ] {
                ctx.set_flag(f, true).await;
            }
            ctx.set_var(VAR_SIL_PARDONS, 2).await;
        })
    })
}

pub fn silas() -> Vec<Step> {
    vec![
        Step::Walk(Dir::Up),
        Step::Check("pg_sc07", |r| {
            assert!(r.beaten(TRAINER_SIL_SILAS_9));
            assert!(r.w.flag(FLAG_SIL_VESTIGE_SILAS));
            assert_eq!(r.w.var(VAR_SIL_STORY), 126);
            assert_eq!(r.w.var(VAR_SIL_PARDONS), 3);
            assert!(r.w.flag(FLAG_SIL_BRECHE_ALERTE));
            assert!(r.has_text(PG_VestigeFin_T1));
        }),
        // The machine: the Debug Key.
        goto(MAP_OWLYS_GROTTE_EST, 33, 5),
        Step::Interact(Dir::Up),
        Step::Check("cle_debug", |r| {
            assert!(r.w.flag(FLAG_SIL_CLE_DEBUG));
            assert_eq!(r.count(ITEM_SIL_KANGASKHANITE), 1);
        }),
    ]
}

/// At the bedroom's PC (6, 3), at 126 (the three Vestiges done).
pub fn mail_start() -> Start {
    start_at(HOUSE_2F, 6, 3, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 126).await })
    })
}

pub fn mail() -> Vec<Step> {
    vec![
        Step::Interact(Dir::Up),
        Step::Check("pg_sc09", |r| {
            assert!(r.w.flag(FLAG_SIL_MAIL_SILAS));
            assert_eq!(r.w.party().len(), 6);
            assert!(r.degraded.contains("give_egg"), "{:?}", r.degraded);
        }),
    ]
}

// ---- Act III: the Breach ----

/// Nébula's square (22, 14), at 126: the sky splits on arrival.
pub fn horde_start() -> Start {
    start_at(MAP_NEBULA, 22, 14, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 126).await })
    })
}

pub fn horde() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Settle,
        Step::Check("pg_sc10", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 127);
            assert_eq!(r.battles.len(), 2);
            r.printed_in_order(&[Nebula_PG_Sc10_T1, Nebula_PG_Sc10_T6]);
        }),
        // The crack is the Breach's entrance.
        goto(MAP_NEBULA, 18, 4),
        Step::Enter {
            dir: Up,
            map: MAP_BRECHE,
        },
    ]
}

/// The Tower's hall, before the elevator's door (12, 2), at 127.
pub fn tour_start() -> Start {
    start_at(HALL, 12, 2, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 127).await })
    })
}

/// The elevator's floors at 127 (no ticket): 1F, 2F, 3F, 4F, SOMMET.
const RUINE1: u8 = 0;
const STUDIO_FLOOR: u8 = 1;
const RUINE3: u8 = 2;
const RUINE4: u8 = 3;
const SOMMET: u8 = 4;

pub fn tour() -> Vec<Step> {
    use Dir::*;
    vec![
        // The hub: 2F, Gwada's Filter.
        Step::Menu(STUDIO_FLOOR),
        Step::Interact(Up),
        Step::Check("studio", |r| assert_eq!(r.w.map(), STUDIO)),
        goto(STUDIO, 14, 7),
        Step::Interact(Up),
        Step::Check("pg_sc11", |r| {
            assert!(r.w.flag(FLAG_SIL_MODULE_FILTRE));
            assert!(r.has_text(Tour_Nebula_2F_PG_Sc11_T5));
        }),
        // 1F: the gas, the component on the still (the cabin by the Studio's north door).
        goto(STUDIO, 20, 2),
        Step::Menu(RUINE1),
        Step::Enter {
            dir: Up,
            map: MAP_TOUR_NEBULA_1F_RUINE,
        },
        // The barrels (east only): the second (7, 10) into (8, 10), the first (10, 8) into
        // (11, 8), then the gas sheet (7, 4).
        goto(MAP_TOUR_NEBULA_1F_RUINE, 6, 10),
        Step::Interact(Right),
        goto(MAP_TOUR_NEBULA_1F_RUINE, 9, 8),
        Step::Interact(Right),
        goto(MAP_TOUR_NEBULA_1F_RUINE, 7, 5),
        Step::Interact(Up),
        goto(MAP_TOUR_NEBULA_1F_RUINE, 7, 3),
        Step::Interact(Up),
        Step::Check("composant_1", |r| assert!(r.w.flag(FLAG_SIL_COMPOSANT_1))),
        // 3F.
        goto(MAP_TOUR_NEBULA_1F_RUINE, 7, 13),
        Step::Menu(RUINE3),
        Step::Interact(Down),
        Step::Check("ruine3", |r| {
            assert_eq!(r.w.map(), MAP_TOUR_NEBULA_3F_RUINE)
        }),
        goto(MAP_TOUR_NEBULA_3F_RUINE, 1, 6),
        Step::Interact(Up),
        goto(MAP_TOUR_NEBULA_3F_RUINE, 8, 3),
        Step::Interact(Up),
        Step::Check("composant_2", |r| assert!(r.w.flag(FLAG_SIL_COMPOSANT_2))),
        // 4F.
        goto(MAP_TOUR_NEBULA_3F_RUINE, 8, 11),
        Step::Menu(RUINE4),
        Step::Interact(Down),
        Step::Check("ruine4", |r| {
            assert_eq!(r.w.map(), MAP_TOUR_NEBULA_4F_RUINE)
        }),
        goto(MAP_TOUR_NEBULA_4F_RUINE, 7, 5),
        Step::Interact(Up),
        goto(MAP_TOUR_NEBULA_4F_RUINE, 7, 2),
        Step::Interact(Up),
        Step::Check("composant_3", |r| assert!(r.w.flag(FLAG_SIL_COMPOSANT_3))),
        // Back to Gwada: Filter v2.
        goto(MAP_TOUR_NEBULA_4F_RUINE, 7, 13),
        Step::Menu(STUDIO_FLOOR),
        Step::Interact(Down),
        goto(STUDIO, 14, 7),
        Step::Interact(Up),
        Step::Check("pg_sc15", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 128);
            assert!(!r.w.flag(FLAG_SIL_HIDE_MOLTRES));
            assert!(!r.w.flag(FLAG_SIL_HIDE_LATIOS));
        }),
    ]
}

/// The Breach's camp (7, 20), at 128 (Filter v2).
pub fn dehors_start() -> Start {
    start_at(MAP_BRECHE, 7, 20, Dir::Up, |ctx| {
        Box::pin(async move {
            post_game_at(&ctx, 128).await;
            for f in [
                FLAG_SIL_MODULE_FILTRE,
                FLAG_SIL_COMPOSANT_1,
                FLAG_SIL_COMPOSANT_2,
                FLAG_SIL_COMPOSANT_3,
                FLAG_SIL_HIDE_GAZ_01,
            ] {
                ctx.set_flag(f, true).await;
            }
        })
    })
}

pub fn dehors() -> Vec<Step> {
    use Dir::*;
    vec![
        // Thomas's camp.
        Step::Interact(Up),
        Step::Check("pg_sc10b", |r| {
            assert!(r.w.flag(FLAG_SIL_ITEM_PG_HERACRONITE));
            assert!(r.healed());
        }),
        goto(MAP_BRECHE, 9, 17),
        Step::Interact(Up),
        goto(MAP_BRECHE, 9, 8),
        Step::Interact(Up),
        Step::Check("gaz_v2", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_GAZ_02));
            assert!(r.w.flag(FLAG_SIL_HIDE_GAZ_03));
        }),
        goto(MAP_BRECHE, 9, 0),
        Step::Enter { dir: Up, map: NML },
        // The first steps out.
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Check("pg_sc16", |r| {
            assert!(r.w.flag(FLAG_SIL_PG_DEHORS_VU));
            assert!(r.has_text(No_Mans_Land_PG_Sc16_T3));
        }),
        // The Relay: Poussin.
        goto(NML, 29, 38),
        Step::Walk(Up),
        Step::Check("pg_sc16b", |r| {
            assert!(r.w.flag(FLAG_SIL_RELAIS_ETEINT));
            assert_eq!(r.count(ITEM_SIL_ORBE_BLEU), 1);
            assert_eq!(r.count(ITEM_TM18), 1);
            assert!(!r.w.flag(FLAG_SIL_HIDE_LUGIA));
            assert!(!r.w.flag(FLAG_SIL_HIDE_KYOGRE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_APEX_POUSSIN_PG));
        }),
        // The three samples (the Centrale's way past gas 04).
        goto(NML, 24, 19),
        Step::Interact(Up),
        Step::Check("gaz_04", |r| assert!(r.w.flag(FLAG_SIL_HIDE_GAZ_04))),
        goto(NML, 17, 27),
        Step::Interact(Up),
        goto(NML, 11, 12),
        Step::Interact(Up),
        // The steel beam (37, 30) shut the way east: four pushes clear the gap.
        goto(NML, 36, 30),
        Step::Interact(Right),
        goto(NML, 37, 30),
        Step::Interact(Right),
        goto(NML, 38, 30),
        Step::Interact(Right),
        goto(NML, 39, 30),
        Step::Interact(Right),
        // The north-east field, past gas 05.
        goto(NML, 47, 19),
        Step::Interact(Up),
        Step::Check("gaz_05", |r| assert!(r.w.flag(FLAG_SIL_HIDE_GAZ_05))),
        goto(NML, 44, 8),
        Step::Interact(Up),
        Step::Check("pg_sc17", |r| {
            assert!(r.w.flag(FLAG_SIL_ECHANTILLON_1));
            assert!(r.w.flag(FLAG_SIL_ECHANTILLON_2));
            assert!(r.w.flag(FLAG_SIL_ECHANTILLON_3));
            assert_eq!(r.count(ITEM_SIL_ECHANTILLON_SOL), 3);
        }),
        // The Ice Vault: the door answers the samples.
        goto(NML, 53, 8),
        Step::Enter {
            dir: Up,
            map: MAP_NO_MANS_LAND_CAVEAU_GLACE,
        },
        goto(MAP_NO_MANS_LAND_CAVEAU_GLACE, 7, 3),
        Step::Interact(Up),
        Step::Check("caveau", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_CAVEAU_PORTE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_REGICE));
        }),
    ]
}

/// The Centrale's entrance (9, 12), at 128.
pub fn centrale_start() -> Start {
    start_at(MAP_NO_MANS_LAND_CENTRALE, 9, 12, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 128).await })
    })
}

/// The switches: B first (wrong, all reset), then A, B, C: the blast door opens, Zapdos.
pub fn centrale() -> Vec<Step> {
    use Dir::*;
    vec![
        goto(MAP_NO_MANS_LAND_CENTRALE, 12, 9),
        Step::Interact(Up),
        Step::Check("wrong_order", |r| {
            assert!(r.has_text(PG_Centrale_T2));
            assert!(!r.w.flag(FLAG_SIL_CENTRALE_B));
        }),
        goto(MAP_NO_MANS_LAND_CENTRALE, 2, 9),
        Step::Interact(Up),
        goto(MAP_NO_MANS_LAND_CENTRALE, 12, 9),
        Step::Interact(Up),
        goto(MAP_NO_MANS_LAND_CENTRALE, 16, 9),
        Step::Interact(Up),
        Step::Check("zapdos", |r| {
            assert!(r.w.flag(FLAG_SIL_HIDE_CENTRALE_PORTE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_ZAPDOS));
        }),
        goto(MAP_NO_MANS_LAND_CENTRALE, 9, 4),
        Step::Interact(Up),
        Step::Check("electhor", |r| {
            assert!(r.has_text(PG_Legendaire_Electhor));
            assert_eq!(r.battles.len(), 1);
        }),
    ]
}

/// Gwada at the Studio (14, 7), at 128 with the three samples.
pub fn balise_start() -> Start {
    start_at(STUDIO, 14, 7, Dir::Up, |ctx| {
        Box::pin(async move {
            post_game_at(&ctx, 128).await;
            for f in [
                FLAG_SIL_MODULE_FILTRE,
                FLAG_SIL_ECHANTILLON_1,
                FLAG_SIL_ECHANTILLON_2,
                FLAG_SIL_ECHANTILLON_3,
                FLAG_SIL_RELAIS_ETEINT,
            ] {
                ctx.set_flag(f, true).await;
            }
            ctx.set_flag(FLAG_SIL_HIDE_TOUR_2F_GWADA, false).await;
        })
    })
}

/// `Sc18`, then the Apex by the elevator: `Sc19`; then home: `Sc20`.
pub fn balise() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Interact(Up),
        Step::Check("pg_sc18", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 129);
            assert!(r.w.flag(FLAG_SIL_ITEM_PG_SCEPTILITE));
        }),
        goto(STUDIO, 20, 2),
        Step::Menu(SOMMET),
        Step::Enter { dir: Up, map: APEX },
        goto_avoiding(APEX, 7, 17, APEX_AVOID),
        Step::Walk(Up),
        Step::Check("pg_sc19", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 130);
            assert_eq!(r.w.var(VAR_SIL_SYNC), 0);
            assert!(r.w.flag(FLAG_SIL_FRAGMENTS_ACTIFS));
            assert!(!r.w.flag(FLAG_SIL_HIDE_SUICUNE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_MEW));
            assert!(!r.w.flag(FLAG_SIL_HIDE_HO_OH));
            r.printed_in_order(&[Sommet_Apex_PG_Sc19_T1, Sommet_Apex_PG_Sc19_T9]);
        }),
    ]
}

/// The Apex's elites' lines of sight (Chapter 11's), avoided.
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

/// Brico-Bourg, two steps south of the house's door (5, 17), at 130: Maman at the door.
pub fn fin_start() -> Start {
    start_at(MAP_BRICO_BOURG, 5, 17, Dir::Up, |ctx| {
        Box::pin(async move {
            post_game_at(&ctx, 130).await;
            ctx.set_flag(FLAG_SIL_FRAGMENTS_ACTIFS, true).await;
        })
    })
}

pub fn fin() -> Vec<Step> {
    vec![
        Step::Walk(Dir::Up),
        Step::Check("pg_sc20", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 131);
            assert!(r.has_text(Brico_Bourg_PG_Sc20_T2));
            assert!(r.has_text(Brico_Bourg_PG_Sc20_T4));
            assert_eq!(r.w.map(), HOUSE_2F);
        }),
    ]
}

// ---- the optional goals ----

/// Before Benou (6, 5), at 130 with the Debug Key.
pub fn benou_start() -> Start {
    start_at(MAP_BENOU_LAB, 6, 5, Dir::Up, |ctx| {
        Box::pin(async move {
            post_game_at(&ctx, 130).await;
            ctx.set_flag(FLAG_SIL_FRAGMENTS_ACTIFS, true).await;
        })
    })
}

/// Benou's battle and the Debug Key installed; again: the blast door; the Zone's Core: the Clone
/// (a copy of the party) and Mewtwo.
pub fn benou() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Interact(Up),
        Step::Check("pg_sc23", |r| {
            assert!(r.beaten(TRAINER_SIL_BENOU_9));
            assert!(r.w.flag(FLAG_SIL_BENOU_BATTU));
            assert!(r.w.flag(FLAG_SIL_CLE_DEBUG_INSTALLEE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_DEOXYS));
            assert_eq!(r.count(ITEM_SIL_PIERRE_LIEN), 2);
        }),
        Step::Interact(Up),
        Step::Check("zone404_ouverte", |r| {
            assert!(r.w.flag(FLAG_SIL_ZONE404_OUVERTE));
        }),
    ]
}

/// The Core (5, 7), at 130: the Clone's row is one step north.
pub fn noyau_start() -> Start {
    start_at(MAP_ZONE_404_NOYAU, 5, 7, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 130).await })
    })
}

pub fn noyau() -> Vec<Step> {
    vec![
        Step::Walk(Dir::Up),
        Step::Check("pg_sc26", |r| {
            assert!(r.w.flag(FLAG_SIL_CLONE_BATTU));
            assert!(!r.w.flag(FLAG_SIL_HIDE_MEWTWO));
            assert!(r.has_text(Zone_404_PG_Sc26_T2));
        }),
        goto(MAP_ZONE_404_NOYAU, 5, 3),
        Step::Interact(Dir::Up),
        Step::Check("pg_sc26b", |r| {
            assert!(r.has_text(Zone_404_PG_Sc26b_T1));
            assert_eq!(r.battles.len(), 2);
        }),
    ]
}

/// Before the Shaker's counter, at 121 (Crow), then the Lounge.
pub fn lounge_start() -> Start {
    start_at(MAP_BRICO_BOURG_BAR, 3, 4, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 121).await })
    })
}

pub fn lounge() -> Vec<Step> {
    use Dir::*;
    vec![
        Step::Interact(Up),
        Step::Check("pg_sc22_bar", |r| {
            assert!(r.w.flag(FLAG_SIL_LOUNGE_OUVERT));
            assert!(r.has_text(Brico_Bourg_Bar_PG_Sc22_T1));
        }),
        goto(MAP_BRICO_BOURG_BAR, 12, 2),
        Step::Enter {
            dir: Up,
            map: MAP_BRICO_BOURG_BAR_LOUNGE,
        },
        goto(MAP_BRICO_BOURG_BAR_LOUNGE, 9, 5),
        Step::Interact(Up),
        Step::Check("pg_sc22", |r| {
            assert!(r.beaten(TRAINER_SIL_CROW_9));
            assert!(r.w.flag(FLAG_SIL_CROW_RECOMPENSE));
            assert_eq!(r.count(ITEM_SHELL_BELL), 1);
            assert!(r.has_text(Brico_Bourg_Bar_PG_Sc22_T4b));
        }),
    ]
}

/// Before Samuel at the helm, at 121 with his badge.
pub fn revanche_start() -> Start {
    start_at(
        MAP_PORT_RESSAC_GYM,
        SAMUEL_FRONT.0,
        SAMUEL_FRONT.1,
        Dir::Up,
        |ctx| Box::pin(async move { post_game_at(&ctx, 121).await }),
    )
}

/// The cell south of Samuel (obj 1).
pub const SAMUEL_FRONT: (i16, i16) = (13, 4);

pub fn revanche() -> Vec<Step> {
    vec![
        Step::Interact(Dir::Up),
        Step::Check("pg_revanche", |r| {
            assert!(r.beaten(TRAINER_SIL_SAMUEL_9));
            assert_eq!(r.count(ITEM_SIL_BLAZIKENITE), 1);
            assert!(r.w.flag(FLAG_SIL_DAILY_SAMUEL));
        }),
        Step::Interact(Dir::Up),
        Step::Check("pg_revanche_deja", |r| {
            assert!(r.has_text(PG_Revanche_Deja))
        }),
    ]
}

/// Before the stele (4, 4), at 121.
pub fn stele_start() -> Start {
    start_at(MAP_ROUTE_S08_GROTTE_STELE, 4, 4, Dir::Up, |ctx| {
        Box::pin(async move { post_game_at(&ctx, 121).await })
    })
}

pub fn stele() -> Vec<Step> {
    vec![
        Step::Interact(Dir::Up),
        Step::Check("pg_sc28", |r| {
            assert!(r.w.flag(FLAG_SIL_FORME_ORIGINELLE));
            assert!(r.has_text(Route_S08_PG_Sc28_T3));
            assert!(r.degraded.contains("MOVE_SIL_CODE_AQUA"));
        }),
    ]
}

/// Before the Antenna Êta's counter (4, 13), the receptionist behind it (4, 11), at 121, a party of three.
pub fn antenne_start() -> Start {
    start_at(MAP_ROUTE_S07_ANTENNE, 4, 13, Dir::Up, |ctx| {
        Box::pin(async move {
            end_of_chapter_11(ctx.clone()).await;
            give_party(&ctx, &PARTY[..3]).await;
            ctx.set_var(VAR_SIL_STORY, 121).await;
        })
    })
}

pub fn antenne() -> Vec<Step> {
    vec![
        Step::Interact(Dir::Up),
        Step::Check("pg_antenne", |r| {
            assert!(r.beaten(TRAINER_SIL_ANTENNE_ETA_1));
            assert!(r.beaten(TRAINER_SIL_ANTENNE_ETA_2));
            assert!(r.beaten(TRAINER_SIL_ANTENNE_ETA_3));
            assert!(r.w.flag(FLAG_SIL_ANTENNE_ETA));
            assert_eq!(r.count(ITEM_SIL_MEDICHAMITE), 1);
            assert!(r.has_text(PG_Antenne_T4));
        }),
    ]
}

/// Three cells south of the first Code Fragment on Route S01 (11, 38), at 130.
pub fn fragment_start() -> Start {
    start_at(MAP_ROUTE_S01, 11, 38, Dir::Up, |ctx| {
        Box::pin(async move {
            post_game_at(&ctx, 130).await;
            ctx.set_flag(FLAG_SIL_FRAGMENTS_ACTIFS, true).await;
        })
    })
}

pub fn fragment() -> Vec<Step> {
    vec![
        Step::Walk(Dir::Up),
        Step::Check("bip", |r| assert!(r.has_text(PG_Fragment_T1))),
        Step::Walk(Dir::Up),
        Step::Interact(Dir::Up),
        Step::Check("pg_fragment", |r| {
            assert!(r.w.flag(FLAG_SIL_FRAGMENT_1));
            assert_eq!(r.w.var(VAR_SIL_FRAGMENTS), 1);
            r.printed_in_order(&[PG_Fragment_T2]);
        }),
    ]
}

/// What every segment's run must keep.
pub fn check_clean(r: &Run) {
    assert_eq!(r.w.events_dropped(), 0);
}
