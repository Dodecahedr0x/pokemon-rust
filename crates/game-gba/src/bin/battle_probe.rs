//! Battle probe ROM (Silice engine): times on the GBA the core turn that carries a Mega
//! Evolution against the same turn without it, on a synthetic game (no content), with the
//! cascaded hardware timers (cycles; 1232 per scanline). Publishes, once done (`frame` 1):
//! - `hash_lines_max`: the plain turn (`Battle::choose(Choice::Move(0))`), in scanlines;
//! - `step_lines_max`: the same turn with the Mega Evolution (`Choice::Mega(0)`);
//! - `loop_lines_max`: `Battle::new` + `enable_player_forms` + `start` with a 34-entry form
//!   table, minus the same without forms;
//! - `hash`: the two turns' cycles (plain in the high word, Mega in the low word).
//!
//! Read by `crates/emu/tests/parity.rs` (`battle_probe_mega_turn`). Built next to the game by
//! `cargo xtask rom`.

#![no_std]
#![no_main]

extern crate alloc;

#[path = "../panic.rs"]
mod panic;

use alloc::boxed::Box;
use alloc::vec::Vec;
use engine::battle::{Battle, BattleKind, BattleSetup, Choice, Environment, Str};
use engine::data::{
    Behavior, Dir, FlagId, Font, FormChange, FormKind, GameData, GfxId, ItemId, MapData, MapId,
    MoveId, NewGame, ScriptId, SpeciesId, SpriteData, TextId, TrainerId, UiText, WindowGfx,
};
use engine::debug::DebugRegion;
use engine::mon::{
    Ability, BattleTables, GrowthRate, MoveEffect, MoveFlags, MoveInfo, MoveTarget, NAME_LEN,
    Party, Pokemon, SpeciesInfo, Status, Type, TypeMatchup,
};
use engine::rng::Rng;
use engine::script::{Ctx, Script};

#[used]
#[unsafe(no_mangle)]
static mut PKR_DEBUG: DebugRegion = DebugRegion::new();

const TM0CNT_L: *mut u16 = 0x0400_0100 as *mut u16;
const TM0CNT_H: *mut u16 = 0x0400_0102 as *mut u16;
const TM1CNT_L: *mut u16 = 0x0400_0104 as *mut u16;
const TM1CNT_H: *mut u16 = 0x0400_0106 as *mut u16;
const CYCLES_PER_LINE: u32 = 1232;

/// Timer 1 counts timer 0's overflows: a free-running 32-bit cycle counter.
fn start_timers() {
    unsafe {
        TM0CNT_H.write_volatile(0);
        TM1CNT_H.write_volatile(0);
        TM0CNT_L.write_volatile(0);
        TM1CNT_L.write_volatile(0);
        TM1CNT_H.write_volatile(0x0084);
        TM0CNT_H.write_volatile(0x0080);
    }
}

fn cycles() -> u32 {
    loop {
        let hi = unsafe { TM1CNT_L.read_volatile() };
        let lo = unsafe { TM0CNT_L.read_volatile() };
        if unsafe { TM1CNT_L.read_volatile() } == hi {
            return (hi as u32) << 16 | lo as u32;
        }
    }
}

const TACKLE: MoveId = MoveId(1);
const STRUGGLE: MoveId = MoveId(2);
const BASE: SpeciesId = SpeciesId(1);
const MEGA: SpeciesId = SpeciesId(2);
const FOE: SpeciesId = SpeciesId(3);
const STONE: ItemId = ItemId(1);
const RING: FlagId = FlagId(1);

const fn mv(effect: MoveEffect, power: u8) -> MoveInfo {
    MoveInfo {
        name: &[0xFF],
        effect,
        power,
        ty: Type::Normal,
        accuracy: 100,
        pp: 35,
        effect_chance: 0,
        target: MoveTarget::Selected,
        priority: 0,
        flags: MoveFlags(MoveFlags::MAKES_CONTACT),
    }
}

static MOVES: [MoveInfo; 3] = [
    mv(MoveEffect::Hit, 0),
    mv(MoveEffect::Hit, 40),
    mv(MoveEffect::Recoil, 50),
];

const fn sp(base: [u8; 6], ty: Type, ability: Ability) -> SpeciesInfo {
    SpeciesInfo {
        name: &[0xFF],
        base,
        types: [ty, ty],
        catch_rate: 45,
        exp_yield: 100,
        ev_yield: [0; 6],
        gender_ratio: 127,
        friendship: 70,
        growth: GrowthRate::MediumFast,
        abilities: [ability, Ability::None],
        learnset: &[],
        weight: 0,
        gfx: None,
    }
}

static SPECIES: [SpeciesInfo; 4] = [
    sp([0; 6], Type::Normal, Ability::None),
    sp(
        [70, 110, 70, 90, 115, 70],
        Type::Fighting,
        Ability::Steadfast,
    ),
    sp(
        [70, 145, 88, 112, 140, 70],
        Type::Steel,
        Ability::Technician,
    ),
    sp([80, 80, 80, 80, 80, 80], Type::Normal, Ability::None),
];

