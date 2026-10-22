//! Where the studio reads and writes (plan: "paths.rs").

use game_native::store::Store;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudioPaths {
    pub workspace: PathBuf,
    /// `target/game.gba`.
    pub rom: PathBuf,
    /// `target/studio/`: the ROM pane's exported saves (`rom-<unix ms>.sav`) and a smoke run's
    /// `smoke-sample.txt`.
    pub scratch: PathBuf,
    /// The committed tree: `Store::default_root(content::STUB)`.
    pub shared: PathBuf,
    /// The personal, git-ignored store: `<shared>/local`.
    pub local: PathBuf,
}

impl StudioPaths {
    pub fn detect() -> StudioPaths {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crates/studio is two levels below the workspace")
            .to_path_buf();
        let target = workspace.join("target");
        let shared = Store::default_root(content::STUB);
        let local = Store::local_root(content::STUB);
        StudioPaths {
            rom: target.join("game.gba"),
            scratch: target.join("studio"),
            workspace,
            shared,
            local,
        }
    }
}

/// What may be committed from a write location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Committable {
    /// The committed tree's `nodes/` (inputs, hashes, digests, `MapId`s and labels): written by
    /// `cargo xtask checkpoints` only, never by the studio.
    NodesOnly,
    /// Git-ignored: may hold ROM-derived data (snapshots, statuses, renders, `.sav`).
    Never,
}

/// Every directory (or file) the studio may write to: the local store and the scratch dir, all
/// git-ignored. The committed tree is the layered store's read-only base.
pub fn write_locations(p: &StudioPaths) -> Vec<(PathBuf, Committable)> {
    ["nodes", "snapshots", "status.ron"]
        .map(|f| p.local.join(f))
        .into_iter()
        .chain([p.scratch.clone()])
        .map(|d| (d, Committable::Never))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// `Some(ignored)` from `git check-ignore`, `None` without git or outside a checkout. A
    /// directory that may not exist yet counts as ignored when a file inside it is.
    fn git_ignored(workspace: &Path, path: &Path) -> Option<bool> {
        Some(check_ignore(workspace, path)? || check_ignore(workspace, &path.join("probe"))?)
    }

    fn check_ignore(workspace: &Path, path: &Path) -> Option<bool> {
        let out = Command::new("git")
            .current_dir(workspace)
            .args(["check-ignore", "-q", "--no-index"])
            .arg(path)
            .output()
            .ok()?;
        match out.status.code() {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        }
    }

    #[test]
    fn paths_are_inside_the_workspace() {
        let p = StudioPaths::detect();
        assert!(p.workspace.join("Cargo.toml").is_file());
        assert!(p.workspace.join("crates/studio").is_dir());
        assert_eq!(p.rom, p.workspace.join("target/game.gba"));
        assert!(p.scratch.starts_with(p.workspace.join("target")));
        if std::env::var_os("PKR_CHECKPOINTS").is_none() {
            assert!(p.local.starts_with(&p.shared));
            let flavor = if content::STUB { "stub" } else { "emerald" };
            assert_eq!(p.shared, p.workspace.join("checkpoints").join(flavor));
        }
    }

    #[test]
    fn the_studio_writes_only_the_local_store_and_scratch() {
        let p = StudioPaths::detect();
        let w = write_locations(&p);
        assert!(w.iter().all(|(_, c)| *c == Committable::Never));
        let dirs: Vec<_> = w.into_iter().map(|(d, _)| d).collect();
        assert_eq!(
            dirs,
            [
                p.local.join("nodes"),
                p.local.join("snapshots"),
                p.local.join("status.ron"),
                p.scratch.clone(),
            ]
        );
    }

    #[test]
    fn never_committable_locations_are_git_ignored() {
        let p = StudioPaths::detect();
        if std::env::var_os("PKR_CHECKPOINTS").is_some() {
            return; // the stores are elsewhere
        }
        for (dir, c) in write_locations(&p) {
            if c == Committable::Never && !dir.starts_with(&p.local) {
                match git_ignored(&p.workspace, &dir) {
                    Some(ignored) => assert!(ignored, "{} is not git-ignored", dir.display()),
                    None => return, // no git here
                }
            }
        }
    }

    /// The whole local store is git-ignored (`checkpoints/*/local/`).
    #[test]
    fn the_local_store_is_git_ignored() {
        let p = StudioPaths::detect();
        for (dir, _) in write_locations(&p) {
            if dir.starts_with(&p.local)
                && let Some(ignored) = git_ignored(&p.workspace, &dir)
            {
                assert!(ignored, "{} is not git-ignored", dir.display());
            }
        }
    }
}
