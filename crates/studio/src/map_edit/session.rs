//! The Edit tab's model, free of Dioxus: the tools and the brush over an [`Editor`], driven by
//! cell presses, drags and releases, and the overlay shapes the tab draws.

use super::model::{Brush, Doc, Editor, Event, EventKind, Op};
use engine::data::{block_collision, block_elevation, block_metatile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pencil,
    Rect,
    Fill,
    /// Takes the brush from a cell, then goes back to the pencil.
    Picker,
    /// Selects an event; dragging moves it.
    Select,
    /// Places a new event of this kind (then selects it).
    Place(EventKind),
}

impl Tool {
    pub const ALL: [(Tool, &'static str); 9] = [
        (Tool::Pencil, "pencil"),
        (Tool::Rect, "rect"),
        (Tool::Fill, "fill"),
        (Tool::Picker, "picker"),
        (Tool::Select, "select"),
        (Tool::Place(EventKind::Object), "+object"),
        (Tool::Place(EventKind::Warp), "+warp"),
        (Tool::Place(EventKind::Coord), "+coord"),
        (Tool::Place(EventKind::Bg), "+sign"),
    ];
}

/// Which part of the cells the cell tools paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Metatile,
    Collision,
    Elevation,
}

impl Mode {
    pub const ALL: [(Mode, &'static str); 3] = [
        (Mode::Metatile, "metatile"),
        (Mode::Collision, "collision"),
        (Mode::Elevation, "elevation"),
    ];
}

pub struct Session {
    pub editor: Editor,
    pub tool: Tool,
    pub mode: Mode,
    pub metatile: u16,
    pub collision: u8,
    pub elevation: u8,
    pub selected: Option<(EventKind, usize)>,
    /// The cell the button went down on, while it is down.
    pub pressed: Option<(i32, i32)>,
    /// The last cell of a drag.
    pub last: Option<(i32, i32)>,
    /// The graphics new objects get.
    pub default_gfx: String,
    /// The outcome of the last action (an error, or what was done).
    pub status: String,
}

impl Session {
    pub fn new(editor: Editor) -> Session {
        Session {
            editor,
            tool: Tool::Pencil,
            mode: Mode::Metatile,
            metatile: 1,
            collision: 1,
            elevation: super::model::GROUND,
            selected: None,
            pressed: None,
            last: None,
            default_gfx: "OBJ_EVENT_GFX_BOY_1".into(),
            status: String::new(),
        }
    }

    pub fn doc(&self) -> &Doc {
        self.editor.doc()
    }

    pub fn brush(&self) -> Brush {
        match self.mode {
            Mode::Metatile => Brush::metatile(self.metatile),
            Mode::Collision => Brush {
                collision: Some(self.collision),
                ..Brush::default()
            },
            Mode::Elevation => Brush {
                elevation: Some(self.elevation),
                ..Brush::default()
            },
        }
    }

    /// Applies `op`; an error goes to `status`. Whether the document changed.
    pub fn apply(&mut self, op: Op) -> bool {
        let before = self.doc().clone();
        match self.editor.apply(&op) {
            Ok(()) => {
                self.status.clear();
                *self.doc() != before
            }
            Err(e) => {
                self.status = format!("{e:#}");
                false
            }
        }
    }

    fn paint(&mut self, x: i32, y: i32) -> bool {
        let b = self.brush();
        self.apply(Op::Paint {
            x,
            y,
            metatile: b.metatile,
            collision: b.collision,
            elevation: b.elevation,
        })
    }

