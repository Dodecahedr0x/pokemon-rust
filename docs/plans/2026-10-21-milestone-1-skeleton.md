# Milestone 1 — Skeleton Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** A Cargo workspace where one deterministic `no_std` engine runs both as a GBA ROM (agb) and natively, renders the same scrolling test tilemap on both, and an automated parity check proves the ROM (in embedded mGBA) and the native build reach the same `World` hash and the same pixels for the same inputs.

**Architecture:** `engine` (game-agnostic, `no_std`) owns `World`, `Buttons`, RNG, hashing, the scene/virtual-PPU types and the debug-region layout. `content` (`no_std`) supplies the test scene. `game-gba` is a separate Cargo project (own `.cargo/config.toml` for `thumbv4t-none-eabi` + `build-std`) that writes VRAM/registers directly and mirrors engine state into a magic-tagged debug region in EWRAM. `game-native` renders the scene in software. `emu` wraps the `mgba` crate, drives the ROM and reads the debug region. `xtask` builds the ROM and runs parity/screenshot jobs.

**Tech Stack:** Rust nightly (edition 2024), agb 0.25.0, agb-gbafix 0.25.0, mgba 0.1.0 (vendored libmgba, needs cmake + libclang), anyhow.

**Design reference:** `docs/plans/2026-10-21-emerald-rust-rework-design.md`

**Ground rules for the implementer:**
- Determinism: no floats, no time, no `HashMap` iteration in `engine`.
- `engine` and `content` must compile for both the host and `thumbv4t-none-eabi`. Use `#![cfg_attr(not(test), no_std)]`.
- Commit after every task. Run `cargo fmt` before each commit.
- GBA key bit order (used everywhere, identical to mGBA's `setKeys` mask): A=0, B=1, Select=2, Start=3, Right=4, Left=5, Up=6, Down=7, R=8, L=9.
- Colors are RGB555 `u16`: `r | g << 5 | b << 10`, each channel 0..=31.

---

### Task 0: Workspace scaffold and tools

**Files:**
- Create: `Cargo.toml`
- Create: `.cargo/config.toml`
- Modify: `.gitignore`
- Create: `crates/engine/Cargo.toml`, `crates/engine/src/lib.rs`

**Step 1: Install the GBA ELF→ROM fixer**

Run: `cargo install agb-gbafix --version 0.25.0 --locked`
Expected: `Installed package agb-gbafix v0.25.0`. Check: `agb-gbafix --help` prints usage.

**Step 2: Verify nightly has `rust-src`**

Run: `rustup component list --installed --toolchain nightly | grep rust-src`
Expected: `rust-src`. If missing: `rustup component add rust-src --toolchain nightly`.

**Step 3: Write root `Cargo.toml`**

```toml
[workspace]
resolver = "3"
members = ["crates/engine", "crates/content", "crates/game-native", "crates/emu", "xtask"]
exclude = ["crates/game-gba"]

[workspace.package]
edition = "2024"
version = "0.1.0"
publish = false

[workspace.dependencies]
engine = { path = "crates/engine" }
content = { path = "crates/content" }
game-native = { path = "crates/game-native" }
emu = { path = "crates/emu" }
anyhow = "1"
mgba = "0.1.0"
```

**Step 4: Write `.cargo/config.toml`**

```toml
[alias]
xtask = "run -p xtask --"
```

**Step 5: Replace `.gitignore`**

```
/rom/
*.zip
*.gba
*.sav
target/
explore.db
.DS_Store
```

**Step 6: Create a minimal `engine` crate so the workspace resolves**

`crates/engine/Cargo.toml`:
```toml
[package]
name = "engine"
edition.workspace = true
version.workspace = true
publish.workspace = true

[dependencies]
```

`crates/engine/src/lib.rs`:
```rust
#![cfg_attr(not(test), no_std)]
//! Game-agnostic engine core. Knows nothing about Hoenn.
```

Temporarily comment out the other members in the root `Cargo.toml` (`members = ["crates/engine"]`). Each later task re-adds its crate.

**Step 7: Verify**

Run: `cargo build -p engine`
Expected: `Finished`.

**Step 8: Commit**

```bash
git add Cargo.toml .cargo/config.toml .gitignore crates/engine
git commit -m "chore: workspace scaffold with engine crate"
```

---

### Task 1: `Buttons` input type

**Files:**
- Create: `crates/engine/src/input.rs`
- Modify: `crates/engine/src/lib.rs`

**Step 1: Write the failing test** in `crates/engine/src/input.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_match_gba_keyinput_order() {
        assert_eq!(Buttons::A.0, 1 << 0);
        assert_eq!(Buttons::RIGHT.0, 1 << 4);
        assert_eq!(Buttons::DOWN.0, 1 << 7);
        assert_eq!(Buttons::L.0, 1 << 9);
    }

    #[test]
    fn contains_and_union() {
        let b = Buttons::RIGHT | Buttons::UP;
        assert!(b.contains(Buttons::RIGHT));
        assert!(b.contains(Buttons::UP));
        assert!(!b.contains(Buttons::LEFT));
        assert_eq!(Buttons::NONE.0, 0);
    }

    #[test]
    fn from_raw_masks_unused_bits() {
        assert_eq!(Buttons::from_raw(0xFFFF).0, 0x03FF);
    }
}
```

Add to `lib.rs`: `pub mod input;` and `pub use input::Buttons;`

**Step 2: Run to verify it fails**

Run: `cargo test -p engine input`
Expected: FAIL to compile with `cannot find type Buttons`.

**Step 3: Implement** (top of `input.rs`):

```rust
/// Pressed buttons, one bit per key, in GBA KEYINPUT order (active-high).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Buttons(pub u16);

impl Buttons {
    pub const NONE: Self = Self(0);
    pub const A: Self = Self(1 << 0);
    pub const B: Self = Self(1 << 1);
    pub const SELECT: Self = Self(1 << 2);
    pub const START: Self = Self(1 << 3);
    pub const RIGHT: Self = Self(1 << 4);
    pub const LEFT: Self = Self(1 << 5);
    pub const UP: Self = Self(1 << 6);
    pub const DOWN: Self = Self(1 << 7);
    pub const R: Self = Self(1 << 8);
    pub const L: Self = Self(1 << 9);
    const MASK: u16 = 0x03FF;

    pub const fn from_raw(bits: u16) -> Self {
        Self(bits & Self::MASK)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for Buttons {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p engine input`
Expected: 3 passed.

**Step 5: Commit**

```bash
git add crates/engine
git commit -m "feat(engine): Buttons input type"
```

---

### Task 2: Deterministic RNG and FNV-1a hasher

**Files:**
- Create: `crates/engine/src/rng.rs`
- Create: `crates/engine/src/hash.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod rng; pub mod hash; pub use rng::Rng;`)

