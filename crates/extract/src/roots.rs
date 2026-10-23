//! Script roots of a map named by the rules in [`crate::schema`]: `<Map>_Obj<local_id>`,
//! `<Map>_Coord<i>`, `<Map>_Bg<i>`, `<Map>_OnLoad`, `<Map>_OnTransition`, `<Map>_OnResume`,
//! `<Map>_OnReturnToField`, `<Map>_OnDiveWarp`, `<Map>_OnFrame<i>`, `<Map>_OnWarpInto<i>`.
//!
//! Map script tables follow `asm/macros/map.inc` (`map_script` type byte + pointer, ended by a 0
//! byte; types 2 and 4 point to `map_script_2` lists of `var, value, script` ended by a 0 var).

use crate::Rom;
use crate::maps::{BgEventKind, MapHeader};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

/// `MAP_SCRIPT_*` (`include/constants/map_scripts.h`).
const ON_LOAD: u8 = 1;
const ON_FRAME_TABLE: u8 = 2;
const ON_TRANSITION: u8 = 3;
const ON_WARP_INTO_MAP_TABLE: u8 = 4;
const ON_RESUME: u8 = 5;
const ON_DIVE_WARP: u8 = 6;
const ON_RETURN_TO_FIELD: u8 = 7;
/// Guard against runaway tables in corrupt data.
const MAX_TABLE: u32 = 256;

/// A `map_script_2` entry: run `script` when `var == value`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VarScript {
    pub var: u16,
    pub value: u16,
    pub script: u32,
}

/// A map's `map_script` table (offsets are ROM file offsets). When a type appears twice, the
/// game only ever runs the first (`MapHeaderGetScriptTable`), so later ones are ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapScriptTable {
    pub on_load: Option<u32>,
    pub on_transition: Option<u32>,
    pub on_resume: Option<u32>,
    pub on_return_to_field: Option<u32>,
    pub on_dive_warp: Option<u32>,
    pub on_frame: Vec<VarScript>,
    pub on_warp_into: Vec<VarScript>,
}

/// Reads the `map_script` table at `table` (a header's `map_scripts`).
pub fn read_map_scripts(rom: &Rom, table: u32) -> Result<MapScriptTable> {
    let mut t = MapScriptTable::default();
    let mut seen = Vec::new();
    for i in 0.. {
        if i == MAX_TABLE {
            bail!("map script table at {table:#x} has no terminator within {MAX_TABLE} entries");
        }
        let at = table + i * 5;
        let kind = rom.u8(at)?;
        if kind == 0 {
            break;
        }
        let ptr = rom.req_ptr(at + 1, "map script")?;
        if seen.contains(&kind) {
            continue;
        }
        seen.push(kind);
        match kind {
            ON_LOAD => t.on_load = Some(ptr),
            ON_TRANSITION => t.on_transition = Some(ptr),
            ON_RESUME => t.on_resume = Some(ptr),
            ON_DIVE_WARP => t.on_dive_warp = Some(ptr),
            ON_RETURN_TO_FIELD => t.on_return_to_field = Some(ptr),
            ON_FRAME_TABLE => t.on_frame = var_scripts(rom, ptr)?,
            ON_WARP_INTO_MAP_TABLE => t.on_warp_into = var_scripts(rom, ptr)?,
            _ => bail!("map script table at {table:#x}: unknown type {kind} at {at:#x}"),
        }
    }
    Ok(t)
}

fn var_scripts(rom: &Rom, list: u32) -> Result<Vec<VarScript>> {
    let mut out = Vec::new();
    for j in 0.. {
        if j == MAX_TABLE {
            bail!("map script list at {list:#x} has no terminator within {MAX_TABLE} entries");
        }
        let e = list + j * 8;
        let var = rom.u16(e)?;
        if var == 0 {
            break;
        }
        out.push(VarScript {
            var,
            value: rom.u16(e + 2)?,
            script: rom.req_ptr(e + 4, "map script")?,
        });
    }
    Ok(out)
}

