//! Tests of the generated tables (both modes) and of the prelude helpers.

use super::*;
use engine::data::*;

use crate::prelude::{MsgBox, msgbox};
use engine::data::{Gfx, block_collision};
use engine::script::{Request, Response, Runner, ScriptHost};
use std::vec::Vec;

/// These tests were written for the Milestone 3 start (the starter received).
#[allow(non_upper_case_globals)]
const Emerald: Emerald = Emerald::after_starter();

/// The charmap of the generated texts (`data::LANG`).
fn lang() -> extract::text::Lang {
    match data::LANG {
        "fr" => extract::text::Lang::Fr,
        "en" => extract::text::Lang::En,
        l => panic!("unknown language {l}"),
    }
}

/// Answers every request at once with a value of the right kind; records them.
#[derive(Default)]
struct Host {
    log: Vec<Request>,
    yes: bool,
    vars: std::collections::BTreeMap<u16, u16>,
    flags: std::collections::BTreeSet<u16>,
    /// The answer to `ChooseStarter`.
    starter: u16,
    /// The answer to `BattleWild` / `BattleTrainer`.
    outcome: Option<engine::battle::BattleOutcome>,
}

impl ScriptHost for Host {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        Some(match *r {
            Request::GetFlag(f) => Response::Bool(self.flags.contains(&f.0)),
            Request::SetFlag(f, on) => {
                if on {
                    self.flags.insert(f.0);
                } else {
                    self.flags.remove(&f.0);
                }
                Response::Unit
            }
            Request::GiveMon { .. } => Response::Value(0),
            Request::PartySize => Response::Value(1),
            // Talked to (no sight approach): the player's usable mons (Task I).
            Request::TrainerApproach { .. } => Response::Value(2),
            Request::ChooseStarter(_) => Response::Value(self.starter),
            Request::BattleWild { .. } | Request::BattleTrainer { .. } => {
                Response::Value(self.outcome.unwrap_or(engine::battle::BattleOutcome::Won) as u16)
            }
            Request::GetVar(v) => Response::Value(self.vars.get(&v.0).copied().unwrap_or(0)),
            Request::SetVar(v, x) => {
                self.vars.insert(v.0, x);
                Response::Unit
            }
            Request::Random(_) | Request::PlayerFacing | Request::Query(_) => Response::Value(0),
            Request::PlayerPos => Response::Pos(0, 0),
            Request::CurrentMap => Response::Value(0),
            Request::YesNo { .. } => Response::Bool(self.yes),
            // The bag and money always succeed.
            Request::ItemAdd { .. }
            | Request::ItemCheck { .. }
            | Request::ItemSpace { .. }
            | Request::ItemRemove { .. }
            | Request::PcItemAdd { .. }
            | Request::MoneyCheck(_) => Response::Bool(true),
            Request::Multichoice { .. } => Response::Value(0),
            _ => Response::Unit,
        })
    }

    fn resume(&mut self, _: &Request) -> Option<Response> {
        Some(Response::Unit)
    }
}

/// Runs `script` for at most 100 frames; true if it finished.
fn run(script: impl FnOnce(Ctx) -> Script, host: &mut Host) -> bool {
    let mut r = Runner::new();
    r.start(script(r.ctx()));
    for _ in 0..100 {
        if !r.step(host) {
            return true;
        }
    }
    false
}

#[test]
fn msgbox_npc_follows_std_msgbox_npc() {
    let mut h = Host::default();
    run(
        |ctx| Box::pin(async move { msgbox(&ctx, TextId(3), MsgBox::Npc).await }),
        &mut h,
    );
    assert_eq!(
        h.log,
        [
            Request::Lock,
            Request::FacePlayer,
            Request::Message(TextId(3)),
            Request::WaitMessage,
            Request::WaitButton,
            Request::Release,
        ]
    );
}

#[test]
fn msgbox_sign_locks_everyone() {
    let mut h = Host::default();
    run(
        |ctx| Box::pin(async move { msgbox(&ctx, TextId(1), MsgBox::Sign).await }),
        &mut h,
    );
    assert_eq!(h.log.first(), Some(&Request::LockAll));
    assert_eq!(h.log.last(), Some(&Request::ReleaseAll));
}

#[test]
fn msgbox_yes_no_stores_the_answer_in_var_result() {
    for (yes, want) in [(true, 1), (false, 0)] {
        let mut h = Host {
            yes,
            ..Host::default()
        };
        h.vars.insert(consts::VAR_RESULT.0, 7);
        run(
            |ctx| Box::pin(async move { msgbox(&ctx, TextId(0), MsgBox::YesNo).await }),
            &mut h,
        );
        assert_eq!(
            &h.log[..3],
            [
                Request::Message(TextId(0)),
                Request::WaitMessage,
                Request::YesNo { default_no: false }
            ]
        );
        assert_eq!(h.vars[&consts::VAR_RESULT.0], want);
        assert!(!h.log.contains(&Request::WaitButton));
    }
}