**Step 1: Write failing tests**

`rng.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xorshift32_reference_value() {
        let mut r = Rng::new(1);
        assert_eq!(r.next_u32(), 270369);
    }

    #[test]
    fn zero_seed_is_replaced() {
        let mut r = Rng::new(0);
        assert_ne!(r.next_u32(), 0);
    }

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
```

`hash.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_offset_basis() {
        assert_eq!(Fnv64::new().finish(), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn known_vector_a() {
        // FNV-1a 64 of b"a"
        let mut h = Fnv64::new();
        h.write(b"a");
        assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn integer_writes_are_little_endian_bytes() {
        let mut a = Fnv64::new();
        a.write_u32(0x0403_0201);
        let mut b = Fnv64::new();
        b.write(&[1, 2, 3, 4]);
        assert_eq!(a.finish(), b.finish());
    }
}
```

**Step 2: Run to verify failure**

Run: `cargo test -p engine`
Expected: compile errors for `Rng` / `Fnv64`.

**Step 3: Implement**

`rng.rs` (above the tests):
```rust
/// xorshift32. The only randomness source in the game; lives inside `World`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rng(u32);

impl Rng {
    pub const fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x1234_5678 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    pub const fn state(self) -> u32 {
        self.0
    }
}
```

`hash.rs`:
```rust
/// FNV-1a 64-bit. Used for World hashes compared across GBA and native.
pub struct Fnv64(u64);

impl Fnv64 {
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    pub fn write_u16(&mut self, v: u16) {
        self.write(&v.to_le_bytes());
    }

    pub fn write_u32(&mut self, v: u32) {
        self.write(&v.to_le_bytes());
    }

    pub const fn finish(&self) -> u64 {
        self.0
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p engine`
Expected: all pass.

**Step 5: Commit**

```bash
git add crates/engine
git commit -m "feat(engine): xorshift RNG and FNV-1a hasher"
```

---

### Task 3: `World` and the frame step

M1 behavior: the D-pad scrolls background 0 by 1 px per frame. The RNG advances once per frame. `step` returns a `FrameOut` (the M1 virtual-PPU output: BG0 scroll).

**Files:**
- Create: `crates/engine/src/world.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod world; pub use world::{World, FrameOut};`)

**Step 1: Write failing tests** in `world.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Buttons;

    #[test]
    fn new_world_starts_at_frame_zero() {
        let w = World::new();
        assert_eq!(w.frame(), 0);
        assert_eq!(w.scroll(), (0, 0));
    }

    #[test]
    fn dpad_scrolls_and_wraps() {
        let mut w = World::new();
        let out = w.step(Buttons::RIGHT | Buttons::DOWN);
        assert_eq!(out.bg0_scroll, (1, 1));
        let out = w.step(Buttons::LEFT);
        assert_eq!(out.bg0_scroll, (0, 1));
        let out = w.step(Buttons::LEFT | Buttons::UP);
        assert_eq!(out.bg0_scroll, (u16::MAX, 0));
        assert_eq!(w.frame(), 3);
    }

    #[test]
    fn same_inputs_same_hash() {
        let inputs = [Buttons::RIGHT, Buttons::NONE, Buttons::A, Buttons::DOWN];
        let (mut a, mut b) = (World::new(), World::new());
        for i in inputs {
            a.step(i);
            b.step(i);
        }
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn different_inputs_different_hash() {
        let (mut a, mut b) = (World::new(), World::new());
        a.step(Buttons::RIGHT);
        b.step(Buttons::LEFT);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn rng_advances_every_frame() {
        let mut a = World::new();
        let h0 = a.hash();
        a.step(Buttons::NONE);
        assert_ne!(a.hash(), h0);
    }
}
```

**Step 2: Run to verify failure**

Run: `cargo test -p engine world`
Expected: compile error, `World` not found.

**Step 3: Implement**

