//! Append-only id allocation (schema docs, "Id allocation").

use super::schema::{IdEntry, IdKind, IdsFile};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

/// Allocates `names` (with their map file names for maps) on top of `existing`: existing entries
/// are kept unchanged (tombstones included), new names are appended in name order after the
/// highest number. Returns the file and the existing live names the source no longer has.
pub fn allocate(
    kind: IdKind,
    existing: Option<&IdsFile>,
    names: &BTreeMap<String, Option<String>>,
) -> Result<(IdsFile, Vec<String>)> {
    let (base, max) = kind.range();
    let mut file = existing.cloned().unwrap_or(IdsFile {
        base,
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

/// Problems in an ids file: duplicate names or numbers, numbers outside the kind's range or
/// reserved, a base lower than the kind's.
pub fn problems(kind: IdKind, file: &IdsFile) -> Vec<(String, String)> {
    let (base, max) = kind.range();
    let mut out = Vec::new();
    if file.base < base {
        out.push((
            format!("ids/{}.ron", kind.file()),
            format!("base {} is below the first free number {base}", file.base),
        ));
    }
    let mut names = BTreeSet::new();
    let mut nums = BTreeMap::new();
    for e in &file.ids {
        if !names.insert(e.name.as_str()) {
            out.push((e.name.clone(), "listed twice".to_string()));
        }
        if let Some(other) = nums.insert(e.num, e.name.as_str()) {
            out.push((e.name.clone(), format!("number {} is also {other}", e.num)));
        }
        if e.num < file.base.max(base) || e.num > max || kind.reserved(e.num) {
            out.push((
                e.name.clone(),
                format!(
                    "number {:#x} is outside {:#x}..={max:#x} or reserved",
                    e.num, base
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
        let (f, stale) =
            allocate(IdKind::Flags, None, &names(&["FLAG_SIL_B", "FLAG_SIL_A"])).unwrap();
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
        )
        .unwrap();
        assert_eq!(stale, vec!["FLAG_SIL_A".to_string()]);
        assert_eq!(h.ids[..2], g.ids[..]);
        assert_eq!((h.ids[2].name.as_str(), h.ids[2].num), ("FLAG_SIL_0", 0x22));
        assert_eq!((h.ids[3].name.as_str(), h.ids[3].num), ("FLAG_SIL_C", 0x23));
        assert!(problems(IdKind::Flags, &h).is_empty());
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
        let (g, _) = allocate(IdKind::Maps, Some(&f), &n).unwrap();
        assert_eq!(g.ids[1].num, 0x7F80);
        let mut bad = g.clone();
        bad.ids[1].num = 0x7F7E;
        assert_eq!(problems(IdKind::Maps, &bad).len(), 1);
    }
}
