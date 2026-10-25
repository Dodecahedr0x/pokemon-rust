//! The Pokédex (Gen3 `pokedex.c`: `CB2_OpenPokedex`, the list, the info page).
//!
//! START on the field menu's POKéDEX opens the list. Up/down move (no wrap; L/R move a page).
//! A on a seen species opens its detail page; B there returns to the list. START opens the
//! rating (seen, caught, and the Birch band of `GetPokedexRatingText`). SELECT switches
//! Hoenn and National mode when `IsNationalPokedexEnabled` (the national flag and
//! [`NATIONAL_DEX_MAGIC`] in its var). B on the list closes back to the start menu.
//!
//! Deviations: no sprites, cries, footprints, area map or search (START is the rating, which
//! Gen3 shows as Birch's message rather than a page of this screen). Text only. National
//! order is [`GameData::national_dex`] when that is non-empty, otherwise species ids `1..=251`
//! followed by Hoenn species whose id is past 251 ([`national_fallback`]). Seen and caught
//! flags stay indexed by species id (`crate::pokedex`).

use super::kit::Gfx;
use super::kit::list::{ListMenu, MenuInput, Paging};
use super::kit::widgets::{
    CHAR_CURSOR, CHAR_SLASH, CHAR_SPACE, CURSOR_W, EOS, ROW_H, push, push_number,
};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::{GameData, SpeciesId};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::pokedex::{DexFlag, Pokedex};
use alloc::vec::Vec;

pub use crate::pokedex::NATIONAL_DEX_MAGIC;

/// Rows of the list window.
pub const LIST_ROWS: u16 = 7;
/// Header and list windows: (left, top, width, height).
pub const HEADER_RECT: (u8, u8, u8, u8) = (1, 1, 28, 2);
pub const LIST_RECT: (u8, u8, u8, u8) = (1, 4, 28, 14);

/// Hoenn (`HOENN_DEX` order) or National.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DexMode {
    Hoenn = 0,
    National = 1,
}

/// The list, one species, or the rating.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Page {
    List = 0,
    Detail = 1,
    Rating = 2,
}

/// `GetPokedexRatingText`: a band of ten caught species, or the completed dex.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rating {
    /// Caught count is in `[band * 10, band * 10 + 10)` (`LessThan10` is band 0, up to
    /// `LessThan200`).
    Band(u8),
    /// `gBirchDexRatingText_DexCompleted`.
    Completed,
}

struct Render {
    header: Panel,
    body: Panel,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PokedexScreen {
    mode: DexMode,
    page: Page,
    list: ListMenu,
    /// Species index the detail page shows.
    focus: u16,
    gfx: Gfx<Render>,
}

impl PokedexScreen {
    pub fn new() -> Self {
        PokedexScreen {
            mode: DexMode::Hoenn,
            page: Page::List,
            list: ListMenu::new(0, LIST_ROWS, Paging::LR),
            focus: 0,
            gfx: Gfx::none(),
        }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn mode(&self) -> DexMode {
        self.mode
    }

    /// The species under the cursor, or the one the detail page shows.
    pub fn species<G: GameData>(&self, game: &G) -> Option<SpeciesId> {
        let index = if self.page == Page::Detail {
            self.focus
        } else {
            self.list.selected()
        };
        species_at(game, self.mode, index)
    }

    fn sync_list<G: GameData>(&mut self, game: &G) {
        if self.page != Page::List {
            return;
        }
        let total = order_len(game, self.mode);
        if self.list.total != total {
            self.list = ListMenu::new(total, LIST_ROWS, Paging::LR);
            self.focus = 0;
        }
    }

    fn toggle<G: GameData>(&mut self, cx: &ScreenCx<'_, G>) -> bool {
        if !national_enabled(cx.game, cx.vars) {
            return false;
        }
        self.mode = match self.mode {
            DexMode::Hoenn => DexMode::National,
            DexMode::National => DexMode::Hoenn,
        };
        self.list = ListMenu::new(order_len(cx.game, self.mode), LIST_ROWS, Paging::LR);
        self.focus = 0;
        self.page = Page::List;
        true
    }

    fn list_input<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        pressed: Buttons,
        redraw: &mut bool,
    ) -> ScreenOut {
        if pressed.contains(Buttons::B) {
            return ScreenOut::Done(ScreenResult::Cancel);
        }
        if pressed.contains(Buttons::START) {
            self.page = Page::Rating;
            *redraw = true;
            return ScreenOut::Running;
        }
        if pressed.contains(Buttons::SELECT) && self.toggle(cx) {
            *redraw = true;
            return ScreenOut::Running;
        }
        match self.list.input(pressed) {
            MenuInput::Chosen(i) => {
                let seen = species_at(cx.game, self.mode, i)
                    .is_some_and(|s| cx.pokedex.get(DexFlag::Seen, s));
                if seen {
                    self.focus = i;
                    self.page = Page::Detail;
                    *redraw = true;
                }
            }
            MenuInput::Moved => {
                self.focus = self.list.selected();
                *redraw = true;
            }
            MenuInput::Cancel | MenuInput::None => {}
        }
        ScreenOut::Running
    }

