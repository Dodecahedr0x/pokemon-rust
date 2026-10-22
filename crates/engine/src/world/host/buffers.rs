//! String buffers (owned by Milestone 8 Task H): Gen3 `buffer*` commands into `gStringVar1..3`
//! (`scrcmd.c`), printed by `{STR_VAR_1}`..`{STR_VAR_3}` (`text::expand_vars`).
//!
//! A source the game cannot resolve (an unknown species, item, move, std string or party slot,
//! or a string var past the third) writes an empty string and raises [`Report::Degraded`]; Gen3
//! would copy whatever its tables hold there.

use super::items::report;
use super::{Host, UNIT};
use crate::data::GameData;
use crate::mon::NAME_LEN;
use crate::script::{BufferSrc, Report, Request, Response};
use crate::str_vars::STR_VARS;

/// Gen3 `CHAR_0`; the digits follow it.
const CHAR_0: u8 = 0xA1;
const EOS: u8 = 0xFF;

/// Gen3 `ConvertIntToDecimalStringN(dest, n, STR_CONV_MODE_LEFT_ALIGN, CountDigits(n))`: the
/// digits of `n`, no padding, 0xFF-terminated.
pub(crate) fn decimal(n: u32) -> ([u8; 11], usize) {
    let mut digits = [EOS; 11];
    let mut len = 0;
    let mut v = n;
    loop {
        digits[len] = CHAR_0 + (v % 10) as u8;
        len += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    digits[..len].reverse();
    (digits, len)
}

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let Request::Buffer { slot, src } = *r else {
        return UNIT;
    };
    let game = h.game;
    let s = &mut *h.s;
    if slot as usize >= STR_VARS {
        report(s, Report::Degraded("buffer: string var out of range"));
        return UNIT;
    }
    let species_name = |id: crate::data::SpeciesId| {
        let info = game.battle().species.get(id.0 as usize);
        info.map(|i| i.name)
    };
    let (text, what): (Option<&[u8]>, &'static str) = match src {
        // Gen3 `bufferspeciesname`: `gSpeciesNames[species]`.
        BufferSrc::Species(id) => (species_name(id), "bufferspeciesname: unknown species"),
        // Gen3 `bufferitemname`: `CopyItemName`.
        BufferSrc::Item(id) => (
            game.item(id).map(|i| i.name),
            "bufferitemname: unknown item",
        ),
        // Gen3 `buffermovename`: `gMoveNames[move]`.
        BufferSrc::Move(id) => (
            game.battle().moves.get(id.0 as usize).map(|m| m.name),
            "buffermovename: unknown move",
        ),
        BufferSrc::Number(n) => {
            let (digits, len) = decimal(n);
            s.str_vars.set(slot as usize, &digits[..len]);
            return UNIT;
        }
        // Gen3 `bufferstdstring`: `gStdStrings[id]`.
        BufferSrc::Std(id) => (game.std_string(id), "bufferstdstring: unknown string"),
        // Gen3 `bufferpartymonnick`: `MON_DATA_NICKNAME`, cut by `StringGet_Nickname`.
        BufferSrc::PartyNick(i) => {
            if let Some(mon) = s.party.get(i as usize) {
                let nick = mon.nickname;
                s.str_vars.set(slot as usize, &nick[..NAME_LEN - 1]);
                return UNIT;
            }
            (None, "bufferpartymonnick: empty party slot")
        }
        // Gen3 `bufferleadmonspeciesname`: the species of `GetLeadMonIndex` (the first mon; no
        // eggs yet).
        BufferSrc::LeadSpecies => (
            s.party.get(0).and_then(|m| species_name(m.species)),
            "bufferleadmonspeciesname: empty party",
        ),
        // Gen3 `bufferstring`: copied as is (its placeholders expand when printed).
        BufferSrc::Text(t) => (Some(game.text(t)), ""),
    };
    match text {
        Some(t) => s.str_vars.set(slot as usize, t),
        None => {
            s.str_vars.set(slot as usize, &[]);
            report(s, Report::Degraded(what));
        }
    }
    UNIT
}
