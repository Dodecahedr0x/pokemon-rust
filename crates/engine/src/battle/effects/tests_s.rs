//! The Silice engine: held items, the Fairy type, Mega Evolution / Primal Reversion and the
//! post-Gen3 abilities, on their own synthetic tables ([`TABLES`], [`SGame`]). Same event
//! format as the core tests (`core::tests::fmt`).

use super::tests::{draws, has, nth, seed_where};
use crate::battle::calc;
use crate::battle::core::tests::{hash, lines, trainer};
use crate::battle::core::{Action, Battle, Env, HitResult, Weather};
use crate::battle::test_data::{Game, mon};
use crate::battle::{BattleKind, BattleSetup, Choice, Effectiveness, Environment, Side, Str};
use crate::data::{
    Behavior, FlagId, Font, FormChange, FormKind, GameData, GfxId, ItemId, MapData, MapId, MoveId,
    NewGame, ScriptId, SpeciesId, SpriteData, TextId, UiText, WindowGfx,
};
use crate::mon::{
    Ability, BattleTables, FAIRY_MATCHUPS, GrowthRate, HoldEffect, MoveEffect, MoveFlags, MoveInfo,
    MoveTarget, Party, Pokemon, SpeciesInfo, Stat, Status, StatusKind, Type, TypeMatchup,
};
use crate::rng::Rng;
use crate::script::{Ctx, Script};
use alloc::string::String;
use alloc::vec::Vec;

const P: Side = Side::Player;
const O: Side = Side::Opponent;

// ---- moves ----
const TACKLE: MoveId = MoveId(1);
const GROWL: MoveId = MoveId(2);
const EMBER: MoveId = MoveId(3);
const STRUGGLE: MoveId = MoveId(4);
const WATER_GUN: MoveId = MoveId(5);
const SHOCK: MoveId = MoveId(6);
const MUD_SHOT: MoveId = MoveId(7);
const MULTI: MoveId = MoveId(8);
const SPLASH: MoveId = MoveId(9);
const BITE_ALL: MoveId = MoveId(10);
const FAIRY_HIT: MoveId = MoveId(11);
const POISON_GAS: MoveId = MoveId(12);
const BIG_HIT: MoveId = MoveId(13);
const ROCK_HIT: MoveId = MoveId(14);
const STUN: MoveId = MoveId(15);
const HEAVY_HIT: MoveId = MoveId(16);
const SONIC: MoveId = MoveId(17);
/// `EFFECT_EXPLOSION` (unsupported: a plain hit here, which Parental Bond skips).
const EXPLODE: MoveId = MoveId(18);

const fn mv(effect: MoveEffect, power: u8, ty: Type, accuracy: u8, flags: u8) -> MoveInfo {
    MoveInfo {
        name: &[0xFF],
        effect,
        power,
        ty,
        accuracy,
        pp: 20,
        effect_chance: 0,
        target: MoveTarget::Selected,
        priority: 0,
        flags: MoveFlags(flags),
    }
}

const CONTACT_KR: u8 = MoveFlags::MAKES_CONTACT | MoveFlags::KINGS_ROCK_AFFECTED;

static MOVES: [MoveInfo; 19] = [
    mv(MoveEffect::Hit, 0, Type::Normal, 0, 0),
    mv(MoveEffect::Hit, 40, Type::Normal, 100, CONTACT_KR),
    mv(
        MoveEffect::StatDown {
            stat: Stat::Atk,
            stages: 1,
        },
        0,
        Type::Normal,
        100,
        MoveFlags::MAGIC_COAT_AFFECTED,
    ),
    mv(MoveEffect::Hit, 40, Type::Fire, 100, 0),
    mv(MoveEffect::Recoil, 50, Type::Normal, 100, 0),
    mv(MoveEffect::Hit, 40, Type::Water, 100, 0),
    mv(MoveEffect::Hit, 40, Type::Electric, 100, 0),
    mv(MoveEffect::Hit, 40, Type::Ground, 100, 0),
    mv(
        MoveEffect::MultiHit,
        15,
        Type::Normal,
        100,
        MoveFlags::MAKES_CONTACT,
    ),
    MoveInfo {
        target: MoveTarget::User,
        ..mv(MoveEffect::Splash, 0, Type::Normal, 0, 0)
    },
    MoveInfo {
        effect_chance: 100,
        ..mv(MoveEffect::FlinchHit, 60, Type::Dark, 100, 0)
    },
    mv(MoveEffect::Hit, 40, Type::Fairy, 100, 0),
    mv(
        MoveEffect::Status(StatusKind::Poison),
        0,
        Type::Poison,
        100,
        MoveFlags::MAGIC_COAT_AFFECTED,
    ),
    mv(MoveEffect::Hit, 250, Type::Normal, 100, 0),
    mv(MoveEffect::Hit, 40, Type::Rock, 100, 0),
    mv(
        MoveEffect::Status(StatusKind::Paralysis),
        0,
        Type::Electric,
        100,
        MoveFlags::MAGIC_COAT_AFFECTED,
    ),
    mv(
        MoveEffect::Hit,
        120,
        Type::Normal,
        100,
        MoveFlags::MAKES_CONTACT,
    ),
    mv(MoveEffect::SonicBoom, 1, Type::Normal, 100, 0),
    mv(MoveEffect::Unsupported(7), 20, Type::Normal, 100, 0),
];

// ---- species ----
const PLAIN: SpeciesId = SpeciesId(1);
const FAIRY: SpeciesId = SpeciesId(2);
const DRIZZLER: SpeciesId = SpeciesId(3);
const SUNNY: SpeciesId = SpeciesId(4);
const SANDY: SpeciesId = SpeciesId(5);
/// Normal, no ability; Mega Evolves into [`MEGA_FORM`] with [`MEGA_STONE`].
const MEGA_BASE: SpeciesId = SpeciesId(6);
/// Steel, Tough Claws, twice the base Attack and Speed.
const MEGA_FORM: SpeciesId = SpeciesId(7);
/// Water, Drizzle; Primal Reverts into [`PRIMAL_FORM`] with [`RED_ORB`].
const PRIMAL_BASE: SpeciesId = SpeciesId(8);
const PRIMAL_FORM: SpeciesId = SpeciesId(9);
const DRAGON: SpeciesId = SpeciesId(10);
const DARK: SpeciesId = SpeciesId(11);
const STEEL: SpeciesId = SpeciesId(13);
const SUN_PRIMAL: SpeciesId = SpeciesId(14);
const SPEEDY: SpeciesId = SpeciesId(15);
/// Electric, no ability; Mega Evolves into [`MEGA_INTIM`] with [`MEGA_STONE_2`].
const MEGA_BASE_2: SpeciesId = SpeciesId(16);
/// Electric, Intimidate.
const MEGA_INTIM: SpeciesId = SpeciesId(17);

