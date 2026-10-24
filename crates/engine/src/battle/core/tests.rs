use super::*;
use crate::battle::calc;
use crate::battle::test_data::*;
use crate::battle::{Environment, Names, text};
use crate::data::{TextId, TrainerId};
use crate::hash::Fnv64;
use crate::mon::{Stat, Status};
use alloc::format;
use alloc::string::String;

pub(crate) fn side(s: Side) -> char {
    match (s.is_player(), s.is_right()) {
        (true, false) => 'P',
        (false, false) => 'O',
        (true, true) => 'p',
        (false, true) => 'o',
    }
}

/// One line per event, compact enough to pin whole battles.
pub(crate) fn fmt(e: &BattleEvent) -> String {
    match e {
        BattleEvent::Msg(m) => {
            let mut s = format!("{:?}@{}", m.id, side(m.active));
            if let (Str::UsedMove, Some(mv)) = (m.id, m.mov) {
                s += &format!(" move:{}", mv.0);
            }
            for b in m.buffs {
                match b {
                    Buff::None => {}
                    Buff::Mon { side: sd, party } => s += &format!(" mon:{}{}", side(sd), party),
                    Buff::MonNick { side: sd, party } => {
                        s += &format!(" nick:{}{}", side(sd), party)
                    }
                    Buff::Move(mv) => s += &format!(" move:{}", mv.0),
                    Buff::Number(n) => s += &format!(" {n}"),
                    Buff::Stat(st) => s += &format!(" {st:?}"),
                    Buff::Text(t) => s += &format!(" {t:?}"),
                    other => s += &format!(" {other:?}"),
                }
            }
            match m.wait {
                MsgWait::Frames(0) => {}
                MsgWait::Frames(n) => s += &format!(" ~{n}"),
                MsgWait::Button => s += " !",
            }
            s
        }
        BattleEvent::SendOut { side: sd, party } => format!("SendOut {}{}", side(*sd), party),
        BattleEvent::Recall { side: sd } => format!("Recall {}", side(*sd)),
        BattleEvent::MoveAnim { attacker, mov } => {
            format!("Anim {} move:{}", side(*attacker), mov.0)
        }
        BattleEvent::Hit { target, eff } => format!("Hit {} {eff:?}", side(*target)),
        BattleEvent::Hp { side: sd, from, to } => format!("Hp {} {from}->{to}", side(*sd)),
        BattleEvent::StatAnim { side: sd, up } => format!("StatAnim {} {up}", side(*sd)),
        BattleEvent::Faint { side: sd } => format!("Faint {}", side(*sd)),
        BattleEvent::Exp { party, from, to } => format!("Exp {party} {from}->{to}"),
        BattleEvent::LevelUp { party, level } => format!("LevelUp {party} {level}"),
        other => format!("{other:?}"),
    }
}

pub(crate) fn lines(events: &[BattleEvent]) -> Vec<String> {
    events.iter().map(fmt).collect()
}

/// The seed whose `Rng`, after `Battle::new`'s four `BattleAI_HandleItemUseBeforeAISetup`
/// draws, is `Rng::new(seed)` (xorshift32 run backwards): the hand-traced goldens below start
/// from the RNG state after those draws, which [`setup_draws_come_first`] checks.
pub(crate) fn before_setup_draws(seed: u32) -> u32 {
    fn unshl(y: u32, s: u32) -> u32 {
        let mut x = y;
        for _ in 0..32 / s {
            x = y ^ (x << s);
        }
        x
    }
    fn unshr(y: u32, s: u32) -> u32 {
        let mut x = y;
        for _ in 0..32 / s {
            x = y ^ (x >> s);
        }
        x
    }
    let mut x = Rng::new(seed).state();
    for _ in 0..4 {
        x = unshl(unshr(unshl(x, 5), 17), 13);
    }
    x
}

/// A started battle and its RNG.
pub(crate) struct Run {
    pub b: Battle,
    pub rng: Rng,
    pub out: Vec<BattleEvent>,
}

