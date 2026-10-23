use super::*;

fn mon(hp: u16) -> Pokemon {
    Pokemon {
        species: SpeciesId(1),
        personality: 0,
        ot_id: 0,
        nickname: [0xFF; NAME_LEN],
        level: 5,
        exp: 0,
        ivs: [0; 6],
        evs: [0; 6],
        moves: [None; 4],
        pp: [0; 4],
        pp_bonuses: 0,
        friendship: 0,
        ability_slot: 0,
        status: Status::None,
        hp,
        stats: [hp; 6],
        held: None,
    }
}

#[test]
fn type_and_stat_orders_match_gen3() {
    for (i, t) in Type::ALL.iter().enumerate() {
        assert_eq!(*t as usize, i);
        assert_eq!(Type::from_u8(i as u8), Some(*t));
    }
    assert_eq!(Type::from_u8(18), None);
    assert_eq!(Type::Dark as u8, 17);
    assert!(Type::Steel.is_physical());
    assert!(!Type::Mystery.is_physical());
    assert!(!Type::Fire.is_physical());
    for (i, s) in Stat::ALL.iter().enumerate() {
        assert_eq!(*s as usize, i);
    }
}

#[test]
fn nature_effects_follow_gnaturestattable() {
    // NATURE_HARDY, LONELY (+Atk -Def), ADAMANT (+Atk -SpA), BOLD (+Def -Atk), QUIRKY (24).
    assert_eq!(Nature(0).effect(Stat::Atk), 0);
    assert_eq!(Nature(1).effect(Stat::Atk), 1);
    assert_eq!(Nature(1).effect(Stat::Def), -1);
    assert_eq!(Nature(3).effect(Stat::SpAtk), -1);
    assert_eq!(Nature(3).effect(Stat::Speed), 0);
    assert_eq!(Nature(5).effect(Stat::Def), 1);
    assert_eq!(Nature(5).effect(Stat::Atk), -1);
    for s in Stat::ALL {
        assert_eq!(Nature(24).effect(s), 0);
        assert_eq!(Nature(1).effect(Stat::Hp), 0);
    }
    for n in 0..25 {
        let sum: i8 = Stat::ALL.iter().map(|&s| Nature(n).effect(s)).sum();
        assert_eq!(sum, 0, "nature {n}");
    }
}

#[test]
fn party_stays_compact() {
    let mut p = Party::default();
    assert!(p.is_empty());
    assert!(p.all_fainted());
    assert_eq!(p.first_usable(), None);
    assert_eq!(p.push(mon(0)), Ok(0));
    assert_eq!(p.push(mon(7)), Ok(1));
    assert_eq!(p.len(), 2);
    assert_eq!(p.first_usable(), Some(1));
    assert!(!p.all_fainted());
    for _ in 2..PARTY_SIZE {
        assert!(p.push(mon(1)).is_ok());
    }
    assert_eq!(p.push(mon(9)), Err(mon(9)));
    assert_eq!(p.get(1).map(|m| m.hp), Some(7));
    assert_eq!(p.get(6), None);
    p.get_mut(0).unwrap().hp = 3;
    assert_eq!(p.first_usable(), Some(0));
    assert_eq!(p.iter().count(), PARTY_SIZE);
}

#[test]
fn hashes_see_every_change() {
    let base = |p: &Party| {
        let mut h = Fnv64::new();
        p.hash(&mut h);
        h.finish()
    };
    let mut p = Party::default();
    let empty = base(&p);
    p.push(mon(5)).unwrap();
    let one = base(&p);
    assert_ne!(empty, one);
    p.get_mut(0).unwrap().status = Status::Sleep(2);
    let slept = base(&p);
    assert_ne!(one, slept);
    p.get_mut(0).unwrap().status = Status::Sleep(1);
    assert_ne!(slept, base(&p));
    p.get_mut(0).unwrap().moves[0] = Some(MoveId(0));
    let with_move = base(&p);
    p.get_mut(0).unwrap().moves[0] = None;
    assert_ne!(with_move, base(&p));
}

#[test]
fn moves_at_level_filters_the_learnset() {
    static LEARNSET: [LevelUpMove; 3] = [
        LevelUpMove {
            level: 1,
            mov: MoveId(1),
        },
        LevelUpMove {
            level: 6,
            mov: MoveId(2),
        },
        LevelUpMove {
            level: 6,
            mov: MoveId(3),
        },
    ];
    static SPECIES: [SpeciesInfo; 2] = [
        SpeciesInfo {
            name: &[0xFF],
            base: [0; 6],
            types: [Type::Normal; 2],
            catch_rate: 0,
            exp_yield: 0,
            ev_yield: [0; 6],
            gender_ratio: 0,
            friendship: 0,
            growth: GrowthRate::MediumFast,
            abilities: [Ability::None; 2],
            learnset: &[],
            weight: 0,
            gfx: None,
        },
        SpeciesInfo {
            name: &[0xFF],
            base: [0; 6],
            types: [Type::Water; 2],
            catch_rate: 0,
            exp_yield: 0,
            ev_yield: [0; 6],
            gender_ratio: 0,
            friendship: 0,
            growth: GrowthRate::MediumSlow,
            abilities: [Ability::Torrent, Ability::None],
            learnset: &LEARNSET,
            weight: 0,
            gfx: None,
        },
    ];
    let t = BattleTables {
        species: &SPECIES,
        ..EMPTY_TABLES
    };
    let m = mon(1);
    let at6: alloc::vec::Vec<_> = m.moves_at_level(&t, 6).collect();
    assert_eq!(at6, [MoveId(2), MoveId(3)]);
    assert_eq!(m.moves_at_level(&t, 2).count(), 0);
}