const fn sp(base: [u8; 6], ty: Type, ability: Ability) -> SpeciesInfo {
    SpeciesInfo {
        name: &[0xFF],
        base,
        types: [ty, ty],
        catch_rate: 255,
        exp_yield: 50,
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

const B: [u8; 6] = [50; 6];

static SPECIES: [SpeciesInfo; 18] = [
    sp(B, Type::Normal, Ability::None),
    sp(B, Type::Normal, Ability::None),
    sp(B, Type::Fairy, Ability::None),
    sp(B, Type::Water, Ability::Drizzle),
    sp(B, Type::Fire, Ability::Drought),
    sp(B, Type::Rock, Ability::SandStream),
    sp(B, Type::Normal, Ability::None),
    sp([50, 100, 50, 100, 50, 50], Type::Steel, Ability::ToughClaws),
    sp(B, Type::Water, Ability::Drizzle),
    sp(
        [50, 50, 50, 50, 100, 50],
        Type::Water,
        Ability::PrimordialSea,
    ),
    sp(B, Type::Dragon, Ability::None),
    sp(B, Type::Dark, Ability::None),
    sp(B, Type::Rock, Ability::None),
    sp(B, Type::Steel, Ability::None),
    sp(B, Type::Ground, Ability::DesolateLand),
    sp(B, Type::Normal, Ability::SpeedBoost),
    sp(B, Type::Electric, Ability::None),
    sp(B, Type::Electric, Ability::Intimidate),
];

const fn tm(attack: Type, defend: Type, multiplier: u8) -> TypeMatchup {
    TypeMatchup {
        attack,
        defend,
        multiplier,
    }
}

/// A Gen3 subset, then [`FAIRY_MATCHUPS`] (as the content appends them), then the Foresight
/// entries.
static TYPE_CHART: [TypeMatchup; 16] = {
    let mut c = [tm(Type::Normal, Type::Rock, 5); 16];
    c[1] = tm(Type::Water, Type::Fire, 20);
    c[2] = tm(Type::Dragon, Type::Dragon, 20);
    let mut i = 0;
    while i < 12 {
        c[3 + i] = FAIRY_MATCHUPS[i];
        i += 1;
    }
    c[15] = tm(Type::Normal, Type::Ghost, 0);
    c
};

static TABLES: BattleTables = BattleTables {
    species: &SPECIES,
    moves: &MOVES,
    type_chart: &TYPE_CHART,
    foresight_split: 15,
    struggle: STRUGGLE,
    nature_power: [TACKLE; 10],
};

// ---- items ----
const ORAN: ItemId = ItemId(1);
const SITRUS: ItemId = ItemId(2);
const CHERI: ItemId = ItemId(3);
const PECHA: ItemId = ItemId(4);
const LUM: ItemId = ItemId(5);
const LEFTOVERS: ItemId = ItemId(6);
const CHARCOAL: ItemId = ItemId(7);
const CHOICE_BAND: ItemId = ItemId(8);
const QUICK_CLAW: ItemId = ItemId(9);
const KINGS_ROCK: ItemId = ItemId(10);
const BRIGHTPOWDER: ItemId = ItemId(11);
const FOCUS_BAND: ItemId = ItemId(12);
const MEGA_STONE: ItemId = ItemId(13);
const RED_ORB: ItemId = ItemId(14);
const PERSIM: ItemId = ItemId(15);
const RAWST: ItemId = ItemId(16);
const ASPEAR: ItemId = ItemId(17);
const CHESTO: ItemId = ItemId(18);
const RED_ORB_SUN: ItemId = ItemId(19);
const MEGA_STONE_2: ItemId = ItemId(20);

const MEGA_RING: FlagId = FlagId(0x900);

static FORMS: [FormChange; 4] = [
    FormChange {
        base: MEGA_BASE,
        item: MEGA_STONE,
        form: MEGA_FORM,
        kind: FormKind::Mega,
        enable_flag: Some(MEGA_RING),
    },
    FormChange {
        base: PRIMAL_BASE,
        item: RED_ORB,
        form: PRIMAL_FORM,
        kind: FormKind::Primal,
        enable_flag: None,
    },
    FormChange {
        base: PLAIN,
        item: RED_ORB_SUN,
        form: SUN_PRIMAL,
        kind: FormKind::Primal,
        enable_flag: None,
    },
    FormChange {
        base: MEGA_BASE_2,
        item: MEGA_STONE_2,
        form: MEGA_INTIM,
        kind: FormKind::Mega,
        enable_flag: Some(MEGA_RING),
    },
];

/// The Silice test game: [`TABLES`], the items' hold effects and [`FORMS`].
struct SGame;

impl GameData for SGame {
    fn map(&self, m: MapId) -> &'static MapData {
        Game.map(m)
    }
    fn sprite(&self, g: GfxId) -> Option<&'static SpriteData> {
        Game.sprite(g)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        Game.player_sprite()
    }
    fn font(&self) -> &'static Font {
        Game.font()
    }
    fn window(&self) -> &'static WindowGfx {
        Game.window()
    }
    fn text(&self, t: TextId) -> &'static [u8] {
        Game.text(t)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Game.ui_text(t)
    }
    fn behavior(&self, b: u8) -> Behavior {
        Game.behavior(b)
    }
    fn script(&self, s: ScriptId, c: Ctx) -> Script {
        Game.script(s, c)
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn battle(&self) -> &'static BattleTables {
        &TABLES
    }
    fn battle_text(&self, s: Str) -> &'static [u8] {
        Game.battle_text(s)
    }
    fn hold_effect(&self, id: ItemId) -> Option<HoldEffect> {
        Some(match id {
            ORAN => HoldEffect::RestoreHp(10),
            SITRUS => HoldEffect::RestoreHp(30),
            CHERI => HoldEffect::CurePar,
            PECHA => HoldEffect::CurePsn,
            LUM => HoldEffect::CureStatus,
            LEFTOVERS => HoldEffect::Leftovers,
            CHARCOAL => HoldEffect::TypePower(Type::Fire, 10),
            CHOICE_BAND => HoldEffect::ChoiceBand,
            QUICK_CLAW => HoldEffect::QuickClaw(20),
            KINGS_ROCK => HoldEffect::KingsRock(10),
            BRIGHTPOWDER => HoldEffect::EvasionUp(10),
            FOCUS_BAND => HoldEffect::FocusBand(10),
            PERSIM => HoldEffect::CureConfusion,
            RAWST => HoldEffect::CureBrn,
            ASPEAR => HoldEffect::CureFrz,
            CHESTO => HoldEffect::CureSlp,
            _ => return None,
        })
    }
    fn form_changes(&self) -> &'static [FormChange] {
        &FORMS
    }
}

// ---- helpers ----

/// L50, 100 HP, the given Atk / Def / Speed, SpAtk = SpDef = 50.
fn m(species: SpeciesId, speed: u16, moves: &[MoveId]) -> Pokemon {
    let mut p = mon(species, 50, [100, 50, 50, speed, 50, 50], &[]);
    p.moves = [None; 4];
    p.pp = [0; 4];
    for (i, &mv) in moves.iter().enumerate().take(4) {
        p.moves[i] = Some(mv);
        p.pp[i] = TABLES.move_info(mv).pp;
    }
    p
}

fn holding(mut p: Pokemon, item: ItemId) -> Pokemon {
    p.held = Some(item);
    p
}

fn party(mons: &[Pokemon]) -> Party {
    let mut p = Party::default();
    for &x in mons {
        p.push(x).unwrap();
    }
    p
}

struct Run {
    b: Battle,
    rng: Rng,
    intro: Vec<String>,
}

impl Run {
    fn new(kind: BattleKind, player: &[Pokemon], foes: &[Pokemon], ring: bool, seed: u32) -> Run {
        let mut rng = Rng::new(seed);
        let setup = BattleSetup {
            kind,
            opponents: party(foes),
            environment: Environment::Grass,
        };
        let mut b = Battle::new(&SGame, setup, &party(player), &mut rng);
        if ring {
            b.enable_player_forms(&SGame, |f| f == MEGA_RING);
        }
        let mut out = Vec::new();
        b.start(&SGame, &mut rng, &mut out);
        Run {
            b,
            rng,
            intro: lines(&out),
        }
    }

    fn wild(p: Pokemon, o: Pokemon) -> Run {
        let kind = BattleKind::Wild {
            first_battle: false,
        };
        Run::new(kind, &[p], &[o], false, 7)
    }

    fn choose(&mut self, c: Choice) -> Vec<String> {
        let mut out = Vec::new();
        self.b.choose(&SGame, &mut self.rng, c, &mut out);
        lines(&out)
    }

