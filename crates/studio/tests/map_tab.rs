//! The Map tab's model (M6 Task H, `map_view`): placement, hit-testing, routes and pictures.

use content::Emerald;
use engine::data::{Behavior, Dir, MapId};
use studio::frames::rgba_from_rgb555;
use studio::map_view::{
    CellLayers, Live, MapEntry, MapRoute, Overlay, Rect, VIEW_MARGIN, behavior_label, camera_cell,
    camera_rect, cell_at, cells, cells_path, click_target, connection_rect, describe, encode_png,
    layer_paths, map_list, overlays, parse_map_request, render_map, shown_map,
};

/// The sampled map with the most overlays (`content::samples::map_ids`: no test walks every
/// map, M8 Task S).
fn busiest_map() -> MapId {
    let game = Emerald::for_shells();
    content::samples::map_ids()
        .into_iter()
        .max_by_key(|&id| overlays(&game, id).len())
        .unwrap()
}

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut r = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

#[test]
fn png_round_trips() {
    let (w, h) = (37usize, 11usize);
    let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 251) as u8).collect();
    assert_eq!(decode_png(&encode_png(w, h, &rgba)), (37, 11, rgba));
}

#[test]
fn an_entry_is_the_rendered_map_and_its_placed_overlays() {
    let game = Emerald::for_shells();
    let map = busiest_map();
    let e = MapEntry::build(&game, map);
    let img = render_map(&game, map, VIEW_MARGIN, &[]);
    let mut rgba = vec![0; img.width * img.height * 4];
    rgba_from_rgb555(&img.rgb555, &mut rgba);
    assert_eq!((e.width, e.height), (img.width, img.height));
    assert_eq!(
        decode_png(&e.png),
        (img.width as u32, img.height as u32, rgba)
    );
    assert_eq!(e.cells, cells(&game, map));
    let ovs = overlays(&game, map);
    assert_eq!(e.placed.len(), ovs.len());
    for (p, o) in e.placed.iter().zip(&ovs) {
        assert_eq!(&p.overlay, o);
        let is_obj = matches!(o, Overlay::Object { .. });
        assert_eq!(p.sprite.is_some(), is_obj);
        match *o {
            Overlay::Object { x, y, .. } => {
                // The sprite stands on its cell: bottoms align, centered horizontally.
                let c = Rect::cell(x, y, VIEW_MARGIN);
                assert_eq!(p.rect.y + p.rect.h, c.y + c.h);
                assert_eq!(p.rect.x + p.rect.w / 2, c.x + 8);
                let (w, h, px) = decode_png(p.sprite.as_ref().unwrap());
                assert_eq!((w as i32, h as i32), (p.rect.w, p.rect.h));
                assert!(px.chunks_exact(4).any(|c| c[3] == 0), "transparent pixels");
            }
            Overlay::Warp { x, y, .. } | Overlay::Coord { x, y, .. } | Overlay::Bg { x, y, .. } => {
                assert_eq!(p.rect, Rect::cell(x, y, VIEW_MARGIN));
            }
            Overlay::Connection { .. } => {
                let r = p.rect;
                assert!(r.x >= 0 && r.y >= 0 && r.w > 0 && r.h > 0);
                assert!(r.x + r.w <= e.width as i32 && r.y + r.h <= e.height as i32);
            }
        }
        if let Some(at) = studio::map_view::overlay_cell(o) {
            assert!(e.events_at(at).any(|x| x == o));
        }
    }
}

#[test]
fn cells_are_looked_up_inside_the_map_only() {
    let game = Emerald::for_shells();
    let e = MapEntry::build(&game, map_list()[0].0);
    let (c, r) = (e.cols as i16, e.rows as i16);
    assert_eq!(e.cell((0, 0)), e.cells.first());
    assert_eq!(e.cell((c - 1, r - 1)), e.cells.last());
    for out in [(-1, 0), (0, -1), (c, 0), (0, r)] {
        assert_eq!(e.cell(out), None, "{out:?}");
    }
}

#[test]
fn cells_and_rects() {
    assert_eq!(
        Rect::cell(0, 0, 8),
        Rect {
            x: 128,
            y: 128,
            w: 16,
            h: 16
        }
    );
    assert_eq!(cell_at(128.0, 128.0, 8, 4, 3), Some((0, 0)));
    assert_eq!(cell_at(128.0 + 63.9, 128.0 + 47.9, 8, 4, 3), Some((3, 2)));
    assert_eq!(cell_at(127.0, 130.0, 8, 4, 3), None);
    assert_eq!(cell_at(128.0 + 64.0, 130.0, 8, 4, 3), None);
    assert_eq!(cell_at(-1.0, 0.0, 0, 4, 3), None);
    let cam = camera_rect((-8, 16), 8);
    assert_eq!(
        cam,
        Rect {
            x: 120,
            y: 144,
            w: 240,
            h: 160
        }
    );
    assert_eq!(cam.center(), (240, 224));
    assert_eq!(camera_cell((-1, 31)), (-1, 1));
    assert_eq!(camera_cell((15, 32)), (0, 2));
}

