# Spine-and-branch explorer (design)

**Decision (user, 2026-10-23):** drop the evolutionary explorer. Testing follows **one canonical, authored main path per flavour (the spine)** that completes the game as far as the engine goes, and **branches** off it to try what the spine never touched. A branch tries to **rejoin the spine**, or is **aborted** when it takes too long for too little novelty.

This replaces the search half of the explorer of Milestone 7 (`2026-10-22-milestone-7-explorer.md`) and Milestone 8 Tasks K and N (`2026-10-22-milestone-8-scale-import.md`). The play half stays: macro-actions, the planner, the policy, the detectors, the softlock probe, findings and repro. Neither `engine` nor `content` changes, so the route and `BUILD_ID` stay where they are.

## 1. Why

- **The evolutionary search is slow on story.** Act 1 took **81 min** of exploring (M8 Task P). Of that, 34 min went to a Torchic chain that never beat Roxanne. Each act needs re-finding after every route change (M9 Task Q/N). With a spine, the story is authored once, as a few hundred steps, and replays in seconds.
- **Exploration is wasted on known ground.** Most nodes re-walk cells the route already covers. Branching only from the spine, toward things the spine never touched, spends frames where the novelty is.
- **Recovery is a test in itself.** A branch that walks off, talks to an NPC, or runs from a battle, then fails to get back to the story, has found a bug: state leaked, a door stays shut, or a script re-triggers. The evolutionary explorer never checked this.

## 2. The spine

