use super::*;
use crate::Tile;
use crate::data::*;
use crate::script::{Ctx, Script};

const N_MT: usize = 512;

/// Metatile k's entry i is `base + 8k + i`; layer type cycles Normal, Covered, Split.
const fn metatiles(base: u16) -> [[u16; 8]; N_MT] {
    let mut out = [[0; 8]; N_MT];
    let mut k = 0;
    while k < N_MT {
        let mut i = 0;
        while i < 8 {
            out[k][i] = base + (8 * k + i) as u16;
            i += 1;
        }
        k += 1;
    }
    out
}

const fn attributes() -> [u16; N_MT] {
    let mut out = [0; N_MT];
    let mut k = 0;
    while k < N_MT {
        out[k] = ((k % 3) as u16) << 12;
        k += 1;
    }
    out
}

const fn palettes(tag: u16) -> [[u16; 16]; 16] {
    let mut out = [[0; 16]; 16];
    let mut b = 0;
    while b < 16 {
        let mut c = 0;
        while c < 16 {
            out[b][c] = tag + (b * 16 + c) as u16 + 1;
            c += 1;
        }
        b += 1;
    }
    out
}

static TILES_P: [Tile; 3] = [[1; 8]; 3];
static TILES_S1: [Tile; 2] = [[2; 8]; 2];
static TILES_S2: [Tile; 2] = [[3; 8]; 2];
static MT_P: [[u16; 8]; N_MT] = metatiles(0);
static MT_S1: [[u16; 8]; N_MT] = metatiles(0x4000);
static MT_S2: [[u16; 8]; N_MT] = metatiles(0x8000);
static ATTR: [u16; N_MT] = attributes();
static PAL_P: [[u16; 16]; 16] = palettes(0x1000);
static PAL_S1: [[u16; 16]; 16] = palettes(0x2000);
static PAL_S2: [[u16; 16]; 16] = palettes(0x3000);

static PRIMARY: Tileset = Tileset {
    tiles: &TILES_P,
    palettes: &PAL_P,
    metatiles: &MT_P,
    attributes: &ATTR,
};
static SECONDARY_1: Tileset = Tileset {
    tiles: &TILES_S1,
    palettes: &PAL_S1,
    metatiles: &MT_S1,
    attributes: &ATTR,
};
static SECONDARY_2: Tileset = Tileset {
    tiles: &TILES_S2,
    palettes: &PAL_S2,
    metatiles: &MT_S2,
    attributes: &ATTR,
};

const fn blocks<const N: usize>(base: u16) -> [u16; N] {
    let mut out = [0; N];
    let mut i = 0;
    while i < N {
        out[i] = base + (i % 500) as u16;
        i += 1;
    }
    out
}

// A: 40x30, primary metatiles; B: 30x20 north of A (offset 3), other secondary, secondary
// metatiles; C: 10x30 east of A, same tilesets as A.
const A: MapId = MapId(0);
const B: MapId = MapId(1);
const C: MapId = MapId(2);
static BLOCKS_A: [u16; 40 * 30] = blocks(0);
static BLOCKS_B: [u16; 30 * 20] = blocks(512);
static BLOCKS_C: [u16; 10 * 30] = blocks(100);

const fn map(
    width: u16,
    height: u16,
    blocks: &'static [u16],
    secondary: &'static Tileset,
    connections: &'static [Connection],
) -> MapData {
    MapData {
        name: "",
        width,
        height,
        blocks,
        border: [400, 401, 402, 403],
        primary: &PRIMARY,
        secondary,
        objects: &[],
        warps: &[],
        coords: &[],
        bgs: &[],
        connections,
        scripts: MapScripts {
            on_load: None,
            on_transition: None,
            on_resume: None,
            on_return_to_field: None,
            on_frame: &[],
            on_warp_into: &[],
        },
        allow_running: true,
    }
}

