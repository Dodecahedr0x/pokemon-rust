//! Append-only id allocation (schema docs, "Id allocation").

use super::schema::{IdEntry, IdKind, IdsFile};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

/// Allocates `names` (with their map file names for maps) on top of `existing`: existing entries
/// are kept unchanged (tombstones included), new names are appended in name order after the
/// highest number. A new file starts at the kind's base, or at `base` when that is higher.
/// Returns the file and the existing live names the source no longer has.
pub fn allocate(
    kind: IdKind,
    existing: Option<&IdsFile>,
    names: &BTreeMap<String, Option<String>>,
    base: Option<u32>,
) -> Result<(IdsFile, Vec<String>)> {
    let (first, max) = kind.range();
    let mut file = existing.cloned().unwrap_or(IdsFile {
        base: base.map_or(first, |b| b.max(first)),
        ids: Vec::new(),
    });
    let known: BTreeSet<&str> = file.ids.iter().map(|e| e.name.as_str()).collect();
    let stale = file
        .ids
        .iter()
        .filter(|e| !e.removed && !names.contains_key(&e.name))
        .map(|e| e.name.clone())
        .collect();
    let mut next = file
        .ids
        .iter()
        .map(|e| e.num + 1)
        .max()
        .unwrap_or(file.base)
        .max(file.base);
    let mut new = Vec::new();
    for (name, map) in names {
        if known.contains(name.as_str()) {
            continue;
        }
        while kind.reserved(next) {
            next += 1;
        }
        if next > max {
            bail!(
                "ids/{}.ron: no number left for {name} (the last is {max:#x})",
                kind.file()
            );
        }
        new.push(IdEntry {
            name: name.clone(),
            num: next,
            map: map.clone(),
            removed: false,
        });
        next += 1;
    }
    file.ids.extend(new);
    Ok((file, stale))
}

/// Marks `names` as removed in `file` (tombstones keep their numbers). Returns the names it did
/// not find. A name already tombstoned stays so.
pub fn tombstone(file: &mut IdsFile, names: &[String]) -> Vec<String> {
    let mut missing = Vec::new();
    for n in names {
        let listed = file.ids.iter().any(|e| &e.name == n);
        match file.ids.iter_mut().find(|e| &e.name == n && !e.removed) {
            Some(e) => e.removed = true,
            None if listed => {}
            None => missing.push(n.clone()),
        }
    }
    missing
}

/// Moves the live `name` to a new number: its entry becomes a tombstone (keeping the old number
/// taken) and the name is appended again, live, at the next free number (append-only, see the
/// content build's `build/silice/ids.rs`). For an id on a number that must not be used, such as
/// one [`IdKind::reserved`] since it was allocated. Returns (old, new).
pub fn relocate(kind: IdKind, file: &mut IdsFile, name: &str) -> Result<(u32, u32)> {
    let (_, max) = kind.range();
    let Some(e) = file.ids.iter_mut().find(|e| e.name == name && !e.removed) else {
        bail!("ids/{}.ron: no live {name}", kind.file());
    };
    e.removed = true;
    let (old, map) = (e.num, e.map.clone());
    let mut next = file
        .ids
        .iter()
        .map(|e| e.num + 1)
        .max()
        .unwrap_or(file.base);
    while kind.reserved(next) {
        next += 1;
    }
    if next > max {
        bail!("ids/{}.ron: no number left for {name}", kind.file());
    }
    file.ids.push(IdEntry {
        name: name.to_string(),
        num: next,
        map,
        removed: false,
    });
    Ok((old, next))
}

