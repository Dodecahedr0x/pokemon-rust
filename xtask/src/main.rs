mod budget;
mod checkpoints;
mod dotenv;
mod explore;
mod game;
mod gates;
mod ledger;
mod silice;
mod studio;
mod sweep;
mod vanilla;

use anyhow::{Context, Result, bail, ensure};
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

/// `--config` for the GBA build that strips the checkout, `CARGO_HOME` and rustc sysroot
/// prefixes from the paths compiled into the ROM (panic locations, `file!()`), so its size and
/// bytes do not depend on where the repository is checked out. Cargo appends these flags to
/// crates/game-gba/.cargo/config.toml's (`-Tgba.ld` and friends).
fn remap_rustflags(gba_dir: &Path) -> Result<String> {
    let mut sysroot = Command::new("rustc");
    sysroot.current_dir(gba_dir).args(["--print", "sysroot"]);
    for var in NESTED_BUILD_ENV_REMOVE {
        sysroot.env_remove(var);
    }
    let out = sysroot.output().context("rustc --print sysroot")?;
    if !out.status.success() {
        bail!("rustc --print sysroot failed: {}", out.status);
    }
    let sysroot = PathBuf::from(String::from_utf8(out.stdout)?.trim());
    let cargo_home = match std::env::var_os("CARGO_HOME") {
        Some(h) => PathBuf::from(h),
        None => PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?).join(".cargo"),
    };
    let flags: Vec<String> = [
        (root(), "/pkr"),
        (cargo_home, "/cargo"),
        (sysroot, "/rustc"),
    ]
    .iter()
    .map(|(from, to)| {
        format!(
            "{:?}",
            format!("--remap-path-prefix={}={to}", from.display())
        )
    })
    .collect();
    Ok(format!(
        "target.thumbv4t-none-eabi.rustflags=[{}]",
        flags.join(", ")
    ))
}

/// game-gba's target dir: `crates/game-gba/target`, and `target/silice` in it for Silice, so
/// that switching flavours (`--game`) does not rebuild the other flavour's content and shell.
fn gba_target() -> PathBuf {
    let target = root().join("crates/game-gba/target");
    match content::FLAVOR {
        content::Flavor::Silice => target.join("silice"),
        content::Flavor::Stub | content::Flavor::Emerald => target,
    }
}

/// A release binary of game-gba ([`gba_target`]).
fn gba_bin(name: &str) -> PathBuf {
    gba_target().join("thumbv4t-none-eabi/release").join(name)
}

/// Builds game-gba (its own workspace/toolchain) in release with `features` and fixes its ELF
/// into `out`. Returns the ELF.
fn build_gba(features: &[&str], out: &Path) -> Result<PathBuf> {
    let gba_dir = root().join("crates/game-gba");
    let mut build = Command::new("cargo");
    build.current_dir(&gba_dir);
    for var in NESTED_BUILD_ENV_REMOVE {
        build.env_remove(var);
    }
    build
        .args(["build", "--release", "--target-dir"])
        .arg(gba_target())
        .arg("--config")
        .arg(remap_rustflags(&gba_dir)?);
    if !features.is_empty() {
        build.args(["--features", &features.join(",")]);
    }
    run(&mut build)?;
    let elf = gba_bin("game-gba");
    std::fs::create_dir_all(out.parent().unwrap())?;
    run(Command::new("agb-gbafix")
        .arg(&elf)
        .arg("-o")
        .arg(out)
        .args(["-t", "PKRUST"]))?;
    println!("ROM: {}", out.display());
    Ok(elf)
}

/// Build game-gba and fix it into target/game.gba, then report and gate its budgets.
fn rom() -> Result<PathBuf> {
    let out = root().join("target/game.gba");
    let elf = build_gba(&[], &out)?;
    check_rom_size(&out)?;
    let content = if content::STUB {
        budget::Content::Stub
    } else if content::FLAVOR == content::Flavor::Silice {
        budget::Content::Silice
    } else if content::data::MAP_COUNT > extract::export::SLICE_MAPS.len() {
        budget::Content::Full
    } else {
        budget::Content::Slice
    };
    budget::check(
        &budget::report(&elf, &out)?,
        &budget::sections(&elf)?,
        content,
    )?;
    // The video probe (crates/game-gba/src/bin/video_probe.rs), compared by the parity test.
    let probe = root().join("target/video-probe.gba");
    run(Command::new("agb-gbafix")
        .arg(gba_bin("video_probe"))
        .arg("-o")
        .arg(&probe)
        .args(["-t", "PKRPROBE"]))?;
    // The battle probe (crates/game-gba/src/bin/battle_probe.rs): the Mega Evolution turn's
    // cost, read by the parity test.
    let battle_probe = root().join("target/battle-probe.gba");
    run(Command::new("agb-gbafix")
        .arg(gba_bin("battle_probe"))
        .arg("-o")
        .arg(&battle_probe)
        .args(["-t", "PKRBPROBE"]))?;
    Ok(out)
}