#[test]
fn special_logs_and_returns_zero() {
    let mut h = Host::default();
    run(
        |ctx| {
            Box::pin(async move {
                let v = prelude::special(&ctx, consts::SPECIAL_HealPlayerParty).await;
                ctx.set_var(consts::VAR_RESULT, v + 5).await;
            })
        },
        &mut h,
    );
    assert_eq!(h.log[0], Request::Log("SPECIAL_HealPlayerParty"));
    assert_eq!(h.vars[&consts::VAR_RESULT.0], 5);
}

#[test]
fn constants_cover_the_names_tables() {
    assert_eq!(consts::MAP_LITTLEROOT_TOWN, MapId(0x0009));
    assert_eq!(consts::MAP_ROUTE101, MapId(0x0010));
    assert_eq!(consts::VAR_RESULT.0, 0x800D);
    assert_eq!(consts::STD_MSGBOX_NPC, consts::MSGBOX_NPC);
    assert_eq!(consts::STD_FIND_ITEM, 1);
    assert_eq!(consts::OBJ_EVENT_GFX_BRENDAN_NORMAL, 0);
}

#[test]
fn ui_texts_follow_the_language() {
    let d = |b: &[u8]| extract::text::decode_lang(b, lang()).unwrap().0;
    let (yes, no, rival) = match data::LANG {
        "fr" => ("OUI", "NON", "FLORA"),
        _ => ("YES", "NO", "MAY"),
    };
    assert_eq!(d(Emerald.ui_text(UiText::Yes)), yes);
    assert_eq!(d(Emerald.ui_text(UiText::No)), no);
    assert_eq!(d(Emerald.ui_text(UiText::RivalName)), rival);
    assert_eq!(d(Emerald.new_game().player_name), "SACHA");
}

#[test]
fn behaviors_follow_the_names_table() {
    let mb = |n| extract::names::METATILE_BEHAVIORS.value(n).unwrap() as u8;
    assert_eq!(
        Emerald.behavior(mb("MB_JUMP_SOUTH")),
        Behavior::Ledge(Dir::Down)
    );
    assert_eq!(Emerald.behavior(mb("MB_ANIMATED_DOOR")), Behavior::Door);
    assert_eq!(
        Emerald.behavior(mb("MB_NON_ANIMATED_DOOR")),
        Behavior::NonAnimDoor(Dir::Down)
    );
    assert_eq!(
        Emerald.behavior(mb("MB_DEEP_SOUTH_WARP")),
        Behavior::NonAnimDoor(Dir::Up)
    );
    assert_eq!(Emerald.behavior(mb("MB_LADDER")), Behavior::Ladder);
    assert_eq!(
        Emerald.behavior(mb("MB_NORTH_ARROW_WARP")),
        Behavior::ArrowWarp(Dir::Up)
    );
    assert_eq!(Emerald.behavior(mb("MB_COUNTER")), Behavior::Counter);
    assert_eq!(Emerald.behavior(mb("MB_TALL_GRASS")), Behavior::TallGrass);
    assert_eq!(Emerald.behavior(mb("MB_POND_WATER")), Behavior::Blocked);
    assert_eq!(Emerald.behavior(mb("MB_NORMAL")), Behavior::Normal);
    // Forced movement (Gen3 `sForcedMovementTestFuncs`) and the per-step puzzles.
    assert_eq!(Emerald.behavior(mb("MB_ICE")), Behavior::Ice);
    assert_eq!(
        Emerald.behavior(mb("MB_TRICK_HOUSE_PUZZLE_8_FLOOR")),
        Behavior::Ice
    );
    for (walk, slide, d) in [
        ("MB_WALK_EAST", "MB_SLIDE_EAST", Dir::Right),
        ("MB_WALK_WEST", "MB_SLIDE_WEST", Dir::Left),
        ("MB_WALK_NORTH", "MB_SLIDE_NORTH", Dir::Up),
        ("MB_WALK_SOUTH", "MB_SLIDE_SOUTH", Dir::Down),
    ] {
        assert_eq!(Emerald.behavior(mb(walk)), Behavior::ForcedWalk(d));
        assert_eq!(Emerald.behavior(mb(slide)), Behavior::Slide(d));
    }
    assert_eq!(Emerald.behavior(mb("MB_MUDDY_SLOPE")), Behavior::MuddySlope);
    assert_eq!(Emerald.behavior(mb("MB_THIN_ICE")), Behavior::ThinIce);
    assert_eq!(Emerald.behavior(mb("MB_CRACKED_ICE")), Behavior::CrackedIce);
    assert_eq!(
        Emerald.behavior(mb("MB_CRACKED_FLOOR")),
        Behavior::CrackedFloor
    );
    assert_eq!(
        Emerald.behavior(mb("MB_CRACKED_FLOOR_HOLE")),
        Behavior::CrackedFloorHole
    );
}

