//! Shared helpers of the `silice_*` tests: run `xtask silice` on the synthetic fixtures.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/silice")
}

pub fn src() -> PathBuf {
    fixtures().join("src")
}

pub fn extracted() -> PathBuf {
    fixtures().join("extracted")
}

pub struct Run {
    pub ok: bool,
    pub out: String,
}

/// Runs `xtask silice <args>` without `PKR_DECOMP`, the extract given by `--extracted` only.
pub fn xtask(args: &[&str]) -> Run {
    let o = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("silice")
        .args(args)
        .env_remove("PKR_DECOMP")
        .env("PKR_EXTRACTED_DIR", "/nonexistent")
        .output()
        .expect("running xtask");
    Run {
        ok: o.status.success(),
        out: format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    }
}

/// Imports `src` into `out` (report next to it).
pub fn import(src: &Path, out: &Path, force: bool) -> Run {
    let report = out.with_extension("report.md");
    let mut args = vec![
        "import",
        src.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
    ];
    if force {
        args.push("--force");
    }
    xtask(&args)
}

/// A fresh import of the fixtures in a temporary directory; the games dir is `<tmp>/games`.
pub fn imported() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let games = tmp.path().join("games");
    let r = import(&src(), &games, false);
    assert!(r.ok, "{}", r.out);
    (tmp, games)
}

pub fn check(games: &Path, extra: &[&str]) -> Run {
    let ex = extracted();
    let mut args = vec![
        "check",
        "--games",
        games.to_str().unwrap(),
        "--extracted",
        ex.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    xtask(&args)
}

pub fn read(games: &Path, rel: &str) -> String {
    std::fs::read_to_string(games.join(rel)).unwrap()
}

/// Replaces the first `from` in `games/rel` by `to`; panics if `from` is absent.
pub fn plant(games: &Path, rel: &str, from: &str, to: &str) {
    let s = read(games, rel);
    assert!(s.contains(from), "{rel} has no {from:?}");
    std::fs::write(games.join(rel), s.replacen(from, to, 1)).unwrap();
}

/// Copies a directory tree.
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// Every file under `dir`, relative path -> bytes.
pub fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, d: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().to_string();
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}
