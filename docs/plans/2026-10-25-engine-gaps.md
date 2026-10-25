# Engine gaps versus official Emerald

Recorded 2026-10-25 from the engine on `master` (`3fdb98d8`). The Emerald spine plays a new game through the Hall of Fame. These are the gaps that remain against the official game (pret/pokeemerald, BPEE). The README "Known simplifications" paragraph is older: the bag, party menu, Poké Mart, start menu, in-game save, field weather, and double battles are already in the engine.

Each row is one item. An agent owns one id. It verifies the gap is still missing, implements it, validates, and merges into `master`. It does not take another id.

Classifiers (current counts in the file headers):

- `crates/content/build/support/commands.ron` — 120 Native, 41 Ignore, 13 Default, 53 Unsupported.
- `crates/content/build/support/specials.ron` — 76 Native, 17 Ignore, 82 Default, 349 Unsupported.

A `Report::Unsupported` or `Report::Degraded` is not an implementation. Closing an item means the official behaviour runs and the report for that item is gone.

## Items

### `audio`

No sound engine. The design defers m4a/Sappy (`docs/plans/2026-10-21-emerald-rust-rework-design.md`). `weather.rs` rolls thunder and plays nothing. The starter screen documents "no cry, no sound" (`crates/engine/src/starter.rs`). The save dialog does not wait on `SE_SAVE` (`crates/engine/src/ui/save.rs`).

**Done when:** the GBA shell and the native shell play the ROM's music and sound effects (a field SE, a battle cry, and the save SE), driven from engine events, with a test that a known effect is queued.

### `rtc` — closed

Merged `033ec277`. `gettime` is Native and writes hours, minutes, and seconds into `VAR_0x8000..VAR_0x8002` from the shell RTC, or from the frame clock when no RTC is set. Test: `gettime_writes_a_known_local_time` (15:42:07). The clock is not hashed, and no checkpoint moved.

### `link`

No link play. `SPECIAL_BufferUnionRoomPlayerName` is `Default(0)` ("no Union Room partner"). `SPECIAL_CloseLink` is Ignore. Link-battle, record-mixing, and Union Room specials are Unsupported.

**Done when:** two engines can complete a Gen3 link battle and a Union Room exchange, or the same flow against a scripted partner that speaks the real protocol. A test covers one full exchange. Unsupported link specials used by that flow are Native.

### `contests` — closed

Merged `fc9d30db`. A script can choose a party mon, run five appeal rounds, and finish with Gen3 standings. `choosecontestmon`, `startcontest`, `showcontestresults`, `contestlinktransfer`, `showcontestpainting`, and `buffercontestname` are Native. Test: `a_script_enters_judges_and_finishes_a_normal_contest`. Checkpoints did not move. Contest specials such as `SPECIAL_GenerateContestRand` were not part of this merge.

### `frontier`

`SPECIAL_CallFrontierUtilFunc` answers "no challenge" and does not store facility data. Battle Tower, Dome, Palace, Arena, Factory, Pyramid, and Pike specials are Unsupported. `DoSpecialTrainerBattle` stays Unsupported for the tents.

**Done when:** one facility (Battle Tower) can be challenged, fought, and recorded, with `GET_STATUS` / `GET_DATA` / `SET_DATA` reading that record. A test clears one challenge. The Tower specials that challenge uses are Native.

### `secret-bases`

Secret-base specials are Unsupported (`SPECIAL_EnterSecretBase`, `SPECIAL_CheckPlayerHasSecretBase`, the PC and battle specials). `MB_SECRET_BASE_BREAKABLE_DOOR` does not break (`crates/content/build/classify/field.rs`).

**Done when:** the player can enter a secret base, decorate it, and fight its trainer. A test covers enter and one decoration. Those specials are Native.

### `slot-machine` — closed

Merged `4acc3483`. `playslotmachine` is Native. The machine takes a 1–3 coin bet, pays the center line and the extra lines the bet covers, and writes the coins back. A replay with no forced stop respins. Reels do not animate. Test: `a_winning_spin_pays_and_a_losing_spin_costs_the_bet` (three red 7s pay 300; a mismatched center line pays 0). Checkpoints did not move. Counts on that merge: 128 Native, 45 Unsupported.

### `decoration-mart` — closed

Merged `90330e3f`. `pokemartdecoration`, `pokemartdecoration2`, and `bufferdecorationname` are Native. The shop is buy-only: buying SMALL DESK (3000) puts it in the first empty desk slot, subtracts the money, and copies `SMALL DESK` into string var 0. A full category takes no money. Decorations are live on the world and are not hashed or snapshotted, so checkpoints were not re-baselined. Test: `buying_a_decoration_puts_it_in_the_pc_and_buffers_its_name`.