#[test]
fn player_sprite_has_walk_and_run_frames() {
    let s = Emerald.player_sprite();
    assert_eq!((s.width, s.height), (16, 32));
    assert_eq!(s.frames.len(), 18);
    assert!(s.frames.iter().all(|f| f.len() == 8));
    assert!(core::ptr::eq(
        s,
        Emerald
            .sprite(GfxId(consts::OBJ_EVENT_GFX_BRENDAN_NORMAL))
            .unwrap()
    ));
}

#[test]
fn window_text_colors() {
    let w = Emerald.window();
    assert_eq!((w.text_fg, w.text_shadow, w.text_bg), (2, 3, 1));
    assert!(w.tiles.len() >= 14);
}

#[test]
fn new_game_starts_on_a_walkable_cell_of_an_exported_map() {
    let g = Emerald.new_game();
    assert_eq!(g.map, consts::MAP_LITTLEROOT_TOWN);
    assert!(has_map(g.map));
    let m = Emerald.map(g.map);
    assert!(g.x >= 0 && g.y >= 0 && (g.x as u16) < m.width && (g.y as u16) < m.height);
    let block = m.blocks[g.y as usize * m.width as usize + g.x as usize];
    assert_eq!(block_collision(block), 0);
}

/// The new-game state (`new_game.rs`): after the starter, before Route 103, male player.
#[test]
fn a_new_world_starts_after_the_starter() {
    use consts::*;
    let w = engine::World::new(&Emerald);
    assert_eq!(w.map(), MAP_LITTLEROOT_TOWN);
    assert_eq!(
        w.objects().player().pos(),
        (data::NEW_GAME.1, data::NEW_GAME.2)
    );
    for (v, x) in [
        (VAR_LITTLEROOT_INTRO_STATE, 7),
        (VAR_LITTLEROOT_TOWN_STATE, 2),
        (VAR_LITTLEROOT_HOUSES_STATE_BRENDAN, 2),
        (VAR_LITTLEROOT_RIVAL_STATE, 3),
        (VAR_BIRCH_LAB_STATE, 3),
        (VAR_ROUTE101_STATE, 3),
        (VAR_OLDALE_TOWN_STATE, 0),
    ] {
        assert_eq!(w.var(v), x, "{v:?}");
    }
    for f in [
        FLAG_SYS_POKEMON_GET,
        FLAG_RESCUED_BIRCH,
        FLAG_HIDE_LITTLEROOT_TOWN_RIVAL,
        FLAG_HIDE_ROUTE_101_BIRCH,
        FLAG_HIDE_OLDALE_TOWN_RIVAL,
    ] {
        assert!(w.flag(f), "{f:?}");
    }
    for f in [
        FLAG_SYS_B_DASH,
        FLAG_ADVENTURE_STARTED,
        FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH,
    ] {
        assert!(!w.flag(f), "{f:?}");
    }
    assert!(!w.script_running());
}

/// `NewGameInitData` (`new_game.rs`): every `ResetAllMapFlags` flag but the ones the intro and
/// the starter clear, 3000 money, the size records, the Potion in the PC.
#[test]
fn a_new_world_has_new_game_init_data() {
    use consts::*;
    let w = engine::World::new(&Emerald);
    let cleared = [
        FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN,
        FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH,
        FLAG_HIDE_ROUTE_101_BOY,
    ];
    assert!(RESET_ALL_MAP_FLAGS.len() > 150);
    for &f in RESET_ALL_MAP_FLAGS {
        assert_eq!(w.flag(f), !cleared.contains(&f), "{f:?}");
    }
    assert_eq!(w.money(), 3000);
    assert_eq!(w.var(VAR_SEEDOT_SIZE_RECORD), 0x8000);
    assert_eq!(w.var(VAR_LOTAD_SIZE_RECORD), 0x8000);
    // A world without the Potion in its item table (the stub) reports it instead.
    if Emerald.item(engine::data::ItemId(ITEM_POTION)).is_some() {
        assert_eq!(
            w.pc().item_entries().collect::<Vec<_>>(),
            [(0, engine::data::ItemId(ITEM_POTION), 1)]
        );
    }
}

#[test]
fn special_ids_are_the_gen3_ones() {
    let ids = Emerald.special_ids();
    assert_eq!(ids.var_facing, Some(consts::VAR_FACING));
    assert_eq!(ids.var_last_talked, Some(consts::VAR_LAST_TALKED));
    assert_eq!(ids.flag_running, Some(consts::FLAG_SYS_B_DASH));
    assert_eq!(ids.flags_cleared_on_map_change.len(), 5);
    assert!(
        ids.flags_cleared_on_map_change
            .contains(&consts::FLAG_SYS_USE_STRENGTH)
    );
}

#[test]
fn unknown_maps_are_missing() {
    // `MAP_DYNAMIC` is never a map; Petalburg is outside the slice and the stub.
    let mut missing = vec![consts::MAP_DYNAMIC];
    if data::MAP_COUNT <= 15 {
        missing.push(consts::MAP_PETALBURG_CITY);
    }
    for m in missing {
        assert!(!has_map(m));
        assert!(core::ptr::eq(Emerald.map(m), &data::MISSING_MAP));
    }
}

