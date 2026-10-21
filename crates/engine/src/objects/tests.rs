use super::*;
use crate::Tile;
use crate::data::*;
use crate::script::{Ctx, Emote, Script};
use crate::video::{Cmd, Frame, Obj, ObjShape, layout};
use alloc::vec;

// ---- synthetic terrain ----

/// 16x16 open field; outside is a wall (as Gen3 border blocks).
struct Grid {
    blocks: Vec<u16>,
    behaviors: Vec<Behavior>,
}

const W: i16 = 16;

impl Grid {
    fn open() -> Self {
        Self {
            blocks: vec![0; (W * W) as usize],
            behaviors: vec![Behavior::Normal; (W * W) as usize],
        }
    }
    fn set(&mut self, x: i16, y: i16, block: u16, behavior: Behavior) {
        let i = (y * W + x) as usize;
        self.blocks[i] = block;
        self.behaviors[i] = behavior;
    }
}

impl Terrain for Grid {
    fn block(&self, x: i16, y: i16) -> u16 {
        if (0..W).contains(&x) && (0..W).contains(&y) {
            self.blocks[(y * W + x) as usize]
        } else {
            3 << BLOCK_COLLISION_SHIFT
        }
    }
    fn behavior(&self, x: i16, y: i16) -> Behavior {
        if (0..W).contains(&x) && (0..W).contains(&y) {
            self.behaviors[(y * W + x) as usize]
        } else {
            Behavior::Normal
        }
    }
}

fn hold(d: Dir) -> PlayerInput {
    PlayerInput {
        dir: Some(d),
        b_held: false,
        can_run: true,
    }
}

const NONE: PlayerInput = PlayerInput {
    dir: None,
    b_held: false,
    can_run: true,
};

fn player_at(x: i16, y: i16, facing: Dir) -> Objects {
    let mut o = Objects::new();
    o.set_pos(PLAYER, x, y);
    o.face(PLAYER, facing);
    o
}

fn npc(local_id: u8, x: i16, y: i16, movement: MovementType, range: (u8, u8)) -> ObjectEvent {
    ObjectEvent {
        local_id,
        gfx: Gfx::Fixed(GfxId(1)),
        x,
        y,
        elevation: 0,
        movement,
        range,
        trainer_type: 0,
        trainer_sight: 0,
        script: None,
        hide_flag: None,
    }
}

// ---- player control ----

#[test]
fn tap_turns_in_place_without_moving() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Down);
    assert_eq!(
        o.update_player(&g, hold(Dir::Right)),
        PlayerEvent::Turned(Dir::Right)
    );
    for _ in 0..30 {
        assert_eq!(o.update_player(&g, NONE), PlayerEvent::Idle);
    }
    assert_eq!(o.player().pos(), (5, 5));
    assert_eq!(o.player().facing, Dir::Right);
}

#[test]
fn turn_then_walk_timing() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Down);
    let mut events = Vec::new();
    for _ in 0..40 {
        events.push(o.update_player(&g, hold(Dir::Right)));
    }
    // Frame 0: turn (8 frames); frame 8: step starts; frames 8..24 walk; finishes on frame 23.
    assert_eq!(events[0], PlayerEvent::Turned(Dir::Right));
    assert!(events[1..8].iter().all(|e| *e == PlayerEvent::Idle));
    assert_eq!(
        events[8],
        PlayerEvent::StepStarted {
            to: (6, 5),
            dir: Dir::Right
        }
    );
    assert_eq!(events[23], PlayerEvent::StepFinished { at: (6, 5) });
    // Holding on: the next step starts right away (16 frames per cell, no gap).
    assert_eq!(
        events[24],
        PlayerEvent::StepStarted {
            to: (7, 5),
            dir: Dir::Right
        }
    );
    assert_eq!(events[39], PlayerEvent::StepFinished { at: (7, 5) });
}

#[test]
fn walking_in_the_facing_direction_starts_at_once() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Up);
    assert_eq!(
        o.update_player(&g, hold(Dir::Up)),
        PlayerEvent::StepStarted {
            to: (5, 4),
            dir: Dir::Up
        }
    );
}

#[test]
fn changing_direction_while_moving_does_not_turn_first() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Right);
    for _ in 0..16 {
        o.update_player(&g, hold(Dir::Right));
    }
    assert_eq!(
        o.update_player(&g, hold(Dir::Down)),
        PlayerEvent::StepStarted {
            to: (6, 6),
            dir: Dir::Down
        }
    );
}

#[test]
fn walk_offsets_each_frame() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Right);
    let mut offs = Vec::new();
    for _ in 0..16 {
        o.update_player(&g, hold(Dir::Right));
        offs.push(o.player().offset().0);
        assert_eq!(o.player().pos(), (6, 5), "cell is the destination at once");
    }
    let want: Vec<i16> = (-15..=0).collect();
    assert_eq!(offs, want);
    assert_eq!(o.player().pixel_pos(), (96, 80));
    assert_eq!(o.player().prev, (6, 5));
}