impl Run {
    /// `seed` is the RNG state after the setup draws ([`before_setup_draws`]).
    pub fn new(kind: BattleKind, player: &[Pokemon], opponents: &[Pokemon], seed: u32) -> Run {
        let mut rng = Rng::new(before_setup_draws(seed));
        let setup = BattleSetup {
            kind,
            opponents: party(opponents),
            environment: Environment::Grass,
            double: None,
        };
        let mut b = Battle::new(&Game, setup, &party(player), &mut rng);
        let mut out = Vec::new();
        b.start(&Game, &mut rng, &mut out);
        Run { b, rng, out }
    }

    pub fn wild(player: &[Pokemon], wild: Pokemon, seed: u32) -> Run {
        Run::new(
            BattleKind::Wild {
                first_battle: false,
            },
            player,
            &[wild],
            seed,
        )
    }

    /// Answers with `c`; returns this answer's events.
    pub fn choose(&mut self, c: Choice) -> Vec<String> {
        let mut out = Vec::new();
        self.b.choose(&Game, &mut self.rng, c, &mut out);
        self.out.extend_from_slice(&out);
        lines(&out)
    }

    pub fn intro(&self) -> Vec<String> {
        lines(&self.out)
    }
}

pub(crate) fn water(level: u8) -> Pokemon {
    // Mudkip L5: IVs 10, Adamant → [20, 13, 10, 9, 9, 10].
    mon(WATER, level, [20, 13, 10, 9, 9, 10], &[TACKLE, GROWL])
}

pub(crate) fn normal(level: u8) -> Pokemon {
    // Zigzagoon L2: IVs 0, Hardy → HP 13, Def 6.
    mon(NORMAL, level, [13, 6, 6, 8, 5, 5], &[TACKLE, GROWL])
}

pub(crate) fn trainer() -> BattleKind {
    BattleKind::Trainer {
        trainer: TrainerId(1),
        defeat: TextId(7),
    }
}

pub(crate) fn first_mover(events: &[String]) -> char {
    let used = events.iter().find(|l| l.starts_with("UsedMove@")).unwrap();
    used.chars().nth("UsedMove@".len()).unwrap()
}

pub(crate) fn hash(b: &Battle) -> u64 {
    let mut h = Fnv64::new();
    b.hash(&mut h);
    h.finish()
}

#[test]
fn wild_battle_won_in_three_turns() {
    let mut r = Run::wild(&[water(5)], normal(2), 1);
    assert_eq!(
        r.intro(),
        ["WildAppear", "Intro1@O", "GoPkmn@P nick:P0", "SendOut P0"]
    );
    assert_eq!(r.b.need(), Need::Action);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:1",
            "Anim P move:1",
            "Hit O Normal",
            "Hp O 13->6",
            "UsedMove@O move:2",
            "Anim O move:2",
            "StatAnim P false",
            "DefenderStatFell@P Atk StatFell ~64",
        ]
    );
    assert_eq!(r.b.need(), Need::Action);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:1",
            "Anim P move:1",
            "Hit O Normal",
            // After Growl (-1): Atk 13 * 2 / 3 = 8, base damage 5 instead of 8.
            "Hp O 6->2",
            "UsedMove@O move:1",
            "Anim O move:1",
            "Hit P Normal",
            "Hp P 20->17",
        ]
    );
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:1",
            "Anim P move:1",
            "Hit O Normal",
            "Hp O 2->0",
            "Faint O",
            "PkmnFainted@O mon:O0",
            "PkmnGainedExp@P mon:P0 17",
            "Exp 0 125->142",
            "End(Won)",
        ]
    );
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::Won));
    // HP, PP and exp are written back to the party.
    let p = r.b.party(Side::Player).get(0).unwrap();
    assert_eq!((p.hp, p.exp, p.pp[0]), (17, 142, 32));
    // Ended: further choices do nothing.
    assert!(r.choose(Choice::Move(0)).is_empty());
}

