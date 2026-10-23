//! Pixel identity of `render_map` with the game (M6 Task E item 4): along the route, every
//! sampled capturable frame of the live session, drawn without its OBJs, equals the crop of the
//! whole-map render at the session's camera.

use content::Emerald;
use engine::data::{GameData, MapId};
use engine::grid;
use game_native::{SCREEN_H, SCREEN_W, Session, scenario};
use std::collections::{BTreeMap, BTreeSet};
use studio::map_view::{MapImage, render_map};

const MARGIN: u16 = 8;
/// A render: the map and its `setmetatile` overrides.
type RenderKey = (MapId, Vec<((i16, i16), u16)>);
/// Compare one capturable frame in this many.
const EVERY: u32 = 7;

/// Whether the view at `camera` shows a cell that `map`'s connections fill.
fn connection_in_view(game: &impl GameData, map: MapId, camera: (i32, i32)) -> bool {
    let m = game.map(map);
    let (x0, y0) = (camera.0.div_euclid(16), camera.1.div_euclid(16));
    let (x1, y1) = (
        (camera.0 + SCREEN_W as i32 - 1).div_euclid(16),
        (camera.1 + SCREEN_H as i32 - 1).div_euclid(16),
    );
    (y0..=y1).any(|y| {
        (x0..=x1).any(|x| {
            let (x, y) = (x as i16, y as i16);
            !grid::in_bounds(m, x, y) && grid::neighbor(game, m, x, y).is_some()
        })
    })
}

#[test]
fn render_map_matches_the_live_session() {
    let rec = scenario::record_route().unwrap_or_else(|e| panic!("{e}"));
    let game = Emerald::for_shells();
    let mut s = Session::new(game);
    let mut cache: BTreeMap<RenderKey, MapImage> = BTreeMap::new();
    let mut fb = vec![0u16; SCREEN_W * SCREEN_H];
    let (mut compared, mut capturable) = (0u32, 0u32);
    let mut maps = BTreeSet::new();
    let mut with_connection = 0u32;
    for frame in 0..rec.frames() {
        s.step(rec.keys_at(frame));
        let Ok(cp) = s.checkpoint() else { continue };
        capturable += 1;
        if capturable % EVERY != 1 {
            continue;
        }
        let field = &cp.world.field;
        assert_eq!(
            s.frame().brightness,
            0,
            "frame {}: capturable is unfaded",
            frame + 1
        );
        let mut f = s.frame().clone();
        f.objs.clear();
        s.ppu().render(&f, &mut fb);
        let img = cache
            .entry((field.map, field.overrides.clone()))
            .or_insert_with(|| render_map(&game, field.map, MARGIN, &field.overrides));
        let want = img.crop(field.camera).unwrap_or_else(|| {
            panic!(
                "frame {}: camera {:?} outside the {}-cell margin",
                frame + 1,
                field.camera,
                MARGIN
            )
        });
        if let Some(i) = (0..fb.len()).find(|&i| fb[i] != want[i]) {
            panic!(
                "frame {} map {:?} camera {:?}: first difference at screen ({}, {}): live {:#06x}, render {:#06x}",
                frame + 1,
                field.map,
                field.camera,
                i % SCREEN_W,
                i / SCREEN_W,
                fb[i],
                want[i]
            );
        }
        compared += 1;
        maps.insert(field.map);
        if connection_in_view(&game, field.map, field.camera) {
            with_connection += 1;
        }
    }
    println!(
        "{compared} frames compared ({capturable} capturable of {}), {} maps, {with_connection} with a connection in view, {} renders ({} with overrides)",
        rec.frames(),
        maps.len(),
        cache.len(),
        cache.keys().filter(|k| !k.1.is_empty()).count()
    );
    assert!(compared >= 3, "only {compared} frames compared");
    let connected = content::data::MAP_IDS
        .iter()
        .any(|&m| !game.map(m).connections.is_empty());
    // Silice's boot route idles on its start map (SIL1 Task D).
    if content::data::MAP_COUNT < 2 || !connected || content::FLAVOR == content::Flavor::Silice {
        println!(
            "skipped: the 2-map and connection-in-view checks need a route over 2 maps and a \
             connection (this build has {} maps; {} route)",
            content::data::MAP_COUNT,
            content::FLAVOR.name()
        );
        return;
    }
    assert!(maps.len() >= 2, "frames on {} map(s) only", maps.len());
    assert!(with_connection >= 1, "no frame with a connection in view");
}
