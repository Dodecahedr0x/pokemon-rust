//! Secret bases: A facing north into an unregistered entrance runs the check script, which
//! claims the base, warps in, and the base's map places one decoration and fights its trainer.

use super::*;
use crate::battle::Side;
use crate::battle::test_data::WATER;
use crate::script::{Ctx, Script, SecretBaseCmd, WarpSlot};
use crate::video::Tile;
use alloc::boxed::Box;

const OUTSIDE: MapId = MapId(4);
const INSIDE: MapId = MapId(5);
const CHECK: ScriptId = ScriptId(50);
const ENTER: ScriptId = ScriptId(51);
const INSIDE_SCRIPT: ScriptId = ScriptId(52);
const DECOR: u8 = 7;

static TILES: [Tile; 1] = [[0; 8]];
static TS: Tileset = Tileset {
    tiles: &TILES,
    palettes: &[[0; 16]; 16],
    metatiles: &[[0; 8], [0; 8], [0; 8]],
    // 0 normal, 1 closed red cave, 2 open red cave.
    attributes: &[0, 0x90, 0x91],
};

const fn outside_blocks(open: bool) -> [u16; 100] {
    let mut b = [0; 100];
    b[5 * 10 + 5] = if open { 2 } else { 1 };
    b
}
static CLOSED_BLOCKS: [u16; 100] = outside_blocks(false);
static OPEN_BLOCKS: [u16; 100] = outside_blocks(true);
static INSIDE_BLOCKS: [u16; 100] = [0; 100];

static BGS: [BgEvent; 1] = [BgEvent {
    x: 5,
    y: 5,
    elevation: 0,
    kind: BgKind::SecretBase { id: 1 },
}];

static WARPS: [WarpEvent; 1] = [WarpEvent {
    x: 4,
    y: 6,
    elevation: 0,
    dest_map: OUTSIDE,
    dest_warp: 0,
}];

static FRAME: [VarScript; 1] = [VarScript {
    var: VarId(0x800D),
    value: 0,
    script: INSIDE_SCRIPT,
}];

const NO_SCRIPTS: MapScripts = MapScripts {
    on_load: None,
    on_transition: None,
    on_resume: None,
    on_return_to_field: None,
    on_frame: &[],
    on_warp_into: &[],
};

static CLOSED: MapData = MapData {
    name: "outside a secret base",
    width: 10,
    height: 10,
    blocks: &CLOSED_BLOCKS,
    border: [0; 4],
    primary: &TS,
    secondary: &TS,
    objects: &[],
    warps: &[],
    coords: &[],
    bgs: &BGS,
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: true,
    map_type: 0,
};

static OPEN: MapData = MapData {
    name: "open secret base",
    width: 10,
    height: 10,
    blocks: &OPEN_BLOCKS,
    border: [0; 4],
    primary: &TS,
    secondary: &TS,
    objects: &[],
    warps: &[],
    coords: &[],
    bgs: &BGS,
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: true,
    map_type: 0,
};

static INSIDE_MAP: MapData = MapData {
    name: "secret base",
    width: 10,
    height: 10,
    blocks: &INSIDE_BLOCKS,
    border: [0; 4],
    primary: &TS,
    secondary: &TS,
    objects: &[],
    warps: &WARPS,
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: MapScripts {
        on_load: None,
        on_transition: None,
        on_resume: None,
        on_return_to_field: None,
        on_frame: &FRAME,
        on_warp_into: &[],
    },
    allow_running: true,
    map_type: 9,
};

struct SecretGame {
    open: bool,
}

/// Claim the faced entrance, keep one decoration, and warp in.
async fn check(ctx: Ctx) {
    ctx.give_mon(WATER, 5).await;
    ctx.secret_base(SecretBaseCmd::Claim).await;
    ctx.secret_base(SecretBaseCmd::SnapshotParty).await;
    ctx.secret_base(SecretBaseCmd::GiveDecor(DECOR)).await;
    ctx.set_var(VarId(0x4054), 0).await;
    let (x, y) = ctx.player_pos().await;
    let map = ctx.current_map().await;
    ctx.set_warp(WarpSlot::Dynamic, map, None, x, y).await;
    ctx.warp(INSIDE, Some(0), 0, 0).await;
}

/// The base's on-frame script: place the decoration, then fight the snapshotted party.
async fn inside(ctx: Ctx) {
    ctx.secret_base(SecretBaseCmd::PlaceFront(0)).await;
    ctx.set_var(VarId(0x800D), 1).await;
    ctx.secret_base_battle(0).await;
}

async fn enter_only(ctx: Ctx) {
    ctx.set_var(VarId(0x40F0), 1).await;
}

impl GameData for SecretGame {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == OUTSIDE {
            if self.open { &OPEN } else { &CLOSED }
        } else if id == INSIDE {
            &INSIDE_MAP
        } else {
            Game.map(id)
        }
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
        match raw {
            0x90 => Behavior::SecretBase(SecretBaseSpot::RedCave),
            0x91 => Behavior::SecretBase(SecretBaseSpot::RedCaveOpen),
            _ => Behavior::Normal,
        }
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        match id {
            CHECK => Box::pin(check(ctx)),
            ENTER => Box::pin(enter_only(ctx)),
            INSIDE_SCRIPT => Box::pin(inside(ctx)),
            _ => Game.script(id, ctx),
        }
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: OUTSIDE,
            x: 5,
            y: 6,
            facing: Dir::Up,
            script: None,
            player_name: &[1, 0xFF],
        }
    }
    fn secret_base_check_script(&self) -> Option<ScriptId> {
        Some(CHECK)
    }
    fn secret_base_enter_script(&self) -> Option<ScriptId> {
        Some(ENTER)
    }
    fn secret_base_var(&self) -> Option<VarId> {
        Some(VarId(0x4054))
    }
}

fn step(g: &SecretGame, w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(g, keys).validate().unwrap();
    }
}

#[test]
fn the_player_enters_a_secret_base_decorates_it_and_fights() {
    let g = SecretGame { open: false };
    let mut w = World::new(&g);
    step(&g, &mut w, Buttons::NONE, 32);
    assert_eq!(w.objects().player().pos(), (5, 6));
    assert_eq!(w.objects().player().facing, Dir::Up);
    step(&g, &mut w, Buttons::A, 1);
    for _ in 0..800 {
        w.step(&g, Buttons::NONE).validate().unwrap();
        if w.battle().is_some() {
            break;
        }
    }
    assert_eq!(w.map(), INSIDE, "events: {:?}", w.events());
    assert_eq!(w.s.secret_bases.player_base_id(), 1);
    assert_eq!(w.s.secret_bases.decoration(0), Some(DECOR));
    let (_, battle, _) = w.battle().expect("the secret-base trainer");
    assert_eq!(
        battle.party(Side::Opponent).get(0).map(|m| m.species),
        Some(WATER)
    );
}

#[test]
fn walking_into_an_open_secret_base_starts_the_enter_script() {
    let g = SecretGame { open: true };
    let mut w = World::new(&g);
    step(&g, &mut w, Buttons::NONE, 32);
    step(&g, &mut w, Buttons::UP, 1);
    step(&g, &mut w, Buttons::NONE, 8);
    assert_eq!(w.var(VarId(0x40F0)), 1);
    assert_eq!(w.var(VarId(0x8004)), 0);
}
