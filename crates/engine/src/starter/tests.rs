use super::*;
use crate::battle::{BattleGfx, EMPTY_GFX, GfxImage, Str};
use crate::data::{
    Behavior, Dir, GfxId, MapData, MapId, NewGame, ScriptId, SpriteData, TextId, UiText, WindowGfx,
};
use crate::mon::{Ability, BattleTables, EMPTY_TABLES, GrowthRate, MonGfx, SpeciesInfo, Type};
use crate::script::{Ctx, Script};
use alloc::boxed::Box;

const A: Buttons = Buttons::A;
const B: Buttons = Buttons::B;
const NONE: Buttons = Buttons::NONE;

/// A `.latfont` glyph with every pixel of its `w` columns set to the 2bpp value `v`.
fn solid_glyph(w: usize, v: u16) -> [u8; 64] {
    let mut g = [0u8; 64];
    for y in 0..16 {
        for x in 0..w {
            let off = ((y / 8) * 2 + x / 8) * 16 + (y % 8) * 2;
            let word = u16::from_le_bytes([g[off], g[off + 1]]) | (v << (14 - 2 * (x % 8)));
            g[off..off + 2].copy_from_slice(&word.to_le_bytes());
        }
    }
    g
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// Glyph 1: 6 px of fg; glyph 2: 4 px of shadow; every other glyph 0 px wide.
fn font() -> &'static Font {
    let mut glyphs = vec![[0u8; 64]; 0x200];
    let mut widths = vec![0u8; 0x200];
    glyphs[1] = solid_glyph(6, 1);
    widths[1] = 6;
    glyphs[2] = solid_glyph(4, 2);
    widths[2] = 4;
    widths[0xEF] = 6;
    Box::leak(Box::new(Font {
        glyphs: leak(glyphs),
        widths: leak(widths),
    }))
}

fn tiles(n: usize, seed: u32) -> &'static [Tile] {
    leak((0..n as u32).map(|i| [seed + i; 8]).collect())
}

fn species(name: &'static [u8], gfx: Option<&'static MonGfx>) -> SpeciesInfo {
    SpeciesInfo {
        name,
        base: [1; 6],
        types: [Type::Normal; 2],
        catch_rate: 0,
        exp_yield: 0,
        ev_yield: [0; 6],
        gender_ratio: 0,
        friendship: 0,
        growth: GrowthRate::MediumFast,
        abilities: [Ability::None; 2],
        learnset: &[],
        weight: 0,
        gfx,
    }
}

fn pic(seed: u32) -> &'static MonGfx {
    Box::leak(Box::new(MonGfx {
        front: tiles(64, seed),
        back: tiles(64, seed + 100),
        palette: Box::leak(Box::new([seed as u16; 16])),
        shiny: Box::leak(Box::new([0; 16])),
        front_y: 0,
        back_y: 0,
        elevation: 0,
    }))
}

fn image(n: usize, seed: u32, map: usize) -> GfxImage {
    GfxImage {
        tiles: tiles(n, seed),
        palettes: leak(vec![[seed as u16; 16], [seed as u16 + 1; 16]]),
        tilemap: leak((0..map as u16).collect()),
    }
}

struct Game {
    font: &'static Font,
    window: &'static WindowGfx,
    tables: &'static BattleTables,
    gfx: &'static BattleGfx,
    question: &'static [u8],
    birch: &'static [u8],
}

fn game() -> Game {
    // Species 0 has no pic (as a build that left it out), 1 and 2 have one.
    let species = leak(vec![
        species(&[1, 1, 0xFF], None),
        species(&[1, 2, 1, 0xFF], Some(pic(1000))),
        species(&[2, 0xFF], Some(pic(2000))),
    ]);
    Game {
        font: font(),
        window: Box::leak(Box::new(WindowGfx {
            tiles: tiles(14, 0),
            palette: Box::leak(Box::new(core::array::from_fn(|i| i as u16))),
            text_fg: 2,
            text_shadow: 3,
            text_bg: 1,
        })),
        tables: Box::leak(Box::new(BattleTables {
            species,
            ..EMPTY_TABLES
        })),
        gfx: Box::leak(Box::new(BattleGfx {
            starter_bag: image(256, 10, 2048),
            starter_ball: image(64, 20, 0),
            starter_circle: image(64, 30, 0),
            ..EMPTY_GFX
        })),
        question: &[1, 1, 1, 0xFF],
        birch: &[1, 2, 1, 2, 0xFF],
    }
}

