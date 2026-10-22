//! Gen3 text for display, with the build's charmap (`content::data::LANG`).
//!
//! See `docs/plans/2026-10-22-milestone-6-studio.md` (Task F).

use extract::text::{Lang, decode_lang};

/// `a == b`, in a const context.
const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

const _: () = assert!(
    str_eq(content::data::LANG, "fr") || str_eq(content::data::LANG, "en"),
    "content::data::LANG is neither \"fr\" nor \"en\": teach studio::text the new charmap"
);

/// The charmap of a `manifest.language` code. Panics on a code other than `"fr"` or `"en"`.
pub fn lang_of(code: &str) -> Lang {
    match code {
        "fr" => Lang::Fr,
        "en" => Lang::En,
        other => unreachable!("unknown content language {other:?} (expected \"fr\" or \"en\")"),
    }
}

/// The charmap of this content build: `content::data::LANG` (checked at compile time).
pub fn lang() -> Lang {
    lang_of(content::data::LANG)
}

/// Gen3 bytes -> display string with the build's charmap ([`lang`]), in the decomp's `.string`
/// syntax: escapes stay `\n`, `\l`, `\p` and placeholders stay `{PLAYER}`, `{STRING 3}`, …
///
/// Decoding stops at the first 0xFF terminator; a missing one is tolerated. A byte the charmap
/// does not know is shown as `<?xx>` (upper-case hex), and so are the bytes of a code whose
/// arguments run past the end (after the decoded valid prefix). Never fails.
pub fn decode(bytes: &[u8]) -> String {
    decode_with(bytes, lang())
}

fn decode_with(bytes: &[u8], lang: Lang) -> String {
    // Always terminate: a 0xFF inside `bytes` may be an argument (`FD FF`), not the end.
    let terminated = |k: usize| [&bytes[..k], &[0xFF]].concat();
    if let Ok((s, _)) = decode_lang(&terminated(bytes.len()), lang) {
        return mark_unknown(&s);
    }
    // A code's arguments run past the end: decode the longest prefix that ends on a code
    // boundary (its appended 0xFF is read as the terminator) and mark the rest raw.
    let (k, s) = (0..bytes.len())
        .rev()
        .find_map(|k| match decode_lang(&terminated(k), lang) {
            Ok((s, used)) if used == k + 1 => Some((k, s)),
            _ => None,
        })
        .unwrap_or_default();
    let mut out = mark_unknown(&s);
    for b in &bytes[k..] {
        out.push_str(&format!("<?{b:02X}>"));
    }
    out
}

/// `decode_lang` writes an unknown single byte as `{0xNN}`; rewrite it as `<?NN>`. The
/// two-byte forms (`{0xF8 0xNN}` keypad icons, `{0xF9 0xNN}` extra symbols) are valid codes and
/// are kept.
fn mark_unknown(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("{0x") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let hex = tail
            .get(3..5)
            .filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
        match hex {
            Some(h) if tail.as_bytes().get(5) == Some(&b'}') => {
                out.push_str("<?");
                out.push_str(h);
                out.push('>');
                rest = &tail[6..];
            }
            _ => {
                out.push_str("{0x");
                rest = &tail[3..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_bytes_are_marked() {
        assert_eq!(mark_unknown("a{0x5A}b"), "a<?5A>b");
        assert_eq!(mark_unknown("{0xF8 0x01}"), "{0xF8 0x01}");
        assert_eq!(mark_unknown("{PLAYER}"), "{PLAYER}");
    }

    #[test]
    fn decode_is_total() {
        assert_eq!(decode(&[0xFF]), "");
        assert_eq!(decode(&[]), "");
        // A placeholder prefix whose argument is missing: raw bytes.
        assert_eq!(decode(&[0xFD]), "<?FD>");
        // FD 01 is the player name placeholder.
        assert_eq!(decode(&[0xFD, 0x01, 0xFF]), "{PLAYER}");
    }

    #[test]
    fn a_ff_argument_is_not_the_terminator() {
        // Placeholder 0xFF (`FD FF`), then a letter; the input has no terminator.
        let letter = decode(&[0xBB]);
        let s = decode(&[0xFD, 0xFF, 0xBB]);
        assert_eq!(s, format!("{{STRING 255}}{letter}"));
    }

    #[test]
    fn a_truncated_tail_keeps_the_valid_prefix() {
        // 0xBB 0xBC are letters; FC 0C (`ESCAPE`) needs one more argument byte.
        let ok = decode(&[0xBB, 0xBC]);
        assert!(ok.chars().count() == 2 && !ok.contains("<?"), "{ok}");
        assert_eq!(decode(&[0xBB, 0xBC, 0xFC, 0x0C]), format!("{ok}<?FC><?0C>"));
        assert_eq!(decode(&[0xBB, 0xBC, 0xFD]), format!("{ok}<?FD>"));
    }

    #[test]
    fn languages() {
        assert_eq!(lang_of("fr"), Lang::Fr);
        assert_eq!(lang_of("en"), Lang::En);
        assert_eq!(lang(), lang_of(content::data::LANG));
        // The charmaps differ only on the overlay bytes (0xB1 / 0xB2).
        assert_ne!(
            decode_with(&[0xB1], Lang::Fr),
            decode_with(&[0xB1], Lang::En)
        );
        assert_eq!(
            decode_with(&[0xBB], Lang::Fr),
            decode_with(&[0xBB], Lang::En)
        );
    }

    #[test]
    #[should_panic(expected = "unknown content language")]
    fn an_unknown_language_fails_clearly() {
        lang_of("de");
    }
}
