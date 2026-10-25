//! Full-screen and overlay screens (Milestone 9): the start menu, party, summary, bag, mart,
//! PC, options, save dialog, main menu, evolution scene and naming screen.
//!
//! A screen is a module implementing [`Screen`]; [`ScreenState`] holds one of each kind and
//! dispatches to it with one `match`. The World keeps a [`ScreenStack`] (start menu → party →
//! summary; mart → bag; bag → party) in its mode while screens own the frame (`world/mode.rs`,
//! hosted by `world/screens.rs`). The stack is hashed while non-empty, and a world with a
//! screen open is not capturable.
//!
//! **Wave 0** (this file is frozen after it): the types, the stack and the dispatch; every
//! screen module is a stub that answers `Done(Cancel)` on its first frame with
//! `Report::Unsupported("<screen>")`. Each owner replaces its module's body behind the same
//! `new` and [`Screen`] impl; the shared widgets go in [`kit`] (Task K).

pub mod bag;
pub(crate) mod credits;
mod credits_pages;
pub mod easy_chat;
pub mod evolution;
pub mod fly_map;
pub mod hall_of_fame;
pub mod kit;
pub mod main_menu;
pub mod mart;
pub mod naming;
pub mod options;
pub mod party;
pub mod pc;
pub mod save;
pub mod start_menu;
pub mod summary;
pub mod title;

use crate::bag::Bag;
use crate::checkpoint::PlayerName;
use crate::data::{GameData, ItemId, MapId, MoveId, ScriptId, SpeciesId};
use crate::flow::Vars;
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::mon::Party;
use crate::options::Options;
use crate::pc::PcStorage;
use crate::pokedex::Pokedex;
use crate::rng::Rng;
use crate::script::Report;
use crate::video::Frame;
use alloc::vec::Vec;

/// Screens open at once at most (start menu → bag → party → summary).
pub const MAX_DEPTH: usize = 4;

/// Which screen to open, with its arguments.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenKind {
    StartMenu,
    Party(PartyMode),
    Summary {
        slot: u8,
        mode: SummaryMode,
    },
    Bag(BagMode),
    Mart {
        items: &'static [ItemId],
    },
    Pc(PcMode),
    Options,
    SaveDialog,
    Evolution {
        slot: u8,
        into: SpeciesId,
        can_cancel: bool,
    },
    MainMenu,
    Naming(NamingKind),
    /// Fly's destination picker (Gen3 `CB2_OpenFlyMap`), from the party menu's FLY.
    FlyMap,
    /// Gen3 `CB2_DoHallOfFameScreen` then the credits (`special GameClear`; Task S).
    HallOfFame,
    /// The Hall of Fame screen from its credits: no record, no save (`Request::Credits`).
    Credits,
    /// Gen3 `ShowEasyChatScreen` (`special ShowEasyChatScreen`) of `EASY_CHAT_TYPE_*` (Wave G).
    EasyChat(u8),
    /// Power-on title (`title_screen.c`): copyright, press Start, the Pokémon flight.
    Title,
}

/// The question the save dialog asks after "Would you like to save the game?" yes (Gen3
/// `SaveConfirmInputCallback` on `gSaveFileStatus` and `gDifferentSaveFile`; Task S).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SaveQuestion {
    /// A new game and no save in flash: straight to "SAVING…".
    #[default]
    None,
    /// "There is already a saved file. Is it okay to overwrite it?" (default yes).
    Overwrite,
    /// "WARNING! There is a different game file…" (default no).
    Different,
}

/// The party menu's modes (Gen3 `PARTY_MENU_TYPE_*` / `PARTY_ACTION_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartyMode {
    /// From the start menu.
    Field,
    /// `special ChoosePartyMon`.
    Choose,
    /// `special ChooseMonForMoveTutor` (ABLE / NOT ABLE for the move).
    MoveTutor(MoveId),
    /// Using an item from the bag on a mon.
    UseItem(ItemId),
    /// Giving an item from the bag.
    GiveItem(ItemId),
    /// The battle's POKéMON action; `forced` after a faint (no CANCEL). Task BM hosts it inside
    /// the battle with the battle's party in battle order (Gen3 `gBattlePartyCurrentOrder`), so
    /// the active mon is slot 0.
    Battle { forced: bool },
    /// `special ChooseHalfPartyForBattle` (`PARTY_MENU_TYPE_CHOOSE_HALF`): ENTER up to `max`
    /// mons (at least `min`) in order, then CONFIRM. Answers the order (see
    /// [`party::pack_order`](crate::ui::party::pack_order)), `Cancel` when cancelled.
    ChooseHalf { max: u8, min: u8 },
}

