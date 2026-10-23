//! Task F: the battle and menu policy and the battle records, driven by hand on a `Session`
//! (no Task A code): the parity route's keys up to a moment, then the policy's intents under
//! the tap rule.

use content::Emerald;
use engine::GameData;
use engine::battle::scene::OpenMenu;
use engine::battle::{BattleMon, BattleOutcome, Side};
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
    /// The last [`Driver::fight`]: the player battler's lowest HP, and the frames a
    /// learn-move prompt (`LearnAsk`/`LearnPick`/`StopAsk`) was open.
    min_hp: u16,
    learn_prompts: u32,
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
        Driver {
            s,
            last,
            min_hp: 0,
            learn_prompts: 0,
        }
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
        self.min_hp = self.w().battle().unwrap().1.mon(Side::Player).hp;
        self.learn_prompts = 0;
        while self.w().in_battle() {
            assert!(
                self.w().frame() - start <= BATTLE_FRAMES,
                "the battle never ended"
            );
            let k = p.battle(self.w(), t, opts);
            self.tap(k);
            tracker.observe(self.w());
            if let Some((_, battle, scene)) = self.s.world().battle() {
                self.min_hp = self.min_hp.min(battle.mon(Side::Player).hp);
                if matches!(
                    scene.open_menu(),
                    Some(OpenMenu::LearnAsk(_) | OpenMenu::LearnPick(_) | OpenMenu::StopAsk(_))
                ) {
                    self.learn_prompts += 1;
                }
            }
        }
        tracker.finish(self.w(), None)
    }
}

/// The frames the route's battles start at (`in_battle` turns true), in order.
#[cfg(not(pkr_silice))]
fn battle_starts(rec: &Recording) -> Vec<u32> {
    battles(rec).into_iter().map(|(f, _)| f).collect()
}

/// [`battle_starts`] with the party lead's level at each start.
fn battles(rec: &Recording) -> Vec<(u32, u8)> {
    let mut s = Session::new(Emerald::for_shells());
    let mut out = Vec::new();
    for f in 0..rec.frames() {
        let was = s.world().in_battle();
        s.step(rec.keys_at(f));
        if !was && s.world().in_battle() {
            out.push((f + 1, s.world().party().get(0).unwrap().level));
        }
    }
    out
}

fn outcome(b: &BattleSummary) -> Option<BattleOutcome> {
    b.outcome.and_then(|o| BattleOutcome::from_u16(o.into()))
}

