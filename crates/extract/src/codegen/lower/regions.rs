//! Functions, their blocks and the call graph.

use super::*;

impl<'a> Low<'a> {
    /// Picks the functions and the blocks each one owns.
    pub(super) fn build_regions(&mut self) {
        self.fns = (0..self.labs.len())
            .filter(|&l| self.labs[l].root || self.overrides.contains_key(self.labs[l].name))
            .collect();
        for l in &self.labs {
            for &(kind, t) in &l.edges {
                if kind == Edge::Call {
                    self.fns.insert(t);
                }
            }
        }
        loop {
            let mut owners: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
            self.regions.clear();
            for &f in &self.fns {
                let mut seen = BTreeSet::from([f]);
                let mut stack = vec![f];
                while let Some(b) = stack.pop() {
                    for &(kind, t) in &self.labs[b].edges {
                        if kind == Edge::Jump && !self.fns.contains(&t) && seen.insert(t) {
                            stack.push(t);
                        }
                    }
                }
                for &b in &seen {
                    owners.entry(b).or_default().insert(f);
                }
                let mut blocks: Vec<usize> = seen.into_iter().collect();
                blocks.sort_by_key(|&b| (b != f, b));
                self.regions.insert(f, blocks);
            }
            // Blocks owned by several functions, or by none (unreachable from the roots).
            let promote: Vec<usize> = (0..self.labs.len())
                .filter(|b| !self.fns.contains(b))
                .filter(|b| owners.get(b).is_none_or(|o| o.len() > 1))
                .collect();
            if promote.is_empty() {
                return;
            }
            // An unowned block whose label is reached by another unowned block is not a
            // function of its own; promote the first in label order and recompute.
            let shared: Vec<usize> = promote
                .iter()
                .copied()
                .filter(|b| owners.contains_key(b))
                .collect();
            if shared.is_empty() {
                self.fns.insert(promote[0]);
            } else {
                self.fns.extend(shared);
            }
        }
    }

    /// Function entries reached from `f` by a tail call (`Jump`) and by `call` (`Call`).
    pub(super) fn fn_targets(&self, f: usize) -> Vec<(Edge, usize)> {
        if self.overrides.contains_key(self.labs[f].name) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for &b in &self.regions[&f] {
            for &(kind, t) in &self.labs[b].edges {
                if self.fns.contains(&t) {
                    out.push((kind, t));
                }
            }
        }
        out
    }

    /// Groups the functions that tail-call each other (a strongly connected component of the
    /// tail-call graph, more than one function): they share one `loop { match pc }`, so a
    /// `goto` loop between them runs in constant memory instead of nesting boxed futures.
    pub(super) fn tail_call_groups(&mut self) -> Result<()> {
        let tails: BTreeMap<usize, BTreeSet<usize>> = self
            .fns
            .iter()
            .map(|&f| {
                let t = self
                    .fn_targets(f)
                    .into_iter()
                    .filter(|&(k, t)| k == Edge::Jump && t != f)
                    .map(|(_, t)| t)
                    .collect();
                (f, t)
            })
            .collect();
        let reaches: BTreeMap<usize, BTreeSet<usize>> = self
            .fns
            .iter()
            .map(|&f| {
                let mut seen = BTreeSet::new();
                let mut stack = vec![f];
                while let Some(g) = stack.pop() {
                    for &t in &tails[&g] {
                        if seen.insert(t) {
                            stack.push(t);
                        }
                    }
                }
                (f, seen)
            })
            .collect();
        // In label order, so the first member met is the group's leader.
        for &f in &self.fns {
            if self.group_of.contains_key(&f) {
                continue;
            }
            let mut members: Vec<usize> = reaches[&f]
                .iter()
                .copied()
                .filter(|&g| g != f && reaches[&g].contains(&f))
                .collect();
            if members.is_empty() {
                continue;
            }
            members.insert(0, f);
            let name = group_name(self.labs[f].name);
            ensure!(
                !self.at.contains_key(name.as_str()),
                "script {name} clashes with the tail-call group of {}",
                self.labs[f].name
            );
            for &m in &members {
                self.group_of.insert(m, f);
            }
            self.groups.insert(f, members);
        }
        Ok(())
    }

    /// Finds the functions that can return `Flow::End`.
    pub(super) fn analyse(&mut self) {
        let mut set: BTreeSet<usize> = self
            .fns
            .iter()
            .copied()
            .filter(|&f| {
                !self.overrides.contains_key(self.labs[f].name)
                    && self.regions[&f].iter().any(|&b| {
                        let l = &self.labs[b];
                        l.end == End::Unknown
                            || l.insns.iter().any(|i| {
                                END_OPS.contains(&i.op.as_str())
                                    || (TERMINATORS.contains(&i.op.as_str())
                                        && self.class(&i.op) != Class::Native)
                            })
                    })
            })
            .collect();
        loop {
            let before = set.len();
            for &f in &self.fns {
                if self.fn_targets(f).iter().any(|(_, t)| set.contains(t)) {
                    set.insert(f);
                }
            }
            if set.len() == before {
                break;
            }
        }
        self.can_end = set;
    }

    /// Functions `f`'s digest closure steps to: its call-graph targets, the std scripts its
    /// `callstd`s name, and the scripts an override's source mentions.
    pub(super) fn fn_refs(&self, f: usize) -> BTreeSet<usize> {
        let mut out: BTreeSet<usize> = self.fn_targets(f).into_iter().map(|(_, t)| t).collect();
        if let Some(src) = self.overrides.get(self.labs[f].name) {
            for w in src.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                if let Some(&t) = self.at.get(w)
                    && self.fns.contains(&t)
                    && t != f
                {
                    out.insert(t);
                }
            }
            return out;
        }
        for &b in &self.regions[&f] {
            for i in self.labs[b].insns {
                if matches!(
                    i.op.as_str(),
                    "callstd" | "gotostd" | "callstd_if" | "gotostd_if"
                ) && let Some(n) = i.args.last().and_then(std_index)
                    && let Some(label) = STD_LABELS.get(n as usize)
                    && let Some(&t) = self.at.get(label)
                    && self.fns.contains(&t)
                {
                    out.insert(t);
                }
            }
        }
        out
    }

    pub(super) fn texts_of(&self, f: usize) -> Vec<&'a str> {
        if self.overrides.contains_key(self.labs[f].name) {
            return Vec::new();
        }
        let mut v = Vec::new();
        for &b in &self.regions[&f] {
            for i in self.labs[b].insns {
                for a in &i.args {
                    if let IrArg::Text(t) = a {
                        v.push(t.as_str());
                    }
                }
            }
        }
        v
    }

    pub(super) fn movements_of(&self, f: usize) -> Vec<&'a str> {
        if self.overrides.contains_key(self.labs[f].name) {
            return Vec::new();
        }
        let mut v = Vec::new();
        for &b in &self.regions[&f] {
            for i in self.labs[b].insns {
                for a in &i.args {
                    if let IrArg::Movement(m) = a {
                        v.push(m.as_str());
                    }
                }
            }
        }
        v
    }
}
