# Milestone 4 — Battle Slice Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (parallel waves) to implement this plan.

**Goal:** Fight the first battles of the game in the new engine, on the GBA build and the native build, with pixel parity.
- A new game starts before Birch's rescue. On Route 101 the player picks Treecko, Torchic or Mudkip through `SPECIAL_ChooseStarter`, then fights the first battle against a Lv2 Zigzagoon.
- Wild Pokémon appear in the tall grass of Routes 101, 102 and 103, from the real encounter tables.
- The rival May battles the player on Route 103 through the generated `trainerbattle` script. The Route 102 trainers also battle when the player talks to them.
- Battles are Gen3-exact singles, with French messages, menus, healthboxes, HP and EXP bars. Experience is gained, the Pokémon levels up and learns moves, the player can run away, and losing whites out.

**Architecture:**
- **Extraction:** schema v3 adds battle data, battle graphics and French battle strings under `content/extracted/`. Content `build.rs` compiles them into `'static` tables behind new `GameData` methods.
- **Battle logic core** (`engine::battle::core`): pure and video-free. It takes `Choice`s and pushes `BattleEvent`s, and draws every random roll from the `World` RNG it is handed. Formulas live in `engine::mon` (the Pokémon model) and `engine::battle::calc` (the battle maths). Move effects are Gen3 mechanics keyed by `MoveEffect`; content maps them from `EFFECT_*` names.
- **Battle presentation** (`engine::battle::{scene, ui, gfx, text, transition}`): plays the event stream and turns input into `Choice`s. While a battle runs it owns the screen and its own VRAM layout.
- **World:** the battle becomes a `World` mode. Scripts start battles through new `Request`s; wild battles start from field steps. On return the field VRAM is restored and the script resumes with the `BattleOutcome`, or the player whites out.

**Tech stack:** Rust 2024, `no_std` + `alloc` engine, agb 0.25 (GBA), mGBA FFI (parity), serde/ron/png (build time).

**Wave 0, already committed (contracts: read them, do not redesign them):** see the next section for the exact Rust.
- `crates/engine/src/mon.rs` (types), `crates/engine/src/battle/mod.rs` (types, events, the core/presentation boundary)
- `crates/engine/src/data.rs` (ids, `GameData` additions), `crates/engine/src/script.rs` (requests, `Ctx` methods, `Runner::abort`)
- `crates/engine/src/video.rs` (`Cmd::ObjTile`, `Frame::lighten`, `Frame::is_black`, `layout::battle`), `crates/engine/src/rng.rs` (`random`, `random32`)
- `crates/extract/src/schema.rs` (v3 additions) and `crates/extract/src/names/battle.rs` (`SPECIES`, `TRAINER_PICS`, `TRAINER_BACK_PICS`; generated and drift-checked by `tests/names.rs` `generate_battle`, where Task A adds its tables)
- empty module files with `todo!()` bodies for every signature below, so each Wave 1 task fills only its own files. `lib.rs` already declares `battle`, `mon`, `wild`, `trainer` and `starter`, and `battle/mod.rs` declares every submodule plus `#[cfg(test)] test_data`, so no Wave 1 task edits `lib.rs` or `battle/mod.rs` just to register a file. Stubs carry `#[allow(unused_variables, clippy::ptr_arg)]`: drop it when implementing.
- already implemented (not stubs): `Rng::random`/`random32`, `Type::ALL`/`from_u8`/`is_physical`, `Stat::ALL`, `Nature::effect`, `Status::code`, `MoveFlags`, `BattleTables::species_info`/`move_info`, `Pokemon::moves_at_level`, `Pokemon::hash`, every `Party` method (`Party::heal` calls Task C's `Pokemon::heal`), `BattleOutcome::from_u16`, `Side::other`, the `Request`s/`Ctx` methods/`Runner::abort`, World placeholders for the new requests, `Cmd::ObjTile` and `Frame::lighten` in both shells (native `ppu::brighten` mirrors mGBA's `_brighten`).
- **As built (deviations, Wave 0):** see "Wave 0 as built" at the end of this section.

A task may add to a contract if it needs to. It may not change the meaning of what is already there, and every addition must be listed in its final report.

---

## Wave 0 contracts

### `engine::rng`
```rust
impl Rng {
    /// Gen3 `Random()`: the high half of the next state (what `Request::Random` already uses, so
    /// M3 hashes do not move).
    pub fn random(&mut self) -> u16 { (self.next_u32() >> 16) as u16 }
    /// Gen3 `Random32()`: `random() | random() << 16`, low half drawn first.
    pub fn random32(&mut self) -> u32;
}
```

### `engine::data` additions
```rust
id! { SpeciesId, MoveId, TrainerId }   // content-assigned = Gen3 SPECIES_*/MOVE_*/TRAINER_* numbers

pub trait GameData {
    // … M3 methods unchanged; every addition has a default so test games keep compiling.
    fn battle(&self) -> &'static BattleTables { &mon::EMPTY_TABLES }
    fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> { None }
    fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> { None }
    /// Gen3-encoded, 0xFF-terminated battle string (`gBattleStringsTable`, `gText_*`).
    fn battle_text(&self, s: battle::Str) -> &'static [u8] { &[0xFF] }
    fn battle_gfx(&self) -> &'static battle::BattleGfx { &battle::EMPTY_GFX } // empty slices: draws nothing
    /// Gen3 heal location `id` (`setrespawn`, whiteout): map and player cell.
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> { None }
}
// SpecialIds gains: `respawn_default: u8` (heal location of a new game; content sets 0 until Task I).
```

### `engine::mon` (Gen3 Pokémon model; numbers are data, formulas are Gen3)
```rust
pub const PARTY_SIZE: usize = 6;
pub const NAME_LEN: usize = 11;        // POKEMON_NAME_LENGTH + EOS

#[repr(u8)] pub enum Type { Normal, Fighting, Flying, Poison, Ground, Rock, Bug, Ghost, Steel,
    Mystery, Fire, Water, Grass, Electric, Psychic, Ice, Dragon, Dark }       // Gen3 TYPE_* order
impl Type { pub const ALL: [Type; 18]; pub const fn is_physical(self) -> bool;
            pub const fn from_u8(v: u8) -> Option<Type>; }
#[repr(u8)] pub enum Stat { Hp, Atk, Def, Speed, SpAtk, SpDef, Accuracy, Evasion } // STAT_* order
impl Stat { pub const ALL: [Stat; 8]; }
pub struct Nature(pub u8);                          // 0..25; boosts n/5, lowers n%5 (gNatureStatTable)
impl Nature { pub const fn effect(self, stat: Stat) -> i8; } // +1 / -1 / 0, for ModifyStatByNature
#[repr(u8)] pub enum GrowthRate { MediumFast, Erratic, Fluctuating, MediumSlow, Fast, Slow }
pub enum Gender { Male, Female, Genderless }
pub enum StatusKind { Sleep, Poison, Burn, Freeze, Paralysis, Toxic }
pub enum Status { None, Sleep(u8), Poison, Burn, Freeze, Paralysis, Toxic(u8) } // STATUS1_*, Default = None
impl Status { pub const fn code(self) -> (u8, u8); } // (tag, counter) for hashes

/// Abilities the engine implements; content maps `ABILITY_*` names, the rest are `Other`.
pub enum Ability { None, Overgrow, Blaze, Torrent, Swarm, RunAway, ShieldDust, KeenEye, Guts,
    EffectSpore, Synchronize, Trace, Intimidate, Static, PoisonPoint, Other }

/// Gen3 move effect as a mechanic. Content maps `EFFECT_*` (battle_move_effects.h).
pub enum MoveEffect {
    Hit,                                   // EFFECT_HIT
    StatUp { stat: Stat, stages: u8 },     // EFFECT_<STAT>_UP / _UP_2
    StatDown { stat: Stat, stages: u8 },   // EFFECT_<STAT>_DOWN / _DOWN_2
    StatDownHit(Stat),                     // EFFECT_<STAT>_DOWN_HIT (secondary, effect_chance)
    StatusHit(StatusKind),                 // EFFECT_POISON_HIT, BURN_HIT, PARALYZE_HIT, FREEZE_HIT
    Status(StatusKind),                    // EFFECT_POISON, PARALYZE, SLEEP, TOXIC
    Confuse, ConfuseHit, FlinchHit, FlinchMinimizeHit, Absorb, LeechSeed, Bide, FocusEnergy,
    NaturePower, QuickAttack, Recoil,      // Recoil = EFFECT_RECOIL (Struggle, Take Down)
    /// Raw EFFECT_* number: logged; acts as `Hit` if power > 0, else "But it failed!".
    Unsupported(u8),
}
pub enum MoveTarget { Selected, Depends, UserOrSelected, Random, BothFoes, User, FoesAndAlly, OpponentsField }
pub struct MoveFlags(pub u8);              // Gen3 FLAG_MAKES_CONTACT = 1, … (bit values as Gen3)
impl MoveFlags { pub const MAKES_CONTACT: u8 = 1; /* … KINGS_ROCK_AFFECTED = 1 << 5 */ pub const fn has(self, flag: u8) -> bool; }

pub struct LevelUpMove { pub level: u8, pub mov: MoveId }
pub struct MonGfx {
    pub front: &'static [Tile],            // 64x64, frame 0 of anim_front, OBJ 1D order (64 tiles)
    pub back: &'static [Tile],
    pub palette: &'static [u16; 16],
    pub shiny: &'static [u16; 16],
    pub front_y: u8, pub back_y: u8,       // gMonFrontPicCoords / gMonBackPicCoords y_offset
    pub elevation: u8,                     // gEnemyMonElevation
}
pub struct SpeciesInfo {
    pub name: &'static [u8],               // FR, Gen3-encoded, 0xFF-terminated
    pub base: [u8; 6],                     // Stat order (HP, Atk, Def, Speed, SpAtk, SpDef)
    pub types: [Type; 2], pub catch_rate: u8, pub exp_yield: u8, pub ev_yield: [u8; 6],
    pub gender_ratio: u8,                  // 0 male, 254 female, 255 genderless
    pub friendship: u8, pub growth: GrowthRate, pub abilities: [Ability; 2],
    pub learnset: &'static [LevelUpMove],
    pub gfx: Option<&'static MonGfx>,      // None for species the build left out
}
pub struct MoveInfo {
    pub name: &'static [u8], pub effect: MoveEffect, pub power: u8, pub ty: Type,
    pub accuracy: u8, pub pp: u8, pub effect_chance: u8, pub target: MoveTarget,
    pub priority: i8, pub flags: MoveFlags,
}
pub struct TypeMatchup { pub attack: Type, pub defend: Type, pub multiplier: u8 } // 0, 5, 20 (/10)
pub struct TrainerPic { pub tiles: &'static [Tile], pub palette: &'static [u16; 16], pub y: u8 }
pub struct TrainerMon { pub species: SpeciesId, pub level: u8, pub iv: u8, pub moves: Option<[Option<MoveId>; 4]> }
pub struct TrainerData {
    pub class_name: &'static [u8], pub name: &'static [u8], pub pic: &'static TrainerPic,
    pub female: bool,                      // gTrainers[].encounterMusic_gender & F_TRAINER_FEMALE
    pub ai: u32,                           // Gen3 AI_SCRIPT_* bits
    pub double: bool, pub party: &'static [TrainerMon],
}
pub struct WildMon { pub species: SpeciesId, pub min_level: u8, pub max_level: u8 }
pub struct LandEncounters { pub rate: u8, pub mons: [WildMon; 12] }
pub struct BattleTables {
    pub species: &'static [SpeciesInfo],   // indexed by SpeciesId
    pub moves: &'static [MoveInfo],        // indexed by MoveId
    pub type_chart: &'static [TypeMatchup],// gTypeEffectiveness order (it affects rounding)
    pub foresight_split: usize,            // entries from here on are ignored under Foresight
    pub struggle: MoveId,
    pub nature_power: [MoveId; 10],        // sNaturePowerMoves, by Environment
}
impl BattleTables {                        // panic on ids outside the tables
    pub fn species_info(&self, id: SpeciesId) -> &'static SpeciesInfo;
    pub fn move_info(&self, id: MoveId) -> &'static MoveInfo;
}
pub static EMPTY_TABLES: BattleTables;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pokemon {
    pub species: SpeciesId, pub personality: u32, pub ot_id: u32, pub nickname: [u8; NAME_LEN],
    pub level: u8, pub exp: u32, pub ivs: [u8; 6], pub evs: [u8; 6],
    pub moves: [Option<MoveId>; 4], pub pp: [u8; 4], pub pp_bonuses: u8,
    pub friendship: u8, pub ability_slot: u8, pub status: Status,
    pub hp: u16, pub stats: [u16; 6],      // CalculateMonStats cache; stats[0] = max HP
}
pub enum Ivs { Random, Fixed(u8) }         // USE_RANDOM_IVS / fixedIV
pub enum Learn { Learned(u8), AlreadyKnown, Full }
impl Pokemon {                             // bodies: Task C
    pub fn new(t: &BattleTables, species: SpeciesId, level: u8, ivs: Ivs, personality: u32,
               ot_id: u32, rng: &mut Rng) -> Pokemon;          // CreateMon + GiveMonInitialMoveset
    pub fn nature(&self) -> Nature; pub fn gender(&self, t: &BattleTables) -> Gender;
    pub fn ability(&self, t: &BattleTables) -> Ability; pub fn is_shiny(&self) -> bool;
    pub fn recalc_stats(&mut self, t: &BattleTables);         // CalculateMonStats
    pub fn heal(&mut self, t: &BattleTables);                 // HP, PP, status
    pub fn gain_evs(&mut self, t: &BattleTables, defeated: SpeciesId); // MonGainEVs, no items
    /// Adds exp up to the next level boundary; returns true if a level was gained (stats are
    /// recalculated). Callers loop, so each level-up gets its own events.
    pub fn add_exp_step(&mut self, t: &BattleTables, exp: &mut u32) -> bool;
    pub fn moves_at_level(&self, t: &BattleTables, level: u8) -> impl Iterator<Item = MoveId>; // done in Wave 0
    pub fn try_learn(&mut self, t: &BattleTables, m: MoveId) -> Learn;
    pub fn replace_move(&mut self, t: &BattleTables, slot: u8, m: MoveId);
    pub fn hash(&self, h: &mut Fnv64);                        // done in Wave 0 (every field)
}
pub fn exp_for_level(g: GrowthRate, level: u8) -> u32;      // gExperienceTables formulas (Task C)
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Party { mons: [Option<Pokemon>; PARTY_SIZE] }    // compact: no holes
impl Party {                                                // done in Wave 0
    pub fn len(&self) -> usize; pub fn is_empty(&self) -> bool; pub fn iter(&self) -> impl Iterator<Item = &Pokemon>;
    pub fn get(&self, i: usize) -> Option<&Pokemon>;
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Pokemon>;
    pub fn push(&mut self, p: Pokemon) -> Result<u8, Pokemon>;
    pub fn first_usable(&self) -> Option<u8>; pub fn all_fainted(&self) -> bool; // HP > 0; empty = all fainted
    pub fn heal(&mut self, t: &BattleTables); pub fn hash(&self, h: &mut Fnv64);
}
```

