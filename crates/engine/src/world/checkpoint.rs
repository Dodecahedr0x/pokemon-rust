//! Capture and restore of a [`World`] as a [`Checkpoint`] (Milestone 5 Task B).
//!
//! Capture needs an idle field (see [`World::capturable`]); everything else is reached by
//! replaying a trace. Restore rebuilds the semantic state, then builds a boot frame that
//! re-uploads every VRAM byte the state implies without mutating hashed state.

use super::{Log, Mode, State, World};
use crate::checkpoint::{
    Checkpoint, Header, NotCapturable, RestoreError, SCHEMA, TemplateDelta, WorldSnapshot,
    build_hash,
};
use crate::data::{GameData, Gfx, ObjectEvent, PLAYER};
use crate::field::Field;
use crate::flow::{Fader, Input, Transition, Vars};
use crate::mon::{PARTY_SIZE, Party};
use crate::objects::Objects;
use crate::rng::Rng;
use crate::script::Runner;
use crate::text::TextBox;
use crate::video::Frame;
use alloc::vec::Vec;

impl World {
    /// `Ok` when the world can be captured: field mode, no transition, no script or pending
    /// answer, a pristine text box, no fade, no object running a movement script, and no cell
    /// in view left from the map before a connection crossing (checked in that order).
    pub fn capturable(&self) -> Result<(), NotCapturable> {
        let s = &self.s;
        match s.mode {
            Mode::Field => {}
            Mode::Starter(_) => return Err(NotCapturable::Starter),
            Mode::Battle(_) => return Err(NotCapturable::Battle),
        }
        if s.transition.is_active() {
            return Err(NotCapturable::Transition);
        }
        if self.runner.is_running() || self.runner.active().is_some() || s.answer.is_some() {
            return Err(NotCapturable::Script);
        }
        if !s.text.is_pristine() {
            return Err(NotCapturable::TextBox);
        }
        if !s.fader.is_done() || s.fader.level() != 0 {
            return Err(NotCapturable::Fade);
        }
        if let Some(id) = s.objects.any_moves() {
            return Err(NotCapturable::Moving(id));
        }
        if s.field.view_stale() {
            return Err(NotCapturable::StaleView);
        }
        Ok(())
    }

    /// The semantic state, if [`World::capturable`]. `game` gives the current map's object
    /// templates, which the snapshot stores as deltas.
    pub fn snapshot(&self, game: &impl GameData) -> Result<WorldSnapshot, NotCapturable> {
        self.capturable()?;
        let s = &self.s;
        Ok(WorldSnapshot {
            frame: self.frame,
            rng: s.rng.state(),
            input: s.input.snapshot(),
            runner_issued: self.runner.issued(),
            player_name: s.player_name,
            player_id: s.player_id,
            field: s.field.snapshot(),
            objects: s.objects.snapshot(),
            templates: template_deltas(game.map(s.field.map()).objects, &s.templates),
            vars: s.vars.snapshot(),
            fader: s.fader.snapshot(),
            transition: s.transition.snapshot(),
            selected: s.selected,
            last_mover: s.last_mover,
            delay: s.delay,
            door: s.door,
            party: s.party.iter().copied().collect(),
            respawn: s.respawn,
            prev_behavior: s.prev_behavior,
            encounter_immunity: s.encounter_immunity,
        })
    }

    /// [`World::snapshot`] with its header (schema, build, frame, hash). The capture predicate
    /// runs first, so a refused capture costs no [`World::hash`] (the GBA retries a pending save
    /// after every step, and the play bin's rewind ring polls every due frame).
    pub fn checkpoint(&self, game: &impl GameData) -> Result<Checkpoint, NotCapturable> {
        let world = self.snapshot(game)?;
        Ok(Checkpoint {
            header: Header {
                schema: SCHEMA,
                build: build_hash(game),
                frame: self.frame,
                world_hash: self.hash(),
            },
            world,
        })
    }

