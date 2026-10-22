//! Task F: the battle and menu policy and the battle records, driven by hand on a `Session`
//! (no Task A code): the parity route's keys up to a moment, then the policy's intents under
//! the tap rule.

use content::Emerald;
use engine::GameData;
use engine::battle::{BattleMon, BattleOutcome};
use engine::data::{MoveId, SpeciesId};
use engine::mon::{BattleTables, Gender, NAME_LEN, Status};
use engine::{Buttons, World};
use explore::balance::BattleTracker;
use explore::model::{BattleSummary, Opts};
use explore::policy::{Policy, first_damaging_slot};
use game_native::Session;
use game_native::scenario::{Recording, cached_route};

/// `Limits::battle_frames`' default.
const BATTLE_FRAMES: u32 = 12_000;

/// A hand driver with the tap rule: a release frame between presses, nothing while a battle
/// or a warp has the screen black.
struct Driver {
    s: Session<Emerald>,
    last: Buttons,
}

impl Driver {
    /// A new game with the route's keys replayed up to `frame`.
    fn at(rec: &Recording, frame: u32) -> Driver {
        let mut s = Session::new(Emerald::for_shells());
        for f in 0..frame {
            s.step(rec.keys_at(f));
        }
        let last = if frame == 0 {
            Buttons::NONE
        } else {
            rec.keys_at(frame - 1)
        };
        Driver { s, last }
    }

    fn w(&self) -> &World {
        self.s.world()
    }

    fn tap(&mut self, k: Buttons) {
        let w = self.s.world();
        let black = (w.in_battle() || w.transition_active()) && self.s.frame().is_black();
        let k = if self.last != Buttons::NONE || black {
            Buttons::NONE
        } else {
            k
        };
        self.s.step(k);
        self.last = k;
    }

    /// Lets the policy fight the running battle to the field's return.
    fn fight(&mut self, opts: Opts) -> BattleSummary {
        let t: &BattleTables = self.s.game().battle();
        let mut p = Policy::new();
        let mut tracker = BattleTracker::start(self.w()).expect("in a battle");
        let start = self.w().frame();
        while self.w().in_battle() {
            assert!(
                self.w().frame() - start <= BATTLE_FRAMES,
                "the battle never ended"
            );
            let k = p.battle(self.w(), t, opts);
            self.tap(k);
            tracker.observe(self.w());
        }
        tracker.finish(self.w(), None)
    }
}

/// The frames the route's battles start at (`in_battle` turns true), in order.
fn battle_starts(rec: &Recording) -> Vec<u32> {
    let mut s = Session::new(Emerald::for_shells());
    let mut out = Vec::new();
    for f in 0..rec.frames() {
        let was = s.world().in_battle();
        s.step(rec.keys_at(f));
        if !was && s.world().in_battle() {
            out.push(f + 1);
        }
    }
    out
}

fn outcome(b: &BattleSummary) -> Option<BattleOutcome> {
    b.outcome.and_then(|o| BattleOutcome::from_u16(o.into()))
}

fn check(b: &BattleSummary, opts: Opts) {
    assert!(b.frames > 0 && b.frames <= BATTLE_FRAMES, "{b:?}");
    assert!(b.player_level > 0 && b.opponent_level > 0, "{b:?}");
    assert!(b.hp_lost_permille <= 1000, "{b:?}");
    let o = outcome(b);
    match b.kind {
        0 => {
            assert_eq!(b.trainer, None);
            let want = if opts.run_wild {
                BattleOutcome::Ran
            } else {
                BattleOutcome::Won
            };
            assert_eq!(o, Some(want), "{b:?} {opts:?}");
        }
        // Birch's rescue (Gen3 `BATTLE_TYPE_FIRST_BATTLE`, which accepts the foe fleeing): no
        // running.
        1 => assert!(
            matches!(o, Some(BattleOutcome::Won | BattleOutcome::MonFled)),
            "{b:?} {opts:?}"
        ),
        2 => {
            assert!(b.trainer.is_some(), "{b:?}");
            assert_eq!(o, Some(BattleOutcome::Won), "{b:?} {opts:?}");
        }
        k => panic!("battle kind {k}"),
    }
}

/// Every battle of the route (stub: the grass battle; extracted: Birch's rescue, the wild
/// battles and May), fought by the policy from its first frame, with and without `run_wild`.
#[test]
fn the_policy_wins_the_route_battles_and_runs_from_wild_ones() {
    let rec = cached_route();
    let starts = battle_starts(rec);
    let mut kinds = Vec::new();
    for &start in &starts {
        for run_wild in [false, true] {
            let opts = Opts {
                run_wild,
                choice: 0,
            };
            let mut d = Driver::at(rec, start);
            let b = d.fight(opts);
            println!("battle at {start} {opts:?}: {b:?}");
            check(&b, opts);
            assert!(!d.w().in_battle());
            if !run_wild {
                kinds.push(b.kind);
            }
        }
    }
    if content::STUB {
        assert_eq!(kinds, [0]);
    } else {
        // Birch's rescue first, May (a trainer) last, wild battles between.
        assert!(kinds.len() >= 5, "{kinds:?}");
        assert_eq!(kinds[0], 1);
        assert_eq!(*kinds.last().unwrap(), 2);
        assert!(
            kinds[1..kinds.len() - 1].iter().all(|&k| k == 0),
            "{kinds:?}"
        );
    }
}