#[test]
fn run_fails_until_odds_win() {
    // Slower than the foe: odds 9 * 128 / 30 = 38, +30 per failed try.
    let fast = mon(NORMAL, 2, [13, 6, 6, 30, 5, 5], &[TACKLE]);
    let mut r = Run::wild(&[water(5)], fast, 1);
    let fail = |hp: &str| -> Vec<String> {
        alloc::vec![
            "CantEscape@P ~64".into(),
            "UsedMove@O move:1".into(),
            "Anim O move:1".into(),
            "Hit P Normal".into(),
            format!("Hp P {hp}"),
        ]
    };
    assert_eq!(r.choose(Choice::Run), fail("20->17"));
    assert_eq!(r.choose(Choice::Run), fail("17->14"));
    assert_eq!(r.choose(Choice::Run), fail("14->11"));
    assert_eq!(r.choose(Choice::Run), fail("11->8"));
    assert_eq!(r.choose(Choice::Run), ["GotAwaySafely@P !", "End(Ran)"]);
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::Ran));
}

#[test]
fn faster_mon_always_escapes() {
    for seed in 1..10 {
        let mut r = Run::wild(&[water(5)], normal(2), seed);
        assert_eq!(r.choose(Choice::Run), ["GotAwaySafely@P !", "End(Ran)"]);
    }
}

#[test]
fn whiteout_when_the_last_mon_faints() {
    let weak = mon(WATER, 2, [5, 5, 5, 5, 5, 5], &[GROWL]);
    let strong = mon(NORMAL, 9, [40, 30, 20, 20, 10, 10], &[MEGA_HIT]);
    let mut r = Run::wild(&[weak], strong, 3);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@O move:11",
            "Anim O move:11",
            "Hit P Normal",
            "Hp P 5->0",
            "Faint P",
            "PkmnFainted@P mon:P0",
            "PlayerWhiteout@P ~64",
            "PlayerWhiteout2@P !",
            "End(Lost)",
        ]
    );
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::Lost));
}

fn level_up_run() -> Run {
    let fire = mon(
        FIRE,
        5,
        [20, 30, 10, 12, 10, 10],
        &[TACKLE, GROWL, HARDEN, SCREECH],
    );
    let mut r = Run::wild(&[fire], mon(NORMAL, 20, [14, 5, 5, 5, 5, 5], &[GROWL]), 1);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:1",
            "Anim P move:1",
            "Hit O Normal",
            "Hp O 14->0",
            "Faint O",
            "PkmnFainted@O mon:O0",
            "PkmnGainedExp@P mon:P0 171",
            "Exp 0 125->216",
            "LevelUp 0 6",
            "PkmnGrewToLv@P mon:P0 6",
            "TryToLearnMove1@P mon:P0 move:3",
            "TryToLearnMove2@P mon:P0 move:3",
            "TryToLearnMove3@P mon:P0 move:3",
        ]
    );
    assert_eq!(
        r.b.need(),
        Need::LearnMove {
            party: 0,
            mov: EMBER
        }
    );
    r
}

#[test]
fn level_up_forgets_moves_to_learn() {
    let mut r = level_up_run();
    // Not a learn answer: refused.
    assert!(r.choose(Choice::Move(0)).is_empty());
    assert_eq!(
        r.choose(Choice::LearnMove { forget: Some(0) }),
        [
            "PkmnForgotMove@P mon:P0 move:1",
            "PkmnLearnedMove@P mon:P0 move:3 ~64",
            "TryToLearnMove1@P mon:P0 move:5",
            "TryToLearnMove2@P mon:P0 move:5",
            "TryToLearnMove3@P mon:P0 move:5",
        ]
    );
    assert_eq!(
        r.b.need(),
        Need::LearnMove {
            party: 0,
            mov: QUICK_ATTACK
        }
    );
    assert_eq!(
        r.choose(Choice::LearnMove { forget: Some(0) }),
        [
            "PkmnForgotMove@P mon:P0 move:3",
            "PkmnLearnedMove@P mon:P0 move:5 ~64",
            "Exp 0 216->296",
            "End(Won)",
        ]
    );
    let p = r.b.party(Side::Player).get(0).unwrap();
    assert_eq!(p.level, 6);
    assert_eq!(
        p.moves,
        [Some(QUICK_ATTACK), Some(GROWL), Some(HARDEN), Some(SCREECH)]
    );
}

