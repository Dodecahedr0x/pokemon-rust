//! The script IR ([`ScriptFile`]) to async Rust (Milestone 8 Task D1), run by the content build.
//!
//! Submodules: `regions` (functions, their blocks, tail-call groups, the call graph), `emit`
//! (Rust source), `operands` (operand expressions), `digest` (script digests).
//!
//! **Functions.** A *block* is an [`crate::schema::IrLabel`]: its instructions, ended by a
//! terminator, by a fallthrough into `next`, or by nothing (an undecodable instruction). Roots
//! (`IrLabel::root`, or `root` in the index), `call`/`call_if` targets, other script operands
//! (`trainerbattle` continue scripts) and overridden scripts are functions; so is any block
//! reached by `goto`/fallthrough from more than one function, or from none. A function owns the
//! blocks it reaches by `goto` and fallthrough without entering another function; with more than
//! one block (or a jump back to its entry) it is a `loop { match pc { … } }`. Entering another
//! function by `goto` or fallthrough is a tail call: `return Box::pin(f(ctx.clone())).await;`.
//! Functions that tail-call each other (a strongly connected component of the tail-call graph)
//! form a *tail-call group*: one `pub async fn <Leader>__tail(ctx: Ctx, mut pc: u16) -> Flow`
//! holds all their blocks in one `loop { match pc }` (so a `goto` loop between them runs in
//! constant memory), and each member is `pub async fn <Name>(ctx: Ctx) -> Flow` entering it at
//! its first block. The leader is the first member in label order (modules sorted by name);
//! [`Stats::tail_call_groups`] counts the groups.
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
//!   [`Stats::unresolved_waitmovement`] counts, per block, the sites with no `applymovement`
//!   earlier in the same block (a mover set by a caller or an earlier block still counts).
//! - Operands the game reads with `VarGet` read the var at run time when they are a `Var` or a
//!   number ≥ 0x4000.
//! - `trainerbattle` is `if !trainer_battle(&ctx, TrainerBattle { … }).await { return
//!   Flow::End; }`. The prelude's `trainer_battle` owns every kind's flag, rematch and facility
//!   logic (Task I); `true` means "go on with the code after it" (Gen3 `gotopostbattlescript`,
//!   the already-beaten path), `false` that the script ends (the battle ran, then its
//!   `event_script` if any). The lowering has no per-kind control flow.
//! - `setwildbattle` + `dowildbattle` keep the pending battle in a local of the function.
//!
//! **Classes** ([`Support`]): a command or special missing from the tables fails. `Native`
//! commands have an arm here ([`NATIVE_COMMANDS`]). `Ignore` is a comment. `Default(v)` writes
//! `v` where the command writes its result, with no report (the default is documented in
//! `commands.ron`). `Unsupported` is `ctx.report(Report::Unsupported("<op>")).await` (logged,
//! an engine event); an `Unsupported` terminator then ends the script. A `Native` command whose
//! operands do not fit its arm is reported the same way. Only reports are counted in
//! [`Stats::reports`].
//!
//! **Output contract** ([`Lowered`]). Every path to another generated item is absolute:
//! `crate::scripts::<Module>::<Name>` for functions and movements of another module,
//! `crate::data::{Flow, script_ids}` for the dispatch (the content crate `include!`s it in
//! `content::data`).
//! - `modules[<Module>]` is the body of `pub mod <Module> { use crate::prelude::*; <body> }`
//!   inside `crate::scripts`, wrapped with the same `#[allow(…)]` as the legacy files. It starts
//!   with `use crate::data::Flow;` when it has functions, then `pub async fn <Name>(ctx: Ctx) ->
//!   Flow` per function (each tail-call group's loop after its leader) and
//!   `pub static <Movement>: [Step; N]` per movement.
//! - `dispatch` goes in `crate::data`: `Flow`, `SCRIPT_COUNT`, `SCRIPT_NAMES` (every function,
//!   sorted by name; the index is the `ScriptId`), `script_ids::<Name>`, the `SCRIPTS` thunk
//!   table and `script(id, ctx)`. It boxes with `alloc::boxed::Box` (the crate has
//!   `extern crate alloc`) and imports nothing.
//! - The prelude provides, besides the legacy contract (`msgbox`, `MsgBox`, `callstd`,
//!   `special`, `check_player_gender`, `trainer_flag`, `Ordering`, `Box`, the `Ctx` types and
//!   every constant): `Report`, `ItemId`, `MoveId`, `BufferSrc`, `WarpSlot`, `TrainerBattle`,
//!   `trainer_battle` (returning `bool`), `give_item`, `find_item`, `check_item`, `money_add`,
//!   `money_remove`, `money_check`, `buffer`, `multichoice` and `set_warp` (Wave 0 names), and
//!   `Ctx::{cond, set_cond, report, item_add, item_remove, item_space}`.
//! - An override (`overrides[<Name>]`) lowers to
//!   `pub async fn <Name>(ctx: Ctx) -> Flow { crate::overrides::<Name>(ctx).await; Flow::Continue }`:
//!   the override is `pub async fn <Name>(ctx: Ctx)` and always returns `Flow::Continue`, so its
//!   callers go on after it (an override cannot stop its caller).
//!
//! **Digests** ([`Lowered::script_digests`]), by function name: FNV-1a 64 over the function's
//! lowered source (with its tail-call group's loop, or its override's source), the texts it
//! names and the movements it uses, then the same for every function in its static call-graph
//! closure (calls, tail calls, other script operands, the std scripts its `callstd`s name when
//! the IR has them, and the script names an override's source mentions), in name order. The
//! order of the input files does not matter.
//!
//! **Undecodable code:** a block with no terminator and no `next` ends in an undecodable
//! instruction: `Report::Unsupported("unknown opcode")`, then `Flow::End`.

