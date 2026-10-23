//! `cargo xtask explore [--debug] <args>` (Milestone 7): runs the `explore` bin, in release
//! unless the first argument is `--debug`. The environment is inherited, so
//! `PKR_EXTRACTED_DIR=/nonexistent cargo xtask explore …` explores the stub world.

use anyhow::Result;
use std::process::Command;

/// `cargo run` arguments for `args`.
fn cargo_args(args: &[String]) -> Vec<String> {
    let (release, rest) = match args.first() {
        Some(a) if a == "--debug" => (false, &args[1..]),
        _ => (true, args),
    };
    let mut v: Vec<String> = ["run", "-p", "explore"].map(String::from).to_vec();
    if release {
        v.push("--release".into());
    }
    v.extend(["--bin", "explore", "--"].map(String::from));
    v.extend(rest.iter().cloned());
    v
}

pub fn main(args: &[String]) -> Result<()> {
    crate::run(
        Command::new("cargo")
            .current_dir(crate::root())
            .args(cargo_args(args)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn release_unless_debug() {
        assert_eq!(
            cargo_args(&s(&["branch", "--jobs", "4"])),
            s(&[
                "run",
                "-p",
                "explore",
                "--release",
                "--bin",
                "explore",
                "--",
                "branch",
                "--jobs",
                "4"
            ])
        );
        assert_eq!(
            cargo_args(&s(&["--debug", "digest"])),
            s(&["run", "-p", "explore", "--bin", "explore", "--", "digest"])
        );
    }
}