// D: 4x4, a short tileset (4 metatiles, 2 attributes, 3 palettes, no tiles) in both slots.
const D: MapId = MapId(3);
static SHORT_PAL: [[u16; 16]; 3] = {
    let p = palettes(0x1000);
    [p[0], p[1], p[2]]
};
static SHORT_MT: [[u16; 8]; 4] = {
    let m = metatiles(0);
    [m[0], m[1], m[2], m[3]]
};
static SHORT: Tileset = Tileset {
    tiles: &[],
    palettes: &SHORT_PAL,
    metatiles: &SHORT_MT,
    attributes: &[0, 1 << 12],
};
static BLOCKS_D: [u16; 16] = [0, 1, 2, 3, 10, 600, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

static MAPS: [MapData; 4] = [
    map(
        40,
        30,
        &BLOCKS_A,
        &SECONDARY_1,
        &[
            Connection {
                dir: Dir::Up,
                offset: 3,
                map: B,
            },
            Connection {
                dir: Dir::Right,
                offset: 0,
                map: C,
            },
        ],
    ),
    map(
        30,
        20,
        &BLOCKS_B,
        &SECONDARY_2,
        &[Connection {
            dir: Dir::Down,
            offset: -3,
            map: A,
        }],
    ),
    map(
        10,
        30,
        &BLOCKS_C,
        &SECONDARY_1,
        &[Connection {
            dir: Dir::Left,
            offset: 0,
            map: A,
        }],
    ),
    {
        let mut d = map(4, 4, &BLOCKS_D, &SHORT, &[]);
        d.primary = &SHORT;
        d
    },
];

struct Game;

impl GameData for Game {
    fn map(&self, id: MapId) -> &'static MapData {
        &MAPS[id.0 as usize]
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        unimplemented!()
    }
    fn window(&self) -> &'static WindowGfx {
        unimplemented!()
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[0xFF]
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        alloc::boxed::Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: A,
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[0xFF],
        }
    }
}

/// The three field tilemaps (top, middle, bottom) after applying a frame's cmds.
#[derive(Clone)]
struct Vram([[u16; 1024]; 3]);

impl Vram {
    fn new() -> Self {
        Vram([[0xDEAD; 1024]; 3])
    }

    fn apply(&mut self, f: &Frame) {
        for c in &f.cmds {
            if let Cmd::MapEntries {
                screen_block,
                start,
                entries,
            } = c
            {
                let i = FIELD_BGS
                    .iter()
                    .position(|&bg| layout::SCREEN_BLOCKS[bg as usize] == *screen_block)
                    .unwrap();
                let s = *start as usize;
                self.0[i][s..s + entries.len()].copy_from_slice(entries);
            }
        }
    }

    /// Asserts every metatile intersecting the view shows the right entries at its ring
    /// slot, and that the scroll matches the camera.
    fn check_view(&self, field: &Field, f: &Frame) {
        let (cx, cy) = field.camera();
        let (ox, oy) = field.ring;
        for bg in FIELD_BGS {
            assert_eq!(
                f.scroll[bg as usize],
                (
                    (cx + 16 * ox as i32).rem_euclid(256) as u16,
                    (cy + 16 * oy as i32).rem_euclid(256) as u16
                )
            );
        }
        let map = Game.map(field.map());
        let v = Rect::view((cx, cy));
        for y in v.y0..=v.y1 {
            for x in v.x0..=v.x1 {
                let want = metatile_layers(map, field.block(&Game, x, y));
                let (col, row) = (
                    2 * (x + ox).rem_euclid(16) as usize,
                    2 * (y + oy).rem_euclid(16) as usize,
                );
                for (l, layer) in want.iter().enumerate() {
                    for (k, &e) in layer.iter().enumerate() {
                        let at = (row + k / 2) * 32 + col + k % 2;
                        assert_eq!(self.0[l][at], e, "cell ({x}, {y}) layer {l} entry {k}");
                    }
                }
            }
        }
    }
}

fn map_entries(f: &Frame) -> Vec<(u8, u16, Vec<u16>)> {
    f.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::MapEntries {
                screen_block,
                start,
                entries,
            } => Some((*screen_block, *start, entries.clone())),
            _ => None,
        })
        .collect()
}

fn loaded(camera: (i32, i32)) -> (Field, Vram, Frame) {
    let mut field = Field::new(A);
    let mut f = Frame::default();
    field.load(&Game, A, camera, &mut f);
    let mut vram = Vram::new();
    vram.apply(&f);
    vram.check_view(&field, &f);
    (field, vram, f)
}

