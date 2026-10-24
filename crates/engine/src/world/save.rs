//! The in-game save (Gen3 `SaveGame` → `TrySavingData`; Milestone 9 Task S owns the file).
//!
//! [`save_now`] is Gen3's `TrySavingData` on a copy of the world: what CONTINUE would bring
//! back. A capturable world saves as its checkpoint (`World::checkpoint`, unchanged). Any
//! other field world (inside a script: `special SaveGame`; the start menu's save dialog open)
//! is taken without the capture checks (`World::snapshot_unchecked`) and normalised to "field
//! idle", as Gen3's CONTINUE rebuilds it from the save block:
//! - the runner is dropped (only its `issued` count stays) and its pending answer with it;
//!   the text box is closed and the screens are gone (the restore starts in `Mode::Field`);
//! - fades are cleared (level 0, Gen3 reloads the palettes unfaded) and screen effects too;
//! - every object is unfrozen (Gen3 `ScriptUnfreezeObjectEvents`; a CONTINUE respawns them
//!   free), and an object with a movement script (or a trainer approach, an emote) ends it on
//!   its `currentCoords` cell (Gen3 saves the destination cell of a step in progress): its
//!   action is dropped and its previous cell is its cell;
//! - every flag and var is kept, temp vars included (Gen3 saves `VAR_TEMP_*` and clears them on
//!   the next map load).
//!
//! The copy is then restored (`World::restore`) and must be capturable as it is: the header's
//! hash is the restored world's, so a restore of the save is checked like any checkpoint.
//! Battles, the starter screen and warps in progress cannot be saved
//! ([`SaveError::NotCapturable`]).
//!
//! Gen3 `gDifferentSaveFile` and `gSaveFileStatus` are [`SaveStatus`]: a new game is a
//! "different file" until its first save; the shells say whether flash holds a save
//! ([`World::set_save_present`]). They decide which question the dialog asks (`ui/save.rs`).
//!
//! `GameClear` (`hall_of_fame.c` `Task_Hof_TrySaveData`): Gen3 saves at the start of the Hall
//! of Fame with the CONTINUE warp set to the player's room (`SetContinueGameWarpToHealLocation`).
//! The engine's save is the world itself, so the Hall of Fame request only marks the save
//! [`SaveStatus::pending`]: the first capturable frame afterwards (the player in their room,
//! after the credits' warp) raises `Event::Saved` as the dialog does, and the shells persist
//! that world: what Gen3's CONTINUE shows.

use super::{Event, Mode, State, World};
use crate::checkpoint::{
    BuildHash, Checkpoint, FaderSnap, Header, MAX_ENCODED, NotCapturable, SCHEMA, SaveError,
    build_hash, encoded_len,
};
use crate::data::GameData;
use crate::hash::Fnv64;

/// Gen3 `gSaveFileStatus` / `gDifferentSaveFile` and the Hall of Fame's pending save. Only
/// [`SaveStatus::pending`] is hashed (and only while set): the rest describes the flash, which
/// the shells own, and only picks the dialog's question.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct SaveStatus {
    /// Flash holds a save (Gen3 `gSaveFileStatus == SAVE_STATUS_OK`).
    pub(crate) present: bool,
    /// A new game not saved yet (Gen3 `gDifferentSaveFile`, set by `NewGameInitData`).
    pub(crate) different: bool,
    /// The Hall of Fame asked for a save: raised on the first capturable frame.
    pub(crate) pending: bool,
}

impl SaveStatus {
    /// A new game's (`NewGameInitData`): a different file, no save known.
    pub(crate) const fn new_game() -> Self {
        SaveStatus {
            present: false,
            different: true,
            pending: false,
        }
    }

    /// A restored save's (CONTINUE): the file in flash.
    pub(crate) const fn continued() -> Self {
        SaveStatus {
            present: true,
            different: false,
            pending: false,
        }
    }

    /// The dialog's question after "Would you like to save the game?" yes
    /// (`SaveConfirmInputCallback`).
    pub(crate) fn question(&self) -> crate::ui::SaveQuestion {
        use crate::ui::SaveQuestion;
        match (self.present, self.different) {
            // `SAVE_STATUS_EMPTY` and a new game: straight to "SAVING…".
            (false, true) => SaveQuestion::None,
            // A different file in flash: the warning, default no.
            (true, true) => SaveQuestion::Different,
            // `SaveFileExistsCallback` with `gDifferentSaveFile == FALSE`.
            (_, false) => SaveQuestion::Overwrite,
        }
    }

    pub(crate) fn hash(&self, h: &mut Fnv64) {
        if self.pending {
            h.write(&[0x5A]);
        }
    }
}

/// Gen3 `TrySavingData` on a normalised copy of the world (see the module docs). It takes the
/// World, not only its `State`, because a capture needs the frame and the runner too.
pub(super) fn save_now(w: &World, game: &impl GameData) -> Result<Checkpoint, SaveError> {
    let cp = match w.capturable() {
        Ok(()) => w.checkpoint(game).map_err(SaveError::NotCapturable)?,
        Err(_) => normalised(w, game)?,
    };
    let n = encoded_len(&cp);
    if n > MAX_ENCODED {
        return Err(SaveError::TooLarge(n));
    }
    Ok(cp)
}

