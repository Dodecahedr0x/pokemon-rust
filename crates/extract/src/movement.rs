//! Movement scripts (`applymovement` data): one `MOVEMENT_ACTION_*` byte per step, ended by
//! `MOVEMENT_ACTION_STEP_END` (`include/constants/event_object_movement.h`). Each action becomes
//! the source of an `engine::script::Step` expression; actions with no `Step` equivalent become
//! `Step::Delay(0)` and are reported.

use crate::{Rom, names};
use anyhow::{Result, bail};

pub const STEP_END: u8 = 0xFE;
/// Guard against runaway data.
const MAX_LEN: u32 = 1024;

/// The action bytes of the movement script at `off`, without the terminator.
pub fn decode(rom: &Rom, off: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for i in 0..MAX_LEN {
        let b = rom.u8(off + i)?;
        if b == STEP_END {
            return Ok(out);
        }
        out.push(b);
    }
    bail!("movement at {off:#x} has no MOVEMENT_ACTION_STEP_END within {MAX_LEN} bytes")
}

/// `Dir` for the 4-direction action groups, in Gen3 `DIR_SOUTH..DIR_EAST` order.
const DIRS: [&str; 4] = ["Dir::Down", "Dir::Up", "Dir::Left", "Dir::Right"];

/// The `Step` expression of one action, or `None` if it has no `Step` equivalent.
pub fn step(action: u8) -> Option<String> {
    let d = |base: u8| DIRS[(action - base) as usize];
    Some(match action {
        0x00..=0x03 => format!("Step::Face({})", d(0x00)),
        0x04..=0x07 => format!("Step::Walk({}, Speed::Slow)", d(0x04)),
        0x08..=0x0B => format!("Step::Walk({}, Speed::Normal)", d(0x08)),
        0x0C..=0x0F => format!("Step::Jump({}, 2)", d(0x0C)),
        0x10..=0x14 => format!("Step::Delay({})", 1u8 << (action - 0x10)),
        0x15..=0x18 => format!("Step::Walk({}, Speed::Fast)", d(0x15)),
        0x19..=0x1C => format!("Step::WalkInPlace({}, Speed::Slow)", d(0x19)),
        0x1D..=0x20 => format!("Step::WalkInPlace({}, Speed::Normal)", d(0x1D)),
        0x21..=0x24 => format!("Step::WalkInPlace({}, Speed::Fast)", d(0x21)),
        0x25..=0x28 => format!("Step::WalkInPlace({}, Speed::Faster)", d(0x25)),
        0x2D..=0x30 => format!("Step::Walk({}, Speed::Faster)", d(0x2D)),
        // PLAYER_RUN: the player's run speed is walk_fast's.
        0x35..=0x38 => format!("Step::Walk({}, Speed::Fast)", d(0x35)),
        0x3E => "Step::FacePlayer".to_string(),
        0x3F => "Step::FaceAwayFromPlayer".to_string(),
        0x40 => "Step::LockFacing".to_string(),
        0x41 => "Step::UnlockFacing".to_string(),
        0x42..=0x45 => format!("Step::Jump({}, 1)", d(0x42)),
        0x46..=0x49 => format!("Step::Jump({}, 0)", d(0x46)),
        0x54 => "Step::Hide".to_string(),
        0x55 => "Step::Show".to_string(),
        0x56 => "Step::Emote(Emote::Exclamation)".to_string(),
        0x57 => "Step::Emote(Emote::Question)".to_string(),
        0x58 => "Step::Emote(Emote::Heart)".to_string(),
        _ => return None,
    })
}

/// The `[Step; N]` array literal of `actions`, one step per line at `indent`, each commented
/// with its `MOVEMENT_ACTION_*` name. Unsupported actions become `Step::Delay(0)`; their names
/// are pushed to `unknown`.
pub fn array_literal(actions: &[u8], indent: &str, unknown: &mut Vec<String>) -> String {
    let mut s = String::from("[\n");
    for &a in actions {
        let name = names::movement_action_name(a);
        let short = name.strip_prefix("MOVEMENT_ACTION_").unwrap_or(&name);
        match step(a) {
            Some(e) => s.push_str(&format!("{indent}    {e}, // {short}\n")),
            None => {
                s.push_str(&format!(
                    "{indent}    Step::Delay(0), // {short} (unsupported)\n"
                ));
                unknown.push(name);
            }
        }
    }
    s.push_str(indent);
    s.push(']');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_map_to_steps() {
        assert_eq!(step(0x00).unwrap(), "Step::Face(Dir::Down)");
        assert_eq!(step(0x09).unwrap(), "Step::Walk(Dir::Up, Speed::Normal)");
        assert_eq!(step(0x14).unwrap(), "Step::Delay(16)");
        assert_eq!(step(0x10).unwrap(), "Step::Delay(1)");
        assert_eq!(step(0x18).unwrap(), "Step::Walk(Dir::Right, Speed::Fast)");
        assert_eq!(
            step(0x26).unwrap(),
            "Step::WalkInPlace(Dir::Up, Speed::Faster)"
        );
        assert_eq!(step(0x0E).unwrap(), "Step::Jump(Dir::Left, 2)");
        assert_eq!(step(0x45).unwrap(), "Step::Jump(Dir::Right, 1)");
        assert_eq!(step(0x49).unwrap(), "Step::Jump(Dir::Right, 0)");
        assert_eq!(step(0x56).unwrap(), "Step::Emote(Emote::Exclamation)");
        assert_eq!(step(0x3E).unwrap(), "Step::FacePlayer");
        assert_eq!(step(0x51), None); // DISABLE_JUMP_LANDING_GROUND_EFFECT
    }

    #[test]
    fn array_literal_comments_and_reports_unknown_actions() {
        let mut unknown = Vec::new();
        let s = array_literal(&[0x08, 0x51], "", &mut unknown);
        assert_eq!(
            s,
            "[\n    Step::Walk(Dir::Down, Speed::Normal), // WALK_NORMAL_DOWN\n    \
             Step::Delay(0), // DISABLE_JUMP_LANDING_GROUND_EFFECT (unsupported)\n]"
        );
        assert_eq!(
            unknown,
            ["MOVEMENT_ACTION_DISABLE_JUMP_LANDING_GROUND_EFFECT"]
        );
    }

    #[test]
    fn decode_stops_at_step_end() {
        let mut b = vec![0u8; 0x100];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b.extend([0x08, 0x09, STEP_END, 0x0A]);
        b.resize(0x200, 0);
        let rom = Rom::from_bytes(b).unwrap();
        assert_eq!(decode(&rom, 0x100).unwrap(), [0x08, 0x09]);
        assert!(decode(&rom, 0x104).is_err());
    }
}
