//! World tests of Milestone 8 Task H (items, money, string buffers, multichoice), on the
//! synthetic game of `world/tests.rs` with the Task H tables added ([`HGame`]: items, multichoice
//! lists, std strings and a few texts with string vars; everything else is [`Game`]'s).

use super::*;
use crate::battle::test_data::{NORMAL, TABLES, TACKLE};
use crate::menu::{INPUT_DELAY, MAX_WIDTH};
use crate::script::BufferSrc;
use crate::str_vars::STR_VAR_LEN;

/// Glyph bytes the synthetic font draws (it has glyphs 0..4).
const A: u8 = 1;
const B: u8 = 2;
const C: u8 = 3;
const EOS: u8 = 0xFF;
/// `{STR_VAR_1}`, `{STR_VAR_2}`, `{STR_VAR_3}`, `{PLAYER}`.
const SV1: [u8; 2] = [0xFD, 0x02];
const SV2: [u8; 2] = [0xFD, 0x03];
const SV3: [u8; 2] = [0xFD, 0x04];
const PLAYER_PH: [u8; 2] = [0xFD, 0x01];

const POTION: ItemId = ItemId(13);
const KEY: ItemId = ItemId(259);
const BALL: ItemId = ItemId(4);
const UNKNOWN_ITEM: ItemId = ItemId(999);

static ITEMS: [(ItemId, ItemInfo); 3] = [
    (
        POTION,
        ItemInfo {
            name: &[A, B, EOS],
            pocket: Pocket::Items,
            price: 300,
            key: false,
        },
    ),
    (
        KEY,
        ItemInfo {
            name: &[C, EOS],
            pocket: Pocket::KeyItems,
            price: 0,
            key: true,
        },
    ),
    (
        BALL,
        ItemInfo {
            name: &[B, B, EOS],
            pocket: Pocket::PokeBalls,
            price: 200,
            key: false,
        },
    ),
];

/// List 0: two choices; list 1: five (the cursor wraps); list 2: empty; list 3: one long one.
static TWO: [&[u8]; 2] = [&[A, EOS], &[B, B, EOS]];
static FIVE: [&[u8]; 5] = [&[A, EOS], &[B, EOS], &[C, EOS], &[A, A, EOS], &[B, B, EOS]];
static LONG: [&[u8]; 1] = [&[A; 60]];

/// "A{STR_VAR_1}B{STR_VAR_2}C{STR_VAR_3}".
static VARS_TEXT: [u8; 10] = [A, SV1[0], SV1[1], B, SV2[0], SV2[1], C, SV3[0], SV3[1], EOS];
const T_VARS: TextId = TextId(100);
/// "{PLAYER}C", copied into a string var by `bufferstring`.
static NESTED_TEXT: [u8; 4] = [PLAYER_PH[0], PLAYER_PH[1], C, EOS];
const T_NESTED: TextId = TextId(101);
/// "{STR_VAR_1}": expanding var 1 holding this recurses until the depth limit.
static SELF_TEXT: [u8; 3] = [SV1[0], SV1[1], EOS];
const T_SELF: TextId = TextId(102);

/// [`Game`] plus the Task H tables.
struct HGame;

impl GameData for HGame {
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
        match id {
            T_VARS => &VARS_TEXT,
            T_NESTED => &NESTED_TEXT,
            T_SELF => &SELF_TEXT,
            _ => Game.text(id),
        }
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
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        Game.heal_location(id)
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        ITEMS.iter().find(|(i, _)| *i == id).map(|(_, info)| info)
    }
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
        match list {
            0 => Some(&TWO),
            1 => Some(&FIVE),
            2 => Some(&[]),
            3 => Some(&LONG),
            _ => None,
        }
    }
    fn std_string(&self, id: u8) -> Option<&'static [u8]> {
        (id == 7).then_some(&[C, C, EOS])
    }
}

fn run_h(w: &mut World, keys: Buttons, frames: u32) -> Frame {
    let mut f = Frame::default();
    for _ in 0..frames {
        f = w.step(&HGame, keys);
        f.validate().unwrap();
    }
    f
}