#[test]
fn running_moves_2px_per_frame_only_when_allowed() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Left);
    let run = PlayerInput {
        dir: Some(Dir::Left),
        b_held: true,
        can_run: true,
    };
    let mut offs = Vec::new();
    for _ in 0..8 {
        o.update_player(&g, run);
        offs.push(o.player().offset().0);
    }
    assert_eq!(offs, [14, 12, 10, 8, 6, 4, 2, 0]);
    assert!(o.player().is_settled());
    let walk = PlayerInput {
        can_run: false,
        ..run
    };
    o.update_player(&g, walk);
    assert_eq!(o.player().offset(), (15, 0));
}

#[test]
fn collision_bits_and_blocked_behavior_bump() {
    let mut g = Grid::open();
    g.set(6, 5, 1 << BLOCK_COLLISION_SHIFT, Behavior::Normal);
    g.set(5, 6, 0, Behavior::Blocked);
    let mut o = player_at(5, 5, Dir::Right);
    assert_eq!(
        o.update_player(&g, hold(Dir::Right)),
        PlayerEvent::Bumped(Dir::Right)
    );
    for _ in 1..BUMP_FRAMES {
        assert_eq!(o.update_player(&g, hold(Dir::Right)), PlayerEvent::Idle);
    }
    assert_eq!(o.player().pos(), (5, 5));
    assert!(o.player().is_settled());
    assert_eq!(
        o.update_player(&g, hold(Dir::Right)),
        PlayerEvent::Bumped(Dir::Right)
    );

    let mut o = player_at(5, 5, Dir::Down);
    assert_eq!(
        o.update_player(&g, hold(Dir::Down)),
        PlayerEvent::Bumped(Dir::Down)
    );
    // The map edge is a wall too.
    let mut o = player_at(0, 0, Dir::Up);
    assert_eq!(
        o.update_player(&g, hold(Dir::Up)),
        PlayerEvent::Bumped(Dir::Up)
    );
}

