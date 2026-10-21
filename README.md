# pokemon-rust

A Rust GBA engine and toolchain for a full rework of Pokémon Émeraude (FR).
Design: `docs/plans/2026-10-21-emerald-rust-rework-design.md`.

## Requirements
- Rust nightly with `rust-src`
- `cargo install agb-gbafix --version 0.25.0 --locked`
- cmake and libclang (Xcode command-line tools) to build the vendored libmgba

## Commands
- `cargo test --workspace`: unit tests plus ROM-backed tests that need both ROMs and the decomp (see Tests)
- `cargo xtask rom`: build `target/game.gba`
- `cargo xtask parity`: check that the ROM (in mGBA) and the native build agree
- `cargo xtask shot`: dump ROM and native screenshots to `target/`
- `cargo xtask extract [--rom <path>]`: export the ROM to `content/extracted/` (see Extract)

## Tests
`cargo test --workspace` runs unit tests and the ROM-backed tests in `crates/extract/tests/`, which
read both ROMs and the decomp. They fail if these are missing:

| Variable      | Default                 | What                                           |
|---------------|-------------------------|------------------------------------------------|
| `PKR_ROM_FR`  | `rom/emeraude-fr.gba`   | Pokémon Émeraude (BPEF) ROM                    |
| `PKR_ROM_US`  | `../emerald.gba`        | Pokémon Emerald (BPEE) ROM                     |
| `PKR_DECOMP`  | `../pokeemerald`        | a checkout of the pret/pokeemerald decomp      |

Defaults are relative to this checkout's root (so `../emerald.gba` sits next to it). Set the variables to point elsewhere, e.g.
`PKR_ROM_US=~/roms/emerald.gba cargo test --workspace`. `cargo test -p extract --lib` runs only
the extractor's unit tests, which need none of these.

## Extract
`cargo xtask extract` reads your French ROM into editable, git-ignored files.

1. Place the ROM at `rom/emeraude-fr.gba` (or pass `--rom <path>`; BPEE also works).
2. Run `cargo xtask extract`. It exports into `content/.extracted.tmp/` and, only if that succeeds,
   replaces `content/extracted/` with it (a failed run keeps the previous output). It prints a
   summary (maps, tilesets, sprite ids, text count, any undecoded script instructions).
3. See the output in `content/extracted/`:
   - `manifest.ron`: game code, ROM sha1, every ROM offset used, sha1 of every other file
   - `maps/<Map>/map.ron`: header, layout blocks (one map row per line), events, connections,
     script and text refs (Littleroot, Oldale, Route 101)
   - `tilesets/<offset>/`: `tiles.png` plus palettes, metatiles and attributes as RON
   - `sprites/<id>.png` + `.ron`: overworld sprites used by those maps, plus Brendan and May
   - `font/latin_normal.png` + `.ron`: the Latin font and its glyph widths
   - `text/fr.ron` (`text/en.ron` for a BPEE ROM): every text the maps' scripts show, keyed
     `<Map>.<hex offset>`, as raw strings in the decomp `.string` syntax (`\p`, `\n`, `{PLAYER}`)
   - `previews/<Map>.png`: map renders

The output is deterministic: two runs on the same ROM give identical files.

## Layout
- `crates/engine`: game-agnostic `no_std` core
- `crates/content`: the game (data and scripts)
- `crates/game-gba`: GBA shell (separate workspace, thumbv4t target)
- `crates/game-native`: host shell (software renderer)
- `crates/emu`: embedded mGBA
- `crates/extract`: reads the ROM into `content/extracted/`
- `xtask`: build and verification tasks

Your ROM goes in `rom/` (git-ignored). Original assets are never committed.
