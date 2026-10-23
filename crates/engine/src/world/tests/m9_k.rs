//! Milestone 9 Task K: screen hosting (START, requests, fades, the field's return) and the
//! kit's widgets on a game without any UI picture.

use super::*;
use crate::data::{FieldMove, FieldMoveGate, ItemInfo, Pocket, StartAction, StartItem};
use crate::mon::{Ivs, Pokemon};
use crate::rng::Rng;
use crate::script::PARTY_NOTHING_CHOSEN;
use crate::ui::kit::{UPLOAD_BYTES_PER_FRAME, UPLOAD_BYTES_VISIBLE};
use crate::ui::{PartyMode, ScreenKindTag};
use crate::video::{Cmd, layout};

/// A TM and an HM (item ids), the moves they teach, and the flag a start-menu entry needs.
pub(super) const TM: ItemId = ItemId(300);
pub(super) const HM: ItemId = ItemId(301);
pub(super) const TM_MOVE: MoveId = crate::battle::test_data::EMBER;
pub(super) const HM_MOVE: MoveId = crate::battle::test_data::MUD_SLAP;
pub(super) const FLAG_POKEMON_MENU: FlagId = FlagId(0x30);
/// Species that cannot learn [`TM`].
pub(super) const NO_TM: SpeciesId = crate::battle::test_data::WATER;

static START: [StartItem; 3] = [
    StartItem {
        label: TextId(0),
        action: StartAction::Party,
        shown_if: Some(FLAG_POKEMON_MENU),
        description: None,
    },
    StartItem {
        label: TextId(0),
        action: StartAction::Options,
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

static TM_INFO: ItemInfo = ItemInfo {
    name: &[1, 0xFF],
    pocket: Pocket::TmHm,
    price: 3000,
    key: false,
};

/// [`Game`] with a start menu, a TM and an HM, a field-move gate and no UI picture.
pub(super) struct UiGame;

impl GameData for UiGame {
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
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        (id == TM || id == HM).then_some(&TM_INFO)
    }
    fn tm_move(&self, id: ItemId) -> Option<MoveId> {
        match id {
            TM => Some(TM_MOVE),
            HM => Some(HM_MOVE),
            _ => None,
        }
    }
    fn can_learn_tm(&self, species: SpeciesId, _: ItemId) -> bool {
        species != NO_TM
    }
    fn is_hm_move(&self, m: MoveId) -> bool {
        m == HM_MOVE
    }
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        match m {
            FieldMove::RockSmash => FieldMoveGate::PartyMove {
                move_id: HM_MOVE,
                badge: None,
            },
            _ => FieldMoveGate::Never,
        }
    }
}

/// UiGame's world after the new-game fade, with `mons` (species, moves) in the party.
pub(super) fn ui_world(mons: &[(SpeciesId, &[MoveId])]) -> World {
    let mut w = World::new(&UiGame);
    ui_run(&mut w, Buttons::NONE, 16);
    let t = UiGame.battle();
    let mut rng = Rng::new(7);
    for &(species, moves) in mons {
        let mut p = Pokemon::new(t, species, 10, Ivs::Fixed(10), 0, 1, &mut rng);
        p.moves = [None; 4];
        for (i, m) in moves.iter().enumerate() {
            p.moves[i] = Some(*m);
            p.pp[i] = t.move_info(*m).pp;
        }
        w.s.party.push(p).unwrap();
    }
    w
}

/// Runs `frames` frames holding `keys`, checking each frame and its upload budget.
pub(super) fn ui_run(w: &mut World, keys: Buttons, frames: u32) -> Frame {
    let mut f = Frame::default();
    for _ in 0..frames {
        f = w.step(&UiGame, keys);
        f.validate().unwrap();
        check_budget(&f);
    }
    f
}

/// One frame pressing `keys`, then `frames - 1` idle frames.
pub(super) fn ui_tap(w: &mut World, keys: Buttons, frames: u32) {
    ui_run(w, keys, 1);
    ui_run(w, Buttons::NONE, frames - 1);
}

