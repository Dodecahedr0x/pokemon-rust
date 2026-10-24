//! Per-battle rules (Silice; no Gen3 counterpart, Emerald never sets one).
//!
//! - [`BattleRules`] travel with a trainer battle request
//!   ([`crate::script::Request::BattleTrainer`]'s `rules`) into [`super::BattleSetup::rules`]:
//!   the inverse type chart, a field rule by id, a background, the foe's pic seen from the back,
//!   and a battle against a copy of the player's party.
//! - A field rule ([`FieldRule`], `GameData::battle_rule(id)`) is data: a list of
//!   [`RuleEffect`]s, each "when, on whom, what, and which text". Instant effects (a message, a
//!   stat stage, a status, a heal or a loss of HP) run at the battle's start, at a turn's start
//!   or end, when a mon enters, or when a side sends out its last mon; continuous ones (accuracy, speed, damage
//!   taken, a drain on the damage dealt, sound moves failing, a confusion ward) hold during the
//!   turns they name. Turns count from 1.
//! - Trainer lines ([`TrainerLine`], `GameData::trainer_lines(trainer)`): authored texts the
//!   foe says when it sends a given mon or its last one, when it Mega Evolves, or when its mon
//!   first falls under half HP. Each line is said once per battle.
//!
//! Authored texts are printed with [`super::Str::Authored`]: `{STR_VAR_1}` is the text's
//! [`Var`] (the battler's nickname or the trainer's name), `{STR_VAR_2}` the trainer's name and
//! `{PLAYER}` the player's.

use super::Environment;
use crate::data::TextId;
use crate::hash::Fnv64;
use crate::mon::{Stat, StatusKind, Type};

/// The rules of one battle. Default: a Gen3 battle.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct BattleRules {
    /// The inverse type chart (×2 and ×0.5 swap, ×0 becomes ×2); abilities' immunities stay.
    pub inverse: bool,
    /// `GameData::battle_rule(id)`.
    pub field: Option<u8>,
    /// The battle background, instead of the player's cell's.
    pub background: Option<Environment>,
    /// The foe trainer's pic seen from the back (drawn mirrored).
    pub foe_back: bool,
    /// The foe battles with a copy of the player's party (healed, same levels and items).
    pub clone_player: bool,
    /// A message after the intro, before the first turn.
    pub intro: Option<TextId>,
}

impl BattleRules {
    /// Hashed only when not default (Gen3 battles hash as before).
    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.inverse as u8,
            self.field.unwrap_or(0xFF),
            self.background.map_or(0xFF, |e| e as u8),
            self.foe_back as u8,
            self.clone_player as u8,
        ]);
        h.write_u16(self.intro.map_or(0xFFFF, |t| t.0));
    }

    /// A stable digest (the request hash).
    pub fn digest(&self) -> u32 {
        let mut h = Fnv64::new();
        self.hash(&mut h);
        h.finish() as u32
    }
}

/// A field rule: its effects, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FieldRule {
    pub effects: &'static [RuleEffect],
}

/// One effect of a [`FieldRule`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuleEffect {
    pub when: When,
    pub who: Who,
    pub what: What,
    /// Printed for each battler it applies to (once for [`What::Say`]). `None`: the effect's
    /// own Gen3 message (a stat change, a status) or none.
    pub text: Option<TextId>,
    /// What `{STR_VAR_1}` names in `text`.
    pub var: Var,
}

impl RuleEffect {
    /// An effect with no text.
    pub const fn new(when: When, who: Who, what: What) -> RuleEffect {
        RuleEffect {
            when,
            who,
            what,
            text: None,
            var: Var::Mon,
        }
    }

    /// With `text`.
    pub const fn say(self, text: TextId) -> RuleEffect {
        RuleEffect {
            text: Some(text),
            ..self
        }
    }

    /// With `{STR_VAR_1}` naming `var`.
    pub const fn var(self, var: Var) -> RuleEffect {
        RuleEffect { var, ..self }
    }
}

/// What `{STR_VAR_1}` names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Var {
    /// The battler's nickname.
    Mon,
    /// The foe trainer's name.
    Trainer,
}

/// When a [`RuleEffect`] runs (instant) or holds (continuous, [`When::During`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
    /// Once, after the leads are out and before the first turn's switch-in effects.
    Start,
    /// Before the actions are chosen, on these turns.
    TurnStart(Turns),
    /// After the end-of-turn effects, on these turns.
    TurnEnd(Turns),
    /// Holds during these turns (the continuous effects).
    During(Turns),
    /// Once per side, when it sends out its last mon that can fight.
    LastIn,
    /// Each time a mon enters: the leads after [`When::Start`], then every switch-in.
    SwitchIn,
}

