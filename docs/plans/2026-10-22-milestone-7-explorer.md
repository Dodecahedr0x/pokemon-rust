# Milestone 7 — Explorer v0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** a headless, persistent, evolutionary **explorer** (`crates/explore`, a library plus the `explore` bin, driven by `cargo xtask explore`). It plays the game natively from a new game. It grows a tree of input-only nodes in `explore.db` (SQLite) and reports bugs, each with a reproducing trace:
- **Macro-actions:** `WalkTo`, `Enter`, `Cross`, `Talk`, `Inspect`, `Grass`, `Settle`, each with menu and battle options. They are derived from the state and expanded to inputs deterministically by pathfinding on collision data, `setmetatile` overrides and object positions.
- **Abstract keys:** map, coarse cell, story flags and vars, party bucket. Nodes with the same key merge: the one that reached more goals wins, then the one with fewer frames (Task G as built).
- **Fitness:** novelty (new flags, var values, maps, scripts started, cells, keys), plus progress along a hand-written goal list for the slice, plus bug signal. Balance is recorded but weighted 0 in v0.
- **Bug signals:**
  - native panics (`catch_unwind`, with the Rust location, map, cell, last script started and log)
  - softlocks: the player is trapped, with no new region reachable within K macro-steps on a map that has exits
  - invariant breaks: the checkpoint round trip and shadow follow, `capturable` stuck outside a battle or script, planner/engine divergence, and slow frames (report-only)
- **Persistence:** `target/explore/{stub,emerald}/explore.db`, git-ignored. Nodes are `(parent, macro, RLE inputs, build, key, digest, fitness)`. States are reached by replay from the nearest cached snapshot (memory LRU, then the db `snapshot` table, then a new game).
- **Rebuilds:** v0 re-validates every node when the build changes. It replays the inputs, checks the digest, re-expands the macro if they diverge, and otherwise marks the node `broken`. Touched maps and scripts are recorded now for the incremental version.
- **Studio (Wave 2b, gated on M6):** a findings list, "Open at frame before" (replays a flattened NewGame trace on the native pane) and a coverage overlay on `map_view`.

**Architecture**
- **Every node ends on a capturable frame.** A macro's expansion always finishes by settling until `World::capturable()` is `Ok`. That gives three things with **no engine change**:
  - every node has a `Checkpoint` (`Send`), so workers restore from it;
  - the abstract key and the goals are pure functions of `WorldSnapshot` (`checkpoint.rs:104`: map, player position, `FieldSnap.overrides`, `VarsSnap` flags and vars, party);
  - pathfinding reads `setmetatile` overrides from `FieldSnap.overrides`.
- **Script coverage without engine events:** `Probe<G>` wraps the `GameData` and records every `GameData::script(id, ctx)` call (`data.rs:371`). It delegates all 18 trait methods, so the game behaves identically: same hashes, same `build_hash`. The engine stays game-agnostic and untouched.
- **Batch-synchronous generations.** Each generation runs in three steps:
  1. **Plan (sequential, seeded `SplitMix64`):** tournament selection with an age penalty, then a mutation op per job.
  2. **Simulate (rayon, `par_iter`, order-preserving):** each worker builds its own `Session<Probe<G>>` from an `Arc<Checkpoint>`, because `World` is `!Send`.
  3. **Merge (sequential, in job order):** keys, novelty, best-by-key, ids. It produces a `GenerationDelta` that the single SQLite writer applies in one transaction.

  The results therefore do not depend on the thread count. A test checks this.
- **Seams so that Wave 1 tasks are independent:**
  - The runner is pure scheduling. It sees game logic only through `Job`/`JobResult` and the `Evaluate` trait.
  - Detectors plug into the expander through `StepObserver`.
  - The softlock BFS and the repair closure take closures.
  - Wave 2 wires everything (`wire.rs`).
- **Emerald-specific knowledge** (goal list, key exclusions) lives in `explore::goals` and `explore::key` (on top of `content::consts`), never in `engine`.

**Tech stack**
- Rust 2024.
- The workspace crates `engine`, `content` and `game-native` (existing APIs only).
- `rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }`. This pulls `libsqlite3-sys 0.38.2` (SQLite 3.53.2, compiled by `cc`), `fallible-iterator 0.3`, `fallible-streaming-iterator 0.1.9`, `smallvec`, `bitflags 2` and `cc` (1.9.0 is already in `Cargo.lock`).
- `rayon = "1.12"` (1.12.0, with `rayon-core 1.13.0`, `crossbeam-deque`/`-epoch`/`-utils` and `either`).
- `serde` 1, `ron = "0.8"` (as game-native), `anyhow` (bin), and `tempfile = "3.27"` (dev; already in the lock through game-native).

A task may add to a contract if it needs to. It may not change the meaning of what is already there, and every addition must be listed in its final report.

### Offline build check (done while planning)
These are all in `~/.cargo/registry` as both `cache/*.crate` and `src/`:
- `rusqlite-0.40.2`, `0.39.0` and `0.37.0`
- `libsqlite3-sys-0.38.2` (its `sqlite3/sqlite3.c` is vendored: `SQLITE_VERSION "3.53.2"`), `0.37.0` and `0.35.0`
- `rayon-1.12.0`/`1.11.0`/`1.10.0`, `rayon-core-1.13.0`/`1.12.1`
- `hashlink-0.12.2` + `hashbrown-0.17.x`, `fallible-*`, `smallvec-1.16.x`, `cc-1.9.0`
- the index cache holds `rusqlite` and `sqlite-wasm-rs`

`default-features = false` is deliberate:
- rusqlite 0.40's defaults are `["cache", "ffi-sqlite-wasm-rs"]`. The wasm FFI is a wasm-only optional dependency we do not want in the lock.
- `cache` (`prepare_cached`, through `hashlink 0.12` → `hashbrown 0.17`) is not needed in v0: statements are prepared once per `apply`.
- **Wave 0 acceptance:** `cargo build -p explore --offline` succeeds. Fallback if it does not: `rusqlite = "=0.39.0"` (→ `libsqlite3-sys 0.37.0`), also cached.
- Bundled SQLite adds about 20–40 s to a clean build (one `sqlite3.c` compile). Nothing links a system libsqlite.

---

## Current code this builds on

- **Native shell:**
  - `Session` (`crates/game-native/src/lib.rs:27-120`): `new :39`, `restore(game, &Checkpoint) :46`, `load :53`, `checkpoint :60`, `step :87`, `game :93`, `world :97`.
  - Headless: no rendering unless `framebuffer` is called.
  - `Session` holds `World`, which is `!Send` (`Ctx(Rc<RefCell<Io>>)`, `engine/src/script.rs:304`).
- **Scenario driver (the reference for Task A; not reused, see decision 1):** `crates/game-native/src/scenario.rs`:
  - `Action` `:24-63`, `Recording` `:88`, `record` `:209`, `cached_route` (after `:873`), `EMERALD_ROUTE :776`, `STUB_ROUTE :843`.
  - Private `Recorder::{press :255, tap :290 (no presses on black battle/warp screens), idle :301, battle_key :311, battle :342, settle :414, next_dir :706, path_dir :710 (BFS over collision bits, ledges, live object pos/prev; tall grass avoided first)}`.
  - Route moments: lab 4856, after_wild 7064, oldale_center 11571, route103_after_may 19082 (M5 As built).
- **Traces:** `crates/game-native/src/trace.rs`:
  - `Run :52`, `rle :70`, `Trace :82` (`frames :139`, `keys :146`), `TraceRecorder :160` (`new`, `push(keys, hash_after)`, `trace(parent, build)`), `replay :225`, `replay_with_hashes :241`, `Parent::{NewGame, Checkpoint}`, `TRACE_SCHEMA` 2, `HASH_EVERY` 60.
  - The `runs_text` serde module is private, so explore keeps its own run text codec.
- **Engine World API:**
  - `crates/engine/src/world.rs`: `frame :518`, `map :522`, `objects :526`, `flag :534`, `var :538`, `log :542` (`Log::{count, lines}` `:71-100`), `script_running :546`, `transition_active :550`, `party :555`, `field_active :581`, `in_battle :586`, `battle :591` (`(Stage, &Battle, &BattleScene)`), `starter :599`, `hash :610`.
  - `crates/engine/src/world/checkpoint.rs`: `capturable :27` (Starter, Battle, Transition, Script, TextBox, Fade, Moving, StaleView, in that order), `snapshot :57`, `checkpoint :87`, `restore :103`.
- **Checkpoint model:** `crates/engine/src/checkpoint.rs`:
  - `BuildHash :44`, `build_hash :53`, `Checkpoint :75`, `WorldSnapshot :104` (field, objects, vars, party, respawn…), `FieldSnap :140` (`map`, `overrides`), `VarsSnap :235` (set flags ascending, non-zero vars ascending), `NotCapturable :262`.
  - `encode :308`, `decode -> Result<_, CodecError> :361`, `state_digest :377`.
- **Battle:**
  - `Battle::{mon(Side) -> &BattleMon :512, setup :521}` (`engine/src/battle/core.rs`).
  - `BattleMon { moves, pp, max_pp, level, hp, max_hp, … }` (`battle/mod.rs`).
  - `BattleScene::{finished :478, open_menu :484}` and `OpenMenu :203` (Action/Move/LearnAsk/LearnPick/StopAsk) (`battle/scene.rs`).
  - `BattleKind`, `BattleOutcome` (`repr(u8)`), `MoveInfo.power` (`mon.rs:333`).
- **Text and starter screen:** `TextBox::{is_yes_no_active :447, is_pristine :480}`, `StarterChoice::selection`.
- **Map data and collision:**
  - `engine/src/data.rs`: `block_collision :91`, `MapData :268` (warps, coords, bgs, connections, objects), `ObjectEvent` (`script`, `hide_flag`), `BgKind`, `Connection`, `Behavior` (`Ledge(Dir)`, `TallGrass`, `Door`, `Warp`, `Blocked`).
  - `grid::{in_bounds :13, block_at :50}`.
  - `flow::{block_behavior, door_warp, warp_on_enter, warp_on_push, coord_trigger, interact_target}` (`flow.rs:17-21`).
  - `flow::{FLAG_COUNT, TEMP_FLAGS_END, VARS_START, TEMP_VARS_END, SPECIAL_VARS_START}`.
- **`GameData` trait** (`data.rs:351-410`): 18 methods.
  - Required: `map`, `sprite`, `player_sprite`, `font`, `window`, `text`, `ui_text`, `behavior`, `script`, `new_game`.
  - Provided: `has_map`, `special_ids`, `build_id`, `battle`, `encounters`, `trainer`, `battle_text`, `battle_gfx`, `heal_location`.
- **Content:**
  - `content::{Emerald::for_shells, STUB, BUILD_ID, has_map, STALE_EXTRACT}`.
  - `content::data::{SCRIPT_NAMES, MAP_IDS, map}` (decomp identifiers, not ROM text).
  - `content::consts::{MAP_ROUTE101, MAP_ROUTE103, MAP_OLDALE_TOWN…, MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB, FLAG_RESCUED_BIRCH, FLAG_SYS_POKEMON_GET, FLAG_DEFEATED_RIVAL_ROUTE103, HEAL_LOCATION_OLDALE_TOWN}`. These exist in both flavors.
  - Stub world: `crates/content/build/stub.rs` (16x16 `StubTown`, start (5,5), one tall-grass cell (12,3), no events).
- **xtask:** `xtask/src/main.rs:300-304` dispatches `rom`, `parity`, `shot`, `extract`, `checkpoints`. The M6 Wave 0 adds `studio` next to them.
- **Ignore rules:** `.gitignore` already ignores `target/` and `explore.db`.
- **Not used in Wave 1 (M6-owned):** `store.rs`, `checkpoints.rs`, `controls.rs`, `trace.rs` additions (`from_prefix`, `flatten`), `engine/src/hash.rs`, `crates/studio/**`.

---

## Wave 0 contracts (committed by you before Wave 1; read them, do not redesign them)

Wave 0 adds the crate, dependencies, xtask dispatch, the SQLite DDL, the shared model types, four small implemented seams (`runs`, `probe`, `guard`, `observe`) and every other signature below with a `todo!()` body. No Wave 1 task then edits `Cargo.toml` member lists, `lib.rs` module lists or xtask dispatch.
- **Engine additions:** none. No hash change. No `game-native` change.
- `cargo test --workspace` (real and stub) stays green.

### Step 0 — reconcile and baseline
1. Rebase on master after the M6 Wave 0 merge. If `xtask/src/main.rs` already has `mod studio;`, add `mod explore;` next to it. That is the only expected conflict, plus `Cargo.lock`, which you regenerate.
2. `cargo build -p explore --offline` (rusqlite bundled).
3. **Baseline:** replay `scenario::cached_route()` single-threaded in `--release` and record frames/s in "Wave 0 as built". The nodes/s thresholds in "Done when" assume at least 50k frames/s per core. If the baseline is lower, scale them and record why.

### Workspace and crate
- **Root `Cargo.toml`:**
  - add `"crates/explore"` to `members` and `explore = { path = "crates/explore" }` to `[workspace.dependencies]`;
  - add `rayon = "1.12"` and `rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }` to `[workspace.dependencies]`.
- **No `panic = "abort"` in any profile.** The explorer relies on unwinding (decision 10).
- **`crates/explore/Cargo.toml`:**
  ```toml
  [package]
  name = "explore"
  edition.workspace = true
  version.workspace = true
  publish.workspace = true

  [dependencies]
  engine.workspace = true
  content.workspace = true
  game-native.workspace = true
  anyhow.workspace = true
  rayon.workspace = true
  rusqlite.workspace = true
  serde = { version = "1", features = ["derive"] }
  ron = "0.8"

  [dev-dependencies]
  tempfile = "3.27"

  [[bin]]
  name = "explore"
  path = "src/bin/explore.rs"
  ```

### Module layout (`src/lib.rs`)
```rust
pub mod model;     // implemented (W0): shared data types
pub mod config;    // implemented (W0): Config + defaults + RON load
pub mod runs;      // implemented (W0): run text codec
pub mod probe;     // implemented (W0): Probe<G> GameData wrapper
pub mod guard;     // implemented (W0): panic capture
pub mod observe;   // implemented (W0): StepObserver + NoObserver
pub mod macros;    // Task A
pub mod path;      // Task A
pub mod expand;    // Task A
pub mod sim;       // Task A
pub mod key;       // Task B
pub mod fitness;   // Task B
pub mod goals;     // Task B
pub mod db;        // Task C (+ schema.sql, implemented W0)
pub mod cache;     // Task C
pub mod rebuild;   // Task C
pub mod graph;     // Task D
pub mod select;    // Task D
pub mod mutate;    // Task D
pub mod runner;    // Task D
pub mod detect;    // Task E
pub mod softlock;  // Task E
pub mod findings;  // Task E
pub mod testing;   // Task E: fault-injection GameData wrappers (pub, #[doc(hidden)])
pub mod policy;    // Task F
pub mod balance;   // Task F
pub mod wire;      // Task G (Wave 2): Evaluate/sim/db glue
pub mod cli;       // Task G (Wave 2)
pub use model::*;
```
`src/bin/explore.rs` (W0) is `fn main() -> anyhow::Result<()> { explore::cli::main(std::env::args().skip(1).collect()) }`. `cli::main` is `todo!()` until Task G.

