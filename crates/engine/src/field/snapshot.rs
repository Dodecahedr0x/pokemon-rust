//! [`Field`] to and from its checkpoint DTO, and the restore upload (Milestone 5 Task B).

use super::{FIELD_BGS, Field, Rect, upload_primary, upload_secondary};
use crate::checkpoint::FieldSnap;
use crate::data::GameData;
use crate::video::{BgControl, Cmd, Frame, layout};
use alloc::vec::Vec;

impl Field {
    pub(crate) fn snapshot(&self) -> FieldSnap {
        FieldSnap {
            map: self.map,
            camera: self.camera_px,
            drawn: self.drawn.map(|r| [r.x0, r.y0, r.x1, r.y1]),
            ring: self.ring,
            overrides: self.overrides.iter().map(|(&c, &b)| (c, b)).collect(),
        }
    }

    pub(crate) fn from_snapshot(s: &FieldSnap) -> Field {
        Field {
            map: s.map,
            camera_px: s.camera,
            drawn: s.drawn.map(|[x0, y0, x1, y1]| Rect { x0, y0, x1, y1 }),
            overrides: s.overrides.iter().copied().collect(),
            ring: s.ring,
        }
    }

    /// BG1–3 controls, both tilesets' tiles and palettes, every cell of `drawn`, the scroll.
    /// Mutates nothing (the hash is unchanged).
    pub fn upload_all(&self, game: &impl GameData, f: &mut Frame) {
        for bg in FIELD_BGS {
            f.cmds.push(Cmd::Bg {
                layer: bg,
                control: BgControl {
                    enabled: true,
                    priority: layout::BG_PRIORITY[bg as usize],
                    char_block: layout::FIELD_CHAR_BLOCK,
                    screen_block: layout::SCREEN_BLOCKS[bg as usize],
                },
            });
        }
        let m = game.map(self.map);
        upload_primary(m.primary, f);
        upload_secondary(m.secondary, f);
        if let Some(r) = self.drawn {
            let cells: Vec<(i16, i16)> = (r.y0..=r.y1)
                .flat_map(|y| (r.x0..=r.x1).map(move |x| (x, y)))
                .collect();
            self.draw_cells(game, &cells, f);
        }
        self.set_scroll(f);
    }
}
