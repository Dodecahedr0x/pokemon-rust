//! ROM-backed checks of the text codec against the decomp (US) and the French ROM.
use extract::{fixtures, text};

/// Every `.string` of a decomp `.inc` file, with consecutive lines joined up to the `$`
/// terminator as the assembler does (each `.string` line emits its bytes with no terminator).
fn decomp_strings(path: &std::path::Path) -> Vec<String> {
    let src = std::fs::read_to_string(path).unwrap();
    let mut strings = Vec::new();
    let mut cur = String::new();
    for line in src.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(".string") else {
            assert!(cur.is_empty(), "unterminated string before {line:?}");
            continue;
        };
        let rest = rest.trim();
        assert!(rest.starts_with('"') && rest.ends_with('"'), "{line}");
        cur.push_str(&rest[1..rest.len() - 1]);
        if let Some(done) = cur.strip_suffix('$') {
            strings.push(done.to_string());
            cur.clear();
        }
    }
    assert!(cur.is_empty());
    strings
}

#[test]
fn littleroot_strings_are_found_and_decoded_in_us_rom() {
    let rom = fixtures::us();
    let path = fixtures::decomp().join("data/maps/LittlerootTown/scripts.inc");
    let strings = decomp_strings(&path);
    let terminators = std::fs::read_to_string(&path)
        .unwrap()
        .matches("$\"")
        .count();
    assert_eq!(strings.len(), terminators);
    for s in &strings {
        let bytes = text::encode(s).unwrap();
        let hits = rom.find_all(&bytes);
        assert!(!hits.is_empty(), "not in ROM: {s:?}");
        for off in hits {
            assert_eq!(&rom.string_at(off).unwrap(), s, "at 0x{off:x}");
        }
    }
    eprintln!("matched {} Littleroot strings", strings.len());
}

#[test]
fn bourg_en_vol_round_trips_in_fr_rom() {
    let rom = fixtures::fr();
    let bytes = text::encode("BOURG-EN-VOL").unwrap();
    let hits = rom.find_all(&bytes);
    assert!(!hits.is_empty());
    for off in hits {
        let s = rom.string_at(off).unwrap();
        assert_eq!(s, "BOURG-EN-VOL");
        // The hit and the town names packed after it decode and re-encode to the same bytes.
        let mut at = off as usize;
        for expected in [
            "BOURG-EN-VOL",
            "ROSYERES",
            "VILLAGE MYOKARA",
            "VERMILAVA",
            "AUTEQUIA",
            "VERGAZON",
            "PACIFIVILLE",
            "CLEMENTI-VILLE",
        ] {
            let (decoded, used) = text::decode(&rom.bytes()[at..]).unwrap();
            assert_eq!(decoded, expected, "at 0x{at:x}");
            assert_eq!(text::encode(&decoded).unwrap(), rom.bytes()[at..at + used]);
            at += used;
        }
    }
}

/// Slow (about 90 s in release, much longer in debug): one ROM scan per string.
/// Run with `cargo test --release -p extract --test text -- --ignored`.
#[test]
#[ignore]
fn every_map_string_is_found_and_round_trips_in_us_rom() {
    let rom = fixtures::us();
    let mut count = 0;
    for entry in std::fs::read_dir(fixtures::decomp().join("data/maps")).unwrap() {
        let path = entry.unwrap().path().join("scripts.inc");
        if !path.exists() {
            continue;
        }
        for s in decomp_strings(&path) {
            let bytes = text::encode(&s).unwrap();
            let hits = rom.find_all(&bytes);
            assert!(!hits.is_empty(), "not in ROM: {s:?} ({})", path.display());
            // Byte-exact; the text may differ in argument spelling ({CLEAR_TO 0x0a} vs 10).
            let decoded = rom.string_at(hits[0]).unwrap();
            assert_eq!(text::encode(&decoded).unwrap(), bytes, "{s:?}");
            count += 1;
        }
    }
    assert!(count > 4000, "only {count} strings");
}

/// One parsed line of `charmap.txt`.
enum Entry {
    Char(char, u8),
    Escape(char, u8),
    Constant(String, Vec<u8>),
}

/// Parses the decomp's `charmap.txt`: `'c' = XX`, `'\e' = XX`, `NAME = XX XX`, `@` comments.
fn parse_charmap(src: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    for line in src.lines() {
        let line = line.split(" @").next().unwrap().trim();
        if line.is_empty() || line.starts_with('@') {
            continue;
        }
        let (lhs, rhs) = line.rsplit_once('=').unwrap_or_else(|| panic!("{line}"));
        let (lhs, rhs) = (lhs.trim(), rhs.trim());
        let bytes: Vec<u8> = rhs
            .split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap_or_else(|_| panic!("{line}")))
            .collect();
        if let Some(body) = lhs.strip_prefix('\'').and_then(|l| l.strip_suffix('\'')) {
            assert_eq!(bytes.len(), 1, "{line}");
            let mut chars = body.chars();
            match (chars.next().unwrap(), chars.next(), chars.next()) {
                // `'\''` is the apostrophe itself, an alias of `’` (both 0xB4).
                ('\\', Some('\''), None) => entries.push(Entry::Char('\'', bytes[0])),
                ('\\', Some(e), None) => entries.push(Entry::Escape(e, bytes[0])),
                (c, None, None) => entries.push(Entry::Char(c, bytes[0])),
                _ => panic!("bad char literal: {line}"),
            }
        } else {
            entries.push(Entry::Constant(lhs.to_string(), bytes));
        }
    }
    entries
}

/// Guards the generated `text/charmap.rs` against drift from the decomp's `charmap.txt`.
#[test]
fn generated_charmap_matches_decomp_charmap_txt() {
    let src = std::fs::read_to_string(fixtures::decomp().join("charmap.txt")).unwrap();
    let (mut chars, mut escapes, mut constants) = (Vec::new(), Vec::new(), Vec::new());
    for entry in parse_charmap(&src) {
        match entry {
            Entry::Char(c, b) => chars.push((c, b)),
            Entry::Escape(c, b) => escapes.push((c, b)),
            Entry::Constant(n, b) => constants.push((n, b)),
        }
    }
    assert!(chars.len() > 200 && constants.len() > 700);
    assert_eq!(text::charmap::CHARS, chars.as_slice());
    assert_eq!(text::charmap::ESCAPES, escapes.as_slice());
    let ours: Vec<(String, Vec<u8>)> = text::charmap::CONSTANTS
        .iter()
        .map(|&(n, b)| (n.to_string(), b.to_vec()))
        .collect();
    assert_eq!(ours, constants);
}
