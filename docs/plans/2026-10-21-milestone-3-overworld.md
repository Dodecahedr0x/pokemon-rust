# Milestone 3 — Overworld Slice Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** Play the start of the game in the new engine, on both the GBA build and the native build, with pixel parity. The player walks around Littleroot Town, Route 101 and Oldale Town, enters the buildings, reaches Routes 102 and 103, and talks to NPCs. NPCs wander. Event scripts, extracted from the French ROM and turned into async Rust, run their dialogue (French text box) and movements.

**Architecture:**
- **Extraction:** the French ROM is extracted (schema v2, stable names) into `content/extracted/`, which is git-ignored. `content/build.rs` turns it into `'static` tables and includes the generated scripts.
- **Engine:** split into independent modules: field/camera, objects, text box, flow state (vars, fades, triggers), video and scripts. The integration task composes them into `World` and implements `ScriptHost`.
- **Shells:** both apply the same `video::Frame`. GBA applies it at vblank; native uses a software PPU with GBA mode-0 rules.

**Tech stack:** Rust 2024, `no_std` + `alloc` engine, agb 0.25 (GBA), mGBA FFI (parity), serde/ron/png (build time).

**Wave 0, already committed (contracts — read them, do not redesign them):**
- `crates/engine/src/data.rs`: `GameData` trait, `MapData`, `Tileset`, events, `Behavior`, `MovementType`, `SpriteData`, `Font`, `WindowGfx`, `NewGame`, the id newtypes, and block/attribute bit helpers.
- `crates/engine/src/video.rs`: `Frame`, `Cmd`, `Obj`, `BgControl`, and the fixed VRAM `layout`.
- `crates/engine/src/script.rs`: `Ctx` (async primitives), `Request`/`Response`, `Step`, `Runner`, `ScriptHost`.
- `crates/engine/src/grid.rs`: `block_at`/`neighbor` across connections and the border.
- `crates/extract/src/schema.rs`: the v2 on-disk format and the **naming rules** for maps, tilesets, scripts, texts and movements.

A task may add to a contract if it needs to. It may not change the meaning of what is already there, and every addition must be listed in its final report.

---

## Rules for every task

