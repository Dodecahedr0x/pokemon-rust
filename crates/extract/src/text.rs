//! Gen 3 Western text codec. The text form is the decomp's `.string` syntax: `\n`, `\l`, `\p`
//! escapes and `{NAME args}` constants from `charmap.txt`, parsed the way `tools/preproc` does.
//! The 0xFF terminator is implicit: `decode` stops after it and `encode` appends it.
//!
//! The codec is language-aware ([`Lang`]): the decomp charmap is the English one, and a language
//! overlay remaps the bytes whose glyphs a localized ROM redraws as other characters. BPEF draws
//! 0xB1/0xB2 (`“`/`”` in English) as `«`/`»`. Its other redrawn glyphs (0x34 `{LV}` drawn `N.`,
//! the unused 0x57-0x59, 0x64, 0x105, 0x108: abbreviation ligatures, not characters) keep
//! their English spelling.

/// The generated charmap tables; public only so the drift test can compare them to the decomp.
#[doc(hidden)]
pub mod charmap;

use crate::{GameCode, Rom};
use anyhow::{Context, Result, bail};
use charmap::{CHARS, COLORS, CONSTANTS, DECODE_CHARS, ESCAPES, SOUNDS, SYMBOLS};
use std::fmt::Write;

const EOS: u8 = 0xFF;
const EXT_CTRL: u8 = 0xFC;
const PLACEHOLDER: u8 = 0xFD;
const DYNAMIC: u8 = 0xF7;
const KEYPAD_ICON: u8 = 0xF8;
const EXTRA_SYMBOL: u8 = 0xF9;

/// Which charmap a ROM's text uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    /// English (BPEE): the decomp's `charmap.txt`.
    En,
    /// French (BPEF): the English charmap with [`FR_OVERLAY`].
    Fr,
}

impl Lang {
    pub fn of(code: GameCode) -> Lang {
        match code {
            GameCode::Bpee => Lang::En,
            GameCode::Bpef => Lang::Fr,
        }
    }

    /// `"en"` or `"fr"`: `manifest.language` and the `text/<lang>.ron` file name.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
        }
    }

    /// Bytes this language draws as a different character than English.
    pub fn overlay(self) -> &'static [(u8, char)] {
        match self {
            Lang::En => &[],
            Lang::Fr => FR_OVERLAY,
        }
    }
}

/// BPEF redraws the English curly double quotes as guillemets.
pub const FR_OVERLAY: &[(u8, char)] = &[(0xB1, '«'), (0xB2, '»')];

/// How an `EXT_CTRL_CODE` (0xFC xx) argument byte group is shown.
#[derive(Clone, Copy)]
enum Arg {
    Color,
    Sound,
    Number,
}

/// Name and arguments of each extended control code, as in `GetExtCtrlCodeLength` (`string_util.c`).
const EXT_CODES: [(&str, &[Arg]); 0x19] = {
    use Arg::*;
    [
        ("NAME_END", &[]),
        ("COLOR", &[Color]),
        ("HIGHLIGHT", &[Color]),
        ("SHADOW", &[Color]),
        ("COLOR_HIGHLIGHT_SHADOW", &[Color, Color, Color]),
        ("PALETTE", &[Number]),
        ("FONT", &[Number]),
        ("RESET_FONT", &[]),
        ("PAUSE", &[Number]),
        ("PAUSE_UNTIL_PRESS", &[]),
        ("WAIT_SE", &[]),
        ("PLAY_BGM", &[Sound]),
        ("ESCAPE", &[Number]),
        ("SHIFT_RIGHT", &[Number]),
        ("SHIFT_DOWN", &[Number]),
        ("FILL_WINDOW", &[]),
        ("PLAY_SE", &[Sound]),
        ("CLEAR", &[Number]),
        ("SKIP", &[Number]),
        ("CLEAR_TO", &[Number]),
        ("MIN_LETTER_SPACING", &[Number]),
        ("JPN", &[]),
        ("ENG", &[]),
        ("PAUSE_MUSIC", &[]),
        ("RESUME_MUSIC", &[]),
    ]
};

/// [`decode_lang`] with the English charmap.
pub fn decode(bytes: &[u8]) -> Result<(String, usize)> {
    decode_lang(bytes, Lang::En)
}

/// [`encode_lang`] with the English charmap.
pub fn encode(s: &str) -> Result<Vec<u8>> {
    encode_lang(s, Lang::En)
}