#[test]
fn layer_routing_per_layer_type() {
    let a = Game.map(A);
    // Metatile 3 is NORMAL (3 % 3 == 0), 4 COVERED, 5 SPLIT; entries are 8k + i.
    let t = |k: u16, i: u16| 8 * k + i;
    assert_eq!(
        metatile_layers(a, 3),
        [
            [t(3, 4), t(3, 5), t(3, 6), t(3, 7)],
            [t(3, 0), t(3, 1), t(3, 2), t(3, 3)],
            [0x3014; 4],
        ]
    );
    assert_eq!(
        metatile_layers(a, 4),
        [
            [0; 4],
            [t(4, 4), t(4, 5), t(4, 6), t(4, 7)],
            [t(4, 0), t(4, 1), t(4, 2), t(4, 3)],
        ]
    );
    assert_eq!(
        metatile_layers(a, 5 | 0xF400),
        [
            [t(5, 4), t(5, 5), t(5, 6), t(5, 7)],
            [0; 4],
            [t(5, 0), t(5, 1), t(5, 2), t(5, 3)],
        ],
        "collision/elevation bits are ignored"
    );
    // Ids >= 512 read the secondary tileset.
    // 513 is secondary metatile 1, COVERED: its top layer goes to the middle BG.
    assert_eq!(metatile_layers(a, 512 + 1)[1][0], 0x4000 + 8 + 4);
}

#[test]
fn load_uploads_tilesets_palettes_and_controls() {
    let (_, _, f) = loaded((0, 0));
    let bgs: Vec<_> = f
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Bg { layer, control } => Some((*layer, *control)),
            _ => None,
        })
        .collect();
    assert_eq!(bgs.len(), 3);
    for (layer, c) in bgs {
        assert!(c.enabled);
        assert_eq!(c.priority, layer);
        assert_eq!(c.char_block, layout::FIELD_CHAR_BLOCK);
        assert_eq!(c.screen_block, layout::SCREEN_BLOCKS[layer as usize]);
    }
    assert!(f.cmds.contains(&Cmd::BgTiles {
        char_block: 0,
        first: 0,
        tiles: &TILES_P
    }));
    assert!(f.cmds.contains(&Cmd::BgTiles {
        char_block: 0,
        first: 512,
        tiles: &TILES_S1
    }));
    let pals: Vec<_> = f
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::BgPalette { bank, colors } => Some((*bank, *colors)),
            _ => None,
        })
        .collect();
    assert_eq!(pals.len(), 13);
    let mut p0 = PAL_P[0];
    p0[0] = 0;
    assert_eq!(pals[0], (0, p0), "backdrop forced black");
    assert_eq!(pals[5], (5, PAL_P[5]));
    assert_eq!(pals[6], (6, PAL_S1[6]));
    assert_eq!(pals[12], (12, PAL_S1[12]));
}

#[test]
fn ring_position_and_wrap() {
    let (mut field, mut vram, _) = loaded((20 * 16 + 5, 18 * 16 + 9));
    // Cell (20, 18) sits at entries (2*4, 2*2); cell (35, 28) wraps to (2*3, 2*12).
    let top = |vram: &Vram, x: usize, y: usize| vram.0[0][y * 32 + x];
    let l = metatile_layers(Game.map(A), field.block(&Game, 20, 18));
    assert_eq!(top(&vram, 8, 4), l[0][0]);
    assert_eq!(top(&vram, 9, 5), l[0][3]);
    let l = metatile_layers(Game.map(A), field.block(&Game, 35, 28));
    assert_eq!(top(&vram, 6, 24), l[0][0]);
    // Walk a long way in pixel steps, crossing ring wraps and the map edges.
    for step in 0..600 {
        let mut f = Frame::default();
        let (dx, dy) = match step / 100 {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            3 => (0, -1),
            4 => (-1, -1),
            _ => (3, 2),
        };
        let (cx, cy) = field.camera();
        field.set_camera(&Game, (cx + dx * 4, cy + dy * 4), &mut f);
        vram.apply(&f);
        vram.check_view(&field, &f);
    }
}

