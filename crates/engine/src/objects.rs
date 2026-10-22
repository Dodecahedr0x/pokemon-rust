//! Map objects: the player and NPCs. Movement (player input, idle movement types, scripted
//! steps), collision, and their sprites. Timings follow Gen3 `event_object_movement.c` and
//! `field_player_avatar.c`; see the comments on each constant.
//!
//! Coordinates follow Gen3: when a step starts, the object's cell `(x, y)` is already the
//! destination and `prev` is the cell it leaves; the pixel offset (`Object::offset`) goes from
//! `-16 * dir` to 0 while the step plays. Between steps `prev == (x, y)`.

mod anim;
mod draw;
mod snapshot;
mod spawn;
#[cfg(test)]
mod tests;

use crate::data::{
    Behavior, Dir, FlagId, Gfx, GfxId, MapData, MapId, MovementType, ObjectEvent, PLAYER, VarId,
    block_collision, block_elevation,
};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::rng::Rng;
use crate::script::{Speed, Step};
use alloc::vec::Vec;
use anim::Anim;

/// What objects need to know about the map around them. Implemented by the integration
/// (field blocks with overrides and connections, and the game's behavior classification).
pub trait Terrain {
    /// Raw block (collision and elevation bits included) at (x, y), which may lie outside the map.
    fn block(&self, x: i16, y: i16) -> u16;
    fn behavior(&self, x: i16, y: i16) -> Behavior;
}

/// The player's controls for one frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PlayerInput {
    /// Held direction, if any.
    pub dir: Option<Dir>,
    /// B held.
    pub b_held: bool,
    /// Running is allowed here (Gen3: `FLAG_SYS_B_DASH`, map `allow_running`, and the current
    /// cell's behavior).
    pub can_run: bool,
}

impl PlayerInput {
    /// Direction priority as Gen3 `FieldGetPlayerInput`: up, down, left, right.
    pub fn from_buttons(held: Buttons, can_run: bool) -> Self {
        let dir = [
            (Buttons::UP, Dir::Up),
            (Buttons::DOWN, Dir::Down),
            (Buttons::LEFT, Dir::Left),
            (Buttons::RIGHT, Dir::Right),
        ]
        .into_iter()
        .find(|(b, _)| held.contains(*b))
        .map(|(_, d)| d);
        Self {
            dir,
            b_held: held.contains(Buttons::B),
            can_run,
        }
    }
}

/// What `update_player` did this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerEvent {
    /// Nothing new: standing still, or an action still in progress.
    Idle,
    /// Started turning in place towards `Dir` (Gen3 `PlayerTurnInPlace`).
    Turned(Dir),
    /// Started a step (walk, run or ledge jump) that ends at cell `to`.
    StepStarted { to: (i16, i16), dir: Dir },
    /// A step ended this frame; the player stands at `at`. Run step-end triggers now: the next
    /// step can start on the next call.
    StepFinished { at: (i16, i16) },
    /// Walked into something: walks in place slowly (Gen3 `PlayerNotOnBikeCollide`).
    Bumped(Dir),
}

/// Result of a collision test (Gen3 `COLLISION_*` subset).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Collision {
    None,
    OutsideRange,
    Impassable,
    ElevationMismatch,
    Object,
}

// ---- Gen3 timings ----

/// Frames of `walk_in_place_{slow,normal,fast,faster}` (`InitMoveInPlace` durations).
const fn in_place_frames(s: Speed) -> u8 {
    match s {
        Speed::Slow => 32,
        Speed::Normal => 16,
        Speed::Fast => 8,
        Speed::Faster => 4,
    }
}