### 2.1 Steps (committed Rust data)
`explore::spine::Step`, one list per flavour (`spine::emerald::STEPS`, `spine::silice::STEPS`, `spine::stub::STEPS`; `spine::steps()` picks the build's). The steps are high level. The macro driver (`expand.rs`: planner, tap rule, policy) expands them into inputs on the live session, deterministically. A step names its map and cells, never inputs, so small engine changes do not break it (unlike a raw input trace).

| step | expands to | notes |
|---|---|---|
| `Goto { map, x, y }` | `MacroKind::WalkTo`, retried (`RETRIES`) until on the cell | `map` is asserted first (a wrong map fails the step at once) |
| `GotoAvoiding { map, x, y, avoid }` | one `MacroKind::Step` per cell, re-planned after each | a shortest path that never enters `avoid` (trainers' sight lines); grass avoided when it can be |
| `Enter { map, x, y, dir }` | `MacroKind::Enter` | warp cell `(x, y)`, entered moving `dir` |
| `Cross { map, x, y, dir }` | `MacroKind::Cross` | edge cell, held `dir` until the map changes |
| `Talk { map, local_id }` | `MacroKind::Talk` | walks to an approach of the object, faces it, presses A, resolves |
| `Use { map, x, y, dir }` | `WalkTo` + `Face` + `Interact` | the generic "stand here, face there, press A" (signs, counters, hidden items) |
| `Face(dir)` | `MacroKind::Face` (new) | turn in place (a one-frame tap) |
| `Walk(dir)` | `MacroKind::Step` (new) | one step: a bump into a scripted cell, or across a cell a script opened |
| `Exit { dir, to }` | `MacroKind::Push` (new) | hold `dir` from the current cell until the map changes; the new map must be `to` |
| `Interact` | `MacroKind::Interact` (new) | A, then resolve what starts |
| `Press(buttons, frames)` | `MacroKind::Press` (new) | hold, then resolve |
| `Wait(frames)` | `MacroKind::Wait` | |
| `WaitScript` | `MacroKind::Wait` (60 frames), resolving | idle long enough for a frame script to start, then resolve it (a `Settle` alone returns at once on a capturable frame) |
| `Grass { map, x, y }` | `MacroKind::Grass` | pace until a wild battle starts; the policy fights it (or runs, see `Battle`) |
| `Train { map, x, y, level }` | `Grass` until the lead's level ≥ `level` | grinding before a gym, robust to small changes in exp or encounter rates; a dry `Grass` (no encounter) rolls back and waits 31 frames, at most `TRAIN_DRY` times in a row |
| `Battle(plan)` | sticky `Opts::run_wild`, `Opts::best_move` | `Fight` (default: the first damaging move), `Best` (the move of the best power x STAB x type matchup) or `RunWild` |
| `Choose(n)` | sticky `Opts::choice` | yes/no (0 yes) and the starter ball |
| `Expect(goal)` | a check | the named goal of the build's act (`goals.rs`) holds now |
| `Checkpoint(label)` | a check | a labelled spine position (below) |

The policy (`policy.rs`) drives battles and scripts as before: first damaging move, B on move-learn prompts, A through text, `choice` on yes/no and the starter screen. BAG and switching come with M9 Task N's policy hooks. When they land, `Battle(plan)` gains `Items` and `Switch`.

### 2.2 Running it
`spine::run(game, steps, cfg)` plays the steps from a new game. A settle comes first: the new game is not capturable until its intro script ends. All of the spine runs on **one** session, with the detectors on (round trip, capturable-stuck, planner divergence, panics, reports).
- **Every step ends on a capturable frame** (the macro contract), so every step boundary is a **position**: the step index, the total frame, the end `Checkpoint`, the map and cell, and the place (`key::place_region`).
- **The spine's coverage** is the union, over its steps, of the maps entered, cells stood on, scripts started, flags ever set, var values ever held, the maps with wild battles (fought or run), the trainers fought and the warp cells taken.
- **A failing step stops the run** with the step index, the map and cell, and the expansion error. That is the gate: `spine_reaches_badge1` and `spine_reaches_ch0_end` assert that the whole list runs and that every `Expect` holds. They also record the frame count.

### 2.3 Flavours
- **Emerald:** from a new game to `FLAG_BADGE01_GET`, along the route the evolutionary explorer found for M8 Task P. The `act1:*` lineages were read back from Task P's dbs as macros, then cleaned of detours and rewritten as steps: Birch's rescue (Mudkip), the lab, May on Route 103, the Pokédex, Route 102, Petalburg and Wally's tutorial, Route 104, the woods grunt, Rustboro, the gym and Roxanne. `Train` steps level the starter where the explorer had wandered into battles instead. The spine grows as M9+ lands; the goal is the whole game.
- **Silice:** Chapter 0 (the bar, the night chase, the house, the morning and the starter, Route S1 to `VAR_SIL_STORY` 9). The steps are those of the buttons-only end-to-end driver `crates/content/tests/sil1_f_ch0.rs`, without its test-only grant of Chapter 1's module. The Ch1/Ch2 drivers are being written as step lists, so they are appended to the spine as their scripts land.
- **Stub:** the parity stub route (east, the grass battle, the far corner). This is the fixture for the branch tests.

### 2.4 Checkpoint nodes (decision: no churn)
A `Checkpoint(label)` step names a spine position. `explore spine --export` writes the labelled positions into the **local** checkpoint store, as `spine:<act>:<label>` nodes chained from a new game (`act1` on Emerald, `sil_ch0` on Silice). `cargo xtask checkpoints promote <id> --label spine:act1:badge1` then commits them like any other node.

**The committed `act1:*` nodes (c00005–c00009) stay.** The spine does not replace them in this change:
- Replacing them would rewrite five committed traces and their parity lanes (`act1.rs`, `parity.rs`). Their frames would differ, so this would be churn for no gain in coverage.
- Instead, `explore spine` checks the shared points. For each spine label `L` that the committed tree also has as `act1:L`, both states must be **progress-equivalent**: the same map, and the same goal progress on the act's list (`goals::state_progress`). The explorer's chain and the spine take different detours, so their digests differ by design.
- If M9 Task Q prefers spine nodes over re-finding `act1:*`, it promotes `spine:act1:*` and drops `act1:*` in its own `chore(checkpoints)` commit.

`cargo xtask checkpoints verify` is untouched: the spine writes nothing to the committed tree.

## 3. Branches

### 3.1 Origins
Branch origins are:
- the positions after each `Checkpoint` step;
- the position after every `every`-th step (`BranchConfig::every`, default 8).

`explore branch --from <label>` limits the origins to that checkpoint's segment, from the label up to the next checkpoint.

### 3.2 Targets: what the spine never touched
`branch::targets(origin)` returns the targets at an origin's state. They are the applicable macro candidates (`macros::candidates`) that the spine's coverage does not already hold, in candidate order:
- **warps not taken:** `Enter` whose warp cell the spine never took, or whose destination map it never entered;
- **edges:** `Cross` into a map the spine never entered;
- **NPCs not talked to:** `Talk` whose object script the spine never started;
- **bg events:** `Inspect` whose script the spine never started (signs, hidden items, PCs);
- **cells:** `WalkTo` one cell per coarse block the spine never stood in;
- **battles:** `Grass` on a map where the spine had no wild battle (fight), and `Grass` with `run_wild` on a map where it never ran (the RUN choice);
- **menu choices:** the spine's own next interaction (`Talk`, `Use`, `Interact`) with each other `choice` (no instead of yes, the other starters).

Item balls and trainers are objects, so they come as `Talk` targets. A trainer also battles by sight on a `WalkTo` through its line, and BAG and switch choices come with M9 Task N's policy. A target's identity is `(place, macro)`, so the same NPC under another story state is another target. The first origin in spine order keeps it. `BranchConfig::max_targets` (default 48) caps the targets per origin.

### 3.3 Running a branch
From the origin's checkpoint, on a fresh session, with the detectors on:
1. **Explore:**
   - Expand the target.
   - Then **chain** more macros, up to `max_steps` in all (default 16; as built, see §8): at each new state, the first candidate that is novel against both the spine's coverage and the branch's own.
   - Novelty is weighted with `Weights` (map 100, flag 40, script 30, var value 20, cell 2). It counts what is new against the spine and the branch so far.
2. **Abort** (no recovery):
   - when the branch's exploration frames exceed `budget_frames` (default 6,000; `--budget`);
   - or when, after the first macro, its novelty per 1,000 frames is below `min_novelty` (default 20; `--min-novelty`).

   Both are in `BranchConfig`. A bug finding (panic, round trip, capturable-stuck, planner divergence) also ends the branch, as outcome `bug`.
3. **Recover to the spine:**
   1. **Route back.** If the branch left the origin's map, it plans over the static map graph (warp destinations and connections) to the origin's map. It then takes one `Enter`/`Cross` toward the next map, re-planning after each arrival, for at most 8 hops. Then it walks to the origin's cell and faces the origin's way. This is bounded by `recover_frames` (default 6,000).
   2. **Check equivalence at the origin.** The **progress state** is:
      - the story vars (`key::story_var`, all of them);
      - the **spine flags**: the story flags whose value changes along the spine, i.e. the ones its steps set;
      - whether the party is usable (a mon with HP left, once the spine has a party).

      A branch that sets an unrelated flag (an item ball, an off-route trainer) is still equivalent. One that sets a flag the spine sets later is not: it took a later item, or beat a later trainer.
   3. **Resume the spine** for a verification window of `window` steps (default 3), from the origin's next step. Each step must expand. The progress state at the window's end must equal the spine's at that position.
4. **Outcome** (one per branch, in the db):
   - `rejoined`: the window ran and the progress state matches.
   - `diverged`: the window failed, or the state differs. It is **expected** when the branch had changed the progress state before recovering, e.g. it answered no, took a later item, or chose another starter. It is **unexpected** otherwise, and is then a `Divergence` finding. Its tail is the branch, the recovery and the window, from the origin.
   - `stranded`: no way back to the origin's cell within the hops and frames. The softlock probe (`sim::trap`) runs on the branch's end state, and a trapped state is a `Softlock` finding.
   - `aborted`: the budget or the novelty rate stopped it.
   - `bug`: a detector or a panic stopped it.

### 3.4 Parallelism and determinism
- Branches are independent. Each has its own session restored from its origin's `Arc<Checkpoint>`; `World` is `!Send`, so it is built on the worker thread.
- They run on a rayon pool of `--jobs` threads (`pool::run_ordered`; it uses the speed-up agent's helper if that lands). `par_iter().map().collect()` keeps the input order.
- The results are merged into the db in target order, so the first branch in spine order gets the first-finder credit in `seen`. Nothing depends on the thread count or the host time (slow frames are report-only, as before). `branches_recover_rejoin_and_do_not_depend_on_the_job_count` checks the results and the db digest with `--jobs 1` and `--jobs 4`.

### 3.5 Findings and repro
- A finding's trace is the spine's inputs from a new game to its origin position, then its tail. For a spine finding, the tail is the failing step from the previous position. For a branch finding, it is the branch's inputs up to the finding frame.
- The db stores the spine's inputs once (`spine.inputs`) and each finding's origin frame and tail.
- `explore repro`, `export` and the studio's "Open at frame before" rebuild the NewGame trace from them, so repro is self-contained as before.
- Signatures and dedupe are unchanged (`findings::signature`). A new kind, `Divergence` (signed on the map and the origin step), is a bug.

## 4. What is kept, adapted and deleted

**Kept as is:**
- `macros`, `path`, `expand`: five new macro kinds, `Face`, `Interact`, `Press`, `Step` and `Push` (spine steps, never candidates);
- `policy`, `balance`, `detect`, `softlock`, `observe`, `guard`, `key`, `goals`, `runs`, `testing`, `findings`: the `Divergence` kind is added to `findings`.

**Adapted:**
- `model`: the macro and finding types stay. The node, job and generation types go. `Macro::code`/`from_code` go: macros are stored as RON text.
- `sim`: the job runner becomes `sim::Live`, a session with its observer that expands macros, collects steps, reports and findings, and builds finding drafts. The softlock probe `sim::trap` stays.
- `config`: the evolutionary fields go (`seed`, `batch`, `max_plan_len`, `tournament`, the age and expanded penalties, `cache_capacity`, `snapshot_every_depth`). `BranchConfig` (`every`, `budget_frames`, `min_novelty`, `max_steps`, `window`, `max_targets`, `recover_frames`, `recover_hops`) and `jobs` replace them.
- `db`: **schema v4**:
  - tables `meta`, `spine` (one row with the inputs, steps and frames), `position`, `branch`, `seen`, `coverage`, `finding` and `run`;
  - a db of an older schema is refused with "run `explore reset --yes`";
  - `meta.flavor` is kept (SIL1 Task D).
- `cli`: the commands are `spine`, `branch`, `branches`, `status`, `findings`, `show`, `export`, `repro`, `coverage`, `digest` and `reset`.
  - `run`, `revalidate` and `export --to-store` go (`spine --export` replaces the last).
  - **`explore sweep` (M9 Task D) is folded into `branch`**, which covers every map the spine reaches. `cargo xtask sweep` stays as the every-map StartAt sweep.
- studio Explorer tab:
  - the run form starts `explore spine` or `explore branch` (budget, jobs, from);
  - the stats show the spine (steps, frames, checkpoints) and the branches by outcome;
  - the findings list, "Open at frame before" and the coverage overlay read the v4 db.

**Deleted:**
- `mutate`, `select`, `fitness`, `graph`, `runner` (the evolutionary machinery);
- `cache`, `rebuild`, `incremental` (node replay, revalidation and touches: there is no node tree left to keep valid; the spine is re-run instead);
- `wire` (its glue moves to `spine`, `branch` and `store`);
- their tests: `b_fitness`, `c_cache`, `c_rebuild`, `d_runner`, `h_tuning`, `m8_k_db`, `m8_k_from`, `m8_n_from`, `m8_n_incremental`, `m8_n_touches` and the run-based parts of `e2e_*`.

## 5. CLI
```
cargo xtask [--game silice] explore spine [--export]
    run the spine from a new game (detectors on); record it in the db; print each checkpoint
    (frame, map, goals) and the progress check against the committed <act>:<label> nodes;
    --export: write the spine:<act>:<label> nodes into the local store
cargo xtask explore branch [--from <label>] [--budget <frames>] [--jobs N] [--every N]
                           [--max-steps N] [--min-novelty N] [--limit N]
    run the spine (or reuse the db's on the same build), enumerate targets, run the branches
    in parallel, merge in order; print outcomes, novelty found and throughput
cargo xtask explore branches [--outcome rejoined|diverged|stranded|aborted|bug]
cargo xtask explore status | findings [--all] [--bugs] | show <id> | export <id> [--before N]
    | repro <id> | coverage <map> | digest | reset --yes
cargo xtask checkpoints promote <local id> --label spine:act1:badge1
```

## 6. Tests
- `spine_reaches_badge1` (Emerald, release-speed engine, `#[ignore]` off: it is the gate): the spine runs to `Checkpoint("badge1")`, every `Expect` holds, no bug finding, and the frame count is printed and recorded in this doc.
- `spine_reaches_ch0_end` (Silice): as above, to `VAR_SIL_STORY` 9.
- Stub and synthetic worlds (`tests/spine_branch.rs`):
  - target enumeration excludes the spine's coverage and is deterministic;
  - recovery rejoins on the stub;
  - a branch that sets a spine-relevant var is an expected divergence;
  - a branch that breaks the spine's way back without touching progress state is an unexpected `Divergence` finding;
  - a sealed pocket strands the branch, and the probe gives a `Softlock` finding;
  - `--budget 1` aborts, and a high `--min-novelty` aborts after the first macro;
  - `--jobs 1` and `--jobs 4` give the same `digest`;
  - a fault injected off the spine (`FadeTrap`, `Reporting`) is found by a branch and reproduces with `explore repro`.

## 7. Numbers to report
- The spine's run time and frames per flavour.
- Branch throughput: novel things found per minute (maps + scripts + flags + var values + cells, first-finder counted), against the old evolutionary Act 1 run (81 min to badge 1).

## 8. As built (2026-10-23)

**Code.** `crates/explore/src/{spine.rs, spine/{emerald,silice,stub}.rs, branch.rs, store.rs, pool.rs}` are new; `db.rs` (schema v4), `cli.rs`, `config.rs` (`BranchConfig`, `jobs`) and `model.rs` (macro kinds `Face`, `Interact`, `Press`, `Step`, `Push`; `Opts::best_move`) are rewritten or extended; `mutate`, `select`, `fitness`, `graph`, `runner`, `cache`, `rebuild`, `incremental` and `wire` are deleted with their tests. `explore::testing` gains `VarTrap` (a cell that sets a story var) and `fill_db` (a small spine-and-branches db for the studio's tests).

**Differences from the design above.**
- **The exploration chain** is `max_steps` (default 16 macros), not `chain`: a branch keeps playing the first novel candidate (against the spine and itself) until none is left, its steps or budget run out, or its novelty rate drops below `min_novelty`.
- **The progress state** also holds the object-template digest, the key items and the respawn point (`spine::Progress`).
- **Recovery** on the origin's map walks back to the origin's cell and facing only when the spine's next playing step needs them (`Face`, `Walk`, `Exit`, `Interact`, `Press`, `Wait`, `WaitScript`); otherwise the window's own `Goto`s walk. A branch over `recover_frames` on the way is `stranded` (probed for a softlock).
- **The macro frame cap of spine and branch sessions** is at least 12,000 (`spine::PLAY_MACRO_FRAMES`): Silice's morning (the call, the starter, the gifts) runs ~6,000 frames in one `WaitScript`.
- **`spine --export`/`--label`/`--trace`** exist; `--trace` prints every position (frame, map, cell, facing, party, the step, the vars and flags it changed), the tool that authored the Emerald spine.
- **Branch origins** are deduplicated by frame (a `Checkpoint` step repeats the position before it).

**The spines.**
- **Emerald:** 78 steps, new game to badge 1 in **109,853 frames** (**1.3 s** release wall time; 15 maps, 797 cells, 44 scripts, 37 spine flags). Checkpoints: `lab`, `oldale_center`, `may`, `pokedex`, `petalburg`, `woods`, `rustboro`, `badge1`. Mudkip (`Choose(2)`), `Battle(Best)`; Center heals before Route 104, the woods and the gym; `Train` to L10 in Petalburg Woods and to L14 on Route 104 before Roxanne.
- **Silice:** 69 steps, new game to `VAR_SIL_STORY` 9 in **21,821 frames** (0.1 s). Checkpoints: `bar`, `explosion`, `mallette`, `chase`, `night`, `starter`, `route_s1`, `first_battle`, `teo`, `ch0_end`. It mirrors `sil1_f_ch0.rs` and SIL2's shared `silice_spine::ch00` (same cells, same `ROUTE_S01_SIGHT` list) without the exit-guard and north-guard detours, the "only once" re-checks and the brambles' test-only grant (branches explore those). SIL2's vocabulary maps one to one: `Goto{avoid}` → `GotoAvoiding`, `Enter{dir,map}` → `Exit{dir,to}`, `Interact(d)` → `Face(d)` + `Interact`, `Walk` → `Walk`, `Settle` → `WaitScript`, `Check(label, _)` → `Checkpoint(label)` (+ an `Expect`), `Hook` → left out. When `silice_spine` lands on master, the explorer can import its step lists through a converter instead of keeping its own copy (it lives in `crates/content/tests`, so a move to a shared module comes first).
- **Stub:** the parity stub route, 10 steps, 1,520 frames.

**Tests.** `tests/spine_branch.rs` (stub; 14 tests): the stub spine and its labels; targets new against the spine, unique and capped; branches rejoin and `--jobs 1`/`--jobs 4` give the same results and the same db digest; `--budget 1` aborts, `min_novelty` at max stops after the target, `max_steps 1`; recovery walks back to a cell (a spine whose next step is `Face`) or strands the branch over `recover_frames`; a `VarTrap` off the spine is an expected divergence; a spine whose recorded states moved (respawn) makes an unexpected `Divergence` finding with its tail and origin; a `FadeTrap` off the spine is a `CapturableStuck` bug that `explore repro` reproduces (and not on the plain game); a panic in the new game stops the spine with a `Panic` finding that reproduces; `Reporting` gives `Unsupported` report findings that reproduce; a new build marks findings stale until found again; the CLI; reset refuses non-dbs and busy dbs; the work tree stays clean. `tests/spine_real.rs`: `spine_reaches_badge1` (Emerald, labels and 109,853 frames, then 6 branches from `lab` without a finding; 16 s debug) and `spine_reaches_ch0_end` (Silice, labels and 21,821 frames, 6 branches from `starter`).

**Numbers (release, 16 jobs, default `BranchConfig`).** `explore spine` (Emerald): 109,853 frames in 0.4–1.3 s. `explore branch` (Emerald, every origin): 319 branches from 14 origins, 2,068,756 frames in **2.3 s** (893,000 frames/s), **1,982 new things** (maps, scripts, flags, var values, cells first found) = **~51,000 per minute**. Outcomes: 176 rejoined, 23 expected divergences, 7 unexpected divergences, 105 aborted (budget), 8 unplayable. Beyond the spine: 20 maps, 75 scripts, 13 flags, 3 var values, 1,871 cells. Against the evolutionary Act 1 run (81 min to badge 1, which this spine replays in about a second), the story is no longer searched at all, and a full off-spine pass takes seconds. The 7 unexpected divergences are one signature, open for triage: after `petalburg`, a `WalkTo (25, 45)` on Route 104 rejoins the map but the window (to the woods) ends with other object templates than the spine's.

**Engine observation (to check against the GBA).** After Wally's tutorial, Petalburg Gym's `ReturnFromWallyTutorial` frame script did not start on idle frames (`Wait`, `Press` with no key), only once the player stepped (`Walk(Up)` twice in the spine). The Pokédex scene on arriving in Littleroot did start on idle frames (`WaitScript`). On the GBA, `ProcessPlayerFieldInput` calls `TryRunOnFrameMapScript` on every field frame, whatever the input (only `CheckForTrainersWantingBattle` comes first), so the gym's case may be an engine divergence in the field loop (M9 Task F's area), or a state (facing, a pending movement) the tutorial's end leaves. Not investigated further here.

## 9. Self-review
- **Does the spine depend on raw inputs anywhere?** No. Every step expands through the planner and the policy, and the expected maps are asserted, so a moved NPC or a longer script shifts frames and nothing else. A changed map layout makes the step fail with its index and cell: the error you want.
- **Determinism:** the spine is one session and fully deterministic. Branches start from `Arc<Checkpoint>`s and use no RNG of their own: targets come in candidate order, with no random choice. The merge goes in target order. The `Wait` before a softlock probe is fixed. **Risk:** host timing reaches the results only through slow frames, which stay report-only.
- **Recovery false positives:**
  - Wandering NPCs can block the way back: the walk waits up to `BLOCKED_FRAMES` (as before), then the branch is `stranded` with a probe. Only a trapped probe makes a finding.
  - Trainers that see the player on the way back battle: the policy fights. The progress state ignores non-spine trainer flags.
  - The party's HP differs after a branch's battles: "usable" only asks for one mon with HP. A spine battle right after a branch may then be lost. The loss's whiteout moves the player and is recorded as an expected divergence (the branch spent HP), not a finding.
- **Cost:** the Emerald spine is 109,853 frames, 1.3 s in release, so `branch` re-runs it (and compares it with the db's) rather than caching positions on disk. Branches are capped by `budget_frames + recover_frames + window` steps, so a run of `n` branches is bounded.
- **What the old explorer found that this might miss:** deep off-route chains, e.g. Briney's boat from Route 104, reached three macros off the route. `max_steps` and `budget` bound how deep a branch goes. Raising them (`--max-steps 32 --budget 20000`) trades throughput for depth, per run.
- **Silice:** the spine and the branches run under `PKR_GAME=silice` with their own db (`target/explore/silice/explore.db`), as SIL1 Task D set up. The Ch0 driver's step list moves into `spine::silice` (the explorer's planner replaces its own). The content test stays as the content-level end to end.
