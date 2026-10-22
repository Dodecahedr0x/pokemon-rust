# Milestone 6 — Studio v0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** a Dioxus desktop **studio** (`crates/studio`) that plays, inspects and manages the game. It has five views:
- **Checkpoint tree:** shared (committed) and local (personal, git-ignored) stores. Each node shows Valid, Stale, Diverged, Broken or unverified, from `Store::verify`. A node can be loaded into the native or the ROM pane, labelled or removed, and branched from: play from the node, then save.
- **Native play pane:** keyboard input, pause, frame-step, fast-forward, rewind (the `Controls` ring) and "open at frame N" inside a node's trace. A minimal state inspector shows frame, map, hash, mode, capturable or the reason it is not, parent, trace frames and the last action, plus a small event timeline.
- **mGBA play pane:** boots `target/game.gba` through `emu`, as a new game or from a node (`flash::encode_sav`). It shows a debug-region timeline (engine frame, hash, late, late_black, boot, saves, hash/step/loop lines) and a "ROM build ≠ native build" banner, with a one-click `cargo xtask rom`.
- **Map view (read-only):** pixel-identical metatile rendering through the engine's `Field` and the native `Ppu`. Overlays: collision/elevation/behavior, warps (click to follow), objects, coord and bg events, connections.
- **Data views (read-only):** species, moves, trainers, encounters and texts. Names are decoded with the content charmap.

**Not in v0:**
- Editing and write-back of data or maps. Edits would land in the ROM-derived, git-ignored `content/extracted/` and be lost on re-extract; where edits live is M8 "scale import" design work.
- The state editor (M5 decision 17).
- Script views and graphs.
- Explorer UI (M7).

**First, frame headroom on the GBA** (M5 Findings 1). The loop runs 223 of 226 gated lines, which leaves no room for new per-frame work. Restructure the hash input so one ARM/IWRAM fold runs per buffer, with bytes identical.

