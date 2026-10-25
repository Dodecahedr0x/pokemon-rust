//! The Pokémon summary (Gen3 `pokemon_summary_screen.c`; Milestone 9 Task PT).
//!
//! Three pages (INFO, SKILLS, BATTLE MOVES; no contest page): a header (page, nickname,
//! level) and a body window.
//! - INFO: species, OT id, held item, nature.
//! - SKILLS: HP and the five stats (`gStatNamesTable` labels), EXP points, to the next level,
//!   the ability.
//! - MOVES: the four moves with PP and type; the cursor's move's power, accuracy and
//!   description (`GameData::move_description`).
//!
//! `View`: left/right change page, up/down change mon (Gen3 `ChangeSummaryPokemon`), B closes
//! with the slot shown (the party menu puts its cursor there). `ReplaceMove(new)` (the learn
//! flow: `ShowSelectMovePokemonSummaryScreen`): the MOVES page with the new move as a fifth
//! row; A on a move answers its slot, unless it is an HM move (`GameData::is_hm_move`, Gen3
//! `CanReplaceMove` → "HM moves can't be forgotten now", shown until the next press); A on the
//! new move or B answers 4 (`MAX_MON_MOVES`: don't forget).
//!
//! Deviations: type and status icons, page tabs and the EXP bar are text (or absent). The mon
//! picture (`species` front pic, Gen3 centre (40, 64)) and `GameData::ui_gfx(SummaryPages)` are
//! drawn when the game has them. INFO has no OT name, met location or trainer memo (not kept by the engine); the
//! move-order swap of `View`'s MOVES page (A) is not there.

use super::kit::Gfx;
use super::kit::pictures::{self, Bg, Load};
use super::kit::widgets::{
    CHAR_CURSOR, CHAR_SLASH, EOS, EXTRA_SYMBOL, SYMBOL_LV, push, push_number,
};
use super::kit::window::{MapBuf, Panel, TileAlloc, colors, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult, SummaryMode};
use crate::battle::Str;
use crate::data::{GameData, MoveId, UiGfx, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::mon::{Pokemon, Stat};
use crate::video::layout;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Header and body windows: (left, top, width, height).
pub const HEADER_RECT: (u8, u8, u8, u8) = (1, 1, 28, 2);
pub const BODY_RECT: (u8, u8, u8, u8) = (1, 4, 28, 15);

/// Gen3 `PSS_PAGE_*` without the contest page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Info,
    Skills,
    Moves,
}