#[test]
fn every_map_has_a_digest() {
    assert_eq!(data::MAP_DIGESTS.len(), data::MAP_COUNT);
    for (&(name, d), &id) in data::MAP_DIGESTS.iter().zip(&data::MAP_IDS) {
        assert_eq!(name, Emerald.map(id).name);
        assert_ne!(d, 0);
    }
    assert_ne!(data::GLOBAL_DIGEST, 0);
}

#[test]
fn the_report_lists_the_degradations() {
    assert!(REPORT.contains("data.bin: "));
    assert!(REPORT.contains("battle: "));
    assert!(REPORT.contains("species without pics"));
}

/// Maps with one wild header read it whatever the wild set; Gen3 reads header 0 for a set the
/// map does not have (Altering Cave has 9).
#[test]
fn wild_sets_fall_back_to_the_first_header() {
    for &id in &data::MAP_IDS {
        let first = Emerald.wild_encounters(id, 0);
        let sets = battle::wild_sets(id.0);
        for set in [0, 1, 8, 9, u16::MAX] {
            if set == 0 || set >= sets {
                assert_eq!(Emerald.wild_encounters(id, set), first, "{id:?} set {set}");
            }
        }
    }
}

#[test]
fn tables_resolve_when_extracted() {
    assert!(Emerald.item(engine::data::ItemId(0)).is_none());
    for i in 1..data::ITEMS.len() as u16 {
        if let Some(it) = Emerald.item(engine::data::ItemId(i)) {
            assert_eq!(it.name.last(), Some(&0xFF));
        }
    }
    for e in [
        engine::data::Emote::Exclamation,
        engine::data::Emote::Question,
        engine::data::Emote::Heart,
    ] {
        if let Some(s) = Emerald.emote(e) {
            assert!(!s.frames.is_empty());
            assert_eq!(
                s.frames[0].len(),
                usize::from(s.width / 8) * usize::from(s.height / 8)
            );
        }
    }
    let (mut lists, mut strings, mut sections) = (0, 0, 0);
    for id in 0..=255u8 {
        if let Some(c) = Emerald.multichoice(id) {
            assert!(c.iter().all(|t| t.last() == Some(&0xFF)));
            lists += 1;
        }
        if let Some(t) = Emerald.std_string(id) {
            assert_eq!(t.last(), Some(&0xFF));
            strings += 1;
        }
        if let Some(t) = data::map_section(id) {
            assert_eq!(t.last(), Some(&0xFF));
            sections += 1;
        }
    }
    let items = data::ITEMS.iter().flatten().count();
    let emotes = [
        engine::data::Emote::Exclamation,
        engine::data::Emote::Question,
        engine::data::Emote::Heart,
    ]
    .iter()
    .filter(|&&e| Emerald.emote(e).is_some())
    .count();
    // `tests/m8_e_tables_build.rs` reads this line.
    std::println!(
        "TABLES items={items} multichoice={lists} std_strings={strings} map_sections={sections} emotes={emotes}"
    );
}

fn all_script_refs(m: &MapData) -> Vec<ScriptId> {
    let mut v: Vec<ScriptId> = m.objects.iter().filter_map(|o| o.script).collect();
    v.extend(m.coords.iter().map(|c| c.script));
    v.extend(m.bgs.iter().filter_map(|b| match b.kind {
        engine::data::BgKind::Script { script, .. } => Some(script),
        _ => None,
    }));
    let s = &m.scripts;
    v.extend(
        [
            s.on_load,
            s.on_transition,
            s.on_resume,
            s.on_return_to_field,
        ]
        .into_iter()
        .flatten(),
    );
    v.extend(s.on_frame.iter().chain(s.on_warp_into).map(|t| t.script));
    v
}

#[test]
fn every_map_is_consistent() {
    for &id in &data::MAP_IDS {
        let m = Emerald.map(id);
        assert_eq!(
            m.blocks.len(),
            m.width as usize * m.height as usize,
            "{}",
            m.name
        );
        for ts in [m.primary, m.secondary] {
            assert_eq!(ts.metatiles.len(), ts.attributes.len());
        }
        // Every metatile the map uses exists. (Their tile entries may point past a secondary
        // tileset's tiles, as in the ROM: Gen3 then draws whatever VRAM holds there.)
        for b in m.blocks.iter().chain(&m.border) {
            let mt = engine::data::block_metatile(*b);
            let ts = if mt < 512 { m.primary } else { m.secondary };
            assert!(
                ((mt % 512) as usize) < ts.metatiles.len(),
                "{}: metatile {mt}",
                m.name
            );
        }
        for r in all_script_refs(m) {
            assert!((r.0 as usize) < data::SCRIPT_COUNT, "{}", m.name);
        }
        for c in m.connections {
            assert!(has_map(c.map), "{}: connection to unexported map", m.name);
        }
        for o in m.objects {
            if let Gfx::Fixed(g) = o.gfx {
                assert!(Emerald.sprite(g).is_some(), "{}: gfx {}", m.name, g.0);
            }
        }
    }
}

