//! Event script walk: follows a map's scripts through their control flow and collects the text,
//! movement and braille pointers they use.
//!
//! The operand layout of every opcode comes from `data/script_cmd_table.inc` and the macros in
//! `asm/macros/event.inc` (see `OPS`). Map script tables follow `asm/macros/map.inc`.

use crate::maps::{BgEventKind, MapHeader};
use crate::{ROM_BASE, Rom};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};

/// What a walk reached.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptScan {
    /// File offset of every decoded instruction. Keyed on the offset only: an instruction is
    /// decoded once, with the `setvaddress` state of the first path that reaches it. So a `v*`
    /// command reached both with and without a prior `setvaddress` gives an order-dependent
    /// result. `setvaddress` only appears in RAM (Mystery Event) scripts, never in map scripts.
    pub visited: BTreeSet<u32>,
    /// Strings shown by `message`/`msgbox` (`loadword 0`), `pokenavcall`, `bufferstring`, trainer texts, ...
    pub texts: BTreeSet<u32>,
    /// Movement data used by `applymovement` (recorded, not walked).
    pub movements: BTreeSet<u32>,
    /// Braille strings used by `braillemessage` (braille charset, not the text codec).
    pub braille: BTreeSet<u32>,
    /// Instructions the walk could not decode: offset -> opcode. Invalid opcodes, and
    /// `v*` commands with no `setvaddress` earlier on the same path.
    pub unknown: BTreeMap<u32, u8>,
}

/// `MAP_SCRIPT_ON_FRAME_TABLE` and `MAP_SCRIPT_ON_WARP_INTO_MAP_TABLE` point to
/// `map_script_2` lists (`var u16, value u16, script ptr`) ended by a 0 var.
const TABLE_TYPES: [u8; 2] = [2, 4];
/// Guard against runaway tables in corrupt data.
const MAX_TABLE: u32 = 256;

/// Every script a map can start: object, coord and bg (sign) event scripts, then the
/// map scripts (`map_script` entries, expanding the two table types). Deduplicated, in that order.
pub fn map_script_roots(rom: &Rom, header: &MapHeader) -> Result<Vec<u32>> {
    let ev = &header.events;
    let mut roots: Vec<u32> = ev.objects.iter().filter_map(|o| o.script).collect();
    roots.extend(ev.coords.iter().filter_map(|c| c.script));
    roots.extend(ev.bgs.iter().filter_map(|b| match b.kind {
        BgEventKind::Script(s) => s,
        _ => None,
    }));
    if let Some(table) = header.map_scripts {
        roots.extend(map_scripts(rom, table)?);
    }
    let mut seen = BTreeSet::new();
    roots.retain(|r| seen.insert(*r));
    Ok(roots)
}

/// Scripts of a `map_script` table at `table` (types 2 and 4 expanded). A table that reaches
/// `MAX_TABLE` entries without its terminator is an error.
fn map_scripts(rom: &Rom, table: u32) -> Result<Vec<u32>> {
    let mut roots = Vec::new();
    for i in 0.. {
        if i == MAX_TABLE {
            bail!("map script table at {table:#x} has no terminator within {MAX_TABLE} entries");
        }
        let at = table + i * 5;
        let kind = rom.u8(at)?;
        if kind == 0 {
            break;
        }
        let ptr = rom
            .ptr(at + 1)?
            .with_context(|| format!("null map script at {:#x}", at + 1))?;
        if !TABLE_TYPES.contains(&kind) {
            roots.push(ptr);
            continue;
        }
        for j in 0.. {
            if j == MAX_TABLE {
                bail!("map script list at {ptr:#x} has no terminator within {MAX_TABLE} entries");
            }
            let entry = ptr + j * 8;
            if rom.u16(entry)? == 0 {
                break;
            }
            roots.push(
                rom.ptr(entry + 4)?
                    .with_context(|| format!("null map script at {:#x}", entry + 4))?,
            );
        }
    }
    Ok(roots)
}

/// Operand kinds. Sizes: `B` 1, `H` 2 (also map ids), every other kind 4.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Arg {
    B,
    H,
    /// Opaque word (RAM pointer, native function, value, mart list, ...).
    W,
    /// Script pointer: walked.
    S,
    /// Text pointer.
    T,
    /// Movement data pointer.
    Mv,
    /// Braille text pointer.
    Br,
    /// `setvaddress` operand: virtual address of the command itself.
    VA,
    /// Virtual script pointer (relative to the last `setvaddress`).
    VS,
    /// Virtual text pointer.
    VT,
}
use Arg::*;

