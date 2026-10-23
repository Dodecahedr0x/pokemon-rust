//! Decomp label transfer (Milestone 8 Task B): names the scripts, texts and movements the ROM's
//! bytecode reaches with the labels of the pokeemerald sources ([`crate::inc`]).
//!
//! 1. **Roots.** Each map's positional roots ([`roots::named_roots`]: objects, coords, bgs, map
//!    scripts) take the `map.json` labels of the same event index (following
//!    `shared_events_map`), and the `<Map>_MapScripts` entries of the same type (following
//!    `shared_scripts_map`).
//! 2. **Parallel walk.** From each `(offset, label)` pair, the straight-line block decoded at the
//!    offset (up to its terminator) is aligned with the label's reference sequence
//!    ([`inc::Decomp::refs`]), extended along the label's fallthrough chain until a body that
//!    ends. Equal length and kinds give `offset -> label` for every operand; script operands are
//!    walked in turn. Labels the chain falls through to are placed by the command count of the
//!    bodies before them, movement fallthroughs by their action count. A mismatch leaves that
//!    block's targets unnamed and is counted for the label's module ([`ModuleStats`]).
//! 3. **Checks.** A label that resolves to two offsets is a transfer error: every claim on those
//!    offsets is dropped ([`Transfer::errors`]). An offset with two labels (aliases, shared
//!    tails) keeps the first in source order; the others are recorded ([`Transfer::collisions`]).
//! 4. **Fallback.** Every offset the walk reaches that has no decomp label gets the positional
//!    name: a root its `<Map>_Obj<n>`-style name (first in map order), a jump target
//!    `<Owner>_L<n>`, a text `<Owner>_T<n>`, a movement `<Owner>_M<n>`, braille `<Owner>_B<n>`,
//!    where the owner is the block that first references it.
//!
//! Every pair's result depends only on the ROM and the decomp, so the decomp names do not depend
//! on the order of the maps fed in (only the positional fallback does).

use crate::inc::{self, Decomp, LabelKind, RefKind};
use crate::maps::{BgEventKind, MapHeader};
use crate::scripts::{self, Operand};
use crate::{Rom, movement, roots};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub use crate::schema::NameOrigin;
use crate::schema::RootKind;

/// What lives at a named offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Script,
    Text,
    Movement,
    Braille,
}

impl From<RefKind> for Kind {
    fn from(k: RefKind) -> Kind {
        match k {
            RefKind::Script => Kind::Script,
            RefKind::Text => Kind::Text,
            RefKind::Movement => Kind::Movement,
            RefKind::Braille => Kind::Braille,
        }
    }
}

/// One map fed to the transfer: its directory name and ROM header.
#[derive(Clone, Debug)]
pub struct MapInput {
    pub map: String,
    pub header: MapHeader,
}

/// Blocks walked from a module's labels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModuleStats {
    /// Blocks whose references aligned.
    pub aligned: usize,
    /// Blocks whose references did not align (their targets stay positional).
    pub mismatched: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    pub label: String,
    pub offset: u32,
    pub reason: String,
}

/// One label that resolved to several offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferError {
    pub label: String,
    pub offsets: Vec<u32>,
}

/// One offset several labels resolved to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collision {
    pub offset: u32,
    pub kept: String,
    pub dropped: Vec<String>,
}

/// What the plain (decomp-free) walk reaches from the roots.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reached {
    /// Roots and script operand targets: the labels of the positional scheme.
    pub scripts: BTreeSet<u32>,
    pub texts: BTreeSet<u32>,
    pub movements: BTreeSet<u32>,
    pub braille: BTreeSet<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Transfer {
    /// Every reached offset (and every decomp fallthrough label), named.
    pub names: BTreeMap<u32, (String, NameOrigin)>,
    pub kinds: BTreeMap<u32, Kind>,
    /// Offset -> module: the decomp source of a decomp label, else the first referencing root's map.
    pub modules: BTreeMap<u32, String>,
    pub reached: Reached,
    /// Positional root name (`<Map>_Obj<n>`, …) -> its offset, every map.
    pub roots: BTreeMap<String, u32>,
    /// Positional root name -> its `map.json` (or `map_script`) label.
    pub root_labels: BTreeMap<String, String>,
    /// Root offset -> the first root (in map order) starting there.
    pub root_kinds: BTreeMap<u32, RootKind>,
    /// Per module of the walked label.
    pub stats: BTreeMap<String, ModuleStats>,
    pub mismatches: Vec<Mismatch>,
    pub errors: Vec<TransferError>,
    pub collisions: Vec<Collision>,
}