- **Workspace:** work in your own git worktree. Commit there with `feat(<crate>): …` or `fix(…)`, and **no attribution or co-author lines**.
- **TDD:** write a failing test first. Engine modules use synthetic data in unit tests. ROM-backed tests use `extract::fixtures` (both ROMs where there is a decomp oracle).
- **Determinism:** no floats, clocks, `HashMap` iteration or other host-dependent behaviour in `engine` and `content`. Randomness comes only from the `World` RNG, passed in explicitly.
- **`engine` stays game-agnostic.** No Hoenn names and no `MB_*`/`FLAG_*` numbers. Gen3 *formats* (block bits, text encoding, OBJ sizes) are fine.
- **Never commit ROM-derived data.** `content/extracted/` stays git-ignored. Decomp identifiers (constant names) are fine.
- **Before committing:**
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`, with env `PKR_ROM_FR=/Users/dode/Documents/pokemon/pokemon-rust/rom/emeraude-fr.gba PKR_ROM_US=/Users/dode/Documents/pokemon/emerald.gba PKR_DECOMP=/Users/dode/Documents/pokemon/pokeemerald`
  - `cargo xtask rom`, if you touched anything the GBA build compiles (engine, content, game-gba). The GBA build must keep compiling.
- **Ownership:** only touch the files your task owns, plus new files in your module. If you need a change elsewhere, describe it in your report instead of making it.
- **Decomp:** the decomp at `PKR_DECOMP` is the reference for Gen3 behaviour. Mirror it where the task says "as Gen3". The parity contract is only between our GBA build and our native build, not with the original game.

---

## Wave 1 (parallel)

### Task A — extract v2: schema, names, more maps, window graphics, French charmap

**Owns:**
- `crates/extract/src/{export.rs, names/*, maps.rs, sprites.rs, font.rs, gfx.rs, rom.rs, text.rs, text/*, window.rs (new)}`
- the matching `tests/*.rs`
- `xtask/src/main.rs` (extract summary only)
- `README.md` (Extract section)

Do not touch `scripts.rs` (Task B) or `schema.rs`, except for additions.

1. **`names` module**: committed tables of decomp identifiers, each guarded by a drift test that parses the decomp source (as `tests/text.rs` does for the charmap). Each table supports lookup by name and by number. Tables:
   - `MAPS`: (group, num) → dir name (`LittlerootTown`) and `MAP_*` constant (from `data/maps/map_groups.json` plus each `map.json`).
   - `FLAGS`, `VARS`: from `include/constants/flags.h` and `vars.h`, including the special vars 0x8000+ such as `VAR_RESULT` and `VAR_FACING`.
   - `OBJ_EVENT_GFX`: from `include/constants/event_objects.h`.
   - `MOVEMENT_TYPES`: `MOVEMENT_TYPE_*`.
   - `TRAINER_TYPES`, `ITEMS`.
   - `METATILE_BEHAVIORS`: `MB_*`.
   - `SPECIALS`: index → name, from `data/specials.inc`.
   - `STD_SCRIPTS`: `callstd` index → name, from `data/script_cmd_table.inc` / `gStdScripts`.
   - `MSGBOX_TYPES`.
   - `MOVEMENT_ACTIONS`: byte → `MOVEMENT_ACTION_*`.
   - `TILESETS`: header index → name, in `src/data/tilesets/headers.h` order.
   - `SONGS`: optional.

   Expose `names::flag_name(u16) -> String`, which falls back to `FLAG_0x123`, and similar helpers for the other tables.
2. **Tileset names:** the `Tileset` header structs are contiguous in the ROM, in `headers.h` order, starting with `gTileset_General`. Locate the array from the General struct M2 already finds, and name each tileset by its index.
3. **Export to schema v2**, replacing the M2 formats; the files are described in `schema.rs`:
   - `manifest.ron` with provenance hex offsets, and nothing duplicated from other files.
   - `maps/<Map>.ron`, `tilesets/<Name>.{ron,png}`, `sprites/<Gfx>.{ron,png}`, `font/…`, `window/…`, `previews/<Map>.png`.
   - Script references in maps follow the naming rules. Implement `pub fn named_roots(rom, &MapHeader, map_name) -> Result<Vec<(String, u32)>>` in a new `crates/extract/src/roots.rs`; Task B reuses it for the canonical name ↔ offset mapping. Canonical names across maps use the export order.
   - Do not write texts or scripts; Task B will. Keep `text/<lang>.ron` out of export for now, and leave a clearly marked hook (`fn export_scripts`) that Task B fills in.
4. **Maps:** export these 15 maps, in this order:
   - LittlerootTown, Route101, OldaleTown, Route102, Route103
   - LittlerootTown_BrendansHouse_1F, _2F, LittlerootTown_MaysHouse_1F, _2F, LittlerootTown_ProfessorBirchsLab
   - OldaleTown_House1, OldaleTown_House2, OldaleTown_PokemonCenter_1F, _2F, OldaleTown_Mart

   Also export every tileset they use. Export the sprites they reference, plus `BRENDAN_NORMAL`, `BRENDAN_RUNNING` (if separate), `MAY_NORMAL`, and every sprite reachable through `VAR` graphics in these maps' scripts (`setvar VAR_OBJ_GFX_ID_*`). If that last part is hard, export all `OBJ_EVENT_GFX` ids below 100.

   Connections and warps to maps that are not exported keep `map: None` plus the raw id.
5. **Window graphics:** `window/message_box.{png,ron}`. These are the Gen3 dialogue frame tiles (`gMessageBox_Gfx`, `graphics/text_window/message_box.png`) and the palette the field dialogue window uses (see `LoadMessageBoxAndBorderGfx`, `GetOverworldTextboxPalettePtr` and `src/text_window.c`).
   - Fill in `text_fg`, `text_shadow` and `text_bg` with the Gen3 dialogue colors (`TEXT_COLOR_DARK_GRAY` on white with a light gray shadow).
   - Anchor on the decomp PNG bytes, as M2 does, and test on both ROMs.
6. **French charmap overlay:** BPEF redraws 0xB1/0xB2 as « ». Decode and encode them as `«`/`»` for BPEF. Check the other redrawn glyphs listed in `tests/font.rs` and map any that are new characters. Keep the US behaviour unchanged.
   - The codec becomes language-aware: `text::decode_lang` and `encode_lang`, or a `Charmap` value.
   - Add a test that every exported FR text re-encodes to its exact ROM bytes. This part moves to Task B once B writes the texts, so leave an `#[ignore]`-free helper for B to call.
7. **Cleanups** from the M2 findings:
   - `Rom::u16s(off, n)` and `Rom::req_ptr`
   - shared `TILE_BYTES` and 512 constants
   - `gfx::rgb555_to_rgba`
   - remove the unused `read_latin_normal` and unneeded serde derives
   - write `tiles.png` with the first used palette
8. **Determinism, round-trip and US smoke tests**, updated to v2.

   **Acceptance:** `cargo xtask extract` writes v2 for the 15 maps. A loader round-trip test passes: every map and tileset `.ron` parses back into the schema types. `git status` stays clean.

### Task C — video: native software PPU and GBA frame applier

**Owns:**
- `crates/game-native/src/*`
- `crates/game-gba/src/*`
- `crates/engine/src/{world.rs, scene.rs}` (M1 bridge only)
- `crates/content/src/lib.rs` (M1 scene only)
- `crates/emu/tests/parity.rs`
- `xtask` shot/parity

1. **`game_native::Ppu`:**
   - It holds 96 KiB of VRAM: BG VRAM 64 KiB, OBJ VRAM 32 KiB, 1D mapping.
   - It holds 256 BG and 256 OBJ palette colors and 4 `BgControl`s.
   - `apply(&mut self, &Frame)` applies `cmds` in order, honouring the `Cmd` docs.
   - `render(&self, &Frame, fb: &mut [u16])` draws one 240×160 RGB555 frame from the frame's registers (scroll, objs, brightness).
2. **GBA mode-0 rules**, exactly:
   - 4bpp text BGs, 32×32 entries; entry bits: tile 0–9, hflip 10, vflip 11, palette bank 12–15.
   - Color index 0 is transparent; the backdrop is BG palette color 0.
   - Priority resolution:
     - The lowest priority number is in front.
     - On equal priority, a sprite is in front of a BG.
     - Among BGs of equal priority, the lower layer index is in front.
     - Among sprites, the lower OAM index is in front, among the sprites that are opaque at that pixel. A transparent pixel of a front sprite reveals the next one.
   - OBJ: regular (non-affine) sprites from the `Obj` table; 1D tile mapping; hflip/vflip; 9-bit x and 8-bit y wrap, as on hardware.
   - Brightness: BLDY decrease on every layer, sprites and backdrop: `c - ((c * evy) >> 4)` per 5-bit channel.
   - Unit tests for each rule.
3. **GBA shell:**
   - Keep a `Frame` from the last step.
   - At vblank, apply its cmds to VRAM and palette RAM, using DMA or word copies for bulk tiles, and write the registers: BGxCNT, scroll, a 128-entry OAM shadow with unused entries hidden, BLDCNT/BLDY.
   - Then read keys and step.
   - Keep the M1 display-lag convention and the debug region.
4. **M1 bridge:** make M1's single-layer scene go through `Frame`.
   - World frame 0 uploads palette, tiles and map via cmds.
   - Each step sets BG0 scroll.
   - Both shells render through the new path.
   - The World hash is unchanged (golden tests stay), and `cargo xtask parity` passes, now with every BG pixel coming from the new pipeline.
   - Delete `world::FrameOut`.
5. **Parity harness:** let `crates/emu/tests/parity.rs` compare frames at chosen checkpoints, not only at the end, so Task I can extend it.

   **Acceptance:**
   - Ppu unit tests pass.
   - `cargo xtask parity` passes.
   - The ROM builds.
   - The xtask `shot` output looks unchanged.

### Task D — engine field: map drawing, camera, connections

**Owns:** `crates/engine/src/field.rs`, new.

`Field` holds the current map and camera, and draws metatiles into BG1–BG3 through `Frame` cmds. API, adjustable if needed:
```rust
pub struct Field { /* map, camera_px: (i32, i32), drawn window, block overrides, loaded tilesets */ }
impl Field {
    pub fn new(map: MapId) -> Self;
    pub fn map(&self) -> MapId;
    /// Uploads tilesets (tiles at FIELD_CHAR_BLOCK: primary 0.., secondary 512..), palettes
    /// (primary 0..6, secondary 6..13), BG1–BG3 controls, then draws the whole view.
    pub fn load(&mut self, game: &impl GameData, map: MapId, camera_px: (i32, i32), f: &mut Frame);
    /// Moves the camera (map pixel of the screen's top-left corner); draws metatiles that come
    /// into view and sets BG1–BG3 scroll in `f.scroll`.
    pub fn set_camera(&mut self, game: &impl GameData, camera_px: (i32, i32), f: &mut Frame);
    /// The block at (x, y) including overrides (`setmetatile`) and connections.
    pub fn block(&self, game: &impl GameData, x: i16, y: i16) -> u16;
    pub fn set_block(&mut self, game: &impl GameData, x: i16, y: i16, block: u16, f: &mut Frame);
    /// Seamless switch to a connected map: the caller has moved the player across the edge;
    /// `shift` is the map-cell offset between the old and new coordinate systems. Re-uploads
    /// tilesets/palettes only if they differ.
    pub fn switch_map(&mut self, game: &impl GameData, to: MapId, shift: (i16, i16), f: &mut Frame);
    pub fn redraw(&mut self, game: &impl GameData, f: &mut Frame);
    pub fn hash(&self, h: &mut Fnv64);
}
```
- **Camera convention:** `camera_px = player_cell * 16 + player_pixel_offset - (112, 64)`. When standing still, the player's cell is drawn at screen (112, 64).
- **Ring buffer:** each 32×32 BG holds a 16×16-metatile ring. Metatile (mx, my) goes at entries (2·(mx & 15), 2·(my & 15)). Scroll is `camera_px` mod 256. Every metatile intersecting the 240×160 view must be drawn before it is shown.
- **Layers, as Gen3 `DrawMetatile`:**
  - NORMAL: bottom tiles → BG2 (middle), top tiles → BG1 (top), BG3 gets entry 0x3014.
  - COVERED: bottom → BG3, top → BG2, BG1 gets entry 0.
  - SPLIT: bottom → BG3, top → BG1, BG2 gets entry 0.
- **Blocks outside the map:** use `grid::block_at` (connections, then border). A neighbor's blocks are drawn with the **current** map's tilesets, as Gen3 does.
- **BG controls:** BG1/2/3 enabled with `layout` priorities and screen blocks, char block `FIELD_CHAR_BLOCK`.
- **Tests:** synthetic tilesets and maps covering:
  - layer routing per layer type
  - the ring position and wrap
  - the exact cmds for a 1-metatile camera move: only the new column or row is drawn
  - connection and border drawing
  - `switch_map` coordinates
  - `set_block`

  Optional ROM-backed test: render Littleroot through a `game-native` `Ppu` and compare the visible region with `extract::render`'s preview. That test may live in `game-native/tests` once C lands. If you add it, mark it `#[ignore]` with a note, because C owns the Ppu.

