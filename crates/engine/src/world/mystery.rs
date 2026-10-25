//! Wonder Cards (Gen3 `SaveWonderCard` + `InitRamScript_NoObjectEvent`).
//!
//! A delivered card points at a script. `trywondercardscript` runs that script when the card
//! passes `ValidateWonderCard` (`GetSavedRamScriptIfValid`). The card and the mystery-event
//! status are session state: they are not in the checkpoint DTO and not in [`World::hash`], so
//! a restore clears them and the committed route hashes stay put.

use super::State;
use crate::data::ScriptId;

/// Gen3 `MEVENT_STATUS_LOAD_OK`.
pub const MEVENT_STATUS_LOAD_OK: u8 = 0;
/// Gen3 `MEVENT_STATUS_LOAD_ERROR`.
pub const MEVENT_STATUS_LOAD_ERROR: u8 = 1;
/// Gen3 `MEVENT_STATUS_SUCCESS`.
pub const MEVENT_STATUS_SUCCESS: u8 = 2;
/// Gen3 `MEVENT_STATUS_FAILURE`.
pub const MEVENT_STATUS_FAILURE: u8 = 3;

/// Gen3 `CARD_TYPE_COUNT`.
const CARD_TYPE_COUNT: u8 = 3;
/// Gen3 `NUM_WONDER_BGS`.
const NUM_WONDER_BGS: u8 = 8;
/// Gen3 `MAX_STAMP_CARD_STAMPS`.
const MAX_STAMP_CARD_STAMPS: u8 = 7;

/// A saved Wonder Card (`struct WonderCard`), the fields `ValidateWonderCard` reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WonderCard {
    /// Event flag id. Zero is not a card.
    pub flag_id: u16,
    pub icon_species: u16,
    pub id_number: u32,
    /// `CARD_TYPE_*` (0 gift, 1 stamp, 2 link stat).
    pub card_type: u8,
    /// Background, below `NUM_WONDER_BGS`.
    pub bg_type: u8,
    /// `SEND_TYPE_*` (0 disallowed, 1 allowed, 2 always).
    pub send_type: u8,
    pub max_stamps: u8,
}

impl WonderCard {
    /// Gen3 `ValidateWonderCard`.
    pub(super) fn is_valid(self) -> bool {
        self.flag_id != 0
            && self.card_type < CARD_TYPE_COUNT
            && self.bg_type < NUM_WONDER_BGS
            && self.max_stamps <= MAX_STAMP_CARD_STAMPS
            && self.send_type <= 2
    }
}

/// The card and the RAM script it points at (`InitRamScript_NoObjectEvent`: no map object).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SavedWonderCard {
    pub(super) card: WonderCard,
    pub(super) script: ScriptId,
}

impl State {
    /// Gen3 `SaveWonderCard`: stores `card` and the script it points at. An invalid card is
    /// refused and leaves a card already saved in place.
    pub(super) fn deliver_wonder_card(&mut self, card: WonderCard, script: ScriptId) -> bool {
        // `0xFFFF` is the "no script" answer of `WonderCardScript`, not a deliverable id.
        if !card.is_valid() || script.0 == crate::script::NO_WONDER_CARD_SCRIPT {
            return false;
        }
        self.wonder_card = Some(SavedWonderCard { card, script });
        true
    }
}