impl Transfer {
    /// How many of `offsets` have a decomp name, and how many there are.
    pub fn coverage(&self, offsets: &BTreeSet<u32>) -> (usize, usize) {
        let named = offsets
            .iter()
            .filter(|o| matches!(self.names.get(o), Some((_, NameOrigin::Decomp))))
            .count();
        (named, offsets.len())
    }
}

/// A claim: `offset` is `name` of `kind`, ranked by source order.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Claim {
    rank: (usize, usize),
    offset: u32,
    name: String,
    kind: Kind,
}

/// The module of the std scripts' positional names (without the decomp).
pub const STD_MODULE: &str = "Common_std_scripts";

/// Maximum fallthrough chain length (guards against a label cycle).
const MAX_CHAIN: usize = 64;

/// Names everything `maps`' roots reach. With no `decomp`, every name is positional.
pub fn transfer(rom: &Rom, decomp: Option<&Decomp>, maps: &[MapInput]) -> Result<Transfer> {
    let mut t = Transfer::default();
    let mut root_list: Vec<(String, u32, String)> = Vec::new();
    // The std scripts' labels (not map roots, so not in `root_labels`).
    let mut std_labels: BTreeMap<String, String> = BTreeMap::new();
    for m in maps {
        let named = roots::named_roots(rom, &m.header, &m.map)?;
        for (name, off) in &named {
            t.roots.insert(name.clone(), *off);
            let rest = name
                .strip_prefix(&format!("{}_", m.map))
                .context("root name")?;
            let kind = if let Some(n) = rest.strip_prefix("Obj") {
                RootKind::Object(n.parse()?)
            } else if let Some(n) = rest.strip_prefix("Coord") {
                RootKind::Coord(n.parse()?)
            } else if let Some(n) = rest.strip_prefix("Bg") {
                RootKind::Bg(n.parse()?)
            } else {
                RootKind::MapScript(rest.to_string())
            };
            t.root_kinds.entry(*off).or_insert(kind);
            root_list.push((name.clone(), *off, m.map.clone()));
        }
        if let Some(d) = decomp {
            for (pos, label) in root_labels(rom, d, m)? {
                t.root_labels.insert(pos, label);
            }
        }
    }
    // The std scripts (`gStdScripts`) after every map, so map roots keep their positional
    // names: lowered as functions nobody calls, they name and export the texts the native std
    // helpers print (`gText_ObtainedTheItem`, …; Milestone 8 Task D2).
    if !maps.is_empty()
        && let Some(stds) = roots::std_scripts(rom)?
    {
        for (i, off) in stds.into_iter().enumerate() {
            let pos = roots::std_root_name(i);
            if decomp.is_some() {
                std_labels.insert(pos.clone(), roots::STD_SCRIPTS[i].to_string());
            }
            root_list.push((pos, off, STD_MODULE.to_string()));
        }
    }
    // The field scripts C starts (Wave C), in the std scripts' module: functions the content
    // looks up by name (`EventScript_UseSurf`).
    if !maps.is_empty() {
        for (i, off) in roots::field_scripts(rom)?.into_iter().enumerate() {
            let Some(off) = off else { continue };
            let pos = roots::field_root_name(i);
            if decomp.is_some() {
                std_labels.insert(pos.clone(), roots::FIELD_SCRIPTS[i].to_string());
            }
            root_list.push((pos, off, STD_MODULE.to_string()));
        }
    }
    let plain = plain_walk(rom, &root_list)?;
    t.reached = plain.reached.clone();
    let mut claims = Vec::new();
    if let Some(d) = decomp {
        let mut w = Walk {
            rom,
            d,
            claims: Vec::new(),
            done: BTreeSet::new(),
            stats: BTreeMap::new(),
            mismatches: Vec::new(),
        };
        let mut work = Vec::new();
        for (pos, off, _) in &root_list {
            let Some(label) = t.root_labels.get(pos).or_else(|| std_labels.get(pos)) else {
                continue;
            };
            match d.label_index(label) {
                Some(li) if d.labels[li].kind == LabelKind::Script => work.push((*off, li)),
                _ => w.mismatches.push(Mismatch {
                    label: label.clone(),
                    offset: *off,
                    reason: format!("root {pos}: no script label {label} in the sources"),
                }),
            }
        }
        // Process in a canonical order so results never depend on the input order.
        work.sort();
        work.dedup();
        w.run(work)?;
        claims = w.claims;
        t.stats = w.stats;
        t.mismatches = w.mismatches;
    }
    resolve(&mut t, claims, decomp);
    if let Some(d) = decomp {
        c_text_runs(rom, d, &mut t)?;
    }
    fallback(&mut t, &plain);
    Ok(t)
}