    /// `f` with an `Env` on `Rng::new(seed)`: its result, the RNG after it, the events.
    fn env<T>(
        &mut self,
        seed: u32,
        f: impl FnOnce(&mut Battle, &mut Env) -> T,
    ) -> (T, Rng, Vec<String>) {
        let mut rng = Rng::new(seed);
        let mut out = Vec::new();
        let mut env = Env::new(&TABLES, &mut rng, &mut out);
        let t = f(&mut self.b, &mut env);
        (t, rng, lines(&out))
    }

    fn use_move(&mut self, seed: u32, side: Side, slot: u8) -> (Rng, Vec<String>) {
        let ((), rng, l) = self.env(seed, |b, env| b.use_move(env, side, Action::Move(slot)));
        (rng, l)
    }
}

fn hit(damage: i32) -> Option<HitResult> {
    Some(HitResult {
        damage,
        hp_dealt: damage as u16,
        crit: false,
        eff: Effectiveness::Normal,
        hung: false,
    })
}

fn starts(l: &[String], prefix: &str) -> bool {
    l.iter().any(|x| x.starts_with(prefix))
}

fn pos(l: &[String], prefix: &str) -> usize {
    l.iter()
        .position(|x| x.starts_with(prefix))
        .unwrap_or_else(|| panic!("no {prefix} in {l:?}"))
}

// ---- held items ----

#[test]
fn a_battler_holds_its_party_item_and_its_effect() {
    let r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), LEFTOVERS),
        m(PLAIN, 10, &[SPLASH]),
    );
    let x = r.b.battler(P).x;
    assert_eq!(
        (x.item, x.hold),
        (Some(LEFTOVERS), Some(HoldEffect::Leftovers))
    );
    assert_eq!(r.b.battler(O).x, Default::default());
}

#[test]
fn hp_berries_heal_at_half_at_items1_and_are_eaten() {
    // ENDTURN_ITEMS1, HOLD_EFFECT_RESTORE_HP: hp <= maxHP / 2; BattleScript_ItemHealHP_RemoveItem
    // prints, then the HP, then `removeitem`.
    let mut r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), ORAN),
        m(PLAIN, 10, &[SPLASH]),
    );
    r.b.battler_mut(P).mon.hp = 51;
    r.b.write_back(P);
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "ItemRestoredHealth"), "above half: {l:?}");
    r.b.battler_mut(P).mon.hp = 50;
    let l = r.choose(Choice::Move(0));
    let i = pos(&l, "ItemRestoredHealth@P Item(ItemId(1)) ~64");
    assert_eq!(l[i + 1], "Hp P 50->60");
    assert_eq!(r.b.battler(P).x.item, None);
    assert_eq!(r.b.party(P).get(0).unwrap().held, None);
    assert_eq!(r.b.x.holds[0][0], None);
    // Sitrus: 30, capped at max HP.
    let mut r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), SITRUS),
        m(PLAIN, 10, &[SPLASH]),
    );
    r.b.battler_mut(P).mon.hp = 50;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "Hp P 50->80"));
}

#[test]
fn leftovers_heal_a_sixteenth_each_turn_and_stay() {
    let mut r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), LEFTOVERS),
        m(PLAIN, 10, &[SPLASH]),
    );
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "ItemRestoredHpALittle"), "full HP: {l:?}");
    r.b.battler_mut(P).mon.hp = 90;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ItemRestoredHpALittle@P Item(ItemId(6)) ~64"));
    assert!(has(&l, "Hp P 90->96"));
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "Hp P 96->100"), "capped: {l:?}");
    assert_eq!(r.b.battler(P).x.item, Some(LEFTOVERS));
}

#[test]
fn cure_berries_cure_their_status_at_the_end_of_the_turn() {
    for (item, status, id) in [
        (CHERI, Status::Paralysis, "ItemCuredParalysis"),
        (PECHA, Status::Toxic(1), "ItemCuredPoison"),
        (RAWST, Status::Burn, "ItemHealedBurn"),
        (ASPEAR, Status::Freeze, "ItemDefrostedIt"),
        (CHESTO, Status::Sleep(3), "ItemWokeIt"),
    ] {
        let mut r = Run::wild(
            holding(m(PLAIN, 50, &[SPLASH]), item),
            m(PLAIN, 10, &[SPLASH]),
        );
        r.b.battler_mut(O).mon.status = status;
        r.b.battler_mut(O).x = r.b.battler(P).x;
        r.b.battler_mut(P).x = Default::default();
        let ((), _, l) = r.env(1, |b, env| super::items::end_turn(b, env, O, false));
        assert_eq!(
            l,
            [
                alloc::format!("{id}@O Item(ItemId({})) ~64", item.0),
                "StatusIcon { side: Opponent, status: None }".into()
            ]
        );
        assert_eq!(r.b.battler(O).mon.status, Status::None);
        assert_eq!(r.b.battler(O).x.item, None);
    }
    // A berry without its status stays.
    let mut r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), CHERI),
        m(PLAIN, 10, &[SPLASH]),
    );
    let ((), _, l) = r.env(1, |b, env| super::items::end_turn(b, env, P, true));
    assert!(l.is_empty());
    assert_eq!(r.b.battler(P).x.item, Some(CHERI));
}

#[test]
fn hp_items_wait_for_items1_but_cures_do_not() {
    // ENDTURN_ITEMS2 runs ItemBattleEffects with moveTurn TRUE: no HP berry, no Leftovers.
    let mut r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), ORAN),
        m(PLAIN, 10, &[SPLASH]),
    );
    r.b.battler_mut(P).mon.hp = 10;
    let ((), _, l) = r.env(1, |b, env| super::items::end_turn(b, env, P, false));
    assert!(l.is_empty());
}

#[test]
fn lum_and_persim_cure_confusion() {
    // HOLD_EFFECT_CURE_STATUS: one problem → CURED_PROBLEM, two → NORMALIZED_STATUS.
    let mut r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), LUM),
        m(PLAIN, 10, &[SPLASH]),
    );
    r.b.battler_mut(P).confusion = 3;
    r.b.battler_mut(P).mon.status = Status::Burn;
    let ((), _, l) = r.env(1, |b, env| super::items::end_turn(b, env, P, true));
    assert!(
        has(&l, "ItemNormalizedStatus@P Item(ItemId(5)) ~64"),
        "{l:?}"
    );
    assert_eq!(
        (r.b.battler(P).confusion, r.b.battler(P).mon.status),
        (0, Status::None)
    );
    let mut r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), PERSIM),
        m(PLAIN, 10, &[SPLASH]),
    );
    r.b.battler_mut(P).confusion = 3;
    let ((), _, l) = r.env(1, |b, env| super::items::end_turn(b, env, P, true));
    assert_eq!(l, ["ItemSnappedOut@P Item(ItemId(15)) ~64"]);
}

#[test]
fn cure_berries_act_right_after_the_move() {
    // MOVEEND_ITEM_EFFECTS_ALL: the paralysis Stun gives is cured before the turn goes on.
    let mut r = Run::wild(
        m(PLAIN, 50, &[STUN]),
        holding(m(PLAIN, 10, &[SPLASH]), CHERI),
    );
    let (_, l) = r.use_move(1, P, 0);
    let par = pos(&l, "StatusIcon { side: Opponent, status: Paralysis }");
    let cure = pos(&l, "ItemCuredParalysis@O");
    assert!(cure > par, "{l:?}");
    assert_eq!(r.b.battler(O).mon.status, Status::None);
    // Lum at move end always says "cured its problem".
    let mut r = Run::wild(
        m(PLAIN, 50, &[POISON_GAS]),
        holding(m(PLAIN, 10, &[SPLASH]), LUM),
    );
    let (_, l) = r.use_move(1, P, 0);
    assert!(starts(&l, "ItemCuredProblem@O"), "{l:?}");
}