/// The battle record of the stub's grass battle, by hand: kind, levels, frames, HP.
#[test]
fn the_tracker_records_a_battle() {
    let rec = cached_route();
    let start = battle_starts(rec)[0];
    let mut d = Driver::at(rec, start);
    let w = d.w();
    let (_, battle, _) = w.battle().unwrap();
    let opp = battle.mon(engine::battle::Side::Opponent).level;
    let lead = w.party().get(0).unwrap().level;
    assert!(BattleTracker::start(w).is_some());
    let b = d.fight(Opts::default());
    assert_eq!(b.opponent_level, opp);
    assert_eq!(b.player_level, lead);
    let want = if content::STUB { 0 } else { 1 };
    assert_eq!(b.kind, want);
    // Not in a battle: no tracker.
    assert!(BattleTracker::start(d.w()).is_none());
}

fn mon(moves: [Option<MoveId>; 4], pp: [u8; 4]) -> BattleMon {
    BattleMon {
        party: 0,
        species: SpeciesId(1),
        nickname: [0xFF; NAME_LEN],
        level: 5,
        hp: 20,
        max_hp: 20,
        status: Status::None,
        gender: Gender::Male,
        moves,
        pp,
        max_pp: [35; 4],
    }
}

#[test]
fn first_damaging_slot_skips_status_and_empty_moves() {
    let t = &content::battle::TABLES;
    let power = |m: MoveId| t.move_info(m).power;
    let slice = &content::battle::SLICE_MOVES;
    // A 0-power move by scanning the slice (then the whole table), never by name.
    let status = slice
        .iter()
        .copied()
        .find(|&m| power(m) == 0)
        .or_else(|| {
            (1..t.moves.len())
                .map(|i| MoveId(i as u16))
                .find(|&m| power(m) == 0)
        })
        .expect("a 0-power move");
    let mut damaging = slice.iter().copied().filter(|&m| power(m) > 0);
    let (a, b) = (damaging.next().unwrap(), damaging.next().unwrap());

    // Slot 0 is a status move, slot 1 has no PP left: slot 2.
    let m = mon([Some(status), Some(a), Some(b), None], [10, 0, 5, 0]);
    assert_eq!(first_damaging_slot(&m, t), 2);
    // First damaging slot with PP.
    let m = mon([Some(a), Some(b), None, None], [1, 1, 0, 0]);
    assert_eq!(first_damaging_slot(&m, t), 0);
    // Empty slot skipped.
    let m = mon([Some(status), None, None, Some(a)], [5, 0, 0, 3]);
    assert_eq!(first_damaging_slot(&m, t), 3);
    // Nothing usable: slot 0.
    let m = mon([Some(status), Some(a), None, None], [5, 0, 0, 0]);
    assert_eq!(first_damaging_slot(&m, t), 0);
    assert_eq!(first_damaging_slot(&mon([None; 4], [0; 4]), t), 0);
}

/// The route's `starter_screen` moment, then `Policy::script` picks ball `choice % 3`.
#[test]
fn the_starter_screen_takes_the_chosen_ball() {
    if content::STUB {
        // The stub's new game starts with a Pokémon: no starter screen.
        return;
    }
    let rec = cached_route();
    let at = rec
        .moments
        .iter()
        .find(|m| m.name == "starter_screen")
        .expect("the route's starter_screen moment")
        .frame;
    for choice in [2u8, 0, 4] {
        let mut d = Driver::at(rec, at);
        assert!(d.w().starter().is_some());
        let opts = Opts {
            run_wild: false,
            choice,
        };
        let mut p = Policy::new();
        let mut chosen = None;
        let mut n = 0;
        while let Some(sel) = d.w().starter().map(|s| s.selection()) {
            n += 1;
            assert!(n < 2400, "the starter was never chosen");
            chosen = Some(sel);
            let k = p.script(d.w(), opts);
            d.tap(k);
        }
        assert_eq!(chosen, Some(choice % 3), "choice {choice}");
        // The first battle follows, with the chosen species out.
        while !d.w().in_battle() {
            n += 1;
            assert!(n < 2400, "no first battle");
            d.tap(Buttons::NONE);
        }
        let (_, battle, _) = d.w().battle().unwrap();
        assert_eq!(
            battle.mon(engine::battle::Side::Player).species,
            content::battle::STARTERS[(choice % 3) as usize],
            "choice {choice}"
        );
    }
}
