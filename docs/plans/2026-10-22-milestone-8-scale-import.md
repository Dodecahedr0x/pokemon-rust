# Milestone 8 — Scale Import (all content, Act 1 playable) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** turn the 15-map slice into the **whole game's content**, and make the first chapter of the story playable end to end. The design's M8 line ("all maps/data/scripts decompiled to Rust; then full engine parity, iteratively") is several milestones of work. This plan is the first, bounded chunk:
- **Import everything:** all 518 maps, 73 tilesets, every layout, sprite, encounter table, species and trainer pic, item, multichoice list and standard string. Every event script reachable from a map (about 4,560 functions) is decompiled to async Rust by the **content build**, from a ROM-derived script IR, with decomp label names.
- **Run everything without crashing:** every script command and every special is classified (native, ignored, default value, or unsupported). Nothing reachable panics or softlocks for want of a feature: unsupported features log, raise an engine `Report` event, and continue with a documented default.
- **Act 1 playable:** from the M4 new game (before Birch's rescue) to **Roxanne's badge** (`FLAG_BADGE01_GET`), natively and on the GBA, with the engine features that path needs: items and money (no bag UI), string buffers, multichoice, trainer sight, the battle effects and abilities of Act 1, dynamic warps, scale-proof map loads and crossings.
- **Fold in the findings** of M5–M7 (a table at the end gives each one's disposition).
- **Keep every gate green at scale:** parity (slice route plus Act 1 segments), checkpoints, the explorer (goal-directed from checkpoints, smaller db), the studio, and the GBA budgets.

Later parity milestones (menus and bag UI, field moves, double battles, contests, audio…) are a roadmap at the end ("Roadmap: Milestone 9 and later").

**Architecture**
- **Script pipeline split.** `extract` decodes the ROM's bytecode into a **script IR** (`scripts/<Module>.ron`, git-ignored like every extracted file). `content/build.rs` lowers the IR to async Rust in `$OUT_DIR`. The generator (`extract::codegen::lower`), the committed support tables (`commands.ron`, `specials.ron`) and hand-written **overrides** are the only committed parts. Adding an engine command is then a build change, never a re-extract.
- **Names from the decomp.** Script, text and movement names come from the decomp's `.inc` labels, transferred onto the FR bytecode by a parallel walk from `map.json` roots. Names no longer depend on which maps are exported, or in what order. The positional `<Map>_Obj<n>` scheme stays as the fallback.
- **Bulk data in blobs.** Map blocks, tilesets, sprites, pics and text bytes go into aligned binary blobs (`include_bytes!`, sliced by `content::blob::Blob`), as battle graphics already do. Rust source keeps only small typed tables. The spike's 11.7 MB `data.rs` (rustc at 4.96 GB) is the reason.
- **One state change, once.** Wave 0 adds all new `World` state (bag, money, string vars, warp slots, the stale-view rect) with its hash and snapshot DTOs. Snapshot `SCHEMA` 2, one golden update and one tree re-accept. Wave 1 tasks implement behaviour behind those fields.
- **Observability in the engine.** `World::events()` (not hashed, not snapshotted) lists, per frame, map entries, script starts with their `ScriptId`, battle starts and ends, and `Report`s (unsupported or degraded features). It replaces the explorer's `Probe` and gives panics their script and map context.
- **Explorer at scale.** It is goal-directed from checkpoints (`--from`). Goals are state predicates per act. The db is smaller (binary inputs, a per-key hit counter) and incremental after content edits (per-script and per-map digests). It finds the Act 1 route, which is exported into the committed checkpoint tree and replayed by `xtask parity` in segments.

**Tech stack**
- Rust 2024. **No new dependency** in `engine`, `content` or `game-gba`.
- `extract` reads decomp sources it already reads (`data/maps/*/map.json`, `data/maps/*/scripts.inc`, `data/scripts/*.inc`, `include/constants/*.h`, `src/data/*.h`), through `PKR_DECOMP`, as the names tables and drift tests do since M3. **Script names need the decomp at extract time**; without it, extraction falls back to positional names and warns.
- `object` 0.37 in xtask (already there) for the per-section ROM report.

A task may add to a contract if it needs to. It may not change the meaning of what is already there, and every addition must be listed in its final report.

---

## Where the slice stands (measured while planning)

Measured on master `7e9bd93` (M7 S1 merged), Apple M4 Max. The "full import" column comes from a **throwaway spike**: the current `extract` with `export::MAPS` replaced by all 518 maps (slice maps first), exported to a scratch dir, then built natively and for the GBA. One assert was disabled for it (the slice-move wording check, see Task E). Nothing from the spike is committed.

| | pokeemerald | slice (today) | full import (spike, today's code) |
|---|---|---|---|
| Maps | 518 (`names::MAPS`; 468 `scripts.inc`) | 15 (2.9%) | **518/518 headers read**; export fails only on Altering Cave's 9 wild headers ("two wild headers") |
| Tilesets / layouts | 73 / 442 | 8 / 15 | 73 / 518 used; 5.5 s export |
| Sprites (OBJ gfx) | 256 `OBJ_EVENT_GFX_*` ids | 27 | 156 placed or VAR-reachable |
| Script functions | 7,042 map labels + 1,369 common | 359 | **4,560** fns, 97,384 lines, 3.9 MB of Rust |
| `SCRIPT_NAMES` started by the explorer | — | **89/359 (24.8%)** in 600 s | — |
| Texts | about 15k string labels (all sources) | 313 | **5,503 reached; 5,503/5,503 re-encode to their FR bytes** |
| Movements / unknown actions | 160 `MOVEMENT_ACTION_*` | 166 / 3 | 978 / 15 kinds (`SLIDE_UP` 16, `WALK_SLOW_DIAGONAL_*` 28, affine 12, …) |
| Script commands | 227 opcodes (`script_cmd_table.inc`) | decoder knows all 227; codegen lowers ~60 | **1,543 `unsupported` sites in 90 kinds**, 1 unknown opcode, 80 `end`-in-callee, 80 unresolved `waitmovement` |
| Specials | 528 (`data/specials.inc`) | 3 implemented (ChooseStarter, HealPlayerParty, ChangePokemonNickname); the rest log and return 0 | **433 used** by the generated scripts |
| Movement types | 81 | 15 exact, the rest approximated (`classify::movement`) | same |
| Move effects | 214 | 61 (26 named + 35 stat-stage) = 28.5% | 142 distinct unsupported effects among the 271 moves reachable mons know |
| Abilities | 77 + NONE | 14 | 52 used by reachable species are `Ability::Other` |
| Species / trainers | 412 / 855 | tables complete; pics for 19; 17 trainers | pics for 177 (land mons only); 518 trainers referenced |
| Content build (native debug) | — | 3 s incremental | **31 s, rustc 4.96 GB peak**; `data.rs` 11.7 MB, `battle.bin` 1.1 MB, `consts.rs` 0.6 MB |
| Extracted dir | — | ~5 MB + previews | 144 MB, of which **previews 121 MB** |
| GBA ROM | — | 1,991,312 B: scripts 763 KB (38%), data 334 KB, battle 351 KB, engine 241 KB | **opt-level 3: `.text` 7.76 MB + `.rodata` 4.68 MB ≈ 12.4 MB** (scripts 7.28 MB in 8,247 symbols, data 2.6 MB, battle 1.17 MB, engine 243 KB); **content at opt-level "s": `.text` 5.49 MB, ≈ 10.2 MB**. Release build 2 m 42 s (1 m 55 s with "s"), 8.9 GB peak RSS |
| Script futures | — | host max 192 B | **host max 200 B, median 160 B** (4,560 roots, `size_of_val` of the boxed future): EWRAM is not at risk |

**Reading.** The decoder, the generator, the text codec and the map readers already scale. What does not:
- the slice assumptions: `export::MAPS`, the slice sets in `content/build/battle.rs`, one wild header per map, `SLICE_*` in tests;
- Rust literals for bulk data (compile memory);
- unsupported commands and specials (1,543 sites, 433 specials);
- names that depend on the export order: shared scripts take the first referencing root's name, for example the Pokémon Center nurse is `OldaleTown_PokemonCenter_1F_Obj1_L0`.

Projected full ROM with every species' pics (about +1.2 MB): about 13.6 MB at opt-level 3, about 11.4 MB with content at "s". It fits under Emerald's own 16 MiB, so no banking or compression is needed (decision 7).

---

## Open questions for the user (defaults taken; nothing blocks on them)

1. **M8's story target.** *Default: Act 1, from the M4 new game to Roxanne's badge (`FLAG_BADGE01_GET`).* Alternatives: (a) no story target, only "everything imports and runs without crashing", which is smaller but has no end-to-end proof; (b) through badge 2 (Dewford), which needs the Mr. Briney boat cutscenes, Granite Cave's Flash darkness and more battle work.
2. **Rename scripts and texts to decomp labels** (decision 3). *Default: yes.* Every `SCRIPT_NAMES`/`TEXT_NAMES` identifier changes (`Route101_Coord0` becomes something like `Route101_EventScript_StartBirchRescue`), and the tests that name slice scripts migrate once. Alternative: keep positional names, which are stable only if the export order is frozen forever.
3. **Double battles before M11.** *Default: a double trainer battle runs as a single battle against the same party, with a `Degraded("double battle")` report, and sets the trainer flag.* Alternative: skip the battle and leave the trainer unbeaten. That is more faithful in effect (nothing that did not happen) but blocks progress where a double is mandatory (none in Act 1; Tate & Liza later).
4. **ROM size gate.** *Default: 16 MiB* (Emerald's own size; the projection is 11–14 MB). The hard limit is 32 MiB.
5. **Commit the Act 1 route to the shared checkpoint tree** (input-only node files, roughly 150k–300k frames, a few hundred KB of RON at the M5 run syntax). *Default: yes, as `act1:*` nodes,* so parity and `verify` cover Act 1.
6. **New-game start.** *Default: keep the M4 start (outside Brendan's house, before the rescue).* The real intro (truck, Birch's speech, naming screen, clock) needs M9's menus.

---

## Decisions (summary; reasons in "Decisions taken" at the end)

| # | Topic | Decision |
|---|---|---|
| 1 | Scope | Import all content; classify every command and special; make Act 1 (new game → badge 1) playable; fold in the M5–M7 findings. Everything else is the roadmap. |
| 2 | Script pipeline | ROM bytecode → **IR** (`extract`, git-ignored `scripts/*.ron`) → async Rust in `$OUT_DIR` (`content/build.rs` calls `extract::codegen::lower`). The pokeemerald `.inc` sources are never compiled: the FR ROM is the source, and the decomp supplies names only. |
| 3 | Names | Decomp labels by **label transfer** (a parallel walk of `map.json` roots and `.inc` label references against the decoded bytecode); positional names as the fallback; modules follow the decomp source file. |
| 4 | Degradation | Committed `commands.ron` and `specials.ron` classify every command and special as `Native`, `Ignore`, `Default(v)` or `Unsupported`. An unclassified one fails the build. `Unsupported` raises `Report::Unsupported(name)`, logs, and continues; never a panic. |
| 5 | Slice assumptions | `export::MAPS` → every map (`--maps` for fixtures); the slice sets in `battle.rs` → every species and trainer; `SLICE_SPECIES`/`SLICE_MOVES`/`PLAYER_LEVEL_CAP` removed; multiple wild headers per map; tests sample fixed ids instead of slice tables. |
| 6 | Bulk data | Binary blobs for maps, layouts, tilesets, sprites, pics and text bytes; Rust literals only for small typed tables. |
| 7 | GBA budgets | No banking (the GBA maps 32 MiB linearly). ROM gate 16 MiB; `content` built at opt-level "s" if Wave 0 confirms the spike; LZ77 only if the gate is threatened. IWRAM and EWRAM statics keep their growth gates over a new baseline. New: heap high-water in the debug region, gated. Futures need no boxing policy beyond today's (200 B max). |
| 8 | Hash, schema, trees | Wave 0 is the only planned hash change (new state). Snapshot `SCHEMA` 2. Tasks that change the slice route's behaviour (F, G, I) refresh trees in a separate `chore(checkpoints)` commit with the cause. Act 1 nodes join the committed tree. |
| 9 | Parity at scale | The slice route as today, plus ROM-resumes-from-native at every `act1:*` node (2,000 frames each), plus a full straight Act 1 replay under `cargo xtask parity --long`. A ROM **map tour** build checks `late == 0` on every map load and crossing. |
| 10 | Explorer at scale | `--from <checkpoint>` roots; per-act goal lists of state predicates; key gaps closed; db schema v3 (binary inputs, `key_hit` counter, no childless merged rows, inputs not resident); `World::events()` replaces `Probe`; incremental revalidation from per-script and per-map digests; `export --to-store` plus `cargo xtask checkpoints promote`. |
| 11 | French text and offsets | Everything stays located by anchors and pointer tracing (no BPEF offset constants). New C-side tables (items, multichoice lists, std strings, map sections) are located the same way, checked on BPEE against the decomp, and every exported string must re-encode to its ROM bytes. |
| 12 | Overrides | `crates/content/src/overrides/<Name>.rs` replaces a generated script by name (committed Rust, text by id only). Data patches and studio write-back are M9+. |
| 13 | Tests at scale | Debug tests use fixed samples of maps and ids; full sweeps (every map, every script root) are release-only: `cargo xtask sweep` and `explore sweep`. |
| 14 | pokeemerald-expansion | Not used as a source (it targets later-generation mechanics). It is at most a readable reference when a vanilla C routine is unclear; vanilla `pokeemerald` is the behaviour spec. |

---

## Current code this builds on

- **Extract:**
  - `crates/extract/src/export.rs`: `MAPS` `:25` (15 maps; its order decides canonical names), `export :181` (map loop `:193`, `battle_data::extract(rom, &MAPS)` `:352`, `Manifest.maps` `:375`), `export_scripts :156`.
  - `codegen.rs`: the module doc (`:1-44`, generated-file contract), `Stats :64` (`summary :91`), `Generated :117`, `header :128`, `generate(rom, maps, header) :225` (decode, labels, functions, lowering in one pass), `emit_insn :804` (the lowered commands).
  - `scripts.rs`: `OPS :172` (all 0xE3 opcodes), `decode :456`, `walk :504`. `roots.rs`: `named_roots :99`, `canonical :141`. `schema.rs`: `FORMAT_VERSION = 4` and the naming rules (`:1-56`).
  - `battle_data.rs`: wild headers, one per map (`ensure!` "two wild headers", `:1073`).
  - `text.rs`: `decode_lang :124`, `encode_lang :216`, `round_trip_at :384`. `names/generated.rs`: decomp tables (`MAPS`, `FLAGS`, `VARS`, `SPECIALS`, …), drift-tested against `PKR_DECOMP`.
- **Content build** (`crates/content/build/`):
  - `main.rs`: the build entry; stub fallback; `BUILD_ID`.
  - `codegen.rs` (774 lines): `consts.rs`, `data.rs` (maps, tilesets, sprites, texts as Rust literals) and `scripts.rs` (one `include!` per map), plus `SCRIPT_NAMES :721`.
  - `battle.rs`: the slice set `slice_uses :112`, `slice_species :128`, `slice_moves :134`, `gen_slice :811`, the unworded stat check `:454`, `PLAYER_LEVEL_CAP :31`.
  - `classify.rs`: `behavior :71`, `movement :109`, `gfx :142`, `ability :280`, `move_effect :317`.
  - `input.rs`, `stub.rs`, `build_id.rs`.
- **Content runtime:** `crates/content/src/prelude.rs`: `msgbox`, `callstd :69`, `special :98` (three implemented), `trainer_battle`, `check_player_gender`, `unsupported :220`. `new_game.rs`, `blob.rs` (`Blob`, used by `battle.bin`).
- **Engine:**
  - `script.rs`: `Request :63`, `Ctx :304` (`flag` … `set_respawn :545`), `Runner :583-729`.
  - `world/host.rs`: request handling (`:45-320`). `world.rs`: `step :289`. `world/checkpoint.rs`: `capturable` with `StaleView :50`.
  - `data.rs`: `id!` `:43`, `MovementType :171`, `ObjectEvent` (`trainer_type`, `trainer_sight` `:197-198`, unused), `MapData :268`, `NewGame :322`, `GameData :351-410`.
  - `checkpoint.rs`: `SCHEMA = 1` `:20`, `WorldSnapshot :104`. `debug.rs`: `PKRDBG02`, 200 bytes. `trainer.rs`: trainer parties only (no sight).
- **Native, xtask, explorer, studio:**
  - `game-native/src/scenario.rs`: `EMERALD_ROUTE :776`, `cached_route :882`.
  - `xtask/src/budget.rs`: `BASELINE :34` (1,992,712 / 596 / 460), `GROWTH` (+64 KiB / +2 KiB / +16 KiB).
  - `crates/explore/src/goals.rs`: `GOALS` real `:56` (7) and stub `:97` (3). `key.rs`, `probe.rs`, `schema.sql` (v2).
  - Studio: `map_view.rs`, `data_view.rs`, `tests/{map_pixels, map_view, data_views}.rs` iterate every exported map or table.
- **Committed trees:** `checkpoints/emerald/nodes` c00001–c00004, `checkpoints/stub/nodes` c00001–c00003.
- **Files naming the slice** (`MAP_IDS`, `export::MAPS`, `SLICE_*`, `slice_species`, `manifest.maps`): `content/build/{battle,battle_load,codegen,input}.rs`, `content/src/{battle_tests,data_tests,lib}.rs`, `explore/src/cli.rs`, `explore/tests/f_policy.rs`, `extract/src/{roots,schema}.rs`, `extract/tests/{codegen,text}.rs`, `studio/src/{data_view,map_view,play_view}.rs`, `studio/tests/{data_views,map_pixels,map_view}.rs`.

---

## Wave 0 contracts (committed by you before Wave 1; read them, do not redesign them)

Wave 0 makes no feature work. It does four things:
- the spike, re-run on the merged tree, whose numbers fix decision 7;
- every new type, file and signature, with `todo!()` bodies or safe defaults;
- the module splits that let Wave 1 run without shared files;
- the one hash change (new state), with the golden update and the tree re-accept.

`cargo test --workspace` (real and stub), `cargo xtask rom`, `cargo xtask parity` and `explore` stay green after Wave 0.

### Step 0 — reconcile and baseline
1. Rebase on master after the M7 close-out (Tasks H and S2 and Wave 3 merged). Read M7's final "As built" and its "Findings for Milestone 8". If the close-out already handled an item in the findings table at the end of this doc, mark it there and drop it from its task.
2. **Re-run the spike** on the merged tree, on a throwaway branch: all maps, a scratch extracted dir, native debug and GBA release builds at content opt-level 3 and "s". Record in "Wave 0 as built":
   - export time;
   - extracted size with and without previews;
   - native build time and peak RSS;
   - ROM `.text` and `.rodata`, and the size by symbol prefix (`content::scripts`, `content::data`, `content::battle`, `engine`, other);
   - the largest script future.
3. **Decide `content`'s GBA opt-level.** Take "s" if it saves at least 15% of ROM and the slice route's `loop_lines_max` stays within its gate (spike: −29% `.text`). Set it as `[profile.release.package.content] opt-level = "s"` in `crates/game-gba/Cargo.toml`.
4. **Baselines on master:** slice route parity (hashes, pictures, `late`, lines); `explore run --secs 60` (nodes/s, db MB per 1k rows); `cargo test --workspace` wall time in both flavors.

### Extract contracts (`crates/extract`)
- **`schema.rs`: `FORMAT_VERSION = 5`.** The layout doc gains:
  ```text
  scripts/<Module>.ron       ScriptFile   (IR; <Module> = decomp source: a map dir name, or
                                           Common_<stem> for data/scripts/<stem>.inc)
  scripts/index.ron          ScriptIndex v5 (name -> module, root, origin, offset)
  layouts/<Layout>.ron       LayoutFile   (every layout, incl. ones only setmaplayoutindex uses)
  tables/items.ron           ItemsFile    (all ITEM_*: name, pocket, price, importance)
  tables/multichoice.ron     MultichoiceFile (MULTI_* -> list of strings)
  tables/std_strings.ron     StdStringsFile  (STDSTRING_* -> string)
  tables/map_sections.ron    MapSectionsFile (MAPSEC_* -> name)
  sprites/field_effects/<Name>.{png,ron}  emote pics (exclamation, question, heart)
  battle/encounters.ron      EncountersFile v5: per map a Vec of headers, each with
                                           land / water / rock_smash / fishing (Option each)
  previews/                  only with `cargo xtask extract --previews`
  ```
- **The IR** (`schema.rs`; implemented types with serde, no logic):
  ```rust
  /// One decomp source file's scripts, in source order.
  pub struct ScriptFile { pub module: String, pub source: Option<String>, pub labels: Vec<IrLabel>,
                          pub movements: Vec<IrMovement> }
  /// A label and its straight-line block, up to the next label or a terminator.
  pub struct IrLabel { pub name: String, pub origin: NameOrigin, pub offset: Hex,
                       pub root: Option<RootKind>, pub insns: Vec<IrInsn>,
                       pub next: Option<String> /* fallthrough label */ }
  pub enum NameOrigin { Decomp, Positional }
  pub enum RootKind { Object(u8), Coord(u16), Bg(u16), MapScript(String /* OnLoad … */) }
  /// One decoded command: its `script_cmd_table.inc` macro name and named operands.
  pub struct IrInsn { pub op: String, pub args: Vec<IrArg> }
  pub enum IrArg {
      Byte(u8), Half(u16), Word(u32),
      Flag(String), Var(String),        // FLAG_*/VAR_* name, or FLAG_0x123 / VAR_0x4123
      Const { table: String, name: String },   // SPECIAL_*, STD_*, ITEM_*, SPECIES_*, MOVE_*, TRAINER_*, MAP_*, OBJ_EVENT_GFX_*, MULTI_*, STDSTRING_*, …
      Script(String), Text(String), Movement(String), Braille(String),
      Raw(u32),                          // VAddress/VScript/VText and other unnamed words
  }
  pub struct IrMovement { pub name: String, pub origin: NameOrigin, pub actions: Vec<String> /* MOVEMENT_ACTION_* */ }
  pub struct ScriptIndex { pub scripts: BTreeMap<String, ScriptEntry>, pub texts: BTreeMap<String, String /* module */> }
  pub struct ScriptEntry { pub module: String, pub root: bool, pub origin: NameOrigin, pub offset: Hex }
  ```
  The IR is a decoded listing, close to the decomp's `.inc`. Functions, loops and tail calls are decided by the lowering, not by the IR.
- **Module moves** (mechanical, no behaviour change):
  - `export_scripts` and `ScriptInput` move from `export.rs` to `export/scripts.rs` (Task B).
  - New `export/tables.rs` with `pub fn export_tables(rom, out, provenance) -> Result<()>`, an empty body that `export` calls (Task C).
- **`codegen.rs`, new entry point** (Task D; stub). `generate` keeps working until Task D swaps the build over:
  ```rust
  pub enum Class { Native, Ignore, Default(u16), Unsupported }
  pub struct Support { pub commands: BTreeMap<String, Class>, pub specials: BTreeMap<String, Class> }
  pub struct Lowered {
      pub modules: BTreeMap<String, String>,       // module -> Rust source for $OUT_DIR
      pub stats: Stats,                            // Stats gains `by_class`, `reports`, `named_decomp_pct`
      pub script_digests: BTreeMap<String, u64>,   // FNV of each fn's lowered source
      pub trainer_scripts: Vec<(String /* script */, String /* TRAINER_* */)>,
  }
  /// IR -> Rust. No ROM access. `overrides`: script names whose body is crate::overrides::<Name>.
  pub fn lower(files: &[ScriptFile], index: &ScriptIndex, support: &Support,
               overrides: &BTreeSet<String>) -> anyhow::Result<Lowered>;
  ```
- **xtask `extract`** accepts `--maps all|<Dir,Dir,…>` (default `all` from Task A on) and `--previews`. Wave 0 parses the flags and passes them through; `export` takes `&ExportOptions { maps: Vec<&'static str>, previews: bool }`, with Wave 0's default being today's 15 maps.

### Content contracts (`crates/content`)
- **Splits** (mechanical):
  - `build/classify.rs` → `build/classify/{mod.rs, field.rs (behavior, movement, gfx), battle.rs (ability, move_effect, effect_stat)}`.
  - `src/prelude.rs` → `src/prelude/{mod.rs (re-exports, unsupported, report), msgbox.rs, std.rs (callstd), special.rs, trainer.rs, items.rs (empty), text.rs (empty), menu.rs (empty)}`.
- **New build files:**
  - `build/scripts.rs` (Task D): `pub fn generate(dir: &Path, out: &Path, w: &World) -> Result<ScriptsOut>`, a stub that runs today's path.
  - `build/blob_data.rs` (Task E): stub.
  - `build/support/commands.ron` and `build/support/specials.ron`: generated once in Wave 0 from the decomp names. Every command today's codegen lowers is `Native`; everything else is `Unsupported`.
  - `build/main.rs` calls both stubs.
- **`src/overrides/mod.rs`:** `pub static OVERRIDES: &[&str] = &[];`, with a doc comment on the convention (one file per script, `pub async fn <Name>(ctx: Ctx)`, texts and movements by generated id).
- **Generated by the build (stubs in Wave 0, filled by D and E):** `content::data::{SCRIPT_DIGESTS: &[(ScriptId, u64)], MAP_DIGESTS: &[(MapId, u64)], GLOBAL_DIGEST: u64}` (battle tables, items, text tables), and `content::REPORT: &str` (the build's classification report, `include_str!` from `$OUT_DIR`).

### Engine contracts (`crates/engine`)
- **`data.rs`:**
  - `id!` gains `ItemId`;
  - `pub struct ItemInfo { pub name: &'static [u8], pub pocket: Pocket, pub price: u16, pub key: bool }`;
  - `pub enum Pocket { Items, KeyItems, PokeBalls, TmHm, Berries }` (Gen3 order).
  - `GameData` provided methods (defaults `None`):
    ```rust
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo>;
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]>;
    fn std_string(&self, id: u8) -> Option<&'static [u8]>;
    /// The trainer flag a trainer object's script checks (Gen3 `GetTrainerFlagFromScriptPointer`).
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId>;
    ```
- **`bag.rs` (new):** `Bag` with Gen3 Emerald pocket capacities (items 30, key items 30, balls 16, TMs/HMs 64, berries 46; 99 per slot). It keeps an O(1) digest like `Vars`. `new`, `count`, `hash`, `snapshot`/`from_snapshot` are implemented; `add`, `remove` and `has_space` are `todo!()` (Task H).
- **`World` state** (implemented, hashed, snapshotted):
  - `bag: Bag`;
  - `money: u32`;
  - `str_vars: [[u8; STR_VAR_LEN]; 3]` (`STR_VAR_LEN` 32, 0xFF-terminated, Gen3 `gStringVar1..3`);
  - `warp_slots: WarpSlots { dynamic, escape: Option<WarpDest> }`;
  - `field.stale: Option<StaleRect>`, always `None` until Task F.

  `World::hash` appends them in this order. Accessors: `bag()`, `money()`, `str_var(i)`, `last_script() -> Option<ScriptId>`.
- **`checkpoint.rs`:** `SCHEMA = 2`. `WorldSnapshot` gains `bag: BagSnap`, `money: u32`, `str_vars: [Vec<u8>; 3]`, `warp_slots: WarpSlotsSnap`. `FieldSnap` gains `stale: Option<StaleSnap>`. The golden-bytes test is updated, `MAX_ENCODED` is unchanged, and old schema-1 snapshots are `Stale` and regenerated by replay (M5 decision 6).
- **`script.rs`, new `Request`s and `Ctx` methods.** Wave 0 host handlers return safe defaults: false, 0, or 127 (`MULTI_B_PRESSED`) for multichoice. Nothing panics.
  ```rust
  ItemAdd { item: ItemId, qty: u16 } -> Bool        // giveitem/additem: true if it fit
  ItemRemove { item: ItemId, qty: u16 } -> Bool
  ItemCheck { item: ItemId, qty: u16 } -> Bool
  ItemSpace { item: ItemId, qty: u16 } -> Bool
  MoneyAdd(u32), MoneyRemove(u32), MoneyCheck(u32) -> Bool
  Buffer { slot: u8, src: BufferSrc }                // BufferSrc::{Species, Item, Move, Number(u32), Std(u8), PartyNick(u8), LeadSpecies, Text(TextId)}
  Multichoice { list: u8, x: u8, y: u8, default: u8, cancel: bool } -> Value
  SetWarp { slot: WarpSlot, map: MapId, warp: Option<u8>, x: i16, y: i16 }   // WarpSlot::{Dynamic, Escape}
  Report(Report)                                     // Report::{Unsupported(&'static str), Degraded(&'static str)}
  ```
  `Ctx` also gains `cond()`/`set_cond()`: the script's comparison register as plain `Io` state, so `cond` can flow across calls as Gen3's `comparisonResult` does. It is not hashed; capture requires an idle runner anyway.
- **`world/host.rs` → `world/host/{mod.rs, items.rs (H), menu.rs (H), buffers.rs (H), trainer.rs (I), warps.rs (F)}`.** Each new request is routed to its file, whose stub answers the default.
- **Events** (implemented; not hashed, not snapshotted):
  ```rust
  pub enum Event { MapEntered(MapId), ScriptStarted(ScriptId), BattleStarted, BattleEnded(BattleOutcome), Report(Report) }
  impl World { pub fn events(&self) -> &[Event] }   // this step's events; cleared at the start of `step`
  ```
  They are pushed where the World loads a map, calls `game.script(id, …)`, enters and leaves `Mode::Battle`, and handles `Request::Report`. Capacity is 16 per frame; extras are dropped and counted (`events_dropped`).
- **Hooks with no behaviour yet:**
  - `world.rs` calls `sight::check(…)` after the player's step-end triggers (`engine/src/sight.rs`, new, returns `None`; Task I);
  - `field.rs` exposes `Field::stale()` (`None`; Task F).
- **The hash change.** Update the goldens in `world/tests.rs` and the README, then `cargo xtask checkpoints verify --accept` on both trees (`chore(checkpoints)`). Run `cargo xtask parity`: hashes must match between ROM and native, and lines must stay within the gates.

### Native, xtask, explorer contracts
- `game-native`: none (the `Session` API is unchanged).
- **xtask:** `mod sweep;` with `Some("sweep") => sweep::main(&args[1..])` (Task L). `budget.rs` gains `pub fn sections(elf: &Path) -> Result<Vec<(String, u64)>>` (stub; Task M).
- **explore:** no Wave 0 change. Task K owns the crate.

---

## Rules for every task

- **Workspace:** work in your own git worktree. Commit with `feat(<crate>): …` or `fix(…)`, and **no attribution or co-author lines**.
- **TDD:** write a failing test first.
  - Engine tests use the synthetic games (`world/tests.rs`, `battle/test_data.rs`); do not add another `GameData` shim.
  - Extract tests run on both ROMs (`PKR_ROM_FR`, `PKR_ROM_US`) and the decomp (`PKR_DECOMP`).
  - Content tests run on whatever the build has (real or stub) and on fixture extracted dirs (`build_modes.rs` style).
- **Test budgets at scale:**
  - A debug test touches at most about 20 maps or 20k simulated frames.
  - Tests that sweep every map, script or species are `#[ignore]` and run in release through `cargo xtask sweep`.
  - Fixed samples are named lists of decomp constants, never "the first N".
- **Determinism:**
  - no floats, clocks or `HashMap` iteration in `engine`/`content`/`explore` decisions;
  - generated code and IR are written in a defined order (`BTreeMap`, source order);
  - the build is reproducible: building twice gives byte-identical `$OUT_DIR` files.
- **Game-agnostic engine:** the engine names no Emerald constant. Roles come through `GameData` (`special_ids`, `item`, `trainer_flag_of`, …).
- **Degradation, not panics:** a missing feature becomes a `Report` plus a documented default. A `panic!`/`unreachable!` reachable from content is a bug.
- **Never commit ROM-derived data:**
  - the IR, generated Rust, extracted tables, pics, `explore.db`, repro traces and snapshots stay git-ignored;
  - committed: generators, `commands.ron`/`specials.ron` (decomp names and classes only), overrides (Rust with ids, no text literals), node files (inputs, hashes, digests);
  - **no FR text literals in tests**: assert counts, ids, decomp names, round trips.
- **Hash:**
  - Wave 0 changes the hash once.
  - A task whose change alters the slice route (F's map loads, G's battles, I's sight) says so in its report, re-records with `cargo xtask checkpoints seed` if the route keys change, and lands `verify --accept` as a separate `chore(checkpoints)` commit stating the cause.
  - Any other task leaves goldens, trees and parity untouched.
- **GBA:** `late == 0`; bulk uploads only on black frames or spread under the line budget; the budgets of `cargo xtask rom` (new baseline from Wave 0, then gated).
- **Before committing:**
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo clippy --workspace --all-targets --features game-native/window,studio/ui -- -D warnings`
  - `cargo test --workspace` with `PKR_ROM_FR=/Users/dode/Documents/pokemon/pokemon-rust/rom/emeraude-fr.gba PKR_ROM_US=/Users/dode/Documents/pokemon/emerald.gba PKR_DECOMP=/Users/dode/Documents/pokemon/pokeemerald`
  - `PKR_EXTRACTED_DIR=/nonexistent cargo test --workspace` (stub)
  - `cargo test -p studio --features ui` if you touched `studio`
  - `cargo xtask rom` and `cargo xtask parity` if you touched anything the GBA build compiles (engine, content, game-gba) or `emu`
  - `cd crates/game-gba && cargo fmt --check && cargo clippy -- -D warnings` if you touched game-gba
- **Extracted data in Wave 1:** Tasks A–C change what `extract` writes. Every other Wave 1 task works on the **Wave 0 extract** (`cargo xtask extract`, 15 maps, format 5) plus its own fixtures. Integration onto the full import is Task L.
- **Ownership:** only touch the files your task owns, plus new test files named `<crate>/tests/m8_<task>_*.rs`. Describe any other change you need in your report.

---

## Wave 1 (parallel, 11 agents)

### Task A — extract: every map, layout, tileset, sprite, encounter and pic

**Owns:** `crates/extract/src/{export.rs, maps.rs, sprites.rs, render.rs, battle_data.rs (encounters, heal locations), battle_gfx.rs (pics)}`, `crates/extract/tests/{export.rs, maps.rs, sprites.rs, battle_data.rs, battle_gfx.rs}`, and `extract()` in `xtask/src/main.rs`.

1. **Maps:**
   - `ExportOptions::maps` defaults to every `names::MAPS` entry in `gMapGroups` order (`--maps` restricts it; fixtures use it). `export::MAPS` is deleted.
   - Write every layout (`layouts/`), including those reachable only through `setmaplayoutindex` operands. Scan the IR walk's `Const { table: "LAYOUT" }`, or simply export all 442.
   - Write every tileset, and `MAP_DYNAMIC` warps as such.
2. **Sprites:** every OBJ gfx id placed by any map or reached through `VAR_OBJ_GFX_ID_*` in any script (`var_gfx_ids` over all maps), plus both player sprites, plus the field-effect emote pics (`gFieldEffectObjectPic_ExclamationMark`, `…QuestionMark`, `…HeartIcon`) under `sprites/field_effects/`. Anchors as for object graphics; BPEE checked against the decomp's `graphics/field_effects/pics/*.png` dimensions.
3. **Encounters (schema v5):**
   - every wild header, in table order, as a `Vec` per map: Altering Cave's 9 sets, each keyed by its index (Gen3 `VAR_ALTERING_CAVE_WILD_SET`);
   - land, water, rock smash and fishing (with the old/good/super rod slot split, `gWildFishingSlots`).
4. **Pics:** front, back and shiny palette for **every** species with ROM graphics (Unown keeps form A, documented), and every trainer front and back pic that any trainer references. This replaces the slice set.
5. **Previews:** off by default (`--previews` writes them). The extracted dir drops from 144 MB to about 23 MB.
6. **Performance:** `cargo xtask extract` stays under 30 s in release (the spike: 5.5 s without the IR).
7. **Tests:**
   - BPEF and BPEE export with `--maps all`: 518 maps, 73 tilesets, the layout count, and the sprite count equal to the decomp's referenced set;
   - Altering Cave has 9 headers;
   - every map's connections name exported maps;
   - every exported string round-trips;
   - `--maps LittlerootTown,Route101` exports exactly what those maps reference;
   - two exports are byte-identical (manifest sha1s).

### Task B — extract: script IR and decomp label transfer

**Owns:** `crates/extract/src/{scripts.rs, roots.rs, movement.rs, export/scripts.rs}`, new `crates/extract/src/{inc.rs, labels.rs}`, `crates/extract/tests/{labels.rs, script_ir.rs}`.

1. **`inc.rs`, a decomp source reader** (not an assembler):
   - For every `data/maps/*/scripts.inc` and `data/scripts/*.inc`, read the labels in source order. For each label: its file, its kind (script, text, movement, data), its fallthrough successor, and the **sequence of label references** in its body, in textual order, each with the operand kind implied by its macro (`goto`/`call`/`goto_if_*`/`call_if_*`/`trainerbattle_*` → script; `msgbox`/`message`/`loadword` → text; `applymovement` → movement).
   - From `data/maps/*/map.json`, read the object, coord and bg event script labels and the `map_scripts` table label.
   - Macro knowledge stays a small table of reference-bearing macros (about 40, from `asm/macros/event.inc`); every other line contributes no reference.
2. **`labels.rs`, the transfer:**
   - Positional roots (`<Map>_Obj<n>`, `_Coord<i>`, `_Bg<i>`, map-script roots) map to `map.json` labels by index.
   - Then a parallel walk: for each decoded straight-line block and its decomp label, align the block's label operands (in instruction order, by kind) with the label's reference sequence. Equal length and kinds give `offset → label` for every target; follow fallthrough by the next-label chain.
   - A mismatch leaves that block's targets `Positional` and is counted per module.
   - Texts and movements take their decomp labels the same way.
   - The result is `BTreeMap<u32 /* offset */, (String, NameOrigin)>`. On a collision (one offset, two labels, as with shared tails), the first label in source order wins and the rest are recorded.
3. **IR emission (`export/scripts.rs`):**
   - decode as today (`scripts::walk`), then group by label and resolve operands to names (`IrArg`);
   - assign each label to its decomp module (`<Map>` or `Common_<stem>`; positional ones stay in the first referencing root's map module);
   - write `scripts/<Module>.ron`, `scripts/index.ron` and `text/<lang>.ron` keyed by the final names.
   - `export` no longer writes `.rs` files once Task D lands. Until then, keep the old output alongside: Wave 0's `generate` keeps compiling.
4. **Without `PKR_DECOMP`:** every name is positional, with one warning.
5. **Tests (BPEF and BPEE, all maps):**
   - at least 95% of functions, texts and movements are `Decomp` (print the % per module);
   - every map.json event label is a root;
   - the slice's known roots get their map.json labels (asserted by lookup in map.json, no literals);
   - names do not depend on `--maps` order (export two orders, compare the index for shared maps);
   - the IR round-trips through RON;
   - the decoded instruction count equals today's walk.

### Task C — extract: item, multichoice, std-string and map-section tables

**Owns:** `crates/extract/src/export/tables.rs`, new `crates/extract/src/tables.rs`, `crates/extract/tests/tables.rs`. It may add charmap entries to `text/charmap.rs` if an FR string needs one (listed in its report).

1. **Items** (`gItems`, `src/data/items.h`): for every `ITEM_*`, the name (FR), pocket, price and importance. Anchor: the struct array's stride and the known `ITEM_POTION`/`ITEM_POKE_BALL` pocket and price pattern, validated on BPEE against `items.h`.
2. **Multichoice lists** (`sMultichoiceLists`, `src/data/script_menu.h`): `MULTI_*` → its strings, in order.
3. **Std strings** (`gStdStrings`, `bufferstdstring`) and **map section names** (`gRegionMapEntries` names, `MAPSEC_*`).
4. Every string re-encodes to its ROM bytes (`round_trip_at`). Counts equal the decomp's.
5. **Tests:** BPEE equality with the decomp tables (names compared through `decode_lang(En)`); BPEF counts and round trips; no FR literals.

### Task D — content: lowering in the build, support tables, overrides, digests

**Owns:** `crates/extract/src/codegen.rs`, `crates/extract/tests/codegen.rs`, `crates/content/build/scripts.rs`, `crates/content/build/support/commands.ron`, `crates/content/src/overrides/**`, `crates/content/tests/m8_d_*.rs`.

1. **`codegen::lower`:**
   - Port today's structuring (functions, `loop { match pc }`, tail calls) from decoded `Insn`s to the IR.
   - `generate` becomes `decode → IR → lower` for the old path, and is deleted once the build uses `lower`.
2. **`build/scripts.rs`:** read `scripts/*.ron` and the index, `support/*.ron` and the override list (file names under `src/overrides/`), call `lower`, and write `$OUT_DIR/scripts/<Module>.rs`, `scripts.rs` (one `pub mod` per module, `include!` from `OUT_DIR`), the digests and `report.txt`.
3. **Commands newly native**, through the Wave 0 requests and the prelude helpers of Tasks H, I and J (call them by their Wave 0 names):
   - `multichoice`, `multichoicedefault` (`multichoicegrid` stays `Unsupported`);
   - `giveitem`/`additem`/`removeitem`/`checkitem`/`checkitemspace`/`finditem`;
   - `checkmoney`/`addmoney`/`removemoney`;
   - every `buffer*` with a `BufferSrc`;
   - `settrainerflag`/`checktrainerflag`/`cleartrainerflag`;
   - all ten `trainerbattle` types (doubles go to the prelude's degrade);
   - `setdynamicwarp`/`setescapewarp`/`setwarp`;
   - `setwildbattle` + `dowildbattle` (a local pending battle in the function);
   - `messageautoscroll`/`messageinstant` (as `message`);
   - every `copyvar`/`loadword` form;
   - `setobjectmovementtype` everywhere.
4. **`Ignore`:** audio (`playse`, `waitse`, `playfanfare`, `waitfanfare`, `playmoncry`, `waitmoncry`, `playbgm`, …), `incrementgamestat`, `setweather`/`doweather`/`resetweather`, the money and coin boxes, `showmonpic`/`hidemonpic`, `setflashlevel`/`animateflash`, `dotimebasedevents`, `setstepcallback`, map popups. Each gets a one-line reason in `commands.ron`.
5. **Everything else is `Unsupported`:** `ctx.report(Report::Unsupported("<op>"))`, and the documented result (VAR_RESULT unchanged unless the command writes it, in which case the class gives `Default(v)`).
6. **Gen3 semantics fixed:**
   - `cond` across calls through `Ctx::cond`;
   - a callee's `end` stops its caller (the 80 sites): callees return `Flow::{Continue, End}`, and roots wrap them;
   - `waitmovement 0` stays "last mover", and the 80 unresolved sites are checked.
   - **Unknown movement actions:** map the 15 kinds to the closest `Step`, or to `Delay` with the frame count of the Gen3 action, listed in `commands.ron`'s movement section.
7. **Overrides:** a name in `OVERRIDES` lowers to `pub async fn <Name>(ctx: Ctx) { crate::overrides::<Name>(ctx).await }`. An override naming an unknown script fails the build. Ship one override as the worked example (an end-in-callee site whose structure is clearer by hand), with a test.
8. **Build checks:** an op or special in the IR missing from `support/*.ron` fails the build with the list. `report.txt` gives the counts per class and per op, and the `Decomp` name %. One `cargo::warning` summary line.
9. **Digests:** `SCRIPT_DIGESTS` (FNV of each function's lowered source plus the IR digest of the texts and movements it references) and per-module digests, for Task N.
10. **Trainer scripts:** `trainer_scripts` → content's `GameData::trainer_flag_of` table (generated into `scripts.rs`).
11. **Tests:**
    - lowering fixtures (IR written by hand in tests: structured flow, tail calls, `end` in callee, `cond` across a call, every newly native op);
    - `build_modes.rs`-style fixture dirs: an unclassified op fails, an override replaces, an unknown override fails;
    - the slice's generated scripts behave identically: the parity route replay hashes are equal on the Wave 0 extract, and only names differ when B's names arrive (L checks this on the full import).

### Task E — content: blob data, no slice sets, classification at scale

**Owns:** `crates/content/build/{codegen.rs, input.rs, battle.rs, battle_load.rs, blob_data.rs, classify/field.rs, main.rs, stub.rs}`, `crates/content/src/{lib.rs, blob.rs, data_tests.rs, battle_tests.rs, new_game.rs}`, and the `SLICE_*` call sites in `crates/explore/tests/f_policy.rs` and `crates/studio/src/data_view.rs` (call sites only, listed in the report).

1. **Blobs:** map blocks and layouts, tileset tiles, metatiles and attributes, palettes, sprite frames, font glyphs and text bytes go into `$OUT_DIR/data.bin` (aligned per item, `Blob` slices), next to `battle.bin`. `data.rs` keeps headers, events, connections, ids and `&'static` slices into the blob.
   - **Acceptance on the spike's full extract:** `data.rs` ≤ 2 MB, a native debug build of `content` in ≤ 20 s, and rustc peak RSS ≤ 2.5 GB. Record all three.
2. **No slice:**
   - `slice_uses`, `slice_species`, `slice_moves`, `gen_slice`, `PLAYER_LEVEL_CAP`, `SLICE_SPECIES` and `SLICE_MOVES` are deleted. Pics are compiled for every species and trainer exported.
   - The unworded-stat-effect `ensure!` becomes a per-move fallback (the effect is `Unsupported` until G gives it words) and is listed in the report.
   - Consumers switch to fixed `MOVE_*`/`SPECIES_*` constants.
3. **New tables:**
   - `ITEMS`, `MULTICHOICE`, `STD_STRINGS` and `MAP_SECTIONS` from Task C's files (Wave 0 schema; a fixture until C merges), behind `GameData::{item, multichoice, std_string}`;
   - encounters v5 (`encounters(map)` returns the active header: the engine passes `VAR_ALTERING_CAVE_WILD_SET` through `special_ids`; water and fishing tables are compiled now and used by M10).
4. **`classify/field.rs`:** every one of the 256 behaviours classified, with the approximation list in the report; movement types mapped onto Task I's new `MovementType` variants (until I merges, keep today's mapping behind one function).
5. **`MAP_DIGESTS`** (FNV of each map's compiled data) and `GLOBAL_DIGEST` (battle tables, items, text tables, behaviour table).
6. **Tests:**
   - the slice's `data.rs` content is equal before and after the blob move: every map's blocks, tiles and texts compared through the `GameData` API against a snapshot taken by the test from the Wave 0 build, so the parity route hashes are unchanged;
   - a fixture dir with 2 maps sharing a tileset dedupes it in the blob;
   - stub mode unchanged.

### Task F — engine: field at scale

**Owns:** `crates/engine/src/{field.rs, field/*, flow/transition.rs, flow/trigger.rs, grid.rs, world/checkpoint.rs, world/host/warps.rs}` and their tests.

1. **Warp loads spread over black frames** (Gen3 `LoadMapInStepsLocal`): tilesets, palettes and the view are uploaded in chunks under the line budget, so `late_black` goes to 0 (M4 finding).
2. **Crossings into a different tileset:** the incoming secondary tileset and palettes are uploaded in chunks while the new map's cells are still off screen. No visible-frame overrun.
   - Test: a synthetic two-map world with different secondary tilesets: no frame's upload exceeds the per-frame byte budget, and pictures equal a direct full upload once the crossing completes.
   - ROM check in Task M's tour.
3. **Stale view capturable** (M7 finding):
   - After a crossing, `FieldSnap.stale` records the still-visible old-map rect.
   - `upload_all` redraws it from the old map's data, overrides included, which also fixes M5's decision 15 risk.
   - `NotCapturable::StaleView` is no longer returned; the variant stays for decode compatibility.
   - The M5 core invariant tests cover crossing frames.
4. **Dynamic and escape warps:** `SetWarp`, and `MAP_DYNAMIC` warp destinations read `warp_slots.dynamic`.
5. **Directional impassable behaviours** (`MB_IMPASSABLE_{EAST,…}` and the diagonal ones) block only the matching direction.
6. **Old-map objects after a crossing** (M3 finding): talking to one sets `selected`/`VAR_LAST_TALKED` as Gen3 does (`GetObjectEventIdByPosition` does not filter by map).
7. **Not in F:** door animation graphics, elevation-based OBJ priority on bridges, and the one-frame-early step triggers. These are documented as M10 (Act 1 does not need them).
8. **Hash and route:** items 1 and 2 change frame timing on warps, so the slice route changes. Re-seed or verify, then `--accept` in a separate commit, citing the M4 finding.

### Task G — engine: battle breadth for Act 1 and beyond

**Owns:** `crates/engine/src/battle/{effects/*, core.rs, core/*, calc.rs, ai.rs, text.rs, mod.rs}`, `crates/engine/src/mon.rs` (evolution only), `crates/content/build/classify/battle.rs`, and their tests.

1. **Effects:**
   - Implement exactly (Gen3 `battle_script_commands.c` + `data/battle_scripts_1.s`) every effect used by:
     - a move that any Act 1 encounter mon (Routes 101–104, Petalburg Woods, Rustboro) knows by its level-up learnset at its max level;
     - any move of a trainer party on those maps, including Roxanne's.
   - Then implement more, by frequency across all trainer parties and learnsets, until **≥ 120 of 214** effects are implemented. Order: `HIGH_CRITICAL`, `ALWAYS_HIT`, `TRAP`, `DOUBLE_HIT`, `PROTECT`, `RESTORE_HP`, `LEVEL_DAMAGE`, `RECHARGE`, `DEFENSE_CURL`, `MEAN_LOOK`, `YAWN`, `ATTACK_UP_HIT`, … (Task G prints the frequency table in its report).
   - Unworded stat effects get their Rose/Fell `Str`s (closing E's fallback).
2. **Abilities:** classify all 77. Implement every ability of an Act 1 species or trainer mon (Sturdy, Rock Head, Magnet Pull's battle part, Truant, Huge Power, Thick Fat, Shed Skin, Natural Cure, Cute Charm, Early Bird, Liquid Ooze, Clear Body, Soundproof, …), plus the top by frequency, to **≥ 40 of 77**. Weather-dependent ones (Swift Swim, Chlorophyll, Rain Dish) are no-ops until weather exists, and that is documented.
3. **Double trainer battles:** a `BattleTrainer` with a double party runs as singles (open question 3), with `Report::Degraded("double battle")`.
4. **Prize money:** Gen3 `GetTrainerMoneyToGive` into `World::money` on a win, with its battle string. Whiteout money loss is Gen3 `GetMoneyLossOnWhiteout` (badges × level).
5. **Evolution after battle** (level, no item, no trade, no friendship): the species changes and stats recalculate, silently, with `Report::Degraded("evolution scene")`. The scene is M9.
6. **Tests:** golden roll tests per effect and ability (explicit `Rng` rolls, as M4); one Act 1 trainer party end to end in the battle test game; AI flags for Roxanne's party.
7. **Route:** if the slice route's battles change, refresh the trees as the rules say.

### Task H — engine: items, money, string buffers, multichoice

**Owns:** `crates/engine/src/{bag.rs, menu.rs (new), world/host/{items.rs, menu.rs, buffers.rs}, text.rs (placeholders), text/printer.rs (placeholders)}`, `crates/content/src/prelude/{items.rs, menu.rs, text.rs}`, and their tests.

1. **`Bag`:** Gen3 `AddBagItem`/`RemoveBagItem`/`CheckBagHasSpace`/`CheckBagHasItem`: pockets by `ItemInfo::pocket`, 99 per slot, capacity per pocket, order kept. Its digest is O(1) per change.
2. **Money:** capped at 999,999 (Gen3 `MAX_MONEY`).
3. **String buffers:** `Buffer` fills `str_vars[slot]` from species, item and move names, `ConvertIntToDecimalString` numbers (Gen3 `STR_CONV_MODE_LEFT_ALIGN`), std strings, party nicknames and the lead species. `{STR_VAR_1..3}` placeholders expand in the printer (today they expand to nothing).
4. **Multichoice:** a Gen3 list window (`ScriptMenu_Multichoice`: position, width from the longest string, cursor, A selects, B gives 127 when cancellable, `multichoicedefault`'s start index) on BG0, sharing the text box's tiles and the yes/no cursor drawing. The cost is bounded per frame (the M3 text-budget rule).
5. **Prelude:**
   - `giveitem`/`finditem` follow `Std_ObtainItem`/`Std_FindItem`: the message with the item and pocket names through the buffers, the bag-full branch, and the item ball's removal for `finditem`, as today;
   - `multichoice` helpers.
6. **Tests:** bag rules against Gen3 cases (full pocket, stacking, key items); buffer expansion bytes (synthetic charmap); multichoice input → `VAR_RESULT`, B → 127, default index; the hash changes only through the Wave 0 fields.

### Task I — engine: trainer sight, emotes, movement types

**Owns:** `crates/engine/src/{sight.rs, objects.rs, objects/*, trainer.rs, world/host/trainer.rs}`, `crates/engine/src/data.rs` (`MovementType` only), `crates/content/src/prelude/trainer.rs`, and their tests.

1. **Sight** (Gen3 `CheckForTrainersWantingBattle`, `TrainerSeeFuncList`):
   - after the player's step, for each visible object with `trainer_type` normal or see-all-directions and a script whose `trainer_flag_of` flag is clear, look along its facing (or all directions) up to `trainer_sight` cells with no blocking cell or object;
   - on sight: lock the player, show the `!` emote over the trainer, walk it to the adjacent cell (`TrainerApproach`), face the player, then start the object's script. The `trainerbattle` intro and battle follow as when talked to.
   - Two trainers at once: the first in object order battles; the second is `Report::Degraded("double sight")` and waits for the next step.
2. **Emotes:** `Emote::{Exclamation, Question, Heart}` sprites from Task A's field-effect pics (`GameData::sprite`-style lookup through a new provided method `emote(Emote) -> Option<&SpriteData>`, `None` draws nothing).
3. **Movement types:** add exact variants for the walk sequences (24), face-two and three directions (12), rotations (2), and jog, run and walk-slowly in place (12). That gives **≥ 65 of 81 exact**. The rest (copy-player, disguises, buried, berry growth) keep approximations, reported.
4. **Tests:** sight geometry (range, blocking, facing, flag set → no sight), the approach timing against Gen3 frame counts, a trainer battle started by sight in the world test game, each new movement type's step pattern.
5. **Route:** if a slice-route trainer now sees the player, the route changes. Refresh as the rules say.

### Task J — content: specials, std scripts, msgbox

**Owns:** `crates/content/build/support/specials.ron`, `crates/content/src/prelude/{mod.rs, msgbox.rs, std.rs, special.rs}`, `crates/content/tests/m8_j_*.rs`.

1. **Classify all 528 specials** in `specials.ron`. The 433 used must be correct; the unused default to `Unsupported`. Every `Native` has a match arm in `special()`, and a test checks that.
2. **Act 1 specials** (the set reachable from the Act 1 maps' IR, computed by a test from the Wave 0 extract plus the decomp map list) are all `Native`, `Ignore` or `Default` with a reason. Examples:
   - `StartWallClock`: no clock UI yet. The special completes and the field fades back in, as `ChangePokemonNickname` does (the Fade `CapturableStuck` findings). The time stays the M8 default; the RTC is M10.
   - `GetPlayerBigGuyGirlString`, `GetRivalSonDaughterString`: buffers.
   - `DrawWholeMapView`: redraw.
   - `SetUnlockedPokedexFlags`, `ShowMapNamePopup`, `DoPCTurnOnEffect`/`Off`, `TurnOnTVScreen`/`Off`: `Ignore`.
   - `ShouldTryRematchBattle`, `IsTrainerRegistered`: `Default(0)`.
   - `StartWallyTutorialBattle`: `Degraded`, no battle; the script continues (Petalburg).
   - `SaveGame`: `Ignore` (saving is a shell action until M9).
   - `PlayerPC`/`BedroomPC`: `Unsupported` (M9); the script must not softlock (tested).
3. **`callstd`:** every `STD_*` is routed (msgbox kinds, `STD_OBTAIN_ITEM`/`STD_FIND_ITEM` to Task H's helpers, `STD_REGISTER_MATCH_CALL` `Ignore`, …).
4. **Tests:** each `Native` special against a synthetic `Ctx` run (requests issued, in order); the Act 1 set is fully classified; no `Unsupported` special is reachable on the Act 1 maps without a reason.

### Task K — explorer at scale (headless)

**Owns:** `crates/explore/**`, `xtask/src/checkpoints.rs` (`promote` only).

1. **Db schema v3** (`SCHEMA_VERSION = 3`; an older db is refused with "run `explore reset`"; explore dbs are caches):
   - `node.inputs` becomes a BLOB: varint `(keys, frames)` runs plus a **repeat-pair** opcode for the A/release alternation of battle inputs (M7: the inputs text was 318 MB of 499 MB);
   - a `key_hit(key, hits)` table replaces leaf `merged` history rows: a duplicate with no child and a digest equal to or worse than the live node's only increments `key_hit` (M7: 37% of rows);
   - `Graph` keeps `NodeRow`s **without inputs** in memory; `cache::reach`/`lineage` load inputs on demand.
   - **Target:** ≤ 0.3 MB per 1,000 rows, and peak RSS at a 600 s slice run ≤ half of M7's 1.06 GB.
2. **Key gaps** (M7 Task B):
   - add `respawn`, a digest of the template deltas, and a digest of the key-items pocket;
   - exclude `VAR_OBJ_GFX_ID_0..F`;
   - record the fold version in `meta.key_version`.
3. **Goals as state:**
   - `Goal { name, reached: fn(&WorldSnapshot) -> bool }`. `rescue_started` → `VAR_ROUTE101_STATE ≥ 1` (verify the value against the decomp script); the stub's `wild_won` → the lead's exp is above its new-game exp.
   - **Per-act lists:** `Act::{Slice, Act1}`. Act 1, in story order:
     1–7. the slice's seven goals;
     8. `FLAG_RECEIVED_POKEDEX_FROM_BIRCH`;
     9. `MAP_PETALBURG_CITY` entered;
     10. Wally's tutorial done (`VAR_PETALBURG_GYM_STATE ≥ 2`, verified);
     11. `MAP_ROUTE104` entered;
     12. Petalburg Woods grunt beaten (its trainer flag);
     13. `MAP_RUSTBORO_CITY` entered;
     14. `FLAG_DEFEATED_RUSTBORO_GYM`;
     15. `FLAG_BADGE01_GET`.
   - Every constant is checked to exist in `content::consts`.
   - `--act` selects the list (default `act1` on real data, `slice` keeps M7's).
   - The `(progress, -frames)` merge rule stays; it is now also consistent with the key for flag and var goals.
4. **Goal-directed segments:**
   - `run --from <checkpoint id|label>` roots the db at a store node (layered store, `Materializer`); `meta.root` records it.
   - Finding and node exports use `Parent::Checkpoint(id)`.
   - `export --to-store <node> [--label L]` writes a local-store node: the lineage's inputs from the root, with hashes by replay.
   - `cargo xtask checkpoints promote <local id> --label L` moves a verified local node into the committed tree. It is the only path into it, so M6 decision 7 holds.
5. **Events in place of `Probe`:**
   - `SimSummary.scripts`, `maps` and `battles` come from `World::events()`.
   - `Probe` is kept only as a test wrapper, or deleted if unused.
   - **New `FindingKind::Unsupported`** (report-only): one finding per `(name, map)` from `Report` events, with the repro tail. Its signature has no digits, so each site is listed once.
   - The panic finding's `script` comes from `World::last_script()`.
6. `softlock_sample` is removed (M7 finding). Old configs still load: unknown fields are ignored.
7. **Tests:**
   - v3 round trip and size (a 2,000-node stub db is ≤ 40% of v2's);
   - determinism (threads 1, 4, 8) and resume digests;
   - the new key parts;
   - Act-list constants exist;
   - `--from` a committed stub node explores and exports a node that `cargo xtask checkpoints verify --local` reports `Valid`;
   - `promote` refuses an unverified node;
   - Unsupported findings from a fixture `GameData` that reports.

---

## Wave 2a — integration (after Wave 1 is merged)

### Task L — full import, end to end, and `cargo xtask sweep`

**Owns:** `xtask/src/sweep.rs`, cross-task fix-ups in the files of Wave 1 tasks A–J (each listed in the report), the README (Content, Scripts and Tests sections), and `checkpoints/*` refreshes.

1. `cargo xtask extract` (all maps, format 5), then build everything. Fix the seams between A–J: IR names in the lowering, tables wired into `GameData`, `trainer_flag_of`, emotes, encounters v5.
2. **The slice still holds:**
   - the parity route replays with identical ROM/native hashes;
   - the committed trees verify `valid (rehash)`, or re-accept once with the cause: names changed, so `BUILD_ID` changed; behaviour changed only through the tasks that announced it;
   - the M5 checkpoint invariant tests pass.
3. **`cargo xtask sweep`** (release, native; the `#[ignore]`d sweeps run under it). For every map:
   - `World::new` on a `StartAt { map, pos }` wrapper `GameData`: the first warp's cell, else the first walkable cell nearest the centre;
   - 600 idle frames with no panic, ending capturable, or the reason, listed;
   - a checkpoint round trip with an equal hash, and 60 frames shadowed;
   - a render (no PPU validation error).

   It also aggregates `Report` events per op and special, and prints the build report (`content::REPORT`). Output goes to `target/sweep/report.txt`; exit 1 on any panic or invariant failure.
4. **Acceptance:**
   - 518/518 maps pass, or each exception is listed with a reason (for example link-only rooms whose map scripts wait for a link);
   - the native debug build of `content` stays within E's numbers on the full import;
   - `cargo test --workspace` wall time grows by at most 2× the Wave 0 baseline.

## Wave 2b (parallel, after L)

### Task M — GBA at scale: budgets, heap, map tour

**Owns:** `crates/game-gba/**`, `xtask/src/{budget.rs, main.rs: rom() and parity() only}`, `crates/engine/src/debug.rs`, `crates/emu/**`.

1. **Debug region v3** (`PKRDBG03`, size grows to 216):
   - `heap_max` (high-water of a counting wrapper around agb's global allocator);
   - `last_script` and `map` (from `World`);
   - `reports` (count of `Report` events);
   - `panicked` (the GBA panic handler writes it and the panic location's line).

   `emu::DebugSnapshot` reads them.
2. **`xtask rom`:**
   - prints the ROM by section and by symbol prefix (`budget::sections`);
   - the gates become: ROM ≤ 16 MiB (open question 4), IWRAM ≤ the Wave 0 full-import baseline + 2 KiB, EWRAM statics ≤ baseline + 16 KiB;
   - `xtask parity` gates `heap_max` ≤ 160 KiB on every route it runs;
   - record every number in "As built".
3. **Map tour** (`--features tour`, ROM-only, not in the normal build): boot, then on L+R+START warp to the next map in `MAP_IDS` order (first warp cell). An mGBA test (`emu/tests/tour.rs`, ignored, run by `xtask parity --long`) drives it through all 518 maps and checks:
   - `late == 0`;
   - every black overrun follows a bulk step;
   - `loop_lines_max` ≤ 217;
   - no panic.

   Any map over budget is fixed (with F's chunking) or listed with its cause.
4. **Compression:** only if the ROM exceeds the gate. LZ77 (Gen3's) for tileset tiles and pics, decompressed in black bulk frames, with IWRAM ARM code if the decompressor is hot. Otherwise record "not needed" with the size.

### Task N — explorer on the full import: incremental revalidation and Act 1 tuning

**Owns:** `crates/explore/**`.

1. **Incremental revalidation** (M7 findings):
   - when only `content` changed (`BuildHash.engine` equal, `GLOBAL_DIGEST` equal), compare `SCRIPT_DIGESTS`/`MAP_DIGESTS` with the ones stored in `meta`;
   - mark Stale only the nodes whose own or any ancestor's `node_touch` includes a changed script or map;
   - every other node becomes Valid on the new build without replay;
   - any engine or global change stays "revalidate all".
   - **Test:** an override that changes one Littleroot script leaves every node that never touched Littleroot's scripts `Valid` with no replay.
2. **`Cross` ends one cell into the new map** now that the stale view is capturable (F): remove the walk-away and talk fallback (M7 Task A deviation), keeping a test that the old fallback path is gone.
3. **`explore sweep`** (release): for every map, root at Task L's `StartAt` state, run 10 generations, and aggregate findings by kind and map into `target/explore/sweep.txt`. Triage the results in the report (bugs to Findings, or fixed in a `fix(…)` commit when trivial and route-neutral).
4. **Act 1 tuning:** `run --act act1 --secs 1800` from an empty db, then `--from` the furthest goal's exported node, in segments, until `FLAG_BADGE01_GET` or the budget runs out. Tune weights and limits in `Config::default` (the M7 order: goal weight, plan length, age penalty, cell shift). Record each attempt.

### Task S — studio at scale

**Owns:** `crates/studio/**`.

1. **Map tab:** a map picker for 518 maps (grouped by map group, with a filter by name or constant); `MapCache` stays an LRU of 32; overlays for the new event kinds (dynamic warps, trainer sight ranges as a layer).
2. **Data tab:** an Items sub-tab; encounters show water, rock smash and fishing; trainers list all.
3. **Tests at scale:** `map_pixels`, `map_view` and `data_views` use a fixed sample (the 15 slice maps plus Petalburg, Rustboro, Route 104, Petalburg Woods and one interior per tileset family). Whole-game versions are `#[ignore]` and run by `cargo xtask sweep --studio`.
4. **M6 and M7 leftovers:**
   - a `Materializer` per worker thread (M6 finding 4): each pane and the explorer-open path get their own, with the shared disk cache;
   - quit kills the explorer run's process group (a tao `RunEvent::ExitRequested` hook, M7 S2 note);
   - nodes/s as a moving average over the last 5 polls.
5. **Gates:** all M6 gates, and `cargo xtask studio --smoke 8` ≥ 58 fps with the full import.

---

## Wave 3 — Act 1 route, parity, close-out

### Task P — Act 1 end to end

**Owns:** the `act1:*` nodes in `checkpoints/emerald/nodes/`, `crates/game-native/tests/act1.rs`, `crates/emu/tests/parity.rs` (the Act 1 lanes), and this doc's "Wave 3 as built".

1. **Find the route:** explorer segments (Task N's best config) from a new game to `FLAG_BADGE01_GET`. Export each goal's node (`export --to-store`), `verify --local`, then `cargo xtask checkpoints promote` as `act1:pokedex`, `act1:petalburg`, `act1:woods`, `act1:rustboro`, `act1:badge1`. The run's seed, config and wall time are recorded.
2. **Native invariant along Act 1** (`act1.rs`, release, `#[ignore]`, run by `xtask parity`): the M5 core invariant at about 100 capturable points of the flattened Act 1 trace (round trip, equal hash, 120 frames equal), and 3 points to the end.
3. **ROM parity:**
   - every `act1:*` node resumes on the ROM from its `.sav` for 2,000 frames, with identical hashes and pictures every 100 frames, `late == 0` and `heap_max` gated;
   - `cargo xtask parity --long` replays the whole Act 1 trace on mGBA from a new game, with every hash compared.
4. **Triage:** every finding and every `Unsupported` report reached on the Act 1 route goes into Findings for M9, or is fixed if trivial.
5. **Numbers:** total frames to the badge, the explorer wall time, maps entered, % of Act 1 cells reached, battles won/lost/ran, ROM/IWRAM/EWRAM/heap, worst lines per segment.

### Close-out
- **Whole-milestone review:** determinism (build reproducibility, explorer digests); write locations (nothing ROM-derived tracked: `git ls-files` against the ignore rules); FR literals; panics reachable from content; engine game-agnosticism. Fix the findings.
- **This doc:** "As built" per task (contracts changed, numbers), the findings table updated, "Findings for Milestone 9".
- **README:** the full-import workflow (`extract`, the build report, `sweep`, overrides), and the Act 1 parity lanes.
- **Cleanup:** clean up worktrees.

---

## Done when

- **Import:** `cargo xtask extract` (BPEF and BPEE) exports **518/518 maps**, 73 tilesets, every layout, every referenced sprite, the emote pics, pics for every species and trainer, every wild header (Altering Cave's 9), items, multichoice lists, std strings and map sections. **100% of exported strings re-encode** to their ROM bytes. No previews unless asked. Two exports are byte-identical.
- **Scripts:**
  - every reachable script (about 4,560 functions) is lowered by `content/build.rs` from the IR;
  - **≥ 95% carry decomp names**;
  - every command and special is classified (the build fails otherwise);
  - the build report lists the `Unsupported` sites by op;
  - **no `Unsupported` command or special is reached on the Act 1 route** without a recorded reason.
- **Run:** `cargo xtask sweep` passes on 518 maps (or a listed, justified exception set): no panic, idle-capturable, checkpoint round trip, render. `explore sweep` findings are triaged.
- **Act 1:**
  - the committed `act1:*` nodes reach `FLAG_BADGE01_GET` from a new game and verify `Valid`;
  - native invariant points pass;
  - every `act1:*` node resumes on the ROM with identical hashes, and `parity --long` replays the whole act identically;
  - `late == 0`.
- **GBA:**
  - ROM ≤ 16 MiB (or the answered gate);
  - IWRAM and EWRAM within the new gates;
  - `heap_max` ≤ 160 KiB on every parity lane;
  - `loop_lines_max` ≤ 217 on the slice route, the Act 1 segments and the map tour (or re-gated ≤ 226 with the cause recorded).
- **Explorer:**
  - db ≤ 0.3 MB per 1,000 rows;
  - thread-count and resume determinism hold;
  - an override of one script revalidates only the touching subtree;
  - Unsupported findings are listed;
  - Act 1 goals are reached by segments within the recorded budget.
- **Folded findings:** every row of the findings table below is Done, Decided or Deferred with a milestone.
- **Gates:**
  - fmt and both clippy runs;
  - `cargo test --workspace` real and stub;
  - `cargo test -p studio --features ui` and `cargo xtask studio --smoke`;
  - `cargo xtask rom`, `cargo xtask parity` (and `--long` once, recorded);
  - game-gba fmt and clippy.
- **Hygiene:** no ROM-derived file committed (IR, generated Rust, tables, pics, db, snapshots); no FR literals in tests; `engine` names no Emerald constant.

---

## Findings folded in (M3–M7)

| Finding (source) | Disposition |
|---|---|
| `special StartWallClock` unimplemented; the fade never clears; 2 `CapturableStuck` (M7 H) | **Task J** (fade back in; RTC M10) |
| Stale view after a crossing should be capturable (M7 A) | **Task F** (stale rect in `FieldSnap`), then **N** (`Cross` ends one cell in) |
| Summary-only goals observable in state (M7) | **Task K** (state predicates; exp-based stub goal) |
| Abstract-key gaps: `respawn`, `templates` deltas, `VAR_OBJ_GFX_ID_*` (M7 B) | **Task K** |
| Party switching, items and bag in policy and key (M7) | **K** adds key items to the key. Policy switching and bag use: **M9** (they need the party and bag menus) |
| explore.db size: merged rows and inputs text (M7 H) | **Task K** (binary inputs, `key_hit`, no leaf merged rows, inputs not resident) |
| `DetectConfig::softlock_sample` unused (M7 H) | **Task K** (removed) |
| Per-script and per-map content digests for incremental re-simulation (M7) | **D**/**E** emit them, **N** uses them |
| Engine `FrameOut` events in place of `Probe`; script-line coverage (M7) | Events: **Wave 0** + **K**. Line coverage: **M14** (explorer v1; needs per-block marks) |
| Engine-side panic context (M7) | **Wave 0** (`last_script`, events), **K** (findings), **M** (GBA panic handler) |
| Elevation-aware pathfinding (M7) | **Deferred to M10** with bridges (no `PlannerDivergence` in M7 H; Act 1 has no elevation-sensitive path) |
| Balance fitness (M7) | **M14** |
| Goal-directed completability per checkpoint (M7) | **Task K** (`--from` segments, per-act goals) |
| Explorer exports as local-store nodes (M7 decision 14, M6 F3) | **Task K** (`export --to-store`, `xtask checkpoints promote`) |
| Own driver vs the scenario `Recorder` (M7 decision 1) | **Decided:** stay separate in M8; the Act 1 route comes from the explorer, so the scenario route stays frozen at the slice. Unify in M14 if useful |
| A per-thread `Materializer` (M6 F4) | **Task S** |
| `Parent::Edited` (M5 decision 17, M6 F5) | **Decided, deferred:** the state editor is M14; when it lands, edited nodes are local-only with a deterministic op list (`Parent::Edited { base, ops_digest }`), and `verify` reports them unverifiable in the committed tree |
| Data editing and write-back target (M6 decision 5) | **Decided:** overrides layer (decision 12) for scripts in M8; data patches over extracted data and studio write-back in M14 |
| Linux CI for the `ui` feature (M6 decision 3) | **Deferred** (M14) |
| Studio quit orphans an explorer run; nodes/s jitter (M7 S2) | **Task S** |
| Spread warp-load uploads, `late_black` → 0; crossings between different tilesets (M4) | **Task F**, checked by **M**'s map tour |
| Doors: no animation graphics (M4) | **M10** (cosmetic) |
| Old-map NPCs after a crossing, `selected` (M4) | **Task F** |
| Step triggers fire one frame early (M4) | **Deferred to M10** (it changes all route timing; not needed for Act 1) |
| Next script commands: `setrespawn` (done), `multichoice`, `trainerbattle`, `givemon` (done), `pokemart`, specials (M4) | **D/H/I/J**. `pokemart` stays `Unsupported` (the Mart UI is M9); Act 1 does not need a purchase |
| Trainer sight; Attract/Rollout; doubles; `bufferleadmonspeciesname` (M4 simplifications) | Sight **I**; Attract/Rollout **G**; doubles degrade **G**; buffers **H** |
| `Emote` TODO; elevation-based OBJ priority (M3) | Emote **I**; priority **M10** |
| Known risk: a stale `setmetatile` override after a crossing (M5 decision 15) | **Task F** (the stale rect redraws overrides) |
| Flash 64 KiB, about 24 KiB per slot, for a future full save (M5 decision 10) | **M9** (full save with bag/PC); M8's state stays under `MAX_ENCODED` 8 KiB (checked by L's sweep) |
| GBA LCD colour-correction option (M4) | **Deferred** (studio display option, M14) |

---

## Roadmap: Milestone 9 and later

Each later milestone keeps M8's shape: Wave 0 contracts, parallel waves, a story target proved by the explorer and by parity.

- **M9 — Menus and UI (Act 2: Dewford, Slateport, Mauville):**
  - the start menu, party screen, summary, bag UI (using M8's `Bag`), Pokédex view, options;
  - the Mart (`pokemart`), the PC (item and Pokémon storage);
  - the naming screen and the real new-game intro (truck, Birch's speech, clock);
  - the evolution scene, catching through the bag;
  - the in-game save through the start menu, with a **full save format** (bag, PC boxes; flash 128 KiB with the mGBA override, or a compact layout in 64 KiB);
  - money and coin boxes; policy switching and items in the explorer.
- **M10 — Field moves and overworld systems (Acts 3–4):**
  - Cut, Rock Smash, Strength, Surf, Flash, Fly (map screen), Dive, Waterfall; bikes and their tiles;
  - weather and field effects; tileset animations;
  - virtual objects (`createvobject`), rotating-tile puzzles, `setmaplayoutindex`;
  - the RTC and time-based events (berries, tides);
  - elevation and bridge priority, door animations, map name popups, step timing;
  - roamers and repel; fishing and water encounters (tables already compiled).
- **M11 — Battle completeness:**
  - double and multi battles; every effect and ability, held items;
  - move animations; the remaining AI flags;
  - catching edge cases (Safari Zone), the Wally tutorial battle.
- **M12 — Audio:** an m4a/Sappy engine (songs, SE, fanfares, cries) in IWRAM, mixed per frame under the line budget; native audio for the studio.
- **M13 — Side content:** contests (and contest painting), the Game Corner (slots, roulette), decorations and secret bases, Pokéblocks and berry blending, Trainer Hill, the Battle Frontier. Link and wireless features stay stubs.
- **M14 — Explorer and studio v1:**
  - script-block coverage, balance fitness, completability per checkpoint across acts;
  - studio editing: data patches and write-back, overrides from the UI, script views and graphs, the state editor (`Parent::Edited`);
  - Linux CI; LCD colour correction.

---

## Decisions taken

1. **Scope: import everything, play Act 1 (taken; open question 1).**
   - "Full engine parity" cannot be one milestone: the spike shows 90 unsupported command kinds and 433 specials, and menus, field moves, doubles, contests and audio are each milestone-sized.
   - Importing everything at once is cheap: the spike exported 518 maps in 5.5 s and compiled them. It is also what removes the slice's structural assumptions (names, slice sets, Rust literals, one wild header).
   - A story target proves the import works end to end. Roxanne is the first point the story needs items, money, trainer sight and broader battles, without menus or field moves.
2. **IR in `extract`, lowering in `content/build.rs` (taken).**
   - Today every codegen change needs a ROM and a re-extract (`FORMAT_VERSION` bump). M8 changes lowering in five tasks, and later milestones in every task. With the IR split, an engine feature is a build change that anyone with an extract can iterate on.
   - The build can apply overrides per function (it cannot drop one function from an `include!`d file today) and emit per-script digests (M7 finding).
   - The IR keeps the hygiene rule: it is ROM-derived and git-ignored like the rest of `content/extracted/`. The generator is committed; the generated Rust exists only in `$OUT_DIR`.
   - **Not decompiling the `.inc` sources:** the design makes the FR ROM the asset source (FR text pointers, FR-specific data). The decomp is a format and naming guide. Bytecode decoding already covers all 227 opcodes and 518 maps.
   - **Cost:** the content build lowers about 97k lines at build time. The probe shows generation takes about a second; the compile dominates, and E's blob move pays for it.
3. **Decomp label names (taken; open question 2).**
   - Positional names are stable only for a frozen export order. Shared scripts are named after their first referencing root (the nurse is `OldaleTown_PokemonCenter_1F_Obj1_L0`), and adding maps before Oldale would rename them.
   - Decomp labels are stable, readable in the studio and explorer findings, and the natural key for overrides.
   - Label transfer needs only reference sequences, not an assembler. Mismatches fall back per block and are measured (≥ 95% target).
   - Snapshots hold no `ScriptId` (M5), so renames do not break checkpoints. Only `BUILD_ID` and tests that name scripts change.
4. **Classify, never crash (taken).**
   - At 1,543 unsupported sites, "log and continue" must be explicit per command. Some commands write `VAR_RESULT` (a default must be chosen); some are cosmetic (`Ignore`, no noise); some are real gaps (`Unsupported`, counted and surfaced by the explorer as findings).
   - Failing the build on an unclassified command forces a decision when the IR gains one.
5. **Remove the slice structurally (taken).** The slice was encoded as constants (`MAPS`, `slice_*`, `SLICE_*`, `PLAYER_LEVEL_CAP`) and as single-instance assumptions (one wild header, previews always). Each is replaced by "everything, filtered by explicit options", and tests move to named fixed samples.
6. **Binary blobs for bulk data (taken).** The spike's 11.7 MB `data.rs` compiled in 31 s at 4.96 GB of rustc memory, and the GBA build peaked at 8.9 GB. `battle.bin` has shown the blob pattern since M4. Rust literals stay for small typed tables, where type checking matters.
7. **GBA budget (taken; open question 4).**
   - The spike's full ROM is ≈ 12.4 MB at opt-level 3, and ≈ 10.2 MB with `content` at "s". Projected with every pic: 11.4–13.6 MB. That is under 16 MiB, so **no banking** (the GBA has none to do below 32 MiB) and **no compression** unless the gate trips.
   - Script code is 59% of the ROM at scale, so "s" on `content` is the cheap lever. Scripts are light per frame, so speed is not at risk; Wave 0 checks the lines.
   - Futures are tiny (host max 200 B; on the GBA smaller with 4-byte pointers), so the M1 risk "box large futures in EWRAM" does not materialize. Heap high-water joins the debug region to keep it measured.
   - **Compile memory** (8.9 GB peak for the GBA LTO build) is a dev-machine cost, reduced by decision 6 and recorded.
8. **One hash change in Wave 0; announced route changes (taken).**
   - The M5/M6 practice was "only one task changes hash bytes". M8 adds state in several tasks, so the state lands at once, unused, with one re-accept.
   - Behaviour that genuinely changes the slice route (warp timing, battles, sight) cannot be avoided. Those tasks re-accept in a separate, reviewed `chore(checkpoints)` commit with the cause, so a tree change is never silent.
9. **Act 1 parity in segments plus one long run (taken).** A 150k–300k-frame ROM replay is too slow for every `xtask parity`. M5's "ROM resumes from native checkpoints" gives segment parity at every `act1:*` node in seconds. The full straight replay is `--long`, run at close-out and before releases. The map tour covers maps the route never visits.
10. **The explorer finds the Act 1 route (taken).** The scenario recorder's hand-written route (M3–M5) does not scale to an act. The explorer already reaches the slice's goals in 1.7 s. With `--from` segments and per-act goals, its exported lineage becomes the committed route, and that route is input-only and verifiable like any node.
11. **French text and offsets (taken).**
    - No offset is hard-coded: maps, tilesets, scripts and texts are found by anchors and pointer tracing, and the spike read all 518 map headers and round-tripped all 5,503 reached texts on BPEF.
    - The new C-side tables follow the same rule: located by structure, validated on BPEE against the decomp, every FR string round-tripped.
    - If a table differs in layout in BPEF (for example longer FR names in a fixed-width field), the reader takes the width from the ROM's structure, never from the decomp's `#define`.
12. **Overrides only for scripts in M8 (taken).** M6 deferred "where edits live" to M8. Script overrides are needed now (worked example in D) and cost little. Data patches need a patch format and studio write-back, which is M14 work with no M8 consumer.
13. **Release-only sweeps (taken).** 518 maps × (load, 600 frames, round trip, render) is minutes in release and far too slow in debug. Debug tests keep the fast red/green loop on fixed samples; the sweeps are gates at the end of Wave 2a and at close-out.
14. **pokeemerald-expansion not used (taken).** It changes mechanics toward later generations and restructures data. Vanilla behaviour is the spec; the expansion is consulted only for readability.

---

## Size estimate

| Wave | Tasks | Size (agent sessions) |
|---|---|---|
| 0 | contracts, spike re-run, hash change | 1 large |
| 1 | A (L), B (L), C (M), D (L), E (M), F (L), G (L), H (M), I (M), J (M), K (L) | 11 parallel |
| 2a | L (L) | 1 |
| 2b | M (M), N (M), S (S) | 3 parallel |
| 3 | P (L), close-out (M) | 2 |

- **In total:** 18 task sessions, against M7's 10. About 1.8–2.5× M7's effort.
- **Hand-written code:** about 20–30k lines including tests (M7 added 13k in `explore` and 10k in `studio`). The generated code (about 100k lines of Rust plus blobs) lives in `$OUT_DIR` and is not counted.
- **Longest chain:** Wave 0 → B → D → L → N → P.
- **Main risks:**
  - the label transfer's match rate (B), with the positional fallback as the floor;
  - the explorer reaching the badge within budget (N/P), with `--from` segments and tuning as levers, and the scenario recorder as a last resort for one hard segment;
  - GBA frame budget on maps with many objects (M's tour, with F's chunking).

---

## Wave 0 as built
*(filled by Wave 0: the spike re-run numbers, the opt-level decision, baselines, contract deltas.)*

## Findings for Milestone 9
*(filled at close-out.)*
