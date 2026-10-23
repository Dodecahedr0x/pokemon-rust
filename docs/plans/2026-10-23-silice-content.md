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

Planned 2026-10-23, after M8 Wave 1 merged. SIL1 Wave 0 waits for M8 Task L (it edits the content build); the studio map editor (SIL1 Task C, now the first task) starts in parallel with L.

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
  ids/{maps,flags,vars,trainers,items,species,moves}.ron
                                   append-only allocation: name -> number (stable saves and checkpoints)
  maps/<Name>.ron                  extract MapFile schema; scripts referenced by Rust fn name
  layouts/<Name>.ron               extract Layout schema; primary/secondary = extracted tileset names
                                   (or games/silice/tilesets/<Name> once original tilesets exist)
  text/fr.ron                      TextId name -> FR .string syntax (from the chapters' [Id] blocks)
  battle/trainers.ron              extract TrainersFile schema + optional `items`, `lines`, `rules`
  battle/encounters.ron            extract EncountersFile v5 (map name -> Vec of headers)
  battle/heal_locations.ron, items.ron (ITEM_SIL_*: name, pocket, price, key, hold effect/clone_of),
  multichoice.ron, marts.ron, species_forms.ron (SIL3), moves.ron (MOVE_SIL_*, SIL6)
  font/glyphs.ron + .png           ♥ · (R-11) and any approved extra glyph, original drawings
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

#### SIL1 Task C as built: the studio map editor

Pulled forward and built first, in parallel with M8 Task L. The code is in `crates/studio/src/map_edit/`. Outside that directory, the task touched:
- `crates/studio/src/{lib.rs, bin/studio_map.rs, paths.rs, ui/mod.rs}`;
- `crates/studio/Cargo.toml`, which gains `ron`, `serde` and the `studio-map` bin;
- `xtask/src/studio.rs`;
- new tests in `crates/studio/tests/map_edit.rs` and the fixture in `crates/studio/tests/fixtures/games/fixture/`.

**Files (the contract SIL1 Task A loads).** A games dir is `games/<game>/`; the editor defaults to `games/silice`.
- `layouts/<Name>.ron` uses `extract::schema::Layout` exactly: size, tileset names, a 2x2 border, and raw blocks (`metatile | collision << 10 | elevation << 12`).
- `maps/<Name>.ron` uses `map_edit::schema::GameMap`. It is the extract's `MapFile` with three differences:
  - `layout` is a layout name, not an inline `Layout`, so maps may share a layout;
  - there is no `group`/`num` (Silice ids come from `ids/maps.ron`);
  - warps and connections name their target map only. `GameWarp { x, y, elevation, dest_map, dest_warp }` and `GameConnection { dir, offset, map }` replace the extract's `dest_map_id` and `map_id` strings.
- `header`, `objects`, `coords`, `bgs` and `scripts` keep the extract's types. Task A can mirror `GameMap` in `crates/content/build/silice/schema.rs`: the content build cannot depend on the studio.
- Both files are written with `extract::export::to_ron`, the extract's RON style. A save rewrites only files whose bytes change. A load followed by a save is byte-identical.

**Model (`model.rs`, no Dioxus).**
- `Doc` holds the `GameMap`, the size, the tileset names, the border and flat blocks. Its constructors are `Doc::new(&NewMap)`, `from_parts(map, layout)` and `from_map_file(&extract MapFile)`.
- `Editor` applies `Op`s, each as one undo step, with a 256-step limit. `begin_stroke`/`end_stroke` group a pencil drag into one step. It also has a clipboard and a dirty flag.
- `Op` is also the edit-script format, a RON list parsed with `implicit_some`, so options take bare values. The ops:
  - cells: `Paint`, `Rect`, `Fill` (flood fill over equal raw blocks, 4-connected), `Ascii` (rows plus a legend of brushes, for drafting from the chapters' plans), `Copy`, `Paste` (clipped at the edges), `Border`, `Resize`;
  - map properties: `Tilesets`, `Header`, `Scripts`;
  - events: `Object`, `Warp`, `Coord`, `Sign`, `HiddenItem`, `Move`, `Replace`, `Delete`;
  - connections: `Connect`, `SetConnection`, `Disconnect`;
  - history: `Undo`, `Redo`.
- Cell ops take optional `metatile`, `collision` and `elevation`. Each part that is set replaces that part of the block.
- Object local ids are always index + 1. They are renumbered after every add, replace or delete, as in Gen3.
- A failing op leaves the document and the history unchanged.

**Catalog and validation (`catalog.rs`, `validate.rs`).**
- Tilesets are the build's own `&'static Tileset`s. They are paired with their extract names through the extracted map files, since every compiled tileset is used by some map. On the stub world the tilesets are `StubPrimary` and `StubSecondary`.
- Maps are the build's maps plus the games dir's maps. Object graphics come from the manifest's sprites. Script names come from `content::data::SCRIPT_NAMES`.
- Errors:
  - unknown tilesets, or a tileset in the wrong slot;
  - metatiles outside `0..primary` and `512..512+secondary`, in cells and in the border;
  - events out of bounds;
  - object local ids, graphics (`OBJ_EVENT_GFX_*`, extracted when a sprite list exists), `MOVEMENT_TYPE_*`, trainer type and `FLAG_*`;
  - warp destinations and their warp index (the map itself counts);
  - `VAR_*`, sign facing, hidden item `ITEM_*` and `FLAG_*`, and secret bases;
  - connections to unknown maps or to the map itself.
- Script names the build does not have are warnings.

**Renders (`render.rs`).** `map_view::render_map` draws the map through `Preview`, a `GameData` that serves a leaked `MapData` at `MapId(0xFFFE)` and delegates everything else to the build.
- One base `MapData` is cached per (size, tilesets, border, connections), and the cells that differ from it are drawn as `setmetatile` overrides. This bounds the leaks.
- Connections to the build's maps are drawn in the margin.
- The palette is the tileset pair drawn as an 8-wide map of its metatiles. The primary's metatiles come first, and the secondary's start on a new row.

**CLI.** The command is `cargo xtask studio map …`. It runs the `studio-map` bin with `cargo run` from the workspace root, so relative paths are relative to the workspace.

```text
studio-map [--games DIR | --game NAME] list
studio-map … create <spec.ron>                       # NewMap; never replaces a saved map
studio-map … apply <Map> <script.ron> [--dry-run]   # saves unless an op fails (then nothing is written)
studio-map … validate [<Map>...]                    # exit 1 on errors; warnings are printed
studio-map … render <Map> [--out FILE] [--margin N] # default target/studio/maps/<Map>.png (git-ignored)
studio-map palette <Primary> <Secondary> [--out FILE]
```

Example spec and edit script (this is how the fixture was made):

```ron
(name: "FixtureTown", width: 8, height: 6, primary: "General", secondary: "Petalburg", fill: 1, border: [468, 469, 476, 477])
```
```ron
[
    Rect(x: 0, y: 0, w: 8, h: 1, metatile: 468, collision: 1),
    Ascii(x: 0, y: 1, rows: ["T......T", "T......T"], legend: {'T': (metatile: 468, collision: 1), '.': (metatile: 1)}),
    Fill(x: 1, y: 4, metatile: 13),
    Object(x: 2, y: 2, gfx: "OBJ_EVENT_GFX_BOY_1", movement: "MOVEMENT_TYPE_WANDER_AROUND", range_x: 1, range_y: 1, script: "FixtureTown_EventScript_Boy"),
    Object(x: 5, y: 3, gfx: "OBJ_EVENT_GFX_YOUNGSTER", trainer_type: "TRAINER_TYPE_NORMAL", sight: 3, script: "FixtureTown_EventScript_Trainer", flag: "FLAG_HIDE_FIXTURE_TRAINER"),
    Warp(x: 4, y: 5, dest_map: "OldaleTown", dest_warp: 0),
    Coord(x: 4, y: 4, var: "VAR_FIXTURE_STATE", value: 0, script: "FixtureTown_EventScript_Trigger"),
    Sign(x: 6, y: 1, script: "FixtureTown_EventScript_Sign"),
    HiddenItem(x: 1, y: 5, item: "ITEM_POTION", flag: "FLAG_HIDDEN_ITEM_FIXTURE_POTION"),
    Connect((dir: North, offset: 0, map: "Route101")),
]
```

**UI (feature `ui`): the studio's Edit tab.**
- `session.rs` holds the tools and overlay shapes as a Dioxus-free core: pencil, rect, fill, picker, select (a drag moves the event), and placing an object, warp, coord or sign. A paint mode chooses whether the brush writes the metatile, the collision or the elevation.
- `ui.rs` is thin: a map list and a new-map form, the toolbar, Undo, Redo and Save, and zoom. The layer toggles are metatiles, collision, elevation and events.
- The map picture and the palette are served by an `edit` asset handler from `EditImages`, which `ui/mod.rs` registers. An SVG layer draws the overlays and takes the mouse.
- Clicking the palette picks a metatile.
- The inspector edits the selected event as RON (Apply, Delete). The connections are edited as a RON list, applied as one undo step.
- The issue list updates on every change.

**Write locations.** `paths::write_locations` gains `<workspace>/games` as `Committable::Authored`, and a test checks that it is not git-ignored.

**Tests.**
- Unit tests in `map_edit/tests.rs` and `session.rs`: paint, rect, fill, ASCII, copy and paste, strokes, undo and redo, event CRUD with local ids, validation errors, the byte-identical round trip and edit-script parsing.
- `tests/map_edit.rs`:
  - every extracted map, loaded into the editor, renders pixel-identical to the game's `render_map` (real extract only);
  - an edited map renders equal to a direct `MapData` render, on both worlds; on the real extract, Route 101 with painted cells also equals the game's render with those cells as overrides;
  - the fixture round-trips byte for byte, and validates on the extract;
  - a CLI session gives identical bytes in two dirs;
  - a failing script writes nothing;
  - `validate` fails on errors;
  - the hygiene check: games dirs hold only canonical `maps/*.ron` and `layouts/*.ron`, and planted PNG, extra-field and reformatted files are flagged.
- ssr tests render the Edit tab empty and with the fixture open.

**Gates:**

| Gate | Result |
|---|---|
| `cargo fmt --check` | clean |
| clippy, plain and with `game-native/window,studio/ui` | clean |
| `cargo test --workspace`, real | 1390 passed |
| `cargo test --workspace`, stub | 1378 passed |
| `cargo xtask studio --check` | 166 passed |

**Limitations.**
- The UI was not driven by hand in a window; only its ssr renders and its core are tested.
- Renders leak one `MapData` per distinct (size, tilesets, border, connections).
- Connections to other games dir maps, and dive and emerge connections, are not drawn.
- Flags, vars, items and movement types are checked by prefix only, until `ids/*.ron` exist (Task A).
- The header is edited only through the `Header` op, not in the UI.
- The palette shows no metatile numbers. The CLI prints its layout: 8 per row, and the cell where the secondary starts.

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
8. **Glyphs missing from the FR charmap** (`crates/extract/src/text/charmap.rs`), counted in text blocks: `—` 71, `·` 24 (R-11), `°` 18, `♥` 14 (R-11), `●` 8, `○` 8, `♪` 4, `₽` 4, `#` 3, `*` 2, `←`/`→` 1 each, and `[`, `]`, `_` in the `[LOG_xx]` titles. R-11 covers only ♥ and ·; the others need a ruling (new glyphs or rewording).
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