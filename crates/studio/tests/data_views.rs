//! Task F: the data view rows. Structure only (no ROM text literals); the stub expectations come
//! from `crates/content/build/stub.rs`.

use content::battle;
use engine::data::{MapId, SpeciesId, TextId, TrainerId};
use engine::mon::Type;
use std::collections::BTreeSet;
use studio::data_view::{self, LAND_SLOT_PERCENT, filter};
use studio::text::decode;

fn clean(s: &str) -> bool {
    !s.is_empty() && !s.contains("<?")
}

#[test]
fn row_counts_equal_the_tables_and_ids_are_indexes() {
    let species = data_view::species();
    assert_eq!(species.len(), battle::TABLES.species.len());
    assert!(
        species
            .iter()
            .enumerate()
            .all(|(i, r)| r.id == SpeciesId(i as u16))
    );

    let moves = data_view::moves();
    assert_eq!(moves.len(), battle::TABLES.moves.len());
    assert!(moves.iter().enumerate().all(|(i, r)| r.id.0 as usize == i));

    let trainers = data_view::trainers();
    assert_eq!(trainers.len(), battle::TRAINER_IDS.len());
    let ids: Vec<TrainerId> = trainers.iter().map(|t| t.id).collect();
    assert_eq!(ids, battle::TRAINER_IDS);

    let with_table: Vec<MapId> = content::data::MAP_IDS
        .iter()
        .copied()
        .filter(|m| battle::encounters(m.0).is_some())
        .collect();
    let enc = data_view::encounters();
    assert_eq!(enc.iter().map(|e| e.map).collect::<Vec<_>>(), with_table);

    let texts = data_view::texts();
    assert_eq!(texts.len(), content::data::TEXTS.len());
    assert_eq!(texts.len(), content::data::TEXT_NAMES.len());
    for (i, t) in texts.iter().enumerate() {
        assert_eq!(t.id, TextId(i as u16));
        assert_eq!(t.key, content::data::TEXT_NAMES[i]);
    }
}

#[test]
fn slice_names_decode_cleanly() {
    let species = data_view::species();
    let moves = data_view::moves();
    assert!(!battle::SLICE_SPECIES.is_empty() && !battle::SLICE_MOVES.is_empty());
    for s in &battle::SLICE_SPECIES {
        let r = &species[s.0 as usize];
        assert!(clean(&r.name), "species {}: {:?}", s.0, r.name);
        assert!(r.has_gfx, "slice species {} has pics", s.0);
    }
    for m in &battle::SLICE_MOVES {
        let r = &moves[m.0 as usize];
        assert!(clean(&r.name), "move {}: {:?}", m.0, r.name);
    }
    for t in Type::ALL {
        assert!(clean(&data_view::type_name(t)), "{t:?}");
    }
}

#[test]
fn learnsets_and_learners_agree() {
    let species = data_view::species();
    let moves = data_view::moves();
    for s in &species {
        for (_, m, name) in &s.learnset {
            let mv = moves.get(m.0 as usize).expect("learnset move in moves()");
            assert_eq!(&mv.name, name);
            assert!(mv.learners.contains(&s.id));
        }
    }
    for m in &moves {
        assert!(m.learners.windows(2).all(|w| w[0] < w[1]));
        for l in &m.learners {
            assert!(species[l.0 as usize].learnset.iter().any(|x| x.1 == m.id));
        }
    }
}

#[test]
fn trainer_parties_and_species_trainers_agree() {
    let species = data_view::species();
    let moves = data_view::moves();
    let trainers = data_view::trainers();
    for t in &trainers {
        assert!(!t.party.is_empty(), "trainer {} has a party", t.id.0);
        for p in &t.party {
            let s = species
                .get(p.species.0 as usize)
                .expect("party species in species()");
            assert_eq!(s.name, p.name);
            assert!(s.trainers.contains(&t.id));
            for (m, name) in p.moves.iter().flatten() {
                assert_eq!(&moves[m.0 as usize].name, name);
            }
        }
    }
    for s in &species {
        for id in &s.trainers {
            let t = trainers.iter().find(|t| t.id == *id).unwrap();
            assert!(t.party.iter().any(|p| p.species == s.id));
        }
    }
}