### `engine::battle` (the core/presentation boundary)
```rust
pub mod core; pub mod calc; pub mod effects; pub mod ai;          // logic (pure)
pub mod scene; pub mod ui; pub mod gfx; pub mod text; pub mod transition; // presentation
#[cfg(test)] pub(crate) mod test_data;                            // Task D fills it
pub use self::core::Battle; pub use self::scene::BattleScene;
// (inside battle/mod.rs, `core` is the submodule: write `::core::` for the core crate)

#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Side { Player, Opponent }
impl Side { pub const fn other(self) -> Side; }
#[repr(u8)] pub enum Environment { Grass, LongGrass, Sand, Underwater, Water, Pond, Mountain,
    Cave, Building, Plain }                                       // BATTLE_ENVIRONMENT_* order
pub enum BattleKind { Wild { first_battle: bool }, Trainer { trainer: TrainerId, defeat: TextId } }
pub struct BattleSetup { pub kind: BattleKind, pub opponents: Party, pub environment: Environment }
/// Gen3 B_OUTCOME_* values, so scripts can store them in VAR_RESULT.
#[repr(u8)] pub enum BattleOutcome { Won = 1, Lost = 2, Drew = 3, Ran = 4, MonFled = 6, Caught = 7 }
impl BattleOutcome { pub const fn from_u16(v: u16) -> Option<BattleOutcome>; }

pub enum Choice { Move(u8), Switch(u8), Run, Ball, LearnMove { forget: Option<u8> } }
pub enum Need { Busy, Action, Switch, LearnMove { party: u8, mov: MoveId }, Ended(BattleOutcome) }
pub enum Effectiveness { Normal, Super, NotVery, NoEffect }
pub enum MsgWait { Frames(u8), Button }    // Cmd_waitmessage: B_WAIT_TIME_* or A/B, or a prompt
/// Gen3 gBattleTextBuff1..3 contents (PREPARE_*_BUFFER).
pub enum Buff { None, Stat(Stat), Move(MoveId), Number(u32), Species(SpeciesId), Type(Type),
    Mon { side: Side, party: u8 }, Text(Str) }
pub struct Msg { pub id: Str, pub attacker: Side, pub target: Side, pub active: Side,
    pub mov: Option<MoveId>, pub buffs: [Buff; 3], pub wait: MsgWait }
/// Battle strings by decomp name (STRINGID_X -> Str::X, gText_X -> Str::X), plus indexed ones.
pub enum Str { Intro1 /* wild appeared */, IntroTrainer /* … wants to battle */, IntroMsg,
    Intro2, GoPkmn, PkmnSentOut, WhatWillPkmnDo, BattleMenu, MoveInterfacePP,
    MoveInterfaceType, UsedMove, AttackMissed, CriticalHit, SuperEffective, NotVeryEffective,
    ItDoesntAffect, PkmnFainted, GotAwaySafely, CantEscape, DontLeaveBirch, WildPkmnFled,
    PlayerDefeatedTrainer, PlayerWhiteout, PlayerWhiteout2, PkmnGainedExp, PkmnGrewToLv,
    PkmnLearnedMove, TryToLearnMove1, TryToLearnMove2, TryToLearnMove3, PkmnForgotMove,
    StopLearningMove, DidNotLearnMove, AttackerStatChanged, DefenderStatChanged, StatRose,
    StatSharplyRose, StatFell, StatHarshlyFell, StatWontIncrease, StatWontDecrease,
    ButItFailed, PkmnWasPoisoned, PkmnWasBurned, PkmnWasParalyzed, PkmnFellAsleep,
    PkmnBecameConfused, PkmnFlinched, PkmnIsConfused, PkmnHurtItself, PkmnIsParalyzed,
    PkmnFastAsleep, PkmnWokeUp, PkmnHurtByPoison, PkmnHurtByBurn, PkmnSeeded, LeechSeedDrain,
    PkmnStoringEnergy, PkmnUnleashedEnergy, GettingPumped, AbsorbDrain, PkmnHitWithRecoil,
    NoPPLeft, Struggle, Gotcha, WildPrefix, FoePrefix,
    BattleYesNoChoice /* gText_BattleYesNoChoice, F */, ConfirmStarterChoice /* gText_ConfirmStarterChoice, G */,
    TypeName(Type), StatName(Stat) }
pub enum BattleEvent {
    Msg(Msg),
    WildAppear, TrainerAppear, TrainerSlideOut, TrainerDefeated,  // pic slides
    SendOut { side: Side, party: u8 }, Recall { side: Side },
    MoveAnim { attacker: Side, mov: MoveId }, Hit { target: Side, eff: Effectiveness },
    Hp { side: Side, from: u16, to: u16 },
    StatusIcon { side: Side, status: Status }, StatAnim { side: Side, up: bool },
    Faint { side: Side },
    Exp { party: u8, from: u32, to: u32 }, LevelUp { party: u8, level: u8 },
    Ball { shakes: u8, caught: bool },
    End(BattleOutcome),
}
/// What presentation reads of a battler (Gen3 struct BattlePokemon subset). Only public fields:
/// the core keeps its private per-battler state (stages, STATUS2/3, …) in `core.rs` next to it.
pub struct BattleMon { pub party: u8, pub species: SpeciesId, pub nickname: [u8; NAME_LEN],
    pub level: u8, pub hp: u16, pub max_hp: u16, pub status: Status, pub gender: Gender,
    pub moves: [Option<MoveId>; 4], pub pp: [u8; 4], pub max_pp: [u8; 4] }
pub struct Names<'a> { pub player: &'a [u8], pub trainer_class: &'a [u8], pub trainer_name: &'a [u8],
    pub wild: bool, pub mons: [&'a BattleMon; 2] }               // index = Side
/// Tiles + palettes + optional tilemap (raw GBA screen entries, 32 per row).
pub struct GfxImage { pub tiles: &'static [Tile], pub palettes: &'static [[u16; 16]], pub tilemap: &'static [u16] }
impl GfxImage { pub const EMPTY: GfxImage; }
/// Wave 0 proposal; Task B confirms it with E/F/G (changes in their reports), Task I fills it.
pub struct BattleGfx {
    pub textbox: GfxImage, pub environments: [GfxImage; 10] /* by Environment */,
    pub healthbox_player: GfxImage, pub healthbox_opponent: GfxImage, pub hpbar: GfxImage,
    pub expbar: GfxImage, pub status: GfxImage, pub misc: GfxImage, pub numbers: GfxImage,
    pub ball: GfxImage, pub small_font: &'static Font, pub player_back: TrainerPic /* 64-tile frames */,
    pub starter_bag: GfxImage, pub starter_ball: GfxImage, pub starter_circle: GfxImage,
}
pub static EMPTY_GFX: BattleGfx;

// core.rs (Task D)
impl core::Battle {
    pub fn new(game: &impl GameData, setup: BattleSetup, player: &Party, rng: &mut Rng) -> Self;
    pub fn start(&mut self, game: &impl GameData, rng: &mut Rng, out: &mut Vec<BattleEvent>);
    pub fn need(&self) -> Need;
    pub fn choose(&mut self, game: &impl GameData, rng: &mut Rng, c: Choice, out: &mut Vec<BattleEvent>);
    pub fn mon(&self, side: Side) -> &BattleMon;
    pub fn party(&self, side: Side) -> &Party;   // Player: written back to World at the end
    pub fn setup(&self) -> &BattleSetup;
    pub fn hash(&self, h: &mut Fnv64);
}
// scene.rs (Task K): plays events, runs menus, returns the player's choice.
impl scene::BattleScene {
    pub fn new(setup: &BattleSetup) -> Self;
    /// Uploads in budgeted chunks while black; true when everything is loaded.
    pub fn load(&mut self, game: &impl GameData, battle: &Battle, f: &mut Frame) -> bool;
    pub fn push(&mut self, events: &mut Vec<BattleEvent>);
    pub fn update(&mut self, game: &impl GameData, battle: &Battle, input: &Input, f: &mut Frame) -> Option<Choice>;
    pub fn finished(&self) -> bool;          // End played and its last message dismissed
    pub fn hash(&self, h: &mut Fnv64);
}
// text.rs (Task F)
pub fn expand(game: &impl GameData, msg: &Msg, names: &Names) -> Vec<u8>;
// transition.rs (Task G)
impl transition::BattleTransition {
    pub fn new(kind: &BattleKind) -> Self;   // wild: grid squares, trainer: bars
    pub fn update(&mut self, f: &mut Frame) -> bool; pub fn hash(&self, h: &mut Fnv64);
}
// Task C / G stubs outside `battle` (signatures from their task sections):
// wild::try_land_encounter(table, prev: Behavior, cur: Behavior, lead: Option<&Pokemon>, t: &BattleTables, rng) -> Option<(SpeciesId, u8)>
// wild::create_wild(t, species, level, lead: Option<&Pokemon>, ot_id: u32, rng) -> Pokemon
// trainer::create_trainer_party(t, trainer: &TrainerData, rng) -> Party
// starter::StarterChoice { new([SpeciesId; 3]), load_step(game, f) -> bool,
//     update(game, input: &Input, f) -> Option<u8>, hash }
```

