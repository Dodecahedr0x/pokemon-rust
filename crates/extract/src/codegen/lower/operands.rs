//! Operands: decomp constants, vars, flags, ids and coordinates as Rust expressions.

use super::*;

/// The special a `special`/`specialvar` names, as its support-table key.
pub(super) fn special_of(i: &IrInsn) -> Option<String> {
    let a = match (i.op.as_str(), &i.args[..]) {
        ("special", [s]) | ("specialvar", [_, s]) => s,
        _ => return None,
    };
    special_name(a).map(|(n, _)| n)
}

/// (`SPECIAL_*` name, number) of a special operand; unnamed ones are `SPECIAL_0x<n>`.
pub(super) fn special_name(a: &IrArg) -> Option<(String, u32)> {
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
pub(super) fn std_index(a: &IrArg) -> Option<u32> {
    match a {
        IrArg::Const { name, .. } => const_value(name).or_else(|| {
            name.strip_prefix("STD_")
                .and_then(|n| names::MSGBOX_TYPES.value(n))
        }),
        a => u8_lit(a).map(u32::from),
    }
}

/// Decomp tables by name prefix (a name may match several prefixes: `TRAINER_TYPE_*`).
pub(super) const TABLES: [(&str, &names::Table); 17] = [
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
pub(super) fn const_value(name: &str) -> Option<u32> {
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
pub(super) fn const_type(name: &str) -> Option<&'static str> {
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

pub(super) fn u16_lit(a: &IrArg) -> Option<u16> {
    match a {
        IrArg::Byte(b) => Some((*b).into()),
        IrArg::Half(h) => Some(*h),
        IrArg::Word(w) | IrArg::Raw(w) => u16::try_from(*w).ok(),
        IrArg::Const { name, .. } => const_value(name).and_then(|v| u16::try_from(v).ok()),
        _ => None,
    }
}

pub(super) fn u8_lit(a: &IrArg) -> Option<u8> {
    u16_lit(a).and_then(|v| u8::try_from(v).ok())
}

pub(super) fn u32_lit(a: &IrArg) -> Option<u32> {
    match a {
        IrArg::Word(w) | IrArg::Raw(w) => Some(*w),
        a => u16_lit(a).map(u32::from),
    }
}

/// A `u16` operand expression. `varget`: Gen3 reads it with `VarGet`, so a var (or a number
/// ≥ 0x4000) reads the var's value.
pub(super) fn u16_expr(a: &IrArg, varget: bool) -> Option<String> {
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
pub(super) fn as_u8(e: &str) -> String {
    match e.parse::<u16>() {
        Ok(v) if v <= 0xFF => v.to_string(),
        _ => format!("{e} as u8"),
    }
}

pub(super) fn var(v: u16) -> String {
    named(&names::VARS, v.into()).map_or_else(|| format!("VarId({v:#x})"), str::to_string)
}

pub(super) fn flag(f: u16) -> String {
    named(&names::FLAGS, f.into()).map_or_else(|| format!("FlagId({f:#x})"), str::to_string)
}

/// A `VAR_*` name (or `VAR_0x<hex>`) as an expression.
pub(super) fn var_name(v: &str) -> Option<String> {
    if let Some(h) = v.strip_prefix("VAR_0x") {
        return Some(var(u16::from_str_radix(h, 16).ok()?));
    }
    is_ident(v).then(|| v.to_string())
}

pub(super) fn flag_name(f: &str) -> Option<String> {
    if let Some(h) = f.strip_prefix("FLAG_0x") {
        return Some(flag(u16::from_str_radix(h, 16).ok()?));
    }
    is_ident(f).then(|| f.to_string())
}

/// A var operand Gen3 reads with `GetVarPointer` (a destination or a `copyvar` source).
pub(super) fn var_target(a: &IrArg) -> Option<String> {
    match a {
        IrArg::Var(v) => var_name(v),
        IrArg::Const { name, .. } if name.starts_with("VAR_") => var_name(name),
        IrArg::Half(h) if *h >= VARS_START => Some(var(*h)),
        _ => None,
    }
}

pub(super) fn flag_arg(a: &IrArg) -> Option<String> {
    match a {
        IrArg::Flag(f) => flag_name(f),
        IrArg::Const { name, .. } if name.starts_with("FLAG_") => flag_name(name),
        IrArg::Half(h) => Some(flag(*h)),
        _ => None,
    }
}

/// A `VarGet` local id as a `u8` expression (`PLAYER` for `LOCALID_PLAYER`).
pub(super) fn local_id(a: &IrArg) -> Option<String> {
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
pub(super) fn coord(a: &IrArg) -> Option<String> {
    match a {
        IrArg::Half(0xFFFF) => Some("-1".to_string()),
        IrArg::Var(_) => u16_expr(a, true).map(|e| format!("{e} as i16")),
        IrArg::Half(h) if *h >= VARS_START => u16_expr(a, true).map(|e| format!("{e} as i16")),
        a => u16_lit(a).map(|v| (v as i16).to_string()),
    }
}

/// A map operand (group byte, then num byte) as a `MapId` expression.
pub(super) fn map_arg(a: &IrArg) -> Option<String> {
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

pub(super) fn warp_id(a: &IrArg) -> Option<String> {
    Some(match u8_lit(a)? {
        WARP_ID_NONE => "None".to_string(),
        w => format!("Some({w})"),
    })
}

/// Whether an `*at` command acts on the current map: always for the player (Gen3 finds the
/// player on any map), else when its map operand is the module's map.
pub(super) fn at_here(id: &IrArg, at: &IrArg, module: &str) -> bool {
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

pub(super) fn typed(a: &IrArg, varget: bool, prefix: &str, ty: &str) -> Option<String> {
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

pub(super) fn species_arg(a: &IrArg, varget: bool) -> Option<String> {
    typed(a, varget, "SPECIES_", "SpeciesId")
}

pub(super) fn move_arg(a: &IrArg) -> Option<String> {
    typed(a, true, "MOVE_", "MoveId")
}

pub(super) fn trainer_arg(a: &IrArg, varget: bool) -> Option<String> {
    typed(a, varget, "TRAINER_", "TrainerId")
}

/// `DIR_SOUTH..DIR_EAST`.
pub(super) fn dir(d: u8) -> Option<&'static str> {
    Some(match d {
        1 => "Dir::Down",
        2 => "Dir::Up",
        3 => "Dir::Left",
        4 => "Dir::Right",
        _ => return None,
    })
}

/// `MOVEMENT_TYPE_*` as its constant name: the content crate defines each one as an
/// `engine::data::MovementType` (`content/build/classify/field.rs` `movement`, the single
/// mapping shared with the object templates).
pub(super) fn movement_type(a: &IrArg) -> Option<String> {
    let name = match a {
        IrArg::Const { name, .. } => name.as_str(),
        a => names::MOVEMENT_TYPES.name(u8_lit(a)?.into())?,
    };
    names::MOVEMENT_TYPES.value(name)?;
    Some(name.to_string())
}

/// The `Step` of a `MOVEMENT_ACTION_*` name, if the engine has one.
pub(super) fn step_of(action: &str) -> Option<String> {
    let v = names::MOVEMENT_ACTIONS.value(action)?;
    movement::step(u8::try_from(v).ok()?)
}
