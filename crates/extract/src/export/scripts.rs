//! The script export (Milestone 3 Task B; Milestone 8 Tasks B and D2): decodes the scripts
//! reachable from the exported maps' roots and writes the **script IR**: `scripts/<Module>.ron`
//! ([`ScriptFile`]), `scripts/index.ron` ([`ScriptIndex`]) and the texts keyed by their final
//! names, `text/<lang>.ron`. The content build lowers the IR to Rust
//! ([`crate::codegen::lower`], Task D2); the export writes no Rust.
//!
//! **Names** come from the decomp label transfer ([`labels::transfer`]); without the decomp
//! (`PKR_DECOMP` unset and no `../pokeemerald` next to the repo) every name is positional and
//! the export warns once. The map files name their scripts by the same final names
//! ([`export_scripts`] returns them by offset). **Modules** follow the decomp source of each label (`<Map>`,
//! `Common_<stem>`, `Text_<stem>`, `Common_event_scripts`); positional labels stay in the module
//! of the map of the first root reaching them.
//!
//! **Blocks.** A label's block runs from its offset to its terminator, an invalid opcode (written
//! as the pseudo-command `unknown` with the opcode byte), or the next label's offset, which is
//! then its `next`. Every instruction the walk decodes is in exactly one block.

use super::{ExportedMap, Out};
use crate::inc::Decomp;
use crate::labels::{self, Kind, MapInput, Transfer};
use crate::names::{self, Table};
use crate::schema::{
    Hex, IrArg, IrInsn, IrLabel, IrMovement, ScriptEntry, ScriptFile, ScriptIndex, TextFile,
};
use crate::scripts::{self, Operand};
use crate::{Rom, movement, text};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Everything the script export needs.
pub struct ScriptInput<'a> {
    /// The exported maps in export order.
    pub maps: &'a [ExportedMap],
}

/// The decomp checkout names come from: `$PKR_DECOMP`, else `../pokeemerald` next to the repo,
/// if it has a `charmap.txt`.
pub fn decomp_dir() -> Option<PathBuf> {
    let p = std::env::var_os("PKR_DECOMP")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../pokeemerald"));
    p.join("charmap.txt").exists().then_some(p)
}

/// The decomp at `dir`, read once per process (the export runs several times in tests).
pub fn load_decomp(dir: &Path) -> Result<Arc<Decomp>> {
    static CACHE: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Decomp>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(d) = cache.lock().unwrap().get(dir) {
        return Ok(d.clone());
    }
    let d = Arc::new(Decomp::load(dir)?);
    cache.lock().unwrap().insert(dir.to_path_buf(), d.clone());
    Ok(d)
}

/// Provenance key recording whether script names come from the decomp (`0x1`) or are all
/// positional (`0x0`).
pub const DECOMP_NAMES_KEY: &str = "scripts/decomp_names";

/// The script IR of some maps.
#[derive(Clone, Debug, Default)]
pub struct Ir {
    /// Module -> its file.
    pub files: BTreeMap<String, ScriptFile>,
    pub index: ScriptIndex,
    /// Final text name -> text in decomp `.string` syntax.
    pub texts: TextFile,
    pub transfer: Transfer,
}

impl Ir {
    /// Every instruction of every label.
    pub fn insn_count(&self) -> usize {
        self.files
            .values()
            .flat_map(|f| &f.labels)
            .map(|l| l.insns.len())
            .sum()
    }
}