fn started_h() -> World {
    let mut w = World::new(&HGame);
    run_h(&mut w, Buttons::NONE, 16);
    w
}

/// Runs `script` to its end with nothing pressed (at most 600 frames); returns the frames.
fn script_h(w: &mut World, script: impl FnOnce(Ctx) -> Script) -> u32 {
    start(w, script);
    for n in 0..600 {
        if !w.script_running() {
            return n;
        }
        run_h(w, Buttons::NONE, 1);
    }
    panic!("script still running");
}

fn reports(w: &World) -> Vec<Report> {
    (w.events().iter())
        .filter_map(|e| match e {
            Event::Report(r) => Some(*r),
            _ => None,
        })
        .collect()
}

/// A string var up to its terminator.
fn var_text(w: &World, i: usize) -> Vec<u8> {
    w.str_var(i)
        .iter()
        .copied()
        .take_while(|&b| b != EOS)
        .collect()
}

// ---- items and money ----

#[test]
fn items_go_to_their_pocket_and_answer_as_gen3() {
    let mut w = started_h();
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            let mut r = 0u16;
            for (i, ok) in [
                ctx.item_add(POTION, 5).await,
                ctx.item_add(KEY, 1).await,
                ctx.item_check(POTION, 5).await,
                !ctx.item_check(POTION, 6).await,
                ctx.item_space(BALL, 99).await,
                !ctx.item_space(BALL, 16 * 99 + 1).await,
                ctx.item_remove(POTION, 2).await,
                !ctx.item_remove(POTION, 4).await,
            ]
            .into_iter()
            .enumerate()
            {
                r |= (ok as u16) << i;
            }
            ctx.set_var(VAR_RESULT, r).await;
        })
    });
    assert_eq!(w.var(VAR_RESULT), 0xFF, "every answer as expected");
    assert_eq!(w.bag().count(POTION), 3);
    assert_eq!(w.bag().pocket(Pocket::KeyItems)[0].item, KEY);
    assert!(w.events().is_empty(), "nothing reported");
}

#[test]
fn unknown_items_answer_false_and_report_item_none_answers_false() {
    let mut w = started_h();
    start(&mut w, |ctx| {
        Box::pin(async move {
            let a = ctx.item_add(UNKNOWN_ITEM, 1).await;
            let b = ctx.item_check(UNKNOWN_ITEM, 0).await;
            let c = ctx.item_add(ItemId(0), 1).await;
            ctx.set_var(VAR_RESULT, (a || b || c) as u16 + 10).await;
        })
    });
    run_h(&mut w, Buttons::NONE, 1);
    assert_eq!(w.var(VAR_RESULT), 10);
    assert_eq!(
        reports(&w),
        [
            Report::Degraded("additem: unknown item"),
            Report::Degraded("checkitem: unknown item"),
        ]
    );
    assert_eq!(w.bag(), &crate::bag::Bag::new(POCKET_SLOTS));
}

#[test]
fn money_caps_at_max_money_and_stops_at_zero() {
    let mut w = started_h();
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            ctx.money_add(3000).await;
            let enough = ctx.money_check(3000).await && !ctx.money_check(3001).await;
            ctx.money_remove(1000).await;
            ctx.set_var(VAR_RESULT, enough as u16).await;
        })
    });
    assert_eq!(w.var(VAR_RESULT), 1);
    assert_eq!(w.money(), 2000);
    script_h(&mut w, |ctx| {
        Box::pin(async move { ctx.money_add(u32::MAX).await })
    });
    assert_eq!(w.money(), 999_999);
    script_h(&mut w, |ctx| {
        Box::pin(async move { ctx.money_remove(1_000_000).await })
    });
    assert_eq!(w.money(), 0);
}

// ---- string buffers ----