    /// The button went down on cell (x, y).
    pub fn press(&mut self, x: i32, y: i32) -> bool {
        if !self.doc().in_bounds(x, y) {
            return false;
        }
        self.pressed = Some((x, y));
        self.last = Some((x, y));
        match self.tool {
            Tool::Pencil => {
                self.editor.begin_stroke();
                self.paint(x, y)
            }
            Tool::Rect => false,
            Tool::Fill => {
                let b = self.brush();
                self.apply(Op::Fill {
                    x,
                    y,
                    metatile: b.metatile,
                    collision: b.collision,
                    elevation: b.elevation,
                })
            }
            Tool::Picker => {
                let b = self.doc().block(x, y).unwrap_or_default();
                self.metatile = block_metatile(b);
                self.collision = block_collision(b);
                self.elevation = block_elevation(b);
                self.tool = Tool::Pencil;
                false
            }
            Tool::Select => {
                self.selected = self.doc().event_at(x as i16, y as i16);
                false
            }
            Tool::Place(kind) => {
                let changed = self.apply(self.place_op(kind, x as i16, y as i16));
                if changed {
                    let n = self.doc().events().iter().filter(|e| e.0 == kind).count();
                    self.selected = Some((kind, n - 1));
                    self.tool = Tool::Select;
                }
                changed
            }
        }
    }

    /// The pointer moved to cell (x, y) with the button down.
    pub fn drag(&mut self, x: i32, y: i32) -> bool {
        if self.pressed.is_none() || self.last == Some((x, y)) {
            return false;
        }
        self.last = Some((x, y));
        match self.tool {
            Tool::Pencil if self.doc().in_bounds(x, y) => self.paint(x, y),
            _ => false,
        }
    }

    /// The button went up on cell (x, y) (`None`: outside the map's picture).
    pub fn release(&mut self, at: Option<(i32, i32)>) -> bool {
        let Some(start) = self.pressed.take() else {
            return false;
        };
        let end = at.or(self.last).unwrap_or(start);
        self.last = None;
        match self.tool {
            Tool::Pencil => {
                self.editor.end_stroke();
                false
            }
            Tool::Rect => {
                let (x, y, w, h) = rect_of(start, end);
                let b = self.brush();
                self.apply(Op::Rect {
                    x,
                    y,
                    w,
                    h,
                    metatile: b.metatile,
                    collision: b.collision,
                    elevation: b.elevation,
                })
            }
            Tool::Select if end != start => match self.selected {
                Some((kind, index)) if self.doc().in_bounds(end.0, end.1) => self.apply(Op::Move {
                    kind,
                    index,
                    x: end.0 as i16,
                    y: end.1 as i16,
                }),
                _ => false,
            },
            _ => false,
        }
    }

    /// The rectangle a rect drag would paint.
    pub fn rect_preview(&self) -> Option<(i32, i32, i32, i32)> {
        match (self.tool, self.pressed, self.last) {
            (Tool::Rect, Some(a), Some(b)) => Some(rect_of(a, b)),
            _ => None,
        }
    }

    fn place_op(&self, kind: EventKind, x: i16, y: i16) -> Op {
        let name = &self.doc().map.name;
        let n = self.doc().events().iter().filter(|e| e.0 == kind).count();
        match kind {
            EventKind::Object => Op::Object {
                x,
                y,
                gfx: self.default_gfx.clone(),
                movement: "MOVEMENT_TYPE_FACE_DOWN".into(),
                range_x: 0,
                range_y: 0,
                script: Some(format!("{name}_EventScript_Object{n}")),
                flag: None,
                trainer_type: "TRAINER_TYPE_NONE".into(),
                sight: 0,
                elevation: super::model::GROUND,
            },
            EventKind::Warp => Op::Warp {
                x,
                y,
                dest_map: name.clone(),
                dest_warp: 0,
                elevation: 0,
            },
            EventKind::Coord => Op::Coord {
                x,
                y,
                var: "VAR_TEMP_0".into(),
                value: 0,
                script: format!("{name}_EventScript_Coord{n}"),
                elevation: super::model::GROUND,
            },
            EventKind::Bg => Op::Sign {
                x,
                y,
                script: format!("{name}_EventScript_Sign{n}"),
                facing: 0,
                elevation: 0,
            },
        }
    }

    /// The selected event as RON (the inspector's text).
    pub fn selected_ron(&self) -> Option<String> {
        let (kind, index) = self.selected?;
        let e = self.doc().event(kind, index)?;
        super::schema::to_ron(&e).ok()
    }