impl Arg {
    fn size(self) -> u32 {
        match self {
            B => 1,
            H => 2,
            _ => 4,
        }
    }
}

const OP_END: u8 = 0x02;
const OP_RETURN: u8 = 0x03;
const OP_GOTO: u8 = 0x05;
const OP_GOTO_STD: u8 = 0x08;
const OP_RETURNRAM: u8 = 0x0c;
const OP_ENDRAM: u8 = 0x0d;
const OP_LOAD_WORD: u8 = 0x0f;
const OP_TRAINERBATTLE: u8 = 0x5c;
const OP_GOTOPOSTBATTLESCRIPT: u8 = 0x5e;
const OP_GOTOBEATENSCRIPT: u8 = 0x5f;
const OP_VGOTO: u8 = 0xb9;

/// Opcodes after which execution never reaches the next instruction.
const TERMINATORS: [u8; 9] = [
    OP_END,
    OP_RETURN,
    OP_GOTO,
    OP_GOTO_STD,
    OP_RETURNRAM,
    OP_ENDRAM,
    OP_GOTOPOSTBATTLESCRIPT,
    OP_GOTOBEATENSCRIPT,
    OP_VGOTO,
];

/// Operands of `trainerbattle` after its fixed `type, trainer, local_id`, by type
/// (`TRAINER_BATTLE_*` in `include/constants/battle_setup.h`, layout from the `trainerbattle` macro).
fn trainerbattle_args(kind: u8) -> Option<&'static [Arg]> {
    Some(match kind {
        0 | 5 | 9..=12 => &[T, T], // single, rematch, pyramid, set trainer A/B, hill
        1 | 2 => &[T, T, S],       // continue script (no music)
        3 => &[T],                 // single, no intro text
        4 | 7 => &[T, T, T],       // double, rematch double
        6 | 8 => &[T, T, T, S],    // continue script double (no music)
        _ => return None,
    })
}

