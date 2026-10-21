# pokemon-rust

A Rust GBA engine and toolchain for a full rework of Pokémon Émeraude (FR).
Design: `docs/plans/2026-10-21-emerald-rust-rework-design.md`.

## Requirements
- Rust nightly with `rust-src`
- `cargo install agb-gbafix --version 0.25.0 --locked`
- cmake and libclang (Xcode command-line tools) to build the vendored libmgba

## Commands
- `cargo test --workspace`: unit tests plus ROM-backed tests that need both ROMs and the decomp (see Tests)
- `cargo xtask rom`: build `target/game.gba` (fails if it exceeds the 32 MiB GBA limit)
- `cargo xtask parity`: play the scripted route on the ROM (in mGBA) and natively; the World hash
  must match on every frame and the picture at every checkpoint
- `cargo xtask shot`: play the same route and write ROM and native PNGs of each named moment to
  `target/shots/<moment>-{rom,native}.png`
- `cargo xtask extract [--rom <path>]`: export the ROM to `content/extracted/` (see Extract)

## Play
1. Extract your ROM (see Extract), then `cargo xtask rom`.
2. Open `target/game.gba` in any GBA emulator (mGBA, …) or on a flash cart. Or play natively:
   `cargo run -p game-native --features window --bin play` (arrows, X = A, Z = B,
   Enter = Start, Backspace = Select, Escape quits).

The slice starts in Littleroot Town outside Brendan's house, after receiving the starter and
before meeting May on Route 103 (`crates/content/src/new_game.rs` lists the story vars and flags).
Littleroot, Route 101, Oldale and their interiors are walkable; doors, doormats, connections,
NPCs, signs and French dialogue work. Battles, menus, running shoes and sound are not there yet.

Without an extracted dir everything builds against a small built-in map (stub mode,
`cfg(pkr_stub)`; force it with `PKR_EXTRACTED_DIR=/nonexistent`).

## Tests
`cargo test --workspace` runs unit tests and the ROM-backed tests in `crates/extract/tests/`, which
read both ROMs and the decomp. They fail if these are missing:

| Variable      | Default                 | What                                           |
|---------------|-------------------------|------------------------------------------------|
| `PKR_ROM_FR`  | `rom/emeraude-fr.gba`   | Pokémon Émeraude (BPEF) ROM                    |
| `PKR_ROM_US`  | `../emerald.gba`        | Pokémon Emerald (BPEE) ROM                     |
| `PKR_DECOMP`  | `../pokeemerald`        | a checkout of the pret/pokeemerald decomp      |

Parity (`cargo xtask parity`, `crates/emu/tests/parity.rs`) replays the route of
`crates/game-native/src/scenario.rs`: with extracted data it walks out of the start, into
Brendan's house and back, across Route 101 to Oldale, talks to an NPC and enters the Pokémon
Center (about 1800 frames); in stub mode it walks around the stub map. The ROM must not apply
any frame late while something is visible (`DebugRegion::late`); late applies on a black screen
(map loads) are only counted (`late_black`).

`crates/engine/src/world/tests.rs` pins two World hashes on a synthetic game (`golden_hashes`):
`0x27378ceb8a110230` after `World::new` and `0x4b5bc94f9ed570e4` after 25 frames each of Right,
A, nothing and Down. They change whenever the hashed state, its order or the frame logic
changes; update them only for an intended change.

Defaults are relative to this checkout's root (so `../emerald.gba` sits next to it). Set the variables to point elsewhere, e.g.
`PKR_ROM_US=~/roms/emerald.gba cargo test --workspace`. `cargo test -p extract --lib` runs only
the extractor's unit tests, which need none of these.

## Extract
`cargo xtask extract` reads your French ROM into editable, git-ignored files.

1. Place the ROM at `rom/emeraude-fr.gba` (or pass `--rom <path>`; BPEE also works).
2. Run `cargo xtask extract`. It exports into `content/.extracted.tmp/` and, only if that succeeds,
   replaces `content/extracted/` with it (a failed run keeps the previous output). It prints a
   summary (maps, tilesets, sprites, skipped graphics, any undecoded script instructions).
3. See the output in `content/extracted/` (format v2, described in `crates/extract/src/schema.rs`).
   Everything is named with decomp identifiers (`LittlerootTown`, `General`, `FLAG_HIDE_…`,
   `OBJ_EVENT_GFX_…`); ROM offsets only appear in the manifest:
   - `manifest.ron`: format, game code, language, ROM sha1, the exported maps, tilesets and
     sprites, the ROM offset of everything read (`provenance`), sha1 of every other file
   - `maps/<Map>.ron`: header, layout (tileset names, blocks one map row per line), objects,
     warps, coord and bg events, connections and map scripts. Script references use the root
     names of `schema.rs` (`LittlerootTown_Obj1`, `Route101_Coord0`, `OldaleTown_OnTransition`).
     The 15 maps: Littleroot Town, Routes 101–103, Oldale Town, and the Littleroot and Oldale
     interiors
   - `tilesets/<Name>.png` + `.ron`: tiles (with the tileset's first palette), palettes,
     metatiles and attributes
   - `sprites/<Gfx>.png` + `.ron`: overworld sprites the maps use (also through
     `VAR_OBJ_GFX_ID_*`), plus Brendan and May
   - `font/latin_normal.png` + `.ron`: the Latin font and its glyph widths
   - `window/message_box.png` + `.ron`: the dialogue window frame, its palette and text colors
   - `previews/<Map>.png`: map renders

   Texts (`text/<lang>.ron`) and generated scripts (`scripts/`) are not written yet.

The output is deterministic: two runs on the same ROM give identical files. The decomp
identifier tables live in `crates/extract/src/names/generated.rs`, checked against the decomp by
`tests/names.rs` (`PKR_BLESS=1 cargo test -p extract --test names` regenerates them).

## Layout
- `crates/engine`: game-agnostic `no_std` core
- `crates/content`: the game (data and scripts)
- `crates/game-gba`: GBA shell (separate workspace, thumbv4t target)
- `crates/game-native`: host shell (software renderer)
- `crates/emu`: embedded mGBA
- `crates/extract`: reads the ROM into `content/extracted/`
- `xtask`: build and verification tasks

Your ROM goes in `rom/` (git-ignored). Original assets are never committed.
