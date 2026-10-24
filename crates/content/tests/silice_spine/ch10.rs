//! Chapter 10, the Nébula Tower, from the elevator (`VAR_SIL_STORY` 99) to the vault elevator
//! up to the Apex (109):
//! - the cabin's first climb (`Tour_Ascenseur_Sc00a`, 100);
//! - 1F, the Cave: the guard, the east barrel pushed off its pontoon (Champ Magnétique), the
//!   maître's barrel C (the grille), the Cœur's pulse (three Persim Berries), Mathias (ACCÈS
//!   ALPHA, 85 %, 101), the door behind the counter;
//! - the cabin (Mathieu's call, 102); 2F, the Studio: the consoles in the order pink, violet,
//!   gold (the set cut), Kimiko (ACCÈS BETA, 90 %, 103), the artists' exit;
//! - the cabin (Mathieu's meal, the Tower's only heal, 104); 3F, the Horizon: the compass taken,
//!   the hidden path across the virtual sea, Arthur (the compass given back: no items, the origami
//!   boat; ACCÈS GAMMA, 95 %, 105), the screen behind him;
//! - the cabin (Gwada's call, 106); 4F, the Reflets: the prisms p1 WEST (three turns) and p3
//!   SOUTH (two turns, from its pocket behind the false mirror), the prism door, Revenge (the
//!   Badge Miroir, 99 %, 107–108), the threshold (`Tour_4F_Sc06`: the scripted save, yes; up,
//!   yes; 109) and the vault elevator to the Apex.
//!
//! **Start** ([`start`]): the cabin's cell from the hall (3, 6), where Chapter 9 hands off (the
//! hall's elevator door, §5.7's proposal), with Chapter 8's end state ([`end_of_chapter_9`]: no
//! Chapter 9 flag is read by Chapters 10–11) and the story at 99. To be rechained on Chapter 9's
//! end once it lands.
//!
//! **The party** ([`PARTY`]): a constructed Chapter 9 end, not Chapter 8's as `sil9_ch8` leaves
//! it. With Chapter 8's roster (KADABRA 69, TYPHLOSION 76 and four members under 36) the
//! Tower's leaders, six 240-IV mons each with items, Mega Evolutions and Revenge's prism, are
//! out of the battle policy's reach at any level: Kimiko's HO-OH (Calm Mind, Recover, Leftovers)
//! beat KADABRA / TYPHLOSION 88/92, Arthur's rain team 90/94. The spine needs six members:
//! ALAKAZAM 89 (Chapter 8's Link Stone), TYPHLOSION 91, AMPHAROS 89 (the MAREEP trained, with
//! Chapter 9's TM24 Thunderbolt), AGGRON 89 (the LAIRON), DUGTRIO 89, PORYGON2 89. Measured
//! (`SPINE_PARTY_<slot>`): 87–91 for the five and 89–93 for the TYPHLOSION all win; 85/88 loses
//! to Revenge. Chapter 9's rechain has to train that far (Route S9's grass, 45–46) or the author
//! has to tune the Tower's rosters down (a plan author question). The Tower has no grass and one
//! heal (Mathieu's meal, 104): between two leaders the run mends with the bag's medicine
//! ([`Step::Mend`]: Revives and potions, Chapter 9's marts).
//!
//! **The fallbacks the path relies on**: Strength is an A press on the barrel; the hidden items
//! are signs (not taken here); Kimiko's send-out lines and her "émotion" are not played. The
//! Mega lines, ARTHUR_1's Full Restores, the Cœur's ward on 1F and Revenge's prism are the
//! battles' own rules (`silice::rules`).

use super::{Run, Start, Step, TeachTm, goto, goto_avoiding};
use content::consts::*;
use engine::data::{Dir, ItemId, MapId, SpeciesId};
use engine::script::Ctx;

const CABIN: MapId = MAP_TOUR_NEBULA_ASCENSEUR;
const F1: MapId = MAP_TOUR_NEBULA_1F;
const F2: MapId = MAP_TOUR_NEBULA_2F;
const F3: MapId = MAP_TOUR_NEBULA_3F;
const F4: MapId = MAP_TOUR_NEBULA_4F;
const APEX: MapId = MAP_SOMMET_APEX;

/// The cabin's cell from the hall (3, 6), facing north, Chapter 9's end ([`end_of_chapter_9`]).
pub fn start() -> Start {
    Start {
        map: CABIN,
        x: 3,
        y: 6,
        facing: Dir::Up,
        setup: |ctx| Box::pin(end_of_chapter_9(ctx)),
    }
}

