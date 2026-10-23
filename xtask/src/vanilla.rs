//! `cargo xtask vanilla [--debug] <spine|branches> [args]` (Emerald parity plan, Task O2): the
//! vanilla oracle, BPEE in mGBA against our engine (`crates/vanilla`'s `vanilla` bin), in
//! release unless `--debug` (the debug build runs about 10x slower). Needs `PKR_ROM_US`.
//! Its per-step frame counts are Milestone 9 Task V's reference timings.

use anyhow::Result;
use std::process::Command;

/// `cargo run` arguments for `args`.
fn cargo_args(args: &[String]) -> Vec<String> {
    let (release, rest) = match args.first() {
        Some(a) if a == "--debug" => (false, &args[1..]),
        _ => (true, args),
    };
    let mut v: Vec<String> = ["run", "-p", "vanilla"].map(String::from).to_vec();
    if release {
        v.push("--release".into());
    }
    v.extend(["--bin", "vanilla", "--"].map(String::from));
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

    #[test]
    fn release_unless_debug() {
        let s = |v: &[&str]| v.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        assert_eq!(
            cargo_args(&s(&["spine"])),
            s(&[
                "run",
                "-p",
                "vanilla",
                "--release",
                "--bin",
                "vanilla",
                "--",
                "spine"
            ])
        );
        assert_eq!(
            cargo_args(&s(&["--debug", "branches", "--limit", "3"])),
            s(&[
                "run", "-p", "vanilla", "--bin", "vanilla", "--", "branches", "--limit", "3"
            ])
        );
    }
}
