//! Build identity hashing, shared by `engine/build.rs` (the engine sources hash), the content
//! build script (`content::BUILD_ID`) and their tests (through `#[path]`).
//!
//! Deterministic across machines and checkouts: files are visited in the byte order of their
//! `/`-joined path relative to the hashed root, and each contributes its relative path, length
//! and bytes. Absolute paths and mtimes never enter. Rust sources are hashed with CRLF folded to
//! LF, so a Windows checkout (`core.autocrlf`) hashes like a Unix one.

use std::path::{Path, PathBuf};

/// FNV-1a 64-bit (the same function as `engine::hash::Fnv64`).
#[derive(Clone, Debug)]
pub struct TreeHash(u64);

impl Default for TreeHash {
    fn default() -> Self {
        Self::new()
    }
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

    pub const fn finish(&self) -> u64 {
        self.0
    }

    /// Every file under `root` that `keep` accepts (given its relative `/`-path), in path order.
    /// `prefix` is prepended to each relative path, so trees hashed into one `TreeHash` stay
    /// distinct. `text` folds CRLF to LF.
    pub fn tree(
        &mut self,
        prefix: &str,
        root: &Path,
        keep: &dyn Fn(&str) -> bool,
        text: bool,
    ) -> std::io::Result<()> {
        for (rel, path) in files(root, keep)? {
            let bytes = std::fs::read(&path)?;
            let bytes = if text { crlf_to_lf(&bytes) } else { bytes };
            self.file(&format!("{prefix}{rel}"), &bytes);
        }
        Ok(())
    }
}

/// The files under `root` that `keep` accepts, as (relative `/`-path, path), sorted by the
/// relative path's bytes. Symlinked directories are not followed.
pub fn files(root: &Path, keep: &dyn Fn(&str) -> bool) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    let mut dirs = vec![(String::new(), root.to_path_buf())];
    while let Some((rel_dir, dir)) = dirs.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| {
                std::io::Error::other(format!("non-UTF-8 file name in {}", dir.display()))
            })?;
            let rel = if rel_dir.is_empty() {
                name.to_owned()
            } else {
                format!("{rel_dir}/{name}")
            };
            let ty = entry.file_type()?;
            if ty.is_dir() {
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

/// The engine half of `checkpoint::BuildHash`: every non-test `.rs` under `src`.
pub fn engine_src_hash(src: &Path) -> std::io::Result<u64> {
    let mut h = TreeHash::new();
    h.tree("", src, &is_build_source, true)?;
    Ok(h.finish())
}

/// Writes `path` only when its bytes change (keeps the mtime, so dependents do not rebuild).
pub fn write_if_changed(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).ok().as_deref() != Some(data) {
        std::fs::write(path, data)?;
    }
    Ok(())
}
