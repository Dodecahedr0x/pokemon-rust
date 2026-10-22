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

fn fought(b: TrainerBattle, h: &mut Host) {
    run(
        |ctx| {
            Box::pin(async move {
                let fought = trainer_battle(&ctx, b).await;
                ctx.set_var(consts::VAR_RESULT, fought as u16).await;
            })
        },
        h,
    );
}

#[test]
fn trainer_battle_single_follows_trainer_battle_inc() {
    let tr = TrainerId(3);
    let b = TrainerBattle::single(tr, 2, TextId(1), TextId(2));
    let mut h = Host::default();
    fought(b, &mut h);
    assert_eq!(
        h.log,
        [
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
            Request::SetVar(consts::VAR_RESULT, 1),
        ]
    );

    // Beaten: the script goes on with its post-battle code.
    let mut h = Host::default();
    h.flags.insert(0x503);
    fought(b, &mut h);
    assert_eq!(h.log.len(), 4);
    assert_eq!(h.vars[&consts::VAR_RESULT.0], 0);

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
    let mut h = Host::default();
    fought(TrainerBattle::no_intro(tr, TextId(4)), &mut h);
    assert_eq!(
        h.log,
        [
            Request::BattleTrainer {
                trainer: tr,
                defeat: TextId(4)
            },
            Request::SetFlag(FlagId(0x507), true),
            Request::SetVar(consts::VAR_RESULT, 1),
        ]
    );
    let mut h = Host::default();
    fought(
        TrainerBattle::continue_script(tr, 1, TextId(1), TextId(2)),
        &mut h,
    );
    assert!(h.log.contains(&Request::Message(TextId(1))));
    assert!(
        !h.log.contains(&Request::ReleaseAll),
        "the continue script releases"
    );
    assert_eq!(h.vars[&consts::VAR_RESULT.0], 1);
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
    for s in battle::SLICE_SPECIES
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

    /// Slice moves whose effect the engine does not implement yet (for Task E2). They come
    /// from the parties of the Route 102 and Route 103 trainers. Drop a move from the list
    /// once its effect is supported.
    const KNOWN_UNSUPPORTED: &[MoveId] = &[consts::MOVE_ATTRACT, consts::MOVE_ROLLOUT];

    #[test]
    fn slice_moves_have_supported_effects() {
        let t = Emerald.battle();
        let unsupported: Vec<MoveId> = battle::SLICE_MOVES
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
                    .encounters(m)
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
            let e = Emerald.encounters(m).unwrap();
            assert_eq!(e.mons.len(), 12);
            assert!(e.rate > 0);
            for w in e.mons {
                assert!(w.min_level <= w.max_level);
                assert!(t.species_info(w.species).gfx.is_some());
            }
        }
        assert!(Emerald.encounters(consts::MAP_LITTLEROOT_TOWN).is_none());
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
        assert!(g.numbers.tiles.is_empty());
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
        let e = Emerald.encounters(g.map).unwrap();
        let t = Emerald.battle();
        assert_eq!(t.species.len(), 3);
        assert_eq!(t.moves.len(), 4);
        for w in e.mons {
            assert!(t.species_info(w.species).gfx.is_some());
        }
        assert!(battle::TRAINER_IDS.is_empty());
        assert!(
            battle::SLICE_MOVES
                .iter()
                .all(|&m| !matches!(t.move_info(m).effect, MoveEffect::Unsupported(_)))
        );
    }
}