### `mystery-gift` — closed

Merged `b20b3e2b`. `setmysteryeventstatus`, `trywondercardscript`, and `setmodernfatefulencounter` are Native. A card that passes the Gen3 checks is stored with the script it points at, and `trywondercardscript` runs that script. The card, the mystery-event status, and the modern-fateful bit are session state: they are not in the checkpoint or the world hash. Tests: `a_delivered_wonder_card_runs_its_script`, `no_card_and_an_invalid_card_do_not_run_a_script`. Counts on that merge: 131 Native, 42 Unsupported.

### `ereader`

e-Reader specials (`SPECIAL_DoesPartyHaveEnigmaBerry` is `Default(0)`; the other e-Reader specials are Unsupported) do not read a card.

**Done when:** an e-Reader payload can grant the Enigma Berry and the special answers from that state. A test covers one payload.

### `braille`

`braillemessage` is Unsupported.

**Done when:** a braille message is drawn and waits for input the way Gen3 does. A test covers one message.

### `pokenav`

`SPECIAL_OpenPokenavForTutorial` is degraded: no PokéNav screen. Match Call text is printed in the field box (`pokenavcall` is Native as text only).

**Done when:** the PokéNav opens from the start menu, shows the Match Call list, and places a call. A test opens it and reads one call. The tutorial special opens that screen.

### `pokedex` — closed

Merged `0f5252bf`. START → POKéDEX opens the Hoenn and National lists, a species page, and the Birch rating. SELECT switches to National mode only when the national flag and `VAR_NATIONAL_DEX == 0x302` are both set. A test opens the list and finds a caught species. Checkpoints did not move.

### `pc` — closed

Merged `4bffb579`. Someone's PC and the bedroom PC withdraw, deposit, and move items and Pokémon, including between the party and a box. A full box, party, or bag is refused, and the last party Pokémon cannot be deposited. Test: `depositing_a_mon_and_withdrawing_it_round_trips_the_party`. Checkpoints did not move.

### `options` — closed

Merged `b672f2fc`. The menu changes text speed, battle scene, battle style, sound, and button mode, and those values change printing, move animations, the shift prompt, the mixer mode, and L=A. Sound is stored only. Nothing is played. The stub checkpoint tree verified. The Emerald tree was not replayed in that worktree.

### `naming` — closed

Merged `847b43e5`. The player, a rival, a nickname, and a box can be named on Gen3's keyboard. Lengths are 7, 7, 10, and 8. A blank confirm keeps the existing name. Schema is 8. The stub checkpoint tree still verifies; the Emerald and Silice trees were not replayed in that worktree.

### `title` — closed

Merged `10253afd`. Power-on with a save shows the title (copyright, blinking PRESS START, a flying Pokémon). Start opens the main menu, and B there returns to the title. Continue still restores the save. Tests: `power_on_shows_the_title_and_b_returns_to_it` and `an_in_game_save_boots_to_the_title_and_continues`. Checkpoints did not move.

### `move-anims` — closed

Merged `3bc3ba89`. A move with extracted data plays that move's Gen3 anim script and sprites. With no script, it still lunges. Send-out uses the emerge zoom (`0x28`, then `+0x12` up to `0x100`) instead of the 4-frame pop. Battle-scene-off still skips the anim. Tests: `tackle_runs_the_gen3_anim`, `send_out_ball_then_mon`. The stub route's after-battle node moved from frame 1211 to 1221. The start node's digest is unchanged. Emerald checkpoints were not re-seeded.

### `evolution` — closed

Merged `81eac8cf`. The scene shows the front picture, queues a cry, and lets B cancel during the picture swap only for level-up and friendship (not a stone or a trade). Friendship evolves at 220. The species changes when the scene commits. Playing the cry is still the audio gap. Checkpoints were not re-baselined.

### `trainer-held-items`

`TRAINER_HELD_ITEMS` is `false` in `crates/content/build/classify/battle.rs`. Emerald trainer Pokémon do not hold their berries or Quick Claw. Turning it on changes battles and the checkpoint tree; re-baseline only for this item, in its own commit, after proving the divergence is the held item.

**Done when:** a trainer mon with a held item enters battle holding it, and the item's effect runs. A test covers a Sitrus Berry.

### `field-moves`

Cut, Surf, Strength, Rock Smash, Waterfall, and Dive work from the field. From the party menu only Fly runs; every other field move reports `Unsupported("field move")` (`crates/engine/src/ui/party.rs`). Flash, Teleport, Dig, Secret Power, Soft-Boiled, Milk Drink, and Sweet Scent have no effect. Fly's picker is a name list, not the region map (`crates/engine/src/ui/fly_map.rs`).

