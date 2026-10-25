//! Contest script commands (`scrcmd.c`): `choosecontestmon`, `startcontest`,
//! `showcontestresults`, `contestlinktransfer`, `showcontestpainting`.
//!
//! `choosecontestmon` is the party menu (`CB2_ChooseContestMon`) and writes `VAR_0x8004`.
//! `startcontest` judges ([`crate::contest::judge`]) and shows the appeals. Results record the
//! artist painting when the player placed first. `contestlinktransfer` is the link-contest
//! category check (`Task_LinkContest_SetUpContest`): with one player the categories match, so
//! `VAR_0x8004` is 0; with no mon chosen it is 1 and the script takes its cancel path. There is
//! no link cable to wait on.

use super::{Host, UNIT};
use crate::contest::{self, AppealMove, Artist, ContestPage, ContestView, Entrant};
use crate::data::{GameData, VarId};
use crate::mon::Pokemon;
use crate::script::{ContestOp, PARTY_NOTHING_CHOSEN, Request, Response};
use crate::ui::{PartyMode, ScreenKind};
use crate::world::screens::{ANSWER_CANCEL, open};

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let Request::Contest(op) = *r else {
        return UNIT;
    };
    match op {
        ContestOp::LinkTransfer => {
            let differ = u16::from(h.s.contest.party_index == PARTY_NOTHING_CHOSEN);
            h.s.vars.set_var(VarId(contest::VAR_CHOSEN), differ);
            Some(Response::Unit)
        }
        ContestOp::ChooseMon => open_choose(h),
        ContestOp::Start => {
            judge(h);
            open_page(h, ContestPage::Appeals)
        }
        ContestOp::Results => {
            record_artist(h);
            open_page(h, ContestPage::Results)
        }
        ContestOp::Painting(id) => open_page(h, ContestPage::Painting(id)),
    }
}

pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let Request::Contest(op) = *r else {
        return Some(Response::Unit);
    };
    if matches!(op, ContestOp::LinkTransfer) {
        return Some(Response::Unit);
    }
    let v = h.s.answer.take()?;
    if matches!(op, ContestOp::ChooseMon) {
        let slot = if v == ANSWER_CANCEL {
            PARTY_NOTHING_CHOSEN
        } else {
            v
        };
        h.s.contest.party_index = slot;
        h.s.vars.set_var(VarId(contest::VAR_CHOSEN), slot);
        return Some(Response::Value(slot));
    }
    Some(Response::Unit)
}

fn open_choose<G: GameData>(h: &mut Host<'_, G>) -> Option<Response> {
    if h.immediate {
        h.s.contest.party_index = PARTY_NOTHING_CHOSEN;
        h.s.vars
            .set_var(VarId(contest::VAR_CHOSEN), PARTY_NOTHING_CHOSEN);
        return Some(Response::Value(PARTY_NOTHING_CHOSEN));
    }
    h.s.answer = None;
    open(h.s, h.game, ScreenKind::Party(PartyMode::Choose), true, h.f);
    None
}

fn open_page<G: GameData>(h: &mut Host<'_, G>, page: ContestPage) -> Option<Response> {
    if h.immediate {
        return Some(Response::Unit);
    }
    let view = ContestView {
        page,
        place: h.s.contest.place,
        player: contest::PLAYER_MON,
        category: h.s.contest.category,
    };
    h.s.answer = None;
    open(h.s, h.game, ScreenKind::Contest(view), true, h.f);
    None
}

fn judge<G: GameData>(h: &mut Host<'_, G>) {
    let game = h.game;
    let s = &mut *h.s;
    let category = s.vars.var(VarId(contest::VAR_CATEGORY)) as u8;
    let rank = s.vars.var(VarId(contest::VAR_RANK)) as u8;
    let player = s
        .party
        .get(s.contest.party_index as usize)
        .map(|mon| entrant(game, mon))
        .unwrap_or_default();
    let outcome = contest::judge(category, rank, player, &mut s.rng);
    s.contest.category = category;
    s.contest.rank = rank;
    s.contest.judged = true;
    s.contest.round1 = outcome.round1;
    s.contest.appeal = outcome.appeal;
    s.contest.total = outcome.total;
    s.contest.place = outcome.place;
}

fn record_artist<G: GameData>(h: &mut Host<'_, G>) {
    let s = &mut h.s;
    if !s.contest.judged {
        return;
    }
    let player = s.contest.place[contest::PLAYER_MON as usize] == 0;
    let species = if player {
        s.party
            .get(s.contest.party_index as usize)
            .map(|mon| mon.species.0)
            .unwrap_or(0)
    } else {
        0
    };
    s.contest.artist = Some(Artist {
        category: s.contest.category,
        species,
        player,
    });
}

fn entrant<G: GameData>(game: &G, mon: &Pokemon) -> Entrant {
    let mut moves = [None; 4];
    for (i, slot) in moves.iter_mut().enumerate() {
        let Some(id) = mon.moves[i] else { continue };
        if id.0 == 0 {
            continue;
        }
        let m = game.contest_move(id);
        *slot = Some(AppealMove {
            appeal: i16::from(m.appeal),
            jam: if m.jam == 0xFF { 0 } else { i16::from(m.jam) },
            category: m.category,
            id: id.0,
        });
    }
    Entrant {
        condition: contest::Condition::default(),
        moves,
    }
}
