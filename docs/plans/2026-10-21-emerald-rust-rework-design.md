# Pokémon Emerald Rust Rework — Design

Date: 2026-10-21
Status: validated (brainstorming)

## Goal

A complete rework of Pokémon Emerald as a **new GBA engine written in Rust**, using the
user-provided **French ROM (BPEF)** only as an asset source. The game is in French.
A Dioxus desktop **studio** previews and edits all content, plays the latest build, manages a
**tree of semantic checkpoints**, and drives a **persistent evolutionary headless explorer**.

## Decisions

| Topic | Decision |
|---|---|
| Rework depth | New engine in Rust (agb), ROM = asset source only |
| Asset source / language | French ROM, French text; US decomp (`../pokeemerald`) used as a format/offset guide |
| Execution | Hybrid: portable `no_std` core; native build for headless/studio, real ROM in embedded mGBA (FFI) |
| Saves | Semantic checkpoints (serde `World` snapshot) + input traces; survive rebuilds |
| Scripts | Plain Rust, `async fn` scripts in a `content` crate, polled per frame by a tiny executor |
| Data | RON + PNG; ROM-derived files under git-ignored `games/emerald/`, compiled to const tables by `build.rs` |
| Explorer goals | Softlocks/crashes, coverage, completability, balance |
| Explorer evolution | Persistent & incremental (SQLite) + evolutionary search |

## 1. Workspace

```
crates/
  engine/       no_std, game-agnostic: overworld, battle, menus, text, async script runtime,
                virtual PPU, World/Checkpoint model, `Content` trait
  content/      no_std, the game: Rust scripts per map, typed flags/vars, data/ (RON/PNG) -> build.rs
  game-gba/     bin: agb + engine + content -> game.gba (flash saves, debug region in EWRAM)
  game-native/  lib: engine + content for host (software PPU, headless mode)
  extract/      French ROM -> git-ignored games/emerald/ (data, graphics, text; generated
                .rs scripts from Milestone 3)
  emu/          mGBA FFI wrapper (frame step, input, framebuffer, memory read)
  explore/      persistent evolutionary action-tree runner
  studio/       Dioxus desktop app
rom/            user ROM, git-ignored. Extracted original assets are never committed.
```

Dependency rule: `engine` never depends on `content`. The `Content` trait (maps, script lookup
by event id, species/moves/items/trainers, text) is the only contract.

## 2. Content & scripts

- Scripts: `async fn x(ctx: &mut Ctx)` using awaitable primitives (`say`, `choose`, `walk`,
  `wait_moves`, `battle`, `give_mon`, …). Flags/vars/species/text are typed enums — typos are
  compile errors.
- Data: RON for structured data, PNG for graphics, text tables per language (`fr`).
- Import: `extract` locates BPEF tables via structure/pattern search guided by the US decomp,
  stores a verified offset map with checksums, and writes everything to the git-ignored
  `games/emerald/` (Milestone 2). Decompiling Emerald event scripts into generated Rust, so
  the starting point is a playable copy of the original, is deferred to Milestone 3 (M2 only
  walks scripts to find their texts).
- Static analysis: `syn` scan of `content/` builds a flag/var read-write graph per script
  (studio event-flow graph, "checked but never set" warnings, explorer progress graph).

## 3. Runtime

- `step(&mut World, &impl Content, Buttons) -> FrameOut` (virtual PPU commands + events:
  `MapEntered`, `ScriptStarted`, `ScriptLine`, `FlagSet`, `BattleEnded`, …).
- Determinism: no floats, no clocks, one seeded RNG in `World`, fixed poll order,
  fixed-capacity collections (`heapless`).
- `Checkpoint` = versioned serde snapshot of `World`, only capturable when no script is running.
  `Trace` = parent checkpoint id + RLE inputs + build hash. Mid-cutscene states are reached by
  replay.
- Checkpoint tree: label, parent, trace, content hash. After rebuild, traces replay in the
  background; diverging nodes are flagged. Hand-authored checkpoints via a state editor.
- Parity: GBA build exposes frame counter + `World` hash + last event in EWRAM; a `parity` job
  replays traces on native and mGBA and compares hashes every N frames (CI gate).
- Panics carry script/map location; native panics become explorer findings with traces.

## 4. Explorer

- Macro-actions derived from state: `WalkTo(target)`, `Interact`, `UseField`, menu choices,
  battle `Move/Switch/Item/Run`. Expanded to inputs deterministically (pathfinding).
- Branch minimisation: abstract state key (map, coarse cell, flags/vars, party-strength bucket,
  key items); same-key nodes merged, fewer-frames wins. Battles resolved by policy.
- Fitness: novelty (new flags/maps/script lines) + progress (story graph distance) + bug signal
  (panic, softlock = no new abstract key reachable in K steps, invariant break) + balance
  deviation.
- Loop: tournament-select frontier (age penalty) -> mutate action suffix -> simulate (rayon) ->
  insert if new/better key.
- Persistence: `explore.db` (SQLite). Nodes record touched scripts/maps; after rebuild only
  intersecting nodes are re-simulated.
- Reports: softlocks/crashes (with repro trace), coverage heatmap, completability per
  checkpoint (goal-directed mode), balance tables. K configurable per map.

## 5. Studio (Dioxus desktop)

- Editors: map (pixel-identical via native software PPU; layers, collision, warps, NPCs,
  encounters), schema-driven forms for species/moves/items/trainers/text with live previews.
  Writes back to `content/`; data edits hot-reload instantly.
- Scripts: code view + "open in editor", per-map event-flow graph, `cargo check` diagnostics.
- Play: Native (fast-forward, frame-step, rewind ring buffer, state inspector, event timeline)
  and ROM (mGBA, debug-region timeline). One-click ROM build.
- Checkpoints: tree view (valid / diverged / broken), load into Native or ROM, branch, replay,
  diff, edit state.
- Explorer: start/stop, fitness weights, live stats, coverage overlay, findings with
  "Open at frame before".
- Script changes: background `cargo build -p content`, native engine relaunched in a child
  process, `World` handed over through a checkpoint.

## Milestones

1. **Skeleton**: workspace, `engine` step loop + virtual PPU, `game-gba` boots on mGBA showing a
   test tilemap; `game-native` renders same frame; parity hash check.
2. **Extract v0**: BPEF offset map; tilesets, Littleroot/Route 101/Oldale maps, player/NPC
   sprites, font, French text for those maps.
3. **Overworld slice**: walking, collision, warps, connections, NPCs, async scripts, text box.
4. **Battle slice**: single wild + trainer battle, Treecko/Torchic/Mudkip, Zigzagoon, Poochyena.
5. **Checkpoints**: serde World, traces, checkpoint tree, flash save on GBA.
6. **Studio v0**: map view, data forms, native play pane, mGBA play pane, checkpoint tree.
7. **Explorer v0**: macro-actions, abstract keys, fitness, SQLite persistence, findings in studio.
8. **Scale import**: all maps/data/scripts decompiled to Rust; then full engine parity
   (menus, bag, PC, field moves, double battles, contests…) iteratively.

## Open risks

- BPEF offsets must be discovered (decomp matches BPEE); French text encoding differs slightly.
- async future size on GBA (IWRAM/EWRAM budget) — keep scripts small, box large futures in EWRAM.
- Audio: the m4a/Sappy engine must be reimplemented or ported; deferred past milestone 4.
- Content recompile latency for script hot reload (seconds) — acceptable, data edits are instant.