#[test]
fn objects_block_on_both_cells_of_their_step() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Right);
    let ev = npc(1, 7, 5, MovementType::Face(Dir::Down), (0, 0));
    o.add(&ev, |_| 0);
    o.start_moves(1, &[Step::Walk(Dir::Left, Speed::Normal)]);
    let mut rng = Rng::new(1);
    o.update(&g, &mut rng, false); // NPC now walking (7,5) -> (6,5)
    assert_eq!(o.get(1).unwrap().pos(), (6, 5));
    assert_eq!(o.collision(&g, PLAYER, Dir::Right), Collision::Object);
    assert_eq!(
        o.update_player(&g, hold(Dir::Right)),
        PlayerEvent::Bumped(Dir::Right)
    );
    // An NPC still on its previous cell blocks it as well.
    let mut o = player_at(5, 5, Dir::Right);
    o.add(&npc(1, 6, 4, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    o.start_moves(1, &[Step::Walk(Dir::Up, Speed::Normal)]);
    o.update(&g, &mut rng, false);
    assert_eq!(o.get(1).unwrap().prev, (6, 4));
    let mut o2 = player_at(6, 5, Dir::Up);
    o2.add(&npc(1, 6, 4, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    o2.start_moves(1, &[Step::Walk(Dir::Up, Speed::Normal)]);
    o2.update(&g, &mut rng, false);
    assert_eq!(o2.collision(&g, PLAYER, Dir::Up), Collision::Object);
}

#[test]
fn elevation_mismatch_blocks_except_0_and_15() {
    let mut g = Grid::open();
    let elev = |e: u16| e << BLOCK_ELEVATION_SHIFT;
    for x in 0..W {
        for y in 0..W {
            g.set(x, y, elev(3), Behavior::Normal);
        }
    }
    g.set(6, 5, elev(4), Behavior::Normal);
    g.set(4, 5, elev(0), Behavior::Normal);
    g.set(5, 4, elev(15), Behavior::Normal);
    let mut o = player_at(5, 5, Dir::Right);
    assert_eq!(
        o.update_player(&g, hold(Dir::Right)),
        PlayerEvent::Bumped(Dir::Right)
    );
    assert_eq!(o.player().elevation, 3);
    assert_eq!(o.collision(&g, PLAYER, Dir::Left), Collision::None);
    assert_eq!(o.collision(&g, PLAYER, Dir::Up), Collision::None);
    assert_eq!(o.collision(&g, PLAYER, Dir::Down), Collision::None);
    // Elevation 0 objects go anywhere.
    let mut o = player_at(5, 5, Dir::Right);
    o.objs[0].elevation = 0;
    assert_eq!(o.collision(&g, PLAYER, Dir::Right), Collision::None);
}

#[test]
fn ledge_jumps_two_cells() {
    let mut g = Grid::open();
    g.set(5, 6, 1 << BLOCK_COLLISION_SHIFT, Behavior::Ledge(Dir::Down));
    let mut o = player_at(5, 5, Dir::Down);
    assert_eq!(
        o.update_player(&g, hold(Dir::Down)),
        PlayerEvent::StepStarted {
            to: (5, 7),
            dir: Dir::Down
        }
    );
    assert_eq!(o.player().pos(), (5, 6));
    assert_eq!(o.player().jump_y(), -4);
    let mut ys = vec![o.player().jump_y()];
    let mut last = PlayerEvent::Idle;
    for i in 1..32 {
        last = o.update_player(&g, NONE);
        ys.push(o.player().jump_y());
        if i == 15 {
            assert_eq!(o.player().pos(), (5, 7), "second cell at halfway");
            assert_eq!(o.player().prev, (5, 6));
            assert_eq!(o.player().offset(), (0, -16));
        }
    }
    assert_eq!(last, PlayerEvent::StepFinished { at: (5, 7) });
    assert_eq!(ys[..4], [-4, -4, -6, -6]);
    assert_eq!(ys[31], 0);
    assert_eq!(o.player().pixel_pos(), (80, 112));
    // A ledge facing elsewhere is just a wall.
    let mut o = player_at(4, 6, Dir::Right);
    assert_eq!(
        o.update_player(&g, hold(Dir::Right)),
        PlayerEvent::Bumped(Dir::Right)
    );
}

#[test]
fn frozen_player_ignores_input() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Down);
    o.freeze(PLAYER);
    assert_eq!(o.update_player(&g, hold(Dir::Down)), PlayerEvent::Idle);
    assert!(o.player().is_settled());
    o.unfreeze(PLAYER);
    assert!(matches!(
        o.update_player(&g, hold(Dir::Down)),
        PlayerEvent::StepStarted { .. }
    ));
}

#[test]
fn input_from_buttons_prefers_up_down_left_right() {
    let i = PlayerInput::from_buttons(Buttons::LEFT | Buttons::DOWN | Buttons::B, false);
    assert_eq!(i.dir, Some(Dir::Down));
    assert!(i.b_held);
    assert_eq!(PlayerInput::from_buttons(Buttons::NONE, true).dir, None);
}

// ---- NPCs ----

fn run_wander(seed: u32, frames: u32) -> (Objects, Vec<(i16, i16)>) {
    let g = Grid::open();
    let mut o = player_at(0, 0, Dir::Down);
    o.add(&npc(1, 8, 8, MovementType::Wander, (2, 1)), |_| 0);
    o.add(
        &npc(2, 4, 10, MovementType::WanderHorizontal, (1, 1)),
        |_| 0,
    );
    let mut rng = Rng::new(seed);
    let mut cells = Vec::new();
    for _ in 0..frames {
        o.update(&g, &mut rng, false);
        let n = o.get(1).unwrap();
        cells.push(n.pos());
        assert!(
            (6..=10).contains(&n.x) && (7..=9).contains(&n.y),
            "{:?}",
            n.pos()
        );
        let h = o.get(2).unwrap();
        assert!((3..=5).contains(&h.x) && h.y == 10, "{:?}", h.pos());
    }
    (o, cells)
}

#[test]
fn wander_stays_in_range_and_is_deterministic() {
    let (a, cells) = run_wander(42, 5000);
    let (b, _) = run_wander(42, 5000);
    let (c, _) = run_wander(43, 5000);
    let hash = |o: &Objects| {
        let mut h = Fnv64::new();
        o.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&a), hash(&b));
    assert_ne!(hash(&a), hash(&c));
    let mut distinct = cells.clone();
    distinct.sort();
    distinct.dedup();
    assert!(distinct.len() > 3, "it wanders: {distinct:?}");
}

#[test]
fn wander_timing_follows_gen3() {
    let g = Grid::open();
    let mut o = player_at(0, 0, Dir::Down);
    o.add(&npc(1, 8, 8, MovementType::Wander, (0, 0)), |_| 0);
    let mut rng = Rng::new(7);
    let mut model = Rng::new(7);
    o.update(&g, &mut rng, false); // frame 0: face + delay
    let delay = DELAYS_MEDIUM[(model.next_u32() >> 16) as usize % 4];
    assert_eq!(o.get(1).unwrap().idle, Idle::Wait(delay));
    for _ in 1..delay {
        o.update(&g, &mut rng, false);
        assert!(o.get(1).unwrap().is_settled());
    }
    o.update(&g, &mut rng, false); // delay over: direction picked, walk starts
    let d = Dir::ALL[(model.next_u32() >> 16) as usize & 3];
    let n = o.get(1).unwrap();
    assert_eq!(n.facing, d);
    let (dx, dy) = d.delta();
    assert_eq!(n.pos(), (8 + dx, 8 + dy));
    assert!(!n.is_settled());
}