**Done when:** each of the 14 `FieldMove`s works from the party menu as in Gen3, including the region map for Fly. A test uses Flash in a dark cave and Sweet Scent on a route.

### `bikes-rods`

Bag USE on a bike or a rod reports `Unsupported` (`crates/engine/src/item_use.rs`, `crates/engine/src/ui/bag.rs`). The Mach Bike does not climb the muddy slope (`classify/field.rs`).

**Done when:** the Mach Bike and Acro Bike change the player's movement (including the muddy slope and the Acro tricks the overworld checks), and a rod starts a fishing encounter. A test rides the Mach Bike up a muddy slope and hooks one fish.

### `move-effects` — closed, already present

Verified 2026-10-25. Every `EFFECT_*` used by an Emerald move already dispatches (`move_effect_for` in `crates/content/build/classify/battle.rs`). 355 moves, 198 effects. `None` remains only for five unused slots no move uses (`EFFECT_UNUSED_60`, `EFFECT_UNUSED_6E`, `EFFECT_UNUSED_83`, `EFFECT_UNUSED_8D`, `EFFECT_UNUSED_A3`), so the content build does not write `MoveEffect::Unsupported`. No change merged.

### `abilities` — closed, already present

Verified 2026-10-25. Under Emerald rules every Gen3 `ABILITY_*` (`0x00`–`0x4d`) maps to a named `Ability`. All 76 abilities on Emerald species have a battle or overworld path. Cacophony is the only Gen3 ability no species has, and Gen3 does not implement it either. The content build reports `0 abilities left as Ability::Other`. The early ledger's 36 missing entries are stale. No change merged.

### `battle-timing` — closed, Emerald checkpoints broken

Merged `a772ee2d`. Trainer parties and the first-battle mon are built on the frame after the transition, after the AI setup draws and before `SetWildMonHeldItem`. Field wilds and `setwildbattle` still create the mon before the transition. Test: `a_wandering_object_consumes_rng_during_the_transition_before_the_party`. The stub checkpoint tree still replays. The Emerald tree is `BROKEN: trace ends while Script` at `route:lab` and `spine:act1:lab`, and later nodes follow their parents. `verify --accept` cannot take broken nodes, so those traces were not re-recorded.

### `ui-pictures` — closed

Merged `7e79cfb3`. The party, summary, bag, mart, and Hall of Fame draw extracted Gen3 pictures and icons when the game provides them. A test finds a non-empty sprite in the party menu and the Hall of Fame. Checkpoints did not move. `a_stub_screen_closes_back_to_the_field` still expects the options menu to report `Unsupported("options menu")`, which it no longer does.

### `credits` — closed

Merged `4feaf93f`. The 57 Gen3 pages scroll, with a scene change every 6 pages (bike rides and Pokémon interludes). START is ignored until THE END. The original ocean, forest, and city tiles are not in the extract, so each scene is its palette, the player sprite as the riders, and front pics when the game has them. Checkpoints did not move.

### `cable-car` — closed

Merged `deb81e08`. `SPECIAL_CableCar` draws the car, the scrolling view, and the arrival fade for 602 frames, then warps. Test: `cable_car_scene_is_up_before_the_warp`. No checkpoint re-baseline.

### `easy-chat` — closed

Merged `1baeea8c`. Easy Chat builds a phrase from Gen3's new-game word groups (Trainer through Adjectives), confirms a changed phrase, and still refuses to quit a contest interview or an apprentice. `ShowEasyChatScreen` no longer reports a degradation. Pokémon names, moves, Events, trendy sayings, and the alphabet keyboard are still absent. Checkpoints did not move.

## Rules for the agent that owns an item

1. Work in a new worktree, `.claude/worktrees/gap-<id>`, branch `gap/<id>`, cut from `master`. The main working tree has unrelated uncommitted edits. Do not stage, stash, revert, or commit them.
2. Verify the item is still missing against the files named above. If it is already the official behaviour, stop and do not merge.
3. Implement that item only. Do not edit this file.
4. Validate with tests that fail before the change and pass after it, plus `cargo fmt` and the tests of every crate touched.
5. If hashed state that a committed checkpoint reaches changes, re-baseline in a separate `chore(checkpoints)` commit after proving the divergence is this item. Do not accept a divergence you cannot explain.
6. Merge into `master` with `--no-ff` only after validation. Take `.git/gap-merge.lock` with `mkdir` (retry until it is created) so two merges do not run at once, rebase onto current `master` first, then release the lock. If the merge would require stashing the main tree's uncommitted files, abort and report.
