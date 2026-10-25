use super::*;

#[test]
fn national_fallback_lists_johto_then_new_hoenn_species() {
    let hoenn = [SpeciesId(252), SpeciesId(1), SpeciesId(300), SpeciesId(0)];
    assert_eq!(national_fallback(400, &hoenn, 0), Some(SpeciesId(1)));
    assert_eq!(national_fallback(400, &hoenn, 250), Some(SpeciesId(251)));
    assert_eq!(national_fallback(400, &hoenn, 251), Some(SpeciesId(252)));
    assert_eq!(national_fallback(400, &hoenn, 252), Some(SpeciesId(300)));
    assert_eq!(national_fallback(400, &hoenn, 253), None);
    // 300 is past a table of 260 species.
    assert_eq!(national_fallback(260, &hoenn, 251), Some(SpeciesId(252)));
    assert_eq!(national_fallback(260, &hoenn, 252), None);
}

#[test]
fn rating_matches_birch_bands() {
    assert_eq!(rating_of(0, 202, false, false), Rating::Band(0));
    assert_eq!(rating_of(9, 202, false, false), Rating::Band(0));
    assert_eq!(rating_of(10, 202, false, false), Rating::Band(1));
    assert_eq!(rating_of(199, 202, false, false), Rating::Band(19));
    assert_eq!(rating_of(200, 202, false, false), Rating::Completed);
    assert_eq!(rating_of(200, 202, true, false), Rating::Band(19));
    assert_eq!(rating_of(200, 202, false, true), Rating::Band(19));
    assert_eq!(rating_of(201, 202, true, true), Rating::Band(19));
    assert_eq!(rating_of(201, 202, true, false), Rating::Completed);
    assert_eq!(rating_of(202, 202, true, true), Rating::Completed);
    assert_eq!(rating_of(1, 3, false, false), Rating::Band(0));
    assert_eq!(rating_of(3, 3, false, false), Rating::Completed);
    assert_eq!(rating_of(0, 0, false, false), Rating::Band(0));
}
