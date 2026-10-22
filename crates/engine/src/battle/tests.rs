use super::*;

#[test]
fn outcomes_are_gen3_b_outcome_values() {
    for o in [
        BattleOutcome::Won,
        BattleOutcome::Lost,
        BattleOutcome::Drew,
        BattleOutcome::Ran,
        BattleOutcome::MonFled,
        BattleOutcome::Caught,
    ] {
        assert_eq!(BattleOutcome::from_u16(o as u16), Some(o));
    }
    assert_eq!(BattleOutcome::Won as u8, 1);
    assert_eq!(BattleOutcome::MonFled as u8, 6);
    assert_eq!(BattleOutcome::from_u16(5), None);
    assert_eq!(BattleOutcome::from_u16(0), None);
}

#[test]
fn environment_order_and_sides() {
    assert_eq!(Environment::Grass as u8, 0);
    assert_eq!(Environment::Plain as u8, 9);
    assert_eq!(Side::Player.other(), Side::Opponent);
    assert_eq!(Side::Opponent as usize, 1);
}