#[test]
fn one_metatile_move_draws_only_the_new_column_or_row() {
    let (mut field, _, _) = loaded((0, 0));
    // Right by one metatile: view 16..255 shows columns 1..=15; column 15, rows 0..=9 is new.
    let mut f = Frame::default();
    field.set_camera(&Game, (16, 0), &mut f);
    let mut want = Vec::new();
    for (i, bg) in FIELD_BGS.into_iter().enumerate() {
        for row in 0..20u16 {
            let l = metatile_layers(Game.map(A), field.block(&Game, 15, (row / 2) as i16));
            let k = (row % 2) as usize * 2;
            want.push((
                layout::SCREEN_BLOCKS[bg as usize],
                row * 32 + 30,
                alloc::vec![l[i][k], l[i][k + 1]],
            ));
        }
    }
    assert_eq!(map_entries(&f), want);
    assert_eq!(f.cmds.len(), want.len(), "nothing but the new column");
    assert_eq!(f.scroll[1], (16, 0));

    // Down by one metatile: row 10, columns 1..=15 -> one run of 30 entries per tilemap row.
    let mut f = Frame::default();
    field.set_camera(&Game, (16, 16), &mut f);
    let e = map_entries(&f);
    assert_eq!(e.len(), 6);
    for (n, (sb, start, entries)) in e.iter().enumerate() {
        assert_eq!(*sb, layout::SCREEN_BLOCKS[FIELD_BGS[n / 2] as usize]);
        assert_eq!(*start, (20 + n as u16 % 2) * 32 + 2);
        assert_eq!(entries.len(), 30);
    }

    // Pixel moves within the drawn cells draw nothing but still scroll.
    field.set_camera(&Game, (24, 16), &mut Frame::default());
    let mut f = Frame::default();
    field.set_camera(&Game, (31, 16), &mut f);
    assert!(f.cmds.is_empty());
    assert_eq!(f.scroll[3], (31, 16));
}

#[test]
fn connections_and_border_drawn_with_current_tilesets() {
    // Top-left corner of A: north rows come from B, the corner and west from the border.
    let (field, vram, _) = loaded((-48, -48));
    let a = Game.map(A);
    // (0, -1) is B's (0 - 3, 19): x < 0 there, so outside B -> border.
    assert_eq!(field.block(&Game, 0, -1), 402 | 0xC00);
    // (5, -1) is B's (2, 19), a secondary block drawn with A's secondary tileset.
    let b = field.block(&Game, 5, -1);
    assert_eq!(b, BLOCKS_B[19 * 30 + 2]);
    let l = metatile_layers(a, b);
    assert!(l.iter().flatten().any(|&e| e & 0xC000 == 0x4000));
    let slot =
        |x: i16, y: i16| (2 * y.rem_euclid(16) as usize) * 32 + 2 * x.rem_euclid(16) as usize;
    assert_eq!(vram.0[1][slot(5, -1)], l[1][0]);
    // Border at (-1, -1): border[(x & 1) + 2 (y & 1)] = border[3].
    let l = metatile_layers(a, 403);
    assert_eq!(vram.0[1][slot(-1, -1)], l[1][0]);
    // East edge: C continues A's rows.
    let (field, _, _) = loaded((35 * 16, 10 * 16));
    assert_eq!(field.block(&Game, 40, 12), BLOCKS_C[12 * 10]);
}

#[test]
fn switch_map_shifts_coordinates_and_reuploads_only_what_differs() {
    // Player near A's top edge at (10, 0); camera puts that cell at screen (112, 64).
    let cam = (10 * 16 - 112, -64);
    let (mut field, mut vram, _) = loaded(cam);
    let before: Vec<_> = (-4..6)
        .flat_map(|y| (3..18).map(move |x| (x, y)))
        .map(|(x, y)| field.block(&Game, x, y))
        .collect();
    // The player steps up to (10, -1).
    let (to, shift) = field.crossing(&Game, 10, -1).unwrap();
    assert_eq!((to, shift), (B, (-3, 20)));
    assert_eq!(field.crossing(&Game, 10, 5), None);
    let mut f = Frame::default();
    field.switch_map(&Game, to, shift, &mut f);
    assert_eq!(field.map(), B);
    assert_eq!(field.camera(), (cam.0 - 3 * 16, cam.1 + 20 * 16));
    // B's own cells and A's cells through B's connection are the same blocks as before.
    let after: Vec<_> = (-4..6)
        .flat_map(|y| (3..18).map(move |x| (x, y)))
        .map(|(x, y)| field.block(&Game, x + shift.0, y + shift.1))
        .collect();
    assert_eq!(before, after);
    // Only the secondary tileset differs.
    assert!(f.cmds.contains(&Cmd::BgTiles {
        char_block: 0,
        first: 512,
        tiles: &TILES_S2
    }));
    let banks: Vec<u8> = f
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::BgPalette { bank, .. } => Some(*bank),
            Cmd::BgTiles { first: 0, .. } | Cmd::Bg { .. } => panic!("primary re-uploaded"),
            _ => None,
        })
        .collect();
    assert_eq!(banks, (6..13).collect::<Vec<u8>>());
    vram.apply(&f);
    vram.check_view(&field, &f);

    // A -> C shares both tilesets: no uploads.
    let (mut field, _, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
    let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
    assert_eq!((to, shift), (C, (-40, 0)));
    let mut f = Frame::default();
    field.switch_map(&Game, to, shift, &mut f);
    assert!(f.cmds.iter().all(|c| matches!(c, Cmd::MapEntries { .. })));
}