**Architecture**
- **`crates/studio`** is a workspace member. It has a headless **core** (no Dioxus; pure controllers with unit tests) and a thin **`ui`** feature (Dioxus 0.7 desktop + the `studio` bin). This mirrors `game-native`'s `window` feature, so `cargo test --workspace` never builds a webview.
- **Panes run on their own threads.** `World` is `!Send` (`Runner`'s `Rc<RefCell<Io>>`, `script.rs:304-308`), so every `Session`/`Controls` is created inside its pane thread. Messages carry only `Send` data: ids, `Checkpoint`, `Trace`, `Buttons`, commands. `emu::Emu` is `Send` (`mgba::Core`).
- **Frame transport:** each pane thread publishes its latest RGB555 framebuffer as RGBA into a `FrameSlot`. The webview pulls it with `fetch("/frames/<pane>?after=<seq>")`, served by `dioxus::desktop::use_asset_handler`, and draws with `putImageData` on a 240x160 canvas scaled by CSS (`image-rendering: pixelated`). The bytes are raw binary: 153,600 bytes per frame, about 9 MB/s per pane at 60 fps, with no JSON or base64. The Wave 0 spike proves it before anything is built on it.
- **Status:** each pane publishes a small `Clone` status struct (`StatusSlot<S>`). The UI polls it at about 15 Hz in a `use_future`.
- **Store:** typed errors, an index with children, single-parse snapshots, a `Materializer` with an LRU, a per-node divergence chain, keyframes and `seek` (M5 Findings 2–4).

**Tech stack**
- Rust 2024.
- `dioxus = { version = "0.7.10", features = ["desktop"] }`, optional under `ui`. Stable 0.7; not the 0.8 alphas.
- `dioxus-ssr = "0.7.10"` as an optional dev-dependency under `ui`, for the display-free smoke test.
- The workspace crates `engine`, `content`, `game-native` and `emu`.
- `extract`, for `text::decode_lang` (`crates/extract/src/text.rs:124`, `Lang::{En, Fr}`).
- No new dependency in `engine`.

A task may add to a contract if it needs to. It may not change the meaning of what is already there, and every addition must be listed in its final report.

---

## Current code this builds on

- **Native shell:**
  - `Session` (`crates/game-native/src/lib.rs:27-140`): `new`, `restore`, `load`, `checkpoint`, `step`, `framebuffer` (lazy render) and `ppu`.
  - `Controls` (`controls.rs`): `new`, `resume`, `step -> Option<SaveRequest>`, `save`, `saved`, `load(id, cp)`, `rewind`, `title`, `ring_frames`, `trace`. The ring is 64 entries every 60 frames, and a rewind goes back at least 120 frames.
  - The play bin (`bin/play.rs`): its `KEYS` table, its RGB555 expansion `(v<<3)|(v>>2)`, `add` with a label suffix retry on `AlreadyExists`, and `load` through `materialize`.
- **Store** (`store.rs`):
  - `NodeMeta`, `Node`, `Status`, `Replayed`.
  - `Store::{default_root, open, root, nodes, find, add, relabel, remove, snapshot, put_snapshot, topo_order, verify}`, all returning `io::Result`.
  - `snapshot` parses twice (a schema probe, then the full parse; `:398-420`).
  - `status.ron` is written whole by `verify`, last writer wins.
- **Materialisation** (`checkpoints.rs`): `replayer`, `materialize -> Materialized { checkpoint, diverged }` (it trusts the cache only on matching schema, build and digest) and `seed`.
- **Traces** (`trace.rs`): `CheckpointId`, `Parent`, `Run`, `Trace { schema, parent, build, start_frame, inputs, hashes }` (`TRACE_SCHEMA` 2), `TraceRecorder::{new, push, frames, truncate, trace}`, and `replay` / `replay_with_hashes(game, world, trace, each)`.
- **Flash** (`flash.rs`): `encode_sav`, `write_sav`, `decode_sav`, `save_meta`.
- **emu** (`crates/emu/src/lib.rs`):
  - `Emu::{open, open_with_save(rom, Option<&[u8]>), save_data, run_frame(keys), framebuffer_rgb555, debug() -> Option<DebugSnapshot>, boot(max)}`.
  - `DebugSnapshot` holds frame, hash, inputs ring, late, late_black, build, boot, saves, save_status, and hash/step/loop lines max.
- **Engine, public surface the studio uses:**
  - `World::{frame, map, hash, capturable, snapshot(game), checkpoint, restore, objects, text, flag, var, log, script_running, transition_active, party, in_battle, battle, starter, field_active}`.
  - `World::initial_frame` is only non-empty right after `new` or `restore` (`world.rs:284-287`). A world reached by stepping has no boot frame, so **seeking must step a `Session`**, not a bare `World`.
  - `engine::field::Field::{new, enter, load, set_camera, block, redraw}` are public (`field.rs:90-290`). Also `field::metatile_layers`, `data::{block_metatile, block_collision, block_elevation, attr_behavior, attr_layer}` and `GameData::behavior`.
  - `Fnv64` (`hash.rs`) is byte-at-a-time with a split multiply for Thumb (no long multiply). `World::hash` has 29 `hash(&self, h)` impls and about 250 small `write*` calls.
- **Content API at runtime:**
  - `content::{Emerald::for_shells, STUB, BUILD_ID, has_map}`.
  - `content::data::{map(u16), MAP_IDS, MAP_COUNT, TEXTS, TEXT_NAMES, LANG, SCRIPT_NAMES, PLAYER_SPRITE, sprite(u16)}`. `MapData.name` is the decomp identifier, not ROM text.
  - `content::battle::{TABLES (all species/moves), TRAINER_IDS, trainer(u16), encounters(u16), SLICE_SPECIES, SLICE_MOVES, heal_location}`.
  - `engine::mon::{SpeciesInfo, MoveInfo, TrainerData, LandEncounters}`. Names are Gen3-encoded `&'static [u8]`.
- **Committed trees:** `checkpoints/{stub,emerald}/nodes` (3 and 4 route nodes).
- **Local store:** the M5 close-out task adds a personal, git-ignored local store. This plan assumes `Store::local_root(stub: bool) -> PathBuf` at `checkpoints/<flavor>/local`, and that the play bin saves there. **Wave 0 step 0 reads the close-out's As built and renames the contract below to match.**
- **xtask:** `main.rs` dispatches `rom`, `parity`, `checkpoints`, …; `checkpoints.rs` has `list/verify/seed/label/rm/sav`.

---

## Wave 0 contracts (committed before Wave 1; read them, do not redesign them)

Wave 0 does the following:
- adds the crate and the dependencies
- does the mechanical `StoreError` migration (implemented)
- implements the frame transport and the pane thread (the spike)
- adds every other signature below with a `todo!()` body
- adds module stubs, so no Wave 1 task touches `Cargo.toml` member lists, `lib.rs` module lists or the xtask dispatch

`cargo test --workspace` (real and stub), `cargo xtask rom` and `cargo xtask parity` stay green, and no hash changes.

### Step 0 — reconcile with the close-out
Read the M5 close-out's local-store As built. If it added cross-store parents (for example a local node whose parent is a shared node), record them here and drop `trace::flatten` from Task C. Otherwise the contract stands: a local node's parent is local or `NewGame`, and branching from a shared node saves a flattened trace (decision 7).

### Workspace and crate
- Root `Cargo.toml`: add `"crates/studio"` to `members` and `studio = { path = "crates/studio" }`.
- `crates/studio/Cargo.toml`:
  ```toml
  [features]
  # The desktop app (`cargo run -p studio --features ui`); the core builds without a webview.
  ui = ["dep:dioxus", "dep:dioxus-ssr"]
  [dependencies]
  engine.workspace = true
  content.workspace = true
  game-native.workspace = true
  emu.workspace = true
  extract.workspace = true
  anyhow.workspace = true
  dioxus = { version = "0.7.10", features = ["desktop"], optional = true }
  dioxus-ssr = { version = "0.7.10", optional = true }   # smoke test only (cfg(test) + ui)
  [[bin]]
  name = "studio"
  required-features = ["ui"]
  ```
- No `asset!()`/manganis. CSS and JS are `include_str!`'d and injected with `Config::with_custom_head`, so plain `cargo run` works without the `dx` CLI.

### `studio` module layout (`src/lib.rs`)
```rust
pub mod paths;      // implemented (W0)
pub mod frames;     // implemented (W0, spike)
pub mod pane;       // implemented (W0, spike)
pub mod native;     // Task C
pub mod rom;        // Task D
pub mod build;      // Task D
pub mod tree;       // Task B
pub mod map_view;   // Task E
pub mod sprite;     // Task E
pub mod data_view;  // Task F
pub mod text;       // Task F
#[cfg(feature = "ui")] pub mod ui;   // mod.rs skeleton W0; play/tree (G), map/data (H)
```

### `paths.rs` (implemented)
```rust
pub struct StudioPaths {
    pub workspace: PathBuf,
    pub rom: PathBuf,          // target/game.gba
    pub scratch: PathBuf,      // target/studio/  (settings.ron, exported .sav, logs)
    pub shared: PathBuf,       // Store::default_root(content::STUB)
    pub local: PathBuf,        // Store::local_root(content::STUB)
}
impl StudioPaths { pub fn detect() -> StudioPaths; }
/// Every directory the studio may write to. Only `local`/`shared` `nodes/` hold committable files
/// (inputs only); everything else is git-ignored. Tested with `git check-ignore` when git exists.
pub fn write_locations(p: &StudioPaths) -> Vec<(PathBuf, Committable)>;
pub enum Committable { NodesOnly, Never }
```

### `frames.rs` (implemented: the transport core)
```rust
pub const FRAME_BYTES: usize = SCREEN_W * SCREEN_H * 4;
/// RGB555 -> RGBA8 with the play bin's expansion ((v << 3) | (v >> 2)), alpha 255.
pub fn rgba_from_rgb555(fb: &[u16], out: &mut [u8]);
#[derive(Clone, Default)]
pub struct FrameSlot(Arc<Mutex<Published>>);           // Published { seq: u64, rgba: Arc<Vec<u8>> }
impl FrameSlot {
    pub fn publish(&self, fb: &[u16]);                    // seq += 1
    /// The latest frame if its seq is > `after`.
    pub fn latest(&self, after: u64) -> Option<(u64, Arc<Vec<u8>>)>;
}
/// `GET /frames/<pane>?after=<seq>`: 200 + body (RGBA) + `x-seq`, or 204. `Cache-Control: no-store`.
pub fn parse_frame_request(path_and_query: &str) -> Option<(&str, u64)>;
```

### `pane.rs` (implemented: the pane thread)
```rust
pub trait Pane {                                   // lives on its thread; need not be Send
    type Cmd: Send + 'static;
    type Status: Clone + Send + 'static;
    fn handle(&mut self, cmd: Self::Cmd);
    /// One 60 Hz tick: steps 0..n frames depending on the run mode.
    fn tick(&mut self);
    fn framebuffer(&mut self) -> &[u16];
    fn status(&self) -> Self::Status;
}
#[derive(Clone)] pub struct StatusSlot<S>(Arc<Mutex<S>>);
pub struct PaneHandle<C, S> { pub frames: FrameSlot, pub status: StatusSlot<S>, tx: Sender<C>, /* join */ }
impl<C, S> PaneHandle<C, S> { pub fn send(&self, c: C); pub fn stop(self); }
/// Spawns a thread that builds the pane with `make` (so !Send state is born there), then loops:
/// drain commands, tick, publish frame + status, sleep to the next 1/60 s (`Pacer`).
pub fn spawn<P: Pane, F: FnOnce() -> P + Send + 'static>(name: &str, make: F) -> PaneHandle<P::Cmd, P::Status>;
pub struct Pacer { /* period, next */ }
impl Pacer { pub fn new(hz: u32) -> Self; /// Returns how long to sleep; never accumulates more than one period of debt.
    pub fn next(&mut self, now: Instant) -> Duration; }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunMode { Play, Pause, Fast(u8) }        // Fast(n): n frames per tick, 2..=16
```

### `ui/mod.rs` (implemented skeleton: the spike)
- `pub fn launch()`: `dioxus::LaunchBuilder::desktop().with_cfg(Config::new().with_custom_head(CSS+JS).with_window(..1280x800..))`.
- The `App` component has tabs Play, Checkpoints, Map, Data. `ui::{play, tree, map, data}` are empty `#[component]` stubs.
- `use_asset_handler("frames", ..)` answers from a `HashMap<&'static str, FrameSlot>` held in context.
- `<canvas data-pane="native">` and a JS loop (`requestAnimationFrame` → `fetch` → `putImageData`) per pane.
- **Spike pane:** a `TestPattern` `Pane` that scrolls a gradient at 60 fps, with an fps counter in JS.
- `src/main.rs`: `fn main() { studio::ui::launch() }`.
- `STUDIO_SMOKE_SECS=<n>`: the app closes itself after `n` seconds and prints the delivered fps per pane. `cargo xtask studio --smoke` reads it.

**Spike acceptance** (recorded in "Wave 0 as built"):
- Two test-pattern panes at the same time, release build, macOS: at least 58 delivered fps each, under one frame of transport latency, and a studio CPU share noted.
- Debug build: note the numbers.

**Fallbacks, in order:**
1. Pre-encoded RGBA through `document::eval` + `send` as base64. This costs about 205 KB of string per frame.
2. Pane pop-out: the existing `play` bin (minifb) as a child process for native, and a minifb window in a `studio-rom` helper for mGBA. The studio keeps the tree, inspector and timeline.

### `game-native` store contract (`store.rs`; error migration implemented, the rest stubs)
```rust
#[derive(Debug)]
pub enum StoreError {
    NotFound(String),                       // id or label
    Label(String),                          // empty, or taken (was io AlreadyExists)
    Parent { id: CheckpointId, parent: CheckpointId },   // missing parent
    Cycle(Vec<CheckpointId>),
    HasChildren(CheckpointId),
    Parse { file: PathBuf, msg: String },
    Io(io::Error),
}
impl Display + std::error::Error + From<io::Error> for StoreError;
// Every existing `Store` method now returns Result<_, StoreError> (W0, mechanical; play.rs
// matches StoreError::Label instead of ErrorKind::AlreadyExists; xtask prints Display).

pub struct Index { /* BTreeMap<CheckpointId, Node>, children: BTreeMap<Parent, Vec<CheckpointId>> */ }
impl Index {
    pub fn node(&self, id: CheckpointId) -> Option<&Node>;
    pub fn children(&self, p: Parent) -> &[CheckpointId];        // ascending
    pub fn topo(&self) -> Vec<CheckpointId>;                     // parents first, ids ascending
    /// `id` and its ancestors, root first.
    pub fn chain(&self, id: CheckpointId) -> Result<Vec<CheckpointId>, StoreError>;
}
impl Store {
    pub fn local_root(stub: bool) -> PathBuf;                    // from the close-out (W0 aligns)
    pub fn index(&self) -> Result<Index, StoreError>;            // one read of nodes/
    pub fn statuses(&self) -> Result<BTreeMap<CheckpointId, Status>, StoreError>; // status.ron, {} if absent
    /// Merges `verdicts` into status.ron (read-modify-write, atomic rename) instead of overwriting.
    pub fn merge_statuses(&self, verdicts: &[(CheckpointId, Status)]) -> Result<(), StoreError>;
}
```

### `game-native` materialisation and seek (`checkpoints.rs`, `trace.rs`, `controls.rs`, `lib.rs`; stubs)
```rust
// checkpoints.rs (C)
pub const KEYFRAME_EVERY: u32 = 600;                     // frames between seek keyframes
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Link { pub id: CheckpointId, pub cached: bool, pub diverged: bool }
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Materialized { pub checkpoint: Checkpoint, pub diverged: bool, pub chain: Vec<Link> /* root first */ }
pub struct Materializer<G: GameData> { /* game, build, LRU (id, BuildHash) -> Checkpoint, keyframes LRU */ }
impl<G: GameData + Clone> Materializer<G> {
    pub fn new(game: G, capacity: usize) -> Self;
    /// `materialize` with an in-memory LRU in front of the disk cache; each snapshot parsed once.
    pub fn get(&mut self, store: &Store, id: CheckpointId) -> Result<Materialized, StoreError>;
    /// Keyframes of node `id`'s trace: (frames since the parent, checkpoint) at the first capturable
    /// frame at or after every KEYFRAME_EVERY frames; built by one replay, then cached.
    pub fn keyframes(&mut self, store: &Store, id: CheckpointId) -> Result<Arc<Vec<(u32, Checkpoint)>>, StoreError>;
}
/// Must run on the thread that will own the session (World is !Send).
pub struct Seeked<G: GameData> { pub session: Session<G>, pub parent: Parent, pub recorder: TraceRecorder, pub diverged: bool }
/// Node `id`'s state `frames` frames after its parent: restore the nearest keyframe <= frames
/// (or the parent), then `Session::step` the trace's keys. `frames` is clamped to the trace.
pub fn seek<G: GameData + Clone>(m: &mut Materializer<G>, store: &Store, id: CheckpointId, frames: u32)
    -> Result<Seeked<G>, StoreError>;
// keep `materialize(store, game, id)` as a thin wrapper (play bin, xtask).

// trace.rs (C)
impl TraceRecorder {
    /// The first `frames` keys of `trace` and its recorded hashes up to there.
    pub fn from_prefix(trace: &Trace, frames: u32) -> TraceRecorder;
}
/// One trace from NewGame: the chain's traces in order (root first), hashes shifted by each
/// trace's offset. Used when a local save branches from a shared node (decision 7).
pub fn flatten(chain: &[&Trace]) -> Result<Trace, String>;

// controls.rs (C)
impl<G: GameData> Controls<G> {
    /// Continues a seek: the session, its parent and the recorded prefix (the ring restarts here).
    pub fn from_seek(s: Seeked<G>) -> Self;
    pub fn held_parent_chain(&self) -> Parent;   // = parent(); alias not needed: remove if unused
}
// lib.rs (C)
impl<G: GameData> Session<G> { pub fn into_game(self) -> G; }   // only if C needs it
```

### Studio core stubs (signatures; owners in Wave 1)
```rust
// native.rs (C)
pub enum NativeCmd {
    Keys(Buttons), Mode(RunMode), Step(u32), Rewind, Save,
    Load { origin: Origin, id: CheckpointId },              // materialize on the pane thread
    Seek { origin: Origin, id: CheckpointId, frames: u32 },
    NewGame,
}
#[derive(Clone, Debug, Default)]
pub struct Inspector {
    pub frame: u32, pub map: MapId, pub map_name: &'static str, pub hash: u64,
    pub mode: &'static str,                                  // "field" | "battle" | "starter"
    pub capturable: Result<(), NotCapturable>,              // Default: Ok
    pub run: RunMode, pub parent: Option<(Origin, Parent)>, pub trace_frames: u32,
    pub pending_save: bool, pub last: String, pub ring_frames: Vec<u32>,
    pub events: Vec<Event>,                                  // newest last, at most 64
    pub saved: Option<(Origin, CheckpointId)>,               // the last save (UI refreshes the tree)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event { pub frame: u32, pub kind: EventKind }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind { MapEntered(MapId), ScriptStarted, ScriptEnded, BattleStarted, BattleEnded, Log(&'static str), Saved(CheckpointId), Loaded(CheckpointId), Rewound(u32) }
pub struct NativePane { /* Controls<Emerald>, RunMode, held, stores, Arc<Mutex<Materializer<Emerald>>> */ }
impl NativePane { pub fn new(stores: Stores, m: Arc<Mutex<Materializer<content::Emerald>>>) -> Self; }
impl Pane for NativePane { type Cmd = NativeCmd; type Status = Inspector; /* … */ }

// rom.rs (D)
pub enum RomSource { NewGame, Node { origin: Origin, id: CheckpointId }, Sav(Vec<u8>) }
pub enum RomCmd { Keys(Buttons), Mode(RunMode), Step(u32), Reset(RomSource), ExportSave }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineEntry { pub emu_frame: u32, pub frame: u32, pub hash: u64, pub late: u32, pub late_black: u32,
    pub saves: u32, pub save_status: u32, pub hash_lines_max: u32, pub step_lines_max: u32, pub loop_lines_max: u32 }
#[derive(Clone, Debug, Default)]
pub struct RomStatus {
    pub rom: RomFile,                 // Missing | Present { modified }
    pub boot: Option<u32>,            // debug::boot code
    pub rom_build: Option<BuildHash>, pub native_build: BuildHash,
    pub run: RunMode, pub timeline: Vec<TimelineEntry>,   // last 600, newest last
    pub error: Option<String>, pub exported: Option<PathBuf>,
}
pub struct RomPane { /* Option<Emu>, … */ }
impl RomPane { pub fn new(paths: StudioPaths, stores: Stores, m: Arc<Mutex<Materializer<content::Emerald>>>) -> Self; }
impl Pane for RomPane { type Cmd = RomCmd; type Status = RomStatus; }

// build.rs (D)
pub struct RomBuild { /* child */ }
impl RomBuild { pub fn start(workspace: &Path) -> io::Result<RomBuild>;      // `cargo xtask rom`
    pub fn poll(&mut self) -> BuildState; }                                     // Running(lines) | Ok | Failed(lines)

// tree.rs (B)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)] pub enum Origin { Shared, Local }
#[derive(Clone, Debug)] pub struct Stores { pub shared: Store, pub local: Store }
impl Stores { pub fn detect(p: &StudioPaths) -> Stores; pub fn get(&self, o: Origin) -> &Store; }
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow { pub origin: Origin, pub id: CheckpointId, pub label: String, pub depth: u16,
    pub parent: Parent, pub frame: u32, pub trace_frames: u32, pub map: MapId, pub map_name: &'static str,
    pub status: Option<Status>, pub children: u32 }
pub struct TreeModel { /* rows in display order: per origin, DFS by id */ }
impl TreeModel {
    pub fn load(stores: &Stores) -> Result<TreeModel, StoreError>;           // index + status.ron
    pub fn rows(&self) -> &[TreeRow];
    pub fn row(&self, o: Origin, id: CheckpointId) -> Option<&TreeRow>;
    pub fn set_statuses(&mut self, o: Origin, v: &[(CheckpointId, Status)]);
}
/// `Store::verify(replayer, accept = false)` + `merge_statuses`, on a thread.
pub fn verify_in_background(store: Store, game: content::Emerald) -> JoinHandle<Result<Vec<(CheckpointId, Status)>, StoreError>>;
pub fn relabel(stores: &Stores, o: Origin, id: CheckpointId, label: &str) -> Result<(), StoreError>;
pub fn remove(stores: &Stores, o: Origin, id: CheckpointId) -> Result<(), StoreError>;
/// Where a save from `parent` goes, and with what trace: Local always; a shared parent is
/// flattened (Parent::NewGame) unless the close-out provides cross-store parents.
pub fn plan_save(stores: &Stores, parent: (Origin, Parent), trace: Trace) -> Result<(Origin, Trace), StoreError>;

// map_view.rs (E)
pub struct MapImage { pub map: MapId, pub margin: u16 /* cells */, pub width: usize, pub height: usize, pub rgb555: Vec<u16> }
/// The map's BG layers as the game draws them: Field::enter + set_camera over a grid of 240x160
/// windows on a fresh Ppu, stitched. Margin cells come from connections/border (Field::block).
/// `overrides`: setmetatile blocks to apply (from a snapshot), else none.
pub fn render_map(game: &impl GameData, map: MapId, margin: u16, overrides: &[((i16, i16), u16)]) -> MapImage;
pub struct CellInfo { pub x: i16, pub y: i16, pub block: u16, pub metatile: u16, pub collision: u8, pub elevation: u8, pub behavior: Behavior, pub raw_behavior: u8 }
pub enum Overlay {
    Object { local_id: u8, x: i16, y: i16, gfx: Gfx, movement: MovementType, range: (u8, u8), script: Option<&'static str>, hide_flag: Option<FlagId> },
    Warp { index: u8, x: i16, y: i16, dest: MapId, dest_name: &'static str, dest_warp: u8 },
    Coord { x: i16, y: i16, var: VarId, value: u16, script: &'static str },
    Bg { x: i16, y: i16, kind: BgKind, script: Option<&'static str> },
    Connection { dir: Dir, offset: i16, map: MapId, name: &'static str },
}
pub fn cells(game: &impl GameData, map: MapId) -> Vec<CellInfo>;      // row-major, width*height
pub fn overlays(game: &impl GameData, map: MapId) -> Vec<Overlay>;
pub fn map_list() -> Vec<(MapId, &'static str)>;                       // content::data::MAP_IDS order

// sprite.rs (E)
/// Frame `frame` of a sprite sheet, decoded (4bpp 1D OBJ tiles + palette; index 0 transparent).
pub fn decode_sprite(s: &SpriteData, frame: usize) -> (usize, usize, Vec<Option<u16>>);

// data_view.rs (F)
pub struct SpeciesRow { pub id: SpeciesId, pub name: String, pub base: [u8; 6], pub types: [Type; 2], pub abilities: [Ability; 2],
    pub catch_rate: u8, pub exp_yield: u8, pub growth: GrowthRate, pub gender_ratio: u8, pub learnset: Vec<(u8, MoveId, String)>,
    pub has_gfx: bool, pub trainers: Vec<TrainerId>, pub maps: Vec<MapId> }
pub struct MoveRow { pub id: MoveId, pub name: String, pub ty: Type, pub power: u8, pub accuracy: u8, pub pp: u8,
    pub effect: MoveEffect, pub effect_chance: u8, pub target: MoveTarget, pub priority: i8, pub flags: MoveFlags }
pub struct TrainerRow { pub id: TrainerId, pub class: String, pub name: String, pub female: bool, pub double: bool, pub ai: u32,
    pub party: Vec<(SpeciesId, String, u8)> /* species, name, level (+ moves if TrainerMon has them) */ }
pub struct EncounterRow { pub map: MapId, pub map_name: &'static str, pub rate: u8, pub mons: Vec<(SpeciesId, String, u8, u8, u8 /* % slot */)> }
pub struct TextRow { pub id: TextId, pub key: &'static str, pub text: String }
pub fn species() -> Vec<SpeciesRow>;  pub fn moves() -> Vec<MoveRow>;  pub fn trainers() -> Vec<TrainerRow>;
pub fn encounters() -> Vec<EncounterRow>;  pub fn texts() -> Vec<TextRow>;
pub fn filter<'a, T>(rows: &'a [T], q: &str, key: impl Fn(&T) -> String) -> Vec<&'a T>;  // case-insensitive

// text.rs (F)
/// Gen3 bytes -> display string with the build's charmap (content::data::LANG); invalid -> "<?xx>".
pub fn decode(bytes: &[u8]) -> String;
```

### xtask (implemented dispatch, stub body)
- `xtask/src/studio.rs`: `pub fn main(args: &[String]) -> Result<()>`.
  - No arguments: `cargo run -p studio --features ui --release`.
  - `--smoke [secs]`: the same with `STUDIO_SMOKE_SECS`. It fails if a pane delivers under 50 fps or the process exits non-zero.
- `main.rs` gains `mod studio;` and `Some("studio") => studio::main(&args[1..])`.

### Gates added in Wave 0 (`Rules for every task` lists them)
- `cargo clippy -p studio --all-targets --features ui -- -D warnings`
- `cargo test -p studio --features ui` (the ssr smoke test)

---

## Rules for every task

- **Workspace:** work in your own git worktree. Commit with `feat(<crate>): …` or `fix(…)`, with **no attribution or co-author lines**.
- **TDD:** write a failing test first. Studio core tests are plain `#[test]`s with no window and no Dioxus. They use `content::Emerald::for_shells()` in whatever flavor the build has, and temp stores (`std::env::temp_dir()` + pid + counter) for store tests.
- **The UI is thin.** Components only map events to `Cmd`s and render `Status`/rows. Logic that needs a test goes in the core.
- **Threads:** never move a `World`, `Session` or `Controls` across threads; build them in `Pane::make`/`handle`. `Materializer` (it holds only `Checkpoint`s) is shared as `Arc<Mutex<_>>`. Never hold its lock across a replay longer than one `get`.
- **Never commit ROM-derived data.** The studio writes only under `paths::write_locations`:
  - store `nodes/` (inputs, hashes, digests, `MapId`s, labels)
  - git-ignored `snapshots/` and `status.ron`
  - `target/studio/`

  No renders, decoded texts or `.sav` files outside `target/`. **No FR text literals in tests.** Assert structure: counts equal the tables, names are non-empty and decode without `<?`, ids are consistent. Stub-mode expectations come from `crates/content/build/stub.rs`, which is repo data.
- **Hash:**
  - Only Task A touches `Fnv64`, and the bytes must be identical: the goldens in `world/tests.rs`, the committed trees and parity must not move.
  - Task A2 (conditional) is the only task allowed to change hash bytes. It must update the goldens, the README and the trees (`verify --accept`).
- **GBA budget:** `late == 0`, `loop_lines_max ≤ 226` (Task A tightens it), and the ROM/IWRAM/EWRAM budgets in `xtask rom`.
- **Determinism:** no floats, clocks or `HashMap` iteration in `engine`/`content`. Studio pacing uses `Instant` only in `pane.rs`.
- **Before committing:**
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo clippy --workspace --all-targets --features game-native/window,studio/ui -- -D warnings`
  - `cargo test --workspace` with the ROM env vars (`PKR_ROM_FR=…/rom/emeraude-fr.gba PKR_ROM_US=… PKR_DECOMP=…`)
  - `PKR_EXTRACTED_DIR=/nonexistent cargo test --workspace` (stub)
  - `cargo test -p studio --features ui`
  - `cargo xtask rom` and `cargo xtask parity` if you touched anything the GBA build compiles or `emu`
  - `cd crates/game-gba && cargo fmt --check && cargo clippy -- -D warnings` if you touched game-gba
- **Ownership:** only touch the files your task owns, plus new files in your module. Describe any other change you need in your report.

---

## Wave 1 (parallel, 6 agents)

### Task A — GBA frame headroom: buffered hash, ARM fold in IWRAM

**Owns:**
- `crates/engine/src/hash.rs`
- `crates/engine/Cargo.toml` (`[features] iwram = []`)
- `crates/game-gba/Cargo.toml` (`engine = { …, features = ["iwram"] }`)
- the README Tests numbers

1. **`Fnv64` becomes buffered.** Its fields are `state: u64`, `buf: [u8; N]` and `len: u8`, with `N` = 64 to start; measure 64, 128 and 256.
   - `write` appends and calls `fold(state, &buf)` when the buffer is full.
   - `finish(&self)` folds the remainder on a copy. It stops being `const`; fix any const callers in engine (grep `finish`) without changing their results.
   - `write_u16`/`write_u32` stay little-endian.
2. **`fold(state, bytes) -> u64`:**
   - Portable: the existing `mul_prime` loop.
   - Under `cfg(all(feature = "iwram", target_arch = "arm"))`:

     ```rust
     #[unsafe(link_section = ".iwram")] #[instruction_set(arm::a32)] #[inline(never)]
     ```

     It uses a plain `wrapping_mul(PRIME)`, since ARM state has `UMULL` (the Thumb limit is why `mul_prime` exists).
   - One interworking call per `N` bytes, not one per `write` (the M5 failure).
   - Check the disassembly (`arm-none-eabi-objdump` or `cargo objdump`) and put its key lines in the report.
3. **Tests:**
   - A random-split property test: any chunking of a byte stream gives the old bytewise FNV value, with explicit vectors and 10k pseudo-random splits.
   - The existing vectors.
   - `World` goldens unchanged, `checkpoint_tree.rs` Valid, parity identical.
4. **Measure** with `cargo xtask parity`: `hash_lines_max`, `step_lines_max` and `loop_lines_max` (route worst and before May), and the ROM/IWRAM delta.
   - Target: `loop_lines_max ≤ 218` (an 8-line margin).
   - Tighten the xtask gate to the measured value + 2 (owned by H in M5; give the change to Wave 2 via your report, or do it if `xtask/src/main.rs` parity gate constants are free; coordinate).
5. **If the margin is still under 8 lines,** write the Task A2 brief in your report (battle sub-digests). Do not do it here.

### Task B — store v2 and the tree model

**Owns:**
- `crates/game-native/src/store.rs` (beyond W0)
- `xtask/src/checkpoints.rs`
- `crates/game-native/tests/checkpoint_tree.rs`
- `crates/studio/src/tree.rs`

1. **`Store::index`:**
   - One `read_dir` and one parse per node file.
   - It detects missing parents (`StoreError::Parent`) and cycles (`Cycle`).
   - `children` and `chain` follow.
   - `topo_order` and `find` go through it.
2. **`snapshot`: single parse.** Parse `Checkpoint` directly; only on failure probe the schema to choose between `None` (another schema) and `Parse`. The existing behaviour stays: an absent file or another schema gives `None`.
3. **`statuses` and `merge_statuses`.**
   - `verify` merges its verdicts (it never drops entries of nodes it did not replay). An entry is removed only when its node is removed.
   - `remove` drops the node's status entry.
4. **`xtask checkpoints list`** reads `index` + `statuses`. The output is unchanged.
5. **`tree.rs`:** `Stores`, `TreeModel`, `verify_in_background`, `relabel`, `remove`, `plan_save` (using `trace::flatten` from C; until C merges, test against the W0 stub behind `#[ignore]` and enable it in Wave 2).
   - Rows: shared first, then local, depth-first by id.
   - `map_name` from `game.map(id).name`.
6. **Tests:**
   - Index: children, chain, a missing parent and a cycle are typed errors.
   - Single parse: a corrupt snapshot is `None` and an unparseable node is `Parse { file }`.
   - Two `merge_statuses` keep both.
   - `TreeModel` on a temp store of three nodes (depths, child counts, statuses).
   - `relabel` of a taken label is `Label`.
   - `remove` with children is `HasChildren`.
   - `verify_in_background` on the committed tree gives all `Valid`.

### Task C — materializer, seek and the native pane

**Owns:**
- `crates/game-native/src/{checkpoints.rs, trace.rs, controls.rs, lib.rs (additions), bin/play.rs}`
- `crates/studio/src/native.rs`

1. **`Materializer`:**
   - An LRU of capacity N (a hand-rolled `VecDeque`) keyed by `(id, build)`.
   - `get` returns `chain` with `cached` and `diverged` per link. Divergence is per node, not only the target (M5 Finding 3).
   - Disk cache rules as M5: schema, build and digest must match; only matching replays are cached.
   - `materialize` wraps it, and the play bin uses `Materializer`.
2. **`keyframes` and `seek`:**
   - Keyframes are captured while replaying the node's trace from its parent, at the first capturable frame at or after each multiple of `KEYFRAME_EVERY`.
   - `seek` restores the nearest keyframe at or before the target, or the parent (`Session::restore` / `Session::new`), then steps a `Session` with the remaining keys. `recorder = TraceRecorder::from_prefix(trace, frames)`.
   - Seeking inside a battle replays from the last capturable keyframe (M5 Finding 4).
3. **`trace::flatten`:** keys concatenated; hashes shifted by the cumulative frames; `start_frame` 0; `build` = the last trace's. Error if a trace's `start_frame` does not equal the previous end.
4. **`Controls::from_seek`.** Remove the placeholder alias `held_parent_chain` if it is unused.
5. **`NativePane`:**
   - `tick`: `Pause` steps 0 frames (or the pending `Step(n)`), `Play` steps 1, `Fast(n)` steps n. Only the last frame is rendered, through the `Session` lazy framebuffer.
   - `Save` goes through `tree::plan_save` (Wave 2 wires it if B is not merged; until then save to local with the native parent) and `Store::add`, retrying the label suffix on `StoreError::Label`.
   - `Load` and `Seek` take the `Materializer` lock, then build the session on this thread.
   - Events are derived per stepped frame from state diffs: map change, `script_running` edges, `in_battle` edges, new `Log` lines (`log().count()`). Design §3's engine `FrameOut` events are not built yet, and this needs no engine change.
   - Inspector fields as in the contract.
6. **Tests:**
   - LRU eviction and hits (no file read on a hit: delete `snapshots/` and `get` again).
   - Chain flags on a forged diverged middle node.
   - `seek` to node frames 0, k, k+1 and the end: hash equal to a straight replay at the same frame, and the framebuffer equal to a straight `Session` replay.
   - `seek` into May's battle (extracted) or the stub battle: equal hash.
   - `from_seek` + 30 frames + save: the saved trace replays (`Valid`).
   - `flatten` of the route chain replays from NewGame to the last node's digest.
   - `NativePane`: pause steps 0, `Step(3)` steps 3, `Fast(4)` steps 4, rewind delegates, and the event sequence over the route's first map change contains `MapEntered`.
   - No window anywhere.

### Task D — ROM pane and one-click build

**Owns:**
- `crates/studio/src/{rom.rs, build.rs}`
- `crates/emu/src/lib.rs` (additive only)

1. **`RomPane`:**
   - `Reset(src)`:
     - `NewGame`: `Emu::open(rom)`.
     - `Node`: `Materializer::get`, then `flash::encode_sav`, then `open_with_save`.
     - `Sav`: `open_with_save` directly.
   - Then `boot(600)`. Record `boot` and `rom_build`, compare with `build_hash(&Emerald::for_shells())`, and show a banner on mismatch.
   - Each run frame `debug()` gives a `TimelineEntry` (ring of 600).
   - `ExportSave` writes `Emu::save_data()` to `target/studio/<timestamp>.sav`, never the repo.
   - A missing ROM is a status, not a panic.
2. **`RomBuild`:** `cargo xtask rom` in the workspace with stdout and stderr captured line by line. `poll` is non-blocking. The UI reloads the pane on `Ok`.
3. **Optional (stretch):** a native shadow. Step a native `Session` with the keys the ROM consumed (the `DebugSnapshot.inputs` ring, aligned as `run_rom_from` aligns them) and flag timeline entries whose hash differs.
4. **Tests:**
   - Unit (no ROM): ring bound, the `RomFile::Missing` status, build-mismatch detection with a fake snapshot, and `RomBuild` command construction.
   - `#[ignore]`d (run by `cargo xtask parity` or by hand, needs `target/game.gba`):
     - a new game boots, the timeline frames are increasing, `late == 0` over 600 frames
     - `Node` (`route:lab` or `stub_start`) boots `RESTORED` with `frame == node frame`

### Task E — map view

**Owns:** `crates/studio/src/{map_view.rs, sprite.rs}`

1. **`render_map`:** `Field::new(map)`, then `enter(game, map, cam0, f)`, then `set_block` for the overrides, then a window loop. For each camera on a 240x160 grid covering `[-margin·16, w·16 + margin·16)`: `set_camera`, `ppu.apply(&f)`, `ppu.render(&f, fb)`, then copy the window into the image. Frames have no OBJs and brightness 0. No engine change.
2. **`cells` and `overlays`:** script names from `content::data::SCRIPT_NAMES[id.0]`; `dest_name` from `game.map(dest).name` (or `"MISSING"` when `!has_map`).
3. **`decode_sprite`** for NPC overlays: `gfx` `Fixed(id)` uses `game.sprite(id)`; `Var` draws a placeholder.
4. **Tests:**
   - **Pixel identity with the game.** On the route, at a capturable frame with fade 0 and a pristine text box, render the live session's last frame with `objs` cleared. It must equal the crop of `render_map(map, margin = 8, snapshot.field.overrides)` at `snapshot.field.camera`. Do this for at least 3 frames on 2 maps, including one with a connection in view.
   - Image size `(w + 2m)·16 x (h + 2m)·16`.
   - Every `MAP_IDS` map renders without panic.
   - Overlay counts equal the `MapData` slices.
   - `decode_sprite` of `PLAYER_SPRITE` frame 0 has the sheet's size and transparent index 0.

### Task F — data views

**Owns:** `crates/studio/src/{data_view.rs, text.rs}`

1. **`text::decode`:** `extract::text::decode_lang` with `Lang` from `content::data::LANG`. Placeholders are kept as `{PLAYER}` etc.
2. **Row builders** over `content::battle::TABLES` (all species and moves), `TRAINER_IDS` + `trainer`, `MAP_IDS` + `encounters`, and `TEXTS`/`TEXT_NAMES`.
   - Cross references: species → trainers and maps; encounter slot percentages use the Gen3 land slot table: 20, 20, 10, 10, 10, 10, 5, 5, 4, 4, 1, 1.
   - `filter`.
3. **Tests:**
   - Row counts equal the table lengths.
   - Every slice species and move name decodes non-empty without `<?`.
   - Learnset moves exist in `moves()`.
   - Each trainer party species is in `species()`.
   - Encounter percentages sum to 100.
   - `filter` is case-insensitive and on the decoded name.
   - Stub mode: the counts of `stub.rs`.
   - No FR literals.

---

## Wave 2 (parallel, after Wave 1 is merged)

### Task G — UI: play panes, inspector, timeline and tree

**Owns:**
- `crates/studio/src/ui/{mod.rs (beyond W0), play.rs, tree.rs, keys.rs}`
- `xtask/src/studio.rs`
- README (a new **Studio** section)

1. **App state:**
   - `StudioPaths`, `Stores`, `Arc<Mutex<Materializer>>`, and the two `PaneHandle`s (spawned once).
   - A `Signal<TreeModel>`, refreshed after a save, relabel or rm, and every 2 s on a change of the nodes dir mtime.
   - Remove the test pattern pane; keep it under `STUDIO_SMOKE_SECS` only if useful.
2. **`keys.rs` (pure, tested):**
   - Maps `KeyboardEvent::code()` to `Buttons` with the play bin's layout: arrows, X = A, Z = B, A = L, S = R, Enter = Start, Backspace = Select.
   - F5 save, F8 rewind, F9 reload parent, Space pause, `.` step, Tab fast-forward while held.
   - `prevent_default` on handled keys (WebView2 reloads on F5).
   - The focused pane gets keys; blur releases all.
3. **Play tab:** native and ROM canvases side by side, controls (pause, step, ff×2/×4/×8, rewind, save, new game, build ROM), the inspector panel and the event list.
   - ROM pane: the timeline table (last 60 rows plus worst values), the boot code by name, the build banner, Export save.
4. **Checkpoints tab:**
   - The tree (indent by depth) with status badges (valid, stale, diverged with the first-mismatch frame, broken with the message, unverified) and a Verify button per store (background).
   - Per row: Load native, Load ROM, Open at frame (slider over `trace_frames` → `NativeCmd::Seek`), Label (inline), Remove (confirm; disabled with children), and "Branch" (= load native and focus).
   - Shared-store relabel or remove asks for confirmation ("committed tree").
5. **Smoke tests (`ui` feature, no display):**
   - `dioxus_ssr` renders `App` with a test context (store on a temp dir, panes not spawned) and finds the tab labels and tree rows.
   - `keys.rs` unit tests.
   - `cargo xtask studio --smoke 5` documented (macOS, manual or CI with display).

### Task H — UI: map and data views

**Owns:** `crates/studio/src/ui/{map.rs, data.rs}`

1. **Map tab:**
   - A map list (`map_list`), and a zoomable image. `render_map` is cached in memory per map, and the PNG-free RGBA is served by the same asset handler under `/maps/<id>` (add the route; describe the `ui/mod.rs` change for G).
   - Overlay toggles: collision, behavior, elevation, warps, objects (sprites), coords, bgs, connections. They are an SVG layer over the image.
   - A hover cell shows `CellInfo`. Clicking a warp or connection opens that map.
   - "Show live map" follows the native pane's map and camera.
2. **Data tab:** sub-tabs Species, Moves, Trainers, Encounters, Texts, with a filter box and a detail panel. Links: species → trainers and maps, and move → learners. Read-only.
3. **Smoke:** ssr render of each view in stub mode finds the expected row counts.

### Task A2 — battle sub-digests (only if Task A left a margin under 8 lines)

**Owns:**
- the battle `hash` impls named in A's brief
- the `world/tests.rs` goldens
- the README hash notes
- `checkpoints/*/nodes` (`verify --accept`)

Cache digests of rarely changing battle state (parties, battlers, palettes, healthbox label tiles) behind dirty flags, as `Vars::digest` does. The parity route must stay identical between ROM and native.

### Wave 2 close: gates
G adds `cargo xtask studio --smoke` to the README Tests paragraph. Every gate in "Rules" must pass on the merged tree.

---

## Wave 3 — close-out

- **Whole-milestone review:** threading (no `World` crossing threads), store write locations, FR literals, and UI logic that belongs in core. Fix the findings.
- **After a 10-minute studio session,** `git status` shows only intended `nodes/` changes or nothing (local store ignored).
- **This doc:** "As built" (spike numbers, final contracts, frame headroom numbers, studio fps and CPU, seek timings on the extracted route) and "Findings for Milestone 7 (Explorer)". The findings cover:
  - findings in the studio ("Open at frame before" = `seek` on a flattened explorer trace)
  - a coverage overlay on `map_view`
  - whether explorer nodes share `Node` files or stay in `explore.db` (M5 decision 7)
  - a background `Materializer` per thread
  - the `Parent::Edited` decision if the state editor is next
- **Cleanup:** clean up worktrees.

## Done when

- `cargo run -p studio --features ui` (or `cargo xtask studio`) opens the studio. On the extracted build:
  - Native pane: plays the route at 60 fps with keyboard input. Pause, step, fast-forward ×8, F8 rewind, F5 save (to the local store) and F9 work. "Open at frame" on `route:after_wild` lands mid-trace with the same hash as a straight replay. The inspector shows frame, map, hash, mode and the capturable reason.
  - ROM pane: boots `target/game.gba` as a new game and from `route:lab` (`boot == RESTORED`) and shows the timeline. A ROM/native build mismatch shows a banner, and "Build ROM" rebuilds and reloads.
  - Tree: shared and local nodes with Verify statuses. Label, rm, load into native or ROM, and branch-then-save produce a local node that `cargo xtask checkpoints verify --root checkpoints/emerald/local` reports `Valid`.
  - Map view: every exported map with overlays. The pixel-identity test passes.
  - Data views: species, moves, trainers, encounters and texts browse and filter.
- `cargo xtask studio --smoke` reports at least 58 fps per pane in release on macOS.
- All gates pass: fmt, clippy (incl. `game-native/window` and `studio/ui`), tests real and stub, `cargo test -p studio --features ui`, `cargo xtask rom` budgets, `cargo xtask parity` (`late == 0`, the loop gate as tightened by Task A), and game-gba fmt/clippy.
- GBA `loop_lines_max` margin is at least 8 lines, or A2 is done with goldens and trees re-accepted.
- No ROM-derived file is committed; no FR literals in tests.

---

## Open questions / decisions taken

1. **Dioxus 0.7.10 desktop (taken).** It is the stable line; 0.8 is alpha-only (0.8.0-alpha.3, 2026-10-16). Not Blitz/native: its webview-free renderer is still maturing, and we need only one canvas. Pin the minor version in `Cargo.lock`, and upgrade to 0.8 after its stable release in its own commit.
2. **Frame transport: asset handler + `fetch` + `putImageData` (taken; Wave 0 spike gates it).**
   - Binary bytes, no JSON. It is pull-based, so the webview never queues stale frames: it asks with `after=seq` and gets 204 when nothing is new.
   - `eval().send()` serializes JSON (about 600 KB per frame as a number array); base64 is about 205 KB of string and a decode in JS. Fallback 1 only.
   - A native child window (minifb, wgpu) cannot be laid out with the HTML panes and needs per-OS parenting. Fallback 2 is a pop-out process.
3. **`ui` feature (taken).** It mirrors `game-native/window`. `cargo test --workspace` builds only the core, so CI without webkit2gtk/WebView2 stays green. The `ui` gates run on macOS dev machines. Linux CI would need `libwebkit2gtk-4.1-dev` + `libxdo-dev` (open: add a CI job later).
4. **One thread per pane (taken).** `World` is `!Send`. The UI thread never steps the game, so the UI cannot stall the game at 60 Hz and the reverse is also true.
5. **Read-only map and data views (taken).** Editing is not v0: write-back targets would be ROM-derived, git-ignored files (`content/extracted/`), lost on re-extract, and hot reload needs the M8 content pipeline. It is not "cheap".
6. **State editor deferred (decision 17 stands).** No `Parent::Edited` in M6. Seek plus branch covers "start here" for now.
7. **Personal saves go to the local store; shared trees change only through the CLI** (`seed`, `verify --accept`).
   - Branching from a shared node saves a flattened NewGame trace (`trace::flatten`), unless the close-out added cross-store parents.
   - Cost of flattening: a route-length replay on verify (about 19k frames, well under a second natively) and no visible link to the shared parent in the tree. The label records it: `from route:lab f…`.
8. **`verify --accept` stays CLI-only (taken).** Re-baselining is a commit-worthy act, like goldens.
9. **Map view through `Field` + `Ppu` (taken).** It is pixel-identical by construction (same `metatile_layers`, tileset upload, border and connections), needs no engine change, and is tested against the live session.
10. **ROM pane: no rewind and no mid-trace seek in v0.** mGBA savestates are out of scope. A ROM seek is "load the nearest node, then feed the trace keys" (a cheap later addition).
11. **Event timeline from state diffs (taken).** The engine still has no `FrameOut` event list (design §3). Diffs of map, script, battle and `Log` give the v0 timeline without touching engine or hash.
12. **Frame headroom:** (a) buffered `Fnv64` + an ARM fold in IWRAM behind `engine/iwram` (the engine stays game-agnostic; only game-gba enables it). (b) battle sub-digests only if (a) leaves under 8 lines (A2), because it costs a golden and tree re-accept.
13. **Open: the dev profile.** Native play and seeks in debug builds may not reach 60 fps. `cargo xtask studio` uses `--release`. Optional: `[profile.dev.package.{engine,game-native,studio}] opt-level = 2` in the root `Cargo.toml` (decide in Wave 0 from the spike's debug numbers).
14. **Open: shared-store relabel or rm from the studio.** It is allowed with a confirmation, since node files are committed and the user commits them. Alternative: shared read-only in v0.

---

## Wave 0 as built
*(filled by Wave 0: spike fps and latency per build, final local-store names, contract deltas.)*

**Step 0 outcome (taken).**
- The M5 close-out built a layered store.
  - `Store::layered(base, local)` writes personal saves to `checkpoints/<flavor>/local/` (git-ignored), or to `$PKR_CHECKPOINTS` when set.
  - The committed tree is a read-only base.
  - Local ids start above `LOCAL_IDS` (10000).
  - A local node may have a committed parent.
- Cross-store parents exist, so `trace::flatten` and `tree::plan_save` are dropped (decision 7 is superseded).
- `Stores` becomes one layered `Store`. `Origin` is only a per-node display and permission flag (base nodes are read-only), and commands are keyed by `CheckpointId`.

**Scheduling.** The studio skeleton (`crates/studio`, the `Pane` trait and `RunMode`) was committed first. The spike, A, D, E and F then started in parallel. B and C started after the M5 close-out merged, against the layered store. `StoreError` is B's job, and C writes its new APIs with `String` errors until Wave 2 aligns them.

---

### Dioxus research note

- **Version.** The newest stable is **dioxus 0.7.10** (2026-07-30). The 0.8 line is in alpha: 0.8.0-alpha.3 came out 2026-10-16 ([crates.io API](https://crates.io/api/v1/crates/dioxus)). 0.7.10 is already in the local cargo registry, so it builds offline.
- **What the 0.7 desktop renderer is.** It is wry/tao with a system webview. Rust runs natively, so "rendering WebGL, Canvas, etc. is not as easy as the Web" and has to go through JS ([Dioxus 0.7 desktop guide](https://dioxuslabs.com/learn/0.7/guides/desktop)).
- **APIs I checked in the 0.7.10 source** (`dioxus-desktop-0.7.10`):
  - `use_asset_handler(name, FnMut(AssetRequest, RequestAsyncResponder))` (`hooks.rs:92`). It routes by the first path segment (`protocol.rs:76`), so `/frames/native?after=N` reaches handler `"frames"`.
  - `Config::with_custom_protocol` / `with_asynchronous_custom_protocol` (`config.rs:226,260`).
  - `Config::with_custom_head` (`config.rs:278`).
  - Pages are served from `dioxus://index.html/`, or `http://dioxus.index.html/` on Windows (`protocol.rs:19-22`), so a relative `fetch` is same-origin.
  - `document::eval` returns an `Eval` whose `send`/`recv` carry JSON values (`dioxus-document-0.7.10/src/eval.rs:31-41`). That is why it is not the frame path.
  - `dioxus` has an `ssr` feature (`dioxus-ssr`) for the display-free smoke test.
- **Not verified:** I found no published benchmark of 60 fps frame streaming through a wry custom protocol. That is why the Wave 0 spike has numeric acceptance and two fallbacks.
- **Other source checked:** [docs.rs dioxus 0.7.6](https://docs.rs/crate/dioxus/0.7.6) for the 0.7 crate layout.
