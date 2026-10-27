//! The repo-root `.env` (image models and anything else the studio or a child `xtask` reads).
//!
//! Loaded once at startup. A variable already set in the environment is left alone, so a shell
//! export wins over the file. Blank values are skipped. A missing file is fine.

use std::path::{Path, PathBuf};

/// `KEY=VALUE` assignments. `#` comments and blank values are dropped. `export ` is optional.
/// A matching pair of quotes around a value is removed.
pub fn parse(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let value = unquote(value.trim());
        if value.is_empty() {
            continue;
        }
        out.push((key.to_string(), value));
    }
    out
}

fn unquote(value: &str) -> String {
    let b = value.as_bytes();
    if b.len() >= 2 {
        let q = b[0];
        if (q == b'"' || q == b'\'') && b[b.len() - 1] == q {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

/// Sets each pair that is not already in the environment. Call once, at process start, before
/// any other thread exists: [`std::env::set_var`] races with `getenv` on other threads.
pub fn apply(pairs: &[(String, String)]) {
    for (key, value) in pairs {
        if std::env::var_os(key).is_none() {
            // SAFETY: process start, before any other thread.
            unsafe { std::env::set_var(key, value) };
        }
    }
}

/// Reads `path`. `NotFound` is ignored; any other error is printed and skipped.
pub fn load_path(path: &Path) {
    match std::fs::read_to_string(path) {
        Ok(text) => apply(&parse(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => eprintln!("studio: could not read {}: {e}", path.display()),
    }
}

/// `<workspace>/.env`, from this crate's manifest (`crates/studio` is two levels down).
pub fn workspace_env() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/studio is two levels below the workspace")
        .join(".env")
}

/// Loads [`workspace_env`].
pub fn load() {
    load_path(&workspace_env());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_keeps_assignments_and_drops_blanks_and_comments() {
        let pairs = parse(
            "
            # comment
            export ARK_MODEL=\"doubao-seedream-4-5-251128\"
            ARK_API_KEY=
            GEMINI_API_KEY='key'
            ARK_BASE_URL=https://ark.example/api/v3
            not a pair
            ",
        );
        assert_eq!(
            pairs,
            vec![
                ("ARK_MODEL".into(), "doubao-seedream-4-5-251128".into()),
                ("GEMINI_API_KEY".into(), "key".into()),
                ("ARK_BASE_URL".into(), "https://ark.example/api/v3".into()),
            ]
        );
    }

    #[test]
    fn apply_does_not_override_the_environment() {
        let _guard = ENV_LOCK.lock().unwrap();
        let kept = "PKR_DOTENV_KEPT";
        let fresh = "PKR_DOTENV_FRESH";
        unsafe {
            std::env::set_var(kept, "shell");
            std::env::remove_var(fresh);
        }
        apply(&[
            (kept.into(), "file".into()),
            (fresh.into(), "from-file".into()),
        ]);
        assert_eq!(std::env::var(kept).unwrap(), "shell");
        assert_eq!(std::env::var(fresh).unwrap(), "from-file");
        unsafe {
            std::env::remove_var(kept);
            std::env::remove_var(fresh);
        }
    }

    #[test]
    fn a_missing_file_is_left_alone() {
        load_path(Path::new("/nonexistent/pkr-studio.env"));
    }
}