/// Decodes one string up to and including its 0xFF terminator. Returns the text and the number
/// of bytes consumed (terminator included).
///
/// The one-byte-argument prefixes 0xFD (placeholder), 0xF7 (`DYNAMIC`), 0xF8 (keypad icon) and
/// 0xF9 (extra symbol), and the arguments of 0xFC control codes, always consume their argument
/// bytes even when one is 0xFF, as the game's text printer does; such a 0xFF does not end the string.
///
/// Battle placeholders share 0xFD codes with field placeholders (`B_BUFF2` and `PLAYER` are both
/// FD 01, for instance). Decoding always uses the field names (FD 01 → `{PLAYER}`), so extracting
/// battle strings must remap FD 00–0D to their `B_*` names. FD 0E and above only have battle names.
pub fn decode_lang(bytes: &[u8], lang: Lang) -> Result<(String, usize)> {
    let overlay = lang.overlay();
    let mut out = String::new();
    let mut i = 0;
    let byte = |j: usize| {
        bytes
            .get(j)
            .copied()
            .with_context(|| format!("unterminated string: no 0xFF within {} bytes", bytes.len()))
    };
    loop {
        let b = byte(i)?;
        if b == EOS {
            return Ok((out, i + 1));
        }
        if let Some(&(c, _)) = ESCAPES.iter().find(|&&(_, e)| e == b) {
            out.push('\\');
            out.push(c);
            i += 1;
            continue;
        }
        if let Some(&(_, c)) = overlay.iter().find(|&&(o, _)| o == b) {
            out.push(c);
            i += 1;
            continue;
        }
        if let Some((seq, name)) = SYMBOLS.iter().find(|(seq, _)| bytes[i..].starts_with(seq)) {
            write!(out, "{{{name}}}").unwrap();
            i += seq.len();
            continue;
        }
        if let Some(c) = DECODE_CHARS[b as usize] {
            out.push(c);
            i += 1;
            continue;
        }
        match b {
            EXT_CTRL if (byte(i + 1)? as usize) < EXT_CODES.len() => {
                let (name, args) = EXT_CODES[byte(i + 1)? as usize];
                out.push('{');
                out.push_str(name);
                i += 2;
                for arg in args {
                    out.push(' ');
                    match arg {
                        Arg::Color => {
                            let v = byte(i)?;
                            match COLORS.get(v as usize) {
                                Some(c) => out.push_str(c),
                                None => write!(out, "{v}").unwrap(),
                            }
                            i += 1;
                        }
                        Arg::Number => {
                            write!(out, "{}", byte(i)?).unwrap();
                            i += 1;
                        }
                        Arg::Sound => {
                            let v = u16::from_le_bytes([byte(i)?, byte(i + 1)?]);
                            match SOUNDS.binary_search_by_key(&v, |&(id, _)| id) {
                                Ok(k) => out.push_str(SOUNDS[k].1),
                                Err(_) => write!(out, "0x{v:04X}").unwrap(),
                            }
                            i += 2;
                        }
                    }
                }
                out.push('}');
            }
            PLACEHOLDER | DYNAMIC => {
                let name = if b == PLACEHOLDER {
                    "STRING"
                } else {
                    "DYNAMIC"
                };
                write!(out, "{{{name} {}}}", byte(i + 1)?).unwrap();
                i += 2;
            }
            KEYPAD_ICON | EXTRA_SYMBOL => {
                write!(out, "{{0x{b:02X} 0x{:02X}}}", byte(i + 1)?).unwrap();
                i += 2;
            }
            _ => {
                write!(out, "{{0x{b:02X}}}").unwrap();
                i += 1;
            }
        }
    }
}

/// Encodes `.string` text (without the `$` terminator) and appends 0xFF. Characters whose byte
/// `lang` redraws (English `“`/`”` in French) are rejected.
pub fn encode_lang(s: &str, lang: Lang) -> Result<Vec<u8>> {
    let mut p = Parser {
        overlay: lang.overlay(),
        chars: s.chars().collect(),
        pos: 0,
        out: Vec::new(),
    };
    p.run().with_context(|| format!("encoding {s:?}"))?;
    p.out.push(EOS);
    Ok(p.out)
}

