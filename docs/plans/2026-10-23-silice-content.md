# Pokémon Silice: authored content on the pokemon-rust engine (plan)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

## Decisions (user, 2026-10-23)

These answer §5's open questions and override the draft where they differ.

1. **Source of truth:** a one-time import of `contenu-jeu/` into `games/silice/` (data) and `crates/content/src/silice/` (scripts). From then on the repo is canonical; `contenu-jeu/` is a reference. `xtask silice import` is a one-shot tool, not a regeneration loop.
2. **Map authoring:** a map-painting editor in the studio comes first (pulled forward from M14): paint metatiles from the referenced tilesets, place objects/warps/coord events/bg events, edit connections, save `games/silice/{maps,layouts}/*.ron`. The ~115 maps are drawn in it (agents may draft from the ASCII, then refine in the editor). Emerald's ROM tilesets are referenced by name (never committed) until original tilesets exist.
3. **Mega and Primal forms are canonical:** the engine gains the Fairy type (type chart, Gen6 matchups, used only by forms that have it) and the post-Gen3 abilities the forms need (about 14, plus Primordial Sea, Desolate Land, Aerilate).
4. **Silice drives the roadmap:** M9–M12 are ordered by what the Silice chapters need next; Emerald story goals beyond Act 1 are deferred.
5. **Still open:** original art and music (about 102 trainer pics, 36 Mega/Primal pics, tilesets, 13 MUS_SIL_* tracks). Until answered, Emerald assets from the extract are placeholders, referenced by name.

## Status

Planned 2026-10-23, after M8 Wave 1 merged. SIL1 Wave 0 waits for M8 Task L (it edits the content build); the studio map editor (SIL1 Task C, now the first task) starts in parallel with L. SIL1 Wave 0 is built (see "SIL1 Wave 0 as built" in §3.1).

Status: draft for review. Work starts after Milestone 8 is fully merged (all of its "Done when"). This is preparation only.
Sources: `/Users/dode/Documents/pokemon/pokemon-silice-notion-export/contenu-jeu/` (bible, chapitres 00–12, donnees, outils) and the engine repo at `/Users/dode/Documents/pokemon/pokemon-rust`.

## 0. Premises (what M8 leaves us)

Per `docs/plans/2026-10-22-milestone-8-scale-import.md` ("Done when", Tasks A–P):
- **Content build.** `crates/content/build/main.rs` turns `content/extracted/` (git-ignored, ROM-derived) into `consts.rs`, `data.rs` with `data.bin`, `battle.rs` with `battle.bin`, and `scripts.rs` with `dispatch.rs`, `overrides.rs`, `report.txt`, `build_id.rs`. Without an extract it builds the built-in stub world (`crates/content/build/stub.rs`, `cfg(pkr_stub)`). That stub is the precedent for building an `input::World` with no ROM.
- **Scripts.** Scripts are async Rust lowered from the ROM IR. Hand-written overrides live in `crates/content/src/overrides/`, keyed by decomp label. Committed Rust holds no text literals (`tests/m8_e_overrides.rs`).
- **`GameData`.** The trait in `crates/engine/src/data.rs` has `map`, `sprite`, `text`, `script`, `battle`, `wild_encounters(map, wild_set)`, `trainer`, `item`, `multichoice(list)`, `std_string`, `trainer_flag_of`, `trainer_battle_kind`, `emote`, `trainer_money`, `evolutions` and `heal_location`. `SpecialIds` passes roles in, never names, so the engine stays game-agnostic.
- **`Ctx`.** `crates/engine/src/script.rs` has flag, var, random, message, yes_no, lock, move_obj, show/hide/add/remove object, turn, set_movement_type, delay, fade, warp, set_metatile, redraw_map, door, give_mon, heal_party, battle_wild and battle_trainer (both returning a `BattleOutcome` that includes `Caught`), choose_starter, set_respawn, the item and money calls, buffer, multichoice, set_warp, report and trainer_approach. `Step` already has `LockFacing`/`UnlockFacing`, so "walk backwards" is just `[LockFacing, Walk, UnlockFacing]`, and `Emote` is there.
- **Flavours.** Shells, explorer, studio and xtask pick their checkpoint tree and db by `content::STUB` (`crates/explore/src/db.rs:127`, `crates/game-native/src/store.rs:474,486`, `crates/studio/src/paths.rs:112,120`, `crates/explore/src/goals.rs:250`, `crates/explore/src/key.rs:70`).
- **Gates.** Parity `loop_lines_max ≤ 217`, `hash_lines_max ≤ 65`, `late == 0`, `events_dropped == 0`, heap ≤ 160 KiB, ROM ≤ 16 MiB, reproducible build, sweep of every map, and an explorer that is goal-directed from checkpoints with per-act goals.
- **Not in the engine after M8:**
  - held items: there is no item field on `TrainerMon` or `Pokemon` in `crates/engine/src/mon.rs`, and the content build ignores the extract's `TrainerMon.item`;
  - forms, double battles, the mart, menus and in-game save;
  - field moves, weather, audio, palette effects, white fades (`Fade` is only `ToBlack`/`FromBlack`);
  - any game other than Emerald or the stub.

## 1. Gap analysis

### 1.1 `bible/besoins-moteur.md`, item by item (status after M8)

The "Where" column gives the existing milestone or a new SIL task (§3). E = engine, C = content, P = pipeline.

| Pri | Need | After M8 | Missing | Where |
|---|---|---|---|---|
| P0 | **Original content pipeline** (new MAP_*, FLAG_SIL_/VAR_SIL_, TextIds outside `fr.ron`, authored trainers, encounters and items) | None: everything is loaded from `content/extracted` (`crates/content/build/input.rs::load`). The stub is the only non-ROM `World`. | All of it: flavour selection, authored schemas, ID allocation, text encoding of authored FR, a script registry for hand-written scripts, and merging with the ROM base assets | SIL1 A/B/D/E (P, C) |
| P0 | give/take/has_item, give_badge, check/remove money, take_money_percent | ItemAdd/Remove/Check/Space and MoneyAdd/Remove/Check (M8 H) | A money **read** (`Request::MoneyGet`) for the −20 %. give_badge is a content helper (flag plus fanfare, fanfare silent until M12). | SIL2 engine-small (E) plus Silice prelude (C) |
| P0 | `mart(id)` with custom stock that depends on flags | `pokemart` is Unsupported in M8; the Mart UI is M9 | Mart UI (M9) with stock passed in by the script (Gen3 passes a list pointer) | M9, then SIL2 |
| P0 | multichoice([...]), yes_no with the cursor on No, random, clear_flag, var arithmetic | Multichoice over `GameData::multichoice(list)` (H); random; `set_flag(f,false)`; arithmetic is plain Rust | Silice lists appended to the multichoice table (data, C). A yes/no that defaults to No (E, small). | SIL1 (data), SIL2 (E) |
| P0 | `call(script)` | Plain Rust function call | Nothing | — |
| P0 | Trainer-beaten test, continuing after a gym battle | `trainer_flag(id)` (prelude), `TrainerBattle` continue-script kinds (I) | A Silice helper `defeated(TRAINER_SIL_X)` | SIL1 E |
| P0 | play/stop/fade music, SE, fanfare | `Ignore` (audio M12) | The audio engine (M12), authored songs for MUS_SIL_*, and Silice SE ids | M12 plus SIL5 |
| P0 | Mallette modules as field actions triggered by a **flag**, not a party move | No field moves (M10) | Field moves (M10) **with a game-agnostic enable hook** (`GameData::field_move_enabled(FieldMove, &state)`), so Silice gates them by FLAG_SIL_MODULE_* | M10 contract plus SIL2/3 |
| P0 | Water, fishing and rock-smash encounters | Tables compiled (E); not used until M10 | M10 | M10 |
| P1 | Mallette menu: Analyse, Map and signal, Calls with portrait, Modules, Logs, Sync gauge | No start menu (M9) | A game-defined start-menu entry (`GameData` menu items that run scripts), a call window with portrait, a log viewer, an animated sync gauge | M9 plus SIL2/3 (E generic UI, C Silice screens) |
| P1 | screen_flash, screen_shake, white fade, show_text_on_black, emote, camera_pan | Emote (I); black fade only | Flash, shake, white fade, a text-on-black mode, camera pan (E, Requests, hashed) | SIL2 |
| P1 | Map weather (fog, rain, **petals**), palettes and tints (night, red alert, Elysium gold, destroyed), sky crack | `setweather` is Ignore | Weather (M10) plus a game-defined weather kind for petals; a palette override Request with a blend; sky-crack is a scripted BG overlay | M10, SIL3/5 |
| P1 | show_book, readable key items | No bag UI (M9) | A full-screen text window; "Read" from the bag | M9 plus SIL3 |
| P1 | Credits with vignettes, two-state title screen, `show_title(GOLD)` plus `reload_last_save()`, scripted `save_game()` | No title or save UI (M9 save format) | Title screen, credits sequence, scripted save, a reload Event handled by the shell (GBA flash reload, native last-save checkpoint) | M9, SIL5 |
| P1 | Sprite variants, **following Pokémon** | Any gfx id per object; no follower | Variants are data. The follower needs an object that copies the player's steps with a one-step lag (E) or the fallback (two objects moved together). | SIL2 (fallback), SIL4 (E, optional) |
| P1 | `♥` and `·` glyphs (R-11); PIERRE LIEN | Font is ROM-extracted (`data::FONT`); `EvoMethod::Trade` is folded; no bag "use" | A glyph overlay in the Silice build. An item-triggered trade evolution (bag "use" in M9; the evolution scene in M9). | SIL1 G, SIL2 |
| P1 | Trainer names over 10 characters; buffers | Buffers (H); `TrainerData.name` is a slice | Pixel-width checks only (window width), done by the validator | SIL1 B |
| P2 | Double battles: one trainer `double: true`, two trainers, multi with a partner | Degrades to singles (M8 Q3) | Doubles, two-trainer and multi battles with a partner AI (M11) | M11, SIL2–4 |
| P2 | **Mega Evolution and Primal Reversion**; Primordial Sea, Desolate Land, Aerilate | Nothing; no held items | Held items, form tables, the trigger, AI, recalculation, pics, abilities, text (see §3.4) | SIL3 (E plus C) |
| P2 | **Inverse battle** | Type chart only (`BattleTables.type_chart`) | A battle rule flag; effectiveness transformed in `battle/calc.rs` and in the AI | SIL4 (E) |
| P2 | Battle-start effects (status, stat stages, forced weather) | None | A `BattleSetup` passed with the trainer request (E) | SIL3 |
| P2 | Per-turn **field rules** (strobe, roll, petals, deaf room, overdose, shadow prism, data injector, confusion immunity) | None | One game-agnostic per-turn hook: `GameData::battle_rule(id)` returning a table of effects (data, not code) | SIL3/4/5 |
| P2 | Trainer lines mid-battle (on send-out, on Mega, under 50 % HP) | None | Trigger table per trainer (C data) and a battle message hook (E) | SIL3 |
| P2 | AI item use (`items` field); a battle that continues after a loss | No `items` in `TrainerData`; whiteout always | `TrainerData.items` and AI use (M11); `battle_trainer(.., no_whiteout)` | M11, SIL2 (no_whiteout, Samuel R-7) |
| P2 | Custom battle background; foe shown from the back | Environment backgrounds | A background id per battle (E); flipping the foe pic (E) | SIL4/5 |
| P2 | battle_wild returning Caught; give_mon with moves and held item | Caught exists; `give_mon(species, level)` | `GiveMon` with moves, item, OT (E) | SIL2 |
| P2 | **Clone of the player's party** | None | A trainer battle against a runtime party copied from the player (E) | SIL6 |
| P3 | **Inverted controls**, NPCs walking backwards, inactivity timer | `LockFacing` exists | A hashed `input_map` World state and Request; an idle-frame counter readable by scripts; explorer planner support | SIL4 (E plus explore) |
| P3 | Ice, arrow and rotation tiles, trapdoors, sea and underwater currents | Trapdoors are warps; others M10 ("rotating-tile puzzles") | M10 behaviours | M10 |
| P3 | Dark maps with a Flash radius, a moving light, metatiles visible only under Flash | M10 Flash | Moving light source and conditional metatiles (E, M10 extension) | M10 plus SIL3 |
| P3 | Non-linear trainer sight (chess pieces) | Not needed: the author resolved it with coord events plus `battle_trainer` (ch02 §9) | Nothing | — |
| P3 | Hidden trainers (pillar, barrel), trapped chests | Buried-trainer movement types approximated (I) | M10/M11 exact types, or the fallback | M10 |
| P3 | `on_load` puzzle reset; invisible walls | Map scripts exist; collision is data | Nothing | — |
| P3 | Casino: coins, slots, Electric Roulette, Plus-or-Minus | M13 Game Corner | Coins (`VAR_SIL_COINS` fallback), mini-games (M13 or the fallback) | SIL2 fallback, M13 |
| P3 | Animated tiles, ground animation on step | M10 | M10 | M10 |
| P3 | `wait_input_or_timeout(frames)` | None | A small Request | SIL2 |
| P3 | add_friendship; scripted NPC evolution scene | None; the evolution scene is M9 | A small Request; scene M9 | SIL2, M9 |
| — (ch. §9) | Real day/night cycle, **daily flags** (post-game) | RTC is M10 | A deterministic clock in World (seeded and advanced by frames, with an RTC input on hardware), daily reset | M10 contract, SIL6 |
| — | `teach_move` outside menus; invented `MOVE_SIL_CODE_AQUA`; `give_egg`; National Dex | None | Tutor Request plus a party picker (M9); new move data (C); eggs (M13) | SIL6 |

### 1.2 Per chapter (counts measured on `chapitres/*.md`)

There are 120 distinct MAP_ names, of which about 115 are real maps (the rest are placeholders or fallbacks). There are 238 trainers. 122 trainer entries mark a "sprite à créer", for about 102 distinct new trainer pics. 438 FLAG_SIL_ and 20 VAR_SIL_ names. About 1,931 texts. 13 MUS_SIL_ tracks. 70 ITEM_SIL_ items (34 Mega stones, 2 orbs). One invented move.

| Ch | Maps | New graphics | New ids (species, moves, items) | New mechanics beyond M8 | Music | Playable with fallbacks once |
|---|---|---|---|---|---|---|
| 0 Prologue | 9 names (BRICO_BOURG, BAR, PLAYER_HOUSE_1F/2F, FRIENDS_HOUSE, ROUTE_S01; BRICO_BOURG_NIGHT is a fallback) | NPC and trainer pics (placeholders), Mallette key item icon | ITEM_SIL_MALLETTE; Johto starters (existing species) | screen_shake, SE, night tint, Mallette call (all have fallbacks) | Emerald placeholders | **SIL1** (the pipeline only) |
| 1 Bois-Éther | 9 (BOIS_ETHER plus 4 interiors, GYM, BRICO_BOURG_LABO) | Lab strobes, sky alert | EVOLUROC (Eviolite-like held item), POUSSIERE_SILICE | Cut module, mart, two-trainer battle, sync gauge, LOG menu | MUS_ placeholders, one invalid | SIL2 (M9 mart, M10 Cut, fallbacks) |
| 2 Gris-Échec | 11 | Chess gym, follower | ALAKAZITE (Mega Alakazam), CARTE_PERFOREE | Strength module, scripted NPC evolution, deaf room, **Mega (Yohan)** | MUS_MOSSDEEP, MUS_KIMIKO_STREAM invalid | SIL2 (Mega replaced by a fallback until SIL3) |
| 3 Viber-City | 12 | Neon animated tiles | PIERRE_LIEN, BROCHETTE_NEON | Flash module, casino, doubles, strobe rule, `give_mon` with moves | MUS_MAUVILLE, MUS_ROUTE111 invalid | SIL2 (casino and doubles as fallbacks) |
| 4 Port-Ressac | 8 | Ship and port tiles | RHUM_PIRATE, VIEUX_PATCH, MILLESIME_SAM | Surf module, Good Rod and fishing, a non-blocking loss with penalty, start status, roll rule | — | SIL3 |
| 5 Owlys | 16 | Dark cave, chests | ABSOLITE, SABLENITE, PLUME_OMBREE, BOUTEILLE_PUNCH | Fly module, Flash variants, multi with a partner, currents, Mega Absol | — | SIL3 |
| 6 Manga-Square | 12 | Japanese tiles, petals | CHARIZARDITE_X/Y; FLAG_SIL_MODULE_MEGA | Rock Smash module, **player Mega**, petal weather (map and battle), book UI, friendship, battle malus | — | SIL3 |
| 7 Reverse-City | 10 | Mirror and glitch palettes, foe from the back | MULTI_EXP_V2, POUDRE_SILICE, ALAKAZITE | **Inverted controls**, **inverse battle**, ice and arrows, Waterfall module, data-injector rule | — | SIL4 |
| 8 Évalon | 14 | Medieval village | 9 items (key items, COEUR_EVALON, Megas: MAWILITE, GARDEVOIRITE, AMPHAROSITE, METAGROSSITE, AGGRONITE) | Dive module, multi battles, overdose rule, confusion immunity, readable items | MUS_RG_SEKAIKAN (exists) | SIL4 |
| 9 Nébula | 12 | Underwater conduit | MANECTITE | Dive and underwater currents, camera pan, water encounters in town | — | SIL4 |
| 10 Tower | 7 | Elevator BG, animated floors | ACCES_*, BOUSSOLE, BATEAU_ORIGAMI, STEELIXITE, GYARADOSITE | Trainer item AI, per-mon trainer lines, shadow prism, start stat effect, **scripted save (R-6)** | — | SIL5 |
| 11 Apex/Zénith | 7 | Gold palettes, sky crack, credits | ORBE_BLEU/ROUGE, SALAMENCITE; **Primal Kyogre/Groudon** | Primal Reversion, abilities, a no-whiteout battle, palettes, credits, title states, **alternate ending with reload** | 9 MUS_SIL_ tracks | SIL5 (needs M12 for music) |
| 12 Post-game | 60 | "Destroyed" palettes, ruins | About 20 Mega stones, MOVE_SIL_CODE_AQUA, CLE_DEBUG, TICKET_ESPACE… | Daily flags (RTC), clone battle, egg, tutor, boss wild battles, rule challenges, toxic air, legendaries (R-9) | MUS_SIL_* | SIL6 |

### 1.3 Pipeline gaps in detail

- **Maps.** The chapters give only coarse ASCII plans, some not to scale (§4 item 12), and no `OBJ_EVENT_GFX_*` id. Each map needs a metatile layout drawn on Emerald (ROM) tilesets or new authored ones, plus objects, warps, coords, bgs and connections in the extract's `MapFile`/`Layout` schema (`crates/extract/src/schema.rs:219-258`). Nothing in the repo authors maps today. The studio is read-only for maps until M14 (M8 decision 12).
- **Graphics.** Placeholders reuse ROM `TRAINER_PIC_*`, `OBJ_EVENT_GFX_*`, tilesets and species pics by name; the build reads them from the extract, so nothing ROM-derived is committed. New art (102 trainer pics, Mega and Primal pics, new tilesets, the Mallette UI, glyphs) must be original before it can be committed (Q3).
- **Text.** FR text is authored and committed as RON (fine: it is original). It is encoded with `extract::text::encode_lang(Lang::Fr)`. Glyphs missing from `crates/extract/src/text/charmap.rs`: `—`, `°`, `●`, `○`, `♪`, `₽`, `#`, `←`, `→`, `[`, `]`, `_`, and `♥`/`·` (R-11). The real limit is pixel width with the FR font widths (`data::FONT.widths`), not 36 characters.
- **IDs.** Silice needs its own numbering, stable across builds so saves and checkpoints survive:
  - maps (group/num), flags (avoiding 0x0–0x1F temp flags, the trainer-flag range 0x500 + id, and the system range);
  - vars (256 saved, 0x4000–0x40FF in `crates/engine/src/flow/vars.rs`; Silice uses 20);
  - trainers (from 1, so trainer flags land in 0x501–0x5EE);
  - items: Gen3 `ITEM_*` keep their numbers for names and icons from the ROM, and `ITEM_SIL_*` are appended;
  - species: 412 Gen3, then forms appended;
  - one appended move.
- **Battle data.** Trainer mon `item` must reach the engine (held items). Optional fields are needed for the trainer `items` bag, mid-battle lines and battle rules.

## 2. Architecture for authored content

### Option A: separate `silice` game crate and a `game` facade
A new no_std crate `crates/silice` implements `GameData`. A host-side library (`crates/gen3-build`) is factored out of `crates/content/build/` and shared by both build scripts. Shells depend on a facade crate that re-exports `content` or `silice` according to a cargo feature.
- For: the cleanest boundary; Emerald and Silice compile independently; the ROM stays smaller without Emerald scripts.
- Against: shells reference `content::` about 150 times (game-native 28, explore 26, studio 62, plus xtask). Cargo features are additive, so mutually exclusive game features are awkward (both games compile, or a `compile_error`). A refactor of every shell before any Silice content exists.

### Option B (recommended): a Silice flavour of `content`
`content` gains a third build flavour next to stub and real, selected by `PKR_GAME=silice` (`cargo::rerun-if-env-changed`, `cfg(pkr_silice)`), the same pattern as `PKR_EXTRACTED_DIR` for the stub:
- Authored data is committed under `games/silice/`.
- Silice Rust (`GameData` impl, prelude, scripts) is committed under `crates/content/src/silice/`, compiled only under `cfg(pkr_silice)`.
- The build loads base assets from the extract (tilesets, sprites, pics, font, window, species, moves, items, std strings) and merges the authored data into the same `input::World`. `codegen.rs`, `battle.rs`, `blob_data.rs`, dispatch and digests are then reused unchanged.
- The Emerald IR and its lowered scripts are **not** compiled in the Silice flavour (ROM size, build time).
- For: zero churn in the shells' `content::` API; checkpoints, explorer, studio, sweep and parity work at once once `content::STUB` becomes `content::FLAVOR`; the stub/real testing pattern already exists. The layout (`games/silice/`, `src/silice/`) can move to option A later as a move, not a rewrite.
- Against:
  - One target dir builds one flavour at a time; CI runs three flavours, as it already runs two.
  - `BUILD_ID` and `GLOBAL_DIGEST` must hash only the active flavour's sources (exclude `src/silice/**` and `games/silice/**` from Emerald and vice versa). Otherwise every Silice edit invalidates Emerald trees and explorer dbs, because the M8 digest covers "every non-generated content source" (`crates/content/build/build_id.rs`, `codegen::sources_digest`).
  - Emerald-only tests in `crates/content/src/{data_tests,battle_tests}.rs` need flavour gates.

### Option C: overlay Silice on the Emerald world
Silice maps, flags and trainers are appended into the Emerald build, with numbers placed in unused Emerald ranges.
- Rejected:
  - Silice trainer ids after Emerald's 855 would collide with system flags (0x500 + id);
  - the 438 Silice flags compete with Emerald's;
  - the new game, special roles, starters and whiteout are Emerald's;
  - the ROM carries about 7 MB of unused Emerald scripts.

### Recommended layout (option B)
```text
games/silice/                      committed, original only (no ROM-derived bytes)
  game.ron                         start map and cell, facing, player name, starters, respawn default,
                                   SpecialIds roles (running-shoes flag, flags cleared on map change)
  ids/{maps,layouts,flags,vars,trainers,items,species,moves}.ron
                                   append-only allocation: name -> number (stable saves and checkpoints)
  maps/<Name>.ron                  extract MapFile schema; scripts referenced by Rust fn name
  layouts/<Name>.ron               extract Layout schema; primary/secondary = extracted tileset names
                                   (or games/silice/tilesets/<Name> once original tilesets exist)
  text/fr.ron                      TextId name -> FR .string syntax (from the chapters' [Id] blocks)
  battle/trainers.ron              extract TrainersFile schema + optional `items`, `lines`, `rules`
  battle/encounters.ron            extract EncountersFile v5 (map name -> Vec of headers)
  battle/heal_locations.ron, items.ron (ITEM_SIL_*: name, pocket, price, key, hold effect/clone_of),
  multichoice.ron, marts.ron, species_forms.ron (SIL3), moves.ron (MOVE_SIL_*, SIL6)
  font/<name>.ron                  ♥ · (R-11) and any approved extra glyph, drawn by hand
                                   (one glyph per file; see "SIL1 Task G as built")
  gfx/…                            original art only (Q3)
crates/content/src/silice/         cfg(pkr_silice)
  mod.rs                           `impl GameData for Game` (Silice)
  prelude.rs                       Silice helpers over Ctx and the shared Gen3 helpers
  new_game.rs
  scripts/chNN_<map>.rs            one `pub async fn <Map>_ScNN(ctx: Ctx)` per scene, texts by TextId only
crates/content/build/silice/       loader, ID allocation, merge into input::World, script registry (syn scan)
xtask/src/silice/                  importer (markdown -> games/silice RON) and validator
```

**How the `chapitres/` RON maps onto it.**
- **§5 trainers** (`donnees/dresseurs.ron`): already the extract `Trainer` schema. The importer keeps `class`/`pic` placeholders (resolved against the extract), keeps `item` (held items, used from SIL3), and adds an id in `ids/trainers.ron`.
- **§6 encounters** (`donnees/rencontres.ron`): reshaped to v5. The key becomes the Silice map name and the value a one-header `Vec`. Fishing slots are split old 2, good 3, super 5, as `gWildFishingSlots`.
- **§2 maps**: the ASCII gives intent only. Layouts are drawn (Q2); objects, warps, signs and NPC texts come from the §2 tables.
- **§3 scenes**: hand-translated to Rust, one scene per `async fn`. Each §9 need maps to an engine call when the engine has it, otherwise to the chapter's documented fallback plus `Report::Degraded`.
- **§7 items** become `items.ron` plus map item balls and hidden items. **§8 flags** become `ids/flags.ron`.

**Reuse of ROM graphics without committing them.** The Silice build requires the extract (`cargo xtask extract`) for base assets and refers to them by decomp name. No PNG, palette or text from the ROM is copied into `games/silice/`. A hygiene test forbids ROM bytes under `games/silice`, using the same idea as `tests/m8_e_overrides.rs`. Without an extract, the Silice flavour falls back to the stub with a warning.

**Determinism and checkpoints.**
- Silice has its own tree `checkpoints/silice/` and its own explorer db and goals (per chapter, as state predicates on `VAR_SIL_STORY`, badges and module flags).
- Parity lanes per committed `sil:*` node.
- Snapshots hold no ScriptId (M5), so script renames only move `BUILD_ID`.

## 3. Milestone breakdown

Rules are the same as M8 ("Rules for every task"): one worktree per task, TDD, file ownership, no ROM-derived data committed, no FR literals in tests, hash discipline (route-neutral first; route-changing tasks one at a time, each followed by a `chore(checkpoints)` on master), GBA gates, and three flavours tested (`PKR_EXTRACTED_DIR=/nonexistent`, default Emerald, `PKR_GAME=silice`).

### 3.1 SIL1: Silice flavour and the Prologue playable (Ch0, to `VAR_SIL_STORY` 9 at the Bois-Éther edge)
SIL1 adds **no engine behaviour**. It is route-neutral for Emerald: only `BUILD_ID` changes, with one `valid (rehash)` refresh, which is why the digests are split by flavour.

**Wave 0: contracts (1 large session, committed before Wave 1)**
- `content::Flavor { Stub, Emerald, Silice }` and `content::FLAVOR`. `STUB` stays as `FLAVOR == Stub`. The `Emerald` type is renamed `Game`, with an `Emerald` alias kept for one milestone.
- `PKR_GAME` is read in `crates/content/build/main.rs` and sets `cfg(pkr_silice)`. `build_id.rs` and `codegen::sources_digest` hash only the active flavour's sources.
- Serde types for every `games/silice/*.ron` file (in `crates/content/build/silice/schema.rs`). Where the extract schema fits, it is reused (`MapFile`, `Layout`, `TrainersFile`, `EncountersFile`, `TextFile`).
- The script registry contract: the build scans `src/silice/scripts/*.rs` with `syn` for `pub async fn` names, assigns ScriptIds in name order, and emits `SCRIPT_NAMES`, a dispatch table and `SCRIPT_DIGESTS` in the same shape as `dispatch.rs`. A map naming an unknown script fails the build.
- ID allocation contract: append-only files; a removed name keeps its number (tombstone). A test fails on renumbering.
- `checkpoints/silice/` with an empty `status.ron`. `Store::default_root(stub: bool)` and `Db::default_path(stub: bool)` change to take a `Flavor`. The explorer gains the act lists `sil_ch0` and onwards.
- Stubs: `crates/content/src/silice/{mod.rs, prelude.rs, new_game.rs}` and one empty map, so `PKR_GAME=silice cargo test -p content` builds.
- Gate: the three flavours build and test; Emerald trees `valid (rehash)`; `cargo xtask parity` unchanged.

**Wave 1 (parallel)**

