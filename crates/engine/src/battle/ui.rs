//! Battle windows and menus ([`BattleWindows`]). **Task F.**
//!
//! Gen3 `sStandardBattleWindowTemplates` (`battle_bg.c`) on the battle VRAM layout
//! (`video::layout::battle`):
//! - the message box (B_WIN_MSG) with a timed [`Printer`] and the `MsgWait` after it,
//! - the action prompt and menu (B_WIN_ACTION_PROMPT / B_WIN_ACTION_MENU,
//!   `PlayerHandleChooseAction`, `HandleInputChooseAction`),
//! - the move menu (B_WIN_MOVE_NAME_1..4, B_WIN_PP, B_WIN_PP_REMAINING, B_WIN_MOVE_TYPE,
//!   `InitMoveSelectionsVarsAndStrings`, `HandleInputChooseMove`) with the PP colors
//!   (`SetPpNumbersPaletteInMoveSelection`, `GetCurrentPpToMaxPpState`),
//! - the learn-move view: the move menu plus the new move as a fifth entry,
//! - the yes/no box (`HandleBattleWindow(YESNOBOX_X_Y)`, B_WIN_YESNO,
//!   `BattleCreateYesNoCursorAt`, `Cmd_yesnoboxlearnmove`'s input) on BG1.
//!
//! Gen3 BG0 is 64 rows tall and scrolls between the message (rows 0..20), action (20..40) and
//! move (40..60) views of `gBattleTextboxTilemap`. Our BGs are 32x32, so a view change (and a
//! cursor move) rewrites the frame rows `FRAME_ROWS` (14..20) of the TEXT BG from the matching
//! rows of the tilemap, with the view's window canvases and cursor laid over them.
//!
//! VRAM:
//! - TEXT (BG0, char block 0): `gBattleTextboxTiles` at 0.. (the first `TEXTBOX_TILES`), then
//!   the canvases from `MSG_WINDOW.base()` on (Gen3 window templates, rebased below 512: Gen3
//!   puts the move windows at 0x300.., past our char block).
//! - WINDOW (BG1, char block 1): the textbox tiles again (yes/no frame and cursor), the yes/no
//!   canvas at `YES_NO_WINDOW.base()`. Tiles 256.. stay free for the ANIM BG.
//! - BG palettes: 0 and 1 `gBattleTextboxPalette` (`BattleGfx::textbox.palettes[0..2]`), 5 the
//!   menu text (`gBattleWindowTextPalette`, `textbox.palettes[2]`), whose colors 12 and 11
//!   take the PP colors (`gPPTextPalette` = `textbox.palettes[3]`, two colors per state).
//!
//! Simplifications (documented deviations):
//! - The move names and the type use the normal font (Gen3: `FONT_NARROW`).
//! - The yes/no box is on BG1 (Gen3: BG0); its frame, text and cursor keep Gen3's positions.
//! - No level-up stats box (B_WIN_LEVEL_UP_BOX, optional in the plan).
//! - In the learn-move view the new move replaces the PP line (no room for both), and the
//!   type line shows the type of the move under the cursor.

#[cfg(test)]
pub(crate) mod test_game;
#[cfg(test)]
mod tests;

use super::{BattleMon, Buff, Msg, MsgWait, Names, Side, Str};
use crate::data::{GameData, MoveId};
use crate::flow::Input;
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::text::{Canvas, Colors, FRAMES_PER_CHAR, Printer, Window};
use crate::video::{BgControl, Cmd, Frame, MAP_SIZE, layout::battle};
use alloc::vec;
use alloc::vec::Vec;

/// `gBattleTextboxTiles` loaded per BG (Gen3 window canvases start at 0x90).
pub const TEXTBOX_TILES: usize = 0x90;
/// Textbox tiles BG1 uses: blank 0, cursor 1..3, erased cursor 0x16, yes/no frame 0x22..0x2B.
pub const BG1_TILES: usize = 0x2C;
/// Rows of the TEXT BG a view redraw rewrites (the textbox frame, rows 14..20).
pub const FRAME_ROWS: core::ops::Range<usize> = 14..20;
/// Menu text palette bank (`paletteNum = 5`).
pub const MENU_PALETTE: u8 = 5;

