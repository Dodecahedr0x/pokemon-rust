//! The Data tab's model (M6 Task H, `data_view`): rows, lists, links and text layout. No FR
//! literals: text inputs are synthetic.

use engine::data::{MapId, TextId};
use studio::data_view::{
    self, Part, Pick, Sub, ability_or_other, gender, or_dash, rows, rows_if_ready, text_parts,
};

#[test]
fn text_parts_split_escapes_and_placeholders() {
    use Part::*;
    let t = |s: &str| Text(s.into());
    assert_eq!(
        text_parts(r"ab\ncd{PLAYER}e\l\pf{STRING 3}"),
        [
            t("ab"),
            Break('n'),
            t("cd"),
            Tag("PLAYER".into()),
            t("e"),
            Break('l'),
            Break('p'),
            t("f"),
            Tag("STRING 3".into())
        ]
    );
    assert_eq!(text_parts(r"a\xb{c"), [t(r"a\xb{c")]);
    assert_eq!(text_parts("x\\"), [t("x\\")]);
    assert_eq!(text_parts(""), []);
    assert_eq!(text_parts("\u{e9}{X}"), [t("\u{e9}"), Tag("X".into())]);
}

#[test]
fn genders_abilities_and_dashes() {
    assert_eq!(gender(255), "genderless");
    assert_eq!(gender(254), "always female");
    assert_eq!(gender(0), "always male");
    assert_eq!(gender(127), "49.6% female");
    assert_eq!(gender(31), "12.1% female");
    assert_eq!(ability_or_other(""), "(other)");
    assert_eq!(ability_or_other("X"), "X");
    assert_eq!(or_dash(0), "—");
    assert_eq!(or_dash(40), "40");
}

#[test]
fn rows_are_built_once_and_ids_index_them() {
    let r = rows();
    assert!(std::ptr::eq(r, rows_if_ready().unwrap()));
    assert_eq!(r.species.len(), data_view::species().len());
    assert_eq!(r.texts.len(), data_view::texts().len());
    for s in &r.species {
        assert_eq!(r.species(s.id).map(|x| x.id), Some(s.id));
        assert!(r.has(Pick::Species(s.id)));
        for &m in &s.maps {
            assert!(r.has(Pick::Map(m)), "species maps have encounter rows");
        }
        for &t in &s.trainers {
            assert!(r.has(Pick::Trainer(t)));
        }
    }
    for m in &r.moves {
        assert_eq!(r.mov(m.id).map(|x| x.id), Some(m.id));
    }
    for t in &r.texts {
        assert_eq!(r.text(t.id).map(|x| x.id), Some(t.id));
    }
    assert!(!r.has(Pick::Map(MapId(u16::MAX))));
    assert!(!r.has(Pick::Text(TextId(u16::MAX))));
}

#[test]
fn lists_follow_the_filter_and_picks_their_sub_tab() {
    let r = rows();
    for (sub, n) in [
        (Sub::Species, r.species.len()),
        (Sub::Moves, r.moves.len()),
        (Sub::Trainers, r.trainers.len()),
        (Sub::Encounters, r.encounters.len()),
        (Sub::Texts, r.texts.len()),
    ] {
        let list = r.list(sub, "");
        assert_eq!(list.len(), n, "{sub:?}");
        assert!(list.iter().all(|(p, _)| p.sub() == sub && r.has(*p)));
    }
    // Case-insensitive, on the decoded name.
    let s = &r.species[1];
    let hits = r.list(Sub::Species, &s.name.to_uppercase());
    assert!(hits.iter().any(|(p, _)| *p == Pick::Species(s.id)));
    assert!(hits.len() < r.species.len());
    assert_eq!(Pick::Map(MapId(1)).sub(), Sub::Encounters);
    assert_eq!(Pick::Text(TextId(1)).sub(), Sub::Texts);
}
