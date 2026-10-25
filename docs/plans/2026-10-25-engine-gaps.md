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

### `rtc`

`gettime` is Unsupported (`commands.ron`): `VAR_0x8000..VAR_0x8002` stay stale. There is no RTC.

**Done when:** `gettime` writes hours, minutes, and seconds the way Gen3 does, and a test covers a known time.

### `link`

No link play. `SPECIAL_BufferUnionRoomPlayerName` is `Default(0)` ("no Union Room partner"). `SPECIAL_CloseLink` is Ignore. Link-battle, record-mixing, and Union Room specials are Unsupported.

**Done when:** two engines can complete a Gen3 link battle and a Union Room exchange, or the same flow against a scripted partner that speaks the real protocol. A test covers one full exchange. Unsupported link specials used by that flow are Native.

### `contests`

Contest commands are Unsupported: `choosecontestmon`, `startcontest`, `showcontestresults`, `contestlinktransfer`, `showcontestpainting`, `buffercontestname`. Contest specials (`SPECIAL_GenerateContestRand`, the contest-lady and contest-hall specials) are Unsupported or Default.

**Done when:** a contest can be entered, judged, and finished with results, matching Gen3's ranking rules. A test runs one contest from a script. Those commands are Native.

### `frontier`

`SPECIAL_CallFrontierUtilFunc` answers "no challenge" and does not store facility data. Battle Tower, Dome, Palace, Arena, Factory, Pyramid, and Pike specials are Unsupported. `DoSpecialTrainerBattle` stays Unsupported for the tents.

**Done when:** one facility (Battle Tower) can be challenged, fought, and recorded, with `GET_STATUS` / `GET_DATA` / `SET_DATA` reading that record. A test clears one challenge. The Tower specials that challenge uses are Native.

### `secret-bases`

Secret-base specials are Unsupported (`SPECIAL_EnterSecretBase`, `SPECIAL_CheckPlayerHasSecretBase`, the PC and battle specials). `MB_SECRET_BASE_BREAKABLE_DOOR` does not break (`crates/content/build/classify/field.rs`).

**Done when:** the player can enter a secret base, decorate it, and fight its trainer. A test covers enter and one decoration. Those specials are Native.

### `slot-machine`

`playslotmachine` is Unsupported.

**Done when:** the Game Corner slot machine plays, pays, and updates coins. A test covers one winning spin and one losing spin.

### `decoration-mart`

`pokemartdecoration`, `pokemartdecoration2`, and `bufferdecorationname` are Unsupported.

**Done when:** a decoration mart buys a decoration into the PC and the name buffer is filled. A test covers one purchase.

### `mystery-gift`

`setmysteryeventstatus`, `trywondercardscript`, and `setmodernfatefulencounter` are Unsupported.

**Done when:** a Wonder Card can be delivered and the script it points at runs. A test covers one card.

### `ereader`

e-Reader specials (`SPECIAL_DoesPartyHaveEnigmaBerry` is `Default(0)`; the other e-Reader specials are Unsupported) do not read a card.

**Done when:** an e-Reader payload can grant the Enigma Berry and the special answers from that state. A test covers one payload.

### `braille`

`braillemessage` is Unsupported.

**Done when:** a braille message is drawn and waits for input the way Gen3 does. A test covers one message.

### `pokenav`

`SPECIAL_OpenPokenavForTutorial` is degraded: no PokéNav screen. Match Call text is printed in the field box (`pokenavcall` is Native as text only).

**Done when:** the PokéNav opens from the start menu, shows the Match Call list, and places a call. A test opens it and reads one call. The tutorial special opens that screen.

### `pokedex`

Seen/caught flags and counts exist (`crates/engine/src/pokedex.rs`, `SPECIAL_ScriptGetPokedexInfo`, `SPECIAL_ShowPokedexRatingMessage`). There is no Pokédex screen.

**Done when:** START → POKéDEX opens the list, the detail page, and the rating, for Hoenn and National mode. A test opens it and finds a caught species.

### `pc`

`crates/engine/src/ui/pc.rs` is a stub (`stub_step`). Box storage data exists.

**Done when:** the Pokémon storage system and the item PC withdraw, deposit, and move, including the party. A test deposits a Pokémon and withdraws it.

### `options`

`crates/engine/src/ui/options.rs` is a stub. `crate::options::Options` already holds the Gen3 defaults.

