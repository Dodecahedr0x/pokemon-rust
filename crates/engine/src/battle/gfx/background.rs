//! The environment background on ENV, as Gen3 `DrawMainBattleBackground` (`battle_bg.c`):
//! tiles to the ENV char block, the first 32x32 screen block of the tilemap (Gen3's BG3 is
//! 64x32; its left half is what shows once the intro slide is over), and the three palettes to
//! BG banks 2..5.

use super::super::{BattleGfx, Environment};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::video::{BgControl, Cmd, Frame, MAP_SIZE, layout::battle::ENV};
use alloc::vec::Vec;

/// Tiles per `load_step` (4 KiB: a small part of a vblank, so a black step is never late).
pub const BG_CHUNK_TILES: usize = 128;
/// First BG palette bank of the environment (`BG_PLTT_ID(2)`, 3 banks).
const PALETTE_BANK: u8 = 2;
const PALETTES: usize = 3;

/// Loads one `Environment` in chunks while the screen is black; static afterwards.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Background {
    env: Environment,
    /// Steps done: 0 = control + palettes, then tile chunks, then the map.
    step: u16,
    done: bool,
}

impl Background {
    pub fn new(env: Environment) -> Self {
        Background {
            env,
            step: 0,
            done: false,
        }
    }

    /// Uploads the next chunk; true when everything is loaded (then a no-op).
    pub fn load_step(&mut self, game: &impl GameData, f: &mut Frame) -> bool {
        self.load_step_gfx(game.battle_gfx(), f)
    }

    /// [`Background::load_step`] from the graphics directly.
    pub fn load_step_gfx(&mut self, gfx: &BattleGfx, f: &mut Frame) -> bool {
        if self.done {
            return true;
        }
        let img = &gfx.environments[self.env as usize];
        let (layer, char_block, screen_block, priority) = ENV;
        let chunks = img.tiles.len().div_ceil(BG_CHUNK_TILES) as u16;
        match self.step {
            0 => {
                f.cmds.push(Cmd::Bg {
                    layer,
                    control: BgControl {
                        enabled: true,
                        priority,
                        char_block,
                        screen_block,
                    },
                });
                for (i, p) in img.palettes.iter().take(PALETTES).enumerate() {
                    f.cmds.push(Cmd::BgPalette {
                        bank: PALETTE_BANK + i as u8,
                        colors: *p,
                    });
                }
            }
            s if s <= chunks => {
                let first = (s as usize - 1) * BG_CHUNK_TILES;
                let end = (first + BG_CHUNK_TILES).min(img.tiles.len());
                f.cmds.push(Cmd::BgTiles {
                    char_block,
                    first: first as u16,
                    tiles: &img.tiles[first..end],
                });
            }
            _ => {
                let n = img.tilemap.len().min(MAP_SIZE * MAP_SIZE);
                let mut entries: Vec<u16> = img.tilemap[..n].to_vec();
                entries.resize(MAP_SIZE * MAP_SIZE, 0);
                f.cmds.push(Cmd::MapEntries {
                    screen_block,
                    start: 0,
                    entries,
                });
                self.done = true;
            }
        }
        self.step += 1;
        self.done
    }

    pub fn loaded(&self) -> bool {
        self.done
    }

    pub fn environment(&self) -> Environment {
        self.env
    }

    /// The background is static once loaded (scroll stays at (0, 0)).
    pub fn update(&mut self, f: &mut Frame) {
        let _ = f;
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.env as u8, self.done as u8]);
        h.write_u16(self.step);
    }
}

#[cfg(test)]
mod tests {
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
}
