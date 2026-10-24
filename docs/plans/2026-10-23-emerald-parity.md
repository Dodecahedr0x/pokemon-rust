# Emerald feature parity through the explorer (plan)

**Goal (user, 2026-10-23):** focus the Emerald explorer on proving that the engine has **feature parity with the original game**. **Decision (user): ledger first, then oracle.**

- **Phase 1: the parity ledger.** An inventory of everything Emerald uses: script commands and specials, metatile behaviours, field moves, battle move effects, abilities and hold effects, item uses, and screens. Each entry is checked against the engine, joined with what the spine, the branches and the sweep reach, and ranked.
- **Phase 2: the vanilla oracle.** The original **BPEE** (`../emerald.gba`; its SHA-1 equals the decomp's `rom.sha1`, so the decomp's symbols are exact) runs in mGBA. The **same spine steps** drive it from its RAM state, and story state is compared with our engine at every step boundary, then on branches.
- **The spine grows** past badge 1 as far as the engine allows. Every step it cannot play is a ranked parity gap with a repro.

Until now, parity meant ROM == native (`emu/tests/parity.rs`), never "behaves like Emerald". This plan adds the second meaning without touching the first.

## 0. Baseline (master `7248118`, measured while planning)

- `explore branch` (release, 12 jobs) plays the spine to badge 1 in 78 steps, 109,851 frames, 0.1 s. It then runs 319 branches in 0.2 s: 273 rejoined, 25 expected divergences, 1 stranded, 12 aborted, 8 unplayable. **No bug finding.** 11 report-only findings:
  - `dofieldeffect` and `waitfieldeffect` (the Center heal and the gym badge animation);
  - `SPECIAL_ShowPokedexRatingMessage`;
  - `copyobjectxytoperm`;
  - `bufferitemnameplural`;
  - degraded: the evolution scene and `SPECIAL_StartWallyTutorialBattle`.
- The content build lowers 4,584 script functions: 47,770 native sites, 2,158 `Unsupported`, 1,387 `Ignore` and 176 `Default`. The biggest unsupported groups are the Frontier (`CallFrontierUtilFunc` 372, Apprentice, Pike, Tower, Dome), `createvobject` 143, `turnvobject` 124 and `setfieldeffectargument` 49. There are also 80 unresolved `waitmovement` and 191 stale `specialvar` sites over 74 specials.
- **Master's gates are red** (`2026-10-23-status-handoff.md`): checkpoint nodes diverge, the explorer `path::Terrain` test fails on GraniteCave_1F, and Silice e2e tests fail.

## 1. Tasks

**Working rule (user, 2026-10-23):** every agent goes as far as it can. It records issues as it meets them and keeps going, with clearly marked and logged fallbacks, instead of stopping at the first problem. The issues are then fixed in bulk.

The rules are M8's and M9's: own worktree, TDD, `feat(<crate>): …` commits, no attribution lines, reviewed then merged with `--no-ff`. Worktrees need `PKR_ROM_FR`, `PKR_ROM_US` and `PKR_DECOMP`. **Nothing ROM-derived is committed:** decomp *names* and counts are fine, while RAM dumps, pictures and `.sav` files are not. The oracle runs only when `PKR_ROM_US` is set, and is skipped otherwise.

### Wave A (parallel)

#### Task G — green gates (owns `explore/src/path*`, `checkpoints/`, `emu/tests/parity.rs`)

1. Make `path::Terrain` use `Behavior::land_encounter()`, like the per-edge rules.
2. Explain the 9 diverged Emerald nodes against Wave 0's and the off-grass branch's intended changes (the start menu, friendship, the repel chain, cave encounters). Re-record or extend inputs where needed, then make one `chore(checkpoints)` commit with `verify --accept`.
3. Run `cargo xtask parity --long` and settle `KNOWN_LATE` (81,769 or a real fix).

**Done when:** `cargo xtask gates` passes its fmt, clippy, test, test-stub and parity lanes. `test-silice` is out of scope: the Silice spines are Silice work.

#### Task L — the parity ledger (owns `crates/parity` (new, host-only), `xtask/src/ledger.rs`, and the `content/build/parity.rs` emitter)

1. **The universe, from the decomp** (`PKR_DECOMP`):
   - script commands (`asm/macros/event.inc`) and specials (`data/specials.inc`), with their call sites per map script;
   - `MB_*` (`include/constants/metatile_behaviors.h`);
   - `EFFECT_*`, `ABILITY_*` and `HOLD_EFFECT_*`;
   - item `fieldUseFunc`/`battleUseFunc`, field moves, weathers, battle types (`BATTLE_TYPE_*`) and map types;
   - a hand-kept list of the screens (`main_menu.c`, `start_menu.c`, …) with their M9/M10 owners.
2. **The engine's answer for each entry: native, degraded, ignored, unsupported or missing.** Take it from the code that dispatches, never from a hand list that can drift:
   - the content build's lowering classes per command and special;
   - the behaviour classifier (`build/classify/field.rs`);
   - the battle effect, ability and hold-effect dispatchers. Where a dispatcher has no enumerable table, add `pub const HANDLED: &[&str]` beside it, with a test that the table and the `match` agree.
