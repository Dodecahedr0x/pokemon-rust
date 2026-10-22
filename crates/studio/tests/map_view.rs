//! Map view core (M6 Task E): image size, every map renders, overrides, cells, overlays and
//! sprites. Structure only: no ROM text is asserted.

use content::Emerald;
use engine::data::{BgKind, Dir, GameData, Gfx, VarId, block_metatile};
use std::time::Instant;
use studio::map_view::{Overlay, cells, map_list, overlays, render_map};
use studio::sprite::{decode_sprite, initial_facing, object_sprite};

#[test]
fn initial_facing_matches_the_engine() {
    use engine::data::{GfxId, MovementType, ObjectEvent};
    use engine::objects::Objects;
    let mut all = vec![
        MovementType::LookAround,
        MovementType::Wander,
        MovementType::WanderVertical,
        MovementType::WanderHorizontal,
        MovementType::Invisible,
    ];
    for d in Dir::ALL {
        all.push(MovementType::Face(d));
        all.push(MovementType::WalkInPlace(d));
    }
    for m in &all {
        // Exhaustive: a new engine variant stops this compiling until it is listed above.
        match m {
            MovementType::Face(_)
            | MovementType::LookAround
            | MovementType::Wander
            | MovementType::WanderVertical
            | MovementType::WanderHorizontal
            | MovementType::WalkInPlace(_)
            | MovementType::Invisible => {}
        }
    }
    for m in all {
        // The engine's facing at spawn (`Objects::add`).
        let mut objs = Objects::new();
        objs.add(
            &ObjectEvent {
                local_id: 1,
                gfx: Gfx::Fixed(GfxId(0)),
                x: 3,
                y: 3,
                elevation: 3,
                movement: m,
                range: (0, 0),
                trainer_type: 0,
                trainer_sight: 0,
                script: None,
                hide_flag: None,
            },
            |_| 0,
        );
        assert_eq!(initial_facing(m), objs.get(1).unwrap().facing, "{m:?}");
    }
}

#[test]
fn map_list_is_map_ids_in_order() {
    let list = map_list();
    assert_eq!(list.len(), content::data::MAP_COUNT);
    for (&(id, name), &want) in list.iter().zip(content::data::MAP_IDS.iter()) {
        assert_eq!(id, want);
        assert!(content::has_map(id));
        assert_eq!(name, Emerald::for_shells().map(id).name);
        assert!(!name.is_empty());
    }
}

#[test]
fn every_map_renders_with_its_margin() {
    let game = Emerald::for_shells();
    let t = Instant::now();
    for (id, name) in map_list() {
        let m = game.map(id);
        for margin in [0u16, 3] {
            let img = render_map(&game, id, margin, &[]);
            assert_eq!(img.map, id);
            assert_eq!(
                img.width,
                (m.width as usize + 2 * margin as usize) * 16,
                "{name}"
            );
            assert_eq!(
                img.height,
                (m.height as usize + 2 * margin as usize) * 16,
                "{name}"
            );
            assert_eq!(img.rgb555.len(), img.width * img.height);
            let mut colors = img.rgb555.clone();
            colors.sort_unstable();
            colors.dedup();
            assert!(colors.len() > 1, "{name} renders a single color");
        }
    }
    println!(
        "{} maps rendered at margins 0 and 3 in {:?}",
        content::data::MAP_COUNT,
        t.elapsed()
    );
}

#[test]
fn an_override_draws_the_overriding_block() {
    let game = Emerald::for_shells();
    // The first map with a cell (1, 1) and a block whose metatile differs from that cell's
    // (the stub map has one), so the test never passes vacuously.
    let found = map_list().into_iter().find_map(|(id, _)| {
        let m = game.map(id);
        if m.width < 3 || m.height < 3 {
            return None;
        }
        let target = m.blocks[m.width as usize + 1];
        let other = m
            .blocks
            .iter()
            .find(|&&b| block_metatile(b) != block_metatile(target))?;
        Some((id, *other))
    });
    let (id, other) = found.expect("a map with two distinct metatiles");
    let m = game.map(id);
    let w = m.width as usize;
    let src = m.blocks.iter().position(|&b| b == other).unwrap();
    let (sx, sy) = (src % w, src / w);
    let margin = 2;
    let plain = render_map(&game, id, margin, &[]);
    // Outside-the-map overrides are ignored, as `Field::set_block` does.
    let img = render_map(&game, id, margin, &[((1, 1), other), ((-1, 0), other)]);
    let cell = |img: &studio::map_view::MapImage, x: usize, y: usize| -> Vec<u16> {
        let (x0, y0) = ((x + margin as usize) * 16, (y + margin as usize) * 16);
        (0..16)
            .flat_map(|r| img.rgb555[(y0 + r) * img.width + x0..][..16].to_vec())
            .collect()
    };
    assert_eq!(cell(&img, 1, 1), cell(&plain, sx, sy));
    let left = |img: &studio::map_view::MapImage| {
        (0..img.height)
            .flat_map(|y| img.rgb555[y * img.width..][..margin as usize * 16].to_vec())
            .collect::<Vec<_>>()
    };
    assert_eq!(left(&img), left(&plain), "margin cells untouched");
    let n = |img: &studio::map_view::MapImage| {
        img.rgb555
            .iter()
            .zip(&plain.rgb555)
            .filter(|(a, b)| a != b)
            .count()
    };
    assert!(n(&img) <= 256, "only cell (1, 1) changes");
}

