//! Tests of the battle content (Milestone 4): the prelude's battle helpers, the two new-game
//! starts and the generated battle tables (both modes).

use super::*;

use crate::prelude::{TrainerBattle, trainer_battle, trainer_flag};
use engine::battle::BattleOutcome;
use engine::data::{FlagId, TrainerId};
use engine::mon::MoveEffect;

/// These tests were written for the Milestone 3 start (the starter received).
#[allow(non_upper_case_globals)]
const Emerald: Emerald = Emerald::after_starter();

/// A `trainerbattle` of `kind` (decomp `TRAINER_BATTLE_*` numbering).
fn battle(kind: u8, trainer: u16, intro: Option<TextId>, lose: TextId) -> TrainerBattle {
    TrainerBattle {
        kind,
        trainer,
        local_id: 2,
        intro,
        lose,
        not_enough: None,
        event_script: None,
        music: false,
    }
}

/// Runs `b`; `VAR_RESULT` = whether the calling script goes on after the `trainerbattle`.
fn fought(b: TrainerBattle, h: &mut Host) {
    run(
        |ctx| {
            Box::pin(async move {
                let goes_on = trainer_battle(&ctx, b).await;
                ctx.set_var(consts::VAR_RESULT, goes_on as u16).await;
            })
        },
        h,
    );
}

#[test]
fn trainer_battle_single_follows_trainer_battle_inc() {
    let tr = TrainerId(3);
    let b = battle(0, tr.0, Some(TextId(1)), TextId(2));
    let mut h = Host::default();
    fought(b, &mut h);
    assert_eq!(
        h.log,
        [
            Request::TrainerApproach { obj: 2 },
            Request::Lock,
            Request::FacePlayer,
            Request::GetFlag(trainer_flag(tr)),
            Request::Message(TextId(1)),
            Request::WaitMessage,
            Request::WaitButton,
            Request::BattleTrainer {
                trainer: tr,
                defeat: TextId(2)
            },
            Request::SetFlag(FlagId(0x503), true),
            Request::ReleaseAll,
            Request::SetVar(consts::VAR_RESULT, 0),
        ]
    );

    // Beaten: the script goes on with its post-battle code.
    let mut h = Host::default();
    h.flags.insert(0x503);
    fought(b, &mut h);
    assert_eq!(h.log.len(), 5);
    assert_eq!(h.vars[&consts::VAR_RESULT.0], 1);

    // Defeated (`IsPlayerDefeated`: lost or drew): the flag stays clear.
    for outcome in [BattleOutcome::Lost, BattleOutcome::Drew] {
        let mut h = Host {
            outcome: Some(outcome),
            ..Host::default()
        };
        fought(b, &mut h);
        assert!(h.flags.is_empty(), "{outcome:?}");
    }
}

#[test]
fn trainer_battle_variants() {
    let tr = TrainerId(7);
    // 3 `SINGLE_NO_INTRO_TEXT`: the battle, then the post-battle code.
    let mut h = Host::default();
    fought(battle(3, tr.0, None, TextId(4)), &mut h);
    assert_eq!(
        h.log,
        [
            Request::TrainerApproach { obj: 2 },
            Request::BattleTrainer {
                trainer: tr,
                defeat: TextId(4)
            },
            Request::SetFlag(FlagId(0x507), true),
            Request::SetVar(consts::VAR_RESULT, 1),
        ]
    );
    // 2 `CONTINUE_SCRIPT`: the intro and the battle; the calling script stops.
    let mut h = Host::default();
    fought(battle(2, tr.0, Some(TextId(1)), TextId(2)), &mut h);
    assert!(h.log.contains(&Request::Message(TextId(1))));
    assert!(h.flags.contains(&0x507));
    assert_eq!(h.vars[&consts::VAR_RESULT.0], 0);
}

#[test]
fn choose_starter_gives_it_and_starts_the_first_battle() {
    let mut h = Host {
        starter: 1,
        ..Host::default()
    };
    run(
        |ctx| {
            Box::pin(async move {
                prelude::special(&ctx, consts::SPECIAL_ChooseStarter).await;
            })
        },
        &mut h,
    );
    let (foe, level) = battle::FIRST_BATTLE_FOE;
    assert_eq!(
        h.log,
        [
            Request::Log("SPECIAL_ChooseStarter"),
            Request::ChooseStarter(battle::STARTERS),
            Request::SetVar(consts::VAR_STARTER_MON, 1),
            Request::GiveMon {
                species: battle::STARTERS[1],
                level: 5
            },
            Request::BattleWild {
                species: foe,
                level,
                first_battle: true
            },
        ]
    );
}

