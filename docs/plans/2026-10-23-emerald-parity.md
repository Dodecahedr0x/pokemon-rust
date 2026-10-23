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
3. **A step the engine cannot play is not worked around.** The spine stops there, and the task records the blocker in this doc's "Spine blockers" table: step, map, cell, the expansion error or report, and the decomp feature. Each blocker is a ledger entry tagged `story-blocking`. Workarounds for *explorer* gaps (the planner, the policy) are fine. Workarounds for *engine* gaps are not.
4. `spine_reaches_badge1` stays. Add `spine_reaches_<last checkpoint>`.

**Done when:** the spine runs to its last playable checkpoint with no bug finding, and every blocker past it is in the table.

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

| step | map (cell) | error / report | decomp feature | owner |
|---|---|---|---|---|
| (filled by Task S) | | | | |

## 3. As built

(per task, at merge)

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

**What the diffs show.** Our new game starts outside Brendan's house with May met (`content::new_game`, `Start::BeforeRescue`). In the truck, the map, the cell, the four `VAR_LITTLEROOT_*` intro vars, the intro flags, the respawn and the objects therefore differ by design. At our start, after vanilla played the intro, **all `VAR_LITTLEROOT_*` vars, the map and the respawn agree**. After the starter, `VAR_BIRCH_LAB_STATE`, `VAR_ROUTE101_STATE`, `VAR_STARTER_MON`, the Littleroot vars, the party species and moves and the HP bucket agree too. What remains are parity gaps for Wave C, all in the new-game data:

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
