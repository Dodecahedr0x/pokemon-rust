use super::player_name;

#[test]
fn truncates_to_seven_and_pads_with_eos() {
    assert_eq!(
        player_name(&[1, 2, 3, 0xFF, 9]),
        [1, 2, 3, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(player_name(&[1; 10]), [1, 1, 1, 1, 1, 1, 1, 0xFF]);
    assert_eq!(player_name(&[]), [0xFF; 8]);
}