mod digest;
mod emit;
mod operands;
mod regions;

use super::{Class, Lowered, MoveFallback, Stats, Support, is_ident, named};
use crate::schema::{IrArg, IrInsn, IrMovement, NameOrigin, ScriptFile, ScriptIndex};
use crate::{movement, names};
use anyhow::{Context, Result, bail, ensure};
use operands::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Every command with a lowering arm. `commands.ron` may only mark these `Native`
/// (`content/tests/m8_d_commands.rs` checks both ways). That each one really lowers without a
/// report is `lower_every_native_command_without_reports` in `extract/tests/codegen.rs`: add a
/// use of a new entry there.
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
    /// Tail-call groups (mutually tail-calling functions): leader -> members, label order.
    groups: BTreeMap<usize, Vec<usize>>,
    /// Member -> its group's leader.
    group_of: BTreeMap<usize, usize>,
    stats: Stats,
    trainer_scripts: BTreeSet<(String, String)>,
}

/// Where the content crate puts the dispatch (`Flow`, `script_ids`, `SCRIPTS`).
const DATA: &str = "crate::data";
/// Where the content crate puts the script modules.
const SCRIPTS: &str = "crate::scripts";

/// The shared loop of the tail-call group led by `leader`.
fn group_name(leader: &str) -> String {
    format!("{leader}__tail")
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
    low.tail_call_groups()?;

    // Function sources, by entry, and the shared loop of each tail-call group, by leader.
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
    let mut group_src: BTreeMap<usize, String> = BTreeMap::new();
    let leaders: Vec<usize> = low.groups.keys().copied().collect();
    for g in leaders {
        let s = low.emit_group(g)?;
        group_src.insert(g, s);
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
            writeln!(body, "use {DATA}::Flow;")?;
        }
        for f in fns {
            body.push('\n');
            body.push_str(&sources[&f]);
            if let Some(g) = group_src.get(&f) {
                body.push('\n');
                body.push_str(g);
            }
        }
        for m in &file.movements {
            let s = low.movement_static(m)?;
            body.push('\n');
            body.push_str(&s);
            movement_src.insert(&m.name, s);
        }
        out.modules.insert(file.module.clone(), body);
    }

    out.script_digests = low.digests(&sources, &group_src, &movement_src);
    out.dispatch = low.dispatch();
    low.stats.fns = low.fns.len();
    low.stats.tail_call_groups = low.groups.len();
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
        // Module order, not `files` order: the label order picks block numbers and promotions.
        let mut files: Vec<&'a ScriptFile> = files.iter().collect();
        files.sort_by(|a, b| a.module.cmp(&b.module));
        for &f in &files {
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
        let file_labels = files.iter().flat_map(|f| f.labels.iter());
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
            groups: BTreeMap::new(),
            group_of: BTreeMap::new(),
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
    }
}
