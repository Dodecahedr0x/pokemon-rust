use super::*;

#[test]
fn set_count_and_round_trip() {
    let mut d = Pokedex::default();
    assert!(d.is_empty());
    d.set(DexFlag::Seen, SpeciesId(25));
    d.set(DexFlag::Seen, SpeciesId(25));
    d.set_caught(SpeciesId(1));
    d.set(DexFlag::Seen, SpeciesId(0));
    d.set(DexFlag::Seen, SpeciesId(MAX_SPECIES));
    assert_eq!(d.count(DexFlag::Seen), 2);
    assert_eq!(d.count(DexFlag::Caught), 1);
    assert!(d.get(DexFlag::Caught, SpeciesId(1)));
    assert!(!d.get(DexFlag::Caught, SpeciesId(25)));
    let back = Pokedex::from_raw(d.raw(DexFlag::Seen), d.raw(DexFlag::Caught)).unwrap();
    assert_eq!(back, d);
    assert!(Pokedex::from_raw(&[1], &[]).is_err());
    assert!(Pokedex::from_raw(&[2, 0], &[]).is_err());
}

#[test]
fn digest_is_order_independent() {
    let mut a = Pokedex::default();
    a.set(DexFlag::Seen, SpeciesId(3));
    a.set(DexFlag::Seen, SpeciesId(7));
    let mut b = Pokedex::default();
    b.set(DexFlag::Seen, SpeciesId(7));
    b.set(DexFlag::Seen, SpeciesId(3));
    assert_eq!(a, b);
}
