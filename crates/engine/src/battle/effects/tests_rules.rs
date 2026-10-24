//! Silice's per-battle rules ([`crate::battle::rules`]): the inverse chart, field rules, trainer
//! lines and trainer items, on the Silice test tables ([`super::SGame`]).

use super::*;
use crate::battle::rules::{
    BattleRules, FieldRule, LineWhen, RuleEffect, TrainerLine, Turns, Var, What, When, Who,
};
use crate::data::{ItemUse, MedicineEffect, TrainerId};

const SOUND_HIT: MoveId = MoveId(19);
const POTION: ItemId = ItemId(40);
const T_START: TextId = TextId(901);
const T_LAST: TextId = TextId(902);
const T_LINE: TextId = TextId(903);
const T_WARD: TextId = TextId(904);

const NOT_WATER: &[Type] = &[Type::Water];

/// Rule 1: at the start the foes' evasion +1 and the player's lead confused; every second
/// turn's end, accuracy −1 for the non-Water battlers (not below −3).
static ROLL: FieldRule = FieldRule {
    effects: &[
        RuleEffect::new(
            When::Start,
            Who::FOE,
            What::Stage {
                stat: Stat::Evasion,
                delta: 1,
                limit: 0,
            },
        ),
        RuleEffect::new(When::Start, Who::PLAYER, What::Confuse),
        RuleEffect::new(
            When::TurnEnd(Turns::cycle(2, 1)),
            Who::BOTH.lacks(NOT_WATER),
            What::Stage {
                stat: Stat::Accuracy,
                delta: -1,
                limit: -3,
            },
        ),
    ],
};

/// Rule 2: a message each turn start, the foe's last mon +2 Attack (named with the trainer),
/// sound moves fail, Electric speed ×2.
static STROBE: FieldRule = FieldRule {
    effects: &[
        RuleEffect::new(When::TurnStart(Turns::ALL), Who::BOTH, What::Say).say(T_START),
        RuleEffect::new(
            When::LastIn,
            Who::FOE,
            What::Stage {
                stat: Stat::Atk,
                delta: 2,
                limit: 0,
            },
        )
        .say(T_LAST)
        .var(Var::Trainer),
        RuleEffect::new(When::During(Turns::ALL), Who::BOTH, What::SoundFails),
        RuleEffect::new(
            When::During(Turns::ALL),
            Who::BOTH.has(&[Type::Electric]),
            What::Speed { num: 2, den: 1 },
        ),
    ],
};

/// Rule 3: the foe takes half damage and its attacker loses a quarter of it (turns 1–3);
/// the player shakes off confusion; every turn end the foe loses 1/16.
static PRISM: FieldRule = FieldRule {
    effects: &[
        RuleEffect::new(
            When::During(Turns::range(1, 3)),
            Who::FOE,
            What::DamageTaken { num: 1, den: 2 },
        ),
        RuleEffect::new(
            When::During(Turns::range(1, 3)),
            Who::PLAYER,
            What::DrainDealt { num: 1, den: 4 },
        ),
        RuleEffect::new(
            When::During(Turns::ALL),
            Who::PLAYER,
            What::ConfusionWard {
                chance: 100,
                max_turns: 0,
            },
        )
        .say(T_WARD),
        RuleEffect::new(When::TurnEnd(Turns::ALL), Who::FOE, What::Hurt(16)),
    ],
};

static LINES: [TrainerLine; 3] = [
    TrainerLine {
        when: LineWhen::SendOut(0),
        text: T_LINE,
    },
    TrainerLine {
        when: LineWhen::LowHp,
        text: TextId(905),
    },
    TrainerLine {
        when: LineWhen::Mega,
        text: TextId(906),
    },
];

static ITEMS: [ItemId; 1] = [POTION];

/// [`SGame`] with the rules, trainer 1's lines and items, and a Potion (20 HP).
struct RGame;