/// Builds the IR of everything `maps`' roots reach, named by `decomp` when given.
pub fn build_ir(rom: &Rom, decomp: Option<&Decomp>, maps: &[MapInput]) -> Result<Ir> {
    let t = labels::transfer(rom, decomp, maps)?;
    let of_kind = |k: Kind| -> Vec<u32> {
        t.names
            .keys()
            .copied()
            .filter(|o| t.kinds.get(o) == Some(&k))
            .collect()
    };
    let scripts_at = of_kind(Kind::Script);
    let roots = t.root_kinds.clone();
    let module = |off: u32| -> Result<String> {
        t.modules
            .get(&off)
            .cloned()
            .with_context(|| format!("no module for {off:#x}"))
    };
    let name = |off: u32| -> String { t.names[&off].0.clone() };
    let res = Resolver { rom, t: &t };

    let mut ir = Ir::default();
    // (module, source rank, offset) -> label, sorted into source order below.
    let mut labels_by_module: BTreeMap<String, Vec<(Rank, IrLabel)>> = BTreeMap::new();
    for (i, &off) in scripts_at.iter().enumerate() {
        let next_label = scripts_at.get(i + 1).copied();
        let mut insns = Vec::new();
        let mut next = None;
        let mut pc = off;
        loop {
            if pc != off && Some(pc) == next_label {
                next = Some(name(pc));
                break;
            }
            let Some(insn) = scripts::decode(rom, pc)? else {
                insns.push(IrInsn {
                    op: "unknown".to_string(),
                    args: vec![IrArg::Byte(rom.u8(pc)?)],
                });
                break;
            };
            insns.push(res.insn(&insn)?);
            if insn.is_terminator() {
                break;
            }
            let end = pc + insn.len;
            if let Some(n) = next_label.filter(|&n| n > pc && n < end) {
                // Two decodings overlap: keep both, each label in its own block.
                println!(
                    "warning: label {} at {n:#x} is inside {} at {pc:#x}",
                    name(n),
                    insn.name
                );
            }
            pc = end;
        }
        let (n, origin) = t.names[&off].clone();
        let m = module(off)?;
        labels_by_module.entry(m).or_default().push((
            rank(decomp, &n, off),
            IrLabel {
                name: n.clone(),
                origin,
                offset: Hex(off),
                root: roots.get(&off).cloned(),
                insns,
                next,
            },
        ));
        ir.index.scripts.insert(
            n,
            ScriptEntry {
                module: module(off)?,
                root: roots.contains_key(&off),
                origin,
                offset: Hex(off),
            },
        );
    }
    let mut movements_by_module: BTreeMap<String, Vec<(Rank, IrMovement)>> = BTreeMap::new();
    for off in of_kind(Kind::Movement) {
        let (n, origin) = t.names[&off].clone();
        let actions = movement::decode(rom, off)
            .with_context(|| format!("movement {n}"))?
            .into_iter()
            .map(names::movement_action_name)
            .collect();
        ir.index.movements.insert(n.clone(), module(off)?);
        movements_by_module.entry(module(off)?).or_default().push((
            rank(decomp, &n, off),
            IrMovement {
                name: n,
                origin,
                actions,
            },
        ));
    }
    for off in of_kind(Kind::Text) {
        let n = name(off);
        let s = text::round_trip_at(rom, off).with_context(|| format!("text {n}"))?;
        ir.texts.insert(n.clone(), s);
        ir.index.texts.insert(n, module(off)?);
    }
    let modules: std::collections::BTreeSet<String> = labels_by_module
        .keys()
        .chain(movements_by_module.keys())
        .cloned()
        .collect();
    for m in modules {
        let mut labels = labels_by_module.remove(&m).unwrap_or_default();
        labels.sort_by_key(|(r, _)| *r);
        let mut movements = movements_by_module.remove(&m).unwrap_or_default();
        movements.sort_by_key(|(r, _)| *r);
        let source = decomp.and_then(|d| {
            d.files
                .iter()
                .find(|f| f.module == m)
                .map(|f| f.path.clone())
        });
        ir.files.insert(
            m.clone(),
            ScriptFile {
                module: m,
                source,
                labels: labels.into_iter().map(|(_, l)| l).collect(),
                movements: movements.into_iter().map(|(_, l)| l).collect(),
            },
        );
    }
    ir.transfer = t;
    Ok(ir)
}

/// Source order: a decomp label's (file, line), then positional names by offset.
type Rank = (usize, usize, u32);

/// The [`Rank`] of a label.
fn rank(decomp: Option<&Decomp>, name: &str, off: u32) -> Rank {
    match decomp.and_then(|d| d.label(name)) {
        Some(l) => (l.file, l.line, off),
        None => (usize::MAX, usize::MAX, off),
    }
}

/// Operand naming.
struct Resolver<'a> {
    rom: &'a Rom,
    t: &'a Transfer,
}

/// `VARS_START`: operands at or above it are read as vars by `VarGet`.
const VARS_START: u16 = 0x4000;