#[test]
fn type_items_and_choice_band_raise_the_attacking_stat() {
    // CalculateBaseDamage: after Huge Power, (100 + param)% of the type's stat; Choice Band
    // 150% Attack.
    let r = Run::wild(m(PLAIN, 50, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    let mut a = r.b.battler(P).fighter();
    let d = r.b.battler(O).fighter();
    let dmg = |a: &calc::Fighter, ty| calc::base_damage(a, &d, 100, ty, false);
    let (fire, normal) = (dmg(&a, Type::Fire), dmg(&a, Type::Normal));
    a.hold = Some(HoldEffect::TypePower(Type::Fire, 10));
    assert!(dmg(&a, Type::Fire) > fire);
    assert_eq!(dmg(&a, Type::Normal), normal);
    a.hold = Some(HoldEffect::ChoiceBand);
    assert_eq!(
        dmg(&a, Type::Fire),
        fire,
        "special: Choice Band does not apply"
    );
    let mut b = a;
    b.stats[Stat::Atk as usize] = 75;
    b.hold = None;
    assert_eq!(dmg(&a, Type::Normal), dmg(&b, Type::Normal));
    // The battler's fighter carries the hold effect.
    let r = Run::wild(
        holding(m(PLAIN, 50, &[SPLASH]), CHARCOAL),
        m(PLAIN, 10, &[SPLASH]),
    );
    assert_eq!(
        r.b.battler(P).fighter().hold,
        Some(HoldEffect::TypePower(Type::Fire, 10))
    );
}

#[test]
fn quick_claw_reads_the_turn_number() {
    // GetWhoStrikesFirst: gRandomTurnNumber < 0xFFFF * 20 / 100 → UINT_MAX speed.
    let mut r = Run::wild(
        holding(m(PLAIN, 5, &[TACKLE]), QUICK_CLAW),
        m(PLAIN, 90, &[TACKLE]),
    );
    r.b.actions = [Some(Action::Move(0)); 2];
    for (turn, first) in [(0u16, P), (13106, P), (13107, O), (0xFFFF, O)] {
        r.b.random_turn = turn;
        let (order, rng, _) = r.env(1, |b, env| b.strike_order(env, false));
        assert_eq!(order[0], first, "turn number {turn}");
        assert_eq!(rng, Rng::new(1), "no draw");
    }
}

#[test]
fn kings_rock_rolls_after_damage_and_flinches() {
    // ITEMEFFECT_KINGSROCK_SHELLBELL: `Random() % 100 < 10` when the move did damage, then
    // the KINGS_ROCK_AFFECTED flag and the target's HP.
    for flinch in [true, false] {
        let seed = seed_where(|s| (nth(s, 0) % 100 < 10) == flinch);
        let mut r = Run::wild(
            holding(m(PLAIN, 90, &[TACKLE, EMBER]), KINGS_ROCK),
            m(PLAIN, 10, &[SPLASH]),
        );
        r.b.actions = [None, Some(Action::Move(0))];
        let ((), rng, _) = r.env(seed, |b, env| {
            let u = b.move_use(env, P, TACKLE, Some(0));
            super::items::move_end(b, env, &u, hit(10));
        });
        assert_eq!(rng, draws(seed, 1));
        assert_eq!(r.b.battler(O).flinched, flinch);
    }
    // No damage: no roll. A move without the flag: the roll, no flinch.
    let seed = seed_where(|s| nth(s, 0) % 100 < 10);
    let mut r = Run::wild(
        holding(m(PLAIN, 90, &[TACKLE, EMBER]), KINGS_ROCK),
        m(PLAIN, 10, &[SPLASH]),
    );
    r.b.actions = [None, Some(Action::Move(0))];
    let ((), rng, _) = r.env(seed, |b, env| {
        let u = b.move_use(env, P, TACKLE, Some(0));
        super::items::move_end(b, env, &u, hit(0));
    });
    assert_eq!(rng, Rng::new(seed));
    let ((), rng, _) = r.env(seed, |b, env| {
        let u = b.move_use(env, P, EMBER, Some(1));
        super::items::move_end(b, env, &u, hit(10));
    });
    assert_eq!(rng, draws(seed, 1));
    assert!(!r.b.battler(O).flinched);
}

#[test]
fn brightpowder_lowers_the_foes_accuracy() {
    // Accuracy 100 at stage 0: calc 100, ×90/100 = 90 with BrightPowder.
    let hold = Some(HoldEffect::EvasionUp(10));
    assert!(calc::accuracy_hits_x(
        100,
        0,
        0,
        Ability::None,
        true,
        false,
        None,
        95
    ));
    assert!(!calc::accuracy_hits_x(
        100,
        0,
        0,
        Ability::None,
        true,
        false,
        hold,
        95
    ));
    assert!(calc::accuracy_hits_x(
        100,
        0,
        0,
        Ability::None,
        true,
        false,
        hold,
        89
    ));
}

#[test]
fn focus_band_rolls_after_the_damage_roll_and_hangs_on() {
    // Cmd_adjustnormaldamage: the damage roll, then `Random() % 100 < 10` for Focus Band.
    let seed = seed_where(|s| nth(s, 3) % 100 < 10);
    let mut r = Run::wild(
        m(PLAIN, 90, &[BIG_HIT]),
        holding(m(PLAIN, 10, &[SPLASH]), FOCUS_BAND),
    );
    r.b.battler_mut(O).mon.hp = 30;
    let (rng, l) = r.use_move(seed, P, 0);
    // accuracy, crit, damage, focus band, secondary.
    assert_eq!(rng, draws(seed, 5));
    assert_eq!(r.b.battler(O).mon.hp, 1);
    assert!(has(&l, "HungOnWithX@O Item(ItemId(12)) ~64"), "{l:?}");
    // The roll is drawn even when the hit would not knock it out.
    let mut r = Run::wild(
        m(PLAIN, 90, &[TACKLE]),
        holding(m(PLAIN, 10, &[SPLASH]), FOCUS_BAND),
    );
    let (rng, l) = r.use_move(seed, P, 0);
    assert_eq!(rng, draws(seed, 5));
    assert!(!starts(&l, "HungOnWithX"));
}

#[test]
fn focus_band_holds_on_set_damage_and_a_confusion_self_hit() {
    // adjustsetdamage (Sonic Boom): accuracy, the Focus Band roll, the secondary roll.
    let seed = seed_where(|s| nth(s, 1) % 100 < 10);
    let mut r = Run::wild(
        m(PLAIN, 90, &[SONIC]),
        holding(m(PLAIN, 10, &[SPLASH]), FOCUS_BAND),
    );
    r.b.battler_mut(O).mon.hp = 20;
    let (rng, l) = r.use_move(seed, P, 0);
    assert_eq!(rng, draws(seed, 3));
    assert_eq!(r.b.battler(O).mon.hp, 1);
    assert!(has(&l, "HungOnWithX@O Item(ItemId(12)) ~64"), "{l:?}");
    // adjustnormaldamage2 on the attacker (its own band): the 50% roll, the damage roll,
    // the band.
    let seed = seed_where(|s| nth(s, 0) & 1 == 0 && nth(s, 2) % 100 < 10);
    let mut r = Run::wild(
        holding(m(PLAIN, 90, &[TACKLE]), FOCUS_BAND),
        m(PLAIN, 10, &[SPLASH]),
    );
    r.b.battler_mut(P).confusion = 3;
    r.b.battler_mut(P).mon.hp = 1;
    let (rng, l) = r.use_move(seed, P, 0);
    assert_eq!(rng, draws(seed, 3));
    assert!(has(&l, "PkmnHurtItself@P ~64"), "{l:?}");
    assert!(has(&l, "HungOnWithX@P Item(ItemId(12)) ~64"), "{l:?}");
    assert_eq!(r.b.battler(P).mon.hp, 1);
}

#[test]
fn kings_rock_rolls_on_every_hit_of_a_multi_hit_move_and_after() {
    // `moveendto MOVEEND_NEXT_TARGET` after each hit, then the script's last `moveendfrom`.
    let draws_with = |item: Option<ItemId>| {
        let mut p = m(PLAIN, 90, &[MULTI]);
        p.held = item;
        let mut r = Run::wild(p, m(PLAIN, 10, &[SPLASH]));
        r.b.battler_mut(P).ability = Ability::SkillLink;
        let (rng, l) = r.use_move(1, P, 0);
        assert!(has(&l, "HitXTimes@P 5 ~64"), "{l:?}");
        (0..64).find(|&n| draws(1, n) == rng).expect("draw count")
    };
    assert_eq!(draws_with(Some(KINGS_ROCK)), draws_with(None) + 5 + 1);
}

#[test]
fn without_items_nothing_is_drawn_or_hashed() {
    // The Gen3 battle state hash does not move for a battle without items, forms or weather.
    let r = Run::wild(m(PLAIN, 50, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    assert_eq!(r.b.x, Default::default());
    let mut r2 = Run::wild(m(PLAIN, 50, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    assert_eq!(hash(&r.b), hash(&r2.b));
    r2.b.battler_mut(P).x.item = Some(ORAN);
    assert_ne!(hash(&r.b), hash(&r2.b));
}

// ---- Fairy ----

#[test]
fn fairy_has_the_gen6_matchups_and_is_special() {
    assert_eq!(FAIRY_MATCHUPS.len(), 12);
    assert!(!Type::Fairy.is_physical());
    let eff = |atk: Type, def: Type| {
        calc::type_effect(&TYPE_CHART, 15, false, atk, [def, def], 40, 100).eff
    };
    assert_eq!(eff(Type::Fairy, Type::Dragon), Effectiveness::Super);
    assert_eq!(eff(Type::Dragon, Type::Fairy), Effectiveness::NoEffect);
    assert_eq!(eff(Type::Fairy, Type::Steel), Effectiveness::NotVery);
    assert_eq!(eff(Type::Poison, Type::Fairy), Effectiveness::Super);
    // In battle: a Fairy move on a Dragon.
    let mut r = Run::wild(m(FAIRY, 90, &[FAIRY_HIT]), m(DRAGON, 10, &[SPLASH]));
    let (_, l) = r.use_move(1, P, 0);
    assert!(has(&l, "Hit O Super"), "{l:?}");
    let mut r = Run::wild(m(STEEL, 90, &[FAIRY_HIT]), m(STEEL, 10, &[SPLASH]));
    let (_, l) = r.use_move(1, P, 0);
    assert!(has(&l, "Hit O NotVery"), "{l:?}");
}

// ---- Mega Evolution / Primal Reversion ----

fn mega_mon(speed: u16) -> Pokemon {
    holding(m(MEGA_BASE, speed, &[TACKLE]), MEGA_STONE)
}

#[test]
fn the_player_mega_evolves_with_the_ring_before_the_turn() {
    let foe = m(PLAIN, 60, &[SPLASH]);
    let mut r = Run::new(trainer(), &[mega_mon(50)], &[foe], true, 7);
    assert!(r.b.can_mega(P));
    let before = hash(&r.b);
    let l = r.choose(Choice::Mega(0));
    let react = pos(
        &l,
        "MegaReacting@P mon:P0 Species(SpeciesId(7)) Item(ItemId(13)) ~64",
    );
    let change = pos(
        &l,
        "FormChange { side: Player, party: 0, species: SpeciesId(7) }",
    );
    let done = pos(&l, "MegaEvolved@P");
    assert!(react < change && change < done, "{l:?}");
    // The new speed (base 100: 105 at L50) orders the turn: the player moves first.
    assert_eq!(crate::battle::core::tests::first_mover(&l), 'P');
    assert!(done < pos(&l, "UsedMove@P"));
    let b = r.b.battler(P);
    assert_eq!(
        (b.mon.species, b.ability, b.types),
        (MEGA_FORM, Ability::ToughClaws, [Type::Steel; 2])
    );
    assert_eq!((b.stats[Stat::Atk as usize], b.stats[0]), (105, 100));
    // The party keeps its species; one Mega per battle.
    assert_eq!(r.b.party(P).get(0).unwrap().species, MEGA_BASE);
    assert!(r.b.x.mega_used[0] && !r.b.can_mega(P));
    assert_ne!(hash(&r.b), before);
    let l = r.choose(Choice::Mega(0));
    assert!(!starts(&l, "MegaReacting"), "{l:?}");
}

#[test]
fn no_ring_no_mega() {
    let mut r = Run::new(
        trainer(),
        &[mega_mon(50)],
        &[m(PLAIN, 60, &[SPLASH])],
        false,
        7,
    );
    assert!(!r.b.can_mega(P));
    let l = r.choose(Choice::Mega(0));
    assert!(
        !starts(&l, "MegaReacting") && starts(&l, "UsedMove@P"),
        "{l:?}"
    );
    assert_eq!(r.b.battler(P).mon.species, MEGA_BASE);
    // Nor without the stone.
    let plain = m(MEGA_BASE, 50, &[TACKLE]);
    let r = Run::new(trainer(), &[plain], &[m(PLAIN, 60, &[SPLASH])], true, 7);
    assert!(!r.b.can_mega(P));
    assert_eq!(r.b.x, Default::default());
}

#[test]
fn a_trainer_mega_evolves_at_once_and_a_wild_mon_never() {
    let mut r = Run::new(
        trainer(),
        &[m(PLAIN, 90, &[SPLASH])],
        &[mega_mon(50)],
        false,
        7,
    );
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "MegaReacting@O"), "{l:?}");
    assert_eq!(r.b.battler(O).mon.species, MEGA_FORM);
    let mut r = Run::wild(m(PLAIN, 90, &[SPLASH]), mega_mon(50));
    assert!(!r.b.can_mega(O));
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "MegaReacting"), "{l:?}");
}

#[test]
fn both_sides_mega_evolve_faster_first() {
    let mut r = Run::new(trainer(), &[mega_mon(50)], &[mega_mon(60)], true, 7);
    let l = r.choose(Choice::Mega(0));
    assert!(
        pos(&l, "MegaReacting@O") < pos(&l, "MegaReacting@P"),
        "{l:?}"
    );
}

#[test]
fn megas_order_by_full_speed() {
    // Swift Swim in rain doubles the slower player's speed: it Mega Evolves first.
    let mut r = Run::new(trainer(), &[mega_mon(40)], &[mega_mon(60)], true, 7);
    r.b.battler_mut(P).ability = Ability::SwiftSwim;
    r.b.x.weather = Weather::Rain;
    let l = r.choose(Choice::Mega(0));
    assert!(
        pos(&l, "MegaReacting@P") < pos(&l, "MegaReacting@O"),
        "{l:?}"
    );
}

#[test]
fn a_mega_comes_after_a_switch_and_before_the_moves() {
    // TryDoGimmicksBeforeMoves: the player's switch first, then the trainer's Mega, whose
    // Intimidate hits the mon that just came in, then its move.
    let party = [m(PLAIN, 50, &[SPLASH]), m(PLAIN, 50, &[SPLASH])];
    let foe = holding(m(MEGA_BASE_2, 10, &[SPLASH]), MEGA_STONE_2);
    let mut r = Run::new(trainer(), &party, &[foe], false, 7);
    let l = r.choose(Choice::Switch(1));
    let sent = pos(&l, "SendOut P1");
    let mega = pos(&l, "MegaReacting@O");
    let cut = pos(&l, "PkmnCutsAttackWith@O AbilityName(Intimidate) ~64");
    let used = pos(&l, "UsedMove@O");
    assert!(sent < mega && mega < cut && cut < used, "{l:?}");
    let b = r.b.battler(P);
    assert_eq!((b.mon.party, b.stages[Stat::Atk as usize]), (1, -1));
    assert_eq!(r.b.battler(O).mon.species, MEGA_INTIM);
}

#[test]
fn a_mon_without_pp_may_mega_evolve_and_struggle() {
    let mut p = mega_mon(50);
    p.pp = [0; 4];
    let mut r = Run::new(trainer(), &[p], &[m(PLAIN, 10, &[SPLASH])], true, 7);
    let l = r.choose(Choice::Mega(0));
    assert!(starts(&l, "MegaReacting@P"), "{l:?}");
    assert_eq!(r.b.battler(P).mon.species, MEGA_FORM);
    assert!(has(&l, &format!("UsedMove@P move:{}", STRUGGLE.0)), "{l:?}");
}

#[test]
fn a_mega_form_lasts_through_a_switch() {
    let party = [mega_mon(50), m(PLAIN, 50, &[SPLASH])];
    let mut r = Run::new(trainer(), &party, &[m(PLAIN, 10, &[SPLASH])], true, 7);
    r.choose(Choice::Mega(0));
    r.choose(Choice::Switch(1));
    assert_eq!(r.b.battler(P).mon.species, PLAIN);
    r.choose(Choice::Switch(0));
    let b = r.b.battler(P);
    assert_eq!((b.mon.species, b.ability), (MEGA_FORM, Ability::ToughClaws));
    assert_eq!(r.b.party(P).get(0).unwrap().species, MEGA_BASE);
}

#[test]
fn primal_reversion_on_entry_brings_its_weather() {
    let foe = holding(m(PRIMAL_BASE, 10, &[SPLASH]), RED_ORB);
    let mut r = Run::wild(m(PLAIN, 90, &[EMBER, WATER_GUN]), foe);
    let primal = pos(&r.intro, "PrimalReversion@O");
    let rain = pos(&r.intro, "HeavyRainBegan@O AbilityName(PrimordialSea) ~64");
    assert!(primal < rain, "{:?}", r.intro);
    assert!(
        !starts(&r.intro, "MadeItRain"),
        "the base form's Drizzle never acts"
    );
    assert_eq!(r.b.x.weather, Weather::HeavyRain);
    assert_eq!(r.b.x.mega_used, [false; 2]);
    // Heavy rain: a damaging Fire move fails after its attack string.
    let (_, l) = r.use_move(1, P, 0);
    assert_eq!(l, ["UsedMove@P move:3", "FizzledInRain@P ~64"]);
    assert_eq!(r.b.battler(O).mon.hp, 100);
    // The end of the turn: rain continues; when its source faints, the rain stops.
    let ((), _, l) = r.env(1, super::end_turn);
    assert_eq!(l[0], "RainContinues@P ~64");
    r.b.battler_mut(O).mon.hp = 0;
    let (_, _, l) = r.env(1, |b, env| b.try_faint(env, O));
    assert_eq!(l.last().unwrap(), "HeavyRainLifted@P ~64");
    assert_eq!(r.b.x.weather, Weather::None);
}

#[test]
fn primal_weathers_override_the_others_and_resist_them() {
    // FirstTurn in speed order: Drizzle's rain, then the foe's Desolate Land replaces it.
    let foe = holding(m(PLAIN, 10, &[SPLASH]), RED_ORB_SUN);
    let mut r = Run::wild(m(DRIZZLER, 90, &[WATER_GUN]), foe);
    let rain = pos(&r.intro, "MadeItRain@P AbilityName(Drizzle) ~64");
    assert!(rain < pos(&r.intro, "HarshSunBegan@O"), "{:?}", r.intro);
    assert_eq!(r.b.x.weather, Weather::HarshSun);
    // Drizzle cannot replace it.
    let ((), _, l) = r.env(1, |b, env| super::on_switch_in(b, env, P));
    assert!(l.is_empty());
    assert_eq!(r.b.x.weather, Weather::HarshSun);
    let (_, l) = r.use_move(1, P, 0);
    assert_eq!(l, ["UsedMove@P move:5", "EvaporatedInSun@P ~64"]);
    // A switch-out ends it too.
    let ((), _, l) = r.env(1, |b, env| super::left_field(b, env, Ability::DesolateLand));
    assert!(l.is_empty(), "still on the field");
    r.b.battler_mut(O).ability = Ability::None;
    let ((), _, l) = r.env(1, |b, env| super::left_field(b, env, Ability::DesolateLand));
    assert_eq!(l, ["HarshSunFaded@P ~64"]);
}

// ---- abilities ----

#[test]
fn weather_abilities_and_the_sandstorm() {
    // ABILITYEFFECT_ON_SWITCH_IN: BattleScript_DrizzleActivates / DroughtActivates /
    // SandstreamActivates.
    for (s, line, w) in [
        (
            DRIZZLER,
            "MadeItRain@P AbilityName(Drizzle) ~64",
            Weather::Rain,
        ),
        (
            SUNNY,
            "IntensifiedSun@P AbilityName(Drought) ~64",
            Weather::Sun,
        ),
        (
            SANDY,
            "WhippedUpSandstorm@P AbilityName(SandStream) ~64",
            Weather::Sand,
        ),
    ] {
        let r = Run::wild(m(s, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
        assert!(has(&r.intro, line), "{:?}", r.intro);
        assert_eq!(r.b.x.weather, w);
    }
    // ENDTURN_SANDSTORM: the message, then max HP / 16 to each non Rock / Ground / Steel
    // battler in turn order.
    let mut r = Run::wild(m(SANDY, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    let l = r.choose(Choice::Move(0));
    let rages = pos(&l, "SandstormRages@P ~64");
    let buffet = pos(&l, "BuffetedBySandstorm@O ~64");
    assert!(rages < buffet && l[buffet + 1] == "Hp O 100->94", "{l:?}");
    assert!(!starts(&l, "BuffetedBySandstorm@P"));
    // Sand Force is immune.
    r.b.battler_mut(O).ability = Ability::SandForce;
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "BuffetedBySandstorm"), "{l:?}");
}

#[test]
fn rain_and_sun_change_fire_and_water_damage() {
    let r = Run::wild(m(PLAIN, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    let (a, d) = (r.b.battler(P).fighter(), r.b.battler(O).fighter());
    let dmg = |a: &calc::Fighter, ty, weather| {
        let x = calc::Extra { weather };
        calc::base_damage_x(a, &d, 80, ty, false, calc::Field::default(), x)
    };
    use calc::DamageWeather::{None as Clear, Rain, Sun};
    let (fire, water) = (dmg(&a, Type::Fire, Clear), dmg(&a, Type::Water, Clear));
    assert!(dmg(&a, Type::Fire, Rain) < fire && dmg(&a, Type::Water, Rain) > water);
    assert!(dmg(&a, Type::Fire, Sun) > fire && dmg(&a, Type::Water, Sun) < water);
    // Solar Power: Sp. Atk x1.5 in the sun.
    let mut s = a;
    s.ability = Ability::SolarPower;
    assert!(dmg(&s, Type::Fire, Sun) > dmg(&a, Type::Fire, Sun));
    assert_eq!(dmg(&s, Type::Fire, Clear), fire);
}

#[test]
fn swift_swim_doubles_speed_in_rain() {
    let mut r = Run::wild(m(DRIZZLER, 30, &[SPLASH]), m(PLAIN, 50, &[SPLASH]));
    r.b.battler_mut(P).ability = Ability::SwiftSwim;
    r.b.actions = [Some(Action::Move(0)); 2];
    let (order, _, _) = r.env(1, |b, env| b.strike_order(env, false));
    assert_eq!(order[0], P);
}

#[test]
fn speed_boost_acts_at_the_end_of_a_leads_first_turn() {
    // isFirstTurn: 2 on entry, decremented at the end of TryDoEventsBeforeFirstTurn.
    let mut r = Run::wild(m(SPEEDY, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    assert_eq!(r.b.battler(P).x.first_turn, 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "StatAnim P true"));
    assert!(
        has(&l, "RaisedSpeed@P AbilityName(SpeedBoost) ~64"),
        "{l:?}"
    );
    assert_eq!(r.b.battler(P).stages[Stat::Speed as usize], 1);
}

#[test]
fn speed_boost_waits_for_the_first_turn_of_a_switch_in() {
    // Sent in during the turn: no boost at its end.
    let party = [m(PLAIN, 90, &[SPLASH]), m(SPEEDY, 90, &[SPLASH])];
    let kind = BattleKind::Wild {
        first_battle: false,
    };
    let mut r = Run::new(kind, &party, &[m(PLAIN, 10, &[SPLASH])], false, 7);
    let l = r.choose(Choice::Switch(1));
    assert!(!starts(&l, "RaisedSpeed"), "{l:?}");
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "RaisedSpeed@P"), "{l:?}");
}

#[test]
fn speed_boost_counts_the_turn_of_a_replacement_after_the_end_turn_faint() {
    // BattleTurnPassed: HandleFaintedMonActions sends the replacement in, then
    // TurnValuesCleanUp(FALSE) decrements its isFirstTurn: it boosts at the next turn's end.
    let mut lead = m(PLAIN, 90, &[SPLASH]);
    lead.hp = 1;
    let party = [lead, m(SPEEDY, 90, &[SPLASH])];
    let kind = BattleKind::Wild {
        first_battle: false,
    };
    let mut r = Run::new(kind, &party, &[m(SANDY, 10, &[SPLASH])], false, 7);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "BuffetedBySandstorm@P ~64"), "{l:?}");
    assert_eq!(r.b.battler(P).mon.hp, 0);
    r.choose(Choice::Switch(1));
    assert_eq!(r.b.battler(P).x.first_turn, 1);
    let l = r.choose(Choice::Move(0));
    assert!(
        has(&l, "RaisedSpeed@P AbilityName(SpeedBoost) ~64"),
        "{l:?}"
    );
}

#[test]
fn solar_power_costs_an_eighth_in_the_sun() {
    let mut r = Run::wild(m(SUNNY, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(P).ability = Ability::SolarPower;
    let l = r.choose(Choice::Move(0));
    let hurt = pos(&l, "SolarPowerHurt@P AbilityName(SolarPower) ~64");
    assert_eq!(l[hurt + 1], "Hp P 100->88", "{l:?}");
}

#[test]
fn rain_dish_heals_in_rain() {
    let mut r = Run::wild(m(DRIZZLER, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).ability = Ability::RainDish;
    r.b.battler_mut(O).mon.hp = 50;
    let l = r.choose(Choice::Move(0));
    let healed = pos(&l, "AbilityRestoredHp@O AbilityName(RainDish) ~64");
    assert_eq!(l[healed + 1], "Hp O 50->56", "{l:?}");
    // Not at full HP.
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "AbilityRestoredHp@O AbilityName(RainDish) ~64"));
    r.b.battler_mut(O).mon.hp = 100;
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "AbilityRestoredHp"), "{l:?}");
}

#[test]
fn sand_veil_in_a_sandstorm() {
    // Cmd_weatherdamage skips it; Cmd_accuracycheck: ×80/100 after Compound Eyes.
    let mut r = Run::wild(m(SANDY, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).ability = Ability::SandVeil;
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "BuffetedBySandstorm"), "{l:?}");
    let hits = |veil, roll| calc::accuracy_hits_x(100, 0, 0, Ability::None, true, veil, None, roll);
    assert!(hits(false, 85) && !hits(true, 85) && hits(true, 79));
    // The core passes it only in a sandstorm.
    let seed = seed_where(|s| nth(s, 0) % 100 >= 80);
    let mut r = Run::wild(m(SANDY, 90, &[TACKLE]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).ability = Ability::SandVeil;
    let (_, l) = r.use_move(seed, P, 0);
    assert!(starts(&l, "AttackMissed"), "{l:?}");
    r.b.x.weather = Weather::None;
    let (_, l) = r.use_move(seed, P, 0);
    assert!(starts(&l, "Hp O"), "{l:?}");
}

#[test]
fn levitate_and_mold_breaker() {
    // CheckWonderGuardAndLevitate: the accuracy roll, then "makes GROUND moves miss".
    let mut r = Run::wild(m(PLAIN, 90, &[MUD_SHOT]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).ability = Ability::Levitate;
    let (_, l) = r.use_move(1, P, 0);
    assert!(
        has(&l, "MakesGroundMiss@O AbilityName(Levitate) ~64"),
        "{l:?}"
    );
    assert_eq!(r.b.battler(O).mon.hp, 100);
    // Mold Breaker ignores it for the move, then the ability is back.
    r.b.battler_mut(P).ability = Ability::MoldBreaker;
    let (_, l) = r.use_move(1, P, 0);
    assert!(starts(&l, "Hp O 100->"), "{l:?}");
    assert_eq!(r.b.battler(O).ability, Ability::Levitate);
    // Its announcement on entry.
    let ((), _, l) = r.env(1, |b, env| super::on_switch_in(b, env, P));
    assert_eq!(l, ["BreaksTheMold@P ~64"]);
}

#[test]
fn shell_armor_skips_the_crit_roll() {
    let mut r = Run::wild(m(PLAIN, 90, &[TACKLE]), m(PLAIN, 10, &[SPLASH]));
    let (rng, _) = r.use_move(1, P, 0);
    assert_eq!(rng, draws(1, 4), "accuracy, crit, damage, secondary");
    for a in [Ability::ShellArmor, Ability::BattleArmor] {
        r.b.battler_mut(O).ability = a;
        let (rng, _) = r.use_move(1, P, 0);
        assert_eq!(rng, draws(1, 3));
    }
}

#[test]
fn shell_armor_skips_the_crit_roll_of_an_immune_multi_hit() {
    let mut r = Run::wild(m(PLAIN, 90, &[MULTI]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).types = [Type::Ghost; 2];
    let (plain, l) = r.use_move(1, P, 0);
    assert!(has(&l, "ItDoesntAffect@O ~64"), "{l:?}");
    r.b.battler_mut(O).ability = Ability::ShellArmor;
    let (armored, _) = r.use_move(1, P, 0);
    let n = |rng| (0..32).find(|&n| draws(1, n) == rng).expect("draw count");
    assert_eq!(n(armored) + 1, n(plain));
}

/// The damage of `mov` by the player with `ability` against the foe, on a non-crit seed.
fn damage_with(r: &mut Run, ability: Ability, mov: MoveId) -> i32 {
    let seed = seed_where(|s| !calc::is_crit(0, nth(s, 0)));
    r.b.battler_mut(P).ability = ability;
    let (d, _, _) = r.env(seed, |b, env| {
        let u = b.move_use(env, P, mov, None);
        b.damage(env, &u).damage
    });
    d
}

#[test]
fn power_abilities() {
    let mut r = Run::wild(m(PLAIN, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    let base = damage_with(&mut r, Ability::None, TACKLE);
    // Technician x1.5 on power 60 or less, not on more.
    assert!(damage_with(&mut r, Ability::Technician, TACKLE) > base);
    let heavy = damage_with(&mut r, Ability::None, HEAVY_HIT);
    assert_eq!(damage_with(&mut r, Ability::Technician, HEAVY_HIT), heavy);
    // Tough Claws x1.3 on contact only.
    assert!(damage_with(&mut r, Ability::ToughClaws, TACKLE) > base);
    let ember = damage_with(&mut r, Ability::None, EMBER);
    assert_eq!(damage_with(&mut r, Ability::ToughClaws, EMBER), ember);
    // Sand Force x1.3 on Rock / Ground / Steel in a sandstorm.
    let rock = damage_with(&mut r, Ability::SandForce, ROCK_HIT);
    r.b.x.weather = Weather::Sand;
    assert!(damage_with(&mut r, Ability::SandForce, ROCK_HIT) > rock);
    assert_eq!(damage_with(&mut r, Ability::SandForce, TACKLE), base);
}

#[test]
fn ate_abilities_change_normal_moves() {
    let mut r = Run::wild(m(PLAIN, 90, &[TACKLE]), m(DRAGON, 10, &[SPLASH]));
    for (a, ty) in [
        (Ability::Aerilate, Type::Flying),
        (Ability::Pixilate, Type::Fairy),
        (Ability::Refrigerate, Type::Ice),
    ] {
        r.b.battler_mut(P).ability = a;
        let (u, _, _) = r.env(1, |b, env| b.move_use(env, P, TACKLE, None));
        assert_eq!(u.ty, ty);
        let (u, _, _) = r.env(1, |b, env| b.move_use(env, P, EMBER, None));
        assert_eq!(u.ty, Type::Fire);
    }
    // Pixilate's Fairy Tackle is super effective on a Dragon.
    r.b.battler_mut(P).ability = Ability::Pixilate;
    let (_, l) = r.use_move(1, P, 0);
    assert!(has(&l, "Hit O Super"), "{l:?}");
}

#[test]
fn filter_softens_super_effective_hits() {
    let mut r = Run::wild(m(PLAIN, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).types = [Type::Fire; 2];
    let full = damage_with(&mut r, Ability::None, WATER_GUN);
    r.b.battler_mut(O).ability = Ability::Filter;
    let soft = damage_with(&mut r, Ability::None, WATER_GUN);
    assert!(soft < full && soft >= full * 3 / 4 - 1, "{soft} vs {full}");
}

#[test]
fn parental_bond_hits_twice() {
    let mut r = Run::wild(m(PLAIN, 90, &[TACKLE]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(P).ability = Ability::ParentalBond;
    let (_, l) = r.use_move(1, P, 0);
    let hits: Vec<_> = l.iter().filter(|x| x.starts_with("Hp O")).collect();
    assert_eq!(hits.len(), 2, "{l:?}");
    assert!(has(&l, "HitXTimes@P 2 ~64"), "{l:?}");
    // Not Explosion / Self-Destruct (among the Gen7 exclusions).
    let mut r = Run::wild(m(PLAIN, 90, &[EXPLODE]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(P).ability = Ability::ParentalBond;
    let (_, l) = r.use_move(1, P, 0);
    let hits: Vec<_> = l.iter().filter(|x| x.starts_with("Hp O")).collect();
    assert_eq!(hits.len(), 1, "{l:?}");
}

#[test]
fn skill_link_always_hits_five_times() {
    let mut r = Run::wild(m(PLAIN, 90, &[MULTI]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(P).ability = Ability::SkillLink;
    for seed in 1..20 {
        r.b.battler_mut(O).mon.hp = 100;
        let (_, l) = r.use_move(seed, P, 0);
        assert!(has(&l, "HitXTimes@P 5 ~64"), "{l:?}");
    }
}

#[test]
fn magic_bounce_sends_status_moves_back() {
    let mut r = Run::wild(m(PLAIN, 90, &[GROWL]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).ability = Ability::MagicBounce;
    let (_, l) = r.use_move(1, P, 0);
    assert_eq!(l[0], "UsedMove@P move:2");
    assert!(
        starts(&l, "BouncedBack@O AbilityName(MagicBounce)"),
        "{l:?}"
    );
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], -1);
    assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], 0);
    assert_eq!(r.b.battler(P).mon.pp[0], 19);
    // A Mold Breaker attacker is not bounced.
    r.b.battler_mut(P).ability = Ability::MoldBreaker;
    r.use_move(1, P, 0);
    assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], -1);
}

#[test]
fn prankster_moves_status_moves_first_but_not_on_dark() {
    let mut r = Run::wild(m(PLAIN, 10, &[GROWL]), m(PLAIN, 90, &[TACKLE]));
    r.b.battler_mut(P).ability = Ability::Prankster;
    r.b.actions = [Some(Action::Move(0)); 2];
    let (order, _, _) = r.env(1, |b, env| b.strike_order(env, false));
    assert_eq!(order[0], P);
    let mut r = Run::wild(m(PLAIN, 10, &[GROWL]), m(DARK, 90, &[TACKLE]));
    r.b.battler_mut(P).ability = Ability::Prankster;
    let (_, l) = r.use_move(1, P, 0);
    assert_eq!(l, ["UsedMove@P move:2", "ItDoesntAffect@O ~64"]);
}

#[test]
fn steadfast_raises_speed_on_a_flinch() {
    let mut r = Run::wild(m(PLAIN, 10, &[TACKLE]), m(PLAIN, 90, &[BITE_ALL]));
    r.b.battler_mut(P).ability = Ability::Steadfast;
    let l = r.choose(Choice::Move(0));
    let f = pos(&l, "PkmnFlinched@P");
    assert_eq!(l[f + 1], "StatAnim P true", "{l:?}");
    assert_eq!(
        l[f + 2],
        "RaisedSpeed@P AbilityName(Steadfast) ~64",
        "{l:?}"
    );
    assert_eq!(r.b.battler(P).stages[Stat::Speed as usize], 1);
}

#[test]
fn lightning_rod_absorbs_electric_moves() {
    let mut r = Run::wild(m(PLAIN, 90, &[SHOCK]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).ability = Ability::LightningRod;
    let (_, l) = r.use_move(1, P, 0);
    assert!(!starts(&l, "Hp O"), "{l:?}");
    assert_eq!(r.b.battler(O).stages[Stat::SpAtk as usize], 1);
    r.b.battler_mut(O).stages[Stat::SpAtk as usize] = 6;
    let (_, l) = r.use_move(1, P, 0);
    assert!(
        has(
            &l,
            "PkmnSXMadeItIneffective@O AbilityName(LightningRod) ~64"
        ),
        "{l:?}"
    );
}

#[test]
fn shadow_tag_traps_the_foe() {
    let mut r = Run::wild(m(PLAIN, 90, &[SPLASH]), m(PLAIN, 10, &[SPLASH]));
    r.b.battler_mut(O).ability = Ability::ShadowTag;
    assert_eq!(
        super::run_block(&r.b, P),
        Some(super::RunBlock::Ability {
            owner: O,
            ability: Ability::ShadowTag
        })
    );
    assert!(super::switch_block(&r.b, P));
    // Gen3: a Shadow Tag foe traps a Shadow Tag battler too; Gen4 exempts it.
    r.b.battler_mut(P).ability = Ability::ShadowTag;
    assert!(super::switch_block(&r.b, P));
    r.b.battler_mut(O).ability = Ability::ShadowTagGen4;
    assert!(super::switch_block(&r.b, P));
    assert_eq!(
        super::run_block(&r.b, P),
        Some(super::RunBlock::Ability {
            owner: O,
            ability: Ability::ShadowTagGen4
        })
    );
    r.b.battler_mut(P).ability = Ability::ShadowTagGen4;
    assert!(!super::switch_block(&r.b, P));
}