    fn detail_input<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        pressed: Buttons,
        redraw: &mut bool,
    ) -> ScreenOut {
        if pressed.contains(Buttons::B) {
            self.page = Page::List;
            place(&mut self.list, self.focus);
            *redraw = true;
            return ScreenOut::Running;
        }
        if pressed.contains(Buttons::START) {
            self.page = Page::Rating;
            *redraw = true;
            return ScreenOut::Running;
        }
        if pressed.contains(Buttons::SELECT) && self.toggle(cx) {
            *redraw = true;
            return ScreenOut::Running;
        }
        let down = if pressed.contains(Buttons::DOWN) || pressed.contains(Buttons::RIGHT) {
            Some(true)
        } else if pressed.contains(Buttons::UP) || pressed.contains(Buttons::LEFT) {
            Some(false)
        } else {
            None
        };
        if let Some(down) = down
            && let Some(i) = self.neighbor(cx, down)
        {
            self.focus = i;
            *redraw = true;
        }
        ScreenOut::Running
    }

    fn rating_input<G: GameData>(
        &mut self,
        cx: &mut ScreenCx<'_, G>,
        pressed: Buttons,
        redraw: &mut bool,
    ) -> ScreenOut {
        if pressed.contains(Buttons::A) || pressed.contains(Buttons::B) {
            self.page = Page::List;
            *redraw = true;
            return ScreenOut::Running;
        }
        if pressed.contains(Buttons::SELECT) && self.toggle(cx) {
            self.page = Page::Rating;
            *redraw = true;
        }
        ScreenOut::Running
    }

