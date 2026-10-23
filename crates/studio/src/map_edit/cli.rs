//! The headless editor for agents: `cargo xtask studio map …` (the `studio-map` bin).
//!
//! ```text
//! studio-map [--games DIR | --game NAME] <command>
//!   list                                   the games dir's maps
//!   create <spec.ron>                      a new map from a NewMap spec (never replaces a map,
//!                                          nor a layout unless the spec shares it)
//!   apply <Map> <script.ron> [--dry-run]   applies an edit script (a RON list of Op) and saves,
//!                                          even a draft that still has errors (they are listed)
//!   build [<edits dir>]                    every <Map>/<phase>_*.ron of an edits dir (default
//!                                          <games>/edits) in one process: creates, then the
//!                                          scripts by phase, then one validate (`batch`)
//!   validate [<Map>...]                    every map by default (and orphan layouts); fails
//!                                          on errors
//!   hygiene                                the games dir holds only canonical names and
//!                                          indices, and no extracted map region; fails if not
//!   render <Map> [--extract] [--out FILE] [--margin N] [--events] [--collision] [--scale N]
//!                                          a PNG (default target/studio/maps/<Map>.png); the
//!                                          overlays mark events and collision (`overlay`);
//!                                          --extract renders the extract's map <Map> (an
//!                                          Emerald map, for reference) instead of the games
//!                                          dir's (default target/studio/maps/extract-<Map>.png)
//!   dump <Map> [--extract] [--out FILE|-]  the map's metatile grid as text (default
//!                                          target/studio/maps/[extract-]<Map>.txt; - prints it)
//!   palette <Primary> <Secondary> [--out FILE] [--labels] [--scale N] [--zoom N]
//!           [--range A..B | --range A..=B]
//!                                          the metatile palette (default
//!                                          target/studio/maps/palette-<P>-<S>.png), 8 per row;
//!                                          --labels draws ids and behaviour codes in a gutter
//!                                          above each cell (never over the art), and prints
//!                                          the codes' legend; --range keeps only those
//!                                          metatile ids; --zoom scales the art (default: the
//!                                          scale), --scale the labels
//!   stamp-from <Map> <x> <y> <w> <h> <name> [--no-elevation] [--force]
//!                                          saves a region of a map as stamps/<name>.ron
//! ```
//!
//! The games dir defaults to `games/silice` (`--game` picks `games/<NAME>`). Pictures and dumps
//! go to the git-ignored scratch dir unless `--out` says otherwise; they are never committed
//! (an extract render or dump is ROM-derived). With an overlay or labels, pictures are scaled
//! 2x unless `--scale` says otherwise.

use super::batch;
use super::catalog::{Catalog, extracted_dir};
use super::games::{DEFAULT_GAME, GamesDir, check_name, hygiene};
use super::model::{Doc, Editor, NewMap, Op};
use super::overlay;
use super::render::{palette_of, render, write_png};
use super::resources::Resources;
use super::schema::from_script_ron;
use super::stamp::Stamp;
use super::validate::{Issue, errors, id_changes, validate};
use crate::paths::StudioPaths;
use anyhow::{Context, Result, bail};
use engine::data::{block_collision, block_metatile};
use std::fmt::Write as _;
use std::ops::RangeInclusive;
use std::path::PathBuf;

pub const USAGE: &str = "usage: studio-map [--games DIR | --game NAME] \
     (list | create <spec.ron> | apply <Map> <script.ron> [--dry-run] | build [<edits dir>] | \
     validate [<Map>...] | hygiene | \
     render <Map> [--extract] [--out FILE] [--margin N] [--events] [--collision] [--scale N] | \
     dump <Map> [--extract] [--out FILE|-] | \
     palette <Primary> <Secondary> [--out FILE] [--labels] [--scale N] [--zoom N] \
     [--range A..B | --range A..=B] | \
     stamp-from <Map> <x> <y> <w> <h> <name> [--no-elevation] [--force])\n\
     apply and build save edited maps even when they still have validation errors (drafts); \
     validate fails on them.";

