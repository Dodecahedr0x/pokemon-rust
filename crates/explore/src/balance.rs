//! Battle records (Task F). Balance is recorded, weight 0 in v0 (decision 13).

use crate::model::BattleSummary;
use engine::World;
use engine::battle::{BattleKind, Side};

/// Tracks one battle from its first frame to the field's return.
///
/// The driver calls [`BattleTracker::start`] on the frame `World::in_battle` turns true,
/// [`BattleTracker::observe`] after every battle frame, and [`BattleTracker::finish`] once the
/// field is back (or when it gives up on the battle).
#[derive(Clone, Debug)]
pub struct BattleTracker {
    start_frame: u32,
    /// 0 wild, 1 first battle, 2 trainer.
    kind: u8,
    trainer: Option<u16>,
    player_level: u8,
    opponent_level: u8,
    /// The player battler's HP and max HP on the first frame.
    hp_start: u16,
    max_hp: u16,
    /// The player battler's HP on the last observed battle frame.
    hp_last: u16,
    /// The scene's outcome, once observed.
    outcome: Option<u8>,
}

impl BattleTracker {
    /// `None` when no battle owns the frame.
    pub fn start(w: &World) -> Option<Self> {
        let (_, battle, scene) = w.battle()?;
        let (kind, trainer) = match battle.setup().kind {
            BattleKind::Wild {
                first_battle: false,
            } => (0, None),
            BattleKind::Wild { first_battle: true } => (1, None),
            BattleKind::Trainer { trainer, .. } => (2, Some(trainer.0)),
        };
        let player = battle.mon(Side::Player);
        let player_level = w.party().get(0).map_or(player.level, |p| p.level);
        Some(BattleTracker {
            start_frame: w.frame(),
            kind,
            trainer,
            player_level,
            opponent_level: battle.mon(Side::Opponent).level,
            hp_start: player.hp,
            max_hp: player.max_hp,
            hp_last: player.hp,
            outcome: scene.finished().map(|o| o as u8),
        })
    }

    /// Records the player battler's HP and the outcome once the scene has one (no-op outside
    /// a battle). HP is read in battle only, since a whiteout heals the party once the field is
    /// back.
    pub fn observe(&mut self, w: &World) {
        if let Some((_, battle, scene)) = w.battle() {
            self.hp_last = battle.mon(Side::Player).hp;
            if let Some(o) = scene.finished() {
                self.outcome = Some(o as u8);
            }
        }
    }

    /// The outcome observed so far (`BattleOutcome as u8`).
    pub fn outcome(&self) -> Option<u8> {
        self.outcome
    }

    /// The battle's record. `outcome` (the driver's) wins over the observed one.
    pub fn finish(mut self, w: &World, outcome: Option<u8>) -> BattleSummary {
        self.observe(w);
        let lost = self.hp_start.saturating_sub(self.hp_last);
        let hp_lost_permille = if self.max_hp == 0 {
            0
        } else {
            (u32::from(lost) * 1000 / u32::from(self.max_hp)).min(1000) as u16
        };
        BattleSummary {
            kind: self.kind,
            trainer: self.trainer,
            outcome: outcome.or(self.outcome),
            frames: w.frame().saturating_sub(self.start_frame),
            player_level: self.player_level,
            opponent_level: self.opponent_level,
            hp_lost_permille,
        }
    }
}