3. **Reach.** For every entry, record:
   - the maps that use it, and whether those maps are on the **main story** (a hand-kept story map list per act, Acts 1–8 and the League, from the decomp's story order);
   - whether the spine, the branches (`explore` db: scripts started, reports, maps) or `cargo xtask sweep` reached it.
4. **Output:** `cargo xtask ledger` writes `target/parity/ledger.{md,ron}`, one table per category, ranked by: story-blocking > on a story map > reached by the explorer > site count. The summary counts per category and act go in this doc's "As built". The full table stays in `target/`, regenerated.

**Done when:** the ledger runs on BPEF and lists the 11 reports above, the 2,158 unsupported sites by name, and each category's coverage percentage, with a test pinning one entry per category on a synthetic decomp fixture.

#### Task S — spine Act 2+ (owns `explore/src/spine/emerald.rs`, `explore/src/goals.rs` (new acts), and `explore/tests/spine_real.rs`)

1. Extend the Emerald spine from `badge1` along the story:
   - Route 116 and the Rusturf Tunnel grunt (Devon Goods), Mr. Stone;
   - Mr. Briney to Dewford, Brawly (badge 2);
   - Granite Cave (Steven's letter), the boat to Slateport, the museum (Aqua);
   - Route 110 (May), Mauville, Wattson (badge 3);
   - then as far as it plays.
2. Use `Train` and Center heals, as Act 1 does. New goals go in a new act per badge (`act2`, `act3`, …), checked against the decomp's names (`m8_k_goals`).
3. **Go as far as possible; record, bypass, continue (user, 2026-10-23).** When the engine cannot play a step, the task records it in the "Spine blockers" table: step, map, cell, the expansion error or report, the decomp feature, and the owner. It then continues past it with an explicit `Step::Bypass { issue, .. }`. The bypass sets the flags, vars, warp or party state that the missing feature would have produced. It lives on the explorer side only, never as an engine change. Bypasses are printed by `explore spine`, and the test asserts that the bypass list equals the blocker list, so fixing a gap means deleting its bypass. The aim is the whole story through the League. The gaps are fixed **in bulk** afterwards (Wave C).
4. `spine_reaches_badge1` stays. Add `spine_reaches_<last checkpoint>`.

**Done when:** the spine runs as far into the story as bypasses allow, with every bypass matching a recorded blocker.

#### Task O1 — vanilla oracle foundation (owns `crates/vanilla` (new, host-only, depends on `emu`) and the `emu` RAM-read API)

1. **RAM reads in `emu`:** `Emu::read_bytes(addr, len)` and `read_u16`/`read_u32`, through mGBA's bus (`busRead*` or the `rawRead` core hooks).
2. **`vanilla::sym`:** BPEE addresses for the symbols the oracle needs:
   - `gSaveBlock1Ptr`, `gSaveBlock2Ptr`, `gPlayerParty`, `gPlayerPartyCount`;
   - `gObjectEvents`, `gMain`, `gBattleTypeFlags`, `gBattleMons`, `gActionSelectionCursor`, `gMoveSelectionCursor`, `gBattleOutcome`;
   - `sGlobalScriptContext`/`sScriptContext*Enabled`, `gStringVar*`, `gSpecialVar_*`, the text printer and the field-message state, and the `CB2_*` and `BattleMainCB2` function addresses.

   Build them from the decomp (`make` with agbcc gives `pokeemerald.map`; the ROM's SHA-1 already matches `rom.sha1`). Commit only the symbol **names**: the build script or test resolves the addresses from `PKR_DECOMP`, or from a `.map` path in `PKR_VANILLA_MAP`. If the decomp cannot be built here, a committed address table for BPEE is acceptable (addresses are not ROM content), validated at runtime by the ROM's SHA-1.
3. **`vanilla::State`:**
   - map and cell, facing, flags, vars, the party (decrypted substructs: species, level, HP, moves, exp), the bag pockets, money (SaveBlock2's key), the object events, badges and the respawn point;
   - the mode: field idle, script running, message waiting for input, yes/no open, multichoice open, battle (action menu, move menu, waiting), or transition.
4. **`explore::semantic::Semantic`:** one normalised struct with `from_world(&World)` (ours) and `vanilla::State::semantic()`, plus a `diff` that names fields. **Tolerances:** RNG-dependent fields (wild species, levels and exp past the first random battle, trainer ID, the personality-derived fields) are compared only where the plan says so; the rest must be equal.
5. **`vanilla::boot`:** from power-on through the title, Birch's speech (male, a fixed name) and the truck, until the player first has control in the truck, reading the mode from RAM, never by frame counts.

**Done when:** an `#[ignore]` test (run with `PKR_ROM_US`) boots BPEE to the truck and reads a `Semantic` that matches our new game's `Semantic` on map, cell, flags, vars and party (empty), with the diff printed.

### Wave B (after O1 and S merge)

#### Task O2 — the vanilla spine and branches (owns `vanilla::drive`, `explore/src/oracle.rs`, `xtask vanilla`)

1. **`vanilla::Live`** expands the same `spine::Step`s on BPEE from RAM state:
   - walking with our planner over our map data (map ids are Gen3's `group << 8 | num`), with the objects read from RAM;
   - A through messages, yes/no and starter choices as `Opts::choice` says;
   - the battle policy (the first damaging move, or `Best`), read from `gBattleMons` and the cursors;
   - `Train` until the lead's level holds.
2. **`cargo xtask vanilla spine`** runs both spines side by side and prints the `Semantic` diff at every step boundary. A diff outside the tolerances is a `Parity` finding in the explorer db (new `FindingKind::Parity`), signed on the step and field.
3. **`cargo xtask vanilla branches`** replays every `rejoined` and `diverged_expected` branch's macros on BPEE from the same spine position. It compares the end state (and the window's), so off-spine behaviour gets an oracle too.
4. **Frame counts per step** are reported, never gated: our engine against Gen3, as M9 Task V wanted.

**Done when:** the vanilla spine reaches `badge1` with every diff either equal or a listed, explained deviation, and branches run on the oracle with their `Parity` findings triaged in this doc.

### Wave C (dynamic) — closing gaps

From the ledger's ranking and the oracle's findings, one task per gap cluster, story-blocking first. M9's tasks (menus, mart, save, evolution) and M10's (field moves) already own many of them. Wave C first re-ranks the ledger against M9 and M10 and takes **only what no milestone owns**. Likely examples:

- the field effects behind `dofieldeffect` (the Center heal, the badge animation);
- `createvobject` and `turnvobject`;
- `copyobjectxytoperm`, `bufferitemnameplural` and `ShowPokedexRatingMessage`;
- the unresolved `waitmovement` sites;
- the stale `specialvar` sites.

After each merge, the spine is extended past the blocker it removed.

## 2. Spine blockers

Each row is one `Step::Bypass` of `explore/src/spine/emerald.rs` (its `issue`); `spine_real::SPINE_BLOCKERS` lists the same ids in spine order. Fixing a gap means deleting its bypass and its row; the spine then plays the real thing and may need a step or two in place of the bypass.

No bypass is left: `SPINE_BLOCKERS` is empty and the Emerald spine plays from a new game to the Hall of Fame on real steps.

The bag-use rows (`bag-use-hm06`, `bag-use-hm03`) went with Milestone 9 Task B's bag; the spine teaches Rock Smash, Surf and Strength through START → BAG → the HM → USE → the party menu (`Step::UseItem`), and drinks Full Restores and the Max Elixir the same way.

The rows `mossdeep-gym-puzzle`, `legendaries-scene`, `rayquaza-scene`, `sky-pillar-layouts` and `game-clear` went with Wave F (the Sky Pillar cluster, as built in §3): the layout override is in checkpoint schema 6 and in `World::hash`, and the planner walks it.

The rows `fly-*`, `water-door-*`, `dive-*` and `waterfall-ever-grande` went with Wave F's field moves (the water and Fly cluster, as built in §3).

**Report-only findings along the spine** (it plays on; first site of each): `bufferitemnameplural` (degraded, Birch's lab); `SPECIAL_StartWallyTutorialBattle` (degraded, Petalburg); the evolution scene (degraded); `SPECIAL_ScriptMenu_CreateStartMenuForPokenavTutorial` (degraded, Rustboro (11,17)); move effects 208 (Brawly's gym), 176 (Route 110 wild), 158 (Route 113) and 043 (Route 112); `SPECIAL_UpdateCyclingRoadState` (Route 110 (17,99)); `ai pursuit viability` (degraded, Route 110 rival); `SPECIAL_SetMauvilleOldManObjEventGfx` (Mauville Center); `SPECIAL_SetMirageTowerVisibility` (Route 111 (16,139)); `FLDEFF_USE_ROCK_SMASH` and `SPECIAL_RockSmashWildEncounter` (Route 111 (18,102)); `SPECIAL_CableCar` (degraded: the ride is a fade and Gen3's 602 frames, Route 112's station). Surf works (Wave C field moves): the spine surfs Route 118 and walks Route 119 to Fortree; the next gap is Fortree's gym, behind the Kecleon that only the Devon Scope (Route 120, Steven) reveals.

## 3. As built

(per task, at merge)

### Task L — the parity ledger (branch `worktree-agent-a5da4b40c92336c41`)

`cargo xtask ledger` writes `target/parity/ledger.{md,ron}`. It needs a built Emerald `content`, `PKR_DECOMP`, and optionally `target/explore/emerald/explore.db` (schema v4) and `target/sweep/report.txt`. The pieces:

- **`crates/parity`** (host-only) holds:
  - the decomp universe and its per-map uses;
  - the reach (explorer db and sweep);
  - the hand lists: story maps per act (`story.rs`), screens and owners (`owners.rs`);
  - the ranking and the Markdown/RON output.
- **Engine statuses** come from the code that dispatches:
  - `build/support/{commands,specials}.ron` and the `degraded(` arms of `prelude/special.rs`;
  - the classifiers in `content/build/classify` (behaviours, move effects, abilities with `Rules::SILICE`, hold effects, map types);
  - the new `prelude::BATTLE_TYPES_HANDLED` / `BATTLE_TYPES_DEGRADED`, with a test against `battle_types_of_kind`;
  - `data::ITEM_USE` (empty) and `battle/core/items.rs` ("Wave 0: refused");
  - `stub_step(` in the `ui/*` modules;
  - the engine's `Report::*`/`stub_step` literals.
- **The downgrade rule:** a "native" entry that the engine reports, or that the explorer saw reported, is downgraded.
- **`content::PARITY`** (`content/build/parity.rs`) gives the lowering's IR sites per command/special/module, its unsupported report sites and its stale `specialvar` sites.

**Measured** on this branch: master `8d11698` + Task L; spine 78/78 steps, 15 spine maps + 20 by branches, 319 branches; sweep with 60 report kinds; 0 ledger issues.

| category | entries | native | degraded | ignored | unsupported | missing | native % | native+degraded % |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| script commands | 229 | 101 | 15 | 42 | 71 | 0 | 44% | 51% |
| specials | 527 | 15 | 45 | 20 | 447 | 0 | 3% | 11% |
| metatile behaviours | 240 | 129 | 7 | 88 | 16 | 0 | 54% | 57% |
| move effects | 214 | 73 | 0 | 0 | 141 | 0 | 34% | 34% |
| abilities | 78 | 42 | 0 | 0 | 0 | 36 | 54% | 54% |
| hold effects | 67 | 33 | 0 | 0 | 0 | 34 | 49% | 49% |
| item field uses | 21 | 0 | 0 | 0 | 0 | 21 | 0% | 0% |
| item battle uses | 6 | 0 | 0 | 0 | 0 | 6 | 0% | 0% |
| field moves | 14 | 0 | 0 | 0 | 0 | 14 | 0% | 0% |
| weathers | 18 | 1 | 0 | 17 | 0 | 0 | 6% | 6% |
| battle types | 32 | 3 | 3 | 0 | 0 | 26 | 9% | 19% |
| map types | 10 | 10 | 0 | 0 | 0 | 0 | 100% | 100% |
| screens | 26 | 1 | 0 | 0 | 0 | 25 | 4% | 4% |
| engine reports | 28 | 0 | 13 | 0 | 15 | 0 | 0% | 46% |

Gaps on story maps, by act: act1 113, act2 35, act3 64, act4 57, act5 17, act6 47, act7 68, act8 36, league 17 (the ledger's "Gaps per act" table splits them by category).

**Top 30 gaps not owned by M9/M10 (feeding Wave C)** — none of them is story-blocking yet: `story::BLOCKING` is empty, and master's spine has no `Bypass` step.

| # | category | name | status | act | reached | sites |
|---:|---|---|---|---|---|---:|
| 1 | command | `dofieldeffect` | unsupported | act1 | reported | 39 |
| 2 | command | `copyobjectxytoperm` | unsupported | act1 | reported | 23 |
| 3 | command | `waitfieldeffect` | unsupported | act1 | reported | 14 |
| 4 | special | `SPECIAL_ShowPokedexRatingMessage` | unsupported | act1 | reported | 1 |
| 5 | special | `SPECIAL_StartWallyTutorialBattle` | degraded | act1 | reported | 1 |
| 6 | special | `SPECIAL_ShouldTryRematchBattle` | degraded | act1 | spine | 77 |
| 7 | command | `setfieldeffectargument` | unsupported | act1 | spine | 57 |
| 8 | move effect | `EFFECT_ROLLOUT` | unsupported | act1 | spine | 49 |
| 9 | command | `setmaplayoutindex` | unsupported | act1 | spine | 18 |
| 10 | move effect | `EFFECT_CURSE` | unsupported | act1 | spine | 18 |
| 11 | ability | `ABILITY_MAGMA_ARMOR` | missing | act1 | spine | 15 |
| 12 | ability | `ABILITY_WHITE_SMOKE` | missing | act1 | spine | 13 |
| 13 | command | `hideplayer` | unsupported | act1 | spine | 11 |
| 14 | ability | `ABILITY_FLAME_BODY` | missing | act1 | spine | 11 |
| 15 | command | `checkpartymove` | degraded | act1 | spine | 9 |
| 16 | command | `pokenavcall` | unsupported | act1 | spine | 8 |
| 17 | special | `SPECIAL_IsTrainerRegistered` | degraded | act1 | spine | 7 |
| 18 | special | `SPECIAL_PlayerHasBerries` | degraded | act1 | spine | 7 |
| 19 | special | `SPECIAL_ObjectEventInteractionGetBerryName` | unsupported | act1 | spine | 5 |
| 20 | command | `bufferboxname` | unsupported | act1 | spine | 3 |
| 21 | special | `SPECIAL_ObjectEventInteractionGetBerryCountString` | unsupported | act1 | spine | 3 |
| 22 | special | `SPECIAL_ObjectEventInteractionGetBerryTreeData` | degraded | act1 | spine | 3 |
| 23 | special | `SPECIAL_DoWateringBerryTreeAnim` | unsupported | act1 | spine | 2 |
| 24 | special | `SPECIAL_HasAllHoennMons` | degraded | act1 | spine | 2 |
| 25 | special | `SPECIAL_ScriptGetPokedexInfo` | degraded | act1 | spine | 2 |
| 26 | move effect | `EFFECT_HELPING_HAND` | unsupported | act1 | spine | 2 |
| 27 | ability | `ABILITY_MINUS` | missing | act1 | spine | 2 |
| 28 | ability | `ABILITY_PLUS` | missing | act1 | spine | 2 |
| 29 | special | `SPECIAL_BufferUnionRoomPlayerName` | degraded | act1 | spine | 1 |
| 30 | special | `SPECIAL_CountPlayerTrainerStars` | degraded | act1 | spine | 1 |

**Gaps by owner** (count, of which on a story map):

| owner | gaps | of which on a story map |
|---|---:|---:|
| none | 333 | 122 |
| M13 | 338 | 96 |
| M11 | 252 | 131 |
| M10 | 88 | 57 |
| M12 | 12 | 12 |
| M9 B | 19 | 14 |
| M9 M | 16 | 8 |
| M9 PT | 9 | 2 |
| M9 W | 8 | 2 |
| M9 BM | 6 | 5 |
| M9 S | 6 | 1 |
| M9 E | 4 | 0 |
| M9 R | 4 | 0 |
| M9 SM | 4 | 1 |
| M9 K | 3 | 3 |

**What the M9/M10 owners hold:**
- **M9 B:** the 21 item field uses and `SPECIAL_Bag_ChooseBerry`.
- **M9 BM:** the 6 item battle uses.
- **M9 M:** `pokemart*`, the PC specials, `shop.c` / `player_pc.c`.
- **M9 PT:** the party-choice specials, plus the party and summary screens.
- **M9 W:** the nickname specials and the naming screen.
- **M9 S / SM:** the save specials and the start, main and option menus.
- **M9 E:** the evolution scene.
- **M9 R:** in-game trades.
- **M9 K:** the money box.
- **M10:** the field moves, roamers, bikes and Cycling Road, weather specials, and the tide.

**Decisions and issues:**
- **The owner column** comes from name rules (`owners::owner`).
  - Battle → M11; audio → M12; contests, coins, the Frontier, berries' minigames, Union Room and the like → M13.
  - Field effects and `*vobject` are left unowned, on purpose, for Wave C.
  - `ignored` entries (mostly audio and weathers) are not in the Wave C list.
- **Reach is per map:** a feature is "reached" when the explorer stood on a map that uses it, which is not proof that it ran. `reported` means the explorer saw the engine report it.
- **Battle uses per map:**
  - trainers from `trainerbattle*` lines only, since the gym's `set_gym_trainers` names every gym's trainers;
  - wild mons: land encounters only, at their max level and with default moves.
- **Gyms belong to their badge's act:** Petalburg Gym is in act5.
- **Commands** are the `event.inc` macros that emit `SCR_OP_*`, plus the four handlers without a macro of their own name (`addobjectat`, `applymovementat`, `removeobjectat`, `waitmovementat`).
- **No `unknown` entries:** every category's status is derived.
- **Bypass steps:** `story::bypassed` reads the `Bypass` steps of `explore/src/spine/emerald.rs`, so rerun the ledger after Task S merges, and its bypassed features become story-blocking.

### Task O1 — vanilla oracle foundation

**Symbols.** The decomp builds here. agbcc (pret/agbcc, `build.sh` + `install.sh`) and `make compare` in a scratch copy outside the repo give a `pokeemerald.gba` whose SHA-1 is BPEE's. `vanilla::sym::BPEE` is a committed address table: 57 names (RAM variables, `CB2_*`, `BattleMainCB2`, task functions, script natives, the battle controller's `HandleInputChoose*`). It was taken from `objdump -t pokeemerald.elf`, because the statics are not in the `.map`. It applies to the ROM whose SHA-1 is `BPEE_SHA1` only, and `Vanilla::open` refuses any other ROM. `PKR_VANILLA_MAP` resolves the names from a listing instead and reports every difference:
- a linker `.map` gives the globals only;
- `make syms`' `.sym` or `objdump -t` also give the statics.

A name defined several times (`sMenu`, `HandleInputChooseAction`) keeps the table's address when the listing has it. The test `the_table_matches_the_decomp_build` checks the table against a listing whenever one is given (it passed on the objdump listing and on the `.map`).

**Struct offsets** were checked by compiling `offsetof`s **with agbcc** against the decomp headers. A modern arm-none-eabi-gcc puts some fields elsewhere: `SaveBlock1.flags` at 0x1248 instead of 0x1270, and the SaveBlock2 encryption key at 0xA8 instead of 0xAC. agbcc rounds struct sizes up to 4. The offsets are commented in `vanilla::state::layout`.

**What is built:**
- **`emu`** gains `read_bytes`, `read_into`, `read_u8`, `read_u16` and `read_u32`, through libmgba's `rawRead8`: no side effects, any alignment. It also gains `Emu::open_retail`: libmgba's own cartridge overrides (BPEE: FLASH1M and an RTC, not our FLASH512 override) and an RTC pinned to `RETAIL_RTC_MS` (2005-01-01), so runs do not depend on the host clock.
- **`explore::semantic`** (shared; the engine depends on nothing new):
  - `Semantic { map, cell, facing, flags, vars, party, bag, money, badges, respawn, objects }` uses the key's story filters (`key::story_flag`/`story_var`) and flags below Gen3's 0x960;
  - `Semantic::from_world(&World, &G)` reads our engine;
  - `diff(ours, vanilla, Tolerance)` returns named `Delta`s (`flag FLAG_…`, `var VAR_…`, `party[0].species`, `bag Items ITEM_POTION`, …).
- **Tolerances** (`Tolerance::default()`):
  - HP is compared as fainted / hurt / full, because max HP depends on the IVs (`exact_hp` compares the values);
  - levels and exp are compared until the caller clears `levels` after a random battle;
  - objects are compared by presence, because wanderers move at random (`object_cells` compares their cells).
  - Gen3 spawns objects only around the camera, so object *presence* also depends on where the player stands.
- **`explore::path::first_dir_on`:** the planner on a map, its overrides and an occupied list, for a world that is not ours.
- **`vanilla`** (new, host-only):
  - `Vanilla::state() -> State` holds the map from `gSaveBlock1Ptr->location`, the player's cell and facing from `gPlayerAvatar`'s object event (minus `MAP_OFFSET`), the flags, the vars, the live party (decrypted: the substruct order, the XOR key and the checksum), the bag and money (SaveBlock2's key), the coins, the object events, the badges, the respawn (`None` while `lastHealLocation` is all zeros), the name, the gender, the trainer id and the mode.
  - `State::semantic()` gives the shared view.
  - `Vanilla::mode()` returns `FieldIdle`, `Script`, `Message`, `YesNo`, `Multichoice`, `BattleAction`, `BattleMove`, `BattleTarget`, `BattleWaiting`, `Transition` or `Screen { callback2 }`. It reads `gMain`'s callbacks, the active tasks, `gPaletteFade.active`, field text printer 0 (states WAIT, CLEAR and SCROLL_START wait for A), `sGlobalScriptContext`/`Status` (the `WaitForAorBPress` native), `sLockFieldControls`, `gPlayerAvatar.tileTransitionState` (mid-step counts as a transition) and `gBattlerControllerFuncs[0]`.
- **`vanilla::boot`** runs power-on, the intro (START), the title (START), the main menu's NEW GAME and Birch's speech (A, with the gender cursor on BOY), then the naming screen. There it presses B until the random preset name is gone and types `SACHA` by walking the cursor, read from `sNamingScreen` and the cursor sprite. The phases come from RAM: a press is held one frame and released one frame, and no input depends on a frame count. The boot ends at the first `FieldIdle` in `MAP_INSIDE_OF_TRUCK` (4,555 frames). It checks that the player is a boy named SACHA.
- **`vanilla::drive`** (beyond O1, at the coordinator's request), all on RAM:
  - `settle` answers waits with A: YES through `sMenu.cursorPos`, the wall clock's NO-default yes/no, the starter screen (`Vanilla::starter`, Mudkip by default), FIGHT and the first move, and a naming screen confirmed as offered. It returns once `FieldIdle` holds over one input-less frame, because on-frame map scripts start one frame after control returns.
  - `walk_to` walks with our planner around the objects in RAM. `push` holds a direction for arrow warps (the truck door, door mats). `interact` faces a cell and presses A.

**Tests:**
- unit: decoding a mon in all 24 substruct orders, listing formats, the table's sanity, the semantic diff's field names and tolerances, the RAM reads on a dummy cartridge;
- `#[ignore]` `vanilla/tests/boot.rs`: boots BPEE to the truck and prints the diff with our new game;
- `#[ignore]` `vanilla/tests/intro.rs`: drives BPEE through the intro (out of the truck, Mom, the clock, May's house, meeting May) to **our new game's start**, then through Birch's rescue (Zigzagoon battle won with Pound) and the lab's starter (Mudkip, species 283, Pound and Growl). It diffs at both points: with `Emerald::default()` and with `Emerald::after_starter()`. It takes about 6 s in release.

**What the diffs show.** Our new game starts outside Brendan's house with May met (`content::new_game`, `Start::BeforeRescue`). In the truck, the map, the cell, the four `VAR_LITTLEROOT_*` intro vars, the intro flags, the respawn and the objects therefore differ by design. At our start, after vanilla played the intro, **all `VAR_LITTLEROOT_*` vars, the map and the respawn agree**. After the starter, `VAR_BIRCH_LAB_STATE`, `VAR_ROUTE101_STATE`, `VAR_STARTER_MON`, the Littleroot vars, the party species and moves and the HP bucket agree too. What remained were parity gaps for Wave C, all in the new-game data (closed by Wave C "new-game state parity", below):

1. **136 `FLAG_HIDE_*` flags** that `EventScript_ResetAllMapFlags` (`new_game.c` `NewGameInitData`) sets are clear in our new game. Examples: `FLAG_HIDE_ROUTE_104_RIVAL`, `FLAG_HIDE_PETALBURG_CITY_WALLY`, the Mr. Briney, Scott, Aqua and Magma, and Steven objects. `content::new_game` sets only the Littleroot subset, so these objects would be visible in our game where Emerald hides them. **Story-relevant: likely the largest single gap.**
2. **`FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN`** is set in ours and clear in vanilla after the intro: the intro clears it, and our new game sets it.
3. **Money: ours 0, vanilla 3000** (`SetMoney(…, 3000)`).
4. **`VAR_SEEDOT_SIZE_RECORD` and `VAR_LOTAD_SIZE_RECORD` = 0x8000** (`InitSeedotSizeRecord` and `InitLotadSizeRecord`) are 0 in ours.
5. **Intro flags our new game skips:** `FLAG_SET_WALL_CLOCK`, `FLAG_SYS_CLOCK_SET`, `FLAG_MET_RIVAL_MOM`, `FLAG_SYS_TV_HOME`, `FLAG_SYS_TV_WATCH`, `FLAG_DECORATION_1..14` (`SecretBase_EventScript_SetDecorationFlags` on the 2F transitions) and `FLAG_UNKNOWN_0x363`/`0x393` (the decomp's "set, however has no purpose").
6. **After the rescue: `VAR_DAILY_WILDS` = 1** in vanilla (`tv.c` counts the wild battle). Ours skips the battle in `Start::AfterStarter`, so this is expected there. On a played rescue it is a gap if our engine never counts.

**Issues and fallbacks:**
- The symbol table is committed rather than resolved at every run. The decomp build lives in a scratch dir, and `PKR_VANILLA_MAP` re-checks the table against it.
- `walk_to` plans on our map data without the RAM's `setmetatile` overrides.
- Object presence depends on the camera in Gen3 (the second issue above shows up only when the camera is near).
- `settle` always says YES, picks the first multichoice item and confirms an offered nickname. O2 needs `Opts::choice`-driven answers.
- `Mode::Message` covers text printer 0 (the field message box). Battle messages show as `BattleWaiting`, which `settle` advances with A.

**What O2 needs:**
- `vanilla::Live` expanding `spine::Step`s, using `walk_to`/`push`/`interact`/`settle`, with choices from `Opts`;
- `setmetatile` overrides read from RAM (`gBackupMapLayout`) for the planner;
- the camera-dependent object comparison: compare only the objects within both games' view, or the templates (`gSaveBlock1Ptr->objectEventTemplates`);
- per-step frame counts (`Vanilla::frames`);
- the battle policy from `gBattleMons` and the cursors (`settle` only presses FIGHT and the first move);
- tolerance switches after wild battles;
- a release build: the debug build runs about 10x slower.

### Task S — spine Act 2+ (branch `worktree-agent-abf2907939cd907aa`)

**Reach.** The Emerald spine runs 456 steps and 726,550 frames, through Norman's badge to HM03 (Act 5's end), about 1.2 s in release, after the Wave C1 (Briney's sails, field effects), C3 (abilities, hold effects) and C5 (new-game state) merges. `badge1` is at step 81, frame 104,879, and no bypass comes before it. Nine bypasses come after it (§2); `spine_reaches_badge5` asserts their list equals `SPINE_BLOCKERS`.

| checkpoint | step | frame | where |
|---|---|---|---|
| `devon_goods` | 95 | 121,108 | Rusturf Tunnel (13,5): the grunt beaten |
| `mr_stone` | 102 | 127,941 | Devon Corp 3F: the PokéNav |
| `dewford` | 122 | 140,604 | Mr. Briney's sail (played since Wave C1) |
| `steven` | 136 | 149,337 | Granite Cave, Steven's room |
| `badge2` | 149 | 223,961 | Brawly |
| `museum` | 172 | 246,032 | Oceanic Museum 2F: the Devon Goods delivered, the Aqua grunts beaten |
| `badge3` | 290 | 469,807 | Wattson (the gym barriers played since Wave C4) |
| `badge4` | 427 | 717,601 | Flannery: the cable car and the gym's holes and geysers played since Wave C (spine warps) |
| `badge5` | 475 | 768,029 | Norman, through the door signs (Randall, Parker, Jody beaten); HM03 at step 478 |

**Goals.** Four new acts:
- `act2` (8 goals, from `goods_stolen` to `badge2`);
- `act3` (6 goals: `slateport`, `devon_goods_delivered`, `rival_route110`, `mauville`, `wattson_defeated`, `badge3`);
- `act4` (6 goals: `fallarbor`, `meteor_falls` (`FLAG_MET_ARCHIE_METEOR_FALLS`), `mt_chimney` (`FLAG_DEFEATED_EVIL_TEAM_MT_CHIMNEY`), `lavaridge`, `flannery_defeated`, `badge4`);
- `act5` (6 goals: `go_goggles`, `verdanturf`, `hm_strength`, `norman_defeated`, `badge5`, `hm_surf`).

Act 4's route: Rock Smash from Mauville, Route 111 to its row 68 exit, Route 112 and the Fiery Path to Route 112's north part, Route 111 north of the desert (the planner would otherwise walk into the sandstorm, which pushes back without the Go-Goggles), the old lady's rest stop (a heal), Route 113, Fallarbor, Route 114, Meteor Falls; back the same way to the cable car; Mt. Chimney (Maxie), the Jagged Pass, Lavaridge, Flannery.

Act 4 starts with levels 34 to 36 on Route 110's north grass (`Step::TrainFresh` rounds, healed in Mauville), so a Swampert crosses the Center-less stretches (Route 111 north to Route 113; the cable car to Mt. Chimney's admins). Act 5's route: the Go-Goggles on leaving Flannery's gym, Lavaridge's Center, Route 112's one-way ledges down to Route 111, past the rock southward to Mauville, Route 117, Verdanturf, Rusturf Tunnel (HM04), Route 116, Rustboro's Center, Route 104 and Petalburg Woods, Petalburg, the gym, Wally's house (HM03).

Their constants are checked against the decomp headers (`m8_k_goals`: 40 constants). A spine `Expect` looks its goal up in the run's act first, then in the build's other acts (`goals::find`).

**Explorer-side changes the spine needed.** None of them touch the engine.

New steps:
- `Step::Bypass(&Bypass { issue, features, set, clear, vars, warp, teach })`. It writes flags and vars, writes moves over the lead's slots (`teach`), debug-warps if asked, and plays no input. `explore spine` lists the bypasses and the frame up to which the inputs reproduce. `SpineRun::bypasses` and `SpineRun::first_bypass_frame` expose them. The parity ledger reads `features`.
- `Step::WaitFor { local_id, x, y }`: wait until an object stands on a cell. Used for Mr. Briney's looping Peeko.

Planner (`path.rs`):
- **Elevation.** The searches carry the player's elevation: the start comes from the snapshot or live object, a step sets it to the new cell's, and a step to or from a 15 cell keeps it. A step is refused on `IsElevationMismatchAt`. This replaces decision 16, which made the Route 109 beach stairs and Route 110's under-bridge passage false `PlannerDivergence`s. `a_planner_divergence_is_flagged_after_a_stall` (stub) is now `#[ignore]`: the stub has no planner-blind obstacle left.
- **Warp behaviours.** A warp behaviour with no warp event is a floor. Route 110's `MB_BRIDGE_OVER_OCEAN` under the cycling road is the case.
- **Muddy slope.** Only a step down onto a muddy slope is allowed (Granite Cave).

Spine execution:
- `Talk` walks next to an unspawned object's template cell first, since Gen3 spawns objects only near the camera (Stern's Shipyard's Dock).
- `Talk` retries when a wild battle interrupted it.
- The talk macro presses A only when both the player and the object are settled.
- `Goto` retries a `NoPath` after a short wait, for wandering NPCs.
- `Train` resets its dry counter on success, and wild-grass walks get 9,600 frames.

Policy:
- A multichoice moves its cursor to `Choose(n)`. It was previously answered as yes/no.
- `Step::TrainFresh`: `Train` that also stops at half HP or under 20 PP, so the spine heals between rounds.
- `Enter` on a warp to another cell of the same map (the Petalburg gym's doors) is done once the player stands on its destination; it used to retry, from the next room.
- `policy::YES_ONCE` (`Choose(3)`): yes to a macro's first yes/no, no to the later ones. The rest stop's old lady offers again after every yes; always answering yes loops.
- `BattlePlan::BestLearn` (`Opts::learn`) learns a level-up move when it hits harder than the weakest known move, and forgets that one. Act 1 keeps `Best`, so its frames are unchanged. Without it, the level-30 Marshtomp never learnt Take Down and lost the Route 110 rival battle to Grovyle.

**Explorer gaps worked around in the spine:**
- `Talk` gives `NoPath` on Mr. Briney on the Dewford dock and on the Rusturf grunt. The spine uses `Use` from a hand-picked cell instead.
- Mr. Briney's and Peeko's `WALK_SEQUENCE` loops never stop. The spine blocks Peeko's path with `WaitFor` and a step.
- `GotoAvoiding` finds no path around a trigger that every way in crosses by design.
- Route 110's trainers drain the lead. The spine fights them first, heals in Slateport, then triggers the rival.
- Level 31 is trained a level at a time, with a Center heal after each: one level uses up Tackle's PP or Take Down's recoil.
- `Talk` on an unspawned object picks the nearest side of its template cell that is no other template's home (Wally's uncle stood on the far side).
- Route 113's first crossing whited out at 47 HP: the spine heals at the Route 111 rest stop first. Lavaridge cannot heal earlier: Route 112's west ledges are one-way.
- `Cross` of Route 113 from Fallarbor timed out at (0,11) while Route 113's two beaten trainers walked up and down column 7; a `Goto` to (14,13) first gets through.
- Route 111's rest stop: the planner's direct walk stalled at (21,12); `Goto`s to (28,11) and (29,17) first.

**Not covered.** Past the first bypass the inputs do not reproduce from a new game; a branch's origin checkpoint does. Surf (HM03 in hand at the spine's end) is the next gap: no field Surf, and `checkpartymove` is stubbed.

**Branches** (`cargo xtask explore branch` on the badge5 spine, fresh db): 1,277 branches from 60 origins, 8.75 M frames in 1.3 s; 783 rejoined, 64 diverged as expected, 62 diverged, 75 stranded, 234 aborted, 59 unplayable; 137 findings (62 divergences), 832 reports. The non-report findings:
- Softlocks on entering any Battle Tent lobby (Slateport 0x0902, Fallarbor 0x0501, Verdanturf 0x0600, at (6,9)): the lobbies' `ON_FRAME` table runs `frontier_getstatus` (`SPECIAL_CallFrontierUtilFunc`, unsupported) while `VAR_TEMP_CHALLENGE_STATUS` is 0, so the script never sets it and reruns every frame. A content/engine gap (frontier utility functions), not the explorer's. Fixed in Wave C (spine warps): `GET_STATUS` sets `VAR_TEMP_CHALLENGE_STATUS` to `CHALLENGE_STATUS_NONE` (0xFF), exact with no challenge saved.
- Divergences "did not rejoin" after `woods` (Route 104 (39,63)) and after `badge3` (Route 111 (21,12), Fiery Path (26,36)): a branch's walk leaves a state where the next spine step fails (`NoPath` on Route 104's `Cross`; the rest stop `Enter` stalling at Route 111 (21,12), where the spine itself needs two `Goto`s). Explorer-side (rejoin and planner), not engine.

### Task S2 — the spine from Fortree to the Hall of Fame (branch `worktree-agent-a57efdb12e72163ac`)

The spine plays to `hall_of_fame` (1,256 steps, 2,850,424 frames, 2.1 s), with 18 bypasses (the rows of section 2). Goals: Act 6 to the League (`ever_grande`, the four Elite Four, `wallace_defeated`, `champion`, `hall_of_fame`). Act 1 is unchanged (badge1 at frame 104,879).

- **`Step::Path`**: raw steps in order, where a shortest path will not do: Sootopolis Gym's thin ice (every cell of a room once, Gen3's `VAR_ICE_STEP_COUNT` thresholds) and the Victory Road B2F waterfall surfed down.
- **`Bypass::buy`** (`spine::buy`, unit-tested): items into the checkpoint's bag at their price, refused short of money.
- **Policy**: `best_selectable_slot` skips the moves the battle refuses (`engine::battle::Battle::move_selectable`: Torment, Disable, Taunt, Imprison). Sidney's Shiftry's Torment otherwise looped the move menu to a macro timeout. No frame changes before that.
- **HM04** forgets Muddy Water (Surf outdoes it), not Earthquake: the lead's only non-Water attack for the League. Frames move from badge4 on.
- **The League** is a lone Swampert with 3 HMs, no switching and no items in battle: 55 PP for 26 foes and their Full Restores. It trains to 96 on Victory Road's last cave (30 `TrainFresh` rounds healed at the League's counter, about 1.4 M frames), drinks a Full Restore after each Elite Four room, and the Max Elixir (picked up on Victory Road 1F (40,26)) before Wallace. At 63, Sidney won; at 80 Glacia; at 90 Wallace (Gyarados).
- Explorer notes: Route 128's west and Route 129's south are cut off by reefs (crossings at (40,39) and (0,23)); talking while surfing needs landing first; Victory Road's 1F exit (39,5) is a south arrow warp (`Enter` with `Down`); the League's door guards need talking to first; Juan's gym is left by falling through the ice to B1F.

**Branch triage** (fresh db, 2,751 branches from 170 origins, 18.4 M frames in 4.5 s; 1,562 rejoined, 257 diverged as expected, 134 diverged, 104 stranded, 584 aborted, 10 bug, 100 unplayable):
- The 10 bug branches are all from step 824, Space Center 2F (1,7): `PlannerDivergence`, held toward a walkable cell without moving. The multi battle left the objects frozen (`steven-multi`, engine). Not the explorer's. (Fixed in Wave F, mart and Steven: the player was frozen too.)
- "Did not rejoin" divergences come in two kinds, both explorer-side:
  - **The branch sets a flag the spine never does** (`after step N: flag …`): a trainer the walk crossed (flags in the trainer range 0x500–0x85F: devon_goods 0x618, badge6 0x57f/0x695, aqua_hideout 0x6c2, seafloor_cavern 0x6c5, badge3 0x6d7) or another side flag (badge6 0x168/0x173). The story did not change. Fix: the rejoin should treat trainer and item flags as expected divergences.
  - **The next spine step fails from where the branch left the player.**
    - Route 104 `Cross` `NoPath` (woods).
    - Route 111's rest-stop `Enter` stalling at (21,12) (badge3; the spine itself needs two `Goto`s there).
    - Route 119's `Use` at (1,80) never reached (badge4).
    - Mossdeep's Space Center `Enter` (badge7).
    - Seafloor Cavern's `Enter`/`Use` (hm_dive).

    Fix: planner/rejoin.
- The Route 111 round-trip finding (origin 448) did not come back on this spine. The remaining round-trip risk is the layout override above (engine).

### Wave F — explorer rejoin (the explorer's own false findings)

**Measured** with `explore branch` on a fresh db, before and after, on master's explorer with F's spine fix laid over the working tree (`worktree-agent-a9dbfbbf12e1b72b8`'s `spine/emerald.rs`, to the Hall of Fame, 2,804,579 frames; not committed here). Master's own spine stops at step 373 (the Route 113 whiteout); on it (49 origins), before → after: 598 → 647 rejoined, 221 → 184 diverged as expected, 4 → 0 diverged, 12 → 12 stranded, 154 → 155 aborted, 56 → 16 unplayable.

| outcome (167 origins) | before | after |
|---|---:|---:|
| branches | 2,850 | 2,772 |
| rejoined | 1,460 | 1,662 |
| diverged as expected | 320 | 304 |
| diverged | 85 | 11 |
| stranded | 107 | 45 |
| aborted | 705 | 709 |
| unplayable | 173 | 41 |
| new things | 10,832 | 11,304 |

The spine itself is unchanged (2,804,579 frames, the same positions): no checkpoint or golden moves.

**What is built** (`branch.rs`, `key.rs`, `path.rs`, `policy.rs`):
- **Side flags** (`key::SIDE_FLAGS`, `side_flag`): Match Call registrations, hidden items, item balls, beaten trainers (`TRAINER_FLAGS_START..+863`) and the daily flags are not spine flags, so a branch that only sets them rejoins (devon_goods 0x618, badge4 0x177, route119 0x513, badge6 0x695/0x173/0x5e2, badge8 0x742). One the spine sets later, set ahead by the branch, makes a failed window expected ("side flag …").
- **Unspawned objects** (`path::reach_all`, the branch candidates): Gen3 spawns object events near the camera only, so a target behind a boulder or an NPC off screen was planned through it and stalled (`NoPath`: 110 `cells` targets; Fiery Path's boulders, Mossdeep). Each template whose hide flag is clear and that is not spawned now blocks its cell (moved by the template deltas).
- **Window retry**: a window step that fails without a finding is retried once from the spine's own cell before it (`back_to`: a `GotoAvoiding` around the coord triggers that would run, else a `Goto`, then the facing). Route 111's rest-stop `Enter` was walking into the vicious sandstorm trigger at (21,12) (`Route111_EventScript_ViciousSandstormTriggerLeft`) until the macro timed out (32 branches after badge3).
- **Recovery**: the neighbours nearest to the origin's map first (`toward`, maps not yet visited first), not only the one on the shortest static path; back in by a crossing whose arrival reaches the origin's cell carrying the player's elevation (`crossings_into`: a surfer crossing from Route 124 at the middle of the edge landed on Mossdeep's west shore, out of the origin's water); and on the origin's map, out and back in while its cell is out of reach (`near_origin`).
- **Whiteouts**: a branch whose exploration lost a battle and then cannot get back is an expected divergence, not stranded (hm_fly: the spine leaves the lead at 0 HP there, so every wild battle whites out to Fortree, 46 branches).
- **Policy**: `first_damaging_slot`'s caller skips the moves the battle refuses (`first_selectable_slot`, as S2's `best_selectable_slot`).

**Tried and dropped:** a surfer's dismount in the planner (Gen3 `CanStopSurfing`, water 1 → land 3): branches then landed on islands and could not get back into the water (no Surf macro), 221 divergences. Nearest-first chaining: more novelty in fewer frames, but more low-novelty aborts.

**Left** (2,772 branches):
- **Aborted (709)**: 653 are the design's low-novelty rule. 56 are macro timeouts, most with a script running and no text or battle, engine-side: `Route117_EventScript_Anna`/`_Girl` (a double battle, 20), Sootopolis Gym B1F (about 30: its trainers `_Connie`, `_Daphne`, `_Annika` spotting the player on the ice, and talks there), `Route123_OnTransition` (1), a trainer battle over its frame cap in Victory Road (1).
- **Unplayable (41)**: `Enter` (1,6)/(1,7) Down on the Pokémon Centers' 1F, "no warp" (16, engine or content); `Talk` `NoPath` toward wandering NPCs (14); no encounter on cave floors in 10 steps (5); objects gone (3).
- **Diverged (11)**: Route 121 `GotoAvoiding` blocked by trainers that stopped in the way after spotting the player (Gen3 keeps them there until the map reloads).
- **Stranded (45)**: rooms the recovery cannot walk back through: the Mt. Chimney cable car station (12), Aqua Hideout B2F's warp pads (10), the Petalburg Gym's doors (6) and Lavaridge Gym's holes (4) (same-map warps and one-way drops it does not use), Seafloor Cavern's rooms (6), Fortree (3); 3 more `Goto`s left the map.
- **Stub world**: `key::EXCLUDED_VARS` is the same list on the stub (it was empty): the engine runs the step counters there too, so `VAR_POISON_STEP_COUNTER`/`VAR_DAILY_WILDS` made every stub branch diverge (`spine_branch` 5 tests and `h_tuning` failed on master's stub build; they pass).

### Wave C — spine warps, doors and field controls

**Spine.** 478 steps and 769,586 frames (`EMERALD_FRAMES`); `badge1` unchanged (step 81, frame 104,879). The `cable-car`, `lavaridge-gym-warps` and `petalburg-gym-doors` bypasses are gone; three remain (§2).

**What is built.**
- **Engine: Gen3 `previousElevation`.** `Object::prev_elevation` keeps the last elevation that was not 0 or 15 (`ObjectEventUpdateElevation`), and warp/coord/bg matching uses it (`PlayerGetElevation`); a map load resets it from the arrival cell. The Lavaridge gym's holes are elevation-0 cells under elevation-3 warps, so "enter: no warp" was this, not the behaviours. The snapshot's elevation byte packs `elevation | (prev + 1) << 4` when they differ (`ObjectSnap::current_elevation`); the hash adds `0xE0 | prev` only when the elevation is 0 and the previous one is not.
- **Specials (content):** `CableCarWarp` (the station from `VAR_0x8004`, (6,4)), `CableCar` (degraded: fade to black, 602 frames, the warp), `PetalburgGymUnlockRoomDoors` and `PetalburgGymSlideOpenRoomDoors` (`sPetalburgGymSlidingDoorMetatiles`, both door halves impassable, 2 frames per slide frame), `CallFrontierUtilFunc` (`GET_STATUS` exact with no challenge; `GET_DATA` degraded to 0; `SET_DATA` degraded; the rest report their `FRONTIER_UTIL_FUNC_*` name), `SetSootopolisGymCrackedIceMetatiles`.
- **Holes and step callbacks:** `warphole` lowers to `Ctx::warp_hole` (MAP_UNDEFINED: `None`, the map's hole warp); `setholewarp` and `setstepcallback` are `Ignore` because `GameData::hole_warp`/`step_callback` answer per map (`prelude::puzzle`: Granite Cave B1F, Mirage Tower 2F/3F, Mt. Pyre 2F, Sky Pillar 2F/4F cracked floors; the Sootopolis gym's ice with `sSootopolisGymIceRowVars`); `fall_script` is `EventScript_FallDownHole`.
- **Spine:** the Lavaridge gym through its holes and geysers (eight `Enter`s); the cable car by talking to the attendant; Petalburg's doors through their signs (`Use` + `Choose(0)`), beating the room trainers on the way; a `Goto` on Route 117 before its `Cross` (the old path drew a Lydia battle that whited out).

**Not done.** The cable car's ride scene (`cable_car.c`) and the Battle Tent challenges themselves; the ash, Fortree/Pacifidlog bridges and truck step callbacks; the explorer's planner still treats cracked floors as plain floor.

### Task O2 — the vanilla spine and branches (branch `worktree-agent-a3d3b14cc3c791e54`)

**What is built.**
- **`vanilla::Live`** expands `spine::Step`s and branch macros on BPEE, all from RAM:
  - walks plan with our planner over our map data, with the objects from RAM (`walk_to`);
  - messages are answered with A; yes/no and multichoice follow `Opts::choice`. The multichoice cursor is `sMenu.cursorPos`. When the same script asks the same multichoice again, the next item is picked (the Rustboro school tutorial). `YES_ONCE` answers yes to the first yes/no only;
  - battles use the first damaging move, `Best` or `BestLearn`, read from `gBattleMons` and the cursors. A lost battle is rerolled from a save state. The learn prompt (0x5A) and the summary screen's forget cursor (`sMonSummaryScreen`, newMove at +0x40C4, cursor at +0x40C6) follow `policy::forget_slot_of` on the lead, and never forget an HM (Gen3 `IsHMMove2`);
  - `Train` and `TrainFresh` run until the lead's level holds; `TrainFresh` also stops at half HP or under 20 PP. `WaitFor` polls the object in RAM;
  - `Talk` tries every approach, doors last. For an object Gen3 has not spawned (off camera), it first walks to the sides of the object's template cell. Match Call (`ExecuteMatchCall`) counts as a message. The PokéNav tutorial (`CB2_Pokenav`, mode `FORCE_CALL_READY`) is passed with A, and DOWN every 48 taps;
  - **`Bypass` steps are played natively.** With no warp, nothing is played. A trip to another map (the Briney sails, the cable car) is played with the scripted answer in `NATIVE_CHOICE`. A target inside the same building (a gym) is found by a DFS over save states: it steps on floor triggers and indoor warps, and talks to the objects next to the target. The DFS goes at most 5 deep and tries at most 150 moves, nearest first. A teach bypass writes the move into vanilla's lead (`Vanilla::teach`: the Attacks substruct, re-encrypted with its checksum, through the new `Emu::write_bytes`). After search battles that our bypass warps past, the party is healed (`heal_party`).
- **`cargo xtask vanilla spine`** (`vanilla::oracle::run_spine`):
  - boots BPEE, plays the intro and aligns it on our position 0, then plays both spines step by step;
  - prints the `Semantic` diff at every boundary, with the deltas new at that boundary, the frames, any vanilla failure and the bypass note;
  - stores each new delta as a `FindingKind::Parity` finding, signed on the step and the field (`--no-db` skips the db);
  - stops after 4 failed vanilla steps in a row;
  - prints the frames per segment.
- **`cargo xtask vanilla branches`** reruns the spine with a save state at every position. It then replays every `rejoined` and `diverged_expected` branch's action and macros on both games, from that position. It diffs the end states relative to the origin (deltas already present at the origin are left out), and stores each delta as a `Parity` finding.

**Spine (badge 5 + HM03 spine, 465 positions).** The last run (`--no-db`) compared **all 465 positions** in 712 s:
- 4 vanilla steps failed (Meteor Falls `Goto`, Flannery's `Talk`, the Rusturf talk after Wanda's scene had already played, and Wally's house `Enter` from inside the Petalburg gym), and the run found 299 Parity deltas;
- frames: ours 757,083, vanilla 1,040,909, plus 15,680 for vanilla's intro up to the start;
- **badges 1, 2 and 3 are reached on vanilla**, and so are the Verdanturf/Rusturf scene (Wanda, HM04) and the trip back to Petalburg;
- **badges 4 and 5 are not**: the Lavaridge geyser search and the Petalburg gym doors search both failed;
- every diff on the way is triaged below.

The earlier db run, which stopped at 447 in Rusturf, stored 271 Parity findings.

Vanilla took 0 to 52% more frames per segment:
- start, oldale_center, may: 36–45%;
- petalburg, rustboro, badge1, devon_goods, dewford: 35–52% (Gen3's walk speed and text, a parity gap only if we choose to count frames);
- lab, pokedex, woods, badge4: about equal.

**Bypasses played natively on vanilla:**
- `mauville-gym-barriers` (before Wave C4 played it): the search found the switches at (4,12), (3,9) … (8,9). Vanilla then lost to Wattson at 40 HP after the gym trainers, which is what `heal_party` is for.
- `teach-hm`: written into the lead.
- `rock-smash-forgotten-south`: reached by talking to the rock at (18,101).
- `cable-car`: the native ride lands at (6,7) on `MAP_MT_CHIMNEY_CABLE_CAR_STATION`. Ours lands at (6,4) on the Route 112 station. Accepted on the map.
- `lavaridge-gym-warps`: **not found**. A B1F geyser does not fire on a walk. The Flannery `Talk` then fails, so every diff from position 418 on is a consequence.
- `rock-smash-forgotten-rusturf`: in the first run, the bypass accepted the player's own cell and never smashed the boulder, so vanilla stayed in Rusturf. It now talks to the object next to it first. Vanilla then smashes the rock and plays Wanda's scene inside the bypass (HM04, the tunnel opened), one step earlier than ours, which is a boundary-timing difference. The spine's following `Talk` fails only because that scene is already over.
- `petalburg-gym-doors`: **not found**. The room doors open after the room's trainer is beaten, and the search only talks to objects next to the target, so it never finds the trigger. Vanilla stays at (4,106), so Norman's badge and HM03 (458 to 464) are oracle consequences.

**Spine diffs, triaged.**

| Diff (positions) | Class | Feature / cause |
|---|---|---|
| `VAR_DAILY_WILDS` ours 0 at every boundary, vanilla counting up (6 → 274) | **engine gap** | `IncrementDailyWildBattles` (`battle_setup.c`, every wild battle start) is never called by our engine. `content::new_game` sets 1 only for the skipped rescue. |
| Party: ours forgot Rock Smash (249) for Muddy Water (330) at 384; vanilla kept it | **engine/explorer gap** | Gen3 `IsHMMove2` refuses to forget an HM; our learn prompt and `policy::forget_slot` allow it. The spine's `rock-smash-forgotten-*` bypasses exist only because of this. |
| `FLAG_ENABLE_ROXANNE_FIRST_CALL` set / `FLAG_ENABLE_ROXANNE_MATCH_CALL` clear (182); the same for Wally (`FIRST_WALLY_POKENAV_CALL` vs `WALLY_MATCH_CALL`, 262) | **engine gap** | The step-count calls in `TryStartStepCountScript` (`field_control_avatar.c`). `ShouldDoRoxanneCall` and `ShouldDoWallyCall` (`field_specials.c`) fire after 250 steps on a town or route while the first-call flag is set. They start `RustboroCity_Gym_EventScript_RegisterRoxanne` (and Wally's equivalent), which swaps the flags. Ours runs only the friendship step counter there. |
| `FLAG_REGISTERED_TRENT`, `_LYDIA`, `_ISAAC`, `_KAREN` (363, 437, 445) | expected (RNG) | The trainers registered after a battle depend on which trainers saw the player (walks and wanderers differ). Follows from trainer-flag differences (below). |
| Trainer flags 0x75B, 0x768, 0x6DC, 0x7EA/0x7EB, 0x6D8, 0x71A/0x721, 0x618/0x769/0x777, 0x647, 0x64E, 0x773 | expected (RNG) | `TRAINER_FLAGS_START` + trainer id: a different trainer battled on the way, depending on positions and wanderers. Money follows: ±64 to 1,044, and up to 2,640 after the Flannery failure. |
| `FLAG_MIRAGE_TOWER_VISIBLE` (396) | **engine gap** | `SPECIAL_SetMirageTowerVisibility` (`field_specials.c`, from the Route 111 on-load script) is `Unsupported`. |
| Boundary timing: `VAR_BIRCH_LAB_STATE` (6), the Pokédex flags and Poké Balls (27), Petalburg gym Wally (45), PokéNav/Devon flags (99), Rustboro Match Call/Route 104 (105), the Briney sail (119), Scott (174) | expected | Vanilla's `settle` runs the next frame/trigger script inside the step, while ours stops at the step's own end. The state matches at the next boundary that has no pending script. |
| Cell/facing ±1 after `Train`/`TrainFresh` (139–344), on talks (8, 164, 252, 356), on Meteor Falls `Goto` (380) | expected / oracle | Grass walks end on the encounter cell. A talk approach picks another side. At 380 vanilla stopped at (13,18) because a scene blocks the cell. |
| `party[0].exp` (13), `party[0].moves` slot 3 (38), `party[0].species` 284 vs 285 (332), HP buckets | expected (RNG) | Wild levels and the timing of level-ups and evolutions under `~lv`/`~hp` tolerance. |
| `object 10`/`19` present in ours, absent in vanilla (46, 64) | expected | Gen3 spawns objects only around the camera (O1's tolerance note). |
| Cable car (403): map, cell, `VAR_CABLE_CAR_STATION_STATE` | oracle | The native ride versus our bypass warp. Accepted on the map. |
| Flannery's badge and Go-Goggles (417 to 423, carried on), Norman's badge and HM03 (457 to 464) | oracle | The Lavaridge geyser and the Petalburg gym doors searches failed, so vanilla never fought Flannery or Norman. |

**Branches** (badge 5 spine db: 789 replayed, 58 more start past vanilla's last position): 568 equal, 119 with deltas, 102 vanilla failures; 788 Parity findings.
- **Top deltas, by count:**
  - cell 184, facing 100, map 54: mostly the vanilla failures;
  - `VAR_DAILY_WILDS` 129: the engine gap above;
  - money 108: RNG trainers;
  - party HP 34: RNG;
  - `FLAG_RECEIVED_TM_BULLET_SEED`/`TM09` 30: origin 64, where vanilla's talk to object 17 failed;
  - `FLAG_LANDMARK_TRICK_HOUSE` 17: origin 232, where vanilla found no path to the Trick House door (`Route110_TrickHouseEntrance` sets it);
  - `VAR_DAILY_PICKED_BERRIES` + `ITEM_ORAN_BERRY` 14;
  - `FLAG_0x5E8`/`0x7BD` 10;
  - `FLAG_MOVE_TUTOR_TAUGHT_ROLLOUT` 6;
  - `FLAG_LANDMARK_TUNNELERS_REST_HOUSE`, `FLAG_LANDMARK_FLOWER_SHOP`, `ITEM_OLD_ROD`: a few each, all on branches where vanilla failed.
- **One engine gap among the branch deltas: berry trees.** Vanilla picks Oran Berries on Route 102/104; ours finds the soil empty. The berry-tree specials (`IncrementDailyPickedBerries`, `ObjectEventInteractionGetBerryTreeData` and the rest of `berry.c`) are `Unsupported`, and the trees start without the map's initial berries.
- **Vanilla failures (oracle):**
  - `Talk` "never next to object 17" (30) and "object 8" (24): trainers and wanderers block every approach;
  - `walk_to` no path on Route 110 (16), in Birch's lab (12,8) (7), and once on map 7436;
  - about 14 `Screen` stalls on callback2 0x0811A278 (to identify; not a field screen the driver knows).

**Wave C gap list** (engine; none fixed here):
1. `VAR_DAILY_WILDS`: `IncrementDailyWildBattles` on every wild battle start.
2. HM forgetting: Gen3 `IsHMMove2` in the learn-move prompt and in `policy::forget_slot`. Fixing it removes the spine's `rock-smash-forgotten-*` bypasses.
3. Berry trees: the initial berries and the `berry.c` specials (`VAR_DAILY_PICKED_BERRIES`).
4. The step-count scripts of `TryStartStepCountScript`, with the registration scripts they start: `ShouldDoRoxanneCall`, `ShouldDoWallyCall`, `ShouldDoScottFortreeCall`, `ShouldDoScottBattleFrontierCall`, `ShouldDoRivalRayquazaCall` and `ShouldDoBrailleRegicePuzzle`.
5. `SetMirageTowerVisibility`.

**Vanilla driver problems left:**
- Lavaridge gym: the geyser warps do not fire on a walk.
- Petalburg gym: the search has no move that opens the room doors (beating each room's trainer).
- Meteor Falls: a scene blocks (14,18).
- `Talk` blocked by trainers and wanderers.
- The 0x0811A278 screen.
- `walk_to` has no `setmetatile` overrides from RAM.

**Checkpoints and goldens:** unchanged. No engine change. In `explore`, `policy::forget_slot_of` is a refactor with the same behaviour. `emu::write_bytes` is new and used only by the oracle.

#### Wave E — the vanilla driver through Fortree

`cargo xtask vanilla spine --no-db` now plays **all 550 steps to Fortree with 0 failed vanilla steps**, in about 290 s (was 712 s for 465 positions): ours 809,595 frames, vanilla 1,190,666 (+15,680 to the start), 2 lost battles rerolled.

**Driver changes** (`crates/vanilla`, no engine change):
- `Step::UseItem` / `MacroKind::UseItem` are played (`Vanilla::use_item`): START, the start menu's cursor to BAG (`sCurrentStartMenuActions`, `sStartMenuCursorPos`), the bag's pocket and list position (`gBagPosition`, the pocket's slot order from the save block), USE (`Task_ItemContext_*`, `sMenu`), the party menu's `gPartyMenu.slotId`, the summary's forget cursor, YES through the prompts, then B out of the bag and the start menu. HM06 and HM03 are taught natively. The symbols are pret's `symbols` branch listing (it matches the committed table).
- In battle: "Will you change Pokémon?" (`yesnobox`, trainer battles) is answered NO and "Use next Pokémon?" YES; the party menu's choose-mon picks the first mon that can fight. The Route 119 rival battle used to loop in the party menu ("already in battle").
- A `Goto` whose target is a trigger counts as reached when the scene moves the player off it (Meteor Falls' Magma scene).
- An `Enter`/`Cross` whose destination vanilla's settle already reached (Norman's win runs the Wally's house scene) is done, and noted.
- The holes, geysers and door signs are ordinary `Enter`/`Use` steps now; the bypass DFS is no longer used by the spine.
- **Speed:** no scanline rendering (`Emu::set_frameskip`; `PKR_VANILLA_RENDER` keeps it); `--cache DIR`, `--from N` and `--to N` replay one stretch from saved positions in seconds.

**Noise kept out of the findings** (`vanilla::oracle`):
- **Boundary timing.** A delta becomes a finding only if it still holds after the next step that moves the player. Waits (`WaitScript`, `Wait`, `WaitFor`) and steps vanilla had already played extend the window, and the player's place is checked at the last wait.
- **RNG trainers.** When the games beat different trainers whose sight depends on the RNG or the timing (their movement type turns or walks), those flags, money, `FLAG_REGISTERED_*` and the lead's exp, level and HP are noise. Fixed-facing trainers stay findings.
- **Levels.** Once a random battle was fought in either game (ours gains exp in a step where vanilla fought nothing), these are noise: evolution timing (species A vs its evolution) and move-slot differences between level-up moves.
- **Talk approaches.** The cell and facing next to the object after a `Talk` (and its scene's waits) are noise.
- **One finding per field.** `ParityLog` reports a field that keeps differing once, even as its values move (`VAR_DAILY_WILDS` counting).

**The Parity findings (14) of the last run, by root cause:**

| Position | Delta | Root cause |
|---|---|---|
| 6 | `VAR_DAILY_WILDS` 0 / counting | engine gap: `IncrementDailyWildBattles` (`battle_setup.c`) |
| 105 | `FLAG_ENABLE_MR_STONE_POKENAV` clear / set | engine gap: the PokéNav tutorial's call to Mr. Stone sets it (`pokenav_match_call_data.c`, `MatchCall_Text_MrStone1`'s flag to set) |
| 182, 262 | Roxanne and Wally `FIRST_CALL` set / `MATCH_CALL` clear | engine gap: the step-count scripts (`TryStartStepCountScript`, `ShouldDoRoxanneCall`, `ShouldDoWallyCall`) |
| 365, 447 | `FLAG_MIRAGE_TOWER_VISIBLE` | engine gap: `SetMirageTowerVisibility` |
| 380 | cell (14,18) / (13,18) at Meteor Falls | **likely engine gap**: the scene's `MeteorFalls_1F_1R_Movement_PushPlayerOutOfWay` (`lock_facing_direction`, `walk_left`) leaves our player on (14,18) |
| 429, 430 | `FLAG_0x5C9` (Cole), `0x788` (Gerald), `0x5CB` (Axle) clear / set | **likely engine gap**: Lavaridge gym's `TRAINER_TYPE_BURIED` trainers (sight 1) fight vanilla on the same `Enter` walks, but not ours |
| 486 | facing Down / Right after Norman | boundary timing: vanilla's settle played the Wally's house HM03 scene inside the talk. The facing after that scene differs (to check) |
| 515 | object 2 present / absent (Route 118) | camera presence (O1's tolerance note) |

**Left:** the branch replays (`vanilla branches`) use the same noise rules but were not rerun here (no explore db in the worktree).

### Wave C — new-game state parity

**What is built.**
- **`EventScript_ResetAllMapFlags` comes from the decomp.** `extract/tests/names.rs` reads its `setflag`s from `data/scripts/new_game.inc` into `names::RESET_ALL_MAP_FLAGS` (159 names, guarded by the drift test like the other decomp tables). The content build emits `consts::RESET_ALL_MAP_FLAGS: &[FlagId]`.
- **`content::new_game` (Emerald only) runs `NewGameInitData`, then the skipped intro.**
  - `NewGameInitData`: every `RESET_ALL_MAP_FLAGS` flag, `SetMoney(3000)`, `VAR_SEEDOT_SIZE_RECORD`/`VAR_LOTAD_SIZE_RECORD` = `DEFAULT_MAX_SIZE` (0x8000), and `NewGameInitPCItems` (a Potion in the PC).
  - The male intro up to meeting May: `SetIntroFlagsMale`, the truck and the Vigoroths, `FLAG_SET_WALL_CLOCK`/`FLAG_SYS_CLOCK_SET`, `FLAG_SYS_TV_HOME`/`FLAG_SYS_TV_WATCH`, `FLAG_MET_RIVAL_MOM`, `FLAG_DECORATION_1..14`, May's 2F Poké Ball. It clears `FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN` and May's bedroom flag, as the intro does.
  - `Start::AfterStarter` adds the rescue and the lab, clears `FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH` and `FLAG_HIDE_ROUTE_101_BOY`, and sets `VAR_DAILY_WILDS` 1 for the rescue's skipped battle.
- **Engine:** `Request::PcItemAdd` (`Ctx::pc_item_add`, hash tag 100) and `PcStorage::add_item` (Gen3 `AddPCItem`: top up to 999, then the first free slot, all or nothing).
- **Oracle:** `vanilla/tests/intro.rs` now asserts both diffs. At our new game's start only `cell` differs: we stand outside Brendan's house, and vanilla has just left May's. After the starter only the placement differs: map, cell, facing and the objects, because vanilla stands in the lab.
- **Spine:** the `new-game-map-flags` bypass and its blocker row are gone. The objects now hidden from the start change which wanderers draw from the RNG, so the battles from Act 1 on play differently. Three spots then lost a battle, and the spine now steps around them:
  - a heal at Oldale's Center before Route 102;
  - `GotoAvoiding` past the Route 116 School Kid's sight (her Shroomish);
  - one more level (33) on Route 110 before Wattson.
  
  `EMERALD_BADGE1_FRAMES` is now 104,335 and `EMERALD_FRAMES` is 455,512.

**Not done.**
- `addpcitem`/`checkpcitem` in scripts are still `Default(0)` (`commands.ron`). The request now exists for lowering them.
- `EventScript_ResetAllBerries` (berry trees) is not run. The engine has no berry trees.
- Options (`SetDefaultOptions`) are left as they are, because the engine's defaults are Gen3's (text speed MID).
- The engine never counts `VAR_DAILY_WILDS` on a played wild battle.

### Wave C — field moves

**Spine.** 550 steps and 809,111 frames (`EMERALD_FRAMES`); badge 5 is unchanged. The two `rock-smash-forgotten-*` bypasses are gone (HM protection), and `teach-hm-surf` (step 492) is new, so two bypasses remain (§2). New checkpoints: `route118_east`, `route119`, `castform`, `hm_fly`, `fortree`. New Act 5 goals: `surfing`, `route119`, `castform`, `hm_fly`, `fortree`.

**What is built.**
- **Surf (engine).** A on surfable water with no script there runs `GameData::field_move_script(Surf)` (`EventScript_UseSurf`, extracted as a root by its signature) once `field_move_gate` allows it (Gen3 `CheckForSurfable`/`PartyHasMonWithSurf`, badge 5). `Request::FieldMoveEffect` (hash tag 160) plays `FldEff_UseSurf`'s timing: init 1, pose 26, show-mon 1+83 outdoors (1+90 indoors), then the jump onto the water (16). Surfing moves at `walk_fast`, water and land swap their collision (Gen3's elevation-1 water), the dismount is a one-cell jump (`CanStopSurfing`), and water cells draw from the map's water table (`WaterEncounters`, slots 60/90/95/99/100). The surfing state survives a checkpoint as the player's `gfx` sentinel `0xFFFE` (no schema bump).
- **Cut, Rock Smash, Strength.** `FLDEFF_USE_CUT_ON_TREE`, `FLDEFF_USE_ROCK_SMASH` and `FLDEFF_USE_STRENGTH` play the same effect timing. Boulders (`OBJ_EVENT_GFX_PUSHABLE_BOULDER`, while `FLAG_SYS_USE_STRENGTH`) slide one cell at `walk_slow` when walked into, the player stepping in place (`TryPushBoulder`). Rock Smash tables are emitted with the water ones.
- **HM protection.** The learn-move prompt refuses an HM slot with "HM moves can't be forgotten now." (`IsHMMove2`, `STRINGID_HMMOVESCANTBEFORGOTTEN`) and reopens the pick; `GameData::is_hm_move` answers from `content::data::HM_MOVES` (decomp names, HM order). The explorer's `forget_slot` skips HM slots too.
- **Content.** Surfable behaviours are `Behavior::Water(_)` (waterfalls, no-surfacing and currents approximated as plain water); HM gates and badges per HM.
- **Explorer.** `GotoAvoiding` now carries the player's elevation (bridges over paths) and never plans up a muddy slope, as `path` does. A walk's target skips the elevation rule only when nobody walks onto it (a door, an object): a walkable target across a ladder (Fortree's walkways) is planned like any cell, which removed four Fortree `PlannerDivergence` bugs.
- **Spine.** Petalburg (HM03, `teach-hm-surf`) → Rustboro → Rusturf → Mauville → Route 118 by Surf → Route 119 round every trainer's sight → the Weather Institute (its bed, the grunts, Shelly, Castform) → the rival (`BattlePlan::Fight`: with `Best`, Take Down misses Grovyle twice and Leaf Blade wins) → Fortree.

**Ledger.** Field moves 14: 0 native, 14 missing → 3 native (Surf, Cut, Strength), 1 degraded (Rock Smash: `SPECIAL_RockSmashWildEncounter` unsupported), 10 missing (Fly, Flash, Dive, Waterfall, Teleport, Dig, Secret Power, Soft-Boiled, Milk Drink, Sweet Scent).

**Branches** (`cargo xtask explore branch`): 1,598 branches from 73 origins, 10.7 M frames in 2.1 s; 861 rejoined, 237 diverged as expected, 61 diverged, 51 stranded, 308 aborted, 80 unplayable, no bug. The 61 divergences are the earlier segments' "did not rejoin" ones (woods, devon_goods, badge3, badge4); none from the new origins.

**Not done.** The surf blob and the field-move pose and show-mon sprites are not drawn; using a field move from the party menu (M9); Waterfall, Dive and the currents' forced movement; the Route 119 bridge's surfing encounters; `SPECIAL_RockSmashWildEncounter`.

### Re-baseline (as built)

The bulk checkpoint re-baseline after Waves C/D and M9 K/PT, then green gates. Re-baselined on master `4ba8485` and again on `d52336e` (O2, M9 B).

**What moved, and why.** One commit, found by bisection: master's Emerald tree verifies on `f6c75ec^` (all 9 nodes valid (rehash)) and diverges on `f6c75ec` (Wave C, new-game state parity). It diverges at c00001/c00005 from frame +60, c00006 is BROKEN (its trace ends while a script runs), and its children break with it. The stub tree bisects to the same commit. The new game now runs `NewGameInitData` and the skipped intro's flags. Money and the flags are hashed from the first frame, and the objects hidden from the start change which wanderers draw from the RNG, so the old inputs replay into other battles. Nothing after it (field effects, Surf, Match Call, abilities, move effects, M9 screens) is reached by the old traces before they break. The new nodes are recorded on the current build. On `d52336e` (M9 B's content changes the build id), every node is "valid (rehash)", and `spine --export` keeps the committed act1 chain unchanged.

- **Stub (3 nodes).** The stub runs the same new game. Its end snapshots differ by money 3000 only (the stub has no Potion). The inputs and frames are unchanged, so the nodes are accepted.
- **Emerald `route:*` (c00001–c00004).** Re-seeded with `checkpoints seed`; the scenario reaches every point, earlier: lab 5,002 → 4,570, after_wild 7,173 → 6,400, oldale_center 13,640 → 11,333, route103_after_may 19,184 → 17,019.
- **Emerald Act 1: the spine replaces `act1:*`** (explorer-spine §2.4). `act1:*` (c00005–c00009) are dropped. `explore spine --export --label badge1` now exports a **chain**: each node starts from the one before (it was one new-game trace per label). The chain is promoted parents first: `spine:act1:lab` c00005 (frame 4,768), `oldale_center` c00006 (7,695), `may` c00007 (12,226), `pokedex` c00008 (16,158), `petalburg` c00009 (35,984), `woods` c00010 (62,561), `rustboro` c00011 (63,664), `badge1` c00012 (104,879). `game-native/tests/act1/trace.rs` reads this chain (8 labels), so the parity lanes replay the spine's Act 1. Every node verifies from a fresh copy of the nodes.
- **Silice tree:** empty, nothing to do.

**Lanes** (`cargo xtask gates`, `parity`, `parity --long` on the merge of `d52336e`):
- **fmt, clippy, clippy-ui, test, test-stub, parity:** pass. **test-silice:** Silice spine tuning only (below).
- **parity:** every route and `spine:act1:*` node resumes on the ROM (2,000 frames each, hashes and pictures identical). Hash at most 47 lines (gate 65); loop at most 154 lines (gate 217), at `spine:act1:pokedex`. The native Act 1 invariant restores 108 checkpoints; the largest is 1,011 bytes (budget 24,016).
- **parity --long, `act1_long`:** frames 0–99,938 of 104,879 replay on the ROM from a new game. 99,906 hashes and 998 pictures are identical, with 32 late applies on a black screen (93 bulk events), in about 6 minutes. Hash 55 lines (gate 65), loop 164 lines (gate 217, margin 53). Heap 51,764 B, stack untouched 10,236 B. Per segment, the worst loop is 163 lines (to `pokedex`).
- **Line gates:** no gate trips and no gate was moved. The only overrun is the `KNOWN_LATE` one.
- **`KNOWN_LATE`: 81,769 (old trace) → 99,942, Roxanne's battle start on the spine trace.**
  - `BattleStarted` comes at 99,940. The step to 99,941 is `Battle::start` with the first `NewTurn`'s gym-leader AI choice: 2.96 µs native against a 0.21 µs median, the act's most expensive battle step. On the GBA it overruns the frame, and the ROM reports the late apply at 99,942.
  - On the old trace, the same start was at 80,366. It was late before `7134c3e` (223 lines), then on time at 212 lines, with a later step at 81,769 over the loop gate (225 lines).
  - On this build (after Wave C's move effects, abilities and AI cases; not bisected) the start is late again. No capturable frame follows it before the act ends, so the long lane stops there, and `spine:act1:badge1`'s resume lane covers the end.
  - **Not fixed:** a real fix would split the AI choice off the start step. That moves native timing and every hash after it (a re-record), so it is left for M9 with the other battle-cost findings.
  - The lane now prints each late apply's cost marks.
- **The tour:** 518 maps (107 gated), worst gated loop 163 lines, stretch 201 lines (4 stretch maps over budget, `KNOWN_OVER` as before), heap 36,244 B.

**Fixed on the way.**
- `emu::run_rom_from`: a black-screen late apply that steps over a picture frame skipped it instead of failing. The spine trace's late apply at 29,500 (a wild battle's load on Route 102) fell on a 100-frame picture.
- The Silice spine driver chose a Disabled move forever (Ch2's Drowzee disables Ember). `engine::Battle::move_selectable` (Gen3 `CheckMoveLimitations`: PP, Disable, Torment, Taunt, Imprison) is now public, and the driver's `best_move` uses it. The explorer's `policy::first_damaging_slot` had the same blind spot (fixed in Wave F, explorer rejoin).
- `m8_e_blob_data` checks an unused effect slot (Transform is native since Wave C2b; master fixed it too).
- `map_edit_tools::reach_tools_on_the_committed_silice_maps` runs on the Silice build only: the redrawn maps name the games dir's own tilesets.
- `spine_real::SILICE_CH0_FRAMES` 21,821 → 21,813, bisected to `2fc4c16` (Brico-Bourg redrawn: the walks are 8 frames shorter).
- Clippy (`xtask/src/silice/derive.rs`).
- On `d52336e`:
  - `vanilla::Live` did not compile against M9 B's `Step::UseItem`/`MacroKind::UseItem`; it now bails with "the bag's USE is not driven on BPEE yet".
  - rustfmt.
  - The tour's warp combo is L+R. With L+R+START, M9 B's content start menu opened on START before the shell's warp, so the tour stopped at its first map.

**Left: test-silice, Silice spine tuning (not fixed here).**
- `sil4_ch3`: a battle lost at frame 189,850 on map 112.
- `sil5_ch4`: stuck at (7,26) going down into Port-Ressac's canal (`PortRessac` (7,27), metatile 0x170, elevation 1). Since Wave C's water collision, elevation-1 water blocks on foot, and the Ch4 spine walked across it. This is the author ruling "fishable but not surfable" canal (status hand-off).
- `sil6_ch5`: a battle lost at 175,512 on map 52 (cave floors).
- `sil7_ch6`: a battle lost at 307,646 on map 115.
- `sil8_ch7`: out of frames on map 86 at (14,40).
- `sil9_ch8`: the unfinished Ch8 spine: `route_s8_palier_1` is stuck at (6,71); the chapter has no way from (33,7) to (13,16) on map 88.

`sil2_ch2` passes again (the Disable fix).

### Re-green after Wave E1 (as built)

**What moved, and why.** E1's step-count scripts run `UpdatePoisonStepCounter`/`DoPoisonFieldEffect` (field poison: a poisoned mon loses 1 HP every 4th step and faints at 0), and 2c8c2b7's guard forwards every `GameData` method. Act 1's frames are unchanged (badge1 at 104,879; the committed chain's inputs and frames are identical, `spine --export --label badge1` keeps all 8 nodes). The spine first diverges at step 183 (Route 110 `Train`, frame 267,859 → 268,775): the lead walks there poisoned, so its HP, and then every battle after, differ. On step 373 the lead reached Route 113 at 37 HP and paralysed and lost a wild battle (a Spinda) to a whiteout in Mauville, not to poison.
- **Spine tuning:** back to the Route 111 rest stop once more before Route 113 (via (21,10): the direct walk sticks at (21,11), the same stall S2's triage notes); a level before Wally (`TrainFresh` to 62 on Victory Road 1F, healed at Ever Grande's Center: at 60 his Altaria's paralysis and Roselia won); back to the Weather Institute's bed after the rival's battle, before the `hm_fly` checkpoint (the lead fainted to Grovyle; branches from there whited out at their first wild battle); the League training to 98, not 96 (with this RNG 96 and 97 lose to Wallace or Glacia on PP, 94 and 98 win: still a coin toss). `EMERALD_FRAMES` 2,850,424 → 2,992,885; `hm_fly` moves from step 553 to 560.
- **Checkpoints:** the 12 Emerald nodes diverged from frame +60 only through the state E1 adds and hashes: the new game's berry trees, `VAR_POISON_STEP_COUNTER` and `VAR_DAILY_WILDS`, and (from the woods on) field poison's HP and friendship. The 3 stub nodes diverged from frame +24 through the same counters. Inputs, frames and parents are unchanged; `verify --accept` re-hashed them (schema 5).
- **Lanes fixed on the way:** `World::hash` took 69 scanlines on the GBA (gate 65): ~90 berry trees hashed byte by byte every frame. `BerryTrees` now hashes a digest cached until the next `set`. Also: vanilla drives S2's `Step::Path`; `MAX_ENCODED` 24,528 → 24,527 − 512 (schema 5's empty tree list makes the flash probe's checkpoints odd-sized, and it grows them two bytes at a time); the export test lists `tables/berries.ron`; the stub's key excludes its poison and daily-wild counters (its grass-pocket and branch-rejoin tests saw a new "place" every step); the berry special tests expect no item without berry data (stub); clippy.
- **Not fixed (content gap):** `EventScript_FieldPoison` (`TryFieldPoisonWhiteOut`, the whiteout when the whole party fainted) is not run. The spine's lone Mudkip, poisoned in Petalburg Woods at 7 HP, faints on Route 104 north (frame ~63,500) and walks fainted into Rustboro's Center. In Emerald those inputs white out to Petalburg. Running the script would move Act 1 (badge1) and every committed node; the spine would then cure or heal before leaving the woods.

### Wave F — the mart and Steven's multi battle (as built, branch `worktree-agent-a3885ba3cfd63f4d8`)

Bypasses `league-mart` and `steven-multi` are gone (rows above, `SPINE_BLOCKERS`); 16 remain.

- **The Poké Mart (Milestone 9 Task M)**, `engine/src/ui/mart.rs` (`shop.c`): the shop menu (BUY / SELL / QUIT), BUY's list with prices, the money box, the quantity (UP/DOWN ±1, LEFT/RIGHT ±10, `AdjustQuantityAccordingToDPadInput`; at most the money's worth, 99), the confirm yes/no, the bag write, then the money; the bonus (10+ Poké Balls → a Premier Ball, `GameData::mart_bonus`); SELL opens the bag in `BagMode::Sell` (Task B). `Request::Mart { items }` → `ScreenKind::Mart`. Deviations in the module doc (no field behind BUY, no fades, no purchase record). 8 tests (`world/tests/m9_m.rs`).
- **`pokemart` native.** The extract decodes the item list a `pokemart` points at (`IrList`/`IrArg::Items`, **format minor 10**) and lowers it to `mart(&ctx, &<list>)` plus a static list; a list it cannot name still reports. Ledger: `pokemart` native (27 sites), `shop.c` native. `pokemartdecoration`/`2` stay unsupported (5 + 5 sites).
- **`Step::Buy { map, local_id, item, count }`** (`MacroKind::Buy`): talk to the clerk, BUY, scroll to the item, raise the quantity, A, yes, then B out of everything. `league-mart` is now `Goto(LEAGUE_1F, 16, 4)` (the clerk only spawns near the camera) + `Buy` of 10 Full Restores. The vanilla driver's stand-in writes the bag and the money (`vanilla::state::buy`). `Bypass::buy` is now unused by the Emerald spine (kept).
- **The frozen-player bug (every branch from step 824 a `PlannerDivergence` at (1,7)).** It was the engine: a script that ends without `release` (Steven's, once its special stopped) left the player frozen. Gen3 `FreezeObjectEvents` skips the player, so `step_script` now unfreezes the player when a script ends (`world.rs`; test `a_script_ending_without_release_frees_the_player`). `explore branch --from space_center`: 10 branches, 10 rejoined, 0 findings.
- **The choose-half party menu** (`PartyMode::ChooseHalf { max, min }`, `PARTY_MENU_TYPE_CHOOSE_HALF`): ENTER / NO ENTRY / SUMMARY / CANCEL, NOT ABLE / ABLE! / FIRST..FOURTH per slot, CONFIRM (START jumps to it, the `max`-th ENTER moves to it, fewer than `min` says "No POKéMON for battle!"), B or CANCEL asks "Cancel the battle?". It answers the order packed a nibble per entry (`pack_order`; Gen3 `gSelectedOrderFromParty`). The explorer's policy enters the HP>0 mons in party order up to `max` and confirms.
- **The party stash** (`world/host/stash.rs`, `Request::PartyStash`): `Save` (`SavePlayerParty`), `Load` (`LoadPlayerParty`), `Reduce` (`ReducePlayerPartyToSelectedMons`), `SaveSelected` (`SaveSelectedParty`, `CallFrontierUtilFunc` `FRONTIER_UTIL_FUNC_SAVE_PARTY`). A running script's transient state: not in the checkpoint, hashed only while non-empty, cleared at script end; a whiteout while it holds a backup restores the party first. 5 tests (`world/tests/wavef_multi.rs`).
- **The specials** (`content/src/prelude/special.rs`): `SavePlayerParty`, `LoadPlayerParty` (were Ignore), `ChooseHalfPartyForBattle` (sets `VAR_FRONTIER_FACILITY` 9 through the story module, max 3, min 1, `VAR_RESULT`, fade in) and `ReducePlayerPartyToSelectedMons` native. `DoSpecialTrainerBattle` with `SPECIAL_BATTLE_STEVEN` is **degraded**: the chosen mons fight Maxie (734), then Tabitha (514), as two single battles without Steven; `VAR_RESULT` is the outcome. The other battle types report `Unsupported` (the ledger keeps it unsupported: 14 of its 15 sites are the Battle Tents). Silice's story module stubs both hooks.
- **Not done:** the real 2v2 multi battle (`BATTLE_TYPE_MULTI | BATTLE_TYPE_INGAME_PARTNER`, Steven's party and partner AI). The engine has no double battles. The choose-half menu's slot pictures and icons are frames and text, as in the field party menu. The stash's whiteout path is tested in the engine only through the unit test of `Stash::run`. `SPECIAL_SetCB2WhiteOut` stays unsupported: it is unreachable, because the engine whites out inside the battle.
- **Spine:** step 824 (`space_center`) now fights both battles (about 9,000 frames more). The lead reaches Victory Road with different EXP and RNG and lost to Wally at 62 (Roselia's Giga Drain), so the Victory Road `TrainFresh` is 63. `EMERALD_FRAMES` 2,992,885 → 2,976,669 (the mart is played, not warped, and the run before Wally is shorter). `spine_real` is green to `hall_of_fame`. `spine_segment` (ignored, by hand) saves positions and replays segments; with `PKR_SPINE_TRACE=1` it prints each map/cell/battle/script change frame by frame.
- **Checkpoints and goldens (not re-accepted):** the Act 1 nodes are untouched (badge1 104,879). From `mossdeep`/`space_center` on (step 824), the Emerald spine's inputs, frames and world hashes move: the stash is hashed while the script holds it, the `ChooseHalf` mode hashes as `(6, max | min << 8)`, and the battles are real. The extract's format goes to minor 10, so the content build hash moves for every node, and the next format bump elsewhere (5.10) collides with it: whichever merges second renumbers.

### Wave F — Mossdeep's gym, the Sky Pillar, the legendaries and GameClear (as built, branch `worktree-agent-ad4739ae30efe32ca`)

Bypasses `mossdeep-gym-puzzle`, `legendaries-scene`, `rayquaza-scene`, `sky-pillar-layouts` and `game-clear` are gone (rows above, `SPINE_BLOCKERS`); what remains is Milestone 10's field moves (Fly, the water door, Dive, Waterfall).

- **The layout override in the checkpoint (schema 6).** `Field`'s `setmaplayoutindex` override (`next_layout` and the current one) is in the checkpoint and in `World::hash`; `Field::from_snapshot`'s TODO is gone. A layout the map has no variant of is no override (`Field::settle_next_layout`, after `on_transition` in both the warp and the connection paths): Route 130's `setmaplayoutindex LAYOUT_ROUTE130` is its own layout, and the content build only emits variants that differ. Tests in `checkpoint/tests.rs` (goldens) and `world/tests/wavec_field.rs`.
- **The planner walks the current layout** (`explore/src/path.rs`): the grid reads the override, not the map's own layout. It also gets a surfer off the water onto elevation-3 land (`CanStopSurfing`, `Terrain::refuses`): Route 131's Sky Pillar islet.
- **The rotating tile puzzle** (`rotating_tile_puzzle.c`): `initrotatingtilepuzzle`, `moverotatingtileobjects`, `turnrotatingtileobjects` and `freerotatingtilepuzzle` are native (Mossdeep's gym, Trick House room 7). They work on the object templates, as Gen3 does. `Behavior::PadWarp` (`MB_MOSSDEEP_GYM_WARP`, postcard index 22) warps without reloading the templates (`DoMossdeepGymWarp`, `SKIP_OBJECT_EVENT_LOAD`; `State.keep_templates`, hashed while set), so the statues keep their turn across the pads. Tests in `world/tests/wavef_rotating_tiles.rs`.
- **The specials, degraded.** `Script_DoRayquazaScene` (both ids): no scene, the field comes back faded in, the script goes on. `GameClear`: `FLAG_SYS_GAME_CLEAR`, the champion flags and the continue warp to the player's room are set, then the warp; no Hall of Fame screen, no ribbons, no credits, and no save (`world/save.rs::save_now` is still a stub). `SetChampionSaveWarp` is ignored (`GameClear` writes the warp).
- **Spine.** Mossdeep's gym is 22 paths over its switches and pads (one extra Right where the twins' double sight eats a press: the engine fights one battle per trainer). Nate is fought on the way so that Tate and Liza's battle falls on a winning RNG (the world RNG at a battle depends on the battle count), with Earthquake's PP kept for the twins. Sootopolis's legendaries scene and Rayquaza's scene are `WaitScript`. The Sky Pillar: Route 131's islet, the Entrance (Wallace), every floor, 4F's cracked floor both ways (on foot: the player falls to 3F, which Gen3 also does without a bike), Rayquaza at the top, back down and out to Route 126. The Hall of Fame is a `WaitScript` that ends in Brendan's room. Two tunings outside the bypasses, both forced by the real battles: the Nate battle above, and Sidney fought with Strength (`BattlePlan::Fight`) because Earthquake's 10 PP did not last past Phoebe and Glacia's Sheer Cold beat Strength alone. `EMERALD_FRAMES` 2,976,669 → 3,010,626; `spine_real` is green to `hall_of_fame`.
- **Ledger** (`cargo xtask ledger`, after merging master `ffdbc16`): script commands 118 native / 14 degraded / 43 ignored / 54 unsupported (the four rotating-tile commands native); specials 66 / 99 / 19 / 343 (`Script_DoRayquazaScene` ×2 and `GameClear` degraded, `SetChampionSaveWarp` ignored).
- **Not done:** the save behind `GameClear`, the Hall of Fame screen and the credits; the vanilla oracle/driver does not read the layout override; `warpmossdeepgym` (`WarpToEntrance`) is a plain warp, not a pad warp; `SpawnCameraObject`, `ShakeCamera` and `WaitWeather` stay unsupported (the scenes' camera); the twins' double sight is one battle per trainer (no double battles).
- **Checkpoints and goldens (not re-accepted):** schema 6 changes every checkpoint's encoding (the engine goldens are updated). World hashes move only on override maps, while `keep_templates` is set, and where a cell is `PadWarp`. From Mossdeep's gym on, the Emerald spine's steps, frames and labels move (+22 gym paths, Nate, the Sky Pillar and the E4 changes). `explore branch`: 3,000 branches, 10 divergences (3 signatures, all at step 896 after `hm_dive` on map 0x181C: `NoPath` back to the spine, outside this cluster).

### Re-green after Wave E2 (as built)

Merged on master `70a3d50` (Waves E2, F3 and F2). The rule now is Gen3's: field poison faints, and `EventScript_FieldPoison` whites out when the whole party has fainted.

**What moved, and why.**
- **The woods.** The fix: a Pecha Berry from Route 104's tree (24, 41), used from the bag (`Step::UseItem`) before the `woods` checkpoint. The berry pick moves the battle RNG, so in this run no Wurmple poisons the lead and the use only says it has no effect. It stays as the guard. The lead never faints from poison on the spine: no `TryFieldPoisonWhiteOut` whiteout and no whiteout at all.
- **Act 1.** The frames move: `woods` 62,561 → 62,320, `rustboro` 63,664 → 63,423, `badge1` 104,879 → 104,523. `EMERALD_BADGE1_FRAMES` is 104,523. `spine --export --label badge1` keeps c00005–c00009; c00010–c00012 are re-exported and promoted as the same ids.
- **Spine tuning for the new RNG:**
  - One more Fortree round (to 50).
  - A Center heal between the gym's trainers and Winona: goto (15, 3), out, the Center, back in.
  - The Magma Hideout's 2F 2R Full Restore and Max Elixir, used at Maxie's side (the grunts drained Earthquake and Surf).
  - Two-frame waits before Victory Road B1F's first boulder and the south rock: an A on the frame a walk settles is not read.
  - The League training to 99 (at 98, Wallace's Milotic won).
- **Frames.** `EMERALD_FRAMES` is 3,006,097. The spine reaches `hall_of_fame` at step 1,365.
- **Merges:**
  - Extract `FORMAT_MINOR`: F3's pokemart lists keep 10. E2's `EventScript_FieldPoison` moves to 11, with its `MINOR_FILES` line.
  - `Behavior`: F2's `PadWarp` keeps postcard index 22. E2's `PopOutWarp` moves to 23 (pin test, hash code).
  - `specials.ron` counts: both waves' changes.
- **Checkpoints:** one accept covers content 5.11 and schema 6. The other Emerald nodes and the 3 stub nodes rehash.

**Fixed on the way.**
- `sil1_w0_contracts`: the shared prelude's comment named `FLAG_ENABLE_MR_STONE_POKENAV`. The flag only lives in the story module.
- F3's `m8_j_specials` on Silice: no Steven multi, no facility var.
- F2's tests:
  - The codegen sample lacked the rotating tile puzzle commands.
  - `GameClear`'s heal location on the stub.
  - The Route 131 surfer test did not compile on Silice.

**Lanes** (`cargo xtask gates`, `parity`):
- **fmt, clippy, clippy-ui, test, test-stub, parity:** pass. Hash at most 48 lines (gate 65); loop at most 159 lines (gate 217).
- **test-silice:** only the known six fail (`sil4_ch3` to `sil9_ch8`).

### Wave F — Waterfall, Dive, the water door and Fly (as built, branch `worktree-agent-a3c4c85bea9a1a653`)

The bypasses `fly-mauville`, `fly-slateport`, `fly-lilycove`, `water-door-in`, `water-door-out`, `dive-seafloor`, `dive-sootopolis` (×2), `dive-out-sootopolis` (×2) and `waterfall-ever-grande` are gone. With the Sky Pillar cluster merged, `SPINE_BLOCKERS` is empty. `spine_real` is green to `hall_of_fame`. After merging master `ae0551e` (E2's field poison), `EMERALD_FRAMES` goes from 3,006,097 to 3,031,502, and `badge1` stays at 104,523.

- **Water doors and arrow warps on water.**
  - `Water::Door` is `MB_WATER_DOOR` and `Water::SouthArrowWarp` is `MB_WATER_SOUTH_ARROW_WARP`. Both are appended to `Water`, so no postcard index shifts.
  - A surfer pushing into a water door takes its warp. South on a south arrow warp takes its warp.
  - The arrival keeps surfing on water. Gen3 `GetInitialPlayerAvatarState` drops surfing on dry land, and so does the engine.
- **Waterfall.**
  - `GetInteractedWaterScript`'s case runs after Surf: an `MB_WATERFALL` cell ahead, surfing, facing north. It needs the badge and a party mon with the move.
  - With those, `EventScript_UseWaterfall` runs. Without them, `EventScript_CannotUseWaterfall` runs (`GameData::waterfall_refused_script`).
  - `FLDEFF_USE_WATERFALL` rides up one slow walk per cell while the player stands on a waterfall (`WaterfallFieldEffect_RideUp`/`_ContinueRideOrEnd`).
- **Dive.**
  - `TrySetupDiveDownScript`: A on diveable water (`MB_DEEP_WATER`, `MB_INTERIOR_DEEP_WATER`, `MB_SOOTOPOLIS_DEEP_WATER`) runs `EventScript_UseDive`.
  - `TrySetupDiveEmergeScript`: B underwater, off the no-surfacing cells, runs `EventScript_UseDiveUnderwater` (`GameData::dive_emerge_script`).
  - `FLDEFF_USE_DIVE` warps (`TrySetDiveWarp`, `GameData::dive_warp`):
    - first the map's dive or emerge connection, which keeps the player's cell (generated from the map headers);
    - else the fixed warp its `on_resume` sets (`setdivewarp`, a 10-entry table in `content::prelude::puzzle`).
  - `setdivewarp` is classified `Ignore`: the table stands for it. Not yet: `Underwater_SealedChamber`'s `MAP_SCRIPT_ON_DIVE_WARP`, which picks a warp by cell.
  - The arrival swaps between surfing and underwater and keeps the facing (`GetAdjustedInitialDirection`).
  - **Extract format minor 12:** the water scripts are exported as `roots::FIELD_SCRIPTS[7..]`, after E2's `EventScript_FieldPoison` (minor 11). They go into `scripts/Common_field_move_scripts.ron`, which every extract since minor 8 already has, so `MINOR_FILES` gets no line, as with minor 10.
- **Fly.**
  - Party menu FLY (`SetUpFieldMove_Fly`) works outdoors only (`Overworld_MapTypeAllowsTeleportAndFly`). Indoors, it reports `Degraded("fly: can't use here")` and goes back to the slots.
  - FLY pushes `ScreenKind::FlyMap` (`ui/fly_map.rs`, degraded). It lists the visited destinations of `GameData::fly_destination`: the `MAPSEC` towns 0 to 15, gated on `FLAG_VISITED_*`, landing on `sMapHealLocations`.
    - The picker is one framed window with the destination's name; UP/DOWN wrap. A gives `ScreenOut::Fly(i)`: the screens close and the field warps (`FieldCallback_UseFly`). B goes back to the party menu.
    - Not done: the region map, the cursor, `FLDEFF_USE_FLY`'s bird, Littleroot by the player's gender (Brendan's house is used), the League entrance, Southern Island and the Frontier.
  - The table lives in `content::emerald_story`, so that `SHARED_GEN3` does not reserve the `FLAG_VISITED_*` names in Silice.
- **Catching (degraded battle bag).**
  - In a wild battle (not the first) with a ball in the bag, the scene's BAG throws the bag's first ball as `Choice::Ball` (`BattleScene::set_balls`/`can_throw`/`balls_thrown`). The field then takes the thrown balls out of the bag.
  - There is no bag screen in battle; the battle side was already Gen3's (`Cmd_handleballthrow`).
- **Explorer.**
  - `Step::Catch { map, x, y, species }` paces the cell with `Opts::catch` until `species` joins the party. The policy throws at that species and runs from the rest, giving up after 60 battles.
  - `Step::Fly { mon, dest, to }` and `MacroKind::Fly` drive START → POKéMON → the slot → FLY → the fly map's cursor → A, then check that the map is `to`.
- **Spine.**
  - Swampert's four slots are Strength, Earthquake, Rock Smash and Surf, and Castform learns no HMs. Teaching HM08 to Swampert over Earthquake lost Victory Road and Wallace.
  - So the spine catches a Tentacool and a Wingull on Route 122 (22,33), before Lilycove's Center. Catching on Route 118 shifted the RNG into a whiteout on Route 121.
  - HM02 goes on the Wingull (slot 3, over Growl). Since E2's field poison, the Tentacool's Poison Sting can poison the lead during the catch, so a Pecha Berry is used after the catches (otherwise the lead faints on Route 121). HM08 goes on the Tentacool after `hm_dive` (slot 2, over Poison Sting) and HM07 after `hm_waterfall` (over Constrict).
  - The spine flies to Mauville (destination 9), Slateport (8) and Lilycove (12).
  - The water door is real both ways: `enter` (70,5) into the pool's arrow warp, then the pool's (13,27) arrow warp back.
  - The dives are real: Route 128 (38,27) → Underwater 128's (38,26) tunnel → the cavern pool; Route 126 (45,66) → Underwater 126 (45,65) → Sootopolis's lake (29,53); the lake's (29,54) → Underwater Sootopolis (9,8) → Route 126.
  - Waterfall is real: `use_` Ever Grande (20,68) Up.
  - A `sootopolis-layout` bypass, which made up for the missing layout override in the checkpoint, went away when merging schema 6.
- **Ledger** (`cargo xtask ledger`, after merging master `70a3d50`): script commands 118 native / 14 degraded / 44 ignored / 53 unsupported, up from 118 / 14 / 43 / 54 (`setdivewarp`). Field moves 5 native / 2 degraded / 7 missing, up from 3 / 1 / 10 (Waterfall and Dive native, Fly degraded). `MB_WATER_DOOR` and `MB_WATER_SOUTH_ARROW_WARP` are native.
- **Branches** (`explore branch --from hm_dive`): 106 branches. 58 rejoined, 41 aborted, 5 diverged, 2 stranded.
  - All 5 divergences are in the Seafloor Cavern's Strength rooms (Room 1 at step 896, Room 2 at step 904): a branch that leaves the room, or pushes a boulder, finds the puzzle reset or blocked, and the spine's next `Use` on the boulder never reaches its cell.
  - This is not a Dive gap. F2 saw it with the dive still bypassed. It is a rejoin problem: positions in the middle of a boulder puzzle.
- **Checkpoints and goldens (not re-accepted):**
  - The battle scene's hash gains the bag's balls and the thrown count. Goldens `bag_*`/`scene_*` move to `(806, 407566406512155080)` and `(1020, 12470217771155615926)`.
  - The Emerald spine's inputs and frames move from Route 122 on (the catches).
  - The extract's minor 12 moves every node's content hash.
- **Checkpoints, accepted after merging `ae0551e`:** one `chore(checkpoints)` commit ran `verify --accept` on the Emerald tree (12 nodes) and the stub tree (3 nodes). Inputs, frames, parents and labels are unchanged; `badge1` stays at 104,523.
- **Lanes** (`cargo xtask gates`, `parity`):
  - **fmt, clippy, clippy-ui, test, test-stub, parity:** pass. Route hash 48 lines (gate 65); loop 159 lines (gate 217).
  - **test-silice:** only the known six fail (`sil4_ch3` to `sil9_ch8`).