impl GameData for Game {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!()
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        self.font
    }
    fn window(&self) -> &'static WindowGfx {
        self.window
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        match t {
            UiText::Yes => &[1, 0xFF],
            UiText::No => &[2, 0xFF],
            UiText::RivalName => &[0xFF],
            _ => &[0xFF],
        }
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[0xFF],
        }
    }
    fn battle(&self) -> &'static BattleTables {
        self.tables
    }
    fn battle_text(&self, s: Str) -> &'static [u8] {
        match s {
            Str::ConfirmStarterChoice => self.question,
            Str::BirchInTrouble => self.birch,
            _ => &[0xFF],
        }
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        self.gfx
    }
}

const CHOICES: [SpeciesId; 3] = [SpeciesId(0), SpeciesId(1), SpeciesId(2)];

/// A screen loaded and entered (the label drawn), and the frames that did it.
fn loaded(g: &Game) -> (StarterChoice, Vec<Frame>) {
    loaded_as(g, [0xFF; 8])
}

/// [`loaded`] for a player named `player`.
fn loaded_as(g: &Game, player: PlayerName) -> (StarterChoice, Vec<Frame>) {
    let mut s = StarterChoice::new(CHOICES, player);
    let mut frames = Vec::new();
    loop {
        let mut f = Frame::default();
        let done = s.load_step(g, &mut f);
        frames.push(f);
        if done {
            break;
        }
    }
    frames.push(step(&mut s, g, NONE));
    (s, frames)
}

fn step(s: &mut StarterChoice, g: &Game, keys: Buttons) -> Frame {
    let mut f = Frame::default();
    let r = s.update(
        g,
        &Input {
            held: keys,
            pressed: keys,
        },
        &mut f,
    );
    assert_eq!(f.validate(), Ok(()));
    assert_eq!(r.is_some(), s.stage == Stage::Done);
    f
}

/// The label entries a frame writes: (left, top) of a drawn label, or the cleared one.
fn label_writes(f: &Frame) -> Vec<(u16, bool)> {
    f.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::MapEntries {
                screen_block: 31,
                start,
                entries,
            } if entries.len() == LABEL_W => Some((*start, entries[0] != 0)),
            _ => None,
        })
        .collect()
}

fn label_start(sel: usize) -> u16 {
    let (l, t) = LABEL_POS[sel];
    t as u16 * 32 + l as u16
}

#[test]
fn loads_in_black_chunks() {
    let g = game();
    let (s, frames) = loaded(&g);
    let load = &frames[..frames.len() - 1];
    assert_eq!(load.len(), 5);
    for f in load {
        assert!(f.is_black());
        assert!(f.objs.is_empty());
        assert_eq!(f.validate(), Ok(()));
    }
    let cmds: Vec<&Cmd> = load.iter().flat_map(|f| &f.cmds).collect();
    let has = |p: &dyn Fn(&Cmd) -> bool| cmds.iter().any(|c| p(c));
    assert!(has(
        &|c| matches!(c, Cmd::BgTiles { char_block: 0, first: 0, tiles } if tiles.len() == 256)
    ));
    assert!(has(
        &|c| matches!(c, Cmd::BgPalette { bank: 1, colors } if colors[0] == 11)
    ));
    for (sb, first) in [(6u8, 0u16), (7, 1024)] {
        assert!(has(
            &|c| matches!(c, Cmd::MapEntries { screen_block, start: 0, entries }
            if *screen_block == sb && entries.len() == 1024 && entries[0] == first)
        ));
    }
    for (layer, sb, prio) in [(3u8, 6u8, 1u8), (2, 7, 3)] {
        assert!(has(&|c| matches!(c, Cmd::Bg { layer: l, control }
            if *l == layer && control.enabled && control.screen_block == sb
                && control.priority == prio && control.char_block == 0)));
    }
    assert!(has(
        &|c| matches!(c, Cmd::Bg { layer: 1, control } if !control.enabled)
    ));
    assert!(has(
        &|c| matches!(c, Cmd::ObjTiles { first: 512, tiles } if tiles[0][0] == 20)
    ));
    assert!(has(
        &|c| matches!(c, Cmd::ObjTiles { first: 576, tiles } if tiles[0][0] == 30)
    ));
    // Species 0 has no pic: nothing at 640; 1 and 2 at 704 and 768.
    assert!(!has(&|c| matches!(c, Cmd::ObjTiles { first: 640, .. })));
    assert!(has(
        &|c| matches!(c, Cmd::ObjTiles { first: 704, tiles } if tiles[0][0] == 1000)
    ));
    assert!(has(
        &|c| matches!(c, Cmd::ObjTiles { first: 768, tiles } if tiles[0][0] == 2000)
    ));
    assert!(has(
        &|c| matches!(c, Cmd::ObjPalette { bank: 4, colors } if colors[0] == 2000)
    ));
    // Label rows 0..2 are filled with the window's text color (2).
    assert!(has(
        &|c| matches!(c, Cmd::BgTile { char_block: 2, index: 200, tile } if tile[0] == 0x2222_2222)
    ));
    // The text box is set up last.
    assert!(
        load[4]
            .cmds
            .iter()
            .any(|c| matches!(c, Cmd::Bg { layer: 0, control } if control.enabled))
    );
    assert_eq!(s.stage, Stage::Choose);
}