    /// Rebuilds the state; `initial_frame()` then returns every upload the state implies
    /// (applied with the screen off, like `World::new`'s boot frame). Same build: also checks
    /// `hash() == header.world_hash` (`HashMismatch`).
    pub fn restore(game: &impl GameData, cp: &Checkpoint) -> Result<World, RestoreError> {
        if cp.header.schema != SCHEMA {
            return Err(RestoreError::Schema(cp.header.schema));
        }
        let w = &cp.world;
        if cp.header.frame != w.frame {
            return Err(RestoreError::Invalid(
                "header frame differs from the world's",
            ));
        }
        for map in [w.field.map, w.objects.map] {
            if !game.has_map(map) {
                return Err(RestoreError::UnknownMap(map));
            }
        }
        if w.objects.map != w.field.map {
            return Err(RestoreError::Invalid("objects and field on different maps"));
        }
        if w.objects.objs.first().is_none_or(|o| o.local_id != PLAYER) {
            return Err(RestoreError::NoPlayer);
        }
        let rng = Rng::from_state(w.rng).ok_or(RestoreError::BadRng)?;
        if w.party.len() > PARTY_SIZE {
            return Err(RestoreError::PartyTooLarge);
        }
        Field::check_snapshot(game, &w.field).map_err(RestoreError::Invalid)?;
        Objects::check_snapshot(&w.objects).map_err(RestoreError::Invalid)?;
        let mut party = Party::default();
        for &p in &w.party {
            let _ = party.push(p);
        }
        let templates =
            apply_deltas(game.map(w.field.map).objects, &w.templates).map_err(|local_id| {
                RestoreError::UnknownTemplate {
                    map: w.field.map,
                    local_id,
                }
            })?;
        check_objects_against_templates(w, &templates)?;
        let mut world = World {
            frame: w.frame,
            runner: Runner::restore_idle(w.runner_issued),
            s: State {
                rng,
                input: Input::from_snapshot(w.input),
                field: Field::from_snapshot(&w.field),
                objects: Objects::from_snapshot(&w.objects),
                text: TextBox::new(),
                vars: Vars::from_snapshot(&w.vars),
                fader: Fader::from_snapshot(w.fader),
                transition: Transition::from_snapshot(&w.transition),
                templates,
                selected: w.selected,
                last_mover: w.last_mover,
                delay: w.delay,
                door: w.door,
                log: Log::new(),
                party,
                respawn: w.respawn,
                player_id: w.player_id,
                prev_behavior: w.prev_behavior,
                encounter_immunity: w.encounter_immunity,
                mode: Mode::Field,
                answer: None,
                player_name: w.player_name,
            },
            boot: None,
        };
        world.boot = Some(world.restore_frame(game));
        if cp.header.build == build_hash(game) {
            let got = world.hash();
            if got != cp.header.world_hash {
                return Err(RestoreError::HashMismatch {
                    want: cp.header.world_hash,
                    got,
                });
            }
        }
        Ok(world)
    }

    /// The boot frame of a restored world: every upload its state implies, for a screen whose
    /// VRAM is unknown (a fresh PPU, or the GBA's blanked boot). Only render caches change
    /// (OBJ VRAM slots and palette banks). The text box is pristine: BG0 is cleared and
    /// disabled as `TextBox::close` leaves it, so the frame is right over any old picture.
    pub(in crate::world) fn restore_frame(&mut self, game: &impl GameData) -> Frame {
        let s = &mut self.s;
        let mut f = Frame {
            bulk: true,
            ..Frame::default()
        };
        TextBox::new().close(&mut f);
        s.field.upload_all(game, &mut f);
        s.objects.invalidate_vram();
        s.objects.draw(game, s.field.camera(), &mut f);
        f.brightness = s.fader.level();
        f.lighten = false;
        f
    }
}

/// The templates of this visit that differ from the map's (scripts only move them and change
/// their movement type), in map order.
fn template_deltas(map: &[ObjectEvent], live: &[ObjectEvent]) -> Vec<TemplateDelta> {
    debug_assert_eq!(map.len(), live.len(), "templates are the current map's");
    map.iter()
        .zip(live)
        .filter_map(|(m, t)| {
            let pos = ((m.x, m.y) != (t.x, t.y)).then_some((t.x, t.y));
            let movement = (m.movement != t.movement).then_some(t.movement);
            (pos.is_some() || movement.is_some()).then_some(TemplateDelta {
                local_id: t.local_id,
                pos,
                movement,
            })
        })
        .collect()
}

/// Every object of the current map comes from one of its templates (scripts spawn and remove
/// objects, they never make new ones), with the template's fixed graphics.
fn check_objects_against_templates(
    w: &WorldSnapshot,
    templates: &[ObjectEvent],
) -> Result<(), RestoreError> {
    let map = w.field.map;
    for o in (w.objects.objs.iter()).filter(|o| o.map == map && o.local_id != PLAYER) {
        let t = (templates.iter())
            .find(|t| t.local_id == o.local_id)
            .ok_or(RestoreError::UnknownTemplate {
                map,
                local_id: o.local_id,
            })?;
        if let Gfx::Fixed(g) = t.gfx
            && o.gfx != Some(g)
        {
            return Err(RestoreError::Invalid(
                "object graphics differ from its template",
            ));
        }
    }
    Ok(())
}

/// The map's templates with `deltas` applied; `Err(local id)` for a delta naming no template.
fn apply_deltas(map: &[ObjectEvent], deltas: &[TemplateDelta]) -> Result<Vec<ObjectEvent>, u8> {
    let mut out = map.to_vec();
    for d in deltas {
        let t = (out.iter_mut())
            .find(|t| t.local_id == d.local_id)
            .ok_or(d.local_id)?;
        if let Some((x, y)) = d.pos {
            (t.x, t.y) = (x, y);
        }
        if let Some(m) = d.movement {
            t.movement = m;
        }
    }
    Ok(out)
}