/// A set of turns (from 1): `from..=until` (`until` 0: no end) where
/// `(turn - from) % every == phase`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Turns {
    pub every: u8,
    pub phase: u8,
    pub from: u8,
    pub until: u8,
}

impl Turns {
    /// Every turn.
    pub const ALL: Turns = Turns {
        every: 1,
        phase: 0,
        from: 1,
        until: 0,
    };

    /// Turns `from..=until`.
    pub const fn range(from: u8, until: u8) -> Turns {
        Turns {
            every: 1,
            phase: 0,
            from,
            until,
        }
    }

    /// One turn in `every`, the `phase`-th (from 0) of each cycle, from turn 1.
    pub const fn cycle(every: u8, phase: u8) -> Turns {
        Turns {
            every,
            phase,
            from: 1,
            until: 0,
        }
    }

    pub fn matches(self, turn: u8) -> bool {
        turn >= self.from
            && (self.until == 0 || turn <= self.until)
            && (turn - self.from) % self.every.max(1) == self.phase
    }
}

/// Which battlers an effect applies to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Who {
    pub sides: Sides,
    pub types: TypeFilter,
    /// Only a side's last mon that can fight.
    pub last: bool,
}

impl Who {
    pub const BOTH: Who = Who::on(Sides::Both);
    pub const PLAYER: Who = Who::on(Sides::Player);
    pub const FOE: Who = Who::on(Sides::Foe);

    pub const fn on(sides: Sides) -> Who {
        Who {
            sides,
            types: TypeFilter::Any,
            last: false,
        }
    }

    /// Only the battlers of one of `types`.
    pub const fn has(self, types: &'static [Type]) -> Who {
        Who {
            types: TypeFilter::Has(types),
            ..self
        }
    }

    /// Only the battlers of none of `types`.
    pub const fn lacks(self, types: &'static [Type]) -> Who {
        Who {
            types: TypeFilter::Lacks(types),
            ..self
        }
    }

    /// Only a side's last mon.
    pub const fn last(self) -> Who {
        Who { last: true, ..self }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sides {
    Player,
    Foe,
    Both,
}

impl Sides {
    pub fn has(self, player: bool) -> bool {
        match self {
            Sides::Player => player,
            Sides::Foe => !player,
            Sides::Both => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeFilter {
    Any,
    Has(&'static [Type]),
    Lacks(&'static [Type]),
}

impl TypeFilter {
    pub fn matches(self, types: [Type; 2]) -> bool {
        match self {
            TypeFilter::Any => true,
            TypeFilter::Has(l) => types.iter().any(|t| l.contains(t)),
            TypeFilter::Lacks(l) => !types.iter().any(|t| l.contains(t)),
        }
    }
}

/// What a [`RuleEffect`] does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum What {
    // ---- instant ----
    /// Only the text (once).
    Say,
    /// Moves a stat stage by `delta`, never past `limit` (0: ±6).
    Stage { stat: Stat, delta: i8, limit: i8 },
    /// A status, unless it has one.
    Status(StatusKind),
    /// Confusion (2 to 5 turns, one roll), unless confused.
    Confuse,
    /// Restores max HP / n (at least 1).
    Heal(u8),
    /// Loses max HP / n (at least 1); it may faint.
    Hurt(u8),
    // ---- continuous (`When::During`) ----
    /// Moves aimed at the battler: accuracy ×num/den (moves that never miss are not affected).
    Accuracy { num: u8, den: u8 },
    /// The battler's speed ×num/den.
    Speed { num: u8, den: u8 },
    /// Damage the battler takes from moves ×num/den.
    DamageTaken { num: u8, den: u8 },
    /// The battler loses num/den of the move damage it deals.
    DrainDealt { num: u8, den: u8 },
    /// The battler's sound moves fail.
    SoundFails,
    /// When a move confuses the battler: `chance`% to shake it off at once (with the text),
    /// else it lasts at most `max_turns` (0: as rolled).
    ConfusionWard { chance: u8, max_turns: u8 },
}

/// When a [`TrainerLine`] is said.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineWhen {
    /// Before the trainer sends out this party slot.
    SendOut(u8),
    /// Before the trainer sends out its last mon (not at the intro).
    LastSendOut,
    /// Before the trainer's mon Mega Evolves.
    Mega,
    /// When the trainer's mon on the field first falls under half its max HP.
    LowHp,
}

/// A trainer's mid-battle line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrainerLine {
    pub when: LineWhen,
    pub text: TextId,
}