#[test]
fn enters_on_the_middle_ball_and_fades_in() {
    let g = game();
    let (mut s, frames) = loaded(&g);
    let enter = frames.last().unwrap();
    assert_eq!(s.selection(), 1);
    assert_eq!(label_writes(enter)[0], (label_start(1), true));
    assert_eq!(enter.brightness, 15);
    let mut last = enter.brightness;
    for _ in 0..15 {
        let f = step(&mut s, &g, NONE);
        assert_eq!(f.brightness, last - 1);
        last = f.brightness;
    }
    assert_eq!(last, 0);
    // Hand above the middle ball, then three balls; only the middle one animates.
    let f = step(&mut s, &g, NONE);
    assert_eq!(f.objs.len(), 4);
    assert_eq!((f.objs[0].x, f.objs[0].tile), (120 - 16, 512 + 48));
    assert_eq!(
        f.objs[1..].iter().map(|o| (o.x, o.y)).collect::<Vec<_>>(),
        [(44, 48), (104, 72), (164, 48)]
    );
    assert!(f.objs.iter().all(|o| o.priority == 1 && o.size == 2));
}

#[test]
fn hand_bobs_with_gen3_sine() {
    let g = game();
    let (mut s, frames) = loaded(&g);
    // The enter frame was bob index 0.
    assert_eq!(frames.last().unwrap().objs[0].y, 56 - 16);
    let ys: Vec<i16> = (1..=64)
        .map(|_| step(&mut s, &g, NONE).objs[0].y - (56 - 16))
        .collect();
    let expected: Vec<i16> = (1..=64).map(|i| BOB[i % 64] as i16).collect();
    assert_eq!(ys, expected);
    // Sin(64, 8) = 8 (gSineTable[64] = 256); Sin(192, 8) = -8.
    assert_eq!((BOB[16], BOB[48]), (8, -8));
}

#[test]
fn selected_ball_plays_the_moving_animation() {
    let g = game();
    let (mut s, frames) = loaded(&g);
    let mut tiles: Vec<u16> = vec![frames.last().unwrap().objs[2].tile - 512];
    for _ in 0..140 {
        let f = step(&mut s, &g, NONE);
        tiles.push(f.objs[2].tile - 512);
        assert_eq!(f.objs[1].tile, 512, "unselected balls stay still");
        assert_eq!(f.objs[3].tile, 512);
    }
    let mut expected = Vec::new();
    for (tile, d) in BALL_ANIM {
        expected.extend(core::iter::repeat_n(tile, d as usize));
    }
    assert_eq!(expected.len(), 128);
    assert_eq!(tiles[..128], expected[..]);
    assert_eq!(tiles[128..], expected[..13], "loops");
}