#[test]
fn set_block_overrides_and_redraws_in_view() {
    let (mut field, mut vram, _) = loaded((0, 0));
    let mut f = Frame::default();
    field.set_block(&Game, 3, 2, 7 | 0x400, &mut f);
    assert_eq!(field.block(&Game, 3, 2), 7 | 0x400);
    // One metatile: 2 runs of 2 entries per BG.
    assert_eq!(map_entries(&f).len(), 6);
    vram.apply(&f);
    vram.check_view(&field, &f);
    assert_eq!(vram.0[1][4 * 32 + 6], 8 * 7 + 4, "metatile 7 is COVERED");

    // Out of view: stored, not drawn; drawn once it scrolls in.
    let mut f = Frame::default();
    field.set_block(&Game, 30, 25, 9, &mut f);
    assert!(f.cmds.is_empty());
    assert_eq!(field.block(&Game, 30, 25), 9);
    field.set_camera(&Game, (25 * 16, 20 * 16), &mut f);
    vram.apply(&f);
    vram.check_view(&field, &f);

    // Outside the map: ignored.
    let mut f = Frame::default();
    field.set_block(&Game, -1, 0, 9, &mut f);
    assert_ne!(field.block(&Game, -1, 0), 9);
    assert!(f.cmds.is_empty());

    // A reload clears overrides.
    field.load(&Game, A, (0, 0), &mut f);
    assert_eq!(field.block(&Game, 3, 2), BLOCKS_A[2 * 40 + 3]);
}

#[test]
fn redraw_draws_the_whole_view_again() {
    let (mut field, _, first) = loaded((8, 8));
    let mut f = Frame::default();
    field.redraw(&Game, &mut f);
    assert_eq!(map_entries(&f), map_entries(&first));
}

#[test]
fn hash_covers_camera_and_overrides() {
    let digest = |field: &Field| {
        let mut h = Fnv64::new();
        field.hash(&mut h);
        h.finish()
    };
    let (mut field, _, _) = loaded((0, 0));
    let h0 = digest(&field);
    let mut f = Frame::default();
    field.set_camera(&Game, (1, 0), &mut f);
    let h1 = digest(&field);
    assert_ne!(h0, h1);
    field.set_block(&Game, 0, 0, 5, &mut f);
    assert_ne!(digest(&field), h1);
}

