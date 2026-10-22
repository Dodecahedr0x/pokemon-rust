# Milestone 5 — Checkpoints Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** Save and restore the game as semantic checkpoints. The restored game must be bit-identical to the run that was never interrupted.
- **Checkpoint:** a versioned serde snapshot of a *capturable* `World`, meaning the field is idle and no script, battle, transition, fade or scripted movement is running. On native it is written as RON for people and the studio, and as postcard for flash.
- **Trace:** the parent checkpoint id, RLE input frames, the build hash, and the `World::hash` every 60 frames. Mid-cutscene and mid-battle states are reached by replaying a trace, never by snapshot.
- **Checkpoint tree** (native): labels, parents, traces and state digests on disk. `cargo xtask checkpoints verify` replays every trace after a rebuild and flags diverged nodes.
- **`play` bin:** F5 saves, F9 loads, F8 rewinds, and `--load <id|label>` starts from a node.
- **GBA:** the 128 KiB flash holds one checkpoint. It is written by a debug combo (L+R+SELECT) and loaded at boot. A ROM booted from a save made natively matches native, and native restores a save the ROM wrote.
- **Player name:** it becomes `World` state.
- **Hash fix:** the `Object.moves` / `Request::Move` hash collision is fixed.

**Core invariant** (tested at many points of the parity route on native, and through mGBA for the ROM). For every capturable `World` `W` after step `n`, let `C = W.checkpoint(game)` and `W' = World::restore(game, &decode(encode(C)))`. Then:
1. `W'.hash() == W.hash()`.
2. Fed the same inputs, `W` and `W'` give equal hashes on every later frame.
3. A fresh PPU that applies `W'.initial_frame()` and then `W'`'s frames renders the same picture as the uninterrupted session at frame `n` and at every later frame.

**Architecture:**
- **`engine::checkpoint`** (no_std, new) holds:
  - explicit snapshot DTO types (`WorldSnapshot` and per-module `*Snap`), separate from the live structs, so the on-disk schema is explicit, versioned and immune to internal refactors;
  - `Checkpoint { header, world }`, `encode`/`decode` (postcard, schema-first), `state_digest`, `build_hash`;
  - the flash constants shared by both shells.
- **Live modules:** each converts to and from its DTO in its own file. `World::{capturable, snapshot, checkpoint, restore}` live in a new `world/checkpoint.rs`.
- **Restore:** `World::restore` rebuilds the semantic state, and its boot frame (`initial_frame`) re-uploads every VRAM byte the state implies through non-mutating hooks. The shells apply it with the screen off (GBA `apply_blanked`, or a fresh native PPU), as for a new game.
- **`game-native`** (std): `trace` (RLE, recorder, replay with divergence detection), `store` (the tree on disk), `flash` (agb_save on the host, to build and read `.sav` images) and `checkpoints` (the real replayer and materialisation). It also holds `Session::{restore, checkpoint}`, the play bin and the scenario recorder.
- **`game-gba`:** agb `SaveManager::init_flash_128k` and a save/boot module. The flash write happens outside the simulation: the World never knows it was saved, so traces stay pure input.
- **Parity:** a native route test for the invariant at about 100 points, mGBA tests in both save directions, the debug region v2 (build hash, boot source, save status, hash and step cost in scanlines), and ELF IWRAM/EWRAM budgets in `xtask rom`.

**Tech stack:**
- Rust 2024, `no_std` + `alloc` engine.
- `serde` 1 (derive, alloc, no default features) and `postcard` 1 (alloc) in the engine. The GBA build already compiles both: agb 0.25's `agb_save` depends on them, and `crates/game-gba/Cargo.lock:501,576` pins postcard 1.1.3 and serde 1.0.229.
- `ron` 0.8 and `agb_save` 0.25 (host) in `game-native`.
- `object` 0.37 in xtask (ELF section sizes).
- mGBA FFI: `mCore::savedataClone`, `mgba-sys` `core.h:161`.

A task may add to a contract if it needs to. It may not change the meaning of what is already there, and every addition must be listed in its final report.

---

## Current code this builds on

- **`World`/`State`:** `crates/engine/src/world.rs:101-147`. `World::new`: `:230-273`. `step`: `:282-292`. `hash`: `:592-628`. `draw_field`: `:632-638`. `load_map`: `:642-666`.
- **Modes:** `crates/engine/src/world/battle.rs:66-73` (`Mode`). `restore_field`: `:321-326`, the mutating field restore after a battle.
- **Runner and requests:**
  - `Runner`: `crates/engine/src/script.rs:559-565`. `Runner::hash` (`issued` survives the end of a script): `:609-616`. `start` resets `issued`: `:627-632`.
  - `Request::Move { steps: &'static [Step] }`: `:96-99`. Its hash `(18, obj, steps.len())`, the collision: `:216`.
- **Objects:**
  - `Moves { steps, next }`: `crates/engine/src/objects.rs:142-145`. Its hash writes `steps.len()`, the collision: `:428-437`.
  - `Object` (render cache `vram`/`uploaded`): `:164-195`. `Objects` (cache `banks`): `:502-509`. `start_moves`: `:752-754`.
  - `invalidate_vram`: `crates/engine/src/objects/draw.rs:84-89`. `draw` only touches the render cache: `:18-81`.
  - `Anim` (private fields): `crates/engine/src/objects/anim.rs:22-26`.
- **`Field`:**
  - `crates/engine/src/field.rs:67-76`: `drawn` and `ring` are hashed and mirror VRAM.
  - `restore` sets `drawn = None`: `:136-146`. `set_camera`: `:155-176`. `draw_cells(&self)`: `:306`. `hash`: `:271-292`.
- **`TextBox`:**
  - `crates/engine/src/text.rs:238-249`: `bg_on` is hashed and mirrors VRAM. `close` sets `*self = Self::new()`: `:443-459`. `hash`: `:467-476`.
  - `expand` reads `game.new_game().player_name`: `:128-147`.
- **Player name, other readers:** `crates/engine/src/battle/scene.rs:890` (`Names.player`). The starter screen's text box: `crates/engine/src/starter.rs:358,387`. `TextBox::print` call site: `world/host.rs:99`. `BattleScene::new`: `world/battle.rs:234`. `StarterChoice::new`: `world/host.rs:268`.
- **Flow state:**
  - `Vars` (3 KiB, O(1) digest): `crates/engine/src/flow/vars.rs:30-35,156-160`.
  - `Fader`: `flow/fade.rs:15-18`. `Transition`, whose fields stay hashed when idle: `flow/transition.rs:53-62,211-229`. `Input`: `flow/input.rs:10-13`. `Rng`: `rng.rs:3`.
- **`GameData` and `NewGame`:** `crates/engine/src/data.rs:317-325,346-398`.
- **Debug region:** `crates/engine/src/debug.rs:12-49` (160 bytes, `MAGIC` `PKRDBG01`).
- **GBA shell:** `crates/game-gba/src/main.rs:53-90`. Boot is `World::new` + `apply_blanked`; the late/late_black detection is at `:72-85`.
- **emu:**
  - `crates/emu/src/lib.rs:64-77` copies the ROM into a scratch dir. mGBA autoloads `<rom>.sav` from there (`mgba-0.1.0/src/lib.rs:226-227`, `mCoreAutoloadSave`).
  - `run_rom`: `:187-258`. It assumes engine frames start at 1.
- **Native shell:**
  - `Session`: `crates/game-native/src/lib.rs:21-79`.
  - Scenario `Recording`: `crates/game-native/src/scenario.rs:84-98`. `record`: `:173-197`. `EMERALD_ROUTE`: `:722`. `STUB_ROUTE`: `:785`.
  - Play bin: `crates/game-native/src/bin/play.rs`.
- **Parity:** `crates/emu/tests/parity.rs:100-163`. xtask `rom`/`parity`: `xtask/src/main.rs:35-67,143-157`.
- **agb save:**
  - `agb-0.25.0/src/save/mod.rs` provides `SaveManager::init_flash_128k(num_slots, magic: [u8; 32], timer)` and `SaveSlotManager::{slot, metadata, read::<T>, write(slot, &T, &Meta)}`.
  - It postcard-encodes `T` internally and keeps ghost slots for crash safety.
  - Flash 128K (Sanyo `0x1362`, Macronix `0x09C2`) is 4 KiB sectors with prepare-write.
  - `agb_save-0.25.0` is a standalone `no_std` crate (crc16, postcard, serde) with a `StorageMedium` trait, so the host can build byte-identical images.

---

## Wave 0 contracts (committed before Wave 1; read them, do not redesign them)