// Windows (`sStandardBattleWindowTemplates`, view rows folded onto 15..19).
pub const MSG_WINDOW: Window = Window::new(2, 15, 26, 4, 0x090);
pub const ACTION_PROMPT_WINDOW: Window = Window::new(1, 15, 14, 4, 0x0F8);
pub const ACTION_MENU_WINDOW: Window = Window::new(17, 15, 12, 4, 0x130);
pub const MOVE_NAME_WINDOWS: [Window; 4] = [
    Window::new(2, 15, 8, 2, 0x160),
    Window::new(11, 15, 8, 2, 0x170),
    Window::new(2, 17, 8, 2, 0x180),
    Window::new(11, 17, 8, 2, 0x190),
];
pub const PP_WINDOW: Window = Window::new(21, 15, 4, 2, 0x1A0);
pub const PP_REMAINING_WINDOW: Window = Window::new(25, 15, 4, 2, 0x1A8);
pub const MOVE_TYPE_WINDOW: Window = Window::new(21, 17, 8, 2, 0x1B0);
/// The learn-move view's fifth entry (the new move), on the PP line.
pub const LEARN_WINDOW: Window = Window::new(22, 15, 7, 2, 0x1C0);
/// B_WIN_YESNO, on BG1.
pub const YES_NO_WINDOW: Window = Window::new(26, 9, 3, 4, 0x090);
/// `YESNOBOX_X_Y`: the frame `HandleBattleWindow` draws (inclusive corners).
const YES_NO_BOX: (usize, usize, usize, usize) = (24, 8, 29, 13);

// `sTextOnWindowsInfo_Normal` colors (fg, shadow, bg).
const MSG_COLORS: Colors = Colors::new(1, 6, 15);
const MENU_COLORS: Colors = Colors::new(13, 15, 14);
const PP_COLORS: Colors = Colors::new(12, 11, 14);

/// Cursor map entries (`ActionSelectionCreateCursorAt`: tiles 1 and 2, palette 0) and the
/// erased cell (`ActionSelectionDestroyCursorAt`: 0x1016).
const CURSOR: [u16; 2] = [1, 2];
const NO_CURSOR: u16 = 0x1016;

const CHAR_SPACER: u8 = 0x77;
const CHAR_SLASH: u8 = 0xBA;
const CHAR_0: u8 = 0xA1;
const EOS: u8 = 0xFF;

/// An action menu entry (`gActionSelectionCursor`: 0 top left .. 3 bottom right).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Fight,
    Bag,
    Pokemon,
    Run,
}

/// The move menu's outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveChoice {
    /// Move slot 0..4 (in the learn-move view: the move to forget).
    Move(u8),
    /// B (`0xFFFF`), or the new move in the learn-move view (do not learn).
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Msg,
    Action,
    Move,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Menu {
    None,
    Action,
    Move,
    YesNo,
}

/// Gen3 `GetCurrentPpToMaxPpState`: 3 normal, 0 at most half, 1 at most a quarter, 2 empty
/// (the index of the color pair in `gPPTextPalette`).
pub const fn pp_state(cur: u8, max: u8) -> u8 {
    if max == cur {
        3
    } else if max <= 2 {
        if cur > 1 { 3 } else { 2 - cur }
    } else if max <= 7 {
        if cur > 2 { 3 } else { 2 - cur }
    } else if cur == 0 {
        2
    } else if cur <= max / 4 {
        1
    } else if cur > max / 2 {
        3
    } else {
        0
    }
}

/// `ConvertIntToDecimalStringN(.., STR_CONV_MODE_RIGHT_ALIGN, 2)`.
fn push_2_digits(out: &mut Vec<u8>, v: u8) {
    let (tens, ones) = (v / 10, v % 10);
    out.push(match tens {
        0 => CHAR_SPACER,
        1..=9 => CHAR_0 + tens,
        _ => 0xAC, // CHAR_QUESTION_MARK
    });
    out.push(CHAR_0 + ones);
}

fn move_name(game: &impl GameData, m: MoveId) -> &'static [u8] {
    game.battle()
        .moves
        .get(m.0 as usize)
        .map_or(&[EOS][..], |i| i.name)
}

fn push(out: &mut Vec<u8>, s: &[u8]) {
    out.extend(s.iter().take_while(|&&b| b != EOS));
}