#[test]
fn npcs_never_walk_onto_the_player() {
    let g = Grid::open();
    let mut o = player_at(5, 4, Dir::Down);
    // Boxed in: player above, range 1 around (5, 5) on x only, walls of objects around.
    o.add(&npc(1, 5, 5, MovementType::Wander, (0, 0)), |_| 0);
    o.add(&npc(2, 4, 5, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    o.add(&npc(3, 6, 5, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    o.add(&npc(4, 5, 6, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    let mut rng = Rng::new(3);
    for _ in 0..3000 {
        o.update(&g, &mut rng, false);
        assert_eq!(o.get(1).unwrap().pos(), (5, 5));
    }
}

#[test]
fn frozen_npcs_do_nothing() {
    let g = Grid::open();
    let mut o = player_at(0, 0, Dir::Down);
    o.add(&npc(1, 8, 8, MovementType::Wander, (3, 3)), |_| 0);
    o.freeze_all();
    let mut rng = Rng::new(9);
    for _ in 0..2000 {
        o.update(&g, &mut rng, false);
    }
    assert_eq!(o.get(1).unwrap().pos(), (8, 8));
    assert_eq!(rng, Rng::new(9), "no random draw while frozen");
}

#[test]
fn look_around_and_walk_in_place() {
    let g = Grid::open();
    let mut o = player_at(0, 0, Dir::Down);
    o.add(&npc(1, 8, 8, MovementType::LookAround, (0, 0)), |_| 0);
    o.add(
        &npc(2, 3, 3, MovementType::WalkInPlace(Dir::Left), (0, 0)),
        |_| 0,
    );
    o.add(&npc(3, 9, 9, MovementType::Invisible, (0, 0)), |_| 0);
    let mut rng = Rng::new(5);
    let mut facings = Vec::new();
    for _ in 0..3000 {
        o.update(&g, &mut rng, false);
        facings.push(o.get(1).unwrap().facing);
        assert_eq!(o.get(1).unwrap().pos(), (8, 8));
        assert_eq!(o.get(2).unwrap().pos(), (3, 3));
    }
    facings.sort();
    facings.dedup();
    assert!(facings.len() >= 3);
    assert_eq!(o.get(2).unwrap().facing, Dir::Left);
    assert!(!o.get(2).unwrap().is_settled(), "always walking in place");
    assert!(!o.get(3).unwrap().visible);
}

// ---- scripted movement ----

/// Frames until `moves_done`, running `steps` on NPC 1 at (5, 5) with the player at (5, 8).
fn script_frames(steps: &'static [Step]) -> (u32, Objects) {
    let g = Grid::open();
    let mut o = player_at(5, 8, Dir::Up);
    o.add(&npc(1, 5, 5, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    let mut rng = Rng::new(1);
    o.update(&g, &mut rng, false); // settle the idle facing first
    o.start_moves(1, steps);
    let mut frames = 0;
    while !o.moves_done(1) {
        o.update(&g, &mut rng, false);
        frames += 1;
        assert!(frames < 1000);
    }
    (frames, o)
}

#[test]
fn every_step_kind() {
    use Speed::*;
    let walk = |s, f, cell| {
        let steps: &'static [Step] =
            alloc::boxed::Box::leak(vec![Step::Walk(Dir::Right, s)].into_boxed_slice());
        let (frames, o) = script_frames(steps);
        assert_eq!(frames, f, "{s:?}");
        assert_eq!(o.get(1).unwrap().pos(), cell);
    };
    walk(Slow, 31, (6, 5));
    walk(Normal, 16, (6, 5));
    walk(Fast, 8, (6, 5));
    walk(Faster, 4, (6, 5));
    for (s, f) in [(Slow, 32), (Normal, 16), (Fast, 8), (Faster, 4)] {
        let steps: &'static [Step] =
            alloc::boxed::Box::leak(vec![Step::WalkInPlace(Dir::Up, s)].into_boxed_slice());
        let (frames, o) = script_frames(steps);
        assert_eq!(frames, f);
        assert_eq!(o.get(1).unwrap().pos(), (5, 5));
        assert_eq!(o.get(1).unwrap().facing, Dir::Up);
    }
    for (n, f, cell) in [(0, 16, (5, 5)), (1, 16, (6, 5)), (2, 32, (7, 5))] {
        let steps: &'static [Step] =
            alloc::boxed::Box::leak(vec![Step::Jump(Dir::Right, n)].into_boxed_slice());
        let (frames, o) = script_frames(steps);
        assert_eq!((frames, o.get(1).unwrap().pos()), (f, cell), "jump {n}");
    }
    let (frames, _) = script_frames(&[Step::Delay(5), Step::Delay(0)]);
    assert_eq!(frames, 6);

    let (frames, o) = script_frames(&[Step::Face(Dir::Left)]);
    assert_eq!((frames, o.get(1).unwrap().facing), (1, Dir::Left));
    let (_, o) = script_frames(&[Step::Face(Dir::Left), Step::FacePlayer]);
    assert_eq!(o.get(1).unwrap().facing, Dir::Down, "player is below");
    let (_, o) = script_frames(&[Step::FaceAwayFromPlayer]);
    assert_eq!(o.get(1).unwrap().facing, Dir::Up);

    let (frames, o) = script_frames(&[
        Step::LockFacing,
        Step::Walk(Dir::Right, Normal),
        Step::Face(Dir::Up),
    ]);
    assert_eq!(frames, 18);
    assert_eq!(o.get(1).unwrap().facing, Dir::Down, "facing locked");
    assert_eq!(o.get(1).unwrap().pos(), (6, 5));
    let (_, o) = script_frames(&[Step::LockFacing, Step::UnlockFacing, Step::Face(Dir::Up)]);
    assert_eq!(o.get(1).unwrap().facing, Dir::Up);

    let (_, o) = script_frames(&[Step::Hide]);
    assert!(!o.get(1).unwrap().visible);
    let (_, o) = script_frames(&[Step::Hide, Step::Show]);
    assert!(o.get(1).unwrap().visible);
    let (frames, _) = script_frames(&[Step::Emote(Emote::Exclamation)]);
    assert_eq!(frames, 1);
}

#[test]
fn moves_wait_for_the_current_action_and_run_back_to_back() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Right);
    let mut rng = Rng::new(1);
    o.update_player(&g, hold(Dir::Right)); // mid-step to (6, 5)
    o.update(&g, &mut rng, true);
    o.start_moves(
        PLAYER,
        &[
            Step::Walk(Dir::Down, Speed::Normal),
            Step::Walk(Dir::Down, Speed::Normal),
        ],
    );
    let mut frames = 0;
    while !o.moves_done(PLAYER) {
        assert_eq!(
            o.update_player(&g, hold(Dir::Up)),
            PlayerEvent::Idle,
            "script owns the player"
        );
        o.update(&g, &mut rng, true);
        frames += 1;
    }
    assert_eq!(frames, 15 + 32);
    assert_eq!(o.player().pos(), (6, 7));
    assert!(o.moves_done(42), "unknown ids are done");
}

#[test]
fn object_commands() {
    let mut o = player_at(5, 5, Dir::Down);
    o.add(&npc(1, 2, 3, MovementType::Wander, (1, 1)), |_| 0);
    o.set_pos(1, 9, 9);
    assert_eq!(o.get(1).unwrap().pos(), (9, 9));
    assert_eq!(o.direction_to_player(1), Some(Dir::Left));
    o.face(1, Dir::Up);
    assert_eq!(o.get(1).unwrap().facing, Dir::Up);
    o.hide(1);
    assert!(!o.get(1).unwrap().visible);
    o.show(1);
    assert!(o.get(1).unwrap().visible);
    o.set_movement_type(1, MovementType::Face(Dir::Left));
    let mut rng = Rng::new(1);
    o.update(&Grid::open(), &mut rng, false);
    assert_eq!(o.get(1).unwrap().facing, Dir::Left);
    assert_eq!(o.at(9, 9, 0), Some(1));
    o.shift(-2, 1);
    assert_eq!(o.get(1).unwrap().pos(), (7, 10));
    assert_eq!(o.player().pos(), (3, 6));
    o.remove(1);
    o.remove(PLAYER);
    assert_eq!(o.iter().count(), 1);
}

#[test]
fn direction_to_prefers_horizontal() {
    assert_eq!(direction_to((5, 5), (3, 9)), Dir::Left);
    assert_eq!(direction_to((5, 5), (6, 0)), Dir::Right);
    assert_eq!(direction_to((5, 5), (5, 0)), Dir::Up);
    assert_eq!(direction_to((5, 5), (5, 5)), Dir::Down);
}

// ---- spawning ----

static TS: Tileset = Tileset {
    tiles: &[],
    palettes: &[],
    metatiles: &[],
    attributes: &[],
};

static OBJS: [ObjectEvent; 3] = [
    ObjectEvent {
        local_id: 1,
        gfx: Gfx::Fixed(GfxId(1)),
        x: 1,
        y: 2,
        elevation: 3,
        movement: MovementType::WanderVertical,
        range: (0, 2),
        trainer_type: 0,
        trainer_sight: 0,
        script: None,
        hide_flag: Some(FlagId(7)),
    },
    ObjectEvent {
        local_id: 2,
        gfx: Gfx::Var(VarId(0x4010)),
        x: 4,
        y: 5,
        elevation: 3,
        movement: MovementType::WanderHorizontal,
        range: (1, 0),
        trainer_type: 0,
        trainer_sight: 0,
        script: None,
        hide_flag: Some(FlagId(8)),
    },
    ObjectEvent {
        local_id: 3,
        gfx: Gfx::Fixed(GfxId(2)),
        x: 6,
        y: 7,
        elevation: 0,
        movement: MovementType::Invisible,
        range: (0, 0),
        trainer_type: 0,
        trainer_sight: 0,
        script: None,
        hide_flag: None,
    },
];

static MAP: MapData = MapData {
    name: "test",
    width: 16,
    height: 16,
    blocks: &[],
    border: [0; 4],
    primary: &TS,
    secondary: &TS,
    objects: &OBJS,
    warps: &[],
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: MapScripts {
        on_load: None,
        on_transition: None,
        on_resume: None,
        on_return_to_field: None,
        on_frame: &[],
        on_warp_into: &[],
    },
    allow_running: true,
};

#[test]
fn spawn_map_skips_hidden_resolves_vars_keeps_player() {
    let mut o = player_at(3, 3, Dir::Left);
    o.add(&npc(9, 0, 0, MovementType::Wander, (0, 0)), |_| 0);
    o.spawn_map(
        MapId(5),
        &MAP,
        |f| f == FlagId(7),
        |v| if v == VarId(0x4010) { 42 } else { 0 },
    );
    let ids: Vec<u8> = o.iter().map(|x| x.local_id).collect();
    assert_eq!(ids, [PLAYER, 2, 3]);
    assert_eq!(o.player().pos(), (3, 3));
    assert_eq!(o.player().facing, Dir::Left);
    let n = o.get(2).unwrap();
    assert_eq!(n.gfx, Some(GfxId(42)));
    assert_eq!((n.elevation, n.range, n.home), (3, (1, 0), (4, 5)));
    assert_eq!(n.facing, Dir::Left, "WanderHorizontal starts facing west");
    assert!(!o.get(3).unwrap().visible);
}

// ---- sprites ----

const fn tile(v: u32) -> Tile {
    [v; 8]
}

static P0: [Tile; 8] = [tile(0); 8];
static P1: [Tile; 8] = [tile(1); 8];
static P2: [Tile; 8] = [tile(2); 8];
static P3: [Tile; 8] = [tile(3); 8];
static P4: [Tile; 8] = [tile(4); 8];
static P5: [Tile; 8] = [tile(5); 8];
static P6: [Tile; 8] = [tile(6); 8];
static P7: [Tile; 8] = [tile(7); 8];
static P8: [Tile; 8] = [tile(8); 8];
static PLAYER_PAL: [u16; 16] = [1; 16];
static NPC_PAL: [u16; 16] = [2; 16];
/// 16x32, 9 frames.
static PLAYER_SPRITE: SpriteData = SpriteData {
    width: 16,
    height: 32,
    frames: &[&P0, &P1, &P2, &P3, &P4, &P5, &P6, &P7, &P8],
    palette: &PLAYER_PAL,
};
static N0: [Tile; 4] = [tile(10); 4];
static N1: [Tile; 4] = [tile(11); 4];
static N2: [Tile; 4] = [tile(12); 4];
/// 16x16, 3 frames: facing frames only.
static NPC_SPRITE: SpriteData = SpriteData {
    width: 16,
    height: 16,
    frames: &[&N0, &N1, &N2],
    palette: &NPC_PAL,
};
static M0: [Tile; 4] = [tile(20); 4];
/// Same size and palette as `NPC_SPRITE`, other pictures.
static OTHER_SPRITE: SpriteData = SpriteData {
    width: 16,
    height: 16,
    frames: &[&M0, &M0, &M0],
    palette: &NPC_PAL,
};

struct Game;

impl GameData for Game {
    fn map(&self, _: MapId) -> &'static MapData {
        &MAP
    }
    fn sprite(&self, g: GfxId) -> Option<&'static SpriteData> {
        match g {
            GfxId(1) => Some(&NPC_SPRITE),
            GfxId(3) => Some(&OTHER_SPRITE),
            _ => None,
        }
    }
    fn player_sprite(&self) -> &'static SpriteData {
        &PLAYER_SPRITE
    }
    fn font(&self) -> &'static Font {
        unimplemented!()
    }
    fn window(&self) -> &'static WindowGfx {
        unimplemented!()
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[0xFF]
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        alloc::boxed::Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        unimplemented!()
    }
}

fn camera_on(o: &Objects) -> (i32, i32) {
    let (x, y) = o.player().pixel_pos();
    (x - 112, y - 64)
}

fn uploads(f: &Frame) -> Vec<(u16, u32)> {
    f.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::ObjTiles { first, tiles } => Some((*first, tiles[0][0])),
            _ => None,
        })
        .collect()
}