struct Parser {
    overlay: &'static [(u8, char)],
    chars: Vec<char>,
    pos: usize,
    out: Vec<u8>,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn run(&mut self) -> Result<()> {
        while let Some(c) = self.peek() {
            match c {
                '{' => self.bracketed()?,
                '\\' => {
                    self.pos += 1;
                    let Some(e) = self.peek() else {
                        bail!("trailing backslash")
                    };
                    let Some(&(_, b)) = ESCAPES.iter().find(|&&(c, _)| c == e) else {
                        bail!("unknown escape '\\{e}' at char {}", self.pos - 1)
                    };
                    self.out.push(b);
                    self.pos += 1;
                }
                '$' => bail!(
                    "'$' at char {}: the terminator is appended by encode",
                    self.pos
                ),
                c if c.is_ascii() && !(' '..='~').contains(&c) => {
                    bail!("unexpected character U+{:X} at char {}", c as u32, self.pos)
                }
                c => {
                    let b = match self.overlay.iter().find(|&&(_, o)| o == c) {
                        Some(&(b, _)) => b,
                        None => match CHARS.iter().find(|&&(m, _)| m == c) {
                            Some(&(_, b)) if self.overlay.iter().all(|&(o, _)| o != b) => b,
                            _ => bail!("unknown character {c:?} at char {}", self.pos),
                        },
                    };
                    self.out.push(b);
                    self.pos += 1;
                }
            }
        }
        Ok(())
    }

    /// `{FOO 12 0x0C BAR}`, mirroring preproc's `ReadBracketedConstants`.
    fn bracketed(&mut self) -> Result<()> {
        let open = self.pos;
        self.pos += 1;
        loop {
            match self.peek() {
                Some('}') => break,
                None => bail!("unclosed '{{' at char {open}"),
                _ => {}
            }
            while matches!(self.peek(), Some(' ' | '\t')) {
                self.pos += 1;
            }
            match self.peek() {
                Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                    let start = self.pos;
                    while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
                        self.pos += 1;
                    }
                    let name: String = self.chars[start..self.pos].iter().collect();
                    let Some(&(_, bytes)) = CONSTANTS.iter().find(|&&(n, _)| n == name) else {
                        bail!("unknown constant '{name}' at char {start}")
                    };
                    self.out.extend_from_slice(bytes);
                }
                Some(c) if c.is_ascii_digit() => self.integer()?,
                Some(c) => bail!(
                    "unexpected character {c:?} within curly brackets at char {}",
                    self.pos
                ),
                None => bail!("unclosed '{{' at char {open}"),
            }
        }
        self.pos += 1;
        Ok(())
    }

    /// Decimal (`15`, `300H`, `7W`) or hex (`0x0F`, `0x000F`, `0x0000000F`) little-endian integer.
    fn integer(&mut self) -> Result<()> {
        let start = self.pos;
        let hex = self.peek() == Some('0') && self.chars.get(self.pos + 1) == Some(&'x');
        if hex {
            self.pos += 2;
        }
        let radix = if hex { 16 } else { 10 };
        let digits_start = self.pos;
        let mut n: u64 = 0;
        while let Some(d) = self.peek().and_then(|c| c.to_digit(radix)) {
            n = n * radix as u64 + d as u64;
            if n >= u32::MAX as u64 {
                bail!("integer literal at char {start} is too large");
            }
            self.pos += 1;
        }
        let size = if hex {
            match self.pos - digits_start {
                2 => 1,
                4 => 2,
                8 => 4,
                _ => bail!("hex literal at char {start} doesn't have 2, 4 or 8 digits"),
            }
        } else {
            match self.peek() {
                Some('H') => {
                    self.pos += 1;
                    if n >= 0x10000 {
                        bail!("{n} is too large to be a halfword");
                    }
                    2
                }
                Some('W') => {
                    self.pos += 1;
                    4
                }
                _ if n >= 0x10000 => 4,
                _ if n >= 0x100 => 2,
                _ => 1,
            }
        };
        self.out
            .extend_from_slice(&(n as u32).to_le_bytes()[..size]);
        Ok(())
    }
}

impl Rom {
    pub fn lang(&self) -> Lang {
        Lang::of(self.code())
    }

    /// Decodes the 0xFF-terminated string at file offset `off` with the ROM's charmap.
    pub fn string_at(&self, off: u32) -> Result<String> {
        Ok(self.string_and_len_at(off)?.0)
    }

    fn string_and_len_at(&self, off: u32) -> Result<(String, usize)> {
        let bytes = self
            .bytes()
            .get(off as usize..)
            .with_context(|| format!("string offset 0x{off:x} is outside the ROM"))?;
        decode_lang(bytes, self.lang()).with_context(|| format!("string at 0x{off:x}"))
    }
}

