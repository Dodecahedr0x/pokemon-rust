//! Script digests (see the module doc of [`super`]).

use super::*;

/// FNV-1a 64.
pub(super) struct Fnv(u64);

impl Fnv {
    pub(super) fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    pub(super) fn str(&mut self, s: &str) {
        self.bytes(&(s.len() as u64).to_le_bytes());
        self.bytes(s.as_bytes());
    }
    pub(super) fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
}

impl Low<'_> {
    /// Digest of every function, by name. `sources`: each function's source, by entry;
    /// `group_src`: the shared loop of each tail-call group, by leader (part of every member's
    /// own source); `movement_src`: each movement's static.
    pub(super) fn digests(
        &self,
        sources: &BTreeMap<usize, String>,
        group_src: &BTreeMap<usize, String>,
        movement_src: &BTreeMap<&str, String>,
    ) -> BTreeMap<String, u64> {
        let own: BTreeMap<usize, u64> = sources
            .iter()
            .map(|(&f, src)| {
                let mut h = Fnv::new();
                h.str(self.labs[f].name);
                h.str(src);
                if let Some(g) = self.group_of.get(&f) {
                    h.str(&group_src[g]);
                }
                if let Some(o) = self.overrides.get(self.labs[f].name) {
                    h.str(o);
                }
                for t in self.texts_of(f) {
                    h.str(t);
                }
                for m in self.movements_of(f) {
                    h.str(m);
                    h.str(movement_src.get(m).map_or("", |s| s.as_str()));
                }
                (f, h.0)
            })
            .collect();
        let graph: BTreeMap<usize, BTreeSet<usize>> =
            self.fns.iter().map(|&f| (f, self.fn_refs(f))).collect();
        let mut out = BTreeMap::new();
        for &f in &self.fns {
            let mut seen = BTreeSet::from([f]);
            let mut stack = vec![f];
            while let Some(g) = stack.pop() {
                for &t in &graph[&g] {
                    if seen.insert(t) {
                        stack.push(t);
                    }
                }
            }
            let mut h = Fnv::new();
            h.u64(own[&f]);
            let mut rest: Vec<usize> = seen.into_iter().filter(|&g| g != f).collect();
            rest.sort_by_key(|&g| self.labs[g].name);
            for g in rest {
                h.str(self.labs[g].name);
                h.u64(own[&g]);
            }
            out.insert(self.labs[f].name.to_string(), h.0);
        }
        out
    }
}
