//! Emerald's Battle Tower challenge record (Gen3 `SaveBlock2.frontier`, the tower half of
//! `frontier_util.c` / `battle_tower.c`). Dome, Palace, Arena, Factory, Pike and Pyramid are
//! not stored here: their util funcs stay unsupported in the content specials.

use crate::data::SpeciesId;
use crate::hash::Fnv64;
use crate::mon::{BattleTables, Ivs, Party, Pokemon};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

/// Gen3 `FRONTIER_FACILITY_TOWER`.
pub const FACILITY_TOWER: u16 = 0;
/// Gen3 `FRONTIER_MODE_SINGLES`.
pub const MODE_SINGLES: u16 = 0;
/// Gen3 `FRONTIER_MODE_MULTIS`.
pub const MODE_MULTIS: u16 = 2;
/// Gen3 `FRONTIER_MODE_LINK_MULTIS`.
pub const MODE_LINK: u16 = 3;
/// Gen3 `FRONTIER_STAGES_PER_CHALLENGE`.
pub const STAGES: u16 = 7;
/// Gen3 `MAX_STREAK` / `MAX_BATTLE_FRONTIER_POINTS`.
pub const MAX_STREAK: u16 = 9999;
/// `GetChallengeStatus` writes this when `challengeStatus` is 0.
pub const STATUS_NONE: u16 = 0xFF;
/// Gen3 `CHALLENGE_STATUS_SAVING`.
pub const STATUS_SAVING: u8 = 1;
/// Gen3 `TRAINER_FRONTIER_BRAIN`.
pub const BRAIN_TRAINER: u16 = 1022;
/// Gen3 `SPECIES_EGG`.
pub const SPECIES_EGG: u16 = 412;

const LVL_50: u8 = 0;
const LVL_OPEN: u8 = 1;
const DATA_STATUS: u16 = 0;
const DATA_LVL: u16 = 1;
const DATA_BATTLE_NUM: u16 = 2;
const DATA_PAUSED: u16 = 3;
const DATA_ORDER: u16 = 4;
const DATA_OUTCOME: u16 = 5;
const DATA_RECORD_DISABLED: u16 = 6;
const DATA_HEARD_BRAIN: u16 = 7;
const TOWER_WIN_STREAK: u16 = 1;
const TOWER_STREAK_ACTIVE: u16 = 2;
const TOWER_LVL_MODE: u16 = 3;
const UTIL_GET_STATUS: u16 = 0;
const UTIL_GET_DATA: u16 = 1;
const UTIL_SET_DATA: u16 = 2;
const UTIL_SET_ORDER: u16 = 3;
const UTIL_CHECK_AIR: u16 = 8;
const UTIL_BRAIN: u16 = 9;
const UTIL_IS_BRAIN: u16 = 10;
const UTIL_POINTS: u16 = 11;
const UTIL_INELIGIBLE: u16 = 15;
const UTIL_INCREMENT: u16 = 18;
const UTIL_RESTORE: u16 = 19;
const UTIL_SKETCH: u16 = 22;
const TOWER_INIT: u16 = 0;
const TOWER_GET: u16 = 1;
const TOWER_SET: u16 = 2;
const TOWER_OPPONENT: u16 = 3;
const TOWER_WON: u16 = 4;
const TOWER_RIBBONS: u16 = 5;
const TOWER_SAVE: u16 = 6;
const TOWER_INTRO: u16 = 7;
const TOWER_NOP: u16 = 8;
const TOWER_NOP2: u16 = 9;
const TOWER_LOAD_LINK: u16 = 12;
const TOWER_INTERVIEW: u16 = 15;

/// Trainer-id bands (`sFrontierTrainerIdRanges` / `sFrontierTrainerIdRangesHard`).
const RANGES: [[u16; 2]; 8] = [
    [0, 99],
    [80, 119],
    [100, 139],
    [120, 159],
    [140, 179],
    [160, 199],
    [180, 219],
    [200, 299],
];
const RANGES_HARD: [[u16; 2]; 8] = [
    [100, 119],
    [120, 139],
    [140, 159],
    [160, 179],
    [180, 199],
    [200, 219],
    [220, 239],
    [200, 299],
];
/// First-clear tower awards (`sBattlePointAwards[][TOWER]`, singles, doubles, multis, link).
const TOWER_BP: [[u16; 4]; 8] = [
    [1, 2, 3, 3],
    [2, 3, 4, 4],
    [3, 4, 5, 5],
    [4, 5, 6, 6],
    [5, 6, 7, 7],
    [6, 7, 8, 8],
    [7, 8, 9, 9],
    [8, 9, 10, 10],
];
const BANNED: [u16; 10] = [150, 151, 249, 250, 251, 382, 383, 384, 385, 386];