    /// Replaces the selected event with `text` (an [`Event`] in RON).
    pub fn replace_selected(&mut self, text: &str) -> bool {
        let Some((kind, index)) = self.selected else {
            return false;
        };
        match super::schema::from_script_ron::<Event>(text) {
            Ok(e) if e.kind() == kind => self.apply(Op::Replace { index, event: e }),
            Ok(e) => {
                self.status = format!("a {:?} cannot become a {:?}", kind, e.kind());
                false
            }
            Err(e) => {
                self.status = format!("{e:#}");
                false
            }
        }
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some((kind, index)) = self.selected.take() else {
            return false;
        };
        self.apply(Op::Delete { kind, index })
    }

    /// Undo or redo; the selection is dropped (indices may have moved).
    pub fn history(&mut self, redo: bool) -> bool {
        self.selected = None;
        if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        }
    }

    /// The connections as RON (the properties' text).
    pub fn connections_ron(&self) -> String {
        super::schema::to_ron(&self.doc().map.connections).unwrap_or_default()
    }

    /// Replaces every connection with `text` (a list in RON), as one undo step.
    pub fn set_connections(&mut self, text: &str) -> bool {
        let list: Vec<super::schema::GameConnection> = match super::schema::from_script_ron(text) {
            Ok(l) => l,
            Err(e) => {
                self.status = format!("{e:#}");
                return false;
            }
        };
        let before = self.doc().clone();
        self.editor.begin_stroke();
        for _ in 0..self.doc().map.connections.len() {
            self.apply(Op::Disconnect { index: 0 });
        }
        for c in list {
            self.apply(Op::Connect(c));
        }
        self.editor.end_stroke();
        *self.doc() != before
    }
}

fn rect_of(a: (i32, i32), b: (i32, i32)) -> (i32, i32, i32, i32) {
    let (x, y) = (a.0.min(b.0), a.1.min(b.1));
    (x, y, (a.0 - b.0).abs() + 1, (a.1 - b.1).abs() + 1)
}

/// The picture's cell under picture pixel (px, py) with `margin` cells around the map, in map
/// cells (negative or past the size in the margin).
pub fn cell_at(px: f64, py: f64, margin: u16) -> (i32, i32) {
    let m = margin as i32;
    (
        (px / 16.0).floor() as i32 - m,
        (py / 16.0).floor() as i32 - m,
    )
}

/// An SVG path (picture pixels) of the cells for which `pick` holds.
pub fn cells_path(d: &Doc, margin: u16, pick: impl Fn(u16) -> bool) -> String {
    let m = margin as i32 * 16;
    let mut s = String::new();
    for y in 0..d.height as i32 {
        for x in 0..d.width as i32 {
            if d.block(x, y).is_some_and(&pick) {
                s.push_str(&format!("M{} {}h16v16h-16z", m + x * 16, m + y * 16));
            }
        }
    }
    s
}

/// An event's mark: kind, index, cell and its letter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub kind: EventKind,
    pub index: usize,
    pub x: i16,
    pub y: i16,
    pub letter: &'static str,
}