#[test]
fn level_up_can_skip_learning() {
    let mut r = level_up_run();
    assert_eq!(
        r.choose(Choice::LearnMove { forget: None }),
        [
            "DidNotLearnMove@P mon:P0 move:3",
            "TryToLearnMove1@P mon:P0 move:5",
            "TryToLearnMove2@P mon:P0 move:5",
            "TryToLearnMove3@P mon:P0 move:5",
        ]
    );
    assert_eq!(
        r.choose(Choice::LearnMove { forget: None }),
        [
            "DidNotLearnMove@P mon:P0 move:5",
            "Exp 0 216->296",
            "End(Won)"
        ]
    );
    let p = r.b.party(Side::Player).get(0).unwrap();
    assert_eq!(
        p.moves,
        [Some(TACKLE), Some(GROWL), Some(HARDEN), Some(SCREECH)]
    );
}

#[test]
fn trainer_battle_sends_the_next_mon_and_pays_trainer_exp() {
    let ace = mon(FIRE, 10, [30, 40, 20, 20, 20, 20], &[MEGA_HIT]);
    let second = mon(DARK, 3, [15, 6, 6, 8, 5, 5], &[TACKLE]);
    let mut r = Run::new(trainer(), &[ace], &[normal(2), second], 1);
    assert_eq!(
        r.intro(),
        [
            "TrainerAppear",
            "IntroTrainer@O",
            "PkmnSentOut@O nick:O0",
            "TrainerSlideOut",
            "SendOut O0",
            "GoPkmn@P nick:P0",
            "SendOut P0",
        ]
    );
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:11",
            "Anim P move:11",
            "Hit O Normal",
            "Hp O 13->0",
            "Faint O",
            "PkmnFainted@O mon:O0",
            // 60 * 2 / 7 = 17, × 1.5 for a trainer's mon.
            "PkmnGainedExp@P mon:P0 25",
            "Exp 0 1000->1025",
            "PkmnSentOut@O nick:O1",
            "SendOut O1",
        ]
    );
    assert_eq!(r.b.mon(Side::Opponent).species, DARK);
    // No running from a trainer: refused at selection, no turn passes.
    assert_eq!(r.choose(Choice::Run), ["NoRunningFromTrainers@P"]);
    assert_eq!(r.b.need(), Need::Action);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:11",
            "Anim P move:11",
            "Hit O Normal",
            "Hp O 15->0",
            "Faint O",
            "PkmnFainted@O mon:O1",
            "PkmnGainedExp@P mon:P0 34",
            "Exp 0 1025->1059",
            "PlayerDefeatedTrainer@O",
            "TrainerDefeated",
            // The test game has no money table (`GameData::trainer_money`).
            "Report(Degraded(\"trainer money\"))",
            "End(Won)",
        ]
    );
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::Won));
}

#[test]
fn exp_is_split_between_participants() {
    let a = mon(WATER, 5, [20, 13, 10, 9, 9, 10], &[GROWL]);
    let b = mon(FIRE, 5, [20, 30, 9, 10, 13, 10], &[MEGA_HIT]);
    let foe = mon(NORMAL, 7, [25, 12, 10, 5, 10, 10], &[TACKLE]);
    let mut r = Run::wild(&[a, b], foe, 9);
    assert_eq!(
        r.choose(Choice::Switch(1)),
        [
            "Recall P",
            "GoPkmn@P nick:P1",
            "SendOut P1",
            "UsedMove@O move:1",
            "Anim O move:1",
            "Hit P Normal",
            "Hp P 20->14",
        ]
    );
    assert_eq!(r.b.mon(Side::Player).species, FIRE);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:11",
            "Anim P move:11",
            "Hit O Normal",
            "Hp O 25->0",
            "Faint O",
            "PkmnFainted@O mon:O0",
            "PkmnGainedExp@P mon:P0 30",
            "Exp 0 125->155",
            "PkmnGainedExp@P mon:P1 30",
            "Exp 1 125->155",
            "End(Won)",
        ]
    );
}

