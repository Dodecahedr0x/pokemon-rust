use super::*;

/// Center row is three red 7s (`sReelSymbols[*][0]`) when each reel rests at 19.
const RED_SEVENS: [u8; 3] = [19, 19, 19];
/// Center row is Azurill / Replay / blue 7: no line pays on a 1-coin bet.
const NO_MATCH: [u8; 3] = [0, 0, 0];

#[test]
fn a_winning_spin_pays_and_a_losing_spin_costs_the_bet() {
    let mut m = Machine::new(u16::from(UNLUCKIEST));
    let mut coins = 100;
    let mut rng = Rng::new(1);

    m.force_stops(RED_SEVENS);
    assert!(!m.input(&mut coins, MAX_COINS, &mut rng, Buttons::DOWN));
    assert_eq!(m.bet, 1);
    assert_eq!(coins, 99);
    assert!(!m.input(&mut coins, MAX_COINS, &mut rng, Buttons::A));
    assert_eq!(m.grid()[1], [Symbol::Red7; 3]);
    assert_eq!(m.last_payout, 300);
    assert_eq!(coins, 399);

    m.force_stops(NO_MATCH);
    assert!(!m.input(&mut coins, MAX_COINS, &mut rng, Buttons::DOWN));
    assert_eq!(coins, 398);
    assert!(!m.input(&mut coins, MAX_COINS, &mut rng, Buttons::A));
    assert_eq!(
        m.grid()[1],
        [Symbol::Azurill, Symbol::Replay, Symbol::Blue7]
    );
    assert_eq!(m.last_payout, 0);
    assert_eq!(coins, 398);
}
