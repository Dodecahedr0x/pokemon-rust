//! Map objects: the player and NPCs. Movement (player input, idle movement types, scripted
//! steps), collision, and their sprites. Timings follow Gen3 `event_object_movement.c` and
//! `field_player_avatar.c`; see the comments on each constant.
//!
//! Coordinates follow Gen3: when a step starts, the object's cell `(x, y)` is already the
//! destination and `prev` is the cell it leaves; the pixel offset (`Object::offset`) goes from
//! `-16 * dir` to 0 while the step plays. Between steps `prev == (x, y)`.

mod anim;
mod approach;
mod draw;
mod emote;
mod forced;
use forced::ForcedState;
mod movement;
mod snapshot;
pub(crate) use snapshot::SURFING_SNAP;
mod spawn;
mod virtual_obj;
pub use spawn::OBJECT_EVENTS_COUNT;
#[cfg(test)]
mod tests;

use crate::data::{
    Behavior, Dir, Gfx, GfxId, MapId, MovementType, ObjectEvent, PLAYER, VarId, block_collision,
    block_elevation,
};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::rng::Rng;
use crate::script::{Speed, Step};
use alloc::vec::Vec;
use anim::Anim;
pub(crate) use approach::{Approach, ApproachStep};
pub use draw::{EMOTE_TILES, SKY_FIRST_TILE, SKY_TILES};
use emote::EmoteFx;
pub use emote::{EMOTE_FRAMES, EMOTES_MAX};
#[cfg(test)]
use movement::{DELAYS_MEDIUM, DELAYS_SHORT, ROTATE_DELAY};

