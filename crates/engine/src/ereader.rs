//! e-Reader mystery-event scripts (`mystery_event_script.c`).
//!
//! A payload is the bytecode `RunMysteryEventScript` runs. `setenigmaberry` copies the
//! 52-byte `struct EnigmaBerry` (`global.berry.h`); `IsEnigmaBerryValid` is its stage,
//! yield and checksum. The other commands are consumed so a card stays aligned. Ribbons,
//! Pokémon, trainers and embedded field scripts are not applied.

/// `sizeof(struct EnigmaBerry)`: `Berry2` (28), `itemEffect[18]`, hold effect, its param,
/// and the `u32` checksum.
pub const ENIGMA_BERRY_LEN: usize = 52;
/// `BERRY_NAME_LENGTH + 1`: the name and its 0xFF terminator.
pub const BERRY_NAME_FIELD: usize = 7;
/// `Berry2.maxYield`.
const MAX_YIELD: usize = 10;
/// `Berry2.stageDuration`.
const STAGE_DURATION: usize = 20;
/// `EnigmaBerry.checksum`, the byte sum of everything before it.
const CHECKSUM: usize = 48;

/// `MEVENT_STATUS_LOAD_OK`.
pub const STATUS_LOAD_OK: u16 = 0;
/// `MEVENT_STATUS_LOAD_ERROR`.
pub const STATUS_LOAD_ERROR: u16 = 1;
/// `MEVENT_STATUS_SUCCESS`.
pub const STATUS_SUCCESS: u16 = 2;
/// `MEVENT_STATUS_FAILURE` (`SetIncompatible`).
pub const STATUS_FAILURE: u16 = 3;

/// Emerald's `VERSION_MASK` (`1 << 9` in `mystery_event_script.c`).
const VERSION_EMERALD: u32 = 1 << 9;

const OP_NOP: u8 = 0;
const OP_CHECKCOMPAT: u8 = 1;
const OP_END: u8 = 2;
const OP_SETMSG: u8 = 3;
const OP_SETSTATUS: u8 = 4;
const OP_RUNSCRIPT: u8 = 5;
const OP_INITRAMSCRIPT: u8 = 6;
const OP_SETENIGMABERRY: u8 = 7;
const OP_GIVERIBBON: u8 = 8;
const OP_GIVENATIONALDEX: u8 = 9;
const OP_ADDRAREWORD: u8 = 10;
const OP_SETRECORDMIXINGGIFT: u8 = 11;
const OP_GIVEPOKEMON: u8 = 12;
const OP_ADDTRAINER: u8 = 13;
const OP_ENABLERESETRTC: u8 = 14;
const OP_CHECKSUM: u8 = 15;
const OP_CRC: u8 = 16;

/// What `RunMysteryEventScript` leaves behind. `berry` is set when `setenigmaberry` copied
/// its 52 bytes, valid or not (`SetEnigmaBerry` always copies).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    pub status: u16,
    pub berry: Option<[u8; ENIGMA_BERRY_LEN]>,
}

/// The saved berry's name field (`GetBerryNameByBerryType` reads it when the berry is valid).
pub fn name(raw: &[u8; ENIGMA_BERRY_LEN]) -> &[u8] {
    &raw[..BERRY_NAME_FIELD]
}

/// `IsEnigmaBerryValid`: a non-zero stage and yield, and a checksum equal to the byte sum
/// of the struct before it (`GetEnigmaBerryChecksum`).
pub fn is_valid(raw: &[u8; ENIGMA_BERRY_LEN]) -> bool {
    if raw[STAGE_DURATION] == 0 || raw[MAX_YIELD] == 0 {
        return false;
    }
    checksum(raw) == u32::from_le_bytes(raw[CHECKSUM..].try_into().unwrap())
}

fn checksum(raw: &[u8; ENIGMA_BERRY_LEN]) -> u32 {
    raw[..CHECKSUM].iter().map(|&b| u32::from(b)).sum()
}

/// `CalcCRC16` (`util.c`): seed `0x1121`, then eight right-shifts per byte with polynomial
/// `0x8408`, returned inverted.
fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0x1121u16;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x8408;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

struct Vm<'a> {
    script: &'a [u8],
    pc: usize,
    /// `ctx->mOffset` (`checkcompat`'s first word): pointers are `word - offset`.
    offset: u32,
    status: u16,
    valid: bool,
    stopped: bool,
    berry: Option<[u8; ENIGMA_BERRY_LEN]>,
}

impl<'a> Vm<'a> {
    fn new(script: &'a [u8]) -> Self {
        Self {
            script,
            pc: 0,
            offset: 0,
            status: STATUS_LOAD_OK,
            valid: false,
            stopped: false,
            berry: None,
        }
    }

    fn fail_load(&mut self) {
        self.status = STATUS_LOAD_ERROR;
        self.stopped = true;
    }

    fn read(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pc.checked_add(n)?;
        let bytes = self.script.get(self.pc..end)?;
        self.pc = end;
        Some(bytes)
    }

    fn u8(&mut self) -> Option<u8> {
        self.read(1).map(|b| b[0])
    }