struct Render {
    header: Panel,
    body: Panel,
    pic: Load,
    bg: Bg,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Summary {
    pub slot: u8,
    pub mode: SummaryMode,
    page: Page,
    /// The move row under the cursor (MOVES page; 4 = the new move).
    pub move_cursor: u8,
    /// The HM refusal is shown.
    hm_refused: bool,
    gfx: Gfx<Render>,
}

impl Summary {
    pub fn new(slot: u8, mode: SummaryMode) -> Self {
        Summary {
            slot,
            mode,
            page: match mode {
                SummaryMode::View => Page::Info,
                SummaryMode::ReplaceMove(_) => Page::Moves,
            },
            move_cursor: 0,
            hm_refused: false,
            gfx: Gfx::none(),
        }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    fn new_move(&self) -> Option<MoveId> {
        match self.mode {
            SummaryMode::ReplaceMove(m) => Some(m),
            SummaryMode::View => None,
        }
    }

    fn draw<G: GameData>(&self, cx: &ScreenCx<'_, G>, r: &mut Render) {
        let game = cx.game;
        let Some(mon) = cx.party.get(self.slot as usize) else {
            r.header.clear(game);
            r.body.clear(game);
            return;
        };
        let t = game.battle();
        r.header.clear(game);
        let title = game.ui_text(match self.page {
            Page::Info => UiText::SummaryInfo,
            Page::Skills => UiText::SummarySkills,
            Page::Moves => UiText::SummaryMoves,
        });
        r.header.text(game, title, 0, 1);
        let mut name = Vec::with_capacity(20);
        push(&mut name, &mon.nickname);
        name.push(EOS);
        r.header.text(game, &name, 80, 1);
        let mut lv = Vec::with_capacity(6);
        lv.extend([EXTRA_SYMBOL, SYMBOL_LV]);
        push_number(&mut lv, mon.level as u32, 3, false);
        lv.push(EOS);
        r.header.text(game, &lv, 176, 1);
        r.body.clear(game);
        match self.page {
            Page::Info => self.draw_info(cx, mon, &mut r.body),
            Page::Skills => self.draw_skills(cx, mon, &mut r.body),
            Page::Moves => self.draw_moves(cx, mon, &mut r.body),
        }
        let _ = t;
    }

    fn line<G: GameData>(game: &G, p: &mut Panel, row: i16, label: &[u8], value: &[u8]) {
        p.text(game, label, 0, row * 16 + 1);
        p.text(game, value, 112, row * 16 + 1);
    }

    fn draw_info<G: GameData>(&self, cx: &ScreenCx<'_, G>, mon: &Pokemon, p: &mut Panel) {
        let game = cx.game;
        let t = game.battle();
        let species = t
            .species
            .get(mon.species.0 as usize)
            .map_or(&[EOS][..], |s| s.name);
        p.text(game, species, 0, 1);
        let mut id = Vec::new();
        push_number(&mut id, mon.ot_id & 0xFFFF, 5, false);
        id.push(EOS);
        Self::line(game, p, 1, game.ui_text(UiText::SummaryIdNo), &id);
        let item = match mon.held {
            Some(i) => game.item(i).map_or(&[EOS][..], |i| i.name),
            None => game.ui_text(UiText::SummaryNone),
        };
        Self::line(game, p, 2, game.ui_text(UiText::SummaryItem), item);
        let nature = game.nature_name(mon.nature().0).unwrap_or(&[EOS]);
        Self::line(game, p, 3, game.ui_text(UiText::SummaryNature), nature);
    }

    fn draw_skills<G: GameData>(&self, cx: &ScreenCx<'_, G>, mon: &Pokemon, p: &mut Panel) {
        let game = cx.game;
        let t = game.battle();
        let mut hp = Vec::new();
        push_number(&mut hp, mon.hp as u32, 3, true);
        hp.push(CHAR_SLASH);
        push_number(&mut hp, mon.stats[0] as u32, 3, true);
        hp.push(EOS);
        Self::line(game, p, 0, game.battle_text(Str::StatName(Stat::Hp)), &hp);
        // Gen3's page order: ATTACK, DEFENSE, SP. ATK, SP. DEF, SPEED (`stats`: HP, Atk,
        // Def, Speed, SpAtk, SpDef).
        for (row, (stat, i)) in [
            (Stat::Atk, 1),
            (Stat::Def, 2),
            (Stat::SpAtk, 4),
            (Stat::SpDef, 5),
            (Stat::Speed, 3),
        ]
        .into_iter()
        .enumerate()
        {
            let mut v = Vec::new();
            push_number(&mut v, mon.stats[i] as u32, 3, true);
            v.push(EOS);
            Self::line(
                game,
                p,
                row as i16 + 1,
                game.battle_text(Str::StatName(stat)),
                &v,
            );
        }
        let growth = t.species.get(mon.species.0 as usize).map(|s| s.growth);
        let mut exp = Vec::new();
        push_number(&mut exp, mon.exp, 7, false);
        exp.push(EOS);
        // Two columns for EXP and the next level on one row.
        p.text(game, game.ui_text(UiText::SummaryExpPoints), 0, 6 * 16 + 1);
        p.text(game, &exp, 112, 6 * 16 + 1);
        if let Some(g) = growth
            && mon.level < crate::mon::MAX_LEVEL
        {
            let next = crate::mon::exp_for_level(g, mon.level + 1).saturating_sub(mon.exp);
            let mut v = Vec::new();
            push_number(&mut v, next, 6, false);
            v.push(EOS);
            p.text(game, game.ui_text(UiText::SummaryNextLv), 168, 6 * 16 + 1);
            p.text(game, &v, 176, 6 * 16 + 1);
        }
        let ability = game.battle_text(Str::AbilityName(mon.ability(t)));
        p.text(game, game.ui_text(UiText::SummaryAbility), 0, 7 * 16 + 1);
        p.text(game, ability, 112, 7 * 16 + 1);
    }

    fn draw_moves<G: GameData>(&self, cx: &ScreenCx<'_, G>, mon: &Pokemon, p: &mut Panel) {
        let game = cx.game;
        let t = game.battle();
        let rows = if self.new_move().is_some() { 5 } else { 4 };
        for row in 0..rows {
            let (mv, pp) = if row < 4 {
                (mon.moves[row as usize], Some(row))
            } else {
                (self.new_move(), None)
            };
            let y = row as i16 * 16 + 1;
            let Some(mv) = mv else {
                p.text(game, &[0xAE, 0xAE, EOS], 8, y);
                continue;
            };
            let Some(info) = t.moves.get(mv.0 as usize) else {
                continue;
            };
            p.text(game, info.name, 8, y);
            p.text(game, game.battle_text(Str::TypeName(info.ty)), 104, y);
            let mut v = Vec::new();
            match pp {
                Some(i) => {
                    push_number(&mut v, mon.pp[i as usize] as u32, 2, true);
                    v.push(CHAR_SLASH);
                    push_number(&mut v, mon.max_pp(t, i) as u32, 2, true);
                }
                None => {
                    v.extend([0xAE, 0xAE, CHAR_SLASH]);
                    push_number(&mut v, info.pp as u32, 2, true);
                }
            }
            v.push(EOS);
            p.text(game, &v, 176, y);
        }
        self.draw_move_cursor(cx, mon, p);
    }

    /// The cursor column and the cursor move's details (redrawn alone when the cursor moves:
    /// about 150 tiles instead of the body's 420).
    fn draw_move_cursor<G: GameData>(&self, cx: &ScreenCx<'_, G>, mon: &Pokemon, p: &mut Panel) {
        let game = cx.game;
        let t = game.battle();
        let bg = colors(game).bg;
        p.canvas.fill_rect(0, 0, 8, 5 * 16, bg);
        p.canvas.fill_rect(0, 5 * 16, 28 * 8, 15 * 8 - 5 * 16, bg);
        p.text(
            game,
            &[CHAR_CURSOR, EOS],
            0,
            self.move_cursor as i16 * 16 + 1,
        );
        let y = 5 * 16 + 4;
        if self.hm_refused {
            p.text(game, game.ui_text(UiText::SummaryHmCantForget), 0, y);
            return;
        }
        let cur = if self.move_cursor < 4 {
            mon.moves[self.move_cursor as usize]
        } else {
            self.new_move()
        };
        let Some(info) = cur.and_then(|m| t.moves.get(m.0 as usize).map(|i| (m, i))) else {
            return;
        };
        let num = |n: u8| {
            let mut v = Vec::new();
            if n <= 1 {
                v.extend([0xAE, 0xAE, 0xAE]);
            } else {
                push_number(&mut v, n as u32, 3, true);
            }
            v.push(EOS);
            v
        };
        p.text(game, game.ui_text(UiText::SummaryPower), 0, y);
        p.text(game, &num(info.1.power), 56, y);
        p.text(game, game.ui_text(UiText::SummaryAccuracy), 104, y);
        p.text(game, &num(info.1.accuracy), 176, y);
        if let Some(d) = game.move_description(info.0) {
            p.text(game, d, 0, y + 16);
        }
    }

    fn redraw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let game = cx.game;
        let mut r = match self.gfx.0.take() {
            Some(r) => r,
            None => {
                let mut tiles = TileAlloc::full();
                let (l, t, w, h) = HEADER_RECT;
                let header = tiles.panel(game, l, t, w, h, true);
                let (l, t, w, h) = BODY_RECT;
                let body = tiles.panel(game, l, t, w, h, true);
                let mut map = MapBuf::full(frame_kind(game, cx.options.frame));
                map.put(&header);
                map.put(&body);
                map.flush(cx.f);
                Box::new(Render {
                    header,
                    body,
                    pic: Load::new(),
                    bg: Bg::new(),
                })
            }
        };
        self.draw(cx, &mut r);
        self.gfx.0 = Some(r);
    }