/// VRAM bytes a frame copies (tiles, maps, palettes).
pub(super) fn upload_bytes(f: &Frame) -> usize {
    crate::ui::kit::frame_bytes(f)
}

/// A screen frame stays within the kit's budgets (black frames: the bulk budget; the field's
/// own spread restore is the field's budget, checked by its tests).
fn check_budget(f: &Frame) {
    let n = upload_bytes(f);
    if f.is_black() || f.bulk {
        assert!(
            n <= UPLOAD_BYTES_PER_FRAME + 8192,
            "black frame uploads {n} bytes"
        );
    } else {
        assert!(n <= UPLOAD_BYTES_VISIBLE, "visible frame uploads {n} bytes");
    }
}

fn bg_enabled(f: &Frame, layer: u8) -> Option<bool> {
    f.cmds.iter().rev().find_map(|c| match c {
        Cmd::Bg { layer: l, control } if *l == layer => Some(control.enabled),
        _ => None,
    })
}

#[test]
fn start_does_nothing_in_a_game_without_a_start_menu() {
    let mut w = started();
    let h = (w.map(), pos(&w), w.objects().player().facing);
    tap(&mut w, Buttons::START, 3);
    assert_eq!(w.screens_open(), 0);
    assert_eq!((w.map(), pos(&w), w.objects().player().facing), h);
}

#[test]
fn start_opens_the_start_menu_over_the_field_and_b_closes_it() {
    let mut w = ui_world(&[]);
    let objs = ui_run(&mut w, Buttons::NONE, 1).objs;
    let f = ui_run(&mut w, Buttons::START, 1);
    assert_eq!(bg_enabled(&f, layout::TEXT_BG), Some(true));
    assert!(
        w.s.events
            .contains(&Event::ScreenOpened(ScreenKindTag::StartMenu))
    );
    let f = ui_run(&mut w, Buttons::NONE, 1);
    assert_eq!(w.screens_open(), 1);
    // The field stays drawn (objects) and the menu window is on BG0.
    assert_eq!(f.objs, objs);
    assert!(f.cmds.iter().any(|c| matches!(
        c,
        Cmd::MapEntries {
            screen_block: 31,
            ..
        }
    )));
    // The Pokémon entry is hidden until its flag is set: two entries (OPTION, EXIT).
    let f = ui_run(&mut w, Buttons::B, 1);
    assert_eq!(w.screens_open(), 0);
    assert!(w.field_active());
    assert_eq!(bg_enabled(&f, layout::TEXT_BG), Some(false));
    assert!(w.text().is_pristine());
    // START works again at once.
    ui_run(&mut w, Buttons::NONE, 1);
    ui_tap(&mut w, Buttons::START, 2);
    assert_eq!(w.screens_open(), 1);
    ui_tap(&mut w, Buttons::START, 2);
    assert_eq!(w.screens_open(), 0);
}

#[test]
fn exit_closes_and_an_unsupported_entry_reports() {
    let mut w = ui_world(&[]);
    ui_tap(&mut w, Buttons::START, 2);
    // OPTION (the first shown entry): reported, the menu stays.
    ui_tap(&mut w, Buttons::A, 2);
    assert_eq!(w.screens_open(), 1);
    assert!(w.log().lines().any(|l| l.contains("options")));
    ui_tap(&mut w, Buttons::DOWN, 2);
    ui_tap(&mut w, Buttons::A, 2);
    assert_eq!(w.screens_open(), 0);
}