/// Margin cells of a CLI render.
pub const RENDER_MARGIN: u16 = 2;

/// The scale of a picture with labels or an overlay.
pub const OVERLAY_SCALE: usize = 2;

/// Options shared by `render`, `dump` and `palette`.
#[derive(Default)]
struct Picture {
    out: Option<PathBuf>,
    margin: Option<u16>,
    scale: Option<usize>,
    zoom: Option<usize>,
    events: bool,
    collision: bool,
    labels: bool,
    extract: bool,
    range: Option<RangeInclusive<u16>>,
}

/// The command whose flags [`picture_flags`] reads.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Render,
    Dump,
    Palette,
}

fn factor(flag: &str, v: &str) -> Result<usize> {
    let s: usize = v.parse().with_context(|| format!("{flag} {v}"))?;
    anyhow::ensure!((1..=8).contains(&s), "{flag} {s} (1..=8)");
    Ok(s)
}

/// `A..B` (B excluded) or `A..=B`, metatile ids.
pub fn metatile_range(v: &str) -> Result<RangeInclusive<u16>> {
    let bad = || format!("--range {v:?} (A..B or A..=B, metatile ids)");
    let (a, b) = v.split_once("..").with_context(bad)?;
    let a: u16 = a.trim().parse().with_context(bad)?;
    let r = match b.strip_prefix('=') {
        Some(b) => a..=b.trim().parse().with_context(bad)?,
        None => {
            let b: u16 = b.trim().parse().with_context(bad)?;
            anyhow::ensure!(b > a, "{}: empty", bad());
            a..=b - 1
        }
    };
    anyhow::ensure!(!r.is_empty(), "{}: empty", bad());
    Ok(r)
}

fn picture_flags(mut f: &[String], kind: Kind) -> Result<Picture> {
    let mut p = Picture::default();
    let (render, palette) = (kind == Kind::Render, kind == Kind::Palette);
    loop {
        match f {
            [] => return Ok(p),
            [k, v, t @ ..] if k == "--out" => (p.out, f) = (Some(PathBuf::from(v)), t),
            [k, t @ ..] if k == "--extract" && !palette => (p.extract, f) = (true, t),
            [k, v, t @ ..] if k == "--scale" && kind != Kind::Dump => {
                (p.scale, f) = (Some(factor(k, v)?), t)
            }
            [k, v, t @ ..] if k == "--margin" && render => {
                p.margin = Some(v.parse().with_context(|| format!("--margin {v}"))?);
                f = t;
            }
            [k, t @ ..] if k == "--events" && render => (p.events, f) = (true, t),
            [k, t @ ..] if k == "--collision" && render => (p.collision, f) = (true, t),
            [k, t @ ..] if k == "--labels" && palette => (p.labels, f) = (true, t),
            [k, v, t @ ..] if k == "--zoom" && palette => (p.zoom, f) = (Some(factor(k, v)?), t),
            [k, v, t @ ..] if k == "--range" && palette => {
                (p.range, f) = (Some(metatile_range(v)?), t)
            }
            _ => bail!(USAGE),
        }
    }
}

/// The extract's map `name` (`<extracted>/maps/<name>.ron`), for reference renders and dumps.
pub fn extracted_doc(name: &str) -> Result<Doc> {
    check_name(name)?;
    let dir = extracted_dir();
    let path = dir.join("maps").join(format!("{name}.ron"));
    let text = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "reading {} (--extract reads the extract's maps; is {} extracted?)",
            path.display(),
            dir.display()
        )
    })?;
    let file: extract::schema::MapFile =
        ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Doc::from_map_file(&file)
}

