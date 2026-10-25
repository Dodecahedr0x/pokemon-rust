//! Decoration catalog and shop lists, found in the ROM rather than taken from a decomp
//! address. The French stock lists are not the US `.inc` lists (Fortree's pretty desk is
//! one the French ROM does not sell).

use crate::rom::ROM_BASE;
use crate::schema::{IrArg, IrInsn, IrList, NameOrigin, ScriptFile};
use std::collections::{BTreeMap, BTreeSet};

/// One `struct Decoration` (32 bytes): id, Gen3 name, price, category, description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decor {
    pub id: u16,
    pub name: Vec<u8>,
    pub price: u16,
    pub category: u8,
    pub description: Vec<u8>,
}

const STRIDE: usize = 32;
/// `DECOR_NONE` plus `NUM_DECORATIONS` (ids 0..=120).
const COUNT: usize = 121;

/// `gDecorations`, anchored on SMALL DESK / PRETTY DESK / COMFY CHAIR (id, category, price)
/// rather than a fixed offset. `None` when that triple is not in the ROM.
pub fn table(rom: &[u8]) -> Option<Vec<Decor>> {
    let last = rom.len().saturating_sub(STRIDE * 3);
    for i in 0..last {
        if !anchor(rom, i) {
            continue;
        }
        let Some(base) = i.checked_sub(STRIDE) else {
            continue;
        };
        if base + STRIDE * COUNT > rom.len() || rom[base] != 0 {
            continue;
        }
        let mut out = Vec::with_capacity(COUNT);
        let mut ok = true;
        for n in 0..COUNT {
            let at = base + n * STRIDE;
            if rom[at] != n as u8 {
                ok = false;
                break;
            }
            out.push(read_one(rom, at, n as u16));
        }
        if ok {
            return Some(out);
        }
    }
    None
}

/// SMALL DESK (id 1, desk, ¥3000), PRETTY DESK (id 2, ¥3000), COMFY CHAIR (id 3, ¥6000).
fn anchor(rom: &[u8], i: usize) -> bool {
    rom[i] == 1
        && rom[i + 19] == 0
        && price_at(rom, i) == 3000
        && rom.get(i + STRIDE).copied() == Some(2)
        && price_at(rom, i + STRIDE) == 3000
        && rom.get(i + STRIDE * 2).copied() == Some(3)
        && price_at(rom, i + STRIDE * 2) == 6000
}

fn price_at(rom: &[u8], i: usize) -> u16 {
    rom.get(i + 20..i + 22)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .unwrap_or(0)
}

fn read_one(rom: &[u8], at: usize, id: u16) -> Decor {
    let rec = &rom[at..at + STRIDE];
    let desc = u32::from_le_bytes(rec[24..28].try_into().unwrap());
    Decor {
        id,
        name: terminated(&rec[1..17]),
        price: u16::from_le_bytes([rec[20], rec[21]]),
        category: rec[19],
        description: text_at(rom, desc),
    }
}

fn terminated(raw: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = raw.iter().copied().take_while(|&b| b != 0xFF).collect();
    out.push(0xFF);
    out
}

fn text_at(rom: &[u8], ptr: u32) -> Vec<u8> {
    let Some(off) = ptr.checked_sub(ROM_BASE).map(|o| o as usize) else {
        return vec![0xFF];
    };
    if off >= rom.len() {
        return vec![0xFF];
    }
    let mut out = Vec::new();
    for &b in rom[off..].iter().take(256) {
        out.push(b);
        if b == 0xFF {
            return out;
        }
    }
    out.push(0xFF);
    out
}

/// Halfwords at `ptr` until 0. Each id is 1..=120 and the list is 1..=64 long.
pub fn read_list(rom: &[u8], ptr: u32) -> Option<Vec<u16>> {
    let mut i = ptr.checked_sub(ROM_BASE)? as usize;
    let mut ids = Vec::new();
    loop {
        if i + 2 > rom.len() {
            return None;
        }
        let id = u16::from_le_bytes([rom[i], rom[i + 1]]);
        i += 2;
        if id == 0 {
            break;
        }
        if !(1..=120).contains(&id) || ids.len() == 64 {
            return None;
        }
        ids.push(id);
    }
    if ids.is_empty() { None } else { Some(ids) }
}

/// True when a decoration mart still points at a raw ROM word (the list was not resolved).
pub fn has_word_lists(files: &[ScriptFile]) -> bool {
    files.iter().any(|f| {
        f.labels.iter().flat_map(|l| &l.insns).any(|insn| {
            is_decor_mart(&insn.op) && matches!(insn.args.first(), Some(IrArg::Word(_)))
        })
    })
}