/// The bag's modes (Gen3 `ITEMMENULOCATION_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BagMode {
    Field,
    /// From the mart's SELL.
    Sell,
    Battle,
    ChooseBerry,
}

/// The summary's modes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SummaryMode {
    View,
    /// Choosing the move to forget for a new one (the learn-move flow).
    ReplaceMove(MoveId),
}

/// The PC entry points (Gen3 `PlayerPC`, `BedroomPC`, `ShowPokemonStorageSystemPC`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PcMode {
    /// `PlayerPC` from a Pokémon Center.
    Someone,
    Bedroom,
    /// `ShowPokemonStorageSystemPC`.
    Storage,
}

/// What the naming screen names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NamingKind {
    /// `NAMING_SCREEN_PLAYER` (`PLAYER_NAME_LENGTH`, 7).
    Player,
    /// The rival (`rivalName`, the same 7 characters as the player).
    Rival,
    /// Party slot (`POKEMON_NAME_LENGTH`, 10).
    Mon(u8),
    /// PC box (`BOX_NAME_LENGTH`, 8).
    Box(u8),
}

/// A [`ScreenKind`] without its arguments (`Event::ScreenOpened`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenKindTag {
    StartMenu,
    Party,
    Summary,
    Bag,
    Mart,
    Pc,
    Options,
    SaveDialog,
    Evolution,
    MainMenu,
    Naming,
    FlyMap,
    HallOfFame,
    Credits,
    EasyChat,
    Title,
}

impl ScreenKind {
    pub const fn tag(self) -> ScreenKindTag {
        match self {
            ScreenKind::StartMenu => ScreenKindTag::StartMenu,
            ScreenKind::Party(_) => ScreenKindTag::Party,
            ScreenKind::Summary { .. } => ScreenKindTag::Summary,
            ScreenKind::Bag(_) => ScreenKindTag::Bag,
            ScreenKind::Mart { .. } => ScreenKindTag::Mart,
            ScreenKind::Pc(_) => ScreenKindTag::Pc,
            ScreenKind::Options => ScreenKindTag::Options,
            ScreenKind::SaveDialog => ScreenKindTag::SaveDialog,
            ScreenKind::Evolution { .. } => ScreenKindTag::Evolution,
            ScreenKind::MainMenu => ScreenKindTag::MainMenu,
            ScreenKind::Naming(_) => ScreenKindTag::Naming,
            ScreenKind::FlyMap => ScreenKindTag::FlyMap,
            ScreenKind::HallOfFame => ScreenKindTag::HallOfFame,
            ScreenKind::EasyChat(_) => ScreenKindTag::EasyChat,
            ScreenKind::Credits => ScreenKindTag::Credits,
            ScreenKind::Title => ScreenKindTag::Title,
        }
    }