/// Problems in an ids file (the content build's `build/silice/ids.rs` rules): a name live
/// twice, or listed again before its tombstone; a number used twice; a number outside the kind's
/// range; a live entry on a reserved number; a base lower than the kind's.
pub fn problems(kind: IdKind, file: &IdsFile) -> Vec<(String, String)> {
    let (base, max) = kind.range();
    let mut out = Vec::new();
    if file.base < base {
        out.push((
            format!("ids/{}.ron", kind.file()),
            format!("base {} is below the first free number {base}", file.base),
        ));
    }
    let mut names = BTreeMap::new();
    let mut nums = BTreeMap::new();
    for e in &file.ids {
        if names.insert(e.name.as_str(), !e.removed) == Some(true) {
            out.push((e.name.clone(), "listed twice".to_string()));
        }
        if let Some(other) = nums.insert(e.num, e.name.as_str()) {
            out.push((e.name.clone(), format!("number {} is also {other}", e.num)));
        }
        if e.num < file.base.max(base) || e.num > max {
            out.push((
                e.name.clone(),
                format!("number {:#x} is outside {:#x}..={max:#x}", e.num, base),
            ));
        }
        if !e.removed && kind.reserved(e.num) {
            out.push((
                e.name.clone(),
                format!(
                    "number {:#x} is reserved (MAP_DYNAMIC or a Gen3 flag or var the shared \
                     code uses); move it: `cargo xtask silice move {}`",
                    e.num, e.name
                ),
            ));
        }
        if kind == IdKind::Maps && e.map.is_none() && !e.removed {
            out.push((
                e.name.clone(),
                "a map entry without its map name".to_string(),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> BTreeMap<String, Option<String>> {
        v.iter().map(|s| (s.to_string(), None)).collect()
    }

    #[test]
    fn append_only() {
        let (f, stale) = allocate(
            IdKind::Flags,
            None,
            &names(&["FLAG_SIL_B", "FLAG_SIL_A"]),
            None,
        )
        .unwrap();
        assert!(stale.is_empty());
        assert_eq!(f.ids[0].name, "FLAG_SIL_A");
        assert_eq!(f.ids[0].num, 0x20);
        assert_eq!(f.ids[1].num, 0x21);
        let mut g = f.clone();
        g.ids[1].removed = true;
        let (h, stale) = allocate(
            IdKind::Flags,
            Some(&g),
            &names(&["FLAG_SIL_0", "FLAG_SIL_C"]),
            Some(0x300),
        )
        .unwrap();
        assert_eq!(stale, vec!["FLAG_SIL_A".to_string()]);
        assert_eq!(h.ids[..2], g.ids[..]);
        assert_eq!((h.ids[2].name.as_str(), h.ids[2].num), ("FLAG_SIL_0", 0x22));
        assert_eq!((h.ids[3].name.as_str(), h.ids[3].num), ("FLAG_SIL_C", 0x23));
        assert!(problems(IdKind::Flags, &h).is_empty());
    }

    #[test]
    fn base_and_tombstones() {
        let (mut f, _) =
            allocate(IdKind::Flags, None, &names(&["FLAG_SIL_A"]), Some(0x113)).unwrap();
        assert_eq!((f.base, f.ids[0].num), (0x113, 0x113));
        assert_eq!(
            tombstone(&mut f, &["FLAG_SIL_A".into(), "FLAG_SIL_Z".into()]),
            vec!["FLAG_SIL_Z".to_string()]
        );
        let (g, stale) = allocate(
            IdKind::Flags,
            Some(&f),
            &names(&["FLAG_SIL_A", "FLAG_SIL_B"]),
            None,
        )
        .unwrap();
        assert!(stale.is_empty());
        assert!(g.ids[0].removed);
        assert_eq!(
            (g.ids[1].name.as_str(), g.ids[1].num),
            ("FLAG_SIL_B", 0x114)
        );
    }

    #[test]
    fn maps_skip_reserved() {
        let mut f = IdsFile {
            base: 0,
            ids: Vec::new(),
        };
        f.ids.push(IdEntry {
            name: "MAP_A".into(),
            num: 0x7F7E,
            map: Some("A".into()),
            removed: false,
        });
        let mut n = BTreeMap::new();
        n.insert("MAP_B".to_string(), Some("B".to_string()));
        let (g, _) = allocate(IdKind::Maps, Some(&f), &n, None).unwrap();
        assert_eq!(g.ids[1].num, 0x7F80);
        let mut bad = g.clone();
        bad.ids[1].num = 0x7F7E;
        assert_eq!(problems(IdKind::Maps, &bad).len(), 1);
    }

    /// A var on a shared Gen3 number (`VAR_STARTER_MON` 0x4023) is a problem; moving it leaves
    /// a tombstone there and appends the name past the reserved numbers.
    #[test]
    fn reserved_vars_move() {
        let (mut f, _) = allocate(
            IdKind::Vars,
            None,
            &names(&["VAR_SIL_A", "VAR_SIL_B", "VAR_SIL_C", "VAR_SIL_D"]),
            None,
        )
        .unwrap();
        assert_eq!(
            f.ids.iter().map(|e| e.num).collect::<Vec<_>>(),
            [0x4020, 0x4022, 0x4024, 0x4025],
            "the allocator skips 0x4021 (VAR_REPEL_STEP_COUNT) and 0x4023 (VAR_STARTER_MON)"
        );
        f.ids[3].num = 0x4023;
        let p = problems(IdKind::Vars, &f);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].1.contains("reserved"), "{p:?}");
        assert_eq!(
            relocate(IdKind::Vars, &mut f, "VAR_SIL_D").unwrap(),
            (0x4023, 0x4025)
        );
        assert!(f.ids[3].removed && f.ids[3].num == 0x4023);
        assert_eq!(
            (f.ids[4].name.as_str(), f.ids[4].removed),
            ("VAR_SIL_D", false)
        );
        assert!(problems(IdKind::Vars, &f).is_empty());
        // A second live entry is listed twice; tombstoning reaches the live one.
        let mut g = f.clone();
        g.ids.push(g.ids[4].clone());
        g.ids[5].num = 0x4026;
        assert_eq!(problems(IdKind::Vars, &g).len(), 1);
        assert!(tombstone(&mut f, &["VAR_SIL_D".into()]).is_empty());
        assert!(f.ids[4].removed);
        assert!(relocate(IdKind::Vars, &mut f, "VAR_SIL_D").is_err());
    }
}
