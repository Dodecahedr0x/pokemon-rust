//! Guards on the committed Silice data (`games/silice`): `check` passes without the extract or
//! the decomp, and every id keeps the number it was first given (`fixtures/silice/ids.golden`).

mod silice_common;
use silice_common::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn games() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../games/silice")
}

#[test]
fn committed_data_passes_check_without_the_extract() {
    let g = games();
    let r = xtask(&[
        "check",
        "--games",
        g.to_str().unwrap(),
        "--extracted",
        "/nonexistent",
    ]);
    assert!(r.ok, "{}", r.out);
    assert!(r.out.contains("silice check: ok"), "{}", r.out);
}

#[test]
fn committed_ids_keep_their_numbers() {
    // (file, name) -> every num it has had (a moved id: its tombstone and its live entry).
    let re = regex::Regex::new(r#"name: "([^"]+)",\s*num: (\d+),"#).unwrap();
    let mut now: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
    for e in std::fs::read_dir(games().join("ids")).unwrap() {
        let p = e.unwrap().path();
        let file = p.file_stem().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&p).unwrap();
        for c in re.captures_iter(&text) {
            (now.entry((file.clone(), c[1].to_string())).or_default()).push(c[2].parse().unwrap());
        }
    }
    let golden = std::fs::read_to_string(fixtures().join("ids.golden")).unwrap();
    let mut n = 0;
    for l in golden
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let mut w = l.split(' ');
        let (file, name, num) = (w.next().unwrap(), w.next().unwrap(), w.next().unwrap());
        let num: u32 = num.parse().unwrap();
        match now.get(&(file.to_string(), name.to_string())) {
            Some(got) => assert!(
                got.contains(&num),
                "ids/{file}.ron: {name} was {num}, now {got:?}"
            ),
            None => panic!(
                "ids/{file}.ron: {name} ({num}) is gone; ids are append-only (tombstone instead)"
            ),
        }
        n += 1;
    }
    assert!(n > 900, "{n} golden ids");
}
