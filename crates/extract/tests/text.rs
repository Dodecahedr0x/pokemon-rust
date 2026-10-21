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
        // The hit and the strings packed after it decode and re-encode to the same bytes.
        let mut at = off as usize;
        for _ in 0..8 {
            let (decoded, used) = text::decode(&rom.bytes()[at..]).unwrap();
            assert_eq!(text::encode(&decoded).unwrap(), rom.bytes()[at..at + used]);
            eprintln!("0x{at:x}: {decoded:?}");
            at += used;
        }
    }
}
