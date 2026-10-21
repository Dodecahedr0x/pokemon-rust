//! ROM-backed checks of the event script walker against the decomp (US) and the French ROM.
use extract::maps::{LITTLEROOT, MapId, OLDALE, ROUTE101, locate_map_table};
use extract::scripts::{self, ScriptScan};
use extract::{Rom, fixtures, text};
use std::collections::{BTreeMap, BTreeSet};

fn scan(rom: &Rom, id: MapId) -> ScriptScan {
    let table = locate_map_table(rom).unwrap();
    let header = table.header(rom, id).unwrap();
    let roots = scripts::map_script_roots(rom, &header).unwrap();
    assert!(!roots.is_empty());
    scripts::walk(rom, &roots).unwrap()
}

/// `label -> text` for every `.string` label of a decomp `scripts.inc`.
fn decomp_texts(src: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut label = None;
    let mut cur = String::new();
    for line in src.lines() {
        let line = line.trim();
        if let Some(l) = line.strip_suffix(':') {
            label = Some(l.trim_end_matches(':').to_string());
        } else if let Some(rest) = line.strip_prefix(".string") {
            let rest = rest.trim();
            cur.push_str(&rest[1..rest.len() - 1]);
            if let Some(done) = cur.strip_suffix('$') {
                out.insert(label.clone().unwrap(), done.to_string());
                cur.clear();
            }
        }
    }
    out
}

/// Labels used as the text operand of `msgbox`/`message` in a decomp `scripts.inc`.
fn message_refs(src: &str) -> BTreeSet<String> {
    src.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l
                .strip_prefix("msgbox ")
                .or_else(|| l.strip_prefix("message "))?;
            Some(rest.split(',').next().unwrap().trim().to_string())
        })
        .collect()
}

/// Rewrites `{CLEAR_TO 0x..}` to decimal, the decoder's form.
fn clear_to_decimal(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("{CLEAR_TO 0x") {
        out.push_str(&rest[..i]);
        let after = &rest[i + "{CLEAR_TO 0x".len()..];
        let end = after.find('}').unwrap();
        let n = u32::from_str_radix(&after[..end], 16).unwrap();
        out.push_str(&format!("{{CLEAR_TO {n}}}"));
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn us_littleroot_reaches_every_decomp_message() {
    let rom = fixtures::us();
    let s = scan(&rom, LITTLEROOT);
    let src =
        std::fs::read_to_string(fixtures::decomp().join("data/maps/LittlerootTown/scripts.inc"))
            .unwrap();
    let texts = decomp_texts(&src);
    let refs = message_refs(&src);
    let mut checked = 0;
    for label in refs.iter().filter(|l| texts.contains_key(*l)) {
        let want = &texts[label];
        let hits = rom.find_all(&text::encode(want).unwrap());
        let reached: Vec<_> = hits.iter().filter(|h| s.texts.contains(h)).collect();
        assert!(
            !reached.is_empty(),
            "{label} not reached (ROM hits {hits:x?})"
        );
        for &&off in &reached {
            assert_eq!(
                rom.string_at(off).unwrap(),
                clear_to_decimal(want),
                "{label}"
            );
        }
        checked += 1;
    }
    assert!(checked > 0 && s.texts.len() >= checked);
    eprintln!(
        "US Littleroot: {checked} decomp messages reached; {} texts, {} movements, {} instructions",
        s.texts.len(),
        s.movements.len(),
        s.visited.len()
    );
}

#[test]
fn fr_littleroot_matches_us_text_count_and_decodes() {
    let us = scan(&fixtures::us(), LITTLEROOT);
    let rom = fixtures::fr();
    let fr = scan(&rom, LITTLEROOT);
    assert_eq!(fr.texts.len(), us.texts.len());
    assert_eq!(fr.movements.len(), us.movements.len());
    assert_eq!(fr.visited.len(), us.visited.len());
    let mut accented = 0;
    for &off in &fr.texts {
        let s = rom.string_at(off).unwrap();
        if s.chars()
            .any(|c| "àâäçéèêëîïôöùûüÀÂÄÇÉÈÊËÎÏÔÖÙÛÜ".contains(c))
        {
            accented += 1;
        }
    }
    assert!(accented > 0);
    for &off in fr.texts.iter().take(5) {
        eprintln!("0x{off:x}: {:?}", rom.string_at(off).unwrap());
    }
}

#[test]
fn all_maps_walk_without_unknown_opcodes() {
    for (name, rom) in [("US", fixtures::us()), ("FR", fixtures::fr())] {
        for id in [LITTLEROOT, OLDALE, ROUTE101] {
            let s = scan(&rom, id);
            assert!(s.unknown.is_empty(), "{name} {id:?}: {:x?}", s.unknown);
            for &off in &s.texts {
                rom.string_at(off).unwrap();
            }
            eprintln!(
                "{name} {id:?}: {} texts, {} movements, {} instructions",
                s.texts.len(),
                s.movements.len(),
                s.visited.len()
            );
        }
    }
}