/// GBA ROM size limit (32 MiB cartridge address space).
const ROM_LIMIT: u64 = 32 << 20;

fn check_rom_size(rom: &Path) -> Result<()> {
    let size = std::fs::metadata(rom)?.len();
    println!(
        "ROM size: {size} bytes ({} KiB of {} MiB)",
        size >> 10,
        ROM_LIMIT >> 20
    );
    if size > ROM_LIMIT {
        bail!("ROM is {size} bytes, over the 32 MiB GBA limit");
    }
    Ok(())
}

fn write_png(path: &Path, fb: &[u16]) -> Result<()> {
    let mut rgb = Vec::with_capacity(fb.len() * 3);
    for &c in fb {
        for ch in [c & 31, (c >> 5) & 31, (c >> 10) & 31] {
            rgb.push(((ch << 3) | (ch >> 2)) as u8);
        }
    }
    let file = std::fs::File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        game_native::SCREEN_W as u32,
        game_native::SCREEN_H as u32,
    );
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgb)?;
    Ok(())
}

/// Plays the parity route on ROM and native and writes `target/shots/<moment>-{rom,native}.png`
/// for each named moment. The ROM shows step `f`'s picture when it is at engine frame `f + 1`
/// (display lag), so it is captured there.
fn shot() -> Result<()> {
    fresh_extract()?;
    let rom = rom()?;
    let rec = game_native::scenario::record_route().map_err(anyhow::Error::msg)?;
    let picture_frames: Vec<u32> = rec.moments.iter().map(|m| m.frame + 1).collect();
    let total = rec
        .frames()
        .max(picture_frames.last().copied().unwrap_or(0));
    let run = emu::run_rom(&rom, |f| rec.keys_at(f), total, &picture_frames)?;
    let mut native = game_native::Session::new(content::Emerald::for_shells());
    let dir = root().join("target/shots");
    std::fs::create_dir_all(&dir)?;
    let mut same = 0;
    for (m, cap) in rec.moments.iter().zip(&run.captures) {
        while native.world().frame() < m.frame {
            let f = native.world().frame();
            native.step(rec.keys_at(f));
        }
        let fb = native.framebuffer();
        write_png(&dir.join(format!("{}-rom.png", m.name)), &cap.fb)?;
        write_png(&dir.join(format!("{}-native.png", m.name)), fb)?;
        let equal = cap.fb == fb;
        same += equal as usize;
        println!(
            "{:<20} frame {:>5}  map {:#06x}  rom {} native",
            m.name,
            m.frame,
            m.map.0,
            if equal { "==" } else { "!=" }
        );
    }
    println!(
        "wrote {} moments to {} ({same} identical)",
        rec.moments.len(),
        dir.display()
    );
    if same != rec.moments.len() {
        bail!("ROM and native pictures differ");
    }
    Ok(())
}

/// Refuses a stale extract: the ROM and native would both run the stub world and agree.
fn fresh_extract() -> Result<()> {
    if content::STALE_EXTRACT {
        bail!(content::STALE_EXTRACT_MSG);
    }
    Ok(())
}