### `model.rs` (implemented: data types only, no logic)
```rust
use engine::checkpoint::{BuildHash, Checkpoint};
use engine::data::{Dir, MapId, ScriptId};
use game_native::trace::Run;
use std::collections::BTreeSet;
use std::sync::Arc;

/// SQLite rowid of a node; assigned by the runner in merge order (deterministic). 1 = root.
pub type NodeId = i64;

/// Menu and battle choices a macro applies to whatever it triggers.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct Opts {
    /// Run from wild battles (not the first battle, never trainers).
    pub run_wild: bool,
    /// 0 = yes / first option, 1 = no / second, 2 = third (starter ball index).
    pub choice: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum MacroKind {
    /// Press nothing (and resolve scripts with the policy) until capturable. The root's macro.
    Settle,
    /// Walk to a cell of the current map.
    WalkTo { x: i16, y: i16 },
    /// Walk next to warp cell (x, y) and move `dir` into it until a transition starts.
    Enter { x: i16, y: i16, dir: Dir },
    /// Walk to edge cell (x, y) and hold `dir` until the map changes, then walk on until
    /// the view is no longer stale.
    Cross { x: i16, y: i16, dir: Dir },
    /// Walk next to object `local_id`, face it, press A, resolve the script.
    Talk { local_id: u8 },
    /// Face bg event (x, y) from the adjacent cell `dir` points from, press A, resolve.
    Inspect { x: i16, y: i16, dir: Dir },
    /// Pace between tall-grass cell (x, y) and a walkable neighbour until a battle starts.
    Grass { x: i16, y: i16 },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Macro { pub kind: MacroKind, pub opts: Opts }

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub struct AbstractKey(pub u64);

/// The parts of an abstract key. `region` = the key without the cell (softlock, stall).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyParts {
    pub map: MapId,
    pub cell: (i16, i16),          // coarse: (x >> shift, y >> shift)
    pub flags: u64,                // digest of story flags (KeyConfig exclusions applied)
    pub vars: u64,                 // digest of story vars
    pub party: PartyBucket,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct PartyBucket {
    pub count: u8,
    pub species: [u16; 6],         // party order, 0 = empty
    pub level_bucket: u16,         // sum of levels / KeyConfig::level_step
    pub fainted: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Fitness { pub novelty: i64, pub progress: i64, pub bug: i64, pub balance: i64, pub total: i64 }

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum NodeStatus { Valid, Stale, Merged, Broken }

#[derive(Clone, PartialEq, Debug)]
pub struct NodeRow {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub depth: u32,
    pub macro_: Macro,
    /// From the parent's end state (a new game for the root) to this node's end state.
    pub inputs: Vec<Run>,
    pub frames: u32,
    pub total_frames: u64,
    pub end_frame: u32,            // World::frame at the end
    pub key: AbstractKey,
    pub region: u64,
    pub digest: u64,               // state_digest of the end snapshot
    pub world_hash: u64,
    pub map: MapId,
    pub pos: (i16, i16),
    pub progress: u32,             // goals reached along the lineage
    pub fitness: Fitness,
    pub generation: u32,
    pub expanded: u32,
    pub status: NodeStatus,
    pub merged_into: Option<NodeId>,
    pub build: BuildHash,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BattleSummary {
    pub kind: u8,                  // 0 wild, 1 first battle, 2 trainer
    pub trainer: Option<u16>,
    pub outcome: Option<u8>,       // BattleOutcome as u8
    pub frames: u32,
    pub player_level: u8,
    pub opponent_level: u8,
    pub hp_lost_permille: u16,
}

/// What one expansion saw. Everything but `max_step_ns` is deterministic.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SimSummary {
    pub frames: u32,
    pub maps: Vec<MapId>,                      // entered, in order (consecutive duplicates dropped)
    pub cells: BTreeSet<(MapId, i16, i16)>,    // player cells stood on
    pub scripts: BTreeSet<ScriptId>,           // started (Probe)
    pub battles: Vec<BattleSummary>,
    pub log_lines: u32,                        // World::log().count() delta
    pub max_step_ns: u64,                      // host time, report-only
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum FindingKind { Panic, Softlock, RoundTrip, CapturableStuck, PlannerDivergence, SlowFrame }

/// A bug seen during a job, with everything needed to reproduce it.
#[derive(Clone, PartialEq, Debug)]
pub struct FindingDraft {
    pub kind: FindingKind,
    pub message: String,
    pub location: Option<String>,  // Rust file:line:col of a panic
    pub macro_: Option<Macro>,
    /// Inputs from the job's parent end state to the frame the finding was seen (inclusive).
    pub tail: Vec<Run>,
    pub map: MapId,
    pub pos: (i16, i16),
    pub script: Option<ScriptId>,  // last started
    pub log: Vec<&'static str>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Touch { Map(MapId), Script(ScriptId) }

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum SeenKind { Flag = 0, VarValue = 1, Map = 2, Script = 3 }

/// What a job does from its start state.
#[derive(Clone, PartialEq, Debug)]
pub enum Plan {
    /// Pick `len` macros at random among the state's candidates (seeded), in sequence.
    Random { len: u8, seed: u64 },
    /// Run these macros in sequence (opts flips, re-expansion).
    Given(Vec<Macro>),
    /// Softlock probe: bounded BFS over macros (Task E's `softlock::probe`).
    Softlock { k: u32, breadth: u32 },
}

#[derive(Clone, Debug)]
pub struct Job {
    pub index: u32,
    pub parent: Option<NodeId>,                // None = from a new game (the root)
    pub start: Option<Arc<Checkpoint>>,        // None = World::new
    pub plan: Plan,
}

#[derive(Clone, Debug)]
pub struct StepResult {
    pub macro_: Macro,
    pub inputs: Vec<Run>,
    pub end: Arc<Checkpoint>,                  // capturable end state
    pub summary: SimSummary,
}

#[derive(Clone, Debug)]
pub enum Stop {
    /// The plan could not continue (no candidate, no path, no effect): not a bug.
    Pruned(String),
    Finding(FindingDraft),
    /// The softlock probe's verdict for the start state.
    NotTrapped,
}

#[derive(Clone, Debug)]
pub struct JobResult {
    pub index: u32,
    pub parent: Option<NodeId>,
    pub steps: Vec<StepResult>,                // each becomes a candidate node, chained
    pub stop: Option<Stop>,
}

/// The merge's output: one SQLite transaction.
#[derive(Clone, Debug, Default)]
pub struct GenerationDelta {
    pub generation: u32,
    pub merged: Vec<(NodeId, NodeId)>,         // (old, new): old -> Merged, applied before inserts
    pub expanded: Vec<(NodeId, u32)>,          // expanded += n
    pub nodes: Vec<NodeRow>,
    pub snapshots: Vec<(NodeId, Arc<Checkpoint>)>,
    pub touches: Vec<(NodeId, Touch)>,
    pub seen: Vec<(SeenKind, i64, NodeId)>,    // first sightings; existing ones get hits += 1
    pub cells: Vec<(MapId, i16, i16, NodeId)>,
    pub battles: Vec<(NodeId, BattleSummary)>,
    pub findings: Vec<(FindingDraft, NodeId /* tail starts at this node's end */, u64 /* total frame */)>,
    pub stats: GenStats,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GenStats { pub jobs: u32, pub steps: u32, pub inserted: u32, pub duplicates: u32, pub pruned: u32, pub frames: u64 }

/// The scorer the runner merges with (Task B implements it; tests use fakes).
pub trait Evaluate {
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation;
}
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub key: AbstractKey,
    pub region: u64,
    pub progress: u32,
    pub fitness: Fitness,
    pub seen: Vec<(SeenKind, i64)>,
    pub touches: Vec<Touch>,
}
```
(`Serialize`/`Deserialize` are `serde::{Serialize, Deserialize}`. `Dir` already derives them, M5.)