/// Texts only C code points to (Wave C), named and exported with the scripts' texts: each run is
/// a script-reached text label and the `n` text labels that follow it in its decomp source. The
/// `.string`s of one `.inc` are contiguous in the ROM, so the run's texts are decoded one after
/// the other from the anchor's offset; the label after the run, when it is reached too, must
/// start where the run ends. A run whose anchor this export does not reach is skipped.
pub const C_TEXT_RUNS: &[(&str, usize)] = &[
    // `GetPokedexRatingText` (`birch_pc.c`): `gBirchDexRatingText_LessThan10` …
    // `_LessThan200`, `_DexCompleted` (`data/text/pokedex_rating.inc`).
    ("gBirchDexRatingText_SoYouveSeenAndCaught", 22),
];

fn c_text_runs(rom: &Rom, d: &Decomp, t: &mut Transfer) -> Result<()> {
    let by_name: BTreeMap<String, u32> = t
        .names
        .iter()
        .map(|(&off, (n, _))| (n.clone(), off))
        .collect();
    for &(anchor, n) in C_TEXT_RUNS {
        let (Some(&start), Some(li)) = (by_name.get(anchor), d.label_index(anchor)) else {
            continue;
        };
        let file = d.labels[li].file;
        let module = d.module_of(&d.labels[li]).to_string();
        let mut off = start + crate::text::string_len_at(rom, start)? as u32;
        for k in 1..=n {
            let l = d
                .labels
                .get(li + k)
                .filter(|l| l.file == file && l.kind == LabelKind::Text)
                .with_context(|| format!("{anchor}: text label {k} after it"))?;
            if let Some(&known) = by_name.get(&l.name) {
                anyhow::ensure!(
                    known == off,
                    "{}: at {known:#x}, the run from {anchor} says {off:#x}",
                    l.name
                );
            }
            t.names.insert(off, (l.name.clone(), NameOrigin::Decomp));
            t.kinds.insert(off, Kind::Text);
            t.modules.insert(off, module.clone());
            t.reached.texts.insert(off);
            off += crate::text::string_len_at(rom, off)? as u32;
        }
        if let Some(next) = d.labels.get(li + n + 1)
            && let Some(&known) = by_name.get(&next.name)
        {
            anyhow::ensure!(
                known == off,
                "{}: at {known:#x}, the run from {anchor} ends at {off:#x}",
                next.name
            );
        }
    }
    Ok(())
}