    /// Feeds the kind and its arguments into `h` (a mart list by its length and address-free
    /// digest).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.tag() as u8]);
        match *self {
            ScreenKind::StartMenu
            | ScreenKind::Options
            | ScreenKind::SaveDialog
            | ScreenKind::MainMenu
            | ScreenKind::FlyMap
            | ScreenKind::HallOfFame
            | ScreenKind::Credits
            | ScreenKind::Title => {}
            ScreenKind::Party(m) => {
                let (tag, v) = match m {
                    PartyMode::Field => (0, 0),
                    PartyMode::Choose => (1, 0),
                    PartyMode::MoveTutor(m) => (2, m.0),
                    PartyMode::UseItem(i) => (3, i.0),
                    PartyMode::GiveItem(i) => (4, i.0),
                    PartyMode::Battle { forced } => (5, forced as u16),
                    PartyMode::ChooseHalf { max, min } => (6, max as u16 | (min as u16) << 8),
                };
                h.write(&[tag]);
                h.write_u16(v);
            }
            ScreenKind::Summary { slot, mode } => {
                let (tag, v) = match mode {
                    SummaryMode::View => (0, 0),
                    SummaryMode::ReplaceMove(m) => (1, m.0),
                };
                h.write(&[slot, tag]);
                h.write_u16(v);
            }
            ScreenKind::Bag(m) => h.write(&[m as u8]),
            ScreenKind::EasyChat(k) => h.write(&[k]),
            ScreenKind::Mart { items } => {
                h.write_u16(items.len() as u16);
                for i in items {
                    h.write_u16(i.0);
                }
            }
            ScreenKind::Pc(m) => h.write(&[m as u8]),
            ScreenKind::Evolution {
                slot,
                into,
                can_cancel,
            } => {
                h.write(&[slot, can_cancel as u8]);
                h.write_u16(into.0);
            }
            ScreenKind::Naming(k) => {
                let (tag, v) = match k {
                    NamingKind::Player => (0, 0),
                    NamingKind::Mon(s) => (1, s),
                    NamingKind::Box(b) => (2, b),
                    NamingKind::Rival => (3, 0),
                };
                h.write(&[tag, v]);
            }
        }
    }
}

/// What a screen answers when it closes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenResult {
    /// Closed with B / CANCEL (or unsupported).
    Cancel,
    /// A value: a party slot, a list index, …
    Value(u16),
    /// A yes/no outcome: saved, evolved, …
    Bool(bool),
}

/// A screen's answer for one frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenOut {
    Running,
    /// Open another screen on top (bag → party).
    Push(ScreenKind),
    /// Close this screen with a result (to the screen below, or to the World).
    Done(ScreenResult),
    /// Close every screen and run the script with the field locked (the start menu's
    /// game-defined entry).
    RunScript(ScriptId),
    /// Ask the World to save now (`world/save.rs`); the screen gets the result next frame
    /// through [`ScreenCx::saved`], and the World raises `Event::Saved`.
    Save,
    /// Close every screen and use the item on the field (Gen3 `SetUpItemUseOnFieldCallback`:
    /// the bag closes, the field comes back, then the item's field callback runs; Escape Rope,
    /// and the M10 bikes, rods and Itemfinder): `item_use::use_on_field` (Task B).
    UseOnField(ItemId),
    /// Close every screen and Fly to destination `i` ([`crate::data::GameData::fly_destination`];
    /// Gen3 `ReturnToFieldFromFlyMapSelect` → `FieldCallback_UseFly`).
    Fly(u8),
}

/// What a screen may touch: the game, this frame's output, and the player's state.
pub struct ScreenCx<'a, G: GameData> {
    pub game: &'a G,
    pub f: &'a mut Frame,
    /// The world's random numbers (friendship events from items, Task E's `on_event`).
    pub rng: &'a mut Rng,
    pub party: &'a mut Party,
    pub bag: &'a mut Bag,
    pub money: &'a mut u32,
    pub pc: &'a mut PcStorage,
    pub options: &'a mut Options,
    pub coins: &'a mut u16,
    pub pokedex: &'a mut Pokedex,
    pub registered_item: &'a mut Option<ItemId>,
    /// Flags and vars: read by most screens, written by item use (Gen3 `VAR_REPEL_STEP_COUNT`)
    /// and the PC (`VAR_PC_BOX_TO_SEND_MON`).
    pub vars: &'a mut Vars,
    pub player_name: &'a mut PlayerName,
    /// The rival's name once the naming screen has set one. `None` leaves `{RIVAL}` on the
    /// game's static name.
    pub rival_name: &'a mut Option<PlayerName>,
    /// The current map (the save dialog's location line).
    pub map: MapId,
    /// The World's frame count (the save dialog's and CONTINUE window's play time).
    pub frame: u32,
    /// The result of the last [`ScreenOut::Save`], given once on the frame after it.
    pub saved: Option<bool>,
    /// The save dialog's question after its first yes (Task S, `world/save.rs`).
    pub save_question: SaveQuestion,
    /// The result of the screen this one pushed, given once on the frame it closed.
    pub child: Option<ScreenResult>,
    /// Reports raised this frame (the World logs them and raises `Event::Report`).
    pub reports: &'a mut Vec<Report>,
}