```rust
use crate::{hash::Fnv64, Buttons, Rng};

/// Virtual-PPU output of one frame. Grows into full layer/sprite commands later.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FrameOut {
    pub bg0_scroll: (u16, u16),
}

/// All mutable game state. Must stay deterministic: same state + same input => same next state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct World {
    frame: u32,
    scroll_x: u16,
    scroll_y: u16,
    rng: Rng,
}

impl World {
    pub const SEED: u32 = 0x5EED_0001;

    pub const fn new() -> Self {
        Self { frame: 0, scroll_x: 0, scroll_y: 0, rng: Rng::new(Self::SEED) }
    }

    pub fn step(&mut self, keys: Buttons) -> FrameOut {
        if keys.contains(Buttons::RIGHT) {
            self.scroll_x = self.scroll_x.wrapping_add(1);
        }
        if keys.contains(Buttons::LEFT) {
            self.scroll_x = self.scroll_x.wrapping_sub(1);
        }
        if keys.contains(Buttons::DOWN) {
            self.scroll_y = self.scroll_y.wrapping_add(1);
        }
        if keys.contains(Buttons::UP) {
            self.scroll_y = self.scroll_y.wrapping_sub(1);
        }
        self.rng.next_u32();
        self.frame = self.frame.wrapping_add(1);
        self.frame_out()
    }

    pub const fn frame_out(&self) -> FrameOut {
        FrameOut { bg0_scroll: (self.scroll_x, self.scroll_y) }
    }

    pub const fn frame(&self) -> u32 {
        self.frame
    }

    pub const fn scroll(&self) -> (u16, u16) {
        (self.scroll_x, self.scroll_y)
    }

    /// Field order is part of the parity contract; append new fields at the end.
    pub fn hash(&self) -> u64 {
        let mut h = Fnv64::new();
        h.write_u32(self.frame);
        h.write_u16(self.scroll_x);
        h.write_u16(self.scroll_y);
        h.write_u32(self.rng.state());
        h.finish()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p engine`
Expected: all pass.

**Step 5: Commit**

```bash
git add crates/engine
git commit -m "feat(engine): World with deterministic step and hash"
```

---

### Task 4: Scene types and the `Content` trait

**Files:**
- Create: `crates/engine/src/scene.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod scene; pub use scene::{Content, Scene, Tile};`)

**Step 1: Write failing test** in `scene.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_pixel_reads_low_nibble_first() {
        let t: Tile = [0x0000_0021, 0, 0, 0, 0, 0, 0, 0x7000_0000];
        assert_eq!(tile_pixel(&t, 0, 0), 1);
        assert_eq!(tile_pixel(&t, 1, 0), 2);
        assert_eq!(tile_pixel(&t, 2, 0), 0);
        assert_eq!(tile_pixel(&t, 7, 7), 7);
    }

    #[test]
    fn map_entry_tile_index_ignores_flags() {
        assert_eq!(map_tile_index(0xF3FF), 0x3FF);
    }
}
```

**Step 2: Run to verify failure**

Run: `cargo test -p engine scene`
Expected: compile error.

**Step 3: Implement**

```rust
/// 8x8 4bpp tile: one u32 per row, pixel x stored in nibble x (low nibble first), GBA layout.
pub type Tile = [u32; 8];

/// Map width/height in tiles (one 256x256 px GBA screenblock).
pub const MAP_TILES: usize = 32;

/// A single-layer scene. M1 ignores palette banks and flip bits.
pub struct Scene {
    /// RGB555; index 0 is also the backdrop.
    pub palette: [u16; 16],
    pub tiles: &'static [Tile],
    /// GBA text-BG map entries, row-major, 32x32.
    pub map: &'static [u16; MAP_TILES * MAP_TILES],
}

/// The only contract between the engine and the game data.
pub trait Content {
    fn scene(&self) -> &Scene;
}

pub const fn tile_pixel(tile: &Tile, x: usize, y: usize) -> u8 {
    ((tile[y] >> (x * 4)) & 0xF) as u8
}

pub const fn map_tile_index(entry: u16) -> usize {
    (entry & 0x03FF) as usize
}
```

**Step 4: Run tests**

Run: `cargo test -p engine`
Expected: all pass.

**Step 5: Commit**

```bash
git add crates/engine
git commit -m "feat(engine): Scene, Tile and Content trait"
```

---

### Task 5: Debug-region layout (shared by GBA writer and emulator reader)

**Files:**
- Create: `crates/engine/src/debug.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod debug;`)

**Step 1: Write failing test** in `debug.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    #[test]
    fn layout_is_stable() {
        assert_eq!(offset_of!(DebugRegion, magic), 0);
        assert_eq!(offset_of!(DebugRegion, frame), OFF_FRAME as usize);
        assert_eq!(offset_of!(DebugRegion, hash), OFF_HASH as usize);
        assert_eq!(offset_of!(DebugRegion, inputs), OFF_INPUTS as usize);
        assert_eq!(size_of::<DebugRegion>(), 152);
    }

    #[test]
    fn input_slot_for_frame() {
        assert_eq!(input_slot(1), 0);
        assert_eq!(input_slot(64), 63);
        assert_eq!(input_slot(65), 0);
    }
}
```

**Step 2: Run to verify failure**

Run: `cargo test -p engine debug`
Expected: compile error.

**Step 3: Implement**

