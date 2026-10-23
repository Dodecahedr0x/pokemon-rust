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

| step | map (cell) | error / report | decomp feature | owner |
|---|---|---|---|---|
| (filled by Task S) | | | | |

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

Gaps on story maps, by act: act1 113, act2 35, act3 64, act4 61, act5 17, act6 47, act7 68, act8 36, league 17 (the ledger's "Gaps per act" table splits them by category).

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