/// The player's turn in place: `walk_in_place_fast` (`PlayerTurnInPlace`).
pub const TURN_FRAMES: u8 = 8;
/// The player's bump: `walk_in_place_slow` (`PlayerNotOnBikeCollide`).
pub const BUMP_FRAMES: u8 = 32;
/// `sMovementDelaysMedium`, picked with `Random() % 4` by look-around and wander types.
const DELAYS_MEDIUM: [u16; 4] = [32, 64, 96, 128];
/// `sJumpY_High` / `sJumpY_Normal`.
const JUMP_Y_HIGH: [i8; 16] = [
    -4, -6, -8, -10, -11, -12, -12, -12, -11, -10, -9, -8, -6, -4, 0, 0,
];
const JUMP_Y_NORMAL: [i8; 16] = [
    -2, -4, -6, -8, -9, -10, -10, -10, -9, -8, -6, -5, -3, -2, 0, 0,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ActionKind {
    /// `walk_slow` (1 px every other frame, 31 frames), `walk_normal` (1 px, 16), `walk_fast`
    /// (2 px, 8), `walk_faster` (4 px, 4).
    Walk(Speed),
    /// `player_run`: `walk_fast` timing with the run animation.
    Run,
    /// Walk in place for this many frames.
    InPlace(u8),
    /// Jump 0, 1 or 2 cells: 16, 16 or 32 frames (`DoJumpSpriteMovement`).
    Jump(u8),
    Delay(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Action {
    kind: ActionKind,
    dir: Dir,
    /// Frames played so far.
    t: u8,
    off: (i16, i16),
    jump_y: i8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Moves {
    steps: &'static [Step],
    /// [`crate::script::steps_digest`] of `steps` (their hashed identity).
    digest: u32,
    next: u16,
}

/// Idle movement state (Gen3 `sTypeFuncId` plus the movement delay).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Idle {
    Start,
    Wait(u16),
    Done,
}

/// Gen3 `gPlayerAvatar.runningState`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RunState {
    NotMoving,
    TurnDirection,
    Moving,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    pub local_id: u8,
    /// Map the object belongs to. Objects of the previous map stay (shifted) after crossing a
    /// connection until they leave the view; only the current map's are addressed by local id.
    pub map: MapId,
    /// `None` for the player, drawn with `GameData::player_sprite`.
    pub gfx: Option<GfxId>,
    /// Current cell (the destination while a step plays).
    pub x: i16,
    pub y: i16,
    /// Cell being left while a step plays; equals (x, y) otherwise.
    pub prev: (i16, i16),
    pub elevation: u8,
    pub facing: Dir,
    pub movement: MovementType,
    /// Spawn cell; wander range is measured from it.
    pub home: (i16, i16),
    /// Wander range; 0 on an axis means unbounded on that axis, as Gen3.
    pub range: (u8, u8),
    pub visible: bool,
    /// Frozen objects take no idle movement or player input (an action in progress finishes).
    pub frozen: bool,
    pub facing_locked: bool,
    action: Option<Action>,
    moves: Option<Moves>,
    idle: Idle,
    anim: Anim,
    /// Render cache (not hashed): OBJ VRAM tiles (first, count), and the sprite (address) and
    /// frame currently uploaded there.
    vram: Option<(u16, u16)>,
    uploaded: Option<(usize, u16)>,
}

/// Initial facing per movement type (Gen3 `gInitialMovementTypeFacingDirections`).
const fn initial_facing(m: MovementType) -> Dir {
    match m {
        MovementType::Face(d) | MovementType::WalkInPlace(d) => d,
        MovementType::WanderVertical => Dir::Up,
        MovementType::WanderHorizontal => Dir::Left,
        _ => Dir::Down,
    }
}

impl Object {
    fn new(
        local_id: u8,
        map: MapId,
        gfx: Option<GfxId>,
        x: i16,
        y: i16,
        movement: MovementType,
    ) -> Self {
        Self {
            local_id,
            map,
            gfx,
            x,
            y,
            prev: (x, y),
            elevation: 0,
            facing: initial_facing(movement),
            movement,
            home: (x, y),
            range: (0, 0),
            visible: movement != MovementType::Invisible,
            frozen: false,
            facing_locked: false,
            action: None,
            moves: None,
            idle: Idle::Start,
            anim: Anim::new(),
            vram: None,
            uploaded: None,
        }
    }

    fn from_event(map: MapId, ev: &ObjectEvent, vars: &impl Fn(VarId) -> u16) -> Self {
        let gfx = match ev.gfx {
            Gfx::Fixed(g) => g,
            Gfx::Var(v) => GfxId(vars(v)),
        };
        let mut o = Self::new(ev.local_id, map, Some(gfx), ev.x, ev.y, ev.movement);
        o.elevation = ev.elevation;
        o.range = ev.range;
        o
    }

    pub const fn pos(&self) -> (i16, i16) {
        (self.x, self.y)
    }

    /// Pixel offset from the current cell while a step plays.
    pub fn offset(&self) -> (i16, i16) {
        self.action.map_or((0, 0), |a| a.off)
    }

    /// Height above ground while jumping (negative is up).
    pub fn jump_y(&self) -> i8 {
        self.action.map_or(0, |a| a.jump_y)
    }

    /// Map pixel of the object's cell top-left, including the step offset (not the jump).
    pub fn pixel_pos(&self) -> (i32, i32) {
        let (ox, oy) = self.offset();
        (
            self.x as i32 * 16 + ox as i32,
            self.y as i32 * 16 + oy as i32,
        )
    }

    /// No action in progress.
    pub fn is_settled(&self) -> bool {
        self.action.is_none()
    }

    fn set_dir(&mut self, d: Dir) {
        if !self.facing_locked {
            self.facing = d;
        }
    }

    /// Gen3 `FaceDirection`: turn and show the standing frame of the walk animation.
    fn face(&mut self, d: Dir) {
        self.set_dir(d);
        self.anim.stand();
    }

    /// Starts an action and plays its first frame, as Gen3 runs `Step0` and `Step1` together.
    /// Returns whether it already finished.
    fn begin(&mut self, kind: ActionKind, dir: Dir) -> bool {
        let cells = match kind {
            ActionKind::Walk(_) | ActionKind::Run => 1,
            ActionKind::Jump(n) => n.min(1) as i16,
            ActionKind::InPlace(_) | ActionKind::Delay(_) => 0,
        };
        if !matches!(kind, ActionKind::Delay(_)) {
            self.set_dir(dir);
        }
        let (dx, dy) = dir.delta();
        self.prev = (self.x, self.y);
        self.x += dx * cells;
        self.y += dy * cells;
        if let Some(table) = anim::table_for(kind) {
            self.anim.start(table);
        }
        self.action = Some(Action {
            kind,
            dir,
            t: 0,
            off: (-16 * dx * cells, -16 * dy * cells),
            jump_y: 0,
        });
        self.tick()
    }

    /// Plays one frame of the current action. Returns whether it finished (true when idle).
    fn tick(&mut self) -> bool {
        let Some(mut a) = self.action else {
            return true;
        };
        let (dx, dy) = a.dir.delta();
        let mut step = |px: i16| {
            a.off.0 += dx * px;
            a.off.1 += dy * px;
        };
        let done = match a.kind {
            ActionKind::Walk(speed) => {
                match speed {
                    // `UpdateWalkSlowAnim`: 1 px on even frames until 16 px have been moved.
                    Speed::Slow if a.t & 1 == 0 => step(1),
                    Speed::Slow => {}
                    Speed::Normal => step(1),
                    Speed::Fast => step(2),
                    Speed::Faster => step(4),
                }
                a.off == (0, 0)
            }
            ActionKind::Run => {
                step(2);
                a.off == (0, 0)
            }
            ActionKind::InPlace(n) => a.t + 1 >= n,
            ActionKind::Jump(n) => {
                if n > 0 {
                    step(1);
                }
                let (frames, shift, table) = match n {
                    0 => (16, 0, &JUMP_Y_HIGH),
                    1 => (16, 0, &JUMP_Y_NORMAL),
                    _ => (32, 1, &JUMP_Y_HIGH),
                };
                a.jump_y = table[(a.t >> shift) as usize];
                // `UpdateJumpAnim`: a far jump moves its cell one more step at halfway.
                if n == 2 && a.t + 1 == 16 {
                    self.prev = (self.x, self.y);
                    self.x += dx;
                    self.y += dy;
                    a.off = (a.off.0 - 16 * dx, a.off.1 - 16 * dy);
                }
                if a.t + 1 >= frames {
                    a.jump_y = 0;
                    true
                } else {
                    false
                }
            }
            ActionKind::Delay(n) => a.t + 1 >= n.max(1),
        };
        a.t += 1;
        if !matches!(a.kind, ActionKind::Delay(_)) {
            self.anim.tick();
        }
        if done {
            self.action = None;
            self.prev = (self.x, self.y);
        } else {
            self.action = Some(a);
        }
        done
    }

    /// Gen3 `ObjectEventUpdateElevation`.
    fn update_elevation(&mut self, terrain: &impl Terrain) {
        let cur = block_elevation(terrain.block(self.x, self.y));
        let prev = block_elevation(terrain.block(self.prev.0, self.prev.1));
        if cur != 15 && prev != 15 {
            self.elevation = cur;
        }
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.local_id]);
        h.write_u16(self.map.0);
        h.write_u16(self.gfx.map_or(0xFFFF, |g| g.0));
        h.write_u16(self.x as u16);
        h.write_u16(self.y as u16);
        h.write_u16(self.prev.0 as u16);
        h.write_u16(self.prev.1 as u16);
        h.write(&[self.elevation, self.facing as u8]);
        h.write(&movement_code(self.movement));
        h.write_u16(self.home.0 as u16);
        h.write_u16(self.home.1 as u16);
        h.write(&[
            self.range.0,
            self.range.1,
            self.visible as u8,
            self.frozen as u8,
            self.facing_locked as u8,
        ]);
        match self.action {
            None => h.write(&[0]),
            Some(a) => {
                let (k, arg) = match a.kind {
                    ActionKind::Walk(s) => (1, speed_code(s)),
                    ActionKind::Run => (2, 0),
                    ActionKind::InPlace(n) => (3, n),
                    ActionKind::Jump(n) => (4, n),
                    ActionKind::Delay(n) => (5, n),
                };
                h.write(&[k, arg, a.dir as u8, a.t, a.jump_y as u8]);
                h.write_u16(a.off.0 as u16);
                h.write_u16(a.off.1 as u16);
            }
        }
        match self.moves {
            None => h.write(&[0]),
            Some(m) => {
                // The steps are static data: their digest and the cursor identify the progress.
                h.write(&[1]);
                h.write_u32(m.digest);
                h.write_u16(m.next);
            }
        }
        match self.idle {
            Idle::Start => h.write(&[0]),
            Idle::Wait(n) => {
                h.write(&[1]);
                h.write_u16(n);
            }
            Idle::Done => h.write(&[2]),
        }
        self.anim.hash(h);
    }
}