Wave 0 adds dependencies, derives, the DTO types, every signature below with `todo!()` bodies (except where marked *implemented*), all module declarations, and the stub files. That way no Wave 1 task edits `lib.rs`, `world.rs` module lists, `game-native/src/lib.rs` module lists, `Cargo.toml`s or xtask dispatch just to register something. `cargo test --workspace` and `cargo xtask rom` stay green, with no World hash change in Wave 0.

### Dependencies and derives
- **`crates/engine/Cargo.toml`:**
  - `serde = { version = "1", default-features = false, features = ["derive", "alloc"] }`
  - `postcard = { version = "1", default-features = false, features = ["alloc"] }`
  - **No `serde` feature: always on.** See decision 1.
- **`#[derive(Serialize, Deserialize)]`** (implemented) on these plain public types:
  - every `id!` newtype (`MapId`, `FlagId`, `VarId`, `TextId`, `ScriptId`, `GfxId`, `SpeciesId`, `MoveId`, `TrainerId`)
  - `Dir`, `MovementType`, `Behavior`, `Gfx`
  - `Buttons`, `script::Speed`
  - `mon::{Pokemon, Status}`
- **`crates/game-native/Cargo.toml`:** `serde` (derive), `ron = "0.8"`, `agb_save = "0.25"`, `postcard` (alloc).
- **`xtask/Cargo.toml`:** `object = "0.37"`, `ron = "0.8"`, `serde`.
- **`.gitignore`:** add `checkpoints/*/snapshots/` and `checkpoints/*/status.ron`.

### `engine::checkpoint` (`crates/engine/src/checkpoint.rs`, new; `pub mod checkpoint;` in `lib.rs`)
```rust
use serde::{Deserialize, Serialize};

/// Snapshot schema. Bump on any change to a DTO below (the golden-bytes test fails otherwise).
pub const SCHEMA: u16 = 1;
/// Largest postcard checkpoint the GBA flash slot accepts (route max is reported by the tests).
pub const MAX_ENCODED: usize = 8 * 1024;
/// agb_save layout, shared by game-gba and game-native::flash (byte-identical images).
pub const FLASH_MAGIC: [u8; 32] = *b"pokemon-rust checkpoint flash v1";
pub const FLASH_SLOTS: usize = 1;
pub const FLASH_SIZE: usize = 128 * 1024;
pub const FLASH_SECTOR: usize = 4096;
/// Gen3 `PLAYER_NAME_LENGTH` (7) + EOS; 0xFF-terminated and 0xFF-padded.
pub const PLAYER_NAME_SIZE: usize = 8;
pub type PlayerName = [u8; PLAYER_NAME_SIZE];

/// Engine sources hash (build.rs, Task C; Wave 0: 0) and content build id (`GameData::build_id`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct BuildHash { pub engine: u64, pub content: u64 }
pub const ENGINE_SRC_HASH: u64 = include!(concat!(env!("OUT_DIR"), "/engine_src_hash.rs"));
pub fn build_hash(game: &impl GameData) -> BuildHash;   // implemented

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Header {
    pub schema: u16,          // first field: `decode` reads it alone before anything else
    pub build: BuildHash,     // the capturing build (informational across builds)
    pub frame: u32,           // World::frame at capture
    pub world_hash: u64,      // World::hash at capture; restore checks it when builds are equal
}
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Checkpoint { pub header: Header, pub world: WorldSnapshot }
/// agb_save slot metadata (readable without loading the slot).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SaveMeta { pub schema: u16, pub build: BuildHash, pub frame: u32, pub map: MapId }

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub frame: u32,
    pub rng: u32,                         // Rng state (never 0)
    pub input: InputSnap,
    pub runner_issued: u32,               // Runner::issued (hashed, survives script end)
    pub player_name: PlayerName,
    pub player_id: u32,
    pub field: FieldSnap,
    pub objects: ObjectsSnap,
    pub templates: Vec<TemplateDelta>,    // only templates differing from the current map's data
    pub vars: VarsSnap,
    pub fader: FaderSnap,
    pub transition: TransitionSnap,       // idle stage implied; the leftover fields are hashed
    pub selected: Option<u8>,
    pub last_mover: u8,
    pub delay: u16,
    pub door: u8,
    pub party: Vec<Pokemon>,
    pub respawn: u8,
    pub prev_behavior: Behavior,
    pub encounter_immunity: u8,
}
pub struct InputSnap { pub held: Buttons, pub pressed: Buttons }
pub struct FieldSnap { pub map: MapId, pub camera: (i32, i32), pub drawn: Option<[i16; 4]> /* x0,y0,x1,y1 */,
                       pub ring: (i16, i16), pub overrides: Vec<((i16, i16), u16)> /* ascending */ }
pub struct ObjectsSnap { pub map: MapId, pub run_state: RunStateSnap, pub objs: Vec<ObjectSnap> /* [0] = player */ }
pub enum RunStateSnap { NotMoving, TurnDirection, Moving }
pub struct ObjectSnap {
    pub local_id: u8, pub map: MapId, pub gfx: Option<GfxId>, pub pos: (i16, i16), pub prev: (i16, i16),
    pub elevation: u8, pub facing: Dir, pub movement: MovementType, pub home: (i16, i16),
    pub range: (u8, u8), pub visible: bool, pub frozen: bool, pub facing_locked: bool,
    pub action: Option<ActionSnap>,       // a wander step in progress is fine; scripted `moves` are not
    pub idle: IdleSnap, pub anim: AnimSnap,
}
pub struct ActionSnap { pub kind: ActionKindSnap, pub dir: Dir, pub t: u8, pub off: (i16, i16), pub jump_y: i8 }
pub enum ActionKindSnap { Walk(Speed), Run, InPlace(u8), Jump(u8), Delay(u8) }
pub enum IdleSnap { Start, Wait(u16), Done }
pub struct AnimSnap { pub table: AnimTableSnap, pub cmd: u8, pub timer: u8 }
pub enum AnimTableSnap { Face, Go(u8), Run }
pub struct TemplateDelta { pub local_id: u8, pub pos: Option<(i16, i16)>, pub movement: Option<MovementType> }
pub struct VarsSnap { pub flags: Vec<FlagId> /* set, ascending */, pub vars: Vec<(VarId, u16)> /* non-zero, ascending */ }
pub struct FaderSnap { pub level: u8, pub target: u8 }
pub struct TransitionSnap { pub map: MapId, pub x: i16, pub y: i16, pub facing: Dir, pub exit_walk: bool, pub player_hidden: bool }
// (all of the above: #[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)], Copy where possible)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotCapturable { Starter, Battle, Transition, Script, TextBox, Fade, Moving(u8) }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestoreError {
    Schema(u16), UnknownMap(MapId), UnknownTemplate { map: MapId, local_id: u8 },
    NoPlayer, BadRng, PartyTooLarge, HashMismatch { want: u64, got: u64 },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecodeError { Postcard, Schema { found: u16 }, TooLarge(usize) }

pub fn encode(cp: &Checkpoint) -> Vec<u8>;                                // postcard (Task C)
pub fn encode_checked(cp: &Checkpoint) -> Result<Vec<u8>, DecodeError>;    // TooLarge past MAX_ENCODED
pub fn decode(bytes: &[u8]) -> Result<Checkpoint, DecodeError>;            // reads the leading u16 schema first
/// Semantic identity of a snapshot: Fnv64 over its postcard bytes (the tree's "content hash").
pub fn state_digest(w: &WorldSnapshot) -> u64;
```
**Ids in snapshots.** By construction a snapshot holds only stable ids:
- `MapId`: Gen3 `MAP_*` numbers (`content/build/codegen.rs:192`)
- `FlagId`/`VarId`/`GfxId`/`SpeciesId`/`MoveId`: Gen3 numbers

It never holds a content-assigned index (`TextId`, `ScriptId`, a movement table):
- the text box is pristine at capture
- the runner is idle
- templates are deltas

So snapshots survive content rebuilds.

### `GameData` addition (`crates/engine/src/data.rs`)
```rust
/// Identity of the content build (inputs of the content build script); 0 for test games.
fn build_id(&self) -> u64 { 0 }
```

### `World` (`crates/engine/src/world/checkpoint.rs`, new; `mod checkpoint;` in `world.rs`)
```rust
impl World {
    pub fn capturable(&self) -> Result<(), NotCapturable>;
    pub fn snapshot(&self) -> Result<WorldSnapshot, NotCapturable>;
    pub fn checkpoint(&self, game: &impl GameData) -> Result<Checkpoint, NotCapturable>;
    /// Rebuilds the state; `initial_frame()` then returns every upload the state implies
    /// (applied with the screen off, like `World::new`'s boot frame). Same build: also checks
    /// `hash() == header.world_hash` (`HashMismatch`).
    pub fn restore(game: &impl GameData, cp: &Checkpoint) -> Result<World, RestoreError>;
}
```
**Implemented in Wave 0:**
- `State::player_name: PlayerName`, initialised in `World::new` from `ng.player_name` (truncated to 7 + 0xFF padding). It is **not hashed yet**: Task A appends it.
- `World::player_name(&self) -> &PlayerName` and `World::set_player_name(&mut self, name: &[u8])`.