### `engine::script` additions
```rust
// Request (and hash_into tags appended in this order):
GiveMon { species: SpeciesId, level: u8 },   // Value(0 party, 1 PC, 2 can't) = Gen3 MON_GIVEN_TO_*
PartySize,                                   // Value
HealParty,
BattleWild { species: SpeciesId, level: u8, first_battle: bool }, // Value(BattleOutcome)
BattleTrainer { trainer: TrainerId, defeat: TextId },             // Value(BattleOutcome)
ChooseStarter([SpeciesId; 3]),               // Value(index 0..3)
SetRespawn(u8),
// Ctx:
pub async fn give_mon(&self, species: SpeciesId, level: u8) -> u16;
pub async fn party_size(&self) -> u16;
pub async fn heal_party(&self);
pub async fn battle_wild(&self, species: SpeciesId, level: u8, first_battle: bool) -> BattleOutcome;
pub async fn battle_trainer(&self, trainer: TrainerId, defeat: TextId) -> BattleOutcome;
pub async fn choose_starter(&self, choices: [SpeciesId; 3]) -> u8;
pub async fn set_respawn(&self, heal_location: u8);
// Runner:
/// Drops the running script and any pending request (Gen3 whiteout resets the script context).
/// `issued` is kept, as when a script ends.
pub fn abort(&mut self);
```
A battle that whites out never answers its request; the World aborts the runner.
Until Task J, the `World` host answers the new requests at once with a log line: `GiveMon` →
2 (`MON_CANT_GIVE`), `PartySize` → 0, `BattleWild`/`BattleTrainer` → `Won`, `ChooseStarter` → 0,
`HealParty`/`SetRespawn` → unit. `Request::Random` now calls `rng.random()` (same values).

### `engine::video` additions
```rust
Cmd::ObjTile { index: u16, tile: Tile }   // dynamic OBJ tile (healthboxes, bars); validate: index < 1024
Frame::lighten: bool                      // BLDY brightens instead of darkening (BLDCNT effect 2)
Frame::is_black(&self) -> bool            // brightness >= 16 && !lighten: the GBA shell's late_black
                                          // test uses it (a lighten frame at 16 is white)
pub mod layout { pub mod battle {
    // BG: (layer, char block, screen block, priority). 32x32 maps only: Gen3's 64-row BG0 trick
    // (scrolling between message / action / move views) becomes redrawing rows 15..20.
    pub const TEXT: (u8, u8, u8, u8)   = (0, 0, 24, 0);  // message box, action and move menus
    pub const WINDOW: (u8, u8, u8, u8) = (1, 1, 28, 0);  // yes/no, learn-move, level-up box
    pub const ANIM: (u8, u8, u8, u8)   = (2, 1, 30, 1);  // flashes, transition overlay (tiles 256..)
    pub const ENV: (u8, u8, u8, u8)    = (3, 2, 26, 3);  // environment background
    // BG palettes: 0..2 textbox (gBattleTextboxPalette), 2..5 environment, 5 menu text
    // (text_pp.pal colors swapped in for PP), 15 windows on BG1.
    // OBJ tiles (1D, 1024): enemy pic 0, player pic 64, opponent trainer 128, player trainer
    // back 192, player healthbox 256 (2x 64x32 = 64), enemy healthbox 320, HP bars 384
    // (2x 32x8), ball 400 (16x16), anim 448..512; 512.. free (transition, starter screen).
    // OBJ palettes: 0 enemy mon, 1 player mon, 2 trainer, 3 player back, 4 healthbox,
    // 5 HP bar, 6 ball, 7..9 anim tints.
    pub const OBJ_PRIORITY_MON: u8 = 2; pub const OBJ_PRIORITY_BOX: u8 = 1;
} }
```

### `extract::schema` v3 additions (`FORMAT_VERSION = 3`)
`FORMAT_VERSION` is already 3 (the export test checks 3): every checkout must re-run `cargo xtask extract`.
Rust types in `schema.rs`: `SpeciesFile`/`Species`, `MovesFile`/`Move`, `TypesFile`, `TrainersFile`/`Trainer`/`TrainerMon`,
`EncountersFile`/`LandEncounters`/`WildMon`, `HealLocationsFile` and `BattleStringsFile` (aliases), `MonCoordsFile`/`MonCoords`
(`front_y`, `back_y`, `elevation`), `TrainerPicsFile { front: BTreeMap<TRAINER_PIC_*, TrainerPicInfo { y }>, back: BTreeMap<TRAINER_BACK_PIC_*, frames: u8> }`,
and one generic `GfxFile { tile_count, width, palettes, palette_bank, tilemap: Vec<Row> }` for `battle/gfx`, `battle/env` and `starter` (Task B may extend).
```text
battle/species.ron     SpeciesFile { species: Vec<Species> }   Gen3 index order, key SPECIES_*
battle/moves.ron       MovesFile { moves: Vec<Move>, nature_power: Vec<String> }
battle/types.ron       TypesFile { names: Vec<String> (FR), chart: Vec<(String, String, u8)>, foresight_split: usize }
battle/trainers.ron    TrainersFile { trainers: Vec<Trainer> }  all trainers, key TRAINER_*
battle/encounters.ron  EncountersFile { maps: BTreeMap<map, LandEncounters> }  exported maps only
battle/heal_locations.ron  BTreeMap<HEAL_LOCATION_*, (map, x, y)>
battle/strings.ron     BTreeMap<STRINGID_* | gText_*, String>   decomp `.string` syntax, FR
pokemon/<SPECIES>/front.png, back.png (own palette), shiny.pal; pokemon/coords.ron
trainers/front/<TRAINER_PIC>.png, trainers/back/<TRAINER_BACK_PIC>.png (frames side by side), trainers/pics.ron
battle/gfx/<name>.{png,ron}    textbox (+tilemap), healthbox_*, hpbar, expbar, status, ball, misc
battle/env/<Environment>.{png,ron}   tiles, 32x? tilemap, palettes (Grass, Plain at least)
font/latin_small.{png,ron}     FontFile
starter/{bag,ball,circle}.{png,ron}
```
`Species { id, name, base: [u8; 6], types: [String; 2], catch_rate, exp_yield, ev_yield: [u8; 6], gender_ratio, friendship, growth, abilities: [String; 2], learnset: Vec<(u8, String)> }`.
`Move { id, name, effect /* EFFECT_* */, power, ty, accuracy, pp, chance, target /* MOVE_TARGET_* */, priority: i8, flags: Vec<String> }`.
`Trainer { id, class /* TRAINER_CLASS_* */, class_name, name, pic, female, ai: Vec<String>, double, party: Vec<TrainerMon { iv: u16, level, species, item: Option<String>, moves: Option<Vec<String>> }> }`.

### Wave 0 as built (deviations from the first draft of this section)
1. `BattleMon` has no core-private fields: nobody owns `battle/mod.rs` in Wave 1, so Task D keeps that state in `core.rs` (e.g. a `Battler { mon: BattleMon, … }`).
2. `BattleGfx` has a concrete proposed field list built on a new `GfxImage`; it is not `Debug` (`Font` is not).
3. `Str` gains `BattleYesNoChoice` and `ConfirmStarterChoice` (needed by F and G; avoids edits to `battle/mod.rs`).
4. Additions: `Type::ALL`/`from_u8`, `Stat::ALL`, `Nature::effect`, `Status::code`, `MoveFlags` consts/`has`, `BattleTables::species_info`/`move_info`, `Party::is_empty`/`iter`, `Side::other`, `BattleOutcome::from_u16`, `Frame::is_black`, `GfxImage`.
5. `Pokemon::moves_at_level`, `Pokemon::hash` and the `Party` methods are implemented in Wave 0 (`todo!` with an `impl Iterator` return does not type-check; the rest are mechanical). Task C keeps every other `Pokemon` body and `exp_for_level`.
6. `wild.rs`, `trainer.rs`, `starter.rs`, `battle/transition.rs` (`BattleTransition::new(&BattleKind)`) and `battle/text.rs` (`expand`) are stubbed from their task sections, and every new module is registered, so Wave 1 never edits `lib.rs`/`battle/mod.rs` to add a file. `calc.rs`, `effects.rs`, `ai.rs`, `gfx.rs`, `ui.rs` and `test_data.rs` are doc-only: their signatures are the owning task's to define.
7. `GameData` defaults write `let _ = arg;` (unused parameters); behaviour as specified.
8. The GBA shell's black-screen test now uses `Frame::is_black` (a `lighten` frame at BLDY 16 is white, not black); `BLDCNT_LIGHTEN_ALL = 0x3F | 2 << 6` is asserted next to the darken constant.
9. Battle OBJ layout (Task E, `battle::gfx::obj`, mirrored in the `video::layout::battle` comment): the player healthbox takes two 64x64 halves as Gen3 (its exp bar row is below 32 px), so player box 256..384, enemy box 384..448, HP bars 448 / 456 (8 tiles each), ball 464, anim tiles 468..512. OBJ palettes: 4 player box, 7 opponent box (its status color lives in the box palette), anim tints 8..10. A visible frame uploads at most 16 OBJ tiles in all (`battle::gfx::OBJ_TILES_PER_FRAME`).

