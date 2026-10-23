//! Capture and restore of a [`World`] as a [`Checkpoint`] (Milestone 5 Task B).
//!
//! Capture needs an idle field (see [`World::capturable`]); everything else is reached by
//! replaying a trace. Restore rebuilds the semantic state, then builds a boot frame that
//! re-uploads every VRAM byte the state implies without mutating hashed state.

use super::{Log, Mode, State, World};
use super::{WarpDest, WarpSlots};
use crate::bag::Bag;
use crate::checkpoint::{
    Checkpoint, Header, NotCapturable, RestoreError, SCHEMA, TemplateDelta, WarpDestSnap,
    WarpSlotsSnap, WorldSnapshot, build_hash,
};
use crate::checkpoint::{PcItemSnap, PcMonSnap, PcSnap, PokedexSnap};
use crate::data::{GameData, Gfx, ObjectEvent, PLAYER};
use crate::field::Field;
use crate::flow::{Fader, Input, Transition, Vars};
use crate::fx::ScreenFx;
use crate::mon::{PARTY_SIZE, Party, Pokemon};
use crate::objects::Objects;
use crate::pc::{BOX_SLOTS, PcStorage};
use crate::pokedex::{DexFlag, Pokedex};
use crate::rng::Rng;
use crate::script::Runner;
use crate::str_vars::StrVars;
use crate::text::TextBox;
use crate::video::Frame;
use alloc::vec::Vec;

impl World {
    /// `Ok` when the world can be captured: field mode, no transition, no script or pending
    /// answer, a pristine text box, no fade, no object running a movement script, and no field
    /// upload spread over frames in progress (checked in that order).
    pub fn capturable(&self) -> Result<(), NotCapturable> {
        let s = &self.s;
        match s.mode {
            Mode::Field => {}
            Mode::Starter(_) => return Err(NotCapturable::Starter),
            Mode::Battle(_) => return Err(NotCapturable::Battle),
            Mode::Screens(_) => return Err(NotCapturable::Screen),
        }
        if s.transition.is_active() {
            return Err(NotCapturable::Transition);
        }
        // A field move's effect is the tail of the script that started it (Wave C).
        if self.runner.is_running()
            || self.runner.active().is_some()
            || s.answer.is_some()
            || s.field_move_busy()
        {
            return Err(NotCapturable::Script);
        }
        if !s.text.is_pristine() {
            return Err(NotCapturable::TextBox);
        }
        // Screen effects are not snapshotted (Milestone 9).
        if !s.fader.is_done() || s.fader.level() != 0 || s.fx.is_active() {
            return Err(NotCapturable::Fade);
        }
        if let Some(id) = s.objects.any_moves() {
            return Err(NotCapturable::Moving(id));
        }
        // Milestone 8 Task F: a crossing redraws the cells it keeps that the new map draws
        // differently, so the view is the current map's, except for the few frames a crossing
        // into other tilesets spreads its upload over (a restore would upload it whole).
        if s.field.uploading() {
            return Err(NotCapturable::StaleView);
        }
        Ok(())
    }

    /// The semantic state, if [`World::capturable`]. `game` gives the current map's object
    /// templates, which the snapshot stores as deltas.
    pub fn snapshot(&self, game: &impl GameData) -> Result<WorldSnapshot, NotCapturable> {
        self.capturable()?;
        Ok(self.snapshot_unchecked(game))
    }