### Task E — engine objects: player, NPCs, movement, sprites

**Owns:** `crates/engine/src/objects.rs` (new), plus a new `crates/engine/src/objects/` directory if needed.

1. **State:** each object has:
   - local id, graphics, cell (x, y), elevation, facing
   - movement type and spawn point with range
   - visible and frozen flags
   - in-progress step: kind, direction, frames left, pixel offset
   - optional movement script: `&'static [Step]` and an index

   The player is an object with id `PLAYER`.
2. **Spawning:** `Objects::spawn_map(game, &MapData, flags: impl Fn(FlagId) -> bool, vars: impl Fn(VarId) -> u16)`.
   - Objects whose `hide_flag` is set are skipped.
   - `Gfx::Var` resolves through `vars`.
   - The player is kept.
3. **Player control:** `update_player(&mut self, terrain: &impl Terrain, input: PlayerInput) -> PlayerEvent`, as Gen3:
   - A tap of a new direction while idle turns in place, taking a few frames, and doesn't move.
   - Holding a direction walks 16 px per cell at 1 px/frame, or runs at 2 px/frame while B is held and `input.can_run`.
   - Collision when the target block's collision bits are non-zero, the behavior is `Blocked`, or an object occupies the cell.
   - Elevation mismatch blocks, except at elevation 0 or 15 (as Gen3 `IsElevationMismatchAt`).
   - Ledges `Ledge(dir)` jump 2 cells.
   - Events: `StepStarted { to, dir }`, `StepFinished { at }`, `Bumped(dir)`, `Turned(dir)`, `Idle`.

   `Terrain` is a small trait the integration implements:
   ```rust
   pub trait Terrain {
       fn block(&self, x: i16, y: i16) -> u16;
       fn behavior(&self, x: i16, y: i16) -> Behavior;
   }
   ```
4. **NPCs:** idle movement per `MovementType`: face, look around, wander within range (vertical/horizontal), walk in place. Timing is as Gen3, approximately. Use `&mut Rng` for every random choice. NPCs never walk onto the player or another object, or outside their range. Frozen objects (`lock`) do nothing.
5. **Scripted movement:** `start_moves(local, steps)`, `moves_done(local)`. Implement every `script::Step`, taking speeds and frame counts from Gen3 `MOVEMENT_ACTION_*`. `FacePlayer` uses the player's position. `Emote` may be a no-op with a TODO.
   Also `face(local, dir)`, `set_pos`, `show`/`hide`, `set_movement_type` and `freeze`/`unfreeze`.
6. **Sprites:** `draw(&mut self, game, camera_px, f: &mut Frame)` pushes `Obj`s, sorted so that lower y is behind.
   - **Animation**, as Gen3's standard 9-frame walker layout (`src/data/object_events/object_event_anims.h`):
     - Frames 0/1/2 face south/north/west; east is west with hflip.
     - Walking alternates the step frames 3/4, 5/6, 7/8.
     - The player's run frames are taken from Brendan's anim table.
     - Sprites with fewer frames show frame 0 (or the facing frame when they have at least 3).
   - **VRAM:** manage OBJ VRAM slots, uploading a frame's tiles with `Cmd::ObjTiles` only when it changes, and OBJ palette banks (one per distinct palette, `Cmd::ObjPalette` when first needed).
   - The player's sprite is drawn at screen (112, 64 − (h − 16)) when standing still, with objects positioned relative to the camera.
   - Priority is `layout::OBJ_PRIORITY`; elevation-based priority is a TODO.
