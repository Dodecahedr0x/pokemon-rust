use super::*;
use alloc::vec::Vec;

/// Answers flag/var requests instantly; `Delay(n)` takes n frames.
#[derive(Default)]
struct Host {
    flags: [bool; 8],
    vars: [u16; 8],
    log: Vec<Request>,
    frames_left: u16,
}

impl ScriptHost for Host {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        match *r {
            Request::GetFlag(f) => Some(Response::Bool(self.flags[f.0 as usize])),
            Request::SetFlag(f, on) => {
                self.flags[f.0 as usize] = on;
                Some(Response::Unit)
            }
            Request::GetVar(v) => Some(Response::Value(self.vars[v.0 as usize])),
            Request::SetVar(v, x) => {
                self.vars[v.0 as usize] = x;
                Some(Response::Unit)
            }
            Request::Delay(n) => {
                self.frames_left = n;
                None
            }
            _ => Some(Response::Unit),
        }
    }

    fn resume(&mut self, r: &Request) -> Option<Response> {
        assert!(matches!(r, Request::Delay(_)));
        self.frames_left -= 1;
        (self.frames_left == 0).then_some(Response::Unit)
    }
}

fn request_hash(r: &Request) -> u64 {
    let mut h = Fnv64::new();
    r.hash_into(&mut h);
    h.finish()
}

#[test]
fn steps_digest_is_pinned() {
    // Bytes: Walk(Left, Fast) = [1, 2, 2], Jump(Up, 2) = [3, 1, 2], Emote(Heart) = [11, 2].
    // FNV-1a-32 (basis 0x811c9dc5, prime 0x01000193) over [1, 2, 2, 3, 1, 2, 11, 2]:
    // 0xb82730cf.
    static T: [Step; 3] = [
        Step::Walk(Dir::Left, Speed::Fast),
        Step::Jump(Dir::Up, 2),
        Step::Emote(Emote::Heart),
    ];
    assert_eq!(steps_digest(&T), 0xb827_30cf);
    assert_eq!(steps_digest(&[]), 0x811c_9dc5);
    const C: u32 = steps_digest(&T);
    assert_eq!(C, 0xb827_30cf);
}