/// `gScriptCmdTable` order: (macro name, operands).
///
/// Opcodes aliased to `ScrCmd_nop1` (0xc7..=0xcc, 0xd0) use the operand layout of their
/// assembler macro, although the runtime handler reads no operands: this table describes the
/// bytes as assembled.
///
/// `callstd`/`gotostd` (and their `_if` forms) only carry a `gStdScripts` index. Those shared
/// std scripts are intentionally not walked: they are not part of any map.
const OPS: [(&str, &[Arg]); 0xe3] = [
    /* 0x00 */ ("nop", &[]),
    /* 0x01 */ ("nop1", &[]),
    /* 0x02 */ ("end", &[]),
    /* 0x03 */ ("return", &[]),
    /* 0x04 */ ("call", &[S]),
    /* 0x05 */ ("goto", &[S]),
    /* 0x06 */ ("goto_if", &[B, S]),
    /* 0x07 */ ("call_if", &[B, S]),
    /* 0x08 */ ("gotostd", &[B]),
    /* 0x09 */ ("callstd", &[B]),
    /* 0x0a */ ("gotostd_if", &[B, B]),
    /* 0x0b */ ("callstd_if", &[B, B]),
    /* 0x0c */ ("returnram", &[]),
    /* 0x0d */ ("endram", &[]),
    /* 0x0e */ ("setmysteryeventstatus", &[B]),
    /* 0x0f */ ("loadword", &[B, W]),
    /* 0x10 */ ("loadbyte", &[B, B]),
    /* 0x11 */ ("setptr", &[B, W]),
    /* 0x12 */ ("loadbytefromptr", &[B, W]),
    /* 0x13 */ ("setptrbyte", &[B, W]),
    /* 0x14 */ ("copylocal", &[B, B]),
    /* 0x15 */ ("copybyte", &[W, W]),
    /* 0x16 */ ("setvar", &[H, H]),
    /* 0x17 */ ("addvar", &[H, H]),
    /* 0x18 */ ("subvar", &[H, H]),
    /* 0x19 */ ("copyvar", &[H, H]),
    /* 0x1a */ ("setorcopyvar", &[H, H]),
    /* 0x1b */ ("compare_local_to_local", &[B, B]),
    /* 0x1c */ ("compare_local_to_value", &[B, B]),
    /* 0x1d */ ("compare_local_to_ptr", &[B, W]),
    /* 0x1e */ ("compare_ptr_to_local", &[W, B]),
    /* 0x1f */ ("compare_ptr_to_value", &[W, B]),
    /* 0x20 */ ("compare_ptr_to_ptr", &[W, W]),
    /* 0x21 */ ("compare_var_to_value", &[H, H]),
    /* 0x22 */ ("compare_var_to_var", &[H, H]),
    /* 0x23 */ ("callnative", &[W]),
    /* 0x24 */ ("gotonative", &[W]),
    /* 0x25 */ ("special", &[H]),
    /* 0x26 */ ("specialvar", &[H, H]),
    /* 0x27 */ ("waitstate", &[]),
    /* 0x28 */ ("delay", &[H]),
    /* 0x29 */ ("setflag", &[H]),
    /* 0x2a */ ("clearflag", &[H]),
    /* 0x2b */ ("checkflag", &[H]),
    /* 0x2c */ ("initclock", &[H, H]),
    /* 0x2d */ ("dotimebasedevents", &[]),
    /* 0x2e */ ("gettime", &[]),
    /* 0x2f */ ("playse", &[H]),
    /* 0x30 */ ("waitse", &[]),
    /* 0x31 */ ("playfanfare", &[H]),
    /* 0x32 */ ("waitfanfare", &[]),
    /* 0x33 */ ("playbgm", &[H, B]),
    /* 0x34 */ ("savebgm", &[H]),
    /* 0x35 */ ("fadedefaultbgm", &[]),
    /* 0x36 */ ("fadenewbgm", &[H]),
    /* 0x37 */ ("fadeoutbgm", &[B]),
    /* 0x38 */ ("fadeinbgm", &[B]),
    /* 0x39 */ ("warp", &[H, B, H, H]),
    /* 0x3a */ ("warpsilent", &[H, B, H, H]),
    /* 0x3b */ ("warpdoor", &[H, B, H, H]),
    /* 0x3c */ ("warphole", &[H]),
    /* 0x3d */ ("warpteleport", &[H, B, H, H]),
    /* 0x3e */ ("setwarp", &[H, B, H, H]),
    /* 0x3f */ ("setdynamicwarp", &[H, B, H, H]),
    /* 0x40 */ ("setdivewarp", &[H, B, H, H]),
    /* 0x41 */ ("setholewarp", &[H, B, H, H]),
    /* 0x42 */ ("getplayerxy", &[H, H]),
    /* 0x43 */ ("getpartysize", &[]),
    /* 0x44 */ ("additem", &[H, H]),
    /* 0x45 */ ("removeitem", &[H, H]),
    /* 0x46 */ ("checkitemspace", &[H, H]),
    /* 0x47 */ ("checkitem", &[H, H]),
    /* 0x48 */ ("checkitemtype", &[H]),
    /* 0x49 */ ("addpcitem", &[H, H]),
    /* 0x4a */ ("checkpcitem", &[H, H]),
    /* 0x4b */ ("adddecoration", &[H]),
    /* 0x4c */ ("removedecoration", &[H]),
    /* 0x4d */ ("checkdecor", &[H]),
    /* 0x4e */ ("checkdecorspace", &[H]),
    /* 0x4f */ ("applymovement", &[H, Mv]),
    /* 0x50 */ ("applymovementat", &[H, Mv, H]),
    /* 0x51 */ ("waitmovement", &[H]),
    /* 0x52 */ ("waitmovementat", &[H, H]),
    /* 0x53 */ ("removeobject", &[H]),
    /* 0x54 */ ("removeobjectat", &[H, H]),
    /* 0x55 */ ("addobject", &[H]),
    /* 0x56 */ ("addobjectat", &[H, H]),
    /* 0x57 */ ("setobjectxy", &[H, H, H]),
    /* 0x58 */ ("showobjectat", &[H, H]),
    /* 0x59 */ ("hideobjectat", &[H, H]),
    /* 0x5a */ ("faceplayer", &[]),
    /* 0x5b */ ("turnobject", &[H, B]),
    /* 0x5c */ ("trainerbattle", &[B, H, H]),
    /* 0x5d */ ("dotrainerbattle", &[]),
    /* 0x5e */ ("gotopostbattlescript", &[]),
    /* 0x5f */ ("gotobeatenscript", &[]),
    /* 0x60 */ ("checktrainerflag", &[H]),
    /* 0x61 */ ("settrainerflag", &[H]),
    /* 0x62 */ ("cleartrainerflag", &[H]),
    /* 0x63 */ ("setobjectxyperm", &[H, H, H]),
    /* 0x64 */ ("copyobjectxytoperm", &[H]),
    /* 0x65 */ ("setobjectmovementtype", &[H, B]),
    /* 0x66 */ ("waitmessage", &[]),
    /* 0x67 */ ("message", &[T]),
    /* 0x68 */ ("closemessage", &[]),
    /* 0x69 */ ("lockall", &[]),
    /* 0x6a */ ("lock", &[]),
    /* 0x6b */ ("releaseall", &[]),
    /* 0x6c */ ("release", &[]),
    /* 0x6d */ ("waitbuttonpress", &[]),
    /* 0x6e */ ("yesnobox", &[B, B]),
    /* 0x6f */ ("multichoice", &[B, B, B, B]),
    /* 0x70 */ ("multichoicedefault", &[B, B, B, B, B]),
    /* 0x71 */ ("multichoicegrid", &[B, B, B, B, B]),
    /* 0x72 */ ("drawbox", &[]),
    /* 0x73 */ ("erasebox", &[B, B, B, B]),
    /* 0x74 */ ("drawboxtext", &[B, B, B, B]),
    /* 0x75 */ ("showmonpic", &[H, B, B]),
    /* 0x76 */ ("hidemonpic", &[]),
    /* 0x77 */ ("showcontestpainting", &[B]),
    /* 0x78 */ ("braillemessage", &[Br]),
    /* 0x79 */ ("givemon", &[H, B, H, W, W, B]),
    /* 0x7a */ ("giveegg", &[H]),
    /* 0x7b */ ("setmonmove", &[B, B, H]),
    /* 0x7c */ ("checkpartymove", &[H]),
    /* 0x7d */ ("bufferspeciesname", &[B, H]),
    /* 0x7e */ ("bufferleadmonspeciesname", &[B]),
    /* 0x7f */ ("bufferpartymonnick", &[B, H]),
    /* 0x80 */ ("bufferitemname", &[B, H]),
    /* 0x81 */ ("bufferdecorationname", &[B, H]),
    /* 0x82 */ ("buffermovename", &[B, H]),
    /* 0x83 */ ("buffernumberstring", &[B, H]),
    /* 0x84 */ ("bufferstdstring", &[B, H]),
    /* 0x85 */ ("bufferstring", &[B, T]),
    /* 0x86 */ ("pokemart", &[W]),
    /* 0x87 */ ("pokemartdecoration", &[W]),
    /* 0x88 */ ("pokemartdecoration2", &[W]),
    /* 0x89 */ ("playslotmachine", &[H]),
    /* 0x8a */ ("setberrytree", &[B, B, B]),
    /* 0x8b */ ("choosecontestmon", &[]),
    /* 0x8c */ ("startcontest", &[]),
    /* 0x8d */ ("showcontestresults", &[]),
    /* 0x8e */ ("contestlinktransfer", &[]),
    /* 0x8f */ ("random", &[H]),
    /* 0x90 */ ("addmoney", &[W, B]),
    /* 0x91 */ ("removemoney", &[W, B]),
    /* 0x92 */ ("checkmoney", &[W, B]),
    /* 0x93 */ ("showmoneybox", &[B, B, B]),
    /* 0x94 */ ("hidemoneybox", &[B, B]),
    /* 0x95 */ ("updatemoneybox", &[B, B, B]),
    /* 0x96 */ ("getpokenewsactive", &[H]),
    /* 0x97 */ ("fadescreen", &[B]),
    /* 0x98 */ ("fadescreenspeed", &[B, B]),
    /* 0x99 */ ("setflashlevel", &[H]),
    /* 0x9a */ ("animateflash", &[B]),
    /* 0x9b */ ("messageautoscroll", &[T]),
    /* 0x9c */ ("dofieldeffect", &[H]),
    /* 0x9d */ ("setfieldeffectargument", &[B, H]),
    /* 0x9e */ ("waitfieldeffect", &[H]),
    /* 0x9f */ ("setrespawn", &[H]),
    /* 0xa0 */ ("checkplayergender", &[]),
    /* 0xa1 */ ("playmoncry", &[H, H]),
    /* 0xa2 */ ("setmetatile", &[H, H, H, H]),
    /* 0xa3 */ ("resetweather", &[]),
    /* 0xa4 */ ("setweather", &[H]),
    /* 0xa5 */ ("doweather", &[]),
    /* 0xa6 */ ("setstepcallback", &[B]),
    /* 0xa7 */ ("setmaplayoutindex", &[H]),
    /* 0xa8 */ ("setobjectsubpriority", &[H, H, B]),
    /* 0xa9 */ ("resetobjectsubpriority", &[H, H]),
    /* 0xaa */ ("createvobject", &[B, B, H, H, B, B]),
    /* 0xab */ ("turnvobject", &[B, B]),
    /* 0xac */ ("opendoor", &[H, H]),
    /* 0xad */ ("closedoor", &[H, H]),
    /* 0xae */ ("waitdooranim", &[]),
    /* 0xaf */ ("setdooropen", &[H, H]),
    /* 0xb0 */ ("setdoorclosed", &[H, H]),
    /* 0xb1 */ ("addelevmenuitem", &[B, H, H, H]),
    /* 0xb2 */ ("showelevmenu", &[]),
    /* 0xb3 */ ("checkcoins", &[H]),
    /* 0xb4 */ ("addcoins", &[H]),
    /* 0xb5 */ ("removecoins", &[H]),
    /* 0xb6 */ ("setwildbattle", &[H, B, H]),
    /* 0xb7 */ ("dowildbattle", &[]),
    /* 0xb8 */ ("setvaddress", &[VA]),
    /* 0xb9 */ ("vgoto", &[VS]),
    /* 0xba */ ("vcall", &[VS]),
    /* 0xbb */ ("vgoto_if", &[B, VS]),
    /* 0xbc */ ("vcall_if", &[B, VS]),
    /* 0xbd */ ("vmessage", &[VT]),
    /* 0xbe */ ("vbuffermessage", &[VT]),
    /* 0xbf */ ("vbufferstring", &[B, VT]),
    /* 0xc0 */ ("showcoinsbox", &[B, B]),
    /* 0xc1 */ ("hidecoinsbox", &[B, B]),
    /* 0xc2 */ ("updatecoinsbox", &[B, B]),
    /* 0xc3 */ ("incrementgamestat", &[B]),
    /* 0xc4 */ ("setescapewarp", &[H, B, H, H]),
    /* 0xc5 */ ("waitmoncry", &[]),
    /* 0xc6 */ ("bufferboxname", &[B, H]),
    /* 0xc7 */ ("textcolor", &[B]),
    /* 0xc8 */ ("loadhelp", &[W]),
    /* 0xc9 */ ("unloadhelp", &[]),
    /* 0xca */ ("signmsg", &[]),
    /* 0xcb */ ("normalmsg", &[]),
    /* 0xcc */ ("comparehiddenvar", &[B, W]),
    /* 0xcd */ ("setmodernfatefulencounter", &[H]),
    /* 0xce */ ("checkmodernfatefulencounter", &[H]),
    /* 0xcf */ ("trywondercardscript", &[]),
    /* 0xd0 */ ("setworldmapflag", &[H]),
    /* 0xd1 */ ("warpspinenter", &[H, B, H, H]),
    /* 0xd2 */ ("setmonmetlocation", &[H, B]),
    /* 0xd3 */ ("moverotatingtileobjects", &[H]),
    /* 0xd4 */ ("turnrotatingtileobjects", &[]),
    /* 0xd5 */ ("initrotatingtilepuzzle", &[H]),
    /* 0xd6 */ ("freerotatingtilepuzzle", &[]),
    /* 0xd7 */ ("warpmossdeepgym", &[H, B, H, H]),
    /* 0xd8 */ ("selectapproachingtrainer", &[]),
    /* 0xd9 */ ("lockfortrainer", &[]),
    /* 0xda */ ("closebraillemessage", &[]),
    /* 0xdb */ ("messageinstant", &[T]),
    /* 0xdc */ ("fadescreenswapbuffers", &[B]),
    /* 0xdd */ ("buffertrainerclassname", &[B, H]),
    /* 0xde */ ("buffertrainername", &[B, H]),
    /* 0xdf */ ("pokenavcall", &[T]),
    /* 0xe0 */ ("warpwhitefade", &[H, B, H, H]),
    /* 0xe1 */ ("buffercontestname", &[B, H]),
    /* 0xe2 */ ("bufferitemnameplural", &[B, H, H]),
];

