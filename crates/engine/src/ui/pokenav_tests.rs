use super::*;

#[test]
fn the_latest_available_line_is_the_one_spoken() {
    let lines = [
        MatchCallLine {
            text: b"\xff",
            requires: None,
            sets: Some(FlagId(1)),
        },
        MatchCallLine {
            text: &[1, 0xFF],
            requires: Some(FlagId(1)),
            sets: None,
        },
        MatchCallLine {
            text: &[2, 0xFF],
            requires: Some(FlagId(2)),
            sets: None,
        },
    ];
    assert_eq!(spoken_line(&lines, |_| false).unwrap().text, b"\xff");
    assert_eq!(
        spoken_line(&lines, |f| f == FlagId(1)).unwrap().text,
        &[1, 0xFF]
    );
    assert_eq!(spoken_line(&lines, |_| true).unwrap().text, &[2, 0xFF]);
}