| Task | Owns | Content | Route |
|---|---|---|---|
| A: authored data loader | `crates/content/build/silice/{mod,load,ids,merge,text}.rs`, `crates/content/tests/sil1_a_*.rs` | Load `games/silice`, validate references (maps, scripts, texts, tilesets, gfx and pics by extract name), allocate ids, encode FR text (including the « » mapping and glyph-overlay codes), and merge into `input::World` with the extract's base assets, without the Emerald IR. Tests: fixture `games/` dirs, a byte-identical rebuild, an unknown tileset or script failing. | neutral |
| B: importer and validator | `xtask/src/silice/{mod,import,validate}.rs`, `xtask/src/main.rs` (one arm) | `cargo xtask silice import <contenu-jeu>` writes `games/silice/{text,battle,ids,items}` from §3 text blocks, §5/§6 RON, §7/§8 tables (depends on Q1). `cargo xtask silice check` runs every check in §4 of this plan (ids against the extract, MUS/SE/WEATHER/HEAL_LOCATION, families such as `FLAG_SIL_FRAGMENT_`, dynamic text ids, duplicates, pixel width with the extracted font widths, charmap coverage, trainer name widths, slot counts, unreachable fishing slots). | neutral |
| C: map authoring path | Per Q2: either `xtask/src/silice/porymap.rs` (import Porymap JSON and bin into `maps/` and `layouts/`), or a minimal studio editor write-back in `crates/studio/src/map_edit/**` (paint metatiles from the referenced tilesets, place events, save RON) | Tests: a round trip of one map; pixels equal to a native PPU render | neutral |
| D: flavour plumbing in shells | `crates/game-native/src/store.rs` (roots), `crates/explore/src/{db.rs, cli.rs, key.rs}`, `crates/studio/src/paths.rs`, `xtask/src/{checkpoints.rs, budget.rs}` and the `parity`/`rom`/`sweep` flag `--game silice` | Every `content::STUB` decision uses `content::FLAVOR`. Silice ROM budget gates are re-baselined at SIL1 L. | neutral |
| E: Silice `GameData`, prelude, new game | `crates/content/src/silice/{mod,prelude,new_game}.rs`, `crates/content/src/lib.rs` (cfg switch only), `games/silice/game.ron` | `SpecialIds` from `game.ron` (FLAG_SIL_RUNNING_SHOES, heal location Brico-Bourg house, the cleared-flags subset). Starters Chikorita/Cyndaquil/Totodile through `choose_starter`. Gen3 std texts and strings reused through `std_string` and std scripts. Helpers: `defeated`, `story`, `give_item` (H's), `fallback(name)` (`Report::Degraded`). | neutral |
| F: Chapter 0 content | `games/silice/maps/{BricoBourg,BricoBourg_Bar,BricoBourg_PlayerHouse_1F,_2F,BricoBourg_FriendsHouse,RouteS01}.ron` and their layouts, `crates/content/src/silice/scripts/ch00_*.rs`, Ch0 rows of the imported RON | 6 maps; scenes Bar_Sc01–04, Brico_Bourg_Sc01–04, Player_House_Sc01–03, Route_S01_Sc01–02; Route S1 trainers (sight from M8 I), item balls, the hidden Antidote, land encounters. Fallbacks: shake becomes a fade, SE is a no-op, night tint is a flag-selected palette-free map variant or nothing, the Mallette call is a msgbox. | neutral (new tree) |
| G: font glyphs | `games/silice/font/*`, `crates/content/build/silice/font.rs` | ♥ and · appended as authored glyphs (R-11), with charmap codes reserved in the Silice text encoder; a validator rule for the other missing glyphs | neutral |

**Wave 2: integration (L-like)**
- `PKR_GAME=silice cargo xtask sweep` on every Silice map: idle 600 frames, checkpoint round trip, render, `events_dropped == 0`.
- Build gates recorded.
- `cargo xtask rom --game silice` with ROM, IWRAM and EWRAM recorded and gated. Without the Emerald scripts the ROM should come in far under 16 MiB.
- Map tour on the Silice maps: `loop_lines_max ≤ 217`, `late == 0`.

**Wave 3: route and parity**
- Explorer: `--act sil_ch0`, with goals in this order:
  1. `FLAG_SIL_INTRO_DONE`;
  2. `VAR_SIL_STORY ≥ 1`;
  3. `FLAG_SIL_HAS_MALLETTE`;
  4. the starter chosen;
  5. `MAP_ROUTE_S01` entered;
  6. `VAR_SIL_STORY ≥ 9`.
- Nodes `sil:ch0:*` promoted to `checkpoints/silice/`. Each node resumes on the ROM for 2,000 frames with identical hashes.
- Close-out: "As built", findings, README section.

**SIL1 done when:** three flavours green; Prologue completed by the explorer from a new game; parity identical; no ROM-derived file under `games/`; validator clean or every exception listed with a reason; Emerald trees unchanged except the rehash.

#### SIL1 Wave 0 as built: the contracts

Commits:
- `refactor(extract): define the games dir formats once in extract::games`
- `feat(content): add the Silice flavour (PKR_GAME=silice) …`
- `feat(shells): key the checkpoint store and explorer db by flavour, add the sil_ch0 act`
- `test(content): …` and `chore(gba): lock syn …`
- after review: `fix(silice): reserve the Gen3 flags and vars the shared code uses, move VAR_SIL_EVALON_PARTENAIRE` and `fix(content): share the GameData forwarding, fail Silice without an extract, keep Silice code out of Emerald's sources`

The contracts below are what Wave 1 builds on. Change one only together with the tasks that rely on it.

**Flavour selection (all tasks).**
- `content::Flavor { Stub, Emerald, Silice }` has `ALL` and `name()`, which gives `stub`, `emerald` or `silice`. `content::FLAVOR` is this build's flavour. `content::STUB` is `FLAVOR == Flavor::Stub` and stays for code that only asks whether there is a game.
- `PKR_GAME` is a build env variable (`rerun-if-env-changed`):
  - unset, empty or `emerald` gives Emerald;
  - `silice` gives Silice and sets `cfg(pkr_silice)`, which is declared in `check-cfg` and applies to the lib, unit tests and `tests/*.rs`;
  - any other value fails the build.
- **Silice needs the extract.** `PKR_GAME=silice` without a usable extract (missing or stale manifest) fails the build. The exception is an explicitly set `PKR_EXTRACTED_DIR`, for example `/nonexistent` in a stub build that inherits `PKR_GAME`; that builds the stub with a warning. A Silice test asserts `FLAVOR == Flavor::Silice`.
- `PKR_SILICE_DIR` overrides the games dir (default `games/silice`), for fixture dirs in Task A's tests.
- The type is `content::Game`; `pub type Emerald = Game` stays for one milestone. Under `cfg(pkr_silice)`, `Game` is `content::silice::Game`, a unit struct with `Game::for_shells()`. The Emerald `Game { start }` (with `before_rescue` and `after_starter`) and `new_game.rs` are `cfg(not(pkr_silice))`.
- **One `GameData` forwarding.** `content::game_data_tables!()` defines every `GameData` method the generated tables answer, `hold_effect` included, and both roots expand it.
  - Each root writes only `special_ids`, `script` (its new-game script) and `heal_location`.
  - `special_ids` takes the shared fields from `content::gen3_special_ids()` (`..gen3_special_ids()`). A new shared `SpecialIds` field, such as M8 Task M's `max_money`, `multichoice_cancel` and `pocket_slots`, goes there once.
  - `form_changes` keeps the trait default until a flavour has form data.

**Digests.** `BUILD_ID` (`build/build_id.rs`) and `GLOBAL_DIGEST` (`codegen::sources_digest_for`) hash only the active flavour's sources, through the filters `codegen::{src_source, build_source, extract_source, games_source}`.
- **Stub and Emerald** exclude `src/silice/`, `build/silice/` and `extract/src/games/`. Emerald's `BUILD_ID` changed once in Wave 0 (the trees read `valid (rehash)`), and Silice work no longer moves it.
  - The Silice code that remains in Emerald-hashed files is small and flavour-neutral:
    - `input::AuthoredIn`: script names, id constants, start;
    - its three uses in `codegen.rs` and `scripts.rs`;
    - the flavour switch in `main.rs`;
    - the `content/src/lib.rs` plumbing.
  - Silice's own generated files are written by `build/silice/dispatch.rs`.
- **Silice** excludes `src/overrides/` and `src/new_game.rs`.
  - `BUILD_ID` leaves out the extract's `maps/` and `scripts/`, and adds `games/silice` minus `edits/`, `IMPORT-REPORT.md` and `check-exceptions.ron`.
  - `GLOBAL_DIGEST` leaves out `src/silice/scripts/`, which have per-script digests, and `games/silice/{maps,layouts}`.
  - A fixture extracted dir in `tests/sil1_w0_contracts.rs` checks both extracted-dir filters.

**Shared formats (Task A, C).**
- The games dir types are defined once in `extract::games`:
  - `data.rs`: ids, text, trainers, encounters, items, refs, `SHARED_GEN3`;
  - `maps.rs`: `GameMap`, `GameHeader`, `GameObject`, `GameWarp`, `GameConnection`, `MAP_DYNAMIC` and the warp-id helpers.
- `extract` is the one crate that the content build (as a build-dep), xtask and studio all depend on, so the types are shared through it rather than forked or `#[path]`-included.
- `crates/content/build/silice/schema.rs`, `xtask/src/silice/schema.rs` and `studio::map_edit::schema` re-export it. The studio keeps `to_ron`, `from_ron` and `header_from_extract`, which needs the studio's `names`.
- `extract::games::parse_texts` reads `text/fr.ron` in file order and rejects a repeated key (Task B's loader contract).

**Ids (Task A, B, F)** (`build/silice/ids.rs`).
- **Append-only with tombstones.** `check_golden` fails when a golden `(name, number)` line disappears or is renumbered. `tests/sil1_w0_contracts.rs` runs it on the committed files with `xtask/tests/fixtures/silice/ids.golden`.
- **Reserved numbers.** `IdKind::reserved` holds `MAP_DYNAMIC` for maps, and for flags and vars the numbers of `extract::games::SHARED_GEN3`: the Gen3 flags and vars that the shared code reads or writes.
  - The shared code is the Gen3 prelude, the content crate root and `SpecialIds`, `samples.rs`, and the Silice root.
  - The list is checked against those sources by a test, which scans them for Gen3 `FLAG_*`/`VAR_*` names and fails on any difference.
  - Inside the Silice blocks this reserves `VAR_STARTER_MON` (0x4023), `VAR_PC_BOX_TO_SEND_MON`, `VAR_ALTERING_CAVE_WILD_SET`, `VAR_DAYS` and `VAR_BIRCH_STATE`.
- **What `validate` and xtask `problems` reject:**
  - a name live twice, or listed again before its tombstone;
  - a number used twice (a tombstone counts);
  - a number out of range;
  - a live entry on a reserved number;
  - a live map entry without its file.

  xtask `check`'s `gen3-collision` rule also reports a live Silice id on a `SHARED_GEN3` number.
- **Moving an id** is the only way a name changes number. `cargo xtask silice move NAME` tombstones the entry in place, keeping the old number taken, and appends the name live at the next free number. The new `(name, number)` line is then appended to the golden.
  - `VAR_SIL_EVALON_PARTENAIRE` moved from 0x4023 (`VAR_STARTER_MON`, which `choose_starter` writes) to 0x4034 this way. 0x4034 is the next free number, not one beyond every reserved number. The allocator skips reserved numbers wherever they fall, so "above the reserved set" means past the colliding one.
  - `Ids::get` and `Ids::live` prefer the live entry. Code naming a tombstoned-only name fails to compile.
- **Constants.**
  - Emitted only for live maps (`MapId`, `MAP_*` = the `ids/maps.ron` number, group `num >> 8`, map `num & 0xFF`), flags (`FlagId`), vars (`VarId`) and songs (`u16`). Task A adds the rest.
  - Silice names resolve to the generator as `FLAG_0x…`/`VAR_0x…` hex names (`extract::names::Table::value` parses them); Gen3 names pass through.
  - The Silice build does not emit a Gen3 `FLAG_*`/`VAR_*` constant whose number a live Silice id holds. Shared code naming one fails to compile instead of aliasing.

**Script registry (Task F, E)** (`build/silice/registry.rs`, `build/silice/dispatch.rs`).
- **What counts as a script.** syn scans `src/silice/scripts/*.rs` in sorted order; the module is the file stem. A script is a top-level `pub async fn name(ctx: Ctx)`: one input, no return type, no generics. Any other `pub async fn` there fails the build, and so does a name defined in two files.
- **Ids.** ScriptIds are assigned in **name order** over all files.
- **Generated files**, in the Emerald shape:
  - `SCRIPT_COUNT`, `SCRIPT_NAMES` (sorted), `script_ids`, the `SCRIPTS` thunks and `script()`;
  - `SCRIPT_DIGESTS`, which pairs each name, in `SCRIPT_NAMES` order, with `TreeHash::file(name, source)`: FNV-1a 64 over the length-prefixed script name and the whole source of its file, with CRLF folded. The module name is not hashed, and any edit of a file moves the digest of every script in it.
- `scripts::DECOMP_NAMES` is `false`; `TRAINER_SCRIPTS` and `overrides::OVERRIDES` are empty. Script files compile as `content::scripts::<stem>` (`#[path]` modules) and `use crate::silice::prelude::*`.
- **Unknown scripts.**
  - Every map is resolved first. A strict build then fails **once**, listing every unknown name with the maps that use it.
  - Wave 0 runs in the **allowlist mode**: `registry::ALLOW_UNWRITTEN = true` drops each reference (object, coord or bg script becomes `None`; a frame script is removed), lists the names in `content::REPORT` and prints one `cargo:warning` (48 names today).
  - `PKR_SILICE_STRICT_SCRIPTS=1` forces strict. **Task F flips `ALLOW_UNWRITTEN` to `false`** once the Ch0 scripts exist.
- `ch00_wave0.rs` holds one placeholder, `BricoBourg_PlayerHouse_2F_EventScript_Fenetre`; Task F replaces it. xtask `silice check`'s given-item rule waits for scene scripts that name a `FLAG_`, so a placeholder does not trigger it.
- `NEW_GAME_SCRIPT` runs `silice::new_game::new_game`, which is empty.

**World (Task A, E).** `build/silice/mod.rs::world` builds the generator's `World` from `input::load_assets` (the extract without its maps or script IR) plus every `maps/*.ron` with its layout (`load.rs`, `merge.rs`). A map's `name` must equal its file stem.
- **Kept and dropped.**
  - Of the extract's texts, only the Gen3 `gText_*` that the prelude prints by label are kept (`STD_TEXT_PREFIX`).
  - The Emerald trainers are dropped, except `TRAINER_NONE` so trainer numbers stay index-equal. **Task A fills Silice's.**
  - The Emerald encounters and heal locations are dropped.
- **Start.** The new game starts at `START = ("BricoBourg_PlayerHouse_2F", 4, 3)`.
- **Headers.** Music, weather, map type and name are 0 in Wave 0.
- **`silice::Game`:**
  - `respawn_default` is 0;
  - every heal location is the start;
  - there is no whiteout script and no wild-set var;
  - `PLAYER_HOUSES_1F` is Brico-Bourg's 1F twice.

**Shells (Task D).**
- `Store::default_root(Flavor)` and `local_root(Flavor)` are `checkpoints/<flavor>/`; `Db::default_path(Flavor)`.
- `checkpoints/silice/nodes/.gitkeep` is committed; `status.ron` is not, because every `checkpoints/*/status.ron` is git-ignored and `verify` rewrites it.
- The explorer has `Act::SilCh0` (`sil_ch0`, no goal yet), `Act::ALL` (3 acts), `Act::EMERALD` and `Act::SILICE`.

**Gates as run (after the review fixes and the merge of master).**
- Stub (`PKR_EXTRACTED_DIR=/nonexistent`) and Emerald: `cargo test --workspace` is green.
- Silice: `PKR_GAME=silice cargo test -p content -p engine -p extract` is green.
- Emerald trees read `valid (rehash)`; the rehash was not accepted.
- `cargo xtask parity` is identical, and `cargo xtask rom` builds.
- fmt is clean. Clippy is clean plain, with `--features game-native/window,studio/ui`, and with `PKR_GAME=silice -p content -p engine`.

**Wave 1 must do next.**
- **A:**
  - texts from `text/fr.ron` (it has 2 duplicate ids that `parse_texts` rejects, so it needs a ruling first);
  - Silice trainers (the Emerald ones are cleared), encounters and items;
  - header names, music and weather;
  - `LOCALID_*` and the other kinds' constants;
  - map limits.

  `text.rs` is the reader stub.
- **D:**
  - make the shells, explorer goals and xtask commands compile and run under `PKR_GAME=silice`. They use Emerald constants today, so only `content`, `engine` and `extract` build for Silice;
  - add the `--game` flag.
  - Review nits deferred to D:
    - `--act sil_ch0` is accepted on an Emerald build and runs an empty goal list;
    - `Db::open` still takes `stub: bool` rather than a `Flavor`.
- **E:**
  - `game.ron` (start, `SpecialIds`, respawn, cleared flags), replacing `START` and the Wave 0 `GameData` defaults;
  - the new-game script.
- **F:**
  - write the Ch0 scripts;
  - set `ALLOW_UNWRITTEN = false`;
  - remove `ch00_wave0.rs`.
- **G:** the font glyphs.

#### SIL1 Task C as built: the studio map editor

Pulled forward and built first, in parallel with M8 Task L, then revised after review (I1 to I8 below). The code is in `crates/studio/src/map_edit/`. Outside that directory, the task touched:
- `crates/studio/src/{lib.rs, bin/studio_map.rs, paths.rs, ui/mod.rs}`;
- `crates/studio/Cargo.toml`, which gains `ron`, `serde` and the `studio-map` bin;
- `xtask/src/studio.rs`;
- new tests in `crates/studio/tests/map_edit.rs` and the fixture in `crates/studio/tests/fixtures/games/fixture/`.

**Files (the contract SIL1 Task A loads).** A games dir is `games/<game>/`; the editor defaults to `games/silice`.
- `layouts/<Name>.ron` uses `extract::schema::Layout` exactly: size, tileset names, a 2x2 border, and raw blocks (`metatile | collision << 10 | elevation << 12`).
- `maps/<Name>.ron` uses `map_edit::schema::GameMap { name, layout, header, objects, warps, coords, bgs, connections, scripts }`. It is the extract's `MapFile` with these differences:
  - `layout` is a layout name, not an inline `Layout`, so maps may share a layout;
  - there is no `group`/`num` (Silice ids come from `ids/maps.ron`; layout ids from `ids/layouts.ron`);
  - `header` is a `GameHeader` that names its constants instead of the extract's numbers: `music: "MUS_*"`, `region_map_section: "MAPSEC_*"`, `weather: "WEATHER_*"`, `map_type: "MAP_TYPE_*"`, `battle_scene: "MAP_BATTLE_SCENE_*"` (the extract's `battle_type`); `cave` and the four flags stay as they are. A new map gets `MUS_NONE`, `MAPSEC_NONE`, `WEATHER_NONE`, `MAP_TYPE_NONE`, `MAP_BATTLE_SCENE_NORMAL`, cycling and running allowed, name shown. Imported Emerald headers are named through `map_edit::names` (from `include/constants/*.h`; an unnamed value is `<PREFIX>0x<HEX>`);
  - `objects` are `GameObject`s: the extract's `Object` plus `id: Option<String>`, a `LOCALID_*` name;
  - warps and connections name their target map only: `GameWarp { x, y, elevation, dest_map, dest_warp }` and `GameConnection { dir, offset, map }`. `dest_map: "MAP_DYNAMIC"` with `dest_warp: 127` is Gen3's dynamic warp.
- `coords`, `bgs` and `scripts` keep the extract's types.
- Both files are written with `extract::export::to_ron`, the extract's RON style. A save rewrites only the files whose bytes change, each through a temporary file and a rename. A load followed by a save is byte-identical.

**Notes for Task A (the loader mirrors `GameMap`, `GameHeader`, `GameObject`, `GameWarp` and `GameConnection` in `crates/content/build/silice/schema.rs`; the content build cannot depend on the studio).**
- Resolve the header names against the decomp constants plus Silice's own (`MUS_SIL_*`…); fail on an unknown name.
- Object `local_id`s are stable and authored: a new object gets the highest id + 1 and a deletion leaves a gap, so the loader must not renumber. Emit `LOCALID_*` constants from the `id` names, per map, with the `local_id` as value. Scripts refer to objects by those constants only. Gen3 looks templates up by local id, so gaps are fine.
- Warp targets: a map of the same games dir, or `MAP_DYNAMIC` (0x7F7F) with warp 127. A Silice map never targets an Emerald map. A connection targets a map of the same games dir.
- Reserved map numbers: 0x7F7F is `MAP_DYNAMIC`, 0xFFFF is `MAP_UNDEFINED`, and the studio uses 0xFFFE (`render::PREVIEW`, the edited map) and 0xFF00..0xFFFD (`render::NEIGHBOR + i`, connected games dir maps in a preview). `ids/maps.ron` must not hand these out.
- The limits the editor checks are the ones the loader should enforce too: at most 64 objects with local ids 1..63; `(width + 15) * (height + 14) <= 10240` (`MAX_MAP_DATA_SIZE`); at most 255 warps, coord events and bg events; elevations up to 15.

**Model (`model.rs`, no Dioxus).**
- `Doc` holds the `GameMap`, the size, the tileset names, the border and flat blocks. Its constructors are `Doc::new(&NewMap)`, `from_parts(map, layout)` and `from_map_file(&extract MapFile)`. `from_map_file` keeps every warp so indices do not shift: a dynamic one becomes `MAP_DYNAMIC`, one to a map the extract lacks `UNEXPORTED_<group>_<num>` (a validation error until the full import exports it).
- `NewMap { name, layout?, shared_layout, width, height, primary, secondary, fill, fill_collision = 0, fill_elevation = 3, border? }`. The fill parts are range-checked like a brush; the size must satisfy `MAX_MAP_DATA_SIZE`. `GamesDir::new_doc` refuses a name whose map exists, and a layout name whose layout exists unless `shared_layout: true`, which loads the saved layout (size, tilesets, border and blocks; the other fields are then ignored).
- `Editor` applies `Op`s, each as one undo step, with a 256-step limit. `begin_stroke`/`end_stroke` group a pencil drag into one step. It has a clipboard. `is_dirty()` compares the document with the saved one, so undoing back to it clears it. `Editor::unsaved` starts a new map that no file holds yet.
- `Op` is also the edit-script format, a RON list parsed with `implicit_some`, so options take bare values. The ops:
  - cells: `Paint`, `Rect`, `Fill` (flood fill over equal raw blocks, 4-connected), `Ascii` (rows plus a legend of brushes, for drafting from the chapters' plans), `Copy`, `Paste` (clipped at the edges), `Border`, `Resize { width, height, fill, fill_collision = 0, fill_elevation = 3, dx = 0, dy = 0 }`. Resize moves old cell (x, y) to (x + dx, y + dy), and with it the events and the connection offsets along their edge (north and south by dx, east and west by dy); new cells get the fill like a new map; events left outside are validation errors;
  - map properties: `Tilesets`, `Header(GameHeader)`, `Scripts`;
  - events: `Object` (with an optional `id`), `Warp`, `Coord`, `Sign`, `HiddenItem`, `Move`, `Replace` (stores the event as given, ids included), `Delete`;
  - connections: `Connect`, `SetConnection`, `Disconnect`;
  - history: `Undo`, `Redo`.
- Cell ops take optional `metatile`, `collision` and `elevation`. Each part that is set replaces that part of the block. Every part is range-checked, and rectangles and ASCII rows are bounds-checked in 64-bit arithmetic, so no input panics.
- A failing op leaves the document and the history unchanged.

**Catalog and validation (`catalog.rs`, `validate.rs`).**
- Tilesets are the build's own `&'static Tileset`s. They are paired with their extract names through the extracted map files, since every compiled tileset is used by some map. On the stub world the tilesets are `StubPrimary` and `StubSecondary`.
- Maps: `Catalog::for_games` has only the games dir's own maps (a `MapRef` with size, warp destinations and connections), so warps and connections can target nothing else (plus `MAP_DYNAMIC`). `Catalog::from_content` has the build's maps, for imported Emerald maps. Object graphics come from the manifest's sprites. Script names come from `content::data::SCRIPT_NAMES`.
- Errors:
  - unknown tilesets, or a tileset in the wrong slot;
  - metatiles outside `0..primary` and `512..512+secondary`, in cells and in the border;
  - a size over `MAX_MAP_DATA_SIZE`; more than 64 objects; more than 255 warps, coord events or bg events;
  - events out of bounds; elevations over 15;
  - header names without their prefix;
  - object local ids outside 1..63 or duplicated, `id`s that are not `LOCALID_*` or are duplicated, graphics (`OBJ_EVENT_GFX_*`, extracted when a sprite list exists), `MOVEMENT_TYPE_*`, trainer type and `FLAG_*`;
  - warp destinations and their warp index (the map itself counts); a `MAP_DYNAMIC` warp whose index is not 127;
  - `VAR_*`, sign facing, hidden item `ITEM_*` and `FLAG_*`, and secret bases;
  - connections to unknown maps or to the map itself.
- Warnings:
  - script names the build does not have;
  - a warp whose destination has no warp back to this map;
  - two warps or coord events on one cell;
  - two objects with one hide flag, two hidden items with one flag;
  - a second connection in one direction;
  - a connection the neighbor does not mirror (opposite direction, negated offset; dive and emerge pair without offsets);
  - a neighbor placed entirely off the edge it connects to;
  - `validate::id_changes(saved, edited)`: a named object whose local id changed (scripts and checkpoints use the number). The CLI's `apply` and the UI list them.

**Renders (`render.rs`).** `map_view::render_map` draws the map through `Preview`, a `GameData` that serves a leaked `MapData` at `PREVIEW` (`MapId(0xFFFE)`) and delegates everything else to the build.
- One base `MapData` is cached per (size, tilesets, border, connections), and the cells that differ from it are drawn as `setmetatile` overrides. This bounds the leaks.
- Connections are drawn in the margin: to the build's maps by id, and to games dir maps (loaded from `Catalog::games`) as leaked `MapData`s, one per layout contents, served at `NEIGHBOR + i` (0xFF00 + the connection's index).
- The palette is the tileset pair drawn as an 8-wide map of its metatiles. The primary's metatiles come first, and the secondary's start on a new row.

**Hygiene (`games::hygiene`).** A games dir may hold only canonical `maps/*.ron` and `layouts/*.ron`. When the extract is present, `games::copies` also flags a layout whose metatiles equal an extracted layout's (collision and elevation ignored), or that holds a 10x10 square of one with at least 12 distinct metatiles (uniform fills and plain patterns match everywhere and are not flagged). Authored maps may use the extract's tilesets, not its maps.

**CLI.** The command is `cargo xtask studio map …`. It runs the `studio-map` bin with `cargo run` from the workspace root, so relative paths are relative to the workspace. `--game` takes a name (letters, digits and `_`).

```text
studio-map [--games DIR | --game NAME] list
studio-map … create <spec.ron>                       # NewMap; never replaces a map, nor a layout unless shared
studio-map … apply <Map> <script.ron> [--dry-run]   # saves even a draft with errors (listed); nothing if an op fails
studio-map … validate [<Map>...]                    # exit 1 on errors; goes on past unreadable maps; lists orphan layouts
studio-map … render <Map> [--out FILE] [--margin N] # default target/studio/maps/<Map>.png (git-ignored)
studio-map palette <Primary> <Secondary> [--out FILE]
```

Example spec and edit script (the shape of the fixture's `FixtureTown`):

```ron
(name: "FixtureTown", width: 8, height: 6, primary: "General", secondary: "Petalburg", fill: 1, border: [468, 469, 476, 477])
```
```ron
[
    Rect(x: 0, y: 0, w: 8, h: 1, metatile: 468, collision: 1),
    Ascii(x: 0, y: 1, rows: ["T......T", "T......T"], legend: {'T': (metatile: 468, collision: 1), '.': (metatile: 1)}),
    Fill(x: 1, y: 4, metatile: 13),
    Object(x: 2, y: 2, id: "LOCALID_FIXTURE_BOY", gfx: "OBJ_EVENT_GFX_BOY_1", movement: "MOVEMENT_TYPE_WANDER_AROUND", range_x: 1, range_y: 1, script: "FixtureTown_EventScript_Boy"),
    Object(x: 5, y: 3, gfx: "OBJ_EVENT_GFX_YOUNGSTER", trainer_type: "TRAINER_TYPE_NORMAL", sight: 3, script: "FixtureTown_EventScript_Trainer", flag: "FLAG_HIDE_FIXTURE_TRAINER"),
    Warp(x: 4, y: 5, dest_map: "FixtureRoute", dest_warp: 0),
    Warp(x: 7, y: 5, dest_map: "MAP_DYNAMIC", dest_warp: 127),
    Coord(x: 4, y: 4, var: "VAR_FIXTURE_STATE", value: 0, script: "FixtureTown_EventScript_Trigger"),
    Sign(x: 6, y: 1, script: "FixtureTown_EventScript_Sign"),
    HiddenItem(x: 1, y: 5, item: "ITEM_POTION", flag: "FLAG_HIDDEN_ITEM_FIXTURE_POTION"),
    Connect((dir: North, offset: 0, map: "FixtureRoute")),
    Header((music: "MUS_LITTLEROOT", region_map_section: "MAPSEC_NONE", cave: 0, weather: "WEATHER_SUNNY", map_type: "MAP_TYPE_TOWN", allow_cycling: true, allow_escaping: false, allow_running: true, show_map_name: true, battle_scene: "MAP_BATTLE_SCENE_NORMAL")),
]
```

**UI (feature `ui`): the studio's Edit tab.**
- `session.rs` holds the tools and overlay shapes as a Dioxus-free core: pencil, rect, fill, picker, select (a drag moves the event), and placing an object, warp, coord or sign. A paint mode chooses whether the brush writes the metatile, the collision or the elevation.
- `ui.rs` is thin: a map list and a new-map form, the toolbar, Undo, Redo and Save, and zoom. The layer toggles are metatiles, collision, elevation and events.
- Opening another map (or creating one) while the open map has unsaved changes is refused with a message and a "Discard changes" button. Create goes through `GamesDir::new_doc` (no existing map or layout), and the first Save of a new map through `GamesDir::create`.
- The map picture and the palette are served by an `edit` asset handler from `EditImages`, which `ui/mod.rs` registers. An SVG layer draws the overlays and takes the mouse.
- Clicking the palette picks a metatile.
- The inspector edits the selected event as RON (Apply, Delete). The connections are edited as a RON list, applied as one undo step.
- The issue list (validation and `id_changes`) updates on every change.

**Write locations.** `paths::write_locations` gains `<workspace>/games` as `Committable::Authored`, and a test checks that it is not git-ignored.

**Tests.**
- Unit tests in `map_edit/tests.rs`, `session.rs` and `names.rs`: paint, rect, fill (and that it does not cross diagonals), ASCII, copy and paste, strokes, undo and redo and the 256-step cap, dirty tracking against the saved state, Resize growing and shrinking (kept cells, fill, shifted events and offsets, events left outside), the Border, Tilesets, Header and Scripts ops, event CRUD with stable local ids, out-of-range and overflowing input as errors, `MAP_DYNAMIC`, the game limits and link warnings, `id_changes`, the byte-identical round trip and edit-script parsing.
- `tests/map_edit.rs`:
  - sample extracted maps (the M7 slice's 15 plus a few later ones the build has; TODO for M8 Task L: `content::samples::map_ids()`) load into the editor without errors and render pixel-identical to the game's `render_map`; an `#[ignore]` variant checks every map; extracted warps keep their indices;
  - an edited map renders equal to a direct `MapData` render, on both worlds; on the real extract, Route 101 with painted cells also equals the game's render with those cells as overrides;
  - the fixture (`FixtureTown` and `FixtureRoute`, which warp and connect to each other only) round-trips byte for byte, validates with only unwritten-script warnings, rejects an Emerald warp target, and draws its connected games dir neighbor;
  - a CLI session gives identical bytes in two dirs, keeps local ids, warns on a renumbered named object, and rejects a bad `--game`;
  - new maps share a layout only when asked (CLI and API), and a refused create writes nothing;
  - a failing script writes nothing; `validate` fails on errors, goes on past a map whose layout is missing and lists orphan layouts;
  - the hygiene check: planted PNG, extra-field and reformatted files are flagged; a copied Route 101 layout (collision repainted) and a pasted 10x10 square of it are flagged, a uniform fill is not.
- ssr tests render the Edit tab empty and with the fixture open.

**Gates:**

| Gate | Result |
|---|---|
| `cargo fmt --check` | clean |
| clippy, plain and with `game-native/window,studio/ui` | clean |
| `cargo test --workspace`, real | 1404 passed, 14 ignored |
| `cargo test --workspace`, stub | 1392 passed, 14 ignored |
| `cargo xtask studio --check` | 180 passed |

**Limitations.**
- The UI was not driven by hand in a window; only its ssr renders and its core are tested.
- Renders leak one `MapData` per distinct (size, tilesets, border, connections), and one per distinct connected games dir layout.
- Dive and emerge connections are not drawn.
- Flags, vars, items and movement types are checked by prefix only, until `ids/*.ron` exist (Task A).
- The header is edited only through the `Header` op, not in the UI.
- The palette shows no metatile numbers. The CLI prints its layout: 8 per row, and the cell where the secondary starts.

#### SIL1 Task C, editor upgrades as built

These upgrades answer the friction the first six Silice maps hit: no metatile numbers or behaviours in the pictures, invisible collision mistakes, buildings typed as throwaway `Ascii` letters, three passes for linked maps, one `cargo run` per call, and confusing case clashes. The code is in `crates/studio/src/map_edit/`: new modules `behavior.rs`, `overlay.rs`, `stamp.rs`, `resources.rs` and `batch.rs`. The tests are in `map_edit/tests.rs`, `tests/map_edit.rs` and the new `tests/map_edit_tools.rs`.

**Games dir layout.** The editor now owns four dirs:

```text
games/<game>/maps/<Name>.ron       GameMap (unchanged)
games/<game>/layouts/<Name>.ron    Layout (unchanged)
games/<game>/stamps/<name>.ron     Stamp: a small cell grid (below)
games/<game>/edits/<Map>/<phase>_<what>.ron
                                   the map's create spec (<what> = create) and edit scripts
```

**Stamp format (`stamp.rs`).** A stamp is canonical RON, written by `stamp-from` or by hand:

```ron
(
    primary: "General",
    secondary: "Petalburg",
    width: 2,
    height: 1,
    elevation: true,
    blocks: [
        (13313, 13314),
    ],
)
```

- `blocks` holds raw blocks, as a layout does: `metatile | collision << 10 | elevation << 12`.
- With `elevation: false`, stamping keeps the map's elevations, and the blocks' elevation bits are 0.
- A stamp is keyed to its tilesets. The map's primary tileset must be the stamp's. Its secondary must match too, but only when the stamp uses secondary metatiles (512 and up).

**Edit script additions (`model.rs`).**
- `AllowCollision(cells: [...], metatiles: [...], reason: "...")` accepts behaviour warnings on purpose (see Allowances below).
- `Stamp(x: 5, y: 4, name: "House")` paints `stamps/House.ron` with its top-left at (x, y). The stamp must fit in the map, and its tilesets are checked as above.
- `AutoCollision()` or `AutoCollision(rect: (x: 0, y: 0, w: 10, h: 4))` sets collision from the usual-collision heuristic below. It changes only cells whose walkability disagrees with the usual collision, to 0 or 1, so a deliberate collision of 2 or 3 on a solid cell stays.
- `Ascii` legends accept any character as a key. Write it as a RON char, such as `'#'`, `'"'`, `' '` or `'é'`, with the escapes `'\''` and `'\\'`. A one-character string such as `"\""` also works.
  - Inside the rows, escape `"` and `\` (`"\"\\"`), or use raw strings (`r#"..."#`).
  - A key of more than one character is an error.
- `Stamp` and `AutoCollision` read outside the document through `model::Env`. The CLI and `build` pass `resources::Resources`, which holds the games dir and the catalog. An `Editor` without an env refuses both ops with "needs a games dir". The UI does not use them.

**Behaviours and usual collision (`behavior.rs`, heuristics, documented there).**
- **Behaviour.** A metatile's behaviour is the `MB_*` byte of its tileset's attributes, the same `attr & 0xFF` the game reads. It is named through `extract::names` and classified by the build's `GameData::behavior`.
- **Codes.** One letter per behaviour class:
  - `D` door;
  - `W` warp cell (a plain door, stairs or ladder);
  - `A` arrow warp (a warp mat or exit);
  - `G` tall grass;
  - `L` ledge (solid; jumped from the uphill side);
  - `C` counter;
  - `S` water (`*WATER*` names);
  - `B` another blocked behaviour (currents, water doors);
  - `I` directional wall (`MB_IMPASSABLE_*`, M8 Task L): walkable, but some sides cannot be crossed. It has no usual collision of its own.
  Normal metatiles have no code.
- **Usual collision of a cell.** The first rule that applies wins:
  1. **The extract's verdict.** If the extracted maps use the metatile (with the same tileset) in at least `MIN_USES` = 5 cells:
     - at least `MIN_AGREEMENT` = 95% walkable gives 0;
     - at least 95% solid (collision 1..=3) gives 1;
     - anything less clear gives no usual collision (water at map edges, for example).
     The counts are taken at run time from the extracted map files that `Catalog::from_content` already reads (`Catalog::collision_uses`). Extracted map files inline their layout, so maps sharing a layout are deduplicated by its contents and counted once. Nothing is committed.
  2. **The behaviour.**
     - Tall grass and water are walkable.
     - Ledges and counters are solid. Gen3 jumps a ledge before it checks collision, and all 1,054 Emerald ledge cells are solid.
     - Warp cells and arrow warps are walkable when a warp event is on the cell, and have no usual collision without one.
     - Doors have none: Emerald has doors with warps both walkable and solid, because the door opens when the player pushes against it.
  3. Otherwise, none.
- **Validation warnings** (never errors):
  - a cell walkable against a solid usual collision, or the reverse (the message gives the reason: "in K of N extracted cells using it", or "as a water metatile (MB_OCEAN_WATER)");
  - a walkable door, warp-cell or arrow-warp metatile without a warp event;
  - a warp event on a metatile that triggers no warp ("fine if it is only an arrival point");
  - an allowance that silences nothing.
- **Allowances.** `AllowCollision(cells: [(4, 7)], metatiles: [584], reason: "...")` accepts the behaviour warnings of the listed cells, and of every cell holding a listed metatile. Either list may be omitted, but not both, and the reason is required.
  - It changes no cell and is not saved in the map file: it lives in the map's edit scripts, where `GamesDir::load` (and so `validate`, `build` and `apply`) reads it.
  - Keep each reason honest (why the cell is right as it is). An allowance that silences nothing is itself a warning, so stale ones surface.
- **Tuning.** On the 15-map extract, these warnings fire for only a few cells (edge water used once or twice) and one Route 103 warp. The full 518-map extract (now the default) has statistics for many more metatiles, so more Silice cells warn (see Ch0 maps as built). With the full extract the fixture also warns (FixtureTown (3, 3)), so its test accepts behaviour warnings. To fix the fixture's warps, its warp cells were repainted with metatile 167 (`MB_NON_ANIMATED_DOOR`).

**Pictures (`overlay.rs`).** Text uses a built-in 3x5 bitmap font: digits, the code letters and `!`. A picture with labels or an overlay is scaled 2x unless `--scale N` (1..=8) says otherwise.
- **`palette --labels`** draws each metatile id at the top-left of its cell and the behaviour code at the bottom-right. It prints the legend: each code, then its metatiles grouped by `MB_*` name.
- **`render --events`** frames each event's cell and labels it:
  - objects in yellow, with their local id;
  - warps in magenta (`W<index>`);
  - coord events in cyan (`C<index>`);
  - signs in green (`S<bg index>`);
  - hidden items in orange (`H<bg index>`).
  Labels stack when several events share a cell.
- **`render --collision`** tints solid cells red and draws each cell's behaviour code. Cells whose collision disagrees with the usual one are outlined in yellow and marked `!`.
- A render without these flags is unchanged, byte for byte.

**Batch mode (`batch.rs`): `studio-map … build [<edits dir>]`.** The edits dir defaults to `<games>/edits`.
- **Order.** In one process, the build runs:
  1. every create, in map-name order;
  2. every script, sorted by phase number, then map name, then file name;
  3. one validation of the whole games dir (plus the orphan-layout listing).
- **Equivalence.** This is exactly `create` for each create spec in that order, then `apply` for each script in that order. Each step reads the map and its layout as the previous steps left them (in memory) and writes both back, so the bytes are identical. A test checks this against the step-by-step CLI.
- **Rebuilds.** A map with a create file is rebuilt from scratch: its saved map file is ignored, and so is its layout unless the spec shares one. Running `build` again on a populated dir gives the same files. A map dir without a create file edits the saved map in place.
- **Links.** Links are checked only by the final validation, so mutually linked maps need one pass and show no transient errors.
- **Failures.** These stop the build before anything is written:
  - a failing op (named by file and op index);
  - a create that does not validate, or whose `name` is not its dir;
  - a stray file (anything but `<digits>_<what>.ron`);
  - a second create for a map;
  - map dirs whose names differ only in case.
  Errors from the final validation fail the command after writing, as `apply` saves drafts.

**Hygiene (`games::hygiene`, and the new `studio-map … hygiene` command, which exits 1 on any problem).**
- `stamps/*.ron` must be canonical `Stamp` RON, as maps and layouts are, with blocks that match the stamp's size. This is checked with or without the extract.
- `edits/` may hold only `<Map>/<phase>_<what>.ron` files that parse: as a `NewMap` for `create`, and as a list of `Op` otherwise. Dot entries (`.DS_Store`…) are skipped there, by `hygiene` and by `build`.
- **Copies.** `games::copies` checks stamps as it checks layouts: a stamp equal to an extracted layout, or holding a 10x10 square of one with at least 12 distinct metatiles, is flagged. A stamp smaller than 10x10 cannot be flagged on its own. A map it is stamped on is flagged if the result copies an extracted square.

**Case collisions.**
- `GamesDir::{new_doc, create, save, load_map, load_layout, load_stamp, save_stamp}` refuse a name that differs from a saved map, layout or stamp only in case, on every OS: "map Routeone differs from map RouteOne only in case (one file on a case-insensitive file system): use RouteOne, or another name".
- `validate` and `build` report saved maps whose names differ only in case as errors (possible on case-sensitive file systems).

**CLI as built:**

```text
studio-map [--games DIR | --game NAME] list
studio-map … create <spec.ron>
studio-map … apply <Map> <script.ron> [--dry-run]
studio-map … build [<edits dir>]                     # default <games>/edits
studio-map … validate [<Map>...]
studio-map … hygiene                                 # exit 1 on any problem
studio-map … render <Map> [--out FILE] [--margin N] [--events] [--collision] [--scale N]
studio-map palette <Primary> <Secondary> [--out FILE] [--labels] [--scale N]
studio-map … stamp-from <Map> <x> <y> <w> <h> <name> [--no-elevation] [--force]
```

**Speed.**
- **Measured.** With the bin already built, one `cargo run` of `studio-map` costs about 0.55 s, and the bin alone about 0.03 s. A `build` of 6 linked 30x20 maps (6 creates and 12 scripts) runs in about 0.06 s, or 0.6 s through `cargo run`.
- **Step by step.** The same work takes 19 `cargo run` calls, about 10 s when nothing recompiles.
- **Reported.** The ~40 s reported for 25 calls included rebuilds.
- **Estimate.** A rebuild is about 15x faster when warm, and about 60x against the reported figure. It is also one call instead of about 25, and needs no create, terrain and events passes.

**Gates:**

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| clippy `-D warnings`, plain and with `game-native/window,studio/ui` | clean |
| `cargo test --workspace`, real (env as in the brief) | 1416 passed, 14 ignored |
| `cargo test --workspace`, stub (`PKR_EXTRACTED_DIR=/nonexistent`) | 1404 passed, 14 ignored |
| `cargo xtask studio --check` | 192 passed |
| after the review fixes and the master merge: `cargo test -p studio`, real and stub | 168 passed, 4 ignored each |
| after the review fixes: `cargo xtask studio --check` | 193 passed |
| after merging M8 Task L (full 518-map extract, `Impassable` code `I`): `cargo test -p studio`, real and stub | 169 passed, 8 ignored each |
| same: `cargo xtask studio --check` | 194 passed, 8 ignored |
| same: `studio-map --game silice validate` / `hygiene` | only unwritten-script warnings / clean |

**Limitations.**
- The usual-collision statistics come from the extract the build has. With the M7 slice (15 maps) most metatiles fell back to their behaviour; the full 518-map extract (M8 Task L, now the default) sharpened them with no code change.
- The labelled palette of a full tileset pair is tall (`General` + `Petalburg` is 256x2624 at scale 2). Viewers downscale it, so crop it or read it at `--scale 1` for an overview.
- The UI's Edit tab does not offer stamps, `AutoCollision` or the overlays yet. Its editors have no env.
- The extract's own maps raise a few behaviour warnings, which is expected for heuristics. The fixture was repainted so that it raises none.

#### SIL1 Task C, editor friction fixes as built

These fixes answer the friction the Ch1 maps hit (see "Editor friction" in the Ch1 maps notes). The code is in `crates/studio/src/map_edit/` (`cli.rs`, `model.rs`, `overlay.rs`, `render.rs`, one `pub` in `behavior.rs`). The catalog and the test gating were left alone. Every change is backward compatible: existing edit scripts parse and build to the same bytes. A test rebuilds a copy of the committed `games/silice/edits` and compares it with the committed maps and layouts byte for byte.

**Reference renders of Emerald maps.**
- `studio-map render <Map> --extract [--events] [--collision] [--margin N] [--scale N] [--out FILE]` renders the extract's map `<Map>` (`<extracted>/maps/<Map>.ron`, loaded with `Doc::from_map_file`) against the build's catalog, so connected Emerald maps are drawn in the margin. The default output is `target/studio/maps/extract-<Map>.png`.
- `studio-map dump <Map> [--extract] [--out FILE|-]` writes the metatile grid as text. It works on a games dir map, or on an extract map with `--extract`. The default output is `target/studio/maps/[extract-]<Map>.txt`, and `--out -` prints the grid. The format:

  ```text
  LittlerootTown: 20x20, General + Petalburg; metatile ids, * = solid
  border (row-major 2x2): 468  469  476  477
  y\x   0    1    2 ...
    0 468* 469* 468* ...
  ```

- Both write only to the scratch dir by default. Extract renders and dumps are ROM-derived and are never committed (copying an extracted layout is also a `hygiene` error).
- Without the extract (the stub world) `--extract` fails and says so.

**Palette.**
- `--range A..B` (B excluded) or `--range A..=B` keeps only those metatile ids. The secondary still starts on a new row. A range that holds none of the pair's metatiles is an error. The output lists the first metatile of each row, and the default file name gains `-A-B`.
- `--labels` now draws each cell's id (left) and behaviour code (right) in a gutter band above the cell, so labels never cover the art. A column is at least as wide as a 4-digit id and a code, and columns and rows are separated by a gutter.
- `--zoom N` (1..=8) scales the art, and `--scale N` scales the labels. The zoom defaults to the scale, so `--scale 1 --zoom 4` gives small labels over large art. Without labels the zoom alone sets the size.
- `palette` also prints the pair's void metatiles: the cells drawn all black (`Palette::void_metatiles`).

**`Object(local_id: N)`.** This is an optional field. When given, it must be in 1..64 and not used by another object of the map, or the op fails and changes nothing. Without it, the object gets the highest local id + 1, as before. An object added after an explicit id takes that id + 1 when it is the highest.

**`ScriptsSet`.** `ScriptsSet(on_load: "Map_OnLoad")` sets only the fields it names and keeps the others. The fields are `on_load`, `on_transition`, `on_resume`, `on_return_to_field`, `on_dive_warp`, `on_frame` and `on_warp_into`.
- A string field set to `""` clears it.
- `on_frame` and `on_warp_into` replace their whole list.
- A `ScriptsSet` with no field is an error, since a misspelled field would otherwise do nothing.
- `Scripts(...)` is unchanged and still replaces the whole record.

**`render --collision` and allowances.** A cell whose collision disagrees with the usual one is still framed yellow and marked `!`. When an `AllowCollision` of the map's edit scripts accepts it (by cell or by metatile), it is framed green and marked `+` instead. The command's legend line says so.

**Building's metatile 1 (investigated).**
- `Building`'s metatile 1 draws its bottom layer from the primary's tiles and palette 5. The secondary's tiles start at 512 and its palettes at 6, so no secondary can change it.
- A test renders it with each of the build's secondaries (70 on the full extract) and finds it all black every time.
- Pictures can mislead, though. Some secondaries also have a black metatile of their own. Emerald's `Lab` and `PokemonCenter` maps use 513 as their border, `GenericBuilding` maps use 513 (46) or 1 (27), and `BrendansMaysHouse` maps mostly use 543. Of Emerald's 267 maps on `Building`, 153 have border metatile 1 and 100 have 513; the rest use 515, 519, 520 or 543.
- Rather than a warning or a hand-kept table, `palette` now prints the pair's void metatiles, computed from the render (for example `void (all-black) metatiles, for an interior's border: 1 513` for `Building` + `Lab`). Either id is a correct interior border.
- The Ch1 report that metatile 1 "draws as floor" with `Lab` could not be reproduced in the editor's renders (the game's own field renderer). Out-of-map cells are solid in Gen3 whatever the border's collision bits.

**CLI as built (changes only):**

```text
studio-map … render <Map> [--extract] [--out FILE] [--margin N] [--events] [--collision] [--scale N]
studio-map … dump <Map> [--extract] [--out FILE|-]
studio-map palette <Primary> <Secondary> [--out FILE] [--labels] [--scale N] [--zoom N] [--range A..B | --range A..=B]
```

```ron
[
    Object(x: 4, y: 6, gfx: "OBJ_EVENT_GFX_WALLY", id: "LOCALID_BOIS_LOIC", local_id: 20),
    ScriptsSet(on_load: "BricoBourg_OnLoad"),
]
```

**Tests.**
- Unit tests (`map_edit/tests.rs`):
  - `objects_take_an_optional_local_id`: explicit ids, the next automatic id, and refusals for a duplicate, 0 or 64, which leave the document unchanged;
  - `scripts_set_merges_into_the_map_scripts`: the merge, clearing with `""`, the lists, the empty-op error, and one undo step each;
  - `allowed_mismatches_render_as_accepted`: yellow `!` against green `+`;
  - `dump_prints_the_metatile_grid`;
  - `metatile_ranges_parse`.
- `tests/map_edit_tools.rs`:
  - `extract_maps_render_and_dump`: Littleroot's size and grid, the file and stdout outputs, refused names, and the stub's error;
  - `palette_ranges_zoom_and_gutter_labels`: sizes for range and zoom, labels in the gutter, and art pixels identical to the plain palette's;
  - `building_metatile_1_is_void_with_every_secondary`;
  - `local_ids_and_scripts_set_through_build`;
  - `committed_silice_edits_rebuild_byte_identical`;
  - `overlays_and_labels` updated for the gutter widths.

**Gates:**

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| clippy `-D warnings`, plain and with `game-native/window,studio/ui` | clean |
| `cargo test -p studio`, real (env as in the brief) | 183 passed, 8 ignored |
| `cargo test -p studio`, stub (`PKR_EXTRACTED_DIR=/nonexistent`) | 183 passed, 8 ignored |
| `cargo xtask studio --check` | 208 passed, 8 ignored |
| `studio-map build` on the committed `games/silice/edits` | byte-identical (no file changed) |

**Limitations.**
- `dump` has no elevation and no raw blocks. `stamp-from` still reads only games dir maps, on purpose, because an extract stamp would be a copy.
- `ScriptsSet` cannot append to `on_frame`; it replaces the list.
- The void list is computed from the render and does not say which void Emerald prefers for a pair. `dump --extract` of a reference map shows its border.

#### SIL1 Task B as built: the importer and the validator

The code is in `xtask/src/silice/`: `mod.rs` (CLI, file writing), `md.rs` (chapter markdown), `names.rs` (constant words, families, map names), `maps.rs` (names in `maps/*.ron` and, later, the scene scripts), `import.rs`, `ids.rs` (allocation), `check.rs` and `schema.rs`. Outside it, the task touched one arm in `xtask/src/main.rs`, `regex` in `xtask/Cargo.toml`, `xtask/tests/silice_{import,check,games}.rs` (with `silice_common/`) and the synthetic fixtures in `xtask/tests/fixtures/silice/`, among them `ids.golden`.

**Commands.**
- `cargo xtask silice import <contenu-jeu> [--out games/silice] [--report FILE] [--maps DIR] [--force]` reads `chapitres/*.md` in name order. It is deterministic: two runs give byte-identical files, and a re-run over unchanged output writes nothing. Without `--force` it refuses, and writes nothing, if any existing file would change. Every ambiguity goes to the report, never into the data. The report defaults to `<out>/IMPORT-REPORT.md` and is committed: it is an output like the others, under the same `--force` rule, and the editor's hygiene only looks at `maps/` and `layouts/`. The import also reads `<out>/maps/*.ron` (`--maps DIR` to change it, `--maps /nonexistent` to skip it) and allocates the `FLAG_SIL_`, `VAR_SIL_`, `MUS_SIL_` and `SE_SIL_` names only the maps use.
- `cargo xtask silice check [--games games/silice] [--extracted DIR] [--scripts DIR] [--allow RULE]...` exits 1 on errors and lists warnings. Its name sources are the decomp tables compiled into `extract::names` and `charmap::SOUNDS` (always), the extract when it has a `manifest.ron` (species, moves, items, heal locations, the FR font widths), and every name in `$PKR_DECOMP/include/constants/*.h` when `PKR_DECOMP` is set (the only source for `WEATHER_*` and the decomp's songs). A missing source skips its rules with a `note:` line, so the check runs without the extract. It reads `maps/*.ron` textually (object and bg flags, coord and frame-script vars, music, weather, warp `dest_map` and connection `map`, hidden items). `--scripts` (default `crates/content/src/silice/scripts/`) is the hook for the scene scripts: every constant-looking word in its `*.rs` files is checked like a reference. It is empty until the scripts exist.
- `cargo xtask silice tombstone [--games DIR] NAME...` marks ids removed in whichever ids file holds them, keeping their numbers.

**Formats (`schema.rs`, ready to move to `crates/content/build/silice/schema.rs`; it reuses `extract::schema`).**
- `text/fr.ron` maps each id to its text in decomp `.string` syntax. Each text is written as a RON raw string, `r"..."` (with as many `#` as the text needs), so `\n`, `\p` and quotes read as in the source. A unit test checks that every text round-trips through `ron` unchanged. Body lines are joined raw; a line without `\n`, `\l` or `\p` is joined with a space and reported. A duplicate id with a different body is written twice, as a repeated key, so `check` can see it. The content stays as authored (the 2 duplicates are `ruling_needed` exceptions). **Loader contract:** the Wave 0 loader must read `text/fr.ron` entry by entry and reject a repeated key, since a map deserializer would keep one silently.
- `text/speakers.ron` holds, per id, the chapter, the speaker and the parenthetical note. `check` requires its id set to equal `fr.ron`'s.
- `battle/trainers.ron` holds the extract's `Trainer` fields plus `items` (the AI bag) and `notes` (the source's `//` comments). A comment goes to the entry it follows on the closing line, or on the next line when indented deeper than that entry; otherwise it goes to the next entry (`LOIC_1` and `YOHAN_1` carry their rematch notes). The party is `extract::schema::TrainerMon`, so held items are kept.
- `battle/encounters.ron` follows the v5 shape: keyed by map name, a `Vec` of headers (land, water, rock smash, fishing, `notes`).
- `items.ron` holds three kinds of record.
  - The `ITEM_SIL_*` definitions: name, pocket, price, `clone_of`, effect and chapters.
  - The §7 placements: flag, map, place, `pos`, name, item, quantity and method. `pos` is a note from the source's plans, which are not to scale. The maps under `maps/` are authoritative for positions.
  - The marts:
    - each has a label (no `**` or backticks), a map, a `condition` (the label's parenthetical when it names a `FLAG_`/`VAR_`), a `note` (prose between the label and the table, or the tail of an inline list), money or coins, and its stock;
    - each stock entry has an item, a `u16` price, and an optional `condition` and `note` (the Condition and Note/Pourquoi columns);
    - all prices are `u16`;
    - inline stock is `(ITEM_X, price)`, with or without backticks;
    - a bold paragraph that names an item but gives no stock is reported.
- `ids/{flags,vars,trainers,items,species,moves,songs,maps}.ron` are `IdsFile { base, ids: [(name, num, map?, removed?)] }`.
  - The ranges:
    - flags run from 0x20 to 0x4FF and vars from 0x4020 to 0x40FF. On the first import, a file starts above every Gen3 story flag or var the data reuses, so flags start at 0x113, above `FLAG_RECEIVED_RUNNING_SHOES` (0x112). The report's "Id bases" says so;
    - trainers run from 1 to 863;
    - items, species and moves come after the decomp's last number;
    - songs come after `SOUNDS`. Music and sounds share one file because Gen3 gives them one number space;
    - maps start at 0 and skip 0x7F7F.
  - Allocation is append-only. Names are numbered in name order on the first import, and later new names are appended after the highest number. A name the source drops stays, and is reported as stale. A tombstone (`removed: true`) is never renumbered.
  - Each map entry carries its map file name (`MAP_BRICO_BOURG_PLAYER_HOUSE_1F` is `BricoBourg_PlayerHouse_1F`).
  - The Ch0 maps' names were appended after the first allocation: 14 `FLAG_SIL_HIDE_*` and `VAR_SIL_CH0_GATE`. `FLAG_SIL_HIDE_BAR_SBIRES` is tombstoned, because the maps split it into `_SBIRE_A` and `_SBIRE_B`.
  - `xtask/tests/silice_games.rs` checks that every (file, name, num) in `fixtures/silice/ids.golden` (951 ids) keeps its number. It also checks that `silice check` passes on the committed data without the extract.
- `ids/refs.ron` holds every constant and text id the chapters use, with their chapters, and every dynamic reference with its expansion.
- `check-exceptions.ron` holds `(rule, subject (with * globs), reason, ruling_needed)` entries.

**What the import reads.**
- Maps come from the constants in §2 and §4 headings, including numbered headings (`### 2.6 MAP_…`) and families (`MAP_X (1F), _X_2F`, `MAP_X, _2F, _3F`).
- Texts come from the `[Id] SPEAKER (note)` headers in `text` fences, several per fence. A header counts only at the start or after a blank line. A `text` fence without a header is a pseudo-script.
- Trainers come from the §5 `ron` fences and encounters from the §6 ones.
- Items come from the §7 tables and the inline shops.
- References come from everything outside data blocks. The scanner handles these shorthands:
  - `X` / `_Y`, `, _Y` and `(+ _Y)`;
  - ranges such as `X_1…4` and `` `X_01` … `_08` ``;
  - placeholders such as `<n>` and `_NN`;
  - `msgbox(A | B)` enumerations.
- Dynamic text ids are expanded against the defined texts. Constant patterns are expanded only within Silice's own prefixes.
- A macro call (`TRAINER_FLAG(…)`) and an elided id (`msgbox(…, Sign)`) are not references.
- Tables the importer cannot type are listed in the report and not imported: prose shop stock, the ch03 Pokémon prize table, the ch08 and ch09 reward-rank tables, and multi-flag cells such as `FLAG_SIL_ECHANTILLON_1/2/3`.

**Check rules.**

| Rule | Level | Fires on |
|---|---|---|
| `id-alloc` | error | duplicate name or number in an ids file, a number outside the range or reserved, a number two song files share, or a legacy `ids/music.ron` or `ids/sounds.ron` |
| `gen3-collision` | error | a Gen3 story flag (0x20–0x4FF) or saved var (0x4020–0x40FF) the data uses, numbered inside the Silice block of its ids file |
| `id-undefined` / `id-removed` | error | a Silice name (from the chapters, maps or scripts) missing from its ids file, or a reference to a tombstone |
| `id-unknown` | error | a Gen3 name (species, class, pic, AI) the decomp lacks; the base species of a form |
| `sil-prefix` | error | an unknown FLAG/VAR/ITEM/MOVE/MUS/SE/WEATHER/HEAL_LOCATION/TRAINER name without `_SIL_` |
| `map-undefined` | error | a `MAP_*`, encounter map, warp or connection target in neither `ids/maps.ron` nor `maps/`; a placement whose map cell is not a `MAP_` constant |
| `placement-map` | error | a ball or hidden-item placement whose map exists in `maps/` but lacks its flag, or hides another item under it. Once scene scripts exist, it also fires when a given item's flag is in neither the map nor a script |
| `trainer-undefined` / `trainer-duplicate` | error | a `TRAINER_SIL_*` that no trainer defines, or one defined twice |
| `trainer-name` / `trainer-class-name` | error | more than 10 or 12 encoded bytes |
| `trainer-items`, `party-size`, `party-moves`, `party-level` | error | more than 4 bag items; 0 or more than 6 mons; 0 or more than 4 moves; a level outside 1–100 |
| `encounter-slots` / `encounter-level` | error | not 12/5/5/10 slots; levels outside 1–100 or min > max |
| `fishing-unreachable` | warning | fishing slots of a rod (old 0–1, good 2–4, super 5–9) that no placement or mart gives |
| `text-duplicate` / `text-missing` / `dynamic-unresolved` | error | a repeated id; an id printed but not defined; a placeholder the source does not enumerate |
| `speakers-mismatch` | error | an id in only one of `text/fr.ron` and `text/speakers.ron` |
| `text-charmap` | error | a character the FR charmap cannot encode; the run prints a histogram of the missing glyphs |
| `text-width` | error | a line over 216 px (27 tiles), or 208 px for a defeat text (an id containing `_Defeat`), measured with the extract's FR widths. Worst cases are used: `{PLAYER}` and `{RIVAL}` as 7 of the widest capital, `{STR_VAR_n}` as 13, and an unencodable character as `W` |
| `text-lines` | error | a box (between `\p`) with more than 2 lines, that is a second `\n` (`\l` scrolls) |
| `invented-move` | warning | a `MOVE_SIL_*` (D-00, D-02b) |
| `item-incomplete` / `item-name-length` / `item-name-clash` | warning | no name, pocket or base item; a name over 13 bytes (`ITEM_NAME_LENGTH` 14 with the terminator); a name equal to another item's, ignoring case and spaces |
| `exception-unused` | warning | an exception that matches nothing |

**Real import (committed under `games/silice/`, authored data only).**
- 13 chapters and 1,923 texts, 2 of them duplicate ids with different bodies.
- 238 trainers.
- 39 encounter maps with 56 tables.
- 69 `ITEM_SIL_*` definitions, 260 item placements and 13 marts.
- 1,053 referenced constants and 1,075 referenced text ids.
- 37 dynamic references, 4 of them unresolved.
- The ids files hold:
  - 490 flags (from 0x113, one of them a tombstone) and 20 vars;
  - 238 trainers and 69 items;
  - 5 species (the Mega forms the source names);
  - 1 move;
  - 13 songs;
  - 115 maps.

**Check on the real import.**
- With the extract and `PKR_DECOMP`, the check finds 169 errors and 87 warnings. All 256 findings are listed in `check-exceptions.ron`, so the check exits 0. Of those entries, 249 carry `ruling_needed`. By rule:
  - `text-charmap` 108;
  - `item-incomplete` 55;
  - `sil-prefix` 29;
  - `item-name-length` 16;
  - `fishing-unreachable` 11;
  - `trainer-class-name` 10;
  - `trainer-name` 8;
  - `map-undefined` 7;
  - `dynamic-unresolved` 4;
  - `item-name-clash` 4;
  - `text-duplicate` 2;
  - `id-removed` 1;
  - `invented-move` 1.
- These rules find nothing:
  - `gen3-collision`;
  - `placement-map` on the six Ch0 maps;
  - `text-width`, even with the worst cases (the widest line is 204 px);
  - `text-lines`;
  - `speakers-mismatch`.
- Without the extract and the decomp, the check also exits 0. It reports 5 `exception-unused` warnings, because those exceptions' sources are missing: the 3 Gen3 name clashes and the 2 `WEATHER_*` names.

**Open rulings (author).**
- Glyphs, counted in texts: `—` 68, `_` 25, `[ ]` 12, `°` 15 (plus the trainer names `N°7` and `N°12`), `♥` 6 (R-11), `₽` 3, `○ ●` 2, and `♪`, `#`, `*`, `←`, `→` 1 each.
- Names over the limits:
  - 8 trainer names over 10 bytes, among them `TAUBER&HOCSEAT`, `LOULPIX & DORIS` and `CONTREMAÎTRE`;
  - 10 class names over 12 bytes;
  - 16 item names over 13 bytes, the 14-byte ones among them: `ITEM_SIL_BROCHETTE_NEON`, `ITEM_SIL_CARTE_PERFOREE` and `ITEM_SIL_PAIN_LEMBAS`.
- Sounds and music:
  - 13 invented `SE_*` and 2 invented `MUS_*` lack the SIL_ prefix;
  - `MUS_MAUVILLE`, `MUS_MOSSDEEP` and `MUS_ROUTE111` are not decomp tracks;
  - `MUS_RG_SEKAIKAN` should be `MUS_RG_NEW_GAME_INTRO`, `MUS_TEAM_AQUA_HIDEOUT` should be `MUS_AQUA_MAGMA_HIDEOUT`, and `SE_EXPLOSION` should be `SE_M_EXPLOSION`.
- Weather, heal locations and pseudo-flags:
  - `WEATHER_PETALS` and `WEATHER_CLOUDS`;
  - `HEAL_LOCATION_{PORT_RESSAC,OWLYS,BRICO_BOURG}` without the SIL_ prefix;
  - the pseudo-flags `FLAG_DAILY`, `FLAG_GEMME` and `FLAG_PREMIERE`.
- Undefined maps:
  - `MAP_ROUTE_SXX_ANTENNE`;
  - `MAP_MANGA_SQUARE_HOUSE_1` and `MAP_MANGA_SQUARE_HOUSE_2`;
  - `MAP_BRICO_BOURG_NIGHT` and `MAP_TRONE_ZENITH_NUIT`;
  - `MAP_MANGA_SQUARE_GYM`, which ch06 §4 describes without a heading constant;
  - the ch12 row that places the 8 Mega gems "in the gyms" with no `MAP_`.
- ch00 still names `FLAG_SIL_HIDE_BAR_SBIRES`. Its scenes must set `FLAG_SIL_HIDE_BAR_SBIRE_A` and `_B`, which the maps use.
- Unresolved dynamic references:
  - `<Champion>_PG_Revanche_Deja`;
  - `FLAG_SIL_ITEM_PG_GEM_LOIC … _LOULPIX`;
  - `Gym_Sc02_Q<n>_Choix`, whose multichoice lists have no ids;
  - `SPECIES_<NOM>`.
- `Tour_Sbire1_Intro` and `Tour_Sbire2_Intro` are each defined in both ch02 and ch09. The loader will reject them until one pair is renamed.
- `MOVE_SIL_CODE_AQUA`.
- The old-rod slots are never reachable.
- Item names:
  - `ITEM_SIL_POUDRE_CLAIRE` clashes with `ITEM_BRIGHT_POWDER`;
  - `ITEM_SIL_ORBE_BLEU` and `ITEM_SIL_ORBE_ROUGE` clash with the Gen3 orbs;
  - both Mewtwonites are named "Mewtwoïte X et Y".
- Mega data: 55 items have no pocket or base item, the Mega stones among them, and only 5 `SPECIES_*_MEGA` forms are named for about 30 stones.
- The AI item bags of Arthur (ch10), Crow, Benou and Julien (ch12) appear only in prose. The `items` field is empty for every trainer, so the bags must be entered by hand.

**Gates.** All pass:
- fmt;
- clippy, plain and with `game-native/window,studio/ui`;
- `cargo test --workspace` real (1,444 passed) and stub (1,432 passed). Both runs need `PKR_DECOMP`, since a worktree's relative default does not resolve;
- `silice check` with and without the extract.

#### SIL1 Ch0 maps as built (Task F, the map half)

The six Chapter 0 maps are in `games/silice/{maps,layouts}/`. They were made with the studio map editor's CLI only (`cargo xtask studio map create` and `apply`). The edit scripts that make them are committed under `games/silice/edits/<Map>/`:
- `00_create.ron`: the `NewMap` spec;
- `01_terrain.ron`: the cells;
- `02_events.ron`: the header, map scripts, warps, objects, coord and bg events, and the connection.

`games::hygiene` (now also `studio-map … hygiene`) checks `edits/` too: only `<Map>/<phase>_<what>.ron` files that parse as a create spec or an edit script (see the editor upgrades above). It was run on `games/silice` with the extract present (so including `games::copies`) and is clean.

To rebuild, run one command: `cargo xtask studio map build` (`studio-map build`, which reads `games/silice/edits` by default). It recreates the six maps from their edits in one process and writes the same bytes as the earlier create-then-apply passes; nothing needs deleting first.

Behaviour warnings (after the editor upgrades, re-checked on the full 518-map extract): `validate` on `games/silice` reports only unwritten scripts.
- Fixed in the terrain scripts:
  - the Labo door at BricoBourg (4, 7) is solid (collision 1) until Chapter 1 gives it a warp;
  - the two house doors (metatile 33) at BricoBourg (16, 6) and (5, 14) are solid, as in all 13 Emerald uses. Pushing up into a solid door warps (`door_warp`);
  - the potted plants' top cells in BricoBourg_FriendsHouse (metatile 656) are walkable, as in all 5 Emerald uses; their bottom cells stay solid.
- `03_allow.ron` in `BricoBourg` and `RouteS01` accepts the rest with reasons:
  - the canal and the pond (General 200–218) are kept solid: these are puddle metatiles, walkable in Emerald, used as decorative water (see Water below);
  - the cables (rail metatile 241) are kept solid on purpose;
  - the vegetable garden's centre soil (metatile 268) is walkable on purpose, though Emerald's uses are solid;
  - Le Shaker's roof ridge cell (14, 9) is kept solid.
- The small-tree tops allowance (metatile 14) was removed: on the full extract metatile 14 raises no warning, so it silenced nothing.

Steps 2 and 3 are separate passes because warps and connections are validated against the other maps. A rebuild gives byte-identical files.

**Names.** The plan's names and the source's `MAP_*` constants in CamelCase agree: `BricoBourg`, `BricoBourg_Bar`, `BricoBourg_PlayerHouse_1F`, `BricoBourg_PlayerHouse_2F`, `BricoBourg_FriendsHouse`, `RouteS01`. Each layout has its map's name.

**Sizes and tilesets** (Emerald tilesets by name; every layout is original):

| Map | Size | Tilesets | Border |
|---|---|---|---|
| BricoBourg | 24 × 20 (the source's suggested size) | General + Petalburg | forest 2×2 |
| BricoBourg_Bar | 14 × 10 | Building + Shop | black, blocked |
| BricoBourg_PlayerHouse_1F | 10 × 8 | Building + BrendansMaysHouse | black, blocked |
| BricoBourg_PlayerHouse_2F | 8 × 7 | Building + BrendansMaysHouse | black, blocked |
| BricoBourg_FriendsHouse | 10 × 8 | Building + GenericBuilding | black, blocked |
| RouteS01 | 20 × 40 | General + Petalburg | forest 2×2 |

The extract has no route secondary other than Petalburg (the M7 slice has 8 tilesets). Route S1 should get a route secondary once the full import has one.

**Script names (the contract for `crates/content/src/silice/scripts/ch00_*.rs`).**
- An event that *is* a scene names the scene's function as `<Map>_ScNN`, where `<Map>` is the map's CamelCase name. For example, the source's `Bar_Sc03` is `BricoBourg_Bar_Sc03`, and `Player_House_Sc01` is `BricoBourg_PlayerHouse_1F_Sc01` (the 1F is where it runs).
- Every other event script is `<Map>_EventScript_<What>`, with `<What>` taken from the source's text id (`Brico_Bourg_Npc_Vieux` becomes `BricoBourg_EventScript_Vieux`).
- Map scripts are `<Map>_OnTransition`.
- `validate` lists them all as "not in the build (yet)" until the scripts exist: 68 warnings.

**Events per map** (my coordinates; local ids follow the source's obj numbers where it gives them).

- **BricoBourg.**
  - Header: `MUS_LITTLEROOT`, `WEATHER_NONE` (the source says none), `MAP_TYPE_TOWN`. `on_transition` is `BricoBourg_OnTransition`. `on_frame` runs `BricoBourg_Sc02` while `VAR_SIL_STORY` is 3, so it fires as the player steps out of the house door (Sc02 chains into Sc03, which sets 5).
  - Warps:
    - 0: the player's house door (5, 14) to 1F warp 0;
    - 1: the Shaker's door (14, 13) to the Bar's warp 0;
    - 2: Adrien's house door (16, 6) to FriendsHouse warp 0.
  - Objects:

    | Local id | Who | Cell | Gfx | Hide flag |
    |---|---|---|---|---|
    | 1 | VIEUX | (11, 10) | MAN_5 | none |
    | 2 | LINGE | (2, 13) | WOMAN_2 | `FLAG_SIL_HIDE_BRICO_LINGE` |
    | 3 | GAMIN | (7, 11), wanders 1 | LITTLE_BOY | `FLAG_SIL_HIDE_BRICO_GAMIN` |
    | 4 | PECHEUR | (18, 6) | FISHERMAN | none |
    | 5 | BRICOLEUSE | (7, 6) | POKEFAN_F | `FLAG_SIL_HIDE_BRICO_BRICOLEUSE` |
    | 6 | SBIRE | (18, 14) | NINJA_BOY | `FLAG_SIL_HIDE_BRICO_SBIRE` |
    | 7 | MATHIEU | (4, 16) | BLACK_BELT | `FLAG_SIL_HIDE_BRICO_MATHIEU` |
    | 8 | GWADA | (5, 16) | BOY_2 | `FLAG_SIL_HIDE_BRICO_GWADA` |
    | 9 | ADRIEN | (6, 16) | SCHOOL_KID_M | `FLAG_SIL_HIDE_BRICO_ADRIEN` |

  - Coord events:
    - `BricoBourg_Sc01` on the three cells around the Shaker's exit cell (14, 14): (13, 14), (15, 14) and (14, 15), `VAR_SIL_STORY` 1. The scene checks `FLAG_SIL_BRICO_CHASE`;
    - `BricoBourg_Sc04` on (10–13, 1), value 5;
    - the north guard `BricoBourg_EventScript_BlockNorth` on (10–13, 2), keyed on a dedicated gate var: `VAR_SIL_CH0_GATE` 0. **Brico_Bourg_Sc03 must set `VAR_SIL_CH0_GATE` to 1 when the starter is chosen**; the ids import must allocate the var. Roots at x 9 and 14 close the row.
  - Signs:
    - the village sign (10, 5);
    - the Shaker sign (12, 13);
    - the Labo sign (6, 8);
    - `BricoBourg_EventScript_Ronces` on the brambles (4, 8).
  - The hidden Potion is at (10, 8), behind the fountain, with `FLAG_SIL_ITEM_BRICO_POTION`.
  - Connection: North to RouteS01, offset 3.

- **BricoBourg_Bar.**
  - Header: `MUS_SLATEPORT` (the source's own placeholder for the jazz track), indoor.
  - Map scripts:
    - `on_frame` runs `BricoBourg_Bar_Sc01` when `VAR_TEMP_1` is 0 (temp vars are 0 on every map load). **Sc01 must set `VAR_TEMP_1` to 1 on every path**, including when `VAR_SIL_STORY` is not 0 or `FLAG_SIL_INTRO_DONE` is set;
    - `on_transition` is `BricoBourg_Bar_OnTransition`.
  - Warps 0 and 1: the mat (6–7, 9) to BricoBourg's warp 1.
  - Objects:

    | Local id | Who | Cell | Gfx | Script | Hide flag |
    |---|---|---|---|---|---|
    | 1 | CROW | (3, 2), behind the counter | GENTLEMAN | `_EventScript_Crow` | none |
    | 2 | MATHIEU | (3, 4), on a stool | BLACK_BELT | `_Sc02` | `FLAG_SIL_HIDE_BAR_MATHIEU` |
    | 3 | GWADA | (6, 4) | BOY_2 | `_Sc02` | `FLAG_SIL_HIDE_BAR_GWADA` |
    | 4 | ADRIEN | (9, 4) | SCHOOL_KID_M | `_Sc02` | `FLAG_SIL_HIDE_BAR_ADRIEN` |
    | 5 | JULIEN | (12, 3) | MANIAC | `_Sc03` | `FLAG_SIL_HIDE_BAR_JULIEN` |
    | 6 | CLIENTE | (3, 7) | WOMAN_4 | `_EventScript_Cliente` | none |
    | 7 | CLIENT | (12, 6) | FAT_MAN | `_EventScript_Client` | none |
    | 8 | SBIRE_A | (6, 9) | NINJA_BOY | none | `FLAG_SIL_HIDE_BAR_SBIRE_A` |
    | 9 | MALLETTE | (13, 3), on Julien's table (a blocked cell) | BIRCHS_BAG | `_Sc04` | `FLAG_SIL_HIDE_BAR_MALLETTE` |
    | 10 | SBIRE_B | (7, 9) | NINJA_BOY | none | `FLAG_SIL_HIDE_BAR_SBIRE_B` |

  - The back door is drawn at (12, 1) with no warp (the source calls it inactive for the player).

- **BricoBourg_PlayerHouse_1F.**
  - Header: `MUS_LITTLEROOT`, indoor.
  - Warps:
    - 0 and 1: the mat (4–5, 7) to BricoBourg's warp 0;
    - 2: the stairs (8, 1) to 2F warp 0.
  - Object 1: MAMAN (MOM) at (2, 2) in the kitchen, script `_EventScript_Maman` (Player_House_Sc03).
  - Map script: `on_frame` runs `BricoBourg_PlayerHouse_1F_Sc01` while `VAR_SIL_STORY` is 1 (the scene sets 2). A coord event could be walked around.
  - Sign: the TV (6, 2).

- **BricoBourg_PlayerHouse_2F.**
  - Warp 0: the stairs (7, 1) to 1F warp 2.
  - Signs:
    - the desk (5, 2), `_EventScript_Bureau`, and the bed (1, 2) and (1, 3), `_EventScript_Lit`: Player_House_Sc02's triggers A and B;
    - the PC (6, 2).
    - The desk (BrendansMaysHouse 590) and PC (601) are normal-behaviour metatiles, so only the bg events act on them (the tileset's PC and notebook metatiles have their own behaviours);
    - the window (3, 1).

- **BricoBourg_FriendsHouse.**
  - Warps 0 and 1: the mat (4–5, 7) to BricoBourg's warp 2.
  - Objects:
    - 1: the mother (WOMAN_5) at the stove (2, 2);
    - 2: the Skitty at (7, 5), asleep on a cushion.
  - Sign: the film poster (6, 1).

- **RouteS01.**
  - Header: `MUS_ROUTE101`, `WEATHER_SUNNY`, `MAP_TYPE_ROUTE`.
  - Objects:

    | Local id | Who | Cell | Gfx | Trainer |
    |---|---|---|---|---|
    | 1 | PROMENEUSE | (6, 32) | WOMAN_5 | |
    | 2 | TECHNICIEN | (9, 3) | SCIENTIST_1 | |
    | 3 | TEO | (3, 24), faces right | YOUNGSTER | sight 3 |
    | 4 | NOAH | (12, 18), faces down | BUG_CATCHER | sight 3 |
    | 5 | LILA | (3, 12), faces right | LASS | sight 3 |
    | 6 | YANIS | (12, 4), faces left | BOY_1 | sight 4; can be passed on the east, x 13–15 |
    | 7 | ITEM_POKE_BALL | (14, 6) | item ball | flag `FLAG_SIL_ITEM_ROUTE_S01_POKE_BALL` |
    | 8 | ITEM_POTION | (14, 22) | item ball | flag `FLAG_SIL_ITEM_ROUTE_S01_POTION` |

  - Coord events:
    - `RouteS01_Sc01` on (8–13, 30), value 5;
    - `RouteS01_Sc02` on (7–10, 2), value 6.
  - Signs: entrance (4, 34), grass (16, 28), trainers (14, 16), cables (6, 3).
  - The hidden Antidote is at (4, 22), at the foot of a root, with `FLAG_SIL_ITEM_ROUTE_S01_ANTIDOTE`.
  - Connection: South to BricoBourg, offset −3.
  - Ledge (jump south, General 213/214 with collision 1 as in Emerald) on x 2–13 at y 10; the way up is x 14–17. The tutorial grass is x 8–13, y 26–30. The pond is x 4–8, y 16–18.

**Coordinate changes from the ASCII plans (§4 item 12).**
- **BricoBourg** is drawn at its suggested 24 × 20, not the plan's 25 × 14.
  - Buildings: the Labo is at x 3–6, y 2–7, with its door at (4, 7) (source (5, 3)) and the brambles at (4, 8) (source (5, 4)). Adrien's house moved to the north-east, door (16, 6) (source (6, 7)). The Shaker's door is (14, 13) (source (15, 10)). The player's door is (5, 14) (source (6, 13), on the border row).
  - Coord events and NPCs: Sc01's cells surround (14, 14) (source (15, 11)); Sc02 is an on-frame script instead of the source's coord at (6, 14). The friends stand at (4–6, 16). Linge is at (2, 13) (source (4, 12)), Gamin at (7, 11) (source (9, 8)), Bricoleuse at (7, 6) (source (8, 3)), the sbire starts at (18, 14) (source (11, 9)), and the village sign is at (10, 5) (source P1 near (4, 8)). The old man is at (11, 10) (source (13, 9)). The fountain is at (9–10, 9–10) (source (12, 9)).
  - The canal is at x 19–20, y 2–17 (source y 3–18).
  - The hidden Potion is at (10, 8) (source (13, 8), still behind the fountain).
  - The north exit stays at x 10–13.
- **The Bar** keeps its 14 × 10.
  - The two-row back wall puts the counter at y 3 (source y 1); Crow stays at (3, 2).
  - The friends sit on stools at y 4 (source y 3). Julien keeps (12, 3), and the Mallette lies on his table at (13, 3) (source (12, 4)).
  - The back door is (12, 1) (source (13, 1)). The clients are at (3, 7) and (12, 6) (source (3, 6) and (10, 6)). The mat stays at (6–7, 9).
  - The scenes' walking paths (Bar_Sc03's sbires and Julien, Brico_Bourg_Sc01's sbire, Brico_Bourg_Sc03's friends) must be re-derived in Rust. For Sc01, the sbire at (18, 14) walks left 2, then up 6 along x 16, which is kept free.
- **1F** keeps its size, stairs (8, 1) and door (4, 7); the source's coord (4, 6) became an on-frame script. **2F** keeps bed (1, 2), desk (5, 2) and PC (6, 2).
- **RouteS01** keeps the source's coordinates for every trainer, item, sign, NPC and both exits.
  - The ledge spans x 2–13 rather than the plan's 4–15.
  - Trees close x 2–7 and 14–17 at y 30–31, so `Route_S01_Sc01`'s row is the only way north.

**Deviations and placeholders.**
- **Music:** every Ch0 track the maps name is a valid decomp `MUS_*`, so no music placeholder was needed. "Slow" or night versions cannot be expressed in a header.
- **Map section:** `MAPSEC_NONE` everywhere, with `show_map_name: false` so no empty popup appears, until `MAPSEC_SIL_*` exist.
- **Graphics** are limited to the extract's 46 sprites. Placeholders:

  | Character | Placeholder gfx |
  |---|---|
  | Crow | GENTLEMAN |
  | Mathieu | BLACK_BELT |
  | Gwada | BOY_2 |
  | Adrien | SCHOOL_KID_M |
  | Julien | MANIAC |
  | Team Cocket sbires | NINJA_BOY |
  | The Mallette | BIRCHS_BAG |
  | Skitty | ZIGZAGOON_1 |
  | Campeur Yanis | BOY_1 |
  | Old man | MAN_5 |

- **Brambles:** a bush metatile (Petalburg 514) with collision and a bg script. The source wants a tile obstacle cut with `set_metatile`, not a cuttable-tree object.
- **Cables:** horizontal rail metatiles, as a placeholder.
- **Water:** the Brico-Bourg canal and the Route S1 pond use General's 200–218, which have the `PUDDLE` behaviour (walkable splash), blocked by collision 1. Ch1's Surf and fishing will need real water metatiles (`POND_WATER`/`OCEAN_WATER`) there.
- **Hide flags:** visibility by `VAR_SIL_STORY` uses one hide flag per object, set by the `_OnTransition` scripts. Each object gets its own flag, so the validator's shared-flag warning does not fire. The source's single `FLAG_SIL_HIDE_BAR_SBIRES` is split into `_SBIRE_A` and `_SBIRE_B`. Every `FLAG_SIL_HIDE_*` flag here is new (the source has only `FLAG_SIL_HIDE_BAR_SBIRES`): the ids import must allocate them. New-game init should set the sbires' flags, `FLAG_SIL_HIDE_BAR_MALLETTE` and `FLAG_SIL_HIDE_BRICO_{SBIRE,MATHIEU,GWADA,ADRIEN}`.
- **Coord events match one value**, so two source conditions are reshaped:
  - `Route_S01_Sc02` ("≤ 8") fires on 6. Sc01's row, which sets 6, is the only way to y 2.
  - The Brico-Bourg guard ("< 5") is keyed on `VAR_SIL_CH0_GATE` 0, which Brico_Bourg_Sc03 sets to 1 with the starter, so it covers every story value before the starter, including leaving the bar at 0.
- **Coord events do not fire on a door's scripted exit step** (it skips the step-finished check). Scenes that start as the player leaves a door are on-frame scripts (Brico_Bourg_Sc02, Player_House_Sc01) or coord events on the cells around the exit cell (Brico_Bourg_Sc01).
- **On-frame scripts must change their own var.** Gen3 re-runs one while its var matches: Bar_Sc01 is keyed on `VAR_TEMP_1` and sets it to 1; Brico_Bourg_Sc02 (via Sc03) and Player_House_Sc01 move `VAR_SIL_STORY` on.
- **Exits to unbuilt maps are left out:**
  - the Labo door warp (`MAP_BRICO_BOURG_LABO`, Ch1);
  - Route S1's north connection (`MAP_BOIS_ETHER`, Ch1);
  - the Bar's back door.
- **Not in map files:** encounters (`battle/encounters.ron`), the player's start cell (`game.ron`; the source's (7, 8) is inside my bar's lounge), and the night tint (fallback: nothing).

**Editor friction (for the remaining ~109 maps).**
1. **Palette.** The palette has no metatile numbers. I cropped it and labeled each cell with its id using a throwaway script. Suggest `palette --labels` and a zoom.
2. **Metatile behaviors** (door, tall grass, ledge, water, warp mat) are not shown anywhere. I read them from the extract's tileset attributes. Suggest `palette --behaviors`, or a behavior overlay.
3. **Collision is per cell, not per metatile.** Every stamp must set it, and a wall left walkable is silent. Suggest a warning for walls and trees with collision 0 (by behavior or by majority use in the extract), or a default collision per metatile.
4. **No stamp or prefab op.** A building is an `Ascii` op with throwaway letters. A `Stamp { x, y, rows: [[u16]], collision }` op and a shared prefab file per tileset pair (house, lab, fountain, tree block…) would make the scripts readable and reusable. I generated the scripts with a small scratch helper.
5. **Ascii rows are RON strings,** so `"` cannot be a legend character. This needs a doc note.
6. **Warps and connections are validated against the other maps,** so a set of maps that link to each other needs the create, then terrain, then events passes. The first pass prints errors that the later passes fix. A batch command (`build <edits dir>`) would hide this.
7. **Renders show no events.** Objects, warps and triggers cannot be checked by eye. Suggest `render --events`.
8. **No CLI command for `games::hygiene` or `copies`.** I ran them with a throwaway test. Suggest adding them to `validate` or as their own command.
9. **Each CLI call is a `cargo run`.** A full rebuild is about 25 calls and about 40 s. Batching would help.
10. **The extract's sprite list is the M7 slice's 46.** Many Silice NPCs need placeholders (no camper, old man, Skitty or grunt). Revisit them once the full import exports every sprite.
11. **macOS paths are case-insensitive:** maps `T1` and `t1` collide with a confusing "names the map" error.

**Time.**
- The whole task took about 3 h. About 1.5 h went to learning four tileset pairs (palettes, behaviors, test stamps) and setting up the tooling.
- Once a pair was known, a map took about 10–15 min for an interior, 30 min for the route and 45 min for the town, including the events and the visual check.
- For the remaining maps: about 20 min per interior and 45 min per town, route or dungeon. Add about 30–45 min the first time each new tileset pair is used.
- That is roughly 50–60 h for ~109 maps, or about half that with stamp ops, prefabs and labeled palettes.

#### SIL2 Ch1 maps as built (Bois-Éther)

The six Chapter 1 maps are in `games/silice/{maps,layouts}/`, made only with the studio map editor. Their edit scripts are in `games/silice/edits/<Map>/` (`00_create`, `01_terrain`, `02_events`, plus `03_allow` for Bois-Éther). Two Ch0 maps gain a phase-4 script each:
- `BricoBourg/04_ch1_labo.ron` adds warp 3 on the Labo door (4, 7) and the `BricoBourg_OnLoad` map script;
- `RouteS01/04_ch1_connect.ron` adds the north connection to Bois-Éther.

`cargo xtask studio map build` rebuilds all 12 maps and writes the same bytes (checked with everything staged: no diff).

**Names.** The chapter's `MAP_*` constants are `MAP_BOIS_ETHER`, `_GYM`, `_HERBORISTE`, `_MART` and `_POKEMON_CENTER`, plus `MAP_BRICO_BOURG_LABO`. Its other names are `MAP_BRICO_BOURG` and `MAP_ROUTE_S01` (Ch0) and `MAP_ROUTE_S02` (Ch2). None is a fallback or a placeholder, so no name was left without a map. The map names are the ones `ids/maps.ron` already gives (`BoisEther`, `BoisEther_Gym`, `BoisEther_Herboriste`, `BoisEther_Mart`, `BoisEther_PokemonCenter`, `BricoBourg_Labo`). Each layout has its map's name. The §1.2 row says "4 interiors": these are the Center, the Mart, the Herboriste and the Labo.

**Sizes and tilesets** (Emerald tilesets by name; every layout is original, and `hygiene`, which includes `games::copies` with the full extract, is clean):

| Map | Size | Tilesets | Why this pair |
|---|---|---|---|
| BoisEther | 36 × 26 (the source's size) | General + Fortree | Fortree is Emerald's forest town: treehouses on stilts, ladders, dense canopy and big trees match "houses in the trunks or on stilts between the roots". General adds the Center, the Mart, tall grass and the rail used as cables. Route 119/120's pair is the same. |
| BoisEther_Gym | 15 × 40 (the source's size) | Building + PetalburgGym | Petalburg Gym is Emerald's gym of rooms joined by sliding doors, which is what the airlocks need. Its dais serves as Loïc's spot and the workbench, its green mats as the greenhouse beds, and its window wall as the glass dome. Fortree Gym has the greenery but no door. |
| BoisEther_Herboriste | 8 × 7 (the source's size) | Building + PrettyPetalFlowerShop | Emerald's only plant shop: wooden counter, potted plants, flower beds. |
| BoisEther_PokemonCenter | 12 × 8 | Building + PokemonCenter | standard |
| BoisEther_Mart | 10 × 8 | Building + Shop | standard |
| BricoBourg_Labo | 14 × 12 (the source's size) | Building + Lab | an old lab: jar shelves, desks, machines |

Borders: Bois-Éther uses Fortree's forest (198/199); the interiors use black, blocked (1025), as in Ch0.

**Warnings and allowances.**
- `validate` reports no errors. Its only warnings are the 194 unwritten scripts: 125 come from the Ch1 maps (Bois-Éther 65, Gym 43, Labo 11, Center 3, Mart 2, Herboriste 1), 69 from Ch0 (68 plus `BricoBourg_OnLoad`). `registry::ALLOW_UNWRITTEN` drops them at build time.
- `BoisEther/03_allow.ron` keeps two kinds of solid cell on purpose:
  - the Team Cocket cables (General's rail metatile 241, as on Route S1);
  - the doors of the two decorative treehouses, (23, 7) and (30, 4), which have no interior.
- Fixed instead of allowed: the Mart's roof ridge (General 40–43) is walkable, as in Emerald.
- The Gym raises no behaviour warning. Its workbench (the dais pieces with collision 1) and its closed doors are solid on purpose.
- `cargo xtask silice check` passes. Every flag, var and item the maps name exists in `ids/` or in Gen3; the ten hide flags were allocated for these maps (see the hide flags below).

**Events per map** (my coordinates; changes from the source are listed after).

- **BoisEther.**
  - Header: `MUS_FORTREE`, `MAP_TYPE_TOWN`, `WEATHER_NONE` (the red sky is scripted).
  - Map scripts:
    - `on_load` is `BoisEther_OnLoad`. It repaints the cut brambles when `FLAG_SIL_BOIS_ETHER_RONCES_CUT` is set, and the open barrier when `VAR_SIL_STORY` ≥ 17;
    - `on_transition` is `BoisEther_OnTransition`. It sets the hide flags, the red sky and the music.
  - Warps:
    - 0: the gym (17, 4);
    - 1: the Center (27, 12);
    - 2: the Mart (27, 17);
    - 3: the Herboriste (5, 12).
  - Objects:

    | Local id | Who | Cell | Gfx | Script | Hide flag |
    |---|---|---|---|---|---|
    | 1 | JARDINIERE | (10, 7) | WOMAN_2 | `_EventScript_Jardiniere` | |
    | 2 | VIEUX | (14, 12) | OLD_MAN | `_EventScript_Vieux` | |
    | 3 | FILLETTE | (20, 7), wanders 1 | LITTLE_GIRL | `_EventScript_Fillette` | |
    | 4 | MARCHAND | (6, 14) | POKEFAN_M | `_EventScript_Marchand` | |
    | 5 | FAN | (16, 6) | BOY_3 | `_EventScript_Fan` | |
    | 6 | TECHNICIEN | (24, 3), faces his cable | SCIENTIST_1 | `_EventScript_Technicien` | `_TECHNICIEN` (VAR ≥ 16) |
    | 7 | TOUX | (22, 14) | WOMAN_4 | `_EventScript_Toux` | |
    | 8 | GARDIEN | (19, 12) | EXPERT_M | `BoisEther_Sc04` | |
    | 9 | GARDE | (32, 13) | MAGMA_MEMBER_M | `BoisEther_Sc07` | `_GARDE` (VAR ≥ 17) |
    | 10 | ADRIEN | (17, 12) | SCHOOL_KID_M | none (Sc02) | `_ADRIEN` (outside VAR 10–11) |
    | 11 | GOGO | (15, 10) | MAXIE | none | `_GOGO` |
    | 12 | ROMAIN | (19, 10) | ARCHIE | none | `_ROMAIN` |
    | 13 | SBIRE_1 | (15, 11) | MAGMA_MEMBER_M | none | `_SBIRE_1` |
    | 14 | SBIRE_2 | (19, 11) | MAGMA_MEMBER_F | none | `_SBIRE_2` |
    | 15 | MATHIEU | (22, 11) | BLACK_BELT | none | `_MATHIEU` |
    | 16 | ITEM_X_DEFEND | (28, 8), behind the Center | item ball | `_EventScript_ItemXDefend` | `FLAG_SIL_ITEM_BOIS_ETHER_X_DEFEND` |

    The hide flags are `FLAG_SIL_HIDE_BOIS_ETHER_<WHO>`. Objects 11–15 are Sc06's actors, hidden outside the scene. The source's obj 16 (reserved) is not drawn, so the item ball takes local id 16.
  - Coord events:
    - `BoisEther_Sc01` on (12–15, 24), `VAR_SIL_STORY` 9;
    - `BoisEther_Sc02` on the whole row y 13 (x 2–31), value 10;
    - `BoisEther_Sc06` on (13–21, 6), value 15;
    - `BoisEther_Sc07` on (32, 12–15), value 16.
  - Signs:
    - the village sign (11, 20);
    - the arena sign (12, 5);
    - the oak's trunk (17, 10);
    - the barrier sign (30, 11);
    - the cable sign (22, 2);
    - `BoisEther_Sc08` on the brambles (8, 6).
  - Hidden items:
    - the Super Potion in the market's high-tech pot (4, 14);
    - the Évoluroc (`ITEM_SIL_EVOLUROC`) in the secret grove (4, 7).
  - Connection: South to RouteS01, offset 5 (our x 12–15 meet its x 7–10). RouteS01 mirrors it: North, offset −5.
  - Terrain:
    - The Grass-Hopper is a 9-wide treehouse (x 13–21, y 1–5) whose deck is at ground level.
    - Two decorative treehouses stand on stilts (x 21–25, y 5–10, and x 28–32, y 2–7).
    - The Herboriste is a ground-level treehouse (x 3–7, y 10–13).
    - The Grand Chêne is Fortree's big tree (x 16–18, y 8–10).
    - The Center is at x 26–29, y 9–12, and the Mart at x 26–29, y 14–17.
    - The secret grove is a hedge (x 3–8, y 5–8) closed by the brambles (8, 6).
    - Tall grass: x 3–7, y 1–4, and x 2–7, y 15–20.
    - The east corridor (y 12–15) has the barrier (33, 13–14) between trees at (33, 12) and (33, 15). Beyond it, x 34 is walkable and x 35 is forest.
- **BoisEther_Gym.**
  - Header: `MUS_GYM`, indoor.
  - Map scripts:
    - `on_load` is `BoisEther_Gym_OnLoad`. Once `FLAG_SIL_GYM1_VALVE` is set, it opens both airlock doors, (7, 8–9) and (7, 11–12). Without the second one, a player who reloads the map in the greenhouse would be locked in;
    - `on_transition` is `BoisEther_Gym_OnTransition`. It hides Loïc (`FLAG_SIL_HIDE_GYM1_LOIC`) once `FLAG_SIL_BADGE_01` is set.
  - Warps 0–1: the mat (6–7, 39) to Bois-Éther's warp 0.
  - Zones, bottom to top:
    - the hall, y 31–39;
    - zone 1, x 3–11 inside its walls, y 22–30;
    - zone 2, x 1–13, y 11–20;
    - zone 3, the chamber (6–8, 10);
    - zone 4, the greenhouse, y 0–9.
  - Objects:
    1. the guide (MAN_2) at (5, 35);
    2. Iris (PICNICKER) at (6, 23), facing right, sight 3;
    3. Jules (CAMPER) at (11, 26), facing left, sight 3;
    4. Milo (HIKER) at (8, 17), facing left, sight 4;
    5. Rose (LASS) at (11, 13), facing down, sight 4;
    6. Paul (YOUNGSTER) at (11, 18);
    7. crate c1's Super Potion, an item object at (2, 15) (`FLAG_SIL_ITEM_GYM1_SUPER_POTION`);
    8. Loïc (WALLY) at (7, 2) on the dais, script `BoisEther_Gym_Sc10`. The source calls him obj 20, but the `Object` op cannot choose a local id, so he is 8; scripts use `LOCALID_GYM1_LOIC`.
  - Coord events:
    - `BoisEther_Gym_Sc01` on (6–7, 38), `VAR_SIL_STORY` 12;
    - the shower on (7, 30) and (7, 29), and the 20 spore grates, all keyed on `VAR_TEMP_1` 0, so they fire on every step. Their scripts test `FLAG_SIL_GYM1_CONTAMINE`;
    - the zone-3 scene (`_EventScript_Sas2`) on (7, 10), keyed on `VAR_TEMP_2` 0. It must set `VAR_TEMP_2` to 1, so it plays once per visit.
  - Signs:
    - airlock door D (8, 22), any facing, so it also opens from above;
    - the poster (4, 22);
    - the workbench (5–7, 14);
    - crates c2 (4, 15, the fuse) and c3 (6, 15, the spring);
    - the private greenhouse door (12, 1).
  - The grates and the walls follow the source's grid exactly (origin (2, 22)), so the safe path and both forced battles are unchanged. A breadth-first search over the layout confirms it: with the grates solid, every path from the shower (7, 30) to the door's front cell (8, 23) crosses Jules's and Iris's lines of sight.
- **BoisEther_Herboriste.** Header `MUS_FORTREE`, indoor. Warps 0–1: the mat (3–4, 6) to Bois-Éther's warp 3. Object 1: the herbalist (MAN_4) behind the counter at (0, 4), script `BoisEther_Herboriste_Sc05` (the source's Bois_Ether_Sc05 runs here). The player talks across the counter (x 1, y 3–5).
- **BoisEther_PokemonCenter.** Header `MUS_POKE_CENTER`. `on_transition` is `BoisEther_PokemonCenter_OnTransition`, which sets the respawn point. Warps 0–1: the mat (5–6, 7) to warp 1. Objects:
  1. the nurse at (6, 2);
  2. the trainer (YOUNGSTER) at (2, 5).
- **BoisEther_Mart.** Header `MUS_POKE_MART`. Warps 0–1: the mat (4–5, 7) to warp 2. Objects:
  1. the clerk at (1, 3), behind the counter;
  2. the client (MAN_1) at (4, 5).
- **BricoBourg_Labo.**
  - Header: `MUS_AQUA_MAGMA_HIDEOUT` (the source's `MUS_TEAM_AQUA_HIDEOUT` does not exist).
  - `on_transition` is `BricoBourg_Labo_OnTransition`. It hides the Chimiste (`FLAG_SIL_HIDE_LABO_CHIMISTE`) once `VAR_SIL_BRICO_LABO_STATE` is 2.
  - Warps 0–1: the mat (6–7, 11) to BricoBourg's warp 3.
  - Objects:
    1. the Chimiste (SCIENTIST_2) at (2, 1). He is a talked-to boss, not a sight trainer: his script battles;
    2. sbire 2 (MAGMA_MEMBER_F) at (1, 3), facing right, sight 4;
    3. sbire 1 (MAGMA_MEMBER_M) at (1, 8), facing right, sight 5;
    4. the TM chest (12, 3), an item ball;
    5. the Silice dust on a bench (6, 7), an item ball;
    6. the Revive (9, 9), an item ball.
  - Coord events: `BricoBourg_Labo_Sc01` on (6–7, 10), `VAR_SIL_BRICO_LABO_STATE` 0.
  - Signs: the memo on the desk at (11, 2) and (12, 2).
  - The crate cordon (x 2–11, y 4) leaves (1, 4) and (12, 4) open, as the source says.
- **BricoBourg** (Ch0, edited):
  - warp 3 on the Labo door (4, 7). The door stays solid: it is an animated door, and pushing into it warps;
  - `on_load` `BricoBourg_OnLoad` repaints the cut brambles (4, 8) when `FLAG_SIL_BRICO_LABO_RONCES_CUT` is set. The `Scripts` op restates Ch0's `on_transition` and `on_frame`.

**Hide flags.** Each scene actor has its own persistent hide flag, as in Ch0: `FLAG_SIL_HIDE_BOIS_ETHER_{TECHNICIEN, GARDE, ADRIEN, GOGO, ROMAIN, SBIRE_1, SBIRE_2, MATHIEU}`, `FLAG_SIL_HIDE_GYM1_LOIC` and `FLAG_SIL_HIDE_LABO_CHIMISTE`. The first build used Gen3's temporary `FLAG_TEMP_11`–`18`, because the ids were owned by another task.
- They were allocated by `cargo xtask silice import … --force` from `maps/*.ron` (flags 765–774) and appended to `xtask/tests/fixtures/silice/ids.golden`. Only `ids/flags.ron` and `IMPORT-REPORT.md` were taken from the import: it would also undo Task A's provisional `Tour_Sbire{1,2}_Intro_Ch09` rename.
- Each `_OnTransition` owns its map's flags, as Ch0's do. It sets them from `VAR_SIL_STORY`, `FLAG_SIL_BADGE_01` and `VAR_SIL_BRICO_LABO_STATE` before the objects spawn. A scene that shows or hides an actor (`show`/`hide`) also sets its flag.
- `game.ron`'s `new_game_flags` sets those hidden at story 0: Adrien (`_ADRIEN`) and Sc06's five actors. The technician, the guard, Loïc and the Chimiste start visible. `game_tests.rs` checks both groups.

No new flags or vars are otherwise needed.

**Metatiles for `set_metatile`** (the source's `METATILE_*` names; each also restores collision 0):
- `METATILE_RONCES_COUPEES`: Bois-Éther (8, 6) becomes General 1, grass. Brico-Bourg (4, 8) becomes General 1.
- `METATILE_BARRIERE_OUVERTE`: (33, 13) and (33, 14) become Fortree 609, the path.
- `METATILE_PORTE_SAS_OUVERTE`: the gym's airlock doors become PetalburgGym 513, the floor:
  - zone 1, (8, 22);
  - zone 2, (7, 11) and (7, 12);
  - zone 3, (7, 8) and (7, 9).

  Each airlock door is two cells tall, so every script that opens or closes one must set both cells. The source's single cells do not suffice:
  - the workbench's `set_metatile(7, 12)` must also open (7, 11);
  - the zone-3 scene's `door(7, 11, false)` must also close (7, 12);
  - its `door(7, 9, true)` must also open (7, 8). Otherwise (7, 8) stays solid and the greenhouse cannot be reached.

**Script names (the contract for `crates/content/src/silice/scripts/ch01_*.rs`)**, following the Ch0 rules (`<Map>_ScNN`, `<Map>_EventScript_<What>`, `<Map>_OnTransition`/`_OnLoad`):
- BoisEther:
  - scenes: `BoisEther_Sc01`, `_Sc02` (which calls Sc03), `_Sc04`, `_Sc06`, `_Sc07`, `_Sc08`;
  - map scripts: `_OnLoad`, `_OnTransition`;
  - NPCs: `_EventScript_{Jardiniere, Vieux, Fillette, Marchand, Fan, Technicien, Toux}`;
  - item: `_EventScript_ItemXDefend`;
  - signs: `_EventScript_{SignVille, SignArene, SignChene, SignBarriere, SignCable}`.
- BoisEther_Gym:
  - scenes: `BoisEther_Gym_Sc01`, `_Sc10` (which calls Sc11 and Sc12);
  - map scripts: `_OnLoad`, `_OnTransition`;
  - `_EventScript_{Conseiller, Iris, Jules, Milo, Rose, Paul, CaissePotion, Douche, Spores, Sas2, PorteSas1, Affiche, Etabli, CaisseFusible, CaisseRessort, SerrePrivee}`.
- BoisEther_Herboriste: `BoisEther_Herboriste_Sc05`.
- BoisEther_PokemonCenter: `_OnTransition`, `_EventScript_Nurse`, `_EventScript_Dresseur`.
- BoisEther_Mart: `_EventScript_Clerk`, `_EventScript_Client`.
- BricoBourg_Labo: `BricoBourg_Labo_Sc01`, `_OnTransition`, `_EventScript_{Chimiste, Sbire1, Sbire2, ItemTm, ItemSilice, ItemRevive, Memo}`.
- BricoBourg: `BricoBourg_OnLoad` (new).

**Link to Chapter 2 (made with the Ch2 maps).** `games/silice/edits/BoisEther/05_ch2_connect.ron` opens the east edge (35, 12–15) with Fortree 609 (collision 0) and connects East to RouteS02 with offset 10. RouteS02's west entrance is (0, 2–5), so Bois-Éther's y 12–15 meets the route's y 2–5; the route mirrors it (`West`, offset −10). See "SIL2 Ch2–Ch3 maps as built".

`BoisEther_Sc07` and `_OnLoad` open the barrier at (33, 13–14). The connection itself needs no script.

**Coordinate changes from the source.**
- **Bois-Éther.**
  - The source's ASCII stops at y 20 for a 36 × 26 map. Rows 21–25 are forest with the south path (x 12–15), so Sc01's row moved from y 19 to y 24, one row inside the edge.
  - The Mart's door is (27, 17), not (27, 16): a 4-row Mart under the Center's door would block it.
  - The X Defend is at (28, 8), behind the Center (the source's (28, 9) is inside its roof).
  - Sc06 fires on the whole row y 6 (x 13–21) rather than on (17, 7). Every step off the gym's deck crosses that row, and the Grand Chêne fills y 8–10 in front of the door.
  - Sc02 fires on the whole row y 13 (x 2–31) rather than on (16–18, 13). The clearing could be walked around, which skipped Sc02, Sc03 and their story steps. The gym, the Center's and the Herboriste's doors are all north of that row. The scene walks the player to (17, 13) before Adrien turns.
  - The second treehouse's stamp keeps the gym deck's right end at (21, 5).
  - The fan stands at (16, 6), off the deck.
  - The east ponds and the clearing's pond are left out: the extract has no small pond that fits. Flower beds and tall grass take their place.
  - The cables are at x 9–12, y 1, and x 22–26, y 2.
  - Sc06's actors start at (15, 10), (19, 10), (15, 11), (19, 11) and (22, 11). Its walking paths (and Sc03's Adrien exit) must be re-derived in Rust.
- **Gym.**
  - Zone 1's door D (8, 22) and zone 2's opening (7, 20) are joined by a two-cell corridor (7–8, 21).
  - The 2-row wall between zones 2 and 3 holds one door: (7, 11) is its top and (7, 12), P, its bottom.
  - Zone 3's front door is (7, 8–9).
  - The guide is at (5, 35).
  - The workbench spans x 4–8, y 13–14 (the source draws it as one row).
  - Loïc's exit (`Walk(Up) x2, Walk(Right) x3` from (7, 2)) walks into the back wall. It must be re-derived in Rust, towards the private greenhouse door (12, 1).
- **Labo.**
  - The memo is read on the desk at (11–12, 2): the source's (12, 1) has no free neighbour once the desk is drawn.
  - Brico-Bourg's Labo door keeps Ch0's (4, 7) (the source's (5, 3)).

**Placeholders.**
- Graphics (the full extract now has every sprite):

  | Character | Placeholder gfx |
  |---|---|
  | Gogo | MAXIE |
  | Romain | ARCHIE |
  | Team Cocket (guard, sbires) | MAGMA_MEMBER_M/F |
  | Loïc | WALLY |
  | Botanistes (Iris, Rose) | PICNICKER, LASS |
  | Herbalist | MAN_4 |
  | Chimiste | SCIENTIST_2 |

  Ch0's sbires (NINJA_BOY) could switch to MAGMA_MEMBER_M too.
- Crates are Building's screen metatiles. The cables are rails (as in Ch0). The barrier is General's fence (6).
- Strobes and a mirror ball cannot be drawn. The Labo's music is Emerald's hideout track.

**Editor friction.**
1. **No reference render of an Emerald map.** To learn Fortree's and the interiors' metatiles, I copied extracted layouts into a scratch games dir and rendered them, plus a metatile-id grid dump. Suggest `studio-map render --extracted <EmeraldMap>` and a `grid <Map>` command that prints ids and collision.
2. **`palette --labels` at scale 2** hides small tiles under the labels, and a full pair is too tall to read. Suggest `--range 512..640`, which also saves cropping by hand. On macOS, `sips` crops from the centre when given offset 0.
3. **`Object` cannot choose a local id.** Loïc is 8, not the source's 20. Suggest `Object(local_id: 20, …)`.
4. **`Scripts` replaces the whole map-scripts record.** Adding `on_load` to Brico-Bourg meant restating Ch0's `on_transition` and `on_frame`. Suggest field-level ops (`OnLoad("…")`).
5. **`render --collision` still marks allowed cells with `!`.** It could grey them out.
6. **Building's metatile 1 is black only with some secondaries.** With Lab it draws as floor; I used Lab 513.
7. **In the agent worktree, shell loops and variables in a command are refused.** Renders and palettes went through small helper scripts.
8. **Ids were owned by another task.** New hide flags could not be added at first, hence the temporary `FLAG_TEMP_*` design (replaced since; see the hide flags). A shared "pending ids" file the ids import picks up would help.

**Time.** About 2.5 h:
- 1 h learning four new tileset pairs (Fortree, PokemonCenter/Shop, PrettyPetalFlowerShop, Lab and PetalburgGym) through reference renders;
- 1 h drafting the six maps;
- 0.5 h events, gates and this write-up.

#### SIL2 Ch2–Ch3 maps as built

The 19 maps of Chapter 2 (Gris-Échec) and Chapter 3 (Viber-City) are in `games/silice/{maps,layouts}/`. As in Ch0, they were made with the studio map editor only. Their edit scripts are under `games/silice/edits/<Map>/` (`00_create`, `01_terrain`, `02_events`, and `03_allow` where needed), and the building stamps are under `games/silice/stamps/`.

`cargo xtask studio map build` rebuilds all 31 Silice maps (Ch0 and Ch1 included). A second build gives byte-identical files. `validate` reports unwritten scripts and one accepted warning: ViberCity's (28, 6) holds two coord events, Sc03 at `VAR_SIL_STORY` 32 and Sc06 at 35. Gen3 (`GetCoordEventScriptAtPosition`) and the engine (`coord_trigger`) run the first event on the cell whose var matches, so the two never compete; the validator cannot be told so. `hygiene` is clean, run with the full extract present, so `games::copies` is included.

**Maps.** 9 for Ch2 and 10 for Ch3. §1.2 counts 11 and 12 because it also counts referenced maps:
- Ch2 references `BOIS_ETHER` (Ch1) and `ROUTE_S03`;
- Ch3 references `GRIS_ECHEC` and `ROUTE_S04` (Ch4).

The placeholder names `MAP_ROUTE_S02_ANTENNE` and `MAP_ROUTE_S03_ANTENNE` (ch12's `MAP_ROUTE_S0x_ANTENNE` expansion) have no map.

Every tileset pair is an Emerald pair by name, and every layout is original. Buildings are stamps of small Emerald pieces (each under 10 × 10), placed in new arrangements.

| Map | Size | Tilesets | Why this pair |
|---|---|---|---|
| RouteS02 | 56 × 22 | General + Mauville | the Route 110 coast: cliff, still sea; Mauville's logs stand in for the containment cables |
| GrisEchec | 40 × 32 | General + Rustboro | gray concrete blocks, paving, iron fences |
| GrisEchec_PokemonCenter | 13 × 9 | Building + PokemonCenter | standard Centre (counter between the two corners, no escalator) |
| GrisEchec_Mart | 12 × 8 | Building + Shop | standard Mart; the fridges are the source's drinks machine |
| GrisEchec_TourObservation | 12 × 10 | General + Facility | the Space Centre set: window wall, screens, railing, stairs as the lift |
| GrisEchec_TourObservation_Top | 10 × 8 | General + Facility | the same, the glass room |
| GrisEchec_Usine | 30 × 24 | General + BikeShop | the New Mauville set: generators, crates, benches as belts |
| GrisEchec_Gym | 15 × 46 | Building + MossdeepGym | gray checker floor, black void, coloured panels (board, binary tiles), warp pad |
| GrisEchec_Gym_Sourde | 9 × 7 | Building + MossdeepGym | the same |
| RouteS03 | 20 × 50 | General + Mauville | the Route 111 north rock: a sand canyon |
| RouteS03_Depot | 15 × 14 | General + Mauville | Mauville pavement between stacked gray boxes (containers) |
| ViberCity | 40 × 32 | General + Mauville | Mauville's pavement and buildings; the neons are static |
| ViberCity_PokemonCenter | 13 × 9 | Building + PokemonCenter | as Gris-Échec's, with two benches and more stools |
| ViberCity_Mart | 12 × 8 | Building + Shop | as Gris-Échec's, with the shelves one row higher |
| ViberCity_House1 | 9 × 7 | Building + GenericBuilding | a small house |
| ViberCity_StationLumiere | 16 × 12 | General + BikeShop | New Mauville's power-plant look |
| ViberCity_Gym | 13 × 16 | Building + MauvilleGym | the electric barriers (closed and open metatiles) |
| ViberCity_Casino | 20 × 11 | Building + MauvilleGameCorner | slots, counters, red carpet |
| ViberCity_Casino_B1F | 16 × 12 | General + Facility | the Aqua Hideout set: metal floor, racks |

The two Centres and the two Marts have their own layouts, not a shared one. They differ slightly, and a shared layout would make two edit dirs write the same file.

**Connections (all mutual):**
- BoisEther East ↔ RouteS02 West (offsets 10 / −10). Bois-Éther's east corridor y 12–15 meets the route's entrance y 2–5. The entrance is four rows, one more than the source's y 3–5, to match the corridor and to cut whole 2×2 trees; `Route_S02_Sc01` covers the extra row.
- RouteS02 East ↔ GrisEchec West (offsets −7 / 7). The route's exit at y 3–5 meets the town's at y 10–12.
- GrisEchec North ↔ RouteS03 South (offsets 29 / −29). The town's corridor at x 38–39 meets the canyon at x 9–10.
- RouteS03 North ↔ ViberCity South (offsets −10 / 10). The canyon's exit at x 10 meets Viber's one-cell ramp at x 20.

**Pending link, left solid on purpose:**
- **Ch3 → Ch4.** Viber-City's east street (y 15–16) ends at a rail barrier (37, 15–16, AllowCollision), with trees at x 38–39.
  - Ch4 must clear the trees at (38–39, 15–16) and add `Connect((dir: East, offset: …, map: "RouteS04"))` on both maps.
  - `ViberCity_OnLoad` draws the barrier open once `FLAG_SIL_VIBER_BARRIERE_EST` is set.

**Warnings and allowances.** Every behaviour warning is fixed or accepted in `03_allow.ron` with a reason:
- **RouteS02:** the security cables c1–c3 and the walls of the Aimant and server pockets are rail metatile 241 kept solid. 241 is walkable in all 23 of its Emerald uses. `RouteS02_OnLoad` redraws a cable walkable once it is cut.
- **GrisEchec:**
  - the Mart annex ladder is an iron fence (Rustboro has no ladder), so it is walkable;
  - the annex roof is walkable, so the J1 sbire stands on it;
  - the roof's top edge is solid, so the ladder is the only way up.
- **GrisEchec_Usine:** the cable before J3 (rail 241) is solid.
- **GrisEchec_TourObservation:** the wall column's floor (0, 6) is solid on the gate railing's row, so the gap (5–6, 6) is the only way past the first sbire.
- **GrisEchec_Gym:** the invisible maze walls are floor 513 with collision 1.
- **ViberCity:** the Route S4 barrier (rail 241) is solid.
- **ViberCity_Casino:** the hidden-stairs warp (18, 1) sits on a wall cell until `Casino_Sc08`. Then `OnLoad` draws exit mat 518 there.

**Source fixes.** Each one is noted in the edit scripts.
- **RouteS02:**
  - c1 (9, 6) and c2 (27, 9) sit on cables A and C, at (10–11, 5) and (31–32, 9);
  - the Promeneur shares René's cell (6, 8), so he moves to the cliff (4, 13) with his Skitty at (5, 13);
  - the cables run down into the sea, so the cliff path does not bypass the corridors;
  - every way north crosses rows y 7 then y 6 of x 48–51, so Sc03's "VAR 21, or 20 if Adrien was skipped" becomes row 7 = 20 and row 6 = 21;
  - the hidden Ether (36, 12) is not "behind c2": cable C's gap (31–32, 3) reaches it. Accepted; `items.ron`'s place text is the source's;
  - the west entrance is y 2–5, not y 3–5 (see the connections).
- **GrisEchec:**
  - the Iron moves from (4, 6), inside the Centre's stamp, to (6, 4) behind it;
  - Sc06 fires at (34, 11), since a door's exit step fires no coord event;
  - the S3 guard (36, 1) and the barrier (37, 1) are at (38, 1) and (39, 1), so Sc06's `set_metatile(37, 1, …)` becomes (39, 1);
  - J1 (16, 5) is on the annex roof at (17, 4), and its sbire (17, 5) at (19, 5) with sight 4, so he sees the ladder top (15, 5);
  - the balayeur (21, 14) stands at (21, 13), beside the bin (20, 14). The bin can be reached from every side, so `EventScript_Brouilleur2` starts his battle first while he is undefeated;
  - the Ouvrier (28, 19), inside the factory's stamp, is at (28, 21);
  - the arena guard's step aside (Sc05) is a second object at (35, 10), `LOCALID_GRIS_GARDE_ECARTE`. Sc05 walks the guard right, then sets `FLAG_SIL_HIDE_GRIS_GARDE` and clears `FLAG_SIL_HIDE_GRIS_GARDE_ECARTE`. Keyed on `VAR_SIL_STORY` ≥ 25, the guard would already stand aside when the player comes down from the Tour at 25, and Sc05 could be skipped; keyed on ≥ 26, he would stand on the arena's exit cell (34, 10);
  - the second Cube (31, 22) has no interior, so its door (34, 30) is wall.
- **GrisEchec_TourObservation:** both sbires face right, not down: tour 1 at (4, 7) (sight 2) watches the gate's approach (5–6, 7), and tour 2 at (4, 2) (sight 3) the lift's approach (5–6, 2). The source's cells could be walked past.
- **GrisEchec_Usine:** J3's sbire is at (23, 5) facing right (sight 2), watching (25, 5), the only cell in front of the cable (25, 4). The source's (24, 6) facing left could be walked past by x 25–26.
- **GrisEchec_Gym:**
  - **Salle 1.** The source's x 7 transition lets a player walk straight up and skip every bit. Each bit tile is a coord event, all on `VAR_TEMP_3`, which no script on this map writes, so they always fire (see "Temp-var triggers" below):
    - wrong tiles → `S1Erreur`;
    - right tiles → `S1Bit`, which counts in `VAR_TEMP_2`, and only when the player's y is 37 − `VAR_TEMP_2` (else stepping on (8, 37) eight times would pass);
    - the doorway (7, 29) → `S1Fin`, which checks that the count is 8. It returns at once when the player faces down: coming back from Salle 2 or from the Chambre Sourde (a warp, so a reload that clears the count) must not raise the error.
  - **Salle 2.** The terminal moves from the entry cell (7, 28) to a console at (6, 28).
  - **Salle 3.** Every watched square has one coord event, keyed on `VAR_TEMP_3` 0. Its script name lists the watchers in the source's priority order: `VueP1`, `VueP2`, `VueC`, `VueF`, `VueT`, `VueCF`, `VueFT`, `VueRoi`, `VueCFRoi`. A search over every path confirms the source: no path without a battle, at least two battles, the Fou on every path.
  - The rules screen `EcranEchiquier` (4, 12) is at the far end of the board, as a wall screen; it can only be read after crossing. Accepted.
- **RouteS03:** Noé is male (`TRAINER_PIC_PSYCHIC_M`), so his gfx is `PSYCHIC_M`.
- **RouteS03_Depot:**
  - the Great Ball moves from (13, 8), a container, to (11, 8) (`items.ron` keeps the source's `pos`, a note);
  - the blocks reset on every load, and A1 (7, 11) only moves down, onto (7, 12), before the south portal. So from `VAR_SIL_STORY` 31 `RouteS03_Depot_OnTransition` puts them in the solved layout (`setobjectxyperm` A1 (8, 6), B1 (7, 3), B2 (8, 2)), which can be crossed both ways.
- **ViberCity:**
  - Sc03 fires on the row in front of the dancers (27–30, 6), not only (28, 6), so the door cannot be approached without it;
  - Sc04's guard stands on (8, 5), so the coords are the cells beside him;
  - Sc06 ("2 tuiles en sortant du Nocturne") is (28, 6), the source's cell, plus (27, 5) and (29, 5): the door's exit step (28, 5) fires no coord event, as for Brico_Bourg_Sc01. The first build put it on row y 7, which rows 5–6 walk around.
- **ViberCity_StationLumiere:**
  - the notice desk N (7, 10) blocks the entry, so it moves to (9, 10);
  - the Technicienne faces down (sight 1): facing left, her sight starts on a transformer (12, 6);
  - the reset lever R is at (11, 2) as in the source's grid (its text says (10, 2)), read from (10, 2).
- **ViberCity_Gym:** Hugo's sight is 3, so he and Lena do not both see the entry's first cell (6, 14).
- **ViberCity_Casino:**
  - the grid draws S at (17, 1), but the text and Sc08's `set_metatile` say (18, 1), and (18, 1) is used;
  - the counter clerks stand behind the counters (3, 7) and (16, 7), and the player talks across from row 9;
  - the poster guard stands at (18, 2): (17, 1), the only cell to read the poster from, is reached through (16, 1);
  - Gogo faces the violet machine (2, 1) on his left.
- **ViberCity_Casino_B1F:** Romain starts at (10, 8), and B1F_Sc02's Walk(Down) ×2 ends at (10, 10), beside the capsule's only reading cell (9, 10). The head croupier moves aside to (6, 10) once `FLAG_SIL_CASINO_CHEF`.
- **Music:** `MUS_RUSTBORO` (Viber-City and the house) and `MUS_ROUTE110` (Route S3) are the decomp's tracks for Mauville and Route 111; the source's `MUS_MAUVILLE` and `MUS_ROUTE111` do not exist.

**Scene paths to re-derive in Rust** (the source's collide on the built layouts; each is in the edit comments):
- Route_S02_Sc03: Gwada's Walk(Left) ×2 runs into a player on (48–49, 6), and Walk(Right) ×6 crosses the trees at (52–55, 6). Go up to (48, 5), then east along y 5.
- Gym2_S1: the robot's Walk(Left) ×2 from (10, 33) must end beside the player's tile.
- Gym2_Sc11: Gwada's Walk(Up) ×7 from (7, 21) ends on e7 (7, 14), possibly the player's cell. Stop below the player.
- Viber_City_Sc01: Walk(Up) ×12 from (20, 29) crosses the Casino. Up ×5, Left ×4, Up ×7, Right ×4 ends at (20, 17).
- Viber_City_Sc03: Tauber stays on the door cell with Face(Down); Walk(Down) ×2 runs into dancer 1 and the player.
- Viber_City_Sc04: when (8, 6) fired, the guard walks Left, Down, Left ×2.
- Viber_City_Sc05: place Adrien at (4, 5) and the cart Mathieu (`LOCALID_VIBER_MATHIEU_CHARIOT`) at (8, 8) before showing them, so they walk up to the player at (8, 5).

**Temp-var triggers** (sound under Gen3: `ClearTempFieldEventData` zeroes `VAR_TEMP_*` and `FLAG_TEMP_*` on every map load, warp or connection, and the engine's `Vars::clear_temp` does the same):
- Coord events on `VAR_TEMP_3` 0 (GrisEchec_Gym's 48 tiles, the Station's fuse-room doorway (7, 4)) always fire, and the scripts decide. This is Gen3's own idiom (Mossdeep's and Mauville's gyms, the Trick House key their tiles on `VAR_TEMP_1` 0). The engine has no "always" trigger: Gen3's var 0 means `TRIGGER_RUN_IMMEDIATELY`, which runs without a script context (no messages), and the engine never matches it. Contract: no script on those maps writes `VAR_TEMP_3`.
- `ViberCity_Sc05` no longer runs on every entry. `ViberCity_OnTransition` sets `VAR_TEMP_1` to 1 only when `FLAG_SIL_VIBER_BLACKOUT` is set and `VAR_SIL_STORY` is 33 (Station_Lumiere_Sc03 has just warped the player to (8, 5)). The on-frame table runs Sc05 on `VAR_TEMP_1` 1, and Sc05 sets it back to 0 with `VAR_SIL_STORY` 34. Keying on `VAR_SIL_STORY` 33 alone would also match before the black-out and loop; the first build's `VAR_TEMP_1` 0 ran a script on every entry (the edit comment also said 34 for 33).

**Hide flags.** Every object whose visibility is story state has its own persistent flag, as in Ch0 and Ch1; no map uses a `FLAG_TEMP_*` hide flag any more. The first build used `FLAG_TEMP_1`–`6`, because the ids were owned by another task.
- The source's own flags stay where it names them: `FLAG_SIL_HIDE_VIBER_ADRIEN`, `_STATION_GARDE`, `_STATION_SBIRES` (sbire 1), `_NOCTURNE_DANSEURS` (dancer 1) and `_DEPOT_GOGO_ROMAIN` (Gogo). The east barrier's guard gets `FLAG_SIL_VIBER_BARRIERE_EST`, which the source defines as "guard gone".
- New, allocated by `cargo xtask silice import … --force` from `maps/*.ron` into a scratch copy (flags 775–803) and appended to `ids.golden`. Only `ids/flags.ron` and `IMPORT-REPORT.md` were taken from the import, as for Ch1. They are in their own commit (`feat(silice): ids for Ch2–Ch3 maps`) so they can be renumbered after the Ch1 ids if needed.

  | Map | Objects | Flags | Owner | New game |
  |---|---|---|---|---|
  | RouteS02 | Adrien, Gwada | `_ROUTE_S02_ADRIEN`, `_ROUTE_S02_GWADA` | OnTransition: Gwada at story 17–21; Adrien at 17–21, then while `FLAG_SIL_ITEM_ADRIEN_CH2` is clear. Sc01 runs at 17 on the map, so they must show at 17 | hidden |
  | GrisEchec | arena guard, its stepped-aside copy | `_GRIS_GARDE`, `_GRIS_GARDE_ECARTE` | Sc05 swaps them | shown / hidden |
  | GrisEchec | Gwada, S3 guard, Lainergie | `_GRIS_GWADA`, `_GRIS_GARDE_S3`, `_GRIS_LAINERGIE` | OnTransition: Gwada at 22–27, the guard until 28, Lainergie at 27 | hidden / shown / hidden |
  | GrisEchec_Gym | robot, Sc11's Gwada, Wattouat, Lainergie | `_GYM2_ROBOT`, `_GYM2_GWADA`, `_GYM2_WATTOUAT`, `_GYM2_LAINERGIE` | their scenes; OnTransition sets all four | hidden |
  | RouteS03_Depot | Romain, the sbire | `_DEPOT_ROMAIN`, `_DEPOT_SBIRE` | Depot_Sc02 (with `_DEPOT_GOGO_ROMAIN`) | shown |
  | ViberCity | Mathieu at the stand | `_VIBER_MATHIEU` | Sc04 sets, Sc05 clears | shown |
  | ViberCity | dancers 2–4 | `_NOCTURNE_DANSEUR_2`/`_3`/`_4` | Sc05 (with `_NOCTURNE_DANSEURS`) | shown |
  | ViberCity | Tauber, Mathieu with the cart | `_VIBER_TAUBER`, `_VIBER_MATHIEU_CHARIOT` | Sc03/Sc06, Sc04/Sc05 | hidden |
  | ViberCity_PokemonCenter | the six people under blankets | `_VIBER_PC_PNJ01`…`06` | OnTransition, from `FLAG_SIL_VIBER_BLACKOUT` | hidden |
  | ViberCity_StationLumiere | sbire 2 | `_STATION_SBIRE_2` | Station_Lumiere_Sc03 (with `_STATION_SBIRES`) | shown |
  | ViberCity_Casino | Gogo, Romain | `_CASINO_GOGO`, `_CASINO_ROMAIN` | OnTransition: from story 32 while `VAR_SIL_CASINO_STATE` < 3 (they stay after Sc03 with their repeatable lines) | hidden |
  | ViberCity_Casino_B1F | Romain | `_CASINO_B1F_ROMAIN` | B1F_Sc02; OnTransition sets it | hidden |

- `game.ron`'s `new_game_flags` sets the hidden ones, plus `FLAG_SIL_ITEM_DEPOT_METAL_COAT`: the Metal Coat shows once Depot_Sc02 clears its flag. `game_tests.rs` checks both groups.
- Scenes that show or hide an actor (`show`/`hide`) also set its flag, as in Ch1.

**Script contracts.** All names stay unwritten under `registry::ALLOW_UNWRITTEN`. They follow the Ch0 convention:
- `<Map>_ScNN` for a source scene;
- `<Map>_EventScript_<What>` for other events;
- `<Map>_OnLoad` and `<Map>_OnTransition` for map scripts.

Below, each map's names are given without the `<Map>_` prefix. Trainer objects carry their battle in `EventScript_<Name>`.
- **`ch02_*.rs` (95 names):**
  - RouteS02:
    - scenes and map scripts: `Sc01`, `Sc02` (Adrien), `Sc03` (coords), `OnLoad`, `OnTransition`;
    - NPCs: `EventScript_Technicien`, `EventScript_Agent`, `EventScript_Gwada`, `EventScript_Promeneur`, `EventScript_Skitty`;
    - trainers: `EventScript_Hugo`, `EventScript_Rene`, `EventScript_Zoe`, `EventScript_Gerard`, `EventScript_Sbire`, `EventScript_Anais`;
    - items: `EventScript_ItemXSpeed`, `EventScript_ItemMagnet`, `EventScript_ItemGreatBall`;
    - signs: `EventScript_SignEntree`, `EventScript_SignCable`, `EventScript_SignTransfo`, `EventScript_SignFalaise`;
    - obstacles: `EventScript_Cable1`, `EventScript_Cable2`, `EventScript_Cable3`, `EventScript_Serveur`.
  - GrisEchec:
    - scenes and map scripts: `Sc01`, `Sc03` (the guard: Sc03 and Sc05), `Sc06`, `OnLoad`, `OnTransition`;
    - NPCs: `EventScript_Echecs`, `EventScript_Pressee`, `EventScript_Reparateur`, `EventScript_Etudiante`, `EventScript_Ouvrier`, `EventScript_Enfant`, `EventScript_Gwada`, `EventScript_Lainergie`, `EventScript_GardeS3`;
    - trainers: `EventScript_Balayeur`, `EventScript_SbireToit`;
    - jammers: `EventScript_Brouilleur1`, `EventScript_Brouilleur2` (Sc02 with n = 1 and 2);
    - signs: `EventScript_SignVille`, `EventScript_SignArene`, `EventScript_SignTour`, `EventScript_SignUsine`.
  - GrisEchec_PokemonCenter: `OnTransition`, `EventScript_Nurse`, `EventScript_Npc`.
  - GrisEchec_Mart: `EventScript_Clerk`, `EventScript_Npc`, `EventScript_Distributeur`.
  - GrisEchec_TourObservation: `EventScript_Sbire1`, `EventScript_Sbire2`, `EventScript_Ecran`.
  - GrisEchec_TourObservation_Top: `Sc01` (Duval), `EventScript_Console`.
  - GrisEchec_Usine: `OnLoad`, `EventScript_Sbire`, `EventScript_Cable`, `EventScript_Brouilleur3`, `EventScript_Caisses`, `EventScript_ItemEther`, `EventScript_ItemMetalCoat`, `EventScript_ItemSuperRepel`.
  - GrisEchec_Gym:
    - scenes and map scripts: `OnTransition`, `Sc10` (Yohan; Sc11 and Sc12 chain from it);
    - hall: `EventScript_Conseiller`, `EventScript_EcranHall`, `EventScript_EcranS1`;
    - Salle 1: `EventScript_S1Erreur`, `EventScript_S1Bit`, `EventScript_S1Fin`;
    - Salle 2: `EventScript_Terminal`;
    - Salle 3, the pieces: `EventScript_Pion1`, `EventScript_Pion2`, `EventScript_Cavalier`, `EventScript_Fou`, `EventScript_Tour`;
    - Salle 3, the watched squares: `EventScript_VueP1`, `EventScript_VueP2`, `EventScript_VueC`, `EventScript_VueF`, `EventScript_VueT`, `EventScript_VueCF`, `EventScript_VueFT`, `EventScript_VueRoi`, `EventScript_VueCFRoi`;
    - Salle 3, the screen: `EventScript_EcranEchiquier`.
  - GrisEchec_Gym_Sourde: `EventScript_Melomane`, `EventScript_Coffre`, `EventScript_Panneau`.
- **`ch03_*.rs` (113 names):**
  - RouteS03:
    - scenes: `Sc01`;
    - NPCs: `EventScript_Pnj01`, `EventScript_Pnj02`, `EventScript_Pnj03`;
    - trainers: `EventScript_Sbire`, `EventScript_Roger`, `EventScript_Sacha`, `EventScript_Marco`, `EventScript_Ines`, `EventScript_Noe`;
    - items: `EventScript_ItemSuperPotion`, `EventScript_ItemHardStone`, `EventScript_ItemSuperRepel`;
    - signs: `EventScript_Panneau1`, `EventScript_Panneau2`.
  - RouteS03_Depot:
    - scenes and map scripts: `OnTransition`, `Sc02`;
    - the blocks: `EventScript_Bloc` (Depot_Sc01, all three blocks);
    - items: `EventScript_ItemMetalCoat`, `EventScript_ItemGreatBall`;
    - signs: `EventScript_Panneau1`, `EventScript_Panneau2`.
  - ViberCity:
    - scenes and map scripts: `OnLoad`, `OnTransition`, `Sc01`, `Sc02` (the neon sign), `Sc03`, `Sc04`, `Sc05` (on_frame on `VAR_TEMP_1` 1, which OnTransition arms; it sets `VAR_TEMP_1` back to 0), `Sc06`;
    - NPCs: `EventScript_Mathieu`, `EventScript_Adrien`, `EventScript_Danseur`, `EventScript_GardeStation`, `EventScript_Pnj01`, `EventScript_Pnj02`, `EventScript_Pnj03`, `EventScript_Pnj04`, `EventScript_Pnj05`, `EventScript_Pnj06`, `EventScript_Pnj07`, `EventScript_GardeBarriere`;
    - items: `EventScript_ItemRevive`;
    - signs: `EventScript_Panneau1`, `EventScript_Panneau2`, `EventScript_Panneau3`, `EventScript_Panneau4`.
  - ViberCity_PokemonCenter: `OnTransition`, `EventScript_Nurse`, `EventScript_Pnj01`, `EventScript_Pnj02`, `EventScript_Couverture`.
  - ViberCity_Mart: `EventScript_Clerk`, `EventScript_Pnj01`.
  - ViberCity_House1: `EventScript_Danseur`.
  - ViberCity_StationLumiere:
    - scenes and map scripts: `OnLoad`, `OnTransition`, `Sc01` (the foreman);
    - trainers: `EventScript_Sbire1`, `EventScript_Sbire2`, `EventScript_Technicienne`;
    - the fuse room: `EventScript_Levier1`, `EventScript_Levier2`, `EventScript_Levier3`, `EventScript_Levier4`, `EventScript_LevierR`, `EventScript_Tube`, `EventScript_SortieFusibles`;
    - decor: `EventScript_Note`, `EventScript_Transfo`;
    - items: `EventScript_ItemElixir`.
  - ViberCity_Gym:
    - scenes and map scripts: `OnLoad`, `Sc02` (Tauber and Hocseat, the coord and talking);
    - consoles: `EventScript_ConsoleA`, `EventScript_ConsoleB`, `EventScript_ConsoleC`;
    - trainers: `EventScript_Lena`, `EventScript_Hugo`, `EventScript_Nina`, `EventScript_Eliott`;
    - NPCs: `EventScript_Gerant`;
    - items: `EventScript_ItemMagnet`.
  - ViberCity_Casino:
    - scenes and map scripts: `OnLoad`, `OnTransition`, `Sc01`–`Sc08` (Sc06 on every slot machine cell);
    - trainers: `EventScript_Fernand`, `EventScript_Gaston`, `EventScript_Solene`;
    - NPCs: `EventScript_Pnj01`, `EventScript_Pnj02`, `EventScript_Pnj03`, `EventScript_Pnj04`;
    - Gogo and Romain after Sc03: `EventScript_Gogo`, `EventScript_Romain` (the source's repeatable `Casino_Gogo_T1` and `Casino_Romain_T1`);
    - decor: `EventScript_MachineViolette`, `EventScript_JetonsCaches` (the hidden coins are a bg script; there is no coin item).
  - ViberCity_Casino_B1F:
    - scenes and map scripts: `OnTransition`, `Sc01` (the terminal), `Sc02` (the capsule);
    - trainers: `EventScript_Sbire1`, `EventScript_Sbire2`, `EventScript_Sbire3`, `EventScript_CroupierChef`;
    - items: `EventScript_ItemSilk`, `EventScript_ItemUltraBall`, `EventScript_ItemRareCandy`.

**Missing graphics and mechanics (static fallbacks in place):**
- **Neons:** Viber-City's 3-phase neons, the dance floor and the Nocturne's lit floor are static. The source wants tile animation.
- **Casino:** no neon, Electric or Ghost symbols, and no poster metatile (the poster is a wall cell). The game corner set has no stairs, so the hidden stairs are a south arrow mat: the player steps on it, then presses down.
- **Nocturne barriers:** all three colours use the same Mauville beam (closed 552/553, open 570/571). The colours exist only in the scripts.
- **Missing objects:**
  - containment cables use Mauville logs, security cables use rail 241, and containers use gray boxes;
  - the Usine's conveyor belts are benches, and its oil puddles are a floor-dot metatile (574);
  - jammers, levers, tubes and consoles are cabinets, machines or pole bases;
  - there is no truck on Route S3;
  - the Mart annex ladder is an iron fence.
- **Sprites.** Placeholders from the extract:

  | Character | Placeholder gfx |
  |---|---|
  | sbires | NINJA_BOY |
  | Gogo | MAGMA_MEMBER_M |
  | Romain | AQUA_MEMBER_M |
  | Yohan | PSYCHIC_M |
  | Tauber, Hocseat | TATE, LIZA |
  | the robot | MOVING_BOX |
  | Wattouat, Lainergie | PIKACHU |
  | the depot blocks | PUSHABLE_BOULDER |
  | Pions | YOUNGSTER |
  | Cavalier | BLACK_BELT |
  | Fou | GENTLEMAN |
  | Tour | EXPERT_M |
  | Mélomane, DJ, Monsieur Dex | MANIAC |
  | Nina | HEX_MANIAC |
  | Guitariste Sacha | MAN_5 |
  | Ranger Inès | WOMAN_3 |
  | Kinésiste Anaïs (Route S2) | EXPERT_F |
  | Kinésiste Noé (Route S3) | PSYCHIC_M |
- **Mechanics left to scripts or the engine:**
  - **Champ Magnétique:** the server block, the crates and the depot blocks;
  - **Lame-Fréquence:** the cables and jammers, done by `set_metatile` in `OnLoad` per flag;
  - the invisible maze's light-up;
  - the Chambre Sourde's sound rule;
  - the Mallette and the follower Pokémon (two objects moved together);
  - **wild encounters:** they live in `battle/encounters.ron`. The Usine's "dark zones" use plain floor; whether the BikeShop floor carries the land-encounter attribute is unchecked.

**Editor friction (new since Ch0):**
1. Arrow-warp exits must be found per tileset (`palette --labels` lists them: General + Facility 518/519, BikeShop 536/544, GameCorner 518/519, MossdeepGym 6/7). Some sets have none, which explains the Casino stairs above.
2. The sandbox refuses `studio-map` calls with shell loops or variables, so every batch of renders went through a small Python driver.
3. Interiors are drawn from small Emerald pieces through a scratch generator that emits `Ascii` ops with auto-assigned legend characters. A `Cells` op (rows of raw blocks), or a stamp per piece, would remove the throwaway legends.
4. A coord event matches one value, so "≥ N" conditions need an always-zero temp var and a script check. A `Coord` with a "fires always" flag, or a value range, would make this explicit.
5. The validator warns when two objects share a hide flag, and when two coord events share a cell, even with different values (Gen3 allows both). Neither can be accepted in `03_allow.ron`, which takes only `AllowCollision`.
6. **Time:** about 6 h for 19 maps, including learning 7 new tileset pairs.

**Check exception.** `games/silice/check-exceptions.ron` gains one `placement-map` entry for `FLAG_SIL_ITEM_CASINO_COINS_FLOOR`. The 20 floor coins are a `Hidden` placement with no item, so they are a bg script (`ViberCity_Casino_EventScript_JetonsCaches`, (14, 3)), not a hidden item. Drop the entry once the ch03 scripts name the flag.

**Follow-ups outside the maps.**
- Both Centres' `OnTransition` call `set_respawn`, which needs `HEAL_LOCATION_SIL_GRIS_ECHEC` and `HEAL_LOCATION_SIL_VIBER_CITY` in `game.ron`'s append-only `heal_locations`. They are left to the ch02/ch03 scripts so that the list is not appended by two branches at once.
- `FLAG_SIL_DEPOT_MAGNET_ON` is a "temporary" flag in the source (cleared on leaving the Depot) but a plain flag in `ids/`. Gen3's boulder gate is `FLAG_SYS_USE_STRENGTH`, which `game.ron` already clears on map change: Depot_Sc01 should set that once the engine pushes boulders.
- `items.ron` keeps the source's `pos` for the moved Iron (4, 6) and Great Ball (13, 8). The maps are authoritative, and the importer would rewrite a hand edit.

#### SIL3 Ch6 maps as built (Manga-Square)

The nine Chapter 6 maps are in `games/silice/{maps,layouts}/`, made only with the studio map editor. Their edit scripts are in `games/silice/edits/<Map>/` (`00_create`, `01_terrain`, `02_events`, plus `03_allow` for the Dojo). Seven stamps cut from Emerald's Battle Frontier (`stamp-from --no-elevation`) are in `games/silice/stamps/ms_*.ron`:
- `ms_pagoda`, the Battle Palace (11 × 7);
- `ms_dojo`, the Battle Arena (9 × 9);
- `ms_gate`, the Arena's gate, used for the four torii (7 × 5);
- `ms_house_small` (3 × 4) and `ms_house_long` (6 × 4);
- `ms_rock_pen`, a fenced boulder (5 × 3);
- `ms_planter` (10 × 3).

No other chapter's map was edited. `cargo xtask studio map build` rebuilds all 21 maps and writes the same bytes (checked against a copy of `maps/` and `layouts/`). `validate` reports no errors, `hygiene` (with `games::copies`) is clean, `PKR_GAME=silice cargo test -p content -p extract` passes, and so do `cargo xtask silice check` and `cargo test -p xtask --test silice_games`.

**Names.** The chapter's map constants are `MAP_MANGA_SQUARE` and its `_BIBLIOTHEQUE`, `_DOJO`, `_GYM`, `_MART`, `_POKEMON_CENTER` and `_TOILETTEUR`, plus `MAP_ROUTE_S06` and `MAP_ROUTE_S06_BOIS_SOUPIRS`. The map names are the ones `ids/maps.ron` gives: `MangaSquare`, `MangaSquare_Bibliotheque`, `MangaSquare_Dojo`, `MangaSquare_Gym`, `MangaSquare_Mart`, `MangaSquare_PokemonCenter`, `MangaSquare_Toiletteur`, `RouteS06` and `RouteS06_BoisSoupirs`. Each layout has its map's name.
- `MAP_MANGA_SQUARE_GYM` had no id: the importer reads map names from headings only, and ch06 names the gym in its §2 and §4 text. It was appended by hand as 115, and its `map-undefined` exception was removed.
- `MAP_MANGA_SQUARE_HOUSE_1` and `_HOUSE_2` are named in §2 with no content. No map is drawn for them. Their houses stand in the Rue des Échoppes with solid doors and no warps: (3, 21) and (7, 21). Their `map-undefined` exceptions stay.
- Other chapters' names: `MAP_OWLYS` (Ch5, south of Route S6) and `MAP_ROUTE_S07` (Ch7, through the Grand Torii). See the pending links below.

**Sizes and tilesets** (Emerald tilesets by name; every layout is original):

| Map | Size | Tilesets | Why this pair |
|---|---|---|---|
| MangaSquare | 44 × 38 (the source's size) | General + BattleFrontierOutsideEast | The Battle Frontier's east side has Emerald's only Japanese buildings: the Arena dojo, its gate (used as the torii) and the Palace pagoda (used as the library). It also has slim trees that read as bamboo, sand and paving. General adds the Center, the Mart, water and the forest. |
| RouteS06 | 28 × 64 (the source's size) | General + Facility | Facility is the secondary of Mt. Pyre's exterior, Emerald's only mountain shrine path. It has terraces, stone stairs and stone markers, used here as lanterns and incense burners. Its weather is fog, as the source asks. |
| RouteS06_BoisSoupirs | 24 × 20 (the source's size) | General + Fortree | Route 119/120's pair: a wooded pond with a wooden bridge. |
| MangaSquare_Dojo | 30 × 22 (the source's size) | Building + BattleArena | Tatami floor, shoji walls with lattice, a training court used as the stage, a giant screen, bonsai and the back door. |
| MangaSquare_Bibliotheque | 26 × 24 (the source's size) | Building + Lab | The only pair with tall bookshelves and server-like racks together. |
| MangaSquare_PokemonCenter | 12 × 9 | Building + PokemonCenter | standard (Ch1's vocabulary, with one more row) |
| MangaSquare_Mart | 10 × 9 | Building + Shop | standard (Ch1's vocabulary, with one more row) |
| MangaSquare_Toiletteur | 10 × 9 | Building + PokemonFanClub | A small salon: a pink rug, armchairs and plants. |
| MangaSquare_Gym | 22 × 30 (the source's size) | General + BattleFrontierOutsideEast | Same as the town: raked gravel (the Frontier's sand), paving, water and boulders. The Frontier's white fence makes the shoji walls, and the Arena's lattice door (657) makes the closed shoji doors. |

Borders: the outdoor maps use General's 2 × 2 forest (468/469/476/477), and Route S6 uses Facility's rock (113). The interiors use black, blocked (1025).

**Warnings and allowances.**
- `validate`'s only Ch6 warnings are the 77 unwritten scripts (MangaSquare 21, RouteS06 28, Bibliotheque 16, Dojo 15, Gym 14, BoisSoupirs 5, Center 3, Toiletteur 2, Mart 1).
- `MangaSquare_Dojo/03_allow.ron` keeps the signing queue's ribbon (BattleArena 629, y 11) solid. In the Arena it is a walkable court line. Here it fences the queue.
- Fixed instead of allowed:
  - The ponds (General 368 in the town and the gym) are surfable water: collision 0, elevation 1, as in Emerald.
  - The Bibliothèque's rack shadows (Lab 640) and the desk's left foot (546) are walkable, as in the Lab.
  - Adrien's table and the archivist's counter first used Lab 575 and 573/574. Those are floor shadows, so the table became a Lab desk (538/539/546/547) and the counter became the partition's pieces (607/615).
- `silice check` still warns about two unused exceptions that are not Ch6's: `sil-prefix` `WEATHER_CLOUDS` and `WEATHER_PETALS`.

**New ids** (commit `feat(silice): ids for Ch6 maps`). The hide flags were allocated by `cargo xtask silice import … --out <scratch copy> --force` from the maps, after Ch1's 765–774. Only `ids/flags.ron` was copied back, because the import would also undo the provisional `Tour_Sbire{1,2}_Intro_Ch09` rename. All are appended to `xtask/tests/fixtures/silice/ids.golden`.
- flags:
  - 775 `FLAG_SIL_HIDE_BIBLIO_ADRIEN`;
  - 776–777 `FLAG_SIL_HIDE_DOJO_DRONE_1`, `_2`;
  - 778 `FLAG_SIL_HIDE_MANGA_SQUARE_LIVREUR`;
  - 779 `FLAG_SIL_HIDE_ROUTE_S06_RANDONNEUR`;
  - 780–781 `FLAG_SIL_HIDE_ROUTE_S06_SBIRE_A`, `_B`.
- map 115: `MAP_MANGA_SQUARE_GYM`.

Every other flag the maps name already existed: `_HIDE_DOJO_ADRIEN`, `_HIDE_DOJO_KIMIKO`, `_HIDE_MANGA_SQUARE_{ADRIEN_FIN, BENOU, CONTROLEUR}`, `_HIDE_ROUTE_S06_ADRIEN`, `_GYM06_PORTE_1`–`5`, `_ROUTE_S06_RACCOURCI` and the `FLAG_SIL_ITEM_*` flags. No `FLAG_TEMP_*` is used as a hide flag. The vars are `VAR_SIL_STORY` and, for the gym's every-step coord events, `VAR_TEMP_1`.

**Events per map** (my coordinates).
- **MangaSquare.**
  - Header: `MUS_LILYCOVE`, town, `WEATHER_NONE`. The petals are missing (see below).
  - `on_transition` `MangaSquare_OnTransition` sets the hide flags from `VAR_SIL_STORY` and `FLAG_SIL_BADGE_06`, and the dawn tint (which has no fallback yet).
  - Warps:
    - 0: the Bibliothèque, the pagoda's door (22, 7);
    - 1: the Dojo (36, 16);
    - 2: the Center (15, 23);
    - 3: the Konbini (3, 14);
    - 4: the Toiletteur (9, 14);
    - 5: the Monks' shortcut (1, 25), to RouteS06's warp 0.
  - Objects:

    | Local id | Who | Cell | Gfx | Script | Hide flag |
    |---|---|---|---|---|---|
    | 1 | COSPLAYEUR | (10, 16) | BLACK_BELT | `_EventScript_Cosplayeur` | |
    | 2 | PURISTE | (20, 9) | OLD_MAN | `_EventScript_Puriste` | |
    | 3 | MÈRE | (12, 26) | WOMAN_3 | `_EventScript_Mere` | |
    | 4 | ANCIEN | (19, 17) | MAN_5 | `_EventScript_Ancien` | |
    | 5 | LIVREUR | (24, 9) | MAN_2 | `_EventScript_Livreur` | `_LIVREUR` (new) |
    | 6 | GAMIN | (25, 17), wanders 2 | LITTLE_BOY | `_EventScript_Gamin` | |
    | 21 | CONTRÔLEUR | (37, 1), under the Grand Torii | MAGMA_MEMBER_M | `_EventScript_Controleur` | `_CONTROLEUR` |
    | 20 | BENOU | (37, 3) | DEVON_EMPLOYEE | none (Sc02) | `_BENOU` |
    | 22 | ADRIEN | (37, 8) | SCHOOL_KID_M | none (Sc03) | `_ADRIEN_FIN` |

    Local ids 20–22 are the source's obj numbers, set with `Object(local_id: …)`.
  - Coord events:
    - `MangaSquare_Sc01`, `VAR_SIL_STORY` 61, on (21–23, 33) under the Torii Sud, and on (2, 25), the first step off the shortcut (a warp arrival fires no coord event on its own cell);
    - `_Sc02`, value 66, on (36–38, 6) in front of the Grand Torii;
    - `_Sc03`, value 67, on (37, 1).
  - Signs:
    - the town sign (26, 34);
    - the statue (22, 16);
    - the Grand Torii sign (39, 6);
    - the two screens (18, 13) and (26, 13).
  - Hidden item: the Revive behind the dragon statue (22, 13).
  - Connection: South to RouteS06, offset 9 (its x 12–14 meet our Torii Sud x 21–23).
  - Terrain:
    - the pagoda (x 17–27, y 1–7) is the Bibliothèque;
    - the Dojo (x 32–40, y 10–18) sits between its two torii, (33, 20) and the Grand Torii (34, 1);
    - the Place du Dragon has the fenced dragon boulder (20–24, 14–16);
    - the Rue des Échoppes (west) has a street torii (3, 5), the Konbini, the Toiletteur and the two houses;
    - the Center is at (14–17, 20–23);
    - there is a pond (29–32, 3–5) and a flower planter (27–36, 26–28);
    - the Torii Sud is at (19–25, 32–36), and the south exit at x 21–23.
- **RouteS06.**
  - Header: `MUS_ROUTE120`, `WEATHER_FOG_HORIZONTAL`, route.
  - Map scripts:
    - `on_load` `RouteS06_OnLoad` reopens the bamboo curtain (23, 26) once `FLAG_SIL_ROUTE_S06_RACCOURCI` is set;
    - `on_transition` `RouteS06_OnTransition` sets the hide flags. The sbires and Adrien are hidden outside `VAR_SIL_STORY` 60, and the randonneur before 61. It also runs Route_S06_Sc01 on the first entry (see the script contract).
  - Warp 0: the shortcut tunnel (25, 26), to MangaSquare's warp 5.
  - Objects:
    1. Adrien (13, 25), hide flag `_ROUTE_S06_ADRIEN`;
    2. Sbire A (MAGMA_MEMBER_M) at (12, 25), flag `_SBIRE_A`;
    3. Sbire B (MAGMA_MEMBER_F) at (14, 25), flag `_SBIRE_B` (1–3 are the source's obj 1–3);
    4. Luna (BEAUTY) at (6, 9), sight 4;
    5. Hoshi (PSYCHIC_M) at (20, 13), sight 4;
    6. Yumi (ARTIST) at (8, 36), sight 3;
    7. Gaston (HIKER) at (12, 49), sight 4;
    8. Kotaro (NINJA_BOY) at (20, 57), sight 3;
    9. the Pèlerine (OLD_WOMAN) at (7, 26);
    10. the Coureur (RUNNING_TRIATHLETE_M) at (16, 39), walking up and down;
    11. the Bonze (OLD_MAN) at (4, 20);
    12. the Randonneur (CAMPER) at (20, 41), flag `_RANDONNEUR`;
    13. cracked rock R1 (BREAKABLE_ROCK) at (5, 57);
    14. cracked rock R2 at (22, 41);
    15. cracked rock R3 at (22, 20). The rocks have no flag, as in Emerald;
    16. item balls: the Hyper Potion (18, 60), the Ultra Ball (23, 2), and behind R1, R2 and R3 the PP Up (3, 56), the Max Ether (24, 40) and the Dragon Scale (23, 18).
  - Coord events: `RouteS06_Sc02`, `VAR_SIL_STORY` 60, on (12–15, 28), the top of the plateau's stairs.
  - Signs:
    - the entrance (11, 60);
    - the incense burner (16, 28);
    - the rock sign (6, 56);
    - the altar (4–5, 19).
  - Hidden item: the Lax Incense (17, 24).
  - Connections: North to MangaSquare, offset −9. West to RouteS06_BoisSoupirs, offset 25 (our y 33–35 meet its y 8–10).
  - Terrain, south to north:
    - the Lacets bas, with R1's niche (3–4, 56–57);
    - the middle zone, with the west exit and R2's niche (23–24, 40–41);
    - the Plateau de l'Encens, with lanterns, the curtain and the tunnel;
    - the altar ledge, with R3's niche (21–24, 18–19);
    - the Lacets hauts.

    Four stone stairs join the terraces.
- **RouteS06_BoisSoupirs.**
  - Header: `MUS_PETALBURG_WOODS`, route, `WEATHER_NONE` (the petals are missing).
  - Objects:
    1. Armand (GENTLEMAN) on the bridge (11, 8), sight 3;
    2. the Amoureux (MAN_3) at (7, 6);
    3. the Heart Scale ball (9, 12), in a notch reached only from the water.
  - Signs: `RouteS06_BoisSoupirs_Sc01`, the Pierre aux Vers (16–17, 12).
  - Hidden item: the Miracle Seed (6, 8).
  - Connection: East to RouteS06, offset −25.
- **MangaSquare_Dojo.**
  - Header: `MUS_CONTEST_LOBBY`, indoor.
  - `on_transition` `MangaSquare_Dojo_OnTransition` moves the monk aside from `VAR_SIL_STORY` 64 (`setobjectxyperm` to (27, 5)).
  - Warps:
    - 0: the exit mat (4, 21), to MangaSquare's warp 1;
    - 1: the back door (27, 2), to MangaSquare_Gym's warp 0. It is solid: pushing into it warps.
  - Objects (local ids are the source's where it numbers them):
    1. the Shinobi-Gear vendor (NINJA_BOY) at (3, 5);
    2. the restauratrice (WOMAN_2) at (3, 11);
    3. a cosplayer (GIRL_3) at (8, 13);
    4. a cosplayer (SAILOR) at (10, 13);
    5. Adrien (13, 10), `MangaSquare_Dojo_Sc01`, flag `_HIDE_DOJO_ADRIEN`;
    6. Kevin (POKEFAN_M) at (15, 10);
    7. Bruno (FAT_MAN) at (16, 10);
    8. Diane (POKEFAN_F) at (17, 10);
    9. Kimiko (LIZA) on the stage (21, 5), flag `_HIDE_DOJO_KIMIKO`;
    10. drone 1 (WINGULL) at (19, 4), flag `_DRONE_1`;
    11. drone 2 (WINGULL) at (24, 4), flag `_DRONE_2`;
    12. the disappointed fan (MANIAC) at (6, 19), `_Sc02`;
    13. the cook (COOK) at (8, 5), `_Sc04`;
    14. Kenta (BLACK_BELT) on the training mat (13, 15), `_Sc03`, sight 3;
    15. the monk (EXPERT_M) at (27, 4), guarding the back door.
  - Sign: the code scroll (7, 2).
- **MangaSquare_Bibliotheque.**
  - Header: `MUS_LILYCOVE_MUSEUM`, indoor.
  - Warps 0–1: the mat (12–13, 23), to MangaSquare's warp 0.
  - Objects:
    1. Adrien at his desk (13, 4), `MangaSquare_Bibliotheque_Sc01`, flag `_HIDE_BIBLIO_ADRIEN`;
    2. the archiviste (OLD_WOMAN) behind the counter (17, 16);
    3. the prestataire (SCIENTIST_1) in the Archives (20, 6);
    4. the étudiant (BOY_1) at (7, 14).
  - Coord events: `_Sc01`, `VAR_SIL_STORY` 63, on (12–15, 7), two cells from Adrien.
  - Signs:
    - the four lore books on the shelves' lower rows: Livre 1 (3, 8), 2 (9, 8), 3 (3, 12) and 4 (9, 12);
    - the four terminals: (6, 9), (12, 9), (6, 13) and (12, 13).
  - Hidden items (Bibliotheque_Sc02): the Moon Stone (1, 18) behind the west rack, the Sun Stone (24, 18) behind the east one.
  - The Archives (x 17–25, y 2–7) are behind a partition with a door at x 19–20, y 8–9.
- **MangaSquare_PokemonCenter.** Header `MUS_POKE_CENTER`. `on_transition` sets the respawn point. Warps 0–1: the mat (5–6, 8). Objects:
  1. the nurse at (6, 2);
  2. the fan (POKEFAN_M) at (4, 6).
- **MangaSquare_Mart.** Header `MUS_POKE_MART`. Warps 0–1: the mat (4–5, 8). Object 1: the clerk at (1, 3).
- **MangaSquare_Toiletteur.**
  - Header: `MUS_LILYCOVE`, the town's music, as Slateport's Fan Club plays Slateport's.
  - Warps 0–1: the mat (4–5, 8).
  - Objects:
    1. the groomer (WOMAN_5) at (5, 3), script `MangaSquare_Sc04` (the source's Manga_Square_Sc04 runs here);
    2. a Skitty at (6, 5).
- **MangaSquare_Gym.**
  - Header: `MUS_GYM`, `MAP_TYPE_TOWN` (open air), `MAP_BATTLE_SCENE_GYM`.
  - `on_load` `MangaSquare_Gym_OnLoad` opens each shoji door whose `FLAG_SIL_GYM06_PORTE_n` is set. P1 is (10, 20), P2 (10, 16), P3 (10, 12), P4 (10, 8) and P5 (10, 4).
  - Warp 0: the south arrow (10, 29), to the Dojo's warp 1.
  - Objects:
    1. Tak (BRAWLY) at (10, 3), `_Sc03`;
    2. Musashi (EXPERT_M) in the alley's east niche (11, 25), `_Sc01`;
    3. Kojiro (EXPERT_M) in the west niche (9, 22), `_Sc01`;
    4. Ayame (EXPERT_F) at (14, 10), sight 4;
    5. Seiji (PSYCHIC_M) at (7, 6), sight 3;
    6. the guide (MAN_2) at (8, 28).
  - Coord events: `_Sc01` on (10, 25) and (10, 22), keyed on `VAR_TEMP_1` 0. The script returns once the samurai's trainer flag is set.
  - Signs: `_Sc02` on the five desks (9, 21), (9, 17), (9, 13), (9, 9) and (9, 5). The script picks the riddle by the row.
  - Terrain, south to north:
    - the entrance row (y 28);
    - the Allée des Regards, a one-cell alley at x 10 between hedges (y 21–27);
    - P1;
    - the carp pond, crossed by a paved causeway (x 9–11, y 17–19);
    - P2;
    - gravel with a rock;
    - P3;
    - the Kunoichi's corridor of slim trees;
    - P4;
    - Seiji's gravel;
    - P5;
    - the Champion's garden: two cherry trees (General's big tree) and the dragon boulder.

**Metatiles for `set_metatile`.**
- `METATILE_SHOJI_OUVERT` (gym doors): General 265, the paving, collision 0, at (10, y) for y in 4, 8, 12, 16 and 20.
- `METATILE_BAMBOU_OUVERT`: the bamboo curtain on Route S6 (23, 26) is General 198, a tree trunk. It opens to General 1, the route's ground, collision 0.

**Script contract** (names for `crates/content/src/silice/scripts/ch06_*.rs`, following the Ch0 rules):
- MangaSquare:
  - scenes: `MangaSquare_Sc01`, `_Sc02`, `_Sc03`, `_Sc04` (the Toiletteur's groomer);
  - `_OnTransition`;
  - `_EventScript_{Cosplayeur, Puriste, Mere, Ancien, Livreur, Gamin, Controleur, SignVille, SignStatue, SignGrandTorii, SignEcran}`.
- RouteS06:
  - scenes: `RouteS06_Sc02`, and Route_S06_Sc01, which `_OnTransition` must call on the first entry. The source keys it on "first entry" without a story value, so no coord or on-frame event carries it;
  - `_OnLoad`, `_OnTransition`;
  - `_EventScript_{Luna, Hoshi, Yumi, Gaston, Kotaro, Pelerine, Coureur, Bonze, Rochers, Rocher, ItemHyperPotion, ItemUltraBall, ItemPpUp, ItemMaxEther, ItemDragonScale, SignEntree, SignEncens, SignRocher, SignAutel}`.
- RouteS06_BoisSoupirs: `RouteS06_BoisSoupirs_Sc01`, `_EventScript_{Armand, Amoureux, ItemHeartScale}`.
- MangaSquare_Dojo:
  - scenes: `_Sc01`–`_Sc04`;
  - `_OnTransition`;
  - `_EventScript_{ShinobiGear, Restauratrice, Cosplay1, Cosplay2, Kevin, Bruno, Diane, Kimiko, Moine, SignCode}`.
- MangaSquare_Bibliotheque: `_Sc01`, `_EventScript_{Archiviste, Prestataire, Etudiant, Livre1, Livre2, Livre3, Livre4, Borne}`.
- MangaSquare_PokemonCenter: `_OnTransition`, `_EventScript_{Nurse, Fan}`.
- MangaSquare_Mart: `_EventScript_Clerk`.
- MangaSquare_Toiletteur: `_EventScript_Skitty` (the groomer is `MangaSquare_Sc04`).
- MangaSquare_Gym: `_Sc01`, `_Sc02`, `_Sc03`, `_OnLoad`, `_EventScript_{Ayame, Seiji, Guide}`.

**Pending links** (not made: other chapters' maps).
- **MangaSquare north → `MAP_ROUTE_S07` (Ch7).** The Grand Torii's passage (x 36–38, y 1–5) ends at forest on row 0. Add `games/silice/edits/MangaSquare/05_ch7_connect.ron`:
  - `Ascii(x: 37, y: 0, rows: [":"], legend: {':': (metatile: 265, collision: 0)})`;
  - `Connect((dir: North, offset: 37 - X, map: "RouteS07"))`, where X is RouteS07's south-exit column;
  - RouteS07 mirrors it with South, offset `X - 37`.

  The controleur at (37, 1) blocks the way until Sc02.
- **RouteS06 south → `MAP_OWLYS` (Ch5).** The path reaches (12–14, 62), and row 63 is rock. Add `RouteS06/05_ch5_connect.ron`:
  - `Ascii(x: 12, y: 63, rows: ["..."], legend: {'.': (metatile: 1, collision: 0)})`, Facility's path;
  - `Connect((dir: South, offset: 12 - X, map: "Owlys"))`, where X is the left column of Owlys's north exit;
  - Owlys mirrors it with North.
- **Houses 1 and 2.** When `MAP_MANGA_SQUARE_HOUSE_1`/`_2` get content, add warps on (3, 21) and (7, 21). Those doors are solid now.

**Changes from the source.**
- The town:
  - The gym has no door on the town, as the source says: it is reached through the Dojo's back door.
  - The Bibliothèque's door is the pagoda's (22, 7), not y 6.
  - The shortcut arrives at (1, 25) in the west forest wall.
- Route S6:
  - The fog covers the whole route. The source wants the upper half only, which a header cannot say.
  - The curtain and tunnel are at (23–25, 26), on the plateau's east side (the source says y ≈ 22).
- The Dojo's back door is at (27, 2), behind the monk.
- The gym:
  - The source's wooden bridge over the pond is a paved causeway. The Frontier pair has no wooden bridge, and Fortree's would need another secondary.
  - The samurai stand in niches either side of the alley, so the alley stays one cell wide.
- The Bibliothèque's coord row is y 7, the first free row before Adrien's desk (y 5–6).

**Missing graphics and mechanics.**
- Graphics:
  - petal weather (`WEATHER_PETALS`, which needs engine work) on the town and the Bois des Soupirs;
  - torii (the Arena's gate stands in);
  - bamboo (slim trees, and a tree trunk for the curtain);
  - paper lanterns (Mt. Pyre's markers);
  - a wooden bridge in the gym (paving);
  - the library's dark wood and ladders (Lab's white shelves);
  - Benou, Kimiko, Tak, the drones (WINGULL), the samurai, Kunoichi and Moine (Expert and Psychic placeholders);
  - koto, taiko and flute music.
- Mechanics:
  - Rock Smash and `FLAG_SIL_MODULE_ECLATE_ROC` (the rocks are `BREAKABLE_ROCK` objects whose script tests the flag);
  - the dawn tint;
  - the samurai double battle (multi);
  - `show_book`;
  - the servers' hum (`play_se` `SE_SERVEURS`);
  - add_friendship and money for the groomer;
  - Mega Evolution (Charizardite Y).

**Editor friction.**
1. **`ascii_op` legends must not clash.** Big multi-cell objects (trees, boulders, fences) ran out of letters: a big tree's `F` hit the fence's `F`, and the Bibliothèque's `G`/`H` clashed. Unicode keys were refused or mis-aligned. A legend keyed by short names, or a `Place(x, y, metatiles: [[…]])` op, would avoid it.
2. **No elevation in `Ascii` without a full triple.** My helper first wrote two-field legends. Water then kept the land's elevation 3, and `validate` flagged it only through collision. A warning for water at elevation ≠ 1 would catch it.
3. **Metatile meaning is guessed from renders.** Lab 573–575 and 640 are floor shadows, not furniture: I learned it from the "walkable in 5 of 5" warnings after the first build. `palette` could mark metatiles whose extracted cells are always walkable.
4. **The ids importer reads only headings.** `MAP_MANGA_SQUARE_GYM` had to be appended by hand. The import also rewrites `text/` (the Ch09 rename), so it must run on a scratch copy and only `ids/flags.ron` may come back.
5. **Worktree guard.** Shell variables, `cd` before `git`, and multi-step pipelines are refused. Helper scripts went through `Write` and plain `python3 -I`.

#### SIL1 Task A as built: the authored-data loader

`build/silice/` loads all of `games/silice` into the generator's `World`. The generator sees Silice only through `input::AuthoredIn`, which gained `map_numbers`, `item_bases`, `rules` and `data_rs`.

**What loads (real data).** 6 maps, 1923 texts, 238 trainers, 39 encounter maps, 69 items and 13 marts. The first line of `content::REPORT`'s data part gives these counts; the line after it lists every placeholder and cut.

**Modules.**
- `text.rs`: reads `text/fr.ron` with `parse_texts`, which fails on a repeated id. The `Encoder` runs the extract codec (FR, with « » from `FR_OVERLAY`) plus the glyph hook `text::glyph_codes() -> Vec<(char, Vec<u8>)>` (G's `font::GLYPHS`). A hook char is written as its bytes, one `{0xNN}` each (one byte, or two for a `{EMOJI_HEART}`); this equals Task G's `font::encode`. Text constants are `TextId`s named after the `fr.ron` ids. The Gen3 `gText_*` texts stay, and a collision with them fails.
- `data.rs`:
  - **Trainers** (`battle/trainers.ron`): party, levels, IVs and custom moves (1–4, padded with `MOVE_NONE`), plus held items, AI flags, class and pic. Only the pics in use are compiled, and they must be in the extract. Trainers are numbered by `ids/trainers.ron`; a tombstone or gap is an empty `TRAINER_SIL_NONE_<n>` with a report line. `TRAINER_SIL_*` are `TrainerId`s. The Emerald trainer table goes, except `TRAINER_NONE`.
  - **Encounters** (`battle/encounters.ron`): `wild_encounters`, keyed by Silice map (numbered by `ids/maps.ron`), with slot counts 12/5/5/10.
  - **Items** (`items.ron`): each `ITEM_SIL_*` sits at its `ids/items.ron` number, after Gen3's. It has a name, a price, and a pocket that falls back to the base item's pocket. Its hold effect comes from `clone_of` (`AuthoredIn::item_bases`). Key items get importance 1. `ITEM_SIL_*` are `u16`.
  - **Marts**: `content::data::MARTS: [Mart { map, coins, stock: &[(item, price)] }]`.
- `load.rs`: checks the Gen3 map limits. Any breach is a build error:
  - 64 objects, with local ids 1–63, unique;
  - unique, prefixed `LOCALID_*` names;
  - 255 warps, coord events and bg events each;
  - `MAX_MAP_DATA_SIZE`, the block count, and a 2x2 border;
  - events in bounds, with elevation ≤ 15;
  - each warp's `dest_warp` within the destination map's warps.
  
  It also loads `check-exceptions.ron` (`Exceptions::find(rule, subject)`, with globs).
- `merge.rs::header`: resolves the header names:
  - `MUS_*`/`SE_*` through `ids/songs.ron`, then the Gen3 sounds;
  - `MAPSEC_*` through the Gen3 names (`MAPSEC_NONE` stays);
  - `WEATHER_*`, `MAP_TYPE_*` and `MAP_BATTLE_SCENE_*` through `extract::games`'s tables, which moved there from the studio.
  
  An unknown name fails. `localid_consts` emits each `LOCALID_*` as `u8` (the local id).
- `mod.rs::world`: the build checks that every tileset and object sprite is in the extract. An unknown tileset fails.
- **Battle rules.** `classify::Rules` turns on the trainer held items, the Gen3 extra abilities and the Silice ability rules per flavour. Silice turns on all three, and Emerald's output is unchanged.

**Placeholders and cuts.** Each needs an exception in `check-exceptions.ron` and gets a report line. Without the exception, the build fails.
- `text-charmap`: an unencodable char becomes `?`. About 108 texts are hit, by — ° _ [ ] ♥ ₽ ●○ ♪ # * ←→, many of which G's glyphs will cover.
- `trainer-name` (10 bytes) and `trainer-class-name` (12 bytes): 2 trainer names get `?`, and the names below are truncated.
- `item-name-length` (13 bytes) and `item-incomplete`: a missing name becomes the id's words, and a missing pocket becomes Items.
- 34 truncations in all, among them trainer names such as TAUBER&HOC and CONTREMAÎT, class names such as TOP DRESSEUS, and item names such as Mewtwoïte X e.

**Form changes.** None are emitted. Five form species are named (the Mega Alakazam, Charizard X/Y, Gyarados and Steelix), but none has battle data and no `items.ron` stone maps to a form. Both are needed: the forms' base stats, types and abilities, and each stone's `(base species, form)`.

**After the merge with Tasks E and G.**
- `text::glyph_codes()` returns G's `font::GLYPHS`: ♥ as `{EMOJI_HEART}` (F9 E7), · as 0xAF. The encoder rejects Japanese characters, as `font::encode` does.
- `world` runs E's `game::load`/`game::check` after the map limits, and G's `font::merge` right after `load_assets`.
- From E's review:
  - `data::NEW_GAME` is now `(MapId, i16, i16, Dir)`, and `game_data_tables!` reads it.
  - `game.ron` has a `player_name`, the bible's NOOBODY, 1 to 7 characters. It is passed through `AuthoredIn::player_name` to `data::PLAYER_NAME`.
  - The start faces up, onto the bar counter, and the Down-only check is gone.
- Emerald keeps `Dir::Down` and SACHA, and its `new_game` digest input is unchanged.

**Texts renamed (provisional, reversible).** The ch09 `Tour_Sbire1_Intro` and `Tour_Sbire2_Intro` are now `Tour_Sbire1_Intro_Ch09` and `Tour_Sbire2_Intro_Ch09`, in `text/fr.ron` and `text/speakers.ron`. The `text-duplicate` exceptions now name those ids and keep `ruling_needed`. `silice check` flags a `<id>_ChNN` whose `<id>` exists, so the rename stays visible. See open question 6.

**Tests.**
- `crates/content/tests/sil1_a_loader.rs` builds a fixture extract (the stub world plus one trainer pic and the item table) and fixture games dirs. It checks every data kind, a byte-identical rebuild, and these failures:
  - an unknown tileset or script;
  - a repeated text id;
  - a charmap error, a name over its limit and an incomplete item, each with and without its exception;
  - the four map-limit kinds and an unknown song.
- A nested `cargo check` with `PKR_SILICE_DIR` fails on a fixture with an unknown tileset.
- With a real extract, the committed dir loads with one constant per `LOCALID`, trainer, item and text.
- Under `cfg(pkr_silice)`, every trainer's party and held items, the encounters, the items' prices and hold effects, `MARTS` and the texts are checked against the RON files.

#### SIL1 Task D as built: flavour plumbing in the shells

`PKR_GAME=silice` now builds and tests the whole workspace (game-native, game-gba, explore, studio, xtask), and every xtask command takes `--game`. Route-neutral for Emerald: only `BUILD_ID` moves (`build/main.rs` gained the metadata line), so the Emerald trees still read `valid (rehash)`, and `cargo xtask parity` is identical.

Commits:
- `feat(content): tell the shells the built flavour (links metadata, cfg(pkr_silice) in their build scripts)`
- `feat(explore): per-flavour acts and a flavour-keyed db (Db::open takes a Flavor, refuses another flavour's db)`
- `feat(shells): Silice boot route and lane, gate the Emerald-only shell code and tests by flavour`
- `feat(xtask): --game emerald|silice for every command, provisional Silice ROM budget`
- the merge of master (M8 Task M), then `fix(shells): gate Task M's after-starter tour and committed-node ROM test by flavour, re-measure the Silice ROM`

**How the shells see the flavour.**
- `content` declares `links = "pkr_content"`, and its build script prints `cargo::metadata=flavor=<stub|emerald|silice>` (the flavour actually built, so `PKR_GAME=silice` with an explicit `PKR_EXTRACTED_DIR=/nonexistent` is the stub).
- `game-native`, `explore`, `studio`, `xtask` and `game-gba` each have a `build.rs` that `include!`s `crates/content/flavor_cfg.rs`: it reads `DEP_PKR_CONTENT_FLAVOR` and sets the crate's own `cfg(pkr_silice)`. Each of the five asserts in a `const` that the cfg agrees with `content::FLAVOR`, so a stale cfg cannot compile.
- Staleness: cargo cannot watch a `DEP_*` variable with `rerun-if-env-changed`. The shell's script reruns because cargo fingerprints a build-script run with the runs of its `links` dependencies, so it reruns whenever `content`'s does (a `PKR_GAME` change among others). It prints `rerun-if-changed=build.rs` so that it does not rerun on every other file change. Checked by building Emerald, then Silice, then Emerald again: the `const` checks hold each time.
- `emu` sees `content` only as a dev-dependency (no `DEP_*`), so its tests use runtime checks plus `game_native::scenario::after_starter()`.
- Rule: Emerald-only code that names Hoenn constants or `Game::after_starter` is `cfg(not(pkr_silice))`. Everything else branches on `content::FLAVOR` at run time. `content::STUB` is left only where the question is "is there a game at all".

**Gated or generic.**
- `game_native::scenario`:
  - `route()` matches on `FLAVOR`. `SILICE_ROUTE` is the boot lane: a new game left idle, with moments and nodes `silice_start` (frame 24), `silice_idle` (600) and `silice_end` (1500). The tail makes the last node's trace longer than a seek keyframe interval, so the seek, materialize and restore tests run on Silice too. No Silice node is committed.
  - `after_starter() -> Option<Game>` is `Some(Game::after_starter())` on Emerald and the stub, `None` on Silice. The sweep, the tour test and the trainer-sighting lane use it.
  - The Emerald route assertions moved to a `cfg(not(pkr_silice))` helper.
- `game-gba`: the tour build plays `Game::for_shells()` on Silice (`cfg(any(not(feature = "tour"), pkr_silice))`).
- `explore`:
  - `goals::emerald` and `goals::constants` are `cfg(not(pkr_silice))`.
  - `key::EXCLUDED_VARS` has a Silice list: `VAR_DAYS` and `VAR_OBJ_GFX_ID_0..F`, the shared Gen3 vars that are not story progress. Silice has no Emerald counters; their numbers may be Silice ids.
  - **Acts per flavour.**
    - `Act::of(flavor)` gives `EMERALD` for the stub and Emerald, `SILICE` for Silice. `Act::in_build` and `Act::parse_in_build` use it.
    - `Act::default()` is `Act1` on Emerald and the stub, `SilCh0` on Silice.
    - `goals_of` gives no goal for another flavour's act.
    - `run --act`, a config file's `act` and a db's `meta.act` are refused when they belong to another flavour. The message names the flavour and the `--game` to use. `sil_ch0` goals stay empty until Wave 3.
  - `Db::open(path, build, Flavor)` and `open_in_memory(build, Flavor)` record `Flavor::name()` in `meta.flavor`. Those names are the old `stub` and `emerald`, so dbs made before Task D open as before (`pre_task_d_dbs_still_open`). A db that records no flavour is refused with the `explore reset --yes` hint. A db made by another flavour is refused with `DbError::Flavor`, whose message names both flavours and says how to build the db's (`PKR_GAME=silice` / `--game silice`, `PKR_EXTRACTED_DIR=/nonexistent`, or `PKR_GAME` unset).
- `studio`:
  - `Catalog::from_content` pairs Silice's tilesets through `games/silice` layouts (`map_edit::catalog::silice_dir()`, which honours `PKR_SILICE_DIR`).
  - The map picker's full-import check is Emerald's only.
- xtask:
  - The sweep starts on `after_starter()` or the new game.
  - `parity` and `sweep` require decomp names on Emerald only.
  - `rom` picks `budget::Content::Silice`.
- **Tests.**
  - Whole files that are Emerald's only and now `cfg(not(pkr_silice))`: `explore/tests/{b_goals, m8_k_goals}.rs`.
  - Single Emerald-only tests that are `cfg(not(pkr_silice))`:
    - in `explore/tests`, the `real_*` tests in `a_expand`, `h_tuning`'s Route 101 softlock, `b_key`'s after-starter and counter tests, and `f_policy`'s battle and starter tests;
    - `game-native/tests/seek`'s battle seek;
    - `studio/tests/native`'s route-event and committed-node tests, and `studio` `rom::tests::node_boots_restored_at_the_node_frame`.
  - Runtime skips on non-Emerald: `e2e_real`, `m8_i_emote`, `e_panic`'s event-script branch, `b_fitness`'s story-flag check, the map editor's Emerald-map renders (`emerald()` in `studio/tests/map_edit.rs`), and the two-map check in `map_pixels`.
  - `m8_k_db`'s 40% blob ratio is required on the stub and Emerald only. Silice measured 43%, and there it only has to beat the text codec.
  - New: `explore/tests/sil1_d_flavor.rs` (acts per flavour, a foreign act refused before any db exists, every flavour pair of dbs refused) and the xtask `game` and `budget` unit tests.

**`--game` (`xtask/src/game.rs`).**
- `--game emerald|silice` is accepted anywhere on the command line, except after `studio map`, whose own `--game NAME` picks a games dir; there it goes before `studio`.
- It covers every command: `rom`, `parity` (and `--long`), `sweep`, `checkpoints` (`checkpoints/silice/`), `studio`, `explore`, `shot`, and the budget, which is part of `rom`.
- **When the running xtask is already that flavour,** it only sets `PKR_GAME` for the child cargo invocations. Silice sets it; Emerald unsets it, so that plain builds keep their fingerprints.
- **Otherwise it re-runs itself** through `cargo run -p xtask` under the new `PKR_GAME`, once (`PKR_XTASK_GAME`). It fails with a clear message if the content still is not that flavour (Silice without an extract).
- **Target dirs.** A flavour switch in one target dir rebuilds `content` and everything above it. So the Silice re-run builds in `target/silice` (unless `CARGO_TARGET_DIR` is set), and its child cargo invocations inherit that. The GBA build of a Silice xtask uses `crates/game-gba/target/silice`. Once each side is built, alternating `cargo xtask parity` and `cargo xtask parity --game silice` rebuilds nothing; `rom --game silice` takes about 2 s. A plain `PKR_GAME=silice cargo …` still shares `target/` with Emerald (pass `CARGO_TARGET_DIR=target/silice` to share the xtask's).
- The re-run clears `PKR_XTASK_GAME` on start, so the commands' own child processes do not inherit it.

**Silice ROM and budget.**
- `cargo xtask rom --game silice` builds a 3,966,784-byte ROM (3,873 KiB) after the merges of Tasks M, E, N and G; it was 4,103,320 bytes before Task M and 3,965,600 bytes after it. IWRAM is 788 bytes and EWRAM 492 bytes, the shared gates.
- After the merge of SIL1 Task A (the authored data), the ROM is 4,172,472 bytes (+205,688). `SILICE_ROM_MEASURED` records this figure.
- `budget::SILICE_ROM_LIMIT` is 8 MiB, provisional; `SILICE_ROM_MEASURED` records the measurement and `check` prints the delta. **SIL1 Wave 2 re-baselines it.**
- The tour ROM (`parity --game silice --long`) is 3,970,344 bytes (3,969,160 after Task M).

**Boot lane.** `cargo xtask parity --game silice`:
- `checkpoints/silice` verifies with 0 nodes;
- `rom_and_native_agree` on `SILICE_ROUTE`: 1,500 hashes and 18 pictures identical, no late frame. Loop 68 lines (gate 217), hash 11 lines, on the Task E new game (the bar). `emu::check_memory`: heap max 27,096 B, stack untouched 11,716 B. 0 reports, 0 events dropped;
- the probes pass;
- the resume, save and mid-battle-save lanes print a skip (`boot_lane_only`), and so does the trainer-sighting lane;
- the studio ROM pane's new-game tests pass (the spawned-pane test walks south on Silice: a customer stands west of the bar start);
- `--long`: the Silice tour passes, 0 reports and 0 events dropped.

`cargo xtask sweep --game silice`: 6/6 maps pass, with no panic, no round-trip failure and no dropped event.

**Gates as run (after the merges of master with Tasks M, E, N and G).** Emerald parity: 19,176 hashes and 205 pictures identical, ROM 9,294,440 bytes. Task N's new explore tests take `content::FLAVOR` for `Db::open`, and their Emerald-only checks (Littleroot scripts, real trainers, Act 1 progress, Petalburg Woods warps) skip off Emerald.
- **Stub** (`PKR_EXTRACTED_DIR=/nonexistent`): `cargo test --workspace` is green, and so is clippy `--all-targets`.
- **Emerald:**
  - `cargo test --workspace` is green;
  - clippy is clean plain and with `--features game-native/window,studio/ui`;
  - `cargo xtask parity` is identical (19,176 hashes and 205 pictures, every lane), and its ROM is 9,293,728 bytes;
  - the trees read `valid (rehash)`, which was not accepted.
- **Silice:** `PKR_GAME=silice cargo test --workspace` (the whole workspace) is green, and clippy is clean with `--all-targets` and with `--features game-native/window,studio/ui`. `parity --game silice [--long]`, `rom --game silice` and `checkpoints verify --game silice` pass as above.
- fmt is clean.

**Left for later tasks.**
- Wave 3: the Silice goals (`goals::silice::CH0`), the `sil:*` nodes and their resume lanes. Ungate the tests above once Silice's new game has a party and the route has battles.
- Task E: once the new game sets story flags, `b_fitness` can check them on Silice too.
- The studio explorer pane names goals of the default act, not the db's `meta.act`. This predates Task D.

#### SIL1 Task E as built: `game.ron`, the Silice root, the new game and the prelude

Commits:
- `feat(silice): load game.ron (start, system ids, heal locations, starters, new-game flags) in the build`
- `feat(content): Silice root, new game and prelude helpers from game.ron`
- `feat(xtask): silice check knows game.ron's heal locations`
- a merge of master (M8 Task M) and `docs(plan): SIL1 Task E as built`

Emerald's `BUILD_ID` does not move: every file touched is Silice-only (`src/silice/`, `build/silice/`, `extract/src/games/`, `games/silice/`), a test, or xtask. `content/src/lib.rs` is untouched.

**`games/silice/game.ron`** (format `extract::games::GameFile`, in `extract/src/games/game.rs`; hand-written, never imported):

| Field | Value | Why |
|---|---|---|
| `start` | `BricoBourg_Bar` (7, 4), `Down` | Bar_Sc01 opens the game in the Shaker. The source's P (7, 8) is "the free stool in front of the counter"; the built bar's stools are y 4, between Gwada (6, 4) and Adrien (9, 4). |
| `running_flag` | `FLAG_SIL_RUNNING_SHOES` | Player_House_Sc03: Maman gives the shoes as a flag. |
| `cleared_on_map_change` | `FLAG_SYS_ENC_UP_ITEM`, `_ENC_DOWN_ITEM`, `_USE_STRENGTH`, `_CTRL_OBJ_DELETE` | Gen3 `ClearTempFieldEventData` minus the Union Room nurse reminder. |
| `heal_locations` | 1: `HEAL_LOCATION_SIL_BRICO_BOURG_PLAYER_HOUSE`, `BricoBourg_PlayerHouse_2F` (4, 2) | As Emerald's `..._BRENDANS_HOUSE_2F` (4, 2); "set_respawn on the house at the start". Append-only, numbered from 1. |
| `respawn` | that heal location | |
| `starters`, `starter_level` | Chikorita, Cyndaquil, Totodile; 5 | Brico_Bourg_Sc03 (D-18); index = `VAR_SIL_STARTER`. |
| `new_game_flags` | `FLAG_SIL_HIDE_BAR_SBIRE_A`, `_B`, `_BAR_MALLETTE`, `_BRICO_SBIRE`, `_BRICO_MATHIEU`, `_GWADA`, `_ADRIEN`, `_BRICO_LINGE`, `_GAMIN`, `_BRICOLEUSE` | Hidden at story 0: shown by a scene (sbires, Mallette), in front of the house at story 3–4 (friends), or by day at story ≥ 3 (day NPCs; `BricoBourg_OnTransition` still owns them after that). The bar's friends and Julien start visible. |
| `new_game_vars` | none | Every var starts at 0. |

No new ids were allocated: every name already exists in `ids/`. Heal locations have no ids kind; `game.ron`'s list is their append-only allocation.

**Build** (`build/silice/game.rs`; three touch points in `build/silice/mod.rs`: `pub mod game`, load and check after the maps, the consts and start into `AuthoredIn`). `START` is gone.
- Checks: start and heal cells are walkable cells of loaded maps; facing is `Down`; flags and vars are live Silice ids, or Gen3 names in `SHARED_GEN3`; heal locations are `HEAL_LOCATION_SIL_*` and unique; the respawn is one of them; three Gen3 starters; level 1–100.
- **Facing is `Down` only**: the shared `game_data_tables!` `new_game` hardcodes it, and reading another facing would change `lib.rs` and `codegen.rs`, which Emerald hashes. Bar_Sc01 can `ctx.turn(PLAYER, Dir::Up)` if the scene wants the player facing the counter.
- Emitted as `content::consts`, each naming the constant it stands for, so a renamed id fails to compile: `HEAL_LOCATION_SIL_*: u8`, `SIL_HEAL_LOCATIONS`, `SIL_RESPAWN`, `SIL_START_FACING`, `SIL_RUNNING_FLAG`, `SIL_FLAGS_CLEARED_ON_MAP_CHANGE`, `SIL_STARTERS`, `SIL_STARTER_LEVEL`, `SIL_NEW_GAME_FLAGS`, `SIL_NEW_GAME_VARS`.
- `game.ron` joins the `SHARED_GEN3` scan (`tests/sil1_w0_contracts.rs`).
- xtask `silice check` adds `game.ron`'s heal location names to its known names, so a script naming `HEAL_LOCATION_SIL_BRICO_BOURG_PLAYER_HOUSE` is not a `sil-prefix` error.

**Root** (`src/silice/mod.rs`): `special_ids` takes `flag_running`, `flags_cleared_on_map_change` and `respawn_default` from `game.ron` over `..gen3_special_ids()` (so M8 Task M's `max_money`, `multichoice_cancel` and `pocket_slots` still reach Silice); no whiteout script, no wild-set var. `heal_location(n)` is `SIL_HEAL_LOCATIONS[n - 1]`. The new-game script (`new_game.rs`) sets `SIL_NEW_GAME_FLAGS` and `SIL_NEW_GAME_VARS`.

**Prelude API for Task F** (`crate::silice::prelude`, `use crate::silice::prelude::*;`). It re-exports the Gen3 prelude (`msgbox`/`MsgBox`, `give_item`, `add_item`, `find_item`, `trainer_battle`, `trainer_flag`, `buffer`, `multichoice`, `text_named`, every generated constant). Every helper takes `&Ctx`.

| Helper | Does |
|---|---|
| `story(&ctx) -> u16`, `set_story(&ctx, n)` | `VAR_SIL_STORY` |
| `defeated(&ctx, TrainerId) -> bool` | the trainer's beaten flag (`0x500 + id`) |
| `gift(&ctx, flag, item, qty, Obtain::Message \| Obtain::Quiet) -> bool` | nothing if `flag` is set; else `give_item` (Gen3 messages) or `add_item` (silent: the scene's text says it), and sets `flag` when it fit |
| `say(&ctx, T)` | one line of a locked scene (`message`, wait, button) |
| `ask(&ctx, T) -> bool` | `msgbox(T, YesNo)` then the answer |
| `walk(Dir)`, `walk_in_place(Dir)`, `face(Dir)` | `const fn` step builders, for `const PATH: &[Step] = &[walk(Dir::Up), …]` |
| `move_wait(&ctx, obj, PATH)` | `move_obj` then `wait_move` |
| `show(&ctx, obj, hide_flag)` | the source's `show_object`: clear the hide flag, `add_object` |
| `hide(&ctx, obj)` | the source's `hide_object`: `remove_object`, which sets the hide flag |
| `warp(&ctx, MAP, x, y)` | warp to a cell; the map load ends the script |
| `choose_starter(&ctx) -> u8` | starter screen on `SIL_STARTERS`, `VAR_SIL_STARTER` = index, the mon at level 5, no battle |
| `battle_wild(&ctx, SPECIES, level) -> BattleOutcome` | an ordinary wild battle (run and catch allowed) |
| `fallback(&ctx, "name")` | `Report::Degraded(name)`, then the scene goes on |
| `screen_shake(&ctx, mag, frames)` | fallback, then a fade to black and back |
| `play_se(&ctx, "SE_…")` | fallback, and logs the name; nothing plays |
| `mallette_call(&ctx, "WHO")`, `end_call(&ctx)` | fallback; the call's lines are plain `say`s |

**Pseudo-script primitives in the Ch0 scenes (§3), with their mapping:**

| Primitive (uses) | Mapping |
|---|---|
| `lock`, `lock_all`, `release`, `release_all`, `face_player` | direct: `ctx.lock()` and so on |
| `msgbox(T, Default)` | `say` |
| `msgbox(T, Npc)` | inside a locked scene: `say`. Gen3's `MsgBox::Npc` locks, turns the talked-to object and *releases*, so use it only for a lone NPC line (Bar_Sc02). |
| `msgbox(T, YesNo)` + `if !yes_no` | `ask` |
| `msgbox(A \| B \| C, Npc)` (Bar_Sc02) | `ctx.var(VAR_LAST_TALKED)` picks the text, then `msgbox(…, MsgBox::Npc)` |
| `move_obj` + `wait_move` (16 and 9 uses) | `move_wait`, or `ctx.move_obj` per object then one `ctx.wait_move`; `Walk(D) xN` is N `walk(D)` |
| `Face(D)`, `FacePlayer`, `WalkInPlace` | `face(D)`, `Step::FacePlayer`, `walk_in_place(D)` |
| `show_object` (4) / `hide_object` (8) | `show(obj, FLAG_SIL_HIDE_…)` / `hide(obj)` |
| `set_flag(F)` / `flag(F)` | direct: `ctx.set_flag(F, true)` / `ctx.flag(F)` |
| `set_var(VAR_SIL_STORY, n)` / conditions on it | `set_story` / `story` (other vars: `ctx.set_var`, `ctx.var`) |
| `fade(In)` / `fade(Out)` | direct: `ctx.fade(Fade::FromBlack)` / `ctx.fade(Fade::ToBlack)` |
| `delay(n)` | direct: `ctx.delay(n)` |
| `heal_party` | direct: `ctx.heal_party()` |
| `give_item(ITEM, n)` (3) | `gift(FLAG_SIL_ITEM_…, ITEM, n, Quiet)` where the scene's text announces it (Sc03 T11/T12); `ITEM_SIL_MALLETTE` in Bar_Sc04 with `FLAG_SIL_HAS_MALLETTE` and T3 |
| `let i = choose_starter([...])` + `set_var(VAR_SIL_STARTER, i)` | `choose_starter` (does both) |
| `battle_wild(SPECIES, lvl)` | `battle_wild` |
| `call(Brico_Bourg_Sc03)` | `BricoBourg_Sc03(ctx.clone()).await` |
| `return` | `return` |
| `screen_shake` (2) | fallback: `screen_shake` (fade). For Sc02's small shake, `fallback(&ctx, "screen_shake")` and `ctx.delay(10)` is closer to the source's own fallback. |
| `play_se` (3) | fallback: `play_se` (nothing plays) |
| `mallette_call` / `end_call` | fallback: `mallette_call`, `end_call` |
| `set_respawn` (§2, at the start) | not needed: `game.ron`'s respawn is the house from the new game. Direct `ctx.set_respawn(HEAL_LOCATION_SIL_…)` if a scene changes it. |
| `set_time_tint(NUIT)` (§9) | missing, no call: the maps leave the night tint out |
| `field_move` / brambles (§9) | `if ctx.flag(FLAG_SIL_MODULE_COUPE)` then `ctx.set_metatile` (Ch1) |

**Tests** (no French literals). `src/silice/game_tests.rs`:
- a new game lands on `game.ron`'s start cell and facing;
- the new game sets exactly `game.ron`'s flags, and nothing of the story (story 0, no shoes, no party; the bar's friends and Julien visible);
- `special_ids` and `heal_location` are `game.ron`'s, and the respawn cell is walkable;
- `choose_starter` gives each `game.ron` species at its level, sets `VAR_SIL_STARTER`, starts no battle, and maps an out-of-range answer to the first starter;
- `fallback` and the fallback helpers report `Degraded` and the script runs to its end;
- `story`, `defeated`, `gift` (once, quiet, a full bag retries) and the scene helpers' requests;
- the Gen3 std texts and pocket strings the shared prelude prints are in the build.

`tests/sil1_e_game.rs` checks the committed `game.ron` and each rule on a broken copy; `xtask/tests/silice_check.rs` checks that `game.ron`'s heal locations are known names.

**What F needs to know.**
- Bar_Sc01 runs on frame 1 of a new game: the player stands at (7, 4) facing down.
- Set `VAR_SIL_CH0_GATE` 1 with the starter, and `VAR_TEMP_1` in Bar_Sc01 (both from the maps section).
- Once scene scripts exist, `silice check`'s `placement-map` rule wants every given item's flag named in a map or a script. With a scratch script, it lists `FLAG_SIL_HAS_MALLETTE`, `FLAG_SIL_ITEM_PROLOGUE_BALLS` (and `_POTIONS`), `FLAG_SIL_ITEM_ROUTE_S01_ORAN`, `FLAG_SIL_RUNNING_SHOES`, and the post-game `FLAG_SIL_MAIL_SILAS`, which needs an exception until Ch12.
- `ITEM_SIL_MALLETTE` and the texts are Task A's constants.

**Gates:** fmt; clippy plain, stub, `game-native/window,studio/ui` and `PKR_GAME=silice -p content -p engine -p extract`; stub and Emerald `cargo test --workspace`; `PKR_GAME=silice cargo test -p content -p engine -p extract`; `cargo xtask silice check`; `cargo xtask parity`; `checkpoints verify`. The results after the master merge are in the task report.

#### SIL1 Task G as built: the font glyphs

Commits:
- `feat(silice): author the ♥ and · glyphs and merge them into the Silice font`
- `feat(xtask): accept the authored glyphs in silice check, add the font-glyph rule`
- `docs(plan): SIL1 Task G as built`
- after review (originality): `fix(silice): redraw the heart, map ♥ to {EMOJI_HEART}, reject kana, guard against ROM glyphs`

**Finding: Gen3 already encodes both characters.** §4 item 8 counted both as missing, wrongly.
- `·` is `'·' = AF` in `charmap.txt`, and the FR font draws it at 0xAF, 3 px wide. Task B's check never flagged it.
- `♥` is Gen3's `{EMOJI_HEART}`, `F9 E7`, which the printer draws as glyph 0x1E7, 8 px wide. Only `encode_lang` lacked a character literal for it.

R-11 is met by redrawing both glyphs with original drawings: 0xAF for `·`, 0x1E7 for `♥`. No new byte is used.

The charmap has other `EMOJI_*` symbols that could answer pending rulings with the same mechanism, for example `EMOJI_UNDERSCORE`, `EMOJI_NOTE`, `EMOJI_CIRCLE` and `EMOJI_SPHERE`. That is the author's call.

**Originality (review fix).**
- The first heart was not original: it repeated the ROM's own `EMOJI_HEART` glyph (0x1E7), with one row duplicated. It was redrawn as a different shape:
  - 9 rows tall instead of 7;
  - single-pixel rounded lobe tops;
  - a notch two rows deep;
  - straight sides;
  - a three-step taper.
- `crates/content/tests/sil1_g_font.rs::authored_glyphs_are_not_rom_glyphs` now guards every authored glyph. Whenever an extract is present, it compares the glyph's foreground mask, normalized to its top-left, with every glyph of the extracted normal and small fonts. It compares the masks as drawn, and again after collapsing runs of equal rows and columns, so a ROM glyph stretched by a duplicated row also fails. The guard fails on the first heart.
- Glyphs of 4 foreground pixels or fewer are exempt, for example the 2x2 middle dot: at that size every font draws the same few shapes, so a match says nothing about copying.

**Files.**
- `crates/extract/src/games/font.rs` holds the format, the codes, the encoder hook and the metric rules. It is shared by the content build and xtask, like the rest of `extract::games`, and is left out of Emerald's `BUILD_ID`.
- `games/silice/font/heart.ron` and `games/silice/font/middle_dot.ron` are the authored glyphs, drawn by hand.
- `crates/content/build/silice/font.rs` is the build side: load, check, merge, and the hook for Task A.
- Outside the owned files, the task touched:
  - `build/silice/mod.rs`, in three places: the module doc names `font`, `pub mod font;`, and one `font::merge` call in `world` right after `load_assets`;
  - one line in `extract/src/games/mod.rs`;
  - `xtask/src/silice/check.rs`;
  - `xtask/tests/silice_check.rs`;
  - `games/silice/check-exceptions.ron`.

**Glyph format** (`games/<game>/font/<name>.ron`, one glyph per file, `GlyphFile`):
- `char` is the character the texts write.
- `width` is the advance in pixels, shadow column included (1 to 16).
- `on_baseline` says that the lowest foreground row is the font's baseline.
- `rows` holds 16 strings, from top to bottom, each exactly `width` characters long:
  - `.` is the background (2bpp 0);
  - `#` is the foreground, the text colour (1);
  - `s` is the shadow (2).
- Row 15 must be blank, because the printer draws 15 rows.
- Columns past `width` are background.
- The drawing sits on the `.latfont` 16x16 cell, so it encodes directly (`Glyph::latfont`).

**Codes** (`extract::games::font::{CODES, RESERVED}`, append-only like the ids).
- `CODES` is `&[(char, &[u8])]`: each character with its byte sequence. `glyph_index` gives the glyph a sequence draws: one byte `b` draws glyph `b`, and `F9 x` draws glyph `0x100 + x`.
  - `♥` (U+2665) is `[0xF9, 0xE7]`, `{EMOJI_HEART}`, glyph 0x1E7. It costs **2 bytes** in a text or a name.
  - `·` (U+00B7) is `[0xAF]`, glyph 0xAF.
- A character Gen3 can encode keeps its sequence: a `CHARS` byte or a `CONSTANTS` `F9` sequence. Its authored glyph redraws that glyph.
- `RESERVED`, 0x37..=0x4F, is kept for future characters Gen3 cannot encode (`—`, `°` and the others, pending author rulings). It has no entry yet.
  - These bytes have no Western character in `charmap.txt`, and the FR normal font leaves them blank. They come right after `Lv`, `=` and `;` (0x34 to 0x36).
  - Only kana share them. `encode_fr` rejects every Japanese character (U+3000–U+30FF, U+FF00–U+FFEF), so a kana can never alias an authored glyph. A unit test checks that every `CHARS` entry on a reserved byte is kana and is rejected.

**Metrics, read at build time from the extract** (`problems`, run by `font::merge`, which fails the build on any problem):
- The cell is 16x16 (`.latfont`).
- The baseline is the lowest foreground row of `A` (0xBB), row 11 in the FR font.
- No glyph inks below the baseline. An `on_baseline` glyph ends exactly on it.
- A redrawn glyph keeps the extract's width: · is 3 px and ♥ is 8 px.
- A reserved glyph must be blank in the extract. Values 0 and 3 both count as background, since the printer draws both as background.
- A sequence must be either Gen3's encoding of the character or one reserved byte.
- When any glyph file exists, every character in `CODES` must have one.
- A games dir without `font/`, such as a test fixture, merges nothing. Its texts still encode and are drawn with the extract's glyphs.

**The drawings.**
- **♥:** 7 x 9 foreground on rows 3 to 11, ending on the baseline. The shadow falls right and down. The advance is 8, the extract's width for 0x1E7.
- **·:** a 2 x 2 dot on rows 8 and 9, centred on the FR x-height (rows 6 to 11), with its shadow. The advance is 3.
  - **Side effect:** Gen3 also prints 0xAF in its own strings, for example the fishing "· · ·" wait and the PokéNav calling dots. Redrawing 0xAF moves those dots one row lower than Emerald's, in the Silice build only.
- **Small font:** not touched. `FONT_SMALL` prints only the healthbox nickname, level and HP, so dialogue never uses it.

**Hook for Task A (the text encoder).**
- Encode every Silice text with `font::encode(text)`, which is `extract::games::font::encode_fr`. It:
  - rejects Japanese characters;
  - writes each authored character as the decomp byte escape `{0xNN ...}`;
  - calls `encode_lang(.., Lang::Fr)`.
- `font::GLYPHS` is `&[(char, &[u8])]`, the **byte sequence** of each character, not one byte. `glyph_codes` can return `super::font::GLYPHS.to_vec()`.
- A per-character encoder calls `font::authored_glyph(ch) -> Option<&'static [u8]>` first, appends the whole sequence, and falls back on the charmap.
- Keep `font::merge` right after `load_assets`, before any width measurement, so widths come from the merged font.
- The three hooks carry `#[allow(dead_code)]` until A's encoder uses them.

**Validator** (`cargo xtask silice check`).
- Texts, trainer names and item names are encoded with `encode_fr`:
  - `♥` and `·` pass;
  - a name counts a heart as 2 bytes;
  - kana now fail `text-charmap`.
- `text-width` reads the authored glyphs' widths at their glyph indices, merged into the extract's widths. `F9 x` is measured at glyph `0x100 + x`.
- New rule `font-glyph` (error), on subject `font`. It fires when a glyph file:
  - does not parse;
  - has no code;
  - breaks the widths: a redrawn glyph at another width, or a sequence that is neither Gen3's nor reserved;
  - is missing for a character.

  The pixel checks (baseline, blank glyph) need the font png, so they run in the build only.
- Without `font/`, the check notes that the authored characters encode and are drawn with the extract's glyphs.
- `text-charmap` reads "no glyph (FR charmap or font/)". It still fires on `—`, `°`, `₽`, `●`, `○`, `♪`, `#`, `*`, `←`, `→`, `[`, `]` and `_`. All remaining `text-charmap` exceptions carry `ruling_needed: true`.
- Changes to `check-exceptions.ron`:
  - the 4 heart-only entries are removed (`Nebula_Ecran_1`, `Tour_2F_Chat_08`, `Tour_2F_Sc04_T1`, `Tour_2F_Sc05_T1`);
  - the heart clause is dropped from 2 reasons (`Gris_Echec_Sc06_T2`, `Owlys_Entrepot_Sc02_T3`).
- No trainer or item name holds a heart, so the 2-byte cost changes no name finding.

**Tests.**
- `extract` unit tests (`games::font`):
  - `substitute` and `encode_fr`: a heart encodes exactly like `{EMOJI_HEART}`;
  - `glyph_index`;
  - reserved bytes are kana only, and kana is rejected;
  - the `.latfont` round trip;
  - parse errors;
  - the metric rules;
  - a reserved glyph is blank on 0 or 3 and not blank on 1 or 2.
- `crates/content/tests/sil1_g_font.rs`, in every flavour:
  - the committed files draw exactly `GLYPHS` and encode to their sequences (glyphs 0xAF and 0x1E7);
  - a merge writes each glyph and width at its index and nothing else;
  - a merge fails on a missing glyph, on a redrawn glyph at another width, and on another baseline;
  - the originality guard, with an extract;
  - the mask helpers.
- `crates/content/tests/sil1_g_font.rs`, with `PKR_GAME=silice` only:
  - `content::data::FONT` holds each authored glyph and width;
  - `A`, then each authored glyph, then `A` is encoded with `font::encode` and drawn on the engine's `Canvas`. Every pixel of each glyph's advance equals the RON drawing.
- `xtask/tests/silice_check.rs::authored_glyph_rules`, on the fixture extract (every glyph 6 px wide):
  - `font-glyph` fires on both widths;
  - `♥` and `·` pass;
  - 36 hearts fit in 216 px, and 37 measure 222 px;
  - 5 hearts make a 10-byte name, and 6 fail `trainer-name`;
  - a kana fails `text-charmap`;
  - `—` and `°` still fail;
  - a missing `heart.ron`, or a glyph without a code, fails;
  - a games dir without `font/` passes, with the note.

**Route.** Neutral. Emerald's sources are unchanged apart from `extract/src/games/`, which its `BUILD_ID` excludes. The Silice `BUILD_ID` moves, because `games/silice/font` and `build/silice` are hashed.

### 3.2 SIL2: Ch1–Ch3 (needs M9 mart, menus and save, and M10 Cut, Strength and Flash)
**Wave 0 engine contracts** (each new piece of state hashed and snapshotted once, with a SCHEMA bump):
- `GameData::field_move_enabled`: the module flags gate field moves;
- `Request::{ScreenFlash(color, frames), ScreenShake(mag, frames), MoneyGet, WaitInputOrTimeout(u16), AddFriendship{slot, n}}`;
- `Fade::{ToWhite, FromWhite}`;
- `YesNo { default_no }`;
- a palette-tint Request (`SetTint{ banks, color, amount }`) spread over frames under the line budget;
- `GiveMon { moves, item, … }`;
- `BattleTrainer { no_whiteout }`;
- a game-defined start-menu entry (the Mallette).

**Route-changing tasks for Emerald:** only the shared field-move hook (if M10 is not yet merged) and the save hook.
**Content waves:** Chapters 1–3 as in Task F. The casino, doubles, Mega Alakazam and PIERRE LIEN use their documented fallbacks until SIL3.

### 3.3 SIL3: Ch4–Ch6 (needs M10 Surf, Fly, Rock Smash and fishing; M11 held items and multi)
New engine features: held items and the Mega and Primal framework (§3.4), `BattleSetup` (start status and stages), the battle-rule table, mid-battle lines, the book UI, petal weather.
Content: `species_forms.ron` (Q5), Mega pics (Q3), Mallette Logs and Sync.

### 3.4 Engine contracts for the three decided mechanics (game-agnostic)
- **Mega Evolution and Primal Reversion (SIL3).**
  - Held items: `Pokemon.held: Option<ItemId>` and `TrainerMon.item: Option<ItemId>` (in `crates/engine/src/mon.rs`), plus `GameData::hold_effect(ItemId)`.
  - `GameData::form_changes() -> &[FormChange { base, item, form, kind: Mega|Primal, enable_flag: Option<FlagId> }]`; form species are appended to `BattleTables.species`.
  - Battle state: `mega_used: [bool; sides]` (one Mega per trainer per battle; Primal does not count), hashed in `BattleRun`.
  - Player trigger: a fight-menu toggle, available when `enable_flag` is set (FLAG_SIL_MODULE_MEGA).
  - AI: Mega on the first opportunity. Primal triggers on switch-in.
  - Stats, types and ability recalculated in battle only; the party species is unchanged after battle.
  - Messages through `battle_text`; trainer lines on Mega through the line hook.
  - Abilities Primordial Sea, Desolate Land and Aerilate, plus whatever Q5 adds. The engine `Ability` enum is mechanics vocabulary, so that is allowed.
  - Measure the Mega animation's frame cost (≤ 217).
- **Inverse battle (SIL4).** `Request::SetBattleRules(BattleRules { inverse })` stored in World (hashed, snapshotted). One `effectiveness()` function in `crates/engine/src/battle/calc.rs` maps ×0 → ×2 and swaps ×2 and ×0.5; the AI (`battle/ai.rs`) calls the same function. Opening message by text id.
- **Inverted controls (SIL4).**
  - `Request::SetInputInverted(bool)` and a World field `input_map` (hashed, snapshotted).
  - Applied only to field walking input: not menus, not forced movement (ice, arrows keep real directions, as ch07 §9 says).
  - The explorer's planner (`crates/explore/src/path/**`) applies the same map when expanding `WalkTo`. Raw inputs stay as traces, so parity is unaffected.
  - Accessibility toggle in Options (M9 menu).
  - An idle-frame counter is readable by scripts for the confusion emitters.

#### Silice engine: held items, Fairy, Mega/Primal, abilities (as built)
Engine side of the three SIL3 battle features, ahead of SIL3 (branch of `db07b1d`). It is game-agnostic, and Emerald does not move: parity is identical (route loop 166 of 217, hash 46 of 65), and the four committed checkpoint nodes verify as `valid (rehash)`. Every new behaviour is gated on data Emerald lacks: an item with a hold effect, a form change, a weather or post-Gen3 ability, or the Fairy type.

- **Held items.**
  - Data and hashing:
    - `Pokemon.held: Option<ItemId>` is hashed only when `Some` (tag 0xA1 + id).
    - `TrainerMon.item` is copied into `held` with no draw.
    - `GameData::hold_effect(ItemId) -> Option<HoldEffect>`.
    - The Emerald content generates `battle::hold_effect` from item names (`classify::hold_effect`).
  - Checkpoint `SCHEMA` 3 adds a trailing `party_held`. Schema 2 still reads: `decode` appends the empty field, and `state_digest` hashes an empty `party_held` as schema 2 did. `schema_readable` covers 2..=3, so the old nodes verify as rehash.
  - In battle:
    - `BattlerExt { item, hold, first_turn }` is hashed with tag 0xA8, only when set.
    - `BattleExt { holds, forms, formed, mega_used, mega_pending, weather }` is hashed with tag 0xA9, only when set.
  - Hold effects (`battle_util.c` `ItemBattleEffects`, `hold_effects.h`), all done and tested in `effects/tests_s.rs`:
    - **Oran / Sitrus** (`RESTORE_HP`): at `ENDTURN_ITEMS1` only, at or below half HP. Message, then HP, then `removeitem`.
    - **Cheri, Chesto, Pecha, Rawst, Aspear, Persim, Lum**: at `ITEMS1`, at `ITEMS2`, and at `MOVEEND_ITEM_EFFECTS_ALL`, which loops until no berry acts. Lum gives `CURED_PROBLEM` or `NORMALIZED_STATUS`, and always `CURED_PROBLEM` at move end. Its status name is empty (the Jpn buffer, as Shed Skin).
    - **Leftovers**: max HP / 16, `ITEMS1` only.
    - **Type-boost items**: `(100+param)%` of the type's attacking stat, applied after Huge Power.
    - **Choice Band**: Attack ×1.5. The move lock is not modelled.
    - **Quick Claw**: `gRandomTurnNumber < 0xFFFF*param/100` gives `UINT_MAX` speed, with no draw of its own.
    - **King's Rock**: `MOVEEND_KINGSROCK_SHELLBELL`. `Random()%100` is drawn only after damage, then the flag check, then the target's HP. It flinches through `SetMoveEffect(FALSE)`. A multi-hit move runs the item effects and King's Rock after each hit (`moveendto MOVEEND_NEXT_TARGET`) and once more on the last hit after its loop, as Gen3.
    - **BrightPowder / Lax Incense**: accuracy ×`(100-param)/100`, after Compound Eyes and Hustle.
    - **Focus Band** (`Battle::focus_band`): `Random()%100` right after the damage roll, drawn whenever the item is held. The "hung on" message comes after the result message. The same roll runs in `adjustsetdamage` (Sonic Boom, Bide's release) and in `adjustnormaldamage2` for the confusion self-hit, where the attacker is the target (its own band).
  - Not modelled:
    - Shell Bell, White Herb, Mental Herb, the stat berries and the species items.
    - The held-item animation.
    - `HandleFaintedMonActions`' item check.
  - Consumption clears the battler's item, its hold, `x.holds` and the party mon's `held`, so the party loses the berry, as in Gen3.
  - **Emerald switch:** `classify::TRAINER_HELD_ITEMS` stays **off**. Turning it on gives Emerald's trainers their `gTrainers` items, which changes the route (berry heals, Quick Claw order, Focus Band draws). That is a later Emerald route change: re-baseline parity and `checkpoints verify --accept`. It would also be **inaccurate** until the effects listed as not modelled are: the Choice Band lock, Shell Bell, the herbs, the stat berries, Amulet Coin and `HandleFaintedMonActions`' item check (Emerald trainers hold several of these).
- **Fairy.**
  - `Type::Fairy` (18) sits after Mystery, so the Gen3 split by type makes it **special**. This is documented on `Type::Fairy`.
  - `FAIRY_MATCHUPS` holds the 12 Gen6 entries. The content build appends them before the Foresight split only when the content uses Fairy, so Emerald's chart is byte-identical. Tested in `fairy_has_the_gen6_matchups_and_is_special`; the build script's copy of the count (`engine_fairy_len`, it cannot link the engine) is checked against `FAIRY_MATCHUPS.len()` by `the_build_fairy_count_matches_the_engine`.
- **Mega / Primal** (`battle/core/forms.rs`).
  - Data: `GameData::form_changes() -> &[FormChange { base, item, form, kind: Mega|Primal, enable_flag }]`. Form species are ordinary entries of `BattleTables.species`.
  - Setup:
    - `Battle::new` finds each held-item change. Trainers ignore the flag.
    - The World calls `Battle::enable_player_forms(game, |f| flag set)` before `start`.
    - The form state lives in `Battle::x`, not in `BattleRun`, so the `BattleRun` field guard is unchanged. This deviates from the plan's "hashed in `BattleRun`" on purpose: it is hashed through `Battle::hash`.
  - Player trigger: in the move menu, START toggles Mega Evolution when `Battle::can_mega(Player)`. While armed, the type window shows `Str::MegaArmed`. A move chosen armed is `Choice::Mega(slot)`.
  - The AI arms Mega Evolution with its first move when the battle is a trainer battle and the mon can Mega Evolve. Wild mons never Mega Evolve.
  - Order (expansion `TryDoGimmicksBeforeMoves`): Mega Evolutions happen after the switches, items and running and before the first move action, faster battler first by full speed (Swift Swim and Chlorophyll count; ties go to the player). When both sides use a move, the turn is then ordered with the new speeds (Gen7); when one side switches, the Mega's switch-in abilities (Intimidate) hit the mon that just came in (`Step::Megas`, queued only when a Mega is pending). Simplified: a Pursuit on a switching foe strikes before the Mega. One Mega per side per battle. A mon with no PP left may still Mega Evolve (then Struggle), for the player and the AI.
  - Primal Reversion happens at the battle start (in speed order) and on each switch-in, before the switch-in abilities. It does not use the side's Mega.
  - The change takes the form's stats (HP stat and HP kept), types and ability, keeps the battle state, and runs the new ability's switch-in effects. The form lasts through switches. The party species is never changed, so the party is "restored" by construction.
  - Messages through `battle_text`: `MegaReacting`, `MegaEvolved`, `PrimalReversion`, the primal weathers', `BreaksTheMold`, `BouncedBack` and `MegaArmed`. They are `StrSrc::Missing` (empty) in Emerald.
  - Event `FormChange { side, party, species }`: the scene reloads the pic (`Play::Form`).
  - **Frame cost** (`crates/game-gba/src/bin/battle_probe.rs`, read by `parity.rs` `battle_probe_mega_turn`, synthetic battle on the GBA):
    - A turn costs 25 lines (30,762 cycles); with the Mega Evolution, **33 lines (39,683 cycles): +8 lines**. After the review fixes and the M8 Task L merge: 28 lines (33,866 cycles), with the Mega 35 lines (42,918 cycles): +7 lines.
    - Finding the forms at battle start (34 changes) costs +2 lines.
    - The scene's pic reload is spread over frames, as a send-out's.
- **Abilities.** All have a test in `effects/tests_s.rs`.
  - The weather (`effects/later.rs`):
    - Drizzle, Drought and Sand Stream give a permanent weather, as in Gen3. Each turn prints "Rain continues…", "The sunlight is strong." or "The sandstorm rages."
    - Sandstorm damage is max HP / 16 in turn order. Rock, Ground, Steel, Sand Veil and Sand Force are immune.
    - **Rain Dish**: max HP / 16 at the end of the turn in rain (`STRINGID_PKMNSXRESTOREDHPALITTLE2`). **Sand Veil**: no sand damage, and the foe's accuracy ×80/100 in sand (after Compound Eyes, before Hustle).
    - Not modelled: **Cloud Nine** and **Air Lock** (weather suppression); Sand Veil's overworld encounter halving.
    - Rain and sun change Fire and Water damage in the special branch. Swift Swim and Chlorophyll double speed.
    - **Primordial Sea / Desolate Land**: heavy rain or harsh sun, which only the other primal weather replaces. Damaging Fire or Water moves fail after their attack string. The weather ends when no battler with the ability is left (switch-out or faint).
  - Done:
    - **Aerilate, Pixilate, Refrigerate** (Normal moves change type, ×1.2). **Content decision:** under the Gen3 split by type, the new type decides the category, so a converted Normal physical move becomes special (Mega Glalie's and Mega Altaria's Return use Sp. Atk); Gen4+ games keep it physical.
    - **Tough Claws** ×1.3 on contact.
    - **Magic Bounce** reflects `MAGIC_COAT_AFFECTED` moves: the bounced move costs no PP and is not bounced again.
    - **Parental Bond** (Gen7 25%): a second hit at a quarter of the damage, then "Hit 2 times!". Not for the multi-strike moves, Uproar, Explosion / Self-Destruct, the two-turn moves, Rollout / Ice Ball, Beat Up and Endeavor (the Gen7 exclusions the engine can name).
    - **Mold Breaker** turns off the target's breakable ability for the move and announces itself on entry.
    - **Technician** ×1.5 at power 60 or less.
    - **Skill Link**: always 5 hits.
    - **Sand Force** ×1.3 on Rock, Ground and Steel moves in sand, and is immune to sand damage.
    - **Solar Power**: Sp. Atk ×1.5 in sun, and loses 1/8 HP each turn with `Str::SolarPowerHurt` (`Missing` in Emerald).
    - **Filter**: super-effective damage ×3/4.
    - **Prankster**: +1 priority for status moves, which fail against a Dark target.
    - **Steadfast**: Speed +1 on a flinch.
    - **Speed Boost** (Gen3 `isFirstTurn`: 2 on entry, decremented by `TurnValuesCleanUp(FALSE)` at the end of `TryDoEventsBeforeFirstTurn` and in `BattleTurnPassed` after the fainted replacements, so a lead boosts at the end of turn 1 and an end-of-turn replacement at the end of the next turn).
    - **Shadow Tag** (Gen3: traps every foe; `Ability::ShadowTagGen4` adds Gen4's exemption of a Shadow Tag foe).
    - **Levitate** (Gen3 `MAKESGROUNDMISS`, accuracy still drawn).
    - **Shell Armor / Battle Armor** (no crit roll).
    - **Lightning Rod** (the Gen5 absorb: Sp. Atk +1). Gen3's only redirects in doubles, so it does nothing in singles: Emerald's maps to `Other`.
  - Silice's stones also need Trace, Thick Fat, Huge/Pure Power, Intimidate, Insomnia and Swift Swim, which were already there.
  - **Deferred** (no Silice stone needs them; canonical Megas only):
    - Adaptability (Lucario, Beedrill), Mega Launcher (Blastoise), Strong Jaw (Sharpedo), Sheer Force (Camerupt), No Guard (Pidgeot) and Scrappy (Lopunny): new damage or accuracy rules.
    - Snow Warning (Abomasnow): needs hail.
    - Delta Stream (Rayquaza): needs strong winds and Dragon Ascent.
    - Healer (Audino): doubles only.
  - **Emerald switch:** `classify::GEN3_EXTRA_ABILITIES` stays **off**. Emerald's Drizzle, Drought, Sand Stream, Speed Boost, Shadow Tag, Levitate, Shell/Battle Armor and Sand Veil species keep `Ability::Other`. Turning it on gives them their Gen3 behaviour; it changes Emerald battles (Torkoal, Tyranitar, Ninjask, Wobbuffet, the Levitate ghosts…) and is a later Emerald route change. Rain Dish keeps its earlier name-only mapping (only these abilities bring rain).
  - **Silice switch:** `classify::SILICE_ABILITY_RULES` (off for Emerald) maps Lightning Rod to the Gen5 absorb and Shadow Tag to `ShadowTagGen4`. These are not Gen3 behaviour, so they stay out of `GEN3_EXTRA_ABILITIES`; the Silice content turns the switch on.
- **Build.**
  - ROM and `.text` against `db07b1d` (Emerald FR, full content): ROM 3,658,784 → 3,683,904 B (+25,120); engine `.text` 244,194 → 258,698 B (+14,504; 77,822 B left under its limit); all `.text` +17,344; `.rodata` +7,776; IWRAM and EWRAM +0.
  - After the review fixes, on top of master with M8 Task L (the full 518-map import, so the ROM is 9,545,968 B and not comparable): engine `.text` 259,754 B (+1,056 over the number above, merge included); IWRAM and EWRAM +0.
  - The battle probe ROM (`target/battle-probe.gba`) is built by `cargo xtask rom` and read by `cargo xtask parity`.
  - The Wave 0 battle digest test leaves out `TrainerMon::item` (all `None`), so its value is unchanged.

### 3.5 SIL4 to SIL6 (roadmap)
- **SIL4, Ch7–Ch9.** Reverse mechanics (§3.4); ice, arrow and rotation tiles, Waterfall and Dive (M10); underwater currents; camera pan; multi battles (M11); the foe's back sprite; optional true follower.
- **SIL5, Ch10–Ch11.**
  - Trainer item AI (M11) and per-mon lines; the shadow-prism rule.
  - The scripted save (R-6, needs the M9 save format).
  - The alternate ending: `Event::ReloadLastSave`, handled by the shells (GBA reloads the flash slot without writing; native reloads the last save checkpoint); the explorer treats it as a terminal goal `ending_alt`.
  - The title screen in two states (`FLAG_SIL_GAME_CLEAR`); credits with vignettes; palettes (gold, true night) and the sky-crack overlay.
  - Primal reversion; music MUS_SIL_* (needs M12 and authored songs, Q3).
- **SIL6, Ch12.** A deterministic clock and daily flags (M10 contract); the clone battle; egg (M13); tutor and MOVE_SIL_CODE_AQUA (§4 item 5); National Dex; boss wild battles; rule challenges; legendaries.

Sequencing: SIL1 can run in parallel with M9 (its files are disjoint except the shell plumbing, Task D, which merges first). Each later SIL milestone waits for, or pulls forward, the Emerald milestone it needs (Q4).

## 4. Contradictions and errors in `contenu-jeu` that block conversion

1. **Encounter schema.** `donnees/rencontres.ron` keys maps by `"MAP_*"` with one header each. The extract's v5 `EncountersFile` keys by map name with a `Vec` of headers (`content/extracted/battle/encounters.ron`). The conventions call it an "extension de encounters.ron", but it cannot be loaded as is.
2. **Trainer data the engine cannot hold.** `item:` is set on 54 distinct held items, 36 of them ITEM_SIL_ including every Mega stone. Neither `crates/engine/src/mon.rs::TrainerMon` nor the content build carries held items.
   - The `items` bag (Arthur, Crow, Benou, Julien) is not in the schema.
   - 5 names exceed `TRAINER_NAME_LENGTH` 10 (`include/constants/global.h`): `TAUBER&HOCSEAT`, `LOULPIX & DORIS`, `CHEF DE SILO`, `CONTREMAÎTRE`, and the 11-character `TAUBER&HOCS`/`LOULPIX&DOR`.
   - 8 class names exceed 12 characters (`gTrainerClassNames[][13]`), for example `MAÎTRES DU DONJON`, `TECHNICIEN COCKET`, `{PKMN} RANGER`.
3. **Maps referenced but not defined.** `MAP_ROUTE_SXX_ANTENNE` (ch12 `warp`, a placeholder); `MAP_MANGA_SQUARE_HOUSE_1`/`_HOUSE_2` (named in ch06 §2, no content); `MAP_BRICO_BOURG_NIGHT` and `MAP_TRONE_ZENITH_NUIT` (fallback-only). Maps defined only in §4 or as sub-maps (`MAP_GRIS_ECHEC_GYM_SOURDE`, `MAP_PORT_RESSAC_GYM_CALE`, `MAP_MANGA_SQUARE_GYM`, `MAP_EVALON_CITADELLE_2F`) need the importer to read §4 too.
4. **Constants that do not exist in pokeemerald.** `assembler.py` only checks SPECIES_/MOVE_/ITEM_.
   - Music: `MUS_MAUVILLE`, `MUS_MOSSDEEP`, `MUS_ROUTE111`, `MUS_TEAM_AQUA_HIDEOUT` (the decomp has `MUS_AQUA_MAGMA_HIDEOUT`), `MUS_ENCOUNTER_MYSTERIOUS`, `MUS_KIMIKO_STREAM`.
   - Sounds: `SE_EXPLOSION` (the decomp has `SE_M_EXPLOSION`), `SE_RING`, `SE_APPEL`, `SE_SIFFLET`, `SE_GONG`, `SE_SERVEURS`, `SE_SHOJI`, `SE_COFFRE`, `SE_HACK`, `SE_CLOCHE_BRONZE`, `SE_GRESILLEMENT`, `SE_FILTRE`, `SE_MALLETTE_APPEL`, `SE_MALLETTE_BIP`. The invented ones break the decided SIL_ prefix rule.
   - Weather: `WEATHER_PETALS`, `WEATHER_CLOUDS`.
   - New heal locations without the SIL_ prefix: `HEAL_LOCATION_PORT_RESSAC`, `_OWLYS`, `_BRICO_BOURG`.
   - Pseudo-flags in ch12: `FLAG_DAILY`, `FLAG_GEMME`, `FLAG_PREMIERE`.
5. **Invented move.** `MOVE_SIL_CODE_AQUA` (ch12 Sc28) contradicts D-00 and D-02b (Gen3 moves only), and `assembler.py` silently whitelists `MOVE_SIL_`. It needs an author ruling by SIL6 (keep it as new move data, or use the documented tutor fallback).
6. **Name clash.** `ITEM_SIL_POUDRE_CLAIRE` ("Poudre Claire", a White Herb clone, ch12 line 4811) has the same FR name as Gen3 `ITEM_BRIGHT_POWDER` (POUDRE CLAIRE), which the same chapters also use. That gives two different items with one name in the bag.
7. **Mega data is underspecified.**
   - 34 Mega stones are listed, but only 4 `SPECIES_*_MEGA` ids appear (Alakazam, Charizard X, Gyarados, Steelix).
   - The canonical forms need the **Fairy type** (Gardevoir, Altaria, Mawile), which Gen3 does not have, and about 14 abilities newer than Gen3 (Tough Claws, Magic Bounce, Pixilate, Refrigerate, Parental Bond, Mold Breaker, Technician, Skill Link, Sand Force, Solar Power, Filter, Prankster, Steadfast, Lightning Rod as reworked, and others). `bible/besoins-moteur.md` names only 3 new abilities. This blocks SIL3 (Q5).
   - The set is uneven (Venusaurite and Charizardite but no Blastoisinite; Latiosite but no Latiasite). That is a design note, not an error.
8. **Glyphs missing from the FR charmap** (`crates/extract/src/text/charmap.rs`), counted in text blocks: `—` 71, `·` 24 (R-11), `°` 18, `♥` 14 (R-11), `●` 8, `○` 8, `♪` 4, `₽` 4, `#` 3, `*` 2, `←`/`→` 1 each, and `[`, `]`, `_` in the `[LOG_xx]` titles. R-11 covers only ♥ and ·; the others need a ruling (new glyphs or rewording). (SIL1 Task G: both were wrongly counted. `·` is Gen3 0xAF, and `♥` is `{EMOJI_HEART}` (`F9 E7`, glyph 0x1E7); both glyphs are now redrawn with original drawings.)
9. **Duplicate text ids.** `Tour_Sbire1_Intro` and `Tour_Sbire2_Intro` are defined in both ch02 (line 747) and ch09 (line 1223).
10. **Non-literal references.** Scripts compose text ids dynamically (`msgbox(Bar_Sc02_T1 | T2 | T3)`, `Gym2_…`, `Nocturne_Sc01_T…`, `Tour_2F_Chat_NN`, `PG_Legendaire_…`, `Evalon_Sc07_Partner_T…`) and use flag families (`FLAG_SIL_FRAGMENT_`, `FLAG_SIL_ITEM_PG_`, `FLAG_SIL_HIDE_GAZ_`, `FLAG_SIL_GYM06_PORTE_`, `FLAG_SIL_PARDON_`). They must be expanded to concrete ids before typed Rust can reference them.
11. **Format drift the importer must handle.** Several `[Id]` texts are grouped in one fenced block in ch03 (38 blocks), ch04 (21) and ch05 (18). Speaker headers carry parenthetical notes. ch03, ch08 and ch09 number their map headings (`### 2.n MAP_…`). The suggested map size ("Taille") is missing for every map in ch03, ch04, ch05 and ch12.
12. **ASCII plans not to scale.** For example, Brico-Bourg says "24 × 20" but the plan is drawn 25 × 14, and the player-house door at (6,13) sits on the drawn border row. Every coordinate (warps, coord events, objects) must be re-derived when the maps are drawn.
13. **Unreachable fishing slots.** Old-rod slots are filled in the fishing tables, but no Old Rod is ever given (noted by the author in ch04 §10).
14. **Scripts use primitives absent from `Ctx`** with no fallback written inline. Examples: `play_music`, `play_se`, `screen_shake`, `screen_flash`, `mallette_call`, `show_sync`, `set_sky_alert`, `set_time_tint`, `set_weather`, `mart`, `battle_two_trainers`, `battle_multi`, `set_input_inverted`, `set_inverse_battle`, `take_money_percent`, `wait_input_or_timeout`, `save_game`, `show_title`, `reload_last_save`, `credits`, `teach_move`, `give_egg`, `battle_mirror`, and the daily flags. They are listed in each §9 with a fallback; the translation must choose per call site, and SIL milestones replace the fallbacks.
15. **Stale and fragile notes.** `bible/besoins-moteur.md` describes the engine "au 22/10/2026" (no give_item, no multichoice), which M8 has since superseded. `outils/assembler.py` validates items against `pokeemerald/include/constants/items.h` rather than the extract's `tables/items.ron`. It has no TM or tutor learnset data, so the "moves plausible for the species" rule in conventions §4 cannot be checked; the extract does not export TM or tutor compatibility.

## 5. Open questions (blocking)
1. **Source of truth after import** (blocks SIL1 Task B): does the `contenu-jeu` markdown stay canonical, with `xtask silice import` re-run on every change, or does the repo's `games/silice/` become canonical after a one-time import?
2. **Map authoring** (blocks SIL1 Tasks C and F): who draws the about 115 maps, and with what tool (Porymap plus an import, the studio editor pulled forward from M14, or agents from the ASCII)? Can all maps use Emerald's ROM tilesets until original tilesets exist?
3. **Original assets** (blocks SIL3 visuals and SIL5 music; SIL1 uses placeholders): will you supply original trainer pics (about 102), Mega and Primal pics (36), new tilesets and the 13 MUS_SIL_* tracks? Nothing ripped from any game can be committed. Or do Emerald placeholders stay indefinitely?
4. **Priority** (decides when SIL2 can start): should M9–M12 be reordered around Silice's needs, with Emerald story targets after Act 1 deferred, or does Silice follow the Emerald roadmap?
5. **Mega forms** (blocks SIL3 data): canonical types and abilities (add a Fairy type and about 14 abilities newer than Gen3), or Gen3-adapted forms?
6. **Duplicate text ids (SIL1 Task A, provisional):** `Tour_Sbire1_Intro` and `Tour_Sbire2_Intro` are defined in both ch02 and ch09, with different bodies. The ch09 ones are renamed `..._Ch09` (`text/fr.ron`, `text/speakers.ron`). Should we keep these names, pick others, or merge the texts? The rename is reversible: undo it, and the loader rejects the duplicate again.

### Critical files for implementation
- /Users/dode/Documents/pokemon/pokemon-rust/crates/content/build/main.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/content/build/input.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/content/src/lib.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/engine/src/data.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/engine/src/mon.rs

Other files relied on:
- /Users/dode/Documents/pokemon/pokemon-rust/crates/content/build/build_id.rs (flavour-scoped digests)
- /Users/dode/Documents/pokemon/pokemon-rust/crates/content/build/stub.rs (precedent for a non-ROM `World`)
- /Users/dode/Documents/pokemon/pokemon-rust/crates/extract/src/schema.rs (MapFile, Layout, Trainer, EncountersFile)
- /Users/dode/Documents/pokemon/pokemon-rust/crates/extract/src/text/charmap.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/engine/src/script.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/engine/src/flow/vars.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/explore/src/db.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/game-native/src/store.rs
- /Users/dode/Documents/pokemon/pokemon-rust/crates/studio/src/paths.rs
- /Users/dode/Documents/pokemon/pokemon-silice-notion-export/contenu-jeu/donnees/dresseurs.ron
- /Users/dode/Documents/pokemon/pokemon-silice-notion-export/contenu-jeu/donnees/rencontres.ron
- /Users/dode/Documents/pokemon/pokemon-silice-notion-export/contenu-jeu/outils/assembler.py