/// The `script_cmd_table.inc` name of opcode `op`.
pub fn op_name(op: u8) -> Option<&'static str> {
    OPS.get(op as usize).map(|&(name, _)| name)
}

/// The opcode of the `script_cmd_table.inc` command `name`.
pub fn opcode(name: &str) -> Option<u8> {
    OPS.iter().position(|&(n, _)| n == name).map(|i| i as u8)
}

/// Whether execution never reaches the instruction after opcode `op`.
pub fn is_terminator_op(op: u8) -> bool {
    TERMINATORS.contains(&op)
}

/// A pointer operand: `None` for null and for RAM (runtime buffers such as `gStringVar4`).
fn target(rom: &Rom, at: u32) -> Result<Option<u32>> {
    let raw = rom.u32(at)?;
    if (0x0200_0000..ROM_BASE).contains(&raw) {
        return Ok(None);
    }
    rom.ptr(at)
}

/// One decoded operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    Byte(u8),
    Half(u16),
    /// Opaque word (RAM pointer, native function, value, mart list, ...).
    Word(u32),
    /// Script pointer (file offset).
    Script(u32),
    /// Text pointer (file offset); `None` for null or RAM. Also the word of `loadword 0, <text>`.
    Text(Option<u32>),
    /// Movement data pointer; `None` for null or RAM.
    Movement(Option<u32>),
    /// Braille text pointer; `None` for null or RAM.
    Braille(Option<u32>),
    /// `setvaddress` operand (raw).
    VAddress(u32),
    /// Virtual script pointer (raw, relative to the last `setvaddress`).
    VScript(u32),
    /// Virtual text pointer (raw).
    VText(u32),
}

