# Status and hand-off (2026-10-23)

On the user's request the running agents were stopped and their branches merged into master **as is**, including their unfinished (`wip`) commits. This page records what each one finished, what it left half done, and what comes next. Master is `f22c108` plus this doc.

## State of master

`cargo xtask gates` on `f22c108` (622 s): **fmt, clippy and clippy-ui pass; test, test-stub, test-silice and parity fail.** Both flavours build (`cargo check --workspace --all-targets`). `cargo xtask silice check` is clean apart from one trainer-name ruling (see below).

| Failure | Lanes | Cause | Fix |
|---|---|---|---|
| `checkpoints verify`: all 9 Emerald nodes `DIVERGED` (c00001 from frame +1500, c00005 from +2700; the rest follow from their parents) | parity, `game-native` `checkpoint_tree`, `studio` `tree::tests`, `studio` `native` `load_seek_and_save_…` | The inputs replay differently after Wave 0's field changes. Its agent was re-recording the nodes when it was stopped, and those uncommitted node edits were discarded. | Check that the divergence comes from Wave 0's intended changes (the start menu, friendship, the repel chain), re-record or extend the route inputs where needed, then make one `chore(checkpoints)` commit with `verify --accept`. Not done here: accepting without checking would hide a regression. |
| `explore` `path::tests::the_terrain_table_answers_as_the_per_edge_rules_on_the_game_maps` (GraniteCave_1F) | test, test-silice | The `Terrain` cache and the per-edge rules disagree once cave floors are encounter cells: one of the two still treats only tall grass as grass. | Make `path::Terrain` use `Behavior::land_encounter()` like the per-edge rules. |
| `sil6_ch5`: a battle lost at frame 234 795 on map 54 | test-silice | Cave floors now draw wild battles. The half-done `28df9c3` re-tune is not enough. | Re-tune the Ch5 spine (levels, `Step::Train`) by the neighbour rule. |
| `sil8_ch7`: out of frames on map 86 at (14, 40) | test-silice | The same cause shifts the RNG or party downstream of Ch5/Ch6. | Re-tune after Ch5. |
| `sil9_ch8`: no way from (33, 7) to (13, 16) on map 88 | test-silice | The spine is unfinished (`379a460`, never run before the stop). | Finish the Ch8 spine. |

## The four stopped agents

### M9 Wave 0 review (merged: `c41a993`)

- **Done:** the Wave 0 contracts, reviewed and fixed (see "Wave 0 as built" and "Contracts added by the Wave 0 review" in `2026-10-23-milestone-9-menus-mart-save.md`). The ids moved off the reserved Gen3 vars are `VAR_SIL_CASINO_STATE` → 16437 and `VAR_SIL_PORT_RESSAC_STATE` → 16439, with tombstones.
- **Half done:** `5b8e99e wip(parity)` moves `KNOWN_LATE` in `crates/emu/tests/parity.rs` from frame 80 366 to 81 769, a loop overrun at 225 lines against the 217-line gate. The agent stopped while `act1_long` was running with that entry, so the move is **unverified**. Its uncommitted checkpoint-node edits were discarded.
- **Next:**
  1. Run `cargo xtask parity --long`. Keep the 81 769 entry if it is the only late frame; otherwise bisect again. A better fix is to remove the overrun, which is Roxanne's battle-start cost, and then drop the entry.
  2. Re-record the diverged checkpoint nodes and do one deliberate `chore(checkpoints)` commit (see "State of master").
  3. Start Wave 1 (Tasks K, X1, X2, C, S, R, E, F, J, T, D) per the M9 plan.

### Engine forced movement (merged: `d0c978a`, fix `8e75c52`)

- **Done and reviewed:**
  - Ice, walk and slide arrows, the muddy slope, thin and cracked ice, and cracked floors and holes.
  - The tile-center input rule and the `CONTROLLABLE` hold.
  - The Sootopolis ice and cracked-floor per-step callbacks.
  - `Request::WarpHole` (hash tag 120) and `GameData::{step_callback, hole_warp, fall_script}`.
  - Studio codes F, K and H.

  See "Forced movement (as built)" in the M9 plan.