#[test]
fn cursor_moves_then_relabels_over_two_frames() {
    let g = game();
    let (mut s, _) = loaded(&g);
    let f = step(&mut s, &g, Buttons::RIGHT);
    assert_eq!(s.selection(), 2);
    assert_eq!(f.objs[0].x, 180 - 16, "the hand follows at once");
    assert_eq!(
        f.objs[3].tile,
        512 + 16,
        "the new ball starts its animation"
    );
    assert!(label_writes(&f).is_empty());
    // Input is ignored while the label is redrawn.
    let f = step(&mut s, &g, Buttons::LEFT);
    assert_eq!(label_writes(&f)[0], (label_start(1), false));
    let f = step(&mut s, &g, Buttons::LEFT);
    assert_eq!(label_writes(&f)[0], (label_start(2), true));
    assert_eq!(s.selection(), 2);
    // No ball right of the last one.
    step(&mut s, &g, Buttons::RIGHT);
    assert_eq!(s.selection(), 2);
    assert_eq!(s.stage, Stage::Choose);
    step(&mut s, &g, Buttons::LEFT);
    step(&mut s, &g, NONE);
    step(&mut s, &g, NONE);
    step(&mut s, &g, Buttons::LEFT);
    assert_eq!(s.selection(), 0);
    step(&mut s, &g, NONE);
    step(&mut s, &g, NONE);
    step(&mut s, &g, Buttons::LEFT);
    assert_eq!(s.selection(), 0);
}

/// Presses A on the current ball and steps until the yes/no is up; returns the frames.
fn zoom_and_ask(s: &mut StarterChoice, g: &Game) -> Vec<Frame> {
    let mut frames = vec![step(s, g, A)];
    while s.stage != Stage::YesNo {
        frames.push(step(s, g, NONE));
        assert!(frames.len() < 200);
    }
    frames
}

#[test]
fn a_zooms_to_the_center_then_asks() {
    // No BirchInTrouble text, so the first message tile written is the question's.
    let g = Game {
        birch: &[0xFF],
        ..game()
    };
    let (mut s, _) = loaded(&g);
    let frames = zoom_and_ask(&mut s, &g);
    assert_eq!(label_writes(&frames[0])[0], (label_start(1), false));
    // Pic (prio 0) then circle in front of the hand and balls, from the ball toward (120, 64).
    let pic = |f: &Frame| (f.objs[0].x + 32, f.objs[0].y + 32);
    assert_eq!(pic(&frames[0]), (120, 86));
    assert_eq!(pic(&frames[11]), (120, 64), "24 px at 2 px per frame");
    let f0 = &frames[0];
    assert_eq!(
        (f0.objs[0].tile, f0.objs[0].palette, f0.objs[0].priority),
        (704, 3, 0)
    );
    assert_eq!(
        (f0.objs[1].tile, f0.objs[1].palette, f0.objs[1].priority),
        (576, 1, 1)
    );
    assert_eq!((f0.objs[0].size, f0.objs[1].size), (3, 3));
    assert_eq!(f0.objs.len(), 6);
    // The question is printed once the 16-frame grow is over (the message canvas is cleared).
    let printed = frames
        .iter()
        .position(|f| {
            f.cmds.iter().any(
                |c| matches!(c, Cmd::BgTile { index, .. } if *index == crate::text::MSG_WINDOW.base()),
            )
        })
        .unwrap();
    assert_eq!(printed, 16);
    // 3 glyphs at the field speed, then the yes/no on the frame the text ends.
    assert!(s.text.is_yes_no_active());
    assert_eq!(frames.len(), 16 + 1 + 3 * 4 - 3);
}

#[test]
fn yes_returns_the_index_at_once_with_the_screen_visible() {
    let g = game();
    let (mut s, _) = loaded(&g);
    step(&mut s, &g, Buttons::RIGHT);
    step(&mut s, &g, NONE);
    step(&mut s, &g, NONE);
    zoom_and_ask(&mut s, &g);
    let mut f = Frame::default();
    let input = Input {
        held: A,
        pressed: A,
    };
    assert_eq!(s.update(&g, &input, &mut f), Some(2));
    assert_eq!(
        f.brightness, 0,
        "no fade: the transition runs over the screen"
    );
    assert_eq!(f.objs.len(), 6, "pic and circle still shown");
    // It keeps answering and ignores input.
    let f = step(&mut s, &g, B);
    assert_eq!(f.brightness, 0);
    assert_eq!(s.stage, Stage::Done);
    let mut f = Frame::default();
    assert_eq!(s.update(&g, &Input::default(), &mut f), Some(2));
}

#[test]
fn load_step_is_inert_once_loaded() {
    let g = game();
    let (mut s, _) = loaded(&g);
    let mut f = Frame {
        brightness: 3,
        ..Frame::default()
    };
    assert!(s.load_step(&g, &mut f));
    assert_eq!(
        f,
        Frame {
            brightness: 3,
            ..Frame::default()
        }
    );
}