---

## Rules for every task

- **Workspace:** work in your own git worktree. Commit there with `feat(<crate>): …` or `fix(…)`, and **no attribution or co-author lines**.
- **TDD:** write a failing test first. Engine modules use synthetic data in unit tests: a small `BattleTables`, at most a few species and moves, in a shared `battle/test_data.rs` that Task D creates and others may extend. ROM-backed tests use `extract::fixtures` (both ROMs where there is a decomp oracle).
- **Gen3 goldens:** every formula test pins values computed **by hand or by a throwaway script from the decomp formula**, never by running our code. The test comment cites the decomp function and the inputs.
- **Determinism:** no floats, clocks, `HashMap` iteration or other host-dependent behaviour in `engine` and `content`. Randomness comes only from the `World` RNG, passed in as `&mut Rng`. Every Gen3 `Random()` becomes one `rng.random()` at the same point in the sequence; `Random32()` becomes `random32()`.
- **`engine` stays game-agnostic.** No species, move, trainer, item, ability or map numbers and no Hoenn names. Gen3 *mechanics* are fine: formulas, `B_OUTCOME_*` values, `Str` message kinds, AI flag bits, the slot thresholds of `ChooseWildMonIndex_Land`.
- **Never commit ROM-derived data.** `content/extracted/` stays git-ignored. Decomp identifiers (constant names) are fine.
- **GBA budget:** a step must finish inside one frame, so the parity `late` counter must stay 0. Bulk uploads (environment tiles, pics, field tilesets on return) only happen while the screen is fully black. They are spread so that one battle costs at most one `late_black` apply at entry and one at exit; the parity test checks `late_black <= warps + 2 × battles`. While the picture is visible, keep each frame's uploads to a few tiles: one glyph tile, healthbox or bar tiles through `Cmd::ObjTile`, a menu redraw.
- **Hash:** every new piece of state that affects future frames goes into the owning module's `hash`, cheaply enough to run every frame on the GBA.
- **Before committing:**
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`, with env `PKR_ROM_FR=/Users/dode/Documents/pokemon/pokemon-rust/rom/emeraude-fr.gba PKR_ROM_US=/Users/dode/Documents/pokemon/emerald.gba PKR_DECOMP=/Users/dode/Documents/pokemon/pokeemerald`
  - `cargo xtask rom`, if you touched anything the GBA build compiles (engine, content, game-gba). The GBA build must keep compiling.
- **Ownership:** only touch the files your task owns, plus new files in your module. If you need a change elsewhere, describe it in your report instead of making it.
- **Decomp:** the decomp at `PKR_DECOMP` is the reference for Gen3 behaviour. Mirror it where the task says "as Gen3", and cite the function. The parity contract is only between our GBA build and our native build, not with the original game.

---

## Wave 1 (parallel, 7 agents)

### Task A — extract: battle data and French battle strings

**Owns:** `crates/extract/src/{battle_data.rs (new), names/battle.rs (additions)}`, `export.rs` (the battle-data hook only), `tests/battle_data.rs`.

1. **Names:** add `MOVES`, `ABILITIES`, `MOVE_EFFECTS` (`battle_move_effects.h`), `TYPES`, `TRAINERS` (`opponents.h`), `TRAINER_CLASSES`, `STRING_IDS` (`battle_string_ids.h`), `GROWTH_RATES`, `MOVE_TARGETS`, `AI_FLAGS`, `HEAL_LOCATIONS` and `BATTLE_ENVIRONMENTS`. Each gets a drift test against the decomp, as the M3 tables have.
2. **Locate the tables** on both ROMs, using the US decomp as the oracle. Anchor on language-independent bytes, and use structural signatures where the text differs:
   - `gSpeciesInfo` (28-byte entries), `gBattleMoves` (12 bytes), `gTypeEffectiveness` (triples ending 0xFF, with 0xFE for Foresight), `gExperienceTables` (a check only)
   - `gLevelUpLearnsetPointers` / learnsets (`LEVEL_UP_MOVE` u16 = level<<9 | move, 0xFFFF end)
   - `gTrainers` (40 bytes, with party pointers) and the four party layouts (`TrainerMon*`)
   - `gWildMonHeaders` (the slice maps' land tables), `sNaturePowerMoves`, `sHealLocations`
   - FR names: `gSpeciesNames` (11 bytes; entry 0 is `??????????`), `gMoveNames` (13), `gTypeNames` (7), `gTrainerClassNames` (13), and trainer names inside `gTrainers`

   Code tables can be found in FR by matching the US function bytes with their literal-pool words masked, then reading the literal.
3. **Strings:** `gBattleStringsTable` (pointer table indexed `STRINGID - BATTLESTRINGS_TABLE_START`, `battle_message.c`). Add the named `gText_*` strings the slice shows:
   - `gText_BattleMenu`, `gText_WhatWillPkmnDo`, `gText_MoveInterfacePP`, `gText_MoveInterfaceType`
   - `gText_BattleYesNoChoice`, `gText_StatRose` / `StatSharply` / `StatFell` / `StatHarshly` …, `gStatNamesTable`
   - the wild and foe prefixes (`sText_WildPkmnPrefix`, `sText_FoePkmnPrefix`)

   Write them as decomp-syntax strings with the FR charmap. **Check whether BPEF puts the "sauvage/ennemi" tag before or after the name.** Report how the FR ROM builds "{name} sauvage", and keep the bytes needed to reproduce it.
4. **Export** the v3 battle files listed in Wave 0: every species, move and trainer, and the encounters of the exported maps. Learnsets go into `Species`. The manifest keeps provenance offsets.
5. **Tests:**
   - US: values match the decomp sources for Treecko, Mudkip, Zigzagoon and Poochyena (stats, types, abilities, learnsets), Tackle, Ember, Growl and Struggle, the type chart length and order, `TRAINER_MAY_ROUTE_103_MUDKIP` and `TRAINER_CALVIN_1`, Route 101's land table (rate 20, 12 slots), and `STRINGID_PKMNGAINEDEXP`.
   - FR: the same numbers, plus names that decode and re-encode to their ROM bytes (`TORCHIC` → its FR name).
   - Export is deterministic, and the round-trip into the schema types passes.

### Task B — extract: battle graphics, small font, starter screen

**Owns:** `crates/extract/src/{battle_gfx.rs (new), font.rs (small font addition)}`, `export.rs` (the battle-gfx hook only), `tests/battle_gfx.rs`.

1. **LZ77 tables:** `gMonFrontPicTable` / `gMonBackPicTable` (`CompressedSpriteSheet`, size 0x800, tag = species: a strong signature on both ROMs), `gMonPaletteTable` / `gMonShinyPaletteTable`, `gMonFrontPicCoords` / `gMonBackPicCoords`, `gEnemyMonElevation`, `gTrainerFrontPicTable` + palettes, and `gTrainerBackPicTable` (Brendan's and May's back frames).
   - Decompress with `lz77.rs`, and anchor each table on the decomp files converted to 4bpp.
   - Export all species. Front pics keep frame 0 of the 64×128 `anim_front`.
2. **Battle interface** (`graphics/battle_interface/*`, `battle_bg.c`):
   - `gBattleTextboxTiles` / `Tilemap` / `Palette`
   - `healthbox_singles_player`, `healthbox_singles_opponent`, `hpbar`, `expbar`, `status*`, `misc` (the level glyph, gender symbols), `numbers*`
   - the HP bar palette and the Poké Ball sprite used for send-out (`gBallGfx_Poke` + palette)

   Status icons and any text drawn into tiles come **from the FR ROM**, since they may be localized.
3. **Environments:** `gBattleEnvironmentTiles_*` / `Tilemap_*` / `Palette_*` for `TallGrass` (Grass) and `Plain` at least (`battle_bg.c` `sBattleEnvironmentTable`). The entry anim tiles are optional.
4. **Small font:** `gFontSmallLatinGlyphs` + widths (`FONT_SMALL`, used by the healthbox nickname), written as `font/latin_small`.
5. **Starter screen:** `graphics/starter_choose/{tiles.png, birch_bag.bin, birch_grass.bin, pokeball_selection.png, starter_circle.png}` and their palettes (`starter_choose.c`).
6. **Fix `BattleGfx`'s field list** in a short design note at the top of `battle_gfx.rs`, matched to what E, F and G need. List it in the report so the Wave 2 content build fills it.
7. **Tests:**
   - US: decompressed bytes equal the decomp PNGs converted to 4bpp, for Treecko front and back, the textbox, the healthboxes and TallGrass.
   - FR: the same tables are found and the counts match.
   - The export round-trip passes, and the run is deterministic.

### Task C — engine: Pokémon model, wild encounters, trainer parties

**Owns:** `crates/engine/src/{mon.rs (fn bodies), wild.rs (new), trainer.rs (new)}`.

1. **`Pokemon::new`**, as Gen3 `CreateMon` + `CreateBoxMon` + `GiveMonInitialMoveset` (`pokemon.c`):
   - With `Ivs::Random`, two `random()` calls give HP/Atk/Def, then Spe/SpA/SpD, 5 bits each.
   - The nickname is the species name; friendship comes from the species; `ability_slot = personality & 1` (when the second ability is `None`, slot 0 is used, as `GetAbilityBySpecies`); exp is the level's minimum.
   - Initial moveset: walk the learnset up to `level`, skip known moves, and shift out the first move when full.
2. **Stats:** `recalc_stats` as `CalculateMonStats` / `CALC_STAT`, with `ModifyStatByNature` (×110/100, ×90/100). The current HP follows max HP as Gen3 does.
3. **Derived values:**
   - `nature = personality % 25`
   - `gender` by `GetGenderFromSpeciesAndPersonality`
   - `is_shiny` with `ot_id` as `IsShinyOtIdPersonality`
4. **Exp:**
   - `exp_for_level` for all six growth rates (`experience_tables.h` macros; MediumSlow L1 is clamped as in the table).
   - `add_exp_step` (Gen3 `Cmd_getexp` level loop + `CalculateMonStats`).
   - `moves_at_level`, `try_learn` (`MonTryLearningNewMove`), `replace_move` (PP reset).
   - `gain_evs` (`MonGainEVs`: the 510 total and 255 per-stat caps, no items).
5. **`wild.rs`**, as Gen3 (`wild_encounter.c`):
   - `fn try_land_encounter(table: &LandEncounters, prev: Behavior, cur: Behavior, lead: Option<&Pokemon>, t, rng) -> Option<(SpeciesId, u8)>`: the new-metatile 60% gate (`AllowWildCheckOnNewMetatile`), `WildEncounterCheck` (rate × 16, the lead's ability modifiers for the abilities in the enum, cap 2880, `random() % 2880`), the slot by `ChooseWildMonIndex_Land` (20/20/10/10/10/10/5/5/4/4/1/1), and the level by `ChooseWildMonLevel`. There is no repel, flute or roamer.
   - `fn create_wild(t, species, level, lead, ot_id, rng) -> Pokemon`: `CreateWildMon` (`PickWildMonNature` with the Synchronize lead rule, then `CreateMonWithNature`, which rerolls `random32()` until `personality % 25 == nature`; random IVs).
6. **`trainer.rs`:** `fn create_trainer_party(t, trainer: &TrainerData, rng) -> Party`, as `CreateNPCTrainerParty`:
   - name hash over the trainer name and the species name bytes
   - personality 0x80, 0x78 or 0x88 (by gender ratio and trainer gender) + `hash << 8`
   - `fixedIV = iv * 31 / 255`, OT id from `random32()`
   - the custom movesets
7. **Tests**, all hand-computed from the decomp formulas:
   - Mudkip L5, IVs all 10, Adamant (3), no EVs → stats `[20, 13, 10, 9, 9, 10]` (Speed 9, SpA 9).
   - Zigzagoon L2, IVs 0, Hardy → HP 13, Def 6.
   - `exp_for_level`: MediumSlow 5, 6, 7, 8 → 135, 179, 236, 314; MediumFast 2/3/5 → 8/27/125; plus one Erratic and one Fluctuating value from the table. With `PKR_DECOMP` set, compare every level of every rate against `gExperienceTables` evaluated by a test-side parser.
   - Level-up: Mudkip at 135 exp gaining 50 reaches L6 with recalculated stats and HP offset kept, and learns Mud-Slap through `moves_at_level`.
   - Initial moveset with more than 4 candidates.
   - EV caps, nature table, gender thresholds.
   - Slot selection at every boundary (`random() % 100` = 19/20/39/40/…/98/99).
   - Rate gate: rate 20 → 320 / 2880.
   - Determinism of `create_wild` for a seed.
   - A trainer party's personality for a synthetic name, IV scaling (255 → 31, 0 → 0).

### Task D — engine: battle core (turn engine and damage pipeline)

**Owns:** `crates/engine/src/battle/{core.rs, core/ (new), calc.rs, test_data.rs (new)}`.

Gen3 references: `battle_main.c` (`GetWhoStrikesFirst`, `SetActionsAndBattlersTurnOrder`, `HandleEndTurn_*`), `battle_script_commands.c`, `battle_util.c`, `data/battle_scripts_1.s`.

1. **`calc.rs`**, pure functions that take explicit random values:
   - `base_damage(attacker, defender, power, ty, crit, ...)`, as `CalculateBaseDamage`: stage ratios `gStatStageRatios`; crit ignores the attacker's drops and the defender's boosts; burn halves physical damage (not with Guts); Overgrow/Blaze/Torrent/Swarm ×150/100 power at HP ≤ max/3; Guts; the minimum 1; then +2. `Mystery` deals 0.
   - Then `× crit (2)`, STAB `×15/10`, type multipliers applied **per chart entry in table order** (`ModulateDmgByType`, minimum 1 unless 0), and random `×(100 - r%16)/100` (`ApplyRandomDmgMultiplier`, minimum 1). This is the `damagecalc → typecalc → adjustnormaldamage` order.
   - `crit_chance_stage` (`sCriticalHitChance` {16, 8, 4, 3, 2}; Focus Energy +2; none in a first battle)
   - `accuracy_hits(move_acc, acc_stage, eva_stage, roll)` (`Cmd_accuracycheck`: `sAccuracyStageRatios`, `random() % 100 + 1 > calc` misses; Keen Eye is handled by the stat-change code)
   - `exp_yield(defeated_species, level, trainer, participants)` (`Cmd_getexp`: `expYield * level / 7`, ×150/100 for trainers, split among the participants, minimum 1)
   - `run_succeeds(speed, foe_speed, run_tries, roll)` (`TryRunFromBattle`: faster or equal always flees; else `speedVar` is a **u8**: `speed*128/foe + tries*30`, wrapping, `> random() & 0xFF`; Run Away always flees)
   - `catch_shakes(...)` (`Cmd_handleballthrow`, Poké Ball only: odds, status ×2 / ×15/10, `Sqrt(Sqrt(16711680/odds))`, 4 shakes `random() < odds`)
2. **`Battle`** state: two `BattleMon`s plus core-private fields (stat stages, the Gen3 STATUS2/STATUS3 subset, Bide, Leech Seed, Focus Energy, confusion turns, flinch, the last move), both parties, run tries, turn counter, exp participants (`sentIn` bits), the pending queue, and `Need`.
3. **Flow:**
   - **Intro:** wild → `WildAppear`, `Msg(Intro1)`, `SendOut(Player)` with `GoPkmn`. Trainer → `TrainerAppear`, `IntroTrainer`, opponent `SendOut` + `PkmnSentOut`, then the player's.
   - **Turn:** actions (Run and Switch before moves), move order by priority, then speed with stages and paralysis /4, and ties by `random() & 1` (`GetWhoStrikesFirst`). The opponent's choice comes from `ai.rs`, called through `ai::choose(battle, rng)`; Task D ships a wild-random stub (`OpponentHandleChooseMove`: `random() & 3` until a non-empty slot with PP). E2 replaces it for trainers.
   - **Move execution** (`BattleScript_EffectHit` order): `UsedMove`, the PP decrement, accuracy, crit, damage, `MoveAnim`/`Hit`/`Hp`, then the crit and effectiveness messages, then faint. Struggle when no PP is left (`NoPPLeft` …).
   - **Effects in D:** `Hit`, `QuickAttack` (priority comes from data), `StatUp` / `StatDown` (stages clamp ±6 with `StatWontIncrease`/`Decrease`, messages with `Buff::Stat`), `Recoil`. Every other effect is dispatched through `effects::apply(...)`, a stub in D that falls back to `Unsupported` handling; Task E2 fills it in.
   - **Faint:** `Faint` + `PkmnFainted`. Exp goes to every participant still standing: `PkmnGainedExp` (`Buff::Number`), then `Exp`, then per level `LevelUp` + `PkmnGrewToLv`, then learnset moves. A full moveset raises `Need::LearnMove`, answered by `Choice::LearnMove`, with the `TryToLearnMove*` / `PkmnForgotMove` / `DidNotLearnMove` messages.
   - **Opponent switch:** after a faint, the next usable party member, in party order (the documented simplification of `GetMostSuitableMonToSwitchInto`).
   - **Player switch:** `Need::Switch` when another mon can fight; the UI auto-picks the first usable one.
   - **End:**
     - all opponents fainted → `Won`; a trainer slides back in, then `PlayerDefeatedTrainer` and the defeat text (the `BattleKind` `TextId`, printed as a message with `MsgWait::Button`).
     - all player mons fainted → `Lost` with `PlayerWhiteout`/`2`.
     - `Run` → `GotAwaySafely` / `CantEscape` (trainer) / `DontLeaveBirch` (first battle); `Ran` ends.
4. **`MsgWait`:** follow the `waitmessage` argument of the matching battle script (`B_WAIT_TIME_LONG` = 64 by default); prompts and the end-of-battle texts use `Button`.
5. **Tests**, golden and hand-computed:
   - **Damage:** Mudkip L5 Atk 13 Tackle vs Zigzagoon L2 Def 6 → base 8. Rolls r=0 → 8, r=15 → 6. Crit → 16 / 13.
   - **STAB + super effective:** Torchic L5 (SpA 13: base 70, IV 31, Hardy) Ember vs Wurmple L3 (SpD 7: base 30, IV 7) → base 7, STAB → 10, ×2 → 20, rolls → 20 / 17.
   - **Stages:** Growl −1 on the attacker (Atk 13 → 13×10/15 = 8) changes base damage as computed by hand. Crit with a −1 attacker uses the raw stat.
   - **Effectiveness rounding:** a dual-type defender where entry order changes the result (pick a pair from the chart and compute both orders).
   - **Exp:** wild Zigzagoon L2 → 17; trainer Poochyena L5 → 39 → 58.
   - **Run:** speeds 9 vs 10, tries 0 → `speedVar` 115 (succeeds iff `random() & 0xFF` < 115); tries 5 → (115 + 150) mod 256 = 9; faster always flees.
   - **Catch:** Zigzagoon catch 255, 13/13 HP → odds 85 → `1048560 / Sqrt(Sqrt(196608))` = 49931.
   - **Turn order:** priority beats speed, speed ties use `random() & 1`, paralysis quarters speed.
   - **Struggle when no PP.**
   - **Full scripted battles** on synthetic tables with a fixed seed: the exact event list for a wild battle won in 2 turns, a run, a loss, a level-up with a learn-move prompt (both answers), and a 2-mon trainer battle with the opponent switching.
   - Determinism: same seed, same events and hash.

### Task E — engine: battle visuals (background, sprites, healthboxes, bars, animations)

**Owns:** `crates/engine/src/battle/gfx.rs` (+ `battle/gfx/` dir).

Components, each with `hash` and a per-frame `update(&mut self, f: &mut Frame)`, drawn with `layout::battle`:
1. **`Background`:** loads an `Environment`'s tiles, map and palettes onto ENV (Gen3 `DrawMainBattleBackground`). It loads in chunks: `load_step(&mut self, game, f) -> bool`, with each chunk inside the black-frame budget.
2. **`MonSprite` / `TrainerSprite`:** 64×64 OBJs from `MonGfx` / `TrainerPic`, placed as Gen3 does:
   - enemy at (176, 40 + front_y − elevation), player back at (72, 80 + back_y): see `GetBattlerSpriteCoord`, `sBattlerCoords`
   - uploads with `Cmd::ObjTiles` + `ObjPalette`
   - animations: slide in, Gen3 intro direction and speed approximated (`battle_intro.c`, `BattleIntroSlide*`); appear from the ball after a 16×16 ball OBJ pops (no affine scaling: documented); faint (slide down and vanish, as `AnimFaintOpponent`/`Player`); hit blink (Gen3 `DoHitAnimBlinkSpriteEffect`: visibility toggled every 4 frames for 32 frames); lunge (attacker moves 8 px toward the target and back); status/stat tint (an OBJ palette swap toward red/blue over 16 frames and back)
   - player trainer back: frame 0 slides out left after `GoPkmn`
3. **`Healthbox`:** two 64×32 OBJs per side (`battle_interface.c` `CreateBattlerHealthboxSprites`, singles), built from the extracted healthbox tiles:
   - nickname in the small font, gender symbol, level, HP numbers for the player, status icon
   - updated through `Cmd::ObjTile` for changed tiles only
   - slides in as Gen3; the player box bobs while the action menu is up (`SpriteCB_HealthBoxOther`, optional)
4. **`HpBar`:** 48 px, colors green/yellow/red at >50% / >20% (`GetHPBarLevel`). Animated toward a target value at Gen3 speed (`MoveBattleBar` / `CalcNewBarValue`, scale 48); `done()`.
5. **`ExpBar`:** 64 px, same model (`B_EXPBAR_PIXELS`); fills toward a target and wraps at a level-up.
6. **Tests:**
   - tile and palette uploads and OBJ placement for a known setup
   - HP bar pixel sequence frame by frame for 20 → 6 of 20, and its color changes
   - exp bar fill across a level
   - healthbox tile diff (a single HP change uploads only the changed tiles)
   - every animation's frame count and final position
   - per-frame `ObjTile` count ≤ 16
   - background chunking

### Task F — engine: battle text, windows and menus

**Owns:** `crates/engine/src/{battle/ui.rs (+ battle/ui/), battle/text.rs, text.rs (printer extraction only)}`.

1. **Shared printer:** pull the glyph canvas and printer state machine out of `TextBox` into a reusable `text::Printer` (window rectangle, canvas base, colors, owned `Vec<u8>` text, speed, A/B speed-up, `\p` / `\l` handling). `TextBox` uses it with **unchanged behaviour and unchanged hash bytes**: the M3 text tests and World goldens must stay green. `Window` gets a `const fn new`. The small font is supported via a `&Font` parameter.
2. **`text::expand(game, msg: &Msg, names: &Names) -> Vec<u8>`**, as Gen3 `BattleStringExpandPlaceholders` (`battle_message.c`), with the `B_TXT_*` FD codes the `Str`s use:
   - the `*_NAME_WITH_PREFIX` forms (wild/foe tag as found in Task A's report; keep the prefix and suffix bytes in content), the trainer class and name, the player name, the current move
   - `B_BUFF1..3` from `Buff`, with `Str::StatName`/`TypeName`/numbers as `ExpandBattleTextBuffPlaceholders`
3. **`BattleWindows`:**
   - message box: B_WIN_MSG (2, 15, 26×4), battle text colors (`sTextOnWindowsInfo_Normal`); textbox frame drawn from `gBattleTextboxTilemap` rows 0..20
   - action prompt "Que doit faire X ?" (B_WIN_ACTION_PROMPT) + action menu (B_WIN_ACTION_MENU, `gText_BattleMenu` with `{CLEAR_TO}`, 2×2 cursor, as `HandleInputChooseAction`)
   - move menu: 4 names (B_WIN_MOVE_NAME_1..4), PP "cur/max" with the Gen3 PP color levels (`text_pp.pal`, `GetCurrentPpToMaxPpState`), "TYPE/" + type name; cursor and B to go back (`HandleInputChooseMove`)
   - yes/no on BG1 (`gText_BattleYesNoChoice`)
   - learn-move: reuse the move menu with the new move shown as a fifth line; B = don't learn
   - optional level-up stats box (B_WIN_LEVEL_UP_BANNER, `DrawLevelUpWindow1/2`)
   - every redraw rewrites rows 15..20 of TEXT (no 64-row BG)
   - API: `load`, `print(Vec<u8>, wait)`, `is_printing`, `update(game, input, f)`, `action_menu(name)`, `move_menu(&BattleMon)`, `yes_no()`, and results `action_result`, `move_result`, `yes_no_result`.
4. **Tests:**
   - expansion goldens on synthetic strings for every placeholder used, including a stat message with two buffs
   - printer timing and \p behaviour shared with M3 (existing tests unchanged)
   - menu cursor logic, including blocked moves (0 PP → `NoPPLeft` is the core's job; the menu still allows the choice, as Gen3)
   - the exact tile rows drawn for each view
   - PP color thresholds

### Task G — battle transition, starter screen, shell video additions

**Owns:** `crates/engine/src/{battle/transition.rs, starter.rs (new)}`, `crates/game-native/src/ppu.rs`, `crates/game-gba/src/video.rs`.

1. **Shells** (done in Wave 0, with PPU unit tests; G checks them on the ROM through parity once the transition uses them):
   - `Cmd::ObjTile` in both shells.
   - `Frame::lighten`: BLDCNT effect 2 brightness increase, `c + (((31 - c) * evy) >> 4)`, in the native PPU and on GBA. Native mirrors mGBA's `_brighten` rounding (`ppu::brighten`).
2. **`BattleTransition`:**
   - Intro flashes: Gen3 `TransitionIntro_FadeToGray`/`FadeFromGray`, three times (`CreateIntroTask(0, 0, 3, 2, 2)`), done with `lighten` + BLDY.
   - Then a tile-based pattern on BG0 (screen block and tiles in char block 3's free area, or reusing TEXT):
     - `GRID_SQUARES`: squares shrinking per tile over a black backdrop (`Task_GridSquares`), for every wild battle.
     - A horizontal-bars sweep for trainers.
     - Gen3's HBlank/window transitions (`SLICE`, `WHITE_BARS_FADE`, `BLUR` for the first battle) are approximated this way and the choice is documented.
   - Ends fully black (`brightness = 16`). `update(&mut self, f) -> bool` returns done; hash.
   - It must not touch field VRAM before the screen is black, except BG0 (the text layer) and its palette bank 15.
3. **`StarterChoice`** (`starter_choose.c`):
   - bag background (`birch_bag.bin` / `birch_grass.bin`) and 3 Poké Ball OBJs at the Gen3 positions
   - a hand or arrow cursor (left/right)
   - A shows the circle with the mon's front pic and "TREECKO / POKéMON <category>?" — the category is optional — then yes/no "Do you choose this POKéMON?" (FR from Task A strings)
   - B or no returns to the choice
   - `new([SpeciesId; 3])`, `load_step`, `update(game, input, f) -> Option<u8>`, `hash`
   - on "yes" `update` returns `Some(index)` on that frame with the screen still visible (no fade), as Gen3: `CB2_GiveStarter` then starts the first battle's transition over the starter screen
4. **Tests:**
   - `lighten` pixels in the PPU
   - `ObjTile` applied in both shells (GBA shell: a host-side unit test of the applier, as M3)
   - transition frame count and ends black
   - starter cursor and confirmation flow
   - no field VRAM writes before black

---

## Wave 2 (parallel, after Wave 1 is merged)

### Task E2 — move effects, statuses, abilities, AI, catching

**Owns:** `crates/engine/src/battle/{effects.rs, ai.rs}`, plus additions in `core/` coordinated through the `effects::apply` / `ai::choose` seams. Changes to D's files go in the report.

1. **Effects**, for every level-up move the slice species learn up to Lv15, plus the trainer parties: Absorb, Mud-Slap, Sand-Attack, Growl, Leer, Tail Whip, Howl, Harden, Growth, Double Team, String Shot, Ember, Poison Sting, Stun Spore, Supersonic, Confusion, Headbutt, Bite, Astonish, Focus Energy, Leech Seed, Bide, Nature Power (Grass → Stun Spore, Plain → Swift, via `nature_power`), Quick Attack, Struggle. Each follows its `BattleScript_Effect*` and `Cmd_seteffect*` in Gen3 order:
   - secondary chance `random() % 100 < chance`, blocked by Shield Dust
   - immunities: Poison types vs poison, Fire vs burn
   - "already" failures
   - Bide's 2 turns then double damage; Absorb/Leech Seed drain (half, minimum 1)
2. **Statuses:**
   - sleep: 2–5 turns (`random() & 3) + 2`); paralysis 25% full para; poison and burn 1/8 at end of turn
   - confusion: 2–5 turns, 50% self-hit as a typeless 40-power physical hit
   - flinch
   - end-of-turn order as `DoBattlerEndTurnEffects` (Leech Seed, poison, burn)
   - status icons through `StatusIcon`
3. **Abilities** in the slice:
   - Overgrow/Blaze/Torrent/Swarm (in calc), Run Away (in run)
   - Shield Dust; Keen Eye (accuracy drops blocked); Guts
   - Effect Spore (contact, 10%: poison, paralysis or sleep as `ABILITYEFFECT_CONTACT`); Static/Poison Point (cheap, same path)
   - Synchronize (status passing back); Trace (copies on send-out)
   - Intimidate (on send-out, Atk −1). None of the slice mons has it, but it is cheap and testable.
   - Pickup, weather abilities and Early Bird have no effect in the slice: documented.
4. **AI** (`battle_ai_script_commands.c`, `data/battle_ai_scripts.s`), a documented simplification of the Gen3 score model:
   - every usable move starts at 100
   - `AI_CHECK_BAD_MOVE` subset: no-effect type −10, status on already-statused −10, stat change at the cap −10, confusing a confused target −5, Leech Seed on Grass or seeded −10
   - `AI_TRY_TO_FAINT`: a KO with the slot's `simulatedRNG` roll (`100 - Random() % 16`, drawn per slot by `BattleAI_SetupAIData`, not the max roll) scores +4, Quick Attack +2 more; "not most powerful" −1; ×4 effectiveness +2 unless `if_random_less_than 80`
   - `AI_SETUP_FIRST_TURN`: on turn 0, +2 for a setup effect unless `if_random_less_than 80`
   - trainer battles ask `AI_TrySwitchOrUseItem` / `ShouldSwitch` first (RNG-relevant subset; no items)
   - `AI_CHECK_VIABILITY`: only the entries for the slice's effects
   - pick the highest score, with ties broken by `random()` among the best (as `BattleAI_ChooseMoveOrAction`)
   - first battle: `AI_FirstBattle`, flee when the target's HP is ≤ 20% → `MonFled` + `WildPkmnFled` (ordered by speed: only the player's run goes first)
   - battle setup draws `BattleAI_HandleItemUseBeforeAISetup`'s four rolls before the held-item roll and `gRandomTurnNumber`
   - wild: random, as Task D
5. **Catching:** `Choice::Ball` in the core: `Ball { shakes, caught }`, `Gotcha`, then the party (or "no room": logged, no PC). No UI entry yet, since the bag is out of scope; covered by core tests.
6. **Tests:**
   - each effect on synthetic tables with pinned seeds (event lists)
   - secondary-chance boundaries
   - Shield Dust blocking
   - sleep counter
   - confusion self-hit damage (hand-computed)
   - poison and burn 1/8 with a minimum 1
   - Bide total
   - AI choice for a few hand-checked situations
   - first-battle flee at 20%
   - a catch with forced shakes

### Task K — battle scene: event playback and input

**Owns:** `crates/engine/src/battle/scene.rs` (+ `battle/scene/`).

1. **Composition:** `Background`, sprites, healthboxes and bars (E), `BattleWindows` (F), in Gen3 order:
   - **Load:** in `load`, black, chunked: environment, textbox, pics, healthboxes.
   - **Play events one at a time:**
     - `Msg`: expand via `text::expand` with `Names` built from the `Battle`, print, then wait per `MsgWait` (frames, or A/B).
     - `Hp`: drain the bar and the numbers together.
     - `Exp`: fill, wrapping at a level-up.
     - `SendOut`, `Faint` and the other animations run to completion.
   - **When idle and the core needs input:**
     - `Need::Action`: action menu; FIGHT → move menu → `Choice::Move`. RUN → `Choice::Run`. BAG and POKéMON do nothing yet (documented).
     - `Need::LearnMove`: the yes/no and replace flow.
     - `Need::Switch`: auto first usable.
   - **End:** the trainer defeat slide, the defeat text, fade to black; `finished()`.
2. **Gen3 timings:** the pause after send-out, the healthbox slide, `B_WAIT_TIME_*`. Each is cited in a comment.
3. **Tests:**
   - scripted playback of a synthetic event list: frame counts, the frame where the menu appears, input → choice
   - a full synthetic battle (Task D test data) driven to the end with a fixed input script and a fixed seed, with a hash golden
   - per-frame upload budget assertions: no bulk cmds once visible

### Task I — content: battle tables, codegen and prelude

**Owns:** `crates/content/{build/*, src/**}`, `crates/extract/src/codegen.rs` (+ `tests/codegen.rs`).

1. **`build.rs`:**
   - `BattleTables` from `battle/*.ron`: species (all; `gfx` only for the **slice set** = starters + the land mons of exported maps + the parties of trainers referenced by exported scripts + Zigzagoon), moves (all), type chart, `struggle`, `nature_power`.
   - `EFFECT_*` → `MoveEffect` and `ABILITY_*` → `Ability` in `classify.rs`. Unknown names map to `Unsupported`/`Other`, with a build summary line.
   - trainers referenced by exported scripts; encounters; heal locations; battle strings (`Str` → bytes by name; the wild/foe tag from Task A)
   - `BattleGfx`; `SPECIES_*` / `MOVE_*` / `TRAINER_*` / `HEAL_LOCATION_*` consts
   - Large tile data goes through `include_bytes!` of aligned `OUT_DIR` blobs, not Rust literals, to keep compile times sane.
2. **Stub mode:** a 2-species, 3-move table, tiny gfx, and one tall-grass cell with an encounter table on the stub map, so CI parity covers a battle without the ROM.
3. **Codegen** (types from `include/constants/battle_setup.h`):
   - **`trainerbattle`:**
     - type 0 SINGLE / 5 REMATCH: `if trainer_battle(&ctx, TrainerBattle::single(TRAINER_X, local, INTRO, DEFEAT)).await { return; }` followed by the already-beaten path.
     - types 1/2 CONTINUE_SCRIPT: on a win, jump to the continue label (`pc = …; continue;` or a boxed call + return).
     - type 3 SINGLE_NO_INTRO_TEXT: `trainer_battle(&ctx, TrainerBattle::no_intro(TRAINER_X, DEFEAT)).await;` then **continue with the following code** (fix the M3 "already-beaten" comment for this type).
     - doubles (4, 6, 7, 8) and others: `unsupported`, treated as beaten.
   - **Other commands:** `givemon` → `ctx.give_mon`, `getpartysize` → `VAR_RESULT = ctx.party_size()`, `setrespawn` → `ctx.set_respawn`. Optional: `bufferspeciesname` / `bufferleadmonspeciesname` / `buffernumberstring` as STR_VAR buffers (needs an engine `Request::Buffer`, which is the integration's call).
   - Golden tests per shape.
4. **Prelude:**
   - `trainer_battle`, mirroring `data/scripts/trainer_battle.inc` (`EventScript_TryDoNormalTrainerBattle`, `EventScript_DoTrainerBattle`, `EventScript_DoNoIntroTrainerBattle`): trainer flag = `TRAINER_FLAGS_START (0x500) + id`, already set → `false`; lock, faceplayer, intro `msgbox`, `ctx.battle_trainer`, set the flag, `release_all` for type 0, return `true`.
   - Specials:
     - `ChooseStarter`, as `ChooseStarter` + `CB2_GiveStarter` + `CB2_StartFirstBattle`: `choose_starter([TREECKO, TORCHIC, MUDKIP])`, then `VAR_STARTER_MON` = index, then `give_mon(starter, 5)`, then `battle_wild(ZIGZAGOON, 2, true)`. The opponent comes from `battle_controllers.c`.
     - `HealPlayerParty` → `ctx.heal_party()`.
     - `PlayerFaceTrainerAfterBattle`: cosmetic, a log.
5. **New game:** `Emerald` gets a `start: Start` (`Emerald::default()` = `Start::BeforeRescue`, `Emerald::after_starter()` = the M3 state + `give_mon(MUDKIP, 5)` and `VAR_STARTER_MON = 2`). Document `BeforeRescue`'s vars and flags from the decomp intro scripts:
   - `VAR_LITTLEROOT_TOWN_STATE` 1 (rival met, go save Birch), `VAR_ROUTE101_STATE` 0
   - Birch and the Zigzagoon visible on Route 101
   - the lab rival hidden, and so on
   - Respawn default: `HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE`.
6. **Tests:**
   - with extracted data: every slice species has gfx, every slice move's effect is not `Unsupported`, May's three Route 103 parties resolve, Route 101/102/103 tables have 12 slots, and every `Str` has text
   - stub builds and runs
   - generated Route 103 compiles against the prelude
   - ROM < 32 MiB, and report the size delta

---

## Wave 3 — integration

### Task J — integration: battles in the World

**Owns:** everything needed, coordinating with the module APIs (`world.rs`, `world/*`, M3 modules for restore hooks, shells, `game-native/src/scenario.rs`, `crates/emu/tests/parity.rs`, xtask).

1. **World state:**
   - `party: Party`, `respawn: u8`, `player_id: u32` (`random32()` in `World::new`, as Gen3 `NewGameInitData`), `prev_behavior`
   - `mode: Mode { Field, Starter(Box<StarterChoice>), Battle(Box<BattleRun>) }`
   - `BattleRun { stage: Transition | Loading | Running | Leaving | Whiteout, core, scene, events, origin: Script | Field }`
2. **Field triggers**, in Gen3 `ProcessPlayerFieldInput` order: on step end, coord and warp scripts first, then `wild::try_land_encounter` when the cell is `TallGrass`, the map has a table, and the party has a usable mon. The environment is `Grass` on tall grass and `Plain` otherwise (`BattleSetup_GetEnvironmentId`, simplified).
3. **ScriptHost** for every new `Request`:
   - `GiveMon` (party, else "can't": no PC), `PartySize`, `HealParty`, `SetRespawn`
   - `ChooseStarter` (Starter mode, answered on choice): `load_step` while black until it returns true, then `update`. When it returns `Some(i)` the starter screen is still visible: answer the request and, for the first battle (`CB2_GiveStarter` → `CB2_StartFirstBattle`), run `BattleTransition` straight over that screen. The starter screen overwrites BG char block 0, screen blocks 6/7, BG0 (text char block, screen block 31), BG palettes 0.. and 15, OBJ tiles 512.. and OBJ palettes 0..4: the field is fully restored afterwards, as after a battle.
   - `BattleWild` / `BattleTrainer`: start a `BattleRun`; answer the outcome after the field has returned.
   - Immediate (map-script) mode logs and answers `Ran` / 0.
4. **Battle lifecycle:**
   - **Freeze objects**, transition (G), then while black `scene.load` chunks. The transition owns `brightness`, `lighten` and `scroll[0]` while it runs and overwrites BG0 (tiles 510/511 of the text char block, screen block 31) and BG palette 15: the field `TextBox` must be closed (`TextBox::close`/`new`) and rebuilt after the battle, never reused as is.
   - **Run:** core + scene; the player party is written back at the end.
   - **Leave:** fade to black, **restore the field** (re-upload tilesets and palettes, redraw, drop the object sprite caches, re-init the text box gfx: hooks on `Field`, `Objects`, `TextBox`), fade in, unfreeze, answer the script.
   - **Lost** (not the first battle): Gen3 `CB2_WhiteOut`, simplified and documented:
     - abort the runner, heal the party, warp to `heal_location(respawn)` while black
     - no money loss and no Mom/Nurse script
   - **First battle:** Gen3 `CB2_EndFirstBattle` returns to the script whatever the outcome.
5. **Hash:** append the party, mode, run stage, core and scene hashes, `respawn`, `player_id` and `prev_behavior`. Update the documented World goldens.
6. **Trainer sight** (optional; do it only if the route is green early): `flow::trainer_sight` (`CheckForTrainersWantingBattle`, `GetTrainerApproachDistance`) with an Exclamation emote, an approach walk, then the trainer's script.
7. **Scenario and parity:**
   - New `Action`s: `Grass { area }` (walk back and forth in grass until a battle starts), `Fight(slot)` (until the battle ends, mashing through messages), `Run`, `Starter(i)`, `Mash` (until the field returns).
   - The route, roughly 10 000–20 000 frames:
     1. new game → Route 101 (rescue cutscene) → bag → choose Mudkip → first battle (win or Zigzagoon flees)
     2. lab return
     3. Route 101 grass: one wild battle won with exp, plus one run
     4. Oldale → Route 103 → talk to May → win
   - Checkpoints: hash and picture at the battle intro, the action menu, the move menu, mid HP drain, the level-up, May's intro, and back on the field.
   - `late == 0`, and `late_black <= warps + 2 × battles`. The stub route must cover the stub grass battle.
8. **`cargo xtask shot`:** add the starter screen, a wild intro, the action menu, the move menu, May's intro, and the level-up message.
9. **README:** "Play" gains the battle controls and the known simplifications.

**Acceptance:**
- All tests pass.
- `cargo xtask parity` passes with real data and stub.
- The shots show French battle text, correct pics and healthboxes.
- ROM and native are identical.

## Wave 4 — close-out

The milestone review covers the whole workspace: fix review findings, update this doc with "As built" notes and "Findings for Milestone 5" (checkpoint serde needs: `Party`, `Battle`, scene state), and clean up worktrees.

## Done when

- `cargo xtask extract && cargo xtask parity` pass.
- In the parity run and the shots, the player:
  - picks a starter on Route 101 and wins the first battle
  - meets wild Pokémon in the tall grass of Routes 101–103 at the Gen3 rates
  - gains exp and levels up
  - runs away
  - beats May on Route 103 through the generated `trainerbattle`
- Every random roll comes from the World RNG, and the ROM and native builds stay hash- and pixel-identical.
- Damage, stats, exp, run and catch formulas are pinned by hand-computed Gen3 goldens.

---

## Open questions / decisions taken

1. **New game start: before the rescue (recommended, taken).** `SPECIAL_ChooseStarter` and the first battle are the heart of the slice, and `VAR_STARTER_MON` drives May's party. `Emerald::after_starter()` keeps the simpler start (M3 state + Mudkip L5) for fast tests and the M3 shots. *Risk:* lab scripts after the rescue use unsupported commands (nickname naming screen, `showmonpic`). They log and continue; the review checks that none softlocks.
2. **Trainer battle:** May on Route 103 is required (type 3, three parties by starter). The Route 102 trainers work when talked to (types 0/2). Trainer sight approach is optional in J. Route 102 is behind the Oldale sketch-man gate until the Pokédex, so the parity route doesn't need it.
3. **Catching:** core-only (formula + events + party add, golden-tested). No bag means no Poké Balls in normal play, so there is no UI entry; scenario tests may submit `Choice::Ball`. Cheap, and ready for the bag milestone.
4. **Party switch screen:** none. A forced player switch auto-picks the first usable mon. POKéMON and SAC do nothing (documented). The opponent switches in party order instead of Gen3 `GetMostSuitableMonToSwitchInto`.
5. **Forget-a-move:** the move menu is reused with the new move as a fifth line; B, or "stop learning", keeps the old set. The Gen3 summary screen is not used.
6. **Trainer AI:** the Gen3 score model with a documented subset of `CHECK_BAD_MOVE`, `TRY_TO_FAINT` and `CHECK_VIABILITY`. `AI_FirstBattle` is exact (Zigzagoon flees at ≤ 20%). Wild AI is random, as Gen3.
7. **Presentation simplifications:**
   - Transitions are tile-based (GRID_SQUARES / bars + the intro flashes) instead of the HBlank/window effects.
   - No affine scaling, so send-out pops from the ball.
   - Move animations are a generic lunge, hit blink and palette tint.
   - No environment entry animation (optional) and no level-up stat window (optional).
   - No audio.
8. **Whiteout:** heal plus warp to the last heal location (`setrespawn`, default Brendan's house). No money loss and no Mom/Nurse script.
9. **Out of scope:** audio, doubles, bag/PC/party/summary menus, evolution, items (Pickup, held items, money), Exp. Share, weather, roamers/repel/flutes, the naming screen, link/frontier.
10. **Data breadth:** stats, names, learnsets and moves for all species, moves and trainers (cheap tables). Pics are compiled only for the slice set, to bound ROM size and compile time. Engine gfx is `Option`: a missing pic draws nothing and logs.
11. **RNG:** we keep our xorshift. Gen3 *call order* is mirrored, but not Gen3 LCG values. Exactness is defined by the formulas and the per-roll mapping, which are tested with explicit rolls.
12. **To verify in Task A:** whether the FR "sauvage/ennemi" tag is a prefix or a suffix (it changes `text::expand`), and FR-localized status icons in the healthbox gfx (Task B).
13. **Battle VRAM:** the battle owns VRAM while it runs, and the field fully re-uploads on return (one black bulk frame). That is simpler and safer than preserving field VRAM across the battle.

---

## As built

Master after the close-out merges. The route: 19 082 frames, 8 battles (first battle, 5 wild won, 1 run, May won). Parity: 19 071 hashes and 204 pictures identical, `late == 0`, 21 black overruns all attributed to the 22 bulk events. Stub: 1 543 frames, 1 battle. ROM 1875 KiB (stub 375 KiB). Extract format v4.

**Contracts that changed after Wave 0**
- `Str`:
  - `Attacker/DefenderStatChanged` became `…StatRose` (codes kept), plus `…StatFell`.
  - Added `NoRunningFromTrainers`, `BirchInTrouble` (a decomp string id, allowed by the rules), the E2 status, ability, catch and misc strings, and `AbilityName(Ability)` (`gAbilityNames`, extracted in v4).
  - `Str::code()` is the stable, exhaustive hash code. Retired codes leave gaps.
- `Msg::slots: [u8; 2]` and `Names::parties`: a queued message names the battler that was active when it was emitted, even after a `SendOut` in the same batch. Battlers are replaced only through `switch_in`.
- `Buff::MonNick`. `MonFled` uses it, following `HandleEndTurn_MonFled`'s `PREPARE_MON_NICK_BUFFER`.
- `Battle::new` draws the 4 `BattleAI_HandleItemUseBeforeAISetup` rolls, then the held-item roll (wild only), then `gRandomTurnNumber`. Parties and the scripted wild mon are created when the battle is requested, before the transition. This is a deviation: Gen3 creates them after the transition.
- Outcomes:
  - `Drew` is a whiteout like `Lost`, except in the first battle, which always returns to its script.
  - The trainer flag is set unless the outcome is `Lost | Drew` (`IsPlayerDefeated`).
- Trainer AI:
  - The Gen3 score model, with flags `CHECK_BAD_MOVE`, `TRY_TO_FAINT`, `CHECK_VIABILITY`, `SETUP_FIRST_TURN` and `FIRST_BATTLE`. Score is an s8 that wraps.
  - `simulatedRNG` rolls, then `ShouldSwitch`, restricted to the subset that draws the RNG.
  - The opponent's Run is ordered by speed. Only the player's Run goes first.
- `BattleScene`:
  - `finished() -> Option<BattleOutcome>`, `Some` only after the fade to black.
  - `step(game, &mut Battle, &mut Rng, &Input, f)` is the World driver.
  - `load` takes 11 black chunks and marks `Frame::bulk`.
  - The scene owns BG0–3, char blocks 0–2, screen blocks 24/26/28, BG palettes 0–5, OBJ tiles 0–468 and OBJ palettes 0–7.
- OBJ layout:
  - Tiles: pics 0/64/128/192, player box 256..384, enemy box 384..448, HP bars 448/456, ball 464, 468..512 free.
  - Palettes: OBJ 4 for the player box, 7 for the enemy box, 8..10 for tints.
  - At most 16 OBJ tiles per visible frame.
- `BattleGfx`:
  - textbox: 4 palettes `[tb0, tb1, text, text_pp]` and the full 32×64 map
  - `starter_bag.tilemap`: the bag map padded to 1024, then the grass map
  - `TrainerPic.y` is Gen3's `MonCoords.size`
- Content:
  - `Emerald { start: Start }`, with `Start::default() = BeforeRescue`.
  - `Emerald::for_shells()` gives BeforeRescue on real data and AfterStarter on the stub. The stub has no story, and its route battles in the StubTown (12,3) grass. The old `Emerald` const was removed.
  - Generated `trainer_battle` covers types 0/1/2/3/5 (5 is treated as 0). Doubles are unsupported.
- World:
  - `Mode { Field, Starter, Battle }`, with `BattleRun` stages Transition → Loading → Running → Leaving → Whiteout(120) → Returning.
  - Only field encounters freeze objects (`DoStandardWildBattle`).
  - Objects update during the return fade-in.
  - The field text box closes when a battle starts.
  - Environment is Grass on tall grass and Plain otherwise.
  - Encounter immunity is 4 steps, reset on map load, connection crossing and battle.
  - A script battle with no usable mon answers `Won`; a map-script battle answers `Ran`.
  - `ChangePokemonNickname` (no naming screen) only fades back in.
- Parity budget:
  - `Frame::bulk` marks warp loads, whole-view redraws (script teleports), starter-screen chunks, battle loads and field restores.
  - Every black overrun must follow a bulk step within `DISPLAY_LAG = 3` frames, and the count must be ≤ the number of bulk events. This replaces `late_black <= warps + 2 × battles`, which the starter screen and teleports broke.
- GBA performance:
  - Text glyphs and `fill_rect` are drawn as 32-bit words, one tile row at a time.
  - `Canvas::redraw` skips unchanged text.
  - `BattleWindows::prepare` pre-draws the menus while the screen is black.
  - The worst visible frame costs about 197 of 273 timer ticks.

**Simplifications (documented in module docs and the README)**
- Move names use the normal font (no FONT_NARROW) and are clipped at 64 px.
- Missing presentation:
  - no level-up stat window, no "Come back X!", no ball-throw animation
  - no intro window reveal, no environment slide
- BAG and POKéMON do nothing.
- Unsupported:
  - trainer sight
  - Attract and Rollout (`KNOWN_UNSUPPORTED`)
  - the Route 103 twins (doubles)
  - the lab's Johto `givemon`
  - `bufferleadmonspeciesname`
- The pale mint field palette is faithful. Emerald's raw palettes look like this; it was checked against the original ROMs' palette RAM in mGBA. A GBA LCD colour-correction filter would have to be an opt-in display option.

## Findings for Milestone 5 (checkpoints and replay traces)

- **Capture point.** "No script running" is not enough, because a field wild battle runs without a script. Capture only when all of these hold:
  - `Mode::Field`
  - the warp transition is idle
  - the text box is closed
  - the runner is idle (`Runner.task` is a `Pin<Box<dyn Future>>`; `io` is an `Rc<RefCell>`)
  - no object has live `moves`

  States in the middle of a battle or a script are then reached by replaying from the last checkpoint.
- **Semantic state that needs serde:**
  - `World`/`State`:
    - core: `frame`, `rng`, `input.held`, `vars`, `fader`, `transition`, `selected`, `last_mover`, `delay`, `door`, `answer`
    - field: map, camera, ring, overrides
    - objects: `objs`, `map`, `run_state`
    - player and party: `party`, `respawn`, `player_id`, `prev_behavior`, `encounter_immunity`
  - `Runner::issued`: hashed, and it survives the end of a script.
  - Object templates, stored as deltas: scripts only change x, y and movement.
  - `Party`/`Pokemon` are plain `Copy` data.
  - `Battle` is Clone and plain data, if battles are ever captured.
  - The player name currently comes from `game.new_game()` (`battle/scene.rs`, `text.rs`). Move it into World state before any save or naming screen.
- **`&'static` references in live state must become ids:**
  - `Request::Move { steps }` and `Object.moves.steps`: use the ScriptId plus a step index.
  - `Request::Log`.
  - `&'static BattleGfx` in the gfx parts: re-inject it from `game.battle_gfx()` instead.
- **Caches that can be rebuilt:**
  - `Field.drawn` (rebuilt by `Field::restore`)
  - object VRAM slots and banks (`Objects::invalidate_vram`)
  - `Vars::digest`, `Pen`, `Printer::text_digest`, the `Canvas` memo
  - the healthbox and bar tile caches
- **Hash/restore hazard.** Several hashed fields mirror VRAM: `Field.drawn`, `HpBar/ExpBar.drawn`, the healthbox dirty flags, `TextBox.bg_on`, `BattleWindows.loaded`, `TrainerSprite.loaded`, and `Background.step`. A restore that re-uploads through the existing hooks changes the hash, so a restored world would diverge from its trace. Pick one approach:
  - serialize these fields verbatim and add "re-upload without mutating" restore hooks (`Field`, `Objects`, `TextBox`, and later `BattleScene`), or
  - take them out of the hash.
- **Hash collisions.** `Object.moves` and `Request::Move` hash only `steps.len()`, so two movement scripts of the same length collide. Hash the step-table id instead. That id is also the serde handle.
- **No derives yet.** None of `World`, `State`, `Runner`, `Mode`, `Field`, `Objects`, `TextBox`, `Printer`, `BattleScene` or `BattleWindows` derives `Clone` or `Debug`. Add `#[derive(Clone, Debug, Serialize, Deserialize)]` on the semantic structs behind a `serde` feature, so the GBA build stays `no_std` and lean.
- **Hash cost.** `World::hash` during a battle covers four parties, the scene event queue, the core and every gfx part. Measure it on GBA before adding more hashed state. Consider hashing the scene queue incrementally.
- **Test shims.** Unify the test `GameData` shims (`battle/test_data.rs`, `mon::test_tables`, the scene, ui and gfx shims) before M5 adds checkpoint fixtures. This is partly done in the close-out.
- **Display.** Consider an optional GBA LCD colour-correction filter for the native window and the studio. It must stay outside parity.
