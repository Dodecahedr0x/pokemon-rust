//! The repo-root `.env`. Same rules as `studio`'s loader: a shell export wins, blank values
//! are skipped, and a missing file is fine. `cargo xtask` loads it itself so a terminal run
//! sees the same keys the studio passes to a child.

use std::path::{Path, PathBuf};

pub fn load() {
    load_path(&workspace_env());
}

fn workspace_env() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is one level below the workspace")
        .join(".env")
}

fn load_path(path: &Path) {
    match std::fs::read_to_string(path) {
        Ok(text) => apply(&parse(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => eprintln!("xtask: could not read {}: {e}", path.display()),
    }
}

fn parse(text: &str) -> Vec<(String, String)> {
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

/// Call once, at process start, before any other thread exists.
fn apply(pairs: &[(String, String)]) {
    for (key, value) in pairs {
        if std::env::var_os(key).is_none() {
            // SAFETY: process start, before any other thread.
            unsafe { std::env::set_var(key, value) };
        }
    }
}
