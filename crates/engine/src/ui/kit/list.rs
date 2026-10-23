//! Menu cursors: Gen3 `Menu_ProcessInput` (a small menu whose cursor wraps: the start menu,
//! the party actions) and `ListMenu` (`list_menu.c`: a scrolling list that does not wrap,
//! with `maxShowed` rows, scroll arrows and paging).
//!
//! Deviation: Gen3 reads `JOY_REPEAT` for the d-pad (a held direction repeats after a delay);
//! screens see new presses only, so a held direction moves once.

use crate::hash::Fnv64;
use crate::input::Buttons;

/// Gen3 `MENU_B_PRESSED` / `LIST_CANCEL` and the chosen entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuInput {
    /// Nothing this frame.
    None,
    /// The cursor moved (redraw it; the list's description callback).
    Moved,
    Chosen(u16),
    Cancel,
}

/// Gen3 `Menu_ProcessInput` over `count` rows (`InitMenuInUpperLeftCorner`, wrapping).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MenuCursor {
    pub pos: u8,
    pub count: u8,
}

impl MenuCursor {
    pub const fn new(count: u8, pos: u8) -> Self {
        MenuCursor {
            pos: if pos < count { pos } else { 0 },
            count,
        }
    }

    /// A chooses, B cancels, up/down move with wrap (`Menu_MoveCursor`).
    pub fn input(&mut self, pressed: Buttons) -> MenuInput {
        if self.count == 0 {
            return if pressed.contains(Buttons::B) || pressed.contains(Buttons::A) {
                MenuInput::Cancel
            } else {
                MenuInput::None
            };
        }
        if pressed.contains(Buttons::A) {
            MenuInput::Chosen(self.pos as u16)
        } else if pressed.contains(Buttons::B) {
            MenuInput::Cancel
        } else if pressed.contains(Buttons::UP) {
            self.pos = if self.pos == 0 {
                self.count - 1
            } else {
                self.pos - 1
            };
            MenuInput::Moved
        } else if pressed.contains(Buttons::DOWN) {
            self.pos = if self.pos + 1 >= self.count {
                0
            } else {
                self.pos + 1
            };
            MenuInput::Moved
        } else {
            MenuInput::None
        }
    }

    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.pos, self.count]);
    }
}

/// Gen3 `scrollMultiple`: what moves a whole page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paging {
    None,
    /// `LIST_MULTIPLE_SCROLL_DPAD`: left/right.
    Dpad,
    /// `LIST_MULTIPLE_SCROLL_L_R`.
    LR,
}

/// Gen3 `struct ListMenu` without headers: `total` items, `max_showed` rows, the selected row
/// in the window and the scroll offset.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ListMenu {
    pub total: u16,
    pub max_showed: u16,
    pub row: u16,
    pub scroll: u16,
    pub paging: Paging,
}

impl ListMenu {
    /// `ListMenuInit` with the cursor on `selected` (`initialCursorPos` and `scrollOffset` as
    /// the bag keeps them: the item shown as low as the window allows).
    pub fn new(total: u16, max_showed: u16, paging: Paging) -> Self {
        ListMenu {
            total,
            max_showed: max_showed.min(total).max(1),
            row: 0,
            scroll: 0,
            paging,
        }
    }

    /// The selected item's index.
    pub const fn selected(&self) -> u16 {
        self.scroll + self.row
    }

    /// Items shown: `scroll .. scroll + max_showed`.
    pub fn shown(&self) -> core::ops::Range<u16> {
        self.scroll..(self.scroll + self.max_showed).min(self.total)
    }

    /// Gen3 `AddScrollIndicatorArrowPair`'s arrows: (more above, more below).
    pub const fn arrows(&self) -> (bool, bool) {
        (
            self.scroll > 0,
            self.scroll + self.max_showed < self.total,
        )
    }

    /// Gen3 `ListMenu_ProcessInput`: A answers the item, B cancels, up/down move one item and
    /// the paging buttons a page (`ListMenuChangeSelection`, no wrap).
    pub fn input(&mut self, pressed: Buttons) -> MenuInput {
        if pressed.contains(Buttons::A) {
            return if self.total == 0 {
                MenuInput::Cancel
            } else {
                MenuInput::Chosen(self.selected())
            };
        }
        if pressed.contains(Buttons::B) {
            return MenuInput::Cancel;
        }
        let (count, down) = if pressed.contains(Buttons::UP) {
            (1, false)
        } else if pressed.contains(Buttons::DOWN) {
            (1, true)
        } else {
            let (prev, next) = match self.paging {
                Paging::None => (false, false),
                Paging::Dpad => (
                    pressed.contains(Buttons::LEFT),
                    pressed.contains(Buttons::RIGHT),
                ),
                Paging::LR => (pressed.contains(Buttons::L), pressed.contains(Buttons::R)),
            };
            if prev {
                (self.max_showed, false)
            } else if next {
                (self.max_showed, true)
            } else {
                return MenuInput::None;
            }
        };
        let mut moved = false;
        for _ in 0..count {
            moved |= self.step(down);
        }
        if moved {
            MenuInput::Moved
        } else {
            MenuInput::None
        }
    }

