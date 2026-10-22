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
- **One state change, once.** Wave 0 adds all new `World` state (bag, money, string vars, warp slots, the stale-view rect) with its hash and snapshot DTOs. Snapshot `SCHEMA` 2, one golden update and one tree re-accept. Wave 1 tasks implement behaviour behind those fields. Behaviour changes that move the slice route are merged one at a time, each followed by a checkpoint refresh on master ("Hash and checkpoint discipline").
- **One owner per file.** Wave 0 splits every file two Wave 1 tasks would edit (`world.rs`, `world/host.rs`, `data.rs`, the prelude, `classify.rs`, `codegen.rs`) and declares every cross-task signature with a default body, so Wave 1 tasks only fill bodies in files they own.
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

**M7 close-out numbers** (the baseline Step 0 starts from): final explore digests stub `0x30ec96cf029cb82f`, real `0xa8a8b7baa262e20c`; ROM 1,991,320 B (+8 B over the table's figure, from the `BUILD_ID` constant).

---

## Decisions (resolved)

The user's defaults, taken autonomously; nothing blocks on them.

- **Q1, story target:** Act 1 runs to badge 1: from the M4 new game to Roxanne's badge (`FLAG_BADGE01_GET`). Rejected: no story target (no end-to-end proof); through badge 2 (needs the Briney boat, Granite Cave's Flash and more battle work).
- **Q2, names:** rename to decomp labels (decision 3). Every `SCRIPT_NAMES`/`TEXT_NAMES` identifier changes once (`Route101_Coord0` becomes something like `Route101_EventScript_StartBirchRescue`), and the tests that name slice scripts migrate once.
- **Q3, doubles before M11:** follow Gen3 when the player has fewer than 2 usable mons: the exact not-enough path (`GetMonsStateToDoubles_2`; `trainer_see.c` skips a double trainer's sight, and the talk path shows its `not_enough_pkmn_text` through `EventScript_NotEnoughMonsForDoubleBattle`). With 2 or more, the double battle degrades to singles against the same party, sets the trainer flag, and emits `Report::Degraded("double battle")`.
- **Q4, ROM gate:** 16 MiB (Emerald's own size; the projection is 11–14 MB; the hard limit is 32 MiB). M12's audio (songs and samples) may bring LZ77 back.
- **Q5, Act 1 nodes:** commit them to the shared tree as `act1:*` nodes (input-only; the Act 1 trace is about 1–1.5 MB of RON at the M5 run syntax), so parity and `verify` cover Act 1.
- **Q6, new-game start:** keep the M4 start (outside Brendan's house, before the rescue). The real intro needs M9's menus.
- **Route churn:** when a later behaviour change breaks `act1:*` nodes, re-find them with the explorer (`--from` the last valid node) where possible; otherwise re-accept them in a `chore(checkpoints)` commit with the cause.
- **RAM:** dev machines with 16 GB are supported; every build-memory gate below is set from that.
- **M8 split:** M8a/M8b (Size estimate) is a fallback only, used if L slips.

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
| 7 | GBA budgets | No banking (the GBA maps 32 MiB linearly). ROM gate 16 MiB; `content` built at opt-level "s" if Wave 0 confirms the spike; LZ77 only if the gate is threatened. Explicit interim gates for Waves 1 and 2a (per section on the slice, 16 MiB absolute on the full import); IWRAM and EWRAM over the slice baseline until M re-baselines. New: heap high-water and IWRAM stack high-water in the debug region, gated. Futures need no boxing policy beyond today's (200 B max). Build memory sized for 16 GB machines. |
| 8 | Hash, schema, trees | Wave 0 is the only planned state change. Snapshot `SCHEMA` 2. Route-changing tasks (D2, H, J, I, G, F) merge one at a time, in that order, and never commit tree refreshes; master re-accepts after each in a `chore(checkpoints)` commit. Act 1 nodes join the committed tree. |
| 9 | Parity at scale | The slice route as today, plus ROM-resumes-from-native at every `act1:*` node (2,000 frames each), plus a full straight Act 1 replay under `cargo xtask parity --long`. A ROM **map tour** build checks `late == 0` on map loads and crossings: gated on the Act 1 maps plus one map per tileset family, all 518 as a stretch. |
| 10 | Explorer at scale | `--from <checkpoint>` roots; per-act goal lists of state predicates; key gaps closed; db schema v3 (binary inputs, `key_hit` counter, no childless merged rows, inputs not resident); `World::events()` replaces `Probe`; incremental revalidation from per-script and per-map digests; `export --to-store` plus `cargo xtask checkpoints promote`. |
| 11 | French text and offsets | Everything stays located by anchors and pointer tracing (no BPEF offset constants). New C-side tables (items, multichoice lists, std strings, map sections) are located the same way, checked on BPEE against the decomp, and every exported string must re-encode to its ROM bytes. |
| 12 | Overrides | `crates/content/src/overrides/<Name>.rs` replaces a generated script by name (committed Rust, text by id only). Data patches and studio write-back are M9+. |
| 13 | Tests at scale | Debug tests use named fixed samples of maps and ids (`export::SLICE_MAPS`, `content::samples`); full sweeps (every map) are release-only: `cargo xtask sweep`. `explore sweep` moves to M9. |
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
  - `main.rs`: the build entry; stub fallback; `BUILD_ID`; `rerun-if-changed` on `src`, `build`, `../extract/src` (`:44-46`).
  - `build_id.rs`: hashes the non-test **`.rs`** files of `src/`, `build/` and `extract/src` (`is_build_source`), plus the extracted dir. A `.ron` under `build/` is not hashed today.
  - `codegen.rs` (774 lines): `consts.rs`, `data.rs` (maps, tilesets, sprites, texts as Rust literals) and `scripts.rs` (one `include!` per map), plus `SCRIPT_NAMES :721`, the `script(id, ctx)` dispatch `match` `:733`, `scripts_rs :750`, `generate :765`.
  - `battle.rs`: the slice set `slice_uses :112`, `slice_species :128`, `slice_moves :134`, `gen_slice :811`, `UNWORDED_STAT_EFFECTS :320` and its check `:455`, `PLAYER_LEVEL_CAP :31`.
  - `classify.rs`: `behavior :71`, `movement :109`, `gfx :142`, `ability :280`, `effect_stat :302`, `move_effect :317`, and the `Str` → `STRINGID_*` table `STRS :403` (already here, not in `battle.rs`).
  - `input.rs`, `stub.rs`.
- **Content runtime:** `crates/content/src/prelude.rs`: `msgbox`, `callstd :69`, `special :98` (three implemented), `trainer_battle`, `check_player_gender`, `unsupported :220`. `new_game.rs`, `blob.rs` (`Blob`, used by `battle.bin`).
- **Engine:**
  - `script.rs`: `Request :63`, `Ctx :304` (`flag` … `set_respawn :545`), `Runner :583-729`.
  - `world/host.rs` (348 lines): `done :42`, `begin :71`, `resume :315`, `wait :337`.
  - `world.rs` (757 lines): `step :289`, `step_field :302`, `start_script :340` (the old-map `selected` filter), `start_warp :360`, `control :379` (talk, push warps, step-end triggers), `run_transition :488`, `hash :616`, `load_map :683`, `cross_connection :711`, `run_immediate :739`.
  - `world/battle.rs` (486 lines, already split): `start_battle :218`, `wild_step :260`, `step_battle :374`, `whiteout :464`. `world/checkpoint.rs`: `capturable` with `StaleView :50`. `world/tests.rs`: one `golden_hashes` test (`:736`, two hashes of one walk).
  - `data.rs`: `id!` `:43`, `MovementType :171`, `ObjectEvent` (`trainer_type`, `trainer_sight` `:197-198`, unused), `MapData :268`, `NewGame :322`, `GameData :351-410` (`encounters(map)` `:385` takes no wild-set argument). `mon.rs`: `TrainerData :378` (no prize money, no evolution data anywhere).
  - `flow/vars.rs`: `Vars::hash :156` feeds an O(1) digest maintained on write; the pattern for `Bag` and `str_vars`.
  - `checkpoint.rs`: `SCHEMA = 1` `:20`, `WorldSnapshot :104`. `debug.rs`: `PKRDBG02`, 200 bytes. `trainer.rs`: trainer parties only (no sight).
  - `crates/emu/tests/parity.rs`: `HASH_LINES_MAX = 65 :109`, `LOOP_LINES_MAX = 217 :110`.
- **Native, xtask, explorer, studio:**
  - `game-native/src/scenario.rs`: `EMERALD_ROUTE :776`, `cached_route :882`.
  - `xtask/src/budget.rs`: `BASELINE :34` (1,992,712 / 596 / 460), `GROWTH` (+64 KiB / +2 KiB / +16 KiB).
  - `crates/explore/src/goals.rs`: `GOALS` real `:56` (7) and stub `:97` (3). `key.rs`, `probe.rs`, `schema.sql` (v2).
  - Studio: `map_view.rs`, `data_view.rs`, `tests/{map_pixels, map_view, data_views}.rs` iterate every exported map or table.
- **Committed trees:** `checkpoints/emerald/nodes` c00001–c00004, `checkpoints/stub/nodes` c00001–c00003.
- **Files naming the slice** (`MAP_IDS`, `export::MAPS`, `SLICE_*`, `slice_species`, `manifest.maps`): `content/build/{battle,battle_load,codegen,input}.rs`, `content/src/{battle_tests,data_tests,lib}.rs`, `explore/src/cli.rs`, `explore/tests/f_policy.rs`, `extract/src/{roots,schema}.rs`, `extract/tests/{codegen,text}.rs`, `studio/src/{data_view,explorer,map_view,play_view}.rs`, `studio/tests/{data_views,map_pixels,map_view}.rs`. Of these, `SLICE_SPECIES`/`SLICE_MOVES` are read by `content/src/battle_tests.rs`, `explore/tests/f_policy.rs` and `studio/tests/data_views.rs` (not `studio/src/data_view.rs`, which reads `MAP_IDS`); `export::MAPS` by `extract/tests/{codegen,text}.rs`.
- **Decomp tables not in `names/generated.rs` yet:** `MULTI_*`, `STDSTRING_*`, `MAPSEC_*`, `LAYOUT_*`. Script command names need no table: `scripts::OPS :172` already carries every macro name.

---

## Wave 0 contracts (committed by you before Wave 1; read them, do not redesign them)

Wave 0 makes no feature work. It does five things:
- the spike, re-run on the merged tree, whose numbers fix decision 7 and the build gates;
- every new type, file and signature, with `todo!()` bodies or safe defaults;
- the module splits that let Wave 1 run without shared files: after Wave 0, no file is owned by two tasks;
- the named fixed samples, with every consumer migrated, so Wave 1 only deletes slice tables;
- the one hash change (new state), with the golden update and the tree re-accept.

`cargo test --workspace` (real and stub), `cargo xtask rom`, `cargo xtask parity` and `explore` stay green after Wave 0.

### Step 0 — reconcile and baseline
1. Rebase on master after the M7 close-out (Tasks H and S2 and Wave 3 merged). Read M7's final "As built" and its "Findings for Milestone 8". If the close-out already handled an item in the findings table at the end of this doc, mark it there and drop it from its task.
2. **Re-run the spike** on the merged tree, on a throwaway branch: all maps, a scratch extracted dir, native debug and GBA release builds at content opt-level 3 and "s". Record in "Wave 0 as built":
   - export time;
   - extracted size with and without previews;
   - native build time and peak RSS;
   - ROM `.text` and `.rodata`, and the size by symbol prefix (`content::scripts`, `content::data`, `content::battle`, `engine`, other);
   - the largest script future;
   - the content **build-script** time (lowering and blob writing, separate from rustc);
   - GBA release time and peak RSS with **thin vs fat LTO**;
   - `loop_lines_max` with a `StartAt` on Route 104 and on Rustboro (the object-heavy Act 1 maps; spike: all templates spawn), against the 217 gate.
3. **Decide `content`'s GBA opt-level.** Take "s" if it saves at least 15% of ROM and the slice route's `loop_lines_max` stays within its gate (spike: −29% `.text`). Set it as `[profile.release.package.content] opt-level = "s"` in `crates/game-gba/Cargo.toml`.
4. **Build profiles** (root `Cargo.toml` and `crates/game-gba/Cargo.toml`, which is its own workspace):
   - `[profile.dev.build-override]` and `[profile.release.build-override]` `opt-level = 3` (the build script lowers ~97k lines and writes the blobs);
   - `[profile.release.package.content] debug = false` in game-gba (its release profile has `debug = true`; `"line-tables-only"` if panic locations need it);
   - keep fat LTO unless thin LTO is within 3% of ROM and saves at least 25% of time or memory (record the choice).
5. **Baselines on master:** slice route parity (hashes, pictures, `late`, lines); `explore run --secs 60` (nodes/s, db MB per 1k rows); `cargo test --workspace` wall time in both flavors.

**Build gates on the full import** (dev machines with 16 GB): native debug build of `content` (data plus scripts) ≤ 30 s with rustc peak RSS ≤ 4 GB; GBA release peak RSS ≤ 9 GB (no regression over the spike's 8.9 GB; ≤ 5 GB is a stretch) and wall time ≤ 2 min. Wave 0 records where the spike stands against each; L and close-out check them.

**Interim ROM gates for Waves 1 and 2a** (`xtask/src/budget.rs`, until M re-baselines; `budget::sections` is implemented in Wave 0, not stubbed):
- on the slice extract, per section: engine `.text` ≤ its Wave 0 baseline + 128 KiB (content sections are reported, not gated, since Wave 1 grows them by design);
- on a full-import build: ROM ≤ 16 MiB absolute;
- IWRAM and EWRAM gated over the slice baseline (+2 KiB / +16 KiB, today's `GROWTH`).

### Extract contracts (`crates/extract`)
- **`schema.rs`: `FORMAT_VERSION = 5`**, plus `FORMAT_MINOR` (starts at 0): every merge that changes what `extract` writes bumps the minor, and the content build fails with "re-run `cargo xtask extract`" and the names of any missing files when the manifest's minor is older. Every file type below is declared in Wave 0 (serde types, no logic): `ScriptFile`, `ScriptIndex`, `LayoutFile`, `EncountersFile` v5, `FieldEffectSpriteFile`, `ItemsFile`, `MultichoiceFile`, `StdStringsFile`, `MapSectionsFile`, `EvolutionsFile`, `TrainerMoneyFile`. The layout doc gains:
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
  battle/evolutions.ron      EvolutionsFile (gEvolutionTable: SPECIES_* -> [(EVO_*, param, SPECIES_*)])
  battle/trainer_money.ron   TrainerMoneyFile (gTrainerMoneyTable: TRAINER_CLASS_* -> value)
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
- **Module moves and stubs** (mechanical, no behaviour change):
  - `export_scripts` and `ScriptInput` move from `export.rs` to `export/scripts.rs` (Task B).
  - New `export/tables.rs` with `pub fn export_tables(rom, out, provenance) -> Result<()>`, an empty body that `export` calls (Task C).
  - New stub modules `inc.rs`, `labels.rs` (B) and `tables.rs` (C), with their `pub mod` lines in `lib.rs`, so no Wave 1 task edits `lib.rs`.
  - `names/generated.rs` regenerated (`PKR_BLESS=1`) with `MULTI_*`, `STDSTRING_*`, `MAPSEC_*` and `LAYOUT_*` tables, drift-tested like the others.
- **`Species.weight`** (`schema.rs`, owned by A, carried into content by E): `gPokedexEntries[SpeciesToNationalPokedexNum(species)].weight` in hectograms (Low Kick, G), `#[serde(default)]`; 0 when a species has no Pokédex entry or the tables cannot be located.
- **Named fixed samples:** `export::SLICE_MAPS` (today's 15 maps, in today's order) replaces every test and tool use of `export::MAPS`, and `ExportOptions` takes it for `--maps slice`. Consumers are migrated in Wave 0 (`extract/tests/{codegen,text}.rs`).
- **`codegen.rs`, new entry point** (Task D1; stub). `generate` keeps working until Task D2 swaps the build over:
  ```rust
  pub enum Class { Native, Ignore, Default(u16), Unsupported }
  pub struct Support { pub commands: BTreeMap<String, Class>, pub specials: BTreeMap<String, Class> }
  pub struct Lowered {
      pub modules: BTreeMap<String, String>,       // module -> Rust source for $OUT_DIR
      pub dispatch: String,                        // static thunk table + SCRIPT_NAMES (no giant match)
      pub stats: Stats,                            // Stats gains `by_class`, `reports`, `named_decomp_pct`
      /// Keyed by script **name**, never by ScriptId (ids shift when scripts are added).
      /// Each digest covers the fn's lowered source, the texts and movements it references,
      /// its static call-graph closure (callees and std scripts) and its override source.
      pub script_digests: BTreeMap<String, u64>,
      pub trainer_scripts: Vec<(String /* script */, String /* TRAINER_* */)>,
  }
  /// IR -> Rust. No ROM access. `overrides`: script name -> override source text (digested).
  pub fn lower(files: &[ScriptFile], index: &ScriptIndex, support: &Support,
               overrides: &BTreeMap<String, String>) -> anyhow::Result<Lowered>;
  ```
- **xtask `extract`** accepts `--maps all|slice|<Dir,Dir,…>` (default `slice` = `export::SLICE_MAPS` until Task L, which flips `ExportOptions::default()` and the xtask `extract` doc to `all` once the content build consumes every map; Task A makes `all` work) and `--previews`. Wave 0 parses the flags and passes them through; `export` takes `&ExportOptions { maps: Vec<&'static str>, previews: bool }`, with Wave 0's default being `slice`.

### Content contracts (`crates/content`)
- **Splits** (mechanical):
  - `build/classify.rs` → `build/classify/{mod.rs (shared helpers: hex_names, idents_with_prefix, type_variant, …), field.rs (behavior, movement, gfx; owned by E), battle.rs (ability, move_effect, effect_stat, the `STRS` table; owned by G)}`. `UNWORDED_STAT_EFFECTS` moves from `build/battle.rs` into `classify/battle.rs`.
  - `build/codegen.rs`: `SCRIPT_NAMES`, `script()` and `scripts_rs` move to `build/scripts.rs` (owned by D2). `codegen.rs` keeps consts and data (owned by E).
  - `src/prelude.rs` → `src/prelude/{mod.rs (re-exports, unsupported, report), msgbox.rs, std.rs (callstd), special.rs, trainer.rs, items.rs, text.rs, menu.rs}`.
- **Prelude helpers, declared with default bodies** (each issues its Wave 0 `Request` and returns the host's default; the owning task fills the Gen3 behaviour; D2 lowers to these names only):
  ```rust
  // items.rs (H)
  pub async fn give_item(ctx: &Ctx, item: u16, qty: u16) -> bool;   // Std_ObtainItem
  pub async fn find_item(ctx: &Ctx, item: u16, qty: u16) -> bool;   // Std_FindItem
  pub async fn check_item(ctx: &Ctx, item: u16, qty: u16) -> bool;
  pub async fn money_add(ctx: &Ctx, n: u32); pub async fn money_remove(ctx: &Ctx, n: u32);
  pub async fn money_check(ctx: &Ctx, n: u32) -> bool;
  // text.rs (H)
  pub async fn buffer(ctx: &Ctx, slot: u8, src: BufferSrc);
  // menu.rs (H)
  pub async fn multichoice(ctx: &Ctx, list: u8, x: u8, y: u8, default: u8, cancel: bool) -> u16;
  // mod.rs: complete in Wave 0 (one request); the behaviour is host-side, in F's warps.rs
  pub async fn set_warp(ctx: &Ctx, slot: WarpSlot, map: u16, warp: Option<u8>, x: i16, y: i16);
  // trainer.rs (I)
  pub struct TrainerBattle { kind: u8 /* TRAINER_BATTLE_* 0..=12 */, trainer: u16, local_id: u8,
                             intro: Option<TextId>, lose: TextId, not_enough: Option<TextId>,
                             event_script: Option<ScriptId>, music: bool }
  pub async fn trainer_battle(ctx: &Ctx, b: TrainerBattle) -> bool;
  ```
  `TrainerBattle` covers every `trainerbattle` type: the nine regular ones (0–8: single, continue-script ± music, no-intro, double, rematch, continue-script double ± music, rematch double) and the four facility ones (9–12: Pyramid, set trainer A/B, Hill), which report `Unsupported`.
  - **`trainer_battle` returns whether the calling script goes on** with the code after the `trainerbattle` (`true`, Gen3 `gotopostbattlescript`) or stops (`false`). Generated code is only `if !trainer_battle(&ctx, b).await { return Flow::End; }`.
  - **I owns every kind's logic** in the helper: the trainer flag, rematches, the double not-enough path and the continue script.
  - The per-kind Gen3 semantics are in its doc comment. Kinds 10 and 11 return `false`, because Gen3 returns a NULL script there.
- **Samples:** `src/samples.rs`: `pub const SPECIES: &[&str]` and `pub const MOVES: &[&str]` (named decomp constants, resolved through `consts`; today's `SLICE_SPECIES`/`SLICE_MOVES` contents). Wave 0 migrates every consumer (`content/src/battle_tests.rs`, `explore/tests/f_policy.rs`, `studio/tests/data_views.rs`), so E only deletes `SLICE_*` and never edits `explore/**` or `studio/**`.
- **New build files:**
  - `build/scripts.rs` (Task D2): `pub fn generate(dir: &Path, out: &Path, w: &World) -> Result<ScriptsOut>`, today's path moved from `codegen.rs`.
  - `build/blob_data.rs` (Task E): stub.
  - `build/support/commands.ron` and `build/support/specials.ron`: generated once in Wave 0 from the decomp names (`scripts::OPS`, `names::SPECIALS`). Every command today's codegen lowers is `Native`; everything else is `Unsupported`.
  - **D1 decisions** (recorded for D2 and L):
    - `commands.ron` stays a top-level map;
    - D1 moves the movement fallbacks into a new `build/support/movements.ron`;
    - `Default(v)` classes emit no `Report`; only `Unsupported` reports.
  - `build/main.rs` calls both stubs.
- **`BUILD_ID`:** `build_id.rs` also hashes `build/support/*.ron` (today it hashes `.rs` files only), and `main.rs` emits `rerun-if-changed` for every `build/support/*.ron` (listed from the directory, so new tables such as `movements.ron` need no `main.rs` edit) and for the directory.
- **`src/overrides/mod.rs`:** `pub static OVERRIDES: &[&str] = &[];`, with a doc comment on the convention (one file per script, `pub async fn <Name>(ctx: Ctx)`, texts and movements by generated id).
- **Generated by the build (stubs in Wave 0, filled by D2 and E):** `content::data::{SCRIPT_DIGESTS: &[(&str, u64)], MAP_DIGESTS: &[(&str, u64)], GLOBAL_DIGEST: u64}`, keyed by decomp name, and `content::REPORT: &str` (the build's classification report, `include_str!` from `$OUT_DIR`). A map's digest covers its compiled data, its tilesets and its connected maps' edges. `GLOBAL_DIGEST` covers the battle tables, items and text tables, **the prelude and every non-generated content source**: a change there counts as global.

### Engine contracts (`crates/engine`)
- **`data.rs`:**
  - `id!` gains `ItemId`;
  - `pub struct ItemInfo { pub name: &'static [u8], pub pocket: Pocket, pub price: u16, pub key: bool }`;
  - `pub enum Pocket { Items, KeyItems, PokeBalls, TmHm, Berries }` (Gen3 order).
  - `pub enum Emote { Exclamation, Question, Heart }`; `pub struct Evolution { pub method: EvoMethod, pub param: u16, pub target: SpeciesId }` with `EvoMethod::{Level, Friendship, Item, Trade, Other}` (Gen3 `EVO_*` folded; the exact kind is kept for M9).
  - `GameData` provided methods (defaults `None`, or the empty slice). Every new method is added here in Wave 0; no Wave 1 task edits `data.rs` except I's `MovementType`:
    ```rust
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo>;
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]>;
    fn std_string(&self, id: u8) -> Option<&'static [u8]>;
    /// The trainer flag a trainer object's script checks (Gen3 `GetTrainerFlagFromScriptPointer`).
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId>;
    fn emote(&self, e: Emote) -> Option<&'static SpriteData>;
    /// Replaces `encounters(map)`: the land table of header `wild_set` of `map`. The engine
    /// passes the value of `SpecialIds::var_wild_set` (Gen3 `VAR_ALTERING_CAVE_WILD_SET`);
    /// maps with one header ignore it. Water, rock smash and fishing come later (M10).
    fn encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters>;
    /// Gen3 `gTrainerMoneyTable` value for the trainer's class (`GetTrainerMoneyToGive`).
    fn trainer_money(&self, id: TrainerId) -> Option<u8>;
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution];
    ```
  - `SpecialIds` gains `var_wild_set: Option<VarId>` and `script_whiteout: Option<ScriptId>` (Gen3 `EventScript_WhiteOut`, run immediately by `DoWhiteOut`).
- **`bag.rs` (new):** `Bag` with Gen3 Emerald pocket capacities (items 30, key items 30, balls 16, TMs/HMs 64, berries 46; 99 per slot; `include/constants/global.h`). It keeps an O(1) digest maintained on write, as `Vars` does (`flow/vars.rs:156`). `new`, `count`, `hash`, `snapshot`/`from_snapshot` are implemented; `add`, `remove` and `has_space` are `todo!()` (Task H).
- **`World` state** (implemented, hashed, snapshotted):
  - `bag: Bag`;
  - `money: u32`;
  - `str_vars: [[u8; STR_VAR_LEN]; 3]` (`STR_VAR_LEN` 0x100 as Gen3's `gStringVar1[0x100]`, at most 255 characters, always 0xFF-terminated, Gen3 `gStringVar1..3`). The digest covers each var up to and including its terminator, so a future length change does not change hashes;
  - `warp_slots: WarpSlots { dynamic, escape: Option<WarpDest> }`;
  - `field.stale: Option<StaleRect>`, always `None` until Task F.

  `World::hash` appends them in this order. `bag` and `str_vars` are hashed as **digests maintained on write** (like `Vars`), so each adds O(1) per frame, never a pass over the 186 bag slots or the 768 string bytes. Wave 0 measures `hash_lines_max` on the slice route and keeps it ≤ 65 (`HASH_LINES_MAX`, `emu/tests/parity.rs:109`). Accessors: `bag()`, `money()`, `str_var(i)`, `last_script() -> Option<ScriptId>`.
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
- **`world/host.rs` → `world/host/{mod.rs, items.rs (H), menu.rs (H), buffers.rs (H), trainer.rs (I), warps.rs (F)}`.** `mod.rs` routes **every** new request in `begin`, and the waiting ones (`Multichoice`, the sight approach) in `done`, `wait` and `resume`, to a function in its file, whose stub answers the default. After Wave 0 no Wave 1 task edits `host/mod.rs`.
- **`world.rs` split** (mechanical; today 757 lines, plus the already split `world/battle.rs`):
  - `world/maps.rs` (**F**): `load_map`, `cross_connection`, `start_warp`, `run_transition`, and `start_script`'s talk target (the old-map `selected`/`VAR_LAST_TALKED` rule);
  - `world/sight.rs` (**I**): `pub(super) fn check(…) -> Option<Sighting>` (returns `None`). Gen3's `ProcessPlayerFieldInput` checks trainers first on every field-input frame (`CheckForTrainersWantingBattle`, field_control_avatar.c:147), so `control` calls it twice:
    - first, before `map_script_on_frame`;
    - first in the `StepFinished` branch, before the coord triggers;
  - `world/battle.rs` (**G**): unchanged content, now owned by G (battle start and end, wild steps, whiteout);
  - `objects/spawn.rs` (**F**): all template spawning.
    - It holds `enter_map` (called by `load_map` and `cross_connection`), `spawn_templates`, `spawn_map`, `spawn_events`, `enter_connected`, `enter_connected_events`, `despawn_outside_view`, `add` (`addobject`) and the view constants.
    - It also holds the no-op hook `Objects::view_moved(templates, vars)`. `control` calls it once per finished player step (before the triggers) and once per crossing, so F never edits `world.rs`;
  - `world.rs` keeps `World`, `step`, `control`, `hash` and the accessors; owned by Wave 0 and edited afterwards only by L (seams) and the `chore(checkpoints)` commits (goldens).
- **Events** (implemented; not hashed, not snapshotted):
  ```rust
  pub enum Event { MapEntered(MapId), ScriptStarted(ScriptId), BattleStarted, BattleEnded(BattleOutcome), Report(Report) }
  impl World { pub fn events(&self) -> &[Event] }   // this step's events; cleared at the start of `step`
  ```
  `World::new`'s boot events (the start map entered, its map scripts started) stay visible through the first step. Events are not hashed, so this changes no hash.
  They are pushed where the World loads a map, calls `game.script(id, …)`, enters and leaves `Mode::Battle`, and handles `Request::Report`. Capacity is 32 per frame; extras are dropped and counted (`events_dropped`, gated at 0 by the sweep and the parity lanes).
- **Hooks with no behaviour yet:** `world/sight.rs` above; `field.rs` exposes `Field::stale()` (`None`; Task F).
- **Goldens per scenario.** `world/tests.rs`'s single `golden_hashes` (`:736`) becomes one constant per scenario (`GOLDEN_WALK`, `GOLDEN_WARP`, `GOLDEN_CROSSING`, `GOLDEN_SCRIPT`, `GOLDEN_WILD_BATTLE`, `GOLDEN_TRAINER`), so a route-changing merge updates only the constants it moves.
- **The hash change.** Update the goldens and the README, then `cargo xtask checkpoints verify --accept` on both trees (`chore(checkpoints)`). Run `cargo xtask parity`: hashes must match between ROM and native, and lines must stay within the gates.

### Native, xtask, explorer contracts
- `game-native`: none (the `Session` API is unchanged).
- **xtask:** `mod sweep;` with `Some("sweep") => sweep::main(&args[1..])` (Task L). `budget.rs` gains `pub fn sections(elf: &Path) -> Result<Vec<(String, u64)>>`, implemented, and the interim gates above (Task M re-baselines them).
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
- **Hash and checkpoint discipline:**
  - Wave 0 changes the hashed state once.
  - **Route-neutral tasks** (A, B, C, D1, E, K) merge first. Each must verify as `valid` or `valid (rehash)` on both trees **without `--accept`**. Their `BUILD_ID` refreshes are batched into one `chore(checkpoints): refresh build ids after A–E, K` commit on master.
  - **Route-changing tasks** merge one at a time, in this order: **D2 → H → J → I → G → F** (D2: lowering semantics such as `cond` and end-in-callee; H: `STR_VAR` expansion in the lab's texts; J: specials such as the clock fade; I: sight; G: battles; F: warp-load and crossing timing).
  - Route-changing branches **never commit tree or golden refreshes**. Each report includes a **route delta** note: which slice-route frames, goldens and nodes it expects to move, and why.
  - After each such merge, on master: re-accept both trees, update the golden constants it moved, run `cargo xtask parity`, and commit `chore(checkpoints): <task> <cause>`. The reviewer checks that the diff touches only build ids, hashes, digests and frame counts, and that the moved goldens match the route delta.
  - If the slice route's keys change, `cargo xtask checkpoints seed` re-records in the same chore commit.
- **GBA:** `late == 0`; bulk uploads only on black frames or spread under the line budget; the budgets of `cargo xtask rom` (new baseline from Wave 0, then gated).
- **Frame headroom.** After Wave 0 the slice route's worst frame (May's trainer-battle start) takes 215 of the 217 loop lines, a margin of 2.
  - A route-changing merge may not push `loop_lines_max` above 217. A task whose work would do so must spread that work over several frames.
  - H and I measure and report their per-frame cost (`cargo xtask parity` before and after).
  - G frees the main margin (G's item 9).
- **Before committing:**
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo clippy --workspace --all-targets --features game-native/window,studio/ui -- -D warnings`
  - `cargo test --workspace` with `PKR_ROM_FR=/Users/dode/Documents/pokemon/pokemon-rust/rom/emeraude-fr.gba PKR_ROM_US=/Users/dode/Documents/pokemon/emerald.gba PKR_DECOMP=/Users/dode/Documents/pokemon/pokeemerald`
  - `PKR_EXTRACTED_DIR=/nonexistent cargo test --workspace` (stub)
  - `cargo test -p studio --features ui` if you touched `studio`
  - `cargo xtask rom` and `cargo xtask parity` if you touched anything the GBA build compiles (engine, content, game-gba) or `emu`
  - `cd crates/game-gba && cargo fmt --check && cargo clippy -- -D warnings` if you touched game-gba
- **Extracted data in Wave 1:** Tasks A–C change what `extract` writes, and each bumps `FORMAT_MINOR`. Every other Wave 1 task works on the **Wave 0 extract** (`cargo xtask extract --maps slice`, format 5) plus its own fixtures. Integration onto the full import is Task L.
- **Ownership:** only touch the files your task owns, plus new test files named `<crate>/tests/m8_<task>_*.rs`. Describe any other change you need in your report. If a needed signature is missing from Wave 0, add it in your own file and list it; never edit another task's file.
- **Interim GBA gates** (Waves 1 and 2a): the per-section and absolute gates of Wave 0 Step 0; nothing re-baselines before M.

---

## Wave 1 (parallel, 11 agents)

Merge order: the route-neutral tasks (A, B, C, D1, E, K) as they finish; the route-changing ones (H, J, I, G, F) wait for D2 (Wave 2a) and then merge in the order of the rules.

### Task A — extract: every map, layout, tileset, sprite, encounter and pic

**Owns:** `crates/extract/src/{export.rs, maps.rs, sprites.rs, render.rs, battle_data.rs (encounters, heal locations, evolutions, trainer money), battle_gfx.rs (pics)}`, `crates/extract/tests/{export.rs, maps.rs, sprites.rs, battle_data.rs, battle_gfx.rs}`, and `extract()` in `xtask/src/main.rs`.

1. **Maps:**
   - `ExportOptions::maps` defaults to every `names::MAPS` entry in `gMapGroups` order (`--maps` restricts it; `--maps slice` gives `export::SLICE_MAPS`; fixtures use it). `export::MAPS` is deleted (its consumers already use `SLICE_MAPS` since Wave 0).
   - Write every layout (`layouts/`), including those reachable only through `setmaplayoutindex` operands. Scan the IR walk's `Const { table: "LAYOUT" }`, or simply export all 442.
   - Write every tileset, and `MAP_DYNAMIC` warps as such.
2. **Sprites:** every OBJ gfx id placed by any map or reached through `VAR_OBJ_GFX_ID_*` in any script (`var_gfx_ids` over all maps), plus both player sprites, plus the field-effect emote pics (`gFieldEffectObjectPic_ExclamationMark`, `…QuestionMark`, `…HeartIcon`) under `sprites/field_effects/`. Anchors as for object graphics; BPEE checked against the decomp's `graphics/field_effects/pics/*.png` dimensions.
3. **Encounters (schema v5):**
   - every wild header, in table order, as a `Vec` per map: Altering Cave's 9 sets, each keyed by its index (Gen3 `VAR_ALTERING_CAVE_WILD_SET`);
   - land, water, rock smash and fishing (with the old/good/super rod slot split, `gWildFishingSlots`).
4. **Pics:** front, back and shiny palette for **every** species with ROM graphics (Unown keeps form A, documented), and every trainer front and back pic that any trainer references. This replaces the slice set.
5. **Battle tables:** `gEvolutionTable` (`src/data/pokemon/evolution.h`; 5 entries per species) into `battle/evolutions.ron` and `gTrainerMoneyTable` (`src/battle_main.c`, `{class, value}` up to the `0xFF` terminator) into `battle/trainer_money.ron`. Anchored by structure, checked on BPEE against the decomp.
6. **Previews:** off by default (`--previews` writes them). The extracted dir drops from 144 MB to about 23 MB.
7. **Performance:** `cargo xtask extract` stays under 30 s in release (the spike: 5.5 s without the IR).
8. **Tests:**
   - BPEF and BPEE export with `--maps all`: 518 maps, 73 tilesets, the layout count, and the sprite count equal to the decomp's referenced set;
   - Altering Cave has 9 headers;
   - evolution and trainer-money tables equal the decomp's on BPEE;
   - every map's connections name exported maps;
   - every exported string round-trips;
   - `--maps LittlerootTown,Route101` exports exactly what those maps reference;
   - two exports are byte-identical (manifest sha1s).

### Task B — extract: script IR and decomp label transfer

**Owns:** `crates/extract/src/{scripts.rs, roots.rs, movement.rs, export/scripts.rs}`, new `crates/extract/src/{inc.rs, labels.rs}`, `crates/extract/tests/{labels.rs, script_ir.rs}`.

1. **`inc.rs`, a decomp source reader** (not an assembler):
   - For every `data/maps/*/scripts.inc`, `data/scripts/*.inc` and `data/text/*.inc` (36 files), and the inline labels of `data/event_scripts.s` itself (about 70, such as `EventScript_WhiteOut`), read the labels in source order. For each label: its file, its kind (script, text, movement, data), its fallthrough successor, and the **sequence of label references** in its body, in textual order, each with the operand kind implied by its macro (`goto`/`call`/`goto_if_*`/`call_if_*`/`trainerbattle_*` → script; `msgbox`/`message`/`loadword` → text; `applymovement` → movement).
   - From `data/maps/*/map.json`, read the object, coord and bg event script labels and the `map_scripts` table label. Follow `shared_scripts_map` (50 maps: `SecretBase`, `ContestHall`, `BattlePyramidSquare01`) and `shared_events_map` (`ContestHall`) to the map whose labels apply.
   - Macro knowledge stays a small table of reference-bearing macros (about 40, from `asm/macros/{event,map}.inc`); every other line contributes no reference. It must include: `switch`/`case`; `map_script` and `map_script_2` (`map.inc`); the `trainerbattle_*` forms with their optional arguments (`event_script`, `music`, and `not_enough_pkmn_text` for doubles); `bufferstring` (text); `pokenavcall` (text); and `vgoto*`/`setvaddress`, whose operands are RAM-relative and stay `Raw`.
2. **`labels.rs`, the transfer:**
   - Positional roots (`<Map>_Obj<n>`, `_Coord<i>`, `_Bg<i>`, map-script roots) map to `map.json` labels by index.
   - Then a parallel walk: for each decoded straight-line block and its decomp label, align the block's label operands (in instruction order, by kind) with the label's reference sequence. Equal length and kinds give `offset → label` for every target; follow fallthrough by the next-label chain.
   - A mismatch leaves that block's targets `Positional` and is counted per module.
   - Texts and movements take their decomp labels the same way.
   - The result is `BTreeMap<u32 /* offset */, (String, NameOrigin)>`. On a collision (one offset, two labels, as with shared tails), the first label in source order wins and the rest are recorded.
   - **Reverse consistency check:** one label that resolves to two different offsets is a transfer error; both offsets are demoted to `Positional` and counted.
3. **IR emission (`export/scripts.rs`):**
   - decode as today (`scripts::walk`), then group by label and resolve operands to names (`IrArg`);
   - assign each label to its decomp module (`<Map>` or `Common_<stem>`; positional ones stay in the first referencing root's map module);
   - write `scripts/<Module>.ron`, `scripts/index.ron` and `text/<lang>.ron` keyed by the final names.
   - `export` no longer writes `.rs` files once Task D2 lands. Until then, keep the old output alongside: Wave 0's `generate` keeps compiling.
4. **Without `PKR_DECOMP`:** every name is positional, with one warning.
5. **Tests (BPEF and BPEE, all maps):**
   - **coverage**, on BPEF: at least 95% of functions, of texts and of movements (each kind separately) are `Decomp` (print the % per module);
   - **precision**, on BPEE (there is no built decomp symbol map, so content decides): a `Decomp` text decodes (`decode_lang(En)`) to the label's `.string` source; a `Decomp` script's decoded op sequence equals its label body's macro sequence; a movement's actions equal its body. Target 100%, any mismatch listed;
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

### Task D1 — extract: `lower` and the support tables (Wave 1)

**Owns:** `crates/extract/src/codegen.rs`, `crates/extract/tests/codegen.rs`, `crates/content/build/support/commands.ron`, `crates/content/tests/m8_d_*.rs`.

D1 is tested on **hand-written IR** only; it does not touch the content build, so it is route-neutral.

1. **`codegen::lower`:**
   - Port today's structuring (functions, `loop { match pc }`, tail calls) from decoded `Insn`s to the IR.
   - `generate` becomes `decode → IR → lower` for the old path, and is deleted once D2 swaps the build.
   - **Dispatch as a static table of thunks:** `pub static SCRIPTS: [fn(Ctx) -> Script; N]` plus `SCRIPT_NAMES`, indexed by `ScriptId`, in place of today's one huge `match` (`build/codegen.rs:733`); `script(id, ctx)` is one bounds-checked call.
2. **Commands newly native**, lowered to the Wave 0 prelude helpers (by their Wave 0 names; their behaviour lands with H, I and J):
   - `multichoice`, `multichoicedefault` (`multichoicegrid` stays `Unsupported`);
   - `giveitem`/`additem`/`removeitem`/`checkitem`/`checkitemspace`/`finditem`;
   - `checkmoney`/`addmoney`/`removemoney`;
   - every `buffer*` with a `BufferSrc`;
   - `settrainerflag`/`checktrainerflag`/`cleartrainerflag`;
   - every `trainerbattle` type (0–12) into `TrainerBattle`;
   - `setdynamicwarp`/`setescapewarp`/`setwarp`;
   - `setwildbattle` + `dowildbattle` (a local pending battle in the function);
   - `messageautoscroll`/`messageinstant` (as `message`);
   - every `copyvar`/`loadword` form;
   - `setobjectmovementtype` everywhere.
3. **`Ignore`:** audio (`playse`, `waitse`, `playfanfare`, `waitfanfare`, `playmoncry`, `waitmoncry`, `playbgm`, …), `incrementgamestat`, `setweather`/`doweather`/`resetweather`, the money and coin boxes, `showmonpic`/`hidemonpic`, `setflashlevel`/`animateflash`, `dotimebasedevents`, `setstepcallback`, map popups. Each gets a one-line reason in `commands.ron`.
4. **Everything else is `Unsupported`:** `ctx.report(Report::Unsupported("<op>"))`, and the documented result (VAR_RESULT unchanged unless the command writes it, in which case the class gives `Default(v)`).
5. **Gen3 semantics fixed:**
   - `cond` across calls through `Ctx::cond`;
   - a callee's `end` stops its caller (the 80 sites): callees return `Flow::{Continue, End}`, and roots wrap them;
   - `waitmovement 0` stays "last mover", and the 80 unresolved sites are checked.
   - **Unknown movement actions:** map the 15 kinds to the closest `Step`, or to `Delay` with the frame count of the Gen3 action, listed in `commands.ron`'s movement section.
6. **Overrides in `lower`:** a name in `overrides` lowers to `pub async fn <Name>(ctx: Ctx) { crate::overrides::<Name>(ctx).await }`; an unknown name is an error.
7. **Digests** as in the Wave 0 contract: by name, over the call-graph closure (std scripts included) and the override source.
8. **Tests** (hand-written IR): structured flow, tail calls, `end` in callee, `cond` across a call, every newly native op, the thunk table, digest closure (changing a callee changes every caller's digest; changing an override's source changes its digest); **every `Native` command in `commands.ron` has a lowering arm** (a test walks the file).

### Task E — content: blob data, no slice sets, classification at scale

**Owns:** `crates/content/build/{codegen.rs (consts and data only), input.rs, battle.rs, battle_load.rs, blob_data.rs, classify/field.rs, main.rs, stub.rs, build_id.rs}`, `crates/content/src/{lib.rs, blob.rs, data_tests.rs, battle_tests.rs, new_game.rs, samples.rs}`. E never edits `explore/**` (K) or `studio/**` (S): Wave 0 already moved their consumers onto `content::samples`.

1. **Blobs:** map blocks and layouts, tileset tiles, metatiles and attributes, palettes, sprite frames, font glyphs and text bytes go into `$OUT_DIR/data.bin` (aligned per item, `Blob` slices), next to `battle.bin`. `data.rs` keeps headers, events, connections, ids and `&'static` slices into the blob.
   - **Acceptance on the spike's full extract:** `data.rs` ≤ 2 MB, a native debug build of `content` (data only) in ≤ 20 s, and rustc peak RSS ≤ 2.5 GB. Record all three.
2. **No slice:**
   - `slice_uses`, `slice_species`, `slice_moves`, `gen_slice`, `PLAYER_LEVEL_CAP`, `SLICE_SPECIES` and `SLICE_MOVES` are deleted (deletion only: no consumer is left after Wave 0). Pics are compiled for every species and trainer exported.
   - The unworded-stat-effect `ensure!` becomes a per-move fallback (the effect is `Unsupported` until G gives it words, through `classify/battle.rs`'s `UNWORDED_STAT_EFFECTS`) and is listed in the report.
3. **New tables:**
   - `ITEMS`, `MULTICHOICE`, `STD_STRINGS` and `MAP_SECTIONS` from Task C's files (Wave 0 schema; a fixture until C merges), behind `GameData::{item, multichoice, std_string}`;
   - encounters v5 behind `encounters(map, wild_set)` (header `wild_set` when the map has several, else the only one; `SpecialIds::var_wild_set` = `VAR_ALTERING_CAVE_WILD_SET`); water and fishing tables are compiled now and used by M10;
   - evolutions and trainer money from Task A's files (fixtures until A merges) behind `GameData::{evolutions, trainer_money}`;
   - `emote` from A's field-effect sprites (a fixture until A merges); `SpecialIds::script_whiteout` = `EventScript_WhiteOut`.
4. **`classify/field.rs`:** every one of the 256 behaviours classified, with the approximation list in the report; movement types mapped onto Task I's new `MovementType` variants (until I merges, keep today's mapping behind one function).
5. **`MAP_DIGESTS`** (by map name: the map's compiled data, its tilesets, and its connected maps' edge data) and `GLOBAL_DIGEST` (battle tables, items, text tables, behaviour table, the prelude and every non-generated content source).
6. **Overrides hygiene test:** every file under `src/overrides/` has no string or byte-string literal and no numeric array literal longer than 8 elements (the "no ROM-derived data in overrides" rule).
7. **Tests:**
   - the slice's `data.rs` content is equal before and after the blob move: every map's blocks, tiles and texts compared through the `GameData` API against a snapshot taken by the test from the Wave 0 build, so the parity route hashes are unchanged;
   - a fixture dir with 2 maps sharing a tileset dedupes it in the blob;
   - stub mode unchanged.

### Task F — engine: field at scale

**Owns:** `crates/engine/src/{field.rs, field/*, flow/transition.rs, flow/trigger.rs, grid.rs, world/maps.rs, world/checkpoint.rs, world/host/warps.rs}`, `crates/engine/src/objects/spawn.rs` (every template-spawning function and the `view_moved` hook, see the engine contracts; I owns the rest of `objects`), `crates/engine/src/world/tests/m8_f.rs`, and their tests. Route-changing; merges last of the ordered merges.

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
6. **Old-map objects after a crossing** (M3 finding): talking to one sets `selected`/`VAR_LAST_TALKED` as Gen3 does (`GetObjectEventIdByPosition` does not filter by map). Today's filter is in `start_script` (`world.rs:340`), moved to `world/maps.rs` by Wave 0.
7. **View-based object spawning** (Gen3 `TrySpawnObjectEvents`, `event_object_movement.c:1645`): templates spawn when they enter the camera view plus margin and despawn when they leave it, with the 16-object cap (`OBJECT_EVENTS_COUNT`). Today `load_map` spawns every template (`spawn_events`, in `objects/spawn.rs`), and Wave 0's `Objects::view_moved` hook (called by `control` per finished step and per crossing) is where the window moves. Spawning every template bounds neither the per-frame object cost nor OBJ VRAM on Route 104 and Rustboro (Wave 0's `loop_lines_max` measurement). Route-changing.
8. **Not in F:** door animation graphics, elevation-based OBJ priority on bridges, and the one-frame-early step triggers. These are documented as M10 (Act 1 does not need them).
9. **Route delta:** items 1, 2 and 7 change frame timing on warps and the hashed object set, so the slice route changes. The report carries the route delta; master refreshes per the rules, citing the M4 finding.

### Task G — engine: battle breadth for Act 1 (G1, the gate) and beyond (G2, stretch)

**Owns:** `crates/engine/src/battle/{effects/*, core.rs, core/*, calc.rs, ai.rs, text.rs, mod.rs}`, `crates/engine/src/world/battle.rs`, `crates/engine/src/mon.rs` (evolution only), `crates/content/build/classify/battle.rs` (`ability`, `move_effect`, `effect_stat`, `STRS`, `UNWORDED_STAT_EFFECTS`), `crates/engine/src/world/tests/m8_g.rs`, and their tests. Route-changing.

**G1 (gate; merges in the ordered sequence):**
1. **Act 1 effects:** implement exactly (Gen3 `battle_script_commands.c` + `data/battle_scripts_1.s`) every effect used by:
   - a move that any Act 1 encounter mon (Routes 101–104, Petalburg Woods, Rustboro) knows by its level-up learnset at its max level;
   - any move of a trainer party on those maps, including Roxanne's.

   Unworded stat effects get their Rose/Fell `Str`s (closing E's fallback).
2. **Act 1 abilities:** classify all 77. Implement every ability of an Act 1 species or trainer mon (Sturdy, Rock Head, Magnet Pull's battle part, Truant, Huge Power, Thick Fat, Shed Skin, Natural Cure, Cute Charm, Early Bird, Liquid Ooze, Clear Body, Soundproof, …). Weather-dependent ones (Swift Swim, Chlorophyll, Rain Dish) are no-ops until weather exists, and that is documented.
3. **Double trainer battles** (decision Q3): with fewer than 2 usable mons, the exact Gen3 not-enough path (the script's `not_enough_pkmn_text` branch; sight is I's side); with 2 or more, the battle runs as singles against the same party, with `Report::Degraded("double battle")`, and sets the trainer flag.
4. **Prize money:** Gen3 `GetTrainerMoneyToGive` (`4 × last mon's level × gTrainerMoneyTable[class]`, ×2 for doubles) into `World::money` on a win, through `GameData::trainer_money`, with its battle string.
5. **Whiteout** (M4 simplification): Emerald's `DoWhiteOut` (`overworld.c:358`): run `SpecialIds::script_whiteout` immediately (`EventScript_WhiteOut`: Elite Four and Mr. Briney resets), **halve the money** (Emerald has no badge-based loss), heal, warp to the last heal location. The Pokémon Center nurse line after a whiteout is FRLG-only; Emerald prints none.
6. **Evolution after battle** (level only, through `GameData::evolutions`; no item, trade or friendship): the species changes and stats recalculate, silently, with `Report::Degraded("evolution scene")`. The scene is M9.
7. **Tests:** golden roll tests per effect and ability (explicit `Rng` rolls, as M4); one Act 1 trainer party end to end in the battle test game; AI flags for Roxanne's party; prize money and whiteout money against Gen3 formulas.
8. **Route delta:** the slice route's battles change if a slice move's effect or a slice ability now acts; stated in the report, refreshed on master per the rules.
9. **Battle start spread over the transition** (frame headroom, see the Rules).
   - The trainer-battle start frame is the route's worst at 215 of 217 lines. Wave 0's scanline marks put `Battle::start` at 38 lines (the first turn and AI; 4 in a wild battle) and `BattleScene::new` at 34 in that one frame.
   - Run them across the transition's frames, as Gen3 does: the battle setup runs inside the transition's tasks, and the intro starts after it.
   - That frees about 70 lines. It is route-changing and hash-changing, and goes in the route delta.

**G2 (stretch, after G1 merges; route-neutral for Act 1 if it changes no Act 1 effect):** more effects by frequency across all trainer parties and learnsets toward **≥ 120 of 214** (order: `HIGH_CRITICAL`, `ALWAYS_HIT`, `TRAP`, `DOUBLE_HIT`, `PROTECT`, `RESTORE_HP`, `LEVEL_DAMAGE`, `RECHARGE`, `DEFENSE_CURL`, `MEAN_LOOK`, `YAWN`, `ATTACK_UP_HIT`, …; the frequency table in the report), and abilities toward **≥ 40 of 77**.

### Task H — engine: items, money, string buffers, multichoice

**Owns:** `crates/engine/src/{bag.rs, menu.rs (empty Wave 0 module), world/host/{items.rs, menu.rs, buffers.rs}, text.rs (placeholders), text/printer.rs (placeholders)}`, `crates/engine/src/world/tests/m8_h.rs`, `crates/content/src/prelude/{items.rs, menu.rs, text.rs}`, and their tests. Route-changing: `{STR_VAR_n}` expansion changes the printed text (and its timing) in the lab.

1. **`Bag`:** Gen3 `AddBagItem`/`RemoveBagItem`/`CheckBagHasSpace`/`CheckBagHasItem`: pockets by `ItemInfo::pocket`, 99 per slot, capacity per pocket, order kept. Its digest is O(1) per change.
   - `Bag::set_slot`'s `assert!` must stay unreachable from content: `add`/`remove` check bounds first, and an unknown item is a `Report`.
   - Measure the per-frame cost of the multichoice window and the buffer expansion (frame headroom, see the Rules).
2. **Money:** capped at 999,999 (Gen3 `MAX_MONEY`).
3. **String buffers:** `Buffer` fills `str_vars[slot]` from species, item and move names, `ConvertIntToDecimalString` numbers (Gen3 `STR_CONV_MODE_LEFT_ALIGN`), std strings, party nicknames and the lead species. `{STR_VAR_1..3}` placeholders expand in the printer (today they expand to nothing).
4. **Multichoice:** a Gen3 list window (`ScriptMenu_Multichoice`: position, width from the longest string, cursor, A selects, B gives 127 when cancellable, `multichoicedefault`'s start index) on BG0, sharing the text box's tiles and the yes/no cursor drawing. The cost is bounded per frame (the M3 text-budget rule).
5. **Prelude** (bodies of the Wave 0 helpers):
   - `give_item`/`find_item` follow `Std_ObtainItem`/`Std_FindItem`: the message with the item and pocket names through the buffers, the bag-full branch, and the item ball's removal for `find_item`, as today;
   - `check_item`, `money_*`, `buffer`, `multichoice`.
6. **Tests:** bag rules against Gen3 cases (full pocket, stacking, key items); buffer expansion bytes (synthetic charmap); multichoice input → `VAR_RESULT`, B → 127, default index; the hash changes only through the Wave 0 fields.
7. **Route delta:** the lab texts that use `STR_VAR_*` (stated in the report).

### Task I — engine: trainer sight, emotes, movement types

**Owns:** `crates/engine/src/{world/sight.rs, objects.rs, objects/* (except F's spawn.rs), trainer.rs, world/host/trainer.rs}`, `crates/engine/src/data.rs` (`MovementType` only), `crates/engine/src/world/tests/m8_i.rs`, `crates/content/src/prelude/trainer.rs`, and their tests. Route-changing. Measure the per-frame cost of sight (it runs on every field-input frame) and of the approach (frame headroom, see the Rules).

1. **Sight** (Gen3 `CheckForTrainersWantingBattle`, `TrainerSeeFuncList`), in `world/sight.rs` behind Wave 0's hook:
   - on every field-input frame and when a player step finishes (Wave 0 calls the hook first in both places, as Gen3 `ProcessPlayerFieldInput`), for each visible object with `trainer_type` normal or see-all-directions and a script whose `trainer_flag_of` flag is clear, look along its facing (or all directions) up to `trainer_sight` cells with no blocking cell or object;
   - a double trainer is skipped when the player has fewer than 2 usable mons (`trainer_see.c:216`, decision Q3);
   - on sight: lock the player, show the `!` emote over the trainer, walk it to the adjacent cell (`TrainerApproach`), face the player, then start the object's script. The `trainerbattle` intro and battle follow as when talked to.
   - Two trainers at once: the first in object order battles; the second is `Report::Degraded("double sight")` and waits for the next step.
2. **Emotes:** `Emote::{Exclamation, Question, Heart}` sprites through Wave 0's `GameData::emote` (`None` draws nothing).
3. **Prelude `trainer_battle -> bool`:** I owns every kind's logic. It returns `true` to go on with the post-battle code and `false` to stop; generated code only does `if !trainer_battle(..).await { return Flow::End; }`.
   - It covers the trainer flag, rematches (`IsTrainerReadyForRematch`), the double not-enough path, the second approaching trainer, and jumping to `event_script` after a continue-script battle.
   - Kinds 0–8 behave as Gen3 `trainer_battle.inc`. Kinds 9 and 12 report `Unsupported` and continue; kinds 10 and 11 report and stop (Gen3 NULL).
   - Wave 0's default body and its doc comment give the per-kind semantics.
4. **Movement types:** add exact variants for the walk sequences (24), face-two and three directions (12), rotations (2), and jog, run and walk-slowly in place (12). That gives **≥ 65 of 81 exact**. The rest (copy-player, disguises, buried, berry growth) keep approximations, reported.
5. **Tests:** sight geometry (range, blocking, facing, flag set → no sight, double trainer with one mon → no sight), the approach timing against Gen3 frame counts, a trainer battle started by sight in the world test game, each new movement type's step pattern.
6. **Route delta:** slice-route NPCs whose movement type becomes exact, and any slice trainer that now sees the player (stated in the report).

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
5. **Route delta:** route-changing (the clock fade after `StartWallClock` in Brendan's house, and any slice special that becomes `Native`); stated in the report, refreshed on master per the rules.

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
   - **As built (town goals):** `oldale` (slice goal 5), `petalburg` (9) and `rustboro` (13) are the visited flags `FLAG_VISITED_OLDALE_TOWN` / `_PETALBURG_CITY` / `_RUSTBORO_CITY` (set by each town's `OnTransition`), not "on the map". Reason: a goal is read only at capturable node ends, and the parity route crosses Oldale into its Pokémon Center without a capturable frame in the town, so "on `MAP_OLDALE_TOWN`" was never seen; the flag stays set. The routes (103, 104) keep "on the map": they have no visited flag.
   - `--act` selects the list (default `act1` on real data, `slice` keeps M7's).
   - The `(progress, -frames)` merge rule stays; it is now also consistent with the key for flag and var goals.
4. **Goal-directed segments:**
   - `run --from <checkpoint id|label>` roots the db at a store node (layered store, `Materializer`); `meta.root` records it.
   - Finding and node exports use `Parent::Checkpoint(id)`.
   - `export --to-store <node> [--label L]` writes a local-store node: the lineage's inputs from the root, with hashes by replay.
   - `cargo xtask checkpoints promote <local id> --label L` moves a verified local node into the committed tree. It is the only path into it, so M6 decision 7 holds.
5. **Events in place of `Probe`:**
   - **First, before E's content implementations reach the explorer:** forward every new `GameData` method in `explore/src/{probe,guard,testing}.rs` (`wild_encounters`, `item`, `multichoice`, `std_string`, `trainer_flag_of`, `emote`, `trainer_money`, `evolutions`), or drop `Probe` for `World::events()`. Otherwise the wrappers silently answer the defaults.
   - `SimSummary.scripts`, `maps` and `battles` come from `World::events()`.
   - `Probe` is kept only as a test wrapper, or deleted if unused.
   - **New `FindingKind::Unsupported`** (report-only): one finding per `(name, map)` from `Report` events, with the repro tail. Its signature has no digits, so each site is listed once.
   - The panic finding's `script` comes from `World::last_script()`, and the `CapturableStuck` and `RoundTrip` signatures gain `last_script`, so distinct traps on one map stay distinct rows (M7 close-out).
6. **Schema v3 clean-ups** (M7 close-out): `node.region` is written but no longer read (the probe uses the place): drop it, or store the place. Node and export labels use decomp constants only (`MAP_*`, `FLAG_*`), never FR map-section names.
7. **Optional:** a repeat-pair syntax for committed traces (`TRACE_SCHEMA` 3, the A/release alternation of battles), if P's Act 1 trace exceeds 1.5 MB. Either K or P takes it, not both, and then owns `crates/game-native/src/trace.rs` (`TRACE_SCHEMA = 2` today); it changes no hash.
8. **Tests:**
   - v3 round trip and size (a 2,000-node stub db is ≤ 40% of v2's);
   - determinism (threads 1, 4, 8) and resume digests;
   - the new key parts;
   - Act-list constants exist;
   - `--from` a committed stub node explores and exports a node that `cargo xtask checkpoints verify --local` reports `Valid`;
   - `promote` refuses an unverified node;
   - Unsupported findings from a fixture `GameData` that reports.

---

## Wave 2a — integration (after the route-neutral Wave 1 tasks merge)

Order: D2 (after B and D1), then the route-changing merges H → J → I → G1 → F (each followed by its `chore(checkpoints)` on master), then L.

### Task D2 — content: the build swap (Wave 2a, after B and D1 merge)

**Owns:** `crates/content/build/scripts.rs`, `crates/content/src/overrides/**`, `crates/content/src/prelude/legacy.rs` and the two `legacy` lines of `prelude/mod.rs` (`mod legacy;`, `pub use legacy::*;`; deleted with the legacy generator), `crates/content/tests/m8_d2_*.rs`; fix-ups in `crates/extract/src/codegen.rs`.

Route-changing (it is the first of the ordered merges): the slice's scripts now run D1's semantics (`cond`, end-in-callee, movement actions).

1. **`build/scripts.rs`:** read `scripts/*.ron` and the index, `support/*.ron` and the override sources (files under `src/overrides/`), call `lower`, and write `$OUT_DIR/scripts/<Module>.rs`, `scripts.rs` (one `pub mod` per module, `include!` from `OUT_DIR`), the thunk table, the digests and `report.txt`. `generate` and the old `.rs` export are deleted.
2. **Build checks:** an op or special in the IR missing from `support/*.ron` fails the build with the list. `report.txt` gives the counts per class and per op, and the `Decomp` name % per kind. One `cargo::warning` summary line.
3. **Overrides:** ship one override as the worked example (an end-in-callee site whose structure is clearer by hand), with a test. An override naming an unknown script fails the build.
4. **Trainer scripts:** `trainer_scripts` → content's `GameData::trainer_flag_of` table (generated into `scripts.rs`).
5. **Tests:**
   - `build_modes.rs`-style fixture dirs: an unclassified op fails, an override replaces, an unknown override fails;
   - the slice's parity route replays with identical ROM/native hashes on the new names; the route delta (D1 semantics) is stated in the report and refreshed on master per the rules.

### Task L — full import, end to end, and `cargo xtask sweep`

**Owns:** `xtask/src/sweep.rs`, cross-task fix-ups in the files of Wave 1 tasks A–J (each listed in the report), the README (Content, Scripts and Tests sections), the fixed test samples of `crates/studio/tests/{map_pixels, map_view, data_views}.rs`, and `checkpoints/*` refreshes.

1. Flip the extract default to `all`: `ExportOptions::default()` and the xtask `extract` doc and warning (Task A keeps `slice`, since the content build cannot consume every map before E). Then `cargo xtask extract` (all maps, format 5), then build everything. Fix the seams between A–J: IR names in the lowering, tables wired into `GameData`, `trainer_flag_of`, emotes, encounters v5.
   - **Remove `GameData::encounters(map)`** (kept by Wave 0 as `wild_encounters`' default so explore needed no change), once E implements `wild_encounters` and K forwards it.
2. **The slice still holds:**
   - the parity route replays with identical ROM/native hashes;
   - the committed trees verify `valid (rehash)`, or re-accept once with the cause: names changed, so `BUILD_ID` changed; behaviour changed only through the merges that announced a route delta;
   - the M5 checkpoint invariant tests pass.
3. **Test sampling** (moved from S): `map_pixels`, `map_view` and `data_views` use a fixed sample (the 15 slice maps plus Petalburg, Rustboro, Route 104, Petalburg Woods and one interior per tileset family). Whole-game versions are `#[ignore]` and run by `cargo xtask sweep --studio`.
3. **`cargo xtask sweep`** (release, native; the `#[ignore]`d sweeps run under it). For every map:
   - `World::new` on a `StartAt { map, pos }` wrapper `GameData`: the first warp's cell, else the first walkable cell nearest the centre;
   - 600 idle frames with no panic, ending capturable, or the reason, listed;
   - a checkpoint round trip with an equal hash, and 60 frames shadowed;
   - a render (no PPU validation error).

   It also aggregates `Report` events per op and special, and prints the build report (`content::REPORT`). Output goes to `target/sweep/report.txt`; exit 1 on any panic, invariant failure, or `events_dropped > 0`.
4. **`cargo xtask sweep --repro`** (in `sweep.rs`, so no new `main.rs` arm): builds `content` twice (native, clean `$OUT_DIR` each time) and diffs the two `$OUT_DIR`s byte for byte; fails on any difference.
5. **Acceptance:**
   - 518/518 maps pass, or each exception is listed with a reason (for example link-only rooms whose map scripts wait for a link); every sweep finding has a verdict, with zero untriaged Panic or RoundTrip findings;
   - the build gates of Wave 0 hold on the full import (native debug data plus scripts ≤ 30 s and ≤ 4 GB; GBA release ≤ 9 GB and ≤ 2 min), recorded;
   - `sweep --repro` passes;
   - `cargo test --workspace` wall time grows by at most 2× the Wave 0 baseline, recorded.

## Wave 2b (parallel, after L)

### Task M — GBA at scale: budgets, heap, map tour

**Owns:** `crates/game-gba/**`, `xtask/src/{budget.rs, main.rs: rom() and parity() only}`, `crates/engine/src/debug.rs`, `crates/emu/**`.

1. **Debug region v3** (`PKRDBG03`, size grows to 216):
   - `heap_max` (high-water of a counting wrapper around agb's global allocator);
   - `last_script` and `map` (from `World`);
   - `reports` (count of `Report` events) and `events_dropped`;
   - `stack_min` (IWRAM stack high-water: the shell paints the stack with a pattern at boot and scans it at the end of each frame's slack);
   - `panicked` (the GBA panic handler writes it and the panic location's line).

   `emu::DebugSnapshot` reads them.
2. **`xtask rom`:**
   - prints the ROM by section and by symbol prefix (`budget::sections`);
   - re-baselines the interim gates: ROM ≤ 16 MiB (decision Q4), IWRAM ≤ the full-import baseline + 2 KiB, EWRAM statics ≤ baseline + 16 KiB;
   - `xtask parity` gates `heap_max` ≤ 160 KiB, the stack high-water (≥ 1 KiB of stack never touched) and `events_dropped == 0` on every lane it runs;
   - record every number in "As built".
3. **Map tour** (`--features tour`, ROM-only, not in the normal build): boot, then on L+R+START warp to the next map of the tour list (first warp cell). An mGBA test (`emu/tests/tour.rs`, ignored, run by `xtask parity --long`) checks, on each map:
   - `late == 0`;
   - every black overrun follows a bulk step;
   - `loop_lines_max` ≤ 217;
   - no panic.

   **Gate:** the Act 1 maps plus one map per tileset family (primary/secondary pair). **Stretch:** all 518 maps. Any gated map over budget is fixed (with F's chunking or spawning) or listed with its cause.
4. **Compression:** only if the ROM exceeds the gate. LZ77 (Gen3's) for tileset tiles and pics, decompressed in black bulk frames, with IWRAM ARM code if the decompressor is hot. Otherwise record "not needed" with the size (M12's audio may bring it back).
5. **Follow-up: Emerald constants in the engine.** `MAX_MONEY` (999,999), the pocket capacities and `MULTI_B_PRESSED` (127) are engine constants today. Move them into `SpecialIds` (or `GameData`) per the game-agnostic rule, with no hash change.
6. **Contingency (only if `hash_lines_max` exceeds 65 on an Act 1 lane):** battle sub-digests maintained on write (M5 finding 1b), so the battle state hashes in O(1) as `Vars` does. It changes hash bytes, so it follows the route-changing merge rule (no tree refresh on the branch; a `chore(checkpoints)` on master).

### Task N — explorer on the full import: incremental revalidation and Act 1 tuning

**Owns:** `crates/explore/**`.

1. **Incremental revalidation** (M7 findings):
   - when only `content` changed (`BuildHash.engine` equal, `GLOBAL_DIGEST` equal), compare `SCRIPT_DIGESTS`/`MAP_DIGESTS` (keyed by name) with the ones stored in `meta`;
   - mark Stale only the nodes whose own or any ancestor's `node_touch` includes a changed script or map. Scripts are touched by name; a script's digest already covers its call-graph closure, std scripts and override source, and a map's covers its tilesets and connected maps (D1, E);
   - every other node becomes Valid on the new build without replay;
   - any engine, prelude, non-generated content source or other global change stays "revalidate all"; so does a name present in one build and missing in the other.
   - **Tests:** an override that changes one Littleroot script leaves every node that never touched Littleroot's scripts `Valid` with no replay; changing a std script that a touched script calls marks it Stale; a prelude change revalidates all.
   - Memory: revalidation at M7 scale (476k rows) peaked at 2.08 GB; with K's non-resident inputs it stays ≤ 1 GB (recorded).
2. **`Cross` ends one cell into the new map** now that the stale view is capturable (F): remove the walk-away and talk fallback (M7 Task A deviation), keeping a test that the old fallback path is gone.
3. **Probe cost** (M7 close-out): the worst case is `(1 + probe_resumes) × softlock_breadth` = 320 expansions per softlock check; tune both on the full import and record nodes/s before and after.
4. **Act 1 tuning:** `run --act act1 --secs 1800` from an empty db, then `--from` the furthest goal's exported node, in segments, until `FLAG_BADGE01_GET` or the budget runs out. Budget: **≤ 4 h total wall time, ≤ 45 min per segment.** Tune weights and limits in `Config::default` (the M7 order: goal weight, plan length, age penalty, cell shift). Record each attempt. If a segment stalls, a hand-authored scenario-recorder segment is allowed; it is labelled as such in the node's label and counted in the numbers.
5. **Elevation check:** confirm or refute "Act 1 has no elevation-sensitive path" (Route 104's bridges): count `PlannerDivergence` findings on Route 104 and Petalburg Woods during tuning; any is a finding for P (or N fixes the planner if trivial).

`explore sweep` (every map, 10 generations) moves to M9.

### Task S — studio at scale

**Owns:** `crates/studio/**` (except the three test files L samples).

Reduced to what the full import needs:
1. **Map tab:** a map picker for 518 maps (grouped by map group, with a filter by name or constant); `MapCache` stays an LRU of 32.
2. **Sampled tests:** the tests L moved onto the fixed sample stay green; nothing in `cargo test -p studio` iterates every map.
3. **M6 leftover:** a `Materializer` per worker thread (M6 finding 4): each pane and the explorer-open path get their own, with the shared disk cache.
4. **Gates:** all M6 gates, and `cargo xtask studio --smoke 8` ≥ 58 fps with the full import.

Items sub-tab, water/fishing encounter views, trainer lists and new overlays (dynamic warps, sight ranges) move to M9.

---

## Wave 3 — Act 1 route, parity, close-out

### Task P — Act 1 end to end

**Owns:** the `act1:*` nodes in `checkpoints/emerald/nodes/`, `crates/game-native/tests/act1.rs`, `crates/emu/tests/parity.rs` (the Act 1 lanes), and this doc's "Wave 3 as built".

1. **Find the route:** explorer segments (Task N's best config, within N's budget: ≤ 4 h total, ≤ 45 min per segment) from a new game to `FLAG_BADGE01_GET`. Export each goal's node (`export --to-store`), `verify --local`, then `cargo xtask checkpoints promote` as `act1:pokedex`, `act1:petalburg`, `act1:woods`, `act1:rustboro`, `act1:badge1` (labels from decomp names, never FR map-section names). The run's seed, config and wall time are recorded; any hand-authored segment is labelled and counted.
   - **Elevation:** check the route through Route 104's bridges against N's elevation finding; if a path depends on elevation, record it as a finding for M10 with its node.
2. **Native invariant along Act 1** (`act1.rs`, release, `#[ignore]`, run by `xtask parity`): the M5 core invariant at about 100 capturable points of the flattened Act 1 trace (round trip, equal hash, 120 frames equal), and 3 points to the end.
3. **ROM parity:**
   - every `act1:*` node resumes on the ROM from its `.sav` for 2,000 frames, with identical hashes and pictures every 100 frames, `late == 0`, `events_dropped == 0`, and `heap_max` and the stack high-water gated;
   - `cargo xtask parity --long` replays the whole Act 1 trace on mGBA from a new game, with every hash compared.
4. **Triage:** every finding and every `Unsupported` report reached on the Act 1 route goes into Findings for M9, or is fixed if trivial.
5. **Numbers:** total frames to the badge, the explorer wall time, maps entered, % of Act 1 cells reached, battles won/lost/ran, ROM/IWRAM/EWRAM/heap, worst lines per segment.

### Close-out
- **Whole-milestone review:** determinism (`sweep --repro`, explorer digests); write locations (nothing ROM-derived tracked: `git ls-files` against the ignore rules); FR literals; panics reachable from content; engine game-agnosticism. Fix the findings.
- **This doc:** "As built" per task (contracts changed, numbers), the findings table updated, "Findings for Milestone 9".
- **README:** the full-import workflow (`extract`, the build report, `sweep`, overrides), and the Act 1 parity lanes.
- **Cleanup:** clean up worktrees.

---

## Done when

- **Import:** `cargo xtask extract` (BPEF and BPEE) exports **518/518 maps**, 73 tilesets, every layout, every referenced sprite, the emote pics, pics for every species and trainer, every wild header (Altering Cave's 9), evolutions, trainer money, items, multichoice lists, std strings and map sections. **100% of exported strings re-encode** to their ROM bytes. No previews unless asked. Two exports are byte-identical.
- **Scripts:**
  - every reachable script (about 4,560 functions) is lowered by `content/build.rs` from the IR;
  - **≥ 95% decomp names, per kind** (functions, texts, movements, each measured separately) on BPEF, with **precision** measured on BPEE (B's content checks; 100% target, mismatches listed);
  - every command and special is classified (the build fails otherwise), and every `Native` command in `commands.ron` has a lowering arm (D1's test);
  - the build report lists the `Unsupported` sites by op;
  - **no `Unsupported` command or special is reached on the Act 1 route** without a recorded reason;
  - the overrides hygiene test passes (no string or byte-string literals, no large numeric arrays).
- **Run:** `cargo xtask sweep` passes on 518 maps (or a listed, justified exception set): no panic, idle-capturable, checkpoint round trip, render, `events_dropped == 0`. **Triage rule:** every sweep and Act 1 finding has a verdict (bug in Findings for M9, fixed, or accepted with a reason), and there are **zero untriaged Panic or RoundTrip findings**.
- **Act 1:**
  - the committed `act1:*` nodes reach `FLAG_BADGE01_GET` from a new game and verify `Valid`;
  - native invariant points pass;
  - every `act1:*` node resumes on the ROM with identical hashes, and `parity --long` replays the whole act identically;
  - `late == 0` and `events_dropped == 0` on every parity lane.
- **GBA:**
  - ROM ≤ 16 MiB;
  - IWRAM and EWRAM within M's gates;
  - `heap_max` ≤ 160 KiB and the IWRAM stack high-water within its gate on every parity lane;
  - `loop_lines_max` ≤ 217 on the slice route, the Act 1 segments and the gated tour maps (or re-gated ≤ 226 with the cause recorded); `hash_lines_max` ≤ 65.
- **Build:** reproducible (`cargo xtask sweep --repro`); on the full import, native debug `content` (data plus scripts) ≤ 30 s and ≤ 4 GB, GBA release ≤ 9 GB peak RSS and ≤ 2 min; `cargo test --workspace` wall time ≤ 2× the Wave 0 baseline. All numbers recorded.
- **Explorer:**
  - db ≤ 0.3 MB per 1,000 rows;
  - thread-count and resume determinism hold;
  - an override of one script revalidates only the touching subtree; a prelude or std-script change is handled as the rules say;
  - Unsupported findings are listed;
  - Act 1 goals are reached by segments within **≤ 4 h total and ≤ 45 min per segment**, hand-authored segments labelled and counted.
- **Folded findings:** every row of the findings table below is Done, Decided or Deferred with a milestone.
- **Gates:**
  - fmt and both clippy runs;
  - `cargo test --workspace` real and stub;
  - `cargo test -p studio --features ui` and `cargo xtask studio --smoke`;
  - `cargo xtask rom`, `cargo xtask parity` (and `--long` once, recorded);
  - game-gba fmt and clippy.
- **Hygiene:** no ROM-derived file committed (IR, generated Rust, tables, pics, db, snapshots); no FR literals in tests or overrides; `engine` names no Emerald constant.

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
| `DetectConfig::softlock_sample` unused (M7 H) | **Done in M7** (close-out) |
| `softlock_message` still says "region" (M7) | **Done in M7** (close-out) |
| `node.region` is written but no longer read; the probe uses the place (M7 close-out) | **Task K** (schema v3 drops it or stores the place) |
| `CapturableStuck`/`RoundTrip` signatures omit the script, so distinct traps on one map collapse into one row (M7 close-out) | **Task K** (`last_script` in the non-panic signatures) |
| Revalidation memory: 2.08 GB for 476k rows (M7 close-out) | **K** (inputs not resident), **N** (incremental revalidation) |
| Probe worst case `(1 + probe_resumes) × softlock_breadth` = 320 expansions (M7 close-out) | **Task N** (tune on the full import) |
| Content build: with `PKR_EXTRACTED_DIR` pointing at a missing dir, creating it later goes unnoticed until the env var changes (M7 close-out) | **Accepted**, documented in the README |
| `SlowFrame` is report-only and host-dependent (M7 close-out) | **Kept report-only**; parity's late-frame and scanline gates are the frame budget |
| Per-script and per-map content digests for incremental re-simulation (M7) | **D1**/**D2**/**E** emit them, **N** uses them |
| Engine `FrameOut` events in place of `Probe`; script-line coverage (M7) | Events: **Wave 0** + **K**. Line coverage: **M14** (explorer v1; needs per-block marks) |
| Engine-side panic context (M7) | **Wave 0** (`last_script`, events), **K** (findings), **M** (GBA panic handler) |
| Elevation-aware pathfinding (M7) | **Deferred to M10** with bridges (no `PlannerDivergence` in M7 H). "Act 1 has no elevation-sensitive path" is an assumption: Route 104 has bridges, so **N** checks it during tuning and **P** on the route |
| Balance fitness (M7) | **M14** |
| Goal-directed completability per checkpoint (M7) | **Task K** (`--from` segments, per-act goals) |
| Explorer exports as local-store nodes (M7 decision 14, M6 F3) | **Task K** (`export --to-store`, `xtask checkpoints promote`) |
| Own driver vs the scenario `Recorder` (M7 decision 1) | **Decided:** stay separate in M8; the Act 1 route comes from the explorer, so the scenario route stays frozen at the slice. Unify in M14 if useful |
| A per-thread `Materializer` (M6 F4) | **Task S** |
| `Parent::Edited` (M5 decision 17, M6 F5) | **Decided, deferred:** the state editor is M14; when it lands, edited nodes are local-only with a deterministic op list (`Parent::Edited { base, ops_digest }`), and `verify` reports them unverifiable in the committed tree |
| Data editing and write-back target (M6 decision 5) | **Decided:** overrides layer (decision 12) for scripts in M8; data patches over extracted data and studio write-back in M14 |
| Linux CI for the `ui` feature (M6 decision 3) | **Deferred** (M14) |
| Studio quit orphans an explorer run; nodes/s jitter (M7 S2) | **Done in M7 S2** (review fixes I1, M8). I1 used `ui::on_quit` on tao `LoopDestroyed`, not `RunEvent::ExitRequested` |
| Spread warp-load uploads, `late_black` → 0; crossings between different tilesets (M4) | **Task F**, checked by **M**'s map tour |
| Doors: no animation graphics (M4) | **M10** (cosmetic) |
| Old-map NPCs after a crossing, `selected` (M4) | **Task F** |
| Step triggers fire one frame early (M4) | **Deferred to M10** (it changes all route timing; not needed for Act 1) |
| Next script commands: `setrespawn` (done), `multichoice`, `trainerbattle`, `givemon` (done), `pokemart`, specials (M4) | **D1/H/I/J**. `pokemart` stays `Unsupported` (the Mart UI is M9); Act 1 does not need a purchase |
| Trainer sight; Attract/Rollout; doubles; `bufferleadmonspeciesname` (M4 simplifications) | Sight **I**; Attract/Rollout **G1** if Act 1 uses them, else **G2**; doubles per decision Q3 (**G1**, **I**); buffers **H** |
| Whiteout: no money loss, no whiteout script (M4 simplification) | **G1**: Emerald `DoWhiteOut` (run `EventScript_WhiteOut` immediately, halve the money, heal, warp). The reviewer's `EventScript_AfterWhiteOutHeal` is a pokefirered label; Emerald has no after-whiteout nurse or Mom script |
| Object spawning: every template spawns at load (no view window, no 16-object cap) | **Task F** (Gen3 `TrySpawnObjectEvents`), measured by Wave 0 on Route 104 and Rustboro |
| Battle state hashed in full each frame (M5 finding 1b) | **M** contingency: sub-digests on write, only if an Act 1 lane exceeds `HASH_LINES_MAX` |
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
  - money and coin boxes; policy switching and items in the explorer;
  - `explore sweep` (every map rooted at L's `StartAt`, 10 generations, findings triaged);
  - the studio views deferred from M8 S: Items sub-tab, water/fishing encounters, trainer lists, dynamic-warp and sight-range overlays.
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

1. **Scope: import everything, play Act 1 (taken; decision Q1).**
   - "Full engine parity" cannot be one milestone: the spike shows 90 unsupported command kinds and 433 specials, and menus, field moves, doubles, contests and audio are each milestone-sized.
   - Importing everything at once is cheap: the spike exported 518 maps in 5.5 s and compiled them. It is also what removes the slice's structural assumptions (names, slice sets, Rust literals, one wild header).
   - A story target proves the import works end to end. Roxanne is the first point the story needs items, money, trainer sight and broader battles, without menus or field moves.
2. **IR in `extract`, lowering in `content/build.rs` (taken).**
   - Today every codegen change needs a ROM and a re-extract (`FORMAT_VERSION` bump). M8 changes lowering in five tasks, and later milestones in every task. With the IR split, an engine feature is a build change that anyone with an extract can iterate on.
   - The build can apply overrides per function (it cannot drop one function from an `include!`d file today) and emit per-script digests (M7 finding).
   - The IR keeps the hygiene rule: it is ROM-derived and git-ignored like the rest of `content/extracted/`. The generator is committed; the generated Rust exists only in `$OUT_DIR`.
   - **Not decompiling the `.inc` sources:** the design makes the FR ROM the asset source (FR text pointers, FR-specific data). The decomp is a format and naming guide. Bytecode decoding already covers all 227 opcodes and 518 maps.
   - **Cost:** the content build lowers about 97k lines at build time. The probe shows generation takes about a second; the compile dominates, and E's blob move pays for it.
3. **Decomp label names (taken; decision Q2).**
   - Positional names are stable only for a frozen export order. Shared scripts are named after their first referencing root (the nurse is `OldaleTown_PokemonCenter_1F_Obj1_L0`), and adding maps before Oldale would rename them.
   - Decomp labels are stable, readable in the studio and explorer findings, and the natural key for overrides.
   - Label transfer needs only reference sequences, not an assembler. Mismatches fall back per block and are measured (≥ 95% target).
   - Snapshots hold no `ScriptId` (M5), so renames do not break checkpoints. Only `BUILD_ID` and tests that name scripts change.
4. **Classify, never crash (taken).**
   - At 1,543 unsupported sites, "log and continue" must be explicit per command. Some commands write `VAR_RESULT` (a default must be chosen); some are cosmetic (`Ignore`, no noise); some are real gaps (`Unsupported`, counted and surfaced by the explorer as findings).
   - Failing the build on an unclassified command forces a decision when the IR gains one.
5. **Remove the slice structurally (taken).** The slice was encoded as constants (`MAPS`, `slice_*`, `SLICE_*`, `PLAYER_LEVEL_CAP`) and as single-instance assumptions (one wild header, previews always). Each is replaced by "everything, filtered by explicit options", and tests move to named fixed samples.
6. **Binary blobs for bulk data (taken).** The spike's 11.7 MB `data.rs` compiled in 31 s at 4.96 GB of rustc memory, and the GBA build peaked at 8.9 GB. `battle.bin` has shown the blob pattern since M4. Rust literals stay for small typed tables, where type checking matters.
7. **GBA budget (taken; decision Q4).**
   - The spike's full ROM is ≈ 12.4 MB at opt-level 3, and ≈ 10.2 MB with `content` at "s". Projected with every pic: 11.4–13.6 MB. That is under 16 MiB, so **no banking** (the GBA has none to do below 32 MiB) and **no compression** unless the gate trips.
   - Script code is 59% of the ROM at scale, so "s" on `content` is the cheap lever. Scripts are light per frame, so speed is not at risk; Wave 0 checks the lines.
   - Futures are tiny (host max 200 B; on the GBA smaller with 4-byte pointers), so the M1 risk "box large futures in EWRAM" does not materialize. Heap high-water joins the debug region to keep it measured.
   - **Compile memory** (8.9 GB peak for the GBA LTO build) is a dev-machine cost. Dev machines with 16 GB are supported, so it is gated (≤ 9 GB, ≤ 5 GB stretch), reduced by decision 6, the thunk dispatch table, `debug = false` on `content` and `build-override` opt-level 3, and recorded.
   - **Interim gates:** a full-import ROM can only be built from Wave 2a on, so Waves 1 and 2a gate the slice build per section (an engine regression cannot hide behind content growth) and the full import against 16 MiB absolute.
8. **One hash change in Wave 0; announced route changes (taken).**
   - The M5/M6 practice was "only one task changes hash bytes". M8 adds state in several tasks, so the state lands at once, unused, with one re-accept.
   - Behaviour that genuinely changes the slice route (lowering semantics, text buffers, specials, sight, battles, warp timing and spawning) cannot be avoided. Six parallel branches each re-accepting the same trees would conflict on every node file and hide which change moved which frame. So route-changing branches never commit refreshes; they merge one at a time in a fixed order (D2 → H → J → I → G → F), and master re-accepts after each in a reviewed `chore(checkpoints)` commit with the cause, so a tree change is never silent and always attributable.
   - The new state is hashed through digests maintained on write, so the hash's per-frame cost stays within `HASH_LINES_MAX` (65).
9. **Act 1 parity in segments plus one long run (taken).** A 150k–300k-frame ROM replay is too slow for every `xtask parity`. M5's "ROM resumes from native checkpoints" gives segment parity at every `act1:*` node in seconds. The full straight replay is `--long`, run at close-out and before releases. The map tour covers maps the route never visits.
10. **The explorer finds the Act 1 route (taken).** The scenario recorder's hand-written route (M3–M5) does not scale to an act. The explorer already reaches the slice's goals in 1.7 s. With `--from` segments and per-act goals, its exported lineage becomes the committed route, and that route is input-only and verifiable like any node.
11. **French text and offsets (taken).**
    - No offset is hard-coded: maps, tilesets, scripts and texts are found by anchors and pointer tracing, and the spike read all 518 map headers and round-tripped all 5,503 reached texts on BPEF.
    - The new C-side tables follow the same rule: located by structure, validated on BPEE against the decomp, every FR string round-tripped.
    - If a table differs in layout in BPEF (for example longer FR names in a fixed-width field), the reader takes the width from the ROM's structure, never from the decomp's `#define`.
12. **Overrides only for scripts in M8 (taken).** M6 deferred "where edits live" to M8. Script overrides are needed now (worked example in D2) and cost little. Data patches need a patch format and studio write-back, which is M14 work with no M8 consumer.
13. **Release-only sweeps (taken).** 518 maps × (load, 600 frames, round trip, render) is minutes in release and far too slow in debug. Debug tests keep the fast red/green loop on fixed samples; the sweeps are gates at the end of Wave 2a and at close-out. `explore sweep` is deferred to M9: it needs a tuned explorer on the full import, which is N's output, and it does not gate Act 1.
14. **pokeemerald-expansion not used (taken).** It changes mechanics toward later generations and restructures data. Vanilla behaviour is the spec; the expansion is consulted only for readability.

---

## Size estimate

| Wave | Tasks | Size (agent sessions) |
|---|---|---|
| 0 | contracts, spike re-run, hash change | 1 large |
| 1 | A (L), B (L), C (M), D1 (M), E (M), F (L), G1 (L), H (M), I (M), J (M), K (L) | 11 parallel |
| 2a | D2 (M), the ordered route-changing merges, L (L) | 2, sequential |
| 2b | M (M), N (M), S (S), G2 (M, stretch) | 3–4 parallel |
| 3 | P (L), close-out (M) | 2 |

- **In total:** 19 task sessions (20 with G2), against M7's 10. About 1.9–2.6× M7's effort.
- **Hand-written code:** about 20–30k lines including tests (M7 added 13k in `explore` and 10k in `studio`). The generated code (about 100k lines of Rust plus blobs) lives in `$OUT_DIR` and is not counted.
- **Critical path:** Wave 0 → **B → D2 → L → N → P**. D1 runs in parallel with B, so D2 is a short swap once B lands; the route-changing merges sit between D2 and L and are each one rebase plus one chore commit.
- **Fallback split (only if L slips):** **M8a** = import, classification, budgets and the explorer (Wave 0, A–E, K, D2, L, M's gates, the sweep); **M8b** = Act 1 (F–J's remaining behaviour, N, P). Decided as a fallback only; the plan runs as one milestone.
- **Main risks:**
  - the label transfer's match rate (B), with the positional fallback as the floor;
  - the explorer reaching the badge within budget (N/P), with `--from` segments and tuning as levers, and the scenario recorder as a last resort for one hard segment;
  - GBA frame budget on maps with many objects (Wave 0's Route 104/Rustboro measurement, F's view-based spawning, M's tour);
  - build memory on 16 GB machines (Wave 0's measurements and profiles; L's gates).

---

## Wave 0 as built

Commits on top of `cbc9a15`, merged with master `dcaa10e` (the M7 close-out; no conflicts). Extract contracts `a823b04`, engine contracts `4c79485` (Wave 1 B, C and K started from these).

### Spike (re-run on the merged tree; throwaway branch, scratch extracted dir, all 518 maps)
Apple M4 Max, with other Wave 1 builds running at the same time: treat the times as upper bounds.
- **Export:** 6.0 s, 67 MB RSS. 518 maps, 156 sprites, 4,560 script fns, 5,503 texts, 978 movements, 3,493 files. Extracted size: 147 MB, of which previews are 124 MB (23.6 MB without previews).
- **Native debug build of `content`** (data plus scripts): 30.0 s wall, rustc 26.9 s, **peak RSS 4.89 GB**. `data.rs` is 11.7 MB.
  - Build script: 2.7 s, of which generation is 2.5 s at opt-level 0. With `build-override` at opt-level 3, generation takes 0.35 s and the build 27.8 s.
  - Against the build gates: the time is at the 30 s gate, but the RSS is **over the 4 GB gate**. Task E's blobs (no Rust literals for bulk data) are the planned fix, and L checks it.
- **Largest script future:** 200 B (median 160 B, minimum 16 B, over 4,560 roots).
- **GBA release builds** (fat LTO unless noted):

  | content opt-level, LTO | time | peak RSS | ROM | `.text` | `.rodata` | `content::scripts` `.text` (+ generics) | `content::data` | `content::battle` | `engine` `.text` |
  |---|---|---|---|---|---|---|---|---|---|
  | 3, fat | 163 s | 9.02 GB | 12,447,176 | 7,760,232 | 4,685,380 | 5,542,384 (+1,721,994) | 2,458,530 | 1,167,380 | 195,896 |
  | **"s", fat (taken)** | **113 s** | **5.11 GB** | **10,178,376 (−18.2%)** | 5,491,424 (−29%) | 4,685,388 | 4,083,034 (+950,454) | 2,458,530 | 1,167,386 | 195,596 |
  | "s", thin | 141 s | 4.84 GB | 11,055,416 (+8.6% over fat) | 6,333,388 | 4,720,284 | 4,435,644 (+1,419,842) | 2,458,564 | 1,167,386 | 202,416 |

  - **LTO: fat kept.** Thin LTO is 8.6% larger (the gate allows 3%) and slower; it saves only 5% of memory.
  - Against the build gates: "s" with fat LTO is within the GBA release gates (≤ 9 GB, ≤ 2 min), and its 5.1 GB is just over the 5 GB stretch.
- **`loop_lines_max` on Act 1 maps.** Spike ROM, 3,000 frames: 600 idle, then walking a square. Every template spawns.

  | start | hash | step | loop | at opt-level |
  |---|---|---|---|---|
  | Route 104 (0x0013 at 17,51) | 42 | 221 | **225** | "s" and 3 alike |
  | Rustboro (0x0003 at 27,20) | 23 | 160 | 164 | "s" |
  | new game | 8 | 141 | 145 | "s" |

  - Route 104 is over the 217 gate whatever the opt-level, because of the object cost. Task F's view-window spawning has to bring it under the gate, and M's map tour checks it.

### Decisions
- **`content` at opt-level "s" on the GBA**, in `crates/game-gba/Cargo.toml`, with `debug = false`. It saves 18% of the full ROM, 29% of `.text`, 31% of the build time and 43% of the build memory.
  - On the slice it saves 13% of the ROM (2,022,576 → 1,761,240 B).
- **The slice route's frame gate first failed with "s".** Measured by `cargo xtask parity` across the Wave 0 commits (`content` at opt-level 3 unless noted):

  | Tree | `loop` lines on the route |
  |---|---|
  | `a823b04` and the split `6d77e05` | 214 |
  | the requests and events `4c79485` | 216 |
  | the hashed state and contracts `fce358f` | 217 |
  | `fce358f` with `content` at "s" | 220 |

  - The worst frame is May's trainer-battle start (frame 16185).
  - Scanline marks on that frame (scratch-only instrumentation) found where the step time goes:
    - `Battle::start`: 38 lines, mostly the first turn, against 4 in a wild battle;
    - `BattleScene::new`: 34 lines;
    - the rest of the field step: 47 lines;
    - **`Box::new` of the ~4 KiB `BattleRun` literal: about 6 lines**, a stack-to-heap copy (the route's loop went from 220 to 215 lines when it was removed).
  - Fix in `59b1114`: `start_battle` builds the run in its heap allocation (`Box::new_uninit` and field writes). This is the engine's first `unsafe` outside `hash.rs`; the hash and behaviour are unchanged.
    - A destructuring guard next to it (`const _: fn(BattleRun) = |BattleRun { .. every field .. }| {};`) fails to compile when a field is added, so the field writes cannot miss one.
  - With "s", the route now takes **hash 63 (gate 65) and loop 215 (gate 217)**.
- **Build scripts** run at opt-level 3 (`[profile.*.build-override]`) in both workspaces.

### Baselines
- **Slice ROM:** `cargo xtask rom` gives 1,761,240 B, IWRAM 788, EWRAM 460. These are now `budget::BASELINE`; it was M5's 1,992,712 / 596 / 460.
  - Engine `.text`: 205,448 (`ENGINE_TEXT_BASELINE`).
  - `budget::sections` breaks down the slice: `content::scripts` `.text` 404,964 (+114,220 generics), `content::data` `.rodata` 323,500, `content::battle` `.rodata` 351,398, other `.text` 83,080, `other(engine)` `.text` 27,204.
- **Interim gates** (`xtask/src/budget.rs`):
  - the ROM is reported, not gated, on the slice and the stub;
  - on a full import (`MAP_COUNT` > `SLICE_MAPS`), the ROM is gated at 16 MiB;
  - on the slice, engine `.text` is gated at the baseline + 128 KiB;
  - IWRAM and EWRAM are gated at the baseline + 2 KiB / 16 KiB.
- **Slice parity** (content at "s"): hashes and pictures are identical, with late 0. Hash and loop lines per lane:

  | lane | hash | loop |
  |---|---|---|
  | lab | 41 | 163 |
  | oldale_center | 59 | 199 |
  | before_may | 63 | 214 |
  | route | 63 | 215 |
  | mid-battle save | 43 | 167 |
  | save run | 43 | 167 |

- **Explorer:**
  - `explore run --generations 50`: 3,822 nodes, 4,042,998 frames, digest `0x65ddd0bdcfb76fc6`, the same on 1 and 8 threads. It takes 18.7 s on 1 thread and 2.6 s on 8.
  - `run --secs 60` (8 threads, other builds running): 52,298 nodes and 60.5 M frames (871 inserted nodes/s, 1.0 M frames/s). The db is 95.2 MB, about 1.8 MB per 1k nodes.
  - Stub `--generations 50` digest: `0x8eae1debc306e2f0` (222 nodes), identical on 1 and 8 threads.
- **`cargo test --workspace`:**
  - before Wave 0: real 1066 passed / 11 ignored in 3:15; stub 1056 / 11 in 2:40;
  - after (merged tree): real 1079 / 11 in 4:28, stub 1069 / 11 in 3:17 (wall times inflated by concurrent Wave 1 builds).

### Contract deviations (with reasons)
- **Extract:**
  - The legacy script index is `scripts/legacy_index.ron` (`LegacyScriptIndex`) next to the IR schema, so the slice codegen keeps working until D2 swaps it.
  - The slice trainer-battle helper moved to `prelude/legacy.rs` as `LegacyTrainerBattle` / `legacy_trainer_battle`, freeing the `TrainerBattle` name for the contract.
  - `export::MAPS` stays as an alias of `SLICE_MAPS` until Task A removes it.
  - `ExportOptions::default()` keeps previews on. `--previews` is parsed; Task A flips the default.
- **Engine:**
  - `GameData::wild_encounters(map, wild_set)` is added and defaults to `encounters(map)`. `encounters` is kept so `explore` (Task K's crate) needs no Wave 0 change.
  - `Request::TrainerApproach { obj }` was added (hash tag 54). Task I needs a host-side approach walk that the sight contract did not list.
  - `Field::stale()` returns the existing stale rect, not `None`, because the rect already existed and is hashed now.
  - `Pocket` is in save-block order (`Items, KeyItems, PokeBalls, TmHm, Berries`), with capacities 30/30/16/64/46.
  - `start_battle` builds its `BattleRun` in place (`unsafe`, see Decisions).
- **Content:**
  - `scripts::generate(dir: Option<&Path>, out, w, &Support)`: it takes the support tables and an optional extracted dir, so the stub build shares the path.
  - Sound commands are classed `Ignore` in `build/support/commands.ron`, not `Unsupported`.
  - Of the specials, 3 are Native, `PlayerFaceTrainerAfterBattle` is `Ignore`, and `ShouldTryRematchBattle`, `IsTrainerRegistered` and `HasAllHoennMons` are `Default(0)`.
  - The samples gained `SPECIES_IDS` and `MOVE_IDS` companions and a per-flavour `cfg(pkr_stub_battle)`, so the stub build has samples too.
- **xtask:** `budget::sections` demangles with `rustc-demangle` (the GBA build uses v0 mangling) and lists every allocated section.

### Review fixes (after the Wave 0 review)
- **Engine:**
  - `BattleRun` field guard (above).
  - `objects/spawn.rs` holds every template spawner. `Objects::view_moved` is a no-op hook, called by `control` once per finished step and once per crossing.
  - `sight::check` runs first, as Gen3 does: before `map_script_on_frame`, and before the coord triggers in `StepFinished`.
  - `World::new`'s boot events stay visible through the first step. This does not change the hash.
  - `menu.rs` and `world/tests/m8_{f,g,h,i}.rs` are empty modules, so H, F, G and I never edit a shared file to add them.
- **String vars:**
  - `STR_VAR_LEN` is 0x100 (Gen3 `gStringVar1[0x100]`, 255 characters plus the terminator). The digest covers each var up to and including its terminator; it stays O(1) per frame (updated only on writes).
  - This changed every hash that a string var touches, so new goldens and both checkpoint trees were re-accepted:
    - goldens: walk `0x66f3_9427_d1ed_0a8f` / `0x0818_a372_6436_0e30`, warp `0x779c_8acf_63d1_6570`, crossing `0xc2b0_b5dc_632d_bb70`, script `0x0fab_8f3d_8206_46d3`, wild battle `0xf5f2_70e6_418e_c1ac`, trainer `0x53fa_55c0_cec2_d61b`.
  - The vars live in the heap `World`, so IWRAM 788 and EWRAM 460 are unchanged, and the ROM is 1,761,000 B.
  - Parity is unchanged: route hash 63 and loop 215; before_may 63 / 214.
  - Explorer `--generations 50` digests (identical on 1 and 8 threads): real `0x010478a4b7a98232` (3,822 nodes), stub `0x6d65a68bea182f33` (222 nodes).
- **Content:**
  - `trainer_battle -> bool` owns all 13 kinds. Its doc comment gives Gen3's per-kind semantics; the default body keeps today's behaviour.
  - A minor-version mismatch lists the missing files (`schema::MINOR_FILES`, `missing_since`).
  - `main.rs` watches every `build/support/*.ron`.
- **`cargo test --workspace`:** real 1081 / 11 ignored, stub 1071 / 11.

### Findings check
M7's final "Findings for Milestone 8" (items 1–18) all appear in "Findings folded in" with an owner, so nothing was added or dropped. Wave 0 handled its share of two rows:
- events and `last_script` (engine panic context, `FrameOut`);
- the stale rect, now hashed and in `FieldSnap`.

## Task K as built (Phase 1: items 1–4, 6)
- Schema v3 (`c4252e1`): blob inputs with a repeat-pair opcode, `Macro::code` integer macros, no `region` column, no `node(key)`/`node(map)` index, `key_hit` table, `meta.key_version` / `act` / `root`. A superseded row stays even when childless (it was live and written); only duplicates leave no row.
- Key version 2: `respawn` and a template-delta digest in the key; `VAR_OBJ_GFX_ID_0..F` excluded. The key-items pocket digest waits for Wave 0's bag (Phase 2, key version 3).
- `TRAINER_GRUNT_PETALBURG_WOODS` is missing from the stub's `content::consts`: the goal reads it from `extract::names::TRAINERS` (the table the real constant is generated from). All 14 Act 1 constants are checked against the decomp headers by `tests/m8_k_goals.rs`.
- 120 s real run: 133,155 rows, 35.2 MB (0.26 MB per 1,000 rows; M7: 167,643 rows, 175.3 MB, 1.05 MB), peak RSS 184 MB (M7: 455 MB).
- **Deviation (RSS):** the target is stated at 600 s (≤ half of M7's 1.06 GB); only 120 s runs were measured, M7 and v3 on the same machine and load. v3 is at 40% of M7 at 120 s; the 600 s figure is an extrapolation, not a measurement.
- **Deviation (size test):** the spec's "a 2,000-node stub db is ≤ 40% of v2's" is not tested: the stub's 196 cells hold about 32 keys (no 2,000-node stub db), and no v2 writer remains to compare with. Instead `tests/m8_k_db.rs` checks, on both flavors, that blob inputs are ≤ 40% of M7's text codec for the same rows, that no duplicate leaves a childless row, and that hits exceed rows; the real-build size gate (≤ 300 B per row on 2,000 inserted nodes; 281 B measured) is `#[ignore]`d in debug (85 s of CPU) and runs in release with `--ignored`.
- `promote` replays the node on the current build from its parent's state (never its cached snapshot) and refuses a stale or diverged node even when `status.ron` says Valid (it carries no build). Local children move with `Store::reparent` (atomic, the store's format); on a partial failure the error says how to recover. `export --to-store` writes only into a layered (local) store.
- 50-generation digests, identical on 1, 4, 8 threads and 25+25: real `0x02157751a7452ae3`, stub `0xf68ea453a6b24e45`.
- **Task S item:** `crates/studio/src/native.rs:180` refuses traces whose parent is not a new game, so findings of a `--from` db (traces from `Parent::Checkpoint`) do not open in the studio pane.

## Task K as built (Phase 2: item 5, key version 3)
- `Probe` is deleted: sessions are `Session<G>`. `SimSummary.maps` / `scripts` come from `MapEntered` / `ScriptStarted`; a battle is tracked from `BattleStarted` to the frame the field is back, with the `BattleEnded` outcome. Dropped events (past `EVENTS_MAX`) become an expansion note.
- `FindingKind::Unsupported` (report-only, `kind_name` "unsupported"): one per (report, map) per job, message `unsupported: NAME` / `degraded: NAME`, tail ending on the reporting frame; the db merges the rest by signature (`hits`). It reproduces when the replay's last frame has the report. Fixture: `testing::Reporting` (every script reports first), `tests/m8_k_events.rs`.
- **Deviation (last script):** the finding's `script` is the last `ScriptStarted` over its tail (the job's steps, then the failed expansion, the panicking frame included), not `World::last_script()`: that resets on restore, and the explorer restores mid-job (a failed random try), while a replay from the root would count the lineage too. `reproduce` folds the same events over the tail's frames. `CapturableStuck` and `RoundTrip` signatures include it; other kinds do not.
- Key version 3: the key items held (`KeyItems` pocket ids, sorted, so the order obtained does not count) join the key, not the place. Pin: key `0xc72f_46f9_8e19_820c` (place unchanged).
- Every `GameData` method is forwarded by the test games through `explore::forward_game_data!` (exported, absolute paths): the `testing` wrappers (`Drifting` drifts `wild_encounters`, which the engine reads, as well as `encounters`), the guard's `Armed<G>` and `a_expand`'s `Fake<G>` (which panics in `wild_encounters` like `encounters`). `testing::same_answers` compares every provided method against `testing::Answers`, a game answering each one on its own.
- A report on a warp frame is attributed to the map after the warp (`Reported::map`). New (report, map) pairs past `MAX_REPORTS` per expansion are counted in a note.
- Report-only findings are counted apart: `FindingKind::report_only`, `Db::report_count`, `GenOutcome`/`RunSummary.reports`, the `run`/`status` lines (`findings` are the bugs, `reports` apart), `explore findings --bugs`. `DbStats.findings` keeps counting every row (the studio reads it).
- Tests (`tests/m8_k_events.rs`): a stub `FadeTrap` run raises a `CapturableStuck` row signed with `FADE_TRAP_SCRIPT` that reproduces, and not without its script; `Unsupported` rows are the same on 1 and 2 threads; reports past `MAX_REPORTS` make a note. `key_items_digest` is pinned on an unsorted mixed-pocket bag (`0x89d1_c97b_d01a_7c7c`).
- 50-generation digests, identical on 1, 4, 8 threads and 25+25 (after merging master `74668c1`): real `0x6cf00f30af70d35d`, stub `0x5ca6da52ed2d7739` (the real run: 4,884 rows, 8/15 goals, 2 `CapturableStuck` findings, no `Unsupported`: the slice's scripts report nothing yet). Before the merge: real `0xeeeeab9ae5eef60f`, stub `0xbb1aa650526b2525`, same rows.

### Task A as built
- `cargo xtask extract --maps all` (BPEF): 518 maps, 418 layouts (every map's plus every `setmaplayoutindex` target; `gMapLayouts` has 441), 73 tilesets, 156 OBJ sprites plus 3 emotes (`sprites/field_effects/`), 124 wild headers (Altering Cave 9; land, water, rock smash, fishing), evolutions for 172 species, trainer money (54 classes plus the `0xFF` default), `Species.weight`. Every count equals the decomp's referenced set.
- Time: one full export about 5–6 s in release (the ignored sweep test's three exports: 17.6 s), 23.8 s with the debug `cargo xtask`; 39.8 s with `--previews`. Size: 26 MB without previews, 147 MB with them (previews 121 MB).
- Default kept at `slice` (L flips it, step 1): the content build panics on a full extract (slice-move wording, E), and since xtask depends on content, so does `cargo xtask`; `extract` warns with the recovery command.
- Location: layouts, emotes, evolutions and `gPokedexEntries`/`sSpeciesToNationalPokedexNum` by structural anchors; `gTrainerMoneyTable` by adjacency (the 4-byte boundary after `gTypeNames`), then validated. All checked on BPEE against the decomp. (The commit message of `bbcc190` says "all located by structure"; trainer money is by adjacency.)
- `FORMAT_MINOR` = 2 (after B's 1): layouts, emotes, encounters v5, evolutions, trainer money, weight; `MINOR_FILES` lists `battle/evolutions.ron`, `battle/trainer_money.ron` and the six emote files.

## Findings for Milestone 9
*(filled at close-out.)*

## Task B as built
- **Label transfer** (`extract::inc`, `extract::labels`): on all 518 maps, 100% of functions (7,233), texts (5,503) and movements (978) get decomp names on both BPEF and BPEE; 0 transfer errors, 0 collisions, 0 mismatched blocks. Precision on BPEE by content: 7,234 scripts (op names and Flag/Var/Const operands; the battle-facility helper macros of `data/scripts/*.inc` are expanded too), 5,503 texts and 978 movements match their source; only the 22 braille strings are unchecked.
- **Fallthrough rules:** a label falls through only to a following label of the same kind (`TrainerHill_1F_EventScript_DummyOnWarp` runs into a table whose first byte is `end`); a chain with no terminator ("forced stop" after `warp`, Slateport Battle Tent) is aligned against the first N commands the ROM decodes, N being the expanded command count of the chain.
- **Modules beyond the contract (accepted):** `data/text/<stem>.inc` is `Text_<stem>` (`cable_club` exists in both `data/scripts` and `data/text`); the inline labels of `data/event_scripts.s` are `Common_event_scripts`.
- **Test budget (accepted):** `extract/tests/labels.rs` runs all 518 maps unignored (about 5 s in debug); `tests/script_ir.rs` likewise for the IR.
- **IR:** `scripts/<Module>.ron` (433 modules on the full import), `scripts/index.ron`, and the texts keyed by final names in **`text/ir_<lang>.ron`**: the legacy `text/<lang>.ron` keeps its `<Script>_T<n>` keys because the legacy scripts number texts by them. D2 renames `text/ir_<lang>.ron` to `text/<lang>.ron` when it deletes the legacy output. Invalid opcodes appear as the pseudo-command `unknown` (one `Byte` operand). The IR's instruction count equals today's walk (49,225 on the full import). `FORMAT_MINOR` = 1.
- **Contract additions:** `scripts::{op_name, opcode, is_terminator_op}`; `export::scripts::{build_ir, Ir, ir_summary, decomp_dir}`; `labels::{transfer, Transfer, Kind, MapInput, …}` (`NameOrigin` re-exported from `schema`; `Transfer.root_kinds`); `ScriptIndex.movements` (movement name to module, `#[serde(default)]`).
- **Without the decomp:** a missing or unreadable `PKR_DECOMP` warns and exports positional names; the manifest provenance key `scripts/decomp_names` is 1 when decomp names were used, else 0. A label inside a decoded command warns and is skipped. `serde_json` is a regular dependency of `extract` (decomp `map.json`).
- **Outside B's files:** `extract/tests/export.rs` (Task A's) lists the IR files in its expected tree; `schema.rs` bumps `FORMAT_MINOR`, documents `text/ir_<lang>.ron` and adds `ScriptIndex.movements`; `export.rs` passes the provenance map to `export_scripts`. The decomp is loaded once per process (cache keyed by dir); `tests/export.rs` takes about 53 s in debug (46 s without `PKR_DECOMP`), the rest is `Decomp::load` and the transfer in an unoptimised build.

## Task I as built
- **Sight** (`world/sight.rs`): Gen3 `CheckForTrainersWantingBattle` for `TRAINER_TYPE_NORMAL` and `_BURIED` (type 2 is skipped as in Gen3), S/N/W/E distance order, `CheckPathBetweenTrainerAndPlayer` (cells between free ignoring the wander range, the player's cell exactly an object collision), the flag from `GameData::trainer_flag_of`. A second trainer seeing at once is `Degraded("double sight")`. Differences from Gen3, documented there: no directionally impassable or out-of-map test on the path, and the scan walks the current map's templates instead of the active object slots.
- **Approach** (`objects/approach.rs`, `world/host/trainer.rs`): `TrainerApproach { obj }` answers `Unit` after a sight approach (lock, `!` 61 frames, `walk_normal` steps, face; the trainer's template keeps its stop cell and `Face(dir)` for the visit), else `Value(usable mons)` at once. Content's `trainer_battle` runs every `TRAINER_BATTLE_*` kind (numbers pinned against `battle_setup.h` by `content/tests/m8_i_trainer.rs`).
- **Emotes** (`objects/emote.rs`): object-attached bubbles, hashed only while shown, 4 fixed OBJ tail slots of 4 tiles (tile 1008+). Palette: the `GameData::emote` sprite's own, through the normal bank allocator (Gen3 uses OBJ palette 0 for `!`/`?`, identical at the indices they use; the heart's palette-2 colour is approximated).
- **Movement types:** 67 of Gen3's 81 as `FaceTwo(Dirs)`, `FaceAllBut`, `Rotate` (a turn every 49 frames: `_Step2` returns FALSE when the 48-frame delay ends), `WalkBackAndForth`, `WalkSequence(Dirs)`, `JogInPlace`, `RunInPlace`, `WalkSlowlyInPlace`. `Dirs` packs four directions in a byte so `MovementType` stays two bytes; objects cache their movement code; `Object::movement` is private (getter).
- **Hash headroom (review I-4):** the templates are `world/templates.rs`'s `Templates`, a digest kept on edit (the one writer, `State::edit_template`) instead of a per-frame loop of `movement_code` and 7 bytes per template; it also counts looking trainers once per load, so sight does nothing on maps without one; sight looks each trainer's object slot up once (`Objects::slot`/`in_slot`/`sight_collision(slot)`). Parity before → after (route / before_may): hash 64 → 59, loop 216 → 211 / 216 → 212. This changes every world hash: the goldens and both trees move (route delta below).
- **Route delta:** every golden constant moves (template digest), and every node's hash; no frame count moves (sight needs `trainer_flag_of`, not in the slice build yet). New goldens: walk `0xa606_3af7_4c9c_3e21` / `0x2d6a_1f1f_392d_3f76`, warp `0x22aa_44b8_1c6d_aa00`, crossing `0x3588_dd81_35ef_97aa`, script `0x725f_3659_70f0_84a5`, wild battle `0xbc95_2822_9ad3_cd5f`, trainer `0x2c20_8618_81c2_c3c5`.
- **For Task L:**
  - **I-2:** `sight::is_double` returns false: `GameData` has no way to tell a double trainer script (Gen3 reads the `trainerbattle` type, `scriptPtr[1]`). Add a query (for example `trainer_flag_of` returning the kind too) so a double trainer is skipped with fewer than two usable mons.
  - **I-3:** re-measure the first sighting frame against Gen3 once sight runs on the real build (the approach's frames are pinned on the synthetic game: done 113 frames after the sighting).