/// Each root of `m` with the label `map.json` (or `<Map>_MapScripts`) gives it.
pub fn root_labels(rom: &Rom, d: &Decomp, m: &MapInput) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let map = &m.map;
    let ev = &m.header.events;
    if let Some(json) = d.events_map(map) {
        for (i, o) in ev.objects.iter().enumerate() {
            if o.script.is_some()
                && let Some(Some(l)) = json.objects.get(i)
            {
                out.push((format!("{map}_Obj{}", o.local_id), l.clone()));
            }
        }
        for (i, c) in ev.coords.iter().enumerate() {
            if c.script.is_some()
                && let Some(Some(l)) = json.coords.get(i)
            {
                out.push((format!("{map}_Coord{i}"), l.clone()));
            }
        }
        for (i, b) in ev.bgs.iter().enumerate() {
            if matches!(b.kind, BgEventKind::Script(Some(_)))
                && let Some(Some(l)) = json.bgs.get(i)
            {
                out.push((format!("{map}_Bg{i}"), l.clone()));
            }
        }
    }
    if let Some(table) = m.header.map_scripts {
        let rt =
            roots::read_map_scripts(rom, table).with_context(|| format!("{map}: map scripts"))?;
        let lt = d.map_scripts(map);
        for (kind, rom_has, label) in [
            ("OnLoad", rt.on_load.is_some(), &lt.on_load),
            (
                "OnTransition",
                rt.on_transition.is_some(),
                &lt.on_transition,
            ),
            ("OnResume", rt.on_resume.is_some(), &lt.on_resume),
            (
                "OnReturnToField",
                rt.on_return_to_field.is_some(),
                &lt.on_return_to_field,
            ),
            ("OnDiveWarp", rt.on_dive_warp.is_some(), &lt.on_dive_warp),
        ] {
            if let (true, Some(l)) = (rom_has, label) {
                out.push((format!("{map}_{kind}"), l.clone()));
            }
        }
        for (kind, n, labels) in [
            ("OnFrame", rt.on_frame.len(), &lt.on_frame),
            ("OnWarpInto", rt.on_warp_into.len(), &lt.on_warp_into),
        ] {
            for (i, l) in labels.iter().enumerate().take(n) {
                out.push((format!("{map}_{kind}{i}"), l.clone()));
            }
        }
    }
    Ok(out)
}

/// The label operands of one decoded instruction, in operand order. `None` targets are null or
/// RAM pointers.
fn operands(insn: &scripts::Insn) -> impl Iterator<Item = (Kind, Option<u32>)> + '_ {
    insn.args.iter().filter_map(|a| match *a {
        Operand::Script(s) => Some((Kind::Script, Some(s))),
        Operand::Text(t) => Some((Kind::Text, t)),
        Operand::Movement(m) => Some((Kind::Movement, m)),
        Operand::Braille(b) => Some((Kind::Braille, b)),
        _ => None,
    })
}

/// The straight-line block at `off`: its instructions, up to and including the terminator, and
/// the offset of an invalid opcode that cut it short.
fn block(rom: &Rom, off: u32) -> Result<(Vec<scripts::Insn>, Option<u32>)> {
    let mut out = Vec::new();
    let mut pc = off;
    loop {
        let Some(insn) = scripts::decode(rom, pc)? else {
            return Ok((out, Some(pc)));
        };
        let (len, stop) = (insn.len, insn.is_terminator());
        out.push(insn);
        if stop {
            return Ok((out, None));
        }
        pc += len;
    }
}

struct Walk<'a> {
    rom: &'a Rom,
    d: &'a Decomp,
    claims: Vec<Claim>,
    done: BTreeSet<(u32, usize)>,
    stats: BTreeMap<String, ModuleStats>,
    mismatches: Vec<Mismatch>,
}