#[test]
fn player_picks_a_replacement_after_a_faint() {
    let weak = mon(WATER, 2, [5, 5, 5, 5, 5, 5], &[GROWL]);
    let back = mon(FIRE, 5, [20, 30, 9, 10, 13, 10], &[MEGA_HIT]);
    let strong = mon(NORMAL, 9, [40, 30, 20, 20, 10, 10], &[MEGA_HIT]);
    let mut r = Run::wild(&[weak, back], strong, 3);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@O move:11",
            "Anim O move:11",
            "Hit P Normal",
            "Hp P 5->0",
            "Faint P",
            "PkmnFainted@P mon:P0",
        ]
    );
    assert_eq!(r.b.need(), Need::Switch);
    // The fainted mon, an empty slot or a move are refused.
    assert!(r.choose(Choice::Switch(0)).is_empty());
    assert!(r.choose(Choice::Switch(5)).is_empty());
    assert!(r.choose(Choice::Move(0)).is_empty());
    assert_eq!(r.b.need(), Need::Switch);
    assert_eq!(
        r.choose(Choice::Switch(1)),
        ["GoPkmn@P nick:P1", "SendOut P1"]
    );
    assert_eq!(r.b.need(), Need::Action);
}

#[test]
fn switching_to_the_active_mon_is_refused() {
    let mut r = Run::wild(&[water(5), normal(5)], normal(2), 1);
    assert!(r.choose(Choice::Switch(0)).is_empty());
    assert!(r.choose(Choice::Switch(2)).is_empty());
    assert_eq!(r.b.need(), Need::Action);
}

#[test]
fn a_move_without_pp_is_refused() {
    let mut m = water(5);
    m.pp = [0, 5, 0, 0];
    let mut r = Run::wild(&[m], normal(2), 7);
    assert_eq!(r.choose(Choice::Move(0)), ["NoPPLeft@P"]);
    assert!(r.choose(Choice::Move(2)).is_empty());
    assert_eq!(r.b.need(), Need::Action);
}

#[test]
fn struggle_when_no_move_has_pp() {
    let mut m = water(5);
    m.pp = [0; 4];
    let mut r = Run::wild(&[m], mon(NORMAL, 2, [13, 6, 6, 8, 5, 5], &[SPLASH]), 7);
    assert_eq!(
        r.choose(Choice::Move(1)),
        [
            "Struggle@P",
            "UsedMove@P move:4",
            "Anim P move:4",
            "Hit O Normal",
            "Hp O 13->5",
            // Recoil: a quarter of the damage dealt.
            "Hp P 20->18",
            "PkmnHitWithRecoil@P ~64",
            // Splash is an E2 effect: reported, the stub fails.
            "Report(UnsupportedEffect(85))",
            "UsedMove@O move:10",
            "ButItFailed@O ~64",
        ]
    );
}

#[test]
fn stat_moves() {
    let me = mon(WATER, 5, [20, 13, 10, 9, 9, 10], &[GROWL, SWORDS_DANCE]);
    let mut r = Run::wild(&[me], mon(GHOST, 5, [20, 10, 10, 5, 10, 10], &[SPLASH]), 8);
    let foe_fails = [
        "Report(UnsupportedEffect(85))",
        "UsedMove@O move:10",
        "ButItFailed@O ~64",
    ];
    let growl = [
        "UsedMove@P move:2",
        "Anim P move:2",
        "StatAnim O false",
        "DefenderStatFell@O Atk StatFell ~64",
    ];
    assert_eq!(r.choose(Choice::Move(0)), [&growl[..], &foe_fails].concat());
    // A self-targeting move names its user as the "defender" (gBattlerTarget == attacker).
    let sd = [
        "UsedMove@P move:8",
        "Anim P move:8",
        "StatAnim P true",
        "DefenderStatRose@P Atk StatSharplyRose ~64",
    ];
    assert_eq!(r.choose(Choice::Move(1)), [&sd[..], &foe_fails].concat());
    for _ in 0..5 {
        r.choose(Choice::Move(0));
    }
    assert_eq!(r.b.battler(Side::Opponent).stages[Stat::Atk as usize], -6);
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:2",
            "StatWontDecrease@O Atk ~64",
            foe_fails[0],
            foe_fails[1],
            foe_fails[2],
        ]
    );
    r.choose(Choice::Move(1));
    assert_eq!(r.b.battler(Side::Player).stages[Stat::Atk as usize], 4);
    r.choose(Choice::Move(1));
    assert_eq!(
        r.choose(Choice::Move(1)),
        [
            "UsedMove@P move:8",
            "StatWontIncrease@P Atk ~64",
            foe_fails[0],
            foe_fails[1],
            foe_fails[2],
        ]
    );
}