/// The battle's text windows and menus. See the module docs.
pub struct BattleWindows {
    loaded: bool,
    view: View,
    menu: Menu,
    msg: Printer,
    /// The wait left once the message is printed (`Frames(n)`: updates left).
    wait: Option<MsgWait>,
    prompt: Canvas,
    action: Canvas,
    names: [Canvas; 4],
    pp_label: Canvas,
    pp: Canvas,
    move_type: Canvas,
    learn: Canvas,
    yes_no: Canvas,
    action_cursor: u8,
    move_cursor: u8,
    yes_no_cursor: u8,
    moves: [Option<MoveId>; 4],
    pp_cur: [u8; 4],
    pp_max: [u8; 4],
    /// The new move of the learn-move view.
    learning: Option<MoveId>,
    action_result: Option<Action>,
    move_result: Option<MoveChoice>,
    yes_no_result: Option<bool>,
}

impl Default for BattleWindows {
    fn default() -> Self {
        Self::new()
    }
}

impl BattleWindows {
    pub fn new() -> Self {
        let text = |w| Canvas::new(w, battle::TEXT.1);
        Self {
            loaded: false,
            view: View::Msg,
            menu: Menu::None,
            msg: Printer::new(MSG_WINDOW, battle::TEXT.1, (0, 1), FRAMES_PER_CHAR),
            wait: None,
            prompt: text(ACTION_PROMPT_WINDOW),
            action: text(ACTION_MENU_WINDOW),
            names: MOVE_NAME_WINDOWS.map(text),
            pp_label: text(PP_WINDOW),
            pp: text(PP_REMAINING_WINDOW),
            move_type: text(MOVE_TYPE_WINDOW),
            learn: text(LEARN_WINDOW),
            yes_no: Canvas::new(YES_NO_WINDOW, battle::WINDOW.1),
            action_cursor: 0,
            move_cursor: 0,
            yes_no_cursor: 0,
            moves: [None; 4],
            pp_cur: [0; 4],
            pp_max: [0; 4],
            learning: None,
            action_result: None,
            move_result: None,
            yes_no_result: None,
        }
    }

    /// Bulk upload, while the screen is black: palettes, the textbox tiles on BG0 and BG1, both
    /// BGs enabled with cleared maps, and the empty message view.
    ///
    /// Exact cost, for one dedicated black frame: 3 `BgPalette` (96 B), `TEXTBOX_TILES` (0x90)
    /// tiles to char block 0 (4608 B), `BG1_TILES` (0x2C) tiles to char block 1 (1408 B), 2
    /// `MapFill` (2 x 2 KiB), 2 `Bg`, one 192-entry `MapEntries` (384 B) and the 104 message
    /// canvas tiles as `BgTile` (3328 B): about 13.9 KiB.
    pub fn load(&mut self, game: &impl GameData, f: &mut Frame) {
        let tb = &game.battle_gfx().textbox;
        for (bank, colors) in tb.palettes.iter().take(2).enumerate() {
            f.cmds.push(Cmd::BgPalette {
                bank: bank as u8,
                colors: *colors,
            });
        }
        if let Some(&colors) = tb.palettes.get(2) {
            f.cmds.push(Cmd::BgPalette {
                bank: MENU_PALETTE,
                colors,
            });
        }
        for ((layer, char_block, screen_block, priority), n) in
            [(battle::TEXT, TEXTBOX_TILES), (battle::WINDOW, BG1_TILES)]
        {
            f.cmds.push(Cmd::BgTiles {
                char_block,
                first: 0,
                tiles: &tb.tiles[..tb.tiles.len().min(n)],
            });
            f.cmds.push(Cmd::MapFill {
                screen_block,
                entry: 0,
            });
            f.cmds.push(Cmd::Bg {
                layer,
                control: BgControl {
                    enabled: true,
                    priority,
                    char_block,
                    screen_block,
                },
            });
        }
        self.loaded = true;
        // The action menu's text never changes: drawn here, on a black frame (text is slow on
        // the GBA), and uploaded with its first view.
        self.draw_action_menu(game);
        self.msg.clear(MSG_COLORS);
        self.show(View::Msg, game, f);
        self.msg.flush(f);
    }