#[test]
fn obj_output_for_a_known_state() {
    let mut o = player_at(10, 10, Dir::Right);
    o.add(&npc(1, 11, 9, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    o.add(&npc(2, 9, 12, MovementType::Face(Dir::Left), (0, 0)), |_| 0);
    o.add(
        &npc(3, 30, 30, MovementType::Face(Dir::Down), (0, 0)),
        |_| 0,
    ); // off-screen
    o.add(
        &npc(4, 10, 11, MovementType::Face(Dir::Down), (0, 0)),
        |_| 0,
    );
    o.objs[4].gfx = Some(GfxId(99)); // no sprite: skipped
    o.update(&Grid::open(), &mut Rng::new(1), false);
    let mut f = Frame::default();
    let cam = camera_on(&o);
    o.draw(&Game, cam, &mut f);
    let player = Obj {
        x: 112,
        y: 48,
        tile: 0,
        palette: 0,
        shape: ObjShape::Tall,
        size: 2,
        hflip: true,
        vflip: false,
        priority: layout::OBJ_PRIORITY,
    };
    let below = Obj {
        x: 96,
        y: 96,
        tile: 12,
        palette: 1,
        shape: ObjShape::Square,
        size: 1,
        hflip: false,
        vflip: false,
        priority: layout::OBJ_PRIORITY,
    };
    let above = Obj {
        x: 128,
        y: 48,
        tile: 8,
        palette: 1,
        shape: ObjShape::Square,
        size: 1,
        hflip: false,
        vflip: false,
        priority: layout::OBJ_PRIORITY,
    };
    // Lower on screen is in front (lower OAM index).
    assert_eq!(f.objs, [below, player, above]);
    assert_eq!(uploads(&f), [(0, 2), (8, 10), (12, 12)]);
    let pals: Vec<_> = f
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::ObjPalette { bank, colors } => Some((*bank, colors[0])),
            _ => None,
        })
        .collect();
    assert_eq!(pals, [(0, 1), (1, 2)], "one bank per distinct palette");
}