/// START → POKéMON (fade out, the party menu alone on BG0, fade in) → B (fade out, the field
/// reloads under black, the start menu over it again) → B: the field is back as it was.
#[test]
fn party_from_start_returns_to_the_start_menu_then_the_field() {
    let mut w = ui_world(&[(crate::battle::test_data::NORMAL, &[TM_MOVE])]);
    w.s.vars.set_flag(FLAG_POKEMON_MENU, true);
    let before = (
        w.map(),
        pos(&w),
        w.objects().player().facing,
        w.party().len(),
    );
    let objs = ui_run(&mut w, Buttons::NONE, 1).objs;
    ui_tap(&mut w, Buttons::START, 2);
    ui_run(&mut w, Buttons::A, 1);
    // The fade out: 16 frames, field still drawn.
    let mut frames = Vec::new();
    for _ in 0..40 {
        frames.push(ui_run(&mut w, Buttons::NONE, 1));
    }
    let entered = frames
        .iter()
        .position(|f| bg_enabled(f, layout::FIELD_TOP_BG) == Some(false))
        .expect("the field's layers go off");
    assert_eq!(entered, kit_fade() - 1);
    assert!(frames[entered].is_black() && frames[entered].objs.is_empty());
    assert_eq!(frames.last().unwrap().brightness, 0);
    assert_eq!(
        w.screens().unwrap().kinds().last(),
        Some(crate::ui::ScreenKind::Party(PartyMode::Field))
    );
    // B: back to the start menu over the reloaded field.
    ui_run(&mut w, Buttons::B, 1);
    let mut back = Vec::new();
    for _ in 0..80 {
        back.push(ui_run(&mut w, Buttons::NONE, 1));
    }
    assert!(
        back.iter()
            .any(|f| bg_enabled(f, layout::FIELD_TOP_BG) == Some(true))
    );
    assert_eq!(w.screens_open(), 1);
    let last = back.last().unwrap();
    assert_eq!(last.brightness, 0);
    assert_eq!(last.objs, objs);
    ui_tap(&mut w, Buttons::B, 2);
    assert!(w.field_active());
    assert_eq!(
        (
            w.map(),
            pos(&w),
            w.objects().player().facing,
            w.party().len()
        ),
        before
    );
    // The field moves again.
    walk_ui(&mut w);
}

fn kit_fade() -> usize {
    crate::ui::kit::FADE_FRAMES as usize
}

fn walk_ui(w: &mut World) {
    let p = pos(w);
    for _ in 0..40 {
        ui_run(w, Buttons::DOWN, 1);
    }
    assert_ne!(pos(w), p, "the player walks after the screens closed");
}

#[test]
fn choose_party_mon_answers_the_slot_or_nothing_chosen() {
    let mons: &[(SpeciesId, &[MoveId])] = &[
        (crate::battle::test_data::NORMAL, &[TM_MOVE]),
        (crate::battle::test_data::NORMAL, &[TM_MOVE]),
    ];
    for (keys, want) in [
        (&[Buttons::DOWN, Buttons::A][..], 1),
        (&[Buttons::B][..], PARTY_NOTHING_CHOSEN),
        (&[Buttons::A][..], 0),
    ] {
        let mut w = ui_world(mons);
        let ctx = w.runner.ctx();
        w.runner.start(Box::pin(async move {
            let v = ctx.choose_party_mon(PartyMode::Choose).await;
            ctx.set_var(VAR_RESULT, v).await;
        }));
        ui_run(&mut w, Buttons::NONE, 40);
        assert_eq!(w.screens_open(), 1);
        for &k in keys {
            ui_tap(&mut w, k, 2);
        }
        for _ in 0..120 {
            if !w.script_running() {
                break;
            }
            ui_run(&mut w, Buttons::NONE, 1);
        }
        assert!(!w.script_running());
        assert_eq!(w.var(VAR_RESULT), want);
        assert!(w.field_active());
    }
}

#[test]
fn a_map_script_request_never_opens_a_screen() {
    let mut w = ui_world(&[]);
    let mut f = Frame::default();
    let mut host = super::super::host::Host {
        game: &UiGame,
        s: &mut w.s,
        f: &mut f,
        immediate: true,
    };
    use crate::script::ScriptHost;
    let r = host.begin(&crate::script::Request::ChoosePartyMon(PartyMode::Choose));
    assert_eq!(
        r,
        Some(crate::script::Response::Value(PARTY_NOTHING_CHOSEN))
    );
    assert_eq!(w.screens_open(), 0);
}