    /// Clears the message box and starts printing `text` (expanded, 0xFF-terminated; see
    /// `battle::text::expand`), then waits per `wait`: `Frames(n)` = `n` more updates once the
    /// last character is drawn (`Cmd_waitmessage`), `Button` = until A or B is pressed.
    /// Switches to the message view.
    pub fn print(&mut self, game: &impl GameData, text: Vec<u8>, wait: MsgWait, f: &mut Frame) {
        self.leave_menu(f);
        self.msg.start(text, MSG_COLORS, true);
        self.wait = match wait {
            MsgWait::Frames(0) => None,
            w => Some(w),
        };
        if self.view != View::Msg {
            self.show(View::Msg, game, f);
        }
        self.msg.flush(f);
    }

    /// True while a message prints or its wait runs. The printer and the wait are frozen while
    /// a menu is open (`update` only runs the menu), and opening a menu drops a pending wait: a
    /// menu must not be opened while a message is still printing (debug-asserted).
    pub fn is_printing(&self) -> bool {
        self.msg.is_printing() || self.wait.is_some()
    }

    /// Closes the open menu without an answer: the yes/no box is erased from BG1 (the other
    /// menus are replaced by the next view).
    fn leave_menu(&mut self, f: &mut Frame) {
        if self.menu == Menu::YesNo {
            self.put_yes_no(f, true);
        }
        self.menu = Menu::None;
    }

    /// Opens a menu: leaves the current one and drops a pending message wait.
    fn enter_menu(&mut self, menu: Menu, f: &mut Frame) {
        debug_assert!(
            !self.msg.is_printing(),
            "a battle menu opened while a message is still printing"
        );
        self.leave_menu(f);
        self.wait = None;
        self.menu = menu;
    }

    /// True while a menu (action, move or yes/no) waits for input.
    pub fn is_menu_active(&self) -> bool {
        self.menu != Menu::None
    }

    /// Shows the action view: "What will X do?" (`gText_WhatWillPkmnDo` for the player's
    /// battler) and the 2x2 menu (`gText_BattleMenu`), the cursor where it was last time.
    pub fn action_menu(&mut self, game: &impl GameData, names: &Names, f: &mut Frame) {
        let msg = Msg {
            id: Str::WhatWillPkmnDo,
            attacker: Side::Player,
            target: Side::Opponent,
            active: Side::Player,
            mov: None,
            buffs: [Buff::None; 3],
            wait: MsgWait::Frames(0),
            slots: [names.mons[0].party, names.mons[1].party],
        };
        let prompt = super::text::expand(game, &msg, names);
        self.prompt.redraw(game.font(), &prompt, 1, 1, MSG_COLORS);
        self.draw_action_menu(game);
        self.enter_menu(Menu::Action, f);
        self.action_result = None;
        self.show(View::Action, game, f);
    }

    /// The 2x2 action menu (`gText_BattleMenu`).
    fn draw_action_menu(&mut self, game: &impl GameData) {
        let menu = game.battle_text(Str::BattleMenu);
        self.action.redraw(game.font(), menu, 0, 1, MENU_COLORS);
    }

    /// Draws the action prompt and the move view of `mon` ahead of time, while the screen is
    /// black (`BattleScene::load`): the menus' first opening then finds its text drawn (the
    /// canvases skip unchanged text), as text is slow on the GBA. Nothing is shown.
    pub fn prepare(&mut self, game: &impl GameData, names: &Names, mon: &BattleMon) {
        let msg = Msg {
            id: Str::WhatWillPkmnDo,
            attacker: Side::Player,
            target: Side::Opponent,
            active: Side::Player,
            mov: None,
            buffs: [Buff::None; 3],
            wait: MsgWait::Frames(0),
            slots: [names.mons[0].party, names.mons[1].party],
        };
        let prompt = super::text::expand(game, &msg, names);
        self.prompt.redraw(game.font(), &prompt, 1, 1, MSG_COLORS);
        self.draw_moves(game, mon, None);
        self.draw_move_text(game);
    }

    /// Shows the move menu of `mon`: its 4 move names, "PP cur/max" in the PP colors and
    /// "TYPE/type" of the move under the cursor. A move without PP stays selectable (the core
    /// answers `NoPPLeft`), as Gen3.
    pub fn move_menu(&mut self, game: &impl GameData, mon: &BattleMon, f: &mut Frame) {
        self.open_moves(game, mon, None, f);
    }