#[test]
fn vram_upload_only_on_frame_change() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Down);
    let mut f = Frame::default();
    o.draw(&Game, camera_on(&o), &mut f);
    assert_eq!(uploads(&f), [(0, 0)]);
    assert_eq!(f.cmds.len(), 2, "tiles + palette");
    let mut f = Frame::default();
    o.draw(&Game, camera_on(&o), &mut f);
    assert!(f.cmds.is_empty());
    assert_eq!(f.objs.len(), 1);

    // Walking down: step frame 3 for 8 frames, then stand frame 0; next step frame 4.
    let mut shown = Vec::new();
    for _ in 0..32 {
        o.update_player(&g, hold(Dir::Down));
        let mut f = Frame::default();
        o.draw(&Game, camera_on(&o), &mut f);
        assert_eq!(f.objs[0].y, 48, "camera follows the player");
        shown.push(uploads(&f).first().map(|u| u.1));
    }
    let mut want = vec![None; 32];
    want[0] = Some(3);
    want[8] = Some(0);
    want[16] = Some(4);
    want[24] = Some(0);
    assert_eq!(shown, want);
}

#[test]
fn turning_shows_a_step_frame_then_stands() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Down);
    let mut frames = Vec::new();
    for i in 0..9 {
        o.update_player(&g, if i == 0 { hold(Dir::Up) } else { NONE });
        frames.push(o.player().anim.frame(Dir::Up, 9).0);
    }
    // walk_in_place_fast: 4 frames of step, 4 of stand, then idle stand.
    assert_eq!(frames, [5, 5, 5, 5, 1, 1, 1, 1, 1]);
}