/// The start state's party (see the module docs).
pub const PARTY: &[(SpeciesId, u8)] = &[
    (SPECIES_ALAKAZAM, 89),
    (SPECIES_TYPHLOSION, 91),
    (SPECIES_AMPHAROS, 89),
    (SPECIES_AGGRON, 89),
    (SPECIES_DUGTRIO, 89),
    (SPECIES_PORYGON2, 89),
];

/// Chapter 9's end, as far as Chapters 10–11 read it ([`end_of_chapter_9_flags`]), with the
/// party ([`PARTY`]) and the two TMs the spine teaches first.
pub async fn end_of_chapter_9(ctx: Ctx) {
    end_of_chapter_9_flags(ctx.clone()).await;
    give_party(&ctx, PARTY).await;
}

/// TM34 (the lead's Shock Wave) and TM24 (the AMPHAROS's Thunderbolt) in the bag, then `party`
/// (the start state cannot set moves: the spines teach both first, [`TEACH`]).
pub async fn give_party(ctx: &Ctx, party: &[(SpeciesId, u8)]) {
    ctx.item_add(ItemId(ITEM_TM34), 1).await;
    ctx.item_add(ItemId(ITEM_TM24), 1).await;
    for (i, &(species, level)) in party.iter().enumerate() {
        ctx.give_mon(species, party_level(i, level)).await;
    }
}

/// The two TMs of [`give_party`]: Shock Wave over the ALAKAZAM's Role Play, Thunderbolt over
/// the AMPHAROS's first move.
pub const TEACH: [Step; 2] = [
    Step::Teach(TeachTm {
        item: ITEM_TM34,
        mon: 0,
        forget: 1,
    }),
    Step::Teach(TeachTm {
        item: ITEM_TM24,
        mon: 2,
        forget: 0,
    }),
];

/// Chapter 9's end without the party: Chapter 7's end (`ch08`'s, without its bag and party),
/// Chapter 8's badge, Sonde de Pression, Cœur d'Évalon and log, the sync at 80, the story at 99;
/// medicine (Chapter 9's marts and finds), the respawn at Évalon (Nébula's centre is Chapter 9's).
pub async fn end_of_chapter_9_flags(ctx: Ctx) {
    super::ch08::end_of_chapter_7_flags(ctx.clone()).await;
    for f in [
        FLAG_SIL_BADGE_08,
        FLAG_SIL_MODULE_PLONGEE,
        FLAG_SIL_COEUR_EVALON,
        FLAG_SIL_LOG_08,
    ] {
        ctx.set_flag(f, true).await;
    }
    ctx.set_var(VAR_SIL_STORY, 99).await;
    ctx.set_var(VAR_SIL_SYNC, 80).await;
    ctx.item_add(ItemId(ITEM_HYPER_POTION), 8).await;
    ctx.item_add(ItemId(ITEM_REVIVE), 4).await;
    ctx.item_add(ItemId(ITEM_FULL_HEAL), 3).await;
    ctx.money_add(30000).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_EVALON).await;
}