    /// The learn-move view: `mon`'s moves plus `new` as a fifth entry. `MoveChoice::Move(i)`
    /// forgets slot `i`; B or the new move gives `MoveChoice::Back` (do not learn).
    pub fn learn_move_menu(
        &mut self,
        game: &impl GameData,
        mon: &BattleMon,
        new: MoveId,
        f: &mut Frame,
    ) {
        self.open_moves(game, mon, Some(new), f);
    }

    fn open_moves(
        &mut self,
        game: &impl GameData,
        mon: &BattleMon,
        learning: Option<MoveId>,
        f: &mut Frame,
    ) {
        self.draw_moves(game, mon, learning);
        self.enter_menu(Menu::Move, f);
        self.move_result = None;
        self.draw_move_info(game, f);
        self.show(View::Move, game, f);
    }

    /// The move view's state and its names (and the "PP" label or the new move).
    fn draw_moves(&mut self, game: &impl GameData, mon: &BattleMon, learning: Option<MoveId>) {
        self.moves = mon.moves;
        self.pp_cur = mon.pp;
        self.pp_max = mon.max_pp;
        self.learning = learning;
        // Gen3 keeps `gMoveSelectionCursor` between turns; it must point at an entry.
        if self.cursor_move().is_none() {
            self.move_cursor = 0;
        }
        let font = game.font();
        for i in 0..4 {
            // An empty slot shows `gMoveNames[MOVE_NONE]`.
            let name = move_name(game, self.moves[i].unwrap_or(MoveId(0)));
            self.names[i].redraw(font, name, 0, 1, MENU_COLORS);
        }
        if let Some(new) = learning {
            let name = move_name(game, new);
            self.learn.redraw(font, name, 0, 1, MENU_COLORS);
        } else {
            let label = game.battle_text(Str::MoveInterfacePP);
            self.pp_label.redraw(font, label, 0, 1, PP_COLORS);
        }
    }

    /// The move under the cursor (the new move on the fifth entry).
    fn cursor_move(&self) -> Option<MoveId> {
        match self.move_cursor {
            4 => self.learning,
            c if c > 4 => None,
            c => self.moves.get(c as usize).copied().flatten(),
        }
    }

    /// Number of moves (`gNumberOfMovesToChoose`). Assumes packed move slots (Gen3 keeps them
    /// packed: no `None` before a `Some`), as the cursor limits `(c ^ 1) < n` / `(c ^ 2) < n` do.
    fn move_count(&self) -> u8 {
        self.moves.iter().filter(|m| m.is_some()).count() as u8
    }

    /// `MoveSelectionDisplayPpNumber` (with its palette) and `MoveSelectionDisplayMoveType`.
    fn draw_move_info(&mut self, game: &impl GameData, f: &mut Frame) {
        self.draw_move_text(game);
        if self.learning.is_none() {
            let c = self.move_cursor as usize & 3;
            let (cur, max) = (self.pp_cur[c], self.pp_max[c]);
            let tb = &game.battle_gfx().textbox;
            if let (Some(&base), Some(pp)) = (tb.palettes.get(2), tb.palettes.get(3)) {
                let s = pp_state(cur, max) as usize * 2;
                let mut colors = base;
                colors[12] = pp[s];
                colors[11] = pp[s + 1];
                f.cmds.push(Cmd::BgPalette {
                    bank: MENU_PALETTE,
                    colors,
                });
            }
        }
    }

    /// The text of [`Self::draw_move_info`]: "cur/max" PP of the move under the cursor (not
    /// in the learn-move view) and "TYPE/type".
    fn draw_move_text(&mut self, game: &impl GameData) {
        let font = game.font();
        if self.learning.is_none() {
            let c = self.move_cursor as usize & 3;
            let (cur, max) = (self.pp_cur[c], self.pp_max[c]);
            let mut text = Vec::with_capacity(6);
            push_2_digits(&mut text, cur);
            text.push(CHAR_SLASH);
            push_2_digits(&mut text, max);
            text.push(EOS);
            self.pp.redraw(font, &text, 2, 1, PP_COLORS);
        }
        let mut text = Vec::with_capacity(16);
        push(&mut text, game.battle_text(Str::MoveInterfaceType));
        if let Some(m) = self.cursor_move()
            && let Some(info) = game.battle().moves.get(m.0 as usize)
        {
            push(&mut text, game.battle_text(Str::TypeName(info.ty)));
        }
        text.push(EOS);
        self.move_type.redraw(font, &text, 0, 1, MENU_COLORS);
    }