impl Walk<'_> {
    fn rank(&self, li: usize) -> (usize, usize) {
        let l = &self.d.labels[li];
        (l.file, l.line)
    }

    fn claim(&mut self, offset: u32, name: &str, kind: Kind) {
        let rank = match self.d.label_index(name) {
            Some(li) => self.rank(li),
            None => (usize::MAX, 0),
        };
        self.claims.push(Claim {
            rank,
            offset,
            name: name.to_string(),
            kind,
        });
    }

    fn run(&mut self, mut work: Vec<(u32, usize)>) -> Result<()> {
        work.reverse();
        while let Some((off, li)) = work.pop() {
            if !self.done.insert((off, li)) {
                continue;
            }
            let name = self.d.labels[li].name.clone();
            self.claim(off, &name, Kind::Script);
            let module = self.d.module_of(&self.d.labels[li]).to_string();
            let found = self.align(off, li)?;
            let stats = self.stats.entry(module).or_default();
            match found {
                Ok(next) => {
                    stats.aligned += 1;
                    for n in next.into_iter().rev() {
                        if !self.done.contains(&n) {
                            work.push(n);
                        }
                    }
                }
                Err(reason) => {
                    stats.mismatched += 1;
                    self.mismatches.push(Mismatch {
                        label: name,
                        offset: off,
                        reason,
                    });
                }
            }
        }
        Ok(())
    }

    /// Aligns the block at `off` with label `li`'s chain. Returns the script pairs to walk.
    fn align(
        &mut self,
        off: u32,
        li: usize,
    ) -> Result<std::result::Result<Vec<(u32, usize)>, String>> {
        let d = self.d;
        let mut chain = vec![li];
        while let Some(n) = d.labels[*chain.last().unwrap()].next {
            if chain.len() == MAX_CHAIN || chain.contains(&n) {
                return Ok(Err("fallthrough chain too long".to_string()));
            }
            chain.push(n);
        }
        let refs: Vec<inc::Ref> = chain.iter().flat_map(|&c| d.refs(&d.labels[c])).collect();
        let (mut insns, invalid) = block(self.rom, off)?;
        // A chain with no terminator runs into data ("forced stop" after a warp): the source
        // only describes the first commands of what the ROM decodes from there, as many as
        // its bodies expand to.
        if !d.terminates(&d.labels[*chain.last().unwrap()]) {
            let mut count = 0;
            for &c in &chain {
                let Some(ops) = d.ops(&d.labels[c]) else {
                    return Ok(Err("forced stop after an unexpanded macro".to_string()));
                };
                count += ops.len();
            }
            if insns.len() < count {
                return Ok(Err(format!(
                    "{} commands in the ROM before an invalid opcode, {count} in the source",
                    insns.len()
                )));
            }
            insns.truncate(count);
        } else if let Some(pc) = invalid {
            return Ok(Err(format!("invalid opcode at {pc:#x}")));
        }
        let ops: Vec<(Kind, Option<u32>)> = insns.iter().flat_map(operands).collect();
        if ops.len() != refs.len() {
            return Ok(Err(format!(
                "{} label operands in the ROM, {} in the source",
                ops.len(),
                refs.len()
            )));
        }
        let mut pairs = Vec::new();
        for (i, (&(kind, target), r)) in ops.iter().zip(&refs).enumerate() {
            if kind != Kind::from(r.kind) {
                return Ok(Err(format!(
                    "operand {i}: {kind:?} in the ROM, {:?}",
                    r.kind
                )));
            }
            let label = d.label_index(&r.target);
            match (target, label) {
                (Some(t), Some(l)) => {
                    let lk = d.labels[l].kind;
                    let ok = matches!(
                        (kind, lk),
                        (Kind::Script, LabelKind::Script)
                            | (Kind::Text, LabelKind::Text)
                            | (Kind::Movement, LabelKind::Movement)
                            | (Kind::Braille, LabelKind::Braille)
                    );
                    if !ok {
                        return Ok(Err(format!(
                            "operand {i}: {kind:?} pointer to {} ({lk:?})",
                            r.target
                        )));
                    }
                    pairs.push((t, kind, r.target.clone(), Some(l)));
                }
                // A C symbol (`gText_…`): named, not walked.
                (Some(t), None) if r.is_symbol() => pairs.push((t, kind, r.target.clone(), None)),
                (Some(_), None) => {
                    return Ok(Err(format!("operand {i}: pointer for {:?}", r.target)));
                }
                (None, Some(_)) => {
                    return Ok(Err(format!("operand {i}: null or RAM for {}", r.target)));
                }
                (None, None) => {}
            }
        }
        let mut next = Vec::new();
        for (t, kind, name, l) in pairs {
            self.claim(t, &name, kind);
            match (kind, l) {
                (Kind::Script, Some(l)) => next.push((t, l)),
                (Kind::Movement, Some(l)) => self.movement_chain(t, l)?,
                _ => {}
            }
        }
        // Labels the chain falls through to, placed by command counts.
        let mut at = 0;
        for w in chain.windows(2) {
            let Some(ops) = d.ops(&d.labels[w[0]]) else {
                break;
            };
            at += ops.len();
            let Some(insn) = insns.get(at) else { break };
            let name = d.labels[w[1]].name.clone();
            self.claim(insn.off, &name, Kind::Script);
        }
        Ok(Ok(next))
    }

    /// Names the labels a movement at `off` falls through to.
    fn movement_chain(&mut self, mut off: u32, mut l: usize) -> Result<()> {
        let d = self.d;
        let actions = movement::decode(self.rom, off).ok();
        let total = actions.as_ref().map_or(0, |a| a.len() as u32);
        let start = off;
        for _ in 0..MAX_CHAIN {
            let Some(n) = d.labels[l].next else {
                return Ok(());
            };
            let Some(acts) = d.movement_actions(&d.labels[l]) else {
                return Ok(());
            };
            off += acts.len() as u32;
            if off - start > total {
                return Ok(());
            }
            let name = d.labels[n].name.clone();
            self.claim(off, &name, Kind::Movement);
            l = n;
        }
        Ok(())
    }
}

