use super::super::test_gfx::*;
use super::*;

#[test]
fn loads_control_palettes_then_chunks_then_map() {
    let game = game(None);
    let gfx = game.gfx;
    let mut bg = Background::new(Environment::Grass);
    let mut frames = Vec::new();
    loop {
        let mut f = Frame::default();
        let done = bg.load_step(&game, &mut f);
        frames.push(f);
        if done {
            break;
        }
    }
    // 300 tiles: control+palettes, 3 chunks (128, 128, 44), map.
    assert_eq!(frames.len(), 5);
    assert_eq!(
        frames[0].cmds[0],
        Cmd::Bg {
            layer: 3,
            control: BgControl {
                enabled: true,
                priority: 3,
                char_block: 2,
                screen_block: 26,
            },
        }
    );
    for i in 0..3 {
        assert_eq!(
            frames[0].cmds[1 + i],
            Cmd::BgPalette {
                bank: 2 + i as u8,
                colors: gfx.environments[0].palettes[i],
            }
        );
    }
    let env = gfx.environments[0].tiles;
    assert_eq!(
        frames[1].cmds,
        [Cmd::BgTiles {
            char_block: 2,
            first: 0,
            tiles: &env[..128],
        }]
    );
    assert_eq!(
        frames[3].cmds,
        [Cmd::BgTiles {
            char_block: 2,
            first: 256,
            tiles: &env[256..300],
        }]
    );
    let Cmd::MapEntries {
        screen_block,
        start,
        entries,
    } = &frames[4].cmds[0]
    else {
        panic!("map expected")
    };
    assert_eq!((*screen_block, *start, entries.len()), (26, 0, 1024));
    assert_eq!(entries[..], gfx.environments[0].tilemap[..1024]);
    for f in &frames {
        assert!(f.validate().is_ok());
        let tiles: usize = f
            .cmds
            .iter()
            .map(|c| match c {
                Cmd::BgTiles { tiles, .. } => tiles.len(),
                _ => 0,
            })
            .sum();
        assert!(tiles <= BG_CHUNK_TILES);
    }
    assert!(bg.loaded());
    let mut f = Frame::default();
    assert!(bg.load_step(&game, &mut f));
    assert!(f.cmds.is_empty());
}

#[test]
fn empty_environment_loads_an_empty_map() {
    let mut bg = Background::new(Environment::Plain);
    let mut f = Frame::default();
    assert!(!bg.load_step_gfx(&crate::battle::EMPTY_GFX, &mut f));
    assert_eq!(f.cmds.len(), 1);
    let mut f = Frame::default();
    assert!(bg.load_step_gfx(&crate::battle::EMPTY_GFX, &mut f));
    assert!(
        matches!(&f.cmds[0], Cmd::MapEntries { entries, .. } if entries.iter().all(|&e| e == 0))
    );
}

#[test]
fn hash_tracks_progress() {
    let game = game(None);
    let mut bg = Background::new(Environment::Grass);
    let h0 = {
        let mut h = Fnv64::new();
        bg.hash(&mut h);
        h.finish()
    };
    bg.load_step(&game, &mut Frame::default());
    let mut h = Fnv64::new();
    bg.hash(&mut h);
    assert_ne!(h.finish(), h0);
}