/// Window geometry against the decomp (skipped without `PKR_DECOMP`).
#[test]
fn window_templates_match_the_decomp() {
    let Some(root) = std::env::var_os("PKR_DECOMP") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap_or_default();
    let field = |src: &str, name: &str| -> Vec<u8> {
        let Some(at) = src.find(name) else {
            return Vec::new();
        };
        let body = &src[at..];
        let body = &body[..body.find("};").unwrap_or(body.len())];
        ["tilemapLeft", "tilemapTop", "width", "height"]
            .iter()
            .filter_map(|k| {
                let i = body.find(k)?;
                let rest = &body[i..];
                let v = rest[rest.find('=')? + 1..].trim_start();
                let end = v.find(|c: char| !c.is_ascii_digit()).unwrap_or(v.len());
                v[..end].parse().ok()
            })
            .collect()
    };
    let party = read("src/data/party_menu.h");
    if party.is_empty() {
        return;
    }
    use crate::ui::party;
    let tuple = |r: (u8, u8, u8, u8)| vec![r.0, r.1, r.2, r.3];
    assert_eq!(
        field(&party, "sCancelButtonWindowTemplate ="),
        tuple(party::CANCEL_RECT)
    );
    assert_eq!(
        field(&party, "sDefaultPartyMsgWindowTemplate ="),
        tuple(party::HINT_RECT)
    );
    assert_eq!(
        field(&party, "sPartyMenuYesNoWindowTemplate ="),
        tuple(crate::ui::kit::widgets::YES_NO_RECT)
    );
    assert_eq!(
        field(&party, "sItemGiveTakeWindowTemplate ="),
        tuple(party::ITEM_RECT)
    );
    let single = &party[party.find("sSinglePartyMenuWindowTemplate[]").unwrap()..];
    for (i, r) in party::SLOT_RECTS.iter().enumerate() {
        let at = single.find(&format!("{{ // Party mon {}", i + 1)).unwrap();
        assert_eq!(field(&single[at..], "{"), tuple(*r), "slot {i}");
    }
}

/// Every widget draws with a game that has no UI picture, no user frame and empty texts.
#[test]
fn widgets_draw_without_pictures() {
    use crate::ui::kit::widgets::{Quantity, YesNo, coin_box, money_box};
    use crate::ui::kit::window::{FrameKind, MapBuf, TileAlloc, enter_full, frame_kind};
    let mut f = Frame::default();
    assert_eq!(enter_full(&UiGame, 0, &mut f), FrameKind::Dialogue);
    assert_eq!(frame_kind(&UiGame, 3), FrameKind::Dialogue);
    let mut tiles = TileAlloc::full();
    let mut money = money_box(&UiGame, &mut tiles, 123_456, 0, 0);
    let mut coins = coin_box(&UiGame, &mut tiles, 42, 0, 5);
    let mut yn = YesNo::new(&UiGame, &mut tiles, true);
    assert_eq!(yn.input(&UiGame, Buttons::B), Some(false));
    let mut map = MapBuf::full(FrameKind::Dialogue);
    map.put(&money);
    map.put(&coins);
    map.put(&yn.panel);
    map.flush(&mut f);
    money.flush(&mut f);
    coins.flush(&mut f);
    yn.panel.flush(&mut f);
    f.validate().unwrap();
    // The money box's frame: a dialogue-frame corner at (0, 0), its canvas from (1, 1).
    assert_eq!(map.entry(0, 0) & 0x3FF, crate::text::FRAME_BASE + 1);
    assert_eq!(map.entry(1, 1) & 0x3FF, crate::ui::kit::window::FULL_BASE);
    assert_eq!(Quantity::new(5).text(2).last(), Some(&0xFF));
}