/// A tuning aid: `SPINE_PARTY_<slot>=<level>` overrides [`PARTY`]'s level (the neighbour rule's
/// measures, as `SPINE_LEVEL_<l>` for training).
pub fn party_level(slot: usize, level: u8) -> u8 {
    std::env::var(format!("SPINE_PARTY_{slot}"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(level)
}

/// Coralie's (3, 9) and Gaston's (3, 14) lines of sight, 4 cells east.
const F1_AVOID: &[(i16, i16)] = &[
    (4, 9),
    (5, 9),
    (6, 9),
    (7, 9),
    (4, 14),
    (5, 14),
    (6, 14),
    (7, 14),
];
/// Basile's (5, 8) and Lou's (16, 8), 4 cells south.
const F2_AVOID: &[(i16, i16)] = &[
    (5, 9),
    (5, 10),
    (5, 11),
    (5, 12),
    (16, 9),
    (16, 10),
    (16, 11),
    (16, 12),
];
/// Yann's (3, 13) east and Maëlle's (16, 13) west, 4 cells.
const F3_AVOID: &[(i16, i16)] = &[
    (4, 13),
    (5, 13),
    (6, 13),
    (7, 13),
    (12, 13),
    (13, 13),
    (14, 13),
    (15, 13),
];
/// Mira's (3, 7) east and Écho's (13, 9) west, 4 cells.
const F4_AVOID: &[(i16, i16)] = &[
    (4, 7),
    (5, 7),
    (6, 7),
    (7, 7),
    (9, 9),
    (10, 9),
    (11, 9),
    (12, 9),
];

/// The chapter's steps, from [`start`].
pub fn steps() -> Vec<Step> {
    use Dir::*;
    let mut s = TEACH.to_vec();
    s.extend([
        // ---- the first climb (Sc00a), 1F ----
        Step::Settle,
        Step::Check("first_climb", |r| {
            assert_eq!(r.w.map(), F1);
            assert_eq!(r.w.var(VAR_SIL_STORY), 100);
            r.printed_in_order(&[Tour_Ascenseur_Sc00a_T1, Tour_Ascenseur_Sc00a_T2]);
        }),
        Step::Walk(Up),
        Step::Check("cave_intro", |r| {
            assert!(r.w.flag(FLAG_SIL_TOUR_1F_INTRO));
            assert!(r.has_text(Tour_1F_Sc01_T2));
        }),
        // The east barrel (17, 10), pushed north off its pontoon from (17, 11).
        goto_avoiding(F1, 17, 12, F1_AVOID),
        Step::Walk(Up),
        Step::Interact(Up),
        Step::Check("barrel", |r| {
            assert!(r.degraded.contains("strength"));
            assert_ne!(r.w.var(VAR_TEMP_5), 0, "the barrel moved");
        }),
        goto_avoiding(F1, 13, 8, F1_AVOID),
        Step::Interact(Up),
        Step::Check("grille", |r| {
            assert!(r.w.flag(FLAG_SIL_CAVE_GRILLE_OUVERTE));
            r.printed_in_order(&[Tour_1F_Sc02_T6, Tour_1F_Sc02_T7, Tour_1F_Sc02_T8]);
        }),
        goto(F1, 12, 6),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Check("pulse", |r| {
            assert!(r.w.flag(FLAG_SIL_TOUR_COEUR_PULSE));
            assert!(r.count(ITEM_PERSIM_BERRY) >= 3);
        }),
        Step::Interact(Up),
        Step::Check("mathias", |r| {
            assert!(r.beaten(TRAINER_SIL_MATHIAS_1));
            assert!(r.w.flag(FLAG_SIL_KEY_ACCES_ALPHA));
            assert_eq!(r.w.var(VAR_SIL_STORY), 101);
            assert_eq!(r.w.var(VAR_SIL_SYNC), 85);
        }),
        Step::Mend(6),
        goto(F1, 16, 2),
        Step::Enter { dir: Up, map: F2 },
        // ---- 2F, the Studio ----
        Step::Check("mathieu_call", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 102);
            assert!(r.has_text(Tour_Ascenseur_Sc01_T1));
        }),
        Step::Walk(Up),
        Step::Check("studio_intro", |r| {
            assert!(r.w.flag(FLAG_SIL_TOUR_2F_INTRO));
        }),
        goto_avoiding(F2, 14, 6, F2_AVOID),
        Step::Interact(Up),
        goto_avoiding(F2, 20, 6, F2_AVOID),
        Step::Interact(Up),
        Step::Check("set_still_closed", |r| {
            assert_eq!(r.w.var(VAR_SIL_TOUR_STUDIO_STATE), 2);
            assert!(!r.w.flag(FLAG_SIL_STUDIO_DECOR_COUPE));
        }),
        goto_avoiding(F2, 4, 6, F2_AVOID),
        Step::Interact(Up),
        Step::Check("set_cut", |r| {
            assert!(r.w.flag(FLAG_SIL_STUDIO_DECOR_COUPE));
            r.printed_in_order(&[Tour_2F_Sc03_T1, Tour_2F_Sc03_T2, Tour_2F_Sc03_T3]);
        }),
        goto_avoiding(F2, 11, 4, F2_AVOID),
        Step::Interact(Up),
        Step::Check("kimiko", |r| {
            assert!(r.beaten(TRAINER_SIL_KIMIKO_1));
            assert!(r.w.flag(FLAG_SIL_KEY_ACCES_BETA));
            assert_eq!(r.w.var(VAR_SIL_STORY), 103);
            assert_eq!(r.w.var(VAR_SIL_SYNC), 90);
        }),
        Step::Mend(6),
        goto(F2, 20, 2),
        Step::Enter { dir: Up, map: F3 },
        // ---- 3F, the Horizon ----
        Step::Check("mathieu_meal", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 104);
            assert!(r.w.flag(FLAG_SIL_TOUR_RAVITAILLEMENT));
            assert!(r.healed());
        }),
        Step::Walk(Up),
        goto_avoiding(F3, 9, 13, F3_AVOID),
        Step::Interact(Left),
        Step::Check("compass", |r| {
            assert!(r.w.flag(FLAG_SIL_ARTHUR_BOUSSOLE_PRISE));
            assert_eq!(r.count(ITEM_SIL_BOUSSOLE_ARTHUR), 1);
        }),
        // The quay's edge (11, 12), then the sea's first path cell (row 11 is falls but x 11).
        goto_avoiding(F3, 11, 12, F3_AVOID),
        Step::Walk(Up),
    ]);
    // The hidden path: north 4, east 3, north 3, west 3, north to the helm's foot (11, 3).
    for d in [Up, Up, Up, Up, Right, Right, Right, Up, Up, Up, Left, Left, Left] {
        s.push(Step::Walk(d));
    }
    s.extend([
        Step::Check("helm", |r| {
            assert_eq!(r.pos(), (11, 4));
            assert!(!r.has_text(Tour_3F_Sc03_T1), "no fall");
        }),
        Step::Walk(Up),
        Step::Interact(Up),
        Step::Check("arthur", |r| {
            assert!(r.w.flag(FLAG_SIL_ARTHUR_BOUSSOLE_RENDUE));
            assert!(r.beaten(TRAINER_SIL_ARTHUR_2));
            assert!(r.w.flag(FLAG_SIL_KEY_ACCES_GAMMA));
            assert_eq!(r.count(ITEM_SIL_BATEAU_ORIGAMI), 1);
            assert_eq!(r.w.var(VAR_SIL_STORY), 105);
            assert_eq!(r.w.var(VAR_SIL_SYNC), 95);
        }),
        Step::Mend(6),
        goto(F3, 12, 2),
        Step::Enter { dir: Up, map: F4 },
        // ---- 4F, the Reflets ----
        Step::Check("gwada_call", |r| {
            assert_eq!(r.w.var(VAR_SIL_STORY), 106);
            assert!(r.has_text(Tour_Ascenseur_Sc03_T2));
        }),
        Step::Walk(Right),
        Step::Settle,
        Step::Check("reflets_intro", |r| {
            assert!(r.w.flag(FLAG_SIL_TOUR_4F_INTRO));
        }),
        // p1 (2, 19), read from (3, 19): three quarter turns, NORTH to WEST.
        goto_avoiding(F4, 3, 19, F4_AVOID),
        Step::Interact(Left),
        Step::Interact(Left),
        Step::Interact(Left),
        // p3 (5, 12), from its pocket (5, 11): two turns, NORTH to SOUTH; the door cracks.
        goto_avoiding(F4, 17, 11, F4_AVOID),
        goto_avoiding(F4, 11, 11, F4_AVOID),
        goto_avoiding(F4, 5, 11, F4_AVOID),
        Step::Interact(Down),
        Step::Interact(Down),
        Step::Check("prisms", |r| {
            assert_eq!(r.w.var(VAR_SIL_TOUR_PRISME_1), 3);
            assert_eq!(r.w.var(VAR_SIL_TOUR_PRISME_2), 0);
            assert_eq!(r.w.var(VAR_SIL_TOUR_PRISME_3), 2);
            assert!(r.w.flag(FLAG_SIL_REFLETS_PORTE_OUVERTE));
        }),
        Step::Mend(6),
        goto_avoiding(F4, 11, 5, F4_AVOID),
        Step::Walk(Up),
        Step::Check("revenge", |r| {
            assert!(r.beaten(TRAINER_SIL_REVENGE_1));
            assert!(r.w.flag(FLAG_SIL_BADGE_MIROIR_RESTORED));
            assert_eq!(r.w.var(VAR_SIL_SYNC), 99);
            assert_eq!(r.w.var(VAR_SIL_STORY), 108);
            assert!(r.w.flag(FLAG_SIL_HIDE_TOUR_REVENGE));
            assert!(!r.w.flag(FLAG_SIL_HIDE_REVENGE_STATUE));
        }),
        // The threshold: the coord row (10–12, 1), west of the statue.
        goto(F4, 10, 3),
        Step::Walk(Up),
        Step::Walk(Up),
        Step::Settle,
    ]);
    s
}

/// What the end of Chapter 10 asserts beyond its checkpoints: the scripted save, the point of
/// no return, the Apex's arrival (Chapter 11's `Sc01` plays on it), and the fallbacks reported.
pub fn check_end(r: &Run) {
    assert_eq!(r.w.map(), APEX);
    assert!(r.w.flag(FLAG_SIL_TOUR_SEUIL_VU));
    assert!(r.has_text(Tour_4F_Sc06_T3y), "the scripted save");
    assert!(r.has_text(Tour_Ascenseur_Sc04_T1));
    assert!(r.w.var(VAR_SIL_STORY) >= 109);
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "strength",
        "mallette_call",
        "show_sync",
        "battle_lines",
        "kimiko_emotion",
        "play_se",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
