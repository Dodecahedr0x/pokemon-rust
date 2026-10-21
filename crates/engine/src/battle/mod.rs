//! Gen3 single battles: the core/presentation boundary.
//!
//! - Logic (pure, video-free): [`core`] runs turns from [`Choice`]s and pushes [`BattleEvent`]s,
//!   with formulas in [`calc`], move effects in [`effects`] and the opponent in [`ai`]. Every
//!   random roll comes from the `World` RNG it is handed.
//! - Presentation: [`scene`] plays the event stream with [`gfx`] (sprites, healthboxes, bars),
//!   [`ui`] (windows, menus) and [`text`] (message expansion), and turns input into `Choice`s.
//!   [`transition`] is the field-to-battle transition.
//!
//! Wave 0 contract (Milestone 4). Every module below is filled by its Wave 1 task.

pub mod ai;
pub mod calc;
pub mod core;
pub mod effects;
pub mod gfx;
pub mod scene;
pub mod text;
pub mod transition;
pub mod ui;

#[cfg(test)]
pub(crate) mod test_data;

use crate::data::{Font, MoveId, SpeciesId, TextId, TrainerId};
use crate::mon::{Gender, NAME_LEN, Party, Stat, Status, TrainerPic, Type};
use crate::video::Tile;

pub use self::core::Battle;
pub use self::scene::BattleScene;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Player,
    Opponent,
}

impl Side {
    pub const fn other(self) -> Side {
        match self {
            Side::Player => Side::Opponent,
            Side::Opponent => Side::Player,
        }
    }
}

/// Gen3 `BATTLE_ENVIRONMENT_*` order.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Environment {
    Grass,
    LongGrass,
    Sand,
    Underwater,
    Water,
    Pond,
    Mountain,
    Cave,
    Building,
    Plain,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattleKind {
    /// `first_battle`: Gen3 `BATTLE_TYPE_FIRST_BATTLE` (Birch's rescue).
    Wild { first_battle: bool },
    /// `defeat`: the text shown after the player wins.
    Trainer { trainer: TrainerId, defeat: TextId },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BattleSetup {
    pub kind: BattleKind,
    pub opponents: Party,
    pub environment: Environment,
}

/// Gen3 `B_OUTCOME_*` values, so scripts can store them in `VAR_RESULT`.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattleOutcome {
    Won = 1,
    Lost = 2,
    Drew = 3,
    Ran = 4,
    MonFled = 6,
    Caught = 7,
}

impl BattleOutcome {
    /// The outcome of a `B_OUTCOME_*` value.
    pub const fn from_u16(v: u16) -> Option<BattleOutcome> {
        Some(match v {
            1 => BattleOutcome::Won,
            2 => BattleOutcome::Lost,
            3 => BattleOutcome::Drew,
            4 => BattleOutcome::Ran,
            6 => BattleOutcome::MonFled,
            7 => BattleOutcome::Caught,
            _ => return None,
        })
    }
}

/// The player's decision for the core.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    /// Move slot 0..4.
    Move(u8),
    /// Party slot.
    Switch(u8),
    Run,
    Ball,
    /// `None`: do not learn the new move.
    LearnMove {
        forget: Option<u8>,
    },
}

/// What the core waits for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Need {
    /// Events are pending: nothing to choose.
    Busy,
    Action,
    Switch,
    LearnMove {
        party: u8,
        mov: MoveId,
    },
    Ended(BattleOutcome),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effectiveness {
    Normal,
    Super,
    NotVery,
    NoEffect,
}

/// `Cmd_waitmessage`: `B_WAIT_TIME_*` frames, or A/B (or a prompt).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MsgWait {
    Frames(u8),
    Button,
}

/// Gen3 `gBattleTextBuff1..3` contents (`PREPARE_*_BUFFER`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Buff {
    None,
    Stat(Stat),
    Move(MoveId),
    Number(u32),
    Species(SpeciesId),
    Type(Type),
    /// `B_BUFF_MON_NICK_WITH_PREFIX`: party slot `party` of `side`, with the wild/foe tag.
    Mon {
        side: Side,
        party: u8,
    },
    /// `B_BUFF_MON_NICK`: the nickname alone (`sText_Trainer1SentOutPkmn2`, send-outs).
    MonNick {
        side: Side,
        party: u8,
    },
    Text(Str),
}