### Module hooks (stubs; owners in parentheses)
```rust
// field/snapshot.rs (B; `mod snapshot;` in field.rs)
impl Field { pub(crate) fn snapshot(&self) -> FieldSnap; pub(crate) fn from_snapshot(s: &FieldSnap) -> Field;
    /// BG1–3 controls, both tilesets' tiles and palettes, every cell of `drawn`, the scroll.
    /// Mutates nothing (the hash is unchanged).
    pub fn upload_all(&self, game: &impl GameData, f: &mut Frame); }
// objects/snapshot.rs (B; `mod snapshot;` in objects.rs) and objects/anim.rs (B)
impl Objects { pub(crate) fn snapshot(&self) -> ObjectsSnap; pub(crate) fn from_snapshot(s: &ObjectsSnap) -> Objects;
    /// First object (local id) with a live movement script.
    pub fn any_moves(&self) -> Option<u8>; }
// flow/{vars,fade,transition,input}.rs, rng.rs (B)
impl Vars { pub fn snapshot(&self) -> VarsSnap; pub fn from_snapshot(s: &VarsSnap) -> Vars; }
impl Fader { pub fn snapshot(&self) -> FaderSnap; pub const fn from_snapshot(s: FaderSnap) -> Fader; }
impl Transition { pub fn snapshot(&self) -> TransitionSnap; pub fn from_snapshot(s: &TransitionSnap) -> Transition; } // stage Idle
impl Rng { pub const fn from_state(state: u32) -> Option<Rng>; } // None for 0
// implemented in Wave 0 (trivial; their files belong to Task A in Wave 1)
impl Runner { pub fn restore_idle(issued: u32) -> Runner; }
impl TextBox { /// Same hash as `TextBox::new()` (what `close` leaves).
    pub fn is_pristine(&self) -> bool; }
// script.rs (A)
/// FNV-1a-32 over a stable encoding of each step (tag byte + args); the identity of a step table.
pub fn steps_digest(steps: &[Step]) -> u32;
```

### Debug region v2 (`crates/engine/src/debug.rs`, implemented; layout only)
`MAGIC = *b"PKRDBG02"`. The fields are appended after `late_black`, with offset constants and the const asserts updated. Size is 200 and alignment 8.

| Offset | Field | Meaning |
|---|---|---|
| 160 | `build_engine: u64` | engine half of the build hash |
| 168 | `build_content: u64` | content half of the build hash |
| 176 | `boot: u32` | `debug::boot::{NEW_GAME=0, RESTORED=1, SKIPPED=2, REJECTED=3, CORRUPT=4, NO_MEDIA=5}` |
| 180 | `saves: u32` | successful flash writes |
| 184 | `save_status: u32` | `debug::save::{NONE=0, OK=1, TIMEOUT=2, TOO_LARGE=3, STORAGE=4}` |
| 188 | `hash_lines_max: u32` | worst scanlines of `World::hash()` on a visible frame |
| 192 | `step_lines_max: u32` | worst scanlines from the apply to the end of publish on a visible frame |

The GBA shell keeps compiling: it writes none of the new fields until Task F.

### `game-native` (module stubs; `pub mod trace; pub mod store; pub mod flash; pub mod checkpoints;` in `lib.rs`)
```rust
// trace.rs (D)
pub const TRACE_SCHEMA: u16 = 1;
pub const HASH_EVERY: u32 = 60;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct CheckpointId(pub u32);                     // files: c00042
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Parent { NewGame /* World::new(&game) */, Checkpoint(CheckpointId) }
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Run { pub keys: Buttons, pub frames: u32 }
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub schema: u16, pub parent: Parent, pub build: BuildHash,
    pub start_frame: u32,                 // the parent's World::frame (0 for NewGame)
    pub inputs: Vec<Run>,                 // RLE, adjacent runs differ
    pub hashes: Vec<(u32, u64)>,          // (frames since start, World::hash) every HASH_EVERY and at the end
}
impl Trace { pub fn frames(&self) -> u32; pub fn keys(&self) -> impl Iterator<Item = Buttons> + '_; }
pub struct TraceRecorder { /* start_frame, keys: Vec<Buttons>, hashes */ }
impl TraceRecorder {
    pub fn new(start_frame: u32) -> Self;
    pub fn push(&mut self, keys: Buttons, hash_after: u64);
    pub fn frames(&self) -> u32;
    pub fn truncate(&mut self, frames: u32);          // rewind
    pub fn trace(&self, parent: Parent, build: BuildHash) -> Trace;
}
pub struct HashMismatch { pub frame: u32, pub want: u64, pub got: u64 }
pub struct ReplayReport { pub frames: u32, pub end_hash: u64, pub first_mismatch: Option<HashMismatch> }
/// Steps `world` through the trace (it must be at `start_frame`), comparing the recorded hashes.
pub fn replay(game: &impl GameData, world: &mut World, trace: &Trace,
              each: impl FnMut(&World, &Frame)) -> ReplayReport;

// store.rs (D)
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct NodeMeta { pub id: CheckpointId, pub label: String, pub schema: u16, pub build: BuildHash,
    pub frame: u32, pub map: MapId, pub digest: u64 /* state_digest */, pub world_hash: u64 }
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Node { pub meta: NodeMeta, pub trace: Trace }        // nodes/c00042.ron (committed)
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Status { Valid, Stale { schema: u16 }, Diverged { hash: Option<HashMismatch>, parent_diverged: bool }, Broken(String) }
pub struct Replayed { pub checkpoint: Checkpoint, pub report: ReplayReport }
pub struct Store { /* root */ }
impl Store {
    /// `$PKR_CHECKPOINTS`, else `<workspace>/checkpoints/{stub|emerald}`.
    pub fn default_root(stub: bool) -> PathBuf;
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Store>;
    pub fn nodes(&self) -> io::Result<Vec<Node>>;                 // by id
    pub fn find(&self, id_or_label: &str) -> io::Result<Node>;
    pub fn add(&self, label: &str, trace: Trace, cp: &Checkpoint) -> io::Result<CheckpointId>;
    pub fn relabel(&self, id: CheckpointId, label: &str) -> io::Result<()>;
    pub fn remove(&self, id: CheckpointId) -> io::Result<()>;      // refuses a node with children
    pub fn snapshot(&self, id: CheckpointId) -> io::Result<Option<Checkpoint>>; // cache; None if absent/other schema
    pub fn put_snapshot(&self, id: CheckpointId, cp: &Checkpoint) -> io::Result<()>;
    pub fn topo_order(&self) -> io::Result<Vec<Node>>;            // parents first; missing parent/cycle = error
    pub fn verify(&self, replay: impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String>,
                  accept: bool) -> io::Result<Vec<(CheckpointId, Status)>>;   // also writes status.ron
}

// flash.rs (E)
pub fn encode_sav(cp: &Checkpoint) -> Result<Vec<u8>, String>;   // FLASH_SIZE bytes, agb_save layout
pub fn decode_sav(sav: &[u8]) -> Result<(SaveMeta, Checkpoint), String>;

// checkpoints.rs (G)
pub fn replayer<'g, G: GameData>(game: &'g G) -> impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> + 'g;
pub fn materialize<G: GameData>(store: &Store, game: &G, id: CheckpointId) -> Result<Checkpoint, String>;

// lib.rs (implemented, glue)
impl<G: GameData> Session<G> {
    pub fn restore(game: G, cp: &Checkpoint) -> Result<Self, RestoreError>;  // World::restore + fresh Ppu + boot frame
    pub fn checkpoint(&self) -> Result<Checkpoint, NotCapturable>;
}
```

### xtask stubs
- `xtask/src/checkpoints.rs` (G): `pub fn main(args: &[String]) -> Result<()>` and `pub fn verify(root: &Path, accept: bool) -> Result<VerifySummary>`.
- `xtask/src/budget.rs` (H): `pub fn report(elf: &Path) -> Result<Budget>`.
- `main.rs` gains `mod checkpoints; mod budget;` and `Some("checkpoints") => checkpoints::main(&args[1..])`.

---

### Wave 0 as built (read this before coding against the contracts)

