//! Per-frame bug detectors (Task E).
//!
//! - **Round trip:** at the first capturable frame of every `roundtrip_every`-frame bucket of
//!   `World::frame`, the checkpoint must survive `encode`/`decode` unchanged and restore (on the
//!   inner game) to the same hash; a shadow session restored there then follows the live one
//!   for `follow` frames with the same keys and must keep the same hash.
//! - **Capturable stuck:** the field is idle (no transition, script, battle or starter, the
//!   player settled, nothing held) yet not capturable (other than `StaleView`) for more than
//!   `capturable_stuck` frames.
//! - **Slow step:** a step took more than `slow_step_us` (report-only, host time).
//!
//! Buckets are absolute (`World::frame / roundtrip_every`), so a replay from a new game checks
//! on the same frames as the job that found the bug, from the job's start state on.

use crate::config::DetectConfig;
use crate::guard;
use crate::model::FindingKind;
use crate::observe::{Flag, StepObserver};
use crate::probe::Probe;
use engine::Buttons;
use engine::checkpoint::{NotCapturable, decode, encode};
use engine::data::GameData;
use game_native::Session;

pub struct Detectors<G: GameData> {
    cfg: DetectConfig,
    /// Shadow session after a round trip, and the frames it still follows.
    shadow: Option<(Session<G>, u32)>,
    /// Consecutive idle-but-not-capturable frames.
    idle_not_capturable: u32,
    /// The round-trip bucket last checked (or the start's bucket); `None` before the first frame.
    checked: Option<u32>,
    /// Round trips done.
    round_trips: u32,
}

impl<G: GameData + Clone> Detectors<G> {
    pub fn new(cfg: &DetectConfig) -> Self {
        Detectors {
            cfg: cfg.clone(),
            shadow: None,
            idle_not_capturable: 0,
            checked: None,
            round_trips: 0,
        }
    }

    /// Round trips checked so far.
    pub fn round_trips(&self) -> u32 {
        self.round_trips
    }

    /// Steps the shadow with `keys` and compares it with the live world.
    fn follow(&mut self, s: &Session<Probe<G>>, keys: Buttons) -> Option<Flag> {
        let (shadow, left) = self.shadow.as_mut()?;
        let since = self.cfg.follow - *left + 1;
        let flag = match guard::catch(|| shadow.step(keys)) {
            Err(p) => Some(format!(
                "shadow panicked {since} frames after the round trip: {}",
                p.message
            )),
            Ok(()) => {
                let (want, got) = (s.world().hash(), shadow.world().hash());
                (want != got || s.world().frame() != shadow.world().frame()).then(|| {
                    format!(
                        "shadow diverged {since} frames after the round trip: hash {got}, live \
                         {want}"
                    )
                })
            }
        };
        *left -= 1;
        if flag.is_some() || *left == 0 {
            self.shadow = None;
        }
        flag.map(|message| Flag {
            kind: FindingKind::RoundTrip,
            message,
        })
    }

    /// The round trip of a capturable frame; keeps a shadow when `follow > 0`.
    fn round_trip(&mut self, s: &Session<Probe<G>>) -> Option<Flag> {
        self.round_trips += 1;
        let flag = |message: String| {
            Some(Flag {
                kind: FindingKind::RoundTrip,
                message,
            })
        };
        let cp = match s.checkpoint() {
            Ok(cp) => cp,
            Err(e) => return flag(format!("capturable frame refused a checkpoint: {e:?}")),
        };
        let back = match decode(&encode(&cp)) {
            Ok(back) => back,
            Err(e) => return flag(format!("checkpoint does not decode: {e:?}")),
        };
        if back != cp {
            return flag("checkpoint changed by encode/decode".into());
        }
        let game = s.game().inner().clone();
        let restored = match guard::catch(|| Session::restore(game, &back)) {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return flag(format!("restore failed: {e:?}")),
            Err(p) => return flag(format!("restore panicked: {}", p.message)),
        };
        let (want, got) = (s.world().hash(), restored.world().hash());
        if want != got {
            return flag(format!("restored hash {got}, live {want}"));
        }
        self.shadow = (self.cfg.follow > 0).then_some((restored, self.cfg.follow));
        None
    }

    /// Counts idle-but-not-capturable frames.
    fn stuck(
        &mut self,
        s: &Session<Probe<G>>,
        keys: Buttons,
        capturable: Result<(), NotCapturable>,
    ) -> Option<Flag> {
        let w = s.world();
        let idle = keys == Buttons::NONE
            && w.field_active()
            && !w.transition_active()
            && !w.script_running()
            && !w.in_battle()
            && w.starter().is_none()
            && w.objects().player().is_settled();
        let reason = match capturable {
            Err(r) if idle && r != NotCapturable::StaleView => r,
            _ => {
                self.idle_not_capturable = 0;
                return None;
            }
        };
        self.idle_not_capturable += 1;
        if self.cfg.capturable_stuck == 0 || self.idle_not_capturable <= self.cfg.capturable_stuck {
            return None;
        }
        let n = self.idle_not_capturable;
        self.idle_not_capturable = 0;
        Some(Flag {
            kind: FindingKind::CapturableStuck,
            message: format!("idle but not capturable for {n} frames: {reason:?}"),
        })
    }
}

impl<G: GameData + Clone> StepObserver<G> for Detectors<G> {
    /// Round trip, follow, capturable stuck, slow step (one flag per frame, in that priority:
    /// follow, round trip, stuck, slow).
    fn after_step(&mut self, s: &Session<Probe<G>>, keys: Buttons, step_ns: u64) -> Option<Flag> {
        let mut flags = Vec::new();
        flags.extend(self.follow(s, keys));
        let capturable = s.world().capturable();
        let frame = s.world().frame();
        if let Some(bucket) = frame.checked_div(self.cfg.roundtrip_every) {
            let every = self.cfg.roundtrip_every;
            let checked = *self.checked.get_or_insert(frame.saturating_sub(1) / every);
            if bucket > checked && capturable.is_ok() {
                self.checked = Some(bucket);
                flags.extend(self.round_trip(s));
            }
        }
        flags.extend(self.stuck(s, keys, capturable));
        let limit = u64::from(self.cfg.slow_step_us) * 1000;
        if limit > 0 && step_ns > limit {
            flags.push(Flag {
                kind: FindingKind::SlowFrame,
                message: format!(
                    "step took {} us (limit {} us)",
                    step_ns / 1000,
                    self.cfg.slow_step_us
                ),
            });
        }
        flags.into_iter().next()
    }
}