#[test]
fn cells_follow_the_blocks() {
    let game = Emerald::for_shells();
    for (id, name) in map_list() {
        let m = game.map(id);
        let c = cells(&game, id);
        assert_eq!(c.len(), m.width as usize * m.height as usize, "{name}");
        for (i, cell) in c.iter().enumerate() {
            assert_eq!(cell.block, m.blocks[i]);
            assert_eq!(
                (cell.x as usize, cell.y as usize),
                (i % m.width as usize, i / m.width as usize)
            );
            assert_eq!(cell.metatile, block_metatile(cell.block));
            assert_eq!(cell.behavior, game.behavior(cell.raw_behavior));
            assert_eq!(
                cell.behavior,
                engine::flow::block_behavior(&game, m, cell.block)
            );
        }
    }
}

#[test]
fn overlay_counts_equal_the_map_data() {
    let game = Emerald::for_shells();
    let mut total = 0;
    for (id, name) in map_list() {
        let m = game.map(id);
        let o = overlays(&game, id);
        let count = |f: fn(&Overlay) -> bool| o.iter().filter(|x| f(x)).count();
        assert_eq!(
            count(|x| matches!(x, Overlay::Object { .. })),
            m.objects.len()
        );
        assert_eq!(count(|x| matches!(x, Overlay::Warp { .. })), m.warps.len());
        assert_eq!(
            count(|x| matches!(x, Overlay::Coord { .. })),
            m.coords.len()
        );
        assert_eq!(count(|x| matches!(x, Overlay::Bg { .. })), m.bgs.len());
        assert_eq!(
            count(|x| matches!(x, Overlay::Connection { .. })),
            m.connections.len()
        );
        total += o.len();
        for x in &o {
            match *x {
                Overlay::Warp {
                    index,
                    dest,
                    dest_name,
                    x,
                    y,
                    ..
                } => {
                    let w = &m.warps[index as usize];
                    assert_eq!((w.x, w.y, w.dest_map), (x, y, dest));
                    assert!(!dest_name.is_empty(), "{name}");
                    assert_eq!(
                        dest_name == "MISSING",
                        !content::has_map(dest),
                        "{name} warp {index}"
                    );
                }
                Overlay::Connection { map, name: n, .. } => {
                    assert_eq!(n == "MISSING", !content::has_map(map), "{name}");
                }
                Overlay::Coord { script, .. } => assert!(!script.is_empty()),
                Overlay::Bg { kind, script, .. } => {
                    assert_eq!(script.is_some(), matches!(kind, BgKind::Script { .. }));
                }
                Overlay::Object {
                    gfx, script, x, y, ..
                } => {
                    assert!(script.is_none_or(|s| !s.is_empty()));
                    let sprite = object_sprite(&game, gfx, Dir::Down);
                    assert_eq!(sprite.pixels.len(), sprite.width * sprite.height);
                    assert_eq!(
                        sprite.placeholder,
                        match gfx {
                            Gfx::Var(_) => true,
                            Gfx::Fixed(g) => game.sprite(g).is_none(),
                        },
                        "{name} object at ({x}, {y})"
                    );
                }
            }
        }
    }
    println!("{total} overlays");
}

#[test]
fn player_sprite_frame_0_decodes() {
    let s = content::data::PLAYER_SPRITE;
    let (w, h, px) = decode_sprite(s, 0);
    assert_eq!((w, h), (s.width as usize, s.height as usize));
    assert_eq!(px.len(), w * h);
    let tiles = s.frames[0];
    let mut transparent = 0;
    for (i, p) in px.iter().enumerate() {
        let (x, y) = (i % w, i / w);
        let t = tiles[(y / 8) * (w / 8) + x / 8];
        let idx = (t[y % 8] >> (4 * (x % 8))) & 0xF;
        match idx {
            0 => {
                assert_eq!(*p, None);
                transparent += 1;
            }
            k => assert_eq!(*p, Some(s.palette[k as usize])),
        }
    }
    assert!(transparent > 0 && transparent < w * h);
}

#[test]
fn facing_frames_and_placeholders() {
    let game = Emerald::for_shells();
    let (id, s) = (0..=u16::MAX)
        .find_map(|g| {
            let id = engine::data::GfxId(g);
            game.sprite(id)
                .filter(|s| s.frames.len() >= 3)
                .map(|s| (id, s))
        })
        .expect("a sprite with facing frames");
    let gfx = Gfx::Fixed(id);
    let down = object_sprite(&game, gfx, Dir::Down);
    assert!(!down.placeholder);
    assert_eq!(down.pixels, decode_sprite(s, 0).2);
    assert_eq!(
        object_sprite(&game, gfx, Dir::Up).pixels,
        decode_sprite(s, 1).2
    );
    let (w, _, left) = decode_sprite(s, 2);
    assert_eq!(object_sprite(&game, gfx, Dir::Left).pixels, left);
    let mut mirrored = left;
    for row in mirrored.chunks_exact_mut(w) {
        row.reverse();
    }
    assert_eq!(object_sprite(&game, gfx, Dir::Right).pixels, mirrored);
    // Standing on cell (3, 4): centered on the cell, bottom on the cell's bottom.
    let (x, y) = down.top_left(3, 4);
    assert_eq!(x + down.width as i32 / 2, 3 * 16 + 8);
    assert_eq!(y + down.height as i32, 5 * 16);

    let p = object_sprite(&game, Gfx::Var(VarId(0x4010)), Dir::Down);
    assert!(p.placeholder);
    assert_eq!(p.pixels.len(), p.width * p.height);
}