#[test]
fn buffers_write_names_numbers_and_strings() {
    let mut w = started_h();
    let expect_species = TABLES.species[NORMAL.0 as usize].name;
    let take = |t: &[u8]| -> Vec<u8> { t.iter().copied().take_while(|&b| b != EOS).collect() };
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            ctx.buffer(0, BufferSrc::Species(NORMAL)).await;
            ctx.buffer(1, BufferSrc::Item(POTION)).await;
            ctx.buffer(2, BufferSrc::Move(TACKLE)).await;
        })
    });
    assert_eq!(var_text(&w, 0), take(expect_species));
    assert_eq!(var_text(&w, 1), [A, B]);
    assert_eq!(var_text(&w, 2), take(TABLES.moves[TACKLE.0 as usize].name));
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            ctx.buffer(0, BufferSrc::Number(0)).await;
            ctx.buffer(1, BufferSrc::Number(1234)).await;
            ctx.buffer(2, BufferSrc::Std(7)).await;
        })
    });
    // Gen3 `STR_CONV_MODE_LEFT_ALIGN` digits: `CHAR_0` is 0xA1.
    assert_eq!(var_text(&w, 0), [0xA1]);
    assert_eq!(var_text(&w, 1), [0xA2, 0xA3, 0xA4, 0xA5]);
    assert_eq!(var_text(&w, 2), [C, C]);
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(NORMAL, 5).await;
            ctx.buffer(0, BufferSrc::PartyNick(0)).await;
            ctx.buffer(1, BufferSrc::LeadSpecies).await;
            ctx.buffer(2, BufferSrc::Text(T_NESTED)).await;
        })
    });
    let nick = take(&w.party().get(0).unwrap().nickname);
    assert_eq!(var_text(&w, 0), nick);
    assert_eq!(var_text(&w, 1), take(expect_species));
    assert_eq!(var_text(&w, 2), take(&NESTED_TEXT), "copied unexpanded");
    assert!(w.events().is_empty());
}

#[test]
fn unresolved_buffers_write_nothing_and_report() {
    let mut w = started_h();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.buffer(0, BufferSrc::Number(7)).await;
            ctx.buffer(0, BufferSrc::Item(UNKNOWN_ITEM)).await;
            ctx.buffer(1, BufferSrc::LeadSpecies).await;
            ctx.buffer(2, BufferSrc::Std(8)).await;
            ctx.buffer(3, BufferSrc::Number(1)).await;
        })
    });
    run_h(&mut w, Buttons::NONE, 1);
    assert!(!w.script_running());
    assert_eq!(var_text(&w, 0), []);
    assert_eq!(
        reports(&w),
        [
            Report::Degraded("bufferitemname: unknown item"),
            Report::Degraded("bufferleadmonspeciesname: empty party"),
            Report::Degraded("bufferstdstring: unknown string"),
            Report::Degraded("buffer: string var out of range"),
        ]
    );
}

#[test]
fn messages_expand_string_vars_nested_and_bounded() {
    let mut w = started_h();
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            ctx.buffer(0, BufferSrc::Number(42)).await;
            ctx.buffer(1, BufferSrc::Text(T_NESTED)).await;
            ctx.message(T_VARS).await;
        })
    });
    // "A" "42" "B" "{PLAYER}C" (the player is glyph 1) "C" "" (var 3 is empty).
    assert_eq!(w.text().printed(), [A, 0xA5, 0xA3, B, A, C, C, EOS]);
    // A var printing itself stops at the depth limit instead of hanging.
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            ctx.buffer(0, BufferSrc::Text(T_SELF)).await;
            ctx.message(T_VARS).await;
        })
    });
    assert_eq!(w.text().printed()[..2], [A, B]);
}

#[test]
fn expansion_keeps_at_most_gen3s_buffer() {
    let mut w = started_h();
    // Fill every var to the brim: 3 x 255 characters plus the text's own.
    let long = [A; STR_VAR_LEN + 10];
    w.s.str_vars.set(0, &long);
    w.s.str_vars.set(1, &long);
    w.s.str_vars.set(2, &long);
    let out = crate::text::expand_vars(&HGame, &VARS_TEXT, &[A, EOS], Some(&w.s.str_vars));
    assert_eq!(out.len(), 3 + 3 * (STR_VAR_LEN - 1) + 1);
    let mut huge = Vec::new();
    for _ in 0..5 {
        huge.extend_from_slice(&VARS_TEXT[..9]);
    }
    huge.push(EOS);
    let out = crate::text::expand_vars(&HGame, &huge, &[A, EOS], Some(&w.s.str_vars));
    assert_eq!(out.len(), crate::text::EXPANDED_MAX);
    assert_eq!(out.last(), Some(&EOS));
}

