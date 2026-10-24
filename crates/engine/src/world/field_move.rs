//! Field moves on the field (Wave C, Milestone 10 pulled forward): the A press on water that
//! offers Surf, the field-move effects scripts start (`dofieldeffect FLDEFF_USE_SURF`,
//! `FLDEFF_USE_CUT_ON_TREE`, `FLDEFF_USE_ROCK_SMASH`, `FLDEFF_USE_STRENGTH`) and Strength's
//! boulders.
//!
//! **The effects** are timed as Gen3's tasks, frame by frame (sprites are not drawn: no pose
//! frame, no mon banner, no surf blob):
//!
//! | phase | frames | Gen3 |
//! |---|---|---|
//! | init | 1 | `SurfFieldEffect_Init`, `Task_DoFieldMove_Init`: controls locked |
//! | pose | [`POSE_FRAMES`] | `MOVEMENT_ACTION_START_ANIM_IN_DIRECTION` with `sAnim_FieldMove` (4+4+4+4+8) |
//! | show mon | [`SHOW_MON_OUTDOORS`] or [`SHOW_MON_INDOORS`] | `FLDEFF_FIELD_MOVE_SHOW_MON_INIT`, then the banner task (below) |
//! | Surf: jump | 16 | `SurfFieldEffect_JumpOnSurfBlob`: a jump special one cell on, onto the water |
//! | end | 1 | `SurfFieldEffect_End` (controls back), `Task_DoFieldMove_RunFunc` (`ScriptContext_Enable`) |
//!
//! The banner (`Task_FieldMoveShowMonOutdoors`): init 1, gfx 1, the window opening 20 (its
//! vertical edges move 2 px a frame from 80 to 40), the mon sliding on 10 (from x 320 by 20 a
//! frame to 120), on screen 30 (`sOnscreenTimer`), sliding off 11 (to x < -64), the window
//! closing 7, restore 1 and end 1: 82, plus the frame `WaitForMon` sees the mon gone. Indoors
//! (`Task_FieldMoveShowMonIndoors`) the banner slides on and off in 17 frames each: 89.
//!
//! Surf sets the player's surfing state at its first frame (`SetPlayerAvatarStateMask`) and
//! freezes every object until its end (`FreezeObjectEvents`); `EventScript_UseSurf` has ended
//! by then (no `waitstate`). The other three end with `ScriptContext_Enable`: the script's
//! `waitstate` waits for the effect ([`State::field_move_busy`]).

use super::State;
use crate::data::{Dir, FieldMove, FieldMoveGate, GameData, MapId, MapType, PLAYER};
use crate::hash::Fnv64;
use crate::objects::Collision;

/// `sAnim_FieldMove` (24 frames) plus the movement action's first frame and the frame
/// `ObjectEventCheckHeldMovementStatus` sees it done.
pub const POSE_FRAMES: u16 = 26;
/// `FLDEFF_FIELD_MOVE_SHOW_MON_INIT`'s frame plus the outdoor banner (module doc).
pub const SHOW_MON_OUTDOORS: u16 = 1 + 83;
/// The same with the indoor banner.
pub const SHOW_MON_INDOORS: u16 = 1 + 90;

/// A field-move effect in progress (Gen3 RAM: not snapshotted; a checkpoint is never taken
/// while one runs, see `World::capturable`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct FieldMoveRun {
    pub(crate) kind: FieldMove,
    /// Frames played.
    t: u16,
    /// The pose and the banner, in frames (from the map type).
    show: u16,
    /// Surf: the direction of the jump onto the water (Gen3 `movementDirection`), and whether
    /// it started.
    dir: Dir,
    jumped: bool,
}

impl FieldMoveRun {
    pub(crate) fn hash(&self, h: &mut Fnv64) {
        h.write(&[0xFE, self.kind as u8, self.dir as u8, self.jumped as u8]);
        h.write_u16(self.t);
        h.write_u16(self.show);
    }
}

/// Gen3 `IsMapTypeOutdoors` (the banner kind): towns, cities, routes and ocean routes.
fn outdoors(t: MapType) -> bool {
    matches!(
        t,
        MapType::Outdoor | MapType::OutdoorSand | MapType::OceanRoute
    )
}

impl State {
    /// Whether a field-move effect runs (the controls are locked, `waitstate` waits).
    pub(crate) fn field_move_busy(&self) -> bool {
        self.field_move.is_some()
    }