/// Reverse check, then collisions: fills `t.names`, `t.kinds`, `t.modules` with decomp names.
fn resolve(t: &mut Transfer, mut claims: Vec<Claim>, decomp: Option<&Decomp>) {
    claims.sort();
    claims.dedup();
    let mut by_name: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for c in &claims {
        by_name.entry(&c.name).or_default().insert(c.offset);
    }
    let mut demoted = BTreeSet::new();
    for (name, offs) in &by_name {
        if offs.len() > 1 {
            t.errors.push(TransferError {
                label: name.to_string(),
                offsets: offs.iter().copied().collect(),
            });
            demoted.extend(offs.iter().copied());
        }
    }
    let mut by_off: BTreeMap<u32, Vec<&Claim>> = BTreeMap::new();
    for c in &claims {
        if !demoted.contains(&c.offset) {
            by_off.entry(c.offset).or_default().push(c);
        }
    }
    for (off, cs) in by_off {
        // Sorted by rank already (claims are sorted by rank first).
        let mut cs = cs;
        cs.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.name.cmp(&b.name)));
        let kept = cs[0];
        let mut dropped: Vec<String> = cs[1..]
            .iter()
            .map(|c| c.name.clone())
            .filter(|n| *n != kept.name)
            .collect();
        dropped.dedup();
        if !dropped.is_empty() {
            t.collisions.push(Collision {
                offset: off,
                kept: kept.name.clone(),
                dropped,
            });
        }
        t.names.insert(off, (kept.name.clone(), NameOrigin::Decomp));
        t.kinds.insert(off, kept.kind);
        if let Some(d) = decomp
            && let Some(l) = d.label(&kept.name)
        {
            t.modules.insert(off, d.module_of(l).to_string());
        }
    }
}

/// The plain walk: what the roots reach, and who reaches it first.
struct Plain {
    reached: Reached,
    /// Offset -> (kind, owner block start, owning root's map), in discovery order.
    order: Vec<(u32, Kind, u32, String)>,
    /// Root offset -> positional names in map order.
    root_names: BTreeMap<u32, String>,
}

fn plain_walk(rom: &Rom, roots: &[(String, u32, String)]) -> Result<Plain> {
    let mut p = Plain {
        reached: Reached::default(),
        order: Vec::new(),
        root_names: BTreeMap::new(),
    };
    let mut visited = BTreeSet::new();
    for (name, off, map) in roots {
        if p.root_names.contains_key(off) {
            continue;
        }
        p.root_names.insert(*off, name.clone());
        p.reached.scripts.insert(*off);
        p.order.push((*off, Kind::Script, *off, map.clone()));
        let mut queue = VecDeque::from([*off]);
        while let Some(start) = queue.pop_front() {
            let mut pc = start;
            while visited.insert(pc) {
                let Some(insn) = scripts::decode(rom, pc).with_context(|| name.clone())? else {
                    break;
                };
                for (kind, target) in operands(&insn) {
                    let Some(target) = target else { continue };
                    let set = match kind {
                        Kind::Script => &mut p.reached.scripts,
                        Kind::Text => &mut p.reached.texts,
                        Kind::Movement => &mut p.reached.movements,
                        Kind::Braille => &mut p.reached.braille,
                    };
                    if set.insert(target) {
                        p.order.push((target, kind, start, map.clone()));
                    }
                    if kind == Kind::Script {
                        queue.push_back(target);
                    }
                }
                if insn.is_terminator() {
                    break;
                }
                pc += insn.len;
            }
        }
    }
    Ok(p)
}

