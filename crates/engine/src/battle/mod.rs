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

use crate::data::{Font, ItemId, MoveId, SpeciesId, TextId, TrainerId};
use crate::mon::{Ability, Gender, NAME_LEN, Party, Stat, Status, TrainerPic, Type};
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
    /// Both last battlers fainted together. Gen3 `IsPlayerDefeated(B_OUTCOME_DREW)`: the
    /// player is defeated, so the overworld (Task J) treats it as a whiteout like `Lost`.
    Drew = 3,
    Ran = 4,
    /// Teleport by the player's mon (`B_OUTCOME_PLAYER_TELEPORTED`).
    PlayerTeleported = 5,
    MonFled = 6,
    Caught = 7,
    /// Teleport by the wild mon (`B_OUTCOME_MON_TELEPORTED`).
    MonTeleported = 10,
}

impl BattleOutcome {
    /// The outcome of a `B_OUTCOME_*` value.
    pub const fn from_u16(v: u16) -> Option<BattleOutcome> {
        Some(match v {
            1 => BattleOutcome::Won,
            2 => BattleOutcome::Lost,
            3 => BattleOutcome::Drew,
            4 => BattleOutcome::Ran,
            5 => BattleOutcome::PlayerTeleported,
            6 => BattleOutcome::MonFled,
            7 => BattleOutcome::Caught,
            10 => BattleOutcome::MonTeleported,
            _ => return None,
        })
    }
}

/// The player's decision for the core.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    /// Move slot 0..4.
    Move(u8),
    /// Move slot 0..4, Mega Evolving first (the fight menu's toggle; refused like `Move` when
    /// the battler cannot, then it is a plain `Move`).
    Mega(u8),
    /// Party slot.
    Switch(u8),
    Run,
    Ball,
    /// An item from the bag on the active mon or the given party slot (Milestone 9 Task BM,
    /// `core/items.rs`; refused until then).
    Item {
        item: crate::data::ItemId,
        party: u8,
    },
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
    /// `B_LAST_ITEM` (`gLastUsedItem`): an item name, from `GameData::item`. The core puts it
    /// in `buffs[2]`, which `{B_LAST_ITEM}` expands.
    Item(ItemId),
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
    /// The party slot of each side's battler when the message was emitted (index = `Side as
    /// usize`). The battler name codes read these, so a message queued before a `SendOut`
    /// still names the mon that was out (Gen3 expands it before the switch).
    pub slots: [u8; 2],
}

