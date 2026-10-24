//! Silice's battle rules (SIL3, plan §3.4): the chapters' field rules
//! (`GameData::battle_rule`), the trainers whose battles always have rules
//! (`GameData::trainer_rules`) and the trainers' mid-battle lines (`GameData::trainer_lines`).
//! The engine runs them (`engine::battle::rules`); this is the data, as the chapters specify:
//!
//! | Rule | Chapter | Battles | Effect |
//! |---|---|---|---|
//! | [`DEAF_ROOM`] | 2 | the Mélomane | sound moves fail, both sides |
//! | [`STROBE`] | 3 | Tauber & Hocseat | white / UV / neon each turn: UV ×0.8 accuracy against Ghosts, neon ×2 Electric speed |
//! | [`ROLL`] | 4 | the ship's crew, Samuel | every 2nd turn's end, accuracy −1 (min −3) for the non-Water, non-Fighting |
//! | [`ROLL_CONFUSED`] | 4 | Samuel after a failed sobriety test | [`ROLL`], the player's lead confused |
//! | [`RUM`] | 4 | Gwada on the coupée | his mons enter with evasion +1, accuracy −1 |
//! | [`PETALS`] | 6 | the Bushido sanctuary | 8 turns: Grass and Bug heal 1/16 each turn's end |
//! | [`INJECTOR_ATK`], [`INJECTOR_SPA`] | 7 | Gogo, Romain | the last mon mutates: Atk (SpAtk) +2, Speed +2, Def −1, accuracy −1, then loses 1/8 a turn |
//! | [`OVERDOSE`] | 8 | Gogo, Romain at Évalon | their mons enter with Atk +1, SpAtk +1, lose 1/16 a turn |
//! | [`PRISM`] | 10 | Revenge | turns 1–3: the foe takes ×0.5, the player's attacker loses 1/4 of its damage |
//! | [`HEART`] | 10 | the Tower's 1F | the Cœur d'Évalon: half the player's confusions shaken off, the rest 2 turns at most |
//!
//! Readings where the source is loose (author questions in the plan's "as built"):
//! - "all his mons start with" (Gwada's rum, the overdose): each mon as it enters
//!   ([`When::SwitchIn`]); Gen3 stat stages belong to the battler on the field.
//! - The injector's "+4 / −2" of the notes: the source's own split (+2 / +2, −1 / −1); in the
//!   two single battles of the fallback, "the last mon" is each trainer's.
//! - The Cœur d'Évalon: Ch10's 50 % / 2 turns, not Ch8's full immunity.

use crate::consts::*;
use engine::battle::rules::{FieldRule, RuleEffect, Turns, Var, What, When, Who};
use engine::battle::{BattleRules, LineWhen, TrainerLine};
use engine::data::TrainerId;
use engine::mon::{Stat, Type};

/// Field rule ids (`BattleRules::field`).
pub const DEAF_ROOM: u8 = 1;
pub const STROBE: u8 = 2;
pub const ROLL: u8 = 3;
pub const ROLL_CONFUSED: u8 = 4;
pub const RUM: u8 = 5;
pub const PETALS: u8 = 6;
pub const INJECTOR_ATK: u8 = 7;
pub const INJECTOR_SPA: u8 = 8;
pub const OVERDOSE: u8 = 9;
pub const PRISM: u8 = 10;
pub const HEART: u8 = 11;

const fn stage(when: When, who: Who, stat: Stat, delta: i8) -> RuleEffect {
    RuleEffect::new(
        when,
        who,
        What::Stage {
            stat,
            delta,
            limit: 0,
        },
    )
}

const ROLL_EFFECT: RuleEffect = RuleEffect::new(
    When::TurnEnd(Turns::cycle(2, 1)),
    Who::BOTH.lacks(&[Type::Water, Type::Fighting]),
    What::Stage {
        stat: Stat::Accuracy,
        delta: -1,
        limit: -3,
    },
)
.say(Port_Ressac_Gym_Roulis);

static R_DEAF_ROOM: FieldRule = FieldRule {
    effects: &[RuleEffect::new(
        When::During(Turns::ALL),
        Who::BOTH,
        What::SoundFails,
    )],
};