- Commit `6343acf`, merged. The World hash did not change. ROM size −600 B. No new GBA packages: postcard and serde were already present through `agb_save`.
- Stub bodies:
  - `const fn` stubs (`Fader::from_snapshot`, `Rng::from_state`) use a bare `todo!()`.
  - `Trace::keys` is `todo!(); core::iter::empty()`.
  - `replayer` returns a closure whose body is `todo!()`.
  - Stub parameters are silenced with `let _ = (...)`.
- Added derives:
  - `HashMismatch`: Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize.
  - `ReplayReport`: Clone, Copy, PartialEq, Eq, Debug.
  - `Replayed`, `Store`, `TraceRecorder`: Clone, PartialEq, Eq, Debug.
  - `TraceRecorder` private fields: `start_frame`, `keys: Vec<Buttons>`, `hashes: Vec<(u32, u64)>`.
  - `Store` private field: `root: PathBuf`.
  - DTOs are `Copy` except the types holding a `Vec`: `WorldSnapshot`, `FieldSnap`, `ObjectsSnap`, `VarsSnap`, `Checkpoint`.
- `#[allow(dead_code)]` on the field and object snapshot `impl` blocks, `xtask::budget::report`, `xtask::checkpoints::verify` and `VerifySummary::ok`. **Remove these when you wire them.**
- Debug region v2:
  - Offsets: `OFF_BUILD_ENGINE=160`, `OFF_BUILD_CONTENT=168`, `OFF_BOOT=176`, `OFF_SAVES=180`, `OFF_SAVE_STATUS=184`, `OFF_HASH_LINES_MAX=188`, `OFF_STEP_LINES_MAX=192`, `SIZE=200`.
  - New modules `debug::boot` and `debug::save` hold the u32 codes.
  - The emu reader still finds the region by magic. Reading the new fields into `DebugSnapshot` is Task E's job.
- xtask:
  - `checkpoints::VerifySummary { nodes: Vec<(CheckpointId, Status)> }` with `ok()`. Task G may reshape it.
  - `budget::Budget { rom, iwram, ewram }`. Task H may reshape it.
- Player name:
  - Private `world.rs` helper `player_name(&[u8]) -> PlayerName`: copies up to the first 0xFF (at most 7 bytes), then pads with 0xFF.
  - `World::player_name` and `set_player_name` are implemented.
  - `State::player_name` is set but **not hashed yet**. Task A appends it.
- `Session::boot(game, world)` is shared by `new` and `restore`. `Session::restore` and `Session::checkpoint` are implemented as glue.
- `TextBox::is_pristine` is implemented: its hash equals `TextBox::new()`'s. `Runner::restore_idle` is implemented. Their tests are in `checkpoint.rs` `mod wave0_tests`.
- Not added (Task B adds them if needed): an `Input` snapshot hook, since `Input`'s fields are public, and an `Anim` snapshot fn.
- Stub mode: the extract tests still need the `PKR_ROM_*`/`PKR_DECOMP` env vars.

### Wave 1 as built (read this before Wave 2; it supersedes the task text where they differ)

- **Flash is 64 KiB** (decision 10). `init_flash_64k`; the emu forces FLASH512. `flash::{encode_sav, write_sav, decode_sav, save_meta}` exist; `encode_sav` returns `Result<Vec<u8>, String>`.
- **Codec (Task C):** `encode`, `encode_checked` (fails above `MAX_ENCODED`), `decode` (rejects input above `MAX_ENCODED` with `TooLarge`), `encoded_len` (no allocation), streaming `state_digest(&WorldSnapshot)`. The store's digest is `state_digest`. The GBA save checks `encoded_len` only; agb encodes itself.
- **Capture predicate (Task B):** besides the planned reasons, `NotCapturable::StaleView` refuses capture after a connection crossing until the pre-crossing view scrolls out, or a warp or battle return redraws the whole view. On the real route about 650 of 3040 sampled frames are `StaleView`. `Action::Checkpoint` waits it out like any other reason.
- **Restore validation (Task B):** `RestoreError::Invalid(&'static str)` for a forged or inconsistent snapshot (header frame, maps, `drawn`, ring, camera, overrides, action timing and offsets, animations, templates). The boot frame starts with `TextBox::new().close(&mut f)` (BG0 fill and disable), so "the text box needs nothing" is superseded.
- **`World::snapshot(&self, game)`** takes `game`.
- **emu:** `run_rom_from(rom, sav: Option<&[u8]>, expect_start: Option<u32>, keys, total, picture_frames)`; `run_rom` passes `None, Some(0)`. `Emu::open_with_save`, `Emu::save_data`. `RomRun` carries `boot` and the final `save`. One hash is sampled per emulator frame in which the engine advanced, so a two-step catch-up during a bulk load drops a sample (the route reports 19070 or 19071 hashes depending on build timing); this is expected.
- **Debug region v2 adds `loop_lines_max` at offset 196** (`SIZE` 200): the worst scanlines of the whole shell loop body on a visible frame (apply, step, publish, save tail; a flash write excluded), from the vblank wake to the next wait. `emu::DebugSnapshot` does not read it yet; Task H adds it.
- **Measured costs (Task F, extracted route):** `hash_lines_max` 72 overall (32 on the field; battles dominate), `step_lines_max` 218, `loop_lines_max` 223 of 228. The planned `hash_lines_max ≤ 24` gate is wrong for M4's battles and is replaced below.
- **ROM size after Wave 1:** 1948 KiB (M4: 1875 KiB), so +73 KiB; above the planned 64 KiB growth budget. Task H re-baselines it (see below).

### Wave 2 gate revisions (taken after Wave 1)

- **Hash cost:** gate `hash_lines_max ≤ 80` (regression guard over the measured 72), not `≤ 24`.
- **Loop margin:** gate `loop_lines_max ≤ 226` (two lines under the 228-line frame) and print the margin. This is the real "no late frame" headroom; `late == 0` stays the hard gate.
- **Optional, if the margin is under 8 lines:** place `Fnv64::write` (or the hash loop) in IWRAM as ARM code (`#[link_section = ".iwram"]`, `#[instruction_set(arm::a32)]`); the hash must stay bit-identical (parity proves it). Caching digests is M6 work.
- **ROM budget:** baseline the ROM, IWRAM and EWRAM on the Wave 1 merge and fail above baseline + 64 KiB ROM, + 2 KiB IWRAM, + 16 KiB EWRAM. Record the M4 → M5 growth (+73 KiB) in As built with its cause.

## Snapshot boundary

| Live state | In the snapshot | How it is restored |
|---|---|---|
| `World.frame`, `Rng`, `Input` (held + pressed) | verbatim | `Rng::from_state` |
| `Runner` | `issued` only (capture needs `task` and `active` to be `None`) | `Runner::restore_idle` |
| `Field` | `map`, `camera_px`, `drawn`, `ring`, `overrides`, verbatim. `drawn` is hashed and kept as is | `Field::upload_all` redraws exactly `drawn` |
| `Objects` / `Object` | every hashed field, including `action`, `idle`, `anim`, `run_state` | cache `vram`, `uploaded`, `banks` start empty; `draw` in the boot frame refills them |
| `Object.moves` (`&'static [Step]`) | never: capture requires `any_moves() == None` | — |
| `State.templates` | deltas (pos, movement) against `game.map(field.map).objects` | `to_vec()` + deltas, in map order |
| `Vars` | sparse set flags and non-zero vars (saved and special) | setters rebuild the digest |
| `Fader`, `Transition` | verbatim; the transition stage must be idle | `from_snapshot` |
| `TextBox` | nothing: capture requires `is_pristine()` | `TextBox::new()`; BG0 stays off, as on a fresh PPU |
| `selected`, `last_mover`, `delay`, `door`, `respawn`, `player_id`, `prev_behavior`, `encounter_immunity`, `party`, `player_name` | verbatim | — |
| `answer`, `mode` | never: capture requires `None` and `Mode::Field` | — |
| `Log`, `boot` | never: not state | `Log::new()`; `boot` is the restore frame |
| `Battle`, `BattleScene`, `StarterChoice`, `&'static BattleGfx` | never: battles and the starter are not capturable; they are reached by replay | — |

**Capture predicate** (`World::capturable`, checked in this order):
1. `Mode::Field`, else `Starter`/`Battle`
2. `!transition.is_active()`, else `Transition`
3. `!runner.is_running() && runner.active().is_none() && answer.is_none()`, else `Script`
4. `text.is_pristine()`, else `TextBox`
5. `fader.is_done() && fader.level() == 0`, else `Fade`
6. `objects.any_moves() == None`, else `Moving(id)`

The player may be mid-step and NPCs mid-wander: `Action` is plain data.

