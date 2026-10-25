//! Pokémon Contests (Gen3 `contest.c`): one contestant is chosen, five appeal rounds are
//! judged, and the results rank the field.
//!
//! Ranking matches `DetermineFinalStandings` / `DidContestantPlaceHigher`: higher total, then
//! higher round-1 points, then a higher unique `Random()`. Round 1 is
//! `CalculateContestantRound1Points` (main stat + (sub1 + sub2 + sheen) / 2). Round 2 is the
//! appeal total times two (`GetContestantRound2Points`). The non-link player is contestant 3
//! (`TryPutPlayerLast`).
//!
//! Appeals are the subset that decides those points when nobody combos: base appeal, the
//! repeat penalty `(moveRepeatCount + 1) * 10`, the shared excitement meter
//! (`sContestExcitementTable`; a repeat neither gains the bonus nor moves the meter), and, when
//! the move's jam is greater than 0, that jam subtracted from contestants who already appealed
//! this turn (`JamContestant` / `StartlePrevMons`, without immunity or safety). The other
//! contest effects, combos, and the contest AI are not run. The anim-target `Random()` does not
//! change these points and is not drawn. The player uses move slots in order (one move repeats);
//! each rival uses one move of appeal 20 and jam 0 in the contest's category, with a main
//! condition of 0 / 80 / 160 / 240 for Normal / Super / Hyper / Master.

use crate::rng::Rng;

/// Contestants in one contest (`CONTESTANT_COUNT`).
pub const COUNT: usize = 4;
/// Appeal rounds (`CONTEST_NUM_APPEALS`).
pub const APPEALS: usize = 5;
/// The player's contestant index outside a link contest.
pub const PLAYER_MON: u8 = 3;

/// `VAR_CONTEST_RANK` / `VAR_CONTEST_CATEGORY` / the slot `choosecontestmon` writes.
pub const VAR_RANK: u16 = 0x8010;
pub const VAR_CATEGORY: u16 = 0x8011;
pub const VAR_CHOSEN: u16 = 0x8004;

/// Cool, Beauty, Cute, Smart, Tough (`CONTEST_CATEGORY_*`).
pub const CATEGORIES: usize = 5;

/// Pokeblock condition (`struct ContestPokemon`'s contest stats, not the in-contest stars).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Condition {
    pub cool: u8,
    pub beauty: u8,
    pub cute: u8,
    pub smart: u8,
    pub tough: u8,
    pub sheen: u8,
}

/// One appeal (`gContestEffects` appeal and jam, `gContestMoves` category). Jam `0` does not
/// startle; jam above `0` startles every contestant who already appealed this turn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AppealMove {
    pub appeal: i16,
    pub jam: i16,
    pub category: u8,
    /// Move id. `0` is `MOVE_NONE` and never counts as a repeat.
    pub id: u16,
}

/// What [`crate::data::GameData::contest_move`] answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MoveAppeal {
    pub appeal: u8,
    pub jam: u8,
    /// `0xFF`: the move has no jam (`CONTEST_JAM_NONE`).
    pub category: u8,
}

/// One contestant: condition and up to four appeal moves (empty slots are skipped).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Entrant {
    pub condition: Condition,
    pub moves: [Option<AppealMove>; 4],
}

/// Places and points after [`judge`]. `place` is 0 for first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    pub round1: [i16; COUNT],
    pub appeal: [i16; COUNT],
    pub total: [i16; COUNT],
    pub place: [u8; COUNT],
}

/// Which contest screen is up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContestPage {
    Appeals,
    Results,
    /// `showcontestpainting`'s painting id. `0` is the artist painting just recorded.
    Painting(u8),
}

/// What the contest screen draws. Judging has already finished.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ContestView {
    pub page: ContestPage,
    pub place: [u8; COUNT],
    pub player: u8,
    pub category: u8,
}

/// The winner shown by `showcontestpainting` of the artist (`CONTEST_WINNER_ARTIST`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Artist {
    pub category: u8,
    pub species: u16,
    pub player: bool,
}

