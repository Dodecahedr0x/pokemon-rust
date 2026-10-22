//! Panic capture: a game panic becomes a `Panicked` finding instead of killing the explorer.
//!
//! Relies on `panic = "unwind"` (no profile may set `abort`).

use engine::Buttons;
use engine::data::GameData;
use game_native::Session;
use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Panicked {
    pub message: String,
    /// `file:line:col` of the panic.
    pub location: Option<String>,
}

thread_local! {
    /// Guards active on this thread (nesting allowed).
    static ACTIVE: Cell<u32> = const { Cell::new(0) };
    /// The last panic caught under a guard on this thread.
    static CAUGHT: RefCell<Option<Panicked>> = const { RefCell::new(None) };
}

/// Installs (once) a panic hook that, while a guard is active on this thread, stores the
/// message and `Location` in a thread-local and prints nothing; otherwise defers to the
/// previous hook.
pub fn install_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if ACTIVE.with(Cell::get) == 0 {
                return previous(info);
            }
            let message = match info.payload_as_str() {
                Some(s) => s.to_owned(),
                None => "non-string panic payload".to_owned(),
            };
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
            CAUGHT.with(|c| *c.borrow_mut() = Some(Panicked { message, location }));
        }));
    });
}

/// Runs `f` under a guard: a panic inside it is caught, silently, as [`Panicked`].
pub fn catch<R>(f: impl FnOnce() -> R) -> Result<R, Panicked> {
    install_hook();
    CAUGHT.with(|c| c.borrow_mut().take());
    ACTIVE.with(|a| a.set(a.get() + 1));
    let r = panic::catch_unwind(AssertUnwindSafe(f));
    ACTIVE.with(|a| a.set(a.get() - 1));
    r.map_err(|payload| {
        CAUGHT
            .with(|c| c.borrow_mut().take())
            .unwrap_or_else(|| Panicked {
                // Only reachable if another hook replaced ours: fall back to the payload.
                message: payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "non-string panic payload".to_owned()),
                location: None,
            })
    })
}

/// `s.step(keys)` under `catch_unwind(AssertUnwindSafe(..))`. After `Err` the session must be
/// dropped.
pub fn step_guarded<G: GameData>(s: &mut Session<G>, keys: Buttons) -> Result<(), Panicked> {
    catch(|| s.step(keys))
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::Emerald;
    use engine::battle::{self, BattleGfx};
    use engine::data::{
        Behavior, Emote, Evolution, FlagId, Font, GfxId, ItemId, ItemInfo, MapData, MapId, NewGame,
        ScriptId, SpecialIds, SpeciesId, SpriteData, TextId, TrainerId, UiText, WindowGfx,
    };
    use engine::mon::{BattleTables, LandEncounters, TrainerData};
    use engine::script::{Ctx, Script};

    #[test]
    fn catches_message_and_location() {
        let line = line!() + 1;
        let e = catch(|| panic!("boom {}", 7)).unwrap_err();
        assert_eq!(e.message, "boom 7");
        let loc = e.location.unwrap();
        assert!(
            loc.contains("guard.rs") && loc.contains(&format!(":{line}:")),
            "{loc}"
        );
        assert_eq!(catch(|| 5), Ok(5));
    }

    #[test]
    fn nested_guards_and_unguarded_panics() {
        let r = catch(|| {
            let inner = catch(|| panic!("inner"));
            assert_eq!(inner.unwrap_err().message, "inner");
            panic!("outer")
        });
        assert_eq!(r.unwrap_err().message, "outer");
        assert_eq!(ACTIVE.with(Cell::get), 0);
        // Outside a guard the previous hook runs and nothing is recorded.
        let r = panic::catch_unwind(|| panic!("unguarded (expected in test output)"));
        assert!(r.is_err());
        assert!(CAUGHT.with(|c| c.borrow().is_none()));
    }

    /// Panics in `map` once armed (the engine reads the map every frame).
    struct Armed(Emerald, Cell<bool>);

    impl GameData for Armed {
        fn map(&self, id: MapId) -> &'static MapData {
            assert!(!self.1.get(), "armed map read");
            self.0.map(id)
        }
        fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
            self.0.sprite(gfx)
        }
        fn player_sprite(&self) -> &'static SpriteData {
            self.0.player_sprite()
        }
        fn font(&self) -> &'static Font {
            self.0.font()
        }
        fn window(&self) -> &'static WindowGfx {
            self.0.window()
        }
        fn text(&self, id: TextId) -> &'static [u8] {
            self.0.text(id)
        }
        fn ui_text(&self, t: UiText) -> &'static [u8] {
            self.0.ui_text(t)
        }
        fn behavior(&self, raw: u8) -> Behavior {
            self.0.behavior(raw)
        }
        fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
            self.0.script(id, ctx)
        }
        fn new_game(&self) -> NewGame {
            self.0.new_game()
        }
        fn has_map(&self, id: MapId) -> bool {
            self.0.has_map(id)
        }
        fn special_ids(&self) -> SpecialIds {
            self.0.special_ids()
        }
        fn build_id(&self) -> u64 {
            self.0.build_id()
        }
        fn battle(&self) -> &'static BattleTables {
            self.0.battle()
        }
        fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> {
            self.0.encounters(map)
        }
        fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
            self.0.trainer(id)
        }
        fn battle_text(&self, s: battle::Str) -> &'static [u8] {
            self.0.battle_text(s)
        }
        fn battle_gfx(&self) -> &'static BattleGfx {
            self.0.battle_gfx()
        }
        fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
            self.0.heal_location(id)
        }
        fn wild_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters> {
            self.0.wild_encounters(map, wild_set)
        }
        fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
            self.0.item(id)
        }
        fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
            self.0.multichoice(list)
        }
        fn std_string(&self, id: u8) -> Option<&'static [u8]> {
            self.0.std_string(id)
        }
        fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
            self.0.trainer_flag_of(script)
        }
        fn emote(&self, e: Emote) -> Option<&'static SpriteData> {
            self.0.emote(e)
        }
        fn trainer_money(&self, id: TrainerId) -> Option<u8> {
            self.0.trainer_money(id)
        }
        fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
            self.0.evolutions(species)
        }
    }

    #[test]
    fn armed_answers_the_milestone_8_methods_like_emerald() {
        let g = Emerald::for_shells();
        assert_eq!(
            crate::testing::same_answers(&Armed(g, Cell::new(false)), &g),
            Ok(())
        );
    }

    #[test]
    fn a_game_panic_while_stepping_is_caught() {
        let mut s = Session::new(Armed(Emerald::for_shells(), Cell::new(false)));
        for _ in 0..30 {
            step_guarded(&mut s, Buttons::NONE).unwrap();
        }
        s.game().1.set(true);
        let mut caught = None;
        for _ in 0..30 {
            if let Err(e) = step_guarded(&mut s, Buttons::DOWN) {
                caught = Some(e);
                break;
            }
        }
        let e = caught.expect("the armed game panics within 30 frames");
        assert_eq!(e.message, "armed map read");
        assert!(e.location.unwrap().contains("guard.rs"));
    }
}