#[test]
fn connection_rects_sit_in_the_margin() {
    // A 10x6 map with a 2-cell margin: image 224x160.
    let (map, m) = ((10, 6), 2);
    let r = |x, y, w, h| Rect { x, y, w, h };
    assert_eq!(
        connection_rect(Dir::Up, 3, map, (4, 9), m),
        r(80, 0, 64, 32)
    );
    assert_eq!(
        connection_rect(Dir::Down, -1, map, (20, 9), m),
        r(16, 128, 208, 32)
    );
    assert_eq!(
        connection_rect(Dir::Left, 0, map, (9, 2), m),
        r(0, 32, 32, 32)
    );
    assert_eq!(
        connection_rect(Dir::Right, 5, map, (9, 9), m),
        r(192, 112, 32, 48)
    );
    // A neighbor without cells spans the edge.
    assert_eq!(
        connection_rect(Dir::Up, 3, map, (0, 0), m),
        r(0, 0, 224, 32)
    );
    assert_eq!(
        connection_rect(Dir::Left, 3, map, (0, 5), m),
        r(0, 0, 32, 160)
    );
}

#[test]
fn missing_destinations_are_not_clickable() {
    let warp = |dest_name| Overlay::Warp {
        index: 0,
        x: 1,
        y: 2,
        dest: MapId(7),
        dest_name,
        dest_warp: 0,
    };
    assert_eq!(click_target(&warp("SomeMap")), Some(MapId(7)));
    assert_eq!(click_target(&warp("MISSING")), None);
    let conn = |name| Overlay::Connection {
        dir: Dir::Up,
        offset: 0,
        map: MapId(3),
        name,
    };
    assert_eq!(click_target(&conn("SomeMap")), Some(MapId(3)));
    assert_eq!(click_target(&conn("MISSING")), None);
    assert!(describe(&warp("SomeMap")).contains("SomeMap"));
}

#[test]
fn the_live_map_is_followed_only_when_asked() {
    let live = Live::from_inspector(MapId(5), Some((16, 32)));
    assert_eq!(
        live,
        Some(Live {
            map: MapId(5),
            camera: (16, 32)
        })
    );
    assert_eq!(Live::from_inspector(MapId(5), None), None);
    assert_eq!(shown_map(MapId(1), true, live), MapId(5));
    assert_eq!(shown_map(MapId(1), false, live), MapId(1));
    assert_eq!(shown_map(MapId(1), true, None), MapId(1));
}

#[test]
fn routes_parse() {
    assert_eq!(
        parse_map_request("/maps/12.png"),
        Some(MapRoute::Image(MapId(12)))
    );
    assert_eq!(
        parse_map_request("/maps/12/3.png?x=1"),
        Some(MapRoute::Sprite(MapId(12), 3))
    );
    for bad in ["/maps/12", "/maps/x.png", "/frames/12.png", "/maps/1/y.png"] {
        assert_eq!(parse_map_request(bad), None, "{bad}");
    }
}

#[test]
fn layer_paths_cover_the_selected_cells() {
    let game = Emerald::for_shells();
    let cs = cells(&game, busiest_map());
    let squares = |d: &str| d.matches('M').count();
    let d = cells_path(&cs, VIEW_MARGIN, |c| c.collision != 0);
    assert_eq!(squares(&d), cs.iter().filter(|c| c.collision != 0).count());
    assert_eq!(cells_path(&cs, VIEW_MARGIN, |_| false), "");
    assert!(layer_paths(&cs, VIEW_MARGIN, CellLayers::default()).is_empty());
    // Elevation groups partition the cells; behavior groups cover the non-Normal ones.
    let elev = CellLayers {
        elevation: true,
        ..CellLayers::default()
    };
    let total: usize = layer_paths(&cs, VIEW_MARGIN, elev)
        .iter()
        .map(|(_, d)| squares(d))
        .sum();
    assert_eq!(total, cs.len());
    let beh = CellLayers {
        behavior: true,
        ..CellLayers::default()
    };
    let total: usize = layer_paths(&cs, VIEW_MARGIN, beh)
        .iter()
        .map(|(_, d)| squares(d))
        .sum();
    assert_eq!(
        total,
        cs.iter().filter(|c| c.behavior != Behavior::Normal).count()
    );
    assert_eq!(behavior_label(Behavior::Normal), None);
    assert_eq!(
        behavior_label(Behavior::Ledge(Dir::Down)).unwrap(),
        "ledge Down"
    );
}