/// Script RAM for the contest in progress (`gContestMonPartyIndex`, the standings, the artist
/// painting). Not hashed and not snapshotted: it lives only while the script runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Session {
    /// Party slot from `choosecontestmon`, or `0xFF` (`PARTY_NOTHING_CHOSEN`).
    pub party_index: u16,
    pub category: u8,
    pub rank: u8,
    pub judged: bool,
    pub round1: [i16; COUNT],
    pub appeal: [i16; COUNT],
    pub total: [i16; COUNT],
    pub place: [u8; COUNT],
    pub artist: Option<Artist>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            party_index: 0xFF,
            category: 0,
            rank: 0,
            judged: false,
            round1: [0; COUNT],
            appeal: [0; COUNT],
            total: [0; COUNT],
            place: [0; COUNT],
            artist: None,
        }
    }
}

/// `sContestNames`: "COOLNESS CONTEST" and the other four, Gen3-encoded, no terminator.
pub fn name(category: u8) -> &'static [u8] {
    match category {
        0 => COOL,
        1 => BEAUTY,
        2 => CUTE,
        3 => SMART,
        4 => TOUGH,
        _ => &[],
    }
}

const fn encode<const N: usize>(s: &[u8; N]) -> [u8; N] {
    let mut o = [0u8; N];
    let mut i = 0;
    while i < N {
        let c = s[i];
        o[i] = if c == b' ' { 0 } else { 0xBB + (c - b'A') };
        i += 1;
    }
    o
}

const COOL: &[u8] = &encode(b"COOLNESS CONTEST");
const BEAUTY: &[u8] = &encode(b"BEAUTY CONTEST");
const CUTE: &[u8] = &encode(b"CUTENESS CONTEST");
const SMART: &[u8] = &encode(b"SMARTNESS CONTEST");
const TOUGH: &[u8] = &encode(b"TOUGHNESS CONTEST");

/// `CalculateContestantRound1Points`. Any category past Tough uses Tough, as the switch does.
pub fn round1(category: u8, c: Condition) -> i16 {
    let (main, sub1, sub2) = match category {
        0 => (c.cool, c.tough, c.beauty),
        1 => (c.beauty, c.cool, c.cute),
        2 => (c.cute, c.beauty, c.smart),
        3 => (c.smart, c.cute, c.tough),
        _ => (c.tough, c.smart, c.cool),
    };
    let points = u16::from(main) + (u16::from(sub1) + u16::from(sub2) + u16::from(c.sheen)) / 2;
    points as i16
}

/// `DidContestantPlaceHigher`: `true` when `b` ranks above `a`.
fn places_higher(
    a_total: i16,
    a_round1: i16,
    a_random: u16,
    b_total: i16,
    b_round1: i16,
    b_random: u16,
) -> bool {
    if a_total < b_total {
        true
    } else if a_total > b_total {
        false
    } else if a_round1 < b_round1 {
        true
    } else if a_round1 > b_round1 {
        false
    } else {
        a_random < b_random
    }
}

/// `DetermineFinalStandings`. `place[contestant]` is 0 for first.
pub fn standings(total: [i16; COUNT], round1: [i16; COUNT], random: [u16; COUNT]) -> [u8; COUNT] {
    let mut who = [0u8, 1, 2, 3];
    for i in 0..COUNT - 1 {
        for j in (i + 1..COUNT).rev() {
            let (a, b) = (who[j - 1] as usize, who[j] as usize);
            if places_higher(
                total[a], round1[a], random[a], total[b], round1[b], random[b],
            ) {
                who.swap(j, j - 1);
            }
        }
    }
    let mut place = [0u8; COUNT];
    for (rank, id) in who.iter().enumerate() {
        place[*id as usize] = rank as u8;
    }
    place
}