```rust
//! Debug mirror the GBA build keeps in EWRAM so the studio/emulator can follow the engine.
//! Write order on the GBA: inputs slot, then hash, then frame (frame last = commit).

pub const MAGIC: [u8; 8] = *b"PKRDBG01";
pub const INPUT_RING: usize = 64;
pub const OFF_FRAME: u32 = 8;
pub const OFF_HASH: u32 = 16;
pub const OFF_INPUTS: u32 = 24;

#[repr(C)]
pub struct DebugRegion {
    pub magic: [u8; 8],
    /// Number of engine steps completed.
    pub frame: u32,
    pub _pad: u32,
    /// `World::hash()` after step `frame`.
    pub hash: u64,
    /// Input consumed by step `f` lives at `input_slot(f)`.
    pub inputs: [u16; INPUT_RING],
}

impl DebugRegion {
    pub const fn new() -> Self {
        Self { magic: MAGIC, frame: 0, _pad: 0, hash: 0, inputs: [0; INPUT_RING] }
    }
}

/// Ring index for the input consumed by the step that produced engine frame `frame` (>= 1).
pub const fn input_slot(frame: u32) -> usize {
    ((frame.wrapping_sub(1)) as usize) % INPUT_RING
}
```

**Step 4: Run tests**

Run: `cargo test -p engine`
Expected: all pass.

**Step 5: Verify engine builds for the GBA target too**

Run: `cargo +nightly build -p engine --target thumbv4t-none-eabi -Zbuild-std=core`
Expected: `Finished`. (If it fails, the crate is pulling in `std` outside `cfg(test)`. Fix that before continuing.)

**Step 6: Commit**

```bash
git add crates/engine
git commit -m "feat(engine): shared debug-region layout"
```

---

### Task 6: `content` crate with the test scene

**Files:**
- Create: `crates/content/Cargo.toml`, `crates/content/src/lib.rs`
- Modify: root `Cargo.toml` members (add `"crates/content"`)

**Step 1: Create the crate manifest**

```toml
[package]
name = "content"
edition.workspace = true
version.workspace = true
publish.workspace = true

[dependencies]
engine.workspace = true
```

**Step 2: Write failing tests** in `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use engine::Content;

    #[test]
    fn every_map_entry_points_at_a_tile() {
        let s = Emerald.scene();
        assert!(s.map.iter().all(|&e| engine::scene::map_tile_index(e) < s.tiles.len()));
    }

    #[test]
    fn palette_is_rgb555() {
        assert!(Emerald.scene().palette.iter().all(|&c| c <= 0x7FFF));
    }

    #[test]
    fn map_has_border_and_checker() {
        let m = Emerald.scene().map;
        assert_eq!(m[0], T_BORDER);
        assert_eq!(m[32 + 1], T_GRASS);
        assert_eq!(m[32 + 4], T_CHECKER);
        assert_eq!(m[3 * 32 + 3 + 8], T_DOT);
    }
}
```

**Step 3: Run to verify failure**

Run: `cargo test -p content`
Expected: compile error, `Emerald` not found.

**Step 4: Implement** (above the tests):

```rust
#![cfg_attr(not(test), no_std)]
//! The game: all Hoenn-specific data and scripts. Depends on `engine`, never the reverse.

use engine::{Content, Scene, Tile};

pub const T_GRASS: u16 = 0;
pub const T_CHECKER: u16 = 1;
pub const T_BORDER: u16 = 2;
pub const T_DOT: u16 = 3;

const fn rgb(r: u16, g: u16, b: u16) -> u16 {
    r | (g << 5) | (b << 10)
}

const PALETTE: [u16; 16] = {
    let mut p = [0u16; 16];
    p[0] = rgb(0, 0, 0);
    p[1] = rgb(8, 24, 8);
    p[2] = rgb(14, 28, 10);
    p[3] = rgb(4, 8, 16);
    p[4] = rgb(31, 28, 4);
    p
};

static TILES: [Tile; 4] = [
    [0x1111_1111; 8],
    [0x2121_2121, 0x1212_1212, 0x2121_2121, 0x1212_1212, 0x2121_2121, 0x1212_1212, 0x2121_2121, 0x1212_1212],
    [0x3333_3333; 8],
    [0x1111_1111, 0x1111_1111, 0x1111_1111, 0x1114_4111, 0x1114_4111, 0x1111_1111, 0x1111_1111, 0x1111_1111],
];

const fn build_map() -> [u16; 1024] {
    let mut m = [0u16; 1024];
    let mut i = 0;
    while i < 1024 {
        let (x, y) = (i % 32, i / 32);
        m[i] = if x == 0 || y == 0 || x == 31 || y == 31 {
            T_BORDER
        } else if x % 8 == 3 && y % 8 == 3 {
            T_DOT
        } else if (x / 4 + y / 4) % 2 == 0 {
            T_GRASS
        } else {
            T_CHECKER
        };
        i += 1;
    }
    m
}

static MAP: [u16; 1024] = build_map();

static SCENE: Scene = Scene { palette: PALETTE, tiles: &TILES, map: &MAP };

/// The game's content root.
pub struct Emerald;

impl Content for Emerald {
    fn scene(&self) -> &Scene {
        &SCENE
    }
}
```

Note on `m[3*32 + 3 + 8]`: x=11, y=3 means `x % 8 == 3 && y % 8 == 3`, so it's `T_DOT`. `m[32+4]`: x=4, y=1 means `(1 + 0) % 2 == 1`, so it's `T_CHECKER`.

**Step 5: Run tests**

Run: `cargo test -p content`
Expected: 3 passed.

**Step 6: Commit**

```bash
git add Cargo.toml crates/content
git commit -m "feat(content): test scene behind the Content trait"
```

---

### Task 7: `game-native` software renderer and session

**Files:**
- Create: `crates/game-native/Cargo.toml`, `crates/game-native/src/lib.rs`
- Modify: root `Cargo.toml` members (add `"crates/game-native"`)

