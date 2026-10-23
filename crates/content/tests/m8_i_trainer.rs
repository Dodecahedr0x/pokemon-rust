//! Milestone 8 Task I: `trainer_battle`'s path per `trainerbattle` type, against a mock host,
//! and the type numbers against the decomp's `battle_setup.h` (when `PKR_DECOMP` is set).

use std::cell::Cell;
use std::rc::Rc;

use content::prelude::{TrainerBattle, kind, trainer_battle, trainer_flag};
use engine::battle::BattleOutcome;
use engine::data::{ScriptId, TextId, TrainerId};
use engine::script::{Report, Request, Response, Runner, ScriptHost};

const INTRO: TextId = TextId(1);
const LOSE: TextId = TextId(2);
const NOT_ENOUGH: TextId = TextId(3);
const TRAINER: u16 = 7;

/// Answers everything at once: `TrainerApproach` with `approach`, the trainer flag with
/// `beaten`, the battle with `outcome`, the rest `Unit`. Logs every request.
struct Host {
    approach: Response,
    beaten: bool,
    outcome: BattleOutcome,
    log: Vec<Request>,
}

impl ScriptHost for Host {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        Some(match *r {
            Request::TrainerApproach { .. } => self.approach,
            Request::GetFlag(_) => Response::Bool(self.beaten),
            Request::BattleTrainer { .. } => Response::Value(self.outcome as u16),
            _ => Response::Unit,
        })
    }
    fn resume(&mut self, _: &Request) -> Option<Response> {
        unreachable!("every request is answered at once")
    }
}

fn battle(kind: u8) -> TrainerBattle {
    TrainerBattle {
        kind,
        trainer: TRAINER,
        local_id: 4,
        intro: Some(INTRO),
        lose: LOSE,
        not_enough: Some(NOT_ENOUGH),
        event_script: None,
        music: true,
    }
}

/// Runs `trainer_battle(b)`; returns its result and the requests issued.
fn run(b: TrainerBattle, approach: Response, beaten: bool) -> (bool, Vec<Request>) {
    run_with(b, approach, beaten, BattleOutcome::Won)
}

fn run_with(
    b: TrainerBattle,
    approach: Response,
    beaten: bool,
    outcome: BattleOutcome,
) -> (bool, Vec<Request>) {
    let mut host = Host {
        approach,
        beaten,
        outcome,
        log: Vec::new(),
    };
    let mut runner = Runner::new();
    let ctx = runner.ctx();
    let out = Rc::new(Cell::new(None));
    let o = out.clone();
    runner.start(Box::pin(async move {
        o.set(Some(trainer_battle(&ctx, b).await))
    }));
    for _ in 0..100 {
        if !runner.step(&mut host) {
            break;
        }
    }
    (out.get().expect("trainer_battle ended"), host.log)
}

const TALKED_ONE: Response = Response::Value(1);
const TALKED_TWO: Response = Response::Value(2);

fn approach() -> Request {
    Request::TrainerApproach { obj: 4 }
}
fn flag() -> Request {
    Request::GetFlag(trainer_flag(TrainerId(TRAINER)))
}
fn set_flag() -> Request {
    Request::SetFlag(trainer_flag(TrainerId(TRAINER)), true)
}
fn fight() -> Request {
    Request::BattleTrainer {
        trainer: TrainerId(TRAINER),
        defeat: LOSE,
        no_whiteout: false,
    }
}
fn intro() -> [Request; 3] {
    [
        Request::Message(INTRO),
        Request::WaitMessage,
        Request::WaitButton,
    ]
}

/// The regular battle path after the checks: intro, battle, flag.
fn battled() -> Vec<Request> {
    let mut v = intro().to_vec();
    v.extend([fight(), set_flag()]);
    v
}

#[test]
fn single_talked_locks_faces_checks_the_flag_and_battles() {
    let (cont, log) = run(battle(kind::SINGLE), TALKED_ONE, false);
    let mut want = vec![approach(), Request::Lock, Request::FacePlayer, flag()];
    want.extend(battled());
    want.push(Request::ReleaseAll);
    assert_eq!((cont, log), (false, want));
}

#[test]
fn a_beaten_trainer_goes_on_with_the_post_battle_script() {
    let (cont, log) = run(battle(kind::SINGLE), TALKED_ONE, true);
    assert_eq!(
        (cont, log),
        (
            true,
            vec![approach(), Request::Lock, Request::FacePlayer, flag()]
        )
    );
}

#[test]
fn a_sight_approach_goes_straight_to_the_intro() {
    let (cont, log) = run(battle(kind::SINGLE), Response::Unit, false);
    let mut want = vec![approach()];
    want.extend(battled());
    want.push(Request::ReleaseAll);
    assert_eq!((cont, log), (false, want));
}

