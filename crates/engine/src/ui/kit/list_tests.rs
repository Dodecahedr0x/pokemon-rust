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