/// What objects need to know about the map around them. Implemented by the integration
/// (field blocks with overrides and connections, and the game's behavior classification).
pub trait Terrain {
    /// Raw block (collision and elevation bits included) at (x, y), which may lie outside the map.
    fn block(&self, x: i16, y: i16) -> u16;
    fn behavior(&self, x: i16, y: i16) -> Behavior;
    /// Whether object `local_id` may be pushed from `from` onto `to` (`GameData::push_allowed`).
    fn push_allowed(&self, local_id: u8, from: (i16, i16), to: (i16, i16)) -> bool {
        let _ = (local_id, from, to);
        true
    }
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
    /// Pushed a boulder with Strength (Gen3 `COLLISION_PUSHED_BOULDER`): walks in place
    /// slowly while the boulder (local id `id`) moves to `to`.
    PushedBoulder { dir: Dir, id: u8, to: (i16, i16) },
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
    /// Walk sequences and back-and-forth walks: Gen3 `directionSequenceIndex` (`Start` is 0).
    Seq(u8),
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
    /// Gen3 `currentElevation`: collisions and object overlaps.
    pub elevation: u8,
    /// Gen3 `previousElevation`: the last elevation other than 0 and 15, which the field's
    /// position checks read (`PlayerGetElevation`: warps, coord and bg events), so a hole on
    /// an elevation-0 cell still matches its warp's elevation. Read with
    /// [`Object::position_elevation`].
    prev_elevation: u8,
    pub facing: Dir,
    /// Read with [`Object::movement`]; changed only through [`Objects::set_movement_type`]
    /// (`code` caches its hash bytes).
    movement: MovementType,
    /// `movement_code(movement)`, kept so the per-frame hash does not match on the type.
    code: [u8; 2],
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
pub const fn initial_facing(m: MovementType) -> Dir {
    match m {
        MovementType::Face(d)
        | MovementType::WalkInPlace(d)
        | MovementType::JogInPlace(d)
        | MovementType::RunInPlace(d)
        | MovementType::WalkSlowlyInPlace(d)
        | MovementType::WalkBackAndForth(d) => d,
        MovementType::FaceTwo(p) | MovementType::WalkSequence(p) => p.get(0),
        // `FACE_UP_LEFT_AND_RIGHT` starts north, the other three-direction ones south.
        MovementType::FaceAllBut(Dir::Down) => Dir::Up,
        MovementType::WanderVertical => Dir::Up,
        MovementType::WanderHorizontal => Dir::Left,
        _ => Dir::Down,
    }
}

impl Object {
    /// The idle movement type.
    pub const fn movement(&self) -> MovementType {
        self.movement
    }

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
            prev_elevation: 0,
            facing: initial_facing(movement),
            movement,
            code: movement_code(movement),
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
        o.prev_elevation = ev.elevation;
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
            if cur != 0 {
                self.prev_elevation = cur;
            }
        }
    }

    /// The elevation of the object's position for event matching (Gen3 `PlayerGetElevation`,
    /// `previousElevation`).
    pub const fn position_elevation(&self) -> u8 {
        self.prev_elevation
    }

    /// `elevation` with `prev_elevation` packed into the high nibble (`prev + 1`) when they
    /// differ (it is then at most 14): the checkpoint's `ObjectSnap::elevation`, unchanged
    /// for the usual equal pair.
    pub(crate) const fn packed_elevation(&self) -> u8 {
        if self.prev_elevation == self.elevation {
            self.elevation
        } else {
            self.elevation | ((self.prev_elevation + 1) << 4)
        }
    }

    /// Inverse of [`Object::packed_elevation`]: `(elevation, prev_elevation)`.
    pub(crate) const fn unpack_elevation(b: u8) -> (u8, u8) {
        let e = b & 0x0F;
        match b >> 4 {
            0 => (e, e),
            p => (e, p - 1),
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
        // Only where it decides an event match (standing on an elevation-0 cell), so the
        // usual hashes are unchanged.
        if self.elevation == 0 && self.prev_elevation != 0 {
            h.write(&[0xE0 | self.prev_elevation]);
        }
        h.write(&self.code);
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
            Idle::Seq(n) => h.write(&[3, n]),
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
        MovementType::FaceTwo(p) => [7, p.0],
        MovementType::FaceAllBut(d) => [8, d as u8],
        MovementType::Rotate { clockwise } => [9, clockwise as u8],
        MovementType::WalkBackAndForth(d) => [10, d as u8],
        MovementType::WalkSequence(r) => [11, r.0],
        MovementType::JogInPlace(d) => [12, d as u8],
        MovementType::RunInPlace(d) => [13, d as u8],
        MovementType::WalkSlowlyInPlace(d) => [14, d as u8],
        MovementType::WanderBackwards => [15, 0],
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

/// What a forced step runs into (Gen3 `DoForcedMovement`'s collisions that still move).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ForcedHit {
    Free,
    Ledge,
    StopSurfing,
}

/// Every object: the current map's, plus the previous map's still in view after crossing a
/// connection. Index 0 is always the player.
pub struct Objects {
    objs: Vec<Object>,
    /// The current map.
    map: MapId,
    run_state: RunState,
    /// Render cache (not hashed): colors last uploaded to each OBJ palette bank.
    banks: [Option<[u16; 16]>; 16],
    /// OBJ palette banks the sky's particles and overlay hold this frame (`crate::sky`): the
    /// objects leave them alone. Display state, set by the World before each draw.
    reserved_banks: u16,
    // ---- Milestone 8 ----
    /// Emote bubbles shown (Gen3 field effects), at most [`EMOTES_MAX`].
    emotes: Vec<EmoteFx>,
    /// A trainer that saw the player and walks up (Gen3 `TrainerSeeFuncList`).
    approach: Option<Approach>,
    // ---- forced movement ----
    /// [`ForcedState::Held`] (Gen3 `PLAYER_AVATAR_FLAG_CONTROLLABLE`) or
    /// [`ForcedState::Centered`] (Gen3 `T_TILE_CENTER` on a forced cell); see [`ForcedState`].
    forced: ForcedState,
    // ---- Wave C ----
    /// Virtual objects (`createvobject`; [`virtual_obj`]).
    virtuals: Vec<virtual_obj::VirtualObject>,
    // ---- Wave C: field moves ----
    /// Gen3 `PLAYER_AVATAR_FLAG_SURFING`: the player moves on water at `walk_fast`, and a step
    /// onto land dismounts ([`Objects::update_player`]). Set by the Surf field effect
    /// (`world/field_move.rs`). Hashed and snapshotted only while set.
    surfing: bool,
    /// The graphics of objects the player pushes with Strength, while it is in use (Gen3
    /// `FLAG_SYS_USE_STRENGTH` and `OBJ_EVENT_GFX_PUSHABLE_BOULDER`). Set by the World before
    /// each `update_player` from the flag: derived state, neither hashed nor snapshotted.
    pushable: Option<(GfxId, &'static [GfxId])>,
    /// The map's rotating gates and their orientations (Gen3 `rotating_gate.c`), set by the
    /// World before each `update_player` from the temp vars and read back after: derived
    /// state, neither hashed nor snapshotted.
    gates: Option<(
        &'static [crate::data::RotatingGate],
        crate::gates::Orientations,
    )>,
}

pub use virtual_obj::{VIRTUAL_MAX, VirtualObject};

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
            reserved_banks: 0,
            emotes: Vec::new(),
            approach: None,
            forced: ForcedState::Free,
            virtuals: Vec::new(),
            surfing: false,
            pushable: None,
            gates: None,
        }
    }

    /// Whether the player surfs (Gen3 `PLAYER_AVATAR_FLAG_SURFING`).
    pub fn surfing(&self) -> bool {
        self.surfing
    }

    /// Sets or clears the surfing state (the Surf field effect, a warp's arrival).
    pub fn set_surfing(&mut self, on: bool) {
        self.surfing = on;
    }

    /// Reserves OBJ palette banks `mask` for the sky (`crate::sky`) until the next call: the
    /// objects drawing in them move to other banks.
    pub fn reserve_banks(&mut self, mask: u16) {
        for (b, bank) in self.banks.iter_mut().enumerate() {
            if mask & (1 << b) != 0 {
                *bank = None;
            }
        }
        self.reserved_banks = mask;
    }

    /// Sets the graphics the player pushes this frame ([`Objects::update_player`]'s Strength
    /// push: the boulder's and the game's others), or `None` while Strength is not in use.
    pub fn set_pushable(&mut self, gfx: Option<(GfxId, &'static [GfxId])>) {
        self.pushable = gfx;
    }

    /// Sets the rotating gates the player collides with this frame ([`Objects::update_player`];
    /// `None` on a map without a puzzle).
    pub fn set_gates(
        &mut self,
        gates: Option<(
            &'static [crate::data::RotatingGate],
            crate::gates::Orientations,
        )>,
    ) {
        self.gates = gates;
    }

    /// The gates' orientations after `update_player` (a step may have turned one).
    pub fn gate_orientations(&self) -> Option<crate::gates::Orientations> {
        self.gates.map(|(_, o)| o)
    }

    /// Gen3 `TryPushBoulder`: the player walks into a pushable object (`pushable`) whose
    /// next cell on is free (`GetCollisionAtCoords == COLLISION_NONE` for the boulder, and not
    /// `MetatileBehavior_IsNonAnimDoor`). Returns the boulder's slot.
    fn boulder_ahead(&self, terrain: &impl Terrain, dir: Dir) -> Option<usize> {
        let (g, more) = self.pushable?;
        let p = &self.objs[0];
        let (dx, dy) = dir.delta();
        let (tx, ty) = (p.x + dx, p.y + dy);
        let i = (self.objs.iter().enumerate().skip(1)).find_map(|(i, o)| {
            ((o.x, o.y) == (tx, ty)
                && o.gfx.is_some_and(|og| og == g || more.contains(&og))
                && elevations_compatible(p.elevation, o.elevation))
            .then_some(i)
        })?;
        let beyond = terrain.behavior(tx + dx, ty + dy);
        (self.objs[i].is_settled()
            && self.collision_at(terrain, i, dir) == Collision::None
            && !matches!(beyond, Behavior::NonAnimDoor(_))
            && terrain.push_allowed(self.objs[i].local_id, (tx, ty), (tx + dx, ty + dy)))
        .then_some(i)
    }

    /// The player starts a one-cell jump towards `dir` (Gen3 `GetJumpSpecialMovementAction`:
    /// the Surf field effect's jump onto the water). Its end is no step (`update_player`'s
    /// `StepFinished` is for keypad and forced steps); the caller waits for
    /// [`Object::is_settled`].
    pub fn jump_player(&mut self, dir: Dir) {
        let o = &mut self.objs[0];
        o.action = None;
        o.begin(ActionKind::Jump(1), dir);
    }

    /// The player starts a one-cell `walk_slow` towards `dir` (Gen3 `GetWalkSlowMovementAction`
    /// as a held movement: Waterfall's ride up). As [`Objects::jump_player`], its end is no step.
    pub fn walk_player_slow(&mut self, dir: Dir) {
        let o = &mut self.objs[0];
        o.action = None;
        o.begin(ActionKind::Walk(crate::script::Speed::Slow), dir);
    }

    /// The current map.
    pub fn map(&self) -> MapId {
        self.map
    }

    fn set_map(&mut self, id: MapId) {
        self.map = id;
        self.objs[0].map = id;
    }

    /// Sets the player's `previousElevation` on a warp load (the arrival cell's, see
    /// `world::maps::load_map`).
    pub fn reset_player_position_elevation(&mut self, e: u8) {
        self.objs[0].prev_elevation = e;
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
        self.index_on(local, self.map)
    }

    /// Index of map `map`'s object `local` (the player for `PLAYER`, whatever `map`: Gen3
    /// `GetObjectEventIdByLocalIdAndMap`).
    fn index_on(&self, local: u8, map: MapId) -> Option<usize> {
        if local == PLAYER {
            return Some(0);
        }
        self.objs
            .iter()
            .position(|o| o.local_id == local && o.map == map)
    }

    /// The slot of the current map's object `local`, for [`Objects::in_slot`] and
    /// [`Objects::sight_collision`] (valid until objects spawn or despawn).
    pub fn slot(&self, local: u8) -> Option<usize> {
        self.index(local)
    }

    /// The object in slot `i` (from [`Objects::slot`]).
    pub fn in_slot(&self, i: usize) -> &Object {
        &self.objs[i]
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
            self.forced.end_center();
        }
    }

    /// [`Objects::set_pos`] for map `map`'s object `local` (tests: the previous map's).
    #[cfg(test)]
    pub(crate) fn set_pos_on(&mut self, local: u8, map: MapId, x: i16, y: i16) {
        if let Some(o) = (self.objs.iter_mut()).find(|o| o.local_id == local && o.map == map) {
            (o.x, o.y, o.prev) = (x, y, (x, y));
            (o.action, o.moves) = (None, None);
        }
    }

    /// Turns an object at once (Gen3 `ObjectEventTurn`, `turnobject`).
    pub fn face(&mut self, local: u8, dir: Dir) {
        self.face_on(local, self.map, dir);
    }

    /// [`Objects::face`] for map `map`'s object `local` (the previous map's after a crossing).
    pub fn face_on(&mut self, local: u8, map: MapId, dir: Dir) {
        if let Some(o) = self
            .objs
            .iter_mut()
            .find(|o| o.local_id == local && o.map == map)
        {
            o.set_dir(dir);
            o.anim.turn();
        }
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

    /// [`Objects::show`] / [`Objects::hide`] for map `map`'s object (Gen3
    /// `SetObjectInvisibility`).
    pub fn set_visible_on(&mut self, local: u8, map: MapId, visible: bool) {
        if let Some(i) = self.index_on(local, map) {
            self.objs[i].visible = visible;
        }
    }

    pub fn set_movement_type(&mut self, local: u8, movement: MovementType) {
        self.with(local, |o| {
            o.movement = movement;
            o.code = movement_code(movement);
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

    /// [`Objects::is_settled`] for map `map`'s object `local`.
    pub fn is_settled_on(&self, local: u8, map: MapId) -> bool {
        self.objs
            .iter()
            .find(|o| o.local_id == local && o.map == map)
            .is_none_or(Object::is_settled)
    }

    /// Gives `local` a movement script. It starts once the current action has finished.
    pub fn start_moves(&mut self, local: u8, steps: &'static [Step]) {
        self.start_moves_on(local, self.map, steps);
    }

    /// [`Objects::start_moves`] for map `map`'s object (Gen3 `applymovementat`).
    pub fn start_moves_on(&mut self, local: u8, map: MapId, steps: &'static [Step]) {
        if let Some(i) = self.index_on(local, map) {
            self.objs[i].moves = Some(Moves {
                steps,
                digest: crate::script::steps_digest(steps),
                next: 0,
            });
        }
    }

    /// The movement script has finished (true for unknown ids, as Gen3 `waitmovement`).
    pub fn moves_done(&self, local: u8) -> bool {
        self.moves_done_on(local, self.map)
    }

    /// [`Objects::moves_done`] for map `map`'s object (Gen3 `waitmovementat`).
    pub fn moves_done_on(&self, local: u8, map: MapId) -> bool {
        self.index_on(local, map)
            .is_none_or(|i| self.objs[i].moves.is_none())
    }

    /// Collision for object `i` taking one step towards `dir` (Gen3 `GetCollisionAtCoords`).
    /// `Behavior::Blocked` and walls ([`Behavior::walled`]) count as impassable.
    fn collision_at(&self, terrain: &impl Terrain, i: usize, dir: Dir) -> Collision {
        let o = &self.objs[i];
        let (dx, dy) = dir.delta();
        let (x, y) = (o.x + dx, o.y + dy);
        let outside = |home: i16, range: u8, v: i16| {
            range != 0 && (v < home - range as i16 || v > home + range as i16)
        };
        if outside(o.home.0, o.range.0, x) || outside(o.home.1, o.range.1, y) {
            return Collision::OutsideRange;
        }
        let block = terrain.block(x, y);
        let to = terrain.behavior(x, y);
        if block_collision(block) != 0
            || to == Behavior::Blocked
            || Behavior::walled(terrain.behavior(o.x, o.y), to, dir)
        {
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

    /// Gen3 `GetCollisionAtCoords` for the object in slot `i` ([`Objects::slot`]) at (x, y),
    /// looking towards `dir`, with its wander range ignored and without the `Behavior::Blocked`
    /// test (Gen3's collision has no water): what trainer sight checks along its line. Walls
    /// are tested as Gen3 does, against the trainer's own cell, not the previous cell of the
    /// line (`currentMetatileBehavior`).
    pub fn sight_collision(
        &self,
        terrain: &impl Terrain,
        i: usize,
        (x, y): (i16, i16),
        dir: Dir,
    ) -> Collision {
        let o = &self.objs[i];
        let block = terrain.block(x, y);
        if block_collision(block) != 0
            || Behavior::walled(terrain.behavior(o.x, o.y), terrain.behavior(x, y), dir)
        {
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
        self.collision_at(terrain, i, dir)
    }

    /// Holds forced movement off until the player first moves (Gen3
    /// `PLAYER_AVATAR_FLAG_CONTROLLABLE`, see `hold`): the World calls it after a warp load with
    /// whether the player stands on a forced-movement cell.
    pub fn hold_forced(&mut self, on_forced_cell: bool) {
        self.forced = if on_forced_cell {
            ForcedState::Held
        } else {
            ForcedState::Free
        };
    }

    /// Whether forced movement is held off ([`Objects::hold_forced`]).
    pub fn forced_held(&self) -> bool {
        self.forced == ForcedState::Held
    }

    /// Frames the player's walk or run step has played (1 on the frame it starts), while one
    /// plays: what the per-step callbacks time their effects by (`world/puzzle.rs`).
    pub fn player_step_frames(&self) -> Option<u8> {
        self.objs[0]
            .action
            .filter(|a| matches!(a.kind, ActionKind::Walk(_) | ActionKind::Run))
            .map(|a| a.t)
    }

    /// Whether the player is at a step's end on a forced-movement cell (Gen3
    /// `T_TILE_CENTER` with `MetatileBehavior_IsForcedMovementTile`): the frame after a step
    /// ended there, before the next one starts, so `FieldGetPlayerInput` reads no A, START or
    /// held direction this frame. Bumping into an obstacle from such a cell is not a step
    /// (Gen3 `T_NOT_MOVING`): input is read.
    pub fn at_forced_tile_center(&self, terrain: &impl Terrain) -> bool {
        let p = &self.objs[0];
        self.forced == ForcedState::Centered
            && p.is_settled()
            && terrain.behavior(p.x, p.y).is_forced_tile()
    }

    /// The forced step the player takes this frame, if one applies and is free: Gen3
    /// `TryDoMetatileBehaviorForcedMovement` up to the collision test (see [`forced`]).
    fn forced_step(&self, terrain: &impl Terrain) -> Option<(forced::ForcedStep, ForcedHit)> {
        if self.forced == ForcedState::Held {
            return None;
        }
        let p = &self.objs[0];
        let step = forced::forced_step(terrain.behavior(p.x, p.y), p.facing)?;
        let (dx, dy) = step.dir.delta();
        let (tx, ty) = (p.x + dx, p.y + dy);
        // Gen3 `DoForcedMovement` / `CheckForPlayerAvatarCollision`: a ledge is jumped
        // (`PlayerJumpLedge`); a current pushing a surfer onto land ends Surf
        // (`COLLISION_STOP_SURFING`: `CanStopSurfing` starts `CreateStopSurfingTask`).
        if terrain.behavior(tx, ty) == Behavior::Ledge(step.dir) {
            return Some((step, ForcedHit::Ledge));
        }
        match self.collision_at(terrain, 0, step.dir) {
            Collision::None => Some((step, ForcedHit::Free)),
            Collision::ElevationMismatch if self.can_stop_surfing(terrain, tx, ty) => {
                Some((step, ForcedHit::StopSurfing))
            }
            _ => None,
        }
    }

    /// Gen3 `CanStopSurfing`: surfing, the land cell (x, y) is elevation 3 and no object
    /// stands there at elevation 3 (`GetObjectEventIdByPosition(x, y, 3)`).
    fn can_stop_surfing(&self, terrain: &impl Terrain, x: i16, y: i16) -> bool {
        self.surfing
            && block_elevation(terrain.block(x, y)) == 3
            && !(self.objs.iter().skip(1))
                .any(|o| (o.x, o.y) == (x, y) && elevations_compatible(o.elevation, 3))
    }

    /// Player control for one frame, as Gen3 `PlayerStep`: forced movement
    /// ([`forced`]), else `MovePlayerNotOnBike`. Does nothing while the player runs a movement
    /// script (`update` plays it).
    pub fn update_player(&mut self, terrain: &impl Terrain, input: PlayerInput) -> PlayerEvent {
        // Player control runs only when no script does: an approach left over by a trainer
        // script that ended without one is dropped.
        self.cancel_approach();
        // Gen3 `T_TILE_CENTER` ends with this `PlayerStep` (it clears the finished step).
        self.forced.end_center();
        if self.objs[0].moves.is_some() {
            return PlayerEvent::Idle;
        }
        self.objs[0].update_elevation(terrain);
        // Gen3 `TryDoMetatileBehaviorForcedMovement`, every frame before the keypad: a forced
        // step sets `MOVING` and skips the keypad; a blocked one leaves it to the keypad.
        let forced = self.forced_step(terrain);
        let p = &self.objs[0];
        if forced.is_some() {
            self.run_state = RunState::Moving;
        } else {
            // `CheckMovementInputNotOnBike` runs every frame, even while an action plays.
            self.run_state = match input.dir {
                None => RunState::NotMoving,
                Some(d) if d != p.facing && self.run_state != RunState::Moving => {
                    RunState::TurnDirection
                }
                Some(_) => RunState::Moving,
            };
            // `PlayerAllowForcedMovementIfMovingSameDirection`.
            if self.run_state == RunState::Moving && self.forced == ForcedState::Held {
                self.forced = ForcedState::Free;
            }
        }
        if let Some(a) = p.action {
            // `PlayerSetAnimId` ignores input while an action plays.
            let done = self.objs[0].tick();
            let stepped = matches!(
                a.kind,
                ActionKind::Walk(_) | ActionKind::Run | ActionKind::Jump(1..)
            );
            return if done && stepped {
                let (x, y) = self.objs[0].pos();
                // Gen3 `T_TILE_CENTER`, next frame: kept only where it matters.
                if self.forced == ForcedState::Free && terrain.behavior(x, y).is_forced_tile() {
                    self.forced = ForcedState::Centered;
                }
                PlayerEvent::StepFinished { at: (x, y) }
            } else {
                PlayerEvent::Idle
            };
        }
        if p.frozen {
            return PlayerEvent::Idle;
        }
        if let Some((step, hit)) = forced {
            let (dx, dy) = step.dir.delta();
            let (x, y) = (p.x + dx, p.y + dy);
            if hit == ForcedHit::StopSurfing {
                // As the keypad's dismount below: a jump onto the land cell.
                self.surfing = false;
                self.objs[0].begin(ActionKind::Jump(1), step.dir);
                return PlayerEvent::StepStarted {
                    to: (x, y),
                    dir: step.dir,
                };
            }
            let ledge = hit == ForcedHit::Ledge;
            let o = &mut self.objs[0];
            let locked = o.facing_locked;
            o.facing_locked |= step.keep_facing;
            let to = if ledge {
                o.begin(ActionKind::Jump(2), step.dir);
                (x + dx, y + dy)
            } else {
                o.begin(step.kind, step.dir);
                (x, y)
            };
            o.facing_locked = locked;
            if step.pause_anim && !ledge {
                o.anim.pause();
            }
            return PlayerEvent::StepStarted { to, dir: step.dir };
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
        let collision = self.collision_at(terrain, 0, dir);
        if collision == Collision::ElevationMismatch && self.can_stop_surfing(terrain, tx, ty) {
            // Gen3 `CanStopSurfing` / `CreateStopSurfingTask`: off the water, a jump special
            // onto the land cell; the flag clears at once.
            self.surfing = false;
            self.objs[0].begin(ActionKind::Jump(1), dir);
            return PlayerEvent::StepStarted { to: (tx, ty), dir };
        }
        if collision == Collision::Object
            && let Some(i) = self.boulder_ahead(terrain, dir)
        {
            // Gen3 `COLLISION_PUSHED_BOULDER`, `Task_PushBoulder`: the player walks in place
            // slowly while the boulder walks one cell on slowly.
            self.objs[0].begin(ActionKind::InPlace(BUMP_FRAMES), dir);
            self.objs[i].begin(ActionKind::Walk(Speed::Slow), dir);
            let o = &self.objs[i];
            return PlayerEvent::PushedBoulder {
                dir,
                id: o.local_id,
                to: (o.x, o.y),
            };
        }
        if collision != Collision::None {
            self.objs[0].begin(ActionKind::InPlace(BUMP_FRAMES), dir);
            return PlayerEvent::Bumped(dir);
        }
        // Gen3 `CheckForRotatingGatePuzzleCollision` on a free cell: the gate whose arm the
        // step pushes turns, or blocks. A blocked step plays nothing (Emerald leaves
        // `COLLISION_ROTATING_GATE` out of `PlayerNotOnBikeCollide`: no bump, no turn).
        if let Some((gates, o)) = self.gates {
            match crate::gates::step(gates, o, dir, (tx, ty), |x, y| {
                block_collision(terrain.block(x, y))
            }) {
                Some(o2) => self.gates = Some((gates, o2)),
                None => return PlayerEvent::Idle,
            }
        }
        let kind = if self.surfing {
            // Gen3 `PlayerNotOnBikeMoving`: surfing is `PlayerWalkFast`, whatever B does.
            ActionKind::Walk(Speed::Fast)
        } else if input.b_held && input.can_run {
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
        // A movement script's step replaces the finished one (Gen3 `ScriptMovement` clears it).
        if self.objs[0].moves.is_some() {
            self.forced.end_center();
        }
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
        self.update_emotes();
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
            // Gen3 `MovementAction_Emote*`: starts the field effect and ends at once.
            Step::Emote(e) => {
                let (local, map) = (o.local_id, o.map);
                self.start_emote(local, map, e);
            }
        }
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
        // Wave C: only while surfing, so hashes on foot are unchanged.
        if self.surfing {
            h.write(&[0x5F]);
        }
        // Milestone 8: written only while present, so hashes without them are unchanged.
        if !self.emotes.is_empty() {
            self.hash_emotes(h);
        }
        self.hash_approach(h);
        // Forced movement: only while set, so hashes without it are unchanged.
        match self.forced {
            ForcedState::Free => {}
            ForcedState::Held => h.write(&[0xF0]),
            ForcedState::Centered => h.write(&[0xF1]),
        }
    }
}