#[test]
fn b_or_no_returns_to_the_choice() {
    let g = game();
    let (mut s, _) = loaded(&g);
    zoom_and_ask(&mut s, &g);
    let f = step(&mut s, &g, B);
    assert_eq!(f.objs.len(), 4, "circle and pic gone");
    assert_eq!(s.stage, Stage::Decline);
    step(&mut s, &g, NONE);
    let f = step(&mut s, &g, NONE);
    assert_eq!(label_writes(&f)[0], (label_start(1), true));
    assert_eq!(s.stage, Stage::Choose);
    // Again, answering no with the cursor.
    zoom_and_ask(&mut s, &g);
    step(&mut s, &g, Buttons::DOWN);
    step(&mut s, &g, A);
    assert_eq!(s.stage, Stage::Decline);
    assert_eq!(s.selection(), 1);
}

#[test]
fn label_name_is_centered_in_104_px_at_y_17() {
    let g = game();
    // Glyphs 1, 2, 1: 6 + 4 + 6 = 16 px, so x = (104 - 16) / 2 = 44.
    let t = label_name_tiles(g.font, &[1, 2, 1, 0xFF], 7);
    assert_eq!(t.len(), 26);
    let px = |x: usize, y: usize| (t[(y / 8) * 13 + x / 8][y % 8] >> (4 * (x % 8))) & 0xF;
    // Row 17 is canvas row 1 of tile row 2.
    assert_eq!(px(43, 1), 7, "opaque fill");
    assert_eq!(px(44, 1), LABEL_FG);
    assert_eq!(px(49, 1), LABEL_FG);
    assert_eq!(px(50, 1), LABEL_SHADOW);
    assert_eq!(px(54, 1), LABEL_FG);
    assert_eq!(px(60, 1), 7);
    assert_eq!(px(44, 0), 7, "nothing above y = 17");
    assert_eq!(px(44, 15), LABEL_FG, "glyph row 14");
}

#[test]
fn empty_tables_and_gfx_draw_nothing_but_still_run() {
    let g = game();
    let bare = Game {
        tables: &EMPTY_TABLES,
        gfx: &EMPTY_GFX,
        ..g
    };
    let (mut s, _) = loaded(&bare);
    zoom_and_ask(&mut s, &bare);
}

#[test]
fn hash_follows_the_state() {
    let g = game();
    let h = |s: &StarterChoice| {
        let mut h = Fnv64::new();
        s.hash(&mut h);
        h.finish()
    };
    let (mut a, _) = loaded(&g);
    let (mut b, _) = loaded(&g);
    assert_eq!(h(&a), h(&b));
    step(&mut a, &g, NONE);
    assert_ne!(h(&a), h(&b));
    step(&mut b, &g, NONE);
    assert_eq!(h(&a), h(&b));
    step(&mut a, &g, Buttons::RIGHT);
    step(&mut b, &g, NONE);
    assert_ne!(h(&a), h(&b));
}

#[test]
fn birch_in_trouble_prints_while_choosing_and_after_no() {
    let g = game();
    let (mut s, _) = loaded(&g);
    assert!(
        s.text.is_printing(),
        "Stage::Enter starts gText_BirchInTrouble"
    );
    for _ in 0..60 {
        step(&mut s, &g, NONE);
    }
    assert!(!s.text.is_printing(), "it prints on its own");
    zoom_and_ask(&mut s, &g);
    step(&mut s, &g, B);
    assert_eq!(s.stage, Stage::Decline);
    step(&mut s, &g, NONE);
    step(&mut s, &g, NONE);
    assert_eq!(s.stage, Stage::Choose);
    assert!(s.text.is_printing(), "reprinted after no");
}

/// `{PLAYER}` in the screen's texts prints the name the World passed in.
#[test]
fn texts_print_the_worlds_player_name() {
    let g = Game {
        birch: &[0xFD, 0x01, 0xFF],
        ..game()
    };
    let printed = |player: PlayerName| {
        let (s, _) = loaded_as(&g, player);
        s.text.printed().to_vec()
    };
    let name = [1, 2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    assert_eq!(printed(name), [1, 2, 0xFF]);
    assert_eq!(printed([0xFF; 8]), [0xFF]);
}