#[test]
fn ghosts_are_immune_to_normal_moves() {
    let me = mon(WATER, 5, [20, 13, 10, 9, 9, 10], &[TACKLE, SWIFT]);
    let mut r = Run::wild(&[me], mon(GHOST, 5, [20, 10, 10, 5, 10, 10], &[SPLASH]), 8);
    let immune = |m: u8| -> Vec<String> {
        alloc::vec![
            format!("UsedMove@P move:{m}"),
            "ItDoesntAffect@O ~64".into(),
            "Report(UnsupportedEffect(85))".into(),
            "UsedMove@O move:10".into(),
            "ButItFailed@O ~64".into(),
        ]
    };
    assert_eq!(r.choose(Choice::Move(0)), immune(1));
    // Swift (an unsupported effect) still goes through the type chart.
    let swift = [&["Report(UnsupportedEffect(17))".into()][..], &immune(14)].concat();
    assert_eq!(r.choose(Choice::Move(1)), swift);
    assert_eq!(r.b.mon(Side::Opponent).hp, 20);
}

#[test]
fn first_battle_cannot_be_left_and_has_no_crits() {
    let first = BattleKind::Wild { first_battle: true };
    let mut r = Run::new(first, &[water(5)], &[normal(2)], 2);
    assert_eq!(r.choose(Choice::Run), ["DontLeaveBirch@P"]);
    assert_eq!(r.b.need(), Need::Action);
    // E2: the opponent now goes through `AI_FirstBattle` (4 `simulatedRNG` rolls + the
    // pick), so the seed moved (seed 1 now misses). Tackle: (13×35×4/6/50 + 2) = 8 → 7 after
    // the damage roll; a crit (×2) would deal 13..=16, and no crit roll is drawn.
    assert_eq!(
        r.choose(Choice::Move(0)),
        [
            "UsedMove@P move:1",
            "Anim P move:1",
            "Hit O Normal",
            "Hp O 13->6",
            "UsedMove@O move:2",
            "Anim O move:2",
            "StatAnim P false",
            "DefenderStatFell@P Atk StatFell ~64",
        ]
    );
}

#[test]
fn priority_beats_speed() {
    let quick = mon(WATER, 5, [20, 13, 10, 1, 9, 10], &[QUICK_ATTACK]);
    let slow_hit = mon(WATER, 5, [20, 13, 10, 99, 9, 10], &[SLOW_HIT]);
    let foe = mon(NORMAL, 9, [40, 6, 30, 50, 5, 5], &[TACKLE]);
    for seed in 1..8 {
        let mut r = Run::wild(&[quick], foe, seed);
        assert_eq!(first_mover(&r.choose(Choice::Move(0))), 'P');
        let mut r = Run::wild(&[slow_hit], foe, seed);
        assert_eq!(first_mover(&r.choose(Choice::Move(0))), 'O');
    }
}

#[test]
fn speed_ties_draw_one_roll() {
    let me = mon(WATER, 5, [40, 5, 30, 10, 9, 10], &[TACKLE]);
    let foe = mon(NORMAL, 5, [40, 5, 30, 10, 5, 5], &[TACKLE; 4]);
    let (mut p_first, mut o_first) = (0, 0);
    for seed in 1..40 {
        let mut r = Run::wild(&[me], foe, seed);
        // The tie roll is the turn's first `Random()`.
        let mut peek = r.rng;
        let roll = peek.random();
        let first = first_mover(&r.choose(Choice::Move(0)));
        assert_eq!(first, if roll & 1 != 0 { 'O' } else { 'P' }, "seed {seed}");
        if first == 'P' {
            p_first += 1;
        } else {
            o_first += 1;
        }
    }
    assert!(p_first > 0 && o_first > 0);
}

#[test]
fn paralysis_quarters_speed() {
    let mut me = mon(WATER, 5, [40, 5, 30, 10, 9, 10], &[TACKLE]);
    let foe = mon(NORMAL, 5, [40, 5, 30, 5, 5, 5], &[TACKLE]);
    let mut r = Run::wild(&[me], foe, 1);
    assert_eq!(first_mover(&r.choose(Choice::Move(0))), 'P');
    me.status = Status::Paralysis;
    let mut r = Run::wild(&[me], foe, 1);
    assert_eq!(first_mover(&r.choose(Choice::Move(0))), 'O');
}

