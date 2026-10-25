//! The PokéNav (Gen3 `pokenav.c`): START → POKéNAV opens it, MATCH CALL lists the
//! registered contacts, and CALL places the call.

use super::*;
use crate::data::{MatchCall, MatchCallLine, StartAction, StartItem};
use crate::ui::kit::{UPLOAD_BYTES_PER_FRAME, UPLOAD_BYTES_VISIBLE};
use crate::ui::{ScreenKind, ScreenKindTag, ScreenState};
use crate::video::Frame;

const FLAG_STONE: FlagId = FlagId(0x41);

/// "MR. STONE"
const NAME: &[u8] = &[0xC7, 0xCC, 0xAD, 0x00, 0xCD, 0xCE, 0xC9, 0xC8, 0xBF, 0xFF];
/// The first call.
const CALL: &[u8] = &[0xC2, 0xBF, 0xC6, 0xC6, 0xC9, 0xFF];
/// Spoken once [`FLAG_STONE`] is set.
const CALL_AGAIN: &[u8] = &[0xBB, 0xC1, 0xBB, 0xC3, 0xC8, 0xFF];

static LINES: [MatchCallLine; 2] = [
    MatchCallLine {
        text: CALL,
        requires: None,
        sets: Some(FLAG_STONE),
    },
    MatchCallLine {
        text: CALL_AGAIN,
        requires: Some(FLAG_STONE),
        sets: None,
    },
];

static CALLS: [MatchCall; 1] = [MatchCall {
    name: NAME,
    desc: NAME,
    shown_if: None,
    lines: &LINES,
}];

static START: [StartItem; 2] = [
    StartItem {
        label: TextId(0),
        action: StartAction::Pokenav,
        shown_if: None,
        description: None,
    },
    StartItem {
        label: TextId(0),
        action: StartAction::Exit,
        shown_if: None,
        description: None,
    },
];

struct NavGame;

impl GameData for NavGame {
    fn map(&self, id: MapId) -> &'static MapData {
        Game.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        Game.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        Game.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        Game.player_sprite()
    }
    fn font(&self) -> &'static Font {
        Game.font()
    }
    fn window(&self) -> &'static WindowGfx {
        Game.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        Game.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Game.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        Game.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        Game.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn start_menu(&self) -> &'static [StartItem] {
        &START
    }
    fn match_calls(&self) -> &'static [MatchCall] {
        &CALLS
    }
}

fn run(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        let f = w.step(&NavGame, keys);
        f.validate().unwrap();
        let n = crate::ui::kit::frame_bytes(&f);
        if f.is_black() || f.bulk {
            assert!(
                n <= UPLOAD_BYTES_PER_FRAME + 8192,
                "black frame uploads {n} bytes"
            );
        } else {
            assert!(n <= UPLOAD_BYTES_VISIBLE, "visible frame uploads {n} bytes");
        }
    }
}

fn tap(w: &mut World, keys: Buttons) {
    run(w, keys, 1);
    run(w, Buttons::NONE, 1);
}

fn world() -> World {
    let mut w = World::new(&NavGame);
    run(&mut w, Buttons::NONE, 16);
    w
}

fn nav(w: &World) -> &crate::ui::pokenav::Pokenav {
    match w.screens().and_then(|s| s.top()) {
        Some((ScreenKind::Pokenav { .. }, ScreenState::Pokenav(p))) => p,
        other => panic!("PokéNav is not open: {other:?}"),
    }
}

/// START → POKéNAV → MATCH CALL → the contact → CALL. The spoken line is the contact's,
/// and it sets the line's flag. Calling again speaks the later line.
#[test]
fn start_menu_opens_the_pokenav_and_a_call_is_read() {
    let mut w = world();
    tap(&mut w, Buttons::START);
    run(&mut w, Buttons::A, 1);
    assert!(
        w.s.events
            .contains(&Event::ScreenOpened(ScreenKindTag::Pokenav)),
        "events: {:?}",
        w.s.events
    );
    run(&mut w, Buttons::NONE, 1);
    // The full-screen fade.
    run(&mut w, Buttons::NONE, 40);
    assert!(matches!(
        w.screens().and_then(|s| s.top()).map(|(k, _)| k),
        Some(ScreenKind::Pokenav { tutorial: false })
    ));
    tap(&mut w, Buttons::A);
    assert_eq!(nav(&w).list_name(), Some(NAME));
    tap(&mut w, Buttons::A);
    tap(&mut w, Buttons::A);
    assert_eq!(nav(&w).call(), Some(CALL));
    assert!(w.s.vars.flag(FLAG_STONE));

    // Dismiss the call, then call again: the line that required the flag.
    let mut again = false;
    for _ in 0..40 {
        tap(&mut w, Buttons::A);
        if nav(&w).call() == Some(CALL_AGAIN) {
            again = true;
            break;
        }
    }
    assert!(again, "the second call did not speak the later line");
}

/// `OpenPokenavForTutorial` (`POKENAV_MODE_FORCE_CALL_READY`): A places Mr. Stone's call,
/// which sets his flag, then A leaves the screen.
#[test]
fn the_tutorial_opens_the_pokenav_and_the_call_sets_the_flag() {
    let mut w = world();
    let mut f = Frame::default();
    crate::world::screens::open(
        &mut w.s,
        &NavGame,
        ScreenKind::Pokenav { tutorial: true },
        true,
        &mut f,
    );
    let mut read = false;
    for _ in 0..80 {
        tap(&mut w, Buttons::A);
        if w.screens().is_some() && nav(&w).call() == Some(CALL) {
            read = true;
        }
        if read && w.screens_open() == 0 {
            break;
        }
    }
    assert!(read, "the tutorial call was not shown");
    assert!(w.s.vars.flag(FLAG_STONE));
    assert_eq!(w.screens_open(), 0);
}
