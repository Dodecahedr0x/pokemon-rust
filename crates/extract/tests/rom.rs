use extract::{GameCode, fixtures};

#[test]
fn fixtures_load_both_versions() {
    let fr = fixtures::fr();
    assert_eq!(fr.code(), GameCode::Bpef);
    assert_eq!(fr.sha1_hex(), "ca666651374d89ca439007bed54d839eb7bd14d0");
    let us = fixtures::us();
    assert_eq!(us.code(), GameCode::Bpee);
    let expected = std::fs::read_to_string(fixtures::decomp().join("rom.sha1")).unwrap();
    assert!(expected.starts_with(&us.sha1_hex()));
}

#[test]
fn littleroot_layout_anchor_matches_ground_truth() {
    let map_bin =
        std::fs::read(fixtures::decomp().join("data/layouts/LittlerootTown/map.bin")).unwrap();
    for (rom, map_off, layout_off) in [
        (fixtures::fr(), 0x3eee50, 0x3ef170),
        (fixtures::us(), 0x3e9f64, 0x3ea284),
    ] {
        assert_eq!(rom.find_all(&map_bin), vec![map_off]);
        assert!(rom.pointers_to(map_off).contains(&(layout_off + 0x0C)));
        assert_eq!(rom.ptr(layout_off + 0x0C).unwrap(), Some(map_off));
    }
}
