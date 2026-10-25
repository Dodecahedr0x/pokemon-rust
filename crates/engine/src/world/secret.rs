//! Secret bases (Gen3 `gSaveBlock1Ptr->secretBases`, `sCurSecretBaseId`) and the decoration
//! inventory. The records, the entrance the player is facing, and placing a decoration are
//! engine state. Which map an entrance warps to, and the metatile a field effect swaps, stay
//! in content.

use super::State;
use super::battle::{Foes, Origin};
use crate::battle::{BattleKind, BattleOutcome, BattleRules};
use crate::data::{
    BLOCK_COLLISION_SHIFT, BLOCK_METATILE_MASK, Behavior, GameData, SpeciesId, TextId, TrainerId,
};
use crate::hash::Fnv64;
use crate::mon::Party;
use crate::objects::Terrain;
use crate::script::SecretBaseCmd;
use crate::video::Frame;
use alloc::vec::Vec;

/// Gen3 `SECRET_BASES_COUNT`.
const BASES: usize = 20;
/// Gen3 `DECOR_MAX_SECRET_BASE`.
const DECOR_MAX: usize = 16;
/// The decoration category arrays together (`DECOR_MAX_*` in `decoration.h`).
const INVENTORY_MAX: usize = 150;

const ELEVATION_MASK: u16 = 0xF000;
const COLLISION_MASK: u16 = 3 << BLOCK_COLLISION_SHIFT;

#[derive(Clone, Debug, Default)]
struct Base {
    id: u16,
    gender: u8,
    trainer_id: u32,
    battled: bool,
    entered: u8,
    /// `(decoration, x << 8 | y)` on this base, at most [`DECOR_MAX`].
    decorations: Vec<(u8, u16)>,
    /// `(species, level)` snapshotted from a party.
    party: Vec<(u16, u8)>,
}

/// The 20 records, the decoration inventory, and the entrance currently faced.
#[derive(Clone, Debug)]
pub(crate) struct SecretBases {
    cur: u16,
    inventory: Vec<u8>,
    bases: [Base; BASES],
}

impl Default for SecretBases {
    fn default() -> Self {
        Self {
            cur: 0,
            inventory: Vec::new(),
            bases: core::array::from_fn(|_| Base::default()),
        }
    }
}

impl SecretBases {
    /// No entrance faced, no inventory, no registered base.
    pub(crate) fn is_empty(&self) -> bool {
        self.cur == 0 && self.inventory.is_empty() && self.bases.iter().all(|b| b.id == 0)
    }

    pub(crate) fn set_cur(&mut self, id: u16) {
        self.cur = id;
    }

    /// The record whose entrance id is `id`.
    pub(crate) fn find(&self, id: u16) -> Option<u16> {
        if id == 0 {
            return None;
        }
        self.bases.iter().position(|b| b.id == id).map(|i| i as u16)
    }

    /// The player's base (record 0).
    #[cfg(test)]
    pub(crate) fn player_base_id(&self) -> u16 {
        self.bases[0].id
    }

    /// The first decoration on record `index`.
    #[cfg(test)]
    pub(crate) fn decoration(&self, index: u16) -> Option<u8> {
        self.bases
            .get(index as usize)
            .and_then(|b| b.decorations.first().map(|d| d.0))
    }

    pub(crate) fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.cur);
        h.write(&[self.inventory.len() as u8]);
        h.write(&self.inventory);
        for (i, b) in self.bases.iter().enumerate() {
            if b.id == 0 {
                continue;
            }
            h.write(&[i as u8, b.gender, b.battled as u8, b.entered]);
            h.write_u16(b.id);
            h.write_u32(b.trainer_id);
            for &(decor, pos) in &b.decorations {
                h.write(&[decor]);
                h.write_u16(pos);
            }
            for &(species, level) in &b.party {
                h.write_u16(species);
                h.write(&[level]);
            }
        }
    }

    pub(crate) fn to_snap(&self) -> Option<crate::checkpoint::SecretBasesSnap> {
        if self.is_empty() {
            return None;
        }
        Some(crate::checkpoint::SecretBasesSnap {
            cur: self.cur,
            inventory: self.inventory.clone(),
            bases: self
                .bases
                .iter()
                .enumerate()
                .filter(|(_, b)| b.id != 0)
                .map(|(i, b)| crate::checkpoint::SecretBaseSnap {
                    index: i as u8,
                    id: b.id,
                    gender: b.gender,
                    trainer_id: b.trainer_id,
                    battled: b.battled,
                    entered: b.entered,
                    decorations: b.decorations.clone(),
                    party: b.party.clone(),
                })
                .collect(),
        })
    }

    pub(crate) fn from_snap(snap: Option<crate::checkpoint::SecretBasesSnap>) -> Self {
        let Some(snap) = snap else {
            return Self::default();
        };
        let mut out = Self {
            cur: snap.cur,
            inventory: snap.inventory,
            ..Self::default()
        };
        for b in snap.bases {
            let Some(slot) = out.bases.get_mut(b.index as usize) else {
                continue;
            };
            *slot = Base {
                id: b.id,
                gender: b.gender,
                trainer_id: b.trainer_id,
                battled: b.battled,
                entered: b.entered,
                decorations: b.decorations,
                party: b.party,
            };
        }
        out
    }
}