/// The arguments of one `CallFrontierUtilFunc` / `CallBattleTowerFunc`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Args {
    pub func: u16,
    pub arg: u16,
    pub value: u16,
    pub facility: u16,
    pub mode: u16,
    pub result: u16,
}

/// A frontier request. The world host applies it to [`Frontier`]; a battle waits for the fight.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Call {
    Util(Args),
    Tower(Args),
    /// `SPECIAL_BATTLE_TOWER` singles. The host starts the fight and answers its outcome.
    Battle,
    /// `GetBattleTowerSinglesStreak` (the game stat `IncrementWinStreak` writes).
    SinglesStreak,
}

impl Call {
    /// Hash parts: kind, func, arg, and the rest folded into one word.
    pub fn parts(self) -> (u16, u16, u16, u16) {
        match self {
            Call::Util(a) => (1, a.func, a.arg, fold(a)),
            Call::Tower(a) => (2, a.func, a.arg, fold(a)),
            Call::Battle => (3, 0, 0, 0),
            Call::SinglesStreak => (4, 0, 0, 0),
        }
    }
}

fn fold(a: Args) -> u16 {
    a.value ^ a.facility.wrapping_shl(4) ^ a.mode.wrapping_shl(8) ^ a.result.wrapping_shl(12)
}

/// What the host does with the result of [`Frontier::apply`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Value(u16),
    /// `frontier_setpartyorder`: the first `count` selected slots, then reduce the party.
    Reduce {
        count: u8,
    },
    /// `frontier_resethelditems`: copy held items from the saved party onto the battle party.
    RestoreHeld,
    StartBattle,
}

/// The tower challenge record. `Default` is a new save (no challenge).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Frontier {
    status: u8,
    lvl_mode: u8,
    tower_lvl_mode: u8,
    paused: bool,
    record_disabled: bool,
    heard_brain: bool,
    battle_num: u16,
    outcome: u16,
    selected: [u8; 4],
    streaks: [[u16; 2]; 4],
    records: [[u16; 2]; 4],
    streak_active: u32,
    trainer_ids: [u16; 7],
    opponent: u16,
    battle_points: u16,
    card_points: u16,
    singles_streak: u16,
    num_wins: u16,
    saved_streak: u16,
    saved_lvl: u8,
    saved_species: [u16; 3],
}

impl Frontier {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn battle_points(&self) -> u16 {
        self.battle_points
    }

    pub fn saved_streak(&self) -> u16 {
        self.saved_streak
    }

    pub fn opponent(&self) -> u16 {
        self.opponent
    }

