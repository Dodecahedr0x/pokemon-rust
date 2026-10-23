# pokemon-rust

A Rust GBA engine and toolchain for a full rework of Pokémon Émeraude (FR).
Design: `docs/plans/2026-10-21-emerald-rust-rework-design.md`.

## Requirements
- Rust nightly with `rust-src`
- `cargo install agb-gbafix --version 0.25.0 --locked`
- cmake and libclang (Xcode command-line tools) to build the vendored libmgba

## Commands
- `cargo test --workspace`: unit tests plus ROM-backed tests that need both ROMs and the decomp (see Tests)
- `cargo xtask rom`: build `target/game.gba` and check its memory budgets (see Tests)
- `cargo xtask parity [--long]`: verify the committed checkpoint tree, then play the scripted
  route and every Act 1 node on the ROM (in mGBA) and natively; the World hash must match on
  every frame and the picture at every picture frame. `--long` also replays the whole Act 1
  trace on the ROM and tours every map (see Tests)
- `cargo xtask checkpoints <command>`: list, verify, seed and promote checkpoint nodes (see Checkpoints)
- `cargo xtask shot`: play the scripted route and write ROM and native PNGs of each named moment to
  `target/shots/<moment>-{rom,native}.png`
- `cargo xtask extract [--rom <path>]`: export the ROM to `content/extracted/` (see Extract)
- `cargo xtask studio [--check | --smoke [secs] | map <args>]`: the studio desktop app (see Studio)
- `cargo xtask explore <command>`: the headless explorer (see Explorer)
- `cargo xtask sweep [--studio | --repro]`: enter and idle every map natively (see Tests)
- `cargo xtask silice <import|check|tombstone|move>`: the Silice importer and data checker (see Silice)
- `cargo xtask ledger [--db <path>] [--sweep <path>] [--out <dir>]`: the Emerald feature-parity
  ledger (see Parity ledger)

Every xtask command takes `--game emerald|silice` (default: the inherited `PKR_GAME`, else
Emerald).

## Play
What plays: the whole game's content is imported (518 maps, every script lowered to Rust), and
**Act 1 plays end to end**, natively and on the GBA: from the new game below to Roxanne's badge
(`FLAG_BADGE01_GET`), through Petalburg (Wally's tutorial), Route 104, Petalburg Woods and
Rustboro's gym. The committed `spine:act1:*` checkpoints prove it (see Checkpoints), and the ROM
replays that route identically to native up to frame 99,938 of 104,879 (Roxanne's battle start
is one late frame on the GBA; see Tests). Beyond Act 1 every map loads and idles without a
panic, but features the later story needs (menus, field moves, double battles, …) are missing.

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
Birch's rescue (`crates/content/src/new_game.rs` lists the story vars and flags; the real intro
needs the menus of Milestone 9). Go north to Route 101: Birch is chased by a Zigzagoon, his bag
gives the starter screen (Left/Right to pick a ball, A to choose, then yes/no) and the first
battle follows. Then his lab, May's battle on Route 103, the Pokédex, Petalburg, and on to
Rustboro. Doors, ladders, step-on warps, connections, NPCs (spawned
in a view window as in Gen3), signs, trainer sight, items and money (no bag screen), multichoice
menus and French dialogue work.

Battles: the D-pad moves the cursor and A confirms in the action menu (ATTAQUE, SAC, POKéMON,
FUITE) and the move menu (B goes back); A or B advances messages; when a move is to be learned,
the yes/no box and the move list work as in Gen3 (B in the list gives up learning). Losing
whites out to the last Pokémon Center (or home), with the party healed.

