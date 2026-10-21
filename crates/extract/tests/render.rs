use extract::maps::{self, LITTLEROOT, MapId, MapLayout, OLDALE, ROUTE101};
use extract::render::{self, TilesetData};
use extract::{Rom, fixtures, gfx};
use std::collections::BTreeSet;
use std::path::Path;

/// `general` tiles translated in the FR ROM (ARN gym and POKé SHOP signs).
const FR_LOCALIZED_GENERAL_TILES: [usize; 10] = [222, 223, 238, 239, 327, 328, 329, 330, 343, 358];

/// An 8x8 tile cell (column, row) in a map render.
type Cell = (usize, usize);

struct Rendered {
    layout: MapLayout,
    primary: TilesetData,
    secondary: TilesetData,
    rgba: Vec<u8>,
}

fn render(rom: &Rom, id: MapId) -> Rendered {
    let table = maps::locate_map_table(rom).unwrap();
    let layout = table.header(rom, id).unwrap().layout;
    let p = maps::read_tileset(rom, layout.primary_tileset, &table).unwrap();
    let s = maps::read_tileset(rom, layout.secondary_tileset, &table).unwrap();
    let primary = render::load_tileset(rom, &p).unwrap();
    let secondary = render::load_tileset(rom, &s).unwrap();
    let rgba = render::render_map(&layout, &primary, &secondary).unwrap();
    Rendered {
        layout,
        primary,
        secondary,
        rgba,
    }
}

/// The two tile entries (bottom, top) drawn in 8x8 cell (cx, cy).
fn cell_entries(r: &Rendered, cx: usize, cy: usize) -> [u16; 2] {
    let w = r.layout.width as usize;
    let id = (r.layout.blocks[cy / 2 * w + cx / 2] & 0x3FF) as usize;
    let mt = if id < 512 {
        r.primary.metatiles[id]
    } else {
        r.secondary.metatiles[id - 512]
    };
    let sub = cy % 2 * 2 + cx % 2;
    [mt[sub], mt[sub + 4]]
}

/// 8x8 cells whose pixels differ between two renders of the same size.
fn differing_cells(a: &Rendered, b: &Rendered) -> BTreeSet<Cell> {
    let (cw, ch) = (a.layout.width as usize * 2, a.layout.height as usize * 2);
    let stride = cw * 8 * 4;
    let mut out = BTreeSet::new();
    for cy in 0..ch {
        for cx in 0..cw {
            let differs = (0..8).any(|y| {
                let p = (cy * 8 + y) * stride + cx * 32;
                a.rgba[p..p + 32] != b.rgba[p..p + 32]
            });
            if differs {
                out.insert((cx, cy));
            }
        }
    }
    out
}

#[test]
fn fr_renders_match_us_except_localized_tiles() {
    let (us, fr) = (fixtures::us(), fixtures::fr());
    let pinned: [(&str, MapId, &[Cell]); 3] = [
        ("Littleroot", LITTLEROOT, &[]),
        ("Route101", ROUTE101, &[]),
        ("Oldale", OLDALE, &[(15, 32), (16, 32), (31, 12), (32, 12)]),
    ];
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/previews");
    std::fs::create_dir_all(&out_dir).unwrap();
    for (name, id, expected) in pinned {
        let (u, f) = (render(&us, id), render(&fr, id));
        assert_eq!(u.layout.blocks, f.layout.blocks, "{name}: blocks");
        assert_eq!(u.primary.metatiles, f.primary.metatiles, "{name}: primary");
        assert_eq!(
            u.secondary.metatiles, f.secondary.metatiles,
            "{name}: secondary"
        );
        let (w, h) = (u.layout.width as u32 * 16, u.layout.height as u32 * 16);
        assert_eq!(f.rgba.len(), (w * h * 4) as usize, "{name}");

        let diff = differing_cells(&u, &f);
        eprintln!("{name}: {w}x{h}, differing cells {diff:?}");
        for &(cx, cy) in &diff {
            let entries = cell_entries(&f, cx, cy);
            eprintln!("{name}: cell ({cx},{cy}) entries {entries:x?}");
            let localized = entries
                .iter()
                .any(|&e| FR_LOCALIZED_GENERAL_TILES.contains(&((e & 0x3FF) as usize)));
            assert!(localized, "{name}: cell ({cx},{cy}) {entries:x?} differs");
        }
        assert_eq!(diff, expected.iter().copied().collect(), "{name}");

        gfx::write_png_rgba(&out_dir.join(format!("{name}.png")), w, h, &f.rgba).unwrap();
    }
}

#[test]
fn littleroot_preview_is_320_square() {
    let r = render(&fixtures::fr(), LITTLEROOT);
    assert_eq!((r.layout.width, r.layout.height), (20, 20));
    assert_eq!(r.rgba.len(), 320 * 320 * 4);
}