/// Battle strings by decomp name (`STRINGID_X` -> `Str::X`, `gText_X` -> `Str::X`), plus the
/// indexed ones. Content resolves them (`GameData::battle_text`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Str {
    /// Wild Pokémon appeared.
    Intro1,
    /// … wants to battle.
    IntroTrainer,
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
    /// `STRINGID_ATTACKERSSTATROSE`.
    AttackerStatRose,
    /// `STRINGID_ATTACKERSSTATFELL`.
    AttackerStatFell,
    /// `STRINGID_DEFENDERSSTATROSE`.
    DefenderStatRose,
    /// `STRINGID_DEFENDERSSTATFELL`.
    DefenderStatFell,
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
    /// `STRINGID_PKMNHASNOMOVESLEFT` ("X has no moves left!"), before Struggle.
    Struggle,
    /// `STRINGID_NORUNNINGFROMTRAINERS`: RUN chosen in a trainer battle.
    NoRunningFromTrainers,
    Gotcha,
    /// `sText_WildPkmnPrefix`.
    WildPrefix,
    /// `sText_FoePkmnPrefix`.
    FoePrefix,
    /// `gText_BattleYesNoChoice` (Task F's yes/no window).
    BattleYesNoChoice,
    /// `gText_ConfirmStarterChoice` (Task G's starter screen).
    ConfirmStarterChoice,
    /// `gText_BirchInTrouble` (the starter screen while choosing).
    BirchInTrouble,
    /// `gTypeNames[t]`.
    TypeName(Type),
    /// `gStatNamesTable[s]`.
    StatName(Stat),
    // ---- Task E2: statuses, abilities, catching ----
    /// `STRINGID_PKMNBADLYPOISONED`.
    PkmnBadlyPoisoned,
    /// `STRINGID_PKMNWASFROZEN`.
    PkmnWasFrozen,
    /// `STRINGID_PKMNISFROZEN`.
    PkmnIsFrozen,
    /// `STRINGID_PKMNWASDEFROSTED2` (thawed when trying to move).
    PkmnWasDefrosted2,
    /// `STRINGID_PKMNWASDEFROSTED` (thawed by a Fire move).
    PkmnWasDefrosted,
    /// `STRINGID_PKMNHEALEDCONFUSION`.
    PkmnHealedConfusion,
    /// `STRINGID_PKMNPOISONEDBY` (an ability poisoned the battler).
    PkmnPoisonedBy,
    /// `STRINGID_PKMNBURNEDBY`.
    PkmnBurnedBy,
    /// `STRINGID_PKMNFROZENBY`.
    PkmnFrozenBy,
    /// `STRINGID_PKMNWASPARALYZEDBY`.
    PkmnWasParalyzedBy,
    /// `STRINGID_PKMNMADESLEEP`.
    PkmnMadeSleep,
    /// `STRINGID_PKMNALREADYASLEEP`.
    PkmnAlreadyAsleep,
    /// `STRINGID_PKMNALREADYPOISONED`.
    PkmnAlreadyPoisoned,
    /// `STRINGID_PKMNISALREADYPARALYZED`.
    PkmnIsAlreadyParalyzed,
    /// `STRINGID_PKMNALREADYHASBURN`.
    PkmnAlreadyHasBurn,
    /// `STRINGID_PKMNALREADYCONFUSED`.
    PkmnAlreadyConfused,
    /// `STRINGID_PKMNEVADEDATTACK` (Leech Seed missed or the target is already seeded).
    PkmnEvadedAttack,
    /// `STRINGID_NATUREPOWERTURNEDINTO`.
    NaturePowerTurnedInto,
    /// `STRINGID_PKMNCUTSATTACKWITH` (Intimidate).
    PkmnCutsAttackWith,
    /// `STRINGID_PKMNTRACED`.
    PkmnTraced,
    /// `STRINGID_PKMNSXPREVENTSYLOSS` (Keen Eye).
    PkmnSXPreventsYLoss,
    /// `STRINGID_PKMNSXHADNOEFFECTONY` (Synchronize on an immune type).
    PkmnSXHadNoEffectOnY,
    /// `STRINGID_PKMNBROKEFREE` (ball, no shake).
    PkmnBrokeFree,
    /// `STRINGID_ITAPPEAREDCAUGHT` (ball, 1 shake).
    ItAppearedCaught,
    /// `STRINGID_AARGHALMOSTHADIT` (ball, 2 shakes).
    AarghAlmostHadIt,
    /// `STRINGID_SHOOTSOCLOSE` (ball, 3 shakes).
    ShootSoClose,
    /// `STRINGID_HITXTIMES` (`B_BUFF1` = the hit count).
    HitXTimes,
    /// `STRINGID_BUTNOTHINGHAPPENED` (Splash).
    ButNothingHappened,
    /// `STRINGID_PKMNCHARGINGPOWER` (Charge).
    PkmnChargingPower,
    // Wave C: move effects (`effects::wc`).
    /// `STRINGID_PKMNREGAINEDHEALTH`.
    PkmnRegainedHealth,
    /// `STRINGID_PKMNHPFULL`.
    PkmnHpFull,
    /// `STRINGID_STATCHANGESGONE`.
    StatChangesGone,
    /// `STRINGID_PKMNWASNTAFFECTED`.
    PkmnWasntAffected,
    /// `STRINGID_PKMNDREAMEATEN`.
    PkmnDreamEaten,
    /// `STRINGID_COINSSCATTERED`.
    CoinsScattered,
    /// `STRINGID_PKMNCRASHED`.
    PkmnCrashed,
    /// `STRINGID_PKMNMUSTRECHARGE`.
    PkmnMustRecharge,
    /// `STRINGID_PKMNRAGEBUILDING`.
    PkmnRageBuilding,
    /// `STRINGID_PKMNFELLINTONIGHTMARE`.
    PkmnFellIntoNightmare,
    /// `STRINGID_PKMNLOCKEDINNIGHTMARE`.
    PkmnLockedInNightmare,
    /// `STRINGID_PKMNLAIDCURSE`.
    PkmnLaidCurse,
    /// `STRINGID_PKMNAFFLICTEDBYCURSE`.
    PkmnAfflictedByCurse,
    /// `STRINGID_PKMNPROTECTEDITSELF2`.
    PkmnProtectedItself2,
    /// `STRINGID_PKMNBRACEDITSELF`.
    PkmnBracedItself,
    /// `STRINGID_PKMNPROTECTEDITSELF`.
    PkmnProtectedItself,
    /// `STRINGID_PKMNENDUREDHIT`.
    PkmnEnduredHit,
    /// `STRINGID_SPIKESSCATTERED`.
    SpikesScattered,
    /// `STRINGID_PKMNHURTBYSPIKES`.
    PkmnHurtBySpikes,
    /// `STRINGID_FAINTINTHREE`.
    FaintInThree,
    /// `STRINGID_PKMNPERISHCOUNTFELL`.
    PkmnPerishCountFell,
    /// `STRINGID_STARTEDTORAIN`.
    StartedToRain,
    /// `STRINGID_SUNLIGHTGOTBRIGHT`.
    SunlightGotBright,
    /// `STRINGID_SANDSTORMBREWED`.
    SandstormBrewed,
    /// `STRINGID_STARTEDHAIL`.
    StartedHail,
    /// `STRINGID_RAINSTOPPED`.
    RainStopped,
    /// `STRINGID_SUNLIGHTFADED`.
    SunlightFaded,
    /// `STRINGID_SANDSTORMSUBSIDED`.
    SandstormSubsided,
    /// `STRINGID_HAILCONTINUES`.
    HailContinues,
    /// `STRINGID_HAILSTOPPED`.
    HailStopped,
    /// `STRINGID_PKMNPELTEDBYHAIL`.
    PkmnPeltedByHail,
    /// `STRINGID_PKMNRAISEDDEF`.
    PkmnRaisedDef,
    /// `STRINGID_PKMNRAISEDSPDEF`.
    PkmnRaisedSpDef,
    /// `STRINGID_PKMNCOVEREDBYVEIL`.
    PkmnCoveredByVeil,
    /// `STRINGID_PKMNSXWOREOFF`.
    PkmnSXWoreOff,
    /// `STRINGID_PKMNSAFEGUARDEXPIRED`.
    PkmnSafeguardExpired,
    /// `STRINGID_PKMNUSEDSAFEGUARD`.
    PkmnUsedSafeguard,
    /// `STRINGID_PKMNSHROUDEDINMIST`.
    PkmnShroudedInMist,
    /// `STRINGID_PKMNPROTECTEDBYMIST`.
    PkmnProtectedByMist,
    /// `STRINGID_PKMNWISHCAMETRUE`.
    PkmnWishCameTrue,
    /// `STRINGID_PKMNPLANTEDROOTS`.
    PkmnPlantedRoots,
    /// `STRINGID_PKMNABSORBEDNUTRIENTS`.
    PkmnAbsorbedNutrients,
    /// `STRINGID_PKMNANCHOREDITSELF`.
    PkmnAnchoredItself,
    /// `STRINGID_PKMNCENTERATTENTION`.
    PkmnCenterAttention,
    /// `STRINGID_PKMNSQUEEZEDBYBIND`.
    PkmnSqueezedByBind,
    /// `STRINGID_PKMNWRAPPEDBY`.
    PkmnWrappedBy,
    /// `STRINGID_PKMNTRAPPEDINVORTEX`.
    PkmnTrappedInVortex,
    /// `STRINGID_PKMNCLAMPED`.
    PkmnClamped,
    /// `STRINGID_PKMNTRAPPEDBYSANDTOMB`.
    PkmnTrappedBySandTomb,
    /// `STRINGID_PKMNHURTBY`.
    PkmnHurtBy,
    /// `STRINGID_PKMNFREEDFROM`.
    PkmnFreedFrom,
    /// `STRINGID_PKMNGOTFREE`.
    PkmnGotFree,
    /// `STRINGID_PKMNSHEDLEECHSEED`.
    PkmnShedLeechSeed,
    /// `STRINGID_PKMNBLEWAWAYSPIKES`.
    PkmnBlewAwaySpikes,
    /// `STRINGID_PKMNFATIGUECONFUSION`.
    PkmnFatigueConfusion,
    /// `STRINGID_PKMNTOOKAIM`.
    PkmnTookAim,
    /// `STRINGID_PKMNWASDRAGGEDOUT`.
    PkmnWasDraggedOut,
    /// `STRINGID_PKMNLOSTFOCUS`.
    PkmnLostFocus,
    /// `STRINGID_PKMNTIGHTENINGFOCUS`.
    PkmnTighteningFocus,
    /// `STRINGID_PKMNSTOCKPILED`.
    PkmnStockpiled,
    /// `STRINGID_PKMNCANTSTOCKPILE`.
    PkmnCantStockpile,
    /// `STRINGID_FAILEDTOSPITUP`.
    FailedToSpitUp,
    /// `STRINGID_FAILEDTOSWALLOW`.
    FailedToSwallow,
    /// `STRINGID_PKMNFORESAWATTACK`.
    PkmnForesawAttack,
    /// `STRINGID_PKMNCHOSEXASDESTINY`.
    PkmnChoseXAsDestiny,
    /// `STRINGID_PKMNTOOKATTACK`.
    PkmnTookAttack,
    /// `STRINGID_PKMNCUTHPMAXEDATTACK`.
    PkmnCutHpMaxedAttack,
    /// `STRINGID_PKMNCOPIEDSTATCHANGES`.
    PkmnCopiedStatChanges,
    /// `STRINGID_PKMNCHANGEDTYPE`.
    PkmnChangedType,
    /// `STRINGID_PKMNMOVEWASDISABLED`.
    PkmnMoveWasDisabled,
    /// `STRINGID_PKMNMOVEISDISABLED`.
    PkmnMoveIsDisabled,
    /// `STRINGID_PKMNMOVEDISABLEDNOMORE`.
    PkmnMoveDisabledNoMore,
    /// `STRINGID_PKMNGOTENCORE`.
    PkmnGotEncore,
    /// `STRINGID_PKMNENCOREENDED`.
    PkmnEncoreEnded,
    /// `STRINGID_PKMNSUBJECTEDTOTORMENT`.
    PkmnSubjectedToTorment,
    /// `STRINGID_PKMNCANTUSEMOVETORMENT`.
    PkmnCantUseMoveTorment,
    /// `STRINGID_PKMNFELLFORTAUNT`.
    PkmnFellForTaunt,
    /// `STRINGID_PKMNCANTUSEMOVETAUNT`.
    PkmnCantUseMoveTaunt,
    /// `STRINGID_PKMNREDUCEDPP`.
    PkmnReducedPP,
    /// `STRINGID_PKMNTRYINGTOTAKEFOE`.
    PkmnTryingToTakeFoe,
    /// `STRINGID_PKMNTOOKFOE`.
    PkmnTookFoe,
    /// `STRINGID_PKMNSTATUSNORMAL`.
    PkmnStatusNormal,
    /// `STRINGID_BELLCHIMED`.
    BellChimed,
    /// `STRINGID_SOOTHINGAROMA`.
    SoothingAroma,
    /// `STRINGID_SHAREDPAIN`.
    SharedPain,
    /// `STRINGID_PKMNSTOLEITEM`.
    PkmnStoleItem,
    /// `STRINGID_PKMNKNOCKEDOFF`.
    PkmnKnockedOff,
    /// `STRINGID_THEWALLSHATTERED`.
    TheWallShattered,
    /// `STRINGID_MAGNITUDESTRENGTH`.
    MagnitudeStrength,
    /// `STRINGID_PKMNWHIPPEDWHIRLWIND`.
    PkmnWhippedWhirlwind,
    /// `STRINGID_PKMNTOOKSUNLIGHT`.
    PkmnTookSunlight,
    /// `STRINGID_PKMNLOWEREDHEAD`.
    PkmnLoweredHead,
    /// `STRINGID_PKMNISGLOWING`.
    PkmnIsGlowing,
    /// `STRINGID_PKMNFLEWHIGH`.
    PkmnFlewHigh,
    /// `STRINGID_PKMNDUGHOLE`.
    PkmnDugHole,
    /// `STRINGID_PKMNHIDUNDERWATER`.
    PkmnHidUnderwater,
    /// `STRINGID_PKMNSPRANGUP`.
    PkmnSprangUp,
    /// `STRINGID_BUTNOEFFECT`.
    ButNoEffect,
    /// `STRINGID_ONEHITKO`.
    OneHitKo,
    /// `STRINGID_PKMNUNAFFECTED`.
    PkmnUnaffected,
    /// `STRINGID_MIRRORMOVEFAILED`.
    MirrorMoveFailed,
    /// `STRINGID_PKMNSLEPTHEALTHY`.
    PkmnSleptHealthy,
    /// `STRINGID_PKMNWENTTOSLEEP`.
    PkmnWentToSleep,
    /// `STRINGID_PKMNWASDEFROSTEDBY`.
    PkmnWasDefrostedBy,
    /// `STRINGID_PKMNALREADYASLEEP2`.
    PkmnAlreadyAsleep2,
    /// `STRINGID_PKMNSXBLOCKSY2`.
    PkmnSXBlocksY2,
    /// `STRINGID_STATSWONTINCREASE2`.
    StatsWontIncrease2,
    /// `STRINGID_STATSWONTDECREASE2`.
    StatsWontDecrease2,
    /// `STRINGID_PKMNHEALEDPARALYSIS`.
    PkmnHealedParalysis,
    /// `STRINGID_PKMNCOPIEDFOE`.
    PkmnCopiedFoe,
    /// `STRINGID_PKMNSWAPPEDABILITIES`.
    PkmnSwappedAbilities,
    /// `STRINGID_PKMNWANTSGRUDGE`.
    PkmnWantsGrudge,
    /// `STRINGID_PKMNLOSTPPGRUDGE`.
    PkmnLostPPGrudge,
    /// `STRINGID_PKMNSEALEDOPPONENTMOVE`.
    PkmnSealedOpponentMove,
    /// `STRINGID_PKMNCANTUSEMOVESEALED`.
    PkmnCantUseMoveSealed,
    /// `STRINGID_PKMNSHROUDEDITSELF`.
    PkmnShroudedItself,
    /// `STRINGID_PKMNMOVEBOUNCED`.
    PkmnMoveBounced,
    /// `STRINGID_PKMNSWITCHEDITEMS`.
    PkmnSwitchedItems,
    /// `STRINGID_PKMNOBTAINEDX`.
    PkmnObtainedX,
    /// `STRINGID_PKMNOBTAINEDX2`.
    PkmnObtainedX2,
    /// `STRINGID_PKMNOBTAINEDXYOBTAINEDZ`.
    PkmnObtainedXYObtainedZ,
    /// `STRINGID_PKMNANCHORSITSELFWITH`.
    PkmnAnchorsItselfWith,
    // Wave D: the last move effects.
    /// `STRINGID_PKMNMADESUBSTITUTE`.
    PkmnMadeSubstitute,
    /// `STRINGID_PKMNHASSUBSTITUTE`.
    PkmnHasSubstitute,
    /// `STRINGID_SUBSTITUTEDAMAGED`.
    SubstituteDamaged,
    /// `STRINGID_PKMNSUBSTITUTEFADED`.
    PkmnSubstituteFaded,
    /// `STRINGID_TOOWEAKFORSUBSTITUTE`.
    TooWeakForSubstitute,
    /// `STRINGID_PKMNTRANSFORMEDINTO`.
    PkmnTransformedInto,
    /// `STRINGID_PKMNLEARNEDMOVE2`.
    PkmnLearnedMove2,
    /// `STRINGID_PKMNSKETCHEDMOVE`.
    PkmnSketchedMove,
    /// `STRINGID_XFOUNDONEY`.
    XFoundOneY,
    /// `STRINGID_PKMNWAITSFORTARGET`.
    PkmnWaitsForTarget,
    /// `STRINGID_PKMNSNATCHEDMOVE`.
    PkmnSnatchedMove,
    /// `STRINGID_PKMNATTACK`.
    PkmnAttack,
    /// `STRINGID_PLAYERPICKEDUPMONEY`.
    PlayerPickedUpMoney,
    // Task G.
    /// `STRINGID_PKMNFELLINLOVE`: Attract: the target (`B_DEF`).
    PkmnFellInLove,
    /// `STRINGID_PKMNINLOVE`: the attacker (`B_ATK`) and the battler it loves (`B_SCR_ACTIVE` = `active`).
    PkmnInLove,
    /// `STRINGID_PKMNIMMOBILIZEDBYLOVE`: the attacker.
    PkmnImmobilizedByLove,
    /// `STRINGID_PKMNPREVENTSROMANCEWITH`: Oblivious: the target and its ability.
    PkmnPreventsRomanceWith,
    /// `STRINGID_PKMNSXINFATUATEDY`: Cute Charm: the target, its ability, the attacker.
    PkmnSXInfatuatedY,
    /// `STRINGID_TARGETCANTESCAPENOW`: Mean Look: the target.
    TargetCantEscapeNow,
    /// `STRINGID_PREVENTSESCAPE`: Magnet Pull: its owner (`active`) and ability.
    PreventsEscape,
    /// `STRINGID_ELECTRICITYWEAKENED`: Mud Sport.
    ElectricityWeakened,
    /// `STRINGID_FIREWEAKENED`: Water Sport.
    FireWeakened,
    /// `STRINGID_PKMNFLEDFROMBATTLE`: Teleport: the attacker.
    PkmnFledFromBattle,
    /// `STRINGID_PKMNSXMADEITINEFFECTIVE`: an ability's owner (`active`) and ability.
    PkmnSXMadeItIneffective,
    /// `STRINGID_PKMNCAUSEDUPROAR`: the attacker.
    PkmnCausedUproar,
    /// `STRINGID_PKMNMAKINGUPROAR`: the attacker.
    PkmnMakingUproar,
    /// `STRINGID_PKMNCALMEDDOWN`: the attacker.
    PkmnCalmedDown,
    /// `STRINGID_PKMNWOKEUPINUPROAR`: the attacker.
    PkmnWokeUpInUproar,
    /// `STRINGID_PKMNCANTSLEEPINUPROAR2`: the target.
    PkmnCantSleepInUproar,
    /// `STRINGID_UPROARKEPTPKMNAWAKE`: the target.
    UproarKeptPkmnAwake,
    /// `STRINGID_PKMNSTAYEDAWAKEUSING`: the target and its ability.
    PkmnStayedAwakeUsing,
    /// `STRINGID_PKMNWASMADEDROWSY`: the attacker and the target.
    PkmnWasMadeDrowsy,
    /// `STRINGID_PKMNIDENTIFIED`: Foresight: the attacker and the target.
    PkmnIdentified,
    /// `STRINGID_PKMNLOAFING`: Truant: the attacker.
    PkmnLoafing,
    /// `STRINGID_PKMNSXCUREDYPROBLEM`: Shed Skin: its owner (`active`), ability, `B_BUFF1` = the status name.
    PkmnSXCuredYProblem,
    /// `STRINGID_PKMNSXPREVENTSFLINCHING`: Inner Focus: the target and its ability.
    PkmnSXPreventsFlinching,
    /// `STRINGID_PKMNSXBLOCKSY`: Soundproof: the target, its ability, the move.
    PkmnSXBlocksY,
    /// `STRINGID_PKMNPROTECTEDBY`: Sturdy: the target and its ability.
    PkmnProtectedBy,
    /// `STRINGID_PKMNPREVENTSSTATLOSSWITH`: Clear Body: its owner (`active`) and ability.
    PkmnPreventsStatLossWith,
    /// `STRINGID_PREVENTEDFROMWORKING`: Clear Body against Intimidate: the defender and its
    /// ability, the Intimidate user (`active`), `B_BUFF1` = "Intimidate".
    PreventedFromWorking,
    /// `STRINGID_ITSUCKEDLIQUIDOOZE`: Liquid Ooze (Absorb, Leech Seed).
    ItSuckedLiquidOoze,
    /// STRINGID_PLAYERGOTMONEY (buffer 1: the money).
    PlayerGotMoney,
    // ---- Silice engine: held items (`buffs[2]` = `Buff::Item`), weather, abilities ----
    /// `STRINGID_PKMNSITEMRESTOREDHEALTH`: an HP berry (`active` = its holder).
    ItemRestoredHealth,
    /// `STRINGID_PKMNSITEMRESTOREDHPALITTLE`: Leftovers.
    ItemRestoredHpALittle,
    /// `STRINGID_PKMNSITEMCUREDPARALYSIS`.
    ItemCuredParalysis,
    /// `STRINGID_PKMNSITEMCUREDPOISON`.
    ItemCuredPoison,
    /// `STRINGID_PKMNSITEMHEALEDBURN`.
    ItemHealedBurn,
    /// `STRINGID_PKMNSITEMDEFROSTEDIT`.
    ItemDefrostedIt,
    /// `STRINGID_PKMNSITEMWOKEIT`.
    ItemWokeIt,
    /// `STRINGID_PKMNSITEMSNAPPEDOUT`: confusion cured.
    ItemSnappedOut,
    /// `STRINGID_PKMNSITEMCUREDPROBLEM` (`B_BUFF1`: the status name, left empty as Shed Skin's).
    ItemCuredProblem,
    /// `STRINGID_PKMNSITEMNORMALIZEDSTATUS`: Lum on several problems.
    ItemNormalizedStatus,
    /// `STRINGID_PKMNHUNGONWITHX`: Focus Band (the target).
    HungOnWithX,
    /// `STRINGID_PKMNSXINTENSIFIEDSUN`: Drought (`active` and `buffs[2]` its ability).
    IntensifiedSun,
    /// `STRINGID_PKMNSXWHIPPEDUPSANDSTORM`: Sand Stream.
    WhippedUpSandstorm,
    /// `STRINGID_PKMNMADEITRAIN`: Drizzle.
    MadeItRain,
    /// `STRINGID_RAINCONTINUES`.
    RainContinues,
    /// `STRINGID_SUNLIGHTSTRONG`.
    SunlightStrong,
    /// `STRINGID_SANDSTORMRAGES`.
    SandstormRages,
    /// `STRINGID_PKMNBUFFETEDBYSANDSTORM`.
    BuffetedBySandstorm,
    /// `STRINGID_PKMNRAISEDSPEED`: Speed Boost, Steadfast (`active`, its ability).
    RaisedSpeed,
    /// `STRINGID_PKMNMAKESGROUNDMISS`: Levitate (the target and its ability).
    MakesGroundMiss,
    /// `STRINGID_PKMNSXRESTOREDHPALITTLE2`: Rain Dish.
    AbilityRestoredHp,
    // Post-Gen3 messages: no Gen3 string (Emerald content leaves them empty; a game with the
    // features supplies them).
    /// Mega Evolution, first message: `active` the mon, `buffs` = [`Mon`, `Species`(form),
    /// `Item`(stone)].
    MegaReacting,
    /// Mega Evolution done (same buffers).
    MegaEvolved,
    /// Primal Reversion (same buffers, the orb).
    PrimalReversion,
    /// Primordial Sea: a heavy rain began (`active`, ability).
    HeavyRainBegan,
    /// Desolate Land: the sunlight turned extremely harsh.
    HarshSunBegan,
    /// The heavy rain lifted (its source left).
    HeavyRainLifted,
    /// The extremely harsh sunlight faded.
    HarshSunFaded,
    /// A Fire move fizzled out in the heavy rain (the attacker).
    FizzledInRain,
    /// A Water move evaporated in the harsh sunlight.
    EvaporatedInSun,
    /// Mold Breaker on entry (`active`, ability).
    BreaksTheMold,
    /// Magic Bounce: the move bounced back (`active` its owner, ability; `mov`).
    BouncedBack,
    /// The fight menu's Mega Evolution mark (in the TYPE window while the toggle is on).
    MegaArmed,
    /// Solar Power's HP loss in the sun (`active`, ability).
    SolarPowerHurt,
    // Wave C: abilities and held items (codes from 200, apart from the move effects').
    /// `STRINGID_PKMNPREVENTSPARALYSISWITH` (Limber; `target` the effect battler, its ability).
    PkmnPreventsParalysisWith,
    /// `STRINGID_PKMNPREVENTSPOISONINGWITH` (Immunity).
    PkmnPreventsPoisoningWith,
    /// `STRINGID_PKMNPREVENTSCONFUSIONWITH` (Own Tempo).
    PkmnPreventsConfusionWith,
    /// `STRINGID_PKMNSXPREVENTSBURNS` (Water Veil).
    PkmnSXPreventsBurns,
    /// `STRINGID_PKMNSXPREVENTSYSZ`: an ability's status blocked by an ability.
    PkmnSXPreventsYSZ,
    /// `STRINGID_PKMNRESTOREDHPUSING` (Volt / Water Absorb).
    PkmnRestoredHpUsing,
    /// `STRINGID_PKMNSXMADEYUSELESS` (Volt / Water Absorb at full HP).
    PkmnSXMadeYUseless,
    /// `STRINGID_PKMNRAISEDFIREPOWERWITH` (Flash Fire).
    PkmnRaisedFirePowerWith,
    /// `STRINGID_PKMNSXMADEYINEFFECTIVE` (Flash Fire, already boosted).
    PkmnSXMadeYIneffective,
    /// `STRINGID_PKMNCHANGEDTYPEWITH` (Color Change; `buffs[0]` the type).
    PkmnChangedTypeWith,
    /// `STRINGID_PKMNHURTSWITH` (Rough Skin).
    PkmnHurtsWith,
    /// `STRINGID_PKMNSXCUREDITSYPROBLEM` (`ABILITYEFFECT_IMMUNITY`).
    PkmnSXCuredItsYProblem,
    /// `STRINGID_PKMNTRANSFORMED` (Forecast).
    PkmnTransformed,
    /// `STRINGID_PKMNPREVENTSUSAGE` (Damp).
    PkmnPreventsUsage,
    /// `STRINGID_AVOIDEDDAMAGE` (Wonder Guard).
    AvoidedDamage,
    /// `STRINGID_PKMNSITEMRESTOREDSTATUS` (White Herb).
    ItemRestoredStatus,
    /// `STRINGID_PKMNSITEMRESTOREDPP` (Leppa Berry; `buffs[0]` the move).
    ItemRestoredPp,
    /// `STRINGID_FORXCOMMAYZ` (a confusing berry; `buffs[0]` the flavour text).
    ForXCommaYZ,
    /// `STRINGID_PKMNUSEDXTOGETPUMPED` (Lansat Berry).
    UsedXToGetPumped,
    /// `STRINGID_USINGITEMSTATOFPKMNROSE` (the stat berries; `buffs` = [stat, rose text, item]).
    UsingItemStatRose,
    /// `STRINGID_PKMNFLEDUSINGITS` (Smoke Ball).
    FledUsingIts,
    /// `STRINGID_HMMOVESCANTBEFORGOTTEN` (the learn-move pick, on an HM move).
    HmMovesCantBeForgotten,
    /// `gAbilityNames[a]`. The ability codes (`B_ATK_ABILITY`, `B_DEF_ABILITY`,
    /// `B_SCR_ACTIVE_ABILITY`, `B_EFF_ABILITY`, `B_LAST_ABILITY`) expand `Msg::buffs[2]`, which
    /// the core sets to `Buff::Text(Str::AbilityName(..))` for the messages that name one.
    /// `Ability::Other` and `Ability::None` have no name (empty).
    AbilityName(Ability),
}

