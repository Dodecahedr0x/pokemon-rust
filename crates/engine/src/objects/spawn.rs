//! Template spawning on map entry (owned by Milestone 8 Task F): what a warp load and a
//! connection crossing keep of the old map's objects, and which templates spawn.

use super::Objects;
use crate::data::{MapId, ObjectEvent};
use crate::flow::Vars;

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
}
