//! The script IR ([`ScriptFile`]) to async Rust (Milestone 8 Task D1), run by the content build.
//!
//! **Functions.** A *block* is an [`crate::schema::IrLabel`]: its instructions, ended by a
//! terminator, by a fallthrough into `next`, or by nothing (an undecodable instruction). Roots
//! (`IrLabel::root`, or `root` in the index), `call`/`call_if` targets, other script operands
//! (`trainerbattle` continue scripts) and overridden scripts are functions; so is any block
//! reached by `goto`/fallthrough from more than one function, or from none. A function owns the
//! blocks it reaches by `goto` and fallthrough without entering another function; with more than
//! one block (or a jump back to its entry) it is a `loop { match pc { … } }`. Entering another
//! function by `goto` or fallthrough is a tail call: `return Box::pin(f(ctx.clone())).await;`.
//!
//! **Gen3 semantics.**
//! - Every function returns `Flow`: `end` is `Flow::End`, `return` is `Flow::Continue`. A
//!   `call` into a function that can end (itself, or through its callees) is
//!   `if … == Flow::End { return Flow::End; }`, so a callee's `end` stops its caller, as in Gen3.
//!   A tail call returns the callee's `Flow`. The thunk table drops the root's `Flow`.
//! - The comparison register is the script's (`ctx.cond()`/`ctx.set_cond()`, Gen3
//!   `comparisonResult`), so it flows across calls. `checkflag` sets `Equal` when the flag is set,
//!   `Less` otherwise (`comparisonResult` 1/0).
//! - `waitmovement 0` is `ctx.wait_move(0)`: the engine's last mover (Gen3 `sMovingNpcId`).
//!   [`Stats::unresolved_waitmovement`] counts the sites with no `applymovement` earlier in the
//!   same block.
//! - Operands the game reads with `VarGet` read the var at run time when they are a `Var` or a
//!   number ≥ 0x4000.
//! - `trainerbattle`: the generated code reads the trainer flag (Gen3 `GetTrainerFlag`), then
//!   calls the prelude's `trainer_battle`. When the flag was clear the battle was fought and its
//!   beaten script ran (Gen3 `gotobeatenscript`; the continue script is `event_script`, run by the
//!   helper), so the script ends; otherwise the code after it is the already-beaten path
//!   (`gotopostbattlescript`). `_NO_INTRO_TEXT` and the facility kinds (9–12) always go on.
//!   The rematch kinds (5, 7) always end: their scripts are only reached when a rematch is due.
//! - `setwildbattle` + `dowildbattle` keep the pending battle in a local of the function.
//!
//! **Classes** ([`Support`]): a command or special missing from the tables fails. `Native`
//! commands have an arm here ([`NATIVE_COMMANDS`]). `Ignore` is a comment. `Default(v)` and
//! `Unsupported` are `ctx.report(Report::Unsupported("<op>")).await` (logged, an engine event);
//! `Default(v)` then writes `v` where the command writes its result. An `Unsupported`
//! terminator ends the script. A `Native` command whose operands do not fit its arm is reported
//! the same way.
//!
//! **Output contract** ([`Lowered`]):
//! - `modules[<Module>]` is the body of `pub mod <Module> { use crate::prelude::*; <body> }`
//!   inside `crate::scripts`, wrapped with the same `#[allow(…)]` as the legacy files. It starts
//!   with `use super::Flow;` when it has functions, then `pub async fn <Name>(ctx: Ctx) -> Flow`
//!   per function and `pub static <Movement>: [Step; N]` per movement. Other modules are
//!   `super::<Module>::<Name>`.
//! - `dispatch` is the rest of `crate::scripts`: `Flow`, `SCRIPT_COUNT`, `SCRIPT_NAMES` (every
//!   function, sorted by name; the index is the `ScriptId`), `script_ids::<Name>`, the
//!   `SCRIPTS` thunk table and `script(id, ctx)`.
//! - The prelude provides, besides the legacy contract (`msgbox`, `MsgBox`, `callstd`,
//!   `special`, `check_player_gender`, `trainer_flag`, `Ordering`, `Box`, the `Ctx` types and
//!   every constant): `Report`, `ItemId`, `MoveId`, `BufferSrc`, `WarpSlot`, `TrainerBattle`,
//!   `trainer_battle`, `give_item`, `find_item`, `check_item`, `money_add`, `money_remove`,
//!   `money_check`, `buffer`, `multichoice` and `set_warp` (Wave 0 names), and `Ctx::{cond,
//!   set_cond, report, item_add, item_remove, item_space}`.
//! - An override (`overrides[<Name>]`) lowers to
//!   `pub async fn <Name>(ctx: Ctx) -> Flow { crate::overrides::<Name>(ctx).await; Flow::Continue }`:
//!   the override is `pub async fn <Name>(ctx: Ctx)`, and its callers go on after it.
//!
//! **Digests** ([`Lowered::script_digests`]), by function name: FNV-1a 64 over the function's
//! lowered source (or its override's source), the texts it names and the movements it uses, then
//! the same for every function in its static call-graph closure (calls, tail calls, other script
//! operands, the std scripts its `callstd`s name when the IR has them, and the script names an
//! override's source mentions), in name order.
//!
//! **Undecodable code:** a block with no terminator and no `next` ends in an undecodable
//! instruction: `Report::Unsupported("unknown opcode")`, then `Flow::End`.

use super::{Class, Lowered, MoveFallback, Stats, Support, is_ident, named};
use crate::schema::{IrArg, IrInsn, IrMovement, NameOrigin, ScriptFile, ScriptIndex};
use crate::{movement, names};
use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Every command with a lowering arm. `commands.ron` may only mark these `Native`.
pub const NATIVE_COMMANDS: &[&str] = &[
    "nop",
    "nop1",
    "end",
    "return",
    "call",
    "goto",
    "goto_if",
    "call_if",
    "gotostd",
    "callstd",
    "gotostd_if",
    "callstd_if",
    "returnram",
    "endram",
    "loadword",
    "setvar",
    "addvar",
    "subvar",
    "copyvar",
    "setorcopyvar",
    "compare_var_to_value",
    "compare_var_to_var",
    "special",
    "specialvar",
    "waitstate",
    "delay",
    "setflag",
    "clearflag",
    "checkflag",
    "warp",
    "warpsilent",
    "warpdoor",
    "warpteleport",
    "warpspinenter",
    "warpmossdeepgym",
    "warpwhitefade",
    "setdynamicwarp",
    "setescapewarp",
    "getplayerxy",
    "getpartysize",
    "additem",
    "removeitem",
    "checkitemspace",
    "checkitem",
    "applymovement",
    "applymovementat",
    "waitmovement",
    "waitmovementat",
    "removeobject",
    "removeobjectat",
    "addobject",
    "addobjectat",
    "setobjectxy",
    "showobjectat",
    "hideobjectat",
    "faceplayer",
    "turnobject",
    "trainerbattle",
    "checktrainerflag",
    "settrainerflag",
    "cleartrainerflag",
    "setobjectxyperm",
    "setobjectmovementtype",
    "waitmessage",
    "message",
    "closemessage",
    "lockall",
    "lock",
    "releaseall",
    "release",
    "waitbuttonpress",
    "yesnobox",
    "multichoice",
    "multichoicedefault",
    "givemon",
    "bufferspeciesname",
    "bufferleadmonspeciesname",
    "bufferpartymonnick",
    "bufferitemname",
    "buffermovename",
    "buffernumberstring",
    "bufferstdstring",
    "bufferstring",
    "bufferitemnameplural",
    "random",
    "addmoney",
    "removemoney",
    "checkmoney",
    "fadescreen",
    "fadescreenspeed",
    "fadescreenswapbuffers",
    "messageautoscroll",
    "messageinstant",
    "setrespawn",
    "checkplayergender",
    "setmetatile",
    "opendoor",
    "closedoor",
    "waitdooranim",
    "setwildbattle",
    "dowildbattle",
];

/// Commands after which execution never reaches the next instruction.
const TERMINATORS: [&str; 9] = [
    "end",
    "return",
    "goto",
    "gotostd",
    "returnram",
    "endram",
    "gotopostbattlescript",
    "gotobeatenscript",
    "vgoto",
];
/// Commands whose lowering can return `Flow::End` (besides an undecodable block end).
const END_OPS: [&str; 6] = [
    "end",
    "endram",
    "gotopostbattlescript",
    "gotobeatenscript",
    "vgoto",
    "trainerbattle",
];
/// `gStdScripts` labels, by `STD_*`/`MSGBOX_*` index (`data/event_scripts.s`).
const STD_LABELS: [&str; 11] = [
    "Std_ObtainItem",
    "Std_FindItem",
    "Std_MsgboxNPC",
    "Std_MsgboxSign",
    "Std_MsgboxDefault",
    "Std_MsgboxYesNo",
    "Std_MsgboxAutoclose",
    "Std_ObtainDecoration",
    "Std_RegisteredInMatchCall",
    "Std_MsgboxGetPoints",
    "Std_MsgboxPokenav",
];
const STD_OBTAIN_ITEM: u32 = 0;
const STD_FIND_ITEM: u32 = 1;