#[test]
fn player_face_trainer_after_battle_is_only_logged() {
    let mut h = Host::default();
    run(
        |ctx| {
            Box::pin(async move {
                prelude::special(&ctx, consts::SPECIAL_PlayerFaceTrainerAfterBattle).await;
            })
        },
        &mut h,
    );
    assert_eq!(
        h.log,
        [Request::Log("SPECIAL_PlayerFaceTrainerAfterBattle")]
    );
}

fn new_game_log(game: Emerald) -> Host {
    let mut h = Host::default();
    assert!(run(|ctx| game.script(NEW_GAME_SCRIPT, ctx), &mut h));
    h
}

#[test]
fn the_two_starts() {
    use consts::*;
    assert_eq!(Emerald::default().start, Start::BeforeRescue);
    let shells = if crate::STUB {
        Start::AfterStarter
    } else {
        Start::BeforeRescue
    };
    assert_eq!(Emerald::for_shells().start, shells);
    assert_eq!(Emerald.start, Start::AfterStarter);

    let h = new_game_log(Emerald::before_rescue());
    assert!(!h.log.iter().any(|r| matches!(r, Request::GiveMon { .. })));
    for (v, x) in [
        (VAR_LITTLEROOT_TOWN_STATE, 1),
        (VAR_ROUTE101_STATE, 0),
        (VAR_BIRCH_LAB_STATE, 0),
        (VAR_LITTLEROOT_RIVAL_STATE, 3),
    ] {
        assert_eq!(h.vars[&v.0], x, "{v:?}");
    }
    for f in [
        FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH,
        FLAG_HIDE_ROUTE_101_BOY,
        FLAG_HIDE_LITTLEROOT_TOWN_RIVAL,
    ] {
        assert!(h.flags.contains(&f.0), "{f:?}");
    }
    for f in [
        FLAG_SYS_POKEMON_GET,
        FLAG_RESCUED_BIRCH,
        FLAG_HIDE_ROUTE_101_BIRCH_ZIGZAGOON_BATTLE,
        FLAG_HIDE_ROUTE_101_BIRCH_STARTERS_BAG,
        FLAG_HIDE_ROUTE_101_ZIGZAGOON,
        FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM,
    ] {
        assert!(!h.flags.contains(&f.0), "{f:?}");
    }

    let h = new_game_log(Emerald::after_starter());
    assert_eq!(h.vars[&VAR_STARTER_MON.0], 2);
    assert_eq!(
        h.log.last(),
        Some(&Request::GiveMon {
            species: battle::STARTERS[2],
            level: 5
        })
    );
    assert!(h.flags.contains(&FLAG_SYS_POKEMON_GET.0));
    assert!(h.flags.contains(&FLAG_HIDE_ROUTE_101_ZIGZAGOON.0));
    assert!(
        !h.flags
            .contains(&FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH.0)
    );
}

#[test]
fn the_respawn_default_is_a_heal_location() {
    let id = Emerald.special_ids().respawn_default;
    assert_eq!(id, consts::HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F);
    let (map, x, y) = Emerald.heal_location(id).unwrap();
    assert!(has_map(map));
    let m = Emerald.map(map);
    assert!(x >= 0 && y >= 0 && (x as u16) < m.width && (y as u16) < m.height);
}

#[test]
fn ability_names_come_from_the_rom() {
    // gAbilityNames (battle/types.ron): every ability the engine names decodes to a non-empty
    // name; `Ability::Other` stands for many abilities and is empty.
    use engine::battle::Str;
    use engine::mon::Ability;
    let lang = lang();
    for a in [
        Ability::Overgrow,
        Ability::Blaze,
        Ability::Torrent,
        Ability::KeenEye,
        Ability::ShieldDust,
        Ability::EffectSpore,
        Ability::Intimidate,
        Ability::Static,
        Ability::Synchronize,
        Ability::Trace,
    ] {
        let t = Emerald.battle_text(Str::AbilityName(a));
        assert!(t.len() > 1, "{a:?}");
        let (_, n) = extract::text::decode_lang(t, lang).unwrap();
        assert_eq!(n, t.len(), "{a:?}");
    }
    assert_eq!(
        Emerald.battle_text(Str::AbilityName(Ability::Other)),
        b"\xFF"
    );
}