#[test]
fn run_frames_come_from_the_18_frame_table() {
    let mut a = Anim::new();
    a.start(anim::Table::Run);
    a.tick();
    assert_eq!(a.frame(Dir::Down, 18), (12, false));
    assert_eq!(a.frame(Dir::Right, 18), (16, true));
    assert_eq!(
        a.frame(Dir::Down, 9),
        (3, false),
        "no run frames: walk frames"
    );
    assert_eq!(a.frame(Dir::Right, 3), (2, true), "facing frame only");
    assert_eq!(a.frame(Dir::Up, 1), (0, false));
    for _ in 0..5 {
        a.tick();
    }
    assert_eq!(
        a.frame(Dir::Down, 18),
        (9, false),
        "5 frames of step, then stand"
    );
}

#[test]
fn palette_banks_are_reused_and_freed() {
    let mut o = player_at(5, 5, Dir::Down);
    let mut f = Frame::default();
    o.banks = [Some([9; 16]); 16];
    o.draw(&Game, camera_on(&o), &mut f);
    // All banks taken by stale colors: the lowest one unused this frame is recycled.
    assert!(f.cmds.contains(&Cmd::ObjPalette {
        bank: 0,
        colors: PLAYER_PAL
    }));
    o.invalidate_vram();
    let mut f = Frame::default();
    o.draw(&Game, camera_on(&o), &mut f);
    assert_eq!(f.cmds.len(), 2, "everything uploaded again");
}

#[test]
fn hash_tracks_state() {
    let hash = |o: &Objects| {
        let mut h = Fnv64::new();
        o.hash(&mut h);
        h.finish()
    };
    let a = player_at(5, 5, Dir::Down);
    let mut b = player_at(5, 5, Dir::Down);
    assert_eq!(hash(&a), hash(&b));
    b.update_player(&Grid::open(), hold(Dir::Down));
    assert_ne!(hash(&a), hash(&b));
}

// ---- follow-ups: decoupled player update, connections, render cache ----

#[test]
fn skipping_update_on_one_frame_does_not_skip_the_player_later() {
    let g = Grid::open();
    let mut o = player_at(5, 5, Dir::Right);
    let mut rng = Rng::new(1);
    o.update_player(&g, hold(Dir::Right)); // walk starts; `update` skipped this frame
    assert_eq!(o.player().offset(), (-15, 0));
    // Next frame no player control (e.g. a script started): `update` plays the step.
    o.update(&g, &mut rng, false);
    assert_eq!(o.player().offset(), (-14, 0));
    o.update_player(&g, NONE);
    o.update(&g, &mut rng, true);
    assert_eq!(o.player().offset(), (-13, 0), "advanced once, not twice");
}

