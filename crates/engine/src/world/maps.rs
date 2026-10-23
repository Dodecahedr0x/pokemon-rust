//! Map changes (owned by Milestone 8 Task F): warp loads ([`load_map`]), connection crossings
//! ([`cross_connection`]), warp starts and the warp transition, and the talk target a script
//! starts with.

use super::{Event, State, WALK, World, camera_for, run_immediate};
use crate::data::{Dir, GameData, MapData, MapId, PLAYER, SpecialIds};
use crate::flow::{TransitionStep, on_warp_into};
use crate::grid::MAP_DYNAMIC;
use crate::objects::Terrain;
use crate::script::Runner;
use crate::video::Frame;

/// The talk target of a starting script. `talked` = (local id, player facing, the object's
/// map) for object interactions.
pub(super) fn set_talk_target(s: &mut State, ids: &SpecialIds, talked: Option<(u8, Dir, MapId)>) {
    // Gen3 `GetObjectEventIdByPosition` does not filter by map: an NPC left over from the
    // previous map after a crossing is selected too, so `faceplayer` / `lock` act on it
    // (`gSelectedObjectEvent` is a slot). `VAR_LAST_TALKED` gets its local id, which scripts'
    // `applymovement` resolve on the current map, as Gen3.
    s.selected = talked.map(|t| (t.0, t.2));
    s.set_var_opt(ids.var_last_talked, talked.map_or(0, |t| t.0 as u16));
    if let Some((_, dir, _)) = talked {
        s.set_var_opt(ids.var_facing, dir as u16 + 1);
    }
}

impl World {
    /// Starts a warp through `map.warps[i]` (Gen3 `SetupWarp`). A warp to [`MAP_DYNAMIC`] goes
    /// to the dynamic slot (`SetWarpDestinationToDynamicWarp`), and is logged and ignored while
    /// that is unset or names a map the game lacks; a warp to any other map the game lacks is
    /// logged and ignored. Taking a warp whose destination warp is itself dynamic stores the
    /// warp taken in the dynamic slot (`SetDynamicWarp`: the way back out of the truck, the
    /// department store elevator, secret bases). Deferred: `UpdateEscapeWarp` (needs the map
    /// type, which `MapData` does not carry; the escape slot only serves Escape Rope and Dig,
    /// which the engine lacks).
    pub(super) fn start_warp(&mut self, game: &impl GameData, map: &MapData, i: usize, door: bool) {
        let w = &map.warps[i];
        let dynamic = w.dest_map == MAP_DYNAMIC;
        let (dest, warp, x, y) = match self.s.warp_slots.dynamic {
            Some(d) if dynamic && game.has_map(d.map) => (d.map, d.warp, d.x, d.y),
            _ if !dynamic && game.has_map(w.dest_map) => (w.dest_map, Some(w.dest_warp), -1, -1),
            _ => {
                self.s.log("warp to a missing map ignored");
                return;
            }
        };
        let s = &mut self.s;
        // `MAP_DYNAMIC` names no map: only a static destination is looked up.
        if !dynamic
            && (game.map(w.dest_map).warps.get(w.dest_warp as usize))
                .is_some_and(|b| b.dest_map == MAP_DYNAMIC)
        {
            let p = s.objects.player();
            s.warp_slots.dynamic = Some(super::WarpDest {
                map: s.field.map(),
                warp: Some(i as u8),
                x: p.x,
                y: p.y,
            });
        }
        if door {
            s.transition.start_door(game, dest, warp, x, y);
        } else {
            s.transition.start(game, dest, warp, x, y);
        }
        s.objects.freeze_all();
    }

    /// A debug warp, outside the game's rules (the GBA map tour, `game-gba --features tour`,
    /// and its native mirror in `emu/tests/tour.rs`): ends any running script and starts the
    /// warp transition to `map`, onto its warp `warp` if it has one, else onto (x, y) (as
    /// [`crate::flow::Transition::start`]). Refused (false) during a battle or the starter
    /// screen, while a transition runs, or for a map the game lacks. Call it between steps.
    pub fn debug_warp(
        &mut self,
        game: &impl GameData,
        map: MapId,
        warp: Option<u8>,
        x: i16,
        y: i16,
    ) -> bool {
        if !self.field_active() || self.s.transition.is_active() || !game.has_map(map) {
            return false;
        }
        self.runner = Runner::new();
        self.s.selected = None;
        self.s.transition.start(game, map, warp, x, y);
        self.s.objects.freeze_all();
        true
    }