pub fn marks(d: &Doc) -> Vec<Mark> {
    d.events()
        .into_iter()
        .map(|(kind, index, x, y)| Mark {
            kind,
            index,
            x,
            y,
            letter: match kind {
                EventKind::Object => "O",
                EventKind::Warp => "W",
                EventKind::Coord => "C",
                EventKind::Bg => "B",
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_edit::model::NewMap;

    fn session() -> Session {
        Session::new(Editor::new(
            Doc::new(&NewMap {
                name: "S".into(),
                layout: None,
                width: 6,
                height: 4,
                primary: "P".into(),
                secondary: "Q".into(),
                fill: 1,
                fill_collision: 0,
                fill_elevation: 3,
                border: None,
            })
            .unwrap(),
        ))
    }

    fn mt(s: &Session, x: i32, y: i32) -> u16 {
        block_metatile(s.doc().block(x, y).unwrap())
    }

    #[test]
    fn a_pencil_drag_is_one_undo_step() {
        let mut s = session();
        s.metatile = 7;
        assert!(s.press(0, 0));
        assert!(s.drag(1, 0));
        assert!(!s.drag(1, 0), "same cell");
        assert!(s.drag(2, 1));
        assert!(!s.drag(9, 9), "outside");
        s.release(Some((9, 9)));
        assert_eq!((mt(&s, 0, 0), mt(&s, 1, 0), mt(&s, 2, 1)), (7, 7, 7));
        assert!(s.history(false));
        assert_eq!((mt(&s, 0, 0), mt(&s, 2, 1)), (1, 1));
        assert!(!s.editor.can_undo());
    }

    #[test]
    fn rect_picker_and_modes() {
        let mut s = session();
        s.tool = Tool::Rect;
        s.metatile = 4;
        s.press(3, 2);
        s.drag(1, 0);
        assert_eq!(s.rect_preview(), Some((1, 0, 3, 3)));
        assert!(s.release(Some((1, 0))));
        assert_eq!((mt(&s, 1, 0), mt(&s, 3, 2), mt(&s, 4, 2)), (4, 4, 1));
        s.mode = Mode::Collision;
        s.tool = Tool::Fill;
        assert!(s.press(0, 3));
        s.release(None);
        assert_eq!(block_collision(s.doc().block(5, 3).unwrap()), 1);
        assert_eq!(mt(&s, 5, 3), 1, "collision only");
        s.tool = Tool::Picker;
        s.press(2, 1);
        s.release(None);
        assert_eq!((s.metatile, s.collision, s.tool), (4, 0, Tool::Pencil));
    }

    #[test]
    fn place_select_move_inspect_and_delete() {
        let mut s = session();
        s.tool = Tool::Place(EventKind::Object);
        assert!(s.press(2, 2));
        s.release(None);
        assert_eq!(s.selected, Some((EventKind::Object, 0)));
        assert_eq!(s.tool, Tool::Select);
        // Drag it.
        s.press(2, 2);
        s.drag(4, 1);
        assert!(s.release(Some((4, 1))));
        assert_eq!(s.doc().event_at(4, 1), Some((EventKind::Object, 0)));
        // Edit it as text.
        let text = s.selected_ron().unwrap();
        assert!(text.contains("OBJ_EVENT_GFX_BOY_1"), "{text}");
        assert!(s.replace_selected(&text.replace("FACE_DOWN", "FACE_UP")));
        assert_eq!(s.doc().map.objects[0].movement, "MOVEMENT_TYPE_FACE_UP");
        assert!(
            !s.replace_selected("Warp((x: 0, y: 0, elevation: 0, dest_map: \"S\", dest_warp: 0))")
        );
        assert!(s.status.contains("cannot become"), "{}", s.status);
        assert!(s.delete_selected());
        assert!(s.doc().map.objects.is_empty());
        // Connections as one step.
        assert!(s.set_connections(
            "[(dir: North, offset: 1, map: \"A\"), (dir: South, offset: 0, map: \"B\")]"
        ));
        assert_eq!(s.doc().map.connections.len(), 2);
        assert!(s.set_connections("[]"));
        assert!(s.history(false));
        assert_eq!(s.doc().map.connections.len(), 2);
        assert_eq!(marks(s.doc()).len(), 0);
    }

    #[test]
    fn overlay_shapes() {
        let mut s = session();
        s.mode = Mode::Collision;
        s.press(1, 2);
        s.release(None);
        assert_eq!(
            cells_path(s.doc(), 1, |b| block_collision(b) != 0),
            "M32 48h16v16h-16z"
        );
        assert_eq!(cell_at(20.0, 47.9, 1), (0, 1));
        assert_eq!(cell_at(3.0, 3.0, 1), (-1, -1));
    }
}