/// Judges `player` in slot [`PLAYER_MON`] against three rivals of `rank`.
pub fn judge(category: u8, rank: u8, player: Entrant, rng: &mut Rng) -> Outcome {
    let mut entrants = [rival(rank, category); COUNT];
    entrants[PLAYER_MON as usize] = player;
    let round1 = entrants.map(|e| round1(category, e.condition));
    let mut turn = apply_next(sort_by_round1(round1, unique_randoms(rng)));
    let mut points = [0i16; COUNT];
    let mut prev = [0u16; COUNT];
    let mut repeats = [0u8; COUNT];
    let mut applause = 0i16;
    for round in 0..APPEALS {
        let appeal = appeal_round(
            category,
            &entrants,
            turn,
            round,
            &mut prev,
            &mut repeats,
            &mut applause,
        );
        for i in 0..COUNT {
            points[i] = points[i].wrapping_add(appeal[i]);
        }
        turn = apply_next(sort_by_ranking(rank_of(points), unique_randoms(rng)));
    }
    let total = core::array::from_fn(|i| round1[i].wrapping_add(points[i].wrapping_mul(2)));
    let place = standings(total, round1, unique_randoms(rng));
    Outcome {
        round1,
        appeal: points,
        total,
        place,
    }
}

/// Main condition of a rival at Normal / Super / Hyper / Master.
fn rival_stat(rank: u8) -> u8 {
    match rank {
        0 => 0,
        1 => 80,
        2 => 160,
        _ => 240,
    }
}

fn rival(rank: u8, category: u8) -> Entrant {
    let mut condition = Condition::default();
    let stat = rival_stat(rank);
    match category {
        0 => condition.cool = stat,
        1 => condition.beauty = stat,
        2 => condition.cute = stat,
        3 => condition.smart = stat,
        _ => condition.tough = stat,
    }
    let mov = AppealMove {
        appeal: 20,
        jam: 0,
        category: category.min(4),
        id: 1,
    };
    Entrant {
        condition,
        moves: [Some(mov), None, None, None],
    }
}

fn move_for(e: &Entrant, round: usize) -> Option<AppealMove> {
    let n = e.moves.iter().flatten().count();
    if n == 0 {
        return None;
    }
    e.moves.iter().flatten().nth(round % n).copied()
}

/// One round. Appeals are in turn order; jam hits contestants with a lower turn index.
fn appeal_round(
    category: u8,
    entrants: &[Entrant; COUNT],
    turn: [u8; COUNT],
    round: usize,
    prev: &mut [u16; COUNT],
    repeats: &mut [u8; COUNT],
    applause: &mut i16,
) -> [i16; COUNT] {
    let mut appeal = [0i16; COUNT];
    let mut order = [0usize; COUNT];
    for (who, &t) in turn.iter().enumerate() {
        order[t as usize] = who;
    }
    for &who in &order {
        let Some(mov) = move_for(&entrants[who], round) else {
            prev[who] = 0;
            repeats[who] = 0;
            continue;
        };
        let repeated = prev[who] == mov.id && mov.id != 0;
        if repeated {
            repeats[who] = repeats[who].saturating_add(1);
        } else {
            repeats[who] = 0;
        }
        appeal[who] = mov.appeal;
        if mov.jam > 0 {
            for other in 0..COUNT {
                if turn[other] < turn[who] {
                    appeal[other] = appeal[other].wrapping_sub(mov.jam);
                }
            }
        }
        if repeated {
            let jam = i16::from(repeats[who].saturating_add(1)) * 10;
            appeal[who] = appeal[who].wrapping_sub(jam);
        }
        let exc = if repeated {
            0
        } else {
            excitement(category, mov.category)
        };
        let bonus = if exc > 0 && *applause + i16::from(exc) > 4 {
            60
        } else if exc > 0 {
            10
        } else {
            0
        };
        if !repeated {
            *applause += i16::from(exc);
            if *applause < 0 {
                *applause = 0;
            }
            if exc > 0 {
                appeal[who] = appeal[who].wrapping_add(bonus);
            }
        }
        prev[who] = mov.id;
    }
    appeal
}

/// `sContestExcitementTable`. Out-of-range categories neither excite nor bore.
fn excitement(contest: u8, mov: u8) -> i8 {
    const TABLE: [[i8; 5]; 5] = [
        [1, 0, -1, -1, 0],
        [0, 1, 0, -1, -1],
        [-1, 0, 1, 0, -1],
        [-1, -1, 0, 1, 0],
        [0, -1, -1, 0, 1],
    ];
    let (c, m) = (contest as usize, mov as usize);
    if c >= CATEGORIES || m >= CATEGORIES {
        0
    } else {
        TABLE[c][m]
    }
}

