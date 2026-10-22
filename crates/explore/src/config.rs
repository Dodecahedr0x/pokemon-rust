//! Explorer configuration: defaults and RON loading.
//!
//! Every struct is `#[serde(default)]`, so a config file only lists what it changes.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub seed: u64,
    /// 0 = rayon default.
    pub threads: usize,
    /// Jobs per generation; 0 = `runner::DEFAULT_BATCH` (32), never the thread count.
    pub batch: usize,
    pub max_plan_len: u8,
    pub tournament: usize,
    /// Per generation since creation.
    pub age_penalty: i64,
    /// Per child already tried.
    pub expanded_penalty: i64,
    pub weights: Weights,
    pub key: KeyConfig,
    pub limits: Limits,
    pub detect: DetectConfig,
    /// Snapshots kept in memory.
    pub cache_capacity: usize,
    /// Also store a snapshot in the db every N depths.
    pub snapshot_every_depth: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weights {
    pub flag: i64,
    pub var_value: i64,
    pub map: i64,
    pub script: i64,
    pub cell: i64,
    pub new_key: i64,
    pub goal: i64,
    pub bug: i64,
    /// 0 in v0.
    pub balance: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    /// 2 = 4x4 cells.
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
    /// Probe 1 in N new nodes, plus every node whose candidates all failed.
    pub softlock_sample: u32,
    /// Report-only; 0 = off (the default in debug builds, 4000 in release).
    pub slow_step_us: u32,
}

/// How long `explore run` goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    Nodes(u64),
    Frames(u64),
    Generations(u32),
    Secs(u64),
}

impl Default for Config {
    fn default() -> Self {
        Config {
            seed: 0x005E_EDE7,
            threads: 0,
            batch: 128,
            max_plan_len: 3,
            tournament: 4,
            age_penalty: 1,
            expanded_penalty: 4,
            weights: Weights::default(),
            key: KeyConfig::default(),
            limits: Limits::default(),
            detect: DetectConfig::default(),
            cache_capacity: 65536,
            snapshot_every_depth: 8,
        }
    }
}

impl Default for Weights {
    fn default() -> Self {
        Weights {
            flag: 40,
            var_value: 20,
            map: 100,
            script: 30,
            cell: 2,
            new_key: 10,
            goal: 1000,
            bug: 500,
            balance: 0,
        }
    }
}

impl Default for KeyConfig {
    /// Content-free defaults; `key::default_key_config` adds the content's exclusions.
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
            softlock_sample: 32,
            slow_step_us: if cfg!(debug_assertions) { 0 } else { 4000 },
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

    /// Pretty RON of the config (the `run` table, `explore status`).
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
        assert_eq!(c.seed, 0x005E_EDE7);
        assert_eq!((c.max_plan_len, c.tournament), (3, 4));
        // Task H: 128 jobs per generation and 65,536 cached snapshots (see the plan's "Wave 2
        // as built").
        assert_eq!(c.batch, 128);
        assert_eq!((c.cache_capacity, c.snapshot_every_depth), (65536, 8));
        assert_eq!((c.key.cell_shift, c.key.level_step), (2, 3));
        assert_eq!(c.weights.balance, 0);
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
        assert_eq!(
            (d.softlock_k, d.softlock_breadth, d.softlock_sample),
            (3, 64, 32)
        );
    }

    #[test]
    fn missing_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let c = Config::load_or_default(&dir.path().join("none.ron")).unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn ron_round_trip_and_partial_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("explore.ron");
        let mut c = Config {
            seed: 42,
            ..Config::default()
        };
        c.detect.softlock_k_per_map.insert(9, 5);
        std::fs::write(&path, c.to_ron()).unwrap();
        assert_eq!(Config::load_or_default(&path).unwrap(), c);

        std::fs::write(
            &path,
            "(seed: 7, weights: (goal: 5), limits: (enter_frames: 9))",
        )
        .unwrap();
        let p = Config::load_or_default(&path).unwrap();
        assert_eq!((p.seed, p.weights.goal, p.limits.enter_frames), (7, 5, 9));
        assert_eq!(p.weights.map, Weights::default().map);
        assert_eq!(p.limits.macro_frames, 3000);

        std::fs::write(&path, "(seed: \"x\")").unwrap();
        assert!(Config::load_or_default(&path).is_err());
    }
}
