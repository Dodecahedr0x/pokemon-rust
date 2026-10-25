//! Menu pictures: the party menu's mon icon and the Hall of Fame's mon picture come from
//! the game's extracted graphics (`GameData::mon_icon`, species front pics).

use super::m9_k::{UiGame, ui_run};
use super::*;
use crate::data::Icon;
use crate::mon::MonGfx;
use crate::ui::{PartyMode, ScreenKind};
use crate::video::{Cmd, Tile};
use std::sync::OnceLock;

/// One 32×32 frame, every pixel a non-zero nibble.
#[repr(C, align(4))]
struct IconBytes(pub [u8; 512]);

static ICON_BYTES: IconBytes = IconBytes([0x12; 512]);
static ICON_PAL: [u16; 16] = [0x001F; 16];
static ICON: Icon = Icon {
    tiles: &ICON_BYTES.0,
    palette: &ICON_PAL,
    frames: 1,
};

static FRONT: [Tile; 64] = [[0x2222_2222; 8]; 64];
static FRONT_PAL: [u16; 16] = [0x7C00; 16];
static MON_GFX: MonGfx = MonGfx {
    front: &FRONT,
    back: &FRONT,
    palette: &FRONT_PAL,
    shiny: &FRONT_PAL,
    front_y: 0,
    back_y: 0,
    elevation: 0,
};

/// [`UiGame`]'s battle tables, with a non-empty front pic on the species the tests send out.
fn pic_battle() -> &'static crate::mon::BattleTables {
    static T: OnceLock<&'static crate::mon::BattleTables> = OnceLock::new();
    T.get_or_init(|| {
        let src = &crate::battle::test_data::TABLES;
        let mut species = src.species.to_vec();
        species[crate::battle::test_data::NORMAL.0 as usize].gfx = Some(&MON_GFX);
        let species: &'static [_] = Box::leak(species.into_boxed_slice());
        Box::leak(Box::new(crate::mon::BattleTables { species, ..*src }))
    })
}

/// [`UiGame`] plus a mon icon. Species front pics are the stub battle tables' (non-empty).
struct PicGame;

impl GameData for PicGame {
    fn map(&self, id: MapId) -> &'static MapData {
        UiGame.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        UiGame.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        UiGame.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        UiGame.player_sprite()
    }
    fn font(&self) -> &'static Font {
        UiGame.font()
    }
    fn window(&self) -> &'static WindowGfx {
        UiGame.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        UiGame.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        UiGame.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        UiGame.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        UiGame.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        pic_battle()
    }
    fn new_game(&self) -> NewGame {
        UiGame.new_game()
    }
    fn start_menu(&self) -> &'static [StartItem] {
        UiGame.start_menu()
    }
    fn mon_icon(&self, _: SpeciesId) -> Option<&'static Icon> {
        Some(&ICON)
    }
}

fn pic_world(mons: &[(SpeciesId, &[MoveId])]) -> World {
    let mut w = World::new(&PicGame);
    ui_run(&mut w, Buttons::NONE, 16);
    let t = PicGame.battle();
    let mut rng = crate::rng::Rng::new(7);
    for &(species, moves) in mons {
        let mut p =
            crate::mon::Pokemon::new(t, species, 10, crate::mon::Ivs::Fixed(10), 0, 1, &mut rng);
        p.moves = [None; 4];
        for (i, m) in moves.iter().enumerate() {
            p.moves[i] = Some(*m);
            p.pp[i] = t.move_info(*m).pp;
        }
        w.s.party.push(p).unwrap();
    }
    w
}

fn open(w: &mut World, kind: ScreenKind) {
    let mut f = Frame::default();
    crate::world::screens::open(&mut w.s, &PicGame, kind, false, &mut f);
}

/// A frame that uploaded a non-zero OBJ tile.
fn obj_pixels(f: &Frame) -> bool {
    f.cmds.iter().any(|c| match c {
        Cmd::ObjTiles { tiles, .. } => tiles.iter().any(|t| t.iter().any(|row| *row != 0)),
        Cmd::ObjTile { tile, .. } => tile.iter().any(|row| *row != 0),
        _ => false,
    })
}

/// The menu has taken the screen (the field's top layer is off).
fn menu_entered(f: &Frame) -> bool {
    f.cmds
        .iter()
        .rev()
        .any(|c| matches!(c, Cmd::Bg { layer: 1, control } if !control.enabled))
}

/// After the menu is up: some frame uploaded a non-empty sprite, and that sprite is on screen
/// (the same frame, or a later one — the last tile of a pic can be transparent).
fn shows_sprite(w: &mut World, frames: u32) -> bool {
    let mut in_menu = false;
    let mut pixels = false;
    let mut shown = false;
    for _ in 0..frames {
        let f = w.step(&PicGame, Buttons::NONE);
        if menu_entered(&f) {
            in_menu = true;
        }
        if !in_menu {
            continue;
        }
        if obj_pixels(&f) {
            pixels = true;
        }
        if pixels && !f.objs.is_empty() {
            shown = true;
        }
    }
    shown
}

#[test]
fn party_menu_draws_a_mon_icon() {
    let mut w = pic_world(&[(
        crate::battle::test_data::NORMAL,
        &[crate::battle::test_data::TACKLE],
    )]);
    open(&mut w, ScreenKind::Party(PartyMode::Field));
    assert!(
        shows_sprite(&mut w, 48),
        "the party menu uploaded no non-empty sprite"
    );
}

#[test]
fn hall_of_fame_draws_a_mon_picture() {
    let mut w = pic_world(&[(
        crate::battle::test_data::NORMAL,
        &[crate::battle::test_data::TACKLE],
    )]);
    open(&mut w, ScreenKind::HallOfFame);
    assert!(
        shows_sprite(&mut w, 80),
        "the Hall of Fame uploaded no non-empty mon picture"
    );
}
