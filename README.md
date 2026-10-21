# pokemon-rust

A Rust GBA engine and toolchain for a full rework of Pokémon Émeraude (FR).
Design: `docs/plans/2026-10-21-emerald-rust-rework-design.md`.

## Requirements
- Rust nightly with `rust-src`
- `cargo install agb-gbafix --version 0.25.0 --locked`
- cmake (to build the vendored libmgba)
- Your ROM goes in `rom/` (git-ignored)

## Commands
- `cargo test --workspace`: unit tests
- `cargo xtask rom`: build `target/game.gba`
- `cargo xtask parity`: check that the ROM (in mGBA) and the native build agree
- `cargo xtask shot`: dump ROM and native screenshots to `target/`

## Layout
- `crates/engine`: game-agnostic `no_std` core
- `crates/content`: the game (data and scripts)
- `crates/game-gba`: GBA shell (separate workspace, thumbv4t target)
- `crates/game-native`: host shell (software renderer)
- `crates/emu`: embedded mGBA
- `xtask`: build and verification tasks

Your ROM goes in `rom/` (git-ignored). Original assets are never committed.