    /// The choose-half order (`selectedPartyMons`), slots + 1.
    pub fn selected(&self) -> [u8; 4] {
        self.selected
    }

    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[0xF7, self.status, self.lvl_mode, self.tower_lvl_mode]);
        h.write(&[
            self.paused as u8,
            self.record_disabled as u8,
            self.heard_brain as u8,
            self.saved_lvl,
        ]);
        for v in [
            self.battle_num,
            self.outcome,
            self.opponent,
            self.battle_points,
            self.card_points,
            self.singles_streak,
            self.num_wins,
            self.saved_streak,
        ] {
            h.write_u16(v);
        }
        h.write(&self.selected);
        for row in self.streaks {
            for v in row {
                h.write_u16(v);
            }
        }
        for row in self.records {
            for v in row {
                h.write_u16(v);
            }
        }
        h.write_u32(self.streak_active);
        for id in self.trainer_ids {
            h.write_u16(id);
        }
        for s in self.saved_species {
            h.write_u16(s);
        }
    }

    pub fn note_outcome(&mut self, outcome: u16) {
        self.outcome = outcome;
    }

    /// Applies `call`. `order` is `gSelectedOrderFromParty` (the choose-half menu's answer).
    /// `party` is the player's current party.
    pub fn apply(&mut self, call: Call, party: &Party, order: &[u8; 4], rng: &mut Rng) -> Step {
        match call {
            Call::Battle => Step::StartBattle,
            Call::SinglesStreak => Step::Value(self.singles_streak),
            Call::Util(a) => self.util(a, party, order),
            Call::Tower(a) => self.tower(a, party, rng),
        }
    }

    /// Three facility-level opponents for the trainer [`Self::opponent`] picked.
    pub fn opponent_party(&self, tables: &BattleTables, rng: &mut Rng) -> Party {
        let n = tables.species.len();
        let mut party = Party::default();
        if n < 2 {
            return party;
        }
        let level = if self.lvl_mode == LVL_OPEN { 100 } else { 50 };
        let iv = fixed_iv(self.opponent);
        let mut used = [0u16; 3];
        for i in 0..3 {
            let mut species = 1u16;
            for _ in 0..24 {
                let s = 1 + (rng.random() as usize % (n - 1));
                if !used[..i].contains(&(s as u16)) {
                    species = s as u16;
                    break;
                }
            }
            used[i] = species;
            let mon = Pokemon::new(
                tables,
                SpeciesId(species),
                level,
                Ivs::Fixed(iv),
                rng.random32(),
                0,
                rng,
            );
            let _ = party.push(mon);
        }
        party
    }

    fn util(&mut self, a: Args, party: &Party, order: &[u8; 4]) -> Step {
        match a.func {
            UTIL_GET_STATUS => Step::Value(self.challenge_status_var()),
            UTIL_GET_DATA => Step::Value(self.get_data(a.arg)),
            UTIL_SET_DATA => {
                self.set_data(a.arg, a.value, order);
                Step::Value(0)
            }
            UTIL_SET_ORDER => Step::Reduce {
                count: (a.arg as u8).min(4),
            },
            UTIL_INELIGIBLE => Step::Value(self.ineligible(party, a)),
            UTIL_INCREMENT if a.facility == FACILITY_TOWER => {
                self.increment(a.mode);
                Step::Value(0)
            }
            UTIL_POINTS if a.facility == FACILITY_TOWER => Step::Value(self.give_points(a.mode)),
            UTIL_CHECK_AIR if a.facility == FACILITY_TOWER => {
                self.check_air(a.mode);
                Step::Value(0)
            }
            UTIL_BRAIN if a.facility == FACILITY_TOWER => Step::Value(self.brain_status(a.mode)),
            UTIL_IS_BRAIN => Step::Value((self.opponent == BRAIN_TRAINER) as u16),
            UTIL_RESTORE => Step::RestoreHeld,
            UTIL_SKETCH => Step::Value(0),
            _ => Step::Value(0),
        }
    }

    fn tower(&mut self, a: Args, party: &Party, rng: &mut Rng) -> Step {
        match a.func {
            TOWER_INIT => {
                self.init(a.mode);
                Step::Value(0)
            }
            TOWER_GET => Step::Value(self.tower_get(a.arg, a.mode)),
            TOWER_SET => {
                self.tower_set(a.arg, a.value, a.mode);
                Step::Value(0)
            }
            TOWER_OPPONENT => {
                self.set_opponent(a.mode, rng);
                Step::Value(self.opponent)
            }
            TOWER_WON => Step::Value(self.battle_won(a.mode)),
            TOWER_RIBBONS => Step::Value((self.streak_of(a.mode) > 55) as u16),
            TOWER_SAVE => {
                self.save(a.arg, a.mode, party);
                Step::Value(0)
            }
            TOWER_INTRO | TOWER_NOP | TOWER_NOP2 | TOWER_INTERVIEW => Step::Value(0),
            TOWER_LOAD_LINK => {
                // The singles lobby calls this with `VAR_RESULT` 0 and waits until it is 6.
                let v = if a.mode != MODE_LINK && a.result == 0 {
                    6
                } else {
                    a.result
                };
                Step::Value(v)
            }
            _ => Step::Value(0),
        }
    }

    fn challenge_status_var(&self) -> u16 {
        match self.status {
            0 => STATUS_NONE,
            s => u16::from(s),
        }
    }

    fn get_data(&mut self, arg: u16) -> u16 {
        match arg {
            DATA_STATUS => u16::from(self.status),
            DATA_LVL => u16::from(self.lvl_mode),
            DATA_BATTLE_NUM => self.battle_num,
            DATA_PAUSED => self.paused as u16,
            DATA_OUTCOME => {
                let v = self.outcome;
                self.outcome = 0;
                v
            }
            DATA_RECORD_DISABLED => self.record_disabled as u16,
            DATA_HEARD_BRAIN => self.heard_brain as u16,
            _ => 0,
        }
    }

    fn set_data(&mut self, arg: u16, value: u16, order: &[u8; 4]) {
        match arg {
            DATA_STATUS => self.status = value as u8,
            DATA_LVL => self.lvl_mode = (value as u8).min(LVL_OPEN),
            DATA_BATTLE_NUM => self.battle_num = value,
            DATA_PAUSED => self.paused = value != 0,
            DATA_ORDER => self.selected = *order,
            DATA_RECORD_DISABLED => self.record_disabled = value != 0,
            DATA_HEARD_BRAIN => self.heard_brain = true,
            _ => {}
        }
    }

    fn ineligible(&mut self, party: &Party, a: Args) -> u16 {
        let need = match a.mode {
            1 if a.facility == FACILITY_TOWER => 4,
            1 => 3,
            2 | 3 => 2,
            _ => 3,
        };
        let lvl = a.result as u8;
        if eligible(party, lvl, a.facility) < need {
            1
        } else {
            self.lvl_mode = lvl.min(LVL_OPEN);
            0
        }
    }

    fn slot(&self, mode: u16) -> Option<(usize, usize)> {
        let mode = mode as usize;
        let lvl = self.lvl_mode as usize;
        (mode < 4 && lvl < 2).then_some((mode, lvl))
    }

    fn streak_of(&self, mode: u16) -> u16 {
        self.slot(mode)
            .map(|(m, l)| self.streaks[m][l].min(MAX_STREAK))
            .unwrap_or(0)
    }

    fn increment(&mut self, mode: u16) {
        let Some((m, l)) = self.slot(mode) else {
            return;
        };
        if self.streaks[m][l] < MAX_STREAK {
            self.streaks[m][l] += 1;
            if mode == MODE_SINGLES {
                self.singles_streak = self.streaks[m][l];
            }
        }
    }

    fn check_air(&mut self, mode: u16) {
        let Some((m, l)) = self.slot(mode) else {
            return;
        };
        if self.streaks[m][l] > self.records[m][l] {
            self.records[m][l] = self.streaks[m][l];
        }
    }

    fn give_points(&mut self, mode: u16) -> u16 {
        let Some((m, _)) = self.slot(mode) else {
            return 0;
        };
        let mut n = (self.streaks[m][self.lvl_mode as usize] / STAGES) as usize;
        if n != 0 {
            n -= 1;
        }
        if n >= TOWER_BP.len() {
            n = TOWER_BP.len() - 1;
        }
        let mut points = TOWER_BP[n][m];
        if self.opponent == BRAIN_TRAINER {
            points += 10;
        }
        self.battle_points = self.battle_points.saturating_add(points).min(MAX_STREAK);
        self.card_points = self.card_points.saturating_add(points);
        points
    }

    /// Tower brain: silver when the streak plus one is 35 (no symbols yet). 0 otherwise.
    fn brain_status(&self, mode: u16) -> u16 {
        if mode != MODE_SINGLES {
            return 0;
        }
        let streak = u32::from(self.streak_of(mode)) + 1;
        if streak == 35 { 1 } else { 0 }
    }

    fn init(&mut self, mode: u16) {
        self.status = STATUS_SAVING;
        self.battle_num = 0;
        self.paused = false;
        self.record_disabled = false;
        self.trainer_ids = [0; 7];
        self.opponent = 0;
        if let Some((m, l)) = self.slot(mode) {
            let bit = 1u32 << (m * 2 + l);
            if self.streak_active & bit == 0 {
                self.streaks[m][l] = 0;
            }
        }
    }

    fn tower_get(&mut self, arg: u16, mode: u16) -> u16 {
        match arg {
            TOWER_WIN_STREAK => self.streak_of(mode),
            TOWER_STREAK_ACTIVE => self
                .slot(mode)
                .map(|(m, l)| ((self.streak_active & (1u32 << (m * 2 + l))) != 0) as u16)
                .unwrap_or(0),
            TOWER_LVL_MODE => {
                self.tower_lvl_mode = self.lvl_mode;
                0
            }
            _ => 0,
        }
    }

    fn tower_set(&mut self, arg: u16, value: u16, mode: u16) {
        let Some((m, l)) = self.slot(mode) else {
            return;
        };
        match arg {
            TOWER_WIN_STREAK => self.streaks[m][l] = value.min(MAX_STREAK),
            TOWER_STREAK_ACTIVE => {
                let bit = 1u32 << (m * 2 + l);
                if value != 0 {
                    self.streak_active |= bit;
                } else {
                    self.streak_active &= !bit;
                }
            }
            TOWER_LVL_MODE => self.tower_lvl_mode = self.lvl_mode,
            _ => {}
        }
    }

    fn set_opponent(&mut self, mode: u16, rng: &mut Rng) {
        if mode != MODE_SINGLES {
            return;
        }
        let challenge = self.streak_of(mode) / STAGES;
        let battle = self.battle_num;
        let id = self.pick_trainer(rng, challenge, battle);
        self.opponent = id;
        if battle + 1 < STAGES {
            let i = battle as usize;
            if i < self.trainer_ids.len() {
                self.trainer_ids[i] = id;
            }
        }
    }

    fn pick_trainer(&self, rng: &mut Rng, challenge: u16, battle: u16) -> u16 {
        let (start, end) = if challenge > 7 {
            (RANGES[7][0], RANGES[7][1])
        } else if battle == STAGES - 1 {
            let i = challenge as usize;
            (RANGES_HARD[i][0], RANGES_HARD[i][1])
        } else {
            let i = challenge as usize;
            (RANGES[i][0], RANGES[i][1])
        };
        let span = end - start + 1;
        let n = (battle as usize).min(self.trainer_ids.len());
        for _ in 0..64 {
            let id = start + rng.random() % span;
            if !self.trainer_ids[..n].contains(&id) {
                return id;
            }
        }
        start
    }

    fn battle_won(&mut self, mode: u16) -> u16 {
        if self.num_wins < MAX_STREAK {
            self.num_wins += 1;
        }
        self.battle_num = self.battle_num.saturating_add(1);
        let _ = mode;
        self.battle_num
    }

    fn save(&mut self, arg: u16, mode: u16, party: &Party) {
        let streak = self.streak_of(mode);
        let challenge = streak / STAGES;
        if arg == 0 && (challenge > 1 || self.battle_num != 0) {
            self.saved_streak = streak;
            self.saved_lvl = self.lvl_mode;
            for (i, mon) in party.iter().take(3).enumerate() {
                self.saved_species[i] = mon.species.0;
            }
        }
        self.status = arg as u8;
        self.paused = true;
    }
}