    /// The next seen species toward `down`, if one exists.
    fn neighbor<G: GameData>(&self, cx: &ScreenCx<'_, G>, down: bool) -> Option<u16> {
        let total = order_len(cx.game, self.mode);
        let mut i = self.focus;
        loop {
            if down {
                if i + 1 >= total {
                    return None;
                }
                i += 1;
            } else if i == 0 {
                return None;
            } else {
                i -= 1;
            }
            let seen =
                species_at(cx.game, self.mode, i).is_some_and(|s| cx.pokedex.get(DexFlag::Seen, s));
            if seen {
                return Some(i);
            }
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
                let (l, t, w, h) = LIST_RECT;
                let body = tiles.panel(game, l, t, w, h, true);
                let mut map = MapBuf::full(frame_kind(game, cx.options.frame));
                map.put(&header);
                map.put(&body);
                map.flush(cx.f);
                alloc::boxed::Box::new(Render { header, body })
            }
        };
        r.header.clear(game);
        r.body.clear(game);
        let (seen, caught) = counts(game, cx.pokedex, self.mode);
        let mut head = label(mode_name(self.mode));
        head.pop();
        head.push(CHAR_SPACE);
        push(&mut head, &labeled("SEEN", seen));
        head.push(CHAR_SPACE);
        push(&mut head, &labeled("CAUGHT", caught));
        head.push(EOS);
        r.header.text(game, &head, 0, 1);
        match self.page {
            Page::List => self.draw_list(cx, &mut r.body),
            Page::Detail => self.draw_detail(cx, &mut r.body),
            Page::Rating => draw_rating(
                game,
                &mut r.body,
                self.mode,
                seen,
                caught,
                rating(game, cx.pokedex, self.mode),
            ),
        }
        self.gfx.0 = Some(r);
    }

    fn draw_list<G: GameData>(&self, cx: &ScreenCx<'_, G>, p: &mut Panel) {
        let game = cx.game;
        for (row, index) in self.list.shown().enumerate() {
            let y = row as i16 * ROW_H + 1;
            if row as u16 == self.list.row {
                p.text(game, &[CHAR_CURSOR, EOS], 0, y);
            }
            let mut line = Vec::new();
            push_number(&mut line, u32::from(index) + 1, 3, true);
            line.push(CHAR_SPACE);
            match species_at(game, self.mode, index) {
                Some(sp) if cx.pokedex.get(DexFlag::Seen, sp) => {
                    if let Some(info) = game.battle().species.get(usize::from(sp.0)) {
                        push(&mut line, info.name);
                    }
                }
                _ => line.extend([0xAE; 5]),
            }
            line.push(EOS);
            p.text(game, &line, CURSOR_W, y);
        }
    }

    fn draw_detail<G: GameData>(&self, cx: &ScreenCx<'_, G>, p: &mut Panel) {
        let game = cx.game;
        let Some(sp) = species_at(game, self.mode, self.focus) else {
            return;
        };
        let info = game.battle().species.get(usize::from(sp.0));
        let seen = cx.pokedex.get(DexFlag::Seen, sp);
        let owned = cx.pokedex.get(DexFlag::Caught, sp);
        p.text(game, &number_line(self.focus), 0, 1);
        let mut name = Vec::new();
        if seen {
            if let Some(info) = info {
                push(&mut name, info.name);
            }
        } else {
            name.extend([0xAE; 5]);
        }
        name.push(EOS);
        p.text(game, &name, 0, ROW_H + 1);
        p.text(game, &types_line(game, info, owned), 0, 2 * ROW_H + 1);
        let weight = info.map(|s| s.weight).unwrap_or(0);
        p.text(game, &weight_line(weight, owned), 0, 3 * ROW_H + 1);
        p.text(
            game,
            &label(if owned { "CAUGHT" } else { "SEEN" }),
            0,
            4 * ROW_H + 1,
        );
    }
}

impl Screen for PokedexScreen {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        self.sync_list(cx.game);
        let mut redraw = self.gfx.0.is_none();
        let pressed = input.pressed;
        let out = match self.page {
            Page::List => self.list_input(cx, pressed, &mut redraw),
            Page::Detail => self.detail_input(cx, pressed, &mut redraw),
            Page::Rating => self.rating_input(cx, pressed, &mut redraw),
        };
        if redraw {
            self.redraw(cx);
        }
        if let Some(r) = self.gfx.0.as_deref_mut() {
            let mut budget = super::kit::frame_budget(cx.f);
            r.body.flush_within(cx.f, &mut budget);
            r.header.flush_within(cx.f, &mut budget);
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.mode as u8, self.page as u8]);
        h.write_u16(self.focus);
        self.list.hash(h);
    }
}

/// `FLAG_SYS_NATIONAL_DEX` and `VAR_NATIONAL_DEX == NATIONAL_DEX_MAGIC`.
pub fn national_enabled(game: &impl GameData, vars: &crate::flow::Vars) -> bool {
    let ids = game.special_ids();
    let Some(flag) = ids.flag_national_dex else {
        return false;
    };
    let Some(var) = ids.var_national_dex else {
        return false;
    };
    vars.flag(flag) && vars.var(var) == NATIONAL_DEX_MAGIC
}