/// Positional names for every reached offset with no decomp name.
fn fallback(t: &mut Transfer, p: &Plain) {
    let mut used: BTreeSet<String> = t.names.values().map(|(n, _)| n.clone()).collect();
    let mut counters: BTreeMap<(u32, char), usize> = BTreeMap::new();
    for (off, kind, owner, map) in &p.order {
        t.modules.entry(*off).or_insert_with(|| map.clone());
        t.kinds.entry(*off).or_insert(*kind);
        if t.names.contains_key(off) {
            continue;
        }
        let base = match p.root_names.get(off) {
            Some(root) if *kind == Kind::Script => root.clone(),
            _ => {
                let owner_name = t.names.get(owner).map_or_else(
                    || p.root_names.get(owner).cloned().unwrap_or_default(),
                    |(n, _)| n.clone(),
                );
                let tag = match kind {
                    Kind::Script => 'L',
                    Kind::Text => 'T',
                    Kind::Movement => 'M',
                    Kind::Braille => 'B',
                };
                let n = counters.entry((*owner, tag)).or_default();
                let name = format!("{owner_name}_{tag}{n}");
                *n += 1;
                name
            }
        };
        let mut name = base.clone();
        let mut k = 1;
        while !used.insert(name.clone()) {
            name = format!("{base}_{k}");
            k += 1;
        }
        t.names.insert(*off, (name, NameOrigin::Positional));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(rank: (usize, usize), offset: u32, name: &str) -> Claim {
        Claim {
            rank,
            offset,
            name: name.to_string(),
            kind: Kind::Script,
        }
    }

    #[test]
    fn collisions_keep_the_first_label_in_source_order() {
        let mut t = Transfer::default();
        let claims = vec![
            claim((3, 10), 0x100, "Late"),
            claim((1, 5), 0x100, "Early"),
            claim((1, 5), 0x100, "Early"),
            claim((2, 1), 0x200, "Other"),
        ];
        resolve(&mut t, claims, None);
        assert_eq!(t.names[&0x100], ("Early".to_string(), NameOrigin::Decomp));
        assert_eq!(
            t.collisions,
            [Collision {
                offset: 0x100,
                kept: "Early".to_string(),
                dropped: vec!["Late".to_string()]
            }]
        );
        assert!(t.errors.is_empty());
    }

    #[test]
    fn a_label_at_two_offsets_demotes_both() {
        let mut t = Transfer::default();
        let claims = vec![
            claim((1, 1), 0x100, "Twice"),
            claim((1, 1), 0x180, "Twice"),
            claim((2, 1), 0x180, "Shared"),
            claim((3, 1), 0x200, "Fine"),
        ];
        resolve(&mut t, claims, None);
        assert_eq!(
            t.errors,
            [TransferError {
                label: "Twice".to_string(),
                offsets: vec![0x100, 0x180]
            }]
        );
        assert!(!t.names.contains_key(&0x100) && !t.names.contains_key(&0x180));
        assert_eq!(t.names[&0x200].0, "Fine");
    }

    #[test]
    fn fallback_names_follow_the_owner() {
        let mut t = Transfer::default();
        t.names
            .insert(0x100, ("Root_Label".to_string(), NameOrigin::Decomp));
        let m = || "M".to_string();
        let p = Plain {
            reached: Reached::default(),
            order: vec![
                (0x100, Kind::Script, 0x100, m()),
                (0x180, Kind::Script, 0x100, m()),
                (0x300, Kind::Text, 0x100, m()),
                (0x400, Kind::Script, 0x400, m()),
                (0x310, Kind::Text, 0x180, m()),
            ],
            root_names: BTreeMap::from([
                (0x100, "M_Obj1".to_string()),
                (0x400, "M_Bg0".to_string()),
            ]),
        };
        fallback(&mut t, &p);
        let name = |o: u32| t.names[&o].0.as_str();
        assert_eq!(name(0x100), "Root_Label");
        assert_eq!(name(0x180), "Root_Label_L0");
        assert_eq!(name(0x300), "Root_Label_T0");
        assert_eq!(name(0x400), "M_Bg0");
        assert_eq!(name(0x310), "Root_Label_L0_T0");
        assert_eq!(t.names[&0x180].1, NameOrigin::Positional);
        assert_eq!(t.modules[&0x310], "M");
    }
}