/// `AppendIfValid`'s bans, plus a faint or a level over the level-50 cap.
pub fn entry_blocked(species: u16, level: u8, hp: u16, cap: u8) -> bool {
    hp == 0 || species == 0 || species == SPECIES_EGG || species_banned(species) || level > cap
}

pub fn species_banned(species: u16) -> bool {
    BANNED.contains(&species)
}

/// Duplicate species or the same held item among the chosen slots (`CheckBattleEntries`).
pub fn duplicate_entry(order: &[u8], party: &Party) -> bool {
    let mut species = [0u16; 4];
    let mut items = [0u16; 4];
    let mut n = 0usize;
    for &o in order.iter().take_while(|&&o| o != 0) {
        let Some(mon) = party.get(o as usize - 1) else {
            continue;
        };
        let item = mon.held.map(|i| i.0).unwrap_or(0);
        if species[..n].contains(&mon.species.0) {
            return true;
        }
        if item != 0 && items[..n].contains(&item) {
            return true;
        }
        if n < 4 {
            species[n] = mon.species.0;
            items[n] = item;
            n += 1;
        }
    }
    false
}

fn species_banned_or_egg(species: u16) -> bool {
    species == 0 || species == SPECIES_EGG || species_banned(species)
}

fn eligible(party: &Party, lvl_mode: u8, facility: u16) -> u8 {
    let mut best = 0u8;
    for start in 0..6 {
        let mut species = [0u16; 6];
        let mut items = [0u16; 6];
        let mut count = 0u8;
        let mut id = start;
        loop {
            if let Some(mon) = party.get(id) {
                let held = mon.held.map(|i| i.0).unwrap_or(0);
                // Pyramid only counts a mon that holds nothing.
                if facility != 6 || held == 0 {
                    append(mon, held, lvl_mode, &mut species, &mut items, &mut count);
                }
            }
            id += 1;
            if id >= 6 {
                id = 0;
            }
            if id == start {
                break;
            }
        }
        best = best.max(count);
    }
    best
}

fn append(
    mon: &Pokemon,
    held: u16,
    lvl_mode: u8,
    species: &mut [u16; 6],
    items: &mut [u16; 6],
    count: &mut u8,
) {
    let n = *count as usize;
    if species_banned_or_egg(mon.species.0) {
        return;
    }
    if lvl_mode == LVL_50 && mon.level > 50 {
        return;
    }
    if species[..n].contains(&mon.species.0) {
        return;
    }
    if held != 0 && items[..n].contains(&held) {
        return;
    }
    if n < 6 {
        species[n] = mon.species.0;
        items[n] = held;
        *count += 1;
    }
}

fn fixed_iv(id: u16) -> u8 {
    if id <= 99 {
        3
    } else if id <= 119 {
        6
    } else if id <= 139 {
        9
    } else if id <= 159 {
        12
    } else if id <= 179 {
        15
    } else if id <= 199 {
        18
    } else if id <= 219 {
        21
    } else {
        31
    }
}