/// Every script root of the map `map` (its decomp directory name), named and in naming-rule
/// order, with its ROM file offset. Several roots can share an offset; see [`canonical`].
pub fn named_roots(rom: &Rom, header: &MapHeader, map: &str) -> Result<Vec<(String, u32)>> {
    let ev = &header.events;
    let mut out = Vec::new();
    for o in &ev.objects {
        if let Some(s) = o.script {
            out.push((format!("{map}_Obj{}", o.local_id), s));
        }
    }
    for (i, c) in ev.coords.iter().enumerate() {
        if let Some(s) = c.script {
            out.push((format!("{map}_Coord{i}"), s));
        }
    }
    for (i, b) in ev.bgs.iter().enumerate() {
        if let BgEventKind::Script(Some(s)) = b.kind {
            out.push((format!("{map}_Bg{i}"), s));
        }
    }
    if let Some(table) = header.map_scripts {
        let t = read_map_scripts(rom, table).with_context(|| format!("{map}: map scripts"))?;
        for (kind, s) in [
            ("OnLoad", t.on_load),
            ("OnTransition", t.on_transition),
            ("OnResume", t.on_resume),
            ("OnReturnToField", t.on_return_to_field),
            ("OnDiveWarp", t.on_dive_warp),
        ] {
            if let Some(s) = s {
                out.push((format!("{map}_{kind}"), s));
            }
        }
        for (kind, list) in [("OnFrame", &t.on_frame), ("OnWarpInto", &t.on_warp_into)] {
            for (i, v) in list.iter().enumerate() {
                out.push((format!("{map}_{kind}{i}"), v.script));
            }
        }
    }
    Ok(out)
}

/// The `gStdScripts` labels (`data/event_scripts.s`), by `STD_*`/`MSGBOX_*` index.
pub const STD_SCRIPTS: [&str; 11] = [
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

/// The positional root name of `gStdScripts[i]` (used without the decomp).
pub fn std_root_name(i: usize) -> String {
    format!("StdScript{i}")
}

/// The ROM file offset of every `gStdScripts` entry, in [`STD_SCRIPTS`] order (Milestone 8 Task
/// D2: the native std scripts' texts, `gText_ObtainedTheItem` and friends, are named and
/// exported through them). Located by structure: `Std_MsgboxNPC` (`lock; faceplayer; message
/// NULL; waitmessage; waitbuttonpress; release; return`) is entry 2 of a table of 11 ROM
/// pointers, each to a decodable script. `None` when no such table is found (a synthetic ROM).
pub fn std_scripts(rom: &Rom) -> Result<Option<Vec<u32>>> {
    let op = |n: &str| crate::scripts::opcode(n).context(n.to_string());
    let mut npc = vec![op("lock")?, op("faceplayer")?, op("message")?, 0, 0, 0, 0];
    for n in ["waitmessage", "waitbuttonpress", "release", "return"] {
        npc.push(op(n)?);
    }
    let mut found = Vec::new();
    for at in rom.find_all(&npc) {
        for p in rom.pointers_to(at) {
            let Some(start) = p.checked_sub(8) else {
                continue;
            };
            let entries: Option<Vec<u32>> = (0..STD_SCRIPTS.len() as u32)
                .map(|i| rom.ptr(start + 4 * i).ok().flatten())
                .collect();
            let Some(entries) = entries else { continue };
            if entries
                .iter()
                .all(|&e| matches!(crate::scripts::decode(rom, e), Ok(Some(_))))
            {
                found.push(entries);
            }
        }
    }
    found.sort();
    found.dedup();
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        n => bail!("{n} candidate gStdScripts tables"),
    }
}

/// The field scripts the game's C code starts (no map, object or std table points at them),
/// by decomp label: `GetInteractedWaterScript`'s `EventScript_UseSurf` (Wave C), then the
/// Match Call registrations `TryStartStepCountScript` (`field_control_avatar.c`) starts
/// ([`STEP_CALL_SCRIPTS`], Wave E), then Waterfall's and Dive's (Wave F: [`water_scripts`]).
pub const FIELD_SCRIPTS: [&str; 10] = [
    "EventScript_UseSurf",
    "RustboroCity_Gym_EventScript_RegisterRoxanne",
    "MauvilleCity_EventScript_RegisterWallyCall",
    "Route119_EventScript_ScottWonAtFortreeGymCall",
    "LittlerootTown_ProfessorBirchsLab_EventScript_ScottAboardSSTidalCall",
    "MossdeepCity_SpaceCenter_2F_EventScript_RivalRayquazaCall",
    "EventScript_UseWaterfall",
    "EventScript_CannotUseWaterfall",
    "EventScript_UseDive",
    "EventScript_UseDiveUnderwater",
];

