//! Milestone 8 Task E: the overrides carry no ROM-derived data. Every file under
//! `src/overrides/` has no string or byte-string literal (texts and movements go by generated
//! id) and no numeric array literal longer than 8 elements. `mod.rs` may quote identifiers: the
//! `OVERRIDES` list of script names.

use std::path::Path;

/// Code without comments: `//` to the end of the line (no string in the code can hold `//`,
/// since strings are what we look for), and `/* … */` blocks.
fn code(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        rest = rest[i..].find("*/").map_or("", |j| &rest[i + j + 2..]);
    }
    out.push_str(rest);
    out.lines()
        .map(|l| l.split_once("//").map_or(l, |(c, _)| c))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What is wrong with an override file, if anything.
fn violations(name: &str, src: &str) -> Vec<String> {
    let code = code(src);
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            // A char literal (`'"'`) is not a string; a lifetime (`'static`) has no closing quote.
            b'\'' if b.get(i + 2) == Some(&b'\'') => i += 3,
            b'\'' if b.get(i + 1) == Some(&b'\\') => {
                i += code[i + 1..].find('\'').map_or(b.len(), |j| j + 2);
            }
            b'"' => {
                let end = code[i + 1..].find('"').map_or(b.len(), |j| i + 1 + j);
                let lit = &code[i + 1..end.min(code.len())];
                let ident =
                    !lit.is_empty() && lit.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if !(name == "mod.rs" && ident) {
                    out.push(format!("string literal {lit:?}"));
                }
                i = end + 1;
            }
            b'[' => {
                let end = code[i..].find(']').map_or(b.len(), |j| i + j);
                let items: Vec<&str> = code[i + 1..end.min(code.len())]
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                if items.len() > 8
                    && items
                        .iter()
                        .all(|s| s.starts_with(|c: char| c.is_ascii_digit() || c == '-'))
                {
                    out.push(format!("numeric array of {} elements", items.len()));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

#[test]
fn overrides_hold_no_rom_data() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/overrides");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for p in files {
        let name = p.file_name().unwrap().to_str().unwrap();
        let v = violations(name, &std::fs::read_to_string(&p).unwrap());
        assert!(v.is_empty(), "{}: {v:?}", p.display());
    }
}

#[test]
fn the_checker_finds_rom_data() {
    let text = "pub async fn A(ctx: Ctx) { msgbox(&ctx, \"BONJOUR\", MsgBox::Npc).await; }";
    assert_eq!(violations("A.rs", text).len(), 1);
    assert_eq!(violations("A.rs", "static B: &[u8] = b\"\\x01\";").len(), 1);
    assert_eq!(
        violations("A.rs", "static C: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];").len(),
        1
    );
    assert!(violations("A.rs", "static D: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];").is_empty());
    assert!(violations("A.rs", "// \"a comment\"\nlet c = '\"';").is_empty());
    assert!(violations("mod.rs", "pub static OVERRIDES: &[&str] = &[\"Foo_Bar\"];").is_empty());
    assert_eq!(
        violations("mod.rs", "static T: &str = \"Hi there\";").len(),
        1
    );
}