#[cfg(not(pkr_silice))]
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
#[cfg(not(pkr_silice))]
#[test]
fn the_policy_wins_the_route_battles_and_runs_from_wild_ones() {
    let rec = cached_route();
    let starts = battle_starts(rec);
    let mut kinds = Vec::new();
    for &start in &starts {
        for run_wild in [false, true] {
            let opts = Opts {
                run_wild,
                ..Opts::default()
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

/// The battle records of the route's battles, by hand (kind, levels, frames, HP), in order up
/// to the first one in which the player's mon loses HP, so that the HP record is checked on a
/// non-zero loss whichever battle the RNG makes it (stub: the grass battle; extracted: Birch's
/// rescue, then the wild battles).
#[cfg(not(pkr_silice))]
#[test]
fn the_tracker_records_a_battle() {
    let rec = cached_route();
    let mut kinds = Vec::new();
    let mut damaged = false;
    for start in battle_starts(rec) {
        let mut d = Driver::at(rec, start);
        let w = d.w();
        let (_, battle, _) = w.battle().unwrap();
        let opp = battle.mon(Side::Opponent).level;
        let player = battle.mon(Side::Player);
        let (hp, max_hp) = (player.hp, player.max_hp);
        let lead = w.party().get(0).unwrap().level;
        assert!(BattleTracker::start(w).is_some());
        let b = d.fight(Opts::default());
        check(&b, Opts::default());
        assert_eq!(b.opponent_level, opp);
        assert_eq!(b.player_level, lead);
        // The deepest HP loss of the battle, against the starting max HP.
        let want = u32::from(hp - d.min_hp) * 1000 / u32::from(max_hp);
        assert_eq!(u32::from(b.hp_lost_permille), want, "{b:?}");
        // Not in a battle: no tracker.
        assert!(BattleTracker::start(d.w()).is_none());
        kinds.push(b.kind);
        if b.hp_lost_permille > 0 {
            damaged = true;
            break;
        }
    }
    assert!(damaged, "no battle of the route loses HP: {kinds:?}");
    // The route's first battle: the stub's grass battle, or Birch's rescue.
    let want = if content::STUB { 0 } else { 1 };
    assert_eq!(kinds.first(), Some(&want), "{kinds:?}");
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
    let slice = content::samples::MOVE_IDS;
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
    // No damaging move with PP: the first move with PP (never a 0-PP slot, which the engine
    // refuses forever while another move has PP).
    let m = mon([Some(status), Some(a), None, None], [5, 0, 0, 0]);
    assert_eq!(first_damaging_slot(&m, t), 0);
    let m = mon([Some(a), Some(status), None, None], [0, 5, 0, 0]);
    assert_eq!(first_damaging_slot(&m, t), 1);
    // No PP anywhere: slot 0 (the engine uses Struggle).
    let m = mon([Some(a), Some(status), None, None], [0, 0, 0, 0]);
    assert_eq!(first_damaging_slot(&m, t), 0);
    assert_eq!(first_damaging_slot(&mon([None; 4], [0; 4]), t), 0);
}

/// The route's `starter_screen` moment, then `Policy::script` picks ball `choice % 3`.
#[cfg(not(pkr_silice))]
#[test]
fn the_starter_screen_takes_the_chosen_ball() {
    if content::STUB {
        // The stub's new game starts with a Pokémon: no starter screen.
        eprintln!("skipped: stub has no starter screen");
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
            choice,
            ..Opts::default()
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

/// A lead with a full moveset levels up into a new move: the policy keeps the known moves
/// (B on `LearnAsk`, yes on `StopAsk`). From the route's `after_wild` node with the lead's empty
/// slots filled, the route's keys to its level-up battle, then the policy.
#[test]
fn a_full_moveset_keeps_its_moves_on_level_up() {
    // Emerald's route and its `after_wild` node (the stub skips below); Silice's boot route has
    // no such node, and does not record on master yet (its intro script is being fixed in
    // game-native).
    if content::FLAVOR == content::Flavor::Silice {
        return;
    }
    let rec = cached_route();
    let Some(&(_, node)) = rec.nodes.iter().find(|(n, _)| *n == "after_wild") else {
        eprintln!("skipped: stub has no after_wild node");
        return;
    };
    let starts = battles(rec);
    // The first battle after the node whose next battle starts a level higher.
    let Some(&(start, level)) = starts
        .windows(2)
        .find(|w| w[0].0 > node && w[1].1 > w[0].1)
        .map(|w| &w[0])
    else {
        eprintln!("skipped: no level-up battle after the node");
        return;
    };
    let game = Emerald::for_shells();
    let t: &BattleTables = game.battle();
    let mut cp = Driver::at(rec, node)
        .s
        .checkpoint()
        .expect("the node is capturable");
    let lead = &mut cp.world.party[0];
    let mut fill = content::samples::MOVE_IDS
        .iter()
        .copied()
        .filter(|&m| t.move_info(m).power > 0 && t.move_info(m).pp > 0);
    for i in 0..4 {
        if lead.moves[i].is_none() {
            let m = fill
                .find(|m| !lead.moves.contains(&Some(*m)))
                .expect("a move to fill");
            lead.moves[i] = Some(m);
            lead.pp[i] = t.move_info(m).pp;
        }
    }
    let moves = lead.moves;
    // Restore checks the capture hash on the same build: recompute it for the edited party.
    let mut other = cp.clone();
    other.header.build.engine ^= 1;
    cp.header.world_hash = World::restore(&game, &other).unwrap().hash();
    let mut d = Driver {
        s: Session::restore(game, &cp).expect("the edited node restores"),
        last: rec.keys_at(node - 1),
        min_hp: 0,
        learn_prompts: 0,
    };
    for f in node..start {
        d.s.step(rec.keys_at(f));
    }
    d.last = rec.keys_at(start - 1);
    assert!(d.w().in_battle(), "the route's battle starts");
    assert_eq!(d.w().party().get(0).unwrap().level, level);
    let b = d.fight(Opts::default());
    assert_eq!(outcome(&b), Some(BattleOutcome::Won), "{b:?}");
    let lead = d.w().party().get(0).unwrap();
    assert!(lead.level > level, "no level-up");
    assert!(d.learn_prompts > 0, "no learn-move prompt");
    assert_eq!(lead.moves, moves, "the known moves are kept");
}