static R_STROBE: FieldRule = FieldRule {
    effects: &[
        RuleEffect::new(When::TurnStart(Turns::cycle(3, 0)), Who::BOTH, What::Say)
            .say(Nocturne_Strobe_Blanche),
        RuleEffect::new(When::TurnStart(Turns::cycle(3, 1)), Who::BOTH, What::Say)
            .say(Nocturne_Strobe_UV),
        RuleEffect::new(
            When::During(Turns::cycle(3, 1)),
            Who::BOTH.has(&[Type::Ghost]),
            What::Accuracy { num: 4, den: 5 },
        ),
        RuleEffect::new(When::TurnStart(Turns::cycle(3, 2)), Who::BOTH, What::Say)
            .say(Nocturne_Strobe_Neon),
        RuleEffect::new(
            When::During(Turns::cycle(3, 2)),
            Who::BOTH.has(&[Type::Electric]),
            What::Speed { num: 2, den: 1 },
        ),
    ],
};

static R_ROLL: FieldRule = FieldRule {
    effects: &[ROLL_EFFECT],
};

static R_ROLL_CONFUSED: FieldRule = FieldRule {
    effects: &[
        RuleEffect::new(When::Start, Who::PLAYER, What::Confuse),
        ROLL_EFFECT,
    ],
};

static R_RUM: FieldRule = FieldRule {
    effects: &[
        stage(When::SwitchIn, Who::FOE, Stat::Evasion, 1),
        stage(When::SwitchIn, Who::FOE, Stat::Accuracy, -1),
    ],
};

static R_PETALS: FieldRule = FieldRule {
    effects: &[
        RuleEffect::new(When::Start, Who::BOTH, What::Say).say(Gym_Petales_Debut),
        RuleEffect::new(
            When::TurnEnd(Turns::range(1, 8)),
            Who::BOTH.has(&[Type::Grass, Type::Bug]),
            What::Heal(16),
        )
        .say(Gym_Petales_Soin),
    ],
};

/// The injector on the foe's last mon: its trainer plants it, it mutates, then burns out.
macro_rules! injector {
    ($attack:expr) => {
        FieldRule {
            effects: &[
                RuleEffect::new(When::LastIn, Who::FOE, What::Say)
                    .say(Injecteur_Active)
                    .var(Var::Trainer),
                RuleEffect::new(When::LastIn, Who::FOE, What::Say).say(Injecteur_Mute),
                stage(When::LastIn, Who::FOE, $attack, 2),
                stage(When::LastIn, Who::FOE, Stat::Speed, 2),
                stage(When::LastIn, Who::FOE, Stat::Def, -1),
                stage(When::LastIn, Who::FOE, Stat::Accuracy, -1),
                RuleEffect::new(When::TurnEnd(Turns::ALL), Who::FOE.last(), What::Hurt(8))
                    .say(Injecteur_Grille),
            ],
        }
    };
}

static R_INJECTOR_ATK: FieldRule = injector!(Stat::Atk);
static R_INJECTOR_SPA: FieldRule = injector!(Stat::SpAtk);

static R_OVERDOSE: FieldRule = FieldRule {
    effects: &[
        stage(When::SwitchIn, Who::FOE, Stat::Atk, 1).say(Evalon_Surdose_Rage),
        stage(When::SwitchIn, Who::FOE, Stat::SpAtk, 1),
        RuleEffect::new(When::TurnEnd(Turns::ALL), Who::FOE, What::Hurt(16))
            .say(Evalon_Surdose_Fatigue),
    ],
};

static R_PRISM: FieldRule = FieldRule {
    effects: &[
        RuleEffect::new(When::TurnStart(Turns::range(1, 1)), Who::BOTH, What::Say)
            .say(Tour_4F_Prisme_On),
        RuleEffect::new(When::TurnStart(Turns::range(4, 4)), Who::BOTH, What::Say)
            .say(Tour_4F_Prisme_Off),
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
    ],
};

static R_HEART: FieldRule = FieldRule {
    effects: &[RuleEffect::new(
        When::During(Turns::ALL),
        Who::PLAYER,
        What::ConfusionWard {
            chance: 50,
            max_turns: 2,
        },
    )
    .say(Tour_Coeur_Pulse)],
};

/// `GameData::battle_rule`.
pub fn battle_rule(id: u8) -> Option<&'static FieldRule> {
    Some(match id {
        DEAF_ROOM => &R_DEAF_ROOM,
        STROBE => &R_STROBE,
        ROLL => &R_ROLL,
        ROLL_CONFUSED => &R_ROLL_CONFUSED,
        RUM => &R_RUM,
        PETALS => &R_PETALS,
        INJECTOR_ATK => &R_INJECTOR_ATK,
        INJECTOR_SPA => &R_INJECTOR_SPA,
        OVERDOSE => &R_OVERDOSE,
        PRISM => &R_PRISM,
        HEART => &R_HEART,
        _ => return None,
    })
}

/// Rules with field rule `id` only.
const fn field(id: u8) -> BattleRules {
    BattleRules {
        inverse: false,
        field: Some(id),
        background: None,
        foe_back: false,
        clone_player: false,
        intro: None,
    }
}