#[test]
fn every_script_id_dispatches() {
    for (id, name) in data::SCRIPT_NAMES.iter().enumerate() {
        let mut h = Host::default();
        // A generated script may be empty (Gen3 `end` only); it must still finish or act.
        let finished = run(|ctx| Emerald.script(ScriptId(id as u16), ctx), &mut h);
        assert!(finished || !h.log.is_empty(), "{name}");
    }
}

/// Only does something on the fixture of `tests/build_modes.rs`: its hand-written IR
/// `StubTown_EventScript_Obj1`, lowered by the build, runs through msgbox, a movement table, a
/// label call, a cross-module call, special, `checkplayergender`, `STD_OBTAIN_ITEM`, an
/// unsupported command and a warp.
#[test]
fn fixture_script_runs_through_the_prelude() {
    let Some(id) = data::SCRIPT_NAMES
        .iter()
        .position(|&n| n == "StubTown_EventScript_Obj1")
    else {
        return;
    };
    let mut h = Host::default();
    run(|ctx| Emerald.script(ScriptId(id as u16), ctx), &mut h);
    let text = |name| TextId(data::TEXT_NAMES.iter().position(|&n| n == name).unwrap() as u16);
    let log = &h.log;
    assert_eq!(log[2], Request::Message(text("StubTown_Text_Hello")));
    assert!(
        log.iter()
            .any(|r| matches!(r, Request::Move { obj: 0xFF, steps } if steps.len() == 3))
    );
    assert!(log.contains(&Request::SetFlag(
        consts::FLAG_HIDE_LITTLEROOT_TOWN_RIVAL,
        true
    )));
    assert!(log.contains(&Request::Message(text("Other_Text_Bye"))));
    assert!(log.contains(&Request::HealParty));
    // checkplayergender: male.
    assert!(log.contains(&Request::SetVar(consts::VAR_RESULT, 0)));
    assert!(log.contains(&Request::ItemAdd {
        item: engine::data::ItemId(consts::ITEM_POTION),
        qty: 1
    }));
    assert!(log.contains(&Request::Warp {
        map: consts::MAP_PETALBURG_CITY,
        warp: Some(0),
        x: 0,
        y: 0
    }));
    assert_eq!(
        log.last(),
        Some(&Request::Report(engine::script::Report::Unsupported(
            "pokemart"
        )))
    );
    let m = Emerald.map(Emerald.new_game().map);
    assert_eq!(m.objects[0].script, Some(ScriptId(id as u16)));
    assert_eq!(m.objects[0].hide_flag, Some(engine::data::FlagId(0x1AC)));
    let t = Emerald.text(text("StubTown_Text_Hello"));
    let (s, _) = extract::text::decode_lang(t, extract::text::Lang::Fr).unwrap();
    assert_eq!(s, "HELLO {PLAYER}!\\nOK É?");
    std::println!("FIXTURE SCRIPT OK");
}

#[test]
fn every_text_decodes() {
    let lang = lang();
    for (i, t) in data::TEXTS.iter().enumerate() {
        let (_, n) = extract::text::decode_lang(t, lang).unwrap();
        assert_eq!(n, t.len(), "text {i}");
    }
}

/// Prints the size of the generated tables (`--nocapture`).
#[test]
fn data_sizes() {
    let mut tilesets: Vec<*const engine::data::Tileset> = Vec::new();
    let (mut ts_bytes, mut map_bytes) = (0, 0);
    for &id in &data::MAP_IDS {
        let m = Emerald.map(id);
        map_bytes += m.blocks.len() * 2;
        for ts in [m.primary, m.secondary] {
            if !tilesets.contains(&(ts as *const _)) {
                tilesets.push(ts);
                ts_bytes += core::mem::size_of_val(ts.tiles)
                    + ts.palettes.len() * 32
                    + ts.metatiles.len() * 16
                    + ts.attributes.len() * 2;
            }
        }
    }
    let mut sprite_bytes = 0;
    for g in 0..=255u16 {
        if let Some(s) = Emerald.sprite(GfxId(g)) {
            sprite_bytes += s
                .frames
                .iter()
                .map(|f| core::mem::size_of_val(*f))
                .sum::<usize>()
                + 32;
        }
    }
    let font = Emerald.font();
    let font_bytes = font.glyphs.len() * 64 + font.widths.len();
    let text_bytes: usize = data::TEXTS.iter().map(|t| t.len()).sum();
    std::println!(
        "content data: tilesets {ts_bytes} B ({} sets), maps {map_bytes} B, sprites {sprite_bytes} B, font {font_bytes} B, texts {text_bytes} B ({} texts), scripts {}",
        tilesets.len(),
        data::TEXTS.len(),
        data::SCRIPT_COUNT
    );
    assert!(ts_bytes + map_bytes + sprite_bytes + font_bytes + text_bytes < 8 << 20);
}