/// `SortContestants`' unique `Random()`s, retrying collisions.
fn unique_randoms(rng: &mut Rng) -> [u16; COUNT] {
    let mut out = [0u16; COUNT];
    let mut i = 0;
    while i < COUNT {
        let n = rng.random();
        if out[..i].contains(&n) {
            continue;
        }
        out[i] = n;
        i += 1;
    }
    out
}

/// `SortContestants(false)`: higher round-1 points appeal first. `turn[contestant]` is the
/// turn index.
fn sort_by_round1(points: [i16; COUNT], random: [u16; COUNT]) -> [u8; COUNT] {
    let mut order = [0u8; COUNT];
    for i in 0..COUNT {
        order[i] = i as u8;
        for v3 in 0..i {
            let at = order[v3] as usize;
            if points[at] < points[i] || (points[at] == points[i] && random[at] < random[i]) {
                for j in (v3 + 1..=i).rev() {
                    order[j] = order[j - 1];
                }
                order[v3] = i as u8;
                break;
            }
        }
    }
    let mut turn = [0u8; COUNT];
    for (pos, &id) in order.iter().enumerate() {
        turn[id as usize] = pos as u8;
    }
    turn
}

/// `RankContestants`' ranks: ties share the best rank and still consume the next place
/// (`[100, 80, 80, 50]` → `[0, 1, 1, 3]`).
fn rank_of(points: [i16; COUNT]) -> [u8; COUNT] {
    let mut arr = points;
    for i in 0..COUNT - 1 {
        for j in (i + 1..COUNT).rev() {
            if arr[j - 1] < arr[j] {
                arr.swap(j, j - 1);
            }
        }
    }
    let mut ranking = [0u8; COUNT];
    for i in 0..COUNT {
        if let Some(j) = arr.iter().position(|&p| p == points[i]) {
            ranking[i] = j as u8;
        }
    }
    ranking
}

/// `SortContestants(true)`.
fn sort_by_ranking(ranking: [u8; COUNT], random: [u16; COUNT]) -> [u8; COUNT] {
    let mut scratch = [0xFFu8; COUNT];
    let mut turn = [0u8; COUNT];
    for i in 0..COUNT {
        let mut j = ranking[i] as usize;
        loop {
            if j >= COUNT {
                break;
            }
            if scratch[j] == 0xFF {
                scratch[j] = i as u8;
                turn[i] = j as u8;
                break;
            }
            j += 1;
        }
    }
    for i in 0..COUNT - 1 {
        for v3 in (i + 1..COUNT).rev() {
            if ranking[v3 - 1] == ranking[v3]
                && turn[v3 - 1] < turn[v3]
                && random[v3 - 1] < random[v3]
            {
                turn.swap(v3, v3 - 1);
            }
        }
    }
    turn
}

/// `ApplyNextTurnOrder` with no contestant asking for a turn (`nextTurnOrder` all
/// `CONTESTANT_NONE`). The unassigned contestant with the lowest current turn index takes the
/// next turn, so an already-unique order is kept.
fn apply_next(turn: [u8; COUNT]) -> [u8; COUNT] {
    const NONE: u8 = 0xFF;
    let next = [NONE; COUNT];
    let mut new = turn;
    let mut ordered = [false; COUNT];
    for i in 0..COUNT as u8 {
        let mut assigned = false;
        for j in 0..COUNT {
            if next[j] == i {
                new[j] = i;
                ordered[j] = true;
                assigned = true;
                break;
            }
        }
        if assigned {
            continue;
        }
        let mut next_c = 0usize;
        let mut j = 0usize;
        while j < COUNT {
            if !ordered[j] && next[j] == NONE {
                next_c = j;
                j += 1;
                break;
            }
            j += 1;
        }
        while j < COUNT {
            if !ordered[j] && next[j] == NONE && turn[next_c] > turn[j] {
                next_c = j;
            }
            j += 1;
        }
        new[next_c] = i;
        ordered[next_c] = true;
    }
    new
}

#[cfg(test)]
mod tests;