/// [`FIELD_SCRIPTS`]`[1..]`'s signatures: the script opens with `lockall` then the named
/// command, and its straight line (no jump followed) reaches `clearflag <flag>`, the flag
/// whose `ShouldDo*Call` (`field_specials.c`) starts it: `FLAG_ENABLE_ROXANNE_FIRST_CALL`,
/// `FLAG_ENABLE_FIRST_WALLY_POKENAV_CALL`, `FLAG_SCOTT_CALL_FORTREE_GYM`,
/// `FLAG_SCOTT_CALL_BATTLE_FRONTIER`, `FLAG_DEFEATED_MAGMA_SPACE_CENTER`.
pub const STEP_CALL_SCRIPTS: [(&str, u16); 5] = [
    ("pokenavcall", 0x80),
    ("pokenavcall", 0x88),
    ("pokenavcall", 0x8A),
    ("pokenavcall", 0x72),
    ("checkplayergender", 0x75),
];

/// How far before its `clearflag` a [`STEP_CALL_SCRIPTS`] script may start.
const STEP_CALL_REACH: u32 = 96;

/// The positional root name of [`FIELD_SCRIPTS`]`[i]` (used without the decomp).
pub fn field_root_name(i: usize) -> String {
    format!("FieldScript{i}")
}

/// Gen3 `MOVE_SURF`, `MOVE_WATERFALL`, `MOVE_DIVE`.
const MOVE_SURF: u16 = 57;
const MOVE_WATERFALL: u16 = 127;
const MOVE_DIVE: u16 = 291;
/// Gen3 `PARTY_SIZE`, `VAR_RESULT`.
const PARTY_SIZE: u16 = 6;
const VAR_RESULT: u16 = 0x800D;

/// The ROM file offset of each [`FIELD_SCRIPTS`] entry, `None` where it is not found (a
/// synthetic ROM). Located by structure: `EventScript_UseSurf` opens with `checkpartymove
/// MOVE_SURF; goto_if_eq VAR_RESULT, PARTY_SIZE, ...` (`compare_var_to_value` then `goto_if 1`),
/// found once.
pub fn field_scripts(rom: &Rom) -> Result<Vec<Option<u32>>> {
    let op = |n: &str| crate::scripts::opcode(n).context(n.to_string());
    // `checkpartymove <mv>; goto_if_eq VAR_RESULT, PARTY_SIZE, ...`, decoding, in ROM order.
    let check = |mv: u16| -> Result<Vec<u32>> {
        let mut sig = vec![op("checkpartymove")?];
        sig.extend(mv.to_le_bytes());
        sig.push(op("compare_var_to_value")?);
        sig.extend(VAR_RESULT.to_le_bytes());
        sig.extend(PARTY_SIZE.to_le_bytes());
        sig.extend([op("goto_if")?, 1]);
        let mut found: Vec<u32> = (rom.find_all(&sig).into_iter())
            .filter(|&at| matches!(crate::scripts::decode(rom, at), Ok(Some(_))))
            .collect();
        found.sort();
        Ok(found)
    };
    let mut out = vec![match check(MOVE_SURF)?.as_slice() {
        [at] => Some(*at),
        _ => None,
    }];
    for (second, flag) in STEP_CALL_SCRIPTS {
        out.push(step_call_script(rom, op(second)?, flag)?);
    }
    out.extend(water_scripts(
        rom,
        &check(MOVE_WATERFALL)?,
        &check(MOVE_DIVE)?,
    )?);
    Ok(out)
}

/// [`FIELD_SCRIPTS`]`[6..]`' offsets from their party checks (`waterfall`: the one
/// `checkpartymove MOVE_WATERFALL`, `dive`: the two `checkpartymove MOVE_DIVE`, in ROM order),
/// each script opening with `lockall` one byte before its check:
/// - `EventScript_UseWaterfall`, and `EventScript_CannotUseWaterfall`: the `lockall` just
///   before the target of its `goto_if` (`EventScript_CantWaterfall`);
/// - `EventScript_UseDive`, then `EventScript_UseDiveUnderwater` (decomp order).
fn water_scripts(rom: &Rom, waterfall: &[u32], dive: &[u32]) -> Result<[Option<u32>; 4]> {
    let lockall = crate::scripts::opcode("lockall").context("lockall")?;
    let opened = |at: u32| -> Result<Option<u32>> {
        Ok((at > 0 && rom.u8(at - 1)? == lockall).then(|| at - 1))
    };
    let mut out = [None; 4];
    if let [at] = waterfall
        && let Some(start) = opened(*at)?
    {
        out[0] = Some(start);
        // `checkpartymove` (3 bytes), `compare_var_to_value` (5), then `goto_if 1, <ptr>`.
        let ptr = rom.u32(at + 3 + 5 + 2)?;
        if let Some(target) = ptr.checked_sub(crate::ROM_BASE)
            && target > 0
            && rom.u8(target - 1)? == lockall
        {
            out[1] = Some(target - 1);
        }
    }
    if let [down, up] = dive {
        out[2] = opened(*down)?;
        out[3] = opened(*up)?;
    }
    Ok(out)
}

