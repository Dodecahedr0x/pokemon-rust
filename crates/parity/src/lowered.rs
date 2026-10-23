//! The content build's lowering, as `content::PARITY` writes it (`content/build/parity.rs`):
//! the IR sites per command and special with their class, the reported and stale sites.

use std::collections::BTreeMap;

/// One command or special of the lowered IR.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lowered {
    /// Support-table class (`Native`, `Ignore`, `Default`, `Unsupported`).
    pub class: String,
    pub sites: usize,
    /// Script module -> sites.
    pub modules: BTreeMap<String, usize>,
}

/// `content::PARITY`, parsed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lowering {
    pub names: BTreeMap<String, Lowered>,
    /// `Report::Unsupported` sites by name (`Stats::reports`).
    pub reports: BTreeMap<String, usize>,
    /// Stale `specialvar` sites by special.
    pub stale: BTreeMap<String, usize>,
    pub stats: BTreeMap<String, usize>,
}

impl Lowering {
    pub fn parse(tsv: &str) -> Lowering {
        let mut l = Lowering::default();
        for line in tsv.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let n = |s: &str| s.parse::<usize>().unwrap_or(0);
            match f[..] {
                ["site", name, class, module, k] => {
                    let e = l.names.entry(name.to_string()).or_default();
                    e.class = class.to_string();
                    e.sites += n(k);
                    *e.modules.entry(module.to_string()).or_default() += n(k);
                }
                ["report", name, k] => {
                    l.reports.insert(name.to_string(), n(k));
                }
                ["stale", name, k] => {
                    l.stale.insert(name.to_string(), n(k));
                }
                ["stat", key, k] => {
                    l.stats.insert(key.to_string(), n(k));
                }
                _ => {}
            }
        }
        l
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_rows() {
        let l = Lowering::parse(
            "site\tdofieldeffect\tUnsupported\tA\t2\nsite\tdofieldeffect\tUnsupported\tB\t1\nreport\tdofieldeffect\t3\nstale\tSPECIAL_X\t1\nstat\tunresolved_waitmovement\t80\n",
        );
        assert_eq!(l.names["dofieldeffect"].sites, 3);
        assert_eq!(l.names["dofieldeffect"].modules.len(), 2);
        assert_eq!(l.reports["dofieldeffect"], 3);
        assert_eq!(l.stale["SPECIAL_X"], 1);
        assert_eq!(l.stats["unresolved_waitmovement"], 80);
    }
}