impl Resolver<'_> {
    fn name(&self, off: u32) -> Result<String> {
        Ok(self
            .t
            .names
            .get(&off)
            .with_context(|| format!("no name for {off:#x}"))?
            .0
            .clone())
    }

    fn konst(table: &str, t: &Table, v: u32) -> IrArg {
        IrArg::Const {
            table: table.to_string(),
            name: t.name_or_hex(v),
        }
    }

    /// A `VarGet` operand: a var at or above `VARS_START`, else a constant of `t`.
    fn var_or(table: &str, t: &Table, v: u16) -> IrArg {
        if v >= VARS_START {
            IrArg::Var(names::var_name(v))
        } else {
            Self::konst(table, t, v.into())
        }
    }

    fn insn(&self, insn: &scripts::Insn) -> Result<IrInsn> {
        let mut at = insn.off + 1;
        let mut args = Vec::with_capacity(insn.args.len());
        for (i, a) in insn.args.iter().enumerate() {
            let size = match a {
                Operand::Byte(_) => 1,
                Operand::Half(_) => 2,
                _ => 4,
            };
            let raw = || self.rom.u32(at);
            args.push(match *a {
                Operand::Script(s) => IrArg::Script(self.name(s)?),
                Operand::Text(Some(o)) => IrArg::Text(self.name(o)?),
                Operand::Movement(Some(o)) => IrArg::Movement(self.name(o)?),
                Operand::Braille(Some(o)) => IrArg::Braille(self.name(o)?),
                Operand::Text(None) | Operand::Movement(None) | Operand::Braille(None) => {
                    IrArg::Raw(raw()?)
                }
                Operand::VAddress(v) | Operand::VScript(v) | Operand::VText(v) => IrArg::Raw(v),
                Operand::Word(w) => IrArg::Word(w),
                Operand::Byte(b) => byte_arg(insn.name, i, b),
                Operand::Half(h) => half_arg(insn.name, i, h),
            });
            at += size;
        }
        Ok(IrInsn {
            op: insn.name.to_string(),
            args,
        })
    }
}

fn byte_arg(op: &str, i: usize, b: u8) -> IrArg {
    match (op, i) {
        ("callstd" | "gotostd", 0) | ("callstd_if" | "gotostd_if", 1) => std_arg(b),
        ("setobjectmovementtype", 1) => {
            Resolver::konst("MOVEMENT_TYPE", &names::MOVEMENT_TYPES, b.into())
        }
        ("multichoice" | "multichoicedefault" | "multichoicegrid", 2) | ("drawboxtext", 2) => {
            Resolver::konst("MULTI", &names::MULTICHOICE, b.into())
        }
        _ => IrArg::Byte(b),
    }
}

/// `callstd` index: `MSGBOX_*` for the message box types, else `STD_*`.
fn std_arg(b: u8) -> IrArg {
    if let Some(n) = names::MSGBOX_TYPES.name(b.into()) {
        return IrArg::Const {
            table: "MSGBOX".to_string(),
            name: n.to_string(),
        };
    }
    Resolver::konst("STD", &names::STD_SCRIPTS, b.into())
}

fn half_arg(op: &str, i: usize, h: u16) -> IrArg {
    use Resolver as R;
    let item = || R::var_or("ITEM", &names::ITEMS, h);
    let species = || R::var_or("SPECIES", &names::SPECIES, h);
    let mov = || R::var_or("MOVE", &names::MOVES, h);
    let var = || IrArg::Var(names::var_name(h));
    let var_or_half = || {
        if h >= VARS_START {
            var()
        } else {
            IrArg::Half(h)
        }
    };
    match (op, i) {
        ("setflag" | "clearflag" | "checkflag", 0) => IrArg::Flag(names::flag_name(h)),
        ("setvar" | "addvar" | "subvar", 0) => var(),
        ("addvar" | "subvar", 1) => var_or_half(),
        ("copyvar", _) | ("compare_var_to_var", _) => var(),
        ("setorcopyvar", 0) => var(),
        ("setorcopyvar", 1) => var_or_half(),
        ("compare_var_to_value", 0) => var(),
        ("special", 0) => R::konst("SPECIAL", &names::SPECIALS, h.into()),
        ("specialvar", 0) => var(),
        ("specialvar", 1) => R::konst("SPECIAL", &names::SPECIALS, h.into()),
        ("getplayerxy", _) | ("random", 0) | ("checkcoins", 0) => var(),
        (
            "additem" | "removeitem" | "checkitemspace" | "checkitem" | "addpcitem" | "checkpcitem",
            0,
        )
        | ("checkitemtype", 0)
        | ("bufferitemname" | "bufferitemnameplural", 1) => item(),
        (
            "additem" | "removeitem" | "checkitemspace" | "checkitem" | "addpcitem" | "checkpcitem",
            1,
        )
        | ("bufferitemnameplural", 2) => var_or_half(),
        ("givemon", 0)
        | ("giveegg", 0)
        | ("bufferspeciesname", 1)
        | ("showmonpic", 0)
        | ("playmoncry", 0)
        | ("setwildbattle", 0) => species(),
        ("givemon", 2) | ("setwildbattle", 2) => item(),
        ("setmonmove", 2) | ("checkpartymove", 0) | ("buffermovename", 1) => mov(),
        ("trainerbattle", 0) => R::konst("TRAINER", &names::TRAINERS, h.into()),
        ("checktrainerflag" | "settrainerflag" | "cleartrainerflag", 0) => {
            R::var_or("TRAINER", &names::TRAINERS, h)
        }
        ("buffertrainerclassname" | "buffertrainername", 1) => {
            R::var_or("TRAINER", &names::TRAINERS, h)
        }
        ("bufferstdstring", 1) => R::var_or("STDSTRING", &names::STD_STRINGS, h),
        ("setmaplayoutindex", 0) => R::var_or("LAYOUT", &names::LAYOUTS, h),
        (
            "warp" | "warpsilent" | "warpdoor" | "warphole" | "warpteleport" | "setwarp"
            | "setdynamicwarp" | "setdivewarp" | "setholewarp" | "setescapewarp" | "warpspinenter"
            | "warpmossdeepgym" | "warpwhitefade",
            0,
        )
        | (
            "applymovementat"
            | "showobjectat"
            | "hideobjectat"
            | "removeobjectat"
            | "addobjectat"
            | "setobjectsubpriority"
            | "resetobjectsubpriority",
            1,
        )
        | ("applymovementat", 2)
        | ("waitmovementat", 1) => map_arg(h),
        _ => IrArg::Half(h),
    }
}

