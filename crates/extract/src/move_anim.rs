//! Move animations (`gBattleAnims_Moves`, `gBattleAnimPicTable`, `gBattleAnimPaletteTable`).
//!
//! Tables are found from the Tackle script's opcode prefix and from sprite tags
//! `10000, 10001, ...` (`ANIM_SPRITES_START`). Callback addresses are learned from that
//! Tackle script, so the same decoder works on BPEE and BPEF. A script that does not
//! decode is left empty; the engine lunges for those moves.

use crate::Rom;
use crate::export::Out;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

/// `MOVES_COUNT` in `include/constants/moves.h`. The status-anim table follows immediately,
/// so a pointer scan must stop here.
const MOVES_COUNT: usize = 355;
const ANIM_SPRITES_START: u16 = 10_000;
const IMPACT_TAG: u16 = 10_135;
const TACKLE_INDEX: usize = 33;
/// `loadspritegfx ANIM_TAG_IMPACT; monbg ANIM_TARGET; setalpha 12, 8; createsprite`.
const TACKLE_PREFIX: &[u8] = &[0x00, 0x97, 0x27, 0x0A, 0x01, 0x0C, 0x0C, 0x08, 0x02];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct File {
    #[serde(default)]
    pub scripts: Vec<Vec<Cmd>>,
    #[serde(default)]
    pub gfx: Vec<Gfx>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gfx {
    pub tag: u16,
    pub tiles: Vec<[u32; 8]>,
    pub palette: [u16; 16],
}

/// The engine's `battle::anim::Cmd`, in the same shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cmd {
    LoadGfx(u16),
    UnloadGfx(u16),
    MonBg(u8),
    ClearMonBg(u8),
    SetAlpha(u8, u8),
    BlendOff,
    Delay(u8),
    Wait,
    End,
    PlaySe {
        se: u16,
        pan: u8,
    },
    Lunge {
        frames: u8,
        delta: i16,
    },
    HitSplat {
        tag: u16,
        x: i16,
        y: i16,
        relative: u8,
        anim: u8,
    },
    Shake {
        who: u8,
        x: i16,
        y: i16,
        times: u8,
        delay: u8,
    },
    Show {
        tag: u16,
        x: i16,
        y: i16,
        relative: u8,
    },
}

struct Ids {
    script: u32,
    lunge: u32,
    splat: u32,
    shake: u32,
}

/// Decodes every move script and the sprite sheets those scripts load.
pub fn extract(rom: &Rom) -> Result<File> {
    let ids = learn(rom).context("Tackle script")?;
    let table = move_table(rom, ids.script).context("gBattleAnims_Moves")?;
    let mut scripts = Vec::with_capacity(MOVES_COUNT);
    let mut tags = BTreeSet::new();
    for i in 0..MOVES_COUNT {
        let ptr = rom.u32(table + i as u32 * 4).unwrap_or(0);
        let off = ptr.checked_sub(crate::ROM_BASE).unwrap_or(u32::MAX);
        let cmds = decode(rom, off, &ids).unwrap_or_default();
        for c in &cmds {
            if let Cmd::LoadGfx(tag) = c {
                tags.insert(*tag);
            }
        }
        scripts.push(cmds);
    }
    ensure!(
        !scripts[TACKLE_INDEX].is_empty(),
        "Tackle's script did not decode"
    );
    let pics = find_table(rom, 6, pic_entry).context("gBattleAnimPicTable")?;
    let pals = find_table(rom, 4, pal_entry).context("gBattleAnimPaletteTable")?;
    let mut gfx = Vec::new();
    for tag in tags {
        let Some(index) = tag.checked_sub(ANIM_SPRITES_START) else {
            continue;
        };
        let (Ok(Some(pic)), Ok(Some(pal))) = (
            rom.ptr(pics + u32::from(index) * 8),
            rom.ptr(pals + u32::from(index) * 8),
        ) else {
            continue;
        };
        let (Ok(tiles), Ok(colors)) = (rom.lz77_at(pic), rom.lz77_at(pal)) else {
            continue;
        };
        if colors.len() < 32 {
            continue;
        }
        let palette =
            core::array::from_fn(|i| u16::from_le_bytes([colors[i * 2], colors[i * 2 + 1]]));
        gfx.push(Gfx {
            tag,
            tiles: tiles_4bpp(&tiles),
            palette,
        });
    }
    ensure!(
        gfx.iter()
            .any(|g| g.tag == IMPACT_TAG && g.tiles.len() == 16),
        "impact sprite (tag {IMPACT_TAG}) was not 16 tiles"
    );
    Ok(File { scripts, gfx })
}