impl GameData for RGame {
    fn map(&self, m: MapId) -> &'static MapData {
        SGame.map(m)
    }
    fn sprite(&self, g: GfxId) -> Option<&'static SpriteData> {
        SGame.sprite(g)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        SGame.player_sprite()
    }
    fn font(&self) -> &'static Font {
        SGame.font()
    }
    fn window(&self) -> &'static WindowGfx {
        SGame.window()
    }
    fn text(&self, t: TextId) -> &'static [u8] {
        SGame.text(t)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        SGame.ui_text(t)
    }
    fn behavior(&self, b: u8) -> Behavior {
        SGame.behavior(b)
    }
    fn script(&self, s: ScriptId, c: Ctx) -> Script {
        SGame.script(s, c)
    }
    fn new_game(&self) -> NewGame {
        SGame.new_game()
    }
    fn battle(&self) -> &'static BattleTables {
        &TABLES
    }
    fn battle_text(&self, s: Str) -> &'static [u8] {
        SGame.battle_text(s)
    }
    fn hold_effect(&self, id: ItemId) -> Option<HoldEffect> {
        SGame.hold_effect(id)
    }
    fn form_changes(&self) -> &'static [FormChange] {
        SGame.form_changes()
    }
    fn battle_rule(&self, id: u8) -> Option<&'static FieldRule> {
        match id {
            1 => Some(&ROLL),
            2 => Some(&STROBE),
            3 => Some(&PRISM),
            _ => None,
        }
    }
    fn trainer_lines(&self, id: TrainerId) -> &'static [TrainerLine] {
        if id == TrainerId(1) { &LINES } else { &[] }
    }
    fn trainer_items(&self, id: TrainerId) -> &'static [ItemId] {
        if id == TrainerId(1) { &ITEMS } else { &[] }
    }
    fn item_use(&self, id: ItemId) -> ItemUse {
        match id {
            POTION => ItemUse::Medicine(MedicineEffect::Heal(20)),
            _ => ItemUse::None,
        }
    }
}

struct RRun {
    b: Battle,
    rng: Rng,
    intro: Vec<String>,
}

impl RRun {
    fn new(kind: BattleKind, rules: BattleRules, player: &[Pokemon], foes: &[Pokemon]) -> RRun {
        let mut rng = Rng::new(7);
        let setup = BattleSetup {
            kind,
            opponents: party(foes),
            environment: Environment::Grass,
            double: None,
            rules,
        };
        let mut b = Battle::new(&RGame, setup, &party(player), &mut rng);
        b.enable_player_forms(&RGame, |f| f == MEGA_RING);
        let mut out = Vec::new();
        b.start(&RGame, &mut rng, &mut out);
        RRun {
            b,
            rng,
            intro: lines(&out),
        }
    }

    fn choose(&mut self, c: Choice) -> Vec<String> {
        let mut out = Vec::new();
        self.b.choose(&RGame, &mut self.rng, c, &mut out);
        lines(&out)
    }
}

fn rule(id: u8) -> BattleRules {
    BattleRules {
        field: Some(id),
        ..BattleRules::default()
    }
}

fn wild() -> BattleKind {
    BattleKind::Wild {
        first_battle: false,
    }
}

#[test]
fn the_inverse_chart_swaps_super_and_not_very_and_turns_immunity_into_super() {
    let eff = |ty, def, inverse| {
        calc::type_effect_in(&TYPE_CHART, 15, false, ty, [def, def], 40, 100, inverse)
    };
    assert_eq!(
        eff(Type::Water, Type::Fire, false).eff,
        Effectiveness::Super
    );
    let r = eff(Type::Water, Type::Fire, true);
    assert_eq!((r.eff, r.damage), (Effectiveness::NotVery, 50));
    assert_eq!(
        eff(Type::Normal, Type::Rock, true).eff,
        Effectiveness::Super
    );
    let r = eff(Type::Normal, Type::Ghost, true);
    assert_eq!((r.eff, r.damage), (Effectiveness::Super, 200));
    // Under Foresight the immunity entries are skipped: neutral.
    let r = calc::type_effect_in(
        &TYPE_CHART,
        15,
        true,
        Type::Normal,
        [Type::Ghost; 2],
        40,
        100,
        true,
    );
    assert_eq!(r.eff, Effectiveness::Normal);
}