/// [`crate::script::Request::SecretBase`].
pub(in crate::world) fn command<G: GameData>(
    s: &mut State,
    game: &G,
    cmd: SecretBaseCmd,
    f: &mut Frame,
) -> u16 {
    let player_id = s.player_id;
    let bases = &mut s.secret_bases;
    match cmd {
        SecretBaseCmd::HasPlayer => (bases.bases[0].id != 0) as u16,
        SecretBaseCmd::Cur => bases.cur,
        SecretBaseCmd::Claim => {
            let id = bases.cur;
            if id == 0 {
                return 0;
            }
            let b = &mut bases.bases[0];
            b.id = id;
            b.gender = 0;
            b.trainer_id = player_id;
            1
        }
        SecretBaseCmd::Entered(index) => {
            let Some(b) = bases.bases.get_mut(index as usize) else {
                return 0;
            };
            b.entered = b.entered.saturating_add(1);
            b.id
        }
        SecretBaseCmd::ClearPlayer => {
            bases.bases[0] = Base::default();
            0
        }
        SecretBaseCmd::OwnedByOther => (bases.bases[0].id != bases.cur) as u16,
        SecretBaseCmd::GiveDecor(id) => {
            if id == 0 || bases.inventory.len() >= INVENTORY_MAX {
                return 0;
            }
            bases.inventory.push(id);
            1
        }
        SecretBaseCmd::PlaceFront(index) => place_front(s, game, index, f),
        SecretBaseCmd::OwnerType(index) => {
            let Some(b) = bases.bases.get(index as usize) else {
                return 0;
            };
            u16::from(b.trainer_id as u8 % 5) + u16::from(b.gender) * 5
        }
        SecretBaseCmd::Battled { index, set } => {
            let Some(b) = bases.bases.get_mut(index as usize) else {
                return 0;
            };
            if set == 0xFFFF {
                b.battled as u16
            } else {
                b.battled = set != 0;
                b.battled as u16
            }
        }
        SecretBaseCmd::SnapshotParty => {
            let party: Vec<(u16, u8)> = s.party.iter().map(|m| (m.species.0, m.level)).collect();
            let n = party.len() as u16;
            if let Some(b) = s.secret_bases.bases.get_mut(0) {
                b.party = party;
            }
            n
        }
        SecretBaseCmd::TypeInFront => type_in_front(s, game),
        SecretBaseCmd::ClearBattled => {
            for b in &mut bases.bases {
                b.battled = false;
            }
            0
        }
    }
}

/// The first owned decoration onto record `index`, at the cell in front of the player.
fn place_front<G: GameData>(s: &mut State, game: &G, index: u16, f: &mut Frame) -> u16 {
    let Some(&decor) = s.secret_bases.inventory.first() else {
        return 0;
    };
    let Some(b) = s.secret_bases.bases.get_mut(index as usize) else {
        return 0;
    };
    if b.decorations.len() >= DECOR_MAX {
        return 0;
    }
    let (x, y) = s.objects.player().pos();
    let (dx, dy) = s.objects.player().facing.delta();
    let (x, y) = (x + dx, y + dy);
    let pos = (x as u8 as u16) << 8 | y as u8 as u16;
    b.decorations.push((decor, pos));
    s.secret_bases.inventory.remove(0);
    let old = s.field.block(game, x, y);
    let block = (old & ELEVATION_MASK) | (old & BLOCK_METATILE_MASK) | COLLISION_MASK;
    s.field.set_block(game, x, y, block, f);
    decor as u16
}

fn type_in_front<G: GameData>(s: &State, game: &G) -> u16 {
    let (x, y) = s.objects.player().pos();
    let (dx, dy) = s.objects.player().facing.delta();
    match s.ground(game).behavior(x + dx, y + dy) {
        Behavior::SecretBase(spot) => spot.entrance_type(),
        _ => 0,
    }
}

/// [`crate::script::Request::SecretBaseBattle`]: the record's party, no whiteout.
/// `Some` is the outcome answered at once (no mon on either side).
pub(in crate::world) fn start_battle<G: GameData>(
    s: &mut State,
    game: &G,
    index: u16,
    f: &mut Frame,
) -> Option<u16> {
    let won = BattleOutcome::Won as u16;
    if !s.can_battle() {
        return Some(won);
    }
    let Some(record) = s.secret_bases.bases.get(index as usize) else {
        return Some(won);
    };
    let party = record.party.clone();
    let mut opponents = Party::default();
    for (species, level) in party {
        if species == 0 {
            continue;
        }
        let mon = s.create_mon(game, SpeciesId(species), level);
        if opponents.push(mon).is_err() {
            break;
        }
    }
    if opponents.is_empty() {
        return Some(won);
    }
    let kind = BattleKind::Trainer {
        trainer: TrainerId(0),
        defeat: TextId(0),
    };
    s.start_battle_with(
        game,
        kind,
        Foes::Ready(opponents),
        None,
        BattleRules::default(),
        Origin::Script,
        f,
    );
    None
}