/// A battle message: a [`Str`] plus the context its placeholders read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Msg {
    pub id: Str,
    pub attacker: Side,
    pub target: Side,
    pub active: Side,
    pub mov: Option<MoveId>,
    pub buffs: [Buff; 3],
    pub wait: MsgWait,
}

/// Battle strings by decomp name (`STRINGID_X` -> `Str::X`, `gText_X` -> `Str::X`), plus the
/// indexed ones. Content resolves them (`GameData::battle_text`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Str {
    /// Wild Pokémon appeared.
    Intro1,
    /// … wants to battle.
    IntroTrainer,
    IntroMsg,
    Intro2,
    GoPkmn,
    PkmnSentOut,
    WhatWillPkmnDo,
    BattleMenu,
    MoveInterfacePP,
    MoveInterfaceType,
    UsedMove,
    AttackMissed,
    CriticalHit,
    SuperEffective,
    NotVeryEffective,
    ItDoesntAffect,
    PkmnFainted,
    GotAwaySafely,
    CantEscape,
    DontLeaveBirch,
    WildPkmnFled,
    PlayerDefeatedTrainer,
    PlayerWhiteout,
    PlayerWhiteout2,
    PkmnGainedExp,
    PkmnGrewToLv,
    PkmnLearnedMove,
    TryToLearnMove1,
    TryToLearnMove2,
    TryToLearnMove3,
    PkmnForgotMove,
    StopLearningMove,
    DidNotLearnMove,
    AttackerStatChanged,
    DefenderStatChanged,
    StatRose,
    StatSharplyRose,
    StatFell,
    StatHarshlyFell,
    StatWontIncrease,
    StatWontDecrease,
    ButItFailed,
    PkmnWasPoisoned,
    PkmnWasBurned,
    PkmnWasParalyzed,
    PkmnFellAsleep,
    PkmnBecameConfused,
    PkmnFlinched,
    PkmnIsConfused,
    PkmnHurtItself,
    PkmnIsParalyzed,
    PkmnFastAsleep,
    PkmnWokeUp,
    PkmnHurtByPoison,
    PkmnHurtByBurn,
    PkmnSeeded,
    LeechSeedDrain,
    PkmnStoringEnergy,
    PkmnUnleashedEnergy,
    GettingPumped,
    AbsorbDrain,
    PkmnHitWithRecoil,
    NoPPLeft,
    Struggle,
    Gotcha,
    /// `sText_WildPkmnPrefix`.
    WildPrefix,
    /// `sText_FoePkmnPrefix`.
    FoePrefix,
    /// `gText_BattleYesNoChoice` (Task F's yes/no window).
    BattleYesNoChoice,
    /// `gText_ConfirmStarterChoice` (Task G's starter screen).
    ConfirmStarterChoice,
    /// `gTypeNames[t]`.
    TypeName(Type),
    /// `gStatNamesTable[s]`.
    StatName(Stat),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattleEvent {
    Msg(Msg),
    // Pic slides.
    WildAppear,
    TrainerAppear,
    TrainerSlideOut,
    TrainerDefeated,
    SendOut { side: Side, party: u8 },
    Recall { side: Side },
    MoveAnim { attacker: Side, mov: MoveId },
    Hit { target: Side, eff: Effectiveness },
    Hp { side: Side, from: u16, to: u16 },
    StatusIcon { side: Side, status: Status },
    StatAnim { side: Side, up: bool },
    Faint { side: Side },
    Exp { party: u8, from: u32, to: u32 },
    LevelUp { party: u8, level: u8 },
    Ball { shakes: u8, caught: bool },
    End(BattleOutcome),
}

/// What presentation reads of a battler (Gen3 `struct BattlePokemon` subset). The core keeps
/// its private per-battler state (stat stages, STATUS2/3 subset, …) next to it in `core`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BattleMon {
    pub party: u8,
    pub species: SpeciesId,
    pub nickname: [u8; NAME_LEN],
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub status: Status,
    pub gender: Gender,
    pub moves: [Option<MoveId>; 4],
    pub pp: [u8; 4],
    pub max_pp: [u8; 4],
}

