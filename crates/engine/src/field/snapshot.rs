//! [`Field`] to and from its checkpoint DTO, and the restore upload (Milestone 5 Task B).

use super::{FIELD_BGS, Field, RING, Rect, upload_primary, upload_secondary};
use crate::checkpoint::FieldSnap;
use crate::data::GameData;
use crate::grid;
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
            // A crossing leaves no stale cells since Milestone 8 Task F (`Field::stale`).
            stale: None,
        }
    }

    /// The field of `s`, on layout `layout` (the snapshot's `map_layout`, schema 6).
    pub(crate) fn from_snapshot(s: &FieldSnap, layout: Option<u16>) -> Field {
        Field {
            map: s.map,
            camera_px: s.camera,
            drawn: s.drawn.map(|[x0, y0, x1, y1]| Rect { x0, y0, x1, y1 }),
            overrides: s.overrides.iter().copied().collect(),
            ring: s.ring,
            spread: None,
            // A capturable field has entered its map: the next layout is the current one (the
            // map scripts set another only in `on_transition`).
            layout,
            next_layout: layout,
        }
    }

    /// Checks a snapshot from an untrusted source (forged, or another engine build) against the
    /// invariants the live field keeps, so restoring it cannot panic: `drawn` is the camera's
    /// view (or nothing yet), the ring origin is in range, the camera is near the map and every
    /// override is a cell of the map (on `layout`, which the game must have for the map). The
    /// map must exist.
    pub(crate) fn check_snapshot(
        game: &impl GameData,
        s: &FieldSnap,
        layout: Option<u16>,
    ) -> Result<(), &'static str> {
        /// How far (in pixels) the camera may be outside the map.
        const MARGIN: i32 = 32 * 16;
        if layout.is_some_and(|l| game.map_layout(s.map, l).is_none()) {
            return Err("layout override the map does not have");
        }
        let map = Self::data_of(game, s.map, layout);
        let (w, h) = (map.width as i32 * 16, map.height as i32 * 16);
        let (cx, cy) = s.camera;
        if !(-MARGIN..=w + MARGIN).contains(&cx) || !(-MARGIN..=h + MARGIN).contains(&cy) {
            return Err("camera far outside the map");
        }
        if let Some([x0, y0, x1, y1]) = s.drawn
            && (Rect { x0, y0, x1, y1 }) != Rect::view(s.camera)
        {
            return Err("drawn is not the camera's view");
        }
        if s.stale.is_some() {
            return Err("stale view cells (a crossing leaves none)");
        }
        if !(0..RING).contains(&s.ring.0) || !(0..RING).contains(&s.ring.1) {
            return Err("ring origin out of range");
        }
        if !s.overrides.is_sorted_by(|a, b| a.0 < b.0) {
            return Err("overrides not ascending");
        }
        if (s.overrides.iter()).any(|&((x, y), _)| !grid::in_bounds(map, x, y)) {
            return Err("override outside the map");
        }
        Ok(())
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
        let m = self.data(game);
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
