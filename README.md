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
  must match on every frame and the picture at every picture frame
- `cargo xtask shot`: play the same route and write ROM and native PNGs of each named moment to
  `target/shots/<moment>-{rom,native}.png`
- `cargo xtask extract [--rom <path>]`: export the ROM to `content/extracted/` (see Extract)
- `cargo xtask studio`: run the studio desktop app (release; see Tests for its gates)
- `cargo xtask explore <command>`: the headless explorer (see Explorer)

## Play
1. Extract your ROM (see Extract), then `cargo xtask rom`.
2. Open `target/game.gba` in any GBA emulator (mGBA, …) or on a flash cart. Or play natively:
   `cargo run -p game-native --features window --bin play` (arrows, X = A, Z = B, A = L,
   S = R, Enter = Start, Backspace = Select, Escape quits). Native play also saves and loads
   checkpoints (see Checkpoints): F5 saves one (at once when the field is idle, else at the
   first idle frame; the title says "saving…"), F9 reloads the last one saved or loaded, F8
   rewinds at least two seconds (a ring of 64 states, one a second; it cannot go back past
   the last save or load).
   `play --load <id|label>` starts at a node and `play --list` prints the tree. The title shows
   the frame, the node the session started from and the last action.

A new game starts in Littleroot Town outside Brendan's house, after meeting May and before
Birch's rescue (`crates/content/src/new_game.rs` lists the story vars and flags). Go north to
Route 101: Birch is chased by a Zigzagoon, his bag gives the starter screen (Left/Right to pick
a ball, A to choose, then yes/no) and the first battle follows. Then his lab, wild battles in the
tall grass of Routes 101 and 103, healing at Oldale's Pokémon Center, and May's battle on Route 103.
Littleroot, Route 101, Oldale, Route 103 and their interiors are walkable; doors, doormats,
connections, NPCs, signs and French dialogue work.

Battles: the D-pad moves the cursor and A confirms in the action menu (ATTAQUE, SAC, POKéMON,
FUITE) and the move menu (B goes back); A or B advances messages; when a move is to be learned,
the yes/no box and the move list work as in Gen3 (B in the list gives up learning). Losing
whites out to the last Pokémon Center (or home), with the party healed.