const VARS_START: u16 = 0x4000;
const VAR_RESULT: u16 = 0x800D;
/// `VAR_0x8000`/`VAR_0x8001`: the item and amount of `giveitem`/`finditem`.
const ITEM_VAR: u16 = 0x8000;
const ITEM_AMOUNT_VAR: u16 = 0x8001;
const LOCALID_PLAYER: u16 = 0xFF;
const WARP_ID_NONE: u8 = 0xFF;
const TRAINER_BATTLE_KINDS: [&str; 13] = [
    "TRAINER_BATTLE_SINGLE",
    "TRAINER_BATTLE_CONTINUE_SCRIPT_NO_MUSIC",
    "TRAINER_BATTLE_CONTINUE_SCRIPT",
    "TRAINER_BATTLE_SINGLE_NO_INTRO_TEXT",
    "TRAINER_BATTLE_DOUBLE",
    "TRAINER_BATTLE_REMATCH",
    "TRAINER_BATTLE_CONTINUE_SCRIPT_DOUBLE",
    "TRAINER_BATTLE_REMATCH_DOUBLE",
    "TRAINER_BATTLE_CONTINUE_SCRIPT_DOUBLE_NO_MUSIC",
    "TRAINER_BATTLE_PYRAMID",
    "TRAINER_BATTLE_SET_TRAINER_A",
    "TRAINER_BATTLE_SET_TRAINER_B",
    "TRAINER_BATTLE_HILL",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Terminated,
    Fallthrough(usize),
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    /// `goto`/`goto_if`/fallthrough: stays in the function or tail-calls.
    Jump,
    /// `call`/`call_if`/other script operands: always a function.
    Call,
}

struct Lab<'a> {
    name: &'a str,
    module: &'a str,
    root: bool,
    origin: NameOrigin,
    insns: &'a [IrInsn],
    end: End,
    /// Jump and call targets, in instruction order, then the fallthrough.
    edges: Vec<(Edge, usize)>,
}

struct Low<'a> {
    labs: Vec<Lab<'a>>,
    at: BTreeMap<&'a str, usize>,
    fns: BTreeSet<usize>,
    /// Function entry -> its blocks, entry first then label order.
    regions: BTreeMap<usize, Vec<usize>>,
    /// Movement name -> (module, movement).
    movements: BTreeMap<&'a str, (&'a str, &'a IrMovement)>,
    support: &'a Support,
    overrides: &'a BTreeMap<String, String>,
    can_end: BTreeSet<usize>,
    stats: Stats,
    trainer_scripts: BTreeSet<(String, String)>,
}

/// See the module doc.
pub fn lower(
    files: &[ScriptFile],
    index: &ScriptIndex,
    support: &Support,
    overrides: &BTreeMap<String, String>,
) -> Result<Lowered> {
    let mut low = Low::new(files, index, support, overrides)?;
    low.check_classes()?;
    low.build_regions();
    low.analyse();

    // Function sources and their direct data, by entry.
    let mut sources: BTreeMap<usize, String> = BTreeMap::new();
    let entries: Vec<usize> = low.fns.iter().copied().collect();
    for f in entries {
        let src = match overrides.get(low.labs[f].name) {
            Some(_) => {
                let n = low.labs[f].name;
                format!(
                    "pub async fn {n}(ctx: Ctx) -> Flow {{\n    crate::overrides::{n}(ctx).await;\n    Flow::Continue\n}}\n"
                )
            }
            None => low.emit_fn(f)?,
        };
        sources.insert(f, src);
    }

    let mut out = Lowered::default();
    let mut movement_src: BTreeMap<&str, String> = BTreeMap::new();
    for file in files {
        let mut body = String::new();
        let fns: Vec<usize> = low
            .fns
            .iter()
            .copied()
            .filter(|&f| low.labs[f].module == file.module)
            .collect();
        if !fns.is_empty() {
            body.push_str("use super::Flow;\n");
        }
        for f in fns {
            body.push('\n');
            body.push_str(&sources[&f]);
        }
        for m in &file.movements {
            let s = low.movement_static(m)?;
            body.push('\n');
            body.push_str(&s);
            movement_src.insert(&m.name, s);
        }
        out.modules.insert(file.module.clone(), body);
    }

    // Digests.
    let own: BTreeMap<usize, u64> = sources
        .iter()
        .map(|(&f, src)| {
            let mut h = Fnv::new();
            h.str(low.labs[f].name);
            h.str(src);
            if let Some(o) = overrides.get(low.labs[f].name) {
                h.str(o);
            }
            for t in low.texts_of(f) {
                h.str(t);
            }
            for m in low.movements_of(f) {
                h.str(m);
                h.str(movement_src.get(m).map_or("", |s| s.as_str()));
            }
            (f, h.0)
        })
        .collect();
    let graph: BTreeMap<usize, BTreeSet<usize>> =
        low.fns.iter().map(|&f| (f, low.fn_refs(f))).collect();
    for &f in &low.fns {
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
        rest.sort_by_key(|&g| low.labs[g].name);
        for g in rest {
            h.str(low.labs[g].name);
            h.u64(own[&g]);
        }
        out.script_digests.insert(low.labs[f].name.to_string(), h.0);
    }

    out.dispatch = low.dispatch();
    low.stats.fns = low.fns.len();
    low.stats.labels = low.labs.len();
    low.stats.movements = low.movements.len();
    let texts: BTreeSet<&str> = low.fns.iter().flat_map(|&f| low.texts_of(f)).collect();
    low.stats.texts = texts.len();
    let pct = |n: usize, d: usize| (n * 100).checked_div(d).map_or(100, |p| p as u32);
    let decomp_fns = low
        .fns
        .iter()
        .filter(|&&f| low.labs[f].origin == NameOrigin::Decomp)
        .count();
    let decomp_texts = texts.iter().filter(|t| !is_positional_text(t)).count();
    let decomp_movements = low
        .movements
        .values()
        .filter(|(_, m)| m.origin == NameOrigin::Decomp)
        .count();
    low.stats.named_decomp_pct = BTreeMap::from([
        ("functions".to_string(), pct(decomp_fns, low.fns.len())),
        (
            "movements".to_string(),
            pct(decomp_movements, low.movements.len()),
        ),
        ("texts".to_string(), pct(decomp_texts, texts.len())),
    ]);
    out.trainer_scripts = low.trainer_scripts.iter().cloned().collect();
    out.stats = low.stats;
    Ok(out)
}

