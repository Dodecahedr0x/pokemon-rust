//! Template spawning (owned by Milestone 8 Task F): what a warp load and a connection crossing
//! keep of the old map's objects, which templates spawn, `addobject`, and the view window
//! ([`Objects::view_moved`], [`Objects::despawn_outside_view`]).

use super::{Object, Objects};
use crate::data::{FlagId, MapData, MapId, ObjectEvent, PLAYER, VarId};
use crate::flow::Vars;

/// Gen3 `RemoveObjectEventIfOutsideView`: the kept area around the player's cell
/// (`pos - 2 ..= pos + 17` by `pos ..= pos + 16`, with the player at `pos + 7`).
const VIEW_X: (i16, i16) = (-9, 10);
const VIEW_Y: (i16, i16) = (-7, 9);

impl Objects {
    /// Enters map `id`: a warp load (`shift` `None`) keeps only the player; a connection crossing
    /// shifts every object by `shift` and keeps the old map's objects still in view. The current
    /// map's templates spawn afterwards ([`Objects::spawn_templates`]), once map scripts ran.
    pub(crate) fn enter_map(&mut self, id: MapId, shift: Option<(i16, i16)>) {
        match shift {
            None => self.spawn_events(id, &[], |_| true, |_| 0),
            Some(shift) => self.enter_connected_events(id, &[], shift, |_| true, |_| 0),
        }
    }

    /// Spawns the templates whose hide flag is clear (Gen3 `TrySpawnObjectEvents`).
    pub(crate) fn spawn_templates(&mut self, templates: &[ObjectEvent], vars: &Vars) {
        for ev in templates {
            if !ev.hide_flag.is_some_and(|f| vars.flag(f)) {
                self.add(ev, |v| vars.var(v));
            }
        }
    }

    /// The view moved: called by `World::control` once per completed player step (before its
    /// step-end triggers) and once per connection crossing (after the new map's objects
    /// spawned). Wave 0: no-op; Task F spawns and despawns templates by the view window here
    /// (Gen3 `TrySpawnObjectEvents`), so it never needs to edit `world.rs`.
    pub(crate) fn view_moved(&mut self, templates: &[ObjectEvent], vars: &Vars) {
        let _ = (templates, vars);
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
    /// from an earlier visit are replaced.
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
    /// `RemoveObjectEventIfOutsideView`).
    pub(super) fn despawn_outside_view(&mut self) {
        let (px, py) = self.objs[0].pos();
        let inside = |(x, y): (i16, i16)| {
            (px + VIEW_X.0..=px + VIEW_X.1).contains(&x)
                && (py + VIEW_Y.0..=py + VIEW_Y.1).contains(&y)
        };
        let map = self.map;
        self.objs
            .retain(|o| o.map == map || inside(o.pos()) || inside(o.home));
    }

    /// Spawns one object of the current map (Gen3 `addobject`), replacing any with the same
    /// local id.
    pub fn add(&mut self, ev: &ObjectEvent, vars: impl Fn(VarId) -> u16) {
        self.remove(ev.local_id);
        self.objs.push(Object::from_event(self.map, ev, &vars));
    }
}
