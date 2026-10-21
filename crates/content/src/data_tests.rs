//! Tests of the generated tables (both modes) and of the prelude helpers.

use super::*;
use crate::prelude::{MsgBox, msgbox};
use engine::data::{Gfx, block_collision};
use engine::script::{Request, Response, Runner, ScriptHost};
use std::vec::Vec;

/// Answers every request at once with a value of the right kind; records them.
#[derive(Default)]
struct Host {
    log: Vec<Request>,
    yes: bool,
    vars: std::collections::BTreeMap<u16, u16>,
}

impl ScriptHost for Host {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        Some(match *r {
            Request::GetFlag(_) => Response::Bool(false),
            Request::GetVar(v) => Response::Value(self.vars.get(&v.0).copied().unwrap_or(0)),
            Request::SetVar(v, x) => {
                self.vars.insert(v.0, x);
                Response::Unit
            }
            Request::Random(_) | Request::PlayerFacing => Response::Value(0),
            Request::PlayerPos => Response::Pos(0, 0),
            Request::YesNo => Response::Bool(self.yes),
            _ => Response::Unit,
        })
    }

    fn resume(&mut self, _: &Request) -> Option<Response> {
        Some(Response::Unit)
    }
}

fn run(script: impl FnOnce(Ctx) -> Script, host: &mut Host) {
    let mut r = Runner::new();
    r.start(script(r.ctx()));
    for _ in 0..100 {
        if !r.step(host) {
            return;
        }
    }
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
                Request::YesNo
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
fn ui_texts_are_french() {
    let d = |b: &[u8]| {
        extract::text::decode_lang(b, extract::text::Lang::Fr)
            .unwrap()
            .0
    };
    assert_eq!(d(Emerald.ui_text(UiText::Yes)), "OUI");
    assert_eq!(d(Emerald.ui_text(UiText::No)), "NON");
    assert_eq!(d(Emerald.ui_text(UiText::RivalName)), "FLORA");
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
    assert_eq!(Emerald.behavior(mb("MB_NON_ANIMATED_DOOR")), Behavior::Warp);
    assert_eq!(
        Emerald.behavior(mb("MB_NORTH_ARROW_WARP")),
        Behavior::ArrowWarp(Dir::Up)
    );
    assert_eq!(Emerald.behavior(mb("MB_COUNTER")), Behavior::Counter);
    assert_eq!(Emerald.behavior(mb("MB_TALL_GRASS")), Behavior::TallGrass);
    assert_eq!(Emerald.behavior(mb("MB_POND_WATER")), Behavior::Blocked);
    assert_eq!(Emerald.behavior(mb("MB_NORMAL")), Behavior::Normal);
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

#[test]
fn unknown_maps_are_missing() {
    assert!(!has_map(consts::MAP_PETALBURG_CITY));
    assert!(core::ptr::eq(
        Emerald.map(consts::MAP_PETALBURG_CITY),
        &data::MISSING_MAP
    ));
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
        run(|ctx| Emerald.script(ScriptId(id as u16), ctx), &mut h);
        assert!(!h.log.is_empty(), "{name}");
    }
}

/// Only does something on the fixture of `tests/build_modes.rs`: its hand-written
/// `StubTown_Obj1` runs through msgbox, a movement table, a label call, a cross-file call,
/// special, callstd, unsupported and a warp.
#[test]
fn fixture_script_runs_through_the_prelude() {
    let Some(id) = data::SCRIPT_NAMES
        .iter()
        .position(|&n| n == "StubTown_Obj1")
    else {
        return;
    };
    let mut h = Host::default();
    run(|ctx| Emerald.script(ScriptId(id as u16), ctx), &mut h);
    let text = |name| TextId(data::TEXT_NAMES.iter().position(|&n| n == name).unwrap() as u16);
    let log = &h.log;
    assert_eq!(log[2], Request::Message(text("StubTown_Obj1_T0")));
    assert!(
        log.iter()
            .any(|r| matches!(r, Request::Move { obj: 0xFF, steps } if steps.len() == 3))
    );
    assert!(log.contains(&Request::SetFlag(
        consts::FLAG_HIDE_LITTLEROOT_TOWN_RIVAL,
        true
    )));
    assert!(log.contains(&Request::Message(text("Other_Main_T0"))));
    assert!(log.contains(&Request::Log("SPECIAL_HealPlayerParty")));
    assert!(log.contains(&Request::Log("STD_OBTAIN_ITEM")));
    assert!(log.contains(&Request::Log("playse")));
    assert!(log.contains(&Request::Warp {
        map: consts::MAP_PETALBURG_CITY,
        warp: Some(0),
        x: 0,
        y: 0
    }));
    assert_eq!(log.last(), Some(&Request::Log("FIXTURE SCRIPT DONE")));
    let m = Emerald.map(Emerald.new_game().map);
    assert_eq!(m.objects[0].script, Some(ScriptId(id as u16)));
    assert_eq!(m.objects[0].hide_flag, Some(engine::data::FlagId(0x1AC)));
    let t = Emerald.text(text("StubTown_Obj1_T0"));
    let (s, _) = extract::text::decode_lang(t, extract::text::Lang::Fr).unwrap();
    assert_eq!(s, "BONJOUR {PLAYER}!\\nÇa va?");
    std::println!("FIXTURE SCRIPT OK");
}

#[test]
fn every_text_decodes() {
    let lang = extract::text::Lang::Fr;
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

#[cfg(not(pkr_stub))]
mod extracted {
    use super::*;

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
        assert_eq!(data::MAP_COUNT, 15);
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
                .position(|&n| n == "LittlerootTown_Obj1")
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
}