#[test]
fn a_full_expansion_stops_at_one_point() {
    // A var ending in a two-byte unit that no longer fits: nothing after it is written, not
    // even the one-byte glyph that would.
    let mut w = started_h();
    w.s.str_vars.set(0, &[0xF9, 0x05, EOS]);
    let max = crate::text::EXPANDED_MAX;
    let mut text = vec![A; max - 2];
    text.extend_from_slice(&[SV1[0], SV1[1], B, EOS]);
    let out = crate::text::expand_vars(&HGame, &text, &[A, EOS], Some(&w.s.str_vars));
    assert_eq!(out.len(), max - 1);
    assert_eq!(out[max - 3..], [A, EOS]);
}

// ---- multichoice ----

/// Starts a multichoice script that stores the answer + 1000 in `VAR_RESULT`.
fn start_menu(w: &mut World, list: u8, default: u8, cancel: bool) {
    start(w, move |ctx| {
        Box::pin(async move {
            let v = ctx.multichoice(list, 2, 2, default, cancel).await;
            ctx.set_var(VAR_RESULT, v + 1000).await;
        })
    });
}

/// Presses `keys` one frame each (waiting a frame between), then waits for the script.
fn answer(w: &mut World, keys: &[Buttons]) -> u16 {
    for &k in keys {
        run_h(w, k, 1);
        run_h(w, Buttons::NONE, 1);
    }
    for _ in 0..10 {
        if !w.script_running() {
            return w.var(VAR_RESULT) - 1000;
        }
        run_h(w, Buttons::NONE, 1);
    }
    panic!("multichoice still open");
}

/// Opens list `list` and waits until it reads input.
fn open(list: u8, default: u8, cancel: bool) -> World {
    let mut w = started_h();
    start_menu(&mut w, list, default, cancel);
    run_h(&mut w, Buttons::NONE, INPUT_DELAY as u32);
    while !w.s.text.multichoice().unwrap().is_drawn() {
        run_h(&mut w, Buttons::NONE, 1);
    }
    w
}

#[test]
fn multichoice_answers_the_index_b_and_the_default() {
    assert_eq!(answer(&mut open(0, 0, true), &[Buttons::A]), 0);
    assert_eq!(
        answer(&mut open(0, 0, true), &[Buttons::DOWN, Buttons::A]),
        1
    );
    assert_eq!(answer(&mut open(0, 1, true), &[Buttons::A]), 1, "default");
    assert_eq!(
        answer(&mut open(0, 9, true), &[Buttons::A]),
        0,
        "bad default"
    );
    assert_eq!(
        answer(&mut open(0, 0, true), &[Buttons::B]),
        MULTI_B_PRESSED
    );
    // Not cancellable: B does nothing.
    assert_eq!(answer(&mut open(0, 1, false), &[Buttons::B, Buttons::A]), 1);
}

#[test]
fn multichoice_wraps_only_past_three_choices() {
    assert_eq!(answer(&mut open(0, 0, true), &[Buttons::UP, Buttons::A]), 0);
    assert_eq!(
        answer(&mut open(0, 1, true), &[Buttons::DOWN, Buttons::A]),
        1
    );
    assert_eq!(answer(&mut open(1, 0, true), &[Buttons::UP, Buttons::A]), 4);
    assert_eq!(
        answer(&mut open(1, 4, true), &[Buttons::DOWN, Buttons::A]),
        0
    );
}

#[test]
fn multichoice_reads_input_from_the_third_update_on() {
    // Gen3 `sProcessInputDelay` 2: A on the first two frames is ignored, on the third read.
    let mut w = started_h();
    start_menu(&mut w, 0, 1, true);
    run_h(&mut w, Buttons::NONE, 1);
    run_h(&mut w, Buttons::A, 1);
    assert!(w.s.text.is_multichoice_active(), "second frame");
    let mut w = started_h();
    start_menu(&mut w, 0, 1, true);
    run_h(&mut w, Buttons::A, 1);
    assert!(w.s.text.is_multichoice_active(), "first frame");
    run_h(&mut w, Buttons::NONE, 1);
    run_h(&mut w, Buttons::A, 1);
    assert!(!w.s.text.is_multichoice_active(), "third frame");
    run_h(&mut w, Buttons::NONE, 1);
    assert_eq!(w.var(VAR_RESULT), 1001);
    assert_eq!(w.s.text.multichoice_result(), None, "taken by the script");
}