/// The metatile grid of `d` as text: a header (size, tilesets, border), then one line per row,
/// each cell its metatile id followed by `*` when solid (collision 1..=3) or a space.
pub fn dump(d: &Doc) -> String {
    let mut out = String::new();
    let cell = |b: u16| {
        let solid = if block_collision(b) != 0 { '*' } else { ' ' };
        format!("{:>4}{solid}", block_metatile(b))
    };
    let _ = writeln!(
        out,
        "{}: {}x{}, {} + {}; metatile ids, * = solid",
        d.map.name, d.width, d.height, d.primary, d.secondary
    );
    let border: String = d.border.iter().map(|&b| cell(b)).collect();
    let _ = writeln!(out, "border (row-major 2x2):{border}");
    out.push_str("y\\x");
    for x in 0..d.width {
        let _ = write!(out, "{x:>4} ");
    }
    out.push('\n');
    for (y, row) in d.blocks.chunks(d.width.max(1) as usize).enumerate() {
        let _ = write!(out, "{y:>3}");
        for &b in row {
            out.push_str(&cell(b));
        }
        out.push('\n');
    }
    out
}

fn number(v: &str, what: &str) -> Result<i32> {
    v.parse().with_context(|| format!("{what} {v:?}"))
}

/// Runs one command; returns what to print. `Err` for a failed command (including a
/// `validate`, `build` or `hygiene` that found errors).
pub fn run(args: &[String]) -> Result<String> {
    let paths = StudioPaths::detect();
    let mut games = GamesDir::for_game(&paths.workspace, DEFAULT_GAME);
    let mut rest = args;
    loop {
        match rest {
            [flag, dir, tail @ ..] if flag == "--games" => {
                games = GamesDir::new(dir);
                rest = tail;
            }
            [flag, name, tail @ ..] if flag == "--game" => {
                check_name(name).context("--game")?;
                games = GamesDir::for_game(&paths.workspace, name);
                rest = tail;
            }
            _ => break,
        }
    }
    let scratch = paths.scratch.join("maps");
    let mut out = String::new();
    match rest {
        [cmd] if cmd == "list" => {
            for name in games.map_names()? {
                writeln!(out, "{name}")?;
            }
        }
        [cmd, spec] if cmd == "create" => {
            let text = std::fs::read_to_string(spec).with_context(|| format!("reading {spec}"))?;
            let spec: NewMap = from_script_ron(&text).with_context(|| format!("parsing {spec}"))?;
            let doc = games.new_doc(&spec)?;
            let catalog = Catalog::for_games(&games)?;
            let issues = validate(&doc, &catalog);
            if errors(&issues) > 0 {
                report(&mut out, &doc.map.name, &issues);
                bail!("{out}{}: not created", doc.map.name);
            }
            games.create(&doc)?;
            writeln!(out, "created {}", games.map_path(&doc.map.name).display())?;
            report(&mut out, &doc.map.name, &issues);
        }
        [cmd, map, script, flags @ ..] if cmd == "apply" => {
            let dry = match flags {
                [] => false,
                [f] if f == "--dry-run" => true,
                _ => bail!(USAGE),
            };
            let text =
                std::fs::read_to_string(script).with_context(|| format!("reading {script}"))?;
            let ops: Vec<Op> =
                from_script_ron(&text).with_context(|| format!("parsing {script}"))?;
            let catalog = Catalog::for_games(&games)?;
            let env = Resources::new(games.clone(), catalog.clone());
            let mut ed = Editor::new(games.load(map)?).with_env(env);
            ed.apply_all(&ops)?;
            let doc = ed.doc();
            let mut issues = validate(doc, &catalog);
            if let Some(before) = ed.saved() {
                issues.extend(id_changes(before, doc));
            }
            if dry {
                writeln!(out, "{map}: {} ops applied (dry run, not saved)", ops.len())?;
            } else {
                games.save(doc)?;
                writeln!(out, "{map}: {} ops applied, saved", ops.len())?;
            }
            report(&mut out, map, &issues);
        }
        [cmd, dir @ ..] if cmd == "build" && dir.len() <= 1 => {
            let edits = dir.first().map_or_else(|| games.edits_dir(), PathBuf::from);
            let catalog = Catalog::for_games(&games)?;
            let env = Resources::new(games.clone(), catalog.clone());
            let b = batch::build(&games, &edits, &catalog, env)?;
            writeln!(
                out,
                "built {}: {} created, {} scripts, {} ops",
                edits.display(),
                b.created,
                b.scripts,
                b.ops
            )?;
            out.push_str(&b.report);
            for l in games.orphan_layouts()? {
                writeln!(out, "layout {l}: warning: no map uses it")?;
            }
            if b.errors > 0 {
                bail!("{out}{} errors in {} maps", b.errors, b.maps);
            }
            writeln!(out, "{} maps, no errors", b.maps)?;
        }
        [cmd, maps @ ..] if cmd == "validate" => {
            let names = if maps.is_empty() {
                games.map_names()?
            } else {
                maps.to_vec()
            };
            let catalog = Catalog::for_games(&games)?;
            let (report, total) = batch::validate_all(&games, &catalog, &names);
            out.push_str(&report);
            if maps.is_empty() {
                for l in games.orphan_layouts()? {
                    writeln!(out, "layout {l}: warning: no map uses it")?;
                }
            }
            if total > 0 {
                bail!("{out}{total} errors in {} maps", names.len());
            }
            writeln!(out, "{} maps, no errors", names.len())?;
        }
        [cmd] if cmd == "hygiene" => {
            let bad = hygiene(&games)?;
            for b in &bad {
                writeln!(out, "{b}")?;
            }
            if !bad.is_empty() {
                bail!(
                    "{out}{} hygiene problems in {}",
                    bad.len(),
                    games.root.display()
                );
            }
            writeln!(out, "{}: clean", games.root.display())?;
        }
        [cmd, map, flags @ ..] if cmd == "dump" => {
            let p = picture_flags(flags, Kind::Dump)?;
            let (doc, prefix) = match p.extract {
                true => (extracted_doc(map)?, "extract-"),
                false => (games.load(map)?, ""),
            };
            let text = dump(&doc);
            match p.out {
                Some(f) if f.as_os_str() == "-" => out.push_str(&text),
                f => {
                    let file = f.unwrap_or_else(|| scratch.join(format!("{prefix}{map}.txt")));
                    if let Some(dir) = file.parent() {
                        std::fs::create_dir_all(dir)?;
                    }
                    std::fs::write(&file, &text)
                        .with_context(|| format!("writing {}", file.display()))?;
                    writeln!(
                        out,
                        "{map}: {}x{} cells: {}",
                        doc.width,
                        doc.height,
                        file.display()
                    )?;
                }
            }
        }
        [cmd, map, flags @ ..] if cmd == "render" => {
            let p = picture_flags(flags, Kind::Render)?;
            let prefix = if p.extract { "extract-" } else { "" };
            let file = p
                .out
                .unwrap_or_else(|| scratch.join(format!("{prefix}{map}.png")));
            let margin = p.margin.unwrap_or(RENDER_MARGIN);
            let overlays = p.events || p.collision;
            let s = p.scale.unwrap_or(if overlays { OVERLAY_SCALE } else { 1 });
            let (doc, catalog) = match p.extract {
                true => (extracted_doc(map)?, Catalog::from_content()?),
                false => (games.load(map)?, Catalog::for_games(&games)?),
            };
            let mut img = render(&doc, &catalog, margin)?;
            if s > 1 || overlays {
                img = overlay::scale(&img, s);
            }
            if p.collision {
                overlay::draw_collision(&mut img, &doc, &catalog, s);
            }
            if p.events {
                overlay::draw_events(&mut img, &doc, s);
            }
            write_png(&img, &file)?;
            writeln!(
                out,
                "{map}: {}x{} cells, margin {margin}, scale {s}: {}",
                doc.width,
                doc.height,
                file.display()
            )?;
            if p.events {
                writeln!(
                    out,
                    "events: objects yellow (local id), warps magenta (W<i>), coords cyan \
                     (C<i>), signs green (S<i>), hidden items orange (H<i>)"
                )?;
            }
            if p.collision {
                writeln!(
                    out,
                    "collision: solid cells red, behaviour codes bottom-right, ! = collision \
                     against the usual one (validate lists them), + = such a cell an \
                     AllowCollision accepts (framed green)"
                )?;
            }
        }
        [cmd, primary, secondary, flags @ ..] if cmd == "palette" => {
            let p = picture_flags(flags, Kind::Palette)?;
            let file = p.out.clone().unwrap_or_else(|| {
                let r = (p.range.as_ref())
                    .map_or(String::new(), |r| format!("-{}-{}", r.start(), r.end()));
                scratch.join(format!("palette-{primary}-{secondary}{r}.png"))
            });
            let catalog = Catalog::from_content()?;
            let pal = palette_of(&catalog, primary, secondary, p.range.clone())?;
            let s = p.scale.unwrap_or(if p.labels { OVERLAY_SCALE } else { 1 });
            let zoom = p.zoom.unwrap_or(s);
            let img = match p.labels {
                true => overlay::label_palette(&pal, &catalog, primary, secondary, s, zoom, true),
                false if zoom > 1 => overlay::scale(&pal.image, zoom),
                false => pal.image.clone(),
            };
            write_png(&img, &file)?;
            let first_secondary = pal
                .metatiles
                .iter()
                .position(|m| m.is_some_and(|m| m >= 512));
            match &p.range {
                None => writeln!(
                    out,
                    "{primary} + {secondary}: {} cells, {} per row; cell (x, y) is metatile \
                     cols*y + x for the primary, the secondary from cell {}: {}",
                    pal.metatiles.len(),
                    pal.cols,
                    first_secondary.unwrap_or(pal.metatiles.len()),
                    file.display()
                )?,
                Some(r) => {
                    let rows: Vec<String> = (pal.metatiles.chunks(pal.cols as usize))
                        .filter_map(|row| row.iter().flatten().next().map(|m| m.to_string()))
                        .collect();
                    writeln!(
                        out,
                        "{primary} + {secondary}, metatiles {}..={}: {} cells, {} per row \
                         (the secondary on a new row); rows start at metatiles {}: {}",
                        r.start(),
                        r.end(),
                        pal.metatiles.len(),
                        pal.cols,
                        rows.join(" "),
                        file.display()
                    )?
                }
            }
            let void: Vec<String> = pal.void_metatiles().iter().map(u16::to_string).collect();
            match void.is_empty() {
                true => writeln!(out, "void (all-black) metatiles: none")?,
                false => writeln!(
                    out,
                    "void (all-black) metatiles, for an interior's border: {}",
                    void.join(" ")
                )?,
            }
            if p.labels {
                out.push_str("behaviour codes (right of a cell's id, above it):\n");
                out.push_str(&overlay::palette_legend(&pal, &catalog, primary, secondary));
            }
        }
        [cmd, map, x, y, w, h, name, flags @ ..] if cmd == "stamp-from" => {
            let (mut elevation, mut force) = (true, false);
            for f in flags {
                match f.as_str() {
                    "--no-elevation" => elevation = false,
                    "--force" => force = true,
                    _ => bail!(USAGE),
                }
            }
            let doc = games.load(map)?;
            let st = Stamp::capture(
                &doc,
                number(x, "x")?,
                number(y, "y")?,
                number(w, "w")?,
                number(h, "h")?,
                elevation,
            )?;
            games.save_stamp(name, &st, force)?;
            writeln!(
                out,
                "stamp {name}: {}x{} cells of {map} at ({x}, {y}): {}",
                st.width,
                st.height,
                games.stamp_path(name).display()
            )?;
        }
        _ => bail!(USAGE),
    }
    Ok(out)
}

fn report(out: &mut String, map: &str, issues: &[Issue]) {
    for i in issues {
        let _ = writeln!(out, "{map}: {i}");
    }
}
