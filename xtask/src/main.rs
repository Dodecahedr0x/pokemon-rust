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

/// Inherited env that would override crates/game-gba's own toolchain and
/// .cargo/config.toml (e.g. RUSTFLAGS replaces its rustflags, dropping -Tgba.ld).
const NESTED_BUILD_ENV_REMOVE: &[&str] = &[
    "RUSTUP_TOOLCHAIN",
    "CARGO",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_BUILD_TARGET",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "RUSTC",
];

/// Build game-gba (its own workspace/toolchain) and fix it into target/game.gba.
fn rom() -> Result<PathBuf> {
    let gba_dir = root().join("crates/game-gba");
    let mut build = Command::new("cargo");
    build.current_dir(&gba_dir);
    for var in NESTED_BUILD_ENV_REMOVE {
        build.env_remove(var);
    }
    build
        .args(["build", "--release", "--target-dir"])
        .arg(gba_dir.join("target"));
    run(&mut build)?;
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

fn write_ppm(path: &Path, fb: &[u16]) -> Result<()> {
    let mut data = b"P6\n240 160\n255\n".to_vec();
    for &c in fb {
        for ch in [c & 31, (c >> 5) & 31, (c >> 10) & 31] {
            data.push(((ch << 3) | (ch >> 2)) as u8);
        }
    }
    std::fs::write(path, data)?;
    Ok(())
}

/// Same idle frames on ROM and native, then dump both images side by side for eyeballing.
fn shot() -> Result<()> {
    let rom = rom()?;
    let mut emu = emu::Emu::open(&rom)?;
    emu.boot(120)?;
    let mut native = game_native::Session::new(content::Emerald);
    for _ in 0..30 {
        emu.run_frame(engine::Buttons::NONE)?;
        native.step(engine::Buttons::NONE);
    }
    let dir = root().join("target");
    write_ppm(&dir.join("shot-rom.ppm"), &emu.framebuffer_rgb555())?;
    write_ppm(&dir.join("shot-native.ppm"), native.framebuffer())?;
    println!("wrote target/shot-rom.ppm and target/shot-native.ppm");
    Ok(())
}

fn parity() -> Result<()> {
    rom()?;
    run(Command::new("cargo").current_dir(root()).args([
        "test",
        "-p",
        "emu",
        "--test",
        "parity",
        "--",
        "--ignored",
        "--nocapture",
    ]))
}

/// Reads the user's ROM into content/extracted/ (cleared first so no stale files remain).
fn extract(args: &[String]) -> Result<()> {
    let rom_path = match args {
        [] => root().join("rom/emeraude-fr.gba"),
        [flag, path] if flag == "--rom" => PathBuf::from(path),
        _ => bail!("usage: cargo xtask extract [--rom <path>]"),
    };
    let rom = extract::Rom::open(&rom_path)?;
    // Export next to the output and swap it in only on success, keeping the last good export.
    let out = root().join("content/extracted");
    let tmp = root().join("content/.extracted.tmp");
    let clear = |dir: &Path| -> Result<()> {
        if dir.exists() {
            std::fs::remove_dir_all(dir).with_context(|| format!("clearing {}", dir.display()))?;
        }
        Ok(())
    };
    clear(&tmp)?;
    let m = extract::export::export(&rom, &tmp)
        .with_context(|| format!("exporting to {}", tmp.display()))?;
    clear(&out)?;
    std::fs::rename(&tmp, &out)
        .with_context(|| format!("renaming {} to {}", tmp.display(), out.display()))?;
    println!(
        "ROM: {} ({}, sha1 {})",
        rom_path.display(),
        m.game_code,
        m.sha1
    );
    println!(
        "maps: {}",
        m.maps.keys().cloned().collect::<Vec<_>>().join(", ")
    );
    println!("tilesets: {}", m.tilesets.len());
    let ids: Vec<String> = m.sprites.keys().map(u8::to_string).collect();
    println!("sprites: {} (ids {})", ids.len(), ids.join(", "));
    if !m.skipped_graphics_ids.is_empty() {
        println!("skipped graphics ids: {:?}", m.skipped_graphics_ids);
    }
    println!("texts: {} ({})", m.texts, m.text_file);
    for (map, ops) in &m.unknown_script_ops {
        println!(
            "warning: {map}: {} undecoded script instructions",
            ops.len()
        );
    }
    println!(
        "wrote {} files + manifest.ron to {}",
        m.files.len(),
        out.display()
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("rom") => rom().map(|_| ()),
        Some("parity") => parity(),
        Some("shot") => shot(),
        Some("extract") => extract(&args[1..]),
        _ => bail!("usage: cargo xtask <rom|parity|shot|extract>"),
    }
}