7. **Hash:** `hash(&self, h)` over every object field that affects the future.
8. **Tests:** synthetic terrain covering:
   - the turn-then-walk timing
   - step pixel offsets each frame
   - collision with bits, objects and elevation
   - ledge jump
   - a wander that stays in range and is deterministic for a seed
   - every `Step` kind
   - VRAM upload only on frame change
   - the Obj output for a known state

### Task F — engine text box: printer, dialogue window, yes/no

**Owns:** `crates/engine/src/text.rs` (new), plus `crates/engine/src/text/` if needed.

1. **Printer:** prints Gen3-encoded text (format in `crates/extract/src/text.rs`).
   - Glyphs come from `Font` (2bpp 16×16 cells in ROM layout, `widths` per glyph; see `crates/extract/src/font.rs`). Characters are drawn left to right with variable width, rendered into a 4bpp tile canvas in `TEXT_CHAR_BLOCK`, with uploads by `Cmd::BgTile` for each changed tile.
   - Colors are `WindowFile`'s fg/shadow/bg indices in palette bank `WINDOW_PALETTE`. The font's 2bpp values map to bg, fg and shadow as Gen3 does.
   - Control codes:
     - `\n` (FE) moves to the second line.
     - `\l` (FA) scrolls up one line, waiting for a button first, as Gen3.
     - `\p` (FB) waits for a button, then clears.
     - `{PLAYER}` and `{RIVAL}` placeholders (FD 01, FD 06?) expand from `NewGame::player_name` and `UiText::RivalName`; check the codes in `charmap.txt`.
     - Other FD placeholders print nothing.
     - FC codes are skipped with their argument lengths (see `EXT_CODES`).
   - Speed: one character every 4 frames ("medium"). Pressing A while a page prints finishes the page at once. A flashing "continue" arrow is optional.
2. **Window:** the dialogue window covers the bottom of the screen (Gen3: frame at tile rows 14–19, two text lines). BG0 uses `layout` priority 0, its screen block, and `TEXT_CHAR_BLOCK`. Frame tiles come from `WindowGfx`, laid out as Gen3 `DrawDialogueFrame`. Palette: `WindowGfx::palette` → bank `WINDOW_PALETTE`. `open`/`close` enable and disable BG0 and clear its map.
3. **Yes/no menu:** a small framed window (Gen3 position) with `UiText::Yes`/`No` and a cursor. Up/down move the cursor, A confirms, B means no.
4. **API**, adjustable:
   ```rust
   pub struct TextBox { … }
   impl TextBox {
       pub fn open(&mut self, game: &impl GameData, f: &mut Frame);
       pub fn print(&mut self, game: &impl GameData, text: &'static [u8], f: &mut Frame);
       /// pressed = buttons newly pressed this frame, held = all held.
       pub fn update(&mut self, game: &impl GameData, pressed: Buttons, held: Buttons, f: &mut Frame);
       pub fn is_printing(&self) -> bool;          // page-wait states count as printing
       pub fn start_yes_no(&mut self, game: &impl GameData, f: &mut Frame);
       pub fn yes_no_result(&self) -> Option<bool>;
       pub fn close(&mut self, f: &mut Frame);
       pub fn is_open(&self) -> bool;
       pub fn hash(&self, h: &mut Fnv64);
   }
   ```
   A message is complete (for `WaitMessage`) when its last character has been printed; `WaitButton` is handled by the integration.
5. **Tests:**
   - glyph rendering of a known glyph into tile bytes
   - variable-width layout
   - each control code
   - timing (4 frames/char; A skips)
   - the frame tile layout
   - yes/no cursor logic

### Task G — engine flow: vars, fades, triggers, warps

**Owns:** `crates/engine/src/flow.rs` (new), plus `crates/engine/src/flow/` if needed.

1. **`Vars`:**
   - Flags 0..0x4000 as a bitset; vars 0x4000..0x4100 and 0x8000..0x8100 as u16.
   - `flag`, `set_flag`, `var`, `set_var`. Out-of-range ids read 0/false and ignore writes.
   - `clear_temp()`: Gen3 temp flags 0x0..0x20 and temp vars 0x4000..0x4010, cleared on map change.
   - `hash`.
2. **`Fader`:** fades brightness from 0 to 16 (to black) or back. Gen3-like speed is fine, about 16 frames. `update() -> brightness` and `is_done()`.
3. **Triggers**, as pure functions over data:
   - `coord_trigger(map, x, y, elevation, vars) -> Option<ScriptId>` (var == value, elevation rules as Gen3)
   - `warp_on_enter(game, map, x, y, behavior) -> Option<usize>` (Door, Warp)
   - `warp_on_push(game, map, x, y, behavior, dir) -> Option<usize>` (ArrowWarp in that dir)
   - `interact_target(...) -> Option<Target>`, where `Target` is `Object(local_id)` or `Bg(index)` for the cell the player faces, reaching across `Counter` behavior to the next cell. Bg facing rules come from `BgKind::Script.facing`. Hidden items are ignored for now.
   - `map_script_on_frame(map, vars) -> Option<ScriptId>` and `on_warp_into` (first entry whose var == value).
4. **`Transition`**, the warp state machine:
   - `start(dest map, dest warp or xy)`.
   - Stages: fade to black → (caller loads map: field, objects, map scripts) → fade in.
   - If the destination cell's behavior is `Door`, the player auto-walks one cell south after the load, as when Gen3 exits a door.
   - Driven by the integration through `update() -> TransitionStep { FadingOut, Load(MapId, x, y, facing), FadingIn, Done }`.
5. **`Input` edge detection:** `Input { held, pressed }` from previous and current `Buttons`.
6. **Tests** for everything above, on synthetic maps.

---

## Wave 2 (parallel, after Wave 1 is merged)

### Task B — script decoder and Rust codegen

**Owns:**
- `crates/extract/src/scripts.rs`
- `crates/extract/src/codegen.rs` (new)
- `crates/extract/src/movement.rs` (new)
- `tests/scripts.rs`, `tests/codegen.rs`
- the `export_scripts` hook in `export.rs`