#[test]
fn turn_draws_rolls_in_gen3_order() {
    let foe = mon(NORMAL, 2, [13, 6, 6, 8, 5, 5], &[TACKLE; 4]);
    let mut checked = 0;
    for seed in 1..20 {
        let mut r = Run::wild(&[water(5)], foe, seed);
        let mut rng = r.rng;
        // Player (faster, no tie roll): accuracy, crit, damage roll, seteffectwithchance.
        let hit = calc::accuracy_hits(95, 0, 0, rng.random());
        let crit = calc::is_crit(0, rng.random());
        let dealt = calc::random_damage(if crit { 16 } else { 8 }, rng.random());
        rng.random();
        // Opponent: the same four (physical minimum 1, + 2, STAB).
        let foe_hit = calc::accuracy_hits(95, 0, 0, rng.random());
        let crit = calc::is_crit(0, rng.random());
        let taken = calc::random_damage(calc::stab(if crit { 6 } else { 3 }), rng.random());
        rng.random();
        if !hit || !foe_hit || dealt >= 13 {
            // A miss or a KO draws fewer rolls: only replay seeds where both hit.
            continue;
        }
        // Next turn: gRandomTurnNumber, then the AI's slot roll (every slot usable).
        rng.random();
        rng.random();
        let events = r.choose(Choice::Move(0));
        assert!(events.contains(&format!("Hp O 13->{}", 13 - dealt)));
        assert!(events.contains(&format!("Hp P 20->{}", 20 - taken)));
        assert_eq!(r.rng, rng, "seed {seed}");
        checked += 1;
    }
    assert!(checked > 10);
}

#[test]
fn setup_draws_come_first() {
    // CB2_InitBattleInternal: SetUpBattleVarsAndBirchZigzagoon's
    // BattleAI_HandleItemUseBeforeAISetup(0xF) (4 simulatedRNG rolls, every battle), then
    // SetWildMonHeldItem's Random() (wild battles only, the first battle included), then
    // BattleStartClearSetData's gRandomTurnNumber.
    let setup = |kind| BattleSetup {
        kind,
        opponents: party(&[normal(2)]),
        environment: Environment::Grass,
        double: None,
    };
    let draws = |kind| {
        let mut rng = Rng::new(3);
        let b = Battle::new(&Game, setup(kind), &party(&[water(5)]), &mut rng);
        let mut reference = Rng::new(3);
        (0..8)
            .position(|_| {
                reference.random();
                reference == rng
            })
            .map(|n| (n + 1, b.random_turn))
    };
    let nth = |n: usize| {
        let mut r = Rng::new(3);
        (0..n - 1).for_each(|_| {
            r.random();
        });
        r.random()
    };
    assert_eq!(
        draws(BattleKind::Wild {
            first_battle: false
        }),
        Some((6, nth(6)))
    );
    assert_eq!(
        draws(BattleKind::Wild { first_battle: true }),
        Some((6, nth(6)))
    );
    assert_eq!(draws(trainer()), Some((5, nth(5))));
    // The test seeds name the state after the four setup draws.
    let mut r = Rng::new(before_setup_draws(77));
    for _ in 0..4 {
        r.random();
    }
    assert_eq!(r, Rng::new(77));
}

#[test]
fn same_seed_same_battle() {
    let play = |seed| {
        let mut r = Run::wild(&[water(5)], normal(3), seed);
        r.choose(Choice::Move(1));
        r.choose(Choice::Move(0));
        (lines(&r.out), hash(&r.b), r.rng)
    };
    assert_eq!(play(5), play(5));
    assert_ne!(play(5).1, play(6).1);
    // The hash covers the core state, not just the parties.
    let mut a = Run::wild(&[water(5)], normal(3), 5);
    let before = hash(&a.b);
    a.choose(Choice::Move(1));
    assert_ne!(hash(&a.b), before);
}