    fn redraw_cursor<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let Some(mut r) = self.gfx.0.take() else {
            return self.redraw(cx);
        };
        if let Some(mon) = cx.party.get(self.slot as usize) {
            self.draw_move_cursor(cx, mon, &mut r.body);
        }
        self.gfx.0 = Some(r);
    }

    /// Up/down through the party (Gen3 skips eggs; the engine has none).
    fn change_mon<G: GameData>(&mut self, cx: &ScreenCx<'_, G>, down: bool) -> bool {
        let len = cx.party.len() as u8;
        if len <= 1 {
            return false;
        }
        let to = if down {
            self.slot + 1
        } else {
            self.slot.wrapping_sub(1)
        };
        if to >= len {
            return false;
        }
        self.slot = to;
        true
    }
}

impl Summary {
    fn input<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if self.gfx.0.is_none() {
            self.redraw(cx);
        }
        let p = input.pressed;
        match self.mode {
            SummaryMode::View => {
                if p.contains(Buttons::B) {
                    return ScreenOut::Done(ScreenResult::Value(self.slot as u16));
                }
                let changed = if p.contains(Buttons::LEFT) {
                    let to = match self.page {
                        Page::Info => None,
                        Page::Skills => Some(Page::Info),
                        Page::Moves => Some(Page::Skills),
                    };
                    to.map(|t| self.page = t).is_some()
                } else if p.contains(Buttons::RIGHT) {
                    let to = match self.page {
                        Page::Info => Some(Page::Skills),
                        Page::Skills => Some(Page::Moves),
                        Page::Moves => None,
                    };
                    to.map(|t| self.page = t).is_some()
                } else if p.contains(Buttons::UP) {
                    self.change_mon(cx, false)
                } else if p.contains(Buttons::DOWN) {
                    self.change_mon(cx, true)
                } else {
                    false
                };
                if changed {
                    self.redraw(cx);
                }
                ScreenOut::Running
            }
            SummaryMode::ReplaceMove(_) => {
                if self.hm_refused {
                    if p.0 != 0 {
                        self.hm_refused = false;
                        self.redraw_cursor(cx);
                    }
                    return ScreenOut::Running;
                }
                if p.contains(Buttons::B) {
                    return ScreenOut::Done(ScreenResult::Value(4));
                }
                if p.contains(Buttons::A) {
                    if self.move_cursor >= 4 {
                        return ScreenOut::Done(ScreenResult::Value(4));
                    }
                    let mv = cx
                        .party
                        .get(self.slot as usize)
                        .and_then(|m| m.moves[self.move_cursor as usize]);
                    if mv.is_some_and(|m| cx.game.is_hm_move(m)) {
                        self.hm_refused = true;
                        self.redraw_cursor(cx);
                        return ScreenOut::Running;
                    }
                    return ScreenOut::Done(ScreenResult::Value(self.move_cursor as u16));
                }
                let moved = if p.contains(Buttons::UP) {
                    self.move_cursor = if self.move_cursor == 0 {
                        4
                    } else {
                        self.move_cursor - 1
                    };
                    true
                } else if p.contains(Buttons::DOWN) {
                    self.move_cursor = (self.move_cursor + 1) % 5;
                    true
                } else {
                    false
                };
                if moved {
                    self.redraw_cursor(cx);
                }
                ScreenOut::Running
            }
        }
    }
}

