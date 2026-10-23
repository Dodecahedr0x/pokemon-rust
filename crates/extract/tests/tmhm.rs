//! `sTMHMMoves` and `gTMHMLearnsets` (`ui_text.rs`) against the decomp: the US ROM gives
//! exactly `FOREACH_TMHM` and `tmhm_learnsets.h`, the French ROM the same tables.
use extract::schema::TmHmFile;
use extract::{Rom, fixtures, ui_text};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn us() -> &'static (Rom, TmHmFile) {
    static D: OnceLock<(Rom, TmHmFile)> = OnceLock::new();
    D.get_or_init(|| {
        let rom = fixtures::us();
        let t = ui_text::extract_tmhm(&rom).unwrap().0;
        (rom, t)
    })
}

fn src(rel: &str) -> String {
    let p = fixtures::decomp().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `FOREACH_TMHM`'s ids (`FOCUS_PUNCH`, …, `DIVE`).
fn tmhm_ids() -> Vec<String> {
    let h = src("include/constants/tms_hms.h");
    h.split("F(")
        .skip(1)
        .filter_map(|s| s.split_once(')').map(|(id, _)| id.trim().to_string()))
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .collect()
}

#[test]
fn us_moves_are_foreach_tmhm() {
    let ids = tmhm_ids();
    assert_eq!(ids.len(), ui_text::TMHM_COUNT);
    let (_, t) = us();
    let moves: Vec<_> = t.moves.iter().map(|(_, m)| m.clone()).collect();
    let want: Vec<_> = ids.iter().map(|id| format!("MOVE_{id}")).collect();
    assert_eq!(moves, want);
    assert_eq!(t.moves[0].0, "ITEM_TM01");
    assert_eq!(t.moves[49].0, "ITEM_TM50");
    assert_eq!(t.moves[50].0, "ITEM_HM01");
    assert_eq!(t.moves[57].0, "ITEM_HM08");
}

#[test]
fn us_learnsets_are_the_decomp_ones() {
    let ids = tmhm_ids();
    let h = src("src/data/pokemon/tmhm_learnsets.h");
    let mut want = BTreeMap::new();
    for block in h.split("[SPECIES_").skip(1) {
        let (name, body) = block.split_once(']').unwrap();
        let body = body.split("}}").next().unwrap_or(body);
        let mut bits = 0u64;
        for part in body.split('.').skip(1) {
            if let Some((id, v)) = part.split_once('=')
                && v.trim_start().starts_with("TRUE")
            {
                let i = ids.iter().position(|x| x == id.trim()).unwrap();
                bits |= 1 << i;
            }
        }
        if bits != 0 {
            want.insert(format!("SPECIES_{name}"), bits);
        }
    }
    let (_, t) = us();
    assert_eq!(t.learnsets.len(), want.len());
    assert_eq!(t.learnsets, want);
}

#[test]
fn fr_tables_match_us() {
    let fr = fixtures::fr();
    let t = ui_text::extract_tmhm(&fr).unwrap().0;
    assert_eq!(&t, &us().1);
}