impl Str {
    /// A stable hash code: the declaration order (retired variants leave gaps: 2, 3), with
    /// the index of `TypeName` / `StatName` in the low byte. Exhaustive on purpose: a new
    /// variant gets its code here, and a removed one's code is never reused.
    pub const fn code(self) -> u16 {
        use Str::*;
        match self {
            Intro1 => 0,
            IntroTrainer => 1,
            // 2 and 3 were `IntroMsg` and `Intro2` (retired, never shown): codes are not reused.
            GoPkmn => 4,
            PkmnSentOut => 5,
            WhatWillPkmnDo => 6,
            BattleMenu => 7,
            MoveInterfacePP => 8,
            MoveInterfaceType => 9,
            UsedMove => 10,
            AttackMissed => 11,
            CriticalHit => 12,
            SuperEffective => 13,
            NotVeryEffective => 14,
            ItDoesntAffect => 15,
            PkmnFainted => 16,
            GotAwaySafely => 17,
            CantEscape => 18,
            DontLeaveBirch => 19,
            WildPkmnFled => 20,
            PlayerDefeatedTrainer => 21,
            PlayerWhiteout => 22,
            PlayerWhiteout2 => 23,
            PkmnGainedExp => 24,
            PkmnGrewToLv => 25,
            PkmnLearnedMove => 26,
            TryToLearnMove1 => 27,
            TryToLearnMove2 => 28,
            TryToLearnMove3 => 29,
            PkmnForgotMove => 30,
            StopLearningMove => 31,
            DidNotLearnMove => 32,
            // Renamed from AttackerStatChanged / DefenderStatChanged (E2): the Rose forms keep
            // the old codes, the Fell forms are new.
            AttackerStatRose => 33,
            DefenderStatRose => 34,
            StatRose => 35,
            StatSharplyRose => 36,
            StatFell => 37,
            StatHarshlyFell => 38,
            StatWontIncrease => 39,
            StatWontDecrease => 40,
            ButItFailed => 41,
            PkmnWasPoisoned => 42,
            PkmnWasBurned => 43,
            PkmnWasParalyzed => 44,
            PkmnFellAsleep => 45,
            PkmnBecameConfused => 46,
            PkmnFlinched => 47,
            PkmnIsConfused => 48,
            PkmnHurtItself => 49,
            PkmnIsParalyzed => 50,
            PkmnFastAsleep => 51,
            PkmnWokeUp => 52,
            PkmnHurtByPoison => 53,
            PkmnHurtByBurn => 54,
            PkmnSeeded => 55,
            LeechSeedDrain => 56,
            PkmnStoringEnergy => 57,
            PkmnUnleashedEnergy => 58,
            GettingPumped => 59,
            AbsorbDrain => 60,
            PkmnHitWithRecoil => 61,
            NoPPLeft => 62,
            Struggle => 63,
            NoRunningFromTrainers => 64,
            Gotcha => 65,
            WildPrefix => 66,
            FoePrefix => 67,
            BattleYesNoChoice => 68,
            ConfirmStarterChoice => 69,
            BirchInTrouble => 72,
            TypeName(t) => 70 << 8 | t as u16,
            StatName(s) => 71 << 8 | s as u16,
            // Task E2 (after BirchInTrouble = 72).
            AttackerStatFell => 73,
            DefenderStatFell => 74,
            PkmnBadlyPoisoned => 75,
            PkmnWasFrozen => 76,
            PkmnIsFrozen => 77,
            PkmnWasDefrosted2 => 78,
            PkmnWasDefrosted => 79,
            PkmnHealedConfusion => 80,
            PkmnPoisonedBy => 81,
            PkmnBurnedBy => 82,
            PkmnFrozenBy => 83,
            PkmnWasParalyzedBy => 84,
            PkmnMadeSleep => 85,
            PkmnAlreadyAsleep => 86,
            PkmnAlreadyPoisoned => 87,
            PkmnIsAlreadyParalyzed => 88,
            PkmnAlreadyHasBurn => 89,
            PkmnAlreadyConfused => 90,
            PkmnEvadedAttack => 91,
            NaturePowerTurnedInto => 92,
            PkmnCutsAttackWith => 93,
            PkmnTraced => 94,
            PkmnSXPreventsYLoss => 95,
            PkmnSXHadNoEffectOnY => 96,
            PkmnBrokeFree => 97,
            ItAppearedCaught => 98,
            AarghAlmostHadIt => 99,
            ShootSoClose => 100,
            HitXTimes => 101,
            ButNothingHappened => 102,
            PkmnChargingPower => 103,
            // Wave C move effects (from 300).
            PkmnRegainedHealth => 300,
            PkmnHpFull => 301,
            StatChangesGone => 302,
            PkmnWasntAffected => 303,
            PkmnDreamEaten => 304,
            CoinsScattered => 305,
            PkmnCrashed => 306,
            PkmnMustRecharge => 307,
            PkmnRageBuilding => 308,
            PkmnFellIntoNightmare => 309,
            PkmnLockedInNightmare => 310,
            PkmnLaidCurse => 311,
            PkmnAfflictedByCurse => 312,
            PkmnProtectedItself2 => 313,
            PkmnBracedItself => 314,
            PkmnProtectedItself => 315,
            PkmnEnduredHit => 316,
            SpikesScattered => 317,
            PkmnHurtBySpikes => 318,
            FaintInThree => 319,
            PkmnPerishCountFell => 320,
            StartedToRain => 321,
            SunlightGotBright => 322,
            SandstormBrewed => 323,
            StartedHail => 324,
            RainStopped => 325,
            SunlightFaded => 326,
            SandstormSubsided => 327,
            HailContinues => 328,
            HailStopped => 329,
            PkmnPeltedByHail => 330,
            PkmnRaisedDef => 331,
            PkmnRaisedSpDef => 332,
            PkmnCoveredByVeil => 333,
            PkmnSXWoreOff => 334,
            PkmnSafeguardExpired => 335,
            PkmnUsedSafeguard => 336,
            PkmnShroudedInMist => 337,
            PkmnProtectedByMist => 338,
            PkmnWishCameTrue => 339,
            PkmnPlantedRoots => 340,
            PkmnAbsorbedNutrients => 341,
            PkmnAnchoredItself => 342,
            PkmnCenterAttention => 343,
            PkmnSqueezedByBind => 344,
            PkmnWrappedBy => 345,
            PkmnTrappedInVortex => 346,
            PkmnClamped => 347,
            PkmnTrappedBySandTomb => 348,
            PkmnHurtBy => 349,
            PkmnFreedFrom => 350,
            PkmnGotFree => 351,
            PkmnShedLeechSeed => 352,
            PkmnBlewAwaySpikes => 353,
            PkmnFatigueConfusion => 354,
            PkmnTookAim => 355,
            PkmnWasDraggedOut => 356,
            PkmnLostFocus => 357,
            PkmnTighteningFocus => 358,
            PkmnStockpiled => 359,
            PkmnCantStockpile => 360,
            FailedToSpitUp => 361,
            FailedToSwallow => 362,
            PkmnForesawAttack => 363,
            PkmnChoseXAsDestiny => 364,
            PkmnTookAttack => 365,
            PkmnCutHpMaxedAttack => 366,
            PkmnCopiedStatChanges => 367,
            PkmnChangedType => 368,
            PkmnMoveWasDisabled => 369,
            PkmnMoveIsDisabled => 370,
            PkmnMoveDisabledNoMore => 371,
            PkmnGotEncore => 372,
            PkmnEncoreEnded => 373,
            PkmnSubjectedToTorment => 374,
            PkmnCantUseMoveTorment => 375,
            PkmnFellForTaunt => 376,
            PkmnCantUseMoveTaunt => 377,
            PkmnReducedPP => 378,
            PkmnTryingToTakeFoe => 379,
            PkmnTookFoe => 380,
            PkmnStatusNormal => 381,
            BellChimed => 382,
            SoothingAroma => 383,
            SharedPain => 384,
            PkmnStoleItem => 385,
            PkmnKnockedOff => 386,
            TheWallShattered => 387,
            MagnitudeStrength => 388,
            PkmnWhippedWhirlwind => 389,
            PkmnTookSunlight => 390,
            PkmnLoweredHead => 391,
            PkmnIsGlowing => 392,
            PkmnFlewHigh => 393,
            PkmnDugHole => 394,
            PkmnHidUnderwater => 395,
            PkmnSprangUp => 396,
            ButNoEffect => 397,
            OneHitKo => 398,
            PkmnUnaffected => 399,
            MirrorMoveFailed => 400,
            PkmnSleptHealthy => 401,
            PkmnWentToSleep => 402,
            PkmnWasDefrostedBy => 403,
            PkmnAlreadyAsleep2 => 404,
            PkmnSXBlocksY2 => 406,
            StatsWontIncrease2 => 407,
            StatsWontDecrease2 => 408,
            PkmnHealedParalysis => 409,
            PkmnCopiedFoe => 410,
            PkmnSwappedAbilities => 411,
            PkmnWantsGrudge => 412,
            PkmnLostPPGrudge => 413,
            PkmnSealedOpponentMove => 414,
            PkmnCantUseMoveSealed => 415,
            PkmnShroudedItself => 416,
            PkmnMoveBounced => 417,
            PkmnSwitchedItems => 418,
            PkmnObtainedX => 419,
            PkmnObtainedX2 => 420,
            PkmnObtainedXYObtainedZ => 421,
            PkmnAnchorsItselfWith => 422,
            PkmnMadeSubstitute => 423,
            PkmnHasSubstitute => 424,
            SubstituteDamaged => 425,
            PkmnSubstituteFaded => 426,
            TooWeakForSubstitute => 427,
            PkmnTransformedInto => 428,
            PkmnLearnedMove2 => 429,
            PkmnSketchedMove => 430,
            XFoundOneY => 431,
            PkmnWaitsForTarget => 432,
            PkmnSnatchedMove => 433,
            PkmnAttack => 434,
            PlayerPickedUpMoney => 435,
            AbilityName(a) => 104 << 8 | a as u16,
            // Task G.
            PkmnFellInLove => 105,
            PkmnInLove => 106,
            PkmnImmobilizedByLove => 107,
            PkmnPreventsRomanceWith => 108,
            PkmnSXInfatuatedY => 109,
            TargetCantEscapeNow => 110,
            PreventsEscape => 111,
            ElectricityWeakened => 112,
            FireWeakened => 113,
            PkmnFledFromBattle => 114,
            PkmnSXMadeItIneffective => 115,
            PkmnCausedUproar => 116,
            PkmnMakingUproar => 117,
            PkmnCalmedDown => 118,
            PkmnWokeUpInUproar => 119,
            PkmnCantSleepInUproar => 120,
            UproarKeptPkmnAwake => 121,
            PkmnStayedAwakeUsing => 122,
            PkmnWasMadeDrowsy => 123,
            PkmnIdentified => 124,
            PkmnLoafing => 125,
            PkmnSXCuredYProblem => 126,
            PkmnSXPreventsFlinching => 127,
            PkmnSXBlocksY => 128,
            PkmnProtectedBy => 129,
            PkmnPreventsStatLossWith => 130,
            PreventedFromWorking => 131,
            ItSuckedLiquidOoze => 132,
            PlayerGotMoney => 133,
            // Silice engine.
            ItemRestoredHealth => 134,
            ItemRestoredHpALittle => 135,
            ItemCuredParalysis => 136,
            ItemCuredPoison => 137,
            ItemHealedBurn => 138,
            ItemDefrostedIt => 139,
            ItemWokeIt => 140,
            ItemSnappedOut => 141,
            ItemCuredProblem => 142,
            ItemNormalizedStatus => 143,
            HungOnWithX => 144,
            IntensifiedSun => 145,
            WhippedUpSandstorm => 146,
            MadeItRain => 147,
            RainContinues => 148,
            SunlightStrong => 149,
            SandstormRages => 150,
            BuffetedBySandstorm => 151,
            RaisedSpeed => 152,
            MakesGroundMiss => 153,
            AbilityRestoredHp => 154,
            MegaReacting => 155,
            MegaEvolved => 156,
            PrimalReversion => 157,
            HeavyRainBegan => 158,
            HarshSunBegan => 159,
            HeavyRainLifted => 160,
            HarshSunFaded => 161,
            FizzledInRain => 162,
            EvaporatedInSun => 163,
            BreaksTheMold => 164,
            BouncedBack => 165,
            MegaArmed => 166,
            SolarPowerHurt => 167,
            PkmnPreventsParalysisWith => 200,
            PkmnPreventsPoisoningWith => 201,
            PkmnPreventsConfusionWith => 202,
            PkmnSXPreventsBurns => 203,
            PkmnSXPreventsYSZ => 204,
            PkmnRestoredHpUsing => 205,
            PkmnSXMadeYUseless => 206,
            PkmnRaisedFirePowerWith => 207,
            PkmnSXMadeYIneffective => 208,
            PkmnChangedTypeWith => 209,
            PkmnHurtsWith => 210,
            PkmnSXCuredItsYProblem => 211,
            PkmnTransformed => 212,
            PkmnPreventsUsage => 213,
            AvoidedDamage => 214,
            ItemRestoredStatus => 215,
            ItemRestoredPp => 216,
            ForXCommaYZ => 217,
            UsedXToGetPumped => 218,
            UsingItemStatRose => 219,
            FledUsingIts => 220,
            HmMovesCantBeForgotten => 221,
        }
    }
}