#[test]
fn std_find_item_removes_the_last_talked_object() {
    let mut h = Host::default();
    h.vars.insert(consts::VAR_LAST_TALKED.0, 4);
    run(
        |ctx| Box::pin(async move { prelude::callstd(&ctx, consts::STD_FIND_ITEM).await }),
        &mut h,
    );
    assert!(h.log.contains(&Request::RemoveObject(4)));
    assert!(!h.log.iter().any(|r| matches!(r, Request::HideObject(_))));
    // Gen3 `Std_FindItem` leaves `PYRAMID_LOCATION_NONE` (0) after a pick-up.
    assert_eq!(h.vars[&consts::VAR_RESULT.0], 0);
}

#[cfg(not(pkr_stub))]
mod extracted {
    use super::*;

    /// Wave C: `ShowPokedexRatingMessage`'s 22 texts (extract's `C_TEXT_RUNS`) are built.
    #[test]
    fn pokedex_rating_texts_are_built() {
        let rated = data::TEXT_NAMES
            .iter()
            .filter(|n| n.starts_with("gBirchDexRatingText_"))
            .count();
        assert!(rated >= 22, "{rated}");
        for n in [
            "gBirchDexRatingText_LessThan10",
            "gBirchDexRatingText_DexCompleted",
        ] {
            assert!(crate::prelude::text_named(n).is_some(), "{n}");
        }
    }

    /// The French ROM, when `PKR_ROM_FR` is set and the data was extracted from it.
    pub(super) fn source_rom() -> Option<extract::Rom> {
        std::env::var_os("PKR_ROM_FR")?;
        let rom = extract::fixtures::fr();
        if rom.sha1_hex() != data::SOURCE_SHA1 {
            std::println!("skipped: content/extracted is not from PKR_ROM_FR");
            return None;
        }
        Some(rom)
    }

    #[test]
    fn font_glyphs_round_trip_to_the_rom() {
        let Some(rom) = source_rom() else { return };
        let (glyphs_off, widths_off) = extract::font::locate_latin_normal(&rom).unwrap();
        let f = Emerald.font();
        let raw = rom.slice(glyphs_off, f.glyphs.len() * 64).unwrap();
        assert!(
            f.glyphs.as_flattened() == raw,
            "glyph bytes differ from the ROM"
        );
        assert_eq!(f.widths, rom.slice(widths_off, f.widths.len()).unwrap());
        // And through extract's own decoder.
        let decoded = extract::font::read_latin_normal_at(&rom, glyphs_off, widths_off).unwrap();
        assert_eq!(decoded.glyphs.len(), f.glyphs.len());
    }

    #[test]
    fn a_tileset_round_trips_to_the_rom() {
        let Some(rom) = source_rom() else { return };
        let table = extract::maps::locate_map_table(&rom).unwrap();
        let header = table
            .header(&rom, extract::maps::MapId { group: 0, num: 9 })
            .unwrap();
        let ours = Emerald.map(consts::MAP_LITTLEROOT_TOWN);
        for (off, ts) in [
            (header.layout.primary_tileset, ours.primary),
            (header.layout.secondary_tileset, ours.secondary),
        ] {
            let r = extract::maps::read_tileset(&rom, off, &table).unwrap();
            let rom_ts = extract::render::load_tileset(&rom, &r).unwrap();
            let tiles: Vec<u8> = ts
                .tiles
                .iter()
                .flatten()
                .flat_map(|r| r.to_le_bytes())
                .collect();
            assert!(tiles == rom_ts.tiles, "tile bytes differ at {off:#x}");
            assert_eq!(ts.metatiles, &rom_ts.metatiles[..]);
            assert_eq!(ts.attributes, &rom_ts.attributes[..]);
            assert_eq!(ts.palettes, &rom_ts.palettes[..]);
        }
    }

    #[test]
    fn slice_maps_are_exported() {
        for m in [
            consts::MAP_LITTLEROOT_TOWN,
            consts::MAP_ROUTE101,
            consts::MAP_OLDALE_TOWN,
            consts::MAP_ROUTE102,
            consts::MAP_ROUTE103,
            consts::MAP_OLDALE_TOWN_POKEMON_CENTER_1F,
        ] {
            assert!(has_map(m), "{m:?}");
        }
        assert!(data::MAP_IDS.len() >= 15);
    }

    #[test]
    fn maps_sharing_a_tileset_share_the_static() {
        let a = Emerald.map(consts::MAP_LITTLEROOT_TOWN);
        let b = Emerald.map(consts::MAP_ROUTE101);
        assert!(core::ptr::eq(a.primary, b.primary));
    }