fn is_decor_mart(op: &str) -> bool {
    matches!(op, "pokemartdecoration" | "pokemartdecoration2")
}

/// Rewrites resolved `pokemartdecoration` / `pokemartdecoration2` words into `Items` lists
/// named `Decor_<ptr>`. A list shared by two modules is emitted once, in the first module.
/// Returns how many commands were rewritten.
pub fn attach_lists(files: &mut [ScriptFile], rom: &[u8]) -> usize {
    let placed: BTreeSet<String> = files
        .iter()
        .flat_map(|f| f.lists.iter().map(|l| l.name.clone()))
        .collect();
    let mut items: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut owner: BTreeMap<String, usize> = BTreeMap::new();
    for (fi, file) in files.iter().enumerate() {
        for insn in file.labels.iter().flat_map(|l| &l.insns) {
            let Some(ptr) = word_ptr(insn) else {
                continue;
            };
            let Some(ids) = read_list(rom, ptr) else {
                continue;
            };
            let name = list_name(ptr);
            if placed.contains(&name) {
                continue;
            }
            items
                .entry(name.clone())
                .or_insert_with(|| ids.iter().map(|id| id.to_string()).collect());
            owner.entry(name).or_insert(fi);
        }
    }
    for (name, fi) in &owner {
        files[*fi].lists.push(IrList {
            name: name.clone(),
            origin: NameOrigin::Positional,
            items: items[name].clone(),
        });
    }
    let mut n = 0;
    for file in files.iter_mut() {
        for insn in file.labels.iter_mut().flat_map(|l| l.insns.iter_mut()) {
            let Some(ptr) = word_ptr(insn) else {
                continue;
            };
            let name = list_name(ptr);
            if !items.contains_key(&name) && !placed.contains(&name) {
                continue;
            }
            insn.args[0] = IrArg::Items(name);
            n += 1;
        }
    }
    n
}

fn word_ptr(insn: &IrInsn) -> Option<u32> {
    if !is_decor_mart(&insn.op) {
        return None;
    }
    match insn.args.first() {
        Some(IrArg::Word(ptr)) => Some(*ptr),
        _ => None,
    }
}

fn list_name(ptr: u32) -> String {
    format!("Decor_{ptr:X}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Hex, IrLabel};

    fn file(op: &str, ptr: u32) -> ScriptFile {
        ScriptFile {
            module: "Shop".into(),
            source: None,
            labels: vec![IrLabel {
                name: "N".into(),
                origin: NameOrigin::Positional,
                offset: Hex(0),
                root: None,
                insns: vec![IrInsn {
                    op: op.into(),
                    args: vec![IrArg::Word(ptr)],
                }],
                next: None,
            }],
            movements: vec![],
            lists: vec![],
        }
    }

    #[test]
    fn a_halfword_list_becomes_a_named_items_operand() {
        let rom = [1u8, 0, 0, 0];
        let mut files = [file("pokemartdecoration", ROM_BASE)];
        assert_eq!(attach_lists(&mut files, &rom), 1);
        assert!(matches!(
            &files[0].labels[0].insns[0].args[0],
            IrArg::Items(n) if n == "Decor_8000000"
        ));
        assert_eq!(files[0].lists[0].items, ["1"]);
        assert!(!has_word_lists(&files));
    }

    #[test]
    fn an_id_past_the_catalog_is_left_as_a_word() {
        let rom = [0xE8u8, 0x03, 0, 0]; // 1000, then 0
        let mut files = [file("pokemartdecoration2", ROM_BASE)];
        assert_eq!(attach_lists(&mut files, &rom), 0);
        assert!(has_word_lists(&files));
    }

    #[test]
    fn the_catalog_anchor_reads_id_price_and_category() {
        let mut rom = vec![0u8; 4 + STRIDE * COUNT];
        for n in 0..COUNT {
            let at = 4 + n * STRIDE;
            rom[at] = n as u8;
            rom[at + 1] = b'A';
            rom[at + 2] = 0xFF;
            rom[at + 19] = u8::from(n == 0);
            let price: u16 = match n {
                1 | 2 => 3000,
                3 => 6000,
                _ => 1,
            };
            rom[at + 20..at + 22].copy_from_slice(&price.to_le_bytes());
            rom[at + 24..at + 28].copy_from_slice(&0u32.to_le_bytes());
        }
        let table = table(&rom).expect("anchor");
        assert_eq!(table.len(), COUNT);
        assert_eq!(table[1].price, 3000);
        assert_eq!(table[1].category, 0);
        assert_eq!(table[1].name, b"A\xFF");
        assert_eq!(table[3].price, 6000);
        assert_eq!(table[0].id, 0);
    }
}