/// One decoded script instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Insn {
    /// File offset of the opcode.
    pub off: u32,
    pub op: u8,
    /// Command name from `data/script_cmd_table.inc` (`goto_if`, not the `goto_if_eq` macro).
    pub name: &'static str,
    pub args: Vec<Operand>,
    /// Size in bytes, opcode included.
    pub len: u32,
}

impl Insn {
    /// Whether execution never reaches the next instruction.
    pub fn is_terminator(&self) -> bool {
        TERMINATORS.contains(&self.op)
    }
}

/// Decodes the instruction at `pc`: `None` for an invalid opcode or `trainerbattle` type.
/// A null script pointer is an error.
pub fn decode(rom: &Rom, pc: u32) -> Result<Option<Insn>> {
    let op = rom.u8(pc)?;
    let Some(&(name, args)) = OPS.get(op as usize) else {
        return Ok(None);
    };
    let extra: &[Arg] = if op == OP_TRAINERBATTLE {
        match trainerbattle_args(rom.u8(pc + 1)?) {
            Some(extra) => extra,
            None => return Ok(None),
        }
    } else {
        &[]
    };
    let ctx = || format!("{name} at {pc:#x}");
    let mut at = pc + 1;
    let mut out = Vec::with_capacity(args.len() + extra.len());
    for (i, &arg) in args.iter().chain(extra).enumerate() {
        out.push(match arg {
            B => Operand::Byte(rom.u8(at)?),
            H => Operand::Half(rom.u16(at)?),
            W if op == OP_LOAD_WORD && i == 1 && rom.u8(pc + 1)? == 0 => {
                Operand::Text(target(rom, at).with_context(ctx)?)
            }
            W => Operand::Word(rom.u32(at)?),
            S => Operand::Script(
                rom.ptr(at)
                    .with_context(ctx)?
                    .with_context(|| format!("null target: {}", ctx()))?,
            ),
            T => Operand::Text(target(rom, at).with_context(ctx)?),
            Mv => Operand::Movement(target(rom, at).with_context(ctx)?),
            Br => Operand::Braille(target(rom, at).with_context(ctx)?),
            VA => Operand::VAddress(rom.u32(at)?),
            VS => Operand::VScript(rom.u32(at)?),
            VT => Operand::VText(rom.u32(at)?),
        });
        at += arg.size();
    }
    Ok(Some(Insn {
        off: pc,
        op,
        name,
        args: out,
        len: at - pc,
    }))
}