/// The world as CONTINUE would bring it back, captured.
fn normalised(w: &World, game: &impl GameData) -> Result<Checkpoint, SaveError> {
    let s = &w.s;
    match s.mode {
        Mode::Battle(_) => return Err(SaveError::NotCapturable(NotCapturable::Battle)),
        Mode::Starter(_) => return Err(SaveError::NotCapturable(NotCapturable::Starter)),
        Mode::Field | Mode::Screens(_) => {}
    }
    if s.transition.is_active() {
        return Err(SaveError::NotCapturable(NotCapturable::Transition));
    }
    let scripted = s.objects.scripted_ids();
    let mut snap = w.snapshot_unchecked(game);
    snap.fader = FaderSnap {
        level: 0,
        target: 0,
    };
    for o in &mut snap.objects.objs {
        o.frozen = false;
        if scripted.contains(&o.local_id) {
            o.action = None;
            o.prev = o.pos;
        }
    }
    if scripted.contains(&crate::data::PLAYER) {
        use crate::checkpoint::RunStateSnap as R;
        snap.objects.run_state = match snap.objects.run_state {
            R::Moving | R::TurnDirection => R::NotMoving,
            R::MovingCentered | R::TurnDirectionCentered => R::NotMovingCentered,
            R::TurnDirectionHeld => R::NotMovingHeld,
            r => r,
        };
    }
    // Restored under another build, the header's hash is not checked: the restored world's
    // own hash becomes it.
    let mut cp = Checkpoint {
        header: Header {
            schema: SCHEMA,
            build: BuildHash {
                engine: !0,
                content: !0,
            },
            frame: snap.frame,
            world_hash: 0,
        },
        world: snap,
    };
    let back =
        World::restore(game, &cp).map_err(|_| SaveError::NotCapturable(NotCapturable::Script))?;
    back.capturable().map_err(SaveError::NotCapturable)?;
    cp.header.build = build_hash(game);
    cp.header.world_hash = back.hash();
    Ok(cp)
}

/// The shell could not write the save (Gen3's "Save error" path, `SaveErrorTimer`): the
/// dialog answers "not saved" on the next frame.
pub(super) fn save_failed(s: &mut State) {
    if let Mode::Screens(stack) = &mut s.mode
        && stack.saved.is_some()
    {
        stack.saved = Some(false);
    }
}

/// The save dialog's save (`SaveDoSaveCallback`): whether it worked; later saves ask to
/// overwrite it.
pub(super) fn dialog_save(w: &mut World, game: &impl GameData) -> bool {
    let ok = save_now(w, game).is_ok();
    if ok {
        w.s.save.present = true;
        w.s.save.different = false;
    }
    ok
}

/// After a step: the Hall of Fame's save, once the world can be captured.
pub(super) fn after_step(w: &mut World, game: &impl GameData) {
    if !w.s.save.pending || w.capturable().is_err() || !w.s.objects.player().is_settled() {
        return;
    }
    w.s.save.pending = false;
    let ok = dialog_save(w, game);
    w.s.event(Event::Saved { ok });
}

/// The main menu's answer (`World::main_menu_choice`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainMenuChoice {
    /// CONTINUE: the shell restores the save (`World::restore`).
    Continue,
    /// NEW GAME: the shell starts `World::new` (the save stays until the first save
    /// overwrites it, as Gen3).
    NewGame,
}

impl World {
    /// The boot main menu over a valid save (Gen3 `CB2_InitMainMenu` with `HAS_SAVED_GAME`;
    /// `ui/main_menu.rs`): a new game's World with the menu open on a black screen. The shells
    /// step it until [`World::main_menu_choice`] answers, then swap it for the chosen World.
    pub fn main_menu(game: &impl GameData, meta: crate::checkpoint::SaveMeta) -> World {
        let mut w = World::new(game);
        w.s.save.present = true;
        let mut f = crate::video::Frame::default();
        super::screens::open(
            &mut w.s,
            game,
            crate::ui::ScreenKind::MainMenu,
            true,
            &mut f,
        );
        if let Mode::Screens(stack) = &mut w.s.mode {
            // Already black (the boot frame): the menu enters on the first step.
            stack.host.level = crate::ui::kit::FADE_FRAMES;
            if let Some(crate::ui::ScreenState::MainMenu(m)) = stack.top_mut() {
                m.set_meta(meta);
            }
        }
        w
    }

    /// The main menu's choice once it has closed ([`World::main_menu`]); `None` before.
    pub fn main_menu_choice(&self) -> Option<MainMenuChoice> {
        if !matches!(self.s.mode, Mode::Field) {
            return None;
        }
        match self.s.answer? {
            v if v == crate::ui::main_menu::CONTINUE => Some(MainMenuChoice::Continue),
            v if v == crate::ui::main_menu::NEW_GAME => Some(MainMenuChoice::NewGame),
            _ => None,
        }
    }

    /// Whether flash holds a save (Gen3 `gSaveFileStatus`): the shells say so after
    /// `World::new` (a restored world assumes it). It picks the save dialog's question.
    pub fn set_save_present(&mut self, present: bool) {
        self.s.save.present = present;
    }
}