**Step 1: Manifest**

```toml
[package]
name = "game-native"
edition.workspace = true
version.workspace = true
publish.workspace = true

[dependencies]
engine.workspace = true
content.workspace = true
```

**Step 2: Write failing tests** in `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Buttons, Scene, Tile};

    fn scene() -> Scene {
        let tiles: &'static [Tile] = Box::leak(Box::new([[0x1111_1111u32; 8], [0x2222_2222u32; 8]]));
        let mut map = [0u16; 1024];
        map[1] = 1; // tile column 1, row 0
        let mut palette = [0u16; 16];
        palette[1] = 0x001F;
        palette[2] = 0x7C00;
        Scene { palette, tiles, map: Box::leak(Box::new(map)) }
    }

    #[test]
    fn renders_tiles_at_zero_scroll() {
        let mut fb = vec![0u16; SCREEN_W * SCREEN_H];
        render(&scene(), (0, 0), &mut fb);
        assert_eq!(fb[7], 0x001F);
        assert_eq!(fb[8], 0x7C00);
        assert_eq!(fb[16], 0x001F);
    }

    #[test]
    fn scroll_shifts_and_wraps_at_256() {
        let mut fb = vec![0u16; SCREEN_W * SCREEN_H];
        render(&scene(), (1, 0), &mut fb);
        assert_eq!(fb[7], 0x7C00);
        render(&scene(), (250, 0), &mut fb);
        assert_eq!(fb[0], 0x001F); // map x 250 -> tile 31
        assert_eq!(fb[14], 0x7C00); // map x 264 & 255 = 8 -> tile 1
    }

    #[test]
    fn session_steps_and_renders() {
        let mut s = Session::new(content::Emerald);
        s.step(Buttons::RIGHT);
        assert_eq!(s.world().frame(), 1);
        assert_eq!(s.framebuffer().len(), SCREEN_W * SCREEN_H);
    }
}
```

**Step 3: Run to verify failure**

Run: `cargo test -p game-native`
Expected: compile error.

**Step 4: Implement**

```rust
//! Host shell: runs engine + content natively, renders the virtual PPU in software.

use engine::scene::{map_tile_index, tile_pixel, MAP_TILES};
use engine::{Buttons, Content, FrameOut, Scene, World};

pub const SCREEN_W: usize = 240;
pub const SCREEN_H: usize = 160;
const MAP_PX: usize = MAP_TILES * 8;

/// Render BG0 into an RGB555 framebuffer, matching GBA text-BG semantics (256x256 wrap).
pub fn render(scene: &Scene, scroll: (u16, u16), fb: &mut [u16]) {
    for y in 0..SCREEN_H {
        let py = (y + scroll.1 as usize) % MAP_PX;
        for x in 0..SCREEN_W {
            let px = (x + scroll.0 as usize) % MAP_PX;
            let entry = scene.map[(py / 8) * MAP_TILES + px / 8];
            let tile = &scene.tiles[map_tile_index(entry)];
            let idx = tile_pixel(tile, px % 8, py % 8);
            fb[y * SCREEN_W + x] = scene.palette[idx as usize] & 0x7FFF;
        }
    }
}

/// A native game instance: World + content + last rendered frame.
pub struct Session<C: Content> {
    content: C,
    world: World,
    out: FrameOut,
    fb: Vec<u16>,
}

impl<C: Content> Session<C> {
    pub fn new(content: C) -> Self {
        let world = World::new();
        let out = world.frame_out();
        let mut s = Self { content, world, out, fb: vec![0; SCREEN_W * SCREEN_H] };
        s.redraw();
        s
    }

    pub fn step(&mut self, keys: Buttons) {
        self.out = self.world.step(keys);
        self.redraw();
    }

    fn redraw(&mut self) {
        render(self.content.scene(), self.out.bg0_scroll, &mut self.fb);
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn framebuffer(&self) -> &[u16] {
        &self.fb
    }
}
```

Note: `scroll as usize` then `% 256` is equivalent to the GBA's 9-bit scroll register on a 256 px map, because 256 divides 512.

**Step 5: Run tests**

Run: `cargo test -p game-native`
Expected: 3 passed.

**Step 6: Commit**

```bash
git add Cargo.toml crates/game-native
git commit -m "feat(native): software BG renderer and Session"
```

---

### Task 8: `game-gba` ROM

This crate is its own Cargo workspace (excluded from the root) because it needs the `thumbv4t-none-eabi` target and `build-std`.

**Files:**
- Create: `crates/game-gba/Cargo.toml`
- Create: `crates/game-gba/rust-toolchain.toml`
- Create: `crates/game-gba/.cargo/config.toml`
- Create: `crates/game-gba/src/main.rs`

**Step 1: Manifest** (the empty `[workspace]` makes it a standalone root)

```toml
[package]
name = "game-gba"
version = "0.1.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
agb = "0.25.0"
engine = { path = "../engine" }
content = { path = "../content" }

[profile.dev]
opt-level = 3
debug = true

[profile.release]
opt-level = 3
lto = "fat"
debug = true
```

**Step 2: Toolchain and target config** (copied from the agb 0.25 template, without the `mgba-qt` runner)

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "nightly"
components = ["rust-src", "clippy", "rustfmt"]
```

`.cargo/config.toml`:
```toml
[unstable]
build-std = ["core", "alloc"]
build-std-features = ["compiler-builtins-mem"]

[build]
target = "thumbv4t-none-eabi"