pub fn export(rom: &Rom, out: &mut Out) -> Result<()> {
    out.ron("battle/move_anims.ron", &extract(rom)?)
}

fn learn(rom: &Rom) -> Result<Ids> {
    let bytes = rom.bytes();
    let mut at = 0usize;
    while let Some(rel) = bytes[at..]
        .windows(TACKLE_PREFIX.len())
        .position(|w| w == TACKLE_PREFIX)
    {
        let script = (at + rel) as u32;
        at += rel + 1;
        if let Some(ids) = learn_at(rom, script) {
            return Ok(ids);
        }
    }
    bail!("Tackle prefix not found");
}

fn learn_at(rom: &Rom, script: u32) -> Option<Ids> {
    let mut pc = script;
    if rom.u8(pc).ok()? != 0x00 || rom.u16(pc + 1).ok()? != IMPACT_TAG {
        return None;
    }
    pc += 3;
    if rom.u8(pc).ok()? != 0x0A || rom.u8(pc + 1).ok()? != 1 {
        return None;
    }
    pc += 2;
    if rom.u8(pc).ok()? != 0x0C {
        return None;
    }
    pc += 3;
    let (lunge, n) = sprite_callback(rom, pc)?;
    pc += n;
    if rom.u8(pc).ok()? != 0x04 {
        return None;
    }
    pc += 2;
    let (splat, n) = sprite_callback(rom, pc)?;
    pc += n;
    if rom.u8(pc).ok()? != 0x03 {
        return None;
    }
    // Script stores the absolute address; `Rom::ptr` is a file offset.
    let shake = rom.ptr(pc + 1).ok()?? + crate::ROM_BASE;
    Some(Ids {
        script,
        lunge,
        splat,
        shake,
    })
}

fn sprite_callback(rom: &Rom, pc: u32) -> Option<(u32, u32)> {
    if rom.u8(pc).ok()? != 0x02 && rom.u8(pc).ok()? != 0x03 {
        return None;
    }
    let template = rom.ptr(pc + 1).ok()??;
    let nargs = rom.u8(pc + 6).ok()?;
    let cb = rom.u32(template + 20).ok()?;
    Some((cb, 7 + u32::from(nargs) * 2))
}

fn move_table(rom: &Rom, script: u32) -> Result<u32> {
    let needle = (script + crate::ROM_BASE).to_le_bytes();
    let bytes = rom.bytes();
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if bytes[i..i + 4] == needle {
            let hit = i as u32;
            if let Some(base) = hit.checked_sub(TACKLE_INDEX as u32 * 4)
                && table_ok(rom, base)
            {
                return Ok(base);
            }
        }
        i += 4;
    }
    bail!("no move-anim table points at the Tackle script")
}

fn table_ok(rom: &Rom, base: u32) -> bool {
    (0..8).all(|k| {
        let i = TACKLE_INDEX as u32 - 4 + k;
        let Ok(ptr) = rom.u32(base + i * 4) else {
            return false;
        };
        let Some(off) = ptr.checked_sub(crate::ROM_BASE) else {
            return false;
        };
        rom.u8(off).ok().is_some_and(|op| op <= 0x2F)
    })
}

fn find_table(rom: &Rom, tag_at: usize, ok: fn(&Rom, u32, u16) -> bool) -> Result<u32> {
    let bytes = rom.bytes();
    let mut i = 0usize;
    while i + tag_at + 8 * 5 + 2 < bytes.len() {
        if bytes[i + tag_at] == 0x10 && bytes[i + tag_at + 1] == 0x27 {
            let base = i as u32;
            if (0..6).all(|k| {
                let tag = ANIM_SPRITES_START + k;
                ok(rom, base + u32::from(k) * 8, tag)
            }) {
                return Ok(base);
            }
        }
        i += 2;
    }
    bail!("sprite tag run 10000.. not found (tag at +{tag_at})")
}

