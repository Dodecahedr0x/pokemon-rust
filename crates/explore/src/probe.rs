//! `Probe<G>`: script coverage without engine events.

use engine::battle::{self, BattleGfx};
use engine::data::{
    Behavior, Font, GameData, GfxId, MapData, MapId, NewGame, ScriptId, SpecialIds, SpriteData,
    TextId, TrainerId, UiText, WindowGfx,
};
use engine::mon::{BattleTables, LandEncounters, TrainerData};
use engine::script::{Ctx, Script};
use std::cell::RefCell;

/// A GameData that records every script started (`GameData::script`) and otherwise delegates
/// every method to `G` (provided ones too, so `build_id`, `special_ids`… are G's): the game
/// behaves identically, with the same hashes and `build_hash`.
pub struct Probe<G> {
    inner: G,
    started: RefCell<Vec<ScriptId>>,
}

impl<G: GameData> Probe<G> {
    pub fn new(inner: G) -> Self {
        Probe {
            inner,
            started: RefCell::new(Vec::new()),
        }
    }

    pub fn inner(&self) -> &G {
        &self.inner
    }

    /// Scripts started since the last call, in order.
    pub fn take_started(&self) -> Vec<ScriptId> {
        std::mem::take(&mut *self.started.borrow_mut())
    }
}

impl<G: GameData> GameData for Probe<G> {
    fn map(&self, id: MapId) -> &'static MapData {
        self.inner.map(id)
    }
    fn has_map(&self, id: MapId) -> bool {
        self.inner.has_map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        self.inner.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        self.inner.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        self.inner.player_sprite()
    }
    fn font(&self) -> &'static Font {
        self.inner.font()
    }
    fn window(&self) -> &'static WindowGfx {
        self.inner.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        self.inner.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        self.inner.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        self.inner.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        self.started.borrow_mut().push(id);
        self.inner.script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        self.inner.new_game()
    }
    fn build_id(&self) -> u64 {
        self.inner.build_id()
    }
    fn battle(&self) -> &'static BattleTables {
        self.inner.battle()
    }
    fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> {
        self.inner.encounters(map)
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
        self.inner.trainer(id)
    }
    fn battle_text(&self, s: battle::Str) -> &'static [u8] {
        self.inner.battle_text(s)
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        self.inner.battle_gfx()
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        self.inner.heal_location(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::Emerald;
    use engine::checkpoint::build_hash;
    use game_native::Session;
    use game_native::scenario::cached_route;

    /// A game whose provided methods differ from the trait defaults, to prove `Probe` forwards
    /// them instead of falling back to the defaults.
    struct Custom(Emerald);

    impl GameData for Custom {
        fn map(&self, id: MapId) -> &'static MapData {
            self.0.map(id)
        }
        fn has_map(&self, id: MapId) -> bool {
            id.0 == 0xBEEF
        }
        fn special_ids(&self) -> SpecialIds {
            SpecialIds {
                respawn_default: 0xA5,
                ..self.0.special_ids()
            }
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
        fn build_id(&self) -> u64 {
            0x1234_5678
        }
        fn battle_text(&self, _: battle::Str) -> &'static [u8] {
            &[1, 2, 0xFF]
        }
        fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
            Some((MapId(id.into()), 3, 4))
        }
    }

    #[test]
    fn build_hash_is_the_inner_games() {
        let g = Emerald::for_shells();
        assert_eq!(build_hash(&Probe::new(g)), build_hash(&g));
        let p = Probe::new(Custom(g));
        assert_eq!(build_hash(&p), build_hash(p.inner()));
        assert_eq!(p.build_id(), 0x1234_5678);
    }

    #[test]
    fn provided_methods_are_forwarded() {
        let p = Probe::new(Custom(Emerald::for_shells()));
        assert!(p.has_map(MapId(0xBEEF)));
        assert!(!p.has_map(MapId(0)));
        assert_eq!(p.special_ids().respawn_default, 0xA5);
        assert_eq!(p.heal_location(9), Some((MapId(9), 3, 4)));
        assert_eq!(p.battle_text(battle::Str::Intro1), &[1, 2, 0xFF]);
    }

    #[test]
    fn replays_the_route_with_identical_hashes() {
        let rec = cached_route();
        let mut s = Session::new(Probe::new(Emerald::for_shells()));
        let mut started = Vec::new();
        for (i, &want) in rec.hashes.iter().enumerate() {
            s.step(rec.keys[i]);
            assert_eq!(s.world().hash(), want, "hash after step {}", i + 1);
            started.extend(s.game().take_started());
        }
        assert!(s.game().take_started().is_empty());
        if !content::STUB {
            assert!(!started.is_empty(), "the route starts scripts");
        }
    }
}
