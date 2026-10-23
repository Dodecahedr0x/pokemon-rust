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

#### SIL1 Task B as built: the importer and the validator

The code is in `xtask/src/silice/`: `mod.rs` (CLI, file writing), `md.rs` (chapter markdown), `names.rs` (constant words, families, map names), `import.rs`, `ids.rs` (allocation), `check.rs` and `schema.rs`. Outside it, the task touched one arm in `xtask/src/main.rs`, `regex` in `xtask/Cargo.toml`, `xtask/tests/silice_{import,check}.rs` (with `silice_common/`) and the synthetic fixtures in `xtask/tests/fixtures/silice/`.

**Commands.**
- `cargo xtask silice import <contenu-jeu> [--out games/silice] [--report target/silice/import-report.md] [--force]` reads `chapitres/*.md` in name order. It is deterministic: two runs give byte-identical files, and a re-run over unchanged output writes nothing. Without `--force` it refuses, and writes nothing, if any existing file would change. Every ambiguity goes to the report, never into the data.
- `cargo xtask silice check [--games games/silice] [--extracted DIR] [--allow RULE]...` exits 1 on errors and lists warnings. Its name sources are the decomp tables compiled into `extract::names` and `charmap::SOUNDS` (always), the extract when it has a `manifest.ron` (species, moves, items, heal locations, the FR font widths), and every name in `$PKR_DECOMP/include/constants/*.h` when `PKR_DECOMP` is set (the only source for `WEATHER_*`). A missing source skips its rules with a `note:` line, so the check runs without the extract.

**Formats (`schema.rs`, ready to move to `crates/content/build/silice/schema.rs`; it reuses `extract::schema`).**
- `text/fr.ron` is the extract's `TextFile` (id -> decomp `.string` syntax). Body lines are joined raw; a line without `\n`, `\l` or `\p` is joined with a space and reported. A duplicate id with a different body is written twice, as a repeated key, so `check` can see it. `text/speakers.ron` holds, per id, the chapter, the speaker and the parenthetical note.
- `battle/trainers.ron` holds the extract's `Trainer` fields plus `items` (the AI bag) and `notes` (the source's `//` comments). The party is `extract::schema::TrainerMon`, so held items are kept.
- `battle/encounters.ron` follows the v5 shape: keyed by map name, a `Vec` of headers (land, water, rock smash, fishing, `notes`).
- `items.ron` holds the `ITEM_SIL_*` definitions (name, pocket, price, `clone_of`, effect, chapters), the §7 placements (flag, map, place, `pos`, name, item, quantity, method) and the marts (label, map, money or coins, stock with prices).
- `ids/{flags,vars,trainers,items,species,moves,music,sounds,maps}.ron` are `IdsFile { base, ids: [(name, num, map?, removed?)] }`.
  - The ranges are: flags from 0x20 to 0x4FF; vars from 0x4020 to 0x40FF; trainers from 1 to 863; items, species and moves after the decomp's last number; music and sounds after `SOUNDS`; maps from 0, skipping 0x7F7F.
  - Allocation is append-only. Names are numbered in name order on the first import, and later new names are appended after the highest number. A name the source drops stays, and is reported as stale. A tombstone (`removed: true`) is never renumbered.
  - Each map entry carries its map file name (`MAP_BRICO_BOURG_PLAYER_HOUSE_1F` is `BricoBourg_PlayerHouse_1F`).
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
| `id-alloc` | error | duplicate name or number in an ids file, or a number outside the range or reserved |
| `id-undefined` / `id-removed` | error | a Silice name missing from its ids file, or a reference to a tombstone |
| `id-unknown` | error | a Gen3 name (species, class, pic, AI) the decomp lacks; the base species of a form |
| `sil-prefix` | error | an unknown FLAG/VAR/ITEM/MOVE/MUS/SE/WEATHER/HEAL_LOCATION/TRAINER name without `_SIL_` |
| `map-undefined` | error | a `MAP_*` or encounter map in neither `ids/maps.ron` nor `maps/` |
| `trainer-undefined` / `trainer-duplicate` | error | a `TRAINER_SIL_*` that no trainer defines, or one defined twice |
| `trainer-name` / `trainer-class-name` | error | more than 10 or 12 encoded bytes |
| `trainer-items`, `party-size`, `party-moves`, `party-level` | error | more than 4 bag items; 0 or more than 6 mons; 0 or more than 4 moves; a level outside 1–100 |
| `encounter-slots` / `encounter-level` | error | not 12/5/5/10 slots; levels outside 1–100 or min > max |
| `fishing-unreachable` | warning | fishing slots of a rod (old 0–1, good 2–4, super 5–9) that no placement or mart gives |
| `text-duplicate` / `text-missing` / `dynamic-unresolved` | error | a repeated id; an id printed but not defined; a placeholder the source does not enumerate |
| `text-charmap` | error | a character the FR charmap cannot encode; the run prints a histogram of the missing glyphs |
| `text-width` | error | a line over 216 px (27 tiles) with the extract's FR widths, `{PLAYER}` as 7 of the widest capital |
| `invented-move` | warning | a `MOVE_SIL_*` (D-00, D-02b) |
| `item-incomplete` / `item-name-length` / `item-name-clash` | warning | no name, pocket or base item; a name over 14 bytes; a name equal to another item's, ignoring case and spaces |
| `exception-unused` | warning | an exception that matches nothing |