#[test]
fn entering_a_connected_map_keeps_old_objects_in_view() {
    let g = Grid::open();
    let mut o = Objects::new();
    let old = MapId(1);
    o.spawn_map(old, &MAP, |_| true, |_| 0); // every MAP object hidden: just the player
    o.set_pos(PLAYER, 5, 2);
    o.add(&npc(2, 5, 4, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    o.add(&npc(7, 5, 15, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    // The new map lies north: old coordinates move down by 8.
    o.enter_connected(MapId(5), &MAP, (0, 8), |_| false, |_| 0);
    assert_eq!(o.map(), MapId(5));
    assert_eq!(o.player().pos(), (5, 10));
    assert_eq!(o.player().map, MapId(5));
    let all: Vec<(MapId, u8)> = o.iter().map(|x| (x.map, x.local_id)).collect();
    assert_eq!(
        all,
        [
            (MapId(5), PLAYER),
            (old, 3), // unflagged MAP object at (6, 7), now (6, 15)
            (old, 2),
            (MapId(5), 1),
            (MapId(5), 2),
            (MapId(5), 3)
        ],
        "old NPC 7 (13 cells below) despawned at once"
    );
    // Local ids address the current map; the old NPC 2 still blocks.
    assert_eq!(o.get(2).unwrap().pos(), (4, 5));
    assert_eq!(o.at(5, 12, 0), None);
    o.set_pos(PLAYER, 5, 11);
    assert_eq!(o.collision(&g, PLAYER, Dir::Down), Collision::Object);
    o.remove(2);
    assert_eq!(
        o.iter().filter(|x| x.local_id == 2).count(),
        1,
        "old one stays"
    );
    // Walk away: once out of view it goes.
    o.set_pos(PLAYER, 5, 3);
    o.update(&g, &mut Rng::new(1), false);
    assert_eq!(o.iter().filter(|x| x.local_id == 2).count(), 1);
    o.set_pos(PLAYER, 5, 2);
    o.update(&g, &mut Rng::new(1), false);
    assert!(o.iter().all(|x| x.map == MapId(5)));
}

#[test]
fn at_honours_elevation() {
    let mut o = player_at(5, 5, Dir::Down);
    let mut ev = npc(1, 5, 6, MovementType::Face(Dir::Down), (0, 0));
    ev.elevation = 3;
    o.add(&ev, |_| 0);
    assert_eq!(o.at(5, 6, 3), Some(1));
    assert_eq!(o.at(5, 6, 0), Some(1));
    assert_eq!(o.at(5, 6, 4), None);
}

#[test]
fn hash_ignores_the_render_cache() {
    let hash = |o: &Objects| {
        let mut h = Fnv64::new();
        o.hash(&mut h);
        h.finish()
    };
    let mut o = player_at(5, 5, Dir::Down);
    let before = hash(&o);
    let mut f = Frame::default();
    o.draw(&Game, camera_on(&o), &mut f);
    assert_eq!(hash(&o), before);
    o.invalidate_vram();
    assert_eq!(hash(&o), before);
}

#[test]
fn a_17th_palette_skips_the_object() {
    let mut o = Objects::new();
    o.banks = core::array::from_fn(|b| Some([b as u16 + 100; 16]));
    let mut f = Frame::default();
    let mut used = 0xFFFF;
    assert_eq!(o.palette_bank(&[7; 16], &mut used, &mut f), None);
    assert!(f.cmds.is_empty(), "no bank clobbered");
    let mut used = 0xFFFE;
    assert_eq!(o.palette_bank(&[7; 16], &mut used, &mut f), Some(0));
}

#[test]
fn a_new_sprite_is_uploaded_even_on_the_same_frame_index() {
    let mut o = player_at(5, 5, Dir::Down);
    o.add(&npc(1, 6, 5, MovementType::Face(Dir::Down), (0, 0)), |_| 0);
    let mut f = Frame::default();
    o.draw(&Game, camera_on(&o), &mut f);
    o.objs[1].gfx = Some(GfxId(3));
    let mut f = Frame::default();
    o.draw(&Game, camera_on(&o), &mut f);
    assert_eq!(uploads(&f), [(8, 20)]);
}

#[test]
fn wander_vertical_picks_south_then_north_as_gen3() {
    let g = Grid::open();
    let mut o = player_at(0, 0, Dir::Down);
    o.add(&npc(1, 8, 8, MovementType::WanderVertical, (0, 0)), |_| 0);
    let mut rng = Rng::new(11);
    let mut model = Rng::new(11);
    let delay = DELAYS_MEDIUM[(model.next_u32() >> 16) as usize % 4];
    for _ in 0..=delay {
        o.update(&g, &mut rng, false);
    }
    let d = [Dir::Down, Dir::Up][(model.next_u32() >> 16) as usize & 1];
    assert_eq!(o.get(1).unwrap().facing, d);
}