**Restore boot frame** (`world/checkpoint.rs`). It must not mutate hashed state; a test checks the hash before and after:
1. `f.bulk = true`.
2. `field.upload_all(game, f)`.
3. `objects.invalidate_vram(); objects.draw(game, camera, f)`.
4. `f.brightness = fader.level()`.
5. `f.lighten = false`.

The text box needs nothing (pristine means BG0 off).

---

## Rules for every task

- **Workspace:** work in your own git worktree. Commit with `feat(<crate>): …` or `fix(…)`, and **no attribution or co-author lines**.
- **TDD:** write a failing test first.
  - Engine checkpoint tests use the synthetic game of `crates/engine/src/world/tests.rs`. Do not add a new `GameData` shim; extend that one if needed.
  - Native tests use `content::Emerald::for_shells()`, in whatever flavor the build has.
- **Determinism:** no floats, clocks, `HashMap` iteration or host-dependent behaviour in `engine` and `content`. Serialization order is defined: `Vec` and `BTreeMap` in key order, flags and vars ascending. Store ids and timestamps never enter a snapshot or a hash.
- **`engine` stays game-agnostic and `no_std`.** It holds no store, no file IO, no RON. Postcard and serde are fine.
- **Never commit ROM-derived data.** Snapshots hold FR species nicknames and the content player name, so `checkpoints/*/snapshots/` is git-ignored. Node files (labels, RLE inputs, hashes, digests, `MapId`s) are committable. Goldens in tests are built from synthetic data.
- **Hash:**
  - Only Task A changes `World::hash` bytes. It updates the goldens in `world/tests.rs` and the README.
  - Every other task must leave every hash unchanged. `cargo test` catches a change through the goldens and the route replay tests.
- **GBA budget:** `late == 0`, and bulk work only on black frames.
  - A flash write happens outside the simulation, with the picture frozen. The shell resyncs to vblank before it applies again (Task F).
  - Restore uploads go in the boot frame, applied with `apply_blanked`.
- **Before committing:**
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace` with `PKR_ROM_FR=/Users/dode/Documents/pokemon/pokemon-rust/rom/emeraude-fr.gba PKR_ROM_US=/Users/dode/Documents/pokemon/emerald.gba PKR_DECOMP=/Users/dode/Documents/pokemon/pokeemerald`
  - `cargo xtask rom` if you touched anything the GBA build compiles (engine, content, game-gba)
- **Ownership:** only touch the files your task owns, plus new files in your module. Describe any other change you need in your report.

---

## Wave 1 (parallel, 6 agents)

### Task A — engine: movement digest, player name in the World, hash update

**Owns:**
- `crates/engine/src/{script.rs, objects.rs (Moves and hash only), text.rs (+ text/), starter.rs (name plumbing only), battle/scene.rs (name plumbing only)}`
- `crates/engine/src/world.rs` (hash and name plumbing only), `world/host.rs`, `world/battle.rs` (`BattleScene::new` call only)
- `crates/engine/src/world/tests.rs` (goldens), README (golden hashes paragraph)

1. **`steps_digest`:** FNV-1a-32 over one tag byte per `Step` variant plus its arguments (`Dir as u8`, `Speed` code, `cells`, `delay`, `Emote` code). It is a `pub fn` in `script.rs`, and is a `const fn` if the loop allows.
2. **`Request::Move` hash** (`script.rs:216`): `(18, obj, steps.len(), digest as u16, (digest >> 16) as u16)`.
3. **`Moves`** (`objects.rs:142-145`) gains `digest: u32`, set in `start_moves` (`:752`). The hash (`:428-437`) writes `digest` (u32) in place of `steps.len()`, then `next`. The cost per frame stays O(1).
4. **Player name:**
   - `World::hash` appends `s.player_name` (8 bytes) at the end (`world.rs:626`, after `mode`).
   - `text::expand(game, text, player: &[u8])` replaces `game.new_game().player_name` (`text.rs:135`). `TextBox::print(game, text, player, f)` follows, with its caller at `world/host.rs:99`.
   - `StarterChoice::new(choices, player: PlayerName)` stores it for its own prints (`starter.rs:358,387`), with its caller at `world/host.rs:268`.
   - `BattleScene::new(setup, player: PlayerName)` stores it, and `names()` (`battle/scene.rs:890`) reads it. The copy is not hashed, because the World hashes the name. Its caller is at `world/battle.rs:234`.
   - Nothing in the engine reads `NewGame::player_name` after `World::new`.
5. **Goldens:** update `golden_hashes` and the README numbers. The report says why each changed: the name bytes, and the digest replacing a length.
6. **Tests:**
   - Two different step tables of the same length give different `Request::Move` hashes and different World hashes after `start_moves`. Identical contents give equal hashes. This is the regression test for the collision.
   - `steps_digest` is pinned for a 3-step table (hand-computed FNV-1a-32 in the test comment).
   - `set_player_name("ASH")` changes the World hash. A `{PLAYER}` message prints the World's name, not `NewGame`'s. The battle names and the starter screen follow `set_player_name`.
   - Every existing text, scene and starter test stays green with the signature changes.

### Task B — engine: capture predicate, snapshot, restore and the boot frame

**Owns:**
- `crates/engine/src/world/checkpoint.rs` (new, `World` impls), `world/tests/checkpoint.rs` (new; register it from `world/tests.rs` with one `mod` line, the only edit B makes there)
- `crates/engine/src/field/snapshot.rs`, `objects/snapshot.rs`, `objects/anim.rs` (snapshot fns)
- `flow/{vars,fade,transition,input}.rs` (snapshot fns), `rng.rs` (`from_state`)

1. **Module conversions:**
   - `Field`: `drawn` maps to `[x0, y0, x1, y1]`, and `overrides` to an ascending `Vec`.
   - `Objects`: `objs` in order, then the `RunState`/`Action`/`Idle`/`Anim` mirrors. `from_snapshot` leaves `vram`, `uploaded` and `banks` empty.
   - `Vars`: sparse ascending. `from_snapshot` goes through the setters, so `digest == full_digest()`.
   - `Fader`, `Transition` (stage `Idle`), `Input`, `Rng::from_state`.
2. **`Field::upload_all(&self)`:**
   - the `Cmd::Bg` for BG1–3 and `upload_primary`/`upload_secondary` of the current map, as `enter` does (`field.rs:103-134`)
   - `draw_cells` (`&self`, `:306`) over every cell of `drawn`, then `set_scroll`
   - it changes no field
3. **`World::capturable`:** the predicate above, in that order.
4. **`World::snapshot`:**
   - `templates` become deltas against `game`-free data. Keep the map's original templates reachable through the current field map: `snapshot` needs `&impl GameData`. If so, change the signature to `snapshot(&self, game)` and report it; `checkpoint` already takes `game`.
   - The party goes into a `Vec<Pokemon>`.
5. **`World::restore`:**
   - Check `header.schema == SCHEMA`, then `has_map` for the field, objects and templates map; that `objs[0].local_id == PLAYER`; a non-zero RNG; at most 6 party members; that every template delta's `local_id` exists in the map.
   - Rebuild `State`: `log` from `Log::new()`, `mode` = `Field`, `answer` = `None`, `text` = `TextBox::new()`, `runner` = `restore_idle`.
   - Build the boot frame and store it in `boot`.
   - When `header.build == build_hash(game)`, require `hash() == header.world_hash`.
6. **Tests** (synthetic game, `world/tests/checkpoint.rs`):
   - **Predicate:** each `NotCapturable` reason is triggered and reported: a running script, an open text box, a fade in progress, a warp transition, a scripted move still running after its script ended, and a starter or battle mode if the synthetic game can reach them (else unit-test the predicate on a constructed `State`). A plain field frame with an NPC mid-wander is capturable.
   - **Round trip:** at every capturable frame of the golden input script (`golden_hashes`'s 100 frames, plus a 600-frame walk with wandering NPCs, a door warp and a `setmetatile` override), `restore(checkpoint)` has an equal hash. Stepping both 120 more frames with the same inputs keeps the hashes equal on every frame.
   - **Picture:** a fresh `ppu`-less check that the boot frame contains BG1–3 controls, both tilesets, a `MapEntries` command for every `drawn` cell, and the OBJ uploads of every visible object. Pixel equality is Task H's job on the real PPU.
   - **No mutation:** the hash after `restore` equals the hash after taking `initial_frame()`, and `upload_all` leaves `Field::hash` unchanged.
   - **Errors:** a wrong schema, an unknown map, an unknown template, RNG 0, 7 party members, a forged `world_hash` with the same build (`HashMismatch`) and with another build (accepted).
   - **Size:** the encoded size stays under `MAX_ENCODED`; print it for the synthetic world.

### Task C — engine: codec, schema policy and build identity

**Owns:** `crates/engine/src/checkpoint.rs` (fn bodies), `checkpoint/tests.rs` (new), `crates/engine/build.rs` (new), `crates/content/build/main.rs` (build-id generation only), `crates/content/src/lib.rs` (`build_id` impl).

1. **`encode`/`encode_checked`/`decode`:**
   - `decode` first runs `postcard::take_from_bytes::<u16>` for the schema. A mismatch returns `Schema { found }`, then the full checkpoint is decoded.
   - Migration-or-fail: v1 has no migrations. The doc comment states the rule. When a DTO changes, bump `SCHEMA`, then either keep the old DTOs in `checkpoint/v<N>.rs` with a `migrate` (decided per bump) or accept `Schema` errors. Native snapshots then regenerate from traces (decision 6), and the GBA starts a new game.
2. **`state_digest`:** an `Fnv64` over `postcard::to_allocvec(world)`.
3. **`engine/build.rs`:**
   - Write `$OUT_DIR/engine_src_hash.rs`: the FNV-1a-64 of `src/**/*.rs`, sorted by relative `/`-path, hashing (path, len, bytes).
   - Exclude `tests.rs`, `test_*.rs` and any `tests/` dir, so editing tests does not change the build.
   - `cargo:rerun-if-changed=src`.
4. **`content` build id:** `build/main.rs` writes `BUILD_ID: u64`, the same FNV over:
   - `crates/content/{src,build}/**/*.rs` minus `*_tests.rs`
   - the extracted dir's `manifest.ron`, which carries the sha1 of every other exported file (README "Extract"); in stub mode, the literal `b"stub"` instead
   - `Emerald::build_id()` returns it
   - Paths are relative, so the native and GBA builds agree; Task H checks this through the debug region.
5. **Tests:**
   - **Golden bytes:** a hand-built `Checkpoint` (2 objects, one mid-walk; 3 flags; 2 vars; 1 override; 1 template delta; 1 party mon with a synthetic nickname) encodes to pinned postcard bytes (hex in the test) and to a pinned RON string. Any DTO change fails here, which forces a schema bump.
   - **Round trip:** postcard and RON both round-trip to equal values.
   - `decode` of bytes with schema 2 returns `Schema { found: 2 }`. Truncated bytes return `Postcard`. `encode_checked` rejects a snapshot padded with 5000 flags (`TooLarge`).
   - `state_digest` is stable for equal values and differs after one flag.
   - **Engine hash:** deterministic (two builds of the hash function over a temp tree, the same) and insensitive to a `tests.rs` edit. Factor the hashing into a `build_support` fn included by `build.rs` and tested through `#[path]`.
   - **Content id:** the stub and a fixture extracted dir give different ids (`crates/content/tests/build_modes.rs` already builds both); the id is non-zero.

