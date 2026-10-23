//! Explorer configuration: defaults and RON loading.
//!
//! Every struct is `#[serde(default)]`, so a config file only lists what it changes; fields a
//! file has that the config no longer knows (the evolutionary explorer's `seed`, `batch` …)
//! are ignored.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Worker threads for branches; 0 = `workers::default_threads` (three quarters of the CPUs).
    pub jobs: usize,
    pub weights: Weights,
    pub key: KeyConfig,
    pub limits: Limits,
    pub detect: DetectConfig,
    pub branch: BranchConfig,
    /// The goal list the spine's `Expect` steps and the stats name.
    pub act: crate::goals::Act,
}

/// Novelty weights: what one new thing (against the spine and the branch so far) is worth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weights {
    pub flag: i64,
    pub var_value: i64,
    pub map: i64,
    pub script: i64,
    pub cell: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    /// 2 = 4x4 cells (the coarse blocks of `WalkTo` targets).
    pub cell_shift: u8,
    pub level_step: u16,
    pub ignore_flags: Vec<u16>,
    pub ignore_vars: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub macro_frames: u32,
    pub battle_frames: u32,
    pub settle_frames: u32,
    pub enter_frames: u32,
    pub grass_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectConfig {
    /// Check at the first capturable frame after every N frames; 0 = off.
    pub roundtrip_every: u32,
    /// Shadow-session frames after a round trip.
    pub follow: u32,
    /// Idle-but-not-capturable frames before a finding.
    pub capturable_stuck: u32,
    /// Macro depth of the trap probe.
    pub softlock_k: u32,
    pub softlock_k_per_map: BTreeMap<u16, u32>,
    /// Expansions per probe.
    pub softlock_breadth: u32,
    /// How often the probe resumes a walk that a battle interrupted before its target (0 =
    /// never). Each resume can cost another full macro, so one probe's worst case is
    /// `(1 + probe_resumes) x softlock_breadth` expansions.
    pub probe_resumes: u32,
    /// How often a start found trapped while an object of its map walks by itself is probed
    /// again, each after a wait of `reprobe_wait_frames` (0 = never).
    pub reprobes: u32,
    /// Frames of each wait before a re-probe.
    pub reprobe_wait_frames: u16,
    /// Report-only; 0 = off (the default in debug builds, 4000 in release).
    pub slow_step_us: u32,
}

/// The branches (`docs/plans/2026-10-23-explorer-spine.md` §3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BranchConfig {
    /// Branch from every `every`-th spine step's end, besides every checkpoint (0: checkpoints
    /// only).
    pub every: u32,
    /// Exploration frames past which a branch is aborted (no recovery).
    pub budget_frames: u32,
    /// Novelty points (`Weights`) per 1,000 frames below which a branch stops exploring after
    /// its first macro.
    pub min_novelty: u32,
    /// Macros a branch plays at most (its target included).
    pub max_steps: u32,
    /// Spine steps resumed after the recovery (the verification window).
    pub window: u32,
    /// Targets per origin, at most.
    pub max_targets: u32,
    /// Frames the recovery may take before the branch is stranded.
    pub recover_frames: u32,
    /// Warps and crossings the recovery may take on the way back.
    pub recover_hops: u32,
}

impl Default for Weights {
    fn default() -> Self {
        Weights {
            flag: 40,
            var_value: 20,
            map: 100,
            script: 30,
            cell: 2,
        }
    }
}

impl Default for KeyConfig {
    /// Content-free defaults; `key::story_flag`/`story_var` add the content's exclusions.
    fn default() -> Self {
        KeyConfig {
            cell_shift: 2,
            level_step: 3,
            ignore_flags: Vec::new(),
            ignore_vars: Vec::new(),
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            macro_frames: 3000,
            battle_frames: 12000,
            settle_frames: 1200,
            enter_frames: 120,
            grass_frames: 2400,
        }
    }
}

impl Default for DetectConfig {
    fn default() -> Self {
        DetectConfig {
            roundtrip_every: 600,
            follow: 60,
            capturable_stuck: 600,
            softlock_k: 3,
            softlock_k_per_map: BTreeMap::new(),
            softlock_breadth: 64,
            probe_resumes: 4,
            reprobes: 4,
            reprobe_wait_frames: 600,
            slow_step_us: if cfg!(debug_assertions) { 0 } else { 4000 },
        }
    }
}

impl Default for BranchConfig {
    fn default() -> Self {
        BranchConfig {
            every: 8,
            budget_frames: 6000,
            min_novelty: 20,
            max_steps: 16,
            window: 3,
            max_targets: 48,
            recover_frames: 6000,
            recover_hops: 8,
        }
    }
}

impl Config {
    /// The RON config at `path` (missing fields take their defaults), or the defaults when
    /// there is no file.
    pub fn load_or_default(path: &Path) -> anyhow::Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// Pretty RON of the config (the `run` table).
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .expect("a Config always serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_plan() {
        let c = Config::default();
        assert_eq!((c.key.cell_shift, c.key.level_step), (2, 3));
        assert_eq!(
            c.limits,
            Limits {
                macro_frames: 3000,
                battle_frames: 12000,
                settle_frames: 1200,
                enter_frames: 120,
                grass_frames: 2400
            }
        );
        let d = &c.detect;
        assert_eq!(
            (d.roundtrip_every, d.follow, d.capturable_stuck),
            (600, 60, 600)
        );
        assert_eq!((d.softlock_k, d.softlock_breadth), (3, 64));
        let b = &c.branch;
        assert_eq!((b.every, b.budget_frames, b.min_novelty), (8, 6000, 20));
        assert_eq!((b.window, b.recover_frames, b.recover_hops), (3, 6000, 8));
    }

    #[test]
    fn missing_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let c = Config::load_or_default(&dir.path().join("none.ron")).unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn ron_round_trip_partial_and_old_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("explore.ron");
        let mut c = Config {
            jobs: 3,
            ..Config::default()
        };
        c.detect.softlock_k_per_map.insert(9, 5);
        std::fs::write(&path, c.to_ron()).unwrap();
        assert_eq!(Config::load_or_default(&path).unwrap(), c);

        // An evolutionary-explorer file (`seed`, `batch`, `weights.goal`) still loads.
        std::fs::write(
            &path,
            "(seed: 7, batch: 3, weights: (goal: 5, map: 9), branch: (window: 2))",
        )
        .unwrap();
        let p = Config::load_or_default(&path).unwrap();
        assert_eq!((p.weights.map, p.branch.window), (9, 2));
        assert_eq!(p.limits.macro_frames, 3000);

        std::fs::write(&path, "(jobs: \"x\")").unwrap();
        assert!(Config::load_or_default(&path).is_err());
    }
}