- **Merge notes:** `StepFinished` runs, in order:
  1. sight;
  2. a forced cell (nothing else runs);
  3. coord;
  4. warp, then `fall_script` on a hole;
  5. Wave 0's friendship, repel and encounter chain.
- **Next (scoped by its review):** Emerald's content hooks are still missing:
  - done in Wave C (spine warps): the `step_callback` and `hole_warp` tables (`content::prelude::puzzle`), `warphole` lowered, `fall_script` = `EventScript_FallDownHole`, Sootopolis's `IceRows`.

  The explorer's path planner still treats forced cells as plain floor.

### Off-grass land encounters (merged as is: `c5bf6b1`)

- **Done:**
  - `Behavior::Floor(Floor)`: cave, long grass, deep sand, indoor encounter, ash grass, footprints, sand and mountain top. It is **index 20**, after the forced variants 12–19; `checkpoint::tests` pins it.
  - `Behavior::land_encounter()` and Gen3's full `StandardWildEncounter` land branch: Cleanse Tag, Stench, White Smoke, Illuminate, Arena Trap, Magnet Pull and the Hustle-like level abilities.
  - The battle background from `GameData::map_type` → `MapType`.
  - Studio code `E`, and planners that avoid encounter floors.

  See "Off-grass land encounters (as built)" in the M9 plan.
- **Merge notes:**
  - The merge kept Wave 0's repel hook. `try_land_encounter_repel` now takes the branch's `Lead` and calls `wild_level(&mon, ability, rng)`, then the repel check, then Keen Eye.
  - **Two map-type contracts now coexist:** Wave 0's `MapData::map_type: u8` (`data::map_type::*`, read by the escape warp) and this branch's `GameData::map_type(map) -> MapType` (the battle background). Fold them into one, preferably by deriving `MapType` from `MapData::map_type` and dropping the `GameData` hook.