    /// Gen3 `ListMenuUpdateSelectedRowIndexAndScrollOffset`: the cursor walks to the middle row
    /// of the window, then the list scrolls under it; at either end the cursor walks on to the
    /// edge row. Whether anything moved.
    fn step(&mut self, down: bool) -> bool {
        if self.total == 0 {
            return false;
        }
        let m = self.max_showed;
        if !down {
            let mid = if m == 1 { 0 } else { m - (m / 2 + m % 2) - 1 };
            if self.scroll == 0 || self.row > mid {
                if self.row == 0 {
                    return false;
                }
                self.row -= 1;
            } else {
                self.row = mid;
                self.scroll -= 1;
            }
        } else {
            let mid = if m == 1 { 0 } else { m / 2 + m % 2 };
            if self.scroll + m >= self.total || self.row < mid {
                if self.row + 1 >= m {
                    return false;
                }
                self.row += 1;
            } else {
                self.row = mid;
                self.scroll += 1;
            }
        }
        true
    }

    pub fn hash(&self, h: &mut Fnv64) {
        for v in [self.total, self.max_showed, self.row, self.scroll] {
            h.write_u16(v);
        }
        h.write(&[self.paging as u8]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_cursor_wraps_and_answers() {
        let mut c = MenuCursor::new(3, 0);
        assert_eq!(c.input(Buttons::UP), MenuInput::Moved);
        assert_eq!(c.pos, 2);
        assert_eq!(c.input(Buttons::DOWN), MenuInput::Moved);
        assert_eq!(c.pos, 0);
        assert_eq!(c.input(Buttons::A), MenuInput::Chosen(0));
        assert_eq!(c.input(Buttons::B), MenuInput::Cancel);
        assert_eq!(c.input(Buttons::NONE), MenuInput::None);
    }

    #[test]
    fn list_scrolls_under_the_middle_row_and_stops_at_the_ends() {
        // 10 items, 5 shown: the cursor walks to row 3 (Gen3's `maxShowed / 2 + maxShowed % 2`),
        // then the list scrolls; at the end it walks to the last row.
        let mut l = ListMenu::new(10, 5, Paging::None);
        let mut seen = alloc::vec::Vec::new();
        for _ in 0..12 {
            l.input(Buttons::DOWN);
            seen.push((l.row, l.scroll));
        }
        assert_eq!(&seen[..3], &[(1, 0), (2, 0), (3, 0)]);
        assert_eq!(seen[3], (3, 1));
        assert_eq!(seen[7], (3, 5));
        assert_eq!(seen[8], (4, 5));
        assert_eq!(l.selected(), 9);
        assert_eq!(l.input(Buttons::DOWN), MenuInput::None);
        assert_eq!(l.arrows(), (true, false));
        // Up: the cursor walks to row 1 (`maxShowed - (maxShowed / 2 + maxShowed % 2) - 1`),
        // then the list scrolls back.
        for _ in 0..3 {
            l.input(Buttons::UP);
        }
        assert_eq!((l.row, l.scroll), (1, 5));
        l.input(Buttons::UP);
        assert_eq!((l.row, l.scroll), (1, 4));
        for _ in 0..10 {
            l.input(Buttons::UP);
        }
        assert_eq!((l.row, l.scroll), (0, 0));
        assert_eq!(l.input(Buttons::UP), MenuInput::None);
    }

    #[test]
    fn list_pages_with_lr_and_answers_the_item() {
        let mut l = ListMenu::new(20, 6, Paging::LR);
        assert_eq!(l.input(Buttons::R), MenuInput::Moved);
        assert_eq!(l.selected(), 6);
        assert_eq!(l.input(Buttons::L), MenuInput::Moved);
        assert_eq!(l.selected(), 0);
        assert_eq!(l.input(Buttons::A), MenuInput::Chosen(0));
        assert_eq!(l.input(Buttons::B), MenuInput::Cancel);
        let mut short = ListMenu::new(2, 6, Paging::None);
        assert_eq!(short.max_showed, 2);
        short.input(Buttons::DOWN);
        assert_eq!(short.input(Buttons::DOWN), MenuInput::None);
        assert_eq!(short.input(Buttons::A), MenuInput::Chosen(1));
    }
}
