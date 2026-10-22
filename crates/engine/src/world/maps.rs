//! Map changes (owned by Milestone 8 Task F): warp loads ([`load_map`]), connection crossings
//! ([`cross_connection`]), warp starts and the warp transition, and the talk target a script
//! starts with.

use super::{Event, State, WALK, World, camera_for, run_immediate};
use crate::data::{Dir, GameData, MapData, MapId, PLAYER, SpecialIds};
use crate::flow::{TransitionStep, on_warp_into};
use crate::objects::Terrain;
use crate::script::Runner;
use crate::video::Frame;

/// The talk target of a starting script. `talked` = (local id, player facing, whether the
/// object belongs to the current map) for object interactions.
pub(super) fn set_talk_target(s: &mut State, ids: &SpecialIds, talked: Option<(u8, Dir, bool)>) {
    // An NPC left over from the previous map is talked to, but not addressable by local id
    // (`selected` names a current-map object), so `faceplayer` / `lock` skip it.
    s.selected = talked.filter(|t| t.2).map(|t| t.0);
    s.set_var_opt(ids.var_last_talked, talked.map_or(0, |t| t.0 as u16));
    if let Some((_, dir, _)) = talked {
        s.set_var_opt(ids.var_facing, dir as u16 + 1);
    }
}

impl World {
    /// Starts a warp through `map.warps[i]`. A destination map the game lacks is Gen3's
    /// `MAP_DYNAMIC` (the only warp target that is not a map): the warp goes to the dynamic
    /// slot (`SetWarpDestinationToDynamicWarp`), and is logged and ignored while that is unset
    /// or itself missing.
    pub(super) fn start_warp(&mut self, game: &impl GameData, map: &MapData, i: usize, door: bool) {
        let w = &map.warps[i];
        let (dest, warp, x, y) = match self.s.warp_slots.dynamic {
            _ if game.has_map(w.dest_map) => (w.dest_map, Some(w.dest_warp), -1, -1),
            Some(d) if game.has_map(d.map) => (d.map, d.warp, d.x, d.y),
            _ => {
                self.s.log("warp to a missing map ignored");
                return;
            }
        };
        let s = &mut self.s;
        if door {
            s.transition.start_door(game, dest, warp, x, y);
        } else {
            s.transition.start(game, dest, warp, x, y);
        }
        s.objects.freeze_all();
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
                load_map_spread(game, s, f, map, x, y, true);
                let b = s.ground(game).behavior(x, y);
                let facing = s.transition.arrive(b);
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
    s.templates = map.objects.to_vec();
    s.objects.enter_map(id, None);
    s.objects.set_pos(PLAYER, x, y);
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
    s.templates = map.objects.to_vec();
    s.objects.enter_map(to, Some(shift));
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