/// `cargo xtask parity [--long]`: the committed tree verified, the ROM built, then the parity
/// lanes (`emu/tests/parity.rs`, but `act1_long`), the native Act 1 invariant
/// (`game-native/tests/act1.rs`) and the studio's ROM pane. `--long` then replays the whole Act 1
/// trace on the ROM (`parity.rs`'s `act1_long`), builds the map tour ROM
/// (`target/game-tour.gba`, `game-gba --features tour`) and runs `emu/tests/tour.rs`.
///
/// Steps that do not depend on each other run at once: the tree's verification, the ROM build
/// and the three test builds; then the parity lanes, the native Act 1 invariant, the studio's
/// ROM pane and (`--long`) the tour ROM's build; then (`--long`) `act1_long` and the tour test.
/// The side steps' output is held and printed in one piece each, so it does not interleave with
/// the parity lanes'.
fn parity(args: &[String]) -> Result<()> {
    let long = match args {
        [] => false,
        [a] if a == "--long" => true,
        _ => bail!("usage: cargo xtask parity [--long]"),
    };
    fresh_extract()?;
    // Positional names (an extract without `PKR_DECOMP`) apply no override: the game would not
    // be the one the trees and the parity fixtures describe.
    ensure!(
        content::FLAVOR != content::Flavor::Emerald || content::scripts::DECOMP_NAMES,
        "the script IR has positional names only (scripts/decomp_names = 0): set PKR_DECOMP and \
         re-run `cargo xtask extract` before `cargo xtask parity`"
    );
    // `cargo test <args>`, or its build only (`--no-run`) with `run_args` dropped.
    let test = |args: &[&str], run_args: &[&str], no_run: bool| {
        let mut c = Command::new("cargo");
        c.current_dir(root()).arg("test").args(args);
        if no_run {
            c.arg("--no-run");
        } else {
            c.args(run_args);
        }
        c
    };
    let parity_test = |no_run: bool| {
        test(
            &["-p", "emu", "--test", "parity"],
            &["--", "--ignored", "--nocapture", "--skip", "act1_long"],
            no_run,
        )
    };
    // The checkpoint invariant along Act 1 (Milestone 8 Task P), native, in release.
    let act1_test = |no_run: bool| {
        test(
            &["-p", "game-native", "--release", "--test", "act1"],
            &["--", "--ignored", "--nocapture"],
            no_run,
        )
    };
    // The studio's ROM pane on the ROM just built (its `#[ignore]`d tests need `target/game.gba`).
    let studio_test = |no_run: bool| {
        test(
            &["-p", "studio", "--release", "--lib"],
            &["rom::tests::", "--", "--ignored"],
            no_run,
        )
    };
    std::thread::scope(|s| -> Result<()> {
        let rom = s.spawn(rom);
        let builds = [
            s.spawn(|| held(&mut parity_test(true))),
            s.spawn(|| held(&mut act1_test(true))),
            s.spawn(|| held(&mut studio_test(true))),
        ];
        // The committed tree must replay to its nodes before the ROM is compared against
        // native.
        let verified = checkpoints::verify(
            &game_native::store::Store::default_root(content::FLAVOR),
            false,
        );
        let rom = rom.join().unwrap();
        let builds = builds.map(|b| b.join().unwrap());
        verified?;
        rom?;
        for b in builds {
            b?;
        }
        let act1 = s.spawn(|| held(&mut act1_test(false)));
        let studio = s.spawn(|| held(&mut studio_test(false)));
        let tour = long.then(|| s.spawn(tour_rom));
        let lanes = run(&mut parity_test(false));
        let act1 = act1.join().unwrap();
        let studio = studio.join().unwrap();
        let tour = tour.map(|t| t.join().unwrap());
        lanes?;
        act1?;
        studio?;
        if let Some(tour) = tour {
            // The whole Act 1 trace on the ROM from a new game (Milestone 8 Task P).
            run(&mut test(
                &["-p", "emu", "--test", "parity", "act1_long"],
                &["--", "--ignored", "--nocapture"],
                false,
            ))?;
            tour?;
            run(Command::new("cargo").current_dir(root()).args([
                "test",
                "-p",
                "emu",
                "--test",
                "tour",
                "--",
                "--ignored",
                "--nocapture",
            ]))?;
        }
        Ok(())
    })
}

/// [`run`] with the output held, then printed in one piece (on failure too).
fn held(cmd: &mut Command) -> Result<()> {
    use std::io::Write;
    let out = cmd.output().with_context(|| format!("spawning {cmd:?}"))?;
    let mut lock = std::io::stdout().lock();
    lock.write_all(&out.stdout)?;
    lock.write_all(&out.stderr)?;
    if !out.status.success() {
        bail!("{cmd:?} failed: {}", out.status);
    }
    Ok(())
}

/// The map tour's ROM (Milestone 8 Task M), `target/game-tour.gba`. The tour build shares
/// game-gba's target dir (only the final link differs); `target/game.gba` stays the game's ROM.
fn tour_rom() -> Result<()> {
    let out = root().join("target/game-tour.gba");
    build_gba(&["tour"], &out)?;
    check_rom_size(&out)
}

/// `--maps` operand: `all` (every `names::MAPS` entry), `slice` (`export::SLICE_MAPS`) or a
/// comma-separated list of decomp map directory names.
fn parse_maps(arg: &str) -> Result<Vec<&'static str>> {
    use extract::export::ExportOptions;
    Ok(match arg {
        "all" => ExportOptions::all(false).maps,
        "slice" => ExportOptions::slice(false).maps,
        list => list
            .split(',')
            .map(|name| {
                extract::names::MAPS
                    .iter()
                    .find(|m| m.dir == name)
                    .map(|m| m.dir)
                    .with_context(|| format!("--maps: unknown map {name:?}"))
            })
            .collect::<Result<_>>()?,
    })
}