### Task D — native: traces and the checkpoint store

**Owns:** `crates/game-native/src/{trace.rs, store.rs}`, `crates/game-native/tests/store.rs` (new).

1. **Trace:**
   - RLE never stores a zero-length run and merges equal neighbours.
   - The recorder pushes a hash every `HASH_EVERY` frames and at `finish`.
   - `truncate` drops runs and hashes past the cut.
   - `replay` steps the world, calls `each`, and records the first mismatch against `hashes`. It never stops early: the report carries the end hash.
2. **Store layout** (`root = checkpoints/{stub|emerald}` or `$PKR_CHECKPOINTS`):
   ```
   nodes/c00001.ron       Node { meta, trace }            committed (no ROM data)
   snapshots/c00001.ron   Checkpoint (RON, pretty)        git-ignored cache (ROM-derived nicknames)
   status.ron             BTreeMap<CheckpointId, Status>  git-ignored, written by verify
   ```
   - Ids are allocated as max + 1.
   - Labels are free text and must be unique; `find` accepts `c00012`, `12` or a label.
   - Nodes are written to `*.tmp` and then renamed (atomic).
   - Files are pretty RON with stable field order.
3. **`verify`** (topological order):
   1. Get the parent state: `None` for `NewGame`, else the parent's `Replayed.checkpoint` from this run. A Broken parent makes the child `Broken("parent broken")`.
   2. Call `replay(parent, &trace)`. An `Err(e)` is `Broken(e)`.
   3. `node.schema != SCHEMA` or a decode failure is `Stale`.
   4. The digest is equal: `Valid`.
   5. The digest differs with `node.build == checkpoint.header.build`: `Broken("nondeterministic: same build, different state")`.
   6. Otherwise `Diverged { hash: report.first_mismatch, parent_diverged }`.
   7. With `accept`, `Diverged` and `Stale` nodes get new meta (schema, build, frame, map, digest, world_hash), re-recorded trace hashes (from a second pass of `each`, or from `Replayed`, as D prefers) and a fresh snapshot. `Broken` nodes are never accepted.
4. **Tests:**
   - **RLE:** round trip, merging, `frames()`, `truncate` mid-run.
   - **Recorder cadence:** hashes at 60, 120 and the end.
   - `replay` on `content::Emerald::for_shells()` reproduces the recorder's hashes over 300 frames of the scenario keys.
   - A tampered hash is reported at the right frame with both values.
   - **Store:** `add` + `nodes` + `find` by id and by label; `relabel`; `remove` refuses a parent; `topo_order` with a missing parent and with a cycle (both errors); `snapshot` returns `None` for another schema.
   - **`verify` with a fake replayer** covers `Valid`, `Stale`, `Diverged` (with and without a hash hint), `Broken` (nondeterminism, replayer error, parent broken), and `accept` rewriting meta and snapshot but not `Broken`.
   - **Determinism:** the RON node files are byte-identical when written twice.

### Task E — host flash images and emu save plumbing

**Owns:** `crates/game-native/src/flash.rs`, `crates/emu/src/lib.rs`, `crates/emu/Cargo.toml`.

1. **`flash.rs`:**
   - An `agb_save::StorageMedium` over `Vec<u8>` (`FLASH_SIZE`, initially 0xFF). Its `StorageInfo` is `{ size: FLASH_SIZE, erase_size: Some(FLASH_SECTOR), write_size: 1 }`, exactly what agb's `SaveData::info` reports for 128K Sanyo/Macronix flash (`agb-0.25.0/src/save/mod.rs`, `StorageMedium for SaveData`).
   - Erase writes 0xFF.
   - `encode_sav` runs `SaveSlotManager::<_, SaveMeta>::new(storage, FLASH_SLOTS, FLASH_MAGIC)` then `write(0, cp, &meta)`.
   - `decode_sav` reads the metadata, then `read::<Checkpoint>(0)`.
2. **emu:**
   - `Emu::open_with_save(rom, sav: Option<&[u8]>)` writes `game.sav` into the scratch dir before `load_rom`, so `mCoreAutoloadSave` picks it up. A 128 KiB file makes mGBA use FLASH1M. `open` is `open_with_save(rom, None)`.
   - `Emu::save_data() -> Vec<u8>` goes through `(*raw).savedataClone` and frees the buffer with libc `free` (declare `extern "C" fn free`).
   - `DebugSnapshot` gains `build: BuildHash`, `boot`, `saves`, `save_status`, `hash_lines_max` and `step_lines_max`, read at the v2 offsets.
   - `run_rom_from(rom, sav: Option<&[u8]>, keys, total, picture_frames) -> RomRun` starts at the ROM's first published frame, which is `n` for a restored save. `consumed[i]` is the input of step `start + i + 1`, and `RomRun` gains `start: u32`. `run_rom` is `run_rom_from(rom, None, …)` with `start == 0`, so the existing parity test is unchanged.
3. **Tests:**
   - `encode_sav` → `decode_sav` round trip; the image is exactly 128 KiB; the metadata matches.
   - Two successive writes decode to the newest; a corrupted data block decodes to an error or to the ghost copy (agb semantics, asserted as observed).
   - `decode_sav` of an all-0xFF image is an error.
   - `open_with_save` places `game.sav` in the scratch dir and nothing next to the ROM. This extends the existing `open_does_not_create_sav_next_to_rom` dummy-header test.
   - The v2 offsets match `engine::debug` (a const assert in `lib.rs`).

### Task F — GBA shell: flash save, boot load, cost counters

**Owns:** `crates/game-gba/src/{main.rs, save.rs (new), save_logic.rs (new, no agb imports)}`, `crates/game-gba/Cargo.toml`, `crates/emu/tests/save_logic.rs` (new, includes `save_logic.rs` through `#[path]`, as `parity.rs` does with `probe_scene.rs`).