**Done when:** the options menu changes text speed, battle scene, battle style, sound, and button mode, and those values affect play. A test changes text speed and reads it back.

### `naming`

`crates/engine/src/ui/naming.rs` is a stub.

**Done when:** the player, a rival, a nickname, and a box can be named, with Gen3's character set and length limits. A test names a Pokémon and the party stores that nickname.

### `title` — closed

Merged `10253afd`. Power-on with a save shows the title (copyright, blinking PRESS START, a flying Pokémon). Start opens the main menu, and B there returns to the title. Continue still restores the save. Tests: `power_on_shows_the_title_and_b_returns_to_it` and `an_in_game_save_boots_to_the_title_and_continues`. Checkpoints did not move.

### `move-anims`

Battle pictures lunge, blink, and tint (`crates/engine/src/battle/gfx.rs`). There is no per-move animation. Send-out has no affine zoom.

**Done when:** a move plays its Gen3 animation (the ROM's anim script and sprites), and send-out zooms. A test plays Tackle and asserts the anim runs.

### `evolution`

Level-up evolution changes the species with no scene and reports `evolution scene` (`crates/engine/src/world/evolve.rs`). Friendship evolutions are not evaluated. `crates/engine/src/ui/evolution.rs` is a stub that answers "not evolved". Stones and Rare Candy report the same.

**Done when:** the evolution scene plays (picture, cry hook, B cancels when Gen3 allows it) and friendship evolutions happen at 220. A test evolves a level-up species through the scene and a friendship species.

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

### `battle-timing`

The opponent party and the wild encounter are rolled when the battle is requested, before the transition (`crates/engine/src/world/battle.rs`, "Deviations from Gen3"). Gen3 rolls them in `CB2_InitBattle`, after the transition.

**Done when:** those draws happen after the transition, in Gen3's order, and a test shows an object that animates during the transition consumes the RNG before the party is built. Re-baseline checkpoints only if this item moves them, in its own commit.

### `ui-pictures`

Menus are frames and text. Item and Pokémon icons are not drawn (`crates/engine/src/ui/kit/mod.rs`). The party, summary, bag, and mart document the missing pictures (`party.rs`, `summary.rs`, `bag.rs`, `mart.rs`). The Hall of Fame has no mon pictures (`hall_of_fame.rs`).

**Done when:** the party, summary, bag, mart, and Hall of Fame draw the Gen3 pictures and icons from extracted graphics. A test finds a non-empty sprite in the party menu and the Hall of Fame.

### `credits`

`GameClear` opens the Hall of Fame, then the credits are a timed black screen (`crates/engine/src/ui/hall_of_fame.rs`). The bike and Pokémon scenes between pages are left out. START skips pages.

**Done when:** the credits scroll the Gen3 pages (text and the scenes between them) and cannot be skipped with START. A test watches the first page's text appear.

### `cable-car`

`SPECIAL_CableCar` fades, waits 602 frames, and warps. There is no ride scene (`specials.ron`).

**Done when:** the cable car scene plays (the car, the view, the arrival) for those 602 frames and then warps. A test starts the ride and asserts the scene is up before the warp.

### `easy-chat`

`crates/engine/src/ui/easy_chat.rs` has no keyboard and no word lists, so the phrase never changes.

**Done when:** Easy Chat edits a phrase from Gen3's word list and confirms it. A test builds a phrase and reads it back. Interview and apprentice modes that Gen3 forbids quitting still forbid it.

## Rules for the agent that owns an item

1. Work in a new worktree, `.claude/worktrees/gap-<id>`, branch `gap/<id>`, cut from `master`. The main working tree has unrelated uncommitted edits. Do not stage, stash, revert, or commit them.
2. Verify the item is still missing against the files named above. If it is already the official behaviour, stop and do not merge.
3. Implement that item only. Do not edit this file.
4. Validate with tests that fail before the change and pass after it, plus `cargo fmt` and the tests of every crate touched.
5. If hashed state that a committed checkpoint reaches changes, re-baseline in a separate `chore(checkpoints)` commit after proving the divergence is this item. Do not accept a divergence you cannot explain.
6. Merge into `master` with `--no-ff` only after validation. Take `.git/gap-merge.lock` with `mkdir` (retry until it is created) so two merges do not run at once, rebase onto current `master` first, then release the lock. If the merge would require stashing the main tree's uncommitted files, abort and report.