/// Seen and caught counts among the species [`mode`](DexMode) lists.
pub fn counts<G: GameData>(game: &G, dex: &Pokedex, mode: DexMode) -> (u16, u16) {
    (
        count_flag(game, dex, mode, DexFlag::Seen),
        count_flag(game, dex, mode, DexFlag::Caught),
    )
}

/// The rating of `mode`'s caught count (`GetPokedexRatingText` when the Hoenn list has 202
/// species; otherwise completed at a full list, else the band of ten).
pub fn rating<G: GameData>(game: &G, dex: &Pokedex, mode: DexMode) -> Rating {
    let caught = count_flag(game, dex, mode, DexFlag::Caught);
    let len = order_len(game, mode);
    let (jirachi, deoxys) = if mode == DexMode::Hoenn && len == 202 {
        (
            species_at(game, mode, 200).is_some_and(|s| dex.get(DexFlag::Caught, s)),
            species_at(game, mode, 201).is_some_and(|s| dex.get(DexFlag::Caught, s)),
        )
    } else {
        (false, false)
    };
    rating_of(caught, len, jirachi, deoxys)
}

/// Species `index` of `mode` (0-based).
pub fn species_at<G: GameData>(game: &G, mode: DexMode, index: u16) -> Option<SpeciesId> {
    match mode {
        DexMode::Hoenn => game.hoenn_dex().get(usize::from(index)).copied(),
        DexMode::National => {
            let listed = game.national_dex();
            if listed.is_empty() {
                national_fallback(game.battle().species.len(), game.hoenn_dex(), index)
            } else {
                listed.get(usize::from(index)).copied()
            }
        }
    }
}

/// How many species `mode` lists.
pub fn order_len<G: GameData>(game: &G, mode: DexMode) -> u16 {
    match mode {
        DexMode::Hoenn => game.hoenn_dex().len() as u16,
        DexMode::National => {
            let listed = game.national_dex();
            if listed.is_empty() {
                national_fallback_len(game.battle().species.len(), game.hoenn_dex())
            } else {
                listed.len() as u16
            }
        }
    }
}

fn count_flag<G: GameData>(game: &G, dex: &Pokedex, mode: DexMode, flag: DexFlag) -> u16 {
    let n = order_len(game, mode);
    let mut c = 0u16;
    for i in 0..n {
        if species_at(game, mode, i).is_some_and(|s| dex.get(flag, s)) {
            c += 1;
        }
    }
    c
}

/// National order without a national table: species `1..=251` (through Celebi), then Hoenn
/// species with an id past 251, in Hoenn order. Ids the species table does not hold are left
/// out, so the old Unown placeholders (past 251, not in the Hoenn list) are not entries.
fn national_fallback(species_len: usize, hoenn: &[SpeciesId], index: u16) -> Option<SpeciesId> {
    let first = species_len.saturating_sub(1).min(251) as u16;
    if index < first {
        return Some(SpeciesId(index + 1));
    }
    let mut at = first;
    for &sp in hoenn {
        if sp.0 == 0 || usize::from(sp.0) >= species_len || sp.0 <= 251 {
            continue;
        }
        if at == index {
            return Some(sp);
        }
        at = at.saturating_add(1);
    }
    None
}

fn national_fallback_len(species_len: usize, hoenn: &[SpeciesId]) -> u16 {
    let mut i = 0u16;
    while national_fallback(species_len, hoenn, i).is_some() {
        i += 1;
        if i == u16::MAX {
            break;
        }
    }
    i
}