1. **`save_logic.rs`** (pure, host-testable):
   - `combo_pressed(prev: Buttons, cur: Buttons) -> bool`: L and R held, and SELECT newly pressed.
   - `BootChoice { Restore, NewGame(u32 /* debug::boot code */) }` and `boot_choice(meta: Option<SaveMeta>, held_at_boot: Buttons) -> BootChoice`:
     - B held → `SKIPPED`
     - no slot → `NEW_GAME`
     - `meta.schema != SCHEMA` → `REJECTED`
     - otherwise `Restore`
   - `PendingSave { frames_left: u16 }`: a combo while not capturable waits up to 600 frames, then `TIMEOUT`.
2. **`save.rs`:**
   - `init(gba) -> Option<SaveSlotManager<SaveMeta>>` calls `init_flash_128k(FLASH_SLOTS, FLASH_MAGIC, Some(timer2))`. An error gives `NO_MEDIA`.
   - `boot_world(game, slots, keys) -> (World, u32)` reads slot 0 and runs `World::restore`. Any read, decode or restore error gives `World::new` with `CORRUPT` or `REJECTED`.
   - `write(slots, &Checkpoint) -> u32 /* debug::save code */`: `encode_checked` for the 8 KiB budget, then `slots.write(0, cp, &meta)`.
3. **`main.rs`:**
   - Boot order: flash init, `boot_world`, `initial_frame` + `apply_blanked`, then publish `build` and `boot` once.
   - After each `publish_debug`: if a combo or a pending save is active and `world.checkpoint(&game)` is `Ok`, write flash. Then **resync**: call `wait_for_vblank` until a wake lands on `VBLANK_LINE`, which drains agb's catch-up. Only then apply the next frame. The frozen picture never tears, and `late` stays meaningful.
   - The World never sees the save: no state change and no extra step.
4. **Counters:**
   - Read `VCOUNT` around `world.hash()` in `publish_debug`, and from after `video.apply` to the end of publish. The deltas are mod 228.
   - Keep the maxima over frames that are not black in `hash_lines_max` and `step_lines_max`.
5. **Tests:**
   - Host: the combo edge cases (held SELECT does not repeat; L without R fails); every `boot_choice` branch; the pending timeout.
   - `cargo xtask rom` builds and reports the ROM size delta.
   - Manual: in mGBA, L+R+SELECT then a reset boots back to the same place (report it). The automated version is Task H's.

---

## Wave 2 (parallel, after Wave 1 is merged)

### Task G — native integration: replayer, play bin, scenario nodes, `xtask checkpoints`

**Owns:**
- `crates/game-native/src/{checkpoints.rs, lib.rs (beyond the Wave 0 glue), bin/play.rs, scenario.rs}`
- `xtask/src/checkpoints.rs`
- `checkpoints/{stub,emerald}/nodes/*` (seed trees)
- `crates/game-native/tests/checkpoint_tree.rs` (new)
- README (Play and Checkpoints sections)

1. **`replayer(game)`:**
   - The parent is `World::new` or `World::restore` (an error is a message).
   - Check `world.frame() == trace.start_frame`, then run `trace::replay`.
   - At the end, `world.checkpoint(game)`. `Err(r)` becomes `Err("trace ends while {r:?}")`.
2. **`materialize`:** use the cached snapshot if its schema matches and its build equals the current one, else replay the chain from the root and cache every snapshot on the way.
3. **Play bin:**
   - **Keys:** A key = L, S key = R (newly mapped).
   - **F5 saves:** if capturable, at once; else pending until the first capturable frame, with "saving…" in the title. The node is `store.add(label = "{map name} f{frame}", recorder.trace(parent, build), cp)`. The new node becomes the parent, and the recorder restarts.
   - **F9:** reload the current parent node (`materialize` + `Session::restore`).
   - **F8:** rewind. A ring holds 64 capturable checkpoints taken every 60 frames (about a minute). F8 restores the entry at least 120 frames back and truncates the recorder to it.
   - **CLI:** `--load <id|label>` starts there, and `--list` prints the tree and exits.
   - The window title shows the frame, the parent id and the last action.
   - The controller is a pure `Controls` struct with unit tests, so `main` only maps keys and draws.
4. **Scenario:**
   - `Action::Checkpoint(&'static str)`: press nothing until `capturable()`, then push `(name, frame)` to `Recording.nodes`.
   - Route points:
     - Emerald: `lab`, `after_wild`, `oldale_center` (after the nurse `Mash`), `route103_after_may`
     - stub: `stub_start`, `stub_after_battle`, `stub_end`
   - `Recording::traces(&self, build) -> Vec<(name, Trace)>` chains them from `NewGame`.
   - Existing moments and parity expectations are re-pinned if the frame counts move.
5. **`cargo xtask checkpoints`:**

   | Subcommand | What it does |
   |---|---|
   | `list` | the tree: id, label, frame, map, status |
   | `verify [--accept]` | exits 1 on `Diverged`/`Broken` without `--accept`; prints a table and the first-mismatch frame |
   | `seed` | records the route and writes `route:<name>` nodes, replacing earlier `route:` nodes and never touching others |
   | `label <id> <text>`, `rm <id>` | relabel; remove |
   | `sav <id> [-o target/game.sav]` | writes a flash image next to `target/game.gba`, so mGBA autoloads it |

   The flavor follows `content::STUB`; `--root` overrides it.
6. **Commit the seed trees** (`checkpoints/stub/nodes`, and `checkpoints/emerald/nodes` built with extracted data). Inputs only; no ROM data.
7. **Tests:**
   - `Session::restore` round trip on the stub route.
   - `Controls`: pending save, F9, rewind truncation (trace frames and inputs), the ring cadence.
   - Every `Action::Checkpoint` frame is capturable, and the seed traces replay to their nodes (`Valid`).
   - `xtask checkpoints verify` on a temp store: valid; a flipped input is `Diverged`; a forged digest with the same build is `Broken`.
   - **`checkpoint_tree.rs`:** verify the committed tree of the current flavor in `cargo test`. It skips with a message if the tree is empty. This behaves like the golden hashes: an intended behaviour change needs `verify --accept`.

### Task H — parity, budgets and CI gates

**Owns:**
- `crates/game-native/tests/checkpoint_route.rs` (new)
- `crates/emu/tests/parity.rs`
- `xtask/src/{budget.rs, main.rs: rom() and parity() only}`
- README (Tests section)

1. **Native invariant along the route** (`checkpoint_route.rs`, not ignored; stub or extracted):
   - Pick about 100 points: the first capturable frame in each window of `frames / 100`, plus every moment frame.
   - At each point:
     - `encode_checked` (track the maximum size, assert `≤ MAX_ENCODED`, print it)
     - `decode`, then `Session::restore`
     - the hash equals the live hash, and the framebuffers are equal at +0 (boot frame against live step `n`)
     - step both 120 frames on `rec.keys_at`, with the hash equal every frame and pictures equal at +1, +2, +60 and +120
   - RON round trip at each point.
   - Three points (first, middle, last node) continue to the end of the route with the hashes equal on every frame.
2. **ROM resumes from native checkpoints** (`parity.rs`, ignored, run by `xtask parity`):
   - Take 3 nodes on the extracted route (2 on the stub): `lab`, `oldale_center`, and the last capturable frame before May's battle.
   - For each: `flash::encode_sav`, then `run_rom_from(sav)` for `min(2000, remaining)` frames on the route keys.
   - Assert `boot == RESTORED`, the ROM build equals `build_hash(&game)`, and `start == node frame`.
   - Compare against the native **uninterrupted** session: every hash, and pictures every 100 frames plus the first 2.
   - `late == 0`, and each `late_black` follows a bulk step.
3. **ROM save → native:**
   - Run the ROM from a new game on the route keys up to node `f`. Feed L+R at `f + 1` and L+R+SELECT at `f + 2`, and feed the same keys natively.
   - Then run 30 frames of nothing so mGBA settles the write. The emulator frame count may run ahead of the engine frame.
   - Assert `saves == 1`, `save_status == OK`, and `Emu::save_data()` → `decode_sav` gives `header.frame == f + 2` and `header.world_hash` equal to the ROM's published hash at `f + 2`.
   - `Session::restore` on it gives the native hash at `f + 2`.
   - Boot a second emu from that `.sav`; its first 300 frames match native.
4. **Build identity and costs:**
   - The ROM's `build` equals native `build_hash(&Emerald::for_shells())` (also asserted in `rom_and_native_agree`).
   - Print `hash_lines_max` and `step_lines_max`; gate them per "Wave 2 gate revisions" (`hash_lines_max ≤ 80`, `loop_lines_max ≤ 226`).
