//! Wave F: the rotating tile puzzle (`Request::RotatingTiles*`, Gen3 `rotating_tile_puzzle.c`).

use super::*;

/// The synthetic puzzle's yellow right arrow; its colours are rows of 8 from there.
const START: u16 = 8;
/// Arrow `arrow` (0 right, 1 down, 2 left, 3 up) of colour `colour`.
const fn arrow(colour: u16, arrow: u16) -> u16 {
    START + colour * 8 + arrow
}

/// A floor switch's script (`MossdeepCity_Gym_EventScript_*FloorSwitch`).
fn switch(w: &mut World, colour: u16) {
    start(w, move |ctx| {
        Box::pin(async move {
            ctx.rotating_tiles_init(START, START).await;
            ctx.rotating_tiles_move(colour).await;
            ctx.wait_move(0).await;
            ctx.rotating_tiles_turn().await;
            ctx.wait_move(0).await;
            ctx.rotating_tiles_free().await;
        })
    });
    until_done(w, 120);
}

fn set_metatile(w: &mut World, x: i16, y: i16, metatile: u16) {
    start(w, move |ctx| {
        Box::pin(async move { ctx.set_metatile(x, y, metatile, false).await })
    });
    until_done(w, 3);
}

fn template(w: &World, local: u8) -> ObjectEvent {
    *w.s.templates.iter().find(|t| t.local_id == local).unwrap()
}

#[test]
fn a_switch_moves_its_colours_objects_along_the_arrows_then_turns_them() {
    let mut w = started();
    // Object 1 (facing left) on a green down arrow, whose next cell is a green right arrow;
    // object 3 on a yellow arrow (another colour).
    set_metatile(&mut w, 6, 5, arrow(2, 1));
    set_metatile(&mut w, 6, 6, arrow(2, 0));
    set_metatile(&mut w, 2, 5, arrow(0, 0));
    switch(&mut w, 2);
    let o = w.objects().get(1).unwrap();
    assert_eq!(o.pos(), (6, 6), "one cell along its arrow");
    // From the down arrow (1) to the right one (0): counterclockwise, left -> down.
    assert_eq!(o.facing, Dir::Down);
    let t = template(&w, 1);
    assert_eq!((t.x, t.y), (6, 6));
    assert_eq!(t.movement, MovementType::Face(Dir::Down));
    assert_eq!(
        w.objects().get(3).unwrap().pos(),
        (2, 5),
        "yellow did not move"
    );
    assert_eq!(w.s.last_mover, 1, "the waitmovement 0 object");
    assert!(w.s.rotating_tiles.is_none(), "freed");
    // Yellow: object 3 walks right, its facing locked (it does not turn to the walk).
    switch(&mut w, 0);
    let o = w.objects().get(3).unwrap();
    assert_eq!(o.pos(), (3, 5));
    // Off the puzzle now (a plain cell): no turn.
    assert_eq!(o.facing, Dir::Left);
    assert_eq!(template(&w, 3).movement, MovementType::Face(Dir::Left));
}

#[test]
fn a_switch_with_no_object_on_its_colour_moves_nothing() {
    let mut w = started();
    switch(&mut w, 4);
    assert_eq!(w.objects().get(1).unwrap().pos(), (6, 5));
    assert_eq!(w.s.last_mover, 0);
}

#[test]
fn gen3_rotation_is_counterclockwise_for_every_one_step_move() {
    use crate::world::host::rotating_tiles::{clockwise, counterclockwise, rotation};
    // The puzzles turn counterclockwise: 0 -> 3 (right to up) ... (`tileDifference` -1 or 3).
    for prev in 0..4u8 {
        let now = (prev + 3) % 4;
        assert_eq!(rotation(now, prev), Some(true), "{prev} -> {now}");
    }
    assert_eq!(rotation(2, 1), Some(false));
    assert_eq!(rotation(1, 1), None);
    for d in [Dir::Right, Dir::Down, Dir::Left, Dir::Up] {
        assert_eq!(clockwise(counterclockwise(d)), d);
    }
}

/// The town, with a pad (warp 0) to the town's own warp 0.
static PAD_TO_TOWN: MapData = MapData {
    name: "pad",
    warps: &[WarpEvent {
        x: 1,
        y: 1,
        elevation: 0,
        dest_map: TOWN,
        dest_warp: 0,
    }],
    width: 10,
    height: 10,
    blocks: &TOWN_BLOCKS,
    border: [WALL; 4],
    primary: &TS,
    secondary: &TS2,
    objects: &[],
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: false,
    map_type: 0,
};

fn finish_warp(w: &mut World) {
    assert!(w.transition_active(), "a warp started");
    for _ in 0..200 {
        if !w.transition_active() {
            return;
        }
        run(w, Buttons::NONE, 1);
    }
    panic!("the warp never ended");
}

#[test]
fn a_pad_warp_keeps_the_maps_templates_a_plain_warp_reloads_them() {
    let mut w = started();
    w.s.templates.edit(1, |t| t.x = 7);
    // Gen3 `DoMossdeepGymWarp` (`SKIP_OBJECT_EVENT_LOAD`): the objects respawn from the moved
    // templates.
    w.start_pad_warp(&Game, &PAD_TO_TOWN, 0);
    finish_warp(&mut w);
    assert_eq!(w.map(), TOWN);
    assert_eq!((template(&w, 1).x, template(&w, 1).y), (7, 5));
    assert_eq!(w.objects().get(1).unwrap().pos(), (7, 5));
    // A plain warp loads the header's templates (`LoadObjEventTemplatesFromHeader`).
    w.start_warp(&Game, &PAD_TO_TOWN, 0, false);
    finish_warp(&mut w);
    assert_eq!((template(&w, 1).x, template(&w, 1).y), (6, 5));
    assert_eq!(w.objects().get(1).unwrap().pos(), (6, 5));
}