static CHART: [TypeMatchup; 1] = [TypeMatchup {
    attack: Type::Normal,
    defend: Type::Ghost,
    multiplier: 0,
}];

static TABLES: BattleTables = BattleTables {
    species: &SPECIES,
    moves: &MOVES,
    type_chart: &CHART,
    foresight_split: 0,
    struggle: STRUGGLE,
    nature_power: [TACKLE; 10],
};

/// 34 form changes (the canonical stones and orbs' count), the match last.
static FORMS: [FormChange; 34] = {
    let mut f = [FormChange {
        base: FOE,
        item: ItemId(99),
        form: MEGA,
        kind: FormKind::Mega,
        enable_flag: Some(RING),
    }; 34];
    f[33] = FormChange {
        base: BASE,
        item: STONE,
        form: MEGA,
        kind: FormKind::Mega,
        enable_flag: Some(RING),
    };
    f
};

struct ProbeGame {
    forms: bool,
}

impl GameData for ProbeGame {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!("the probe has no maps")
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!("the probe has no sprites")
    }
    fn font(&self) -> &'static Font {
        unimplemented!("the probe draws nothing")
    }
    fn window(&self) -> &'static WindowGfx {
        unimplemented!("the probe draws nothing")
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[0xFF]
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[0xFF],
        }
    }
    fn battle(&self) -> &'static BattleTables {
        &TABLES
    }
    fn battle_text(&self, _: Str) -> &'static [u8] {
        &[0xFF]
    }
    fn form_changes(&self) -> &'static [FormChange] {
        if self.forms { &FORMS } else { &[] }
    }
}

fn mon(species: SpeciesId, held: Option<ItemId>) -> Pokemon {
    let mut p = Pokemon {
        species,
        personality: 0,
        ot_id: 0,
        nickname: [0xFF; NAME_LEN],
        level: 50,
        exp: 125_000,
        ivs: [31; 6],
        evs: [0; 6],
        moves: [Some(TACKLE), None, None, None],
        pp: [35, 0, 0, 0],
        pp_bonuses: 0,
        friendship: 70,
        ability_slot: 0,
        status: Status::None,
        hp: 0,
        stats: [0; 6],
        held,
    };
    p.recalc_stats(&TABLES);
    p.hp = p.stats[0];
    p
}

fn party(mons: &[Pokemon]) -> Party {
    let mut p = Party::default();
    for &m in mons {
        let _ = p.push(m);
    }
    p
}

fn setup() -> BattleSetup {
    BattleSetup {
        kind: BattleKind::Trainer {
            trainer: TrainerId(1),
            defeat: TextId(0),
        },
        opponents: party(&[mon(FOE, None), mon(FOE, None)]),
        environment: Environment::Grass,
        double: None,
    }
}

/// Cycles of `Battle::new` + `enable_player_forms` + `start`, and the started battle.
fn new_battle(game: &ProbeGame, rng: &mut Rng) -> (u32, Battle) {
    let player = party(&[mon(BASE, Some(STONE)), mon(FOE, None)]);
    let mut out = Vec::new();
    let t0 = cycles();
    let mut b = Battle::new(game, setup(), &player, rng);
    b.enable_player_forms(game, |f| f == RING);
    b.start(game, rng, &mut out);
    (cycles().wrapping_sub(t0), b)
}

/// Cycles of the first turn's `choose(c)`.
fn turn(c: Choice) -> u32 {
    let game = ProbeGame { forms: true };
    let mut rng = Rng::new(0x1234_5678);
    let (_, mut b) = new_battle(&game, &mut rng);
    let mut out = Vec::new();
    let t0 = cycles();
    b.choose(&game, &mut rng, c, &mut out);
    cycles().wrapping_sub(t0)
}

fn lines(c: u32) -> u32 {
    c.div_ceil(CYCLES_PER_LINE)
}

#[agb::entry]
fn main(_gba: agb::Gba) -> ! {
    start_timers();
    // Warm up (first-call costs), then the measures.
    let _ = turn(Choice::Move(0));
    let plain = turn(Choice::Move(0));
    let mega = turn(Choice::Mega(0));
    let (with, _) = new_battle(&ProbeGame { forms: true }, &mut Rng::new(1));
    let (without, _) = new_battle(&ProbeGame { forms: false }, &mut Rng::new(1));
    let d = core::ptr::addr_of_mut!(PKR_DEBUG);
    unsafe {
        (&raw mut (*d).hash).write_volatile((plain as u64) << 32 | mega as u64);
        (&raw mut (*d).hash_lines_max).write_volatile(lines(plain));
        (&raw mut (*d).step_lines_max).write_volatile(lines(mega));
        (&raw mut (*d).loop_lines_max).write_volatile(lines(with.saturating_sub(without)));
        (&raw mut (*d).frame).write_volatile(1);
    }
    let vblank = agb::interrupt::VBlank::get();
    loop {
        vblank.wait_for_vblank();
    }
}
