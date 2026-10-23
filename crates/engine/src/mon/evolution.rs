//! Evolution checks and the species change (moved out of `mon.rs` by Milestone 9 Wave 0,
//! unchanged; Task E owns the file: the exact `EVO_*` methods, the friendship and item checks,
//! `try_evolve`).

use super::{BattleTables, Pokemon, species_nickname};
use crate::data::{EvoMethod, Evolution, SpeciesId};

impl Pokemon {
    /// Gen3 `GetEvolutionTargetSpecies(mon, EVO_MODE_NORMAL, ..)` over `evos` (the species'
    /// `gEvolutionTable` row), level methods only: the last entry whose level is reached wins,
    /// as Gen3's loop. Friendship, item and trade methods never match here (M9). The content
    /// contract's `Level` may fold several Gen3 level methods (the stat and personality
    /// variants, such as Wurmple's and Tyrogue's): the second value is true when more than one
    /// level entry matched, a choice the contract cannot make as Gen3 does (the caller reports
    /// it). Whatever a `Level` entry stands for, it is taken at its level, as `EVO_LEVEL`.
    pub fn evolution_target(&self, evos: &[Evolution]) -> Option<(SpeciesId, bool)> {
        let mut found = None;
        let mut matches = 0;
        for e in evos {
            if e.method == EvoMethod::Level && e.param <= self.level as u16 {
                found = Some(e.target);
                matches += 1;
            }
        }
        found.map(|t| (t, matches > 1))
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