/// Two bytes identifying a movement type, for hashes.
pub const fn movement_code(m: MovementType) -> [u8; 2] {
    match m {
        MovementType::Face(d) => [0, d as u8],
        MovementType::LookAround => [1, 0],
        MovementType::Wander => [2, 0],
        MovementType::WanderVertical => [3, 0],
        MovementType::WanderHorizontal => [4, 0],
        MovementType::WalkInPlace(d) => [5, d as u8],
        MovementType::Invisible => [6, 0],
    }
}

const fn speed_code(s: Speed) -> u8 {
    match s {
        Speed::Slow => 0,
        Speed::Normal => 1,
        Speed::Fast => 2,
        Speed::Faster => 3,
    }
}

/// Gen3 `GetDirectionToFace`: horizontal difference first.
pub const fn direction_to(from: (i16, i16), to: (i16, i16)) -> Dir {
    if from.0 > to.0 {
        Dir::Left
    } else if from.0 < to.0 {
        Dir::Right
    } else if from.1 > to.1 {
        Dir::Up
    } else {
        Dir::Down
    }
}

/// Gen3 `IsElevationMismatchAt`.
const fn elevation_mismatch(obj: u8, cell: u8) -> bool {
    obj != 0 && cell != 0 && cell != 15 && cell != obj
}