/// What the engine did not do as the game does, for the host to log (not presentation: the
/// scene keeps these aside, see [`BattleScene::take_reports`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattleReport {
    /// A move effect the engine does not implement (the raw effect number); the move fell back
    /// to a plain hit or "But it failed!".
    UnsupportedEffect(u8),
    /// A behaviour approximated for lack of data or support (what, in a few words).
    Degraded(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattleEvent {
    Msg(Msg),
    // Pic slides.
    WildAppear,
    TrainerAppear,
    TrainerSlideOut,
    /// The trainer slides back in after "Player defeated X!". The caller (Task K) then prints
    /// the defeat text, `BattleKind::Trainer::defeat` of `Battle::setup()`, waiting for a
    /// button (`MsgWait::Button`), before the battle ends.
    TrainerDefeated,
    SendOut {
        side: Side,
        party: u8,
    },
    Recall {
        side: Side,
    },
    MoveAnim {
        attacker: Side,
        mov: MoveId,
    },
    Hit {
        target: Side,
        eff: Effectiveness,
    },
    Hp {
        side: Side,
        from: u16,
        to: u16,
    },
    StatusIcon {
        side: Side,
        status: Status,
    },
    StatAnim {
        side: Side,
        up: bool,
    },
    Faint {
        side: Side,
    },
    Exp {
        party: u8,
        from: u32,
        to: u32,
    },
    LevelUp {
        party: u8,
        level: u8,
    },
    Ball {
        shakes: u8,
        caught: bool,
    },
    End(BattleOutcome),
    /// Not presentation: [`BattleScene::push`] sets it aside.
    Report(BattleReport),
    /// The battler on `side` (party slot `party`) changed form for the rest of the battle
    /// (Mega Evolution, Primal Reversion): its pic becomes `species`'.
    FormChange {
        side: Side,
        party: u8,
        species: SpeciesId,
    },
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

/// Battle interface graphics. The exact contents of each field are listed in
/// `crates/extract/src/battle_gfx.rs` (Task B); Task I fills it. Empty slices draw nothing.
#[derive(Clone, Copy)]
pub struct BattleGfx {
    /// `gBattleTextboxTiles` / `Tilemap` / `Palette`; palettes `[textbox_0, textbox_1, text,
    /// text_pp]`.
    pub textbox: GfxImage,
    /// `gBattleEnvironment{Tiles,Tilemap,Palette}_*`, by `Environment as usize`.
    pub environments: [GfxImage; 10],
    /// `healthbox_singles_player` (64x64 x2, OBJs at tiles 0 and 64) / `_opponent` (64x32 x2,
    /// OBJs at tiles 0 and 32), OBJ 1D order, the healthbox palette in `palettes[0]`.
    pub healthbox_player: GfxImage,
    pub healthbox_opponent: GfxImage,
    /// `hpbar` tiles and the HP bar palette.
    pub hpbar: GfxImage,
    pub expbar: GfxImage,
    /// `status*` icons (FR ROM: may be localized).
    pub status: GfxImage,
    /// `misc`: the healthbox's black sections and blank "no status" tile, then the HP bar frame
    /// end. The level, HP numbers and gender symbol are printed with `small_font`.
    pub misc: GfxImage,
    /// `gBallGfx_Poke` + palette (send-out ball).
    pub ball: GfxImage,
    /// `FONT_SMALL` (`gFontSmallLatinGlyphs`), healthbox nicknames.
    pub small_font: &'static Font,
    /// The player's back pic, frames of 64 tiles one after the other; `y` is the Gen3 `.size`.
    pub player_back: TrainerPic,
    /// `starter_choose.c`: bag + grass background (tilemap: the bag map padded to 1024 entries,
    /// then the grass map), Poké Ball and circle OBJs.
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
    ball: GfxImage::EMPTY,
    small_font: &EMPTY_FONT,
    player_back: TrainerPic {
        tiles: &[],
        palette: &[0; 16],
        y: 8,
    },
    starter_bag: GfxImage::EMPTY,
    starter_ball: GfxImage::EMPTY,
    starter_circle: GfxImage::EMPTY,
};

#[cfg(test)]
mod tests;