#[test]
fn a_lost_battle_leaves_the_flag_unset() {
    for outcome in [BattleOutcome::Lost, BattleOutcome::Drew] {
        let (_, log) = run_with(battle(kind::SINGLE), TALKED_ONE, false, outcome);
        assert!(log.contains(&fight()));
        assert!(!log.contains(&set_flag()), "{outcome:?}");
    }
}

#[test]
fn no_intro_battles_without_checks_and_continues() {
    let (cont, log) = run(battle(kind::SINGLE_NO_INTRO_TEXT), TALKED_ONE, false);
    assert_eq!((cont, log), (true, vec![approach(), fight(), set_flag()]));
}

#[test]
fn doubles_need_two_usable_mons_then_run_as_singles() {
    for k in [
        kind::DOUBLE,
        kind::CONTINUE_SCRIPT_DOUBLE,
        kind::CONTINUE_SCRIPT_DOUBLE_NO_MUSIC,
    ] {
        let (cont, log) = run(battle(k), TALKED_ONE, false);
        let mut want = vec![approach(), Request::Lock, Request::FacePlayer, flag()];
        want.extend([
            Request::Message(NOT_ENOUGH),
            Request::WaitMessage,
            Request::WaitButton,
            Request::Release,
        ]);
        assert_eq!((cont, log), (false, want), "kind {k}");
        let (_, log) = run(battle(k), TALKED_TWO, false);
        // The engine reports the single battle when it creates it, not the script.
        assert!(
            !log.contains(&Request::Report(Report::Degraded("double battle"))),
            "kind {k}"
        );
        assert!(log.contains(&fight()), "kind {k}");
    }
}

#[test]
fn continue_kinds_without_their_script_report_and_release() {
    for k in [
        kind::CONTINUE_SCRIPT_NO_MUSIC,
        kind::CONTINUE_SCRIPT,
        kind::CONTINUE_SCRIPT_DOUBLE,
        kind::CONTINUE_SCRIPT_DOUBLE_NO_MUSIC,
    ] {
        let (cont, log) = run(battle(k), TALKED_TWO, false);
        let n = log.len();
        assert!(!cont, "kind {k}");
        assert_eq!(
            log[n - 2..],
            [
                Request::Report(Report::Degraded("trainerbattle continue script")),
                Request::ReleaseAll
            ],
            "kind {k}"
        );
    }
    // With a script, `gotobeatenscript` runs it instead of releasing (an id with no script).
    let mut b = battle(kind::CONTINUE_SCRIPT);
    b.event_script = Some(ScriptId(u16::MAX));
    let (cont, log) = run(b, TALKED_ONE, false);
    assert!(!cont);
    let after = &log[log.iter().position(|r| *r == set_flag()).unwrap() + 1..];
    assert!(!after.contains(&Request::ReleaseAll), "{after:?}");
}

#[test]
fn rematches_and_facility_and_set_trainer_kinds() {
    for k in [kind::REMATCH, kind::REMATCH_DOUBLE] {
        assert_eq!(run(battle(k), TALKED_ONE, false), (true, vec![approach()]));
    }
    for (k, cont) in [
        (kind::PYRAMID, true),
        (kind::HILL, true),
        (kind::SET_TRAINER_A, false),
        (kind::SET_TRAINER_B, false),
        (13, true),
    ] {
        let (c, log) = run(battle(k), TALKED_ONE, false);
        assert_eq!(c, cont, "kind {k}");
        assert!(
            matches!(log[..], [Request::Report(Report::Unsupported(_))]),
            "kind {k}: {log:?}"
        );
    }
}

/// Every `kind` constant is the decomp's `TRAINER_BATTLE_*` value, in order.
#[test]
fn kinds_match_the_decomp() {
    for (i, (k, _)) in kind::ALL.iter().enumerate() {
        assert_eq!(*k as usize, i);
    }
    let Ok(decomp) = std::env::var("PKR_DECOMP") else {
        eprintln!("PKR_DECOMP unset: the decomp check is skipped");
        return;
    };
    let path = std::path::Path::new(&decomp).join("include/constants/battle_setup.h");
    let header = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let defined: Vec<(String, u8)> = header
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            (w.next() == Some("#define")).then_some(())?;
            let name = w.next()?;
            let value = w.next()?.parse().ok()?;
            name.starts_with("TRAINER_BATTLE_")
                .then(|| (name.to_string(), value))
        })
        .collect();
    assert_eq!(defined.len(), kind::ALL.len(), "{defined:?}");
    for (name, value) in &defined {
        let ours = kind::ALL.iter().find(|(_, n)| n == name);
        assert_eq!(ours.map(|(k, _)| *k), Some(*value), "{name}");
    }
}