/// Gen3 `AreElevationsCompatible`.
const fn elevations_compatible(a: u8, b: u8) -> bool {
    a == 0 || b == 0 || a == b
}

/// Gen3 `RemoveObjectEventIfOutsideView`: the kept area around the player's cell
/// (`pos - 2 ..= pos + 17` by `pos ..= pos + 16`, with the player at `pos + 7`).
const VIEW_X: (i16, i16) = (-9, 10);
const VIEW_Y: (i16, i16) = (-7, 9);

/// Every object: the current map's, plus the previous map's still in view after crossing a
/// connection. Index 0 is always the player.
pub struct Objects {
    objs: Vec<Object>,
    /// The current map.
    map: MapId,
    run_state: RunState,
    /// Render cache (not hashed): colors last uploaded to each OBJ palette bank.
    banks: [Option<[u16; 16]>; 16],
}

impl Default for Objects {
    fn default() -> Self {
        Self::new()
    }
}

impl Objects {
    /// Just the player, at (0, 0) facing down; place it with `set_pos` and `face`.
    pub fn new() -> Self {
        let player = Object::new(PLAYER, MapId(0), None, 0, 0, MovementType::Face(Dir::Down));
        Self {
            objs: alloc::vec![player],
            map: MapId(0),
            run_state: RunState::NotMoving,
            banks: [None; 16],
        }
    }