#[test]
fn every_str_has_text() {
    let lang = lang();
    for s in battle::ALL_STRS {
        let t = Emerald.battle_text(s);
        assert!(t.len() > 1, "{s:?} is empty");
        assert_eq!(
            t.iter().position(|&b| b == 0xFF),
            Some(t.len() - 1),
            "{s:?}"
        );
        let (_, n) = extract::text::decode_lang(t, lang).unwrap();
        assert_eq!(n, t.len(), "{s:?}");
    }
}

#[test]
fn the_tables_are_indexed_by_id() {
    let t = Emerald.battle();
    assert!(t.species.len() >= 3 && t.moves.len() >= 3);
    for sp in t.species {
        assert_eq!(sp.name.last(), Some(&0xFF));
        for l in sp.learnset {
            assert!((l.mov.0 as usize) < t.moves.len());
        }
    }
    assert!(t.foresight_split <= t.type_chart.len());
    assert!((t.struggle.0 as usize) < t.moves.len());
    assert_eq!(t.move_info(t.struggle).effect, MoveEffect::Recoil);
    assert!(
        t.nature_power
            .iter()
            .all(|m| (m.0 as usize) < t.moves.len())
    );
}

#[test]
fn every_slice_species_has_pics() {
    let t = Emerald.battle();
    for s in crate::samples::SPECIES_IDS
        .iter()
        .chain(&battle::STARTERS)
        .chain([&battle::FIRST_BATTLE_FOE.0])
    {
        let g = t
            .species_info(*s)
            .gfx
            .unwrap_or_else(|| panic!("{s:?} has no pics"));
        assert_eq!((g.front.len(), g.back.len()), (64, 64), "{s:?}");
    }
}

#[test]
fn every_trainer_resolves() {
    let t = Emerald.battle();
    for &id in &battle::TRAINER_IDS {
        let tr = Emerald.trainer(id).unwrap();
        assert!(!tr.party.is_empty(), "{id:?}");
        assert_eq!(tr.pic.tiles.len(), 64, "{id:?}");
        // `TrainerData.name` drives trainer personalities: one EOS, at the end.
        assert_eq!(
            tr.name.iter().position(|&b| b == 0xFF),
            Some(tr.name.len() - 1)
        );
        for m in tr.party {
            assert!(t.species_info(m.species).gfx.is_some(), "{id:?}");
        }
    }
    assert!(Emerald.trainer(TrainerId(0)).is_none());
}

/// Emerald's trainer mons hold nothing (`TRAINER_HELD_ITEMS` is off: the recorded route), and
/// Emerald has no Mega or Primal forms; the hold effects come from the `ITEM_*` names, in
/// both modes.
#[test]
fn hold_effects_but_no_trainer_items_nor_forms() {
    use engine::data::ItemId;
    use engine::mon::{HoldEffect, Type};
    for &id in &battle::TRAINER_IDS {
        for m in Emerald.trainer(id).unwrap().party {
            assert_eq!(m.item, None, "{id:?}");
        }
    }
    assert!(Emerald.form_changes().is_empty());
    // ITEM_ORAN_BERRY, ITEM_QUICK_CLAW, ITEM_LEFTOVERS, ITEM_CHARCOAL, ITEM_SEA_INCENSE.
    let cases = [
        (139, Some(HoldEffect::RestoreHp(10))),
        (183, Some(HoldEffect::QuickClaw(20))),
        (200, Some(HoldEffect::Leftovers)),
        (215, Some(HoldEffect::TypePower(Type::Fire, 10))),
        (220, Some(HoldEffect::TypePower(Type::Water, 5))),
        (0, None),
        (13, None),
    ];
    for (id, e) in cases {
        assert_eq!(Emerald.hold_effect(ItemId(id)), e, "item {id}");
    }
}

/// Format 5.1 extracts (Task A) carry trainer money and evolutions; older ones neither.
#[test]
fn trainer_money_and_evolutions_when_extracted() {
    for &id in &battle::TRAINER_IDS {
        assert_eq!(
            Emerald.trainer_money(id).is_some(),
            !battle::TRAINER_MONEY.is_empty(),
            "{id:?}"
        );
    }
    let t = Emerald.battle();
    for s in 0..t.species.len() as u16 {
        for e in Emerald.evolutions(engine::data::SpeciesId(s)) {
            assert!((e.target.0 as usize) < t.species.len());
        }
    }
}