#[test]
fn need_is_busy_before_start() {
    let mut rng = Rng::new(1);
    let setup = BattleSetup {
        kind: BattleKind::Wild {
            first_battle: false,
        },
        opponents: party(&[normal(2)]),
        environment: Environment::Grass,
        double: None,
    };
    let b = Battle::new(&Game, setup, &party(&[water(5)]), &mut rng);
    assert_eq!(b.need(), Need::Busy);
    assert_eq!(b.mon(Side::Player).species, WATER);
    assert_eq!(b.party(Side::Opponent).len(), 1);
}

#[test]
fn a_double_faint_whites_out() {
    // Take Down KOs the 1-HP foe and the recoil KOs the 1-HP user.
    let mut me = mon(WATER, 5, [20, 13, 10, 9, 9, 10], &[TAKE_DOWN]);
    me.hp = 1;
    let mut foe = normal(2);
    foe.hp = 1;
    let mut r = Run::wild(&[me], foe, 1);
    let events = r.choose(Choice::Move(0));
    assert_eq!(
        events[events.len() - 3..],
        ["PlayerWhiteout@P ~64", "PlayerWhiteout2@P !", "End(Drew)"]
    );
    assert!(events.contains(&"Faint P".into()) && events.contains(&"Faint O".into()));
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::Drew));
}

#[test]
fn a_faint_message_names_the_fainted_mon_after_its_replacement() {
    let ace = mon(FIRE, 10, [30, 40, 20, 20, 20, 20], &[MEGA_HIT]);
    let second = mon(DARK, 3, [15, 6, 6, 8, 5, 5], &[TACKLE]);
    let mut r = Run::new(trainer(), &[ace], &[normal(2), second], 1);
    let mut out = Vec::new();
    r.b.choose(&Game, &mut r.rng, Choice::Move(0), &mut out);
    // The replacement is out by the time the batch is expanded.
    assert_eq!(r.b.mon(Side::Opponent).species, DARK);
    let fainted = out
        .iter()
        .find_map(|e| match e {
            BattleEvent::Msg(m) if m.id == Str::PkmnFainted => Some(*m),
            _ => None,
        })
        .unwrap();
    assert_eq!(fainted.slots, [0, 0, 0, 0]);
    let sent = out
        .iter()
        .find_map(|e| match e {
            BattleEvent::Msg(m) if m.id == Str::PkmnSentOut => Some(*m),
            _ => None,
        })
        .unwrap();
    assert_eq!(sent.slots, [0, 1, 0, 0]);
    let names = Names {
        player: &[0xFF],
        trainer_class: &[0xFF],
        trainer_name: &[0xFF],
        trainer2_class: &[0xFF],
        trainer2_name: &[0xFF],
        wild: false,
        mons: Side::ALL.map(|s| r.b.mon(s)),
        parties: [r.b.party(Side::Player), r.b.party(Side::Opponent)],
    };
    // NORMAL's nickname (0xBB + 2), not DARK's (0xBB + 5).
    assert_eq!(text::expand(&Game, &fainted, &names), [0xBD, 0xFF]);
}

#[test]
fn balls_are_refused_in_trainer_battles() {
    let mut r = Run::new(trainer(), &[water(5)], &[normal(2)], 1);
    assert!(r.choose(Choice::Ball).is_empty());
    assert_eq!(r.b.need(), Need::Action);
    // Wild: the ball action takes the turn (its effect is E2's stub).
    let mut r = Run::wild(&[water(5)], normal(2), 1);
    assert!(!r.choose(Choice::Ball).is_empty());
}

#[test]
fn move_damage_is_stored_for_bide() {
    let mut m = water(5);
    m.pp = [0; 4];
    let mut r = Run::wild(&[m], mon(NORMAL, 2, [13, 6, 6, 8, 5, 5], &[SPLASH]), 7);
    let events = r.choose(Choice::Move(1));
    // Struggle: 8 to the foe, a quarter of it back as recoil.
    assert!(events.contains(&"Hp O 13->5".into()) && events.contains(&"Hp P 20->18".into()));
    assert_eq!(r.b.battler(Side::Opponent).bide_damage, 8);
    assert_eq!(r.b.battler(Side::Player).bide_damage, 2);
}