1. **Decoder:** decode every script reachable from the 15 maps' roots (`roots::named_roots`) into an instruction list, with labels for `goto`/`call` targets. `callstd`/`gotostd` stay calls to std scripts, which are not walked. Movement data decodes into `script::Step`s via `names::MOVEMENT_ACTIONS`; unknown actions become a `Step::Delay(0)` with a comment, and are reported.
2. **Texts:** write `text/<lang>.ron` with keys `<Script>_T<n>` (naming rules in `schema.rs`), using the language-aware charmap from Task A. Test that every text re-encodes to its ROM bytes.
3. **Codegen:** write `scripts/<Map>.rs` per map, plus `scripts/index.ron`. Each root and label becomes `pub async fn <Name>(ctx: Ctx)`, with `#![allow(non_snake_case)]` (the file is included inside a module, so use an inner allow on the generated module from content's side; coordinate via the prelude contract below).
   - **Control flow:** `goto`/`goto_if` within one script become a `loop { match pc { … } }` dispatch over that script's labels. Each label block runs straight-line code and sets `pc` or returns. Labels jumped to from another root are emitted as their own fns as well.
   - **Calls:** `call` becomes `Box::pin(<Label>(ctx.clone())).await;`, and `return` returns from the fn.
   - **Comparisons:** `compare`/`goto_if`/`call_if` use a local `cond: core::cmp::Ordering` register, as Gen3 does. `checkflag` sets `cond` to `Equal` when the flag is set.
   - **msgbox:** the pattern `loadword 0, <text>` + `callstd MSGBOX_<KIND>` becomes `msgbox(&ctx, <TEXT>, MsgBox::<Kind>).await;`. Other `callstd` calls become `callstd(&ctx, STD_<NAME>).await`.
   - **Mapped commands:** every command in this list maps to a `Ctx` primitive or a prelude helper:
     - lock, lockall, release, releaseall, faceplayer
     - message, waitmessage, waitbuttonpress, closemessage, yesnobox
     - applymovement, waitmovement (and the `*at` forms when the map is the current one)
     - setflag, clearflag, checkflag, setvar, addvar, subvar, copyvar, compare*
     - delay, fadescreen, warp*, setobjectxy, setobjectmovementtype, addobject, removeobject, showobjectat, hideobjectat, turnobject
     - setmetatile, special, specialvar, random, goto*, call*, return, end
   - **Specials:** `special(&ctx, SPECIAL_<NAME>).await` returns a u16. `specialvar` stores it into a var.
   - **Sound commands** (playse, playbgm, fanfare…) become comments.
   - **Everything else:** `unsupported(&ctx, "<macro>").await`.
   - **Movement tables:** `pub static <Script>_M<n>: [Step; N] = [...]`.
   - **Header:** each generated file starts `// @generated by cargo xtask extract from <code> <sha1>; edit by copying into content/src/scripts/`.
4. **Prelude contract** (implemented by Task H in `content::prelude`). Generated code may only use:
   ```rust
   use crate::prelude::*;
   // brings in: Ctx, Step, Speed, Emote, Fade, Dir, MapId, FlagId, VarId, TextId, MovementType,
   // PLAYER, core::cmp::Ordering, Box,
   // every FLAG_*/VAR_*/MAP_*/SPECIAL_*/STD_* constant, and every text constant (<Script>_T<n>),
   // async fn msgbox(ctx: &Ctx, text: TextId, kind: MsgBox), enum MsgBox { … one per MSGBOX_* },
   // async fn callstd(ctx: &Ctx, std: u8), async fn special(ctx: &Ctx, id: u16) -> u16,
   // async fn unsupported(ctx: &Ctx, what: &'static str)
   ```
   MAP constants: `MAP_LITTLEROOT_TOWN: MapId`. Warps to maps that were not exported still compile, through `MAP_*` constants for all maps; content maps an unknown map to a log plus a no-op.
5. **Tests:**
   - Golden codegen for a few synthetic scripts (every control-flow shape).
   - US Littleroot: the generated Rust for one known script matches a hand-checked expectation.
   - FR: generation is deterministic.
   - Compile check: a test crate (or `content` in Wave 3) compiles the output. Within B, run `rustc --edition 2024 --crate-type lib` on a scratch crate containing a stub prelude, and keep that stub in `tests/` for the test.

### Task H — content build: data tables, prelude, stub fallback

**Owns:**
- `crates/content/{build.rs, Cargo.toml, src/**}`
- `.gitignore` (if needed)

`crates/content/src/lib.rs` keeps the M1 scene until Task I removes it; add next to it.

1. **`build.rs`:** reads `content/extracted/` with `extract` as a build-dependency (schema types, `gfx::read_png_indexed`, names, and `text::encode` with the charmap of `manifest.language`). It generates `$OUT_DIR/data.rs`:
   - tilesets: tiles as `[Tile; N]`, palettes, metatiles, attributes
   - maps: `MapData`, with events converted:
     - `MovementType` and `Behavior` mapping from names
     - `Gfx::Var` for `OBJ_EVENT_GFX_VAR_*`
     - connections and warps to unexported maps dropped (warps keep the warp but point to a `MapId` the content resolves to a "missing map" log)
   - sprites: frames as tile slices in GBA OBJ 1D order
   - font, window
   - texts as `&[u8]`, Gen3-encoded and 0xFF-terminated
   - `MAP_*`/`FLAG_*`/`VAR_*`/`SPECIAL_*`/`STD_*` constants, and text constants
   - a script dispatch: `ScriptId` → `Box::pin(fn(ctx))`, `include!`ing `scripts/<Map>.rs` inside per-map modules
   - `cargo:rerun-if-changed` for the extracted dir
2. **Without `content/extracted/`** (fresh clone, CI): generate a small built-in stub world so everything compiles and runs. The stub is one tiny map using the M1 test tiles, a 1-glyph font and so on, and needs no ROM. Emit `cargo:rustc-cfg=pkr_stub`. Parity and unit tests must pass in both modes.
3. **`content::prelude`:** implements the Task B contract.
   - `msgbox` per Gen3 std scripts (`data/scripts/std_msgbox.inc`). For example, `MSGBOX_NPC` = lock, faceplayer, message, waitmessage, waitbutton, release.
   - `callstd`: implement the ones the 15 maps use; log the rest.
   - `special`: a dispatcher; every special logs and returns 0, except a few the slice needs, implemented as small async fns.
4. **`Emerald: GameData` impl:** `behavior()` maps `MB_*` (Normal, Blocked for water/impassable, ledges, doors, warps, arrow warps, counters, tall grass). `new_game()` comes from Task I; provide a placeholder: Littleroot, outside Brendan's house.
5. **Tests:**
   - with extracted data: every map references valid tilesets and scripts, every text decodes, and every `ScriptId` dispatches
   - stub mode: builds and runs
   - the GBA build still compiles with the real data (`cargo xtask rom`)
   - ROM size stays below 32 MiB

---

## Wave 3

### Task I — integration: the overworld World

**Owns:** everything needed, coordinating with the module APIs.

1. **`World`:** generic over `GameData`.
   - It holds: `Field`, `Objects`, `TextBox`, `Vars`, `Fader`, `Transition`, `Runner`, `Rng`, `Input`, the frame counter and the dialogue-lock state.
   - `World::new(game)` starts a new game: runs `NewGame::script` synchronously-ish at frame 0, loads the map and fades in.
   - `step(&mut self, game, keys) -> Frame`, frame order:
     1. update input
     2. if a transition is active, run it
     3. else if a script is running, step the runner (World implements `ScriptHost`)
     4. else run player control and triggers: interact on A, coord and warp triggers on step end, `on_frame` map scripts, connection crossing
     5. update NPCs
     6. move the camera
     7. draw objects
     8. text box update
   - `hash()` covers all state, extending the parity contract. Replace the M1 golden values with new ones, documented.
2. **`ScriptHost`:** implement every `Request`.
   - Instant: flags/vars (including special vars such as `VAR_FACING` and `VAR_RESULT`), random, player position and facing, log (kept in a ring buffer for tests and the debug region).
   - Frame-spanning: `Message` and `WaitMessage`/`WaitButton`, `YesNo`, `Move`/`WaitMove`, `Delay`, `Fade`, `Warp`.
   - Map scripts: `on_load` (before drawing), `on_transition`, `on_warp_into`, `on_frame`, `on_resume` run at the right moments, as Gen3.
3. **New game:** start in LittlerootTown (or Brendan's house 2F) in a state where:
   - Littleroot, Route 101 and Oldale are freely walkable.
   - Their cutscenes do not softlock without battles: set the vars as after receiving the starter and before Route 103.
   - The rival is May.

   Document the chosen vars and flags, mirroring the decomp's new-game and intro scripts. The player name is "SACHA".
4. **Remove the M1 scene path:** `scene.rs` and `Content`. Shells use `GameData`; the debug region stays.
5. **Parity:** extend `crates/emu/tests/parity.rs` with a scripted run of roughly 1500–3000 frames:
   - walk out of the start
   - enter and leave a house (warp and door step-out)
   - talk to an NPC (text box, \p, closing)
   - walk north to Route 101 (connection) and on to Oldale (triggering the Oldale guide coord script if the vars allow it)
   - enter the Pokémon Center

   Compare the World hash and framebuffer at several checkpoints and at the end. Both the stub and the real data must pass; the real-data run needs the ROM, so skip it with a clear message when `content/extracted` is missing.
6. **`cargo xtask shot`:** dump a few named moments (Littleroot start, inside a house, a text box, Oldale) for both ROM and native as PNGs in `target/shots/`.
7. **README:** a "Play" section, covering a native interactive runner if cheap (optional: a minimal `minifb` window binary in `game-native` behind a feature) and `target/game.gba` in mGBA.

   **Acceptance:**
   - All tests pass.
   - `cargo xtask parity` passes with the real data.
   - The shots show correct French text boxes and maps.
   - ROM and native are identical.

## Wave 4 — close-out

The milestone review covers the whole workspace: fix review findings, update the plan doc with "As built" notes and "Findings for Milestone 4", and clean up worktrees.

## Done when

- `cargo xtask extract && cargo xtask parity` pass.
- In the shots and the parity run, the player walks between Littleroot, Route 101 and Oldale, enters buildings, and talks to French-speaking NPCs.
- NPCs wander.
- Scripts are generated Rust run by the engine.

---

## As built

All nine tasks are merged into master (`git log 8c3af1d..f95001e`). Each entry lists what was built and every deviation from the plan.

### Task A — extract v2
- **Schema v2** with stable names: maps by decomp dir name, tilesets by `gTileset_<Name>`, sprites by `OBJ_EVENT_GFX_*`, script roots by the `schema.rs` rules (`roots::named_roots`). ROM offsets appear only in `manifest.ron` (`provenance`).
- **Tileset names:** deviation. The `Tileset` headers are not one array of equal structs, because two 4-byte `Tileset` pointers sit among them. `TilesetHeaders::locate` starts from `gTileset_General` (Littleroot's primary) and uses a generated offset table, `names::TILESET_HEADER_OFFSETS`, in `headers.h` order. Every entry must read as a `Tileset`.
- **`names` tables** are generated from the decomp into `crates/extract/src/names/generated.rs`. The drift test `tests/names.rs` checks them against the decomp, and `PKR_BLESS=1` regenerates them. The plan said "committed tables", and generated plus committed satisfies that.
- **Review fixes to the tables:**
  - `METATILE_BEHAVIORS` was off by one from `MB_DEEP_WATER` onwards, because a comment in the enum was not stripped.
  - `FLAGS` now evaluates `%` and enum members, and fails on any define it cannot evaluate.
  - `ITEMS` is limited to `ITEM_NONE..ITEMS_COUNT`.
  - `SPECIALS` names have comments stripped, and every name is tested to be an identifier.
- **French charmap:** `text::Lang` with `decode_lang`/`encode_lang`. BPEF 0xB1/0xB2 are `«`/`»`. `text::round_trip_at` checks that a text re-encodes to its ROM bytes.
- **VAR graphics:** the sprites reachable through `setvar VAR_OBJ_GFX_ID_*` are found by a scan of the maps' scripts, factored out as `var_gfx_ids` and tested. The fallback of exporting every id below 100 was not needed.
- **Cleanups:** all the planned cleanups were done (`Rom::u16s`/`req_ptr`, shared constants, `gfx::rgb555_to_rgba`, no `read_latin_normal`, no serde on ROM structs, `tiles.png` with the first used palette).

### Task B — script decoder and codegen
- **Output:** the 15 maps generate 359 functions, 313 texts and 166 movement tables. `extract` prints the counts as a codegen summary (`codegen::Stats::summary`).
- **Control flow:** as planned, a function with more than one block (or a jump back to its entry) is a `loop { match pc { … } }` dispatch. `goto`/fallthrough into another function is a tail call. A block reached from several functions becomes its own function.
- **`cond`:** a local `Ordering` register, as Gen3 `comparisonResult`. `checkflag` sets `Equal` or `Less`.
- **Unsupported commands:** what remains becomes `unsupported(&ctx, "<macro>")`. Counts, most frequent first: trainerbattle 23, multichoice 13, hidemonpic 7, and a tail of rarer commands (see the summary).
- **Unknown movement actions:** `FACE_ORIGINAL_DIRECTION`, `NURSE_JOY_BOW_DOWN` and `CUT_TREE` become `Step::Delay(0)` with a comment.
- **Known gaps:** a local `cond` and plain functions cannot express two Gen3 behaviours: a callee's `end` stopping its caller, and `cond` flowing across a call. Both are commented in the output and counted. There are 5 `end`-in-callee sites.
- **Additions:** the decoder gained `addobject`/`removeobject`, `setobjectxyperm`, the door commands, `waitstate` and `checkplayergender`. These need the contract additions listed below.
- **Texts:** `text/<lang>.ron` is written, and every text is tested to re-encode to its ROM bytes.
- **Compile check:** `content` itself compiles the output, with no scratch crate. `crates/content/tests/build_modes.rs` builds a fixture extracted dir with hand-written scripts.

### Task C — video
- **Native PPU:** `game_native::Ppu`, a mode-0 software PPU implementing the plan's rules, with a unit test per rule.
- **GBA applier:** `game-gba/src/video.rs` writes the registers first and then the frame's commands at vblank. Bulk copies use DMA3, and register fields are masked in release.
- **Shared validation:** `Frame::validate` is used by both shells. Native panics on an invalid frame, and GBA debug builds check too.
- **`late` detection:** an addition to the plan. The shell detects an apply that overran vblank, either because it woke late or because `VCOUNT` was back in the picture. It counts these in the debug region's `late`, and parity requires `late == 0`.
- **Parity:** compares hashes at every frame and pictures at checkpoints. Native rendering is lazy, so frames are only drawn when a picture is compared.
- **M1 bridge:** the M1 scene went through `Frame` until Task I removed it. `world::FrameOut` is gone.

### Task D — field
- **Drawing:** a ring buffer of 16×16 metatiles per BG, Gen3 layer routing, and border/connection drawing with the current map's tilesets.
- **Split `load`:** `enter`/`switch_map_enter` were split from drawing. This lets `on_load` overrides (`setmetatile`) land before the first draw.
- **Crossings:** a deviation, added during Task I. A connection crossing keeps the drawn view. The ring has an origin that moves with the coordinates, so only cells coming into view are drawn, as in Gen3 `LoadMapFromCameraTransition`. A full redraw only happens when a tileset changes. The planned whole-view redraw on crossing overran the GBA frame.

### Task E — objects
- **Built as planned:** player control with turn-then-walk, Gen3 collision, elevation and ledges. NPC idle movement driven by the World `Rng`. Every `Step`. Sprites with the 9-frame walker layout and OBJ VRAM/palette slot management.
- **Review changes:**
  - Player update is explicit, and the World drives it.
  - Objects of connected maps are kept on a crossing.
  - The render cache (VRAM slots) is left out of the hash.
- **Not done yet:** `Emote` and elevation-based priority are still TODOs.

### Task F — text box
- **Built as planned:** the Gen3 printer with the control codes and placeholders, the dialogue frame, and the yes/no menu.
- **Deviation:** pressing A while a page prints does not finish the page at once. It follows Gen3 `RenderText` `hasPrintBeenSpedUp`: a new A/B press skips the current delay, and while A/B stays held one glyph is drawn per frame. This also bounds the per-frame text cost on the GBA.
- **Review changes:**
  - Glyphs are blitted row by row.
  - The hash covers a per-message digest instead of the text.
  - Advances saturate, and frame entries are clipped.

### Task G — flow
- **Built as planned:** `Vars`, `Fader`, the triggers, `Transition` and `Input`.
- **Additions:**
  - Behaviors are looked up on the live field, so `setmetatile` overrides count.
  - The arrival after a load (the door step-out) is part of the transition.
  - `Vars` keeps an O(1) digest, so the per-frame hash stays cheap.

### Task H — content build
- **Layout:** `build.rs` lives in `crates/content/build/` (`main.rs`, `input.rs`, `classify.rs`, `codegen.rs`, `stub.rs`). It generates:
  - `consts.rs`: the `FLAG_`/`VAR_`/`MAP_`/`SPECIAL_`/`STD_`/`ITEM_`/`OBJ_EVENT_GFX_` constants and the text ids.
  - `data.rs`: the static tables, the behavior table and the script dispatch, plus `LANG` and `SOURCE_SHA1`.
  - `scripts.rs`: one module per map, `include!`ing `scripts/<Map>.rs` under the prelude.
- **Prelude:** `content::prelude` implements the Task B contract: `msgbox` per `std_msgbox.inc`, `callstd`, `special`, `unsupported`, and `check_player_gender` (male). `STD_FIND_ITEM` removes the talked-to object.
- **Stub mode:** with no extracted dir, the build creates a one-map stub world and sets `cfg(pkr_stub)`.
  - `PKR_EXTRACTED_DIR` overrides the extracted dir. A relative path resolves against the workspace root.
  - `content/.gitkeep` is tracked, so a stub build watches `content/` instead of rebuilding every time.
  - A missing explicit `PKR_EXTRACTED_DIR` warns, and the build stays always dirty.
- **Behavior mapping** (`classify.rs`):
  - `Blocked`: water that needs Surf, `MB_WATER_DOOR` and the directional `MB_IMPASSABLE_*` (there is no directional blocking yet).
  - `Door`: only `MB_ANIMATED_DOOR`, as Gen3 `IsWarpDoor`.
  - `Warp`: stairs (`MB_NON_ANIMATED_DOOR`), `MB_DEEP_SOUTH_WARP`, ladders, escalators and the other `IsWarpMetatileBehavior` warps.
  - Also `Ledge`, `ArrowWarp`, `Counter` and `TallGrass`.
  - Everything else is `Normal`.
- **Movement types:** a movement type with no exact engine equivalent maps to the closest one, and the build lists it as a `// movement fallback`. Examples: walk sequences become `Wander`, rotations `LookAround`, and the remaining types `Face(Down)`.
- **Sprites:** a sprite with frames of mixed sizes (BERRY_TREE) is padded to the cell size, with a warning.
- **Specials:** the decomp lists some specials twice (for example `ShowGlassWorkshopMenu` at 0x115 and 0x15C). A name keeps its first number, and the duplicates are reported. A constant defined twice with different values is an error.

### Task I — integration
- **World:** `World<…>` composes every module. The frame order is documented at the top of `world.rs`, following Gen3 `CB1_Overworld`:
  1. input
  2. the transition, or else the script, or else player control (`on_frame`, arrow warps, A interactions, door warps, then the step and its step-end triggers, then the crossing)
  3. objects
  4. the fader
  5. the camera
  6. sprites
  7. the text box

  Map loads follow `LoadMapFromWarp`: clear the temp flags and vars, then `on_transition` before the field switches maps, then `on_load` before the first draw, `on_resume`, object spawning and `on_warp_into`. Map scripts run to completion at once.
- **New game:** a content script (`crates/content/src/new_game.rs`). The player is a male SACHA, the rival is May, Birch has been rescued (`VAR_ROUTE101_STATE` 3), the starter has been received (`VAR_BIRCH_LAB_STATE` 3), and the player has been told to meet May on Route 103 (`VAR_LITTLEROOT_INTRO_STATE` 7, `VAR_LITTLEROOT_TOWN_STATE` 2, `VAR_LITTLEROOT_RIVAL_STATE` 3, `VAR_LITTLEROOT_HOUSES_STATE_BRENDAN` 2). The vars mirror the decomp. `FLAG_SYS_B_DASH` and `FLAG_ADVENTURE_STARTED` stay clear. The player starts outside Brendan's house.
- **`GameData` additions:** `has_map`, and `special_ids` (`SpecialIds`). `special_ids` gives `VAR_FACING`, `VAR_LAST_TALKED`, the running flag and the flags cleared on a map change, so the engine stays game-agnostic.
- **Warps end the script:** a warp load resets the runner, `selected` and the last mover, and closes the text box, as Gen3 `CB2_LoadMap`. A script `Warp` is never answered.
- **Gen3 interaction order:** an object without a script falls through to the bg event ahead.
- **Hash:** `World::hash` covers the runner (the active request and the number issued, O(1)). The FNV-1a multiply avoids a 64-bit multiply: Thumb has no long multiply, and the prime is 2^40 + 0x1b3, so the product splits into a shift and 32-bit multiplies. The per-frame hash on the GBA went from about 46 to about 9 scanlines, bit-identical. New golden hashes are documented in the README.
- **`late_black`:** a deviation from strict no-late. A whole-tileset upload on a warp load can overrun vblank. The GBA shell counts such an apply in `late_black` when the screen is fully black both before and after it, and in `late` otherwise.
  - A const assertion ties this rule to the blend setup: BLDCNT darkens BG0–3, OBJ and the backdrop, and no window is enabled.
  - Parity requires `late == 0` and `late_black <= warps`.
- **Parity route:** `game_native::scenario` records a high-level route. It leaves the start, goes into Brendan's house and back, crosses Route 101 to Oldale, follows the Oldale Mart guide's tour (lock, faceplayer, `VAR_FACING` switch, `waitmovement 0`, multi-page text), talks to an NPC, and enters the Pokémon Center.
  - The route is 2899 frames. All 2896 hashes and all 39 compared pictures are identical.
  - `late_black` is 3, for 3 warps.
  - Stub mode walks around the stub map.
- **Shots:** `cargo xtask shot` writes `target/shots/<moment>-{rom,native}.png` and fails if any pair differs.
- **ROM:** `cargo xtask rom` checks the 32 MiB limit. The ROM is 1410 KiB.
- **Play:** `cargo run -p game-native --features window --bin play` runs a minifb window.
- **Removed:** the M1 scene path (`scene.rs`, `Content`). `Tile` moved to `video`.

### Contract additions during the milestone
- `script::Request::AddObject` / `RemoveObject` (Gen3 `addobject`/`removeobject`). `AddObject` is a no-op for an object that exists.
- `Request::SetObjectTemplatePos` (`setobjectxyperm`).
- `Request::Door` / `WaitDoor` (timing only, `DOOR_FRAMES` = 20) and `Request::WaitState`.
- `WaitMove(0)` waits for the last mover (Gen3 `waitmovement 0`). `WaitMove(n != 0)` names the last mover.
- `check_player_gender` in the prelude.
- `GameData::has_map` and `GameData::special_ids`.

---

## Findings for Milestone 4

- **Strict no-late:** spread warp-load uploads over several black frames, as Gen3 `LoadMapInStepsLocal` does, so that `late_black` can return to 0. A connection crossing between maps with different tilesets still does a full redraw on a visible frame. The slice's maps never hit it, but other maps will.
- **Doors:** `Door`/`WaitDoor` only take the Gen3 time. There are no door animation graphics yet.
- **Next script commands**, by need: `setrespawn`, `multichoice`, `trainerbattle`, `givemon`, `pokemart`, and the specials the next maps use.
- **Scenario pathing ignores elevation.** Routes through multi-level maps will need it.
- **Old-map NPCs after a crossing:** they can be talked to (Gen3 `GetObjectEventIdByPosition` does not filter by map), but `selected` is empty for them. Check what their scripts see.
- **Step timing:** step triggers (coord events, warp cells) fire one frame earlier than in Gen3.
- **Hashing cost on the GBA:** the per-frame World hash is about 9 scanlines. Keep new state's hash contribution O(1) per frame (digests, counters).
- **The next milestone** is planned in `docs/plans/2026-10-21-milestone-4-battle.md`.