impl<G: GameData> ScreenCx<'_, G> {
    pub fn report(&mut self, r: Report) {
        self.reports.push(r);
    }
}

/// One screen.
pub trait Screen {
    /// One frame with `input` newly pressed (`Input::pressed`) and held.
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut;
    /// Feeds the screen's state into `h` (parity contract).
    fn hash(&self, h: &mut Fnv64);
}

/// The buttons a screen sees this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ScreenInput {
    pub pressed: Buttons,
    pub held: Buttons,
}

/// One open screen, by kind (the dispatch).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScreenState {
    StartMenu(start_menu::StartMenu),
    Party(party::PartyMenu),
    Summary(summary::Summary),
    Bag(bag::BagMenu),
    Mart(mart::Mart),
    Pc(pc::PcMenu),
    Options(options::OptionsMenu),
    SaveDialog(save::SaveDialog),
    Evolution(evolution::EvolutionScene),
    MainMenu(main_menu::MainMenu),
    Naming(naming::NamingScreen),
    FlyMap(fly_map::FlyMap),
    HallOfFame(hall_of_fame::HallOfFame),
    EasyChat(easy_chat::EasyChat),
    Title(title::Title),
}

impl ScreenState {
    pub fn new(kind: ScreenKind) -> ScreenState {
        match kind {
            ScreenKind::StartMenu => ScreenState::StartMenu(start_menu::StartMenu::new()),
            ScreenKind::Party(m) => ScreenState::Party(party::PartyMenu::new(m)),
            ScreenKind::Summary { slot, mode } => {
                ScreenState::Summary(summary::Summary::new(slot, mode))
            }
            ScreenKind::Bag(m) => ScreenState::Bag(bag::BagMenu::new(m)),
            ScreenKind::Mart { items } => ScreenState::Mart(mart::Mart::new(items)),
            ScreenKind::Pc(m) => ScreenState::Pc(pc::PcMenu::new(m)),
            ScreenKind::Options => ScreenState::Options(options::OptionsMenu::new()),
            ScreenKind::SaveDialog => ScreenState::SaveDialog(save::SaveDialog::new()),
            ScreenKind::Evolution {
                slot,
                into,
                can_cancel,
            } => ScreenState::Evolution(evolution::EvolutionScene::new(slot, into, can_cancel)),
            ScreenKind::MainMenu => ScreenState::MainMenu(main_menu::MainMenu::new()),
            ScreenKind::Naming(k) => ScreenState::Naming(naming::NamingScreen::new(k)),
            ScreenKind::FlyMap => ScreenState::FlyMap(fly_map::FlyMap::new()),
            ScreenKind::HallOfFame => ScreenState::HallOfFame(hall_of_fame::HallOfFame::new()),
            ScreenKind::EasyChat(k) => ScreenState::EasyChat(easy_chat::EasyChat::new(k)),
            ScreenKind::Credits => ScreenState::HallOfFame(hall_of_fame::HallOfFame::credits()),
            ScreenKind::Title => ScreenState::Title(title::Title::new()),
        }
    }