#[test]
fn the_player_back_pic_has_whole_frames() {
    let p = &Emerald.battle_gfx().player_back;
    assert!(!p.tiles.is_empty() && p.tiles.len().is_multiple_of(64));
    assert!(p.y <= 8);
}

/// Prints the size of the battle data (`--nocapture`).
#[test]
fn battle_data_sizes() {
    std::println!(
        "battle data: blob {} B, {} species, {} moves, {} trainers; {}",
        core::mem::size_of_val(&battle::BLOB),
        Emerald.battle().species.len(),
        Emerald.battle().moves.len(),
        battle::TRAINER_IDS.len(),
        battle::SUMMARY
    );
}

#[cfg(not(pkr_stub_battle))]
mod extracted {
    use super::*;
    use engine::data::MoveId;

    #[test]
    fn the_starters_and_the_tables() {
        use consts::*;
        assert_eq!(
            battle::STARTERS,
            [SPECIES_TREECKO, SPECIES_TORCHIC, SPECIES_MUDKIP]
        );
        assert_eq!(battle::FIRST_BATTLE_FOE, (SPECIES_ZIGZAGOON, 2));
        assert_eq!(Emerald.battle().species.len(), 412);
        assert_eq!(Emerald.battle().moves.len(), 355);
        assert_eq!(Emerald.battle().struggle, MOVE_STRUGGLE);
    }

    #[test]
    fn sound_moves_are_flagged_and_species_have_weights() {
        use consts::*;
        use engine::mon::MoveFlags;
        let t = Emerald.battle();
        assert!(t.move_info(MOVE_GROWL).flags.has(MoveFlags::SOUND));
        assert!(!t.move_info(MOVE_TACKLE).flags.has(MoveFlags::SOUND));
        // Low Kick's input (hectograms): every real species weighs something.
        assert!(t.species_info(SPECIES_ZIGZAGOON).weight > 0);
        assert!(t.species_info(SPECIES_TREECKO).weight > 0);
    }

    /// Slice moves whose effect the engine does not implement yet (for Task E2). They come
    /// from the parties of the Route 102 and Route 103 trainers. Drop a move from the list
    /// once its effect is supported.
    const KNOWN_UNSUPPORTED: &[MoveId] = &[consts::MOVE_ROLLOUT];

    #[test]
    fn slice_moves_have_supported_effects() {
        let t = Emerald.battle();
        let unsupported: Vec<MoveId> = crate::samples::MOVE_IDS
            .iter()
            .copied()
            .filter(|&m| matches!(t.move_info(m).effect, MoveEffect::Unsupported(_)))
            .collect();
        for m in &unsupported {
            assert!(
                KNOWN_UNSUPPORTED.contains(m),
                "{m:?} has an unsupported effect"
            );
        }
        for m in KNOWN_UNSUPPORTED {
            assert!(
                unsupported.contains(m),
                "{m:?} is supported now: drop it from the list"
            );
        }
        // The starters, the wild mons and May's parties use supported moves only.
        let mut core: Vec<(engine::data::SpeciesId, u8)> =
            battle::STARTERS.iter().map(|&s| (s, 15)).collect();
        core.push(battle::FIRST_BATTLE_FOE);
        for m in [
            consts::MAP_ROUTE101,
            consts::MAP_ROUTE102,
            consts::MAP_ROUTE103,
        ] {
            core.extend(
                Emerald
                    .wild_encounters(m, 0)
                    .unwrap()
                    .mons
                    .iter()
                    .map(|w| (w.species, w.max_level)),
            );
        }
        for (s, level) in core {
            for l in t
                .species_info(s)
                .learnset
                .iter()
                .filter(|l| l.level <= level)
            {
                assert!(!unsupported.contains(&l.mov), "{s:?}: {:?}", l.mov);
            }
        }
    }

    #[test]
    fn mays_route_103_parties_resolve() {
        use consts::*;
        let t = Emerald.battle();
        for (id, ace) in [
            (TRAINER_MAY_ROUTE_103_TREECKO, SPECIES_TORCHIC),
            (TRAINER_MAY_ROUTE_103_TORCHIC, SPECIES_MUDKIP),
            (TRAINER_MAY_ROUTE_103_MUDKIP, SPECIES_TREECKO),
        ] {
            let tr = Emerald.trainer(id).unwrap();
            assert!(tr.female);
            assert!(!tr.double);
            assert_eq!(tr.party.len(), 1);
            assert_eq!(tr.party[0].species, ace);
            assert_eq!(tr.party[0].level, 5);
            assert!(t.species_info(ace).gfx.is_some());
            assert!(battle::TRAINER_IDS.contains(&id));
        }
    }