### `config.rs` (implemented)
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub seed: u64,                 // 0x5EED_E7
    pub threads: usize,            // 0 = rayon default
    pub batch: usize,              // jobs per generation; 0 = 4 x threads
    pub max_plan_len: u8,          // 3
    pub tournament: usize,         // 4
    pub age_penalty: i64,          // per generation since creation
    pub expanded_penalty: i64,     // per child already tried
    pub weights: Weights,
    pub key: KeyConfig,
    pub limits: Limits,
    pub detect: DetectConfig,
    pub cache_capacity: usize,     // snapshots kept in memory (4096)
    pub snapshot_every_depth: u32, // also store a snapshot in the db every N depths (8)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Weights { pub flag: i64, pub var_value: i64, pub map: i64, pub script: i64, pub cell: i64,
                     pub new_key: i64, pub goal: i64, pub bug: i64, pub balance: i64 /* 0 in v0 */ }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyConfig { pub cell_shift: u8 /* 2 = 4x4 cells */, pub level_step: u16 /* 3 */,
                       pub ignore_flags: Vec<u16>, pub ignore_vars: Vec<u16> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Limits { pub macro_frames: u32 /* 3000 */, pub battle_frames: u32 /* 12000 */,
                    pub settle_frames: u32 /* 1200 */, pub enter_frames: u32 /* 120 */, pub grass_frames: u32 /* 2400 */ }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectConfig {
    pub roundtrip_every: u32,      // check at the first capturable frame after every N frames (600); 0 = off
    pub follow: u32,               // shadow-session frames after a round trip (60)
    pub capturable_stuck: u32,     // idle-but-not-capturable frames before a finding (600)
    pub softlock_k: u32,           // macro depth of the trap probe (3)
    pub softlock_k_per_map: BTreeMap<u16, u32>,
    pub softlock_breadth: u32,     // expansions per probe (64)
    pub softlock_sample: u32,      // probe 1 in N new nodes, plus every node whose candidates all failed (32)
    pub slow_step_us: u32,         // report-only (0 = off; default 0 in debug, 4000 in release)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget { Nodes(u64), Frames(u64), Generations(u32), Secs(u64) }
impl Default for Config { /* the values above */ }
impl Config { pub fn load_or_default(path: &Path) -> anyhow::Result<Config>; }
```

### `runs.rs` (implemented)
```rust
/// "keys*frames keys*frames …" (keys = raw Buttons bits), the Trace node-file run syntax.
pub fn to_text(runs: &[Run]) -> String;
pub fn from_text(s: &str) -> Result<Vec<Run>, String>;
pub fn keys(runs: &[Run]) -> impl Iterator<Item = Buttons> + '_;
pub fn frames(runs: &[Run]) -> u32;
```

### `probe.rs` (implemented)
```rust
/// A GameData that records every script started (`GameData::script`) and otherwise delegates
/// all 18 methods to `G` (provided ones too, so `build_id`, `special_ids`… are G's).
pub struct Probe<G> { inner: G, started: RefCell<Vec<ScriptId>> }
impl<G: GameData> Probe<G> {
    pub fn new(inner: G) -> Self;
    pub fn inner(&self) -> &G;
    /// Scripts started since the last call, in order.
    pub fn take_started(&self) -> Vec<ScriptId>;
}
impl<G: GameData> GameData for Probe<G> { /* every method delegates; `script` records first */ }
```
W0 tests:
- `Session<Probe<Emerald>>` replaying `cached_route()` gives the route's hashes on every frame.
- `build_hash(&probe) == build_hash(&inner)`.
- On the real flavor, at least one script is recorded.

### `guard.rs` (implemented)
```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Panicked { pub message: String, pub location: Option<String> }
/// Installs (once) a panic hook that, while a guard is active on this thread, stores the
/// message and `Location` in a thread-local and prints nothing; otherwise defers to the
/// previous hook.
pub fn install_hook();
/// `s.step(keys)` under `catch_unwind(AssertUnwindSafe(..))`. After `Err` the session must be
/// dropped.
pub fn step_guarded<G: GameData>(s: &mut Session<G>, keys: Buttons) -> Result<(), Panicked>;
```

### `observe.rs` (implemented)
```rust
/// A flag raised by an observer; the driver turns it into a FindingDraft with the context.
pub struct Flag { pub kind: FindingKind, pub message: String }
pub trait StepObserver<G: GameData> {
    /// After every stepped frame; `step_ns` is the host time of the step (report-only).
    fn after_step(&mut self, s: &Session<Probe<G>>, keys: Buttons, step_ns: u64) -> Option<Flag>;
}
pub struct NoObserver;
impl<G: GameData> StepObserver<G> for NoObserver { fn after_step(..) -> Option<Flag> { None } }
```

### Wave 1 stubs (signatures; `todo!()` bodies)
```rust
// path.rs (A)
/// Walkable cells reachable from the player, from a capturable snapshot (collision bits with
/// `FieldSnap.overrides`, ledges one-way, objects' pos and prev occupied; elevation ignored).
pub struct Reach { /* width, height, dist: Vec<u16> (u16::MAX = unreachable), map */ }
impl Reach {
    pub fn dist(&self, x: i16, y: i16) -> Option<u16>;
    pub fn cells(&self) -> impl Iterator<Item = (i16, i16)> + '_;   // row-major
}
pub fn reach<G: GameData>(game: &G, snap: &WorldSnapshot) -> Reach;
/// The first step toward `to` from the live world (re-planned every frame; tall grass only
/// when there is no other way). `overrides` come from the macro's start snapshot.
pub fn next_dir<G: GameData>(game: &G, w: &World, overrides: &[((i16, i16), u16)], to: (i16, i16)) -> Option<Dir>;

// macros.rs (A)
/// Every applicable macro of a capturable state, deterministic order, deduplicated, default
/// `Opts`. Order: Enter, Cross, Talk, Inspect, Grass, then WalkTo (coord-event cells first,
/// then one cell per coarse block, nearest its centre).
pub fn candidates<G: GameData>(game: &G, snap: &WorldSnapshot, reach: &Reach) -> Vec<Macro>;
/// Whether `m` is still applicable to `snap` (mutations replaying a sibling's macro).
pub fn applicable<G: GameData>(game: &G, snap: &WorldSnapshot, reach: &Reach, m: &Macro) -> bool;

// expand.rs (A)
#[derive(Clone, PartialEq, Debug)]
pub enum ExpandError {
    NoPath, NoEffect(&'static str), Timeout(&'static str),
    Panicked(Panicked), Flagged(Flag),
}
pub struct Expanded { pub keys: Vec<Buttons>, pub summary: SimSummary }
/// Plays `m` on `s` to the next capturable frame. On error, `keys`/`summary` so far are
/// returned for the repro.
pub fn expand<G: GameData>(
    s: &mut Session<Probe<G>>, m: &Macro, policy: &mut Policy,
    obs: &mut dyn StepObserver<G>, limits: &Limits,
) -> Result<Expanded, (ExpandError, Expanded)>;
/// Root: a new game settled to its first capturable frame (`MacroKind::Settle`).
pub fn new_game<G: GameData>(game: G) -> Session<Probe<G>>;

// sim.rs (A)
/// Runs one job on a fresh session built on this thread. Plan::Softlock is wired by Task G.
pub fn simulate<G: GameData + Clone>(
    game: &G, job: &Job, cfg: &Config, obs: &mut dyn FnMut() -> Box<dyn StepObserver<G>>,
) -> JobResult;

// key.rs (B)
pub fn key_parts(snap: &WorldSnapshot, cfg: &KeyConfig) -> KeyParts;
pub fn key(parts: &KeyParts) -> AbstractKey;          // stable splitmix fold, platform-independent
pub fn region(parts: &KeyParts) -> u64;               // key without the cell
pub fn default_key_config() -> KeyConfig;             // content-aware exclusions (temp, special, step-counter vars)

// goals.rs (B)
pub struct Goal { pub name: &'static str, pub reached: fn(&GoalCtx) -> bool }
pub struct GoalCtx<'a> { pub snap: &'a WorldSnapshot, pub summary: &'a SimSummary }
/// The slice's goals in story order (`content::STUB` picks the stub list).
pub fn goals() -> &'static [Goal];
/// `parent` goals reached so far; advances over consecutive goals satisfied by `ctx`.
pub fn progress(parent: u32, ctx: &GoalCtx) -> u32;

// fitness.rs (B)
#[derive(Default)] pub struct Archive { /* BTreeMaps: flags, var values, maps, scripts, cells, keys -> hits */ }
impl Archive {
    pub fn from_db(seen: &[(SeenKind, i64, u32)], keys: &[(AbstractKey, u32)], cells: &[(MapId, i16, i16, u32)]) -> Archive;
}
pub struct Evaluator { pub archive: Archive, pub cfg: Config }
impl Evaluate for Evaluator { /* key_parts/key/region, novelty vs archive (then records),
                                 goals::progress, score */ }
pub fn score(novelty: &Novelty, progress_gain: u32, bugs: u32, w: &Weights) -> Fitness;
pub struct Novelty { pub flags: u32, pub var_values: u32, pub maps: u32, pub scripts: u32, pub cells: u32, pub new_key: bool }
/// Selection weight: fitness.total - age*age_penalty - expanded*expanded_penalty, with a
/// 1/(1+hits) novelty decay on the node's key (integer arithmetic only).
pub fn selection_score(n: &NodeRow, generation: u32, key_hits: u32, cfg: &Config) -> i64;

// db.rs (C)
#[derive(Debug)]
pub enum DbError { Sql(rusqlite::Error), Schema { found: i64, want: i64 }, Flavor { db: String, build: String }, Codec(String) }
impl Display + Error + From<rusqlite::Error> for DbError;
pub const SCHEMA_VERSION: i64 = 1;
pub struct OpenState { pub fresh: bool, pub build_changed: bool }
pub struct Db { /* rusqlite::Connection */ }
impl Db {
    pub fn default_path(stub: bool) -> PathBuf;              // <workspace>/target/explore/{stub|emerald}/explore.db
    pub fn open(path: &Path, build: BuildHash, stub: bool) -> Result<(Db, OpenState), DbError>;
    pub fn open_read_only(path: &Path) -> Result<Db, DbError>;
    pub fn apply(&mut self, d: &GenerationDelta) -> Result<(), DbError>;   // one transaction
    pub fn nodes(&self) -> Result<Vec<NodeRow>, DbError>;                 // id ascending
    pub fn node(&self, id: NodeId) -> Result<Option<NodeRow>, DbError>;
    pub fn lineage(&self, id: NodeId) -> Result<Vec<NodeRow>, DbError>;   // root first
    pub fn seen(&self) -> Result<Vec<(SeenKind, i64, u32)>, DbError>;
    pub fn coverage(&self, map: Option<MapId>) -> Result<Vec<(MapId, i16, i16, u32)>, DbError>;
    pub fn findings(&self) -> Result<Vec<FindingRow>, DbError>;
    pub fn finding(&self, id: i64) -> Result<Option<FindingRow>, DbError>;
    pub fn put_snapshot(&mut self, id: NodeId, cp: &Checkpoint) -> Result<(), DbError>;
    pub fn snapshot(&self, id: NodeId, build: BuildHash) -> Result<Option<Checkpoint>, DbError>;
    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<(), DbError>;
    pub fn meta(&self, key: &str) -> Result<Option<String>, DbError>;
    pub fn mark_all_stale(&mut self) -> Result<u64, DbError>;
    pub fn set_status(&mut self, rows: &[NodeRow]) -> Result<(), DbError>;   // revalidation results
    pub fn record_run(&mut self, r: &RunRow) -> Result<(), DbError>;
    pub fn stats(&self) -> Result<DbStats, DbError>;
    /// FNV of the node, finding, seen and coverage tables in key order, without timings or
    /// run rows (determinism tests, `explore digest`).
    pub fn content_digest(&self) -> Result<u64, DbError>;
}
pub struct FindingRow { pub id: i64, pub signature: u64, pub kind: FindingKind, pub node: NodeId, pub macro_: Option<Macro>,
    pub tail: Vec<Run>, pub total_frame: u64, pub map: MapId, pub pos: (i16, i16), pub message: String,
    pub location: Option<String>, pub script: Option<ScriptId>, pub log: String, pub build: BuildHash,
    pub status: String, pub hits: u32, pub first_generation: u32, pub last_generation: u32 }
pub struct RunRow { pub started: String, pub config: String, pub threads: u32, pub generations: u32, pub nodes: u64, pub frames: u64, pub wall_ms: u64 }
pub struct DbStats { pub nodes: u64, pub valid: u64, pub stale: u64, pub merged: u64, pub broken: u64,
    pub findings: u64, pub maps: u32, pub cells: u64, pub scripts: u32, pub max_progress: u32, pub generation: u32 }

// cache.rs (C)
pub struct SnapCache { /* cap, BTreeMap<NodeId, (Arc<Checkpoint>, tick)> */ }
impl SnapCache {
    pub fn new(cap: usize) -> Self;
    pub fn get(&mut self, id: NodeId) -> Option<Arc<Checkpoint>>;
    pub fn put(&mut self, id: NodeId, cp: Arc<Checkpoint>);
}
#[derive(Debug)] pub enum ReachError { Db(DbError), Missing(NodeId), Restore(String), Diverged { node: NodeId, want: u64, got: u64 } }
/// Node `id`'s end state: the nearest ancestor in the cache, then in the db (same build), else
/// a new game; replays the inputs down the lineage, checks each digest, caches the result.
pub fn reach<G: GameData + Clone>(game: &G, db: &Db, cache: &mut SnapCache, id: NodeId) -> Result<Arc<Checkpoint>, ReachError>;

// rebuild.rs (C)
pub struct RevalidateReport { pub valid: u64, pub repaired: u64, pub broken: u64 }
/// After a build change: every Stale node, parents first, depth levels in parallel (rayon).
/// Replays its inputs from its parent's (new) end state; digest equal -> Valid on this build;
/// otherwise `repair(start, macro)` re-expands; Some -> Valid with new inputs, digest, key and
/// pos (repaired); None -> Broken, and its subtree Broken.
pub fn revalidate<G: GameData + Clone + Send + Sync>(
    db: &mut Db, game: &G, cfg: &Config,
    repair: &(dyn Fn(&Checkpoint, &Macro) -> Option<(Vec<Run>, Checkpoint)> + Sync),
) -> Result<RevalidateReport, DbError>;

// graph.rs, select.rs, mutate.rs, runner.rs (D)
pub struct Rng64(pub u64);                         // splitmix64
impl Rng64 { pub fn next_u64(&mut self) -> u64; pub fn below(&mut self, n: u64) -> u64; }
pub struct Graph { /* nodes: BTreeMap<NodeId, NodeRow>, best: BTreeMap<AbstractKey, NodeId>,
                      key_hits: BTreeMap<AbstractKey, u32>, next_id: NodeId */ }
impl Graph {
    pub fn from_rows(rows: Vec<NodeRow>) -> Graph;
    pub fn node(&self, id: NodeId) -> Option<&NodeRow>;
    pub fn frontier(&self) -> impl Iterator<Item = &NodeRow>;   // Valid, not merged, id ascending
    pub fn best(&self, k: AbstractKey) -> Option<NodeId>;
    pub fn len(&self) -> usize;
}
pub fn tournament(g: &Graph, rng: &mut Rng64, generation: u32, cfg: &Config) -> Option<NodeId>;
pub enum MutationOp { Extend, ReplaceSuffix { up: u8 }, FlipOpts, Probe }
pub fn mutate(g: &Graph, rng: &mut Rng64, sel: NodeId, cfg: &Config) -> (Option<NodeId>, Plan);
pub struct Runner { pub graph: Graph, pub generation: u32, rng: Rng64, cfg: Config }
impl Runner {
    pub fn new(cfg: Config, rows: Vec<NodeRow>, generation: u32) -> Runner;
    /// The next generation's jobs (an empty graph plans the root job). `start` is filled by the
    /// caller (cache::reach).
    pub fn plan(&mut self) -> Vec<(Option<NodeId>, Plan)>;
    /// Merges in job order: evaluates every step, inserts new keys, supersedes a same-key node
    /// with more total frames (old -> Merged), drops duplicates, collects findings.
    pub fn merge(&mut self, results: Vec<JobResult>, eval: &mut dyn Evaluate) -> GenerationDelta;
}
/// rayon par_iter, results in job order.
pub fn run_jobs<F: Fn(&Job) -> JobResult + Sync>(jobs: &[Job], f: F) -> Vec<JobResult>;

// detect.rs, softlock.rs, findings.rs, testing.rs (E)
pub struct Detectors<G> { /* cfg, shadow: Option<(Session<G>, u32)>, idle-not-capturable count, frames */ }
impl<G: GameData + Clone> Detectors<G> { pub fn new(cfg: &DetectConfig) -> Self; }
impl<G: GameData + Clone> StepObserver<G> for Detectors<G> { /* round trip, follow, capturable stuck, slow step */ }
pub enum Trap { Trapped { reached: u32 }, Free, NoExits }
/// From `start` (whose region is `region`, the node's), BFS over macros to depth k (at most
/// `breadth` expansions, deterministic order): Trapped iff the map has exits and every reached
/// state has start's region and map. NoExits if map_exits == 0; Free if k == 0 || breadth == 0.
pub fn probe(start: &Checkpoint, region: u64, map_exits: usize, k: u32, breadth: u32,
             expand_all: &mut dyn FnMut(&Checkpoint, u32 /* remaining */) -> Vec<(u64 /* region */, MapId, Checkpoint)>) -> Trap;
pub fn map_exits<G: GameData>(game: &G, map: MapId) -> usize;   // warps + connections
/// Dedupe signature: FNV of the kind's stable name, the location's file (no line:col), the
/// message with digits stripped, and the map. The row keeps the full location.
pub fn signature(f: &FindingDraft) -> u64;
/// The finding as one NewGame trace: the lineage's inputs then the tail, hashes by replay.
/// Err((trace to the panic, frame)) if a frame before the last one panics.
pub fn repro_trace<G: GameData + Clone>(game: &G, lineage: &[&[Run]], tail: &[Run]) -> Result<Trace, (Trace, u32)>;
/// Replays `trace` with the guard and the detectors; Ok iff a finding with `want`'s signature
/// is raised at its last frame.
pub fn reproduce<G: GameData + Clone>(game: &G, trace: &Trace, want: &FindingDraft, cfg: &DetectConfig) -> Result<(), String>;
// testing.rs: fault-injection wrappers (each delegates like Probe)
pub struct PanicOnScript<G> { pub inner: G, pub script: ScriptId }          // script() returns a panicking script
pub struct WallAround<G> { pub inner: G, pub map: MapId, pub at: (i16, i16) } // leaked MapData copy, cells around `at` blocked
pub struct Drifting<G> { pub inner: G, pub calls: Cell<u32> }               // encounters() depends on a call counter (round-trip break)
pub struct Rebuilt<G> { pub inner: G, pub build_id: u64 }                    // a different build id, same behaviour

// policy.rs, balance.rs (F)
pub struct Policy { last: Buttons }
impl Policy {
    pub fn new() -> Policy;
    /// Next intent while a battle owns the frame: first damaging usable move (power > 0, pp > 0;
    /// else the first move with pp > 0; slot 0 if none), RUN from wild battles when
    /// `opts.run_wild` (never the first battle or a trainer), keep known moves (B on learn
    /// prompts), A through messages.
    pub fn battle(&mut self, w: &World, t: &BattleTables, opts: Opts) -> Buttons;
    /// Next intent while a script or the starter screen runs: A through text; yes/no -> A for
    /// choice 0, B otherwise; starter -> move to ball `opts.choice % 3`, then A.
    pub fn script(&mut self, w: &World, opts: Opts) -> Buttons;
}
pub fn first_damaging_slot(m: &BattleMon, t: &BattleTables) -> u8;
/// Tracks one battle from its first frame to the field's return.
pub struct BattleTracker { /* start frame, levels, hp at start */ }
impl BattleTracker { pub fn start(w: &World) -> Option<Self>; pub fn finish(self, w: &World, outcome: Option<u8>) -> BattleSummary; }
```
The driver (A) owns the **tap rule**:
- Policy keys are intents. The driver inserts a release frame between two presses of the same keys.
- It presses nothing while a battle or warp has the screen black (`scenario.rs:290`). Explorer traces therefore stay valid ROM inputs for later parity.

### `schema.sql` (implemented, `include_str!` in `db.rs`; `Db::open` sets `PRAGMA journal_mode=WAL; synchronous=NORMAL; foreign_keys=ON;`)
```sql
-- u64 values (keys, digests, hashes, build ids) are stored bit-cast to i64.
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,           -- schema, flavor ('stub'|'emerald'), build_engine, build_content, seed, generation
  value TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS node (
  id             INTEGER PRIMARY KEY,
  parent         INTEGER REFERENCES node(id),   -- NULL = root (new game)
  depth          INTEGER NOT NULL,
  macro          TEXT    NOT NULL,              -- RON of Macro
  inputs         TEXT    NOT NULL,              -- runs text from the parent's end state
  frames         INTEGER NOT NULL,
  total_frames   INTEGER NOT NULL,
  end_frame      INTEGER NOT NULL,
  key            INTEGER NOT NULL,
  region         INTEGER NOT NULL,
  digest         INTEGER NOT NULL,              -- state_digest of the end snapshot
  world_hash     INTEGER NOT NULL,
  map            INTEGER NOT NULL,
  x              INTEGER NOT NULL,
  y              INTEGER NOT NULL,
  progress       INTEGER NOT NULL,
  f_novelty      INTEGER NOT NULL,
  f_progress     INTEGER NOT NULL,
  f_bug          INTEGER NOT NULL,
  f_balance      INTEGER NOT NULL,
  f_total        INTEGER NOT NULL,
  generation     INTEGER NOT NULL,
  expanded       INTEGER NOT NULL DEFAULT 0,
  status         TEXT    NOT NULL CHECK (status IN ('valid','stale','merged','broken')),
  merged_into    INTEGER REFERENCES node(id),
  build_engine   INTEGER NOT NULL,
  build_content  INTEGER NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS node_parent ON node(parent);
CREATE INDEX IF NOT EXISTS node_key    ON node(key);
CREATE INDEX IF NOT EXISTS node_map    ON node(map);
-- One live node per abstract key ('merged' rows keep their key for history).
CREATE UNIQUE INDEX IF NOT EXISTS node_live_key ON node(key) WHERE status IN ('valid','stale');

CREATE TABLE IF NOT EXISTS node_touch (           -- incremental re-simulation (v1)
  node INTEGER NOT NULL REFERENCES node(id),
  kind INTEGER NOT NULL,                          -- 0 map, 1 script
  id   INTEGER NOT NULL,
  PRIMARY KEY (node, kind, id)
) STRICT, WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS touch_by_id ON node_touch(kind, id);

CREATE TABLE IF NOT EXISTS seen (                 -- novelty archive
  kind       INTEGER NOT NULL,                    -- SeenKind
  id         INTEGER NOT NULL,                    -- flag id, (var << 16 | value), map id, script id
  first_node INTEGER NOT NULL REFERENCES node(id),
  hits       INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (kind, id)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS coverage (             -- studio heatmap
  map        INTEGER NOT NULL,
  x          INTEGER NOT NULL,
  y          INTEGER NOT NULL,
  first_node INTEGER NOT NULL REFERENCES node(id),
  hits       INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (map, x, y)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS battle (               -- balance tables (v0: recorded, weight 0)
  id               INTEGER PRIMARY KEY,
  node             INTEGER NOT NULL REFERENCES node(id),
  kind             INTEGER NOT NULL,
  trainer          INTEGER,
  outcome          INTEGER,
  frames           INTEGER NOT NULL,
  player_level     INTEGER NOT NULL,
  opponent_level   INTEGER NOT NULL,
  hp_lost_permille INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS finding (
  id               INTEGER PRIMARY KEY,
  signature        INTEGER NOT NULL UNIQUE,
  kind             TEXT    NOT NULL,
  node             INTEGER NOT NULL REFERENCES node(id),  -- the tail starts at this node's end state
  macro            TEXT,
  tail             TEXT    NOT NULL,              -- runs text to the finding frame (inclusive)
  total_frame      INTEGER NOT NULL,              -- frames from a new game to the finding
  map              INTEGER NOT NULL,
  x                INTEGER NOT NULL,
  y                INTEGER NOT NULL,
  message          TEXT    NOT NULL,
  location         TEXT,
  script           INTEGER,
  log              TEXT    NOT NULL,
  build_engine     INTEGER NOT NULL,
  build_content    INTEGER NOT NULL,
  status           TEXT    NOT NULL DEFAULT 'open' CHECK (status IN ('open','stale','gone')),
  hits             INTEGER NOT NULL DEFAULT 1,
  first_generation INTEGER NOT NULL,
  last_generation  INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot (             -- disk snapshot cache (git-ignored db)
  node          INTEGER PRIMARY KEY REFERENCES node(id),
  build_engine  INTEGER NOT NULL,
  build_content INTEGER NOT NULL,
  bytes         BLOB    NOT NULL                  -- engine::checkpoint::encode
) STRICT;

CREATE TABLE IF NOT EXISTS run (                  -- timings: never in content_digest
  id          INTEGER PRIMARY KEY,
  started     TEXT    NOT NULL,
  config      TEXT    NOT NULL,                   -- RON of Config
  threads     INTEGER NOT NULL,
  generations INTEGER NOT NULL,
  nodes       INTEGER NOT NULL,
  frames      INTEGER NOT NULL,
  wall_ms     INTEGER NOT NULL
) STRICT;
```
STRICT tables need SQLite 3.37 or later; the bundled version is 3.53.2. `apply` order:
1. `merged` updates (frees the live key)
2. `expanded`
3. node inserts
4. snapshots
5. touches, seen (upsert `hits = hits + 1`), coverage (upsert), battles
6. findings (upsert on `signature`: `hits + 1`, `last_generation`)
7. `meta.generation`

### xtask (implemented dispatch and body)
- `xtask/src/explore.rs`: `pub fn main(args: &[String]) -> Result<()>`.
  - It runs `cargo run -p explore --release --bin explore -- <args>`, or without `--release` when the first argument is `--debug`.
  - It inherits `PKR_EXTRACTED_DIR`, so `PKR_EXTRACTED_DIR=/nonexistent cargo xtask explore …` explores the stub.
- `main.rs` gains `mod explore;` and `Some("explore") => explore::main(&args[1..])`.

---

## Rules for every task

- **Workspace:** work in your own git worktree. Commit with `feat(explore): …` or `fix(…)`, and **no attribution or co-author lines**.
- **TDD:** write a failing test first.
  - Unit tests are plain `#[test]`s, headless, no window.
  - Game tests use `content::Emerald::for_shells()` in whatever flavor the build has, `testing::*` wrappers for faults, and `tempfile` dirs or `Connection::open_in_memory` for databases.
  - Keep debug-mode test budgets small: at most about 20k simulated frames per test (the parity route is about 19k). Larger runs go through `cargo xtask explore` in release (Wave 2).
- **Determinism (the explorer's results are a function of seed, config and build):**
  - no `HashMap`/`HashSet` iteration in any decision path (use `BTreeMap`/sorted `Vec`);
  - no floats in keys, fitness or selection (integer and fixed-point only);
  - all randomness from `Rng64` seeded by `Config::seed` and job indices, never the engine `Rng`.
  - `Instant` is allowed only in `guard`/`detect` (slow step) and run stats. It never feeds back into the search or into `content_digest`.
  - Merge happens in job order. Results must not depend on `threads`.
- **Threads:**
  - Never move a `World`, `Session` or `Probe` across threads. Build them inside the job closure.
  - Only `Arc<Checkpoint>`, `NodeRow`s, `Job`/`JobResult` and `Config` cross threads.
  - The `rusqlite::Connection` stays on the main thread (single writer). Studio readers open it read-only.
- **Game-agnostic engine:** **no change to `engine`** in Waves 1–2. If a task believes it needs one, it describes the addition in its report. It must be additive, with no hash change, and it lands as a separate reviewed commit. Emerald knowledge goes in `goals.rs`/`key.rs`, built on `content::consts`.
- **Wave 1 never edits `game-native`, `crates/studio/**` or `xtask`.** It uses only the existing `Session`, `scenario` and `trace` APIs listed above.
- **Never commit ROM-derived data:**
  - `explore.db`, exported repro traces and logs live under `target/explore/` (git-ignored).
  - Tests write only to temp dirs.
  - **No FR text literals in tests.** Assert structure (counts, ids, goal indices, `SCRIPT_NAMES` identifiers, map ids from `content::consts`). Stub expectations come from `crates/content/build/stub.rs`.
- **Hash and parity:** M7 compiles nothing into the GBA build. Every task leaves `World::hash`, the goldens, the committed trees and parity untouched; the existing tests catch a change.
- **Before committing:**
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - the M6 feature clippy in force on master (`--features game-native/window,studio/ui`)
  - `cargo test --workspace` with `PKR_ROM_FR=/Users/dode/Documents/pokemon/pokemon-rust/rom/emeraude-fr.gba PKR_ROM_US=/Users/dode/Documents/pokemon/emerald.gba PKR_DECOMP=/Users/dode/Documents/pokemon/pokeemerald`
  - `PKR_EXTRACTED_DIR=/nonexistent cargo test --workspace` (stub)
- **Ownership:** only touch the files your task owns, plus new test files in `crates/explore/tests/<task>_*.rs`. Describe any other change you need in your report.

---

## Wave 1 (parallel, 6 agents)

### Task A — macro-actions, pathfinding, expansion and simulation

**Owns:** `crates/explore/src/{macros.rs, path.rs, expand.rs, sim.rs}`

1. **`path::reach`:**
   - BFS over `MapData.blocks`, with `FieldSnap.overrides` applied, `block_collision == 0` and `block_behavior` (not `Blocked`; `Door`/`Warp` cells are targets, not passages; `Ledge(d)` only jumped toward `d`, two cells).
   - Cells of objects other than the player (`pos` and `prev` from `ObjectsSnap`) are occupied.
   - Elevation is ignored (decision 16).
   - `next_dir`: the same rules on the live `World` (objects re-read every frame), tall grass avoided first, as `scenario.rs:706-770`.
2. **`macros::candidates`**, in deterministic order. Each candidate requires a reachable approach cell; the first one in `Dir` order wins:
   - `Enter` for every warp: `flow::door_warp`/`warp_on_enter`/`warp_on_push` decide the entry direction;
   - `Cross` per connection: the reachable edge cell nearest the middle of the overlap;
   - `Talk` per visible object with a template script;
   - `Inspect` per `BgKind::Script`;
   - `Grass` for the nearest reachable tall-grass cell;
   - `WalkTo` for every coord-event cell, then one cell per coarse block (`1 << key.cell_shift`, default 4), nearest the block centre, excluding the player's block.

   Unit tests:
   - stub: the start state has no `Enter`/`Cross`/`Talk`, one `Grass`, and WalkTo cells covering all 16 blocks reachable;
   - real flavor: the Littleroot start has `Enter` for the houses and `Cross Up`;
   - candidates are identical across two calls.
3. **`expand::expand`:** the driver.
   - **Pressing:**
     - `press` = `guard::step_guarded`, then `obs.after_step`, then the summary update (cells, maps, `probe.take_started()`, log count, battles through `balance::BattleTracker`, which is Task F's type: use a local placeholder until F merges, or add the tracker in Wave 2).
     - The tap rule above applies.
   - **Interrupts:** a script, battle or starter screen that starts during a walk ends the walk. The driver resolves it with `Policy` and the macro's `Opts`, then settles. The macro always ends at the first frame with `capturable() == Ok`.
   - **StaleView:** after `Cross`, keep walking into the new map (straight on, else any open direction) until it clears.
   - **Errors:**
     - `NoPath`: planned up front;
     - `NoEffect`: the warp never starts in `enter_frames`, or no encounter in `grass_frames`;
     - `Timeout`: `macro_frames`, `battle_frames` or `settle_frames` exceeded;
     - `Flagged(PlannerDivergence)`: the player holds a direction toward a cell the planner calls walkable and has not moved for 64 frames, with no script or battle running;
     - `Panicked`.
   - `new_game` steps a new game (`Settle`) to its first capturable frame.
4. **`sim::simulate`:**
   - Builds `Session<Probe<G>>` from `job.start`: `Session::restore`, or `expand::new_game` for the root.
   - For `Plan::Random { len, seed }`, it picks among `candidates` with `Rng64(seed ^ step)`, skipping `NoPath`/`NoEffect` candidates up to 8 tries. For `Plan::Given`, it uses the given macros.
   - Each successful macro becomes a `StepResult` with `end = session.checkpoint()` and `inputs = rle(keys)`.
   - A failure stops the job:
     - `Panicked`/`Flagged` → `Stop::Finding(FindingDraft { tail: rle(partial keys) … })`;
     - other errors → `Stop::Pruned`.
   - `Plan::Softlock` returns `Stop::Pruned("unwired")` until Task G wires it.
5. **Tests:**
   - **Stub:** from a new game, `Given([WalkTo (10, 9)])` ends capturable at (10, 9). `Grass` gives a won battle in `summary.battles` (default policy). Two runs of the same job give equal inputs and end digests.
   - **Real flavor:** `Given` macros reproduce the first two route moments: `WalkTo (11, 2)`, then `WalkTo (11, 1)` interrupted by `LittlerootTown_Coord2`, which runs and the node ends after it. Then `Cross Up` from (10, 0) lands on `MAP_ROUTE101` capturable, with `Route101_Coord0` in `summary.scripts` (by `SCRIPT_NAMES` identifier).
   - Every `StepResult.end` passes `World::restore`.
   - A replay of `inputs` from `start` reaches `state_digest(end)`.

### Task B — abstract key, goals, novelty and fitness

**Owns:** `crates/explore/src/{key.rs, goals.rs, fitness.rs}`

1. **`key_parts`:**
   - map;
   - coarse cell `(x >> shift, y >> shift)` of `objects.objs[0].pos`;
   - flags digest: set flags `>= TEMP_FLAGS_END`, minus `cfg.ignore_flags`, minus `special_ids().flags_cleared_on_map_change`;
   - vars digest: `VARS_START + 0x10 (TEMP_VARS_END)..VARS_END` non-zero, minus `ignore_vars`; special vars `0x8000..` are excluded;
   - party bucket.

   `key`/`region` fold the parts with splitmix64. They are documented as stable: changing the fold is a db schema change.
   - `default_key_config()` lists Emerald exclusions from `content::consts`, for example step and day counters if present. The stub list is empty.
   - Tests:
     - the same snapshot after a `World::restore` gives the same key;
     - moving one cell inside a block keeps the key, and moving one block changes it while `region` stays;
     - setting a temp flag changes nothing, and a story flag changes both;
     - party level changes below the bucket step keep the key.
2. **`goals`, slice list in story order:**
   1. `rescue_started`: `MAP_ROUTE101` entered, from `summary.maps` or the end map
   2. `starter`: party non-empty
   3. `birch_rescued`: `FLAG_RESCUED_BIRCH`
   4. `lab`: in or entered `MAP_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB` with `FLAG_SYS_POKEMON_GET`
   5. `oldale`: `MAP_OLDALE_TOWN` entered
   6. `route103`: `MAP_ROUTE103` entered
   7. `may_defeated`: `FLAG_DEFEATED_RIVAL_ROUTE103`

   Stub list:
   1. `wild_won`: a battle with outcome Won in the summary
   2. `east`: x ≥ 10
   3. `far_corner`: cell (5, 12) or below

   `progress` advances over consecutive goals only, so it is monotone along a lineage.
   - Tests: on the real flavor, replaying `cached_route()` to each `Recording::nodes` frame (`lab`, `after_wild`, `oldale_center`, `route103_after_may`) and summarising maps entered gives progress 4, 4, 5 and 7. The stub route gives 3.
3. **`fitness`:**
   - `Archive` hit counts. `Evaluator::evaluate` computes the parts and key, novelty (first sightings of flags, `(var, value)`, maps, scripts and cells in the summary, plus a new key), progress, then `score`. It then records the sightings and returns `seen`/`touches` (maps and scripts touched).
   - `score` (integer): `novelty = Σ weight × count (+ new_key)`, `progress = goal × gained goals`, `bug = bug × findings`, `balance = 0`.
   - `selection_score` applies the age and expanded penalties and `total / (1 + key_hits)` decay.
   - Tests:
     - a fresh archive scores the root's flags and maps, and the same step evaluated twice scores less the second time;
     - integer-only (a clippy `float_arithmetic` deny on the module);
     - `Archive::from_db` round trip.

### Task C — SQLite store, snapshot cache, re-validation

**Owns:** `crates/explore/src/{db.rs, cache.rs, rebuild.rs}` (`schema.sql` is W0; schema changes bump `SCHEMA_VERSION` and are listed in the report)

1. **`Db::open`:**
   - creates the parent dirs and runs the DDL plus pragmas;
   - writes `meta` (schema, flavor, build, seed) on a fresh db;
   - fails with `Flavor` on a stub/emerald mismatch and with `Schema` on another version;
   - sets `build_changed` when `meta.build_*` ≠ `build` (it does not mark nodes; the caller decides).

   `open_read_only` uses `SQLITE_OPEN_READ_ONLY` and works while a writer holds the db (WAL).
2. **`apply`:** one transaction in the order given in Wave 0. `nodes`, `lineage`, `findings`, `coverage`, `stats` and `content_digest` follow it.
   - u64 values are bit-cast to i64.
   - Macro as RON, inputs and tail through `runs::to_text`.
3. **`SnapCache` and `cache::reach`:**
   - LRU by tick.
   - DB snapshots are trusted only on the same build and when `state_digest == node.digest`.
   - The replay steps `Session` with `runs::keys`, then requires `capturable` and an equal digest; otherwise it returns `Diverged`.
4. **`rebuild::revalidate`:**
   - The caller has already called `mark_all_stale` on `build_changed`.
   - Works one depth level at a time, `par_iter` per level, results applied in id order.
   - A Broken parent makes its subtree Broken.
   - Updates `meta.build_*` at the end, and marks open findings of other builds `stale`.
5. **Tests (temp dirs and in-memory):**
   - schema creation is idempotent;
   - flavor mismatch;
   - an `apply` round trip (rows equal, `merged` before the live-key insert does not violate `node_live_key`);
   - finding upsert increments `hits`;
   - `content_digest` ignores `run` rows;
   - `reach` from an empty cache replays a 3-node stub lineage built with `Session` by hand (no Task A code) to the stored digests, and a wrong digest gives `Diverged`;
   - `revalidate` with `testing::Rebuilt`-style different build ids (construct the `BuildHash` by hand): all Valid when the game is unchanged; a repair closure returning `None` on a mutated digest makes the subtree Broken.

### Task D — runner: graph, selection, mutation, parallel generations, merge

**Owns:** `crates/explore/src/{graph.rs, select.rs, mutate.rs, runner.rs}`

1. **`Graph::from_rows`** rebuilds `best` (live keys) and `key_hits`. `next_id` is max id + 1.
2. **`tournament`:** `cfg.tournament` uniform picks from the frontier (by `Rng64::below` over the id-sorted frontier). The best by `fitness::selection_score` wins, lower id on ties. Until Task B merges, use a local integer score with the same formula; Wave 2 switches to B's.
3. **`mutate`**, ops by fixed weights:
   - `Extend` (60%): `Plan::Random { len: 1..=max_plan_len }` from the selected node;
   - `ReplaceSuffix { up: 1..=2 }` (20%): from an ancestor;
   - `FlipOpts` (15%): `Plan::Given([sel.macro_ with opts.run_wild toggled or choice + 1])` from `sel.parent`;
   - `Probe` (5%, or forced for nodes with `expanded ≥ 3` and no child): `Plan::Softlock`.

   Seeds come from `Rng64`.
4. **`Runner::plan`:** an empty graph plans `[(None, Plan::Given([Settle]))]`. Otherwise it plans `batch` jobs.

   **`Runner::merge`:** for each result in index order, for each step in order:
   - `eval.evaluate(parent_row, step)`;
   - if the key is unseen, insert a new node: next id, parent = the previous step's node or the job parent;
   - if the key is live with more `total_frames`, insert the new node and supersede the old one (`merged`) (as built: more goals reached first, then fewer frames; see Task G as built);
   - otherwise it is a duplicate: `key_hits += 1`, and later steps of this job chain from the existing best node;
   - the job parent's `expanded += 1`;
   - findings take the node the tail starts from and the total frame;
   - `Stop::Pruned` counts in stats.

   `snapshots`: every new node goes to the cache. A node goes to the db when `depth % snapshot_every_depth == 0`.
5. **`run_jobs`:** `rayon` `par_iter().map(f).collect()`, using the caller's thread pool (`ThreadPoolBuilder` in Task G).
6. **Tests (fake `Evaluate` and a fake simulate closure; no game needed):**
   - `plan` and `merge` are deterministic for a seed;
   - `run_jobs` on pools of 1 and 4 threads gives an identical `GenerationDelta`;
   - a same-key result with fewer frames supersedes, and one with more is a duplicate (as built: among equal goals reached; more goals supersede first, Task G);
   - the age penalty makes an old high-fitness node lose to a fresh one after N generations;
   - `from_rows(merge output)` equals the in-memory graph;
   - frontier ids ascend.

### Task E — bug detectors, softlock probe, findings and repro

**Owns:** `crates/explore/src/{detect.rs, softlock.rs, findings.rs, testing.rs}`

1. **`Detectors::after_step`:**
   - **Round trip:** at the first capturable frame after every `roundtrip_every` frames, take `cp = s.checkpoint()`, then `decode(&encode(&cp))` must equal `cp`. `World::restore(s.game().inner(), &cp')` must succeed with the same `hash`. Then keep a shadow `Session<G>` (inner game) for `follow` frames: step it with the same keys and compare hashes every frame. A mismatch is a `RoundTrip` flag.
   - **Capturable stuck:** count frames where the field is active, no transition, no script, no battle, no starter, the player settled, nothing held, and `capturable()` is `Err` other than `StaleView`. More than `capturable_stuck` frames is a `CapturableStuck` flag that carries the `NotCapturable` reason.
   - **Slow step:** `step_ns > slow_step_us` is a `SlowFrame` flag (report-only; `reproduce` skips it).
2. **`softlock::probe`:** BFS as specified. `map_exits` counts warps plus connections. `NoExits` is never a finding.
3. **`findings`:**
   - `signature` (FNV of the kind's stable name, the location's file without line:col, the digit-stripped message, map).
   - `repro_trace` replays from `World::new` through `TraceRecorder` (`Parent::NewGame`, the build from `build_hash(game)`, hashes every 60 frames).
   - `reproduce`.
4. **`testing` wrappers**, each with a test proving the detector fires (use `Session` + `observe` directly; no Task A code):
   - **`PanicOnScript`** (on the stub there are no event scripts: inject at `NEW_GAME_SCRIPT`, or wrap `script` for the first script started on the real flavor): `step_guarded` returns `Panicked` with a `location` naming the test file. The repro trace replayed under the guard panics at the same frame.
   - **`WallAround`** at the stub start: `probe` with a fake `expand_all` that walks the reach set says `Trapped`, given that `map_exits` ≥ 1 is faked by the wrapper adding one warp. Without the wall it says `Free`.
   - **`Drifting`:** the round-trip follow detects a hash mismatch within `follow` frames on the stub grass (encounters differ).
   - **`Rebuilt`:** `build_hash` differs and behaviour is equal (used by C and G).

### Task F — battle and menu policy, battle records

**Owns:** `crates/explore/src/{policy.rs, balance.rs}`

1. **`Policy::battle`:**
   - `OpenMenu::Action(c)`: navigate to FIGHT, or RUN when `opts.run_wild` and the battle is `Wild { first_battle: false }`. Use the 2x2 cursor walk of `scenario.rs:311-340`.
   - `Move(c)`: navigate to `first_damaging_slot(battle.mon(Side::Player))`, where power > 0 and pp > 0 (from `BattleTables::move_info`); else the first slot with a move and pp > 0; slot 0 only when no move has PP (the engine then uses Struggle, `core.rs` `player_action`). Falling back to a 0-PP slot 0 while another move has PP would loop on "no PP left" until `battle_frames`.
   - `LearnAsk`/`LearnPick`: B. `StopAsk(0)`: A, else UP.
   - Otherwise A.
2. **`Policy::script`:** A through text. On `is_yes_no_active`, A for choice 0 and B otherwise. On the starter screen, LEFT/RIGHT to ball `choice % 3`, then A.
3. **`balance::BattleTracker`:** starts on the frame `in_battle()` turns true. It records levels (party lead, opponent `Battle::mon(Side::Opponent)`) and HP. `finish` gives a `BattleSummary` with `hp_lost_permille` and frames.
4. **Tests:**
   - **Stub:** a hand-driven `Session` (no Task A code): walk into the grass with fixed keys until `in_battle`, then let `Policy::battle` drive to the end. The outcome is Won within `battle_frames`. With `run_wild`, it is Ran.
   - `first_damaging_slot` skips a 0-power move and a 0-pp move. Use a synthetic `BattleMon` with move ids from `content::battle::SLICE_MOVES`, and pick a 0-power move by scanning the table, never by name.
   - **Real flavor:** from the `lab` route node state (replay `cached_route()` to that frame, then `checkpoint`), `Policy::script` with choice 0 through a `Talk` to Birch's aide... is too content-specific; instead assert that the starter screen from the route's `starter_screen` moment chooses ball 2 with `choice = 2` (the replay of the route keys up to the moment, then policy keys).

---

## Wave 2 (after Wave 1 is merged)

### Task G — integration, CLI, `cargo xtask explore`, end-to-end tests

**Owns:**
- `crates/explore/src/{wire.rs, cli.rs, bin/explore.rs}`
- `crates/explore/tests/e2e_*.rs`
- the README (a new **Explorer** section)
- the Wave 1 placeholders it replaces: the tracker in `expand`, the selection score in `select`

It may make small edits to Wave 1 files for wiring, listed in its report.

1. **`wire.rs`:**
   - `Explore<G>` holds `Db`, `SnapCache`, `Runner`, `Evaluator` (with its archive loaded from the db), `Config` and a `rayon::ThreadPool`.
   - `generation()`: `plan` → fill `start` through `cache::reach` → `run_jobs(sim::simulate with Detectors)` → `merge` → `db.apply`.
   - `Plan::Softlock` in `sim` goes through `softlock::probe`. Its `expand_all` runs `candidates` + `expand`, capped by breadth. `Trapped` becomes a `Softlock` finding whose tail is empty (the node itself) and whose message lists the region and map.
   - The repair closure for `revalidate` is `expand` from the start checkpoint.
   - On `build_changed`: `mark_all_stale` → `revalidate` before exploring.
2. **`cli.rs`:**
   - `run [--nodes N|--frames N|--generations N|--secs N] [--threads N] [--seed S] [--db P] [--config P]` prints one line per 10 generations (nodes, nodes/s, frames/s, goals, maps, cells, findings).
   - `status`
   - `findings [--all]`
   - `show <id>`
   - `export <id> [--before N] [--out P]`: writes a NewGame `Trace` RON under `target/explore/repro/`
   - `repro <id>`: exit 0 iff it reproduces
   - `revalidate`
   - `coverage <map>`: an ASCII heat grid
   - `digest`: `content_digest`
   - `reset`: deletes the db after confirmation with `--yes`
3. **E2E tests** (stub, debug, small budgets; temp db):
   - `run --generations 30` covers 100% of the stub's reachable cells and reaches all three stub goals.
   - The same seed with `--threads 1` and `--threads 4` gives an equal `content_digest`.
   - Resume: 15 + 15 generations in two processes (two `Explore` instances on one db) gives the same `content_digest` as 30 in one.
   - `PanicOnScript` and `WallAround` games produce one finding each; `export` + `reproduce` succeed.
   - A `Rebuilt` game on an existing db revalidates every node `Valid`.
   - `git status --porcelain` is unchanged by the tests (they write to temp dirs).

   Real flavor, debug:
   - a 5-generation smoke run: no findings, progress ≥ 1;
   - the full run is Task H.
4. **xtask:** document `cargo xtask explore run --secs 600` and the stub variant in the README.

### Task H — slice runs, numbers and tuning (starts when G merges)

**Owns:** `Config::default` values (in `config.rs`, listed in its report), and this doc's "Wave 2 as built".
1. **Release runs, 8 threads, default seed, empty db:**
   - **Stub:** `--generations 200`. Record nodes/s, frames/s, cells covered and goals.
   - **Extracted:** `--secs 1800`. Record:
     - nodes/s and frames/s;
     - db size;
     - the generation and wall time at which each goal is first reached, especially `may_defeated`;
     - maps entered;
     - cell coverage per slice map;
     - the % of `SCRIPT_NAMES` started;
     - battles (won, lost, ran);
     - findings by kind.
2. **Tuning:** if `may_defeated` is not reached in budget, tune `Weights.goal`, `max_plan_len`, the age penalty and the `WalkTo` density (the cell shift) in that order. Re-run and record each attempt (config RON plus numbers). Then fix the defaults.
3. **Triage every finding:**
   - a real bug: file it in "Findings for Milestone 8", or fix it in a separate `fix(engine|content)` commit only if it is trivial and hash-neutral;
   - an explorer bug: fix it in explore;
   - a false positive: tighten the detector and record why.
4. **Revalidation on an engine-source change:** touch a comment in an engine source, which changes `ENGINE_SRC_HASH` but not behaviour. `revalidate` must then report every node `Valid`. Record the time taken.

### Wave 2 close: gates
Every gate in "Rules" passes on the merged tree, plus `PKR_EXTRACTED_DIR=/nonexistent cargo xtask explore run --generations 50` and `cargo xtask explore digest`.

---

## Wave 2b — studio integration (gated: starts only after M6 Tasks B, C, E and G are merged)

It depends on M6's `Materializer`, `TraceRecorder::from_prefix`, `Seeked`/`Controls::from_seek`, the `map_view` overlays and the UI shell. If M6's As built renames any of these, Wave 2b follows the rename.

### Task S1 — studio core: explorer model, open-at-frame, coverage overlay

**Owns:**
- `crates/studio/src/explorer.rs` (new)
- the additions to `studio/src/{native.rs, map_view.rs, paths.rs, lib.rs}` (listed)
- `crates/studio/Cargo.toml` (`explore.workspace = true`)

1. **`StudioPaths::explore_db`** = `explore::db::Db::default_path(content::STUB)`.
2. **`explorer.rs` (headless, tested):**
   - `ExplorerModel::load(path)` reads through `Db::open_read_only`: `DbStats`, `Vec<FindingRow>` (open first, by hits) and `coverage(map)` as `CoverageGrid { w, h, hits: Vec<u32> }`.
   - `ExplorerProc::{start(args), stop, running}` spawns `cargo run -p explore --release -- run …` as a child process. This isolates panics and keeps the UI thread free. It is stopped with `kill` on drop.
   - `finding_trace(db, id, before) -> (Trace, u32 frames)` builds the trace with `findings::repro_trace` on the lineage and the tail, then `total_frame - before` (clamped).
3. **`NativeCmd::OpenTrace { trace: Trace, frames: u32 }`:**
   - On the pane thread: `Session::new`, then step `trace.keys().take(frames)`, recording through `TraceRecorder::from_prefix(&trace, frames)`.
   - Then `Controls::from_seek(Seeked { session, parent: Parent::NewGame, recorder, diverged: false })`.
   - Saving from there gives a local node with a NewGame parent, as M6 decision 7 does.
4. **`map_view`:** `Overlay::Coverage(&CoverageGrid)` as a per-cell heat bucket (4 levels).
5. **Tests:**
   - `ExplorerModel` on a temp db filled by the explore API: stats and findings order;
   - `OpenTrace` on a stub finding trace lands at the expected frame with a hash equal to a straight replay;
   - a coverage grid round trip.

### Task S2 — studio UI: Explorer tab and map overlay toggle

**Owns:** `crates/studio/src/ui/explorer.rs` (new), and the additions to `ui/{mod.rs, map.rs}` (listed).

1. **Explorer tab:**
   - start/stop with a budget field, seed and threads;
   - live stats polled every 2 s (nodes, nodes/s, goals reached by name, maps, cells, findings);
   - a findings table (kind, map, cell, message, hits, build or stale);
   - per row: "Open at frame before" (default 120 frames, editable), which sends `NativeCmd::OpenTrace` and focuses the native pane; "Export repro"; "Copy `cargo xtask explore repro <id>`".
2. **Map tab:** a "Coverage" toggle that fetches `coverage(map)` for the shown map.
3. **Smoke:** an ssr render of the Explorer tab on a temp db finds the table rows.
4. **Gates:** all M6 gates plus `cargo test -p studio --features ui`.

---

## Wave 3 — close-out

- **Whole-milestone review:**
  - determinism (no `HashMap` iteration or floats in decisions, merge order);
  - threading (no `World` across threads; the single writer);
  - write locations (`target/explore/`, temp dirs only);
  - FR literals;
  - the engine untouched.

  Fix the findings.
- **Parity unaffected:** run `cargo xtask rom` and `cargo xtask parity` once on the merged tree. Hashes and budgets are unchanged.
- **After a 30-minute explorer run and a studio session,** `git status` is clean.
- **This doc:**
  - "As built": contracts changed, schema version, the numbers from H, db size per 100k nodes, snapshot cache hit rate, revalidation time;
  - "Findings for Milestone 8", covering:
    - per-script and per-map content digests from `content/build.rs`, to make re-simulation incremental through `node_touch`;
    - an engine `FrameOut` event list (`ScriptStarted`/`ScriptLine`) in place of `Probe`, and script-line coverage;
    - engine-side panic context (script id and map in the panic message, design §3);
    - elevation-aware pathfinding;
    - party switching, items and the bag in the policy and the key;
    - abstract-key gaps (Task B): the key ignores `WorldSnapshot.respawn` (the whiteout heal location) and the object `templates` deltas (moved or re-scripted NPCs that no flag or var reflects), and the `VAR_OBJ_GFX_ID_*` vars are story vars in the key, which is harmless in the slice but varies outside it;
    - balance fitness;
    - goal-directed completability per checkpoint;
    - whether explorer exports should become local-store nodes.
- **Cleanup:** clean up worktrees.

## Done when

- **Determinism and persistence:**
  - `PKR_EXTRACTED_DIR=/nonexistent cargo xtask explore run --generations 200` (release) covers 100% of the stub's reachable cells and reaches all stub goals.
  - Two runs with the same seed and `--threads 1` versus `--threads 8` print the same `explore digest`.
  - Resuming a run gives the same digest as running it straight.
- **Extracted slice** (release, 8 threads, default config and seed, empty db):
  - `cargo xtask explore run --secs 1800` reaches `may_defeated` (`FLAG_DEFEATED_RIVAL_ROUTE103`), recorded with its generation and wall time;
  - it enters Littleroot, the lab, Route 101, Oldale (with its Pokémon Center) and Route 103;
  - it visits ≥ 80% of the reachable cells of Route 101, Oldale Town and Route 103;
  - it sustains ≥ 100 nodes/s, or the threshold rescaled in Wave 0 from the measured frames/s, with the reason recorded.
- **Bugs:**
  - Every finding row has a tail and a total frame.
  - `cargo xtask explore repro <id>` reproduces 100% of Panic, RoundTrip, CapturableStuck and Softlock findings on the same build.
  - Fault-injection tests prove each detector fires: panic with a Rust location, trap, round-trip drift, stuck capturable.
- **Rebuild:** an engine-source change that does not change behaviour revalidates every node `Valid`. A behaviour change (a `testing` wrapper) marks the affected subtree `Repaired` or `Broken`, and never silently `Valid`.
- **Studio (Wave 2b):**
  - the Explorer tab lists findings;
  - "Open at frame before" lands on the native pane at `total_frame - 120` with a hash equal to a straight replay;
  - the Coverage overlay renders on the map view.
- **Gates:** fmt, `clippy -D warnings` (workspace and the M6 feature set), `cargo test --workspace` in real and stub mode, the studio ui tests after 2b, and `cargo xtask parity` unchanged.
- **Hygiene:** no `explore.db`, repro trace or ROM-derived file is committed, no FR literals in tests, and `engine` is unchanged.

---

## Open questions / decisions taken

1. **Own driver, not the scenario `Recorder` (taken).**
   - `Recorder` is private and is the parity test driver. The explorer needs:
     - a pluggable policy with `Opts`;
     - observer hooks on every frame;
     - interrupt semantics (a script or battle ends a walk);
     - reach maps with `setmetatile` overrides;
     - error kinds that separate "prune" from "bug".
   - About 300 lines are adapted (BFS, tap rule, battle cursor walk); `scenario.rs` stays frozen.
   - Unifying them would mean moving the driver into `game-native` (M8 option).
2. **A macro ends on a capturable frame (taken).**
   - Every node has a `Checkpoint` (`Send`), so workers restore from it and states are cacheable.
   - Key, goals and pathfinding read `WorldSnapshot`, with no engine accessor added.
   - Cost: a node can never sit mid-script. Mid-script states are reached by a finding's tail, which is the M5 "replay, never snapshot" rule.
3. **Abstract key (taken):**
   - map + 4x4 coarse cell + story flags + story vars + party bucket (count, species, level sum / 3, fainted);
   - temp and special flags and vars, and content-listed counters, are excluded;
   - no key items yet (there is no bag).

   The fold is part of the db schema.
4. **Integer fitness and seeded `SplitMix64` (taken).** Results are reproducible across machines and thread counts. Floats are not allowed in decisions.
5. **Batch-synchronous generations (taken).** This is slightly less parallel than a free-running pool (the tail of a batch waits). In return the tree is deterministic and testable. Batch = 4 × threads hides most of the wait.
6. **Single SQLite writer, WAL, one transaction per generation (taken).** The studio and CLI read concurrently (`open_read_only`). The `run` table holds timings and is excluded from `content_digest`.
7. **Store inputs and macro (taken).** Revalidation replays the inputs first, which is cheap and exact. It re-expands the macro only on divergence (repair). Inputs use the Trace run syntax, so exports are direct.
8. **Rebuild v0 = re-validate everything on any build change (taken; gap documented).**
   - `node_touch` already records maps and scripts.
   - Incremental re-simulation needs per-script and per-map digests from `content/build.rs`; the content build id is a single number today (M8 finding).
   - An engine change can affect any node, so engine changes stay "re-validate all".
9. **Script coverage through `Probe` (taken).** Design §3's `FrameOut` events (`ScriptStarted`, `ScriptLine`) do not exist yet (M6 decision 11). `Probe` sees script starts, including immediate map scripts, at zero engine cost. Script *line* coverage waits for engine events.
10. **Panics:**
    - **Captured:** the explorer records the Rust `Location`, the map, the cell, the last script started (`Probe`) and the log ring. The "script/map location inside the panic" of design §3 is an engine follow-up.
    - **Requirement:** profiles must keep `panic = "unwind"`, and the hook suppresses output only while a guard is active.
11. **Softlock definition v0 (taken):** trapped = the map has exits, and every state reachable within K macro-steps (bounded breadth) has the same map and region.
    - It runs on sampled nodes, and on nodes whose candidate expansions all failed.
    - A K per map is configurable.
    - Stuck scripts and fades show up as `CapturableStuck` or `Timeout` instead.
12. **Slow frames are report-only (taken).** Native step time is host-dependent. The real frame budget remains `cargo xtask parity`'s `late == 0` and loop-lines gates.
13. **Balance (stubbed):** battles are recorded in the `battle` table and `Weights.balance` = 0. Balance tables and a deviation score come later.
14. **Explorer storage stays separate from checkpoint stores (taken; M5 decision 7).**
    - The studio opens a finding by replaying its flattened NewGame trace (cheap natively).
    - Saving from there makes a normal local node.
    - Open: an `explore export --to-store` that writes a local-store node at a node boundary.
15. **Goal order (taken: story order).** The brief lists "lab → starter → Route 101 → Oldale → Route 103 May". From the `BeforeRescue` start, the story order is Route 101 (rescue) → starter → Birch rescued → lab (Mudkip given) → Oldale → Route 103 → May. Task B uses the story order (progress only advances in order). The story order is used.
16. **Pathfinding ignores elevation (taken for the slice).**
    - Failures are pruned.
    - Engine disagreements with the planner become `PlannerDivergence` findings, a useful collision-bug signal.
    - Elevation-aware search is an M8 item.
17. **Dependencies (taken):** rusqlite 0.40.2 with `default-features = false, features = ["bundled"]` (SQLite 3.53.2), rayon 1.12. Both are verified in the local registry; the offline build is checked in Wave 0, with fallback `=0.39.0`.
18. **The policy has a one-mon party, no bag, no switching (taken).** Losing leads to a whiteout and the respawn, a normal state. Party switching arrives with the M8 party menu.
19. **Debug versus release:** unit and e2e tests run small budgets in debug. Measured runs use `cargo xtask explore` (release by default).
20. **Open: the default `snapshot_every_depth` (8) and `cache_capacity` (4096).** Task H tunes them from the cache hit rate and the db size.

---

## Wave 0 as built
**Step 0.** M6 Wave 0 had not merged: `xtask/src/main.rs` has no `mod studio;`, so `mod explore;` and the `explore` dispatch were added alone (`<rom|parity|shot|extract|checkpoints|explore>` usage). The merge with M6 resolves that line and `Cargo.lock`.

**Offline build.** `cargo build -p explore --offline` (and `--locked`) succeeds with rusqlite 0.40.2; the `=0.39.0` fallback was not needed. Offline resolution also downgraded `redox_syscall` 0.9.5 → 0.9.4 (redox-only, unrelated); that was reverted by hand in the lock, and the `--offline --locked` build passes with 0.9.5.

**Versions in the lock (new):** `rusqlite 0.40.2`, `libsqlite3-sys 0.38.2` (bundled SQLite 3.53.2), `fallible-iterator 0.3.0`, `fallible-streaming-iterator 0.1.9`, `rayon 1.12.0`, `rayon-core 1.13.0`, `crossbeam-deque 0.8.8`, `crossbeam-epoch 0.9.21`, `crossbeam-utils 0.8.23`, plus `libsqlite3-sys`'s build dependencies `pkg-config 0.3.34` and `vcpkg 0.2.15` (compiled, unused with `bundled`). Already present: `cc`, `smallvec 1.16.3`, `bitflags 2`, `either`, `tempfile`. No `hashlink`/`hashbrown` (`cache` feature off).

**Baseline (release, one thread, Apple M4 Max).** Replaying `scenario::cached_route()`, best of 3 × 5 replays (`cargo test -p explore --release --test w0_baseline -- --ignored --nocapture`):

| flavor | route frames | `Session` | `Session<Probe>` |
|---|---|---|---|
| extracted | 19082 | ≈ 2.60M frames/s | ≈ 2.65M frames/s |
| stub | 1543 | ≈ 2.76M frames/s | ≈ 2.72M frames/s |

That is about 50× the 50k frames/s that the "Done when" nodes/s thresholds assume, so **no rescaling**. `Probe` costs nothing measurable.

**Implemented and tested (W0):** `model` (RON round trip of every `MacroKind`, `SeenKind` numbers), `config` (defaults, partial RON, errors), `runs` (round trip, whitespace, bad runs, saturation), `probe` (route replay with identical hashes on every frame in both flavors; at least one script recorded on the real flavor; `build_hash(&probe) == build_hash(&inner)`; provided methods forwarded, checked against a game that overrides them), `guard` (message and `file:line:col`, nesting, unguarded panics untouched, a game panic inside `Session::step` caught), `observe` (object safety on a `Session<Probe<_>>`), `schema.sql` (idempotent DDL on a file db, WAL/NORMAL/foreign keys, STRICT, the `node_live_key` partial index, status check, u64 bit-cast).

**Contract deltas (additions only):**
- `guard::catch(f) -> Result<R, Panicked>`: the guard for any closure. `step_guarded` is `catch(|| s.step(keys))`.
- `db::SCHEMA_SQL` (`include_str!("schema.sql")`) and `db::init_connection(&Connection)`, which sets `journal_mode=WAL`, `synchronous=NORMAL` and `foreign_keys=ON`, then runs the DDL. **Task C:** `Db::open` calls it. `DbError`'s `Display`, `Error` (with `source`) and `From<rusqlite::Error>` are implemented.
- Type aliases with the same types as the contract: `rebuild::Repair` (the `repair` closure type) and `softlock::ExpandAll<'a>` (the `expand_all` closure type).
- `Graph::is_empty` (clippy `len_without_is_empty`) and `impl Default for Policy` (clippy `new_without_default`).
- `Detectors<G: GameData>`: the struct carries the bound, because `Session<G>` requires it.
- `Config::to_ron()` (pretty RON, for the `run` table). Every config struct is `#[serde(default)]` and derives `PartialEq`, so a config file lists only what it changes.
- `Flag` and `Panicked` derive `Clone, PartialEq, Eq, Debug` (`ExpandError` needs them).
- `expand::expand` carries `#[allow(clippy::result_large_err)]` (the error returns the partial expansion).
- Defaults the plan left open (Task H tunes them): `age_penalty 1`, `expanded_penalty 4`, `Weights { flag 40, var_value 20, map 100, script 30, cell 2, new_key 10, goal 1000, bug 500, balance 0 }`. The seed is written `0x005E_EDE7` (= `0x5EED_E7`, clippy byte grouping).

**Notes for Wave 1:**
- The module list in `lib.rs` is alphabetical (rustfmt reorders `mod` items), with each module's owner in a trailing comment.
- Each stub file starts with `#![allow(unused_variables, dead_code)] // Wave 0 stub`. Remove it when you implement the module.
- The private fields of the stub structs (`Reach`, `Graph`, `SnapCache` with a `tick`, `Archive`, `Detectors`, `BattleTracker`, `Policy`) are placeholders from the comments. The owning task may change them.
- `testing.rs` has the four wrapper structs without `GameData` impls (Task E). A test game must forward the provided `GameData` methods too: the stub's `AfterStarter` new game needs `battle()`, or `World::new` panics in `mon.rs`.
- The guard's hook is process-wide and installed once; it is silent only on a thread with an active guard. Panics elsewhere still print, test-harness panics included.
- `cli::main` and `wire.rs` are empty (Task G). Until then the `explore` bin and `cargo xtask explore` hit `todo!()`.
- `.gitignore` already covers `target/explore/` (`target/`) and `explore.db`. Unchanged.


**Scheduling (taken).** M7 started early, in parallel with M6 Wave 1 (requested by the user). Wave 0 and Wave 1 are headless and independent of M6 files. Wave 2b waits for M6. Once M6 is complete, M7 gets a second pass against M6's final APIs.

## Task D as built
**Duplicates (deviation from "later steps chain from the existing best node").** An equal key does not mean an equal state, and a node's inputs must replay from its parent's end state. So a duplicate step (its key is live with a node that reached more goals, or as many goals in at most as many total frames; Task G) is handled like this:
- If it is the job's last step, it is dropped. Its sightings (`seen`, cells) are credited to the live node.
- If later steps follow and its digest equals the live node's, the job chains from the live node, with no row.
- Otherwise it is inserted as a `Merged` history row (`merged_into` = the live node, its key kept), and the job chains from it.

Consequence: the `node` table grows by one non-live row per such duplicate. Those rows never enter the frontier, but they count in `key_hits`.

**Key hits.** `key_hits(k)` = rows with key `k` (any status) − 1. `Graph::from_rows` rebuilds it exactly, so the graph is a pure function of the rows. A duplicate that leaves no row adds no hit. Nothing about key hits is persisted beyond the rows.

**Batch.** `Config::batch == 0` means `runner::DEFAULT_BATCH` (32), independent of the thread count. Results do not depend on `threads` with the default config. A test compares 1 and 8 threads.

**Findings.** A `FindingDraft.tail` starts at the job parent's end state and includes the job's steps. The delta attaches it to the job parent, with `total = parent.total_frames + frames(tail)`. The model's `GenerationDelta::findings` node is `Option<NodeId>`: `None` means the root job failed and the tail starts at a new game (Task C: `finding.node` is nullable).

**Stall.** `Runner::stalled()` is true when the root job already ran and left no node (the graph is empty at generation > 0), or when the frontier is empty. `plan()` then returns no jobs, and the caller stops.

**Other contract additions:**
- `graph`: `mix`, `is_live`, `Graph::{nodes, key_hits, children, next_id, ancestor}`.
- `select`: `tournament_in` (a frontier collected once per plan) and the placeholder `selection_score` (Wave 2 switches to `fitness::selection_score`).
- `mutate`: `barren`, `choose`, `choose_with`, `apply`, `mutate_with` and `PROBE_EXPANDED` (3).
- `runner`: `DEFAULT_BATCH`, `batch_size`, `generation_rng`, `ROOT_MACRO`, `Runner::{config, stalled, take_fresh}` and `jobs(planned, start)`.

**Behaviour:**
- **Planning RNG:** a local seeded by `(seed, generation)` only, so resumed runs plan identically.
- **Forced probe:** a barren node (`expanded ≥ 3`, no child) gets it at most once per plan. Its `expanded` grows with every job planned from it, so the forced probe never repeats at the same `expanded` count, without state outside the rows.
- **Snapshots:** `delta.snapshots` holds the nodes with `depth % snapshot_every_depth == 0` (0 = never). Every inserted node's end state is in `take_fresh()` for the cache.
- **In-delta supersession:** a supersession inside one delta writes the old row already `Merged` instead of listing it in `merged`.
- **`stats.frames`:** counts steps' input frames plus each finding's tail beyond its job's steps.

## Task C as built
**Schema v2** (`SCHEMA_VERSION = 2`): `finding.node` is nullable (`NULL` = the tail starts from a new game, the root job's findings); `GenerationDelta.findings` and `FindingRow.node` carry `Option<NodeId>`. No `key_hit` table: `Graph` key hits are rows per key − 1, rebuilt from the node rows.

**Contract deltas:**
- `rebuild::Repair<'a> = dyn Fn(Option<&Checkpoint>, &Macro) -> Option<(Vec<Run>, Checkpoint)> + Sync + 'a`: `None` is a new game, so a diverged root re-expands from boot; the lifetime lets the closure borrow. `revalidate` takes `&Repair<'_>`.
- `rebuild::revalidate_with_key` (`#[doc(hidden)]`): `revalidate` with the repaired nodes' key and region from a closure (`KeyFn`); `revalidate` passes `key::key_parts` + `key::key`/`region` with `cfg.key`. Tests use fakes.
- `ReachError::Diverged.got` is `Option<u64>` (`None` = the end frame was not capturable). `ReachError` implements `Display`, `Error` and `From<DbError>`.
- `Db`: `open_in_memory`, `build`, `meta_build`, `set_build` (meta build + open findings of other builds → `stale`), `drop_snapshots_except`, `set_status_and_snapshots` (one transaction per revalidated level). `apply` errors (`Codec`) on unknown ids in `merged`/`expanded`; `merged_into` may name a later row of the same delta (Task G: written after the inserts instead of deferring foreign keys).
- `SnapCache`: `contains`, `len`, `is_empty`, `hit_stats` (one hit or one miss per `reach`), `clear`, `set_build` (`reach` ties the cache to the game's build and clears it on a mismatch).
- `FindingRow`/`RunRow`/`DbStats`/`RevalidateReport` derive `Clone, PartialEq, Debug` (+ `Eq`, `Default` where possible).

**Behaviour:**
- `Db::open` checks schema and flavor before the DDL and the WAL switch; both connections set `busy_timeout(5 s)`. Directory creation errors map to `DbError::Codec`. The seed is the caller's `set_meta("seed", …)`.
- `apply` writes every `delta.snapshots` entry to the db: the delta carries only the `snapshot_every_depth` ones. A stale or gone finding seen again reopens with the new repro and build.
- `revalidate`: merged nodes of another build are replayed too (they can be ancestors); a key collision resolves only against holders already checked in the run, pending otherwise (the holder that keeps the key competes by more goals reached, then fewer total frames, then lower id, as `Runner::merge` (Task G); a holder that breaks or changes key hands it to the best waiting or merged-into node). Total frames are recomputed down the tree; repaired nodes keep `progress` and fitness. A db error while reaching a parent is returned; `Diverged`/`Missing`/`Restore` break the child.

**For Task G:** `db.rs` signs findings with a private `draft_signature` that follows `findings::signature`'s spec (Task E was a stub); swap in `crate::findings::signature` when wiring. `progress` of repaired nodes is not re-scored (`goals::progress` needs the summary).

## Task E as built
**Contract changes (review decisions):**
- `softlock::probe(start, region, map_exits, k, breadth, expand_all)` takes the start's region (the node's `region`); there is no region-inferring variant. `k == 0 || breadth == 0` gives `Free`. `ExpandAll` is `FnMut(&Checkpoint, u32 remaining)`, with `remaining = breadth - reached` (at least 1).
- `findings::signature`: FNV-1a 64 of `kind_name(kind)` (a stable per-kind string), the location's file without `:line:col`, the message with ASCII digits stripped, and the map. A panic keeps its signature when an unrelated edit moves its line; the finding row keeps the full location. `signature_of(kind, message, location, map)` and `kind_name` are public (Task G replaces Task C's private `draft_signature` with it).
- `findings::repro_trace` returns `Result<Trace, (Trace, u32)>`: `Err` with the trace up to the panicking frame and that frame (0 = `World::new`) when a frame before the last one panics.
- `findings::reproduce` replays to the last frame: same-signature raises before it are not terminal, and every flag of the last frame (`Detectors::after_step_all`) is matched. A panic is terminal and reproduces only on the last frame.
- `explore::testing` is behind the `testing` cargo feature, enabled for the crate's own tests through a self dev-dependency.

**Additions:** `Detectors::{after_step_all, round_trips}`; `findings::{signature_of, kind_name, end_state}`; `testing::{FadeTrap, FADE_TRAP_SCRIPT, INJECTED_PANIC, NOWHERE, DRIFT_CALLS, Drifting::new}`.

**Detectors:**
- Round trips run at the first capturable frame of each absolute `World::frame / roundtrip_every` bucket, so a replay from a new game checks on the same frames as the job.
- A frame that does not follow the last observed one (a `Session::load`) is a discontinuity: the shadow is dropped and the schedule restarts.
- The whole round trip (checkpoint, encode, decode, restore, compare) runs under the guard; a panic is a `RoundTrip` flag "round trip panicked: {msg} at {loc}". Shadow panics carry the location too.

## Task A as built
**Deviation: `Cross` and the stale view.**
- After a connection crossing, the engine's `StaleView` keeps the state uncapturable until the camera has entirely left the view at the crossing (`STALE_CLEAR` = 17x12 cells). The spec's "walk one cell in" never settles a capturable frame.
- Fallback in `Driver::resolve`: walk toward the nearest reachable cell a full view away. After `STALE_PUSHES` (2) script push-backs, talk to the nearest scripted object not yet tried; each object is tried once, whether the talk succeeds or not.
- Each fallback talk is visible in `Expanded::notes`, as "stale view after a crossing: talked to object N".
- **Changed test expectation:** on Route 101 during Birch's rescue, the triggers push the player back near the crossing. `Cross Up` from Littleroot therefore ends in Birch's lab, after the bag talk, the starter and the first battle. It does not end on Route 101. `real_route_moments` asserts the lab, a one-mon party and one battle of kind 1. `real_stale_view_tries_each_object_once` covers a text-only nearest object.
- The engine fix is an M8 follow-up (see Findings).

**Contract additions:**
- `macros::{candidates_with(game, snap, reach, cell_shift), enter_approach}`; `path::Reach::{map, width, height}`.
- `expand::{Intents, expand_with}` (`Intents` is the policy seam, implemented for `policy::Policy`); `sim::simulate_with`.
- `Expanded::{last_script, slow, notes}`; `expand::{MAX_SLOW, STALL_FRAMES, BLOCKED_FRAMES}`; `sim::RANDOM_TRIES`.

**Conventions:**
- **Only `SlowFrame` is report-only:** it is recorded in `Expanded::slow` (at most `MAX_SLOW`) and never ends a macro. Every other flag ends it (`observe.rs`).
- **Finding tail:** runs from the job's start (its checkpoint, or a new game) through every earlier step to the failing frame.
- **Finding map:** `World::map()` after the failing frame. For a panic inside `World::new`, or in a restore, it is `new_game().map` with an empty tail, or the checkpoint's map with the tail of the steps so far.
- **Settle step:** a job without a start gets an implicit `Settle` step first.
- **`summary.maps`:** excludes the start map.
- **WalkTo candidates:** active coord-event cells (var condition met) come first; inactive ones are not candidates. Then there is one cell per coarse block, nearest its centre, excluding the player's block.
- **ArrowWarp(d):** walkable, never left toward d.
- **Overrides:** the start's `setmetatile` overrides apply until the map changes, then none.
- **Host time:** `Instant` is used in expand only for `max_step_ns` and `SlowFrame`; it is report-only.
- **Stalls:** more than `STALL_FRAMES` frames holding one direction toward a planner-walkable cell gives `PlannerDivergence`; more than `BLOCKED_FRAMES` blocked frames gives `NoPath`.

**Notes for Task H:**
- **(M4) Elevation:** the planner ignores elevation (decision 16), so a `PlannerDivergence` on a map with elevation changes (bridges, stairs) can be a false positive. Triage those before treating them as collision bugs. `a_planner_divergence_is_flagged_after_a_stall` builds one on purpose.
- **(M5) Seeds:** `Plan::Random` draws from `Rng64(seed ^ i)` per step `i`. Jobs whose seeds differ only in the low bits can share streams across steps (`seed ^ 1` at step 0 equals `seed' ^ 0` for `seed' = seed ^ 1`). This is harmless for determinism, but it is worth knowing when reading diversity numbers.

## Findings
- **House map 0x101, `Inspect` at (5,1) and (3,1) facing Up (Task A's probe):** the script ends but the fade never returns to 0, so the state is never capturable again. Likely a real engine or content bug; `CapturableStuck` should report it once Task G runs.
- **M8 follow-up (engine): the stale view after a connection crossing should be capturable.** Today it is capturable only once the camera has left the crossing's view, so `Cross` needs the walk-away and talk fallback (Task A as built). Capturing it, by snapshotting the stale rect or redrawing the view on restore, would let `Cross` end one cell into the new map as specified.
- **M8 follow-up (explorer): make summary-only goals observable in state.** `wild_won` (stub) and `rescue_started` (real) are read from an expansion's summary, not from the end state, so the key cannot tell a lineage that reached them from one that did not. Task G's merge rule (more goals first) keeps such lineages alive, at the cost of the invariant below (a node can be superseded by its own descendant). A flag or var set by the engine (or a key part derived from the save state) would make them part of the key, and the merge rule could return to frames only.

## Task G as built
**`wire.rs`.** `Explore<G: GameData + Clone + Send + Sync + 'static>` holds the game, `Db`, `SnapCache`, `Runner`, `Evaluator`, `Config` and its own rayon pool (`cfg.threads`, 0 = rayon's default), plus `opened: OpenState`, `revalidated: Option<RevalidateReport>` and `config_changed` (the config differs from the last `run` row's, `Db::last_run_config`; the CLI prints a note).
- `Explore::open(game, path, cfg)`:
  - `Db::open` with `build_hash(&game)` and `content::STUB`;
  - a fresh db records `meta.seed`; an existing db's seed replaces `cfg.seed`, so a resumed run plans as a straight one;
  - on `build_changed`: `mark_all_stale`; then, on `build_changed` or when stale nodes remain (an interrupted revalidation), `revalidate` on the pool;
  - the runner is `Runner::new(cfg, db.nodes(), next)`, where `next` is `meta.generation + 1` once a generation was applied and 0 otherwise. A fresh db no longer writes `meta.generation` (`Db::open`); `apply` sets it, so its presence means "applied";
  - the archive is `Archive::from_db(seen, distinct row keys, coverage)`.
- `generation() -> Option<GenOutcome>` (`None` = stalled, nothing planned) runs `plan` → `runner::jobs` with `cache::reach` (a reach error fails the generation) → `pool.install(run_jobs(sim::simulate))` with a fresh `Detectors` from the observer factory → `merge` through `BugWeighted` → `db.apply` → `take_fresh()` into the cache.
- `run(budget, progress)` loops until the budget (`Budget`, counted from the call) is spent or the runner stalls, calls `progress(&Progress { summary, last, db })` after every generation, and records a `run` row. `RunSummary` holds generations, inserted nodes, frames, findings, wall time, `phases` and `stalled`.
- `revalidate()` (CLI `revalidate`): `mark_all_stale`, `rebuild::revalidate`, then reloads the runner and archive and clears the cache.
- `repair(game, start, macro, cfg)`: the revalidation repair. It re-expands the macro with `expand::expand` (policy, `NoObserver`) from the start checkpoint, or from `Session::new` for `None`. **Repaired nodes keep their `progress` and fitness** (not recomputed: `goals::progress` needs the summary of every expansion along the lineage).
- `finding_trace(game, db, id, before)` is the lineage inputs plus the tail through `findings::repro_trace`. `before > 0` replays a prefix that ends `before` frames early, with its hashes.
- `reproduce(game, db, id, cfg) -> anyhow::Result<Result<(), String>>`: `Err` when it cannot be tried (no such finding, a db error), `Ok(Err(why))` when it does not reproduce. `findings::reproduce`, or for a `Softlock`, `findings::end_state` and then `sim::trap` again, trapped with the same `softlock_signature(map, region)`.
- `draft_of(&FindingRow)`.
- **Bug weight:** `BugWeighted { inner, weight }` wraps the `Evaluator`. The last step of a job that stopped on a finding (`StepResult::bug`, set by `sim`) gets `weights.bug` added to `fitness.bug` and `fitness.total`. A finding raised before the job's first step credits no node. The bonus is stored in the row, so a resumed run sees it.
- `GenOutcome.phases` / `RunSummary.phases` (`Phases { plan, jobs, merge, apply }`): host time, report-only, never in the db.

**`sim.rs` (wiring edit).**
- `Plan::Softlock { k, breadth }` calls `sim::trap(game, start, k, breadth, cfg)`, which is `softlock::probe` with the start's region (`key::region(key_parts(start))`, equal to the node's) and `map_exits`.
- `expand_all` restores a fresh `Session<Probe<G>>` per candidate, takes `MacroKind::Wait { frames: PROBE_WAIT_FRAMES }` (300) first and then `candidates_with` in order, and `expand`s each under `guard::catch` with the policy and no detectors. It keeps at most `remaining` successful ends `(region, map, cp)`; panics and failed expansions are skipped. `Wait` (a new last `MacroKind` variant: press nothing, then settle) lets a wandering object free the way; it is never a search candidate. The probe's frames are counted in `JobResult::probe_frames` / `GenStats::probe_frames`, and the run's frames/s includes them.
- `Trapped` gives a `Softlock` finding with an empty tail, no macro, the start's map and cell, and the message `findings::softlock_message(region, k, reached)` (decimal, no hex). Its signature is `findings::softlock_signature(map, region)` (`findings::signature` reads the region back from the message), so the same trap dedupes whatever the reached count. `Free`/`NoExits` give `Stop::NotTrapped`. A probe without a start state is `Pruned`.
- `sim::softlock_k(cfg, map)` (per-map K, else `softlock_k`) is used by `repro`. The root's `World::new` panic was already a finding (`Session::new` under `guard::catch`, empty tail, `new_game().map`), so `PanicOnScript` at `NEW_GAME_SCRIPT` gives one finding with `node = NULL`, and the runner stalls.

**Other wiring edits (Wave 1 files):**
- `db.rs`: the private `draft_signature` is gone; `apply` signs with `findings::signature`.
- **`db.rs`, `apply` performance:** foreign keys are no longer deferred in `apply`. The in-delta forward `merged_into`s (a row naming a later row of the same delta) made SQLite's deferred-violation counter non-zero, and while it is non-zero SQLite scans the child tables referencing `node` on each parent-row change to see whether a violation is resolved. `seen`, `coverage`, `battle` and `node.merged_into` have no index on that column, so the cost grew with the db: `apply` was 46% of a 30 s real run (13.7 s). Now:
  - a superseded node first becomes `merged` (freeing its live key);
  - rows insert with `merged_into` only when it names an existing node;
  - the in-delta `merged_into`s (ids in a `BTreeSet` of the delta's ids) are written after the inserts.

  The db share fell to 2.8 s of 30 s. Same rows and same digests. `set_status_and_snapshots` (revalidation) still defers. No schema change.
- `select.rs`: `selection_score` is a re-export of `fitness::selection_score` (same formula).
- **Merge rule (deviation): more goals reached wins a key, then fewer frames.** In `runner.rs`, a step whose key is live supersedes the live node when `(progress, -total_frames)` is greater; before, frames alone decided. `progress` is not part of the key, so a lineage that reached a summary-only goal (the stub's `wild_won`, or the real `rescue_started`) was dropped as a duplicate whenever an earlier node held the same key with fewer frames. Seen on the stub with `cell_shift` 0: stuck at 2/3 goals. `rebuild.rs`'s collision winner follows the same order (`Reverse(progress)`, total frames, id), otherwise revalidation flipped such keys back. Tests: `d_runner::more_goals_supersede_even_with_more_frames`, `c_rebuild::more_goals_keep_the_key_against_fewer_frames`. **New invariant:** a node can be superseded by its own descendant (a later step of the same lineage, same key, more goals), so a `merged` node can be an ancestor of the live node that holds its key; nothing assumed otherwise (replay walks `parent`, never `merged_into`). The M8 follow-up in Findings would remove the need.
- `tests/a_expand.rs`: `Plan::Softlock` without a start is `Pruned` (was `"unwired"`).
- Slow frames (`Expanded::slow`) and the Cross notes (`Expanded::notes`) are carried as `JobResult::{slow_frames, notes}` and counted in `GenStats::{slow_frames, notes}` and `RunSummary`; the run summary prints both counts. They are not stored per node.
- Wave 0 type additions: `StepResult::bug`, `JobResult::{probe_frames, slow_frames, notes}`, `GenStats::{probe_frames, slow_frames, notes}`, `MacroKind::Wait`.

**`cli.rs`.** `explore <command> [--db P] [--config P]` (defaults `Db::default_path(content::STUB)` and `Config::default()`):
- A `--config` path that does not exist is an error.
- `run [--nodes N|--frames N|--generations N|--secs N] [--threads N] [--seed S]`: default 10 generations, one budget only. It prints a note when the config differs from the last run's, the revalidation counts when `open` revalidated, then a line every 10 generations (`gens` = the 1-based count of generations applied, `valid nodes`, `inserted/s`, `frames/s` with probe frames, goals `i/n (name)` via `goals::name(progress - 1)`, maps, cells, findings), a summary (with slow frames and notes), and the host time per phase with the cache hits.
- `status`, `findings [--all]` (open only by default, by hits), `show <id>`.
- `export <id> [--before N] [--out P]`. **Deviation:** the default output is `Db::default_path(flavor).parent()/repro/finding-<id>[-before-N].ron`, i.e. `target/explore/{stub|emerald}/repro/`, whatever `--db` is.
- `repro <id>`: exit 0 when it reproduces, 1 when it does not (`cli::NotReproduced`), 2 on any other error (`cli::exit_code`; other commands exit 1 on error).
- `revalidate` prints the time and report; when `open` already revalidated (a build change or stale nodes), it reports that run instead of revalidating twice.
- `coverage <map>`: the map is an id (decimal or `0x`) or a `MapData::name`, case-insensitive. Cells print as `#` blocked, `.` open, `1`–`4` for 1, 2–9, 10–99 and 100+ hits.
- `digest`; `reset --yes` deletes the db and its `-wal`/`-shm` after proving it is an explore db no one writes: `Db::open_read_only` (schema check), then `BEGIN IMMEDIATE` with a zero busy timeout (refused as busy), rolled back and closed before the delete. A missing db prints "no db at …".

The bin exits with `cli::exit_code`; xtask is unchanged (`cargo xtask explore` already forwarded).

**E2E tests.** `tests/e2e_stub.rs` runs on the stub and returns at once on the real flavor. `tests/e2e_real.rs` is the reverse.
- 30 generations with `cell_shift` 0 stand on all 196 reachable stub cells (`path::reach` from the first capturable state) and reach 3/3 goals. **Deviation:** with the default 4x4 blocks, the BFS walks between block centres follow the same lines and cover only 105 of the 196 cells, even at 200 generations; `cell_shift` 1 covers 181. That is the "Done when" 100% item for Task H.
- `--threads` 1, 4 and 8 give an equal `content_digest` (10 generations).
- Resume: 15 + 15 generations with a reopen equal 30 straight (rows and digest).
- `PanicOnScript(NEW_GAME_SCRIPT)`: one `Panic` finding (node `NULL`, location in `testing.rs`, an empty trace). `reproduce` passes, the run stalls after one generation, and the db stays stalled once reopened.
- `WallAround` at the start: one `Softlock` finding on node 1 with an empty tail. The trace length equals `total_frame`, `reproduce` passes, and it does not reproduce without the wall.
- A `Rebuilt` game on an 8-generation db revalidates every row `Valid` (0 repaired, 0 broken), with the same digests and statuses on the new build, then explores on.
- The CLI end to end on a temp db: run, status, findings, digest, coverage by id and by name, error cases (a missing `--config` among them), `export <id> --out <tmp>`, the exit codes, `reset --yes`.
- `reset` refuses a file that is not a db and a db with an open writer (a held `BEGIN IMMEDIATE`).
- An interrupted revalidation (stale nodes on the same build) is revalidated on open, to the same digest.
- `git status --porcelain` is unchanged by a run.
- Real flavor, debug, `e2e_real`: **10 generations** (the plan's 5 end just before the first goal) reach progress ≥ 1 and ≥ 2 maps, and every finding raised reproduces.

**Numbers** (Apple M4 Max, 16 rayon threads by default, release unless noted):
- **Debug test times:** `e2e_stub` 9.6 s (10 tests); `e2e_real` 4.3 s.
- **After the review (probe `Wait`, probe frames counted), real `run --generations 50`:** 984 nodes inserted (615 valid), 1,585,889 frames with the probes, 7/7 goals (`may_defeated`), 13 maps, 988 cells, 2 findings (the two Fades, no softlock). `--threads 1`: 4.5 s, about 356k frames/s; `--threads 8`: 1.6 s, about 991k frames/s. `explore digest` is `0x4309279c8e23c2e1` on both. The numbers below predate the review: the probe now waits first, so the search and digests differ, and frames/s then left probe frames out.
- **Stub, `cargo xtask explore run --generations 50`:**
  - 0.5 s, 130 nodes inserted (32 valid), 3/3 goals, 105 cells, about 1.3M frames/s;
  - `--threads 1`: 2.2 s, about 313k frames/s;
  - `explore digest` is `0x51be88a0fa670e1b` on both.
- **Real, determinism (60 generations, 1,204 nodes, 1.03M frames):**

  | Run | Wall | Frames/s | `content_digest` |
  |---|---|---|---|
  | 1 thread | 5.6 s | ≈ 183k | `0x084f2da3e59fb613` |
  | 8 threads | 2.0 s | | `0x084f2da3e59fb613` |
  | 16 threads | 1.9 s | | `0x084f2da3e59fb613` |
  | 30 + 30 resumed | | | `0x084f2da3e59fb613` |

- **Real, `run --generations 100` from an empty db:**
  - goals by generation: 3/7 at generation 9, 4/7 at 19, 5/7 (oldale) at 29, **7/7 `may_defeated` by generation 49** (under 2 s);
  - 3.8 s in total;
  - 13 maps by generation 49.
- **Real, `run --secs 60` from an empty db:**
  - 1,062 generations; 20,567 nodes inserted, 9,645 valid (35,580 rows);
  - 342.7 nodes/s and 348.8k frames/s;
  - 14 maps, 1,614 cells, 3 findings;
  - the db is 34 MB (about 1 MB per 1,000 rows);
  - host time: plan + reach 7.3 s, jobs 46.0 s, merge 0.4 s, db 4.3 s; cache 27,195 hits, 6,757 misses.
- **Real, `explore revalidate` of that db** (same build, forced): 3.4 s, 9,645 valid, 0 repaired, 0 broken.
- **Real, `run --secs 30`:** 427.8 nodes/s and 427.8k frames/s after the `apply` fix (it was 212k frames/s before).

**Findings raised by the real runs** (all three reproduce with `explore repro`):
1. `CapturableStuck` on map `0x0103` (3,2), "idle but not capturable … Fade", after `Inspect (3,1) Up`, script id 148, log `SPECIAL_StartWallClock`. This is the house fade already listed in Findings: the wall-clock special is not implemented and leaves the screen black.
2. `CapturableStuck` on map `0x0101` (5,2), the same Fade.
3. `Softlock` on map `0x0104` (12,8): the probe reached 51 states within 3 macros and none left the region. **Triaged as a false positive:** the wandering NPC of the map (Birch's aide) stood on the exit approach (10,8), and every probe expansion ran from the same frozen checkpoint, so the aide never moved and no macro could pass. Fixed by the probe's `Wait` candidate (300 idle frames first, so the aide walks on). With it, `explore repro 3` on that db says "not trapped any more: Free" (exit 1), and a 50-generation run raises no softlock. `WallAround` (stub) still reproduces as `Trapped`.

**Notes for Task H (tuning):**
- **Probe cost:** softlock probes (5% of jobs, up to `softlock_breadth` 64 expansions each, plus the 300-frame `Wait`) run in the parallel phase. Before the review their frames were not counted; they are now (`GenStats::probe_frames`). They and the restores make frames/s look low next to the 2.6M frames/s per core baseline. Per thread it is about 27k counted frames/s; jobs take 77% of the wall time.
- **Stragglers:** a batch of 32 waits for its slowest job (probes, battles). 8 → 16 threads barely helps (2.0 → 1.9 s). Larger batches or cheaper probes (lower breadth, or `softlock_sample` as specified rather than D's 5% op weight) are the levers.
- **Cache misses:** they grow with the tree (6,757 in 60 s, each a replay of up to `snapshot_every_depth` − 1 nodes on the main thread; plan + reach 7.3 s of 60 s). Raise `cache_capacity` or lower `snapshot_every_depth`.
- **Stub coverage** needs `cell_shift ≤ 1`, or a WalkTo target choice that varies, for the 100% "Done when".
- **Supersession:** more than 70% of rows are `merged` history (more goals or fewer frames supersede); each keeps a row for `key_hits`.
- **Findings to triage:** the two Fade `CapturableStuck`s (real engine gaps, for Milestone 8). The `0x0104` softlock is resolved (above).

**Notes for Wave 2b (studio):**
- `wire::finding_trace(game, &db, id, before)` already gives the NewGame `Trace` cut `before` frames early with its hashes. S1's `finding_trace` can call it, or `findings::repro_trace` on `db.lineage(node)` plus the tail when node is `None` (an empty lineage).
- `Db::open_read_only` works while `explore run` writes (WAL). Read `DbStats`, `findings()` (`status` `open`/`stale`/`gone`, sort by hits) and `coverage(Some(map))`. The CLI's coverage grid shows the bucket rule (1, 2–9, 10–99, 100+).
- The explorer process: `cargo run -p explore --release -- run --secs N --db P`. Its stdout prints a line every 10 generations, and `--threads` and `--seed` are flags. A db keeps its own seed after creation.
- Goal names: `goals::name(progress - 1)`; `DbStats::max_progress` counts broken nodes out.