    pub(super) fn run_transition(&mut self, game: &impl GameData, f: &mut Frame) {
        let s = &mut self.s;
        let moving = !s.objects.moves_done(PLAYER) || !s.objects.player().is_settled();
        let loading = s.field.uploading();
        match s.transition.update_loading(&mut s.fader, moving, loading) {
            TransitionStep::Walk(d) => s.objects.start_moves(PLAYER, &WALK[d as usize]),
            TransitionStep::Load(map, x, y) => {
                // Gen3 `CB2_LoadMap` restarts the script context: a script that warped ends
                // here, and the dialogue window goes with the old map.
                self.runner = Runner::new();
                s.selected = None;
                s.last_mover = 0;
                if s.text.is_open() {
                    s.text.close(f);
                }
                // The facing the player warped with (kept on a ladder). Gen3 stores it when a
                // step or push warp starts; a scripted warp reuses it too.
                let warped_facing = s.objects.player().facing;
                load_map_spread(game, s, f, map, x, y, true);
                let b = s.ground(game).behavior(x, y);
                let facing = s.transition.arrive(b, warped_facing);
                s.objects.face(PLAYER, facing);
                s.objects.freeze_all();
            }
            TransitionStep::Done => s.objects.unfreeze_all(),
            TransitionStep::Walking
            | TransitionStep::FadingOut
            | TransitionStep::Loading
            | TransitionStep::FadingIn => {}
        }
        if s.transition.player_hidden() {
            s.objects.hide(PLAYER);
        } else {
            s.objects.show(PLAYER);
        }
    }
}

/// Loads `id` with the player on (x, y) (Gen3 warp load). Drawing happens at the frame's
/// camera update.
pub(super) fn load_map(
    game: &impl GameData,
    s: &mut State,
    f: &mut Frame,
    id: MapId,
    x: i16,
    y: i16,
) {
    load_map_spread(game, s, f, id, x, y, false);
}

/// [`load_map`]; with `spread`, the field's tilesets and view are uploaded over the next
/// frames ([`crate::field::Field::enter_spread`]) while the screen stays black, instead of
/// all in this frame (Milestone 8 Task F).
fn load_map_spread(
    game: &impl GameData,
    s: &mut State,
    f: &mut Frame,
    id: MapId,
    x: i16,
    y: i16,
    spread: bool,
) {
    f.bulk = true;
    let map = game.map(id);
    s.event(Event::MapEntered(id));
    s.clear_temp(&game.special_ids());
    s.encounter_immunity = 0; // `RestartWildEncounterImmunitySteps`
    s.templates = super::templates::Templates::new(map.objects.to_vec());
    s.objects.enter_map(id, None);
    s.objects.set_pos(PLAYER, x, y);
    // Gen3 `LoadCurrentMapData`: the map's own layout, unless `on_transition` sets another.
    s.field.set_next_layout(None);
    // Gen3 `LoadMapFromWarp`: `RunOnTransitionMapScript` runs before `InitMap`.
    if let Some(script) = map.scripts.on_transition {
        run_immediate(game, s, f, script);
    }
    let camera = camera_for((x as i32 * 16, y as i32 * 16));
    if spread {
        s.field.enter_spread(id, camera);
    } else {
        s.field.enter(game, id, camera, f);
    }
    // Gen3 `InitPlayerAvatar` makes a new player object (elevations 0) and its first
    // `ObjectEventUpdateElevation` takes the arrival cell's: `previousElevation` is that one
    // unless it is 0 or 15.
    let e = crate::data::block_elevation(s.field.block(game, x, y));
    s.objects
        .reset_player_position_elevation(if e == 15 { 0 } else { e });
    for script in [map.scripts.on_load, map.scripts.on_resume]
        .into_iter()
        .flatten()
    {
        run_immediate(game, s, f, script);
    }
    s.spawn_templates();
    if let Some(script) = on_warp_into(map, &s.vars) {
        run_immediate(game, s, f, script);
    }
    // Gen3 `InitPlayerAvatar` sets `PLAYER_AVATAR_FLAG_CONTROLLABLE`: no forced movement until
    // the player first moves. It only matters on a forced-movement cell (`Objects::hold`).
    let on_forced = s.ground(game).behavior(x, y).is_forced_tile();
    s.objects.hold_forced(on_forced);
}

/// Switches to connected map `to` as the player steps across the edge (Gen3
/// `LoadMapFromCameraTransition`).
pub(super) fn cross_connection(
    game: &impl GameData,
    s: &mut State,
    f: &mut Frame,
    to: MapId,
    shift: (i16, i16),
) {
    let map = game.map(to);
    s.event(Event::MapEntered(to));
    s.clear_temp(&game.special_ids());
    s.encounter_immunity = 0; // `RestartWildEncounterImmunitySteps`
    s.templates = super::templates::Templates::new(map.objects.to_vec());
    s.objects.enter_map(to, Some(shift));
    s.field.set_next_layout(None);
    // As a warp load: `on_transition` before the map switch (`InitMap`).
    if let Some(script) = map.scripts.on_transition {
        run_immediate(game, s, f, script);
    }
    s.field.switch_map_enter(game, to, shift, f);
    for script in [map.scripts.on_load, map.scripts.on_resume]
        .into_iter()
        .flatten()
    {
        run_immediate(game, s, f, script);
    }
    s.spawn_templates();
}