#[test]
fn an_inverse_battle_hits_with_the_inverse_chart_and_says_its_intro() {
    let rules = BattleRules {
        inverse: true,
        intro: Some(T_START),
        ..BattleRules::default()
    };
    let mut r = RRun::new(
        trainer(),
        rules,
        &[m(PLAIN, 90, &[WATER_GUN])],
        &[m(SUNNY, 10, &[SPLASH])],
    );
    assert!(starts(&r.intro, "Authored(TextId(901))@P"), "{:?}", r.intro);
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "NotVeryEffective"), "{l:?}");
    assert!(!starts(&l, "SuperEffective"), "{l:?}");
}

#[test]
fn default_rules_hash_as_a_gen3_battle() {
    let a = RRun::new(
        wild(),
        BattleRules::default(),
        &[m(PLAIN, 90, &[TACKLE])],
        &[m(PLAIN, 10, &[SPLASH])],
    );
    let b = Run::wild(m(PLAIN, 90, &[TACKLE]), m(PLAIN, 10, &[SPLASH]));
    assert_eq!(hash(&a.b), hash(&b.b));
    let c = RRun::new(
        wild(),
        BattleRules {
            inverse: true,
            ..BattleRules::default()
        },
        &[m(PLAIN, 90, &[TACKLE])],
        &[m(PLAIN, 10, &[SPLASH])],
    );
    assert_ne!(hash(&a.b), hash(&c.b));
}

#[test]
fn start_effects_and_the_roll_every_second_turn() {
    let mut r = RRun::new(
        trainer(),
        rule(1),
        &[m(PLAIN, 90, &[SPLASH])],
        &[m(DRIZZLER, 10, &[SPLASH])],
    );
    assert_eq!(r.b.battler(O).stages[Stat::Evasion as usize], 1);
    assert!(r.b.battler(P).confusion > 0);
    assert!(starts(&r.intro, "PkmnBecameConfused@P"), "{:?}", r.intro);
    r.b.battler_mut(P).confusion = 0;
    r.choose(Choice::Move(0));
    assert_eq!(r.b.battler(P).stages[Stat::Accuracy as usize], 0);
    for _ in 0..7 {
        r.choose(Choice::Move(0));
    }
    // Turns 2, 4, 6, 8 ended: −1 each, but never below −3; the Water foe keeps its accuracy.
    assert_eq!(r.b.battler(P).stages[Stat::Accuracy as usize], -3);
    assert_eq!(r.b.battler(O).stages[Stat::Accuracy as usize], 0);
}

#[test]
fn turn_start_text_last_mon_sound_and_speed_rules() {
    let mut r = RRun::new(
        trainer(),
        rule(2),
        &[m(MEGA_BASE_2, 10, &[SOUND_HIT])],
        &[m(PLAIN, 15, &[TACKLE])],
    );
    // The single foe mon is its last: +2 Attack, the trainer named.
    assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], 2);
    assert!(
        starts(&r.intro, "Authored(TextId(902))@O TrainerName TrainerName"),
        "{:?}",
        r.intro
    );
    // The turn-start text comes before the choice.
    assert!(starts(&r.intro, "Authored(TextId(901))"), "{:?}", r.intro);
    // The Electric player (speed 10, ×2 = 20) outspeeds the foe (15); its sound move fails.
    let l = r.choose(Choice::Move(0));
    assert!(pos(&l, "UsedMove@P") < pos(&l, "UsedMove@O"), "{l:?}");
    assert!(starts(&l, "ButItFailed@P"), "{l:?}");
    assert_eq!(r.b.battler(O).mon.hp, r.b.battler(O).mon.max_hp);
}

