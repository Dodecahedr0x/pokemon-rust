use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("spawning {cmd:?}"))?;
    if !status.success() {
        bail!("{cmd:?} failed: {status}");
    }
    Ok(())
}

/// Build game-gba (its own workspace/toolchain) and fix it into target/game.gba.
fn rom() -> Result<PathBuf> {
    let gba_dir = root().join("crates/game-gba");
    run(Command::new("cargo")
        .current_dir(&gba_dir)
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("CARGO")
        .args(["build", "--release"]))?;
    let elf = gba_dir.join("target/thumbv4t-none-eabi/release/game-gba");
    let out = root().join("target/game.gba");
    std::fs::create_dir_all(out.parent().unwrap())?;
    run(Command::new("agb-gbafix")
        .arg(&elf)
        .arg("-o")
        .arg(&out)
        .args(["-t", "PKRUST"]))?;
    println!("ROM: {}", out.display());
    Ok(out)
}

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("rom") => rom().map(|_| ()),
        _ => bail!("usage: cargo xtask <rom>"),
    }
}