fn pic_entry(rom: &Rom, off: u32, tag: u16) -> bool {
    let Ok(Some(ptr)) = rom.ptr(off) else {
        return false;
    };
    let Ok(size) = rom.u16(off + 4) else {
        return false;
    };
    let Ok(got) = rom.u16(off + 6) else {
        return false;
    };
    got == tag
        && (0x20..=0x2000).contains(&size)
        && size % 0x20 == 0
        && rom.u8(ptr).ok() == Some(0x10)
}

fn pal_entry(rom: &Rom, off: u32, tag: u16) -> bool {
    let Ok(Some(ptr)) = rom.ptr(off) else {
        return false;
    };
    rom.u16(off + 4).ok() == Some(tag)
        && rom.u16(off + 6).ok() == Some(0)
        && rom.u8(ptr).ok() == Some(0x10)
}

fn decode(rom: &Rom, mut pc: u32, ids: &Ids) -> Option<Vec<Cmd>> {
    let mut out = Vec::new();
    let mut stack = Vec::new();
    let mut seen: HashMap<u32, u8> = HashMap::new();
    let mut fuel = 2048u32;
    loop {
        if fuel == 0 || out.len() >= 240 {
            out.push(Cmd::End);
            return Some(out);
        }
        fuel -= 1;
        let n = seen.entry(pc).or_insert(0);
        *n += 1;
        if *n > 4 {
            out.push(Cmd::End);
            return Some(out);
        }
        let op = rom.u8(pc).ok()?;
        let len = cmd_len(rom, pc)?;
        match op {
            0x00 => out.push(Cmd::LoadGfx(rom.u16(pc + 1).ok()?)),
            0x01 => out.push(Cmd::UnloadGfx(rom.u16(pc + 1).ok()?)),
            0x02 => {
                if let Some(cmd) = sprite_cmd(rom, pc, ids) {
                    out.push(cmd);
                }
            }
            0x03 => {
                if let Some(cmd) = task_cmd(rom, pc, ids) {
                    out.push(cmd);
                }
            }
            0x04 => out.push(Cmd::Delay(rom.u8(pc + 1).ok()?)),
            0x05 => out.push(Cmd::Wait),
            0x08 => {
                out.push(Cmd::End);
                return Some(out);
            }
            0x09 => out.push(Cmd::PlaySe {
                se: rom.u16(pc + 1).ok()?,
                pan: 0,
            }),
            0x0A => out.push(Cmd::MonBg(rom.u8(pc + 1).ok()?)),
            0x0B => out.push(Cmd::ClearMonBg(rom.u8(pc + 1).ok()?)),
            0x0C => {
                let w = rom.u16(pc + 1).ok()?;
                out.push(Cmd::SetAlpha(w as u8, (w >> 8) as u8));
            }
            0x0D => out.push(Cmd::BlendOff),
            0x0E => {
                if stack.len() >= 8 {
                    out.push(Cmd::End);
                    return Some(out);
                }
                stack.push(pc + len);
                pc = rom.ptr(pc + 1).ok()??;
                continue;
            }
            0x0F => {
                pc = stack.pop()?;
                continue;
            }
            0x11 => {
                pc = rom.ptr(pc + 1).ok()??;
                continue;
            }
            0x13 => {
                pc = rom.ptr(pc + 1).ok()??;
                continue;
            }
            0x19 => out.push(Cmd::PlaySe {
                se: rom.u16(pc + 1).ok()?,
                pan: rom.u8(pc + 3).ok()?,
            }),
            0x22 => out.push(Cmd::MonBg(rom.u8(pc + 1).ok()?)),
            0x23 => out.push(Cmd::ClearMonBg(rom.u8(pc + 1).ok()?)),
            // Fall through: the turn-0 / non-contest path. Other opcodes are timing only.
            0x06 | 0x07 | 0x10 | 0x12 | 0x14 | 0x15 | 0x16 | 0x17 | 0x18 | 0x1A | 0x1B | 0x1C
            | 0x1D | 0x1E | 0x1F | 0x20 | 0x21 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28 | 0x29 | 0x2A
            | 0x2B | 0x2C | 0x2D | 0x2E | 0x2F => {}
            _ => return None,
        }
        pc += len;
    }
}