    /// Starts the effect of field move `m` (`dofieldeffect FLDEFF_USE_*`, Gen3 `FldEff_Use*`).
    /// Surf, Cut, Rock Smash, Strength, Waterfall and Dive have one; the others are ignored.
    pub(crate) fn start_field_move(&mut self, game: &impl GameData, m: FieldMove) {
        if !matches!(
            m,
            FieldMove::Surf
                | FieldMove::Cut
                | FieldMove::RockSmash
                | FieldMove::Strength
                | FieldMove::Waterfall
                | FieldMove::Dive
        ) {
            return;
        }
        let map_type = game.map_type(self.field.map());
        // Gen3 `Task_DoFieldMove_Init` skips the pose underwater; Waterfall's and Dive's tasks
        // show the mon without one (`FLDEFF_FIELD_MOVE_SHOW_MON_INIT` straight away).
        let pose = if map_type == MapType::Underwater
            || matches!(m, FieldMove::Waterfall | FieldMove::Dive)
        {
            0
        } else {
            POSE_FRAMES
        };
        let banner = if outdoors(map_type) {
            SHOW_MON_OUTDOORS
        } else {
            SHOW_MON_INDOORS
        };
        self.field_move = Some(FieldMoveRun {
            kind: m,
            t: 0,
            show: pose + banner,
            dir: self.objects.player().facing,
            jumped: false,
        });
    }

    /// One frame of the running effect (after the script and player control, before the
    /// objects move: Gen3 runs tasks after `ScriptContext_RunScript`).
    pub(crate) fn field_move_tick(&mut self, game: &impl GameData) {
        let Some(mut run) = self.field_move else {
            return;
        };
        run.t += 1;
        if run.t == 1 {
            // Init.
            if run.kind == FieldMove::Surf {
                self.objects.set_surfing(true);
                self.objects.freeze_all();
            }
            self.field_move = Some(run);
            return;
        }
        let shown = run.t > 1 + run.show;
        if !shown {
            self.field_move = Some(run);
            return;
        }
        match run.kind {
            FieldMove::Surf => {
                if !run.jumped {
                    // `SurfFieldEffect_JumpOnSurfBlob`.
                    self.objects.jump_player(run.dir);
                    run.jumped = true;
                    self.field_move = Some(run);
                } else if self.objects.is_settled(PLAYER) {
                    // `SurfFieldEffect_End`.
                    self.objects.unfreeze_all();
                    self.field_move = None;
                } else {
                    self.field_move = Some(run);
                }
            }
            FieldMove::Waterfall => {
                // `WaterfallFieldEffect_RideUp`: a slow walk north per cell, then
                // `_ContinueRideOrEnd` rides again while the player stands on a waterfall.
                if !run.jumped {
                    self.objects.walk_player_slow(Dir::Up);
                    run.jumped = true;
                    self.field_move = Some(run);
                } else if !self.objects.is_settled(PLAYER) {
                    self.field_move = Some(run);
                } else if self.on_waterfall(game) {
                    self.objects.walk_player_slow(Dir::Up);
                    self.field_move = Some(run);
                } else {
                    self.field_move = None;
                }
            }
            FieldMove::Dive => {
                // `DiveFieldEffect_TryWarp`: `TryDoDiveWarp`, then the effect ends.
                self.field_move = None;
                self.try_dive_warp(game);
            }
            // `Task_DoFieldMove_RunFunc`: `ScriptContext_Enable`.
            _ => self.field_move = None,
        }
    }

    /// Whether the player stands on a waterfall (Gen3 `currentMetatileBehavior`).
    fn on_waterfall(&self, game: &impl GameData) -> bool {
        let (x, y) = self.objects.player().pos();
        let ground = self.ground(game);
        crate::objects::Terrain::behavior(&ground, x, y)
            == crate::data::Behavior::Water(crate::data::Water::Waterfall)
    }

    /// Gen3 `TrySetDiveWarp`'s destination from the player's cell: up (`emerge`) on an
    /// underwater map off the no-surfacing cells, else down from diveable water. Returns the
    /// map and cell, `None` without one.
    fn dive_destination(&self, game: &impl GameData) -> Option<(bool, MapId, i16, i16)> {
        let map = self.field.map();
        let (x, y) = self.objects.player().pos();
        let ground = self.ground(game);
        let here = crate::objects::Terrain::behavior(&ground, x, y);
        let water = match here {
            crate::data::Behavior::Water(w) => Some(w),
            _ => None,
        };
        let emerge = game.map_type(map) == MapType::Underwater
            && !water.is_some_and(|w| w.unable_to_emerge());
        if !emerge && !water.is_some_and(|w| w.diveable()) {
            return None;
        }
        let (to, cell) = game.dive_warp(map, emerge, (x, y))?;
        let (tx, ty) = cell.unwrap_or((x, y));
        Some((emerge, to, tx, ty))
    }

    /// Gen3 `TryDoDiveWarp` (`DoDiveWarp`): the warp transition to the dive destination.
    fn try_dive_warp(&mut self, game: &impl GameData) {
        if let Some((_, map, x, y)) = self.dive_destination(game)
            && game.has_map(map)
        {
            self.transition.start(game, map, None, x, y);
            self.objects.freeze_all();
        }
    }