    #[test]
    fn littleroot_has_its_events() {
        let m = Emerald.map(consts::MAP_LITTLEROOT_TOWN);
        assert_eq!(m.name, "LittlerootTown");
        let twin = m.objects.iter().find(|o| o.local_id == 1).unwrap();
        assert_eq!(twin.movement, engine::data::MovementType::Wander);
        assert_eq!(
            twin.script,
            data::SCRIPT_NAMES
                .iter()
                .position(|&n| n == "LittlerootTown_EventScript_Twin")
                .map(|i| ScriptId(i as u16)),
            "objects point at their canonical script"
        );
        let rival = m.objects.iter().find(|o| o.local_id == 7).unwrap();
        assert_eq!(rival.gfx, Gfx::Var(consts::VAR_OBJ_GFX_ID_0));
        assert_eq!(
            rival.hide_flag,
            Some(consts::FLAG_HIDE_LITTLEROOT_TOWN_RIVAL)
        );
        assert!(
            m.connections
                .iter()
                .any(|c| c.dir == Dir::Up && c.map == consts::MAP_ROUTE101)
        );
        assert!(
            m.warps
                .iter()
                .any(|w| w.dest_map == consts::MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F)
        );
    }

    #[cfg(not(pkr_stub_battle))]
    /// FNV-1a-64 over formatted values (`write!` into it).
    struct Fnv(u64);

    #[cfg(not(pkr_stub_battle))]
    impl core::fmt::Write for Fnv {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for &b in s.as_bytes() {
                self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
            }
            Ok(())
        }
    }

    #[cfg(not(pkr_stub_battle))]
    /// Digests of the bulk data (field, battle) the slice's route reads through `GameData`.
    pub(super) fn slice_bulk_digests() -> (u64, u64) {
        use core::fmt::Write;
        let mut f = Fnv(0xcbf2_9ce4_8422_2325);
        for &id in &data::MAP_IDS {
            let m = Emerald.map(id);
            write!(
                f,
                "{} {} {} {:?} {:?}",
                m.name, m.width, m.height, m.blocks, m.border
            )
            .unwrap();
            for ts in [m.primary, m.secondary] {
                write!(
                    f,
                    "{:?}{:?}{:?}{:?}",
                    ts.tiles, ts.palettes, ts.metatiles, ts.attributes
                )
                .unwrap();
            }
        }
        for g in 0..=0x3FFu16 {
            if let Some(s) = Emerald.sprite(GfxId(g)) {
                write!(
                    f,
                    "{g} {} {} {:?} {:?}",
                    s.width, s.height, s.frames, s.palette
                )
                .unwrap();
            }
        }
        let p = Emerald.player_sprite();
        write!(f, "{:?}{:?}", p.frames, p.palette).unwrap();
        let font = Emerald.font();
        write!(f, "{:?}{:?}", font.glyphs, font.widths).unwrap();
        let w = Emerald.window();
        write!(
            f,
            "{:?}{:?}{}{}{}",
            w.tiles, w.palette, w.text_fg, w.text_shadow, w.text_bg
        )
        .unwrap();
        write!(f, "{:?}{:?}", data::TEXTS, data::TEXT_NAMES).unwrap();
        write!(f, "{:?}", data::BEHAVIORS).unwrap();

        let mut b = Fnv(0xcbf2_9ce4_8422_2325);
        let t = Emerald.battle();
        for s in crate::samples::SPECIES_IDS.iter().chain(&battle::STARTERS) {
            write!(b, "{:?}", t.species_info(*s)).unwrap();
        }
        for tr in SLICE_TRAINERS {
            // `TrainerMon::item` (Silice held items) is `None` for every Emerald trainer mon
            // (`classify::TRAINER_HELD_ITEMS` is off): left out, the digest stays the Wave 0
            // build's.
            let s = alloc::format!("{:?}", Emerald.trainer(*tr));
            write!(b, "{}", s.replace(", item: None", "")).unwrap();
        }
        let g = Emerald.battle_gfx();
        write!(
            b,
            "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{}",
            g.textbox,
            g.environments,
            g.healthbox_player,
            g.healthbox_opponent,
            g.hpbar,
            g.expbar,
            g.status,
            g.misc,
            g.ball,
            g.small_font.glyphs,
            g.small_font.widths,
            g.starter_bag,
            g.starter_ball,
            g.starter_circle,
            g.player_back.tiles,
            g.player_back.y
        )
        .unwrap();
        (f.0, b.0)
    }

    #[cfg(not(pkr_stub_battle))]
    /// The trainers the slice's scripts name (the Wave 0 build's `TRAINER_IDS`).
    const SLICE_TRAINERS: &[TrainerId] = &[
        consts::TRAINER_ALLEN,
        consts::TRAINER_ANDREW,
        consts::TRAINER_BRENDAN_ROUTE_103_MUDKIP,
        consts::TRAINER_BRENDAN_ROUTE_103_TORCHIC,
        consts::TRAINER_BRENDAN_ROUTE_103_TREECKO,
        consts::TRAINER_CALVIN_1,
        consts::TRAINER_DAISY,
        consts::TRAINER_ISABELLE,
        consts::TRAINER_MARCOS,
        consts::TRAINER_MAY_ROUTE_103_MUDKIP,
        consts::TRAINER_MAY_ROUTE_103_TORCHIC,
        consts::TRAINER_MAY_ROUTE_103_TREECKO,
        consts::TRAINER_MIGUEL_1,
        consts::TRAINER_PETE,
        consts::TRAINER_RHETT,
        consts::TRAINER_RICK,
        consts::TRAINER_TIANA,
    ];

    #[cfg(not(pkr_stub_battle))]
    /// The bulk data the slice reads is the Wave 0 build's, byte for byte, whatever form the
    /// build gives it (Rust literals then, the `data.bin` blob since Milestone 8 Task E): the
    /// parity route hashes depend on it. The digests are of the French ROM's slice extract
    /// (format 5.4, `cargo xtask extract --maps slice`); other extracts only print theirs. The
    /// field digest covers the texts and their names: Task D2 (5.4) names them as the IR does
    /// and adds the std scripts' texts, which moved it from `0x35d7_3d4c_82aa_efe1`.
    #[test]
    fn slice_bulk_data_is_the_wave0_builds() {
        let (field, battle) = slice_bulk_digests();
        std::println!("slice bulk digests: field {field:#018x}, battle {battle:#018x}");
        if source_rom().is_none() || data::MAP_COUNT != 15 {
            return;
        }
        assert_eq!(field, 0x2dad_729f_bec3_4fe9);
        // Task G1 changed the battle data on purpose: the abilities it implements (no longer
        // `Ability::Other`) and `SpeciesInfo::weight` (Low Kick). Was 0x0f9d_09db_0c67_77b7.
        assert_eq!(battle, 0x36df_8eb5_5517_e48f);
    }

    #[test]
    fn the_font_has_every_glyph() {
        let f = Emerald.font();
        assert_eq!(f.glyphs.len(), 512);
        assert_eq!(f.widths.len(), 512);
        // 'A' (0xBB) is drawn.
        assert!(f.glyphs[0xBB].iter().any(|&b| b != 0));
    }
}

