//! Where the studio reads and writes (plan: "paths.rs").

use game_native::store::Store;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudioPaths {
    pub workspace: PathBuf,
    /// `target/game.gba`.
    pub rom: PathBuf,
    /// `target/studio/` (settings.ron, exported .sav, logs).
    pub scratch: PathBuf,
    /// The committed tree: `Store::default_root(content::STUB)`.
    pub shared: PathBuf,
    /// The personal, git-ignored store: `<shared>/local`.
    pub local: PathBuf,
    /// The explorer's db (M7 Wave 2b): `explore::db::Db::default_path(content::STUB)`, i.e.
    /// `target/explore/{stub|emerald}/explore.db`. The studio only reads it; the `explore`
    /// process it starts writes it.
    pub explore_db: PathBuf,
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
            explore_db: explore::db::Db::default_path(content::STUB),
        }
    }
}

/// What may be committed from a write location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Committable {
    /// Store `nodes/`: inputs, hashes, digests, `MapId`s and labels only (never ROM data).
    /// The shared one is committed; the local one is git-ignored with the rest of its store.
    NodesOnly,
    /// Git-ignored: may hold ROM-derived data (snapshots, statuses, renders, `.sav`).
    Never,
}

/// Every directory (or file) the studio may write to. Only `local`/`shared` `nodes/` hold
/// committable files (inputs only); everything else is git-ignored.
pub fn write_locations(p: &StudioPaths) -> Vec<(PathBuf, Committable)> {
    let mut v = Vec::new();
    for store in [&p.shared, &p.local] {
        v.push((store.join("nodes"), Committable::NodesOnly));
        v.push((store.join("snapshots"), Committable::Never));
        v.push((store.join("status.ron"), Committable::Never));
    }
    v.push((p.scratch.clone(), Committable::Never));
    v
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
        assert!(p.local.starts_with(&p.shared));
        if std::env::var_os("PKR_CHECKPOINTS").is_none() {
            let flavor = if content::STUB { "stub" } else { "emerald" };
            assert_eq!(p.shared, p.workspace.join("checkpoints").join(flavor));
        }
    }

    #[test]
    fn the_explore_db_is_under_target_explore() {
        let p = StudioPaths::detect();
        let flavor = if content::STUB { "stub" } else { "emerald" };
        let want = p.workspace.join("target/explore").join(flavor);
        assert_eq!(p.explore_db, want.join("explore.db"));
    }

    #[test]
    fn only_nodes_dirs_are_committable() {
        let p = StudioPaths::detect();
        let w = write_locations(&p);
        let nodes: Vec<_> = w
            .iter()
            .filter(|(_, c)| *c == Committable::NodesOnly)
            .map(|(d, _)| d.clone())
            .collect();
        assert_eq!(nodes, [p.shared.join("nodes"), p.local.join("nodes")]);
        assert!(w.iter().any(|(d, _)| *d == p.scratch));
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
