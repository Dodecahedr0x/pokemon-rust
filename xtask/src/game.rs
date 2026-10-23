//! `--game <emerald|silice>` (SIL1 Task D): which game a command builds and runs, for every
//! command (`rom`, `parity`, `sweep`, `checkpoints`, `studio`, `explore`, …), anywhere on the
//! command line except after `studio map`, whose own `--game NAME` picks a games dir.
//!
//! The flag sets `PKR_GAME` for this process, so every child cargo invocation (the GBA build,
//! `cargo test -p emu`, the studio, the explorer) builds that flavour. xtask itself links
//! `content`, and its in-process commands (`checkpoints`, `sweep`, the budget and the store
//! roots) read `content::FLAVOR`: when the running xtask was built for another flavour, it
//! re-runs itself through `cargo run -p xtask` under the new `PKR_GAME` (once: [`REEXEC`]).
//! Without the flag the inherited `PKR_GAME` applies, as before.

use anyhow::{Context, Result, bail};
use content::Flavor;
use std::process::Command;

// `cfg(pkr_silice)` (`build.rs`, from `content`'s `links` metadata) is the content's flavour.
const _: () = assert!(cfg!(pkr_silice) == matches!(content::FLAVOR, Flavor::Silice));

/// Set on the re-run, so a build that still is not the asked flavour (no extract: the stub)
/// fails instead of re-running forever.
const REEXEC: &str = "PKR_XTASK_GAME";

/// A `--game` value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Game {
    Emerald,
    Silice,
}

impl Game {
    fn parse(s: &str) -> Result<Game> {
        match s {
            "emerald" => Ok(Game::Emerald),
            "silice" => Ok(Game::Silice),
            _ => bail!("--game {s:?}: expected `emerald` or `silice`"),
        }
    }

    /// The `PKR_GAME` value.
    pub fn name(self) -> &'static str {
        match self {
            Game::Emerald => "emerald",
            Game::Silice => "silice",
        }
    }

    /// Whether a content build of `flavor` is this game: `PKR_GAME=emerald` without an extract
    /// builds the stub, which counts as Emerald (as it always has).
    pub fn built_by(self, flavor: Flavor) -> bool {
        match self {
            Game::Emerald => matches!(flavor, Flavor::Stub | Flavor::Emerald),
            Game::Silice => flavor == Flavor::Silice,
        }
    }
}

/// The game the inherited `PKR_GAME` selects (unset, empty or `emerald`: Emerald), `None` for
/// another value (which the content build refuses).
fn inherited() -> Option<Game> {
    match std::env::var("PKR_GAME").as_deref() {
        Err(_) | Ok("") => Some(Game::Emerald),
        Ok(s) => Game::parse(s).ok(),
    }
}

/// The part of `args` where `--game` is xtask's: all of it, except the arguments of
/// `studio map`, whose own `--game NAME` picks a games dir (`games/NAME`); there, xtask's goes
/// before `studio`.
fn own(args: &[String]) -> &[String] {
    let end = args
        .windows(2)
        .position(|w| w[0] == "studio" && w[1] == "map")
        .unwrap_or(args.len());
    &args[..end]
}

/// Removes `--game <name>` from `args` and returns the game.
pub fn take(args: &mut Vec<String>) -> Result<Option<Game>> {
    let Some(i) = own(args).iter().position(|a| a == "--game") else {
        return Ok(None);
    };
    let name = args
        .get(i + 1)
        .context("--game needs a value: emerald or silice")?;
    let game = Game::parse(name)?;
    args.drain(i..=i + 1);
    if own(args).iter().any(|a| a == "--game") {
        bail!("--game given twice");
    }
    Ok(Some(game))
}

/// Applies `--game` (module docs): returns `Some(exit code)` when this run re-ran itself for the
/// asked flavour (the caller exits with it), `None` to go on with `args` (the flag removed).
pub fn select(args: &mut Vec<String>) -> Result<Option<i32>> {
    let all = args.clone();
    let Some(game) = take(args)? else {
        return Ok(None);
    };
    if game.built_by(content::FLAVOR) {
        // Emerald is an unset `PKR_GAME`, as plain builds have it: a changed value would redo
        // the content build in the child cargo invocations.
        // SAFETY: at the start of `main`, before any thread is spawned.
        match game {
            _ if inherited() == Some(game) => {}
            Game::Emerald => unsafe { std::env::remove_var("PKR_GAME") },
            Game::Silice => unsafe { std::env::set_var("PKR_GAME", game.name()) },
        }
        return Ok(None);
    }
    if std::env::var_os(REEXEC).is_some() {
        bail!(
            "--game {}: xtask re-ran under PKR_GAME={} but content still built the {} flavour \
             (Silice needs an extract: run `cargo xtask extract`)",
            game.name(),
            game.name(),
            content::FLAVOR.name()
        );
    }
    println!(
        "--game {}: this xtask is the {} build; re-running it under PKR_GAME={}",
        game.name(),
        content::FLAVOR.name(),
        game.name()
    );
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(crate::root()).args(["run", "-p", "xtask"]);
    if !cfg!(debug_assertions) {
        cmd.arg("--release");
    }
    cmd.arg("--").args(&all).env(REEXEC, "1");
    match game {
        Game::Emerald => cmd.env_remove("PKR_GAME"),
        Game::Silice => cmd.env("PKR_GAME", game.name()),
    };
    let status = cmd.status().with_context(|| format!("spawning {cmd:?}"))?;
    Ok(Some(status.code().unwrap_or(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn the_flag_is_taken_anywhere() {
        let mut a = s(&["rom", "--game", "silice"]);
        assert_eq!(take(&mut a).unwrap(), Some(Game::Silice));
        assert_eq!(a, s(&["rom"]));
        let mut a = s(&["--game", "emerald", "checkpoints", "verify"]);
        assert_eq!(take(&mut a).unwrap(), Some(Game::Emerald));
        assert_eq!(a, s(&["checkpoints", "verify"]));
        let mut a = s(&["explore", "run", "--act", "sil_ch0"]);
        assert_eq!(take(&mut a).unwrap(), None);
        assert_eq!(a.len(), 4);
    }

    #[test]
    fn studio_map_keeps_its_own_flag() {
        let mut a = s(&["studio", "map", "--game", "fixture", "list"]);
        assert_eq!(take(&mut a).unwrap(), None);
        assert_eq!(a.len(), 5);
        let mut a = s(&[
            "--game", "silice", "studio", "map", "--game", "fixture", "list",
        ]);
        assert_eq!(take(&mut a).unwrap(), Some(Game::Silice));
        assert_eq!(a, s(&["studio", "map", "--game", "fixture", "list"]));
        let mut a = s(&["studio", "--game", "silice"]);
        assert_eq!(take(&mut a).unwrap(), Some(Game::Silice));
    }

    #[test]
    fn bad_values_are_refused() {
        for bad in [
            &["rom", "--game"][..],
            &["rom", "--game", "ruby"],
            &["rom", "--game", "silice", "--game", "silice"],
        ] {
            assert!(take(&mut s(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_stub_counts_as_emerald() {
        assert!(Game::Emerald.built_by(Flavor::Stub));
        assert!(Game::Emerald.built_by(Flavor::Emerald));
        assert!(!Game::Emerald.built_by(Flavor::Silice));
        assert!(Game::Silice.built_by(Flavor::Silice));
        assert!(!Game::Silice.built_by(Flavor::Stub));
        assert!(!Game::Silice.built_by(Flavor::Emerald));
    }
}