#[test]
fn encounter_slots_sum_to_100_and_species_maps_agree() {
    assert_eq!(
        LAND_SLOT_PERCENT.iter().map(|&p| p as u32).sum::<u32>(),
        100
    );
    let species = data_view::species();
    let enc = data_view::encounters();
    for e in &enc {
        assert!(!e.map_name.is_empty());
        assert_eq!(e.mons.len(), 12);
        assert_eq!(e.mons.iter().map(|m| m.4 as u32).sum::<u32>(), 100);
        for (sp, name, min, max, _) in &e.mons {
            let s = &species[sp.0 as usize];
            assert!(clean(name), "species {} in {}", sp.0, e.map_name);
            assert_eq!(&s.name, name);
            assert!(min <= max);
            assert!(s.maps.contains(&e.map));
        }
    }
    for s in &species {
        for m in &s.maps {
            let e = enc.iter().find(|e| e.map == *m).unwrap();
            assert!(e.mons.iter().any(|x| x.0 == s.id));
        }
    }
}

#[test]
fn texts_decode_cleanly() {
    for t in data_view::texts() {
        assert!(!t.key.is_empty());
        assert!(!t.text.contains("<?"), "{}", t.key);
    }
}

/// Mixed case: upper-case the even characters, lower-case the odd ones.
fn mixed(s: &str) -> String {
    s.chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i % 2 == 0 {
                c.to_uppercase().collect::<Vec<_>>()
            } else {
                c.to_lowercase().collect()
            }
        })
        .collect()
}

#[test]
fn filter_is_case_insensitive_on_the_decoded_name() {
    let species = data_view::species();
    let name = |r: &data_view::SpeciesRow| r.name.clone();
    let s = &species[battle::SLICE_SPECIES[0].0 as usize];
    for q in [s.name.to_uppercase(), s.name.to_lowercase(), mixed(&s.name)] {
        let hits = filter(&species, &q, name);
        assert!(hits.iter().any(|r| r.id == s.id), "query {q:?}");
        assert!(
            hits.iter()
                .all(|r| r.name.to_lowercase().contains(&q.to_lowercase()))
        );
    }
    // The key is the decoded name, so the raw Gen3 bytes do not match.
    let raw: String = battle::TABLES.species[s.id.0 as usize]
        .name
        .iter()
        .take_while(|&&b| b != 0xFF)
        .map(|&b| b as char)
        .collect();
    if raw.to_lowercase() != s.name.to_lowercase() {
        assert!(filter(&species, &raw, name).iter().all(|r| r.id != s.id));
    }
    assert_eq!(filter(&species, "", name).len(), species.len());
    assert_eq!(filter(&species, "  ", name).len(), species.len());
    assert!(filter(&species, "\u{1}no such name\u{1}", name).is_empty());
}

#[test]
fn decode_matches_the_tables() {
    for s in &battle::SLICE_SPECIES {
        assert_eq!(
            decode(battle::TABLES.species[s.0 as usize].name),
            data_view::species()[s.0 as usize].name
        );
    }
}

/// `crates/content/build/stub.rs`: species NONE, STUB_A, STUB_B; moves NONE, STUB_HIT,
/// STUB_GROWL, STUB_STRUGGLE; no referenced trainer; one land table on StubTown (rate 20,
/// slots alternating STUB_A level 2 / STUB_B level 3); no texts.
#[test]
fn stub_counts() {
    if !content::STUB {
        return;
    }
    let species = data_view::species();
    assert_eq!(species.len(), 3);
    assert_eq!(data_view::moves().len(), 4);
    assert!(data_view::trainers().is_empty());
    assert!(data_view::texts().is_empty());
    let enc = data_view::encounters();
    assert_eq!(enc.len(), 1);
    assert_eq!(enc[0].map_name, "StubTown");
    assert_eq!(enc[0].rate, 20);
    let (a, b) = (SpeciesId(1), SpeciesId(2));
    for (i, m) in enc[0].mons.iter().enumerate() {
        let (sp, lv) = if i % 2 == 0 { (a, 2) } else { (b, 3) };
        assert_eq!((m.0, m.2, m.3), (sp, lv, lv));
    }
    assert_eq!(species[1].maps, vec![enc[0].map]);
    assert_eq!(species[1].learnset.len(), 2);
    assert_eq!(species[2].learnset.len(), 1);
    assert!(species[1].has_gfx && species[2].has_gfx && !species[0].has_gfx);
    let learners: BTreeSet<_> = data_view::moves()[1].learners.iter().copied().collect();
    assert_eq!(learners, BTreeSet::from([a, b]));
}
