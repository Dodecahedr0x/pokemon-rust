//! Build identity hashing, shared by `engine/build.rs` (the engine sources hash), the content
//! build script (`content::BUILD_ID`) and their tests (through `#[path]`).
//!
//! Deterministic across machines and checkouts: files are visited in the byte order of their
//! `/`-joined path relative to the hashed root, and each contributes its relative path, length
//! and bytes. Absolute paths and mtimes never enter. Rust sources are hashed with CRLF folded to
//! LF, so a Windows checkout (`core.autocrlf`) hashes like a Unix one. Hidden files and dirs
//! (`.DS_Store`, `.git`, editor swap files) and `*~` backups are never inputs; symlinks are
//! followed.

use std::path::{Path, PathBuf};

/// FNV-1a 64-bit (the same function as `engine::hash::Fnv64`).
#[derive(Clone, Debug)]
pub struct TreeHash(u64);

/// How a file's bytes enter the hash.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Content {
    /// Verbatim (data files).
    Raw,
    /// Text with CRLF folded to LF (sources).
    Text,
}

impl TreeHash {
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// One file: (path length, path, content length, content), so no two different file sets
    /// can produce the same byte stream.
    pub fn file(&mut self, rel: &str, bytes: &[u8]) {
        self.write(&(rel.len() as u64).to_le_bytes());
        self.write(rel.as_bytes());
        self.write(&(bytes.len() as u64).to_le_bytes());
        self.write(bytes);
    }

    /// The hash, never 0 (0 means "no build id" in `GameData::build_id`).
    pub const fn finish(&self) -> u64 {
        if self.0 == 0 { 1 } else { self.0 }
    }

    /// Every file under `root` that `keep` accepts (given its relative `/`-path), in path order.
    /// `prefix` is prepended to each relative path, so trees hashed into one `TreeHash` stay
    /// distinct.
    pub fn tree(
        &mut self,
        prefix: &str,
        root: &Path,
        keep: &dyn Fn(&str) -> bool,
        content: Content,
    ) -> std::io::Result<()> {
        for (rel, path) in files(root, keep)? {
            let bytes = std::fs::read(&path)?;
            let bytes = match content {
                Content::Raw => bytes,
                Content::Text => crlf_to_lf(&bytes),
            };
            self.file(&format!("{prefix}{rel}"), &bytes);
        }
        Ok(())
    }
}

/// Never a build input: hidden files and dirs, and `*~` backups.
pub fn is_ignored_name(name: &str) -> bool {
    name.starts_with('.') || name.ends_with('~')
}

/// The files under `root` that `keep` accepts, as (relative `/`-path, path), sorted by the
/// relative path's bytes. Symlinks (to files or dirs) are followed; ignored names
/// ([`is_ignored_name`]) are skipped with everything under them.
pub fn files(root: &Path, keep: &dyn Fn(&str) -> bool) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    let mut dirs = vec![(String::new(), root.to_path_buf())];
    // Canonical dirs already walked (never hashed): a dir reached twice, through a symlink
    // cycle or two links to one dir, is an error rather than a loop or a double count.
    let mut seen = std::collections::BTreeSet::new();
    while let Some((rel_dir, dir)) = dirs.pop() {
        if !seen.insert(std::fs::canonicalize(&dir)?) {
            return Err(std::io::Error::other(format!(
                "{} is reached twice (symlink cycle or duplicate link)",
                dir.display()
            )));
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| {
                std::io::Error::other(format!("non-UTF-8 file name in {}", dir.display()))
            })?;
            if is_ignored_name(name) {
                continue;
            }
            let rel = if rel_dir.is_empty() {
                name.to_owned()
            } else {
                format!("{rel_dir}/{name}")
            };
            // `metadata` follows symlinks (`DirEntry::file_type` does not).
            if std::fs::metadata(entry.path())?.is_dir() {
                dirs.push((rel, entry.path()));
            } else if keep(&rel) {
                out.push((rel, entry.path()));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn crlf_to_lf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (i, &b) in bytes.iter().enumerate() {
        if !(b == b'\r' && bytes.get(i + 1) == Some(&b'\n')) {
            out.push(b);
        }
    }
    out
}

/// A test-only source: `tests.rs`, `test_*.rs`, `*_tests.rs`, or anything in a `tests/` dir.
pub fn is_test_source(rel: &str) -> bool {
    let mut parts = rel.split('/');
    let name = parts.next_back().unwrap_or("");
    parts.any(|d| d == "tests")
        || name == "tests.rs"
        || name.starts_with("test_")
        || name.ends_with("_tests.rs")
}

/// A non-test `.rs` file: the sources that define a build.
pub fn is_build_source(rel: &str) -> bool {
    rel.ends_with(".rs") && !is_test_source(rel)
}

/// The engine half of `checkpoint::BuildHash`: every non-test `.rs` under `src`. Never 0.
pub fn engine_src_hash(src: &Path) -> std::io::Result<u64> {
    let mut h = TreeHash::new();
    h.tree("", src, &is_build_source, Content::Text)?;
    Ok(h.finish())
}

/// Writes `path` only when its bytes change (keeps the mtime, so dependents do not rebuild).
pub fn write_if_changed(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).ok().as_deref() != Some(data) {
        std::fs::write(path, data)?;
    }
    Ok(())
}