[target.thumbv4t-none-eabi]
rustflags = ["-Clink-arg=-Tgba.ld", "-Ctarget-cpu=arm7tdmi", "-Cforce-frame-pointers=yes"]
```

**Step 3: `src/main.rs`**

```rust
#![no_std]
#![no_main]

extern crate alloc;

use content::Emerald;
use engine::debug::{input_slot, DebugRegion};
use engine::scene::MAP_TILES;
use engine::{Buttons, Content, World};

const DISPCNT: *mut u16 = 0x0400_0000 as *mut u16;
const BG0CNT: *mut u16 = 0x0400_0008 as *mut u16;
const BG0HOFS: *mut u16 = 0x0400_0010 as *mut u16;
const BG0VOFS: *mut u16 = 0x0400_0012 as *mut u16;
const KEYINPUT: *const u16 = 0x0400_0130 as *const u16;
const BG_PALETTE: *mut u16 = 0x0500_0000 as *mut u16;
const CHARBLOCK0: *mut u32 = 0x0600_0000 as *mut u32;
const SCREENBLOCK31: *mut u16 = 0x0600_F800 as *mut u16;

const DISPCNT_FORCED_BLANK: u16 = 1 << 7;
const DISPCNT_MODE0_BG0: u16 = 1 << 8;
const BG0CNT_CB0_SB31_4BPP_256: u16 = 31 << 8;

// Initialised static => .data => EWRAM (agb gba.ld). The emulator finds it by MAGIC.
#[used]
#[unsafe(no_mangle)]
static mut PKR_DEBUG: DebugRegion = DebugRegion::new();

fn upload_scene(content: &impl Content) {
    let scene = content.scene();
    unsafe {
        DISPCNT.write_volatile(DISPCNT_FORCED_BLANK);
        for (i, &c) in scene.palette.iter().enumerate() {
            BG_PALETTE.add(i).write_volatile(c);
        }
        for (t, tile) in scene.tiles.iter().enumerate() {
            for (r, &row) in tile.iter().enumerate() {
                CHARBLOCK0.add(t * 8 + r).write_volatile(row);
            }
        }
        for i in 0..MAP_TILES * MAP_TILES {
            SCREENBLOCK31.add(i).write_volatile(scene.map[i]);
        }
        BG0CNT.write_volatile(BG0CNT_CB0_SB31_4BPP_256);
        DISPCNT.write_volatile(DISPCNT_MODE0_BG0);
    }
}

fn read_keys() -> Buttons {
    Buttons::from_raw(!unsafe { KEYINPUT.read_volatile() })
}

fn publish_debug(world: &World, keys: Buttons) {
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).inputs[input_slot(world.frame())]).write_volatile(keys.0);
        (&raw mut (*d).hash).write_volatile(world.hash());
        (&raw mut (*d).frame).write_volatile(world.frame());
    }
}

#[agb::entry]
fn main(_gba: agb::Gba) -> ! {
    let content = Emerald;
    upload_scene(&content);
    let vblank = agb::interrupt::VBlank::get();
    let mut world = World::new();
    let mut out = world.frame_out();
    loop {
        vblank.wait_for_vblank();
        unsafe {
            BG0HOFS.write_volatile(out.bg0_scroll.0 & 0x1FF);
            BG0VOFS.write_volatile(out.bg0_scroll.1 & 0x1FF);
        }
        let keys = read_keys();
        out = world.step(keys);
        publish_debug(&world, keys);
    }
}
```

**Step 4: Build**

Run: `cd crates/game-gba && cargo build --release`
Expected: `Finished`, producing `crates/game-gba/target/thumbv4t-none-eabi/release/game-gba` (an ELF).

**Step 5: Convert to ROM**

Run (from repo root): `agb-gbafix crates/game-gba/target/thumbv4t-none-eabi/release/game-gba -o target/game.gba -t PKRUST`
Expected: `target/game.gba` exists. Check: `ls -la target/game.gba`.

**Step 6: Commit**

```bash
git add crates/game-gba
git commit -m "feat(gba): agb ROM shell with direct VRAM upload and debug region"
```

---

### Task 9: `xtask rom`

**Files:**
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs`
- Modify: root `Cargo.toml` members (add `"xtask"`)

**Step 1: Manifest**

```toml
[package]
name = "xtask"
edition.workspace = true
version.workspace = true
publish.workspace = true

[dependencies]
anyhow.workspace = true
```

**Step 2: `src/main.rs`**

```rust
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
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
    run(Command::new("agb-gbafix").arg(&elf).arg("-o").arg(&out).args(["-t", "PKRUST"]))?;
    println!("ROM: {}", out.display());
    Ok(out)
}

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("rom") => rom().map(|_| ()),
        _ => bail!("usage: cargo xtask <rom>"),
    }
}
```

**Step 3: Verify**

Run: `rm -f target/game.gba && cargo xtask rom`
Expected: ends with `ROM: …/target/game.gba`, and the file exists.

**Step 4: Commit**

```bash
git add Cargo.toml xtask
git commit -m "feat(xtask): rom command"
```

---

### Task 10: `emu` crate (mGBA wrapper)

**Files:**
- Create: `crates/emu/Cargo.toml`, `crates/emu/src/lib.rs`
- Modify: root `Cargo.toml` members (add `"crates/emu"`)

**Step 1: Manifest**

```toml
[package]
name = "emu"
edition.workspace = true
version.workspace = true
publish.workspace = true

[dependencies]
engine.workspace = true
anyhow.workspace = true
mgba.workspace = true

[dev-dependencies]
content.workspace = true
game-native.workspace = true
```