    pub fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        match self {
            ScreenState::StartMenu(s) => s.step(cx, input),
            ScreenState::Party(s) => s.step(cx, input),
            ScreenState::Summary(s) => s.step(cx, input),
            ScreenState::Bag(s) => s.step(cx, input),
            ScreenState::Mart(s) => s.step(cx, input),
            ScreenState::Pc(s) => s.step(cx, input),
            ScreenState::Options(s) => s.step(cx, input),
            ScreenState::SaveDialog(s) => s.step(cx, input),
            ScreenState::Evolution(s) => s.step(cx, input),
            ScreenState::MainMenu(s) => s.step(cx, input),
            ScreenState::Naming(s) => s.step(cx, input),
            ScreenState::FlyMap(s) => s.step(cx, input),
            ScreenState::HallOfFame(s) => s.step(cx, input),
            ScreenState::EasyChat(s) => s.step(cx, input),
            ScreenState::Title(s) => s.step(cx, input),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        match self {
            ScreenState::StartMenu(s) => s.hash(h),
            ScreenState::Party(s) => s.hash(h),
            ScreenState::Summary(s) => s.hash(h),
            ScreenState::Bag(s) => s.hash(h),
            ScreenState::Mart(s) => s.hash(h),
            ScreenState::Pc(s) => s.hash(h),
            ScreenState::Options(s) => s.hash(h),
            ScreenState::SaveDialog(s) => s.hash(h),
            ScreenState::Evolution(s) => s.hash(h),
            ScreenState::MainMenu(s) => s.hash(h),
            ScreenState::Naming(s) => s.hash(h),
            ScreenState::FlyMap(s) => s.hash(h),
            ScreenState::HallOfFame(s) => s.hash(h),
            ScreenState::EasyChat(s) => s.hash(h),
            ScreenState::Title(s) => s.hash(h),
        }
    }
}

/// The open screens, bottom first (at most [`MAX_DEPTH`]), with the results waiting for the
/// top screen and the hosting state (`world/screens.rs`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ScreenStack {
    screens: Vec<(ScreenKind, ScreenState)>,
    /// The World's answer to the last [`ScreenOut::Save`], given to the top screen once
    /// ([`ScreenCx::saved`]) on the next frame. `World::save_failed` turns it to `false`.
    pub saved: Option<bool>,
    /// The result of the screen that just closed, given once to the one below it
    /// ([`ScreenCx::child`]).
    pub child: Option<ScreenResult>,
    /// The hosting state (fades, VRAM handover, the waiting request; Task K's [`kit::Hosting`]).
    pub host: kit::Hosting,
}

impl ScreenStack {
    pub fn new() -> Self {
        ScreenStack {
            screens: Vec::with_capacity(MAX_DEPTH),
            ..ScreenStack::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    pub fn len(&self) -> usize {
        self.screens.len()
    }

    /// Opens `kind` on top; `false` (nothing opened) when [`MAX_DEPTH`] screens are open.
    pub fn push(&mut self, kind: ScreenKind) -> bool {
        if self.screens.len() >= MAX_DEPTH {
            return false;
        }
        self.screens.push((kind, ScreenState::new(kind)));
        true
    }

    /// Closes the top screen.
    pub fn pop(&mut self) -> Option<ScreenKind> {
        self.screens.pop().map(|(k, _)| k)
    }

    /// The top screen.
    pub fn top(&self) -> Option<&(ScreenKind, ScreenState)> {
        self.screens.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut ScreenState> {
        self.screens.last_mut().map(|(_, s)| s)
    }

    /// The open kinds, bottom first.
    pub fn kinds(&self) -> impl Iterator<Item = ScreenKind> + '_ {
        self.screens.iter().map(|(k, _)| *k)
    }

    /// Feeds the stack into `h`: nothing when empty.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        if self.screens.is_empty() {
            return;
        }
        h.write(&[0xA6, self.screens.len() as u8]);
        for (k, s) in &self.screens {
            k.hash(h);
            s.hash(h);
        }
        h.write(&[self.saved.map_or(0xFF, u8::from)]);
        match self.child {
            None => h.write(&[0xFF]),
            Some(ScreenResult::Cancel) => h.write(&[0]),
            Some(ScreenResult::Value(v)) => {
                h.write(&[1]);
                h.write_u16(v);
            }
            Some(ScreenResult::Bool(b)) => h.write(&[2, b as u8]),
        }
        self.host.hash(h);
    }
}

/// The body every Wave 0 stub screen runs: `Report::Unsupported(name)` and `Done(Cancel)` on
/// its first frame.
pub(crate) fn stub_step<G: GameData>(cx: &mut ScreenCx<'_, G>, name: &'static str) -> ScreenOut {
    cx.report(Report::Unsupported(name));
    ScreenOut::Done(ScreenResult::Cancel)
}
