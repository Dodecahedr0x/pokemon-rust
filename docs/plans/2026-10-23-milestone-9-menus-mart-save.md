# Milestone 9 — Menus, Mart and Save (Silice SIL2 prerequisites) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** give the engine the player-facing UI a real game needs, in the order the Silice chapters need it. Silice drives the roadmap (user decision 4 of `2026-10-23-silice-content.md`): M9 delivers everything SIL2 (Silice Ch1–Ch3) waits on except the field moves, and Emerald story goals past Act 1 are deferred.
- **Menus:** the field start menu (with a game-defined entry, Silice's Mallette), the party menu and summary, the bag with item use, the options menu, and the battle's BAG and POKéMON actions (player switching, battle items, ball choice, Gen3's shift prompt).
- **Mart and PC:** `pokemart` buy and sell, PC item storage, Pokémon storage with box overflow (`givemon` and catches past six), and a simplified storage UI.
- **Save and load:** an in-game save (start menu SAVE and `special SaveGame`) written to flash in our own format, and a boot main menu (CONTINUE / NEW GAME / OPTION).
- **SIL2 Wave 0 engine contracts**, built: `ScreenFlash`, `ScreenShake`, `MoneyGet`, `WaitInputOrTimeout`, `AddFriendship`, white fades, `YesNo { default_no }`, `SetTint`, `GiveMon` with moves, item and OT, `BattleTrainer { no_whiteout }`, the game-defined start-menu entry, and the `field_move` gate hook M10 fills.
- **Evolution:** the evolution scene, exact evolution methods, friendship (walking, levels, fainting, scripts) and item evolutions from the bag.
- **The M8 findings** that block Silice or the next Emerald act: the dirty-cell `DrawWholeMapView`, scripted ladder facing, `MB_PETALBURG_GYM_DOOR` arrivals, the Briney-boat `RoundTrip`, the specials the new screens make reachable, `explore sweep`, explorer policy switching and items, the deferred studio views.

**Not in M9 (and why)**
- **Cut, Strength and Flash are M10.** M9 is the UI milestone (eight screens, a save format, a battle UI rework). Field moves need their own engine pieces (Strength's boulder physics, Flash's darkness window, the field-effect animations) and SIL2 has script fallbacks for all three (`if flag(FLAG_SIL_MODULE_*) { set_metatile }`, §9 of each chapter). M9 adds the hook M10 fills (`GameData::field_move_gate`) and lists field moves in the party menu, so M10 is pure behaviour. SIL2 Ch1 does not need Cut before its gym (the module is its reward).
- **The Pokédex screen, trainer card, PokéNav, naming screen and the Emerald intro** (truck, Birch's speech). The Pokédex's seen and caught flags are recorded from Wave 0 on, so the screen (M10 or later) needs no state change. The naming screen is a Wave 3 stretch (Task W). The Emerald intro waits for Emerald's Act 2 work; Silice has its own opening.
- **Gen3's exact Pokémon storage UI** (`pokemon_storage_system.c`, box wallpapers, the hand cursor, mon icons dragged between boxes). M9 ships the storage logic and a list UI reported as `Degraded("pc storage ui")`.
- **The escalator ride and the special warp effects** (Lavaridge, Aqua Hideout, Mossdeep, Mt. Pyre, Union Room): Emerald Act 3+ and no Silice Ch1–Ch3 map has them. M10 with the field effects.

**Architecture**
- **Screens.** A screen is a module under `crates/engine/src/ui/` implementing one `Screen` trait. `World` keeps a small **screen stack** (start menu → party → summary; mart → bag; bag → party), hashed while non-empty, never captured. A full-screen screen takes the VRAM the way the battle scene does (`video::layout::menu`, spread uploads under the line budget) and the field comes back through `Field::restore_spread`, as after a battle. The start menu and save dialog overlay the field on BG0.
- **One UI kit.** List menus (Gen3 `list_menu.c`), the 20 user window frames, the money box, the quantity selector, item and mon icons, scroll arrows and screen fades live in `ui/kit/`, written once (Task K) before the screens fan out (Wave 2).
- **Our own save format.** The in-game save is the M5 checkpoint (`WorldSnapshot`, postcard) in the agb flash slot, normalised to "field idle" (decision 4). It already survives content rebuilds and already round-trips byte-identically between the ROM and native.
- **One state change, once.** Wave 0 adds all new `World` state (options, Pokédex flags, PC storage, coins, the registered item, screen effects, the screen stack), with its hash, its snapshot DTOs (`SCHEMA` 4) and one tree refresh. Behaviour that moves the Emerald route merges one task at a time, as in M8.
- **Game-agnostic as before.** The engine names no Emerald constant: the start-menu layout, item use classes, field-move gates, PC capacity, badge flags and texts come through `GameData` and `SpecialIds`.

**Tech stack**
- Rust 2024. **No new dependency** in `engine`, `content` or `game-gba`.
- `extract` locates the new ROM tables by anchors and pointer tracing (M8 decision 11), checked on BPEE against the decomp (`PKR_DECOMP`), every exported string re-encoded to its ROM bytes.

A task may add to a contract if it needs to. It may not change the meaning of what is already there, and every addition must be listed in its final report.

---

## Where things stand (measured while planning, master `cfc9070`)

| Area | After M8 | Gap for M9 |
|---|---|---|
| Start menu | none; START is never read in the field (`world.rs`, `flow/input.rs`) | the whole menu, a game-defined entry |
| Party / summary | `Party` (`mon.rs:1102`), no screen; battle `Need::Switch` takes the first usable mon (`battle/scene.rs:61`) | party menu (field, choose, battle modes), summary |
| Bag | `Bag` state and rules (M8 H); `ItemInfo { name, pocket, price, key }`; battle BAG does nothing (`battle/scene.rs:59`) | bag UI, item use effects, item descriptions, icons |
| Mart | `pokemart` `Unsupported` (`commands.ron`), its list pointer undecoded (`scripts.rs:307` `W`) | buy/sell screens, item lists in the IR |
| PC | `SPECIAL_PlayerPC`, `BedroomPC`, `ShowPokemonStorageSystemPC` `Unsupported`; `ScriptMenu_CreatePCMultichoice` answers B; `givemon` with a full party logs "no PC" (`world/host/mod.rs:255`) | item storage, boxes, overflow |
| Save | the debug combo (L+R+SELECT) writes a checkpoint to a 64 KiB flash slot (`game-gba/src/save.rs`), `MAX_ENCODED` 8 KiB; `SPECIAL_SaveGame` is `Default(0)` ("not saved"), which keeps the link specials unreachable | in-game save, main menu, capacity for a PC |
| Options | text at a fixed 4 frames a glyph (`text.rs:75`, ≈ Gen3 MID); no battle style (Gen3's default SHIFT prompt is missing) | options state and screen; shift prompt |
| Evolution | silent level evolution after battle, `Degraded("evolution scene")`; `EvoMethod::Level` folds Silcoon/Cascoon and Tyrogue; friendship never changes (`mon.rs:688`) | scene, exact methods, friendship events |
| Fades and effects | `Fade::{ToBlack, FromBlack}`; `Frame.lighten` exists (`video.rs:204`); FADE_TO_WHITE is lowered as black (`extract/src/codegen/lower/emit.rs:726`); no flash, shake or tint | SIL2 requests |
| Specials | 448 of 528 `Unsupported` (most unreachable), 69 commands `Unsupported` | the menu-related ones (Task J) |
| Explorer | 8 macro kinds in 3 bits of `Macro::code` (all used), no START, battle policy Fight/Run only | menu macros, switching and items, `explore sweep` |

---

## Decisions (summary; reasons in "Decisions taken" at the end)

| # | Topic | Decision |
|---|---|---|
| 1 | Scope | SIL2's prerequisites minus field moves: start menu, party, summary, bag and item use, options, battle BAG/POKéMON, mart, PC, in-game save, main menu, the SIL2 Wave 0 requests, evolution and friendship, and the M8 findings that block Silice or Act 2. |
| 2 | Field moves | M10. M9 adds `GameData::field_move_gate` and the party menu's field-move entries; using one reports `Unsupported("field move")` until M10. |
| 3 | Screens | `ui/<screen>.rs` per screen behind one `Screen` trait; a hashed screen stack in `World`; full-screen screens own the VRAM like the battle scene; the field is restored by `Field::restore_spread`. Not capturable while a screen is open. |
| 4 | Save format | **Our own**: the checkpoint snapshot, normalised to field idle, in the agb slot. Not Gen3's sector layout. A host-only Gen3 save *reader* (stretch, Task V) feeds the vanilla reference lanes. |
| 5 | Flash and PC capacity | 64 KiB flash stays (stock mGBA detects 64 KiB for our ROM, M5). Boxed mons use a compact DTO (no stats, HP or status: recomputed on withdraw). PC capacity is a game role (`SpecialIds::pc_boxes`); Emerald takes Gen3's 14 boxes **if the worst case fits the slot** (Wave 0 spike), else the largest count that fits, reported as a deviation. 128 KiB only if the spike shows a stock mGBA reads it back. |
| 6 | Save timing | The engine runs Gen3's save dialog frame for frame; the shell writes flash when the World says so. Flash time stalls the shell, not the engine: native and ROM step the same frames, and late frames while writing are counted apart (`late_saving`), not gated. |
| 7 | Main menu | A small engine state machine (`ui/main_menu.rs`) the shells run at boot when a save exists: CONTINUE (info window: name, Pokédex, time, badges), NEW GAME, OPTION. Tools keep `World::new` and `World::restore`. No title screen. |
| 8 | Pokémon storage UI | Logic exact (Gen3 `SendMonToPC`, box order, `ScriptCheckFreePokemonStorageSpace`), UI a list (`Degraded("pc storage ui")`). The Gen3 box screen is M13. |
| 9 | Options | Gen3's six options with Gen3 defaults (text MID, battle scene on, style SHIFT, stereo, normal buttons, frame 1). The defaults reproduce today's behaviour, so the options are route-neutral; the shift prompt (a missing Gen3 behaviour) is route-changing (Task BM). |
| 10 | Hash, schema, trees | Wave 0 is the only planned state change, `SCHEMA` 4. New state is hashed only when it differs from its default (as `Pokemon.held`), except the Pokédex flags, which change on the first encounter (one re-accept). Schema 3 stays readable (new fields empty, digest unchanged). Route-changing tasks merge one at a time: **F → E → BM**, then any other task whose route check moved. |
| 11 | SIL2 contracts | `GameData::field_move_enabled` of the Silice plan is named `field_move_gate` here (it returns the gate, not a bool); close-out updates §3.2. Delivered in full by Wave 0 (shapes) and Task R (behaviour). The existing `YesNo`, `GiveMon` and `BattleTrainer` keep their hash bytes when the new fields are at their defaults, so Emerald does not move. |
| 12 | Parity | ROM-vs-native lanes for every screen and for save → continue; a GBA **screen tour** (every screen opened on a fixed save, `late == 0`, loop ≤ 217); decomp-constant tests for layouts; opt-in **vanilla reference lanes** (BPEF in mGBA from the user's own `.sav`, never committed) for outcomes and frame counts (Task V, stretch). |
| 13 | Silice | M9 unlocks SIL2: Ch1 (Bois-Éther) fully, Ch2–Ch3 with the documented field-move fallbacks until M10. The Mallette's own panels (calls, logs, sync gauge) are SIL2 content on Task K's kit. |

---

## Current code this builds on

- **Engine:**
  - `script.rs`: `Request :98` (tags 0–55; `CurrentMap` is 55), `Response :415` (`Unit`, `Bool`, `Value(u16)`, `Pos`), `Fade :52`, `Report :89`, `Ctx` helpers `:508–813`.
  - `world.rs` (801 lines): `State :156`, `World::hash :707`; `world/battle.rs`: `Mode :82` (`Field`, `Starter`, `Battle`), post-battle evolution `:458`; `world/host/mod.rs` (386 lines) routes to `host/{buffers,items,menu,trainer,warps}.rs`, `givemon` `:255`; `world/checkpoint.rs` (capturable, `MAX_MONEY`).
  - `menu.rs` (multichoice, `GLYPHS_PER_FRAME` 12), `text.rs` (`YES_NO_WINDOW :63`, `FRAMES_PER_CHAR :75`), `text/printer.rs`, `bag.rs`, `mon.rs` (`Pokemon :676`, `Party :1102`, evolution helpers), `video.rs` (`layout :23`, `Frame.lighten :204`).
  - `battle/scene.rs` (input rules `:56–66`), `battle/scene/menu.rs` (`Choice::Switch :85`), `battle/core.rs` (`Action::Ball :1187`, `Need::Switch :1385`), `battle/ui.rs` (action menu `:574`), `battle/calc.rs` (`catch_odds :463`).
  - `checkpoint.rs`: `SCHEMA` 3, `OLDEST_READABLE` 2, `MAX_ENCODED` 8 KiB, `FLASH_SIZE` 64 KiB, `FLASH_SLOTS` 1.
  - `data.rs`: `UiText :494`, `SpecialIds :516`, `GameData :564` (`item`, `multichoice`, `std_string`, `evolutions`, `hold_effect`, `form_changes`, …).
- **Content:** `build/support/{commands,specials}.ron`, `src/prelude/{special,items,menu,msgbox,std,trainer}.rs`, `src/lib.rs` (`game_data_tables! :246`, `gen3_special_ids`), `build/classify/{field,battle}.rs`, `src/silice/{mod,prelude}.rs` (the SIL1 fallbacks `screen_shake`, `play_se`, `mallette_call`).
- **Extract:** `scripts.rs :307` (`pokemart` operand `W`), `export/tables.rs`, `tables.rs`, `codegen/lower/{emit,operands}.rs`, `schema.rs` (`FORMAT_VERSION` 5, `FORMAT_MINOR` 5).
- **Shells:** `game-gba/src/{main,save,save_logic}.rs`, `game-native/src/{flash,lib,store}.rs`, `emu/tests/{parity,tour,save_logic}.rs`, `xtask/src/{budget,sweep}.rs`.
- **Explorer:** `model.rs :26` (`MacroKind`, 3-bit tag full), `policy.rs`, `expand.rs`, `goals.rs`.
- **Committed trees:** `checkpoints/emerald/nodes` (c00001–c00004 plus Task P's `act1:*`), `checkpoints/stub/nodes` (c00001–c00003), `checkpoints/silice/nodes` (SIL1's `sil:ch0:*` once promoted).

---

## Wave 0 contracts (committed before Wave 1; read them, do not redesign them)

Wave 0 makes no feature work beyond two tiny behaviours that would otherwise put two tasks on one file (`YesNo { default_no }` and the text-speed plumbing, both route-neutral at their defaults) and the Pokédex flag recording (part of the one hash change). It does five things:
- the spikes whose numbers fix decisions 5 and 6 and the screen upload budget;
- every new type, file, request and `GameData` method, with stub bodies or safe defaults;
- the module splits that let Waves 1 and 2 run without shared files;
- the one state change (new state, hash, `SCHEMA` 4, goldens, tree refresh);
- the route-usage scan that tells each task what its route delta should be.

`cargo test --workspace` (stub and Emerald), `PKR_GAME=silice cargo test -p content -p engine -p extract`, `cargo xtask rom`, `cargo xtask parity` and `explore` stay green after Wave 0.

### Step 0 — reconcile, spikes and baselines
1. **Rebase on master after the M8 close-out** (Task P, its "Wave 3 as built" and "Findings for Milestone 9"). Every finding there goes into this doc's carried table ("Findings carried from M8") with an owner task or a later milestone; a finding the close-out already fixed is marked done.
2. **Save spike** (throwaway branch):
   - Build the **worst-case save**: six party mons with every field at its widest varint, every flag and var set, every pocket full at 99, PC items full, every box full with the compact boxed DTO, the longest names, every Wave 0 field non-default. Record `encoded_len`.
   - Measure the agb slot's real capacity on 64 KiB with one slot (write growing payloads in `game-native::flash` until `OutOfSpace`).
   - Try 128 KiB: `init_flash_128k` on a scratch ROM in `emu` (forced FLASH1M) and in a **stock** mGBA (boot, save, reboot, read back).
   - Apply decision 5: set `SpecialIds::pc_boxes` for Emerald (14 if it fits) and `MAX_ENCODED` to the measured slot capacity minus a 512-byte margin. Record the numbers in "Wave 0 as built".
3. **Screen budget spike:** a scratch full-screen upload the size of the party menu (three BG tilemaps, about 8 KiB of tiles, two palette banks, six mon icons) spread over black frames on the GBA. Set `ui::kit::UPLOAD_BYTES_PER_FRAME` (black) and `UPLOAD_BYTES_VISIBLE` from it, as Task F did for warps (`SPREAD_TILES_BLACK`).
4. **Route-usage scan:** over the flattened Act 1 trace and the slice route, count START presses in the field, `pokemart` and `SaveGame` reached, `ChoosePartyMon`/`ChooseMonForMoveTutor` reached, FADE_TO_WHITE executed, BAG/POKéMON chosen in battle, field steps with a poisoned or Wurmple-line party, evolutions, and trainer battles won with two or more usable mons (the shift prompt). Write the table into "Wave 0 as built"; every task's route delta is checked against it.
5. **Baselines on master:** parity (route, `act1:*` lanes, tour), `explore spine` and `explore branch` (since the spine rework), `cargo test --workspace` wall time in all three flavours, ROM / IWRAM / EWRAM, `heap_max` per lane.

### Engine contracts (`crates/engine`)
- **`data.rs`** (Wave 0 is its only editor in M9, except Task E's `EvoMethod` variants, which Wave 0 adds too):
  ```rust
  /// One start-menu entry (Gen3 `sStartMenuItems`), in display order.
  pub struct StartItem { pub label: TextId, pub action: StartAction, pub shown_if: Option<FlagId>,
                         pub description: Option<TextId> }
  pub enum StartAction { Pokedex, Party, Bag, PlayerCard, Save, Options, Exit,
                         /// Game-defined (Silice's Mallette, Emerald's PokéNav placeholder): runs the
                         /// script with the field locked, after the menu closes (Gen3 `ScriptContext_SetupScript`).
                         Script(ScriptId) }
  pub enum FieldMove { Cut, Flash, RockSmash, Strength, Surf, Fly, Dive, Waterfall, Teleport, Dig,
                       SecretPower, SoftBoiled, MilkDrink, SweetScent }
  /// How a field move becomes usable (M10 implements the uses).
  pub enum FieldMoveGate { Never, PartyMove { move_id: MoveId, badge: Option<FlagId> }, Flag(FlagId) }
  /// What using an item does (classified by content from Gen3 `gItemEffectTable` and the item's
  /// field/battle use functions).
  pub enum ItemUse { None, Medicine(MedicineEffect), Ball(u8 /* multiplier ×10 */), StatStage(u8, i8),
                     Evolution, Repel(u16), EscapeRope, Tm(MoveId), Berry, Key(KeyUse), Other(&'static str) }
  pub enum UiGfx { /* one variant per extracted UI picture: PartyBg, PartySlots, BagBg, BagSprite(u8),
                     SummaryPages, ShopBg, OptionsBg, EvolutionBg, StatusIcons, TypeIcons,
                     ScrollArrows, MenuCursor, MoneyBox, … (final list from Task X1) */ }
  pub struct UiPicture { pub tiles: &'static [u8], pub map: &'static [u8], pub palette: &'static [u16] }
  pub struct Icon { pub tiles: &'static [u8] /* 4bpp */, pub palette: &'static [u16], pub frames: u8 }
  pub struct InGameTrade { /* Gen3 `sIngameTrades` entry: species asked and given, nickname, ivs,
                            ability, personality, OT name and id, held item, mail */ }
  ```
  `UiText` gains every fixed UI string the screens print (start-menu labels come from `StartItem`; the rest: party actions, bag actions and pocket names, shop prompts, save prompts, option names and values, main-menu labels, summary headings), each a variant, answered by content through `ui_text`.
  `EvoMethod` gains the exact Gen3 kinds (`FriendshipDay`, `FriendshipNight`, `LevelAtkGtDef`, `LevelAtkEqDef`, `LevelAtkLtDef`, `LevelSilcoon`, `LevelCascoon`, `LevelNinjask`, `LevelShedinja`, `Beauty`, `TradeItem`), with `Level`, `Friendship`, `Item` and `Trade` kept.
  New `GameData` provided methods (defaults: empty, `None` or `Never`):
  ```rust
  fn start_menu(&self) -> &'static [StartItem];
  fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate;
  fn item_use(&self, id: ItemId) -> ItemUse;
  fn item_description(&self, id: ItemId) -> Option<&'static [u8]>;
  fn move_description(&self, id: MoveId) -> Option<&'static [u8]>;
  fn tm_move(&self, id: ItemId) -> Option<MoveId>;
  fn can_learn_tm(&self, species: SpeciesId, tm: ItemId) -> bool;
  fn ui_gfx(&self, g: UiGfx) -> Option<&'static UiPicture>;
  fn window_frame(&self, n: u8) -> Option<&'static WindowGfx>;   // the 20 user frames
  fn item_icon(&self, id: ItemId) -> Option<&'static Icon>;
  fn mon_icon(&self, species: SpeciesId) -> Option<&'static Icon>;
  fn in_game_trade(&self, n: u8) -> Option<&'static InGameTrade>;
  fn nature_name(&self, n: u8) -> Option<&'static [u8]>;
  ```
  `SpecialIds` gains, filled in `gen3_special_ids()` for both roots: `pc_boxes: u8`, `pc_item_slots: u8` (Gen3 50), `max_coins: u16` (9,999), `flag_badges: [Option<FlagId>; 8]`, `var_repel_steps`, `var_friendship_steps`, `script_repel_wore_off: Option<ScriptId>`, `flag_pokedex: Option<FlagId>` (the CONTINUE window's Pokédex line). A game without them gets 0 / `None` and the matching feature stays off.
- **`ui/` (new):**
  - `ui/mod.rs` (Wave 0; frozen after): the `Screen` trait, `ScreenKind`, `ScreenOut`, `ScreenResult`, the `ScreenStack` (depth ≤ 4, hashed), `ScreenCx` (the game, the frame, and `&mut` party, bag, money, PC, options, coins, Pokédex, plus read access to flags and vars), and the dispatch `match` to every screen module.
    ```rust
    pub enum ScreenKind { StartMenu, Party(PartyMode), Summary { slot: u8, mode: SummaryMode }, Bag(BagMode),
                          Mart { items: &'static [ItemId] }, Pc(PcMode), Options, SaveDialog,
                          Evolution { slot: u8, into: SpeciesId, can_cancel: bool }, MainMenu, Naming(NamingKind) }
    pub enum PartyMode { Field, Choose /* ChoosePartyMon */, MoveTutor(MoveId), UseItem(ItemId),
                         GiveItem(ItemId), Battle { forced: bool } }
    pub enum BagMode { Field, Sell, Battle, ChooseBerry }
    pub enum SummaryMode { View, ReplaceMove(MoveId) }
    pub enum PcMode { Someone /* PlayerPC from a Center */, Bedroom, Storage /* ShowPokemonStorageSystemPC */ }
    pub enum NamingKind { Player, Mon(u8 /* party slot */), Box(u8) }
    pub enum ScreenOut { Running, Push(ScreenKind), Done(ScreenResult), RunScript(ScriptId),
                         /// Ask the World to save now (`world/save.rs`); the screen gets the result next frame.
                         Save }
    pub trait Screen { fn step(&mut self, cx: &mut ScreenCx, input: Buttons) -> ScreenOut;
                       fn hash(&self, h: &mut Fnv64); }
    ```
  - Stub modules, one per owner, each a `Screen` that answers `Done(Cancel)` on its first frame with `Report::Unsupported("<screen>")`: `ui/{start_menu, options, party, summary, bag, mart, pc, save, main_menu, evolution, naming}.rs`. `ui/kit/mod.rs` is an empty module (Task K).
- **`world/` splits** (mechanical):
  - `world/mode.rs`: `Mode` moves out of `world/battle.rs` and gains `Screens(ScreenStack)`; `world/battle.rs` keeps the battle (Task BM owns it).
  - `world/screens.rs` (Task K): `open`, `step_screens`, `close`, `start_pressed` and the field restore. Wave 0 adds the calls: `control` calls `screens::start_pressed` (Gen3 `ProcessPlayerFieldInput` order: after the trainer sight and before the A checks; only when the field is idle and the player is not moving), and `step` runs `step_screens` while `Mode::Screens`. Wave 0's bodies never open anything.
  - `world/save.rs` (Task S): `save_now(&mut State, game) -> Result<Checkpoint, SaveError>`, called by `world/screens.rs` on `ScreenOut::Save`; Wave 0's stub answers `Err`.
  - `world/evolve.rs` (Task E): the post-battle evolution moved out of `world/battle.rs` (`:458`), unchanged.
  - `world/caught.rs` (Task M): `place_mon(&mut State, mon) -> Placed` (party, else PC), used by the catch in `world/battle.rs` and by `givemon` in `world/host/mod.rs` (`:255`); Wave 0's body is today's behaviour (party, else "no PC").
  - `world/friendship.rs` (Task E): `on_step(&mut State, game)` called by `control` once per finished player step, before the step triggers; Wave 0's body does nothing.
  - `world/host/mod.rs` routes every new request to `world/host/{screens.rs (K), fx.rs (R), party.rs (R), save.rs (S)}`; stubs answer the defaults below. After Wave 0 no task edits `host/mod.rs`.
  - `mon.rs` → `mon.rs` plus `mon/evolution.rs` (Task E: `evolution_target`, the friendship and item checks, `try_evolve`) and `mon/boxed.rs` (Task M: `BoxedMon`, the compact DTO, `Pokemon::to_boxed`/`from_boxed`, implemented in Wave 0 because the snapshot needs it).
  - `battle/core.rs` → `battle/core.rs` plus `battle/core/items.rs` (Task BM: `Choice::Item` handling, stub refuses).
  - `wild.rs`: `pub(crate) fn repel_blocks(level, state) -> bool` hook (returns `false`; Task B).
  - **Field hooks for F and B:** `MapData` gains `map_type: u8` (Gen3 `MAP_TYPE_*`, content fills it; F reads it) and `WarpSlots.escape` (already M8 state) is the escape warp; F writes it, B reads it. `control` calls `item_use::on_step(&mut State, game)` once per finished step (B: the repel counter, Gen3 `UpdateRepelCounter`) and `item_use::select_pressed(&mut State, game)` on SELECT when the field is idle (B: the registered item). Both are no-ops in Wave 0.
  - **Hooks for R:** `world/host/party.rs::give_mon(host, species, level, extra) -> u16` (the `givemon` arm of `host/mod.rs` calls it; Wave 0's body is today's code, routed through M's `place_mon`) and `world/host/fx.rs::whiteout_allowed(&BattleTrainerReq) -> bool` (read by `world/battle.rs` when a trainer battle is lost; `true` in Wave 0). A white frame's upload budget needs no hook: `Frame::is_black` already treats a lightened frame as visible, so F's spread uploads count it as visible.
  - **Hooks for E:** `world/friendship.rs::on_event(&mut Pokemon, FriendshipEvent, game, rng)`, called by Wave 0 at Gen3's sites: level up (`battle/core.rs` exp gain, `mon.rs` level-up helper), faint (`battle/core.rs`), and a no-op until E fills it. E's move learning after an evolution reuses the M4 learn-move flow through `ScreenKind::Summary { slot, mode: SummaryMode::ReplaceMove(MoveId) }` (PT draws it, E pushes it), not `battle/ui.rs`.
  - **`World` methods for S:** `save_snapshot(&self, game) -> Result<Checkpoint, SaveError>` and `save_failed(&mut self)` declared in `world.rs`, forwarding to `world/save.rs` (stub `Err`).
  - **Trades for J:** `Request::InGameTrade { slot: u8, trade: u8 }` (answers `Unit`; Wave 0 stub does nothing; J fills it in `prelude/special.rs` through a host handler in `world/host/party.rs`, which R owns: R implements the swap from `GameData::in_game_trade`, J the special).
  - `fx.rs` (new, Task R): `ScreenFx { flash, shake, tint }`, `apply(&ScreenFx, &mut Frame)` called last in `World::step` (Wave 0 calls the no-op). It post-processes the frame's palette uploads and scroll offsets, so a tint survives every field restore, map load and screen return without an edit in `field.rs`, `world/screens.rs` or `world/battle.rs`.
  - `lib.rs`: the `mod` lines for `ui`, `fx`, `item_use`, `pc`; `debug.rs`: the new debug-region offsets (`late_saving`, `screens`).
- **New `World` state** (implemented, hashed when non-default unless noted, snapshotted):
  - `options: Options { text_speed, battle_scene, battle_style, sound, button_mode, frame }`, Gen3 defaults;
  - `pokedex: Pokedex { seen, caught }` (bitsets over national numbers, a digest kept on write; hashed when non-empty, which on every route means from the first battle). **Recording is implemented in Wave 0:** seen for each opponent battler at the battle intro and on each opponent switch-in (Gen3 `HandleSetPokedexFlag(.., FLAG_SET_SEEN, ..)` in `battle_main.c` and `Cmd_switchindataupdate`), caught on a catch, both on `givemon` and on a trade (Gen3 `GetSetPokedexFlag`); Task E adds the evolution scene's own calls;
  - `pc: PcStorage { items: [ItemSlot; pc_item_slots], boxes: Vec<[Option<BoxedMon>; 30]>, current_box: u8 }` with digests on write (Gen3 `gPokemonStoragePtr`, `pcItems`); box names and wallpapers are not kept (the list UI shows "BOX n");
  - `coins: u16` (Gen3 `coins`, for the Silice casino and the Game Corner later);
  - `registered_item: Option<ItemId>` (Gen3 `registeredItem`, SELECT);
  - `fx: ScreenFx` (not snapshotted: a save or capture needs it clear; hashed only while active);
  - the screen stack (in `Mode`; hashed while non-empty; capture refused while non-empty, `NotCapturable::Screen`).
  `World::hash` appends them in this order. Accessors: `options()`, `pokedex()`, `pc()`, `coins()`, `registered_item()`, `screens_open()`.
- **`checkpoint.rs`:** `SCHEMA = 4`; `WorldSnapshot` gains `options`, `pokedex`, `pc`, `coins`, `registered_item` as trailing fields; schema 3 decodes them empty and `state_digest` of an all-default schema 4 snapshot equals its schema 3 digest (the M8 schema 3 pattern). `MAX_ENCODED` from the spike. Golden-bytes test updated. A `Checkpoint::save_meta()` adds the CONTINUE window's data (name, Pokédex count, play frames, badges) to `SaveMeta` (agb slot metadata, read without decoding the slot).
- **`script.rs`:** new `Request`s (tags 56 and up) and `Ctx` helpers. Wave 0 host stubs answer the default in brackets; nothing panics.
  ```rust
  // screens (host/screens.rs, Task K)
  Mart { items: &'static [ItemId] }                 // [Unit]   Gen3 `pokemart`
  ChoosePartyMon(PartyMode) -> Value                // [PARTY_NOTHING_CHOSEN 0xFF] ChoosePartyMon, ChooseMonForMoveTutor
  PlayerPc(PcMode)                                  // [Unit]   PlayerPC, BedroomPC, ShowPokemonStorageSystemPC
  SaveDialog -> Bool                                // [false]  special SaveGame (Gen3 `SaveGame`: the dialog, then saved?)
  Evolve { slot: u8, into: SpeciesId, can_cancel: bool } -> Bool  // [false] the scene; Silice `play_evolution`
  // effects (host/fx.rs, Task R)
  ScreenFlash { color: u16 /* BGR555 */, frames: u16 }
  ScreenShake { magnitude: u8, frames: u16 }
  SetTint { banks: u16 /* BG bit mask */, obj_banks: u16, color: u16, amount: u8 /* 0..=16 */, frames: u16 }
  WaitInputOrTimeout(u16) -> Bool                   // [false = timed out] true when A or B came first
  // party and money (host/party.rs, Task R)
  MoneyGet -> Word                                  // [Word(0)]
  CoinsGet -> Value, CoinsAdd(u16) -> Bool, CoinsRemove(u16) -> Bool   // [0 / false]
  AddFriendship { slot: u8, n: i8 }                 // [Unit]  slot 0xFF = the whole party
  PartyMonSpecies(u8) -> Value                      // [0]     Gen3 ScriptGetPartyMonSpecies
  ```
  `Response` gains `Word(u32)`. Changed shapes that keep their hash bytes at the default:
  - `YesNo` becomes `YesNo { default_no: bool }`: tag 12, `a = default_no as u16`, which is 0 for every Emerald `yesnobox`. **Wave 0 implements the cursor start** (`text.rs`), so R does not edit `text.rs`.
  - `GiveMon` gains `extra: Option<&'static GiveMonExtra>` (`moves: Option<[MoveId; 4]>`, `item: Option<ItemId>`, `ot: Option<(&'static [u8], u32)>`, `nickname: Option<TextId>`): the hash adds the extra's digest only when `Some`.
  - `BattleTrainer` gains `no_whiteout: bool`: hashed only when true.
  - `Fade` gains `ToWhite`, `FromWhite` (new hash values; `fadescreen` keeps lowering to black until Task J switches it).
- **`text.rs`:** `TextBox::set_speed(TextSpeed)` (Wave 0): MID keeps `FRAMES_PER_CHAR` 4, SLOW 8, FAST 1 (Gen3 `sTextSpeedFrameDelays`); the World passes `options.text_speed` when a message starts. Route-neutral at MID.
- **`video.rs`:** `layout::menu`: BG roles, char and screen blocks, palette banks and OBJ tile ranges for full-screen screens (from the decomp's party, bag and summary BG templates, folded onto our 32×32 maps as the battle layout does).
- **Events:** `Event::ScreenOpened(ScreenKindTag)`, `Event::ScreenClosed`, `Event::Saved { ok: bool }` (the shells' cue to persist), `Event::Evolved(SpeciesId)`. Not hashed.
- **Goldens:** `world/tests/goldens.rs` gains `GOLDEN_SCREEN` (start menu open and closed) and `GOLDEN_SAVE` (a save dialog run); the other `GOLDEN_*` move once (the Pokédex digest).

### Content contracts (`crates/content`)
- **Prelude** (default bodies issue the Wave 0 request; owners fill Gen3 behaviour; Task J lowers to these names only):
  ```rust
  // prelude/mart.rs (J)
  pub async fn mart(ctx: &Ctx, items: &'static [ItemId]);            // pokemart + its Gen3 std texts
  // prelude/party.rs (J)
  pub async fn choose_party_mon(ctx: &Ctx, mode: PartyMode) -> u8;   // writes VAR_0x8004 as Gen3
  pub async fn save_game(ctx: &Ctx) -> bool;                          // special SaveGame -> VAR_RESULT
  pub async fn player_pc(ctx: &Ctx, mode: PcMode);
  ```
  Silice gets the SIL2 helpers in `src/silice/prelude.rs` (Task R): `screen_flash`, `screen_shake` (now real), `fade_white`, `set_tint`, `money`, `take_money_percent`, `wait_input_or_timeout`, `add_friendship`, `give_mon_with`, `battle_trainer_no_whiteout`, `play_evolution`, `mart`, `ask_no` (yes/no defaulting to No), `coins`.
- **Generated tables** (stubs in Wave 0, filled by Task C): `content::data::{START_MENU, ITEM_USE, ITEM_DESCRIPTIONS, MOVE_DESCRIPTIONS, TM_MOVES, TM_COMPAT, UI_GFX, WINDOW_FRAMES, ITEM_ICONS, MON_ICONS, IN_GAME_TRADES, NATURE_NAMES}`, answered by `game_data_tables!` for both roots. `src/silice/mod.rs` answers `start_menu` (Task C writes Silice's table from `game.ron`).
- **New build files:** `build/ui.rs` (C: blobs for UI pictures and icons), `build/classify/items.rs` (C: `ItemUse` from item names and Gen3 effect data), `build/classify/evolution.rs` (E: the exact `EvoMethod` from `EVO_*`; the fold moves here from `build/battle.rs`).
- **`games/silice/game.ron`** gains `start_menu: [..]` (Task C), schema in `extract::games::GameFile`.

### Extract contracts (`crates/extract`)
- `schema.rs`: `FORMAT_MINOR` stays 5 in Wave 0; X1 and X2 each bump it when they merge (the content build then asks for a re-extract and lists the missing files, M8's `MINOR_FILES`). New file types declared (serde, no logic):
  ```text
  ui/<Name>.{png,ron}          UiPictureFile   (party, bag, summary, shop, options, evolution, icons sheets)
  ui/frames/<n>.{png,ron}      the 20 user window frames (gUserFrames)
  icons/items/<ITEM>.{png,ron} ItemIconFile    (gItemIconTable: tiles + palette)
  icons/mons/<SPECIES>.png     + icons/mons/palettes.ron  (gMonIconTable, gMonIconPaletteIndices)
  tables/item_text.ron         ItemTextFile    (ITEM_* -> description)
  tables/move_text.ron         MoveTextFile    (MOVE_* -> description)
  tables/ui_text.ron           UiTextFile      (gText_* / gMenuText_* labels the screens print, by decomp name)
  tables/item_effects.ron      ItemEffectsFile (gItemEffectTable bytes, field/battle use function names, hold effect)
  tables/tmhm.ron              TmHmFile        (TM/HM item -> move; sTMHMLearnsets bitsets)
  tables/natures.ron           NaturesFile
  tables/ingame_trades.ron     InGameTradesFile (sIngameTrades)
  ```
- **The IR** gains `IrArg::Items(String)` (a named item list) and `ScriptFile.lists: Vec<IrList { name, origin, items: Vec<String /* ITEM_* */> }>` for `pokemart` (and the two decoration marts, decoded but left `Unsupported`). Names come from the decomp (`<Map>_Pokemart`…) by the label transfer, positional fallback `<Script>_Items<n>`.
- Stub modules with their `pub mod` lines in `lib.rs`: `ui_gfx.rs`, `export/ui.rs` (X1), `ui_text.rs` (X2). `export` calls `export::ui::export_ui` (empty body).

### Shells, xtask, explorer contracts
- **game-native:** `Session` exposes `take_save() -> Option<Checkpoint>` (after `Event::Saved { ok: true }`); the window build maps START/SELECT if it does not already. Store: a `saves/` dir per flavour next to the checkpoint store (git-ignored).
- **game-gba:** `save.rs` keeps the slot API; `save_logic.rs` gains `BootChoice::MainMenu` (Task S fills).
- **emu:** `DebugSnapshot` gains `late_saving` and `screens` (v3 padding, region still `PKRDBG03` if it fits, else `PKRDBG04` with the shell and `emu` in one commit).
- **xtask:** `mod vanilla;` with `Some("vanilla") => vanilla::main(..)` (stub; Task V).
- **Mod lines and build calls:** Wave 0 adds every new `mod`/`pub mod` line (`engine/src/lib.rs`, `extract/src/lib.rs`, `extract/src/export.rs` for `ui`, `content/src/prelude/mod.rs` for `mart` and `party`, `content/build/classify/mod.rs` for `items` and `evolution`, `world/tests.rs` for every `m9_<task>` test module with an empty file each) and the `content/build/main.rs` call to `build/ui.rs`, so no Wave 1 task edits those files.
- **explore:** no Wave 0 change (Task D owns the crate in Wave 1, Task N in Wave 3).

### The state change
Update the goldens and the README, run `cargo xtask checkpoints verify --accept` on the three trees (`chore(checkpoints): M9 Wave 0 state`), and `cargo xtask parity`: hashes must match between ROM and native, lines within the gates. Expected: frames unchanged everywhere; hashes move from the first battle start of each route (the Pokédex digest).

---

## Rules for every task

M8's rules apply unchanged ("Rules for every task" in `2026-10-22-milestone-8-scale-import.md`): own worktree, `feat(<crate>): …` commits with **no attribution lines**, TDD, synthetic test games, test budgets, determinism, game-agnostic engine, degradation not panics, no ROM-derived data committed, no FR literals in tests, the commit gates. Additions for M9:
- **Three flavours.** Every task runs the stub, Emerald and `PKR_GAME=silice cargo test -p content -p engine -p extract` gates. A screen must work with a game that answers `None` for every picture (it draws frames and text only) and must never panic on a missing table.
- **Test files:** `<crate>/tests/m9_<task>_*.rs` and `engine/src/world/tests/m9_<task>.rs`.
- **Screens and frame budget.**
  - A screen's frame never exceeds `loop_lines_max` 217 or uploads more than the kit's per-frame budget; full redraws are spread (the M3 text rule, M8 H's `GLYPHS_PER_FRAME`).
  - Every screen has a GBA probe in Task Q's screen tour; owners measure their open, scroll and close frames on a scratch build and report them.
  - `heap_max` stays under 160 KiB on every lane; a screen allocates once on open.
- **Hash and checkpoint discipline** (as M8):
  - **Route-neutral tasks** (K, X1, X2, C, S, R, J, T, D, and the Wave 2 screens, each unless Step 0's scan says otherwise) verify `valid` or `valid (rehash)` on the three trees **without `--accept`**; their `BUILD_ID` refreshes are batched into one `chore(checkpoints)` per wave.
  - **Route-changing tasks** merge one at a time: **F → E → BM**, then any task whose check moved. They never commit tree or golden refreshes; each report has a **route delta**; master re-accepts after each merge in a reviewed `chore(checkpoints): <task> <cause>` commit. A changed `act1:*` node is re-accepted with the cause, or replaced by the spine's node of the same label (`explore spine --export`, then `checkpoints promote`).
  - Every engine or content merge moves Emerald's `BUILD_ID`; Silice-only files (`src/silice/`, `build/silice/`, `games/silice/`, `extract/src/games/`) move only Silice's.
- **Gen3 is the spec** for Emerald behaviour (`start_menu.c`, `party_menu.c`, `pokemon_summary_screen.c`, `item_menu.c`, `item_use.c`, `shop.c`, `player_pc.c`, `pokemon_storage_system.c` logic, `option_menu.c`, `main_menu.c`, `evolution_scene.c`, `pokemon.c`, `battle_*`). Window geometry and timings are tested against decomp constants read through `PKR_DECOMP` (as the names drift tests); each deviation is listed in the task's report and in the module doc.
- **Ownership:** only touch your task's files plus your test files. Describe any other change you need in your report; if a signature is missing from Wave 0, add it in your own file and list it.

---

## Wave 1 (parallel, 11 agents)

### Task K — engine: UI kit and screen hosting
**Owns:** `crates/engine/src/ui/kit/**`, `crates/engine/src/world/screens.rs`, `crates/engine/src/world/host/screens.rs`, `crates/engine/src/{text.rs, text/printer.rs, menu.rs}` (after Wave 0: re-hosting the text box and multichoice on a screen layer), `crates/engine/src/world/tests/m9_k.rs`, `crates/emu/tests/m9_k_*.rs`. Route-neutral (nothing opens a screen until Wave 2, and START opens a start-menu stub that closes the same frame; Step 0's scan confirms no START on the routes).
1. **List menu** (Gen3 `ListMenu`, `list_menu.c`): items, cursor, page scrolling with `maxShowed`, scroll arrows (`AddScrollIndicatorArrowPair`), the "cancel" item, A/B/LR paging per Gen3 `LIST_MULTIPLE_SCROLL_L_R`, and a cursor-moved callback for descriptions. Drawn incrementally (at most `GLYPHS_PER_FRAME` glyphs a frame).
2. **Windows:** `DrawStdWindowFrame` with the 20 user frames (`options.frame`, `GameData::window_frame`; the dialogue frame when absent), `ClearStdWindowAndFrame`, the standard menu palette.
3. **Money box** (`DrawMoneyBox`, `PrintMoneyAmountInMoneyBox`), **coin box**, **quantity selector** (`PrintItemQuantity`: up/down by 1, left/right by 10, wrap, the max from bag space or money), **yes/no in screens**, **message in screen** (the field `TextBox` re-hosted on the screen layer).
4. **Icons:** item icons (Gen3 `AddItemIconSprite`: 24×24 in a 32×32 OBJ) and mon icons (two-frame animation, `gMonIconPaletteIndices`), loaded spread under the budget; placeholders when `None`.
5. **Hosting** (`world/screens.rs`, `host/screens.rs`): open a screen from START or a request, push and pop, fade the field out (Gen3 `BeginNormalPaletteFade`, 16 frames), hand the VRAM over, run, fade back, restore the field (`Field::restore_spread`, objects, text box), and answer the waiting request with the screen's result. `ScreenOut::RunScript` closes every screen first. Requests issued from a map script (immediate) are answered with the default and logged, as M8 does for waiting requests there.
6. **Tests:** list menu input → index (paging, wrap, cancel), the frame and box geometry against the decomp (`sMoneyBoxWindowTemplate`, list menu templates), quantity limits, a pushed-and-popped stub screen restores the field byte-for-byte (frame equality with a direct `upload_all`), no frame over the upload budget.
- **Done when:** a stub screen opened from START and from a request returns to an identical field on native and ROM (a `m9_k` lane in `emu/tests`, run by Q later), and every widget renders with all `GameData` UI answers `None`.

### Task X1 — extract: UI graphics, icons and window frames
**Owns:** `crates/extract/src/{ui_gfx.rs, export/ui.rs}`, `crates/extract/tests/m9_x1_*.rs`. Route-neutral (extract only; bumps `FORMAT_MINOR`).
1. Locate and export, by anchors and pointer tracing, verified on BPEE: the party menu (bg, slot tiles, palettes, HP bar), bag (bg, the bag sprite per pocket, palettes), summary (page tilemaps, type and status icons, palettes), shop (`gShopMenu_Gfx`, tilemap), options (bg, frame previews), evolution scene backgrounds, main menu windows, the scroll arrows and menu cursor, the 20 user window frames (`gUserFrames`), item icons (`gItemIconTable`), mon icons (`gMonIconTable`, `gMonIconPaletteIndices`, the 3 palettes).
2. Decompress LZ77 at extract time; write PNG + RON per `schema.rs`.
3. **Tests:** every file's count equals the decomp's table length; BPEF and BPEE images equal where the decomp says the graphics are language-independent; two exports byte-identical.
- **Done when:** `cargo xtask extract` writes every `ui/`, `ui/frames/` and `icons/` file on BPEF and BPEE, and the extracted size grows by less than 2 MB without previews.

### Task X2 — extract: UI texts, item and move data, mart lists
**Owns:** `crates/extract/src/{ui_text.rs, tables.rs, export/tables.rs}`, `crates/extract/src/export/scripts.rs` (mart lists only), `crates/extract/src/labels.rs` (list labels only), `crates/extract/tests/m9_x2_*.rs`. Route-neutral (bumps `FORMAT_MINOR`).
1. **Texts:** item and move descriptions; the `gText_*`/`gMenuText_*` strings every screen prints (the list is the union of the decomp strings referenced by the Gen3 sources above, committed as decomp names in `ui_text.rs`); nature names. Every string re-encodes to its ROM bytes.
2. **Data:** `gItemEffectTable` and each item's field and battle use function (by the decomp name of the pointer target); TM/HM item → move and `sTMHMLearnsets`; `sIngameTrades`.
3. **Mart lists in the IR:** `pokemart`'s operand pointer is followed to its `ITEM_NONE`-terminated halfword list; the IR records `IrArg::Items(name)` and the list in `ScriptFile.lists`, named by the decomp label. `pokemartdecoration`/`2` lists too.
4. **Tests:** counts against the decomp; the Oldale and Rustboro mart lists equal the decomp's `pokemartlistend` lists on BPEE; TM compatibility of five sampled species equals the decomp; the M8 IR instruction count is unchanged.
- **Done when:** every new `tables/*` file exports on both ROMs with 100% re-encoding, and every `pokemart` site in the IR names a list.

### Task C — content: UI data, item uses, start menus, field-move gates
**Owns:** `crates/content/build/{ui.rs, classify/items.rs}`, `crates/content/build/codegen.rs` and `blob_data.rs` (the new tables only), `crates/content/src/lib.rs` (`game_data_tables!` arms and `gen3_special_ids()` only), `crates/content/src/silice/mod.rs` (`start_menu`, `field_move_gate`), `games/silice/game.ron` (`start_menu`), `crates/extract/src/games/game.rs` (the `start_menu` field), `crates/content/tests/m9_c_*.rs`. Route-neutral; moves `BUILD_ID` (both flavours).
1. Wire X1 and X2's files into blobs and the Wave 0 tables (`UI_GFX`, icons, frames, texts, TM data, trades, natures). Without them (older extract) the build fails with M8's minor-version message.
2. **`classify::items`:** every `ITEM_*` to an `ItemUse` from its effect bytes and use functions (Potion family, status heals, Revive, Ether/Elixir, Rare Candy, vitamins, stones, balls with Gen3 multipliers, X items, Guard Spec, Repels, Escape Rope, Poké Doll/Fluffy Tail, TMs/HMs, berries, key items); anything else `Other(name)`. A test covers every pocket.
3. **Emerald start menu** (Gen3 `BuildNormalStartMenu`): Pokédex (`FLAG_SYS_POKEDEX_GET`), Pokémon (`FLAG_SYS_POKEMON_GET`), Bag, PokéNav (`FLAG_SYS_POKENAV_GET`, `Script` of a content stub that reports `Unsupported("pokenav")`), Player, Save, Option, Exit, with Gen3's descriptions where the ROM has them.
4. **Field-move gates:** Emerald's from `sHMMoves` and the badge checks (`FLAG_BADGE0n_GET` per HM, Gen3 `CheckIfPartyHasMove…`/`party_menu.c` field moves); Silice's from `game.ron` (module flags, `FLAG_SIL_MODULE_*`).
5. **`SpecialIds`:** the Wave 0 fields for Emerald (`pc_boxes` from the spike, 50 PC item slots, 9,999 coins, the eight badge flags, `VAR_REPEL_STEP_COUNT`, `VAR_FRIENDSHIP_STEP_COUNTER`, `EventScript_RepelWoreOff`, `FLAG_SYS_POKEDEX_GET`).
6. **Silice:** `game.ron`'s `start_menu` (the Mallette replaces the PokéNav slot as `Script(..)`; the rest follow Gen3), checked by the Silice build; `game.ron`'s start facing honoured (SIL1 E found `game_data_tables!` hardcodes Down).
- **Done when:** every `GameData` UI method answers on the Emerald build, both start menus build, and the item-use classification has no unexpected `Other` (the report lists the rest by name).

### Task S — save, load and the main menu
**Owns:** `crates/engine/src/{world/save.rs, ui/save.rs, ui/main_menu.rs, world/host/save.rs}`, `crates/game-gba/src/{save.rs, save_logic.rs, main.rs}`, `crates/game-native/src/{flash.rs, lib.rs}`, `crates/emu/tests/save_logic.rs`, `crates/engine/src/world/tests/m9_s.rs`, `crates/emu/tests/m9_s_*.rs`. Route-neutral unless Step 0 finds `SaveGame` on a route.
1. **`world/save.rs::save_now`** (Gen3 `SaveGame` → `TrySavingData`), also exposed as `World::save_snapshot()`. It normalises a clone of the state and captures it with the existing capture code (`world/checkpoint.rs`, which S calls but does not edit): the state as Gen3 would restore it on CONTINUE. Allowed in the field or inside a script (`special SaveGame`, the start menu). It drops the runner, closes the text box, clears fades and screen effects, unfreezes objects, ends movement scripts with each object on its `currentCoords` cell (Gen3 saves the destination cell of a step in progress), and keeps every flag and var (temp vars too: Gen3 saves `VAR_TEMP_*` and clears them on the next map load). A restore of it is capturable at once (tested from mid-script saves).
2. **Save dialog** (`ui/save.rs`, Gen3 `start_menu.c` `SaveGame` tasks): the info window (`sSaveInfoWindowTemplate`: location, name, badges, Pokédex, time), "Would you like to save?", the overwrite question when a save exists, "SAVING… DON'T TURN OFF THE POWER", the frame counts of Gen3's save callbacks (`SaveStartTimer`, `SaveSuccesTimer`), then "{PLAYER} saved the game." Raises `Event::Saved { ok }`; `SaveDialog` answers `Bool(ok)`.
3. **Shells:** the GBA shell writes the slot on `Event::Saved` (frames stall; the debug region counts `late_saving`, not `late`); native's `Session::take_save` gives the checkpoint, the window build writes `saves/<flavor>/game.sav` with `flash::encode_sav` (byte-identical to the ROM's). A failed write answers `ok = false` on the next frame through `World::save_failed()` (Gen3's "Save error" path, `SaveErrorTimer`).
4. **Main menu** (`ui/main_menu.rs`, Gen3 `main_menu.c`): CONTINUE (with the info window from `SaveMeta`), NEW GAME, OPTION; the GBA and native window shells boot into it when the slot is valid (`BootChoice::MainMenu`); B held at boot still skips the save (M5). CONTINUE → `World::restore`; NEW GAME → `World::new` (the slot stays until the first save overwrites it, as Gen3).
5. **Capacity gate:** a test builds Step 0's worst-case save and asserts `encoded_len ≤ MAX_ENCODED` and a real agb write on 64 KiB succeeds.
6. **Tests:** normalisation (saved mid-movement, mid-message, with a locked NPC), the dialog's frame counts against the decomp, a restored save equals a capture after Gen3's continue path, native image = ROM image after an in-game save (`m9_s` lane: ROM saves through the start menu at a fixed frame, native does the same, `.sav` bytes equal; the ROM reboots into CONTINUE and the hashes match native from there).
- **Done when:** saving through the start menu (once SM merges; through `special SaveGame` before) and continuing gives the same World on native and ROM, the worst case fits, and the debug combo still works.

### Task R — SIL2 requests (SIL2's Wave 0, built)
**Owns:** `crates/engine/src/{fx.rs, flow/fade.rs, world/host/fx.rs, world/host/party.rs}`, `crates/content/src/silice/prelude.rs`, `crates/engine/src/world/tests/m9_r.rs`, `crates/content/tests/m9_r_*.rs`. Emerald route-neutral (no Emerald lowering uses the new requests; the changed shapes keep their bytes); moves Silice's `BUILD_ID`.
1. **Fades to and from white** (`Frame.lighten`; Gen3 `FADE_TO_WHITE` timings, `fadescreen` speeds). A map load under a white frame is a visible frame for F's spread uploads (no change needed in F's files).
   Also: the **in-game trade swap** (`Request::InGameTrade`, Gen3 `CreateInGameTradePokemon`): the party slot becomes the `GameData::in_game_trade` mon (Task J calls it).
2. **`ScreenFlash`** (a full-screen colour for `frames`, then back: Gen3 `BeginNormalPaletteFade` to a colour and back), **`ScreenShake`** (Gen3 `ShakeCamera` field effect: BG and OBJ offsets alternating ±magnitude every frame, ending at 0), **`SetTint`** (Gen3 `BlendPalettes`-style blend of the given banks towards a colour, stepped over `frames`, at most N banks re-blended a frame within the line budget, re-applied after every field restore and map load until cleared with amount 0).
3. **`WaitInputOrTimeout`**, **`MoneyGet`** (`Response::Word`), **coins** (cap `max_coins`), **`PartyMonSpecies`**, **`AddFriendship`** (clamped 0..=255; Gen3's Soothe Bell and level modifiers do not apply to scripted changes).
4. Through Wave 0's hooks in R's own files (`host/party.rs::give_mon`, `host/fx.rs::whiteout_allowed`): **`GiveMon` extras:** moves (replacing the level-up set), held item, OT name and id, nickname; `BattleTrainer { no_whiteout }`: a loss answers `Lost`, heals nothing, and the field returns where the battle started, without `DoWhiteOut` (Silice's Samuel, R-7; documented as a game rule, Emerald never sets it).
5. **Silice prelude:** the SIL1 fallbacks (`screen_shake`) become real; new helpers listed in the content contracts. Silice Ch0 scenes keep working (SIL1 tests).
6. **Tests:** each request's frames and hash; tint survives a battle return and a map load; `YesNo { default_no: true }` starts on No and B answers No; Emerald goldens unchanged (no Emerald script issues the new requests).
- **Done when:** every SIL2 Wave 0 contract in §3.2 of the Silice plan answers with its behaviour on the three flavours, and Emerald's parity lanes are identical.

### Task E — evolution scene, exact methods, friendship
**Owns:** `crates/engine/src/{ui/evolution.rs, mon/evolution.rs, world/evolve.rs, world/friendship.rs}`, `crates/content/build/classify/evolution.rs`, `crates/engine/src/world/tests/m9_e.rs`, `crates/content/tests/m9_e_*.rs`. **Route-changing** (walking friendship draws the RNG every 128 steps; Wurmple-line and other evolutions get a scene).
1. **Exact methods** (Gen3 `GetEvolutionTargetSpecies`): every `EVO_*` with its condition (Silcoon/Cascoon by personality, Tyrogue by stats, Ninjask + Shedinja into an empty slot with a Poké Ball, friendship ≥ 220 with day/night: the RTC is M10, so day; Beauty never; item; trade and trade-with-item through `ItemUse::Evolution` of Silice's PIERRE LIEN, an item use that triggers trade evolutions).
2. **Evolution scene** (`evolution_scene.c`, simplified): "What? {NAME} is evolving!", the pic alternating between both species (Gen3 sprite flashing, no sparkles), B cancels when allowed, "Congratulations!…", then the new species' level-up move at this level (`MonTryLearningNewMove` with the move-replace flow on the M4 learn-move UI). Reached after battles (`world/evolve.rs`, replacing `Degraded("evolution scene")`), from Rare Candy and stones (Task B calls `Evolve`), and from scripts (`Request::Evolve`, Silice `play_evolution`).
3. **Friendship** (Gen3 `AdjustFriendship`, in `world/friendship.rs` through Wave 0's `on_event` call sites): walking (every 128 steps, `VAR_FRIENDSHIP_STEP_COUNTER`, a 50% `Random()` per call), level up, vitamins and battle items (Task B and BM call `adjust_friendship(event)`), fainting (small/large), Soothe Bell, the met-location and ball bonuses as Gen3.
4. **Tests:** each `EVO_*` against Gen3 cases; the scene's frame counts against the decomp's task states; the walking draw on step 128; a cancelled evolution keeps the species.
5. **Route delta:** the friendship draw shifts every later RNG roll on routes over 128 steps (all Emerald nodes after Route 101 diverge); evolutions on the Act 1 route (Step 0's scan) gain the scene's frames.
- **Done when:** no `Degraded("evolution scene")` or `("friendship evolution")` remains, and the route delta matches Step 0's scan.

### Task F — field: the carried M8 findings
**Owns:** `crates/engine/src/{field.rs, field/*, flow/transition.rs, world/maps.rs, world/checkpoint.rs}`, `crates/content/build/classify/field.rs`, `crates/engine/src/world/tests/m9_f.rs`. **Route-changing** (redraw commands and timing).
1. **Dirty-cell `DrawWholeMapView`** (M8 Task M review issue 5): `redraw_map` redraws only the cells `setmetatile` changed since the last draw, spread under the per-frame budget; a full redraw only when the camera or tilesets changed. Sidney's room leaves `KNOWN_OVER`; Phoebe's, Glacia's, Drake's and the Trick House end come back within budget (the tour's stretch list). Silice's Ch2 invisible-maze redraws and the Mauville gym switches depend on it.
2. **Scripted ladder facing:** a warp onto a ladder keeps the facing Gen3 stores (`gObjectEvents[player].facingDirection` at the warp), not the current one.
3. **`MB_PETALBURG_GYM_DOOR` arrivals:** `Task_ExitDoor` (face south, hidden, walk south), classified in content.
4. **Briney-boat `RoundTrip`** (M8 N, Route 104 (71, 189), "camera far outside the map"): either capture refuses that state or restore accepts it; Gen3 has no such limit, so restore accepts a camera following a scripted player off the map (tested on a synthetic map).
5. **`UpdateEscapeWarp`** (M8 F/L deferral): `MapData` gains the map type (content classifies `MAP_TYPE_*`), and entering an indoor map from an outdoor one stores the escape warp (Gen3 `UpdateEscapeWarp`), which Task B's Escape Rope uses.
6. **Route delta:** item 1 changes the draw commands and frames of every `DrawWholeMapView`/`redraw_map` (none on the Act 1 route per M8 M; confirm with Step 0's scan); items 2–5 change no Act 1 frame unless the scan finds them.
- **Done when:** the tour has no gated or stretch overrun caused by `DrawWholeMapView`, and items 2–5 have tests.

### Task J — content: specials, std scripts and `pokemart` lowering
**Owns:** `crates/content/build/support/{specials.ron, commands.ron}`, `crates/content/src/prelude/{special.rs, std.rs, mart.rs, party.rs}`, `crates/extract/src/codegen/lower/{emit.rs, operands.rs}`, `crates/content/tests/m9_j_*.rs`, `crates/extract/tests/m9_j_*.rs`. Route: neutral unless Step 0 finds `SaveGame`, `ChoosePartyMon` or FADE_TO_WHITE on a route; then route-changing (merges after BM).
1. **Lowering:** `pokemart` → `mart(&ctx, &<LIST>).await` with the list as a generated `static [ItemId]`; `fadescreen FADE_TO_WHITE`/`FADE_FROM_WHITE` → white fades (R's); `yesnobox` unchanged (`default_no: false`).
2. **Reclassify** with reasons in `specials.ron`: `SaveGame` (Native → `save_game`), `PlayerPC`, `BedroomPC`, `ShowPokemonStorageSystemPC`, `ScriptMenu_CreatePCMultichoice` (Gen3's flag-built list), `ChoosePartyMon`, `ChooseMonForMoveTutor`, `ScriptGetPartyMonSpecies`, `CountPartyAliveNonEggMons` (+ `_IgnoreVar0x8004Slot`), `ScriptCheckFreePokemonStorageSpace`, `BufferMonNickname`, `IsStarterInParty`, `GetLeadMonFriendshipScore`, `CreateInGameTradePokemon` and `GetInGameTradeSpeciesInfo` (from `in_game_trade`), `DoInGameTradeScene` (Degraded: no trade animation, the swap happens), `ScriptMenu_CreateStartMenuForPokenavTutorial` (the real start menu's labels).
3. **Link safety:** with `SaveGame` answering "saved", the cable club and Union Room scripts go past their save. Every link special they then reach gets a `Default` that takes Gen3's no-link path (for example `TryBattleLinkup` → the "link error" result, `IsWirelessAdapterConnected` 0), with a reachability test over the Pokémon Center 2F scripts: no `Unsupported` link special is reachable.
4. **`callstd`/msgbox:** the item-message texts that need TM move names (`BufferTMHMMoveName`, via `tm_move`) and plural names where Gen3 has them (M8 H's reported differences).
5. **Tests:** each new `Native` special's requests on a synthetic `Ctx`; every `pokemart` site lowers; the newly reachable specials table (M8 J's test, extended) has no `Unsupported` without a reason.
- **Done when:** the build report lists no `pokemart` and no reachable menu special as `Unsupported`.

### Task T — studio: the views deferred from M8
**Owns:** `crates/studio/**` except `crates/studio/src/map_edit/**`, `crates/studio/tests/m9_t_*.rs`. Route-neutral.
1. **Items sub-tab** (name, pocket, price, use class, description, icon), **water, rock smash and fishing encounters** next to land, **trainer lists** per map (with sight ranges), overlays for **dynamic warps** and **sight ranges** on the map view (M8 S deferrals).
2. **Save files:** open a `.sav` (ours) in the native pane as a checkpoint (read-only), and show its `SaveMeta`.
3. **Gates:** M6/M8 studio gates; `cargo xtask studio --smoke` on the full import.
- **Done when:** the four deferred views render on the fixed map sample and their tests pass with the UI feature.

### Task D — explorer: spine and branches, schema v4, policy hooks
**Owns:** `crates/explore/**`. Route-neutral (tools only).

**Redesigned (user decision, 2026-10-23; `2026-10-23-explorer-spine.md`).** The evolutionary explorer is gone. Testing follows one authored main path per flavour (the **spine**: committed `spine::Step` lists, Emerald to badge 1, Silice's Chapter 0) and **branches** off it toward what the spine never touched, each recovering to the spine and verifying a short window (after its target, its frame budget or its step cap), or aborted when its novelty rate falls too low.
1. **`explore sweep` is folded into `explore branch`**, which covers every map the spine reaches (and every target off it). `cargo xtask sweep` stays the every-map `StartAt` sweep.
2. **Schema v4** is the spine schema: `spine`, `position`, `branch`, `seen`, `coverage`, `finding`, `run`. An older db is refused with "run `explore reset --yes`". `Macro::code` is gone with the node tree (macros are stored as RON text).
3. **Policy hooks** for Task N: still to add (`Policy::choose_switch`, `Policy::choose_item`), with today's behaviour. The spine's `Battle(Best)` already picks the move of the best expected damage.
- **Done when:** `spine_reaches_badge1` and `spine_reaches_ch0_end` pass with their frame counts (Emerald 109,851 since the review's `WaitScript`s at Petalburg; Silice 21,821), branch runs are identical across `--jobs`, and the studio's Explorer tab shows the spine and the branches. **Done** by the spine rework.

---

## Wave 2 (parallel, after K, X1, X2 and C merge; 5 agents)

### Task SM — start menu and options
**Owns:** `crates/engine/src/ui/{start_menu.rs, options.rs}`, `crates/engine/src/flow/input.rs` (the L=A button mode), `crates/engine/src/world/tests/m9_sm.rs`. Route-neutral (no START on the routes; default options = today).
1. **Start menu** (`start_menu.c`): the entries from `GameData::start_menu` filtered by `shown_if`, the window at Gen3's position and width, the cursor kept between openings (`sStartMenuCursorPos`), A opens, B and START close; Pokédex and Player open nothing yet and report `Degraded("pokedex")`/`("trainer card")`; `Script` closes the menu and runs the script with `lockall`; Save pushes Task S's dialog; Party, Bag and Options push their screens. While open the field objects freeze (Gen3 `FreezeObjectEvents`).
2. **Options** (`option_menu.c`): six rows with Gen3's values and defaults, left/right changes, A on CANCEL or B saves, START also; effects: text speed (Wave 0 plumbing), battle scene (read by the battle scene: no animations yet, so it changes nothing visible), battle style (read by BM's shift prompt), sound (stored), button mode (L=A in the field and menus; LR is Gen3's help shortcut, stored only), frame (the kit's window frame).
3. **Tests:** entries per flag set; cursor memory; each option's stored value and effect; Gen3 geometry from the decomp templates.
- **Done when:** every start-menu action opens its target (or reports) and returns to the same field, and changing the frame or text speed shows in the next message.

### Task PT — party menu and summary
**Owns:** `crates/engine/src/ui/{party.rs, summary.rs}`, `crates/engine/src/world/tests/m9_pt.rs`. Route-neutral.
1. **Party menu** (`party_menu.c`): the six slots with icon, name, level, gender, HP bar and status, the CANCEL button, the action list per mode:
   - `Field`: SUMMARY, SWITCH (reorders the party, Gen3's slide), ITEM (GIVE, TAKE), the field moves the `field_move_gate`s allow (using one reports `Unsupported("field move")` until M10), CANCEL;
   - `Choose`, `MoveTutor` (Gen3's ABLE/NOT ABLE, `CanMonLearnTMHM`/tutor), `UseItem`, `GiveItem`: one slot answered;
   - `Battle { forced }`: SHIFT and SUMMARY; forced (after a faint) has no CANCEL; refuses fainted, active and egg slots with Gen3's messages.
2. **Summary** (`pokemon_summary_screen.c`, pages INFO, SKILLS, MOVES; no contest page): species, nature, OT, item, stats, exp, abilities with descriptions if present, moves with PP, type, power, accuracy and description; up/down switches mon, left/right pages; the move-replace mode used by E's learn flow.
3. **Tests:** reorder keeps everything else; each mode's refusals; summary values equal `Pokemon` fields; decomp geometry.
- **Done when:** the party menu opens from the start menu, from `ChoosePartyMon` and in BM's battle mode, and the summary from all three.

### Task B — bag and item use
**Owns:** `crates/engine/src/{ui/bag.rs, item_use.rs (new), wild.rs (repel only)}`, `crates/engine/src/world/tests/m9_b.rs`. Route-neutral (the routes use no item; Step 0 confirms).
1. **Bag** (`item_menu.c`): five pockets with Gen3's order and switching, the bag sprite per pocket, the item list (kit list menu) with icon and description, the per-pocket cursor and scroll memory, the actions per mode: `Field` (USE, GIVE, TOSS with quantity, REGISTER for key items, CANCEL), `Sell` (Task M: quantity, the price at half, "Is that okay?"), `Battle` (USE only, Gen3's battle-usable classes), `ChooseBerry`.
2. **Item use out of battle** (`item_use.c`, `ItemUseCB_*`): medicine on a party slot (heal, status, revive, PP, Rare Candy with level-up, move learning and E's evolution), vitamins (EVs, Gen3 caps, friendship), stones (E's evolution), Repel (`var_repel_steps`, refused while active; `wild::repel_blocks` against the lead's level; `script_repel_wore_off` when it runs out, Gen3 `UpdateRepelCounter`), Escape Rope (F's escape warp, refused where Gen3 refuses), TMs/HMs (teach through the party menu's tutor mode; HMs not consumed), key items registered on SELECT (`registered_item`; the Bike is M10: `Unsupported("bike")`).
3. **Tests:** each use class's state change and message against Gen3; toss and give quantities; repel blocking and wear-off; the sell price.
- **Done when:** every `ItemUse` class except `Other` works from the field bag, and the bag opens in all four modes.

### Task BM — battle: BAG and POKéMON, items, balls, the shift prompt
**Owns:** `crates/engine/src/battle/{core.rs, core/items.rs, scene.rs, scene/menu.rs, ui.rs, effects/ball.rs, effects/items.rs, calc.rs}`, `crates/engine/src/world/battle.rs`, `crates/engine/src/world/tests/m9_bm.rs`. **Route-changing** (the shift prompt; `Need::Switch` no longer automatic).
1. **Screens in battle:** BAG opens the bag in battle mode, POKéMON the party in battle mode; the battle scene hands its VRAM over and reloads after (its `load` spread), with the healthboxes and pics as before. `Need::Switch` (after a faint) opens the forced party menu.
2. **Battle items** (`Choice::Item { item, target }`, Gen3 `HandleAction_UseItem`, `ExecuteTableBasedItemEffect`): medicine on the active or a party mon with Gen3's messages, X items and Guard Spec, Poké Doll/Fluffy Tail (escape in wild battles), balls with their Gen3 multipliers (Net, Dive, Nest, Repeat, Timer, Luxury, Premier, Master; Safari is M11), the turn order of item use, the AI unchanged.
3. **Shift prompt** (Gen3 `BattleScript_FaintedMonTryChoose` with `OPTIONS_BATTLE_STYLE_SHIFT`): "{TRAINER} is about to use X. Will {PLAYER} change POKéMON?" when the player has another usable mon; SET skips it.
4. **Caught mon overflow:** handled by Task M's `world/caught.rs`; BM only prints Gen3's "sent to BOX n" strings from M's result.
5. **Pickup** (M8 G): a Pickup mon without an item gets Gen3's `Cmd_pickup` item (held items exist since the Silice engine work).
6. **Tests:** each item effect against Gen3 (explicit RNG rolls for balls); the shift prompt in both styles; a forced switch; the scene reload is byte-identical to a fresh load.
7. **Route delta:** trainer battles where the player has two or more usable mons gain the shift prompt (Step 0's scan); the chore commit re-accepts the broken nodes with the cause, and N's policy and Q re-find them in Wave 3.
- **Done when:** a wild battle can be won by switching and healing with player choices on native and ROM, and catching uses the chosen ball.

### Task M — mart and PC
**Owns:** `crates/engine/src/{ui/mart.rs, ui/pc.rs, world/caught.rs, mon/boxed.rs, pc.rs (new: storage logic)}`, `crates/engine/src/world/tests/m9_m.rs`. Route-neutral unless Step 0 finds a clerk talked to on a route.
1. **Mart** (`shop.c`): BUY/SELL/QUIT, the buy list with prices, icon and description, the money box, quantity (bounded by money and bag space), "Is that okay?", the Premier Ball bonus for 10+ Poké Balls, "You can't carry any more", "You don't have enough money"; SELL opens the bag in sell mode. Gen3's shop view scrolls the field map behind the windows; M9 shows the windows over the frozen field (documented).
2. **PC** (`player_pc.c`): the Gen3 PC menu built from flags (Someone's/Lanette's PC, {PLAYER}'s PC, Hall of Fame only after the game is beaten, LOG OFF); item storage (WITHDRAW, DEPOSIT, TOSS, 50 slots); Pokémon storage (WITHDRAW, DEPOSIT, MOVE between boxes, RELEASE with Gen3's last-usable-mon checks) as a list per box, `Degraded("pc storage ui")`. Bedroom PC: item storage; DECORATION and MAILBOX report `Unsupported`.
3. **Storage logic** (`pc.rs`): Gen3 `SendMonToPC` (current box first, then the next with room), `CountPartyMons`, the storage-full answer `MON_CANT_GIVE`; `givemon` and catches past six go to a box with Gen3's messages and `VAR_PC_BOX_TO_SEND_MON`-equivalent answer; withdraw recomputes stats from the boxed DTO.
4. **Tests:** buy/sell arithmetic and limits; storage order and overflow; boxed round trip (`to_boxed` → `from_boxed` equals Gen3 `BoxMonToMon`: status cleared and HP set to the recalculated max, since a `BoxPokemon` keeps neither, so a withdrawn mon comes back healed).
- **Done when:** Oldale's mart buys and sells on native and ROM, a seventh caught mon lands in BOX 1, and the Pokémon Center PC deposits and withdraws.

---

## Wave 3 — integration, route and parity

### Task Q — integration, Emerald route, parity lanes, screen tour
**Owns:** `crates/emu/tests/{parity.rs, tour.rs}`, `crates/emu/tests/m9_q_*.rs`, `crates/game-gba/src/{tour_logic.rs, main.rs}` (the `tour` feature's screen mode, after S merged), `xtask/src/{budget.rs, sweep.rs}`, the Emerald, stub and Silice checkpoint nodes (refreshes), and this doc's "Wave 3 as built".
1. **Route check and re-find:** the route-changing merges happened in Waves 1–2 (merge order); Q verifies the three trees on the merged tree and re-finds the `act1:*` nodes that BM's shift prompt broke, with Task N's policy (until then BM's chore commit re-accepts them with the cause, M8's fallback rule).
2. **Parity lanes** (ROM vs native, hashes and pictures every 100 frames, `late == 0`, `events_dropped == 0`, memory gates): start menu open/close; party reorder and summary; bag use of a Potion; Oldale mart buy and sell; PC deposit and withdraw; a wild battle with a switch, a Potion and a Great Ball; an evolution after battle; save through the start menu → reboot → CONTINUE (Task S's lane, folded in); a Silice lane once SIL1's nodes exist (Mallette entry runs its script).
3. **Screen tour** (`--features tour`, screen mode): from a fixed native-built save, open every screen and mode in turn with scripted inputs (lists scrolled to the end, every pocket, every summary page); gate loop ≤ 217 lines and `late == 0` on every frame; report the worst frame per screen.
4. **Budgets:** ROM, IWRAM, EWRAM re-baselined (UI graphics and icons are expected to add about 1 MB); `heap_max` per lane ≤ 160 KiB; `MAX_ENCODED` gate.
5. **Sweep:** `cargo xtask sweep` still passes on 518 maps (screens closed); `sweep --repro` byte-identical.
6. **Cleanup:** deduplicate the `StartAt` wrappers of sweep, parity and explore (M8 L follow-up 12) where Q's files allow; list the rest.
- **Done when:** every lane above passes, the tour has no overrun, and the three trees verify `Valid`.

### Task N — explorer: menus, switching and items
**Owns:** `crates/explore/**` (after D merged). Route-neutral: the spine replaces the re-found `act1:*` nodes (below).
1. **Policy:** battle switching (to the best type matchup when the active mon would faint or is resisted) and item use (Potion below a third of HP, the best ball once the wild mon is below half), the shift prompt answered by policy (M7 finding). They fill the Task D hooks; `Battle(plan)` gains `Items` and `Switch`.
2. **Spine steps and branch targets:** `Heal` (walk to a nurse, talk, resolve), `Buy { item, qty }` (enter a mart, buy), `UseItem { item, slot }` from the start menu, `Deposit` when the party is full; macros expanded by reading the screen state (no blind input sequences). As spine steps they replace the spine's `Train` detours where a Potion does; as branch targets, the menus become things the branches try off the spine.
3. **Goals:** Emerald Act 1 unchanged; `sil_ch1` goals and the Silice Chapter 1 spine from the content tests' `silice_spine::ch01` (the step list SIL2 wrote; `GotoAvoiding`, `Exit`, `Walk`, `Use` and `WaitScript` cover its vocabulary).
4. **Act 1 with menus:** the Emerald spine plays to badge 1 with the shift prompt on and the new policy, and grows past it as M9 lands; `explore branch` from every checkpoint has no bug and no unexpected divergence (or each is triaged). If Task Q prefers spine nodes to re-found `act1:*` nodes, `explore spine --export` writes `spine:act1:*` for promotion in Q's `chore(checkpoints)` commit.
- **Done when:** `spine_reaches_badge1` passes with the shift prompt on, and a full `explore branch` is triaged.

### Task V (stretch) — vanilla reference lanes
**Owns:** `xtask/src/vanilla.rs`, `crates/emu/tests/m9_v_*.rs`.
1. A host-only **Gen3 save reader** (sections, checksums, the mon substruct decryption) that turns the user's own vanilla BPEF `.sav` (path in `PKR_VANILLA_SAV`, git-ignored, never committed) into our checkpoint where both describe the same state (party, bag, money, flags, vars, position).
2. **Reference lanes** (skipped without the env vars): the same inputs on vanilla BPEF and on our ROM from the equivalent state, comparing outcomes read from RAM (money, bag slots, party order, VAR_RESULT) and frame counts at named points (start menu drawn, mart purchase done, save done, the sight frame M8 L carried to P if P did not close it).
- **Done when:** the mart, start-menu and save lanes run against a user save and their differences are listed as accepted deviations or bugs.

### Task W (stretch) — naming screen
**Owns:** `crates/engine/src/ui/naming.rs`, `crates/engine/src/world/tests/m9_w.rs`. `ChangePokemonNickname` and the catch "Give a nickname?" prompt open the Gen3 naming screen (`naming_screen.c`: the three keyboards, page swap, BACK, OK); route-changing only if a route nicknames (Step 0's scan).

### Close-out
- **Whole-milestone review:** determinism, write locations (nothing ROM-derived tracked), FR literals, panics reachable from content, engine game-agnosticism (no Emerald constant in `ui/`).
- **This doc:** "As built" per task, the carried-findings table updated, "Findings for Milestone 10".
- **Silice plan:** §1.1 and §3.2 statuses updated (what SIL2 can now call).
- **README:** the save files (where native writes them, how the ROM boots into CONTINUE), the screen tour, the vanilla lanes.
- **Cleanup:** worktrees.

---

## Merge order

1. **Wave 0** (state change, one `chore(checkpoints)`).
2. **Wave 1, route-neutral**, any order as they finish: K, X1, X2, C (after X1 and X2, as M8's E after A), S, R, J (if neutral per Step 0), T, D. One batched `chore(checkpoints): refresh build ids after Wave 1` on master.
3. **Wave 1, route-changing**, one at a time: **F, then E**, each followed by its chore commit.
4. **Wave 2** (after K, X1, X2, C): SM, PT, B, M as they finish (route-neutral, batched refresh); **BM last**, route-changing, with its chore commit (nodes it breaks are re-accepted with the cause, then re-found by N and Q in Wave 3).
5. **Any other task Step 0's scan flags** (J, S, M, W) becomes route-changing and merges after BM, one at a time, each with its chore commit.
6. **Wave 3:** Q integrates and refreshes; N's nodes promoted; V and W when ready.

Dependencies inside waves: S's start-menu lane waits for SM (S tests through `special SaveGame` first); B's Rare Candy and stones call E's `Evolve` (stub until E merges); B's Escape Rope uses F's escape warp (refused while unset); M's SELL uses B's sell mode (a stub list until B merges); BM uses PT's battle mode and B's battle mode.

---

## Gates

- `cargo fmt --all --check`; clippy `-D warnings` plain, with `--features game-native/window,studio/ui`, and `PKR_GAME=silice -p content -p engine -p extract`; `game-gba` fmt and clippy with and without `tour`.
- `cargo test --workspace` (Emerald) and `PKR_EXTRACTED_DIR=/nonexistent cargo test --workspace` (stub); `PKR_GAME=silice cargo test -p content -p engine -p extract`; `cargo test -p studio --features ui` if the studio is touched.
- `cargo xtask rom`; `cargo xtask parity` (and `--long` at close-out: the map tour, the screen tour, the Act 1 replay); `cargo xtask checkpoints verify` on the three trees; `cargo xtask sweep` and `--repro`; `cargo xtask silice check`.
- **Numbers:** `loop_lines_max` ≤ 217 and `hash_lines_max` ≤ 65 on every lane and tour frame; `late == 0` (with `late_saving` reported apart); `events_dropped == 0`; `heap_max` ≤ 160 KiB; stack untouched ≥ 1 KiB; ROM ≤ 16 MiB; worst-case save ≤ `MAX_ENCODED`; `cargo test --workspace` wall time ≤ 1.5× the Wave 0 baseline.

---

## Emerald parity expectations

| Area | Exact (tested against the decomp or ROM-vs-native) | Documented deviations |
|---|---|---|
| Start menu | entries and flags, order, window position and width, cursor memory, freeze | Pokédex, trainer card and PokéNav open nothing |
| Party menu | slot layout, actions per mode, refusals and messages, switch order | the switch slide animation is shortened to a redraw if over budget (reported) |
| Summary | values, pages 1–3, navigation | no contest page, no ribbons |
| Bag | pockets, order, capacities, actions, item effects and messages, repel and Escape Rope rules | the bag sprite's shake on pocket change is skipped if over budget |
| Mart | prices, sell at half, Premier Ball bonus, limits, messages | no map scroll behind the windows |
| PC | menu by flags, item storage, storage logic and box order | list UI for Pokémon storage; no box names or wallpapers; no decorations or mailbox |
| Options | six options, values, defaults, effects | battle scene off changes nothing (no animations yet); sound stored only |
| Save | dialog texts and flow, frame counts of the timers, CONTINUE window | our save format; flash time stalls the shell |
| Battle | BAG/POKéMON flows, item effects, ball multipliers, shift prompt, forced switch | no ball throw animation (M11), Safari Zone M11 |
| Evolution | methods, conditions, cancel, move learning | no sparkle animation; day-only friendship evolutions until the RTC (M10) |

**mGBA lanes:** ROM-vs-native lanes (Task Q) prove our ROM equals our native build hash for hash. The **vanilla** comparison is outcome- and frame-based (Task V, opt-in, the user's own save): it compares RAM results and named frame points, never pixels, and nothing it reads is committed.

---

## The Silice chapter M9 unlocks

**SIL2 (Ch1–Ch3) can start its content waves after M9 Wave 2 merges.**
- **Ch1 Bois-Éther:** fully playable with M9: the two marts with flag-dependent stock (`mart(&ctx, if flag { &STOCK_B } else { &STOCK_A })`), the party and bag, the Mallette entry, saves; the Cut module's brambles use the documented script fallback until M10.
- **Ch2 Gris-Échec:** playable with fallbacks: `screen_flash` real, `redraw_map` for the invisible maze within budget (F), the NPC evolution scene (`play_evolution`, E); Strength-like blocks by script until M10; Mega Alakazam already exists (Silice engine work).
- **Ch3 Viber-City:** playable with fallbacks: `give_mon` with moves and item (Porygon), money checks, coins (casino counters), white flash (smoke bomb); Flash (blackout scene) and the doubles use their documented fallbacks until M10/M11.
- **Still SIL2's own work** (content, on Task K's kit): the Mallette's panels (calls with portrait, logs, the sync gauge), Silice's marts and items data.

---

## Findings carried from M8

| Finding (source) | Disposition |
|---|---|
| Escalator ride; Lavaridge, Aqua Hideout, Mossdeep, Mt. Pyre, Union Room warp effects (M8 P prerequisite) | **M10** (field effects; Emerald Act 3+, no Silice Ch1–3 use) |
| `MB_PETALBURG_GYM_DOOR` arrivals stay plain (M8 P) | **Task F** |
| Scripted warp onto a ladder keeps the current facing (M8 P) | **Task F** |
| `MB_WATER_DOOR`, `MB_WATER_SOUTH_ARROW_WARP` blocked (M8 P) | **M10** (Surf) |
| `DrawWholeMapView` redraws the whole view on a visible frame; Sidney's room in `KNOWN_OVER`; 4 stretch overruns (M8 M review 5) | **Task F** (dirty cells), checked by Q's tour |
| Champion's room loop 220 in the walk-in cutscene (M8 M) | **Task F** checks whether the dirty redraw fixes it; else **M10** |
| Remaining `Unsupported` specials (M8 J, L) | **Task J** for the menu, PC, party, save, trade and link ones; contests, decorations, secret bases, Frontier **M13**; field effects, RTC, bikes **M10**; battle facilities **M11/M13** |
| `SaveGame` `Default(0)`, `PlayerPC`/`BedroomPC` unsupported (M8 J) | **Task J** + **S** + **M** |
| Full save format with bag and PC (M5 decision 10) | **Wave 0** (spike, `SCHEMA` 4) + **Task S** |
| Evolution scene; friendship evolution; `EvoMethod::Level` folds Silcoon/Cascoon (M8 G) | **Task E** |
| Party switching, items and bag in the explorer policy and key (M7) | **Task D** (hooks, key) + **Task N** (policy, macros) |
| `explore sweep` (M8 decision 13) | **Task D** |
| Items sub-tab, water/fishing encounters, trainer lists, dynamic-warp and sight overlays (M8 S) | **Task T** |
| `RoundTrip` "camera far outside the map" during Briney's boat (M8 N; P met it again at Route 104 (71, 189), off the Act 1 trace) | **Task F** |
| PlannerDivergence on Route 115 (elevation, M8 N; P: (10, 63), (10, 64), (12, 61)) | **M10** (elevation and bridges) |
| `UpdateEscapeWarp` needs the map type (M8 F, L) | **Task F** (map type, escape warp); Escape Rope **Task B** |
| Pickup gives no item (M8 G) | **Task BM** |
| `waitfanfare` as `waitmessage`, plural item names, `BufferTMHMMoveName` (M8 H) | **Task J** (names, TM move names); the fanfare hold waits for audio (**M12**) |
| `StartWallyTutorialBattle` degraded (M8 J) | **M11** |
| `TRAINER_HELD_ITEMS`, `GEN3_EXTRA_ABILITIES` Emerald switches off (Silice engine) | **M11** (they move the Emerald route and need the unmodelled effects) |
| Sight frame count vs vanilla Emerald (M8 L, carried to P) | **Task V** if P did not close it |
| Deduplicate the `StartAt` wrappers (M8 L follow-up 12; close-out 7: also the two start-cell rules, `sweep::start_cell` and `tour_logic::landing`, and the three `script_name` helpers) | **Task Q** |
| `explore`'s `testing` feature linked into xtask only for `forward_game_data!` (M8 close-out 8) | **Task Q** (moves the macro and the shared `StartAt` out of `testing`) |
| Test gaps (M8 close-out 9): G1 Act 1 party end to end and Roxanne's AI flags; F crossing vs full upload and `VAR_LAST_TALKED` for a previous-map NPC; A string round trips vs ROM bytes; N a real override edit; J the Act 1 specials test without `PKR_DECOMP`; M memory gates on the probe lanes; E recursive overrides hygiene | G1 **Task BM**; F **Task F**; J **Task J**; M **Task Q** (with the screen tour's gates); A, N, E **M10** or whoever next edits those areas |
| As-built gaps: M8 C, D1, J have no section (M8 close-out 10) | Written when those areas change (J: **Task J**'s as-built) |
| Gen3-wide constants in the engine: caps 99/999, `MAP_DYNAMIC`, `sight.rs` trainer kinds, `CHAR_0`, `OBJECT_EVENTS_COUNT` (M8 close-out 12) | Into `SpecialIds` when a second game needs other values (not M9: Silice keeps Gen3's) |
| `battle::encounters(id)` still generated (first header only); `Objects::spawn_map`/`enter_connected` public for tests; `#[allow(unused_imports)]` in `battle/core.rs` (M8 close-out 13) | **Task T** (water and fishing views use `wild_encounters`, then drop `encounters`); the other two **Task Q** |
| Studio smoke 50 fps gate to re-measure on an idle machine (M8 close-out 14) | **Task T** |
| Silice start facing hardcoded to Down (SIL1 E) | **Task C** |
| Door animations, step timing, elevation priority, field poison (`TryFieldPoisonWhiteOut`) | **M10** |
| Late apply at Roxanne's battle start, engine frame 80,366 of the Act 1 trace (M8 P): `Battle::start` runs on the transition's first frame and the first `NewTurn` runs the gym leader's AI choice in the same step (223 lines). `KNOWN_LATE` in `emu/tests/parity.rs` cuts the long lane around it (the ROM does not replay 80,366–85,307) and checks it still happens | **Task BM** (owns `world/battle.rs` and the battle step): run the first turn's AI choice (or `start`) on a later transition frame, after checking Gen3's RNG order there; then remove the `KNOWN_LATE` entry. Task F's budget work does not cover it (a battle step, not a redraw) |
| Explorer policy has no type-matchup or level term: a Torchic chain stalls at Roxanne; `Weights::balance` is 0 (M8 P) | **Task N** (a matchup/level term in the battle policy), or the act runner tries each starter; checked on Q's Emerald route |
| Reached by P's explorer off the Act 1 trace, `Unsupported`: `pokemart` (Oldale, Petalburg, Rustboro) | **Task M** (mart) |
| — `SPECIAL_BedroomPC` | **Task M** (PC; with `PlayerPC`, row above) |
| — `SPECIAL_ShowPokedexRatingMessage` | **Task J** (the rating text; the Pokédex counts are Wave 0's) |
| — `dofieldeffect` / `waitfieldeffect` (the Pokémon Center heal; also on the slice route) | **M10** (field effects) |
| — `pokenavcall`, `copyobjectxytoperm` (Route 104) | `pokenavcall` **M13** (PokéNav); `copyobjectxytoperm` **Task J** (a plain object-template write) |
| — `move effect 109` (`EFFECT_CURSE`) | **M11** (battle effects) |
| Reached off the trace, `Degraded`: `SPECIAL_ChoosePartyMon` | **Task PT** (party menu, `PartyMode::Choose`) wired by **J** |
| — `SPECIAL_ObjectEventInteractionGetBerryTreeData` | **M10** (berry trees, with the RTC) |
| — `SPECIAL_ShowEasyChatScreen`, `SPECIAL_DoWaldaNamingScreen` | **M13** (Easy Chat; Walda's phrase) |
| On the Act 1 trace itself, `Degraded` (the only three): `bufferitemnameplural`, `SPECIAL_StartWallyTutorialBattle`, `evolution scene` after Roxanne | plural names **Task J** (row above); Wally **M11** (row above); the scene **Task E** (it opens through `world/evolve.rs::after_battle`, which moves the Act 1 route from Roxanne's battle end: E re-accepts `act1:badge1`) |
| `SaveMeta` gained the player name, Pokédex caught count and badges (Wave 0): an older slot fails agb's metadata check and boots as a new game (`boot::CORRUPT`) | **Task S** (the main menu shows the metadata; no migration: pre-M9 slots are debug saves) |

Task P's "Findings for Milestone 9" (P findings 1–5) and the M8 close-out's "Findings carried to Milestone 9" (6–14; 6 is the `DrawWholeMapView` row, 11 is M11's G2 breadth) were folded in at the Wave 0 merge prep (master `64dbb4c`); none was fixed in M8, so none is marked done.

---

## Roadmap after M9 (Silice-ordered)

- **M10 — Field moves and overworld (SIL2's rest, SIL3's start):** Cut, Strength, Flash first (through `field_move_gate`), then Rock Smash, Surf, Fly, Waterfall, Dive; fishing and water encounters; weather (with a game-defined petals kind); the RTC and a deterministic clock; elevation and bridges; door animations; step timing; field poison; the escalator ride and special warp effects; ice, arrow and rotation tiles; bikes.
- **M11 — Battle completeness (SIL3–SIL4):** doubles, two-trainer and multi battles with a partner; every effect and ability, held items for Emerald trainers; trainer item AI; ball animations; the Wally tutorial; Safari.
- **M12 — Audio** (as M8's roadmap).
- **M13 — Side content:** the Pokédex screen, trainer card, PokéNav, Gen3's box UI, contests, Game Corner, decorations and secret bases, the Frontier.
- **M14 — Explorer and studio v1** (as M8's roadmap).

---

## Decisions taken

1. **Scope follows SIL2 (taken; user decision 4 of the Silice plan).** SIL2's list (Silice §3.2) is mart, menus, save, the Wave 0 requests and three field moves. The first four are UI and state work that shares one kit and one state change; field moves are field-physics work that shares nothing with them and has script fallbacks in every SIL2 chapter. Splitting there keeps each milestone one kind of work. The evolution scene and friendship come in because SIL2 Ch2 asks for both (`play_evolution`, `add_friendship`) and M8 left them degraded.
2. **Screens as one trait plus a stack (taken).** Gen3 chains `CB2` callbacks and tasks per screen; our `Mode` already swaps the screen owner for battles and the starter screen. One trait lets each screen be a file with one owner, lets the stack nest the Gen3 way (bag → party), and keeps one hashing and capture rule ("no capture while a screen is open", like battles).
3. **A kit first (taken).** Every screen needs list menus, frames, icons and quantity input; writing them once avoids five divergent copies and lets the per-frame budget be enforced in one place. The cost is a dependency (Wave 2 waits for K), accepted because the kit is small next to the screens.
4. **Our own save format, not Gen3's (taken).**
   - *For ours:* it is the M5 checkpoint, already a versioned DTO with stable ids, already proved byte-identical between ROM and native, already crash-safe (agb's ghost slot), already the explorer's and the studio's currency. Silice's ids use Gen3 numbering, but its state is ours (template edits, screen effects), which Gen3's save blocks cannot hold.
   - *For Gen3's:* interop with vanilla saves and tools, and an exact `SaveGame` sector timing. Against: 128 KiB flash (stock mGBA detects 64 KiB for our ROM, M5), a second encoding of the same state with its own drift, encrypted substructs and section checksums to maintain, and Gen3 sections sized for Gen3 structures only.
   - *Kept:* the interop value that matters (comparing with vanilla) comes cheaper as a one-way host-side reader for reference lanes (Task V).
5. **64 KiB and a compact PC (taken, measured in Wave 0).** A full Gen3 PC is 33.6 KB of `BoxPokemon`; agb's slot on 64 KiB holds roughly 24 KiB. Dropping derived fields (stats, HP, status) and using postcard varints brings a boxed mon to about 45–55 bytes. Whether 14 boxes fit is measured, not assumed; if not, Emerald gets fewer boxes (a reported deviation, rarely felt) rather than a save type stock mGBA cannot detect.
6. **Flash time outside the engine (taken).** Determinism needs native and ROM to step the same frames; flash erase and write take real milliseconds only on hardware. Running Gen3's dialog timers in the engine and letting the shell stall keeps parity exact; `late_saving` keeps the measurement honest.
7. **Options at Gen3 defaults are today's behaviour (taken).** Text MID is the current 4 frames a glyph, so adding options moves nothing. The shift prompt is the exception: Gen3's default style asks, the engine never did, so BM is route-changing by design.
8. **Hash only what changed (taken).** M8's Silice engine work hashed held items only when present, so Emerald did not move. M9 does the same for every new field except the Pokédex flags, which Gen3 sets on the first encounter of every route: one deliberate re-accept in Wave 0 instead of one per screen task.
9. **No title screen, a main menu (taken).** Continuing a save needs a choice at boot; Gen3's main menu is that choice. The title screen is presentation (Silice wants its own two-state title in SIL5).
10. **Pokémon storage as a list (taken).** The box UI is one of Gen3's largest screens and adds nothing SIL2 needs beyond deposit and withdraw. Exact storage logic keeps saves and overflow correct; the box screen can replace the list later without a state change.

---

## Size estimate

| Wave | Tasks | Size (agent sessions) |
|---|---|---|
| 0 | contracts, spikes, state change | 1 large |
| 1 | K (L), X1 (M), X2 (M), C (M), S (L), R (M), E (L), F (M), J (M), T (S), D (M) | 11 parallel |
| 2 | SM (M), PT (L), B (L), BM (L), M (L) | 5 parallel |
| 3 | Q (L), N (M), V (M, stretch), W (M, stretch), close-out (M) | 3–5 |

- **In total:** 20 task sessions (22 with the stretches), about M8's size.
- **Hand-written code:** about 20–25k lines including tests, most of it in `engine/src/ui/`.
- **Critical path:** Wave 0 → **X1/X2 → C → (K) → PT/B → BM → Q**. K runs in parallel with the extract tasks; BM is last because it uses PT's and B's battle modes.
- **Main risks:**
  - the save capacity (Wave 0 measures it before anything depends on it);
  - the per-frame budget of full-screen screens on the GBA (the kit's spread uploads, the screen tour);
  - the route re-find after BM's shift prompt (N's policy, M8's re-find rule);
  - UI asset location on BPEF (X1; the M8 anchors approach, verified on BPEE).

---

## Wave 0 as built

### Step 0 numbers
- **Save size.** The worst-case save (`engine` `checkpoint/tests/worst_case.rs`: six party mons and every boxed mon at their widest varints, every flag and var set, every bag pocket full at 99, PC items full, the longest names, every M9 field non-default) encodes to **9,748 bytes + 1,950 per full PC box**: 0 boxes 9,748, 7 boxes 23,399, 14 boxes 37,049.
- **Flash capacity.** agb's one-slot layout on 64 KiB accepts **24,528 bytes** for every repeated write (`game-native` `flash::tests::slot_capacity_holds_max_encoded`, a binary search over three successive writes; the ghost copy halves the chip). So **`MAX_ENCODED = 24,016`** (capacity − 512) and **Emerald `pc_boxes = 7`** (14 does not fit: 37,049 bytes). Silice uses the same constant (`content` `PC_BOXES`).
- **128 KiB not attempted.** M5 found that mGBA presents FLASH512 (Panasonic id) to a homebrew ROM and ignores agb's `FLASH1M_V` marker and the `.sav` size, so a 128 KiB layout would not boot in a stock mGBA; decision 5's fallback (fewer boxes on 64 KiB) applies. Revisit only with a cartridge-detection change in agb or our own flash driver.
- **Screen budget.** No separate GBA spike: `ui::kit::UPLOAD_BYTES_PER_FRAME = SPREAD_TILES_BLACK × 32` (**16 KiB** on a black frame) and `UPLOAD_BYTES_VISIBLE = SPREAD_TILES_VISIBLE × 32` (**4 KiB** on a visible one), the per-frame tile budgets Task F measured on the GBA for warps (`field.rs`). A party menu's ~8 KiB of tiles plus maps and palettes therefore fits one black frame; Task K re-measures on the ROM with the first real screen and may lower them.
- **Route-usage scan:** see below.
- **Findings carried from M8:** Task P's findings were folded in when P merged (before Wave 0 merged), each with an owner.

### Route-usage scan
`crates/game-native/tests/m9_route_scan.rs` (`#[ignore]`d; `cargo test -p game-native --release --test m9_route_scan -- --ignored --nocapture`, with `PKR_DECOMP` for the static column). It replays the slice route and every committed node. First run on master at Wave 0 (four `route:*` nodes, the slice route itself); re-run at the merge prep after M8 Task P merged, when the Emerald tree has nine nodes: the four `route:*` (19,184 frames) and P's five `act1:*` from a new game to badge 1 (`act1:pokedex` → `act1:badge1`, 85,307 frames). The tree column below is that second run (104,491 frames).

| Count | Slice route (19,184 frames) | Committed tree (9 nodes, 104,491 frames) |
|---|---|---|
| START / SELECT pressed on the idle field | 0 / 0 | 0 / 0 |
| `pokemart` executed (report) / reachable from the started scripts (decomp, over-approximation) | 0 / 0 of 74 labels | 0 / 0 of 74 (49 scripts started, 1,866 labels reached statically) |
| `special SaveGame` reachable | 0 of 2 | 0 of 2 |
| `ChoosePartyMon` / `ChooseMonForMoveTutor` reachable | 0 of 11 / 0 of 3 | 0 of 11 / 0 of 3 |
| `PlayerPC` / `ShowPokemonStorageSystemPC` reachable | 0 of 2 / 0 of 2 | 0 of 2 / 0 of 2 (the lab PC script runs: Birch's PC is a message, not `PlayerPC`) |
| FADE_TO_WHITE / FROM_WHITE executed | 0 (Emerald's scripts have none: R and J's white fades are Silice-only) | 0 |
| BAG / POKéMON chosen in battle | 0 (the route only fights or runs: `Action::Fight`, `Run`, `Mash`) | 0 (P's explorer policy has no item or switch action, carried to D/N) |
| Battles; trainer wins; with 2+ usable mons (shift prompt) | 8; 1; 0 | 27; 13; 0 (never two usable mons at a trainer win) |
| Level evolutions at a won battle's end (silent, `Degraded("evolution scene")`); still due after it | 0; 0 | 1 (after Roxanne, `act1:badge1`); 0 |
| Field steps; with a poisoned party; with a Wurmple-line mon | 181; 0; 0 | 1,593; 0; 0 |
| Screens opened | 0 | 0 |
| Reports | `Unsupported(dofieldeffect)`, `Unsupported(waitfieldeffect)` (once each, M8) | those two, plus `Degraded`: `bufferitemnameplural`, `SPECIAL_StartWallyTutorialBattle`, `evolution scene` (once each) |

So on the committed routes every M9 task is **route-neutral** except:
- the Pokédex digest Wave 0 adds (one re-accept, Wave 0's own);
- **Task E**, whose evolution scene opens after Roxanne on `act1:badge1` (from that battle's end on; E re-accepts the node);
- **Task BM**, if it moves Roxanne's battle start off the transition's first frame (the carried `KNOWN_LATE`), from engine frame 80,366 on.

K, SM, S, J, M, B and PT change no route frame until a route presses START or reaches one of the sites above (Task Q's Emerald route decides that). The off-trace reach of P's explorer (`pokemart`, `ChoosePartyMon`, `BedroomPC`) is listed in the carried table; it moves no committed node.

### Contracts changed versus this plan (and why)
- **`world/save.rs::save_now(&World, game)`** takes the World, not `&mut State`: the capture code it reuses (`world/checkpoint.rs`) reads the runner and mode too. `World::save_snapshot(&self, game)` forwards to it.
- **`ScreenCx` input** is `ScreenInput { held, pressed }` rather than a bare `Buttons`, so screens see edges the same way the field does.
- **`Pokedex`** is indexed by internal species id (the game's `SpeciesId`), not national number: the engine has no national-dex table, and Silice's species are not national. The numbering only matters for the Pokédex screen (M10+), which can map it.
- **`Options::sound`** defaults to MONO (Gen3 `OPTIONS_SOUND_MONO` is 0).
- **Friendship hook** is `friendship::on_event(&mut Pokemon, FriendshipEvent, hold: Option<HoldEffect>, &BattleTables, &mut Rng)`: the battle core has the hold effect and its tables at those sites, not a `GameData`.
- **`repel_blocks(level, vars, party, ids)`** takes the pieces it reads instead of `&State` (the wild code is called from the battle module with a split borrow), and `wild::try_land_encounter_repel` was added to thread it through.
- **`whiteout_allowed(Option<&Request>)`** takes the waiting request (the runner's `waiting_on()`), since the battle module sees the request, not a `BattleTrainerReq` struct.
- **START/SELECT hooks** run in Gen3 `ProcessPlayerFieldInput` order: after the A interaction and the door warp checks, not before them (the plan's "before the A checks" contradicted the decomp).
- **`Choice::Item { item, party }`** (the party slot a Potion targets) instead of `Choice::Item(ItemId)`.
- **`SaveMeta` grew** (name, Pokédex caught, badges): pre-M9 slots fail agb's metadata check and boot as a new game (carried to Task S).
- **Checkpoint schema 4 compatibility**: the M9 fields at their defaults encode as 8 trailing zero bytes (`M9_DEFAULT_TAIL`); `decode` of schema 2/3 appends them, and `state_digest` drops them when `m9_is_default()`, so every recorded node keeps its digest.
- **Silice gets `start_menu`** through the shared `game_data_tables!` macro (Emerald's Gen3 list) until Task C gives it its own.
- **`World::screens()`** accessor (the stack, for tests and the debug region) beside `screens_open()`.

### Contracts added by the Wave 0 review
Each closes a gap that would have made a Wave 1 or 2 task edit a file it does not own.
- **`ScreenCx`** gains `rng: &mut Rng` (Task E's `on_event` from B's vitamins and Rare Candy), `vars: &mut Vars` (was read-only: B's Repel writes `VAR_REPEL_STEP_COUNT`, M's PC writes `VAR_PC_BOX_TO_SEND_MON`), `map: MapId` and `frame: u32` (S's save info window: location and play time).
- **`ScreenStack`** carries the waiting results, hashed with it: `saved: Option<bool>` (the World's answer to `ScreenOut::Save`, given once next frame; S's `save_failed` turns it to `false`), `child: Option<ScreenResult>` (a closed screen's result for the one below), and `host: ui::kit::Hosting` (Task K's fade, VRAM-handover and waiting-request state, defined in K's `ui/kit/mod.rs`, so K changes neither `ui/mod.rs` nor `world/mode.rs`). `world/screens.rs` fills `saved` and `child` and raises `Event::Saved { ok }` on `ScreenOut::Save`.
- **`ScreenOut::UseOnField(ItemId)`** and the hook **`item_use::use_on_field(&mut State, game, item) -> Option<ScriptId>`** (Gen3 `SetUpItemUseOnFieldCallback`): the bag closes, the field comes back, then the item's field use (B's Escape Rope; M10's bikes, rods, Itemfinder). `ScreenOut::RunScript` now raises `Event::ScreenClosed` (it raised nothing: the stack was already out of the mode) and draws the field.
- **`PartyMode::Battle`** documents that BM hosts the party menu inside the battle with the battle's party in battle order (Gen3 `gBattlePartyCurrentOrder`): the active mon is slot 0, so PT needs no battle state.
- **Post-battle evolution hook** `world/evolve.rs::after_battle(game, &mut State, &Battle) -> bool`, called by `world/battle.rs` when a won battle leaves, over the black screen, before the field is restored (Gen3 `FreeResetData_ReturnToOvOrDoEvolutions` → `TryEvolvePokemon`). `true` means E opened the evolution scene (`world/screens.rs::open`): the battle ends there (`end_battle`: the objects, the script's answer and the encounter immunity as before) and K's host restores the field when the last screen closes. Wave 0 answers `false` (the silent evolution stays in `after_win`). Without it E would have edited `world/battle.rs` (BM's) to show a scene.
- **`World::snapshot_unchecked`** (`world/checkpoint.rs`): the snapshot without the capture checks, which `snapshot` now wraps. Task S's `save_now` saves mid-script (`special SaveGame`) or under the save dialog, which `capture` refuses; it normalises this DTO instead of editing F's file.
- **White fades:** `Fader::is_white()` (R's `flow/fade.rs`, `false` in Wave 0), copied into `Frame::lighten` by `draw_field`; `fx::apply` takes `&mut ScreenFx` (R advances the effects there). `host/mod.rs` routes the polling (`resume`) of the effect and party requests to `host/fx.rs` and `host/party.rs` (they went to the generic `done`), so R's `WaitInputOrTimeout`, flash and shake can wait.
- **Button mode:** `World::step` calls `Input::update_mode(keys, options.button_mode)` (SM's `flow/input.rs`; Wave 0 ignores the mode), the place Gen3 `ReadKeys` maps L to A.
- **`SpecialIds`** gains `var_pc_box_to_send_mon`, `var_mon_box_id`, `var_mon_box_pos` and `flag_shown_box_was_full` (Gen3 `CopyMonToPC`), filled for both roots; `SHARED_GEN3` reserves `VAR_MON_BOX_ID`, `VAR_MON_BOX_POS` and `FLAG_SHOWN_BOX_WAS_FULL_MESSAGE` (no Silice id held them).
- **Schema 3 RON snapshots** (the stores' snapshot caches) read too: the schema 4 fields are `#[serde(default)]`, as `party_held` was in schema 3.
- **Notes for the owners:** the text speeds keep the engine's frames-per-glyph model (Gen3's printer takes one frame more per glyph at each speed; M3 chose 4 for MID), so SLOW 8 and FAST 1; Gen3's battle messages read the text speed too (`battle_message.c`), which BM wires; Task S's 24 KiB worst-case encode is a heap allocation on the GBA, to count against `heap_max`.

### State change
- `SCHEMA` 4; the new state is hashed only away from its default except the Pokédex digest (`0xA3`, once non-empty), so the routes move from their first battle start. Engine goldens: `GOLDEN_WILD_BATTLE` and `GOLDEN_TRAINER` moved; `GOLDEN_SCREEN` and `GOLDEN_SAVE` pinned; the rest unchanged.
- `cargo xtask checkpoints verify --accept` (`chore(checkpoints): M9 Wave 0 state`, the last commit): Emerald's four `route:*` nodes diverged from `route:lab` +1500 (the starter's Pokédex entry; the rest follow their parent), the stub's three from `route:stub_start` +24 (its new game gives a mon); frames, inputs and maps unchanged, only hashes and digests moved. Silice's tree is empty. Re-done after the Wave 0 review on the merged tree (master `582e508`), and again at the merge prep on master `64dbb4c` (after M8 Task P, the M8 close-out and the explorer and gate speed-ups) (M8 Task P's five `act1:*` nodes added): the `route:*` and stub divergence points are unchanged, and `act1:pokedex` diverges from +2700 (its first Pokédex entry, the starter in Birch's rescue, which P's explorer reaches later than the slice route), the other `act1:*` nodes only because their parent did. With Pokédex recording switched off (a scratch build) every node of both trees, the `act1:*` ones included, verifies `valid (rehash)`, so the Pokédex digest is the only route change; frames, inputs and maps of every node are unchanged.
- `cargo xtask parity` after the accept: ROM and native agree on the whole route (19,184 frames, 19,176 hashes and 205 pictures identical, no late frames); worst route lines hash 46 (gate 65), loop 172 (gate 217, margin 45), `heap_max` 47,412 B; save/resume, mid-battle save and native-checkpoint lanes identical; the ROM save is 431 checkpoint bytes.
- Silice: its two vars on Gen3 numbers the shared code now names (`VAR_SIL_CASINO_STATE` on `VAR_REPEL_STEP_COUNT`, `VAR_SIL_PORT_RESSAC_STATE` on `VAR_FRIENDSHIP_STEP_COUNTER`) moved with `cargo xtask silice move` (tombstones kept, `ids.golden` appended); `SHARED_GEN3` lists the new names.

## Findings for Milestone 10
*(filled at close-out.)*

---

## Ownership check (no file has two owners)

| File or glob | Owner |
|---|---|
| `engine/src/{data.rs, script.rs, checkpoint.rs, world.rs, world/mode.rs, world/host/mod.rs, ui/mod.rs, video.rs, mon.rs, lib.rs, debug.rs}`, `world/tests.rs` (mod lines), `world/tests/goldens.rs`; `emu/src/lib.rs`; `extract/src/{schema.rs, lib.rs}`; `content/src/prelude/mod.rs`, `content/build/main.rs` | Wave 0 (then only `chore(checkpoints)` commits for goldens; `schema.rs` see below) |
| Wave 0's hook and split edits in files a later task owns: `world/battle.rs`, `world/checkpoint.rs`, `battle/core.rs`, `wild.rs`, `mon/*`, `text.rs`, `flow/{fade,input}.rs`, `fx.rs`, `item_use.rs`, `ui/kit/mod.rs`, `game-gba/src/{main,save_logic}.rs`, `game-native/src/{lib,store,controls}.rs`, `content/src/lib.rs`, `content/build/{codegen,blob_data}.rs`, `extract/src/export.rs`, `xtask/src/main.rs` | Wave 0 first (merged before any task starts), then the owner below |
| `extract/src/games/data.rs` (`SHARED_GEN3`) | Wave 0; a task whose shared code names a new Gen3 flag or var appends it there (the `sil1_w0_contracts` test fails otherwise) and lists it in its report |
| `engine/src/ui/kit/**`, `world/screens.rs`, `world/host/screens.rs`, `text.rs`, `text/printer.rs`, `menu.rs` (after Wave 0's text-speed and yes/no edits) | K |
| `engine/src/world/save.rs`, `ui/save.rs`, `ui/main_menu.rs`, `world/host/save.rs`; `game-gba/src/{save,save_logic,main}.rs`; `game-native/src/{flash,lib}.rs`; `emu/tests/save_logic.rs` | S |
| `engine/src/fx.rs`, `flow/fade.rs`, `world/host/{fx,party}.rs`; `content/src/silice/prelude.rs` | R |
| `engine/src/ui/evolution.rs`, `mon/evolution.rs`, `world/evolve.rs`, `world/friendship.rs`; `content/build/classify/evolution.rs` | E |
| `engine/src/field.rs`, `field/*`, `flow/transition.rs`, `world/maps.rs`, `world/checkpoint.rs`; `content/build/classify/field.rs` | F |
| `content/build/support/{specials,commands}.ron`, `content/src/prelude/{special,std,mart,party}.rs`; `extract/src/codegen/lower/{emit,operands}.rs` | J |
| `extract/src/{ui_gfx.rs, export/ui.rs}` | X1 |
| `extract/src/{ui_text.rs, tables.rs, export/tables.rs}`, `export/scripts.rs` (lists), `labels.rs` (lists), `scripts.rs` (the `pokemart` operand) | X2 |
| `content/build/{ui.rs, classify/items.rs}`, `codegen.rs` + `blob_data.rs` (new tables), `content/src/lib.rs` (tables, `gen3_special_ids`), `content/src/silice/mod.rs`, `games/silice/game.ron`, `extract/src/games/game.rs` | C |
| `studio/**` except `map_edit/**` | T |
| `explore/**` | D (Wave 1), then N (Wave 3) |
| `engine/src/ui/{start_menu,options}.rs`, `flow/input.rs` | SM |
| `engine/src/ui/{party,summary}.rs` | PT |
| `engine/src/ui/bag.rs`, `item_use.rs`, `wild.rs` (repel) | B |
| `engine/src/battle/{core.rs, core/items.rs, scene.rs, scene/menu.rs, ui.rs, effects/ball.rs, effects/items.rs, calc.rs}`, `world/battle.rs` | BM |
| `engine/src/ui/{mart,pc}.rs`, `pc.rs`, `world/caught.rs`, `mon/boxed.rs` (after Wave 0's DTO) | M |
| `emu/tests/{parity,tour}.rs`, `game-gba/src/tour_logic.rs` (and `main.rs` after S), `xtask/src/{budget,sweep}.rs`, checkpoint nodes | Q |
| `xtask/src/vanilla.rs` | V |
| `engine/src/ui/naming.rs` | W |

Shared files touched by more than one wave are sequential, never concurrent: `explore/**` (D in Wave 1, N in Wave 3); `game-gba/src/main.rs` (S in Wave 1, then Q in Wave 3 for the screen tour's `tour`-cfg hooks); `mon/boxed.rs` (Wave 0's DTO, then M); `text.rs` (Wave 0, then K).

One accepted concurrent edit: `extract/src/schema.rs`'s `FORMAT_MINOR` constant and its `MINOR_FILES` rows. X1 and X2 each bump the minor and add their rows (M8's A and B did the same); whichever merges second rebases a one-line conflict and takes the next number. Nothing else in `schema.rs` changes after Wave 0.

Cross-task calls go through Wave 0 signatures, never through another task's file: K calls S's `save_now` and M's `place_mon` (stubs until they merge); B pushes E's `ScreenKind::Evolution` and uses F's escape warp slot; BM pushes PT's and B's battle modes and prints M's `Placed` result.

---

## Open questions (for the user)

Each has a default the plan applies if there is no answer before Wave 0 Step 0 ends.

1. **PC capacity versus flash size.** If the Wave 0 spike shows that Gen3's 14 boxes do not fit the 64 KiB slot, do you accept fewer boxes for Emerald (default; a documented deviation), or do you want 128 KiB flash, which a stock mGBA does not detect for our ROM (players would need an mGBA save-type override)? Silice's box count is its own `game.ron` choice either way.
2. **Silice start menu.** The plan makes the Mallette a game-defined entry in PokéNav's slot and keeps Pokédex, Pokémon, Bag, Save, Options and Exit as Gen3 has them (default). Should the Pokédex entry be folded into the Mallette's "Analyse" instead (then Silice's menu has no POKéDEX line)?
