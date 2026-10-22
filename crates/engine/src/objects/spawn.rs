//! Template spawning (owned by Milestone 8 Task F): what a warp load and a connection crossing
//! keep of the objects, which templates spawn (Gen3 `TrySpawnObjectEvents`: those in the view
//! window, while object slots are free), `addobject`, and the view window
//! ([`Objects::view_moved`], [`Objects::despawn_outside_view`]).

use super::{Object, Objects};
use crate::data::{FlagId, MapData, MapId, ObjectEvent, PLAYER, VarId};
use crate::flow::Vars;

/// Gen3 `TrySpawnObjectEvents` / `RemoveObjectEventIfOutsideView`: the window around the
/// player's cell (`pos - 2 ..= pos + 17` by `pos ..= pos + 16` in Gen3's offset coordinates,
/// with the player at `pos + 7`) in which templates spawn and outside which objects despawn.
const VIEW_X: (i16, i16) = (-9, 10);
const VIEW_Y: (i16, i16) = (-7, 9);

/// Gen3 `OBJECT_EVENTS_COUNT`: object slots, the player's included.
pub const OBJECT_EVENTS_COUNT: usize = 16;

/// Whether `cell` is in the view window around `player`.
fn in_view(player: (i16, i16), (x, y): (i16, i16)) -> bool {
    (player.0 + VIEW_X.0..=player.0 + VIEW_X.1).contains(&x)
        && (player.1 + VIEW_Y.0..=player.1 + VIEW_Y.1).contains(&y)
}

impl Objects {
    /// Enters map `id`: a warp load (`shift` `None`) keeps only the player (Gen3
    /// `ResetObjectEvents`); a connection crossing shifts every object by `shift` (old map
    /// coordinates to the new map's) and keeps them all, the new map's objects left from an
    /// earlier visit included (Gen3 `LoadMapFromCameraTransition` removes none). The current
    /// map's templates spawn afterwards ([`Objects::spawn_templates`]), once map scripts ran.
    pub(crate) fn enter_map(&mut self, id: MapId, shift: Option<(i16, i16)>) {
        match shift {
            None => self.objs.truncate(1),
            Some((dx, dy)) => self.shift(dx, dy),
        }
        self.set_map(id);
    }

    /// Spawns the templates in the view window whose hide flag is clear and whose object does
    /// not exist yet, while object slots are free, then despawns the objects outside the window
    /// (Gen3 `TrySpawnObjectEvents`, then `RemoveObjectEventsOutsideView`).
    pub(crate) fn spawn_templates(&mut self, templates: &[ObjectEvent], vars: &Vars) {
        let player = self.objs[0].pos();
        let map = self.map;
        for ev in templates {
            if !in_view(player, (ev.x, ev.y))
                || ev.hide_flag.is_some_and(|f| vars.flag(f))
                || (self.objs.iter()).any(|o| o.local_id == ev.local_id && o.map == map)
            {
                continue;
            }
            if self.objs.len() >= OBJECT_EVENTS_COUNT {
                break;
            }
            self.objs
                .push(Object::from_event(map, ev, &|v| vars.var(v)));
        }
        self.remove_outside_view();
    }

    /// The view moved: called by `World::control` once per completed player step (before its
    /// step-end triggers) and once per connection crossing (after the new map's objects
    /// spawned). Spawns and despawns by the view window, as [`Objects::spawn_templates`] (Gen3
    /// `UpdateObjectEventsForCameraUpdate`).
    pub(crate) fn view_moved(&mut self, templates: &[ObjectEvent], vars: &Vars) {
        self.spawn_templates(templates, vars);
    }

    /// Removes every NPC whose cell and home are both outside the view window (Gen3
    /// `RemoveObjectEventsOutsideView`).
    fn remove_outside_view(&mut self) {
        let player = self.objs[0].pos();
        let mut i = 1;
        while i < self.objs.len() {
            let o = &self.objs[i];
            if in_view(player, o.pos()) || in_view(player, o.home) {
                i += 1;
            } else {
                self.objs.remove(i);
            }
        }
    }

    fn spawn(
        &mut self,
        id: MapId,
        events: &[ObjectEvent],
        flags: impl Fn(FlagId) -> bool,
        vars: impl Fn(VarId) -> u16,
    ) {
        for ev in events {
            if !ev.hide_flag.is_some_and(&flags) {
                self.objs.push(Object::from_event(id, ev, &vars));
            }
        }
    }

    /// Makes `id` the current map and replaces every NPC with `map`'s objects, skipping those
    /// whose `hide_flag` is set and resolving `Gfx::Var` through `vars`. The player is kept.
    /// Every template spawns, wherever the player is (tools; the World spawns by the view).
    pub fn spawn_map(
        &mut self,
        id: MapId,
        map: &MapData,
        flags: impl Fn(FlagId) -> bool,
        vars: impl Fn(VarId) -> u16,
    ) {
        self.spawn_events(id, map.objects, flags, vars);
    }

    /// [`Objects::spawn_map`] from a copy of the map's templates (edited by scripts for this
    /// visit, Gen3 `setobjectxyperm` / `setobjectmovementtype`).
    pub fn spawn_events(
        &mut self,
        id: MapId,
        events: &[ObjectEvent],
        flags: impl Fn(FlagId) -> bool,
        vars: impl Fn(VarId) -> u16,
    ) {
        self.objs.truncate(1);
        self.set_map(id);
        self.spawn(id, events, flags, vars);
    }

    /// Crossing into connected map `id`: shifts every object by `shift` cells (old map
    /// coordinates to the new map's), makes `id` current and spawns its objects alongside the
    /// old map's. Those stay, still moving, until they leave the view (checked here and on
    /// every `update`), as Gen3 `RemoveObjectEventsOutsideView`. Objects of `id` left over
    /// from an earlier visit are replaced. Every template spawns (tools; the World spawns by
    /// the view).
    pub fn enter_connected(
        &mut self,
        id: MapId,
        map: &MapData,
        shift: (i16, i16),
        flags: impl Fn(FlagId) -> bool,
        vars: impl Fn(VarId) -> u16,
    ) {
        self.enter_connected_events(id, map.objects, shift, flags, vars);
    }

    /// [`Objects::enter_connected`] from a copy of the map's templates.
    pub fn enter_connected_events(
        &mut self,
        id: MapId,
        events: &[ObjectEvent],
        shift: (i16, i16),
        flags: impl Fn(FlagId) -> bool,
        vars: impl Fn(VarId) -> u16,
    ) {
        self.shift(shift.0, shift.1);
        self.objs.retain(|o| o.local_id == PLAYER || o.map != id);
        self.set_map(id);
        self.spawn(id, events, flags, vars);
        self.despawn_outside_view();
    }

    /// Removes other maps' objects whose cell and home are both outside the view (Gen3
    /// `RemoveObjectEventIfOutsideView`; run every frame by `update`).
    pub(super) fn despawn_outside_view(&mut self) {
        let player = self.objs[0].pos();
        let map = self.map;
        self.objs
            .retain(|o| o.map == map || in_view(player, o.pos()) || in_view(player, o.home));
    }

    /// Spawns one object of the current map (Gen3 `addobject`), replacing any with the same
    /// local id, if an object slot is free ([`OBJECT_EVENTS_COUNT`]).
    pub fn add(&mut self, ev: &ObjectEvent, vars: impl Fn(VarId) -> u16) {
        self.remove(ev.local_id);
        if self.objs.len() < OBJECT_EVENTS_COUNT {
            self.objs.push(Object::from_event(self.map, ev, &vars));
        }
    }
}

#[cfg(test)]
mod tests;