    /// The current map.
    pub fn map(&self) -> MapId {
        self.map
    }

    fn set_map(&mut self, id: MapId) {
        self.map = id;
        self.objs[0].map = id;
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
    fn despawn_outside_view(&mut self) {
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

    /// Despawns an object of the current map (Gen3 `removeobject`). The player cannot be
    /// removed.
    pub fn remove(&mut self, local: u8) {
        if local != PLAYER {
            let map = self.map;
            self.objs.retain(|o| o.local_id != local || o.map != map);
        }
    }

    /// Index of the current map's object `local` (the player for `PLAYER`).
    fn index(&self, local: u8) -> Option<usize> {
        self.objs
            .iter()
            .position(|o| o.local_id == local && o.map == self.map)
    }

    fn with(&mut self, local: u8, f: impl FnOnce(&mut Object)) {
        if let Some(i) = self.index(local) {
            f(&mut self.objs[i]);
        }
    }

    /// The current map's object `local`.
    pub fn get(&self, local: u8) -> Option<&Object> {
        self.index(local).map(|i| &self.objs[i])
    }

    pub fn player(&self) -> &Object {
        &self.objs[0]
    }

    /// The player first, then NPCs in spawn order (other maps' included).
    pub fn iter(&self) -> impl Iterator<Item = &Object> {
        self.objs.iter()
    }

    /// The NPC on cell (x, y) at an elevation compatible with `elevation`, if any, as (local
    /// id, map). Like Gen3 `GetObjectEventIdByPosition` it does not filter by map: an NPC of the
    /// previous map still in view after a crossing is found too.
    pub fn at(&self, x: i16, y: i16, elevation: u8) -> Option<(u8, MapId)> {
        self.objs[1..]
            .iter()
            .find(|o| (o.x, o.y) == (x, y) && elevations_compatible(o.elevation, elevation))
            .map(|o| (o.local_id, o.map))
    }

    /// Moves every object by (dx, dy) cells, keeping steps in progress.
    pub fn shift(&mut self, dx: i16, dy: i16) {
        for o in &mut self.objs {
            o.x += dx;
            o.y += dy;
            o.prev = (o.prev.0 + dx, o.prev.1 + dy);
            o.home = (o.home.0 + dx, o.home.1 + dy);
        }
    }

    /// Places an object on a cell, cancelling its action and movement script (Gen3 `setobjectxy`).
    pub fn set_pos(&mut self, local: u8, x: i16, y: i16) {
        self.with(local, |o| {
            o.x = x;
            o.y = y;
            o.prev = (x, y);
            o.action = None;
            o.moves = None;
        });
        if local == PLAYER {
            self.run_state = RunState::NotMoving;
        }
    }

    /// Turns an object at once (Gen3 `ObjectEventTurn`, `turnobject`).
    pub fn face(&mut self, local: u8, dir: Dir) {
        self.with(local, |o| {
            o.set_dir(dir);
            o.anim.turn();
        });
    }

    /// Direction from `local` towards the player.
    pub fn direction_to_player(&self, local: u8) -> Option<Dir> {
        let p = self.player().pos();
        self.get(local).map(|o| direction_to(o.pos(), p))
    }

    pub fn show(&mut self, local: u8) {
        self.with(local, |o| o.visible = true);
    }

    pub fn hide(&mut self, local: u8) {
        self.with(local, |o| o.visible = false);
    }

    pub fn set_movement_type(&mut self, local: u8, movement: MovementType) {
        self.with(local, |o| {
            o.movement = movement;
            o.idle = Idle::Start;
        });
    }

    pub fn freeze(&mut self, local: u8) {
        self.with(local, |o| o.frozen = true);
    }

    pub fn unfreeze(&mut self, local: u8) {
        self.with(local, |o| o.frozen = false);
    }

    pub fn freeze_all(&mut self) {
        self.objs.iter_mut().for_each(|o| o.frozen = true);
    }

    pub fn unfreeze_all(&mut self) {
        self.objs.iter_mut().for_each(|o| o.frozen = false);
    }

    /// The object has no action in progress (true for unknown ids).
    pub fn is_settled(&self, local: u8) -> bool {
        self.get(local).is_none_or(Object::is_settled)
    }

    /// Gives `local` a movement script. It starts once the current action has finished.
    pub fn start_moves(&mut self, local: u8, steps: &'static [Step]) {
        self.with(local, |o| {
            o.moves = Some(Moves {
                steps,
                digest: crate::script::steps_digest(steps),
                next: 0,
            })
        });
    }

    /// The movement script has finished (true for unknown ids, as Gen3 `waitmovement`).
    pub fn moves_done(&self, local: u8) -> bool {
        self.get(local).is_none_or(|o| o.moves.is_none())
    }

    /// Collision for object `i` entering (x, y) (Gen3 `GetCollisionAtCoords`). `Behavior::Blocked`
    /// counts as impassable.
    fn collision_at(&self, terrain: &impl Terrain, i: usize, x: i16, y: i16) -> Collision {
        let o = &self.objs[i];
        let outside = |home: i16, range: u8, v: i16| {
            range != 0 && (v < home - range as i16 || v > home + range as i16)
        };
        if outside(o.home.0, o.range.0, x) || outside(o.home.1, o.range.1, y) {
            return Collision::OutsideRange;
        }
        let block = terrain.block(x, y);
        if block_collision(block) != 0 || terrain.behavior(x, y) == Behavior::Blocked {
            return Collision::Impassable;
        }
        if elevation_mismatch(o.elevation, block_elevation(block)) {
            return Collision::ElevationMismatch;
        }
        let blocked = self.objs.iter().enumerate().any(|(j, other)| {
            j != i
                && ((other.x, other.y) == (x, y) || other.prev == (x, y))
                && elevations_compatible(o.elevation, other.elevation)
        });
        if blocked {
            Collision::Object
        } else {
            Collision::None
        }
    }

    /// Collision for `local` taking one step towards `dir`.
    pub fn collision(&self, terrain: &impl Terrain, local: u8, dir: Dir) -> Collision {
        let Some(i) = self.index(local) else {
            return Collision::None;
        };
        let o = &self.objs[i];
        let (dx, dy) = dir.delta();
        self.collision_at(terrain, i, o.x + dx, o.y + dy)
    }

    /// Player control for one frame, as Gen3 `MovePlayerNotOnBike`. Does nothing while the
    /// player runs a movement script (`update` plays it).
    pub fn update_player(&mut self, terrain: &impl Terrain, input: PlayerInput) -> PlayerEvent {
        if self.objs[0].moves.is_some() {
            return PlayerEvent::Idle;
        }
        self.objs[0].update_elevation(terrain);
        // `CheckMovementInputNotOnBike` runs every frame, even while an action plays.
        let p = &self.objs[0];
        self.run_state = match input.dir {
            None => RunState::NotMoving,
            Some(d) if d != p.facing && self.run_state != RunState::Moving => {
                RunState::TurnDirection
            }
            Some(_) => RunState::Moving,
        };
        if let Some(a) = p.action {
            // `PlayerSetAnimId` ignores input while an action plays.
            let done = self.objs[0].tick();
            let stepped = matches!(
                a.kind,
                ActionKind::Walk(_) | ActionKind::Run | ActionKind::Jump(1..)
            );
            return if done && stepped {
                PlayerEvent::StepFinished {
                    at: self.objs[0].pos(),
                }
            } else {
                PlayerEvent::Idle
            };
        }
        if p.frozen {
            return PlayerEvent::Idle;
        }
        let Some(dir) = input.dir else {
            let f = p.facing;
            self.objs[0].face(f);
            return PlayerEvent::Idle;
        };
        if self.run_state == RunState::TurnDirection {
            self.objs[0].begin(ActionKind::InPlace(TURN_FRAMES), dir);
            return PlayerEvent::Turned(dir);
        }
        let (dx, dy) = dir.delta();
        let (tx, ty) = (p.x + dx, p.y + dy);
        if terrain.behavior(tx, ty) == Behavior::Ledge(dir) {
            self.objs[0].begin(ActionKind::Jump(2), dir);
            return PlayerEvent::StepStarted {
                to: (tx + dx, ty + dy),
                dir,
            };
        }
        if self.collision_at(terrain, 0, tx, ty) != Collision::None {
            self.objs[0].begin(ActionKind::InPlace(BUMP_FRAMES), dir);
            return PlayerEvent::Bumped(dir);
        }
        let kind = if input.b_held && input.can_run {
            ActionKind::Run
        } else {
            ActionKind::Walk(Speed::Normal)
        };
        self.objs[0].begin(kind, dir);
        PlayerEvent::StepStarted { to: (tx, ty), dir }
    }

    /// Advances every object one frame: actions in progress, movement scripts, and NPC idle
    /// movement. `player_updated` says whether `update_player` ran this frame; if not, the
    /// player's action in progress is played here. The player's movement script always is.
    /// Then other maps' objects outside the view are despawned.
    pub fn update(&mut self, terrain: &impl Terrain, rng: &mut Rng, player_updated: bool) {
        let skip_player = player_updated && self.objs[0].moves.is_none();
        for i in 0..self.objs.len() {
            if i == 0 && skip_player {
                continue;
            }
            self.objs[i].update_elevation(terrain);
            if self.advance(i) || i == 0 || self.objs[i].frozen {
                continue;
            }
            self.idle(terrain, rng, i);
        }
        self.despawn_outside_view();
    }

    /// Plays the action in progress or the next scripted step. Returns false if the object is
    /// free for idle movement.
    fn advance(&mut self, i: usize) -> bool {
        let o = &mut self.objs[i];
        if o.action.is_some() {
            o.tick();
        } else if let Some(m) = &mut o.moves {
            match m.steps.get(m.next as usize) {
                Some(&step) => {
                    m.next += 1;
                    self.run_step(i, step);
                }
                None => o.moves = None,
            }
        } else {
            return false;
        }
        let o = &mut self.objs[i];
        if let Some(m) = o.moves
            && m.next as usize >= m.steps.len()
            && o.action.is_none()
        {
            o.moves = None;
        }
        true
    }

    /// One `script::Step`, as the Gen3 `MOVEMENT_ACTION_*` it stands for. Instant steps take
    /// the frame.
    fn run_step(&mut self, i: usize, step: Step) {
        let player = self.objs[0].pos();
        let o = &mut self.objs[i];
        match step {
            Step::Face(d) => o.face(d),
            Step::Walk(d, s) => {
                o.begin(ActionKind::Walk(s), d);
            }
            Step::WalkInPlace(d, s) => {
                o.begin(ActionKind::InPlace(in_place_frames(s)), d);
            }
            Step::Jump(d, n) => {
                o.begin(ActionKind::Jump(n.min(2)), d);
            }
            Step::Delay(n) => {
                let f = o.facing;
                o.begin(ActionKind::Delay(n), f);
            }
            Step::FacePlayer => o.face(direction_to(o.pos(), player)),
            Step::FaceAwayFromPlayer => o.face(direction_to(o.pos(), player).opposite()),
            Step::LockFacing => o.facing_locked = true,
            Step::UnlockFacing => o.facing_locked = false,
            Step::Hide => o.visible = false,
            Step::Show => o.visible = true,
            // TODO: emote bubble sprite (Gen3 field effect, ~60 frames, independent of the
            // movement script).
            Step::Emote(_) => {}
        }
    }

    /// Idle movement per `MovementType` (Gen3 `MovementType_*` callbacks).
    fn idle(&mut self, terrain: &impl Terrain, rng: &mut Rng, i: usize) {
        let random = |rng: &mut Rng| (rng.next_u32() >> 16) as u16;
        let o = &mut self.objs[i];
        let dirs: &[Dir] = match o.movement {
            MovementType::Face(d) => {
                if o.idle == Idle::Start {
                    o.face(d);
                    o.idle = Idle::Done;
                }
                return;
            }
            MovementType::Invisible => {
                if o.idle == Idle::Start {
                    o.visible = false;
                    o.idle = Idle::Done;
                }
                return;
            }
            MovementType::WalkInPlace(d) => {
                o.begin(ActionKind::InPlace(in_place_frames(Speed::Normal)), d);
                return;
            }
            MovementType::LookAround | MovementType::Wander => &Dir::ALL,
            // `gUpAndDownDirections` / `gLeftAndRightDirections`.
            MovementType::WanderVertical => &[Dir::Down, Dir::Up],
            MovementType::WanderHorizontal => &[Dir::Left, Dir::Right],
        };
        if let Idle::Wait(n) = o.idle {
            if n > 1 {
                o.idle = Idle::Wait(n - 1);
                return;
            }
            // Delay over: pick a direction (`Random() & 3` / `& 1`) and walk if free.
            let d = dirs[random(rng) as usize & (dirs.len() - 1)];
            o.set_dir(d);
            let wanders = o.movement != MovementType::LookAround;
            if wanders && self.collision(terrain, self.objs[i].local_id, d) == Collision::None {
                let o = &mut self.objs[i];
                o.begin(ActionKind::Walk(Speed::Normal), d);
                o.idle = Idle::Start;
                return;
            }
        }
        // Face the current direction, then wait `sMovementDelaysMedium[Random() % 4]`.
        let o = &mut self.objs[i];
        let f = o.facing;
        o.face(f);
        o.idle = Idle::Wait(DELAYS_MEDIUM[random(rng) as usize % 4]);
    }

    /// Hashes everything that affects future frames, except the render cache (VRAM slots,
    /// uploaded frames and palette banks).
    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write_u32(self.objs.len() as u32);
        for o in &self.objs {
            o.hash(h);
        }
        h.write_u16(self.map.0);
        h.write(&[self.run_state as u8]);
    }
}