    fn u16(&mut self) -> Option<u16> {
        self.read(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
    }

    fn u32(&mut self) -> Option<u32> {
        self.read(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }

    /// `ptr - mOffset + mScriptBase`, with the script based at 0.
    fn reloc(&self, word: u32) -> Option<usize> {
        let at = word.wrapping_sub(self.offset) as usize;
        (at <= self.script.len()).then_some(at)
    }

    fn range(&self, start: u32, end: u32) -> Option<&'a [u8]> {
        let start = self.reloc(start)?;
        let end = self.reloc(end)?;
        self.script.get(start..end)
    }

    /// One command. `true` yields to the outer loop (`RunScriptCommand`'s TRUE).
    fn command(&mut self) -> bool {
        if self.stopped {
            return false;
        }
        let Some(op) = self.u8() else {
            self.fail_load();
            return true;
        };
        match op {
            OP_NOP => false,
            OP_CHECKCOMPAT => self.checkcompat(),
            OP_END => {
                self.stopped = true;
                true
            }
            OP_SETMSG => self.skip(1 + 4),
            OP_SETSTATUS => match self.u8() {
                Some(status) => {
                    self.status = u16::from(status);
                    false
                }
                None => {
                    self.fail_load();
                    true
                }
            },
            OP_RUNSCRIPT => self.skip(4),
            OP_INITRAMSCRIPT => self.skip(1 + 1 + 1 + 4 + 4),
            OP_SETENIGMABERRY => self.set_enigma_berry(),
            OP_GIVERIBBON => self.skip(1 + 1),
            OP_GIVENATIONALDEX | OP_ENABLERESETRTC => false,
            OP_ADDRAREWORD => self.skip(1),
            OP_SETRECORDMIXINGGIFT => self.skip(1 + 1 + 2),
            OP_GIVEPOKEMON | OP_ADDTRAINER => self.skip(4),
            OP_CHECKSUM => self.check_sum(false),
            OP_CRC => self.check_sum(true),
            _ => {
                self.fail_load();
                true
            }
        }
    }

    /// Consume `n` operand bytes and keep going. A short read is a load error and yields.
    fn skip(&mut self, n: usize) -> bool {
        if self.read(n).is_none() {
            self.fail_load();
            true
        } else {
            false
        }
    }

    fn checkcompat(&mut self) -> bool {
        let Some(offset) = self.u32() else {
            self.fail_load();
            return true;
        };
        let Some(unk0) = self.u16() else {
            self.fail_load();
            return true;
        };
        let Some(unk1) = self.u32() else {
            self.fail_load();
            return true;
        };
        let Some(unk2) = self.u16() else {
            self.fail_load();
            return true;
        };
        let Some(version) = self.u32() else {
            self.fail_load();
            return true;
        };
        self.offset = offset;
        // `CheckCompatibility`: English (`unk0`/`unk1` bit 0), RS (`unk2` bit 2), Emerald.
        if unk0 & 1 != 0 && unk1 & 1 != 0 && unk2 & 4 != 0 && version & VERSION_EMERALD != 0 {
            self.valid = true;
        } else {
            self.status = STATUS_FAILURE;
        }
        true
    }

    fn set_enigma_berry(&mut self) -> bool {
        let Some(word) = self.u32() else {
            self.fail_load();
            return true;
        };
        // `MEScrCmd_setenigmaberry` copies, reports success, then `IsEnigmaBerryValid`.
        self.status = STATUS_SUCCESS;
        let Some(at) = self.reloc(word) else {
            self.status = STATUS_LOAD_ERROR;
            return false;
        };
        let Some(bytes) = self.script.get(at..at + ENIGMA_BERRY_LEN) else {
            self.status = STATUS_LOAD_ERROR;
            return false;
        };
        let mut raw = [0; ENIGMA_BERRY_LEN];
        raw.copy_from_slice(bytes);
        if !is_valid(&raw) {
            self.status = STATUS_LOAD_ERROR;
        }
        self.berry = Some(raw);
        false
    }

    fn check_sum(&mut self, crc: bool) -> bool {
        let Some(expect) = self.u32() else {
            self.fail_load();
            return true;
        };
        let Some(start) = self.u32() else {
            self.fail_load();
            return true;
        };
        let Some(end) = self.u32() else {
            self.fail_load();
            return true;
        };
        let ok = match self.range(start, end) {
            Some(data) if crc => u32::from(crc16(data)) == expect,
            Some(data) => data.iter().map(|&b| u32::from(b)).sum::<u32>() == expect,
            None => false,
        };
        if !ok {
            self.valid = false;
            self.status = STATUS_LOAD_ERROR;
        }
        true
    }

    /// `RunScriptCommand`: commands until one yields. A script of only non-yielding commands
    /// stops as a load error instead of spinning.
    fn inner(&mut self) -> bool {
        let mut steps = 0;
        loop {
            if self.stopped {
                return false;
            }
            steps += 1;
            if steps > self.script.len().saturating_add(1) {
                self.fail_load();
                return true;
            }
            if self.command() {
                return true;
            }
        }
    }
}

/// `RunMysteryEventScript`. The outer loop continues only while a command yielded and the
/// card passed `checkcompat` (`ctx->mValid`).
pub fn run(script: &[u8]) -> Outcome {
    let mut vm = Vm::new(script);
    let mut steps = 0;
    loop {
        steps += 1;
        if steps > script.len().saturating_add(2) {
            vm.fail_load();
            break;
        }
        let yielded = vm.inner();
        if !yielded || !vm.valid {
            break;
        }
    }
    Outcome {
        status: vm.status,
        berry: vm.berry,
    }
}

#[cfg(test)]
#[path = "ereader_tests.rs"]
mod tests;