Known simplifications: singles only (a double trainer fights singles against the same party);
no bag, party screen or Poké Balls (SAC and POKéMON return to the menu, and the next usable
Pokémon is sent out by itself); no Mart, PC, start menu or in-game save (the shells save
checkpoints); evolution happens silently after a battle (no scene); Wally's tutorial battle is
skipped; one battle background (grass or plain); move animations are a lunge, the hit blink and
the healthbox shake; about a third of the move effects and half the abilities (including all of the
Act 1 route); no sound, running shoes, field moves or weather; parties and encounter rolls are drawn
when the battle is requested, before its transition (Gen3 draws some after it). Every script
command and special is classified: one the engine lacks raises a `Report` event and continues
with a documented default (the build's `content::REPORT` lists them).

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
- `promote <local id> --label <text>`: moves a local node (for example a spine checkpoint
  exported with `explore spine --export`) into the committed tree, after replaying it from
  its parent on this build; the only way into the committed tree besides `seed`
- `label <id> <text>`, `rm <id>` (refused for a node with children)
- `sav <id> [-o <path>]`: writes the node's flash image to `target/game.sav`, which mGBA loads
  with `target/game.gba`: the ROM boots at the node (hold B at boot to ignore a save)

The Emerald tree holds the four `route:*` nodes (c00001–c00004) and the Act 1 chain, the
explorer spine's checkpoints up to badge 1 (`explore spine --export --label badge1`, then
promoted parents first; no hand-authored segment): `spine:act1:lab` (c00005, frame 4,768),
`oldale_center` (c00006), `may` (c00007), `pokedex` (c00008), `petalburg` (c00009), `woods`
(c00010), `rustboro` (c00011) and `spine:act1:badge1` (c00012, frame 104,879,
`FLAG_BADGE01_GET`). `cargo run -p game-native --features window --bin play -- --load
spine:act1:rustboro` starts at Rustboro.

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
- **Map** and **Data**: the map view with its overlays (a picker lists the 518 maps by map
  group, filtered by name or `MAP_` constant), and the species, moves, trainers, encounters and
  texts tables. `cargo xtask studio map <command>` is the headless map editor for authored
  games (`list`, `create`, `apply`, `build`, `validate`, `render`, …; see
  `crates/studio/src/map_edit/cli.rs`). The map's "coverage" layer shades each cell the
  explorer stood on by its hits (1, 2-9, 10-99, 100+, as `explore coverage` prints them),
  read from the explorer db for the shown map, and again every couple of seconds while the db
  changes (a run from the studio or from `cargo xtask explore branch`).