**Real import (committed under `games/silice/`, authored data only).**
- 13 chapters and 1,923 texts, 2 of them duplicate ids with different bodies.
- 238 trainers.
- 39 encounter maps with 56 tables.
- 69 `ITEM_SIL_*` definitions, 260 item placements and 12 marts.
- 1,053 referenced constants and 1,075 referenced text ids.
- 37 dynamic references, 4 of them unresolved.
- The ids files hold:
  - 476 flags and 19 vars;
  - 238 trainers and 69 items;
  - 5 species (the Mega forms the source names);
  - 1 move, 13 music tracks and 0 sounds;
  - 115 maps.

**Check on the real import.**
- With the extract and `PKR_DECOMP`, the check finds 169 errors and 84 warnings. All 251 findings are listed in `check-exceptions.ron`, so the check exits 0, and 244 of those entries carry `ruling_needed`. By rule:
  - `text-charmap` 108;
  - `item-incomplete` 55;
  - `sil-prefix` 29;
  - `item-name-length` 13;
  - `fishing-unreachable` 11;
  - `trainer-class-name` 10;
  - `trainer-name` 8;
  - `map-undefined` 6;
  - `dynamic-unresolved` 4;
  - `item-name-clash` 4;
  - `text-duplicate` 2;
  - `invented-move` 1.
- Without the extract and the decomp, the check also exits 0. It reports 5 `exception-unused` warnings: the 3 Gen3 name clashes and the 2 `WEATHER_*` names, whose sources are missing.

**Open rulings (author).**
- Glyphs, counted in texts: `—` 68, `_` 25, `[ ]` 12, `°` 15 (plus the trainer names `N°7` and `N°12`), `♥` 6 (R-11), `₽` 3, `○ ●` 2, and `♪`, `#`, `*`, `←`, `→` 1 each.
- Names over the limits:
  - 8 trainer names over 10 bytes, among them `TAUBER&HOCSEAT`, `LOULPIX & DORIS` and `CONTREMAÎTRE`;
  - 10 class names over 12 bytes;
  - 13 item names over 14 bytes.
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
  - `MAP_MANGA_SQUARE_GYM`, which ch06 §4 describes without a heading constant.
- Unresolved dynamic references:
  - `<Champion>_PG_Revanche_Deja`;
  - `FLAG_SIL_ITEM_PG_GEM_LOIC … _LOULPIX`;
  - `Gym_Sc02_Q<n>_Choix`, whose multichoice lists have no ids;
  - `SPECIES_<NOM>`.
- `Tour_Sbire1_Intro` and `Tour_Sbire2_Intro` are each defined in both ch02 and ch09.
- `MOVE_SIL_CODE_AQUA`.
- The old-rod slots are never reachable.
- Item names:
  - `ITEM_SIL_POUDRE_CLAIRE` clashes with `ITEM_BRIGHT_POWDER`;
  - `ITEM_SIL_ORBE_BLEU` and `ITEM_SIL_ORBE_ROUGE` clash with the Gen3 orbs;
  - both Mewtwonites are named "Mewtwoïte X et Y".
- Mega data: 55 items have no pocket or base item, the Mega stones among them, and only 5 `SPECIES_*_MEGA` forms are named for about 30 stones.
- The AI item bags of Arthur (ch10), Crow, Benou and Julien (ch12) appear only in prose. The `items` field is empty for every trainer, so the bags must be entered by hand.

**Gates.** See the final report of the task for the counts. All gates pass: fmt, clippy (plain and with `game-native/window,studio/ui`), `cargo test --workspace` real and stub.

#### SIL1 Ch0 maps as built (Task F, the map half)

The six Chapter 0 maps are in `games/silice/{maps,layouts}/`. They were made with the studio map editor's CLI only (`cargo xtask studio map create` and `apply`). The edit scripts that make them are committed under `games/silice/edits/<Map>/`:
- `00_create.ron`: the `NewMap` spec;
- `01_terrain.ron`: the cells;
- `02_events.ron`: the header, map scripts, warps, objects, coord and bg events, and the connection.

`games::hygiene` looks only at `maps/` and `layouts/`, so `edits/` is allowed. It was run on `games/silice` with the extract present (so including `games::copies`) and is clean.

To rebuild, delete the six maps and layouts, then:
1. `create` every map;
2. `apply` every `01_terrain.ron`;
3. `apply` every `02_events.ron`.

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