pub static WITH_DEAF_ROOM: BattleRules = field(DEAF_ROOM);
pub static WITH_STROBE: BattleRules = field(STROBE);
pub static WITH_ROLL: BattleRules = field(ROLL);
/// Samuel's battle after a failed sobriety test (`FLAG_SIL_PORT_GYM_CONFUS`).
pub static WITH_ROLL_CONFUSED: BattleRules = field(ROLL_CONFUSED);
pub static WITH_RUM: BattleRules = field(RUM);
pub static WITH_PETALS: BattleRules = field(PETALS);
pub static WITH_INJECTOR_ATK: BattleRules = field(INJECTOR_ATK);
pub static WITH_INJECTOR_SPA: BattleRules = field(INJECTOR_SPA);
pub static WITH_OVERDOSE: BattleRules = field(OVERDOSE);
pub static WITH_PRISM: BattleRules = field(PRISM);
pub static WITH_HEART: BattleRules = field(HEART);
/// Reverse's battle (Ch7, R-2): the inverse chart, its opening line, Reverse seen from the back.
pub static INVERSE: BattleRules = BattleRules {
    inverse: true,
    field: None,
    background: None,
    foe_back: true,
    clone_player: false,
    intro: Some(Gym07_Inverse_Debut),
};

/// `GameData::trainer_rules`: the battles that always have rules, wherever they start.
pub fn trainer_rules(id: TrainerId) -> Option<&'static BattleRules> {
    Some(match id {
        TRAINER_SIL_MELOMANE_SOURDE => &WITH_DEAF_ROOM,
        TRAINER_SIL_TAUBER_HOCSEAT_1 => &WITH_STROBE,
        TRAINER_SIL_PORT_GYM_GUS
        | TRAINER_SIL_PORT_GYM_JESSY
        | TRAINER_SIL_PORT_GYM_PATRICE
        | TRAINER_SIL_PORT_GYM_TIMOTHE
        | TRAINER_SIL_SAMUEL_1 => &WITH_ROLL,
        TRAINER_SIL_GWADA_2 => &WITH_RUM,
        TRAINER_SIL_KUNOICHI_AYAME
        | TRAINER_SIL_MOINE_SEIJI
        | TRAINER_SIL_SAMOURAI_KOJIRO
        | TRAINER_SIL_SAMOURAI_KOJIRO_HATIF
        | TRAINER_SIL_SAMOURAI_MUSASHI
        | TRAINER_SIL_SAMOURAI_MUSASHI_HATIF
        | TRAINER_SIL_TAK_1 => &WITH_PETALS,
        TRAINER_SIL_REVERSE_1 => &INVERSE,
        TRAINER_SIL_GOGO_3 => &WITH_INJECTOR_ATK,
        TRAINER_SIL_ROMAIN_3 => &WITH_INJECTOR_SPA,
        TRAINER_SIL_GOGO_4 | TRAINER_SIL_ROMAIN_4 => &WITH_OVERDOSE,
        TRAINER_SIL_REVENGE_1 => &WITH_PRISM,
        TRAINER_SIL_TOUR_GASTON | TRAINER_SIL_TOUR_CORALIE | TRAINER_SIL_MATHIAS_1 => &WITH_HEART,
        _ => return None,
    })
}

const fn mega(text: engine::data::TextId) -> TrainerLine {
    TrainerLine {
        when: LineWhen::Mega,
        text,
    }
}

static YOHAN: [TrainerLine; 1] = [mega(Gym2_Sc10_Mega)];
static SKYCKER: [TrainerLine; 1] = [mega(Owlys_Gym_Sc01_Mega)];
static TAK: [TrainerLine; 1] = [mega(Gym_Mega_T1)];
static KIMIKO: [TrainerLine; 1] = [mega(Tour_2F_Sc04_Mega)];
static ARTHUR: [TrainerLine; 1] = [mega(Tour_3F_Sc04_Mega)];

/// `GameData::trainer_lines`: the leaders' Mega lines.
pub fn trainer_lines(id: TrainerId) -> &'static [TrainerLine] {
    match id {
        TRAINER_SIL_YOHAN_1 => &YOHAN,
        TRAINER_SIL_SKYCKER_1 => &SKYCKER,
        TRAINER_SIL_TAK_1 => &TAK,
        TRAINER_SIL_KIMIKO_1 => &KIMIKO,
        TRAINER_SIL_ARTHUR_1 | TRAINER_SIL_ARTHUR_2 => &ARTHUR,
        _ => &[],
    }
}