Known simplifications: singles only; no bag, party screen or Poké Balls (SAC and POKéMON return
to the menu, and the next usable Pokémon is sent out by itself); one battle background (grass or
plain); move animations are a lunge, the hit blink and the healthbox shake; no trainer sight
(trainers battle when spoken to); no sound, running shoes or start menu; parties and encounter
rolls are drawn when the battle is requested, before its transition (Gen3 draws some after it);
a script battle without a Pokémon that can fight is skipped and answers `Won` (Gen3 would start
it and lose); a starter screen that a script answers without a battle fades back to the field
with the script already resumed (Emerald's always battles).

Without an extracted dir everything builds against a small built-in map (stub mode,
`cfg(pkr_stub)`; force it with `PKR_EXTRACTED_DIR=/nonexistent`). There the shells and parity
(`Emerald::for_shells()`) start a new game with a Pokémon, and the one tall-grass cell at (12, 3) has wild battles.

## Checkpoints
A checkpoint is a versioned snapshot of an idle `World` (field mode, no script, battle, warp,
fade or scripted movement, and the view drawn from the current map); restoring it is
bit-identical to the run that was never interrupted. Natively they live in a tree:
`checkpoints/stub` or `checkpoints/emerald` after the content flavor.

- `nodes/c00001.ron` (committed): a label, the parent node (or a new game), the RLE input
  trace from the parent, the `World::hash` every 60 frames, and the state digest. No ROM data.
- `snapshots/` and `status.ron` (ignored): the snapshot cache (it holds ROM-derived names and
  is always rebuilt by replaying the traces) and the last verdicts.
- `local/` (ignored): `play`'s personal saves (or `$PKR_CHECKPOINTS` when set), a store of its
  own layered over the committed tree. `play --load`, `--list` and F9 see both trees, and a
  save may start from a committed node; local ids start at `c10001`, and nothing `play` does
  writes the committed tree. `cargo test` and parity only see the committed tree, and so does
  `cargo xtask checkpoints` except `list --local` and `verify --local`.

`cargo xtask checkpoints [--root <dir>] <command>`:
- `list [--local]`: the tree (id, frame, map, parent, label, last status). `--local` lists your
  local store layered over the committed tree (or `--root`), as `play` and the studio see it.
- `verify [--local | --accept]`: replays every trace on the current build. A node is Valid when it reaches
  the same state, Stale after a snapshot schema change, Diverged when this build reaches another
  state (with the first frame whose hash differs), Broken on a replay error or on a different
  state or hashes on the node's own build (nondeterminism). Fails unless every node is Valid;
  `--accept` re-baselines Diverged and Stale nodes, like the golden hashes, and still fails on
  Broken ones. A Valid node recorded on another build shows "valid (rehash)": `--accept` also
  refreshes its build, world hash and trace hashes, so the nondeterminism check covers the
  current build. `--local` verifies the local store layered over the committed tree (a local
  node may start from a committed one) and records the verdicts in the local `status.ron`; it
  cannot be combined with `--accept`, since the committed tree is read-only there.
- `seed`: records the parity route and writes its `route:<name>` nodes (replacing the earlier
  `route:` nodes, never others): `lab`, `after_wild`, `oldale_center`, `route103_after_may`
  (stub: `stub_start`, `stub_after_battle`, `stub_end`)
- `label <id> <text>`, `rm <id>` (refused for a node with children)
- `sav <id> [-o <path>]`: writes the node's flash image to `target/game.sav`, which mGBA loads
  with `target/game.gba`: the ROM boots at the node (hold B at boot to ignore a save)

Ids are `c00012`, `12` or a label. `cargo test` verifies the committed tree of the current
flavor (`crates/game-native/tests/checkpoint_tree.rs`), so an intended behaviour change needs
`verify --accept` and the updated nodes committed. The emerald tree's digests assume data
extracted from the FR ROM (`rom/emeraude-fr.gba`); another extract diverges. Loading or `sav`
of a node this build no longer reaches warns "cNNNNN diverged on this build". On the GBA,
L+R+SELECT saves to flash.

## Studio
`cargo xtask studio` (or `cargo run -p studio --features ui --release`) opens a desktop app
(Dioxus) that plays, inspects and manages the game. It reads the committed tree and writes only
your local store (`checkpoints/<flavor>/local/`, or `$PKR_CHECKPOINTS`) and `target/studio/`;
the explorer it starts writes `target/explore/`.

- **Play**: the native game and the ROM (`target/game.gba` in mGBA) side by side, each on its
  own thread. Click a pane to give it the keyboard (the layout of `play`: arrows, X = A,
  Z = B, A = L, S = R, Enter = Start, Backspace = Select); leaving it releases every key. Space
  pauses or resumes, `.` steps a frame, Tab held fast-forwards ×8, F9 reloads the node the
  trace starts from (the ROM pane: boots its last source again). On the native pane, F5 saves a
  local node (at the first idle frame) and F8 rewinds; the buttons do the same, plus ×2/×4/×8
  and New game. The inspector shows frame, map, camera, hash, mode, whether the state is
  capturable (or why not), the run mode, the parent node and the trace length, and an event
  list (map changes, scripts, battles, log lines, saves, loads, rewinds).
- The ROM pane boots a new game at start-up, or a node from the Checkpoints tab (materialised
  natively, then written as its flash). It shows the boot result, a timeline of the debug
  region (the last 60 frames: engine frame, hash, the native shadow's hash, late counters,
  saves, hash/step/loop scanlines) with the worst values, and a banner when the ROM was built
  from other sources than the studio. Build ROM runs `cargo xtask rom` and reloads the pane;
  Export save writes the flash to `target/studio/rom-<ms>.sav`.
- **Checkpoints**: committed nodes, then local ones, as a tree with their last verdict
  (valid, stale, diverged at frame N, broken, unverified). Verify replays every node in the
  background and keeps the verdicts in the local `status.ron`. Per node: load it into the
  native pane or boot it in the ROM pane, Branch (load and play; F5 saves a local child),
  "Open at…" (a slider over the node's trace: the native pane seeks to that frame, paused,
  and a save from there branches inside the trace), and, for local nodes only, Label and
  Remove (with a confirmation; refused for a node with children). Committed nodes change only
  through `cargo xtask checkpoints`. The tree refreshes after a save or an edit, and when a
  `nodes/` directory changes on disk.
- **Map** and **Data**: the read-only map view with its overlays, and the species, moves,
  trainers, encounters and texts tables. The map's "coverage" layer shades each cell the
  explorer stood on by its hits (1, 2-9, 10-99, 100+, as `explore coverage` prints them),
  read from the explorer db for the shown map, and again every couple of seconds while the db
  changes (a run from the studio or from `cargo xtask explore run`).
- **Explorer**: starts and stops `explore run` (`cargo run -p explore --release`, logging to
  `target/studio/explore.log`) with a budget in generations, seconds or nodes, an optional
  seed and thread count. The run keeps going when you leave the tab; quitting the studio
  (closing the window or Cmd+Q) stops it, and any export in flight. The state line says
  "building explore…" while cargo compiles, and a failed run (a build error, a panic) shows
  the log's tail. Every 2 s the tab reads `target/explore/<flavor>/explore.db` read-only (also
  a db written by `cargo xtask explore run`): nodes, nodes/s (averaged over the last 5 polls),
  the goals reached by name, maps, cells and findings. The findings
  table (open first, then by hits) gives kind, map, cell, message, hits, status, and "found
  on another build" when the finding's build is not this one. Per finding: "Open at frame
  before" (120 by default) replays it into the native pane, paused that many frames before
  its last frame, and shows the Play tab (the inspector's "last" repeats the other-build
  warning) (F5 from there saves a local node with a NewGame
  parent); "Export repro" runs `explore export <id>` (a NewGame trace under
  `target/explore/<flavor>/repro/`); "Copy repro command" copies `cargo xtask explore repro
  <id>`.