/// The length in bytes (terminator included) of the string at file offset `off`.
pub fn string_len_at(rom: &Rom, off: u32) -> Result<usize> {
    Ok(rom.string_and_len_at(off)?.1)
}

/// Decodes the string at `off` with the ROM's charmap and checks that re-encoding the text gives
/// exactly the ROM bytes (terminator included). Returns the text.
pub fn round_trip_at(rom: &Rom, off: u32) -> Result<String> {
    let (s, len) = rom.string_and_len_at(off)?;
    let bytes = encode_lang(&s, rom.lang()).with_context(|| format!("string at 0x{off:x}"))?;
    let want = rom.slice(off, len)?;
    if bytes != want {
        bail!("string at 0x{off:x} ({s:?}) re-encodes to {bytes:02x?}, ROM has {want:02x?}");
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(s: &str) -> Vec<u8> {
        let bytes = encode(s).unwrap();
        assert_eq!(*bytes.last().unwrap(), 0xFF, "{s}");
        let (back, used) = decode(&bytes).unwrap();
        assert_eq!(back, s);
        assert_eq!(used, bytes.len());
        bytes
    }

    #[test]
    fn ascii() {
        assert_eq!(
            round_trip("Az09!?"),
            [0xBB, 0xEE, 0xA1, 0xAA, 0xAB, 0xAC, 0xFF]
        );
        round_trip("MOM: Wait, don't go. (100%) +=-;/&<>");
        assert_eq!(round_trip(""), [0xFF]);
    }

    #[test]
    fn french_and_symbols() {
        assert_eq!(
            round_trip("é è ê à ç ô É"),
            [
                0x1B, 0, 0x1A, 0, 0x1C, 0, 0x16, 0, 0x19, 0, 0x24, 0, 0x06, 0xFF
            ]
        );
        round_trip("POKéMON… ♂ ♀ “Où?” ‘Œuf' ÀÇÈÊËÎÏÔÙÛ àâçèéêëîïôùûœ");
        assert_eq!(round_trip("…♂♀"), [0xB0, 0xB5, 0xB6, 0xFF]);
    }

    #[test]
    fn apostrophe_variants_share_a_byte() {
        assert_eq!(encode("’").unwrap(), encode("'").unwrap());
        assert_eq!(decode(&[0xB4, 0xFF]).unwrap().0, "'");
    }

    #[test]
    fn escapes() {
        assert_eq!(
            round_trip("a\\nb\\lc\\pd"),
            [0xD5, 0xFE, 0xD6, 0xFA, 0xD7, 0xFB, 0xD8, 0xFF]
        );
    }

    #[test]
    fn placeholders_and_control_codes() {
        assert_eq!(round_trip("{PLAYER}"), [0xFD, 0x01, 0xFF]);
        assert_eq!(
            round_trip("{STR_VAR_1}{RIVAL}{KUN}"),
            [0xFD, 2, 0xFD, 6, 0xFD, 5, 0xFF]
        );
        assert_eq!(round_trip("{COLOR RED}"), [0xFC, 0x01, 0x04, 0xFF]);
        assert_eq!(round_trip("{PAUSE 15}"), [0xFC, 0x08, 0x0F, 0xFF]);
        assert_eq!(
            round_trip("{COLOR_HIGHLIGHT_SHADOW DARK_GRAY WHITE LIGHT_GRAY}"),
            [0xFC, 0x04, 2, 1, 3, 0xFF]
        );
        assert_eq!(
            round_trip("{PLAY_BGM MUS_CAUGHT}"),
            [0xFC, 0x0B, 0x60, 0x01, 0xFF]
        );
        assert_eq!(
            round_trip("{PLAY_SE SE_FLEE}"),
            [0xFC, 0x10, 0x11, 0x00, 0xFF]
        );
        assert_eq!(
            round_trip("{PLAY_SE 0x1234}"),
            [0xFC, 0x10, 0x34, 0x12, 0xFF]
        );
        assert_eq!(
            round_trip("{FONT_SMALL}{FONT 3}"),
            [0xFC, 6, 0, 0xFC, 6, 3, 0xFF]
        );
        assert_eq!(
            round_trip("{TALL_PLUS}{ESCAPE 4}"),
            [0xFC, 0x0C, 0xFB, 0xFC, 0x0C, 4, 0xFF]
        );
        round_trip("{NAME_END}{RESET_FONT}{PAUSE_UNTIL_PRESS}{WAIT_SE}{FILL_WINDOW}");
        round_trip("{HIGHLIGHT TRANSPARENT}{SHADOW LIGHT_GREEN}{SHADOW 161}{PALETTE 5}");
        round_trip("{SHIFT_RIGHT 3}{SHIFT_DOWN 4}{CLEAR 2}{SKIP 1}{CLEAR_TO 89}");
        round_trip("{MIN_LETTER_SPACING 0}{JPN}{ENG}{PAUSE_MUSIC}{RESUME_MUSIC}");
        assert_eq!(round_trip("{DYNAMIC 0}"), [0xF7, 0, 0xFF]);
        assert_eq!(round_trip("{STRING 99}"), [0xFD, 99, 0xFF]);
        round_trip("{B_BUFF1}{B_ATK_NAME_WITH_PREFIX}");
        round_trip("{A_BUTTON}{DPAD_UPDOWN}{PLUS}{EMOJI_HEART}{0xF9 0x80}");
        round_trip("{PKMN}{PK}{POKEBLOCK}{LV}{UP_ARROW}{SUPER_ER}");
        assert_eq!(round_trip("{0x0A}{0x37}"), [0x0A, 0x37, 0xFF]);
    }

    #[test]
    fn non_canonical_forms_encode_like_preproc() {
        assert_eq!(
            encode("{PAUSE 0x0F}").unwrap(),
            encode("{PAUSE 15}").unwrap()
        );
        assert_eq!(encode("{ PLAYER}").unwrap(), [0xFD, 1, 0xFF]);
        assert_eq!(encode("{PAUSE\t15}").unwrap(), [0xFC, 8, 15, 0xFF]);
        assert_eq!(encode("{300}").unwrap(), [0x2C, 0x01, 0xFF]);
        assert_eq!(encode("{5H}").unwrap(), [5, 0, 0xFF]);
        assert_eq!(encode("{5W}").unwrap(), [5, 0, 0, 0, 0xFF]);
        assert_eq!(
            encode("{PLAY_BGM}{MUS_CAUGHT}").unwrap(),
            [0xFC, 0x0B, 0x60, 0x01, 0xFF]
        );
    }

    #[test]
    fn encode_errors() {
        for bad in [
            "{FOO}",
            "{PLAYER",
            "{PLAYER }",
            "{0x123}",
            "\\q",
            "\\\"",
            "\\",
            "a$",
            "\t",
            "€",
        ] {
            assert!(encode(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn french_overlay_maps_guillemets() {
        let bytes = [0xB1, 0x00, 0xBB, 0x00, 0xB2, 0xFF];
        assert_eq!(decode_lang(&bytes, Lang::Fr).unwrap().0, "« A »");
        assert_eq!(decode_lang(&bytes, Lang::En).unwrap().0, "“ A ”");
        assert_eq!(encode_lang("« A »", Lang::Fr).unwrap(), bytes);
        assert_eq!(encode_lang("“ A ”", Lang::En).unwrap(), bytes);
        assert!(encode_lang("“", Lang::Fr).is_err());
        assert!(encode_lang("«", Lang::En).is_err());
        // Everything else is shared.
        assert_eq!(
            encode_lang("Héhé {PLAYER}\\n{LV}", Lang::Fr).unwrap(),
            encode("Héhé {PLAYER}\\n{LV}").unwrap()
        );
        assert_eq!(Lang::of(GameCode::Bpef).code(), "fr");
        assert_eq!(Lang::of(GameCode::Bpee), Lang::En);
    }

    #[test]
    fn round_trip_at_checks_rom_bytes() {
        let mut b = vec![0u8; 0xC0];
        b[0xAC..0xB0].copy_from_slice(b"BPEF");
        // "«A»", then "'" whose terminator is the last ROM byte.
        b.extend([0xB1, 0xBB, 0xB2, 0xFF, 0xB4, 0xFF, 0xB4]);
        let rom = Rom::from_bytes(b).unwrap();
        assert_eq!(round_trip_at(&rom, 0xC0).unwrap(), "«A»");
        assert_eq!(rom.string_at(0xC4).unwrap(), "'");
        assert_eq!(round_trip_at(&rom, 0xC4).unwrap(), "'");
        assert!(round_trip_at(&rom, 0xC6).is_err(), "unterminated");
    }

    #[test]
    fn decode_stops_at_terminator() {
        assert_eq!(decode(&[0xBB, 0xFF, 0xBC, 0xFF]).unwrap(), ("A".into(), 2));
        assert!(decode(&[0xBB]).is_err());
        assert!(decode(&[0xFC, 0x08]).is_err());
        assert!(decode(&[]).is_err());
    }
}