/// The summary's mon picture (Gen3 `CreateSprite` at centre (40, 64)) and the page background.
fn draw_pictures<G: GameData>(
    cx: &mut ScreenCx<'_, G>,
    slot: u8,
    r: &mut Render,
    budget: &mut usize,
) {
    let game = cx.game;
    if let Some(mon) = cx.party.get(slot as usize) {
        let tables = game.battle();
        if (mon.species.0 as usize) < tables.species.len()
            && let Some(gfx) = tables.species_info(mon.species).gfx
            && gfx.front.len() >= 64
        {
            let y = 32 + i16::from(gfx.front_y);
            r.pic.step(
                cx.f,
                budget,
                &pictures::tiles(
                    &gfx.front[..64],
                    gfx.palette,
                    512,
                    8,
                    8,
                    y,
                    64,
                    64,
                    u32::from(mon.species.0),
                ),
            );
        }
    }
    r.bg.step(
        cx.f,
        budget,
        game.ui_gfx(UiGfx::SummaryPages),
        layout::menu::BACKGROUND,
        1,
    );
}

impl Screen for Summary {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let out = self.input(cx, input);
        let slot = self.slot;
        if let Some(r) = self.gfx.0.as_deref_mut() {
            let mut budget = super::kit::frame_budget(cx.f);
            r.body.flush_within(cx.f, &mut budget);
            r.header.flush_within(cx.f, &mut budget);
            draw_pictures(cx, slot, r, &mut budget);
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.slot,
            self.page as u8,
            self.move_cursor,
            self.hm_refused as u8,
        ]);
    }
}