#[test]
fn the_prism_halves_damage_drains_the_attacker_and_wards_confusion() {
    let plain = RRun::new(
        trainer(),
        BattleRules::default(),
        &[m(PLAIN, 90, &[TACKLE])],
        &[m(PLAIN, 10, &[SPLASH])],
    );
    let mut r = RRun::new(
        trainer(),
        rule(3),
        &[m(PLAIN, 90, &[TACKLE])],
        &[m(PLAIN, 10, &[SPLASH])],
    );
    let mut p = plain;
    p.choose(Choice::Move(0));
    let full = p.b.battler(O).mon.max_hp - p.b.battler(O).mon.hp;
    r.choose(Choice::Move(0));
    let o = r.b.battler(O).mon;
    let taken = o.max_hp - o.hp - o.max_hp / 16;
    assert!(taken < full && taken * 2 <= full + 2, "{taken} vs {full}");
    let lost = r.b.battler(P).mon.max_hp - r.b.battler(P).mon.hp;
    assert_eq!(lost, (taken / 4).max(1));
    // The ward: a move's confusion is shaken off at once.
    let ((), _, l) = {
        let mut rng = Rng::new(1);
        let mut out = Vec::new();
        let mut env = Env::new(&TABLES, &mut rng, &mut out);
        r.b.battler_mut(P).confusion = 3;
        r.b.rule_confusion_ward(&mut env, P);
        ((), rng, lines(&out))
    };
    assert_eq!(r.b.battler(P).confusion, 0);
    assert!(starts(&l, "Authored(TextId(904))@P"), "{l:?}");
}

#[test]
fn trainer_lines_on_send_out_low_hp_and_mega() {
    let mut r = RRun::new(
        trainer(),
        BattleRules::default(),
        &[m(PLAIN, 90, &[BIG_HIT])],
        &[mega_mon(50)],
    );
    let send = pos(&r.intro, "Authored(TextId(903))@O");
    assert!(send < pos(&r.intro, "PkmnSentOut@O"), "{:?}", r.intro);
    let l = r.choose(Choice::Move(0));
    // The Mega line before the Mega, then the low-HP line once hit.
    assert!(
        pos(&l, "Authored(TextId(906))@O") < pos(&l, "MegaReacting@O"),
        "{l:?}"
    );
    if r.b.battler(O).mon.hp > 0 && r.b.battler(O).mon.hp * 2 < r.b.battler(O).mon.max_hp {
        assert!(starts(&l, "Authored(TextId(905))@O"), "{l:?}");
    }
    // Each line once.
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "Authored(TextId(903))"), "{l:?}");
    assert!(!starts(&l, "Authored(TextId(906))"), "{l:?}");
}

#[test]
fn a_trainer_uses_its_potion_once_under_a_quarter_hp() {
    let mut r = RRun::new(
        trainer(),
        BattleRules::default(),
        &[m(PLAIN, 90, &[SPLASH])],
        &[m(PLAIN, 10, &[SPLASH])],
    );
    // Full HP: (max − hp) = 0 is not over 20, and HP is not under a quarter: no item.
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "Trainer1UsedItem"), "{l:?}");
    // The AI chose this turn's action at the end of the last one: the item comes next turn.
    r.b.battler_mut(O).mon.hp = 10;
    r.choose(Choice::Move(0));
    let l = r.choose(Choice::Move(0));
    assert!(
        pos(&l, "Trainer1UsedItem@O") < pos(&l, "UsedMove@P"),
        "{l:?}"
    );
    assert!(starts(&l, "ItemRestoredHealth@O"), "{l:?}");
    assert_eq!(r.b.battler(O).mon.hp, 30);
    r.b.battler_mut(O).mon.hp = 10;
    r.choose(Choice::Move(0));
    let l = r.choose(Choice::Move(0));
    assert!(!starts(&l, "Trainer1UsedItem"), "{l:?}");
}