fn sprite_cmd(rom: &Rom, pc: u32, ids: &Ids) -> Option<Cmd> {
    let template = rom.ptr(pc + 1).ok()??;
    let nargs = rom.u8(pc + 6).ok()? as usize;
    let mut args = [0i16; 8];
    for i in 0..nargs.min(8) {
        args[i] = rom.i16(pc + 7 + i as u32 * 2).ok()?;
    }
    let cb = rom.u32(template + 20).ok()?;
    let tile = rom.u16(template).ok()?;
    if cb == ids.lunge && nargs >= 2 {
        let frames = args[0].clamp(1, 255) as u8;
        return Some(Cmd::Lunge {
            frames,
            delta: args[1],
        });
    }
    if cb == ids.splat && nargs >= 4 {
        return Some(Cmd::HitSplat {
            tag: tile,
            x: args[0],
            y: args[1],
            relative: args[2].clamp(0, 255) as u8,
            anim: args[3].clamp(0, 255) as u8,
        });
    }
    if tile != 0 && tile != 0xFFFF && nargs >= 2 {
        return Some(Cmd::Show {
            tag: tile,
            x: args[0],
            y: args[1],
            relative: if nargs >= 3 {
                args[2].clamp(0, 1) as u8
            } else {
                1
            },
        });
    }
    None
}

fn task_cmd(rom: &Rom, pc: u32, ids: &Ids) -> Option<Cmd> {
    let func = rom.u32(pc + 1).ok()?;
    if func != ids.shake {
        return None;
    }
    let nargs = rom.u8(pc + 6).ok()? as usize;
    if nargs < 5 {
        return None;
    }
    let arg = |i: usize| rom.i16(pc + 7 + i as u32 * 2).ok();
    Some(Cmd::Shake {
        who: arg(0)?.clamp(0, 255) as u8,
        x: arg(1)?,
        y: arg(2)?,
        times: arg(3)?.clamp(0, 255) as u8,
        delay: arg(4)?.clamp(0, 255) as u8,
    })
}

fn cmd_len(rom: &Rom, pc: u32) -> Option<u32> {
    let op = rom.u8(pc).ok()?;
    let fixed = match op {
        0x00 | 0x01 | 0x09 | 0x0C | 0x1E => 3,
        0x04 | 0x0A | 0x0B | 0x14 | 0x18 | 0x1A | 0x22 | 0x23 | 0x28 | 0x2A | 0x2B | 0x2C
        | 0x2D | 0x2E => 2,
        0x05 | 0x06 | 0x07 | 0x08 | 0x0D | 0x0F | 0x15 | 0x16 | 0x17 | 0x20 | 0x29 | 0x2F => 1,
        0x0E | 0x13 | 0x24 => 5,
        0x10 | 0x19 | 0x25 => 4,
        0x11 => 9,
        0x12 => 6,
        0x1B | 0x26 | 0x27 => 7,
        0x1C => 6,
        0x1D => 5,
        0x21 => 8,
        0x02 | 0x03 => {
            let nargs = rom.u8(pc + 6).ok()?;
            return Some(7 + u32::from(nargs) * 2);
        }
        0x1F => {
            let nargs = rom.u8(pc + 5).ok()?;
            return Some(6 + u32::from(nargs) * 2);
        }
        _ => return None,
    };
    Some(fixed)
}

fn tiles_4bpp(bytes: &[u8]) -> Vec<[u32; 8]> {
    bytes
        .chunks(32)
        .filter(|c| c.len() == 32)
        .map(|c| {
            core::array::from_fn(|row| {
                let i = row * 4;
                u32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]])
            })
        })
        .collect()
}