/// The one script that opens with `lockall; <second>` and reaches `clearflag flag` in a
/// straight line ([`STEP_CALL_SCRIPTS`]), `None` when there is not exactly one.
fn step_call_script(rom: &Rom, second: u8, flag: u16) -> Result<Option<u32>> {
    let lockall = crate::scripts::opcode("lockall").context("lockall")?;
    let mut clear = vec![crate::scripts::opcode("clearflag").context("clearflag")?];
    clear.extend(flag.to_le_bytes());
    let mut found = Vec::new();
    for hit in rom.find_all(&clear) {
        for start in hit.saturating_sub(STEP_CALL_REACH)..hit {
            if rom.u8(start)? != lockall || rom.u8(start + 1)? != second {
                continue;
            }
            let mut pc = start;
            while pc < hit {
                match crate::scripts::decode(rom, pc) {
                    Ok(Some(i)) if !i.is_terminator() => pc += i.len,
                    _ => break,
                }
            }
            if pc == hit {
                found.push(start);
            }
        }
    }
    found.sort();
    found.dedup();
    Ok(match found.as_slice() {
        [at] => Some(*at),
        _ => None,
    })
}

/// Offset -> canonical name: the first name for each offset in `roots`, which must list the
/// maps' [`named_roots`] concatenated in export (`manifest.maps`) order.
pub fn canonical<'a>(roots: impl IntoIterator<Item = &'a (String, u32)>) -> BTreeMap<u32, String> {
    let mut out = BTreeMap::new();
    for (name, off) in roots {
        out.entry(*off).or_insert_with(|| name.clone());
    }
    out
}

#[cfg(test)]
mod step_call_tests {
    use super::*;

    fn op(n: &str) -> u8 {
        crate::scripts::opcode(n).unwrap()
    }