#[test]
fn steps_digest_tells_every_argument_apart() {
    let all = [
        Step::Face(Dir::Down),
        Step::Face(Dir::Up),
        Step::Walk(Dir::Down, Speed::Slow),
        Step::Walk(Dir::Down, Speed::Faster),
        Step::WalkInPlace(Dir::Down, Speed::Slow),
        Step::Jump(Dir::Down, 0),
        Step::Jump(Dir::Down, 1),
        Step::Delay(0),
        Step::Delay(1),
        Step::FacePlayer,
        Step::FaceAwayFromPlayer,
        Step::LockFacing,
        Step::UnlockFacing,
        Step::Hide,
        Step::Show,
        Step::Emote(Emote::Exclamation),
        Step::Emote(Emote::Question),
    ];
    let digests: Vec<u32> = all.iter().map(|s| steps_digest(&[*s])).collect();
    for (i, a) in digests.iter().enumerate() {
        for b in &digests[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

/// Regression: `Request::Move` hashed only the table length, so two different movement
/// scripts of the same length collided.
#[test]
fn move_requests_hash_the_step_contents() {
    static A: [Step; 2] = [Step::Walk(Dir::Up, Speed::Normal), Step::Face(Dir::Left)];
    static B: [Step; 2] = [Step::Walk(Dir::Down, Speed::Normal), Step::Face(Dir::Left)];
    static A2: [Step; 2] = [Step::Walk(Dir::Up, Speed::Normal), Step::Face(Dir::Left)];
    let mv = |steps: &'static [Step]| request_hash(&Request::Move { obj: 1, steps });
    assert_ne!(mv(&A), mv(&B));
    assert_eq!(mv(&A), mv(&A2));
}

async fn sample(ctx: Ctx) {
    if !ctx.flag(FlagId(1)).await {
        ctx.set_var(VarId(2), 7).await;
    }
    ctx.delay(3).await;
    ctx.set_flag(FlagId(1), true).await;
}

#[test]
fn instant_requests_run_within_one_frame_and_delays_span_frames() {
    let mut r = Runner::new();
    let mut h = Host::default();
    r.start(Box::pin(sample(r.ctx())));
    assert!(r.step(&mut h)); // GetFlag, SetVar, then Delay(3) begins
    assert_eq!(h.vars[2], 7);
    assert_eq!(r.waiting_on(), Some(&Request::Delay(3)));
    assert!(r.step(&mut h)); // 2 left
    assert!(r.step(&mut h)); // 1 left
    assert!(!h.flags[1]);
    assert!(!r.step(&mut h)); // delay done, SetFlag, script ends
    assert!(h.flags[1]);
    assert!(!r.is_running());
    assert_eq!(h.log.len(), 4);
}

async fn busy(ctx: Ctx) {
    loop {
        ctx.flag(FlagId(0)).await;
    }
}

#[test]
fn a_script_that_never_waits_still_yields_each_frame() {
    let mut r = Runner::new();
    let mut h = Host::default();
    r.start(Box::pin(busy(r.ctx())));
    assert!(r.step(&mut h));
    assert_eq!(h.log.len(), INSTANT_LIMIT as usize);
    assert!(r.step(&mut h));
}

#[test]
fn abort_drops_the_script_and_its_pending_request() {
    let mut r = Runner::new();
    let mut h = Host::default();
    r.start(Box::pin(sample(r.ctx())));
    assert!(r.step(&mut h));
    assert_eq!(r.waiting_on(), Some(&Request::Delay(3)));
    r.abort();
    assert!(!r.is_running());
    assert_eq!(r.waiting_on(), None);
    assert!(!r.step(&mut h));
    assert!(!h.flags[1], "the rest of the script never ran");
    r.start(Box::pin(sample(r.ctx())));
    assert!(r.step(&mut h));
}

async fn battles(ctx: Ctx) {
    let i = ctx
        .choose_starter([SpeciesId(1), SpeciesId(2), SpeciesId(3)])
        .await;
    ctx.set_var(VarId(0), i as u16).await;
    let o = ctx.battle_wild(SpeciesId(4), 2, true).await;
    ctx.set_var(VarId(1), o as u16).await;
    let o = ctx.battle_trainer(TrainerId(5), TextId(6)).await;
    ctx.set_var(VarId(2), o as u16).await;
    let n = ctx.give_mon(SpeciesId(1), 5).await;
    ctx.set_var(VarId(3), n).await;
    ctx.set_var(VarId(4), ctx.party_size().await).await;
    ctx.heal_party().await;
    ctx.set_respawn(7).await;
}

/// Answers the M4 requests with fixed values.
struct BattleHost(Vec<Request>, [u16; 8]);

impl ScriptHost for BattleHost {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.0.push(r.clone());
        Some(match *r {
            Request::ChooseStarter(_) => Response::Value(2),
            Request::BattleWild { .. } => Response::Value(BattleOutcome::MonFled as u16),
            Request::BattleTrainer { .. } => Response::Value(BattleOutcome::Won as u16),
            Request::GiveMon { .. } => Response::Value(0),
            Request::PartySize => Response::Value(1),
            Request::SetVar(v, x) => {
                self.1[v.0 as usize] = x;
                Response::Unit
            }
            _ => Response::Unit,
        })
    }

    fn resume(&mut self, _: &Request) -> Option<Response> {
        unreachable!()
    }
}

#[test]
fn battle_requests_round_trip_through_ctx() {
    let mut r = Runner::new();
    let mut h = BattleHost(Vec::new(), [0; 8]);
    r.start(Box::pin(battles(r.ctx())));
    assert!(!r.step(&mut h));
    assert_eq!(h.1[..5], [2, 6, 1, 0, 1]);
    assert!(h.0.contains(&Request::SetRespawn(7)));
    assert!(h.0.contains(&Request::HealParty));
}

#[test]
fn battle_request_hashes_differ() {
    let hash = |r: Request| {
        let mut h = Fnv64::new();
        r.hash_into(&mut h);
        h.finish()
    };
    let all = [
        Request::GiveMon {
            species: SpeciesId(1),
            level: 5,
            extra: None,
        },
        Request::PartySize,
        Request::HealParty,
        Request::BattleWild {
            species: SpeciesId(1),
            level: 5,
            first_battle: false,
            rules: None,
        },
        Request::BattleWild {
            species: SpeciesId(1),
            level: 5,
            first_battle: true,
            rules: None,
        },
        Request::BattleTrainer {
            trainer: TrainerId(1),
            defeat: TextId(5),
            no_whiteout: false,
            rules: None,
        },
        Request::BattleTrainer {
            trainer: TrainerId(1),
            defeat: TextId(5),
            no_whiteout: true,
            rules: None,
        },
        Request::ChooseStarter([SpeciesId(1), SpeciesId(5), SpeciesId(0)]),
        Request::SetRespawn(1),
    ];
    let hashes: Vec<u64> = all.iter().cloned().map(hash).collect();
    for (i, a) in hashes.iter().enumerate() {
        for b in &hashes[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn a_finished_runner_accepts_a_new_script() {
    let mut r = Runner::new();
    let mut h = Host::default();
    h.flags[1] = true;
    r.start(Box::pin(async {}));
    assert!(!r.step(&mut h));
    r.start(Box::pin(sample(r.ctx())));
    assert!(r.step(&mut h));
    assert_eq!(h.vars[2], 0, "flag was set, var untouched");
}