5. **`xtask rom` budgets (`budget.rs`):**
   - Use `object` to sum the allocated ELF sections by link address: IWRAM (`.iwram*`, `.text_iwram*`) and EWRAM statics. agb 0.25's `gba.ld` puts `.data*` inside the `.ewram` output section and `.bss` in EWRAM (NOLOAD), not in IWRAM as first written here.
   - Print them with the ROM size. Measure the baseline on master first.
   - Fail above baseline + 2 KiB IWRAM, baseline + 16 KiB EWRAM, or a ROM growth over 64 KiB compared with M4's 1875 KiB (record the numbers in As built).
   - Also `cargo xtask parity` calls `checkpoints::verify(Store::default_root(..), false)` before the ROM tests.
6. **Close the README Tests paragraph:** the checkpoint invariant, the save tests, the seed tree rule and the budgets.

---

## Wave 3 — close-out

- **Whole-milestone review:** fix the findings.
- **This doc:** add "As built" (contracts changed, sizes: checkpoint max bytes, ROM/IWRAM/EWRAM, hash lines) and "Findings for Milestone 6" (studio needs: tree view over `Store`, `materialize`, mid-trace seek by replay, a state editor over `WorldSnapshot`).
- **Cleanup:** rename the parity "checkpoints" (picture compare frames: `Recording::checkpoints`, `run_rom`'s argument) to `picture_frames`, to end the name clash. Clean up worktrees.

## Done when

- `cargo test --workspace` passes in stub mode and with extracted data:
  - the native invariant at about 100 route points (hash equal on restore and every later frame, pictures equal)
  - the golden bytes
  - the committed tree verifying `Valid`
- `cargo xtask parity` passes:
  - the existing route
  - the ROM resumes from native checkpoints with identical hashes and pictures and `boot == RESTORED`
  - a ROM in-game save restores natively to the ROM's hash
  - `late == 0`, `hash_lines_max ≤ 80`, `loop_lines_max ≤ 226`, and the budgets hold
- `play`: F5/F9/F8 and `--load` work; `cargo xtask checkpoints list|verify|seed|sav` work; `target/game.sav` from `sav` boots in mGBA at the node.
- `Request::Move` / `Object.moves` hash step contents; the player name is World state and hashed.
- No ROM-derived file is committed.

---

## Open questions / decisions taken

1. **Serde: always on, no feature (taken).**
   - The GBA needs serde to write flash anyway.
   - agb 0.25 already compiles `serde` + `postcard` in the GBA build (through `agb_save`), so there is no new dependency.
   - Derives sit only on small DTOs and plain id/enum types, and LTO drops what is unused.
   - A feature would create two engine variants that can drift.
   - Cost check: ROM growth ≤ 64 KiB, gated in `xtask rom`.
2. **Formats (taken).**
   - **Postcard** is the binary form: flash, `encode`, and `state_digest`.
   - **RON** is the human form: nodes, snapshots, status. It reuses the same serde types, which the studio will read.
   - No JSON.
3. **Explicit DTOs instead of deriving on live structs (taken).**
   - The schema is visible, versioned and pinned by golden bytes.
   - Caches, `&'static` references and futures are excluded by construction.
   - Internal refactors do not break saves.
   - Cost: one conversion function pair per module.
4. **The VRAM-mirroring hashed fields stay hashed (taken).**
   - `Field.drawn` is serialized verbatim, and `upload_all` redraws exactly that rect without mutating anything.
   - `TextBox.bg_on` and friends: capture requires a pristine text box (equal to `TextBox::new()` by hash), so nothing needs to be uploaded.
   - The battle-side mirrors (bars, healthbox, `BattleWindows.loaded`, …) are out of scope, because battles are never captured.
   - Taking these fields out of the hash would weaken parity's detection of drawing bugs.
5. **`&'static` references (taken).**
   - `Object.moves` and `Request::Move` never appear in a snapshot, because the predicate requires no live moves.
   - Their hash uses `steps_digest`, an id derived from the table's contents. Identical tables behave identically, so they rightly hash the same. This fixes the collision without touching codegen.
   - A registry (`GameData::movement(id)`) is deferred until mid-script capture, which is never needed: mid-cutscene states are reached by replay.
   - `&'static BattleGfx` is unchanged, since battles are not captured.
   - `Request::Log` is not state.
6. **Committable data (taken).**
   - Node files (`Node { meta, trace }`) are committed: inputs, hashes, digests, Gen3 map ids, labels.
   - Snapshots contain FR species nicknames and the content player name, which are ROM-derived. They stay a git-ignored cache that is always rebuildable from the traces.
   - This also makes "migration-or-fail" cheap on native: a snapshot with an old schema is regenerated by replay, and only traces carry `TRACE_SCHEMA`.
7. **Store: one RON file per node, no SQLite (taken).**
   - Merge-friendly, and the files can be diffed by hand.
   - SQLite stays the explorer's (`explore.db`).
   - Ids are sequential `c00001…`, local to a store.
8. **Verify semantics (taken).**
   - Replay runs from `NewGame` down the tree on the current build.
   - Equality is semantic (`state_digest`), so hash-layout changes and content renumbering do not flag nodes.
   - Periodic hashes only localise a divergence.
   - A different digest on the same build is `Broken` (nondeterminism).
   - A schema change is `Stale`.
   - `--accept` re-baselines, like the golden hashes.
9. **GBA trigger: L+R+SELECT (taken).**
   - There is no start menu yet. Emerald's slice scripts do not save, and Gen3 saving goes through the start menu.
   - The menu milestone will call the same `save::write`.
   - Hold B at boot to ignore a save.
   - The save happens outside the simulation, with the picture frozen during the write and a vblank resync. Traces stay pure input.
10. **Flash: 64 KiB, one slot (revised in Wave 1 by Task E's finding; the original 128 KiB text follows for the record).**
    - **Why 64 KiB:**
      - mGBA 0.11 never reads agb's `FLASH1M_V` marker. It autodetects FLASH512 (Panasonic 0x1B32) on the first flash write (`savedata.c:267-270`) and maps only 64 KiB of a 128 KiB `.sav`.
      - agb_save puts data in the top sectors, so a 128 KiB host image comes back Corrupted unless mGBA is overridden by hand.
    - **Implementation:** `FLASH_SIZE = 64 * 1024`. The GBA calls `init_flash_64k`. The emu forces a FLASH512 override.
    - **Layout:** 16 sectors: global, slot header, ghost, and 13 data sectors.
    - **Capacity:** about 24 KiB per slot with a ghost copy. A future full save (PC boxes, bag) has to fit in that, or move to 128 KiB with an explicit mGBA override.
    - **Store parse errors (Task D, accepted):** an unparseable node file is an error that names the file, not `Stale`. Merge-conflict markers fail loudly, and `--accept` needs a parseable trace.

    *Original decision:*
    - agb ghost slots give crash safety.
    - The checkpoint budget is 8 KiB; the route is expected under 4 KiB, with flags and vars stored sparse.
    - 128 KiB leaves room for later full saves (PC boxes, bag), where 64 KiB would be tight with ghost copies.
    - mGBA picks FLASH1M from agb's `FLASH1M_V` marker, and from the 128 KiB `.sav` size when one is injected. Task E/H verify this. The fallback is `init_flash_64k`, with the same code paths.
11. **Load across builds (taken).**
    - Snapshots hold only stable ids, so a checkpoint loads into another build when the schema matches. The world hash is checked only on the same build.
    - The GBA falls back to a new game on any error and reports why in `boot`.
12. **Native restore needs no fade (taken).**
    - The boot frame goes to a fresh PPU (native) or is applied blanked (GBA boot).
    - A future in-game "load" on the GBA will fade to black, apply the boot frame blanked, then continue. That is shell work; the World is unchanged.
13. **Rewind ring (taken; cheap).** 64 capturable snapshots, one every 60 frames, in the play bin only. Rewinding inside a battle lands before the battle.
14. **Player name (taken).**
    - `World.player_name: [u8; 8]` is copied from `NewGame` at a new game, hashed and snapshotted.
    - `set_player_name` is ready for the naming screen.
    - The rival name stays content `UiText`.
15. **Known risk.** After a connection crossing, a `setmetatile` override on the old map can stay visible in the uninterrupted run, because Gen3 keeps stale entries. `upload_all` redraws that cell without the override. The pictures would then differ until the cell scrolls out. No slice route hits this; Task H's picture checks would catch it. Fixing it later means snapshotting the stale cells; not done now.
16. **OBJ allocation after restore.** Fresh first-fit slots and palette banks give the same pictures unless more than 1024 OBJ tiles or 16 palettes are needed. The slice is far below both, and Task B asserts that no object fails to draw in its tests.
17. **Hand-authored checkpoints (studio state editor):** not in M5. Every node needs a trace. M6 can add `Parent::Edited` nodes, which `verify` would mark as unverifiable.