    /// Shows the yes/no box on BG1 (`gText_BattleYesNoChoice`), cursor on yes
    /// (`Cmd_yesnoboxlearnmove`). The current view stays.
    pub fn yes_no(&mut self, game: &impl GameData, f: &mut Frame) {
        let text = game.battle_text(Str::BattleYesNoChoice);
        self.yes_no.redraw(game.font(), text, 0, 1, MENU_COLORS);
        self.yes_no_cursor = 0;
        self.yes_no_result = None;
        self.enter_menu(Menu::YesNo, f);
        self.put_yes_no(f, false);
        self.yes_no.flush(f);
    }

    /// One frame: menu input, or the printer and its wait.
    pub fn update(&mut self, game: &impl GameData, input: &Input, f: &mut Frame) {
        let p = input.pressed;
        match self.menu {
            Menu::Action => self.update_action(game, p, f),
            Menu::Move => self.update_move(game, p, f),
            Menu::YesNo => self.update_yes_no(p, f),
            Menu::None => {
                if self.msg.is_printing() {
                    self.msg.update(game.font(), p, input.held);
                } else {
                    self.wait = match self.wait {
                        Some(MsgWait::Frames(n)) if n > 1 => Some(MsgWait::Frames(n - 1)),
                        Some(MsgWait::Button)
                            if !p.contains(Buttons::A) && !p.contains(Buttons::B) =>
                        {
                            Some(MsgWait::Button)
                        }
                        _ => None,
                    };
                }
            }
        }
        self.msg.flush(f);
    }

    /// `HandleInputChooseAction` (singles: B does nothing).
    fn update_action(&mut self, game: &impl GameData, p: Buttons, f: &mut Frame) {
        let c = self.action_cursor;
        let to = if p.contains(Buttons::A) {
            self.action_result =
                Some([Action::Fight, Action::Bag, Action::Pokemon, Action::Run][c as usize & 3]);
            self.menu = Menu::None;
            return;
        } else if p.contains(Buttons::LEFT) {
            if c & 1 != 0 { c ^ 1 } else { c }
        } else if p.contains(Buttons::RIGHT) {
            if c & 1 == 0 { c ^ 1 } else { c }
        } else if p.contains(Buttons::UP) {
            if c & 2 != 0 { c ^ 2 } else { c }
        } else if p.contains(Buttons::DOWN) {
            if c & 2 == 0 { c ^ 2 } else { c }
        } else {
            c
        };
        if to != c {
            self.action_cursor = to;
            self.put_rows(game, f);
        }
    }

    /// `HandleInputChooseMove` (singles, no target selection, no SELECT move swap), plus the
    /// fifth entry of the learn-move view (right of slots 1 and 3).
    fn update_move(&mut self, game: &impl GameData, p: Buttons, f: &mut Frame) {
        let c = self.move_cursor;
        let n = self.move_count();
        let learn = self.learning.is_some();
        let to = if p.contains(Buttons::A) {
            self.move_result = Some(if c == 4 {
                MoveChoice::Back
            } else {
                MoveChoice::Move(c)
            });
            self.menu = Menu::None;
            return;
        } else if p.contains(Buttons::B) {
            self.move_result = Some(MoveChoice::Back);
            self.menu = Menu::None;
            return;
        } else if p.contains(Buttons::LEFT) {
            match c {
                4 => 1,
                c if c & 1 != 0 => c ^ 1,
                c => c,
            }
        } else if p.contains(Buttons::RIGHT) {
            match c {
                4 => 4,
                c if c & 1 == 0 && (c ^ 1) < n => c ^ 1,
                c if c & 1 != 0 && learn => 4,
                c => c,
            }
        } else if p.contains(Buttons::UP) {
            if c != 4 && c & 2 != 0 { c ^ 2 } else { c }
        } else if p.contains(Buttons::DOWN) {
            if c != 4 && c & 2 == 0 && (c ^ 2) < n {
                c ^ 2
            } else {
                c
            }
        } else {
            c
        };
        if to != c {
            self.move_cursor = to;
            self.draw_move_info(game, f);
            self.put_rows(game, f);
        }
    }