#[cfg(pkr_stub)]
mod stub {
    use super::*;

    #[test]
    fn the_stub_world_is_one_small_map() {
        assert_eq!(data::MAP_COUNT, 1);
        let m = Emerald.map(Emerald.new_game().map);
        assert_eq!(m.name, "StubTown");
        assert_eq!((m.width, m.height), (16, 16));
        assert_eq!(Emerald.font().glyphs.len(), 1);
        assert_eq!(data::SCRIPT_COUNT, 0);
        assert!(data::TEXTS.is_empty());
    }

    /// `build_modes` checks the marker: a stale extract builds the stub and says so.
    #[test]
    fn a_stale_extract_is_flagged() {
        if crate::STALE_EXTRACT {
            println!("STALE EXTRACT FLAGGED");
        }
    }
}

/// The map sample: its names and ids come from one list (`samples!`), all distinct; on the full
/// import the build has every one, in order; the stub samples its every map.
#[test]
fn the_map_sample_is_one_list() {
    let ids = samples::map_ids();
    #[cfg(not(pkr_stub))]
    {
        assert_eq!(samples::MAPS.len(), samples::MAP_SAMPLE.len());
        let mut distinct = samples::MAP_SAMPLE.to_vec();
        distinct.sort_by_key(|m| m.0);
        distinct.dedup();
        assert_eq!(distinct.len(), samples::MAP_SAMPLE.len());
        if data::MAP_IDS.len() > samples::MAP_SAMPLE.len() {
            assert_eq!(ids, samples::MAP_SAMPLE);
        }
    }
    #[cfg(pkr_stub)]
    assert_eq!(ids, data::MAP_IDS);
}

#[path = "battle_tests.rs"]
mod battle_tests;

/// The maps' `on_resume` puzzles (`setstepcallback`, `setholewarp`) and the fall script.
#[test]
fn hole_warps_and_step_callbacks() {
    use consts::*;
    assert_eq!(
        Emerald.hole_warp(MAP_GRANITE_CAVE_B1F),
        Some(MAP_GRANITE_CAVE_B2F)
    );
    assert_eq!(Emerald.hole_warp(MAP_SKY_PILLAR_4F), Some(MAP_SKY_PILLAR_3F));
    assert_eq!(Emerald.hole_warp(MAP_ROUTE101), None);
    assert!(matches!(
        Emerald.step_callback(MAP_SOOTOPOLIS_CITY_GYM_1F),
        Some(StepCallback::Ice(_))
    ));
    let Some(StepCallback::CrackedFloor(f)) = Emerald.step_callback(MAP_MT_PYRE_2F) else {
        panic!("Mt. Pyre 2F has cracked floors");
    };
    assert_eq!((f.hole_for(0x22F), f.hole_for(0x22E)), (0x206, 0x237));
    assert_eq!(Emerald.step_callback(MAP_ROUTE101), None);
    // The full import has `EventScript_FallDownHole`; the stub and the build-mode fixture
    // (one map, a few scripts) have no Granite Cave and no fall script.
    assert_eq!(
        Emerald.fall_script().is_some(),
        Emerald.has_map(MAP_GRANITE_CAVE_B1F)
    );
}
