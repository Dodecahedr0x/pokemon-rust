//! `gBerries` (`berries.rs`) against the decomp: the US and the French ROM give `berry.c`'s
//! yields and stage durations, in `ITEM_*_BERRY` order (Wave E).
use extract::{berries, fixtures};

/// `(maxYield, minYield, stageDuration)` of each `gBerries` entry in `src/berry.c`.
fn decomp() -> Vec<(u8, u8, u8)> {
    let p = fixtures::decomp().join("src/berry.c");
    let c = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let table = c.split("const struct Berry gBerries[] =").nth(1).unwrap();
    let table = table.split("};").next().unwrap();
    let field = |entry: &str, name: &str| -> u8 {
        let v = entry.split(&format!(".{name} = ")).nth(1).unwrap();
        v.split(',').next().unwrap().trim().parse().unwrap()
    };
    table
        .split("[ITEM_")
        .skip(1)
        .map(|e| {
            (
                field(e, "maxYield"),
                field(e, "minYield"),
                field(e, "stageDuration"),
            )
        })
        .collect()
}

#[test]
fn us_and_fr_berries_are_the_decomp_table() {
    let want = decomp();
    assert_eq!(want.len(), berries::BERRY_COUNT as usize);
    for rom in [fixtures::us(), fixtures::fr()] {
        let (file, _) = berries::extract(&rom).unwrap();
        let got: Vec<_> = (file.berries.iter())
            .map(|b| (b.max_yield, b.min_yield, b.stage_duration))
            .collect();
        assert_eq!(got, want);
        assert_eq!(file.berries[6].item, "ITEM_ORAN_BERRY");
    }
}
