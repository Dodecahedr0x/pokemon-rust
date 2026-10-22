//! Per-frame observer hooks: detectors plug into the expander through [`StepObserver`].

use crate::model::FindingKind;
use crate::probe::Probe;
use engine::Buttons;
use engine::data::GameData;
use game_native::Session;

/// A flag raised by an observer; the driver turns it into a FindingDraft with the context.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Flag {
    pub kind: FindingKind,
    pub message: String,
}

pub trait StepObserver<G: GameData> {
    /// After every stepped frame; `step_ns` is the host time of the step (report-only).
    fn after_step(&mut self, s: &Session<Probe<G>>, keys: Buttons, step_ns: u64) -> Option<Flag>;
}

/// Observes nothing.
pub struct NoObserver;

impl<G: GameData> StepObserver<G> for NoObserver {
    fn after_step(&mut self, _: &Session<Probe<G>>, _: Buttons, _: u64) -> Option<Flag> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::Emerald;

    /// Flags the first frame whose world frame reaches `at`.
    struct At(u32);

    impl<G: GameData> StepObserver<G> for At {
        fn after_step(&mut self, s: &Session<Probe<G>>, _: Buttons, _: u64) -> Option<Flag> {
            (s.world().frame() >= self.0).then(|| Flag {
                kind: FindingKind::SlowFrame,
                message: format!("frame {}", s.world().frame()),
            })
        }
    }

    #[test]
    fn observers_are_object_safe_and_see_the_session() {
        let mut s = Session::new(Probe::new(Emerald::for_shells()));
        let start = s.world().frame();
        let mut obs: Vec<Box<dyn StepObserver<Emerald>>> =
            vec![Box::new(NoObserver), Box::new(At(start + 3))];
        let mut flags = Vec::new();
        for _ in 0..3 {
            s.step(Buttons::NONE);
            for o in &mut obs {
                flags.extend(o.after_step(&s, Buttons::NONE, 0));
            }
        }
        assert_eq!(
            flags,
            [Flag {
                kind: FindingKind::SlowFrame,
                message: format!("frame {}", start + 3)
            }]
        );
    }
}