/// Positional text names are `<Script>_T<n>` ([`crate::schema`] naming rules); the index has no
/// origin for texts.
fn is_positional_text(name: &str) -> bool {
    name.rsplit_once("_T")
        .is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// FNV-1a 64.
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn str(&mut self, s: &str) {
        self.bytes(&(s.len() as u64).to_le_bytes());
        self.bytes(s.as_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
}

/// Per-function emission state.
struct Fx<'a> {
    module: &'a str,
    /// Block -> arm index (entry is 0).
    local: BTreeMap<usize, usize>,
    looped: bool,
}

impl<'a> Low<'a> {
    fn new(
        files: &'a [ScriptFile],
        index: &'a ScriptIndex,
        support: &'a Support,
        overrides: &'a BTreeMap<String, String>,
    ) -> Result<Low<'a>> {
        let mut labs = Vec::new();
        let mut at = BTreeMap::new();
        let mut movements = BTreeMap::new();
        let mut modules = BTreeSet::new();
        for f in files {
            ensure!(is_ident(&f.module), "module name {:?}", f.module);
            ensure!(modules.insert(&f.module), "module {} given twice", f.module);
            for l in &f.labels {
                ensure!(is_ident(&l.name), "script name {:?}", l.name);
                ensure!(
                    at.insert(l.name.as_str(), labs.len()).is_none(),
                    "script {} defined twice",
                    l.name
                );
                let root = l.root.is_some() || index.scripts.get(&l.name).is_some_and(|e| e.root);
                labs.push(Lab {
                    name: &l.name,
                    module: &f.module,
                    root,
                    origin: l.origin,
                    insns: &l.insns,
                    end: End::Unknown,
                    edges: Vec::new(),
                });
            }
            for m in &f.movements {
                ensure!(is_ident(&m.name), "movement name {:?}", m.name);
                ensure!(
                    movements
                        .insert(m.name.as_str(), (f.module.as_str(), m))
                        .is_none(),
                    "movement {} defined twice",
                    m.name
                );
            }
        }
        for name in overrides.keys() {
            ensure!(
                at.contains_key(name.as_str()),
                "override {name} names no script of the IR"
            );
        }
        let file_labels = files.iter().flat_map(|f| &f.labels);
        for (lab, l) in labs.iter_mut().zip(file_labels) {
            let insns = &l.insns;
            if let Some(p) = insns
                .iter()
                .position(|i| TERMINATORS.contains(&i.op.as_str()))
            {
                ensure!(
                    p + 1 == insns.len(),
                    "{}: {} is followed by more instructions",
                    l.name,
                    insns[p].op
                );
            }
            let mut edges = Vec::new();
            for insn in insns {
                for a in &insn.args {
                    if let IrArg::Script(t) = a {
                        let &ti = at.get(t.as_str()).with_context(|| {
                            format!("{} refers to {t}, which is not in the IR", l.name)
                        })?;
                        let kind = match insn.op.as_str() {
                            "goto" | "goto_if" => Edge::Jump,
                            _ => Edge::Call,
                        };
                        edges.push((kind, ti));
                    }
                }
            }
            let end = if insns
                .last()
                .is_some_and(|i| TERMINATORS.contains(&i.op.as_str()))
            {
                End::Terminated
            } else if let Some(n) = &l.next {
                let &ni = at.get(n.as_str()).with_context(|| {
                    format!("{} falls through into {n}, which is not in the IR", l.name)
                })?;
                edges.push((Edge::Jump, ni));
                End::Fallthrough(ni)
            } else {
                End::Unknown
            };
            lab.end = end;
            lab.edges = edges;
        }
        Ok(Low {
            labs,
            at,
            fns: BTreeSet::new(),
            regions: BTreeMap::new(),
            movements,
            support,
            overrides,
            can_end: BTreeSet::new(),
            stats: Stats::default(),
            trainer_scripts: BTreeSet::new(),
        })
    }

    /// Every command and special the IR uses must be classified.
    fn check_classes(&self) -> Result<()> {
        let mut ops = BTreeSet::new();
        let mut specials = BTreeSet::new();
        for l in &self.labs {
            for i in l.insns {
                if !self.support.commands.contains_key(&i.op) {
                    ops.insert(i.op.clone());
                }
                if let Some(s) = special_of(i)
                    && !self.support.specials.contains_key(&s)
                {
                    specials.insert(s);
                }
            }
        }
        for (op, class) in &self.support.commands {
            ensure!(
                *class != Class::Native || NATIVE_COMMANDS.contains(&op.as_str()),
                "commands.ron: {op} is Native but has no lowering"
            );
        }
        if !ops.is_empty() || !specials.is_empty() {
            bail!(
                "unclassified script commands: [{}]; unclassified specials: [{}] \
                 (add them to content/build/support/{{commands,specials}}.ron)",
                ops.into_iter().collect::<Vec<_>>().join(", "),
                specials.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
        Ok(())
    }

    /// Picks the functions and the blocks each one owns.
    fn build_regions(&mut self) {
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
    fn fn_targets(&self, f: usize) -> Vec<(Edge, usize)> {
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

    /// Finds the functions that can return `Flow::End`.
    fn analyse(&mut self) {
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
    fn fn_refs(&self, f: usize) -> BTreeSet<usize> {
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

    fn texts_of(&self, f: usize) -> Vec<&'a str> {
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

    fn movements_of(&self, f: usize) -> Vec<&'a str> {
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

    fn class(&self, op: &str) -> Class {
        self.support
            .commands
            .get(op)
            .copied()
            .unwrap_or(Class::Unsupported)
    }

    fn count_class(&mut self, c: Class) {
        let k = match c {
            Class::Native => "Native",
            Class::Ignore => "Ignore",
            Class::Default(_) => "Default",
            Class::Unsupported => "Unsupported",
        };
        *self.stats.by_class.entry(k.to_string()).or_default() += 1;
    }

    fn count_report(&mut self, what: &str) {
        *self.stats.reports.entry(what.to_string()).or_default() += 1;
        *self.stats.unsupported.entry(what.to_string()).or_default() += 1;
    }

    // ---- emission ----

    fn fn_path(&self, from_module: &str, f: usize) -> String {
        let l = &self.labs[f];
        if l.module == from_module {
            l.name.to_string()
        } else {
            format!("super::{}::{}", l.module, l.name)
        }
    }

    fn movement_path(&self, from_module: &str, m: &str) -> Option<String> {
        let (module, mv) = self.movements.get(m)?;
        Some(if *module == from_module {
            mv.name.clone()
        } else {
            format!("super::{module}::{}", mv.name)
        })
    }

    fn emit_fn(&mut self, f: usize) -> Result<String> {
        let blocks = self.regions[&f].clone();
        let local: BTreeMap<usize, usize> =
            blocks.iter().enumerate().map(|(i, &b)| (b, i)).collect();
        let jumps_home = blocks.iter().any(|&b| {
            self.labs[b]
                .edges
                .iter()
                .any(|&(k, t)| k == Edge::Jump && t == f)
        });
        let looped = blocks.len() > 1 || jumps_home;
        let wild = blocks
            .iter()
            .any(|&b| self.labs[b].insns.iter().any(|i| i.op == "setwildbattle"));
        let name = self.labs[f].name;
        let fx = Fx {
            module: self.labs[f].module,
            local,
            looped,
        };
        let mut s = format!("pub async fn {name}(ctx: Ctx) -> Flow {{\n");
        if wild {
            s.push_str("    let mut wild: Option<(SpeciesId, u8)> = None;\n");
        }
        if looped {
            s.push_str("    let mut pc: u16 = 0;\n    loop {\n        match pc {\n");
            for (i, &b) in blocks.iter().enumerate() {
                writeln!(s, "            {i} => {{")?;
                writeln!(s, "                // {}", self.labs[b].name)?;
                for line in self.emit_block(b, &fx, wild)? {
                    writeln!(s, "                {line}")?;
                }
                s.push_str("            }\n");
            }
            s.push_str("            _ => return Flow::End,\n        }\n    }\n");
        } else {
            for line in self.emit_block(f, &fx, wild)? {
                writeln!(s, "    {line}")?;
            }
        }
        s.push_str("}\n");
        Ok(s)
    }

    fn emit_block(&mut self, b: usize, fx: &Fx, wild: bool) -> Result<Vec<String>> {
        let insns = self.labs[b].insns;
        let end = self.labs[b].end;
        let mut out = Vec::new();
        let mut st = BlockState {
            moved: false,
            word0: None,
            wild,
        };
        for (i, insn) in insns.iter().enumerate() {
            // `loadword 0, <text>` feeding a later `callstd MSGBOX_*` of the block is the
            // `msgbox` macro; it emits nothing itself.
            if insn.op == "loadword"
                && self.class("loadword") == Class::Native
                && let [a, IrArg::Text(t)] = &insn.args[..]
                && u8_lit(a) == Some(0)
                && is_ident(t)
            {
                self.count_class(Class::Native);
                st.word0 = Some(t.clone());
                let used = insns[i + 1..].iter().any(|n| {
                    matches!(
                        n.op.as_str(),
                        "callstd" | "gotostd" | "callstd_if" | "gotostd_if"
                    ) && n.args.last().and_then(std_index).is_some_and(is_msgbox)
                });
                if !used {
                    out.push(format!("// loadword 0, {t}"));
                }
                continue;
            }
            self.emit_insn(insn, b, fx, &mut st, &mut out)?;
        }
        match end {
            End::Terminated => {
                if !out
                    .last()
                    .is_some_and(|l| l.starts_with("return ") || l == "continue;")
                {
                    out.push("return Flow::End;".to_string());
                }
            }
            End::Fallthrough(t) => out.extend(self.jump(t, fx)),
            End::Unknown => {
                self.count_report("unknown opcode");
                out.push("ctx.report(Report::Unsupported(\"unknown opcode\")).await;".to_string());
                out.push("return Flow::End;".to_string());
            }
        }
        Ok(out)
    }

    /// Lines that continue at `t`: a local block or a tail call.
    fn jump(&self, t: usize, fx: &Fx) -> Vec<String> {
        match fx.local.get(&t) {
            Some(i) if fx.looped => vec![format!("pc = {i};"), "continue;".to_string()],
            _ => vec![format!(
                "return Box::pin({}(ctx.clone())).await;",
                self.fn_path(fx.module, t)
            )],
        }
    }

    fn call(&mut self, t: usize, fx: &Fx) -> Vec<String> {
        let call = format!(
            "Box::pin({}(ctx.clone())).await",
            self.fn_path(fx.module, t)
        );
        if self.can_end.contains(&t) {
            self.stats.end_in_callee += 1;
            vec![
                format!("if {call} == Flow::End {{"),
                "    return Flow::End;".to_string(),
                "}".to_string(),
            ]
        } else {
            vec![format!("{call};")]
        }
    }

    /// `ctx.report(Report::Unsupported(what))`, and `return Flow::End` after a terminator.
    fn report(&mut self, insn: &IrInsn, what: &str, out: &mut Vec<String>) {
        self.count_report(what);
        out.push(format!(
            "ctx.report(Report::Unsupported(\"{what}\")).await;{}",
            args_comment(&insn.args)
        ));
        if TERMINATORS.contains(&insn.op.as_str()) {
            out.push("return Flow::End;".to_string());
        }
    }

    fn emit_insn(
        &mut self,
        insn: &IrInsn,
        b: usize,
        fx: &Fx,
        st: &mut BlockState,
        out: &mut Vec<String>,
    ) -> Result<()> {
        let class = self.class(&insn.op);
        if class != Class::Native {
            self.count_class(class);
        }
        match class {
            Class::Native => {
                let before = out.len();
                if self.native(insn, b, fx, st, out)? {
                    self.count_class(Class::Native);
                } else {
                    // Operands that do not fit the arm: reported like an unsupported command.
                    out.truncate(before);
                    self.count_class(Class::Unsupported);
                    self.report(insn, &insn.op, out);
                }
            }
            Class::Ignore => {
                let args: Vec<String> = insn.args.iter().map(arg_str).collect();
                out.push(
                    format!("// {} {}", insn.op, args.join(", "))
                        .trim_end()
                        .to_string(),
                );
            }
            Class::Default(v) => {
                self.report(insn, &insn.op, out);
                let dest = match (insn.op.as_str(), insn.args.first()) {
                    ("checkcoins", Some(a)) => var_target(a),
                    _ => Some(var(VAR_RESULT)),
                };
                match dest {
                    Some(d) => out.push(format!("ctx.set_var({d}, {v}).await;")),
                    None => out.push(format!("// {} has no result var", insn.op)),
                }
            }
            Class::Unsupported => self.report(insn, &insn.op, out),
        }
        Ok(())
    }

    /// The lowering of a `Native` command; false when its operands do not fit.
    fn native(
        &mut self,
        insn: &IrInsn,
        b: usize,
        fx: &Fx,
        st: &mut BlockState,
        out: &mut Vec<String>,
    ) -> Result<bool> {
        use IrArg::*;
        let args = &insn.args[..];
        let op = insn.op.as_str();
        let r = var(VAR_RESULT);
        match (op, args) {
            ("nop" | "nop1", _) => out.push(format!("// {op}")),
            ("end" | "endram", _) => out.push("return Flow::End;".to_string()),
            ("return" | "returnram", _) => out.push("return Flow::Continue;".to_string()),
            ("call", [Script(t)]) => {
                let t = self.at[t.as_str()];
                out.extend(self.call(t, fx));
            }
            ("goto", [Script(t)]) => out.extend(self.jump(self.at[t.as_str()], fx)),
            ("goto_if" | "call_if", [c, Script(t)]) => {
                let Some(c) = condition(c) else {
                    return Ok(false);
                };
                let t = self.at[t.as_str()];
                let body = if op == "goto_if" {
                    self.jump(t, fx)
                } else {
                    self.call(t, fx)
                };
                out.push(format!("if {c} {{"));
                out.extend(body.into_iter().map(|l| format!("    {l}")));
                out.push("}".to_string());
            }
            ("callstd" | "gotostd", [n]) => {
                let Some(lines) = self.callstd(n, st) else {
                    return Ok(false);
                };
                out.extend(lines);
                if op == "gotostd" {
                    out.push("return Flow::Continue;".to_string());
                }
            }
            ("callstd_if" | "gotostd_if", [c, n]) => {
                let (Some(c), Some(lines)) = (condition(c), self.callstd(n, st)) else {
                    return Ok(false);
                };
                out.push(format!("if {c} {{"));
                out.extend(lines.into_iter().map(|l| format!("    {l}")));
                if op == "gotostd_if" {
                    out.push("    return Flow::Continue;".to_string());
                }
                out.push("}".to_string());
            }
            // `loadword` other than the msgbox text (see `emit_block`): only a register.
            ("loadword", _) => return Ok(false),
            ("setvar", [v, x]) => {
                let (Some(v), Some(x)) = (var_target(v), u16_expr(x, false)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({v}, {x}).await;"));
            }
            ("addvar" | "subvar", [v, x]) => {
                // Gen3 `addvar` adds the raw operand; `subvar` subtracts its `VarGet`.
                let (Some(v), Some(x)) = (var_target(v), u16_expr(x, op == "subvar")) else {
                    return Ok(false);
                };
                let f = if op == "addvar" {
                    "wrapping_add"
                } else {
                    "wrapping_sub"
                };
                out.push(format!(
                    "ctx.set_var({v}, ctx.var({v}).await.{f}({x})).await;"
                ));
            }
            ("copyvar", [a, c]) => {
                let (Some(a), Some(c)) = (var_target(a), var_target(c)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({a}, ctx.var({c}).await).await;"));
            }
            ("setorcopyvar", [a, x]) => {
                let (Some(a), Some(x)) = (var_target(a), u16_expr(x, true)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({a}, {x}).await;"));
            }
            ("compare_var_to_value", [v, x]) => {
                let (Some(v), Some(x)) = (var_target(v), u16_expr(x, false)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_cond(ctx.var({v}).await.cmp(&{x}));"));
            }
            ("compare_var_to_var", [a, c]) => {
                let (Some(a), Some(c)) = (var_target(a), var_target(c)) else {
                    return Ok(false);
                };
                out.push(format!(
                    "ctx.set_cond(ctx.var({a}).await.cmp(&ctx.var({c}).await));"
                ));
            }
            ("special", [s]) => return Ok(self.special(s, None, out)),
            ("specialvar", [v, s]) => {
                let Some(v) = var_target(v) else {
                    return Ok(false);
                };
                return Ok(self.special(s, Some(v), out));
            }
            ("waitstate", _) => out.push("ctx.wait_state().await;".to_string()),
            ("delay", [n]) => {
                let Some(n) = u16_expr(n, false) else {
                    return Ok(false);
                };
                out.push(format!("ctx.delay({n}).await;"));
            }
            ("setflag" | "clearflag", [f]) => {
                let Some(f) = flag_arg(f) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_flag({f}, {}).await;", op == "setflag"));
            }
            ("checkflag", [f]) => {
                let Some(f) = flag_arg(f) else {
                    return Ok(false);
                };
                out.push(format!(
                    "ctx.set_cond({});",
                    flag_cond(&format!("ctx.flag({f}).await"))
                ));
            }
            (
                "warp" | "warpsilent" | "warpdoor" | "warpteleport" | "warpspinenter"
                | "warpmossdeepgym" | "warpwhitefade",
                [m, w, x, y],
            ) => {
                let (Some(m), Some(w), Some(x), Some(y)) =
                    (map_arg(m), warp_id(w), coord(x), coord(y))
                else {
                    return Ok(false);
                };
                let note = if op == "warp" {
                    String::new()
                } else {
                    format!(" // {op}")
                };
                out.push(format!("ctx.warp({m}, {w}, {x}, {y}).await;{note}"));
            }
            ("setdynamicwarp" | "setescapewarp", [m, w, x, y]) => {
                let (Some(m), Some(w), Some(x), Some(y)) =
                    (map_arg(m), warp_id(w), coord(x), coord(y))
                else {
                    return Ok(false);
                };
                let slot = if op == "setdynamicwarp" {
                    "Dynamic"
                } else {
                    "Escape"
                };
                out.push(format!(
                    "set_warp(&ctx, WarpSlot::{slot}, {m}.0, {w}, {x}, {y}).await;"
                ));
            }
            ("getplayerxy", [a, c]) => {
                let (Some(a), Some(c)) = (var_target(a), var_target(c)) else {
                    return Ok(false);
                };
                out.push(format!(
                    "{{ let (x, y) = ctx.player_pos().await; ctx.set_var({a}, x as u16).await; ctx.set_var({c}, y as u16).await; }}"
                ));
            }
            ("getpartysize", _) => {
                out.push(format!("ctx.set_var({r}, ctx.party_size().await).await;"))
            }
            ("additem" | "removeitem" | "checkitemspace" | "checkitem", [i, q]) => {
                let (Some(i), Some(q)) = (u16_expr(i, true), u16_expr(q, true)) else {
                    return Ok(false);
                };
                let call = match op {
                    "additem" => format!("ctx.item_add(ItemId({i}), {q}).await"),
                    "removeitem" => format!("ctx.item_remove(ItemId({i}), {q}).await"),
                    "checkitemspace" => format!("ctx.item_space(ItemId({i}), {q}).await"),
                    _ => format!("check_item(&ctx, {i}, {q}).await"),
                };
                out.push(format!("ctx.set_var({r}, {call} as u16).await;"));
            }
            ("applymovement", [id, Movement(m)]) => {
                let (Some(id), Some(m)) = (local_id(id), self.movement_path(fx.module, m)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.move_obj({id}, &{m}).await;"));
                st.moved = true;
            }
            ("applymovementat", [id, Movement(m), at]) => {
                let (Some(lid), Some(m)) = (local_id(id), self.movement_path(fx.module, m)) else {
                    return Ok(false);
                };
                if !at_here(id, at, fx.module) {
                    return Ok(false);
                }
                out.push(format!(
                    "ctx.move_obj({lid}, &{m}).await; // applymovementat"
                ));
                st.moved = true;
            }
            ("waitmovement", [id]) => {
                let Some(line) = self.wait_move(id, st) else {
                    return Ok(false);
                };
                out.push(line);
            }
            ("waitmovementat", [id, at]) => {
                if !at_here(id, at, fx.module) {
                    return Ok(false);
                }
                let Some(line) = self.wait_move(id, st) else {
                    return Ok(false);
                };
                out.push(format!("{line} // waitmovementat"));
            }
            ("addobject" | "removeobject", [id]) => {
                let Some(id) = local_id(id) else {
                    return Ok(false);
                };
                let f = if op == "addobject" { "add" } else { "remove" };
                out.push(format!("ctx.{f}_object({id}).await;"));
            }
            ("addobjectat" | "removeobjectat" | "showobjectat" | "hideobjectat", [id, at]) => {
                let Some(lid) = local_id(id) else {
                    return Ok(false);
                };
                if !at_here(id, at, fx.module) {
                    return Ok(false);
                }
                let f = &op[..op.len() - "objectat".len()];
                out.push(format!("ctx.{f}_object({lid}).await; // {op}"));
            }
            ("setobjectxy" | "setobjectxyperm", [id, x, y]) => {
                let (Some(id), Some(x), Some(y)) = (local_id(id), coord(x), coord(y)) else {
                    return Ok(false);
                };
                let f = if op == "setobjectxy" {
                    "set_object_pos"
                } else {
                    "set_object_template_pos"
                };
                out.push(format!("ctx.{f}({id}, {x}, {y}).await;"));
            }
            ("faceplayer", _) => out.push("ctx.face_player().await;".to_string()),
            ("turnobject", [id, d]) => {
                let (Some(id), Some(d)) = (local_id(id), u8_lit(d).and_then(dir)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.turn({id}, {d}).await;"));
            }
            ("setobjectmovementtype", [id, t]) => {
                let (Some(id), Some(t)) = (local_id(id), movement_type(t)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_movement_type({id}, {t}).await;"));
            }
            ("trainerbattle", [k, tr, local, rest @ ..]) => {
                return self.trainerbattle(k, tr, local, rest, b, out);
            }
            ("checktrainerflag" | "settrainerflag" | "cleartrainerflag", [t]) => {
                let Some(t) = trainer_arg(t, true) else {
                    return Ok(false);
                };
                let f = format!("trainer_flag({t})");
                out.push(match op {
                    "checktrainerflag" => {
                        format!(
                            "ctx.set_cond({});",
                            flag_cond(&format!("ctx.flag({f}).await"))
                        )
                    }
                    "settrainerflag" => format!("ctx.set_flag({f}, true).await;"),
                    _ => format!("ctx.set_flag({f}, false).await;"),
                });
            }
            ("waitmessage", _) => out.push("ctx.wait_message().await;".to_string()),
            ("message" | "messageautoscroll" | "messageinstant", [Text(t)]) if is_ident(t) => {
                let note = if op == "message" {
                    String::new()
                } else {
                    format!(" // {op}")
                };
                out.push(format!("ctx.message({t}).await;{note}"));
            }
            ("closemessage", _) => out.push("ctx.close_message().await;".to_string()),
            ("lock", _) => out.push("ctx.lock().await;".to_string()),
            ("lockall", _) => out.push("ctx.lock_all().await;".to_string()),
            ("release", _) => out.push("ctx.release().await;".to_string()),
            ("releaseall", _) => out.push("ctx.release_all().await;".to_string()),
            ("waitbuttonpress", _) => out.push("ctx.wait_button().await;".to_string()),
            ("yesnobox", _) => out.push(format!(
                "ctx.set_var({r}, ctx.yes_no().await as u16).await;"
            )),
            ("multichoice", [x, y, list, ignore_b]) => {
                let (Some(x), Some(y), Some(list), Some(ignore_b)) =
                    (u8_lit(x), u8_lit(y), u8_lit(list), u8_lit(ignore_b))
                else {
                    return Ok(false);
                };
                out.push(format!(
                    "ctx.set_var({r}, multichoice(&ctx, {list}, {x}, {y}, 0, {}).await).await;",
                    ignore_b == 0
                ));
            }
            ("multichoicedefault", [x, y, list, default, ignore_b]) => {
                let (Some(x), Some(y), Some(list), Some(default), Some(ignore_b)) = (
                    u8_lit(x),
                    u8_lit(y),
                    u8_lit(list),
                    u8_lit(default),
                    u8_lit(ignore_b),
                ) else {
                    return Ok(false);
                };
                out.push(format!(
                    "ctx.set_var({r}, multichoice(&ctx, {list}, {x}, {y}, {default}, {}).await).await;",
                    ignore_b == 0
                ));
            }
            ("givemon", [s, level, item, ..]) => {
                let (Some(s), Some(level)) = (species_arg(s, true), u8_lit(level)) else {
                    return Ok(false);
                };
                let note = match u16_expr(item, true) {
                    Some(i) if i == "0" => String::new(),
                    Some(i) => format!(" // held item {i} not given"),
                    None => " // held item not given".to_string(),
                };
                out.push(format!(
                    "ctx.set_var({r}, ctx.give_mon({s}, {level}).await).await;{note}"
                ));
            }
            (
                "bufferspeciesname"
                | "bufferpartymonnick"
                | "bufferitemname"
                | "buffermovename"
                | "buffernumberstring"
                | "bufferstdstring"
                | "bufferstring"
                | "bufferitemnameplural",
                [slot, v, ..],
            ) => {
                let Some(slot) = u8_lit(slot) else {
                    return Ok(false);
                };
                let src = match op {
                    "bufferspeciesname" => species_arg(v, true).map(|s| format!("Species({s})")),
                    "bufferpartymonnick" => {
                        u16_expr(v, true).map(|n| format!("PartyNick({})", as_u8(&n)))
                    }
                    "bufferitemname" | "bufferitemnameplural" => {
                        u16_expr(v, true).map(|i| format!("Item(ItemId({i}))"))
                    }
                    "buffermovename" => move_arg(v).map(|m| format!("Move({m})")),
                    "buffernumberstring" => {
                        u16_expr(v, true).map(|n| format!("Number(u32::from({n}))"))
                    }
                    "bufferstdstring" => u16_expr(v, true).map(|n| format!("Std({})", as_u8(&n))),
                    _ => match v {
                        Text(t) if is_ident(t) => Some(format!("Text({t})")),
                        _ => None,
                    },
                };
                let Some(src) = src else {
                    return Ok(false);
                };
                let note = if op == "bufferitemnameplural" {
                    " // bufferitemnameplural: the singular name"
                } else {
                    ""
                };
                out.push(format!(
                    "buffer(&ctx, {slot}, BufferSrc::{src}).await;{note}"
                ));
            }
            ("bufferleadmonspeciesname", [slot]) => {
                let Some(slot) = u8_lit(slot) else {
                    return Ok(false);
                };
                out.push(format!(
                    "buffer(&ctx, {slot}, BufferSrc::LeadSpecies).await;"
                ));
            }
            ("random", [n]) => {
                let Some(n) = u16_expr(n, true) else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_var({r}, ctx.random({n}).await).await;"));
            }
            ("addmoney" | "removemoney" | "checkmoney", [n, ignore]) => {
                let (Some(n), Some(ignore)) = (u32_lit(n), u8_lit(ignore)) else {
                    return Ok(false);
                };
                if ignore != 0 {
                    out.push(format!("// {op} {n}, ignored"));
                    return Ok(true);
                }
                out.push(match op {
                    "addmoney" => format!("money_add(&ctx, {n}).await;"),
                    "removemoney" => format!("money_remove(&ctx, {n}).await;"),
                    _ => format!("ctx.set_var({r}, money_check(&ctx, {n}).await as u16).await;"),
                });
            }
            ("fadescreen" | "fadescreenspeed" | "fadescreenswapbuffers", [mode, ..]) => {
                let Some(mode) = u8_lit(mode) else {
                    return Ok(false);
                };
                let note = if op == "fadescreen" {
                    String::new()
                } else {
                    format!(" // {op}")
                };
                match mode {
                    0 => out.push(format!("ctx.fade(Fade::FromBlack).await;{note}")),
                    1 => out.push(format!("ctx.fade(Fade::ToBlack).await;{note}")),
                    // FADE_FROM_WHITE / FADE_TO_WHITE: black fades of the same length.
                    2 | 3 => {
                        let f = if mode == 2 { "FromBlack" } else { "ToBlack" };
                        out.push(format!("ctx.report(Report::Degraded(\"{op}\")).await;"));
                        out.push(format!("ctx.fade(Fade::{f}).await; // {op}, white"));
                    }
                    _ => return Ok(false),
                }
            }
            ("setrespawn", [h]) => {
                let h = match h {
                    Const { name, .. } if name.starts_with("HEAL_LOCATION_") && is_ident(name) => {
                        Some(name.clone())
                    }
                    Half(h) if *h < VARS_START => {
                        named(&names::HEAL_LOCATIONS, (*h).into()).map(str::to_string)
                    }
                    _ => u16_expr(h, true).map(|e| as_u8(&e)),
                };
                let Some(h) = h else {
                    return Ok(false);
                };
                out.push(format!("ctx.set_respawn({h}).await;"));
            }
            ("checkplayergender", _) => out.push(format!(
                "ctx.set_var({r}, check_player_gender(&ctx).await).await;"
            )),
            ("setmetatile", [x, y, m, imp]) => {
                let (Some(x), Some(y), Some(m), Some(imp)) =
                    (coord(x), coord(y), u16_expr(m, true), u16_expr(imp, true))
                else {
                    return Ok(false);
                };
                let blocked = match imp.as_str() {
                    "0" => "false".to_string(),
                    "1" => "true".to_string(),
                    _ => format!("{imp} != 0"),
                };
                out.push(format!("ctx.set_metatile({x}, {y}, {m}, {blocked}).await;"));
            }
            ("opendoor" | "closedoor", [x, y]) => {
                let (Some(x), Some(y)) = (coord(x), coord(y)) else {
                    return Ok(false);
                };
                out.push(format!("ctx.door({x}, {y}, {}).await;", op == "opendoor"));
            }
            ("waitdooranim", _) => out.push("ctx.wait_door().await;".to_string()),
            ("setwildbattle", [s, level, _item]) => {
                let (Some(s), Some(level)) = (species_arg(s, false), u8_lit(level)) else {
                    return Ok(false);
                };
                out.push(format!("wild = Some(({s}, {level}));"));
            }
            ("dowildbattle", _) => {
                if !st.wild {
                    return Ok(false);
                }
                out.push(
                    "if let Some((s, l)) = wild { ctx.battle_wild(s, l, false).await; }"
                        .to_string(),
                );
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// `callstd`/`gotostd` of `n` (without the `goto`'s return); `None` when `n` is no index.
    fn callstd(&mut self, n: &IrArg, st: &BlockState) -> Option<Vec<String>> {
        let n = std_index(n)?;
        let r = var(VAR_RESULT);
        let std_name = names::std_name(n as u8);
        self.stats.stds.insert(std_name.clone());
        if is_msgbox(n)
            && let Some(t) = &st.word0
            && let Some(k) = msgbox_kind(n)
        {
            return Some(vec![format!("msgbox(&ctx, {t}, MsgBox::{k}).await;")]);
        }
        let item = format!(
            "ctx.var({}).await, ctx.var({}).await",
            var(ITEM_VAR),
            var(ITEM_AMOUNT_VAR)
        );
        Some(vec![match n {
            STD_OBTAIN_ITEM => {
                format!("ctx.set_var({r}, give_item(&ctx, {item}).await as u16).await;")
            }
            STD_FIND_ITEM => {
                format!("ctx.set_var({r}, find_item(&ctx, {item}).await as u16).await;")
            }
            _ => {
                let c = std_const(n);
                format!("callstd(&ctx, {c}).await;")
            }
        }])
    }

    /// `special`/`specialvar` by the special's class; false when the operand is no special.
    fn special(&mut self, s: &IrArg, dest: Option<String>, out: &mut Vec<String>) -> bool {
        let Some((name, value)) = special_name(s) else {
            return false;
        };
        self.stats.specials.insert(name.clone());
        let class = self.support.specials.get(&name).copied();
        let Some(class) = class else {
            return false;
        };
        self.count_class(class);
        let c = if is_ident(&name) && !name.contains("0x") {
            name.clone()
        } else {
            format!("{value:#x}")
        };
        match class {
            Class::Native => out.push(match &dest {
                Some(v) => format!("ctx.set_var({v}, special(&ctx, {c}).await).await;"),
                None => format!("special(&ctx, {c}).await;"),
            }),
            Class::Ignore => out.push(format!("// special {name}")),
            Class::Default(v) => {
                self.count_report(&name);
                out.push(format!(
                    "ctx.report(Report::Unsupported(\"{name}\")).await;"
                ));
                let d = dest.unwrap_or_else(|| var(VAR_RESULT));
                out.push(format!("ctx.set_var({d}, {v}).await;"));
            }
            Class::Unsupported => {
                self.count_report(&name);
                out.push(format!(
                    "ctx.report(Report::Unsupported(\"{name}\")).await;"
                ));
            }
        }
        true
    }

    fn wait_move(&mut self, id: &IrArg, st: &mut BlockState) -> Option<String> {
        if u16_lit(id) == Some(0) {
            if !st.moved {
                self.stats.unresolved_waitmovement += 1;
            }
            return Some("ctx.wait_move(0).await; // the last mover".to_string());
        }
        let id = local_id(id)?;
        Some(format!("ctx.wait_move({id}).await;"))
    }

    /// `trainerbattle` (`data/scripts/trainer_battle.inc`): see the module doc.
    fn trainerbattle(
        &mut self,
        k: &IrArg,
        tr: &IrArg,
        local: &IrArg,
        rest: &[IrArg],
        b: usize,
        out: &mut Vec<String>,
    ) -> Result<bool> {
        let kind = match k {
            IrArg::Const { name, .. } => TRAINER_BATTLE_KINDS
                .iter()
                .position(|n| n == name)
                .map(|p| p as u8),
            a => u8_lit(a),
        };
        let (Some(kind), Some(trainer), Some(lid)) =
            (kind, trainer_arg(tr, false), local_id(local))
        else {
            return Ok(false);
        };
        let text = |a: &IrArg| match a {
            IrArg::Text(t) if is_ident(t) => Some(Some(t.clone())),
            IrArg::Raw(0) | IrArg::Word(0) => Some(None),
            _ => None,
        };
        let script = |a: &IrArg| match a {
            IrArg::Script(s) => Some(s.clone()),
            _ => None,
        };
        // (intro, lose, not_enough, event_script) by kind (`scripts::trainerbattle_args`).
        let parts = match (kind, rest) {
            (0 | 5 | 9..=12, [i, l]) => (text(i), text(l), Some(None), None),
            (1 | 2, [i, l, s]) => (text(i), text(l), Some(None), Some(script(s))),
            (3, [l]) => (Some(None), text(l), Some(None), None),
            (4 | 7, [i, l, n]) => (text(i), text(l), text(n), None),
            (6 | 8, [i, l, n, s]) => (text(i), text(l), text(n), Some(script(s))),
            _ => return Ok(false),
        };
        let (Some(intro), Some(Some(lose)), Some(not_enough)) = (parts.0, parts.1, parts.2) else {
            return Ok(false);
        };
        let event_script = match parts.3 {
            None => None,
            Some(Some(s)) => Some(s),
            Some(None) => return Ok(false),
        };
        // Gen3 `GetTrainerFlagFromScriptPointer` reads the trainer of a script that starts with
        // `trainerbattle`: record those scripts (functions, so they have a `ScriptId`).
        let starts = self.fns.contains(&b)
            && self.labs[b]
                .insns
                .first()
                .is_some_and(|i| i.op == "trainerbattle");
        let trainer_name = match tr {
            IrArg::Const { name, .. } => Some(name.as_str()),
            IrArg::Half(t) => names::TRAINERS.name((*t).into()),
            _ => None,
        };
        if starts && let Some(n) = trainer_name {
            self.trainer_scripts
                .insert((self.labs[b].name.to_string(), n.to_string()));
        }
        let opt = |o: &Option<String>| {
            o.as_ref()
                .map_or("None".to_string(), |t| format!("Some({t})"))
        };
        let ev = event_script.as_ref().map_or("None".to_string(), |s| {
            format!("Some(super::script_ids::{s})")
        });
        let music = !matches!(kind, 1 | 8);
        let flagged = !matches!(kind, 3 | 5 | 7 | 9..=12);
        if flagged {
            out.push(format!(
                "let beaten = ctx.flag(trainer_flag({trainer})).await;"
            ));
        }
        out.push(format!(
            "trainer_battle(&ctx, TrainerBattle {{ kind: {kind}, trainer: {trainer}.0, local_id: {lid}, intro: {}, lose: {lose}, not_enough: {}, event_script: {ev}, music: {music} }}).await;",
            opt(&intro),
            opt(&not_enough)
        ));
        match kind {
            5 | 7 => out.push("return Flow::End;".to_string()),
            _ if flagged => {
                out.push("if !beaten {".to_string());
                out.push("    return Flow::End;".to_string());
                out.push("}".to_string());
            }
            _ => {}
        }
        Ok(true)
    }

    fn movement_static(&mut self, m: &IrMovement) -> Result<String> {
        let mut s = String::from("[\n");
        let mut n = 0;
        for a in &m.actions {
            let short = a.strip_prefix("MOVEMENT_ACTION_").unwrap_or(a);
            if let Some(e) = step_of(a) {
                writeln!(s, "    {e}, // {short}")?;
                n += 1;
                continue;
            }
            match self.support.movements.get(a) {
                Some(MoveFallback::As(list)) => {
                    for t in list {
                        let e = step_of(t).with_context(|| {
                            format!("commands.ron: {a} -> {t}, which has no Step")
                        })?;
                        writeln!(
                            s,
                            "    {e}, // {short} (as {})",
                            t.strip_prefix("MOVEMENT_ACTION_").unwrap_or(t)
                        )?;
                        n += 1;
                    }
                }
                Some(MoveFallback::Delay(d)) => {
                    writeln!(s, "    Step::Delay({d}), // {short} (as a delay)")?;
                    n += 1;
                }
                Some(MoveFallback::Skip) => writeln!(s, "    // {short} (no effect here)")?,
                None => {
                    writeln!(s, "    Step::Delay(0), // {short} (unsupported)")?;
                    n += 1;
                    *self
                        .stats
                        .unknown_movement_actions
                        .entry(a.clone())
                        .or_default() += 1;
                }
            }
        }
        s.push(']');
        Ok(format!("pub static {}: [Step; {n}] = {s};\n", m.name))
    }

    fn dispatch(&self) -> String {
        let mut names: Vec<(&str, &str)> = self
            .fns
            .iter()
            .map(|&f| (self.labs[f].name, self.labs[f].module))
            .collect();
        names.sort();
        let n = names.len();
        let mut o = String::from(
            "// @generated by content/build.rs (extract::codegen::lower)\n\
             use crate::prelude::Box;\n\
             /// How a script function finished: Gen3 `return` (`Continue`) or `end` (`End`, which\n\
             /// also stops every caller).\n\
             #[derive(Clone, Copy, PartialEq, Eq, Debug)]\n\
             pub enum Flow {\n    Continue,\n    End,\n}\n",
        );
        let _ = writeln!(o, "pub const SCRIPT_COUNT: usize = {n};");
        let _ = write!(o, "pub static SCRIPT_NAMES: [&str; {n}] = [");
        for (name, _) in &names {
            let _ = write!(o, "\"{name}\",");
        }
        o.push_str("];\n");
        o.push_str(
            "/// `ScriptId` of every script, by name.\n#[allow(non_upper_case_globals)]\npub mod script_ids {\n    use engine::data::ScriptId;\n",
        );
        for (i, (name, _)) in names.iter().enumerate() {
            let _ = writeln!(o, "    pub const {name}: ScriptId = ScriptId({i});");
        }
        o.push_str("}\n");
        o.push_str(
            "async fn run(f: impl core::future::Future<Output = Flow>) {\n    f.await;\n}\n",
        );
        let _ = write!(
            o,
            "/// One thunk per `ScriptId`.\npub static SCRIPTS: [fn(engine::script::Ctx) -> engine::script::Script; {n}] = [\n"
        );
        for (name, module) in &names {
            let _ = writeln!(o, "    |ctx| Box::pin(run({module}::{name}(ctx))),");
        }
        o.push_str("];\n");
        o.push_str(
            "/// Instantiates script `id`.\npub fn script(id: u16, ctx: engine::script::Ctx) -> engine::script::Script {\n    match SCRIPTS.get(usize::from(id)) {\n        Some(f) => f(ctx),\n        None => Box::pin(async move { ctx.log(\"unknown script id\").await }),\n    }\n}\n",
        );
        o
    }
}

struct BlockState {
    /// An `applymovement` ran earlier in the block.
    moved: bool,
    /// The text of the block's last `loadword 0, <text>`.
    word0: Option<String>,
    /// The function has the `wild` local.
    wild: bool,
}

/// `// a, b, c` of an unsupported command's operands, or nothing.
fn args_comment(args: &[IrArg]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let v: Vec<String> = args.iter().map(arg_str).collect();
    format!(" // {}", v.join(", "))
}

fn arg_str(a: &IrArg) -> String {
    match a {
        IrArg::Byte(b) => b.to_string(),
        IrArg::Half(h) => format!("{h:#x}"),
        IrArg::Word(w) | IrArg::Raw(w) => format!("{w:#x}"),
        IrArg::Flag(s)
        | IrArg::Var(s)
        | IrArg::Script(s)
        | IrArg::Text(s)
        | IrArg::Movement(s)
        | IrArg::Braille(s) => s.clone(),
        IrArg::Const { name, .. } => name.clone(),
    }
}

/// The special a `special`/`specialvar` names, as its support-table key.
fn special_of(i: &IrInsn) -> Option<String> {
    let a = match (i.op.as_str(), &i.args[..]) {
        ("special", [s]) | ("specialvar", [_, s]) => s,
        _ => return None,
    };
    special_name(a).map(|(n, _)| n)
}

/// (`SPECIAL_*` name, number) of a special operand; unnamed ones are `SPECIAL_0x<n>`.
fn special_name(a: &IrArg) -> Option<(String, u32)> {
    match a {
        IrArg::Const { name, .. } => Some((name.clone(), const_value(name)?)),
        IrArg::Half(n) => Some((
            names::SPECIALS
                .name((*n).into())
                .map_or_else(|| format!("SPECIAL_{n:#x}"), str::to_string),
            (*n).into(),
        )),
        _ => None,
    }
}

/// The `gStdScripts` index of a `callstd` operand.
fn std_index(a: &IrArg) -> Option<u32> {
    match a {
        IrArg::Const { name, .. } => const_value(name).or_else(|| {
            name.strip_prefix("STD_")
                .and_then(|n| names::MSGBOX_TYPES.value(n))
        }),
        a => u8_lit(a).map(u32::from),
    }
}

fn is_msgbox(n: u32) -> bool {
    msgbox_kind(n).is_some()
}

fn msgbox_kind(n: u32) -> Option<&'static str> {
    Some(match names::MSGBOX_TYPES.name(n)? {
        "MSGBOX_NPC" => "Npc",
        "MSGBOX_SIGN" => "Sign",
        "MSGBOX_DEFAULT" => "Default",
        "MSGBOX_YESNO" => "YesNo",
        "MSGBOX_AUTOCLOSE" => "Autoclose",
        "MSGBOX_GETPOINTS" => "GetPoints",
        "MSGBOX_POKENAV" => "Pokenav",
        _ => return None,
    })
}

/// The prelude constant (`u8`) of std index `n`, or the number.
fn std_const(n: u32) -> String {
    match names::STD_SCRIPTS.name(n) {
        Some(s) if s.starts_with("STD_") && is_ident(s) => s.to_string(),
        Some(s) if is_ident(s) => format!("STD_{s}"),
        _ => n.to_string(),
    }
}

/// `sScriptConditionTable` row as a test of the comparison register.
fn condition(c: &IrArg) -> Option<&'static str> {
    Some(match u8_lit(c)? {
        0 => "ctx.cond() == Ordering::Less",
        1 => "ctx.cond() == Ordering::Equal",
        2 => "ctx.cond() == Ordering::Greater",
        3 => "ctx.cond() != Ordering::Greater",
        4 => "ctx.cond() != Ordering::Less",
        5 => "ctx.cond() != Ordering::Equal",
        _ => return None,
    })
}

fn flag_cond(e: &str) -> String {
    format!("if {e} {{ Ordering::Equal }} else {{ Ordering::Less }}")
}

/// Decomp tables by name prefix (a name may match several prefixes: `TRAINER_TYPE_*`).
const TABLES: [(&str, &names::Table); 17] = [
    ("FLAG_", &names::FLAGS),
    ("VAR_", &names::VARS),
    ("SPECIAL_", &names::SPECIALS),
    ("STD_", &names::STD_SCRIPTS),
    ("MSGBOX_", &names::MSGBOX_TYPES),
    ("ITEM_", &names::ITEMS),
    ("SPECIES_", &names::SPECIES),
    ("MOVE_", &names::MOVES),
    ("TRAINER_", &names::TRAINERS),
    ("OBJ_EVENT_GFX_", &names::OBJ_EVENT_GFX),
    ("MOVEMENT_TYPE_", &names::MOVEMENT_TYPES),
    ("MOVEMENT_ACTION_", &names::MOVEMENT_ACTIONS),
    ("MULTI_", &names::MULTICHOICE),
    ("STDSTRING_", &names::STD_STRINGS),
    ("HEAL_LOCATION_", &names::HEAL_LOCATIONS),
    ("MAPSEC_", &names::MAP_SECTIONS),
    ("LAYOUT_", &names::LAYOUTS),
];

/// The number a decomp constant name stands for: a table entry, or `<PREFIX>0x<hex>`.
fn const_value(name: &str) -> Option<u32> {
    if let Some(m) = names::map_by_constant(name) {
        return Some(u32::from(m.group) | u32::from(m.num) << 8);
    }
    for (prefix, table) in TABLES {
        let Some(rest) = name.strip_prefix(prefix) else {
            continue;
        };
        if let Some(h) = rest.strip_prefix("0x") {
            return u32::from_str_radix(h, 16).ok();
        }
        if let Some(v) = table.value(name) {
            return Some(v);
        }
    }
    match name {
        "TRUE" => Some(1),
        "FALSE" => Some(0),
        "LOCALID_PLAYER" => Some(LOCALID_PLAYER.into()),
        "LOCALID_NONE" => Some(0),
        _ => name
            .strip_prefix("0x")
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .or_else(|| name.parse().ok()),
    }
}

/// The prelude type of a named constant, when the prelude defines it (`None`: use the number).
fn const_type(name: &str) -> Option<&'static str> {
    if !is_ident(name) || name.contains("_0x") {
        return None;
    }
    if names::map_by_constant(name).is_some() {
        return Some("MapId");
    }
    let has = |t: &names::Table| t.value(name).is_some();
    Some(if has(&names::FLAGS) {
        "FlagId"
    } else if has(&names::VARS) {
        "VarId"
    } else if has(&names::SPECIES) {
        "SpeciesId"
    } else if has(&names::MOVES) {
        "MoveId"
    } else if has(&names::TRAINERS) {
        "TrainerId"
    } else if has(&names::SPECIALS) || has(&names::ITEMS) || has(&names::OBJ_EVENT_GFX) {
        "u16"
    } else if has(&names::STD_SCRIPTS) || has(&names::HEAL_LOCATIONS) {
        "u8"
    } else {
        return None;
    })
}

fn u16_lit(a: &IrArg) -> Option<u16> {
    match a {
        IrArg::Byte(b) => Some((*b).into()),
        IrArg::Half(h) => Some(*h),
        IrArg::Word(w) | IrArg::Raw(w) => u16::try_from(*w).ok(),
        IrArg::Const { name, .. } => const_value(name).and_then(|v| u16::try_from(v).ok()),
        _ => None,
    }
}

fn u8_lit(a: &IrArg) -> Option<u8> {
    u16_lit(a).and_then(|v| u8::try_from(v).ok())
}

fn u32_lit(a: &IrArg) -> Option<u32> {
    match a {
        IrArg::Word(w) | IrArg::Raw(w) => Some(*w),
        a => u16_lit(a).map(u32::from),
    }
}

/// A `u16` operand expression. `varget`: Gen3 reads it with `VarGet`, so a var (or a number
/// ≥ 0x4000) reads the var's value.
fn u16_expr(a: &IrArg, varget: bool) -> Option<String> {
    match a {
        IrArg::Var(v) if varget => Some(format!("ctx.var({}).await", var_name(v)?)),
        IrArg::Var(v) => Some(format!("{}.0", var_name(v)?)),
        IrArg::Flag(f) => Some(format!("{}.0", flag_name(f)?)),
        IrArg::Half(h) if varget && *h >= VARS_START => Some(format!("ctx.var({}).await", var(*h))),
        IrArg::Const { name, .. } => match const_type(name) {
            Some("u16") => Some(name.clone()),
            Some("u8") => Some(format!("u16::from({name})")),
            Some(_) if name.starts_with("VAR_") && varget => Some(format!("ctx.var({name}).await")),
            Some(_) => Some(format!("{name}.0")),
            None => u16_lit(a).map(|v| v.to_string()),
        },
        a => u16_lit(a).map(|v| v.to_string()),
    }
}

/// `expr` as a `u8` expression.
fn as_u8(e: &str) -> String {
    match e.parse::<u16>() {
        Ok(v) if v <= 0xFF => v.to_string(),
        _ => format!("{e} as u8"),
    }
}

fn var(v: u16) -> String {
    named(&names::VARS, v.into()).map_or_else(|| format!("VarId({v:#x})"), str::to_string)
}

fn flag(f: u16) -> String {
    named(&names::FLAGS, f.into()).map_or_else(|| format!("FlagId({f:#x})"), str::to_string)
}

/// A `VAR_*` name (or `VAR_0x<hex>`) as an expression.
fn var_name(v: &str) -> Option<String> {
    if let Some(h) = v.strip_prefix("VAR_0x") {
        return Some(var(u16::from_str_radix(h, 16).ok()?));
    }
    is_ident(v).then(|| v.to_string())
}

fn flag_name(f: &str) -> Option<String> {
    if let Some(h) = f.strip_prefix("FLAG_0x") {
        return Some(flag(u16::from_str_radix(h, 16).ok()?));
    }
    is_ident(f).then(|| f.to_string())
}

/// A var operand Gen3 reads with `GetVarPointer` (a destination or a `copyvar` source).
fn var_target(a: &IrArg) -> Option<String> {
    match a {
        IrArg::Var(v) => var_name(v),
        IrArg::Const { name, .. } if name.starts_with("VAR_") => var_name(name),
        IrArg::Half(h) if *h >= VARS_START => Some(var(*h)),
        _ => None,
    }
}

fn flag_arg(a: &IrArg) -> Option<String> {
    match a {
        IrArg::Flag(f) => flag_name(f),
        IrArg::Const { name, .. } if name.starts_with("FLAG_") => flag_name(name),
        IrArg::Half(h) => Some(flag(*h)),
        _ => None,
    }
}

/// A `VarGet` local id as a `u8` expression (`PLAYER` for `LOCALID_PLAYER`).
fn local_id(a: &IrArg) -> Option<String> {
    match a {
        IrArg::Var(_) => u16_expr(a, true).map(|e| format!("{e} as u8")),
        IrArg::Half(h) if *h >= VARS_START => u16_expr(a, true).map(|e| format!("{e} as u8")),
        a => match u16_lit(a)? {
            LOCALID_PLAYER => Some("PLAYER".to_string()),
            v => u8::try_from(v).ok().map(|v| v.to_string()),
        },
    }
}

/// A `VarGet` coordinate as an `i16` expression. 0xFFFF is the decomp's dummy coordinate
/// (`formatwarp`'s `-1` next to a warp id), not a var.
fn coord(a: &IrArg) -> Option<String> {
    match a {
        IrArg::Half(0xFFFF) => Some("-1".to_string()),
        IrArg::Var(_) => u16_expr(a, true).map(|e| format!("{e} as i16")),
        IrArg::Half(h) if *h >= VARS_START => u16_expr(a, true).map(|e| format!("{e} as i16")),
        a => u16_lit(a).map(|v| (v as i16).to_string()),
    }
}

/// A map operand (group byte, then num byte) as a `MapId` expression.
fn map_arg(a: &IrArg) -> Option<String> {
    if let IrArg::Const { name, .. } = a
        && (names::map_by_constant(name).is_some() || name == "MAP_DYNAMIC")
    {
        return Some(name.clone());
    }
    let m = u16_lit(a)?;
    let (group, num) = (m as u8, (m >> 8) as u8);
    if (group, num) == names::MAP_DYNAMIC {
        return Some("MAP_DYNAMIC".to_string());
    }
    Some(match names::map(group, num) {
        Some(m) => m.constant.to_string(),
        None => format!("MapId({:#x})", u16::from(group) << 8 | u16::from(num)),
    })
}

fn warp_id(a: &IrArg) -> Option<String> {
    Some(match u8_lit(a)? {
        WARP_ID_NONE => "None".to_string(),
        w => format!("Some({w})"),
    })
}

/// Whether an `*at` command acts on the current map: always for the player (Gen3 finds the
/// player on any map), else when its map operand is the module's map.
fn at_here(id: &IrArg, at: &IrArg, module: &str) -> bool {
    if u16_lit(id) == Some(LOCALID_PLAYER) {
        return true;
    }
    let constant = match at {
        IrArg::Const { name, .. } => Some(name.as_str()),
        a => u16_lit(a).and_then(|m| names::map(m as u8, (m >> 8) as u8).map(|m| m.constant)),
    };
    constant
        .and_then(names::map_by_constant)
        .is_some_and(|m| m.dir == module)
}

fn typed(a: &IrArg, varget: bool, prefix: &str, ty: &str) -> Option<String> {
    match a {
        IrArg::Const { name, .. } if name.starts_with(prefix) && const_type(name) == Some(ty) => {
            Some(name.clone())
        }
        IrArg::Half(h) if !(varget && *h >= VARS_START) => {
            let t = TABLES.iter().find(|(p, _)| *p == prefix)?.1;
            Some(
                match t.name((*h).into()).filter(|n| const_type(n) == Some(ty)) {
                    Some(n) => n.to_string(),
                    None => format!("{ty}({h})"),
                },
            )
        }
        a => u16_expr(a, varget).map(|e| format!("{ty}({e})")),
    }
}

fn species_arg(a: &IrArg, varget: bool) -> Option<String> {
    typed(a, varget, "SPECIES_", "SpeciesId")
}

fn move_arg(a: &IrArg) -> Option<String> {
    typed(a, true, "MOVE_", "MoveId")
}

fn trainer_arg(a: &IrArg, varget: bool) -> Option<String> {
    typed(a, varget, "TRAINER_", "TrainerId")
}

/// `DIR_SOUTH..DIR_EAST`.
fn dir(d: u8) -> Option<&'static str> {
    Some(match d {
        1 => "Dir::Down",
        2 => "Dir::Up",
        3 => "Dir::Left",
        4 => "Dir::Right",
        _ => return None,
    })
}

/// `MOVEMENT_TYPE_*` as an `engine::data::MovementType` expression: exact where the engine has
/// the type, else the nearest (as `content/build/classify.rs` does for templates).
fn movement_type(a: &IrArg) -> Option<String> {
    let name = match a {
        IrArg::Const { name, .. } => name.clone(),
        a => names::MOVEMENT_TYPES.name(u8_lit(a)?.into())?.to_string(),
    };
    let t = name.strip_prefix("MOVEMENT_TYPE_")?;
    let compass = |s: &str| {
        Some(match s {
            "DOWN" => "Dir::Down",
            "UP" => "Dir::Up",
            "LEFT" => "Dir::Left",
            "RIGHT" => "Dir::Right",
            _ => return None,
        })
    };
    if let Some(d) = t.strip_prefix("FACE_").and_then(compass) {
        return Some(format!("MovementType::Face({d})"));
    }
    for p in [
        "WALK_IN_PLACE_",
        "JOG_IN_PLACE_",
        "RUN_IN_PLACE_",
        "WALK_SLOWLY_IN_PLACE_",
    ] {
        if let Some(d) = t.strip_prefix(p).and_then(compass) {
            return Some(format!("MovementType::WalkInPlace({d})"));
        }
    }
    Some(
        match t {
            "LOOK_AROUND" => "LookAround",
            "WANDER_AROUND" => "Wander",
            "WANDER_UP_AND_DOWN" | "WANDER_DOWN_AND_UP" | "WALK_UP_AND_DOWN"
            | "WALK_DOWN_AND_UP" => "WanderVertical",
            "WANDER_LEFT_AND_RIGHT"
            | "WANDER_RIGHT_AND_LEFT"
            | "WALK_LEFT_AND_RIGHT"
            | "WALK_RIGHT_AND_LEFT" => "WanderHorizontal",
            "INVISIBLE" => "Invisible",
            _ if t.starts_with("WALK_SEQUENCE_") => "Wander",
            _ if t.starts_with("FACE_") || t.starts_with("ROTATE_") => "LookAround",
            _ => "Face(Dir::Down)",
        }
        .to_string(),
    )
    .map(|v| format!("MovementType::{v}"))
}

/// The `Step` of a `MOVEMENT_ACTION_*` name, if the engine has one.
fn step_of(action: &str) -> Option<String> {
    let v = names::MOVEMENT_ACTIONS.value(action)?;
    movement::step(u8::try_from(v).ok()?)
}