/// Walks every script reachable from `roots` (file offsets).
pub fn walk(rom: &Rom, roots: &[u32]) -> Result<ScriptScan> {
    let mut scan = ScriptScan::default();
    // (offset, setvaddress delta on this path)
    let mut work: Vec<(u32, Option<u32>)> = roots.iter().rev().map(|&r| (r, None)).collect();
    while let Some((start, mut vdelta)) = work.pop() {
        let mut pc = start;
        while scan.visited.insert(pc) {
            let Some(insn) = decode(rom, pc)? else {
                scan.unknown.insert(pc, rom.u8(pc)?);
                break;
            };
            let mut stop = insn.is_terminator();
            for arg in &insn.args {
                match *arg {
                    Operand::Script(s) => work.push((s, vdelta)),
                    Operand::Text(Some(t)) => {
                        scan.texts.insert(t);
                    }
                    Operand::Movement(Some(m)) => {
                        scan.movements.insert(m);
                    }
                    Operand::Braille(Some(b)) => {
                        scan.braille.insert(b);
                    }
                    Operand::VAddress(raw) => vdelta = Some(raw.wrapping_sub(ROM_BASE + pc)),
                    Operand::VScript(raw) | Operand::VText(raw) => {
                        let Some(d) = vdelta else {
                            scan.unknown.insert(pc, insn.op);
                            stop = true;
                            break;
                        };
                        let addr = raw.wrapping_sub(d);
                        let off = addr.wrapping_sub(ROM_BASE);
                        if addr < ROM_BASE || off as usize >= rom.bytes().len() {
                            bail!(
                                "{} at {pc:#x}: virtual pointer {addr:#x} is outside the ROM",
                                insn.name
                            );
                        }
                        if matches!(arg, Operand::VScript(_)) {
                            work.push((off, vdelta));
                        } else {
                            scan.texts.insert(off);
                        }
                    }
                    _ => {}
                }
            }
            if stop {
                break;
            }
            pc += insn.len;
        }
    }
    Ok(scan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u32 = 0x100;

    /// A ROM whose bytes from `BASE` are `script`.
    fn rom(script: &[u8]) -> Rom {
        let mut b = vec![0u8; BASE as usize];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b.extend_from_slice(script);
        b.resize(0x400, 0xFF);
        Rom::from_bytes(b).unwrap()
    }

    fn p(off: u32) -> [u8; 4] {
        (ROM_BASE + off).to_le_bytes()
    }

    #[test]
    fn op_table_matches_known_sizes() {
        let size = |op: usize| OPS[op].1.iter().map(|a| a.size()).sum::<u32>();
        assert_eq!(OPS[0x0f].0, "loadword");
        assert_eq!(size(0x0f), 5);
        assert_eq!(size(0x39), 7); // warp: map, warp id, x, y
        assert_eq!(size(0x50), 8); // applymovementat
        assert_eq!(size(0x79), 14); // givemon
        assert_eq!(OPS[0xe2].0, "bufferitemnameplural");
    }

    #[test]
    fn follows_msgbox_calls_branches_and_movements() {
        // 0x100: loadword 0, text@0x180 ; callstd 2 ; call 0x120 ; goto_if 1, 0x130 ; end
        let mut s = vec![0x0f, 0];
        s.extend(p(0x180));
        s.extend([0x09, 2, 0x04]);
        s.extend(p(0x120));
        s.extend([0x06, 1]);
        s.extend(p(0x130));
        s.push(0x02);
        s.resize(0x20, 0);
        // 0x120: applymovement 1, 0x190 ; message 0x1a0 ; return
        s.extend([0x4f, 1, 0]);
        s.extend(p(0x190));
        s.push(0x67);
        s.extend(p(0x1a0));
        s.push(0x03);
        s.resize(0x30, 0);
        // 0x130: trainerbattle single: type 0, trainer, local id, intro, defeat ; goto 0x100
        s.extend([0x5c, 0, 1, 0, 0, 0]);
        s.extend(p(0x1b0));
        s.extend(p(0x1c0));
        s.push(0x05);
        s.extend(p(0x100));
        let r = rom(&s);
        let scan = walk(&r, &[BASE]).unwrap();
        assert_eq!(scan.texts, BTreeSet::from([0x180, 0x1a0, 0x1b0, 0x1c0]));
        assert_eq!(scan.movements, BTreeSet::from([0x190]));
        assert!(scan.unknown.is_empty());
        assert!(scan.visited.contains(&0x12c)); // `return`
        assert!(scan.visited.contains(&0x13e)); // `goto`
        assert!(!scan.visited.contains(&0x143)); // nothing after `goto`
    }

    /// A ROM of `len` bytes with `data` at `BASE`, zero-filled elsewhere.
    fn big_rom(data: &[u8], len: usize) -> Rom {
        let mut b = vec![0u8; len];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b[BASE as usize..BASE as usize + data.len()].copy_from_slice(data);
        Rom::from_bytes(b).unwrap()
    }

    #[test]
    fn map_script_tables_expand_and_must_terminate() {
        // 0x100: ON_TRANSITION -> 0x140 ; ON_FRAME_TABLE -> list at 0x120 ; end
        let mut s = vec![3];
        s.extend(p(0x140));
        s.push(2);
        s.extend(p(0x120));
        s.resize(0x20, 0);
        // 0x120: var 0x4050, value 1, script 0x150 ; end
        s.extend([0x50, 0x40, 1, 0]);
        s.extend(p(0x150));
        assert_eq!(
            map_scripts(&big_rom(&s, 0x400), BASE).unwrap(),
            [0x140, 0x150]
        );

        // An outer table with no terminator.
        let entry: Vec<u8> = std::iter::once(3).chain(p(0x140)).collect();
        let err = map_scripts(&big_rom(&entry.repeat(300), 0x1000), BASE).unwrap_err();
        assert!(format!("{err:#}").contains("table at 0x100"), "{err:#}");

        // A `map_script_2` list with no terminator.
        let mut s = vec![4];
        s.extend(p(0x108));
        s.resize(8, 0);
        let item: Vec<u8> = [0x50, 0x40, 1, 0].into_iter().chain(p(0x140)).collect();
        s.extend(item.repeat(300));
        let err = map_scripts(&big_rom(&s, 0x1000), BASE).unwrap_err();
        assert!(format!("{err:#}").contains("list at 0x108"), "{err:#}");
    }

    #[test]
    fn decode_gives_typed_operands_and_length() {
        // loadword 0, text@0x180 ; warp MAP 0.9, warp 0xff, x 0x4051, y 5 ; loadword 1, 0x1234
        let mut s = vec![0x0f, 0];
        s.extend(p(0x180));
        s.extend([
            0x39, 0, 9, 0xff, 0x51, 0x40, 5, 0, 0x0f, 1, 0x34, 0x12, 0, 0,
        ]);
        let r = rom(&s);
        let i = decode(&r, BASE).unwrap().unwrap();
        assert_eq!((i.name, i.len), ("loadword", 6));
        assert_eq!(i.args, [Operand::Byte(0), Operand::Text(Some(0x180))]);
        let w = decode(&r, BASE + 6).unwrap().unwrap();
        assert_eq!(
            w.args,
            [
                Operand::Half(0x0900),
                Operand::Byte(0xff),
                Operand::Half(0x4051),
                Operand::Half(5)
            ]
        );
        let l = decode(&r, BASE + 14).unwrap().unwrap();
        assert_eq!(l.args[1], Operand::Word(0x1234));
        assert!(!w.is_terminator());
        assert_eq!(decode(&rom(&[0xf0]), BASE).unwrap(), None);
    }

    #[test]
    fn ram_text_pointers_are_skipped() {
        let mut s = vec![0x67];
        s.extend(0x0202_1dc4u32.to_le_bytes());
        s.push(0x02);
        let scan = walk(&rom(&s), &[BASE]).unwrap();
        assert!(scan.texts.is_empty());
    }

    #[test]
    fn bad_opcode_is_unknown_and_bad_pointer_is_error() {
        let scan = walk(&rom(&[0x29, 1, 0, 0xf0]), &[BASE]).unwrap();
        assert_eq!(scan.unknown, BTreeMap::from([(0x103, 0xf0)]));
        let mut s = vec![0x05];
        s.extend(0x1234_5678u32.to_le_bytes());
        let err = walk(&rom(&s), &[BASE]).unwrap_err();
        assert!(format!("{err:#}").contains("0x100"), "{err:#}");
    }

    #[test]
    fn virtual_commands_need_setvaddress() {
        // setvaddress 0x02000000 (virtual address of this command) ; vgoto 0x02000010 -> 0x110
        let mut s = vec![0xb8];
        s.extend(0x0200_0000u32.to_le_bytes());
        s.push(0xb9);
        s.extend(0x0200_0010u32.to_le_bytes());
        s.resize(0x10, 0);
        s.extend([0xbd]); // 0x110: vmessage 0x02000020 -> 0x120
        s.extend(0x0200_0020u32.to_le_bytes());
        s.push(0x02);
        let scan = walk(&rom(&s), &[BASE]).unwrap();
        assert!(scan.unknown.is_empty());
        assert_eq!(scan.texts, BTreeSet::from([0x120]));
        let alone = walk(&rom(&s), &[BASE + 0x10]).unwrap();
        assert_eq!(alone.unknown, BTreeMap::from([(0x110, 0xbd)]));
    }
}