/// Names the message expansion reads (`text::expand`).
#[derive(Clone, Copy, Debug)]
pub struct Names<'a> {
    pub player: &'a [u8],
    pub trainer_class: &'a [u8],
    pub trainer_name: &'a [u8],
    pub wild: bool,
    /// The battlers. Index = `Side as usize`.
    pub mons: [&'a BattleMon; 2],
    /// Both parties (index = `Side as usize`): `Buff::Mon` / `Buff::MonNick` name a party slot,
    /// so a queued message names the same Pokémon whichever battler is out when it plays.
    pub parties: [&'a Party; 2],
}

/// A tile graphic with its palettes and optional tilemap (raw GBA screen entries, 32 per row).
#[derive(Clone, Copy, Debug)]
pub struct GfxImage {
    pub tiles: &'static [Tile],
    pub palettes: &'static [[u16; 16]],
    pub tilemap: &'static [u16],
}

impl GfxImage {
    pub const EMPTY: GfxImage = GfxImage {
        tiles: &[],
        palettes: &[],
        tilemap: &[],
    };
}

/// Battle interface graphics. Wave 0 proposal: Task B fixes the final list with E, F and G
/// (additions in their reports); Task I fills it. Empty slices draw nothing.
#[derive(Clone, Copy)]
pub struct BattleGfx {
    /// `gBattleTextboxTiles` / `Tilemap` / `Palette`.
    pub textbox: GfxImage,
    /// `gBattleEnvironment{Tiles,Tilemap,Palette}_*`, by `Environment as usize`.
    pub environments: [GfxImage; 10],
    /// `healthbox_singles_player` / `_opponent` (64x32 x2, OBJ 1D order), palette in
    /// `healthbox.palettes[0]`.
    pub healthbox_player: GfxImage,
    pub healthbox_opponent: GfxImage,
    /// `hpbar` tiles and the HP bar palette.
    pub hpbar: GfxImage,
    pub expbar: GfxImage,
    /// `status*` icons (FR ROM: may be localized).
    pub status: GfxImage,
    /// `misc`: level glyph, gender symbols.
    pub misc: GfxImage,
    pub numbers: GfxImage,
    /// `gBallGfx_Poke` + palette (send-out ball).
    pub ball: GfxImage,
    /// `FONT_SMALL` (`gFontSmallLatinGlyphs`), healthbox nicknames.
    pub small_font: &'static Font,
    /// The player's back pic, frames of 64 tiles side by side.
    pub player_back: TrainerPic,
    /// `starter_choose.c`: bag + grass background, Poké Ball and circle OBJs.
    pub starter_bag: GfxImage,
    pub starter_ball: GfxImage,
    pub starter_circle: GfxImage,
}

static EMPTY_FONT: Font = Font {
    glyphs: &[],
    widths: &[],
};

/// Nothing at all: the default of `GameData::battle_gfx`.
pub static EMPTY_GFX: BattleGfx = BattleGfx {
    textbox: GfxImage::EMPTY,
    environments: [GfxImage::EMPTY; 10],
    healthbox_player: GfxImage::EMPTY,
    healthbox_opponent: GfxImage::EMPTY,
    hpbar: GfxImage::EMPTY,
    expbar: GfxImage::EMPTY,
    status: GfxImage::EMPTY,
    misc: GfxImage::EMPTY,
    numbers: GfxImage::EMPTY,
    ball: GfxImage::EMPTY,
    small_font: &EMPTY_FONT,
    player_back: TrainerPic {
        tiles: &[],
        palette: &[0; 16],
        y: 0,
    },
    starter_bag: GfxImage::EMPTY,
    starter_ball: GfxImage::EMPTY,
    starter_circle: GfxImage::EMPTY,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes_are_gen3_b_outcome_values() {
        for o in [
            BattleOutcome::Won,
            BattleOutcome::Lost,
            BattleOutcome::Drew,
            BattleOutcome::Ran,
            BattleOutcome::MonFled,
            BattleOutcome::Caught,
        ] {
            assert_eq!(BattleOutcome::from_u16(o as u16), Some(o));
        }
        assert_eq!(BattleOutcome::Won as u8, 1);
        assert_eq!(BattleOutcome::MonFled as u8, 6);
        assert_eq!(BattleOutcome::from_u16(5), None);
        assert_eq!(BattleOutcome::from_u16(0), None);
    }

    #[test]
    fn environment_order_and_sides() {
        assert_eq!(Environment::Grass as u8, 0);
        assert_eq!(Environment::Plain as u8, 9);
        assert_eq!(Side::Player.other(), Side::Opponent);
        assert_eq!(Side::Opponent as usize, 1);
    }
}