    /// `lockall; pokenavcall <text>; waitmessage; clearflag 0x80; releaseall; end` at 0x110,
    /// after a decoy `clearflag 0x80` that no `lockall; pokenavcall` reaches.
    #[test]
    fn finds_the_registration_by_its_cleared_flag() {
        let mut b = vec![0u8; 0x200];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b[0x100..0x103].copy_from_slice(&[op("clearflag"), 0x80, 0]);
        let mut s = vec![op("lockall"), op("pokenavcall")];
        s.extend((crate::ROM_BASE + 0x180).to_le_bytes());
        s.extend([op("waitmessage"), op("clearflag"), 0x80, 0]);
        s.extend([op("releaseall"), op("end")]);
        b[0x110..0x110 + s.len()].copy_from_slice(&s);
        let rom = Rom::from_bytes(b).unwrap();
        let pokenavcall = op("pokenavcall");
        assert_eq!(
            step_call_script(&rom, pokenavcall, 0x80).unwrap(),
            Some(0x110)
        );
        assert_eq!(step_call_script(&rom, pokenavcall, 0x88).unwrap(), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ROM_BASE;
    use crate::maps::{BgEvent, CoordEvent, MapEvents, MapLayout, ObjectEvent};

    const BASE: u32 = 0x100;

    fn rom(data: &[u8], len: usize) -> Rom {
        let mut b = vec![0u8; len];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b[BASE as usize..BASE as usize + data.len()].copy_from_slice(data);
        Rom::from_bytes(b).unwrap()
    }

    fn p(off: u32) -> [u8; 4] {
        (ROM_BASE + off).to_le_bytes()
    }

    /// 0x100: ON_TRANSITION -> 0x140 ; ON_FRAME_TABLE -> 0x120 ; ON_LOAD -> 0x150 ;
    /// ON_TRANSITION again -> 0x160 (ignored) ; end. 0x120: two var scripts.
    fn table() -> Vec<u8> {
        let mut s = vec![3];
        s.extend(p(0x140));
        s.push(2);
        s.extend(p(0x120));
        s.push(1);
        s.extend(p(0x150));
        s.push(3);
        s.extend(p(0x160));
        s.resize(0x20, 0);
        s.extend([0x50, 0x40, 1, 0]);
        s.extend(p(0x170));
        s.extend([0x51, 0x40, 2, 0]);
        s.extend(p(0x140));
        s.extend([0, 0]);
        s
    }

    #[test]
    fn reads_typed_map_script_table() {
        let t = read_map_scripts(&rom(&table(), 0x400), BASE).unwrap();
        assert_eq!(t.on_transition, Some(0x140));
        assert_eq!(t.on_load, Some(0x150));
        assert_eq!(
            t.on_frame,
            vec![
                VarScript {
                    var: 0x4050,
                    value: 1,
                    script: 0x170
                },
                VarScript {
                    var: 0x4051,
                    value: 2,
                    script: 0x140
                }
            ]
        );
        assert!(t.on_warp_into.is_empty());

        let mut bad = table();
        bad[0] = 9;
        let err = read_map_scripts(&rom(&bad, 0x400), BASE).unwrap_err();
        assert!(format!("{err:#}").contains("unknown type 9"), "{err:#}");

        let entry: Vec<u8> = std::iter::once(3).chain(p(0x140)).collect();
        let err = read_map_scripts(&rom(&entry.repeat(300), 0x1000), BASE).unwrap_err();
        assert!(format!("{err:#}").contains("table at 0x100"), "{err:#}");
    }

    #[test]
    fn names_follow_the_schema_order() {
        let r = rom(&table(), 0x400);
        let obj = |local_id, script| ObjectEvent {
            local_id,
            graphics_id: 0,
            x: 0,
            y: 0,
            elevation: 0,
            movement_type: 0,
            movement_range_x: 0,
            movement_range_y: 0,
            trainer_type: 0,
            trainer_range_berry_tree_id: 0,
            script,
            flag_id: 0,
        };
        let coord = |script| CoordEvent {
            x: 0,
            y: 0,
            elevation: 0,
            trigger_var: 0,
            index: 0,
            script,
        };
        let bg = |kind| BgEvent {
            x: 0,
            y: 0,
            elevation: 0,
            kind_raw: 0,
            kind,
        };
        let header = MapHeader {
            offset: 0,
            layout: MapLayout {
                offset: 0,
                width: 1,
                height: 1,
                border: vec![0; 4],
                blocks: vec![0],
                primary_tileset: 0,
                secondary_tileset: 0,
            },
            events: MapEvents {
                objects: vec![obj(2, Some(0x200)), obj(3, None), obj(4, Some(0x140))],
                warps: vec![],
                coords: vec![coord(None), coord(Some(0x210))],
                bgs: vec![
                    bg(BgEventKind::SecretBase(1)),
                    bg(BgEventKind::Script(Some(0x220))),
                ],
            },
            map_scripts: Some(BASE),
            connections: vec![],
            music: 0,
            layout_id: 0,
            region_map_section: 0,
            cave: 0,
            weather: 0,
            map_type: 0,
            allow_cycling: false,
            allow_escaping: false,
            allow_running: false,
            show_map_name: false,
            battle_type: 0,
        };
        let roots = named_roots(&r, &header, "M").unwrap();
        let names: Vec<(&str, u32)> = roots.iter().map(|(n, o)| (n.as_str(), *o)).collect();
        assert_eq!(
            names,
            [
                ("M_Obj2", 0x200),
                ("M_Obj4", 0x140),
                ("M_Coord1", 0x210),
                ("M_Bg1", 0x220),
                ("M_OnLoad", 0x150),
                ("M_OnTransition", 0x140),
                ("M_OnFrame0", 0x170),
                ("M_OnFrame1", 0x140),
            ]
        );
        let other = [("A_Obj1".to_string(), 0x140), ("A_Obj2".to_string(), 0x300)];
        let canon = canonical(other.iter().chain(&roots));
        assert_eq!(canon[&0x140], "A_Obj1");
        assert_eq!(canon[&0x300], "A_Obj2");
        assert_eq!(canon[&0x150], "M_OnLoad");
        assert_eq!(canon.len(), 7);
    }
}