    /// `Cmd_yesnoboxlearnmove`'s input: up/down, A picks, B is no.
    fn update_yes_no(&mut self, p: Buttons, f: &mut Frame) {
        let c = self.yes_no_cursor;
        if p.contains(Buttons::UP) && self.yes_no_cursor != 0 {
            self.yes_no_cursor = 0;
        }
        if p.contains(Buttons::DOWN) && self.yes_no_cursor == 0 {
            self.yes_no_cursor = 1;
        }
        if p.contains(Buttons::A) {
            self.yes_no_result = Some(self.yes_no_cursor == 0);
        } else if p.contains(Buttons::B) {
            self.yes_no_result = Some(false);
        }
        if self.yes_no_result.is_some() {
            self.menu = Menu::None;
            self.put_yes_no(f, true);
        } else if self.yes_no_cursor != c {
            self.put_yes_no(f, false);
        }
    }

    /// The cursor of the menu waiting for input (action, move or yes/no), for scripted input
    /// and debugging; `None` when no menu is open.
    pub fn menu_cursor(&self) -> Option<u8> {
        match self.menu {
            Menu::Action => Some(self.action_cursor),
            Menu::Move => Some(self.move_cursor),
            Menu::YesNo => Some(self.yes_no_cursor),
            Menu::None => None,
        }
    }

    /// The action menu's choice, until the next `action_menu`.
    pub fn action_result(&self) -> Option<Action> {
        self.action_result
    }

    /// The move menu's choice, until the next `move_menu` / `learn_move_menu`.
    pub fn move_result(&self) -> Option<MoveChoice> {
        self.move_result
    }

    /// The yes/no choice (`Some(true)` = yes), until the next `yes_no`.
    pub fn yes_no_result(&self) -> Option<bool> {
        self.yes_no_result
    }

    fn show(&mut self, view: View, game: &impl GameData, f: &mut Frame) {
        self.view = view;
        self.put_rows(game, f);
    }

    /// The TEXT BG entries of rows `FRAME_ROWS` for the current view.
    fn rows(&self, game: &impl GameData) -> Vec<u16> {
        let map = game.battle_gfx().textbox.tilemap;
        let view_row = match self.view {
            View::Msg => 0,
            View::Action => 20,
            View::Move => 40,
        };
        let first = (view_row + FRAME_ROWS.start) * MAP_SIZE;
        let mut e = vec![0u16; FRAME_ROWS.len() * MAP_SIZE];
        for (i, v) in e.iter_mut().enumerate() {
            *v = map.get(first + i).copied().unwrap_or(0);
        }
        let mut put = |w: Window, pal: u16| {
            for y in 0..w.height() as usize {
                for x in 0..w.width() as usize {
                    let (col, row) = (w.left() as usize + x, w.top() as usize + y);
                    let tile = w.base() + (y * w.width() as usize + x) as u16;
                    e[(row - FRAME_ROWS.start) * MAP_SIZE + col] = tile | pal << 12;
                }
            }
        };
        let menu = MENU_PALETTE as u16;
        match self.view {
            View::Msg => put(MSG_WINDOW, 0),
            View::Action => {
                put(ACTION_PROMPT_WINDOW, 0);
                put(ACTION_MENU_WINDOW, menu);
            }
            View::Move => {
                for w in MOVE_NAME_WINDOWS {
                    put(w, menu);
                }
                if self.learning.is_some() {
                    put(LEARN_WINDOW, menu);
                } else {
                    put(PP_WINDOW, menu);
                    put(PP_REMAINING_WINDOW, menu);
                }
                put(MOVE_TYPE_WINDOW, menu);
            }
        }
        let mut cursor = |col: usize, row: usize, on: bool| {
            for k in 0..2 {
                e[(row + k - FRAME_ROWS.start) * MAP_SIZE + col] =
                    if on { CURSOR[k] } else { NO_CURSOR };
            }
        };
        match self.view {
            View::Msg => {}
            // `ActionSelectionCreateCursorAt`: x 7 * (pos & 1) + 16, y 35 + (pos & 2).
            View::Action => {
                for pos in 0..4u8 {
                    let col = 7 * (pos as usize & 1) + 16;
                    cursor(col, 15 + (pos as usize & 2), pos == self.action_cursor);
                }
            }
            // `MoveSelectionCreateCursorAt`: x 9 * (pos & 1) + 1, y 55 + (pos & 2).
            View::Move => {
                for pos in 0..4u8 {
                    let col = 9 * (pos as usize & 1) + 1;
                    cursor(col, 15 + (pos as usize & 2), pos == self.move_cursor);
                }
                if self.learning.is_some() {
                    cursor(LEARN_WINDOW.left() as usize - 1, 15, self.move_cursor == 4);
                }
            }
        }
        e
    }