#[test]
fn multichoice_draws_one_choice_per_frame_and_erases_its_rows() {
    let mut w = started_h();
    start_menu(&mut w, 1, 0, true);
    let mut rows = Vec::new();
    for _ in 0..5 {
        let f = run_h(&mut w, Buttons::NONE, 1);
        let m = w.s.text.multichoice().unwrap();
        let entries = f
            .cmds
            .iter()
            .filter(|c| matches!(c, Cmd::MapEntries { .. }));
        rows.push(entries.count());
        let glyph_tiles = f.cmds.iter().filter(|c| matches!(c, Cmd::BgTile { .. }));
        assert!(glyph_tiles.count() <= 2 * m.window().width() as usize);
    }
    // The top frame row comes with the first choice, the bottom one with the last.
    assert_eq!(rows, [3, 2, 2, 2, 3]);
    let m = w.s.text.multichoice().unwrap();
    // Width from the longest choice (2 glyphs of 6 px: Gen3 `(12 + 9) / 8 + 1`).
    assert_eq!((m.window().width(), m.window().height()), (3, 10));
    let f = run_h(&mut w, Buttons::A, 1);
    assert!(!w.s.text.is_multichoice_active());
    // Gen3 `ClearToTransparentAndRemoveWindow`: the window's columns of its 12 rows, blanked.
    let blank: Vec<_> = (f.cmds.iter())
        .filter_map(|c| match c {
            Cmd::MapEntries { entries, .. } => Some(entries),
            _ => None,
        })
        .collect();
    assert_eq!(blank.len(), 12);
    assert!(
        blank
            .iter()
            .all(|e| e.len() == 6 && e.iter().all(|&x| x == 0))
    );
}

#[test]
fn multichoice_windows_stay_within_gen3s_width() {
    let mut w = started_h();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.multichoice(3, 20, 2, 0, true).await;
        })
    });
    run_h(&mut w, Buttons::NONE, 1);
    let win = w.s.text.multichoice().unwrap().window();
    // Moved left to 0, then one tile in (Gen3 `CreateWindowFromRect`).
    assert_eq!((win.left(), win.width()), (1, MAX_WIDTH));
}

#[test]
fn unknown_or_empty_lists_answer_b_and_report() {
    for list in [2, 9] {
        let mut w = started_h();
        start_menu(&mut w, list, 0, true);
        run_h(&mut w, Buttons::NONE, 1);
        assert_eq!(w.var(VAR_RESULT), 1000 + MULTI_B_PRESSED);
        assert_eq!(reports(&w), [Report::Degraded("multichoice: unknown list")]);
    }
}

// ---- state ----

/// Items, money, string vars and the multichoice answer live in the hashed, snapshotted state:
/// a world restored from a checkpoint after them hashes the same and goes on the same.
#[test]
fn task_h_state_survives_a_checkpoint() {
    let mut w = started_h();
    let before = w.hash();
    script_h(&mut w, |ctx| {
        Box::pin(async move {
            ctx.item_add(POTION, 120).await;
            ctx.money_add(500).await;
            ctx.buffer(1, BufferSrc::Item(POTION)).await;
        })
    });
    let after = w.hash();
    assert_ne!(after, before);
    let cp = w.checkpoint(&HGame).expect("capturable");
    let mut back = World::restore(&HGame, &cp).expect("restores");
    assert_eq!(back.hash(), after);
    for keys in [Buttons::NONE, Buttons::A, Buttons::NONE] {
        run_h(&mut w, keys, 1);
        run_h(&mut back, keys, 1);
        assert_eq!(back.hash(), w.hash());
    }
    assert_eq!(back.bag().count(POTION), 120);
    assert_eq!(var_text(&back, 1), [A, B]);
}