/// A `map` operand (group byte, then num byte): its `MAP_*` constant.
fn map_arg(h: u16) -> IrArg {
    let (group, num) = ((h & 0xFF) as u8, (h >> 8) as u8);
    match names::map(group, num) {
        Some(m) => IrArg::Const {
            table: "MAP".to_string(),
            name: m.constant.to_string(),
        },
        None if (group, num) == names::MAP_DYNAMIC => IrArg::Const {
            table: "MAP".to_string(),
            name: "MAP_DYNAMIC".to_string(),
        },
        None => IrArg::Half(h),
    }
}

/// Decodes the scripts reachable from `input`'s roots and writes the IR: `scripts/<Module>.ron`,
/// `scripts/index.ron` and `text/<lang>.ron`. Returns the final name of every script by ROM
/// offset, for the map files.
pub fn export_scripts(
    rom: &Rom,
    input: &ScriptInput,
    o: &mut Out,
    provenance: &mut BTreeMap<String, Hex>,
) -> Result<BTreeMap<u32, String>> {
    let decomp = match decomp_dir() {
        Some(dir) => match load_decomp(&dir) {
            Ok(d) => Some(d),
            Err(e) => {
                println!(
                    "warning: cannot read the decomp at {}: {e:#}; script names are positional",
                    dir.display()
                );
                None
            }
        },
        None => {
            println!(
                "warning: PKR_DECOMP is not set and no ../pokeemerald was found: script names \
                 are positional"
            );
            None
        }
    };
    provenance.insert(DECOMP_NAMES_KEY.to_string(), Hex(decomp.is_some().into()));
    let inputs: Vec<MapInput> = input
        .maps
        .iter()
        .map(|m| MapInput {
            map: m.name.to_string(),
            header: m.header.clone(),
        })
        .collect();
    let ir = build_ir(rom, decomp.as_deref(), &inputs)?;
    for (module, file) in &ir.files {
        o.ron(&format!("scripts/{module}.ron"), file)?;
    }
    o.ron("scripts/index.ron", &ir.index)?;
    o.ron(&format!("text/{}.ron", rom.lang().code()), &ir.texts)?;
    println!("{}", ir_summary(&ir));
    let t = &ir.transfer;
    Ok(t.names
        .iter()
        .filter(|&(off, _)| t.kinds.get(off) == Some(&Kind::Script))
        .map(|(&off, (name, _))| (off, name.clone()))
        .collect())
}

/// One line: modules, labels, texts, movements, and the decomp-named share of each kind.
pub fn ir_summary(ir: &Ir) -> String {
    let t = &ir.transfer;
    let pct = |set| {
        let (n, of) = t.coverage(set);
        format!("{n}/{of}")
    };
    format!(
        "script IR: {} modules, {} labels, {} texts; decomp names: functions {}, texts {}, \
         movements {}; transfer: {} mismatched blocks, {} errors, {} collisions",
        ir.files.len(),
        ir.index.scripts.len(),
        ir.texts.len(),
        pct(&t.reached.scripts),
        pct(&t.reached.texts),
        pct(&t.reached.movements),
        t.mismatches.len(),
        t.errors.len(),
        t.collisions.len()
    )
}