**Step 2: Check that libmgba builds (it's vendored, compiled with cmake, and needs libclang for bindgen)**

Run: `cargo build -p emu` (with an empty `src/lib.rs`)
Expected: `Finished` (the first build takes a few minutes). If bindgen can't find libclang: `export LIBCLANG_PATH=$(xcode-select -p)/Toolchains/XcodeDefault.xctoolchain/usr/lib` and retry.

**Step 3: Write a failing pure-logic test** for the RGB conversion, in `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbgr8_to_rgb555_keeps_top_five_bits() {
        // mGBA expands 5->8 bits; the top 5 bits are always the original channel.
        let r5 = 0x1F_u32;
        let g5 = 0x0A_u32;
        let b5 = 0x03_u32;
        let px = ((r5 * 0x21) >> 2) | (((g5 * 0x21) >> 2) << 8) | (((b5 * 0x21) >> 2) << 16);
        assert_eq!(xbgr8_to_rgb555(px), (r5 | g5 << 5 | b5 << 10) as u16);
        assert_eq!(xbgr8_to_rgb555(r5 << 3 | (g5 << 3) << 8 | (b5 << 3) << 16), (r5 | g5 << 5 | b5 << 10) as u16);
    }
}
```

**Step 4: Run to verify failure**

Run: `cargo test -p emu`
Expected: compile error, `xbgr8_to_rgb555` not found.

**Step 5: Implement**

```rust
//! Embedded mGBA running the real ROM, plus access to the engine debug region.

use anyhow::{anyhow, Context, Result};
use engine::debug::{INPUT_RING, MAGIC, OFF_FRAME, OFF_HASH, OFF_INPUTS};
use engine::Buttons;
use std::path::Path;

const EWRAM_START: u32 = 0x0200_0000;
const EWRAM_SIZE: u32 = 256 * 1024;

pub fn xbgr8_to_rgb555(px: u32) -> u16 {
    let r = (px & 0xFF) >> 3;
    let g = ((px >> 8) & 0xFF) >> 3;
    let b = ((px >> 16) & 0xFF) >> 3;
    (r | g << 5 | b << 10) as u16
}

#[derive(Clone, Debug)]
pub struct DebugSnapshot {
    pub frame: u32,
    pub hash: u64,
    pub inputs: [u16; INPUT_RING],
}

pub struct Emu {
    core: mgba::Core,
    debug_base: Option<u32>,
}

impl Emu {
    pub fn open(rom: &Path) -> Result<Self> {
        let mut core = mgba::Core::new().map_err(|e| anyhow!("mgba core: {e:?}"))?;
        core.load_rom(rom).map_err(|e| anyhow!("load {}: {e:?}", rom.display()))?;
        core.reset().map_err(|e| anyhow!("reset: {e:?}"))?;
        Ok(Self { core, debug_base: None })
    }

    pub fn run_frame(&mut self, keys: Buttons) -> Result<()> {
        self.core.set_keys(keys.0 as u32).map_err(|e| anyhow!("set_keys: {e:?}"))?;
        self.core.run_frame().map_err(|e| anyhow!("run_frame: {e:?}"))
    }

    pub fn framebuffer_rgb555(&self) -> Vec<u16> {
        self.core.video_buffer().iter().map(|&p| xbgr8_to_rgb555(p)).collect()
    }

    fn read16(&mut self, addr: u32) -> u16 {
        // SAFETY: core is initialised and loaded; busRead16 is a pure memory read.
        unsafe {
            let raw = self.core.raw_ptr();
            ((*raw).busRead16.expect("busRead16"))(raw, addr) as u16
        }
    }

    fn read32(&mut self, addr: u32) -> u32 {
        // SAFETY: as above.
        unsafe {
            let raw = self.core.raw_ptr();
            ((*raw).busRead32.expect("busRead32"))(raw, addr)
        }
    }

    fn find_debug_base(&mut self) -> Option<u32> {
        let m0 = u32::from_le_bytes(MAGIC[0..4].try_into().unwrap());
        let m1 = u32::from_le_bytes(MAGIC[4..8].try_into().unwrap());
        (EWRAM_START..EWRAM_START + EWRAM_SIZE)
            .step_by(4)
            .find(|&a| self.read32(a) == m0 && self.read32(a + 4) == m1)
    }

    /// `None` until the ROM has initialised its debug region.
    pub fn debug(&mut self) -> Option<DebugSnapshot> {
        if self.debug_base.is_none() {
            self.debug_base = self.find_debug_base();
        }
        let base = self.debug_base?;
        let frame = self.read32(base + OFF_FRAME);
        let hash = self.read32(base + OFF_HASH) as u64 | (self.read32(base + OFF_HASH + 4) as u64) << 32;
        let mut inputs = [0u16; INPUT_RING];
        for (i, slot) in inputs.iter_mut().enumerate() {
            *slot = self.read16(base + OFF_INPUTS + 2 * i as u32);
        }
        Some(DebugSnapshot { frame, hash, inputs })
    }

    /// Boot until the debug region is visible.
    pub fn boot(&mut self, max_frames: u32) -> Result<DebugSnapshot> {
        for _ in 0..max_frames {
            self.run_frame(Buttons::NONE)?;
            if let Some(s) = self.debug() {
                return Ok(s);
            }
        }
        Err(anyhow!("debug region not found after {max_frames} frames")).context("is the ROM a pokemon-rust build?")
    }
}
```

**Step 6: Run tests**

Run: `cargo test -p emu --lib`
Expected: 1 passed.

**Step 7: Commit**

```bash
git add Cargo.toml crates/emu
git commit -m "feat(emu): mGBA wrapper with debug-region reader"
```

---

### Task 11: Parity test (ROM vs native: hash and pixels)

**Files:**
- Create: `crates/emu/tests/parity.rs`

**Step 1: Write the test**

```rust
//! Drives the real ROM in mGBA, records the inputs the ROM actually consumed,
//! replays them natively, and requires identical World hash and framebuffer.
//! Needs target/game.gba: run via `cargo xtask parity`.

use emu::Emu;
use engine::debug::{input_slot, INPUT_RING};
use engine::Buttons;
use game_native::Session;
use std::path::Path;

/// (keys, engine frames) segments; must end with an idle tail so the image is stable.
const SCRIPT: &[(Buttons, u32)] = &[
    (Buttons::RIGHT, 40),
    (Buttons::DOWN, 30),
    (Buttons(Buttons::RIGHT.0 | Buttons::DOWN.0), 20),
    (Buttons::LEFT, 15),
    (Buttons::UP, 10),
    (Buttons::A, 5),
    (Buttons::NONE, 40),
];

fn keys_for(frame: u32) -> Buttons {
    let mut f = frame;
    for &(k, n) in SCRIPT {
        if f < n {
            return k;
        }
        f -= n;
    }
    Buttons::NONE
}

#[test]
#[ignore = "needs target/game.gba; run `cargo xtask parity`"]
fn rom_and_native_agree() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/game.gba");
    let mut emu = Emu::open(&rom).expect("open rom");
    let mut snap = emu.boot(120).expect("boot");
    let total: u32 = SCRIPT.iter().map(|s| s.1).sum();

    let mut consumed: Vec<Buttons> = Vec::new();
    let mut guard = 0;
    while snap.frame < total {
        emu.run_frame(keys_for(snap.frame)).unwrap();
        snap = emu.debug().unwrap();
        let have = consumed.len() as u32;
        assert!(snap.frame - have <= INPUT_RING as u32, "ROM ran >64 steps between reads");
        for f in have + 1..=snap.frame {
            consumed.push(Buttons(snap.inputs[input_slot(f)]));
        }
        guard += 1;
        assert!(guard < total * 3, "ROM stopped advancing at frame {}", snap.frame);
    }

    let mut native = Session::new(content::Emerald);
    for &k in &consumed {
        native.step(k);
    }
    assert_eq!(native.world().frame(), snap.frame);
    assert_eq!(native.world().hash(), snap.hash, "World hash diverged");

    let rom_fb = emu.framebuffer_rgb555();
    let diff = rom_fb.iter().zip(native.framebuffer()).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "{diff} pixels differ between ROM and native");
}
```

**Step 2: Run it**

Run: `cargo xtask rom && cargo test -p emu --test parity -- --ignored --nocapture`
Expected: PASS.

If the hash matches but pixels differ by a one-frame scroll offset: the framebuffer is showing the previous frame's registers. Lengthen the idle tail (it should already be stable). If it still differs, dump both images (Task 12) and compare before changing any code. Use superpowers:systematic-debugging.

**Step 3: Commit**

```bash
git add crates/emu/tests
git commit -m "test: ROM/native parity on hash and pixels"
```

---

### Task 12: `xtask parity` and `xtask shot`

**Files:**
- Modify: `xtask/Cargo.toml` (add `emu.workspace = true`, `engine.workspace = true`, `content.workspace = true`, `game-native.workspace = true`)
- Modify: `xtask/src/main.rs`

**Step 1: Add the commands**

```rust
fn write_ppm(path: &Path, fb: &[u16]) -> Result<()> {
    let mut data = format!("P6\n240 160\n255\n").into_bytes();
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
    run(Command::new("cargo")
        .current_dir(root())
        .args(["test", "-p", "emu", "--test", "parity", "--", "--ignored", "--nocapture"]))
}
```

Update `main`:
```rust
        Some("rom") => rom().map(|_| ()),
        Some("parity") => parity(),
        Some("shot") => shot(),
        _ => bail!("usage: cargo xtask <rom|parity|shot>"),
```

**Step 2: Verify**

Run: `cargo xtask parity`
Expected: the test result is `ok. 1 passed`.

Run: `cargo xtask shot && open target/shot-rom.ppm target/shot-native.ppm`
Expected: two identical images, a green checkerboard with yellow dots and a blue border on the top/left edges.

**Step 3: Commit**

```bash
git add xtask
git commit -m "feat(xtask): parity and shot commands"
```

---

### Task 13: Full check and README

**Files:**
- Create: `README.md`

**Step 1: Run everything**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo xtask parity`
Expected: all clean; parity passes.

**Step 2: Write `README.md`**

```markdown
# pokemon-rust

A Rust GBA engine and toolchain for a full rework of Pokémon Émeraude (FR).
Design: `docs/plans/2026-10-21-emerald-rust-rework-design.md`.

## Requirements
- Rust nightly with `rust-src`
- `cargo install agb-gbafix --version 0.25.0 --locked`
- cmake (to build the vendored libmgba)

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
```

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: README for milestone 1"
```

---

## Done when

- `cargo test --workspace` is green.
- `cargo xtask parity` passes: same `World` hash and 0 differing pixels after a scripted input run.
- `target/shot-rom.ppm` and `target/shot-native.ppm` look identical.
- `engine` and `content` contain no GBA- or host-specific code.