## Tests
`cargo test --workspace` runs unit tests and the ROM-backed tests in `crates/extract/tests/`, which
read both ROMs and the decomp. The `content` data tests check the real tables when
`content/extracted/` exists and the stub world otherwise; `crates/content/tests/build_modes.rs`
also builds `content` in stub mode and against a fixture extracted dir. The ROM-backed tests fail
if these are missing:

| Variable      | Default                 | What                                           |
|---------------|-------------------------|------------------------------------------------|
| `PKR_ROM_FR`  | `rom/emeraude-fr.gba`   | Pokémon Émeraude (BPEF) ROM                    |
| `PKR_ROM_US`  | `../emerald.gba`        | Pokémon Emerald (BPEE) ROM                     |
| `PKR_DECOMP`  | `../pokeemerald`        | a checkout of the pret/pokeemerald decomp      |

Parity (`cargo xtask parity`, `crates/emu/tests/parity.rs`) replays the route of
`crates/game-native/src/scenario.rs`: with extracted data it plays a new game: north to Birch's
rescue on Route 101, the starter screen (Mudkip) and the first battle, the lab, Route 101's tall
grass (a wild battle won with exp, one run from, one with a level-up), Oldale's Pokémon Center,
then Route 103 and May's battle (about 19000 frames, 8 battles); in stub mode it walks around the
stub map and fights in its tall-grass cell. Its named moments (`cargo xtask shot`) include the
starter screen, a wild battle's intro, the action and move menus, an HP bar draining, a level-up
message and May's intro. The ROM must not apply any frame late while something is visible
(`DebugRegion::late`); late applies on a black screen are counted in `late_black`, and each
must be explained: steps doing bulk work set `Frame::bulk` (a map load for a warp or a whiteout,
a whole-view field redraw such as a script teleport, the starter screen's load, a battle's load
and the field's restore after it), every late-black frame must follow such a step within the
display lag (`scenario::DISPLAY_LAG`), and there are at most as many as bulk events (runs of
bulk steps, `Recording::bulk_events`; about warps + 2 × battles + starters + teleports, 21 of 22
on the extracted route).

Checkpoints are held to one invariant: a capturable World, encoded, decoded and restored, has
the same hash, picture and future as the World that was never interrupted.
`crates/game-native/tests/checkpoint_route.rs` (plain `cargo test`) checks it natively at about
100 capturable frames of the route plus each named moment: the postcard checkpoint fits the
8 KiB flash budget (`MAX_ENCODED`; the route's largest is about 800 bytes) and round-trips, as
does its RON form; the restored session matches the live one on restore, then hash every frame
and picture at +1, +2, +60 and +120 frames; three points (the first, the middle, and the last
at least 120 frames before the end) follow the route to its end.
Parity adds both save directions through mGBA: native checkpoints (the lab, Oldale's Pokémon
Center after the nurse and the last capturable frame before May's battle; two points on the
stub) are written as `.sav` images, and the ROM must boot them (`boot == RESTORED`, the native
`build_hash`) and play up to 2000 route frames with every hash and a picture every 100 frames
equal to native; and a ROM save by the L+R+SELECT combo must decode to the ROM's hash, restore
natively to native's, equal `game_native::flash::encode_sav` byte for byte, and boot a second
ROM that matches native for 300 frames (each restore point's map is checked: the lab, then
Oldale's Pokémon Center). The ROM's costs are gated on the route: `World::hash` at
most 65 scanlines (`hash_lines_max`, 63 measured) and the whole loop body at most 217 of the 228
lines (`loop_lines_max`, 214–215 measured). The hash is buffered 256 bytes at a time and folded by
ARM code in IWRAM (`engine`'s `iwram` feature, enabled by `game-gba`), with the same bytes as the
plain bytewise FNV-1a. If the line gates trip, re-measure and justify the cost; don't just bump
the gates. Parity then runs the studio ROM pane's `#[ignore]`d tests on the ROM it built
(`cargo test -p studio --release --lib rom::tests:: -- --ignored`). Seeding
(`cargo xtask checkpoints seed`, run by hand
after an intended route or behaviour change, never by the tests) records the parity route and
writes its named points as `route:<name>` nodes of the current flavor's tree, replacing earlier
`route:` nodes. The committed checkpoint trees (`checkpoints/*/nodes`) hold inputs and hashes
only, never snapshots or other ROM-derived data, and verify `Valid` in `cargo test` like the
golden hashes: an intended behaviour change needs `cargo xtask checkpoints verify --accept`.

`cargo xtask rom` prints the ROM size, the ELF's IWRAM and EWRAM statics and the code and data
by symbol prefix (`xtask/src/budget.rs`). Until Milestone 8 Task M re-baselines them, it fails
when IWRAM or EWRAM grows past the slice baseline (IWRAM 788, EWRAM 460) by 2 KiB or 16 KiB,
when engine `.text` on the slice extract grows past its baseline (205448 bytes) by 128 KiB, or
when a full import's ROM exceeds 16 MiB; the slice and stub ROM sizes are reported only. The build remaps the checkout, `CARGO_HOME` and sysroot
paths (`--remap-path-prefix`), so the ROM does not depend on where the repository lives.

`crates/engine/src/world/tests/goldens.rs` pins World hashes on a synthetic game, one constant
per scenario (`golden_hashes`): `GOLDEN_WALK` (`0x66f39427d1ed0a8f` after `World::new` and
`0x0818a37264360e30` after 25 frames each of Right, A, nothing and Down), `GOLDEN_WARP`,
`GOLDEN_CROSSING`, `GOLDEN_SCRIPT`, `GOLDEN_WILD_BATTLE` and `GOLDEN_TRAINER`. They change
whenever the hashed state, its order or the frame logic changes; update them only for an
intended change (a change that moves one scenario updates only its constant).

Defaults are relative to this checkout's root (so `../emerald.gba` sits next to it). Set the variables to point elsewhere, e.g.
`PKR_ROM_US=~/roms/emerald.gba cargo test --workspace`. `cargo test -p extract --lib` runs only
the extractor's unit tests, which need none of these.

The studio's desktop UI is behind the `studio` crate's `ui` feature, so `cargo test --workspace`
and the workspace clippy skip it. Its gates are `cargo test -p studio --features ui` (display-free)
and `cargo clippy -p studio --all-targets --features ui -- -D warnings`; `cargo xtask studio
--check` runs both; they include display-free renders of the Play and Checkpoints tabs
(`dioxus-ssr`, on the committed tree with a temp local store) and the key-map tests.
`cargo xtask studio --smoke [secs]` (default 10) opens the real window, with the play panes and
two transport test-pattern panes, for `secs` seconds and fails if a test pattern delivers under
50 fps, a play pane's thread died or it drew nothing, or the run outlives `secs` + 60 s (then it
prints the output so far and a `sample` of the studio's threads). The play panes' rates are printed
as `INFO` lines, since they publish only changed pictures. It needs a logged-in GUI session (not SSH or CI without a display); the
window stays on top and takes focus while it runs. The delivered rate depends on the machine's
load (a load average of about 20 gave 35 fps), so a run that fails only the fps gate is retried
once. One `ERROR dioxus_desktop::edits` line when the window closes is harmless (an edit batch
the closing webview never acknowledged).

## Extract
`cargo xtask extract` reads your French ROM into editable, git-ignored files.

1. Place the ROM at `rom/emeraude-fr.gba` (or pass `--rom <path>`; BPEE also works).
2. Run `cargo xtask extract`. It exports into `content/.extracted.tmp/` and, only if that succeeds,
   replaces `content/extracted/` with it (a failed run keeps the previous output). It prints a
   summary (maps, tilesets, sprites, skipped graphics, script and text files, the codegen
   statistics, any undecoded script instructions).
3. See the output in `content/extracted/` (format v5, described in `crates/extract/src/schema.rs`).
   `--maps all|slice|<Dir,Dir,…>` picks the maps (default: the 15-map slice) and `--previews`
   asks for the map renders (written by default until Milestone 8 turns them off).
   Everything is named with decomp identifiers (`LittlerootTown`, `General`, `FLAG_HIDE_…`,
   `OBJ_EVENT_GFX_…`, `SPECIES_…`, `TRAINER_PIC_…`); ROM offsets only appear in the manifest:
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
   - `font/latin_normal.png` + `.ron`, `font/latin_small.png` + `.ron`: the Latin fonts and
     their glyph widths
   - `window/message_box.png` + `.ron`: the dialogue window frame, its palette and text colors
   - `previews/<Map>.png`: map renders
   - `text/<lang>.ron`: every text the scripts use, keyed `<Script>_T<n>`, re-encoding to its
     exact ROM bytes
   - `scripts/<Map>.rs` + `scripts/legacy_index.ron`: the map's event scripts as generated async Rust
     (compiled into `content` by its build script), and the root name to function index
   - `battle/*.ron`: battle data. `species.ron` (stats, types, abilities, learnsets, FR names),
     `moves.ron`, `types.ron` (type chart, type and ability names), `trainers.ron` (every
     trainer and party), `encounters.ron` (the exported maps' land encounters),
     `heal_locations.ron`, `strings.ron` (the French battle strings in decomp `.string` syntax)
   - `battle/gfx/<name>.png` + `.ron`: the battle interface (text box and its tilemap, the two
     healthboxes, HP and EXP bars, status icons, misc, the ball)
   - `battle/env/<Env>.png` + `.ron`: the 10 battle backgrounds (tiles, tilemap, palettes)
   - `pokemon/<SPECIES>/front.png`, `back.png` (64x64, own palette), `shiny.pal`, and
     `pokemon/coords.ron` (pic placement)
   - `trainers/front/<TRAINER_PIC>.png`, `trainers/back/<TRAINER_BACK_PIC>.png` (frames side by
     side), `trainers/pics.ron` (placement, back pic frame counts)
   - `starter/`: the starter screen's bag, ball and circle graphics (`.png` + `.ron`) and the
     grass tilemap (`grass.ron`)

   An extract in an older format still builds, as the stub world with a build warning
   (`content::STALE_EXTRACT`), so that `cargo xtask extract` can run to replace it; `cargo
   xtask parity`, `cargo xtask shot` and the route tests refuse it until you extract again.

The output is deterministic: two runs on the same ROM give identical files. The decomp
identifier tables live in `crates/extract/src/names/generated.rs`, checked against the decomp by
`tests/names.rs` (`PKR_BLESS=1 cargo test -p extract --test names` regenerates them).

## Explorer
`cargo xtask explore <command>` runs the headless explorer (`crates/explore`, release; put
`--debug` first for a debug build). It plays the native game from a new game, grows a tree of
input-only nodes (macro-actions such as walk to a cell, enter a warp, cross a connection, talk,
inspect, fight in tall grass) in a SQLite db, scores them by novelty and progress along the
slice's goals, and records bugs with a trace that reproduces them from a new game: game panics,
softlocks (trapped on a map with exits), checkpoint round-trip breaks, a field that is idle but
never capturable, and planner/engine disagreements. Results depend only on the seed, the config
and the build: neither the thread count nor stopping and resuming changes the db
(`explore digest`).

- `run [--generations N | --secs N | --nodes N | --frames N] [--threads N] [--seed S]`: explores
  (default 10 generations), one line per 10 generations (nodes, nodes/s, frames/s, goals, maps,
  cells, findings). `cargo xtask explore run --secs 600` is a ten-minute run; a db resumes where
  it stopped, with its own seed. On a new build every node is first replayed (and re-expanded
  or marked broken where the game changed).
  - `--cache N` sets how many snapshots stay in memory (default 65536; 16384 uses less memory
    and runs about 10% slower).
  - Each goal this run reaches first prints a line `goal i/n (name) first reached in generation
    G at T s` (G counts the db's generations from 1, T is this run's wall time).
- `status`, `findings [--all]`, `show <id>`: the db's totals and the findings (kind, map, cell,
  hits, message; `show` adds the Rust location, macro, script and log).
- `repro <id>`: replays the finding from a new game; exits 0 iff it reproduces.
- `export <id> [--before N] [--out <path>]`: the finding as a NewGame trace (RON), ending N
  frames before it, in `target/explore/<flavor>/repro/` by default.
- `coverage <map>`: an ASCII grid of the cells stood on (map id or name, e.g. `LittlerootTown`).
  Its last line ends with `N of the M cells reachable from them (P%)`: M counts the walkable
  cells (terrain only, no objects) that a walk from a cell stood on can reach, N those stood on.
- `revalidate`, `digest`, `reset --yes`.

Every command takes `--db <path>` (default `target/explore/emerald/explore.db`, or
`target/explore/stub/explore.db` in stub mode, both git-ignored) and `--config <path>`, a RON
file with the fields of `explore::config::Config` to change (e.g. `(key: (cell_shift: 1))`).
`PKR_EXTRACTED_DIR=/nonexistent cargo xtask explore run --generations 50` explores the stub world.
The db holds ROM-derived states: never commit it or the exported traces.

## Layout
- `crates/engine`: game-agnostic `no_std` core
- `crates/content`: the game (data and scripts)
- `crates/game-gba`: GBA shell (separate workspace, thumbv4t target)
- `crates/game-native`: host shell (software renderer)
- `crates/emu`: embedded mGBA
- `crates/explore`: the headless explorer
- `crates/extract`: reads the ROM into `content/extracted/`
- `xtask`: build and verification tasks

Your ROM goes in `rom/` (git-ignored). Original assets are never committed.
