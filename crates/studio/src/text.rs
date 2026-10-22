//! Gen3 text for display, with the build's charmap (`content::data::LANG`).
//!
//! See `docs/plans/2026-10-22-milestone-6-studio.md` (Task F).

use extract::text::{Lang, decode_lang};

/// The charmap of this content build: `content::data::LANG` (`"fr"` or `"en"`).
pub fn lang() -> Lang {
    match content::data::LANG {
        "en" => Lang::En,
        _ => Lang::Fr,
    }
}

/// Gen3 bytes -> display string with the build's charmap ([`lang`]), in the decomp's `.string`
/// syntax: escapes stay `\n`, `\l`, `\p` and placeholders stay `{PLAYER}`, `{STRING 3}`, …
///
/// Decoding stops at the first 0xFF; a missing terminator is tolerated. A byte the charmap does
/// not know is shown as `<?xx>` (upper-case hex). Never fails.
pub fn decode(bytes: &[u8]) -> String {
    let owned;
    let bytes = if bytes.contains(&0xFF) {
        bytes
    } else {
        owned = [bytes, &[0xFF]].concat();
        &owned
    };
    match decode_lang(bytes, lang()) {
        Ok((s, _)) => mark_unknown(&s),
        // Only an argument byte running past the end gets here: show the raw bytes.
        Err(_) => bytes
            .iter()
            .take_while(|&&b| b != 0xFF)
            .map(|b| format!("<?{b:02X}>"))
            .collect(),
    }
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
}
