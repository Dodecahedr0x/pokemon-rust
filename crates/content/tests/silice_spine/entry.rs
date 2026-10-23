//! [`Run::at_chapter_entry`]: a run that starts at a chapter's entry state rather than at the new
//! game, for the chapters whose predecessors are not chained into the spine yet.
//!
//! The state is a test grant ([`Entry`]): the story step, the flags and vars the earlier chapters
//! leave, a party of the levels a player brings, a few items and some money, then a warp to the
//! chapter's first cell. It is applied by a script that replaces the new game's first scene
//! (`BricoBourg_Bar_Sc01`, the bar's on-frame entry), through a `GameData` wrapper of the spine's
//! [`Spy`], so the runner itself is unchanged. Everything after is buttons only.

use super::Run;
use super::driver::{Spy, script_id};
use engine::data::{
    Behavior, Emote, Evolution, FlagId, Font, FormChange, GameData, GfxId, ItemId, ItemInfo,
    MapData, MapId, NewGame, ScriptId, SpecialIds, SpeciesId, SpriteData, TextId, TrainerId,
    UiText, VarId, WindowGfx,
};
use engine::input::Buttons;
use engine::script::{Ctx, Report, Script};
use engine::world::Event;

/// A chapter's entry state.
pub struct Entry {
    /// Where the chapter starts (a warp to the cell, no warp id).
    pub map: MapId,
    pub x: i16,
    pub y: i16,
    /// The flags the earlier chapters set (badges, modules, the Mallette's).
    pub flags: &'static [FlagId],
    /// The vars they leave (`VAR_SIL_STORY` first).
    pub vars: &'static [(VarId, u16)],
    /// The party, lead first: species and level.
    pub party: &'static [(SpeciesId, u8)],
    /// Items and quantities (`ITEM_*`).
    pub items: &'static [(u16, u16)],
    pub money: u32,
    /// The respawn (`HEAL_LOCATION_*`).
    pub respawn: u8,
}

/// The script that replaces the new game's first scene: the grant, then the warp.
fn setup(e: &'static Entry, ctx: Ctx) -> Script {
    Box::pin(async move {
        for &f in e.flags {
            ctx.set_flag(f, true).await;
        }
        for &(v, n) in e.vars {
            ctx.set_var(v, n).await;
        }
        for &(species, level) in e.party {
            ctx.give_mon(species, level).await;
        }
        for &(item, qty) in e.items {
            ctx.item_add(ItemId(item), qty).await;
        }
        ctx.money_add(e.money).await;
        ctx.set_respawn(e.respawn).await;
        ctx.warp(e.map, None, e.x, e.y).await;
    })
}

/// The spine's [`Spy`] with the new game's first scene replaced by [`setup`].
struct Setup<'a> {
    spy: &'a Spy,
    entry: &'static Entry,
    first: ScriptId,
}

impl Run {
    /// A run at `entry`: a new game whose first scene grants the entry state and warps there;
    /// returns once the field is free on the entry map (its arrival scene, if any, still to
    /// settle; the fallbacks its load scripts report are kept). At most `limit` frames in all.
    pub fn at_chapter_entry(limit: u64, entry: &'static Entry) -> Run {
        let mut r = Run::new(limit);
        let first = script_id("BricoBourg_Bar_Sc01");
        {
            let Run {
                w,
                g,
                frame,
                degraded,
                ..
            } = &mut r;
            let game = Setup {
                spy: g,
                entry,
                first,
            };
            let mut n = 0;
            while w.map() != entry.map || w.transition_active() || !w.field_active() {
                w.step(&game, Buttons::NONE);
                *frame += 1;
                for e in w.events() {
                    if let Event::Report(Report::Degraded(name)) = e {
                        degraded.insert(*name);
                    }
                }
                n += 1;
                assert!(n < 2_000, "the entry warp to {:?} never landed", entry.map);
            }
        }
        r
    }
}

impl GameData for Setup<'_> {
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id == self.first {
            setup(self.entry, ctx)
        } else {
            self.spy.script(id, ctx)
        }
    }
    fn map(&self, id: MapId) -> &'static MapData {
        self.spy.map(id)
    }
    fn has_map(&self, id: MapId) -> bool {
        self.spy.has_map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        self.spy.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        self.spy.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        self.spy.player_sprite()
    }
    fn font(&self) -> &'static Font {
        self.spy.font()
    }
    fn window(&self) -> &'static WindowGfx {
        self.spy.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        self.spy.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        self.spy.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        self.spy.behavior(raw)
    }
    fn new_game(&self) -> NewGame {
        self.spy.new_game()
    }
    fn build_id(&self) -> u64 {
        self.spy.build_id()
    }
    fn battle(&self) -> &'static engine::mon::BattleTables {
        self.spy.battle()
    }
    fn wild_encounters(
        &self,
        map: MapId,
        wild_set: u16,
    ) -> Option<&'static engine::mon::LandEncounters> {
        self.spy.wild_encounters(map, wild_set)
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static engine::mon::TrainerData> {
        self.spy.trainer(id)
    }
    fn battle_text(&self, s: engine::battle::Str) -> &'static [u8] {
        self.spy.battle_text(s)
    }
    fn battle_gfx(&self) -> &'static engine::battle::BattleGfx {
        self.spy.battle_gfx()
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        self.spy.heal_location(id)
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        self.spy.item(id)
    }
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
        self.spy.multichoice(list)
    }
    fn std_string(&self, id: u8) -> Option<&'static [u8]> {
        self.spy.std_string(id)
    }
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        self.spy.trainer_flag_of(script)
    }
    fn trainer_battle_kind(&self, script: ScriptId) -> Option<u8> {
        self.spy.trainer_battle_kind(script)
    }
    fn emote(&self, e: Emote) -> Option<&'static SpriteData> {
        self.spy.emote(e)
    }
    fn trainer_money(&self, id: TrainerId) -> Option<u8> {
        self.spy.trainer_money(id)
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        self.spy.evolutions(species)
    }
    fn hold_effect(&self, id: ItemId) -> Option<engine::mon::HoldEffect> {
        self.spy.hold_effect(id)
    }
    fn form_changes(&self) -> &'static [FormChange] {
        self.spy.form_changes()
    }
}