    /// The state as [`World::snapshot`] writes it, without the capture checks (Milestone 9: the
    /// in-game save, `world/save.rs`, snapshots mid-script or with a screen open and normalises
    /// the DTO to field idle before it captures). What the DTO cannot hold is dropped: the
    /// runner's future (only its `issued` count stays), the mode, the text box, screen effects.
    pub(super) fn snapshot_unchecked(&self, game: &impl GameData) -> WorldSnapshot {
        let s = &self.s;
        WorldSnapshot {
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
            selected: s.selected.map(|t| t.0),
            last_mover: s.last_mover,
            delay: s.delay,
            door: s.door,
            party: s.party.iter().copied().collect(),
            respawn: s.respawn,
            prev_behavior: s.prev_behavior,
            encounter_immunity: s.encounter_immunity,
            bag: s.bag.snapshot(),
            money: s.money,
            str_vars: s.str_vars.snapshot(),
            warp_slots: WarpSlotsSnap {
                dynamic: s.warp_slots.dynamic.map(warp_dest_snap),
                escape: s.warp_slots.escape.map(warp_dest_snap),
            },
            party_held: if s.party.iter().any(|p| p.held.is_some()) {
                s.party.iter().map(|p| p.held).collect()
            } else {
                Vec::new()
            },
            options: (s.options != Default::default()).then_some(s.options),
            pokedex: PokedexSnap {
                seen: s.pokedex.raw(DexFlag::Seen).to_vec(),
                caught: s.pokedex.raw(DexFlag::Caught).to_vec(),
            },
            pc: pc_snap(&s.pc),
            coins: s.coins,
            registered_item: s.registered_item,
            berry_trees: (s.berry_trees.entries())
                .map(|(id, tree)| crate::checkpoint::BerryTreeSnap { id, tree })
                .collect(),
            map_layout: s.field.layout(),
        }
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
        if !crate::checkpoint::schema_readable(cp.header.schema) {
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
        Field::check_snapshot(game, &w.field, w.map_layout).map_err(RestoreError::Invalid)?;
        Objects::check_snapshot(&w.objects).map_err(RestoreError::Invalid)?;
        if !w.party_held.is_empty() && w.party_held.len() != w.party.len() {
            return Err(RestoreError::Invalid("party_held differs from the party"));
        }
        let mut party = Party::default();
        for (i, &p) in w.party.iter().enumerate() {
            let held = w.party_held.get(i).copied().flatten();
            let _ = party.push(Pokemon { held, ..p });
        }
        let templates =
            apply_deltas(game.map(w.field.map).objects, &w.templates).map_err(|local_id| {
                RestoreError::UnknownTemplate {
                    map: w.field.map,
                    local_id,
                }
            })?;
        check_objects_against_templates(w, &templates)?;
        let ids = game.special_ids();
        let bag = Bag::from_snapshot(&w.bag, ids.pocket_slots).map_err(RestoreError::Invalid)?;
        let str_vars = StrVars::from_snapshot(&w.str_vars).map_err(RestoreError::Invalid)?;
        if w.money > ids.max_money {
            return Err(RestoreError::Invalid("money over the game's max_money"));
        }
        let pokedex =
            Pokedex::from_raw(&w.pokedex.seen, &w.pokedex.caught).map_err(RestoreError::Invalid)?;
        let pc =
            pc_from_snap(&w.pc, ids.pc_item_slots, ids.pc_boxes).map_err(RestoreError::Invalid)?;
        if w.coins > ids.max_coins {
            return Err(RestoreError::Invalid("coins over the game's max_coins"));
        }
        let mut berry_trees = crate::berry::BerryTrees::new(ids.berry_trees);
        for t in &w.berry_trees {
            if t.id >= ids.berry_trees {
                return Err(RestoreError::Invalid(
                    "berry tree id over the game's berry_trees",
                ));
            }
            berry_trees.set(t.id, t.tree);
        }
        let mut world = World {
            frame: w.frame,
            runner: Runner::restore_idle(w.runner_issued),
            s: State {
                rng,
                input: Input::from_snapshot(w.input),
                field: Field::from_snapshot(&w.field, w.map_layout),
                objects: Objects::from_snapshot(&w.objects),
                text: TextBox::new(),
                vars: Vars::from_snapshot(&w.vars),
                fader: Fader::from_snapshot(w.fader),
                transition: Transition::from_snapshot(&w.transition),
                templates: super::templates::Templates::new(templates),
                selected: w.selected.map(|id| (id, w.field.map)),
                last_mover: w.last_mover,
                keep_templates: false,
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
                bag,
                money: w.money,
                str_vars,
                warp_slots: WarpSlots {
                    dynamic: w.warp_slots.dynamic.map(warp_dest),
                    escape: w.warp_slots.escape.map(warp_dest),
                },
                options: w.options.unwrap_or_default(),
                pokedex,
                pc,
                coins: w.coins,
                registered_item: w.registered_item,
                berry_trees,
                fx: ScreenFx::default(),
                field_effects: super::host::FieldEffects::default(),
                rotating_tiles: None,
                field_move: None,
                stash: super::host::Stash::default(),
                events: Vec::with_capacity(super::EVENTS_MAX),
                events_dropped: 0,
                last_script: None,
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

/// The PC's non-empty item and box slots, ascending.
fn pc_snap(pc: &PcStorage) -> PcSnap {
    let items = (pc.item_entries())
        .map(|(index, item, qty)| PcItemSnap { index, item, qty })
        .collect();
    let mut mons = Vec::new();
    for (b, slots) in pc.boxes().iter().enumerate() {
        for (slot, m) in slots.iter().enumerate() {
            if let Some(mon) = *m {
                mons.push(PcMonSnap {
                    pc_box: b as u8,
                    slot: slot as u8,
                    mon,
                });
            }
        }
    }
    PcSnap {
        items,
        mons,
        current_box: pc.current_box(),
    }
}

/// The PC of a snapshot: slots in range and strictly ascending (as [`pc_snap`] writes them),
/// quantities Gen3 allows.
fn pc_from_snap(p: &PcSnap, item_slots: u8, boxes: u8) -> Result<PcStorage, &'static str> {
    let mut pc = PcStorage::new(item_slots, boxes);
    let mut prev = None;
    for it in &p.items {
        if it.index >= item_slots || prev.is_some_and(|i| i >= it.index) {
            return Err("pc item slot out of range or out of order");
        }
        if !PcStorage::qty_valid(it.qty) {
            return Err("pc item quantity out of range");
        }
        prev = Some(it.index);
        pc.set_item(
            it.index as usize,
            crate::bag::Slot {
                item: it.item,
                qty: it.qty,
            },
        );
    }
    let mut prev = None;
    for m in &p.mons {
        let at = (m.pc_box, m.slot);
        if m.pc_box >= boxes || m.slot as usize >= BOX_SLOTS || prev.is_some_and(|p| p >= at) {
            return Err("pc box slot out of range or out of order");
        }
        prev = Some(at);
        pc.set_mon(m.pc_box as usize, m.slot as usize, Some(m.mon));
    }
    if p.current_box != 0 && p.current_box >= boxes {
        return Err("pc current box out of range");
    }
    pc.set_current_box(p.current_box);
    Ok(pc)
}

fn warp_dest_snap(d: WarpDest) -> WarpDestSnap {
    WarpDestSnap {
        map: d.map,
        warp: d.warp,
        x: d.x,
        y: d.y,
    }
}

fn warp_dest(d: WarpDestSnap) -> WarpDest {
    WarpDest {
        map: d.map,
        warp: d.warp,
        x: d.x,
        y: d.y,
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