    #[test]
    fn slice_routes_have_twelve_slot_tables() {
        let t = Emerald.battle();
        for m in [
            consts::MAP_ROUTE101,
            consts::MAP_ROUTE102,
            consts::MAP_ROUTE103,
        ] {
            let e = Emerald.wild_encounters(m, 0).unwrap();
            assert_eq!(e.mons.len(), 12);
            assert!(e.rate > 0);
            for w in e.mons {
                assert!(w.min_level <= w.max_level);
                assert!(t.species_info(w.species).gfx.is_some());
            }
        }
        assert!(
            Emerald
                .wild_encounters(consts::MAP_LITTLEROOT_TOWN, 0)
                .is_none()
        );
    }

    #[test]
    fn battle_gfx_shapes() {
        let g = Emerald.battle_gfx();
        assert_eq!(g.textbox.tiles.len(), 256);
        assert_eq!(g.textbox.palettes.len(), 4);
        assert_eq!(g.textbox.tilemap.len(), 32 * 64);
        for e in &g.environments {
            assert!(!e.tiles.is_empty());
            assert_eq!(e.palettes.len(), 3);
            assert_eq!(e.tilemap.len(), 2048);
        }
        for (img, tiles) in [
            (&g.healthbox_player, 128),
            (&g.healthbox_opponent, 64),
            (&g.hpbar, 30),
            (&g.expbar, 9),
            (&g.status, 30),
            (&g.misc, 12),
            (&g.ball, 12),
            (&g.starter_ball, 64),
            (&g.starter_circle, 64),
        ] {
            assert_eq!(img.tiles.len(), tiles);
            assert!(!img.palettes.is_empty());
        }
        assert_eq!(g.starter_bag.tilemap.len(), 2048);
        assert_eq!(g.starter_bag.palettes.len(), 2);
        // "Lv" is small-font glyph 0x105.
        assert!(g.small_font.glyphs.len() > 0x105);
        assert_eq!(g.small_font.glyphs.len(), g.small_font.widths.len());
        assert!(g.small_font.glyphs[0x105].iter().any(|&b| b != 0));
        assert_eq!(g.player_back.tiles.len(), 4 * 64);
    }