#[test]
fn enter_then_set_block_is_drawn_by_the_first_redraw() {
    let mut field = Field::new(A);
    let mut f = Frame::default();
    field.enter(&Game, A, (0, 0), &mut f);
    assert!(map_entries(&f).is_empty(), "enter draws nothing");
    field.set_block(&Game, 3, 2, 7, &mut f);
    assert!(
        map_entries(&f).is_empty(),
        "only recorded before the first draw"
    );
    field.redraw(&Game, &mut f);
    let mut vram = Vram::new();
    vram.apply(&f);
    vram.check_view(&field, &f);
    assert_eq!(field.block(&Game, 3, 2), 7);
    assert_eq!(vram.0[1][4 * 32 + 6], 8 * 7 + 4, "metatile 7 is COVERED");
    // Each entry of the 15x10-metatile view is written exactly once per BG.
    let total: usize = map_entries(&f).iter().map(|e| e.2.len()).sum();
    assert_eq!(total, 3 * 15 * 10 * 4);

    // A connection switch keeps the view drawn (A -> C share tilesets: nothing is emitted),
    // so an on-load `set_block` in view is drawn at once.
    let (mut field, mut vram, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
    let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
    let mut f = Frame::default();
    field.switch_map_enter(&Game, to, shift, &mut f);
    assert!(f.cmds.is_empty());
    field.set_block(&Game, 0, 10, 8, &mut f);
    assert_eq!(map_entries(&f).len(), 6);
    let cam = field.camera();
    field.set_camera(&Game, cam, &mut f);
    assert_eq!(map_entries(&f).len(), 6, "nothing else is drawn");
    vram.apply(&f);
    vram.check_view(&field, &f);
    assert_eq!(field.block(&Game, 0, 10), 8);
}

#[test]
fn switch_map_clears_overrides() {
    let (mut field, _, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
    let mut f = Frame::default();
    field.set_block(&Game, 39, 10, 9, &mut f);
    field.set_block(&Game, 0, 0, 9, &mut f);
    let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
    field.switch_map(&Game, to, shift, &mut f);
    // A's (39, 10) is C's (-1, 10), seen through C's west connection: the original block.
    assert_eq!(
        field.block(&Game, 39 + shift.0, 10 + shift.1),
        BLOCKS_A[10 * 40 + 39]
    );
    assert!(field.overrides.is_empty(), "no overrides left");
}

#[test]
fn crossing_keeps_the_view_drawn() {
    // A -> C (shared tilesets): walking on draws only the new column, at the shifted ring.
    let (mut field, mut vram, _) = loaded((39 * 16 - 112, 10 * 16 - 64));
    let (to, shift) = field.crossing(&Game, 40, 10).unwrap();
    let mut f = Frame::default();
    field.switch_map_enter(&Game, to, shift, &mut f);
    let cam = field.camera();
    field.set_camera(&Game, cam, &mut f);
    assert!(map_entries(&f).is_empty(), "no redraw");
    vram.apply(&f);
    vram.check_view(&field, &f);
    let mut f = Frame::default();
    field.set_camera(&Game, (cam.0 + 16, cam.1), &mut f);
    let total: usize = map_entries(&f).iter().map(|e| e.2.len()).sum();
    assert_eq!(total, 3 * 4 * 10, "one new column of 10 metatiles");
    vram.apply(&f);
    vram.check_view(&field, &f);
}

#[test]
fn missing_metatiles_attributes_and_palettes() {
    let d = Game.map(D);
    let bottom = |k: u16| [8 * k, 8 * k + 1, 8 * k + 2, 8 * k + 3];
    let top = |k: u16| [8 * k + 4, 8 * k + 5, 8 * k + 6, 8 * k + 7];
    // Metatile 1 has its attribute (COVERED); 3 has none (NORMAL); 10 and 600 do not exist.
    assert_eq!(metatile_layers(d, 1), [[0; 4], top(1), bottom(1)]);
    assert_eq!(
        metatile_layers(d, 3),
        [top(3), bottom(3), [NORMAL_UNDERLAY; 4]]
    );
    for missing in [10, 600] {
        assert_eq!(
            metatile_layers(d, missing),
            [[0; 4], [0; 4], [NORMAL_UNDERLAY; 4]]
        );
    }

    let mut field = Field::new(D);
    let mut f = Frame::default();
    field.load(&Game, D, (-112, -64), &mut f);
    assert!(!f.cmds.iter().any(|c| matches!(c, Cmd::BgTiles { .. })));
    let banks: Vec<u8> = f
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::BgPalette { bank, .. } => Some(*bank),
            _ => None,
        })
        .collect();
    assert_eq!(banks, [0, 1, 2], "only the palettes the tileset has");
    let mut vram = Vram::new();
    vram.apply(&f);
    vram.check_view(&field, &f);
}

#[test]
fn switch_map_round_trip_with_offsets() {
    // A -> B (north, offset 3) -> A (south, offset -3): shifts cancel, view stays right.
    let cam = (10 * 16 - 112, -64);
    let (mut field, mut vram, _) = loaded(cam);
    let (to, shift) = field.crossing(&Game, 10, -1).unwrap();
    let mut f = Frame::default();
    field.switch_map(&Game, to, shift, &mut f);
    vram.apply(&f);
    vram.check_view(&field, &f);
    // The player is now at B's (7, 19) and steps back down to (7, 20).
    let (back, shift_back) = field.crossing(&Game, 7, 20).unwrap();
    assert_eq!((back, shift_back), (A, (3, -20)));
    let mut f = Frame::default();
    field.switch_map(&Game, back, shift_back, &mut f);
    assert_eq!((field.map(), field.camera()), (A, cam));
    vram.apply(&f);
    vram.check_view(&field, &f);

    // C -> A (west): shift +40.
    let mut field = Field::new(C);
    let mut f = Frame::default();
    field.load(&Game, C, (-112, 10 * 16 - 64), &mut f);
    let (to, shift) = field.crossing(&Game, -1, 10).unwrap();
    assert_eq!((to, shift), (A, (40, 0)));
    field.switch_map(&Game, to, shift, &mut f);
    assert_eq!(field.camera(), (40 * 16 - 112, 10 * 16 - 64));
}