/// Reads the user's ROM into games/emerald/, replacing it whole (no stale files remain).
///
/// `--maps all|slice|<Dir,Dir,...>` picks the exported maps (default `all`, every map since
/// Milestone 8 Task L; `slice` is `extract::export::SLICE_MAPS`, for fixtures) and `--previews`
/// also writes `previews/<Map>.png` (off by default: they are most of the extracted size and
/// nothing in the build reads them).
///
/// Exports into games/.emerald.tmp/, then moves the previous output aside to
/// games/.emerald.old/, renames the new export into place and only then deletes the old
/// one. If any step fails, the previous output is left (or put back) in games/emerald/.
/// `games/emerald/.gitkeep` is put back afterwards (tracked, so a stub build can watch the
/// directory; the content hash skips hidden files).
fn extract(args: &[String]) -> Result<()> {
    const USAGE: &str =
        "usage: cargo xtask extract [--rom <path>] [--maps all|slice|<Dir,Dir,...>] [--previews]";
    let mut rom_path = root().join("rom/emeraude-fr.gba");
    let mut opts = extract::export::ExportOptions::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--rom" => rom_path = PathBuf::from(it.next().context(USAGE)?),
            "--maps" => opts.maps = parse_maps(it.next().context(USAGE)?)?,
            "--previews" => opts.previews = true,
            _ => bail!(USAGE),
        }
    }
    let rom = extract::Rom::open(&rom_path)?;
    let out = root().join("games/emerald");
    let tmp = root().join("games/.emerald.tmp");
    let old = root().join("games/.emerald.old");
    let clear = |dir: &Path| -> Result<()> {
        if dir.exists() {
            std::fs::remove_dir_all(dir).with_context(|| format!("clearing {}", dir.display()))?;
        }
        Ok(())
    };
    let rename = |from: &Path, to: &Path| -> Result<()> {
        std::fs::rename(from, to)
            .with_context(|| format!("renaming {} to {}", from.display(), to.display()))
    };
    // A run interrupted mid-swap leaves the last good export in .emerald.old: put it back.
    if !out.exists() && old.exists() {
        rename(&old, &out)?;
    }
    clear(&old)?;
    clear(&tmp)?;
    let m = extract::export::export(&rom, &tmp, &opts)
        .with_context(|| format!("exporting to {}", tmp.display()))?;
    let had_out = out.exists();
    if had_out {
        rename(&out, &old)?;
    }
    if let Err(e) = rename(&tmp, &out) {
        if had_out && let Err(restore) = rename(&old, &out) {
            return Err(e.context(format!(
                "also failed to restore the old output: {restore:#}"
            )));
        }
        return Err(e);
    }
    clear(&old)?;
    // Hidden, so it is not part of `content::BUILD_ID`. The swap above removes it with the
    // previous directory.
    std::fs::write(out.join(".gitkeep"), [])
        .with_context(|| format!("restoring {}", out.join(".gitkeep").display()))?;
    println!(
        "ROM: {} ({}, {}, sha1 {}), format v{}",
        rom_path.display(),
        m.game_code,
        m.language,
        m.sha1,
        m.format
    );
    let count = |prefix: &str, ext: &str| {
        m.files
            .keys()
            .filter(|f| f.starts_with(prefix) && f.ends_with(ext))
            .count()
    };
    println!(
        "maps: {}, layouts: {}, tilesets: {}, sprites: {} (+{} field effects), previews: {}",
        m.maps.len(),
        count("layouts/", ".ron"),
        m.tilesets.len(),
        m.sprites.len(),
        count("sprites/field_effects/", ".ron"),
        count("previews/", ".png"),
    );
    if !m.skipped_graphics.is_empty() {
        let skipped: Vec<&str> = m.skipped_graphics.iter().map(String::as_str).collect();
        println!("skipped graphics: {}", skipped.join(", "));
    }
    println!("font: font/latin_normal, window: window/message_box");
    let scripts = m.files.keys().filter(|f| f.starts_with("scripts/")).count();
    let texts = m.files.keys().filter(|f| f.starts_with("text/")).count();
    println!("scripts: {scripts} files, texts: {texts} files");
    for (script, ops) in &m.unknown_script_ops {
        println!(
            "warning: {script}: {} undecoded script instructions",
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
    dotenv::load();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--game <emerald|silice>`, anywhere (SIL1 Task D).
    if let Some(code) = game::select(&mut args)? {
        std::process::exit(code);
    }
    match args.first().map(String::as_str) {
        Some("rom") => rom().map(|_| ()),
        Some("parity") => parity(&args[1..]),
        Some("shot") => shot(),
        Some("extract") => extract(&args[1..]),
        Some("checkpoints") => checkpoints::main(&args[1..]),
        Some("studio") => studio::main(&args[1..]),
        Some("explore") => explore::main(&args[1..]),
        Some("sweep") => sweep::main(&args[1..]),
        Some("silice") => silice::main(&args[1..]),
        Some("vanilla") => vanilla::main(&args[1..]),
        Some("gates") => gates::main(&args[1..]),
        Some("ledger") => ledger::main(&args[1..]),
        _ => bail!(
            "usage: cargo xtask <rom|parity|shot|extract|checkpoints|studio|explore|sweep|silice|vanilla|gates|ledger> \
             [--game emerald|silice]"
        ),
    }
}