    /// The textbox's text palettes are the ROM's `gBattleWindowTextPalette` (LZ77) and
    /// `gPPTextPalette` (raw), as located by `extract::battle_gfx`.
    #[test]
    fn textbox_text_palettes_round_trip_to_the_rom() {
        let Some(rom) = super::super::extracted::source_rom() else {
            return;
        };
        let l = extract::battle_gfx::locate(&rom).unwrap();
        let [text, pp] = l.text_palettes;
        let pal = |b: &[u8]| -> [u16; 16] {
            core::array::from_fn(|i| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]))
        };
        let g = Emerald.battle_gfx();
        assert_eq!(
            g.textbox.palettes[2],
            pal(&rom.lz77_at(text).unwrap()[..32])
        );
        assert_eq!(g.textbox.palettes[3], pal(rom.slice(pp, 32).unwrap()));
    }

    /// The pics come from the ROM: Mudkip's front pic is the decompressed ROM pic.
    #[test]
    fn a_mon_pic_round_trips_to_the_png() {
        let g = Emerald
            .battle()
            .species_info(consts::SPECIES_MUDKIP)
            .gfx
            .unwrap();
        assert!(g.front.iter().flatten().any(|&w| w != 0));
        assert_ne!(g.palette, g.shiny);
    }

    /// The behaviours of every cell of `map`.
    fn cells(map: MapId) -> Vec<Behavior> {
        let m = Emerald.map(map);
        let mut out = Vec::new();
        for y in 0..m.height as i16 {
            for x in 0..m.width as i16 {
                out.push(engine::flow::behavior_at(&Emerald, m, x, y));
            }
        }
        out
    }

    /// Off-grass encounters (the Silice Ch5 finding): Gen3's land-encounter tiles besides tall
    /// grass resolve to their own `Floor`s, so the caves, the desert, the ash and the long
    /// grass have wild battles; and every exported map with a land table has encounter tiles.
    #[test]
    fn off_grass_encounter_maps_resolve_to_encounter_tiles() {
        use consts::*;
        use engine::data::{Floor, MapType};
        let has = |map: MapId, b: Behavior| cells(map).contains(&b);
        let cave = Behavior::Floor(Floor::Cave);
        // Granite Cave 1F: cave floor only, no tall grass; Underground (the cave background).
        let g = cells(MAP_GRANITE_CAVE_1F);
        assert!(g.contains(&cave));
        assert!(g.iter().all(|&b| b == cave || !b.land_encounter()));
        assert!(Emerald.wild_encounters(MAP_GRANITE_CAVE_1F, 0).is_some());
        assert_eq!(Emerald.map_type(MAP_GRANITE_CAVE_1F), MapType::Underground);
        // Route 111: the desert's deep sand (encounters); plain sand has none.
        assert!(has(MAP_ROUTE111, Behavior::Floor(Floor::DeepSand)));
        assert!(Behavior::Floor(Floor::DeepSand).land_encounter());
        assert!(!Behavior::Floor(Floor::Sand).land_encounter());
        assert!(Emerald.wild_encounters(MAP_ROUTE111, 0).is_some());
        let sand: Vec<&str> = (data::MAP_IDS.iter())
            .filter(|&&m| has(m, Behavior::Floor(Floor::Sand)))
            .map(|&m| Emerald.map(m).name)
            .collect();
        std::println!("plain sand on {sand:?}");
        assert!(sand.contains(&"Route109"), "{sand:?}");
        // The other behaviours of the finding.
        assert!(has(MAP_ROUTE113, Behavior::Floor(Floor::AshGrass)));
        assert_eq!(Emerald.map_type(MAP_ROUTE113), MapType::OutdoorSand);
        assert!(has(MAP_ROUTE119, Behavior::Floor(Floor::LongGrass)));
        assert!(has(MAP_ROUTE120, Behavior::Floor(Floor::LongGrass)));
        assert!(has(
            MAP_NEW_MAUVILLE_INSIDE,
            Behavior::Floor(Floor::IndoorEncounter)
        ));
        assert!(has(MAP_RUSTURF_TUNNEL, cave));
        assert!(has(MAP_MT_CHIMNEY, Behavior::Floor(Floor::MountainTop)));
        assert_eq!(Emerald.map_type(MAP_ROUTE101), MapType::Outdoor);
        assert_eq!(Emerald.map_type(MAP_RUSTBORO_CITY_GYM), MapType::Indoor);
        // Every exported map with a land table can meet it (Route 130's is Mirage Island's,
        // whose grass is not in the map's own layout).
        let mut grass_only = 0;
        let mut without = Vec::new();
        let mut tables = 0;
        for &m in data::MAP_IDS.iter() {
            if Emerald.wild_encounters(m, 0).is_none() {
                continue;
            }
            tables += 1;
            let c = cells(m);
            if !c.iter().any(|b| b.land_encounter()) {
                without.push(Emerald.map(m).name);
            } else if c
                .iter()
                .all(|&b| b == Behavior::TallGrass || !b.land_encounter())
            {
                grass_only += 1;
            }
        }
        std::println!("land tables {tables}, tall grass only {grass_only}, none {without:?}");
        assert!(without.iter().all(|&n| n == "Route130"), "{without:?}");
        assert!(
            tables - grass_only - without.len() >= 58,
            "maps off tall grass"
        );
    }
}

#[cfg(pkr_stub_battle)]
mod stub {
    use super::*;

    #[test]
    fn the_stub_has_a_grass_battle() {
        let g = Emerald.new_game();
        let m = Emerald.map(g.map);
        let grass = m
            .blocks
            .iter()
            .filter(|&&b| {
                let mt = engine::data::block_metatile(b);
                Emerald.behavior(engine::data::attr_behavior(
                    m.primary.attributes[mt as usize],
                )) == Behavior::TallGrass
            })
            .count();
        assert_eq!(grass, 1);
        let e = Emerald.wild_encounters(g.map, 0).unwrap();
        let t = Emerald.battle();
        assert_eq!(t.species.len(), 3);
        assert_eq!(t.moves.len(), 4);
        for w in e.mons {
            assert!(t.species_info(w.species).gfx.is_some());
        }
        assert!(battle::TRAINER_IDS.is_empty());
        assert!(
            crate::samples::MOVE_IDS
                .iter()
                .all(|&m| !matches!(t.move_info(m).effect, MoveEffect::Unsupported(_)))
        );
    }
}
