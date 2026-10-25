//! Evolution checks and the species change.
//!
//! [`Pokemon::evolution_target`] is Gen3 `GetEvolutionTargetSpecies` for `EVO_MODE_NORMAL`:
//! level methods, and friendship at [`FRIENDSHIP_EVO`] (220). Item and trade methods are the
//! item-use paths. The scene ([`crate::ui::evolution`]) performs the species change.

use super::{BattleTables, Pokemon, species_nickname};
use crate::data::{EvoMethod, Evolution, SpeciesId};

/// Gen3 `FRIENDSHIP_EVO_THRESHOLD`.
pub const FRIENDSHIP_EVO: u8 = 220;

impl Pokemon {
    /// Gen3 `GetEvolutionTargetSpecies(mon, EVO_MODE_NORMAL, ..)` over `evos` (the species'
    /// `gEvolutionTable` row): the last entry that matches wins, as Gen3's loop. A `Level`
    /// entry matches when `param` is at most the level. A `Friendship` entry matches when
    /// friendship is at least [`FRIENDSHIP_EVO`]. Item and trade methods never match here.
    /// The content contract's `Level` may fold several Gen3 level methods (the stat and
    /// personality variants): the second value is true when more than one level entry
    /// matched, a choice the contract cannot make as Gen3 does.
    pub fn evolution_target(&self, evos: &[Evolution]) -> Option<(SpeciesId, bool)> {
        let mut found = None;
        let mut level_matches = 0;
        for e in evos {
            let hit = match e.method {
                EvoMethod::Level if e.param <= u16::from(self.level) => true,
                EvoMethod::Friendship if self.friendship >= FRIENDSHIP_EVO => true,
                _ => false,
            };
            if !hit {
                continue;
            }
            found = Some(e.target);
            if e.method == EvoMethod::Level {
                level_matches += 1;
            }
        }
        found.map(|t| (t, level_matches > 1))
    }

    /// The species change of Gen3 `EvolutionScene`, without the scene: `EvolutionRenameMon`
    /// (a nickname equal to the old species name becomes the new one), then
    /// `CalculateMonStats` (the current HP moves with the max HP). The new species' moves at
    /// this level are not learned (the scene's `MonTryLearningNewMove`, M9).
    pub fn evolve(&mut self, t: &BattleTables, target: SpeciesId) {
        if self.nickname == species_nickname(t.species_info(self.species)) {
            self.nickname = species_nickname(t.species_info(target));
        }
        self.species = target;
        self.recalc_stats(t);
    }
}