    /// The badge flag of field move `m`'s gate, if it has one (`None`: no badge needed).
    fn field_move_badge(&self, game: &impl GameData, m: FieldMove) -> bool {
        match game.field_move_gate(m) {
            FieldMoveGate::Never => false,
            FieldMoveGate::Flag(f) => self.vars.flag(f),
            FieldMoveGate::PartyMove { badge, .. } => badge.is_none_or(|f| self.vars.flag(f)),
        }
    }

    /// Gen3 `GetInteractedWaterScript`'s Waterfall case: the player faces a waterfall cell;
    /// with the badge (`FLAG_BADGE08_GET`) and surfing north (`IsPlayerSurfingNorth`),
    /// `EventScript_UseWaterfall` (which checks the party), else
    /// `EventScript_CannotUseWaterfall`.
    pub(crate) fn waterfall_prompt(&self, game: &impl GameData) -> Option<crate::data::ScriptId> {
        let p = self.objects.player();
        let (dx, dy) = p.facing.delta();
        let ground = self.ground(game);
        let front = crate::objects::Terrain::behavior(&ground, p.x + dx, p.y + dy);
        if front != crate::data::Behavior::Water(crate::data::Water::Waterfall) {
            return None;
        }
        if self.field_move_badge(game, FieldMove::Waterfall)
            && self.objects.surfing()
            && p.facing == Dir::Up
        {
            game.field_move_script(FieldMove::Waterfall)
        } else {
            game.waterfall_refused_script()
        }
    }

    /// Gen3 `TrySetupDiveDownScript`: A on diveable water with a dive warp and the badge
    /// (`FLAG_BADGE07_GET`) runs `EventScript_UseDive` (which checks the party).
    pub(crate) fn dive_down_prompt(&self, game: &impl GameData) -> Option<crate::data::ScriptId> {
        if !self.field_move_badge(game, FieldMove::Dive) {
            return None;
        }
        match self.dive_destination(game)? {
            (false, ..) => game.field_move_script(FieldMove::Dive),
            _ => None,
        }
    }

    /// Gen3 `TrySetupDiveEmergeScript`: B underwater where Dive surfaces, with the badge, runs
    /// `EventScript_UseDiveUnderwater`.
    pub(crate) fn dive_emerge_prompt(&self, game: &impl GameData) -> Option<crate::data::ScriptId> {
        if !self.field_move_badge(game, FieldMove::Dive) {
            return None;
        }
        match self.dive_destination(game)? {
            (true, ..) => game.dive_emerge_script(),
            _ => None,
        }
    }

    /// The party slot of the first mon that can use field move `m` from the field (Gen3
    /// `PartyHasMonWithSurf`: a mon that knows the move; the badge is checked by the caller),
    /// if the gate is a party move.
    pub(crate) fn party_slot_knowing(&self, game: &impl GameData, m: FieldMove) -> Option<u8> {
        let FieldMoveGate::PartyMove { move_id, .. } = game.field_move_gate(m) else {
            return None;
        };
        self.party
            .iter()
            .position(|p| p.moves.contains(&Some(move_id)))
            .map(|i| i as u8)
    }

    /// Whether field move `m` is usable from the field: its gate (the badge flag, a party mon
    /// knowing the move, or the game's flag).
    pub(crate) fn field_move_usable(&self, game: &impl GameData, m: FieldMove) -> bool {
        match game.field_move_gate(m) {
            FieldMoveGate::Never => false,
            FieldMoveGate::Flag(f) => self.vars.flag(f),
            FieldMoveGate::PartyMove { badge, .. } => {
                badge.is_none_or(|f| self.vars.flag(f))
                    && self.party_slot_knowing(game, m).is_some()
            }
        }
    }

    /// Gen3 `GetInteractedWaterScript`'s Surf case: the player on foot at elevation 3 faces a
    /// surfable, fishable water cell they cannot step onto (`IsPlayerFacingSurfableFishableWater`:
    /// `COLLISION_ELEVATION_MISMATCH`), and Surf is usable (`FLAG_BADGE05_GET` and
    /// `PartyHasMonWithSurf`). Returns the script to run (`EventScript_UseSurf`).
    pub(crate) fn surf_prompt(&self, game: &impl GameData) -> Option<crate::data::ScriptId> {
        let p = self.objects.player();
        if self.objects.surfing() || p.elevation != 3 {
            return None;
        }
        let (dx, dy) = p.facing.delta();
        let ground = self.ground(game);
        let front = crate::objects::Terrain::behavior(&ground, p.x + dx, p.y + dy);
        let fishable = matches!(front, crate::data::Behavior::Water(w) if w.fishable());
        if !fishable
            || self.objects.collision(&ground, PLAYER, p.facing) != Collision::ElevationMismatch
            || !self.field_move_usable(game, FieldMove::Surf)
        {
            return None;
        }
        game.field_move_script(FieldMove::Surf)
    }
}