- **Explorer**: starts and stops `explore spine` or `explore branch` (`cargo run -p explore
  --release`, logging to `target/studio/explore.log`) with an optional branch budget (frames),
  job count and `--from` checkpoint (offered from the spine's labels). The run keeps going when
  you leave the tab; quitting the studio (closing the window or Cmd+Q) stops it, and any export
  in flight. The state line says "building explore…" while cargo compiles, and a failed run (a
  build error, a panic) shows the log's tail. Every 2 s the tab reads
  `target/explore/<flavor>/explore.db` read-only (also a db written by `cargo xtask explore`):
  the spine (steps played, frames, where it stopped), its checkpoints, the branches by outcome,
  branches/s (averaged over the last 5 polls), what the branches found beyond the spine (maps,
  scripts, flags, var values, cells) and the findings. The Spine table lists each checkpoint
  (step, frame, map, cell); the Branches table lists up to 100 branches, those needing a look
  first (bugs, unexpected divergences, stranded), with origin, kind, outcome, frames (explore +
  recover + window), what each found and its message. The findings table (open first, then by
  hits) gives kind, map, cell, message, hits, status, and "found on another build" when the
  finding's build is not this one. Per finding: "Open at frame before" (120 by default) replays
  it into the native pane, paused that many frames before its last frame, and shows the Play
  tab (the inspector's "last" repeats the other-build warning) (F5 from there saves a local
  node with a NewGame parent); "Export repro" runs `explore export <id>` (a NewGame trace under
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
bulk steps, `Recording::bulk_events`; about warps + 2 × battles + starters + teleports; 8 late
applies for 22 bulk events on the extracted route).

Checkpoints are held to one invariant: a capturable World, encoded, decoded and restored, has
the same hash, picture and future as the World that was never interrupted.
`crates/game-native/tests/checkpoint_route.rs` (plain `cargo test`) checks it natively at about
100 capturable frames of the route plus each named moment: the postcard checkpoint fits the
flash budget (`MAX_ENCODED`, 24,016 bytes: the 64 KiB one-slot capacity minus 512; the route's
largest is about 800 bytes, and `engine::checkpoint` `worst_case_save_fits_max_encoded` pins the
worst-case save with 7 full PC boxes under it) and round-trips, as
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
Oldale's Pokémon Center). The ROM's costs are gated on every lane: `World::hash` at
most 65 scanlines (`hash_lines_max`; 46 on the route, 56 at worst on Act 1) and the whole loop
body at most 217 of the 228 lines (`loop_lines_max`; 166 on the route, 176 at worst on Act 1),
and the debug region's memory marks: heap high-water at most 160 KiB (`emu::HEAP_MAX`; about
48 KiB measured), at least 1 KiB of IWRAM stack never touched, no dropped events and no panic.
The hash is buffered 256 bytes at a time and folded by
ARM code in IWRAM (`engine`'s `iwram` feature, enabled by `game-gba`), with the same bytes as the
plain bytewise FNV-1a. If the line gates trip, re-measure and justify the cost; don't just bump
the gates.

Act 1 (Milestone 8): `cargo xtask parity` also resumes the ROM from every `spine:act1:*` node's
`.sav` for 2,000 frames with identical hashes and a picture every 100 frames
(`rom_resumes_from_act1_nodes`), and runs the native checkpoint invariant at 108 points of the
flattened Act 1 trace (`crates/game-native/tests/act1.rs`, release). `cargo xtask parity --long`
adds `act1_long`, the whole Act 1 trace played on the ROM from a new game (about 6 min), and
the map tour (`emu/tests/tour.rs`: a `game-gba --features tour` ROM warps through all 518 maps,
in lockstep with native, gating the frame budget on the 45 Act 1 maps and one map per tileset
pair). Known failure, held by `KNOWN_LATE` in `parity.rs`: Roxanne's battle start is one late
frame on the ROM (engine frame 99,942; the first turn's AI choice), after which the trace no
longer drives the ROM, so the long lane compares frames 0–99,938 and the `spine:act1:badge1` resume
lane covers the end; the entry fails once the cause is fixed. The tour lists its known
overruns (Sidney's room's whole-view redraw) in `KNOWN_OVER`.

Parity then runs the studio ROM pane's `#[ignore]`d tests on the ROM it built
(`cargo test -p studio --release --lib rom::tests:: -- --ignored`). Seeding
(`cargo xtask checkpoints seed`, run by hand
after an intended route or behaviour change, never by the tests) records the parity route and
writes its named points as `route:<name>` nodes of the current flavor's tree, replacing earlier
`route:` nodes. The committed checkpoint trees (`checkpoints/*/nodes`) hold inputs and hashes
only, never snapshots or other ROM-derived data, and verify `Valid` in `cargo test` like the
golden hashes: an intended behaviour change needs `cargo xtask checkpoints verify --accept`.

`cargo xtask rom` prints the ROM size, the ELF's IWRAM and EWRAM statics and the code and data
by symbol prefix (`xtask/src/budget.rs`). It fails when a full import's ROM exceeds 16 MiB
(about 9.3 MB today), when IWRAM or EWRAM statics grow past the full-import baseline (IWRAM
788, EWRAM 492) by 2 KiB or 16 KiB, or when engine `.text` on the slice extract grows past its
baseline (205448 bytes) by 128 KiB; the slice and stub ROM sizes are reported only, and the
Silice ROM has its own provisional limit. The build remaps the checkout, `CARGO_HOME` and sysroot
paths (`--remap-path-prefix`), so the ROM does not depend on where the repository lives.

`cargo xtask sweep` (`xtask/src/sweep.rs`, release, native) starts a new game on every map of
the content build (the first warp's cell, else the walkable cell nearest the centre) and idles
600 frames (then mashes A for up to 3,000 frames if the map is not capturable): no panic,
then, when the World is capturable, a checkpoint round trip with an equal hash and 60 frames
shadowed, and a render. Maps that end uncapturable are listed with a verdict (a cutscene still
waiting, a facility script looping on unsupported specials); `Report` events are counted per op
and special, and the build report is appended. The report goes to `target/sweep/report.txt`; it
fails on a panic, a round-trip or shadow mismatch, or dropped events. On the full import: 490
of 518 maps pass, 24 are checked on the map a cutscene warped to, 4 are not capturable, 0
panics, 0 round-trip failures. `--studio` runs the studio's `#[ignore]`d whole-game tests (map
pixels, views, data views and the map editor on every map) in release; `--repro` builds `content`
twice with a clean `$OUT_DIR` and fails on any byte of difference. In `cargo test` the studio's
map tests use `content::samples::map_ids()` (the slice plus a few maps per tileset family).

`crates/engine/src/world/tests/goldens.rs` pins World hashes on a synthetic game, one constant
per scenario (`golden_hashes`): `GOLDEN_WALK` (`0xa6063af74c9c3e21` after `World::new` and
`0x2d6a1f1f392d3f76` after 25 frames each of Right, A, nothing and Down), `GOLDEN_WARP`,
`GOLDEN_CROSSING`, `GOLDEN_SCRIPT`, `GOLDEN_WILD_BATTLE`, `GOLDEN_TRAINER`, `GOLDEN_SCREEN`
(START and SELECT on the idle field) and `GOLDEN_SAVE` (a script's save dialog). They change
whenever the hashed state, its order or the frame logic changes; update them only for an
intended change (a change that moves one scenario updates only its constant).

`crates/game-native/tests/m9_route_scan.rs` (`#[ignore]`d; `cargo test -p game-native --release
--test m9_route_scan -- --ignored --nocapture`) prints what Milestone 9 can change on the parity
route and the committed checkpoint tree: START/SELECT on the idle field, reports, the decomp
specials reachable from the scripts started, poisoned or Wurmple-line steps, evolutions due and
trainer wins with two usable mons.

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
   `--maps all|slice|<Dir,Dir,…>` picks the maps (default `all`, the 518 maps; `slice` is the
   15-map slice the fixtures use) and `--previews` also writes the map renders (off by default:
   they are most of the extracted size and the build never reads them).
   Everything is named with decomp identifiers (`LittlerootTown`, `General`, `FLAG_HIDE_…`,
   `OBJ_EVENT_GFX_…`, `SPECIES_…`, `TRAINER_PIC_…`); ROM offsets only appear in the manifest:
   - `manifest.ron`: format, game code, language, ROM sha1, the exported maps, tilesets and
     sprites, the ROM offset of everything read (`provenance`), sha1 of every other file
   - `maps/<Map>.ron`: header, layout (tileset names, blocks one map row per line), objects,
     warps, coord and bg events, connections and map scripts. Script references use decomp
     labels (`LittlerootTown_EventScript_Twin`), transferred from the decomp's `.inc` sources
     onto the FR bytecode (`PKR_DECOMP`; without it, positional names such as
     `LittlerootTown_Obj1` and a warning). Every map by default (518; `--maps slice` gives
     Littleroot Town, Routes 101–103, Oldale Town, and the Littleroot and Oldale interiors)
   - `layouts/<Layout>.ron`: every layout (418), including those only `setmaplayoutindex` uses
   - `tilesets/<Name>.png` + `.ron`: tiles (with the tileset's first palette), palettes,
     metatiles and attributes
   - `sprites/<Gfx>.png` + `.ron`: overworld sprites the maps use (also through
     `VAR_OBJ_GFX_ID_*`), plus Brendan and May; `sprites/field_effects/`: the emote bubbles
   - `font/latin_normal.png` + `.ron`, `font/latin_small.png` + `.ron`: the Latin fonts and
     their glyph widths
   - `window/message_box.png` + `.ron`: the dialogue window frame, its palette and text colors
   - `previews/<Map>.png`: map renders (with `--previews`)
   - `text/<lang>.ron`: every text the scripts use, keyed by its decomp label, re-encoding to
     its exact ROM bytes
   - `scripts/<Module>.ron` + `scripts/index.ron`: the script IR, a decoded listing of every
     reachable event script per decomp source file (a map, or `Common_<stem>`), and the name
     index. The content build lowers it to async Rust in `$OUT_DIR` (see below)
   - `battle/*.ron`: battle data. `species.ron` (stats, types, abilities, learnsets, FR names),
     `moves.ron`, `types.ron` (type chart, type and ability names), `trainers.ron` (every
     trainer and party), `encounters.ron` (every wild header of the exported maps: land,
     water, rock smash, fishing), `evolutions.ron`, `trainer_money.ron`, `heal_locations.ron`,
     `strings.ron` (the French battle strings in decomp `.string` syntax)
   - `battle/gfx/<name>.png` + `.ron`: the battle interface (text box and its tilemap, the two
     healthboxes, HP and EXP bars, status icons, misc, the ball)
   - `battle/env/<Env>.png` + `.ron`: the 10 battle backgrounds (tiles, tilemap, palettes)
   - `pokemon/<SPECIES>/front.png`, `back.png` (64x64, own palette), `shiny.pal`, and
     `pokemon/coords.ron` (pic placement)
   - `trainers/front/<TRAINER_PIC>.png`, `trainers/back/<TRAINER_BACK_PIC>.png` (frames side by
     side), `trainers/pics.ron` (placement, back pic frame counts)
   - `starter/`: the starter screen's bag, ball and circle graphics (`.png` + `.ron`) and the
     grass tilemap (`grass.ron`)
   - `tables/`: `items.ron`, `multichoice.ron`, `std_strings.ron`, `map_sections.ron` and
     `field_strings.ron` (`gText_BigGuy`/`BigGirl`/`Son`/`Daughter`/`Mom`/`Dad`, which the
     scripts buffer as std strings from `FIELD_STRING_BASE`; the French ROM's BigGuy and
     BigGirl are empty)

   An extract in an older format still builds, as the stub world with a build warning
   (`content::STALE_EXTRACT`), so that `cargo xtask extract` can run to replace it; `cargo
   xtask parity`, `cargo xtask shot` and the route tests refuse it until you extract again.

The content build consumes every exported map: metatile behaviours are classified by
`content/build/classify/field.rs` (the directional `MB_IMPASSABLE_*` become
`Behavior::Impassable(sides)`, walls that block crossing those sides of the cell, in collision
and trainer sight), and the scripts are lowered from the IR (`content::REPORT` lists what is
still unsupported; `cargo xtask sweep` prints it). Scripts read the current map through
`Ctx::current_map` (the TV special in Brendan's house).

The output is deterministic: two runs on the same ROM give identical files. The decomp
identifier tables live in `crates/extract/src/names/generated.rs`, checked against the decomp by
`tests/names.rs` (`PKR_BLESS=1 cargo test -p extract --test names` regenerates them).

## Explorer
`cargo xtask [--game silice] explore <command>` runs the headless explorer (`crates/explore`,
release; put `--debug` first for a debug build). It tests the native game along **one authored
main path per flavour, the spine** (`crates/explore/src/spine/`: Emerald from a new game to
Roxanne's badge, Silice's Chapter 0, the stub's route), then **branches** off it toward what the
spine never touched (warps, crossings, NPCs, signs, coord events, unvisited blocks, wild battles
fought or run from, the other answer of a yes/no or another starter). A branch explores until
its frame budget or its step cap runs out (or nothing new is left), walks back to the spine, and
plays the spine's next steps: it **rejoins**, **diverges** (expected when the branch itself
changed the story state; otherwise a `Divergence` finding), is **stranded** (no way back; a
softlock probe runs), is **aborted** (too little novelty for its frames once it chained past its
target: no recovery), or ends on a **bug**. Findings: game panics, softlocks,
checkpoint round-trip breaks, a field that is idle but never capturable, planner/engine
disagreements, divergences, and report-only unsupported script commands, each with a trace that
reproduces it from a new game. Results depend only on the config and the build: the job count
does not change the db (`explore digest`). See `docs/plans/2026-10-23-explorer-spine.md`.

- `spine [--trace] [--export]`: plays the spine (Emerald: 78 steps, 109,851 frames, about 0.1 s
  in release) and prints its checkpoints and its coverage; `--trace` prints every step's end
  (map, cell, party, vars and flags changed). A failing step stops it with the step, the map
  and cell, the session state and the macros played. `--export` writes the checkpoints to the
  local checkpoint store as `spine:<act>:<label>`, chained from a new game (`--label L`: the
  chain up to L; checkpoints past a bypass cannot export). Promote them parents first with
  `cargo xtask checkpoints promote <id> --label spine:act1:<label>`; a label already committed
  with the same state is kept, with another state the export fails. The checkpoints shared
  with committed `<act>:<label>` nodes are compared (same map and goal progress).
- `branch [--from L] [--budget F] [--jobs N] [--every N] [--min-novelty N] [--max-steps N]
  [--limit N]`: plays the spine, then the branches from its checkpoints and every N-th step
  (`--from`: one checkpoint's segment), on N workers; prints the outcomes, the frames and the
  new things found per minute.
- `branches [--outcome O]`, `status`, `findings [--all] [--bugs]`, `show <id>`: the db's
  branches, totals and findings (kind, map, cell, hits, message; `show` adds the Rust location,
  macro, script and log).
- `repro <id>`: replays the finding from a new game (a divergence: the spine and its branch
  again); exits 0 iff it reproduces.
- `export <id> [--before N] [--out <path>]`: the finding as a NewGame trace (RON), ending N
  frames before it, in `target/explore/<flavor>/repro/` by default.
- `coverage <map>`: an ASCII grid of the cells stood on (map id or name, e.g. `LittlerootTown`):
  `s` by the spine, `1`..`4` by branches only. Its last line ends with `N of the M cells
  reachable from them (P%)`.
- `digest`, `reset --yes`. A db of the evolutionary explorer (schema 3 and older) is refused:
  `explore reset --yes`.

Every command takes `--db <path>` (default `target/explore/<flavor>/explore.db`, git-ignored)
and `--config <path>`, a RON file with the fields of `explore::config::Config` to change (e.g.
`(branch: (budget_frames: 20000))`). `PKR_EXTRACTED_DIR=/nonexistent cargo xtask explore branch`
explores the stub world. The db holds ROM-derived states: never commit it or the exported
traces.

## Parity ledger
`cargo xtask ledger` (`crates/parity`, host-only; `docs/plans/2026-10-23-emerald-parity.md`
Task L) lists every Emerald feature the decomp defines (`PKR_DECOMP`) and what our engine does
with it. Categories: script commands, specials, `MB_*` behaviours, `EFFECT_*`, `ABILITY_*`,
`HOLD_EFFECT_*`, item field and battle use functions, field moves, weathers, `BATTLE_TYPE_*`,
map types, the Gen3 screens (a hand-kept list, `parity::owners::SCREENS`) and the engine's own
`Report` names. For each entry:
- **status** (native, degraded, ignored, unsupported, missing, unknown), taken from the code
  that dispatches: the support tables (`content/build/support/*.ron`) and the `Degraded` arms of
  `prelude::special`, the build's classifiers (`content/build/classify/`),
  `prelude::BATTLE_TYPES_{HANDLED,DEGRADED}`, `content::data::ITEM_USE`, and whether a screen
  is still its Wave 0 stub (`xtask/src/ledger.rs`);
- **uses**: decomp sites and the maps whose scripts (through calls and std scripts), layouts,
  trainers, land wild mons, items and objects use it; the content build's IR sites, reported and
  stale `specialvar` sites (`content::PARITY`, written by `content/build/parity.rs`);
- **story**: the first act (Acts 1–8, the League; `parity::story::ACTS`) whose maps use it, and
  story-blocking (`parity::story::BLOCKING`, or a spine `Bypass` step);
- **reach**: reported by the explorer, or a map the spine or a branch stood on
  (`target/explore/emerald/explore.db`), and the sweep's report counts
  (`target/sweep/report.txt`), when they exist;
- **owner**: the M9 task, M10, M11, M12, M13 or none (`parity::owners`).

Gaps are ranked story-blocking > on a story map (earlier act first) > reached > site count. It
writes `target/parity/ledger.md` (coverage per category, gaps per act, the top gaps M9/M10 do
not own, gaps by owner, then one table per category and the issues) and `ledger.ron`, and
prints the coverage and the top 30. It needs the Emerald content build; run
`cargo xtask explore reset --yes && cargo xtask explore branch` (and `cargo xtask sweep`)
first for the reach columns. `cargo test -p parity` pins one entry per category on a synthetic
decomp (`crates/parity/tests/fixture.rs`) and checks the story maps and screen openers against
the real decomp.

## Silice
Pokémon Silice is a second game on the same engine: authored French content
(`docs/plans/2026-10-23-silice-content.md`) in `games/silice/` (maps, layouts, texts, ids,
`game.ron`) and `crates/content/src/silice/` (scripts), built over the extract's base assets
(tilesets, sprites, battle data). Build it with `PKR_GAME=silice` (needs `content/extracted/`),
or pass `--game silice` to any xtask command, which builds in `target/silice` so the Emerald
build is kept:

- `cargo xtask --game silice rom`, `parity`, `explore spine`, `explore branch`, `studio`: as for
  Emerald, on Silice (its parity lane is a new game left idle; it has no committed nodes yet).
- `PKR_GAME=silice cargo run -p game-native --features window --bin play`: play it natively.
- `cargo xtask silice check`: validates `games/silice` (ids, flags and vars, warp and
  connection targets, texts, scripts) against the extract and the decomp; `cargo xtask studio
  map …` edits its maps.

State: the maps of Chapters 0–9 and the scripts of Chapters 0–2 are in; the later chapters'
scripts and mechanics need Milestones 9–11. Known failure: since the Chapter 0 scripts, a
script holds the Silice new game's start, so the Silice-flavour `game-native`, `emu` and `xtask`
tests that need a capturable start fail (being fixed separately).

## Layout
- `crates/engine`: game-agnostic `no_std` core
- `crates/content`: the game (data and scripts): Emerald from the extract, or Silice
- `crates/studio`: the desktop studio and the headless map editor
- `games/silice`: Silice's authored data
- `crates/game-gba`: GBA shell (separate workspace, thumbv4t target)
- `crates/game-native`: host shell (software renderer)
- `crates/emu`: embedded mGBA
- `crates/explore`: the headless explorer
- `crates/parity`: the Emerald feature-parity ledger (host-only)
- `crates/extract`: reads the ROM into `content/extracted/`
- `xtask`: build and verification tasks

Your ROM goes in `rom/` (git-ignored). Original assets are never committed.