- **Half done:** `28df9c3 wip(silice)` adjusts the Ch5 spine (`sil6_ch5.rs`, `silice_spine/ch05.rs`) for wild battles that now happen on cave floors. The agent stopped before running it, so its state is unknown.
- **Next:**
  1. Re-tune the Silice spines that cross caves and sand (Ch5's Grotte Est first; then every later chapter with a cave), using the neighbour rule for levels.
  2. Accept the checkpoints. Act 1 crosses no off-grass tile, so the agent measured only a rehash. The other changes are battle backgrounds for 5 of Act 1's 19 battles (Rustboro Gym, among others), which change no sampled hash.
  3. Run `parity` afterwards.

### Silice Ch8 scripts, Évalon (merged as is: `48fce2b`, `f22c108` rustfmt)

- **Done:**
  - Flags 1184–1188 and the Évalon heal location.
  - The map phase: pushables and hidden items as script events.
  - The scripts (`ch08_*.rs`): Route S8, Évalon and its buildings, the Tour du Scribe, the Galeries and the Citadelle with its golem bridge and Sphinge.
- **Half done:** `379a460 wip(silice)` adds the Ch8 spine (`silice_spine/ch08.rs`, `ch08::start` and `ch08::route_start`) and its e2e test `sil9_ch8.rs`. Neither has been run or reviewed.
- **Next:**
  1. Run `sil9_ch8` and finish the spine.
  2. Do the usual review pass: source scene by scene, softlocks and bypasses, the check exceptions, and the party chain from Ch7.
  3. Write the "SIL4 Ch8 scripts as built" section of the Silice plan, which does not exist yet.

## Other work and loose ends

- **The explorer** (spine and branches; merged earlier, `ff141f1`; `2026-10-23-explorer-spine.md`):
  - The rest of Tasks D and N: `choose_switch` and `choose_item`; Heal, Buy, UseItem and Deposit steps once M9's menus exist.
  - Extend the Silice spine export past Ch0 by sharing `content/tests/silice_spine` with `explore`.
- A local branch `speed/explore-planner-reference` (`940b711`, an older planner perf commit) is still around. The planner's `Terrain` cache was ported by hand. That is why the agent's `git cherry-pick 9153cd9` was denied and never run. Delete the branch if nothing else is wanted from it.
- **Silice content still to write:**
  - **Ch12 (the post-game) is written** (SIL6, "SIL6 Ch12 scripts as built" in the plan): every Ch12 script, the optional maps (Lounge, Debug Island, Stele cave, three Antennas, Cloud Temple), the fragments and legendaries, the rematches, the daily flags on the engine's clock, and the `sil13_ch12` spine. Open: G-44, G-45, G-51 and T-11 in `silice-author-questions.md` (G-01, G-48–G-50 ruled 2026-10-24 and built).
  - **Free flag blocks** (`ids/flags.ron`, valid range 0x20..=0x4FF): 1061–1079, 1084–1099, 1160–1164, 1166–1179, 1189–1199, 1208–1244 and 1261–1274, 1278–1279. SIL6 used 1124–1141 (gems, dailies), 1142–1149 (fragments), 1150–1159 (antenna trainers, Tak) and 1165 (`FLAG_SIL_ANTENNE_MONTEE`); the G-50 Dojo tournament used 1245–1249 (1275–1277 were already taken).
- **Party continuity across the Silice spines** (Ch1–Ch8):
  - The chained parties disagree: Ch4 → Ch5 loses the KADABRA, the MAREEP line is never trained, and a WAILORD appears without Surf.
  - The cause is that the spine can only train its lead: there is no party menu or PC until M9 Wave 2.
  - Do one continuity pass over every chapter once PT and M land.
- **Engine gaps the Silice scripts work around:** Surf and Dive, Fly, the bikes, multichoice menus, marts, double and multi battles, field weather, and the fallbacks in each chapter's §9.
- **M8 findings carried to M9:**
  - Roxanne's late battle start (the `KNOWN_LATE` frame);
  - `DrawWholeMapView` cost;
  - and the rest in "Findings carried to Milestone 9 (and later)" in `2026-10-22-milestone-8-scale-import.md`, also listed under "Findings carried from M8" in the M9 plan.

## Author rulings waiting

The full lists are in `2026-10-23-silice-content.md`: §4 (contradictions), §5 (open questions) and each chapter's "Questions for the author" or "Author questions" block. The ones that block or bend work now are:

1. **Ch9 → Ch10 elevator** (§5.7). The proposed fix sends `Tour_Hall_Sc02` to `TourNebulaAscenseur` (3, 6).
2. **Duplicate `Tour_Sbire{1,2}_Intro`** (§5.6). They are provisionally renamed `_Ch09`.
3. **Trainer names over 10 bytes.** `silice check` reports `TRAINER_SIL_TAUBER_HOCSEAT_9` ("TAUBER&HOCS", 11 bytes): shorten it, or make it a double battle. The others are in §4.2.
4. **`MOVE_SIL_CODE_AQUA`** (§4.5): new move data, or the tutor fallback.
5. **The Port-Ressac canal**: the source wants fishable but not surfable water, which Gen3 cannot express (Ch4 maps review).
6. **The chapter question blocks** (Ch2, Ch3, Ch4, Ch5, Ch6, Ch7, Ch9 maps, Ch10–11 maps and Ch12 maps):
   - a legendary that needs a Ground type it lacks (Raikou);
   - Samuel's and Tak's teams and levels;
   - the Ch6 quiz option texts;
   - the missing glyphs (`—`, `°`, …).

## Suggested order

1. Make master's gates green again: the checkpoint divergence, the explorer `Terrain` test, then `parity` and `parity --long`.
2. Run the half-done e2e tests: the Ch5 spine on cave floors, then Ch8.
3. Start M9 Wave 1. It unblocks the party menu and PC, which unblock the continuity pass.
4. Continue the Silice scripts from Ch9, with the author rulings above.
