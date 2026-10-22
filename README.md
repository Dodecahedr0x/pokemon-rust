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
  writes the committed tree. `cargo xtask checkpoints`, `cargo test` and parity only see the
  committed tree.

`cargo xtask checkpoints [--root <dir>] <command>`:
- `list`: the tree (id, frame, map, parent, label, last status)
- `verify [--accept]`: replays every trace on the current build. A node is Valid when it reaches
  the same state, Stale after a snapshot schema change, Diverged when this build reaches another
  state (with the first frame whose hash differs), Broken on a replay error or on a different
  state or hashes on the node's own build (nondeterminism). Fails unless every node is Valid;
  `--accept` re-baselines Diverged and Stale nodes, like the golden hashes, and still fails on
  Broken ones. A Valid node recorded on another build shows "valid (rehash)": `--accept` also
  refreshes its build, world hash and trace hashes, so the nondeterminism check covers the
  current build.
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
most 80 scanlines (`hash_lines_max`, 73 measured) and the whole loop body at most 226 of the 228
lines (`loop_lines_max`, 223 measured). Seeding (`cargo xtask checkpoints seed`, run by hand
after an intended route or behaviour change, never by the tests) records the parity route and
writes its named points as `route:<name>` nodes of the current flavor's tree, replacing earlier
`route:` nodes. The committed checkpoint trees (`checkpoints/*/nodes`) hold inputs and hashes
only, never snapshots or other ROM-derived data, and verify `Valid` in `cargo test` like the
golden hashes: an intended behaviour change needs `cargo xtask checkpoints verify --accept`.

`cargo xtask rom` prints the ROM size and the ELF's IWRAM and EWRAM statics
(`xtask/src/budget.rs`) and fails above the Milestone 5 baseline (ROM 1992712 bytes with
extracted data, IWRAM 596, EWRAM 460) plus 64 KiB of ROM, 2 KiB of IWRAM or 16 KiB of EWRAM; a
stub-content ROM skips the ROM gate. The build remaps the checkout, `CARGO_HOME` and sysroot
paths (`--remap-path-prefix`), so the ROM does not depend on where the repository lives.

`crates/engine/src/world/tests.rs` pins two World hashes on a synthetic game (`golden_hashes`):
`0x54c3f87e89e27c7e` after `World::new` and `0xb94e51c5d0228cf7` after 25 frames each of Right,
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
   summary (maps, tilesets, sprites, skipped graphics, script and text files, the codegen
   statistics, any undecoded script instructions).
3. See the output in `content/extracted/` (format v4, described in `crates/extract/src/schema.rs`).
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
   - `scripts/<Map>.rs` + `scripts/index.ron`: the map's event scripts as generated async Rust
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

## Layout
- `crates/engine`: game-agnostic `no_std` core
- `crates/content`: the game (data and scripts)
- `crates/game-gba`: GBA shell (separate workspace, thumbv4t target)
- `crates/game-native`: host shell (software renderer)
- `crates/emu`: embedded mGBA
- `crates/extract`: reads the ROM into `content/extracted/`
- `xtask`: build and verification tasks

Your ROM goes in `rom/` (git-ignored). Original assets are never committed.