    /// Rewrites rows `FRAME_ROWS` of the TEXT BG and uploads the view's canvases.
    fn put_rows(&mut self, game: &impl GameData, f: &mut Frame) {
        f.cmds.push(Cmd::MapEntries {
            screen_block: battle::TEXT.2,
            start: (FRAME_ROWS.start * MAP_SIZE) as u16,
            entries: self.rows(game),
        });
        match self.view {
            View::Msg => self.msg.flush(f),
            View::Action => {
                self.prompt.flush(f);
                self.action.flush(f);
            }
            View::Move => {
                for c in &mut self.names {
                    c.flush(f);
                }
                if self.learning.is_some() {
                    self.learn.flush(f);
                } else {
                    self.pp_label.flush(f);
                    self.pp.flush(f);
                }
                self.move_type.flush(f);
            }
        }
    }

    /// The BG1 entries of rows 8..14 (`YESNOBOX_X_Y`): the box with its window and cursor, or
    /// nothing (`HandleBattleWindow(.., WINDOW_CLEAR)`).
    fn yes_no_rows(&self, clear: bool) -> Vec<u16> {
        let (x0, y0, x1, y1) = YES_NO_BOX;
        let mut e = vec![0u16; (y1 - y0 + 1) * MAP_SIZE];
        if clear {
            return e;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                // `HandleBattleWindow`: corners 0x22/0x24/0x28/0x2A, edges 0x23/0x25/0x27/0x29,
                // inside 0x26, all palette 1.
                let row = if y == y0 {
                    0x22
                } else if y == y1 {
                    0x28
                } else {
                    0x25
                };
                let col = if x == x0 {
                    0
                } else if x == x1 {
                    2
                } else {
                    1
                };
                e[(y - y0) * MAP_SIZE + x] = 0x1000 | (row + col);
            }
        }
        let w = YES_NO_WINDOW;
        for y in 0..w.height() as usize {
            for x in 0..w.width() as usize {
                let (col, row) = (w.left() as usize + x, w.top() as usize + y);
                let tile = w.base() + (y * w.width() as usize + x) as u16;
                e[(row - y0) * MAP_SIZE + col] = tile | (MENU_PALETTE as u16) << 12;
            }
        }
        // `BattleCreateYesNoCursorAt`: x 0x19, y 9 + 2 * pos.
        for pos in 0..2usize {
            for k in 0..2 {
                e[(9 + 2 * pos + k - y0) * MAP_SIZE + 0x19] = if pos == self.yes_no_cursor as usize
                {
                    CURSOR[k]
                } else {
                    NO_CURSOR
                };
            }
        }
        e
    }

    fn put_yes_no(&self, f: &mut Frame, clear: bool) {
        f.cmds.push(Cmd::MapEntries {
            screen_block: battle::WINDOW.2,
            start: (YES_NO_BOX.1 * MAP_SIZE) as u16,
            entries: self.yes_no_rows(clear),
        });
    }

    /// Feeds all state into `h` (the canvases follow from it and the game data).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        let opt = |o: Option<u8>| o.unwrap_or(0xFF);
        h.write(&[
            self.loaded as u8,
            self.view as u8,
            self.menu as u8,
            self.action_cursor,
            self.move_cursor,
            self.yes_no_cursor,
            opt(self.action_result.map(|a| a as u8)),
            opt(self.move_result.map(|m| match m {
                MoveChoice::Move(i) => i,
                MoveChoice::Back => 4,
            })),
            opt(self.yes_no_result.map(|r| r as u8)),
        ]);
        h.write(&match self.wait {
            None => [0, 0],
            Some(MsgWait::Frames(n)) => [1, n],
            Some(MsgWait::Button) => [2, 0],
        });
        for i in 0..4 {
            h.write_u16(self.moves[i].map_or(0xFFFF, |m| m.0));
            h.write(&[self.pp_cur[i], self.pp_max[i]]);
        }
        h.write_u16(self.learning.map_or(0xFFFF, |m| m.0));
        self.msg.hash(h);
    }
}