/// `GetPokedexRatingText` for a Hoenn list of 202, and a full-list completion otherwise.
fn rating_of(caught: u16, len: u16, jirachi: bool, deoxys: bool) -> Rating {
    if len == 202 {
        if caught < 200 {
            return Rating::Band((caught / 10) as u8);
        }
        return match caught {
            200 if jirachi || deoxys => Rating::Band(19),
            200 => Rating::Completed,
            c if c == 201 && jirachi && deoxys => Rating::Band(19),
            201 | 202 => Rating::Completed,
            _ => Rating::Band(0),
        };
    }
    if len > 0 && caught >= len {
        Rating::Completed
    } else {
        Rating::Band((caught / 10).min(19) as u8)
    }
}

fn mode_name(mode: DexMode) -> &'static str {
    match mode {
        DexMode::Hoenn => "HOENN",
        DexMode::National => "NATIONAL",
    }
}

/// Uppercase A–Z and spaces, Gen3-encoded, 0xFF-terminated.
fn label(s: &str) -> Vec<u8> {
    let mut o = Vec::with_capacity(s.len() + 1);
    for c in s.bytes() {
        o.push(match c {
            b'A'..=b'Z' => 0xBB + (c - b'A'),
            b' ' => CHAR_SPACE,
            _ => CHAR_SPACE,
        });
    }
    o.push(EOS);
    o
}

fn labeled(name: &str, n: u16) -> Vec<u8> {
    let mut v = label(name);
    v.pop();
    v.push(CHAR_SPACE);
    push_number(&mut v, u32::from(n), 3, true);
    v.push(EOS);
    v
}

fn number_line(index: u16) -> Vec<u8> {
    let mut v = label("NO");
    v.pop();
    v.push(CHAR_SPACE);
    push_number(&mut v, u32::from(index) + 1, 3, true);
    v.push(EOS);
    v
}

fn types_line<G: GameData>(
    game: &G,
    info: Option<&crate::mon::SpeciesInfo>,
    caught: bool,
) -> Vec<u8> {
    let Some(info) = info else {
        return dashes();
    };
    if !caught {
        return dashes();
    }
    let mut v = Vec::new();
    push(
        &mut v,
        game.battle_text(crate::battle::Str::TypeName(info.types[0])),
    );
    if info.types[1] != info.types[0] {
        v.push(CHAR_SLASH);
        push(
            &mut v,
            game.battle_text(crate::battle::Str::TypeName(info.types[1])),
        );
    }
    v.push(EOS);
    v
}

fn weight_line(hectograms: u16, caught: bool) -> Vec<u8> {
    if !caught {
        return dashes();
    }
    let mut v = label("WT");
    v.pop();
    v.push(CHAR_SPACE);
    push_number(&mut v, u32::from(hectograms), 4, false);
    v.push(EOS);
    v
}

fn dashes() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend([0xAE; 5]);
    v.push(EOS);
    v
}

fn draw_rating(
    game: &impl GameData,
    p: &mut Panel,
    mode: DexMode,
    seen: u16,
    caught: u16,
    band: Rating,
) {
    p.text(game, &label(mode_name(mode)), 0, 1);
    p.text(game, &labeled("SEEN", seen), 0, ROW_H + 1);
    p.text(game, &labeled("CAUGHT", caught), 0, 2 * ROW_H + 1);
    p.text(game, &rating_glyphs(band), 0, 3 * ROW_H + 1);
}

fn rating_glyphs(r: Rating) -> Vec<u8> {
    match r {
        Rating::Completed => label("DONE"),
        Rating::Band(b) => {
            let mut v = label("LT");
            v.pop();
            v.push(CHAR_SPACE);
            push_number(&mut v, u32::from(b + 1) * 10, 3, false);
            v.push(EOS);
            v
        }
    }
}

fn place(list: &mut ListMenu, index: u16) {
    let total = list.total;
    let paging = list.paging;
    *list = ListMenu::new(total, LIST_ROWS, paging);
    if total == 0 {
        return;
    }
    let index = index.min(total - 1);
    for _ in 0..index {
        let _ = list.input(Buttons::DOWN);
    }
}

#[cfg(test)]
#[path = "pokedex_tests.rs"]
mod tests;
