//! `special CableCar`: the ride scene is on screen before the warp.

use super::{HOUSE, run, start, started};
use crate::cable_car::{RIDE_FRAMES, pose};
use crate::input::Buttons;
use crate::script::{Fade, Request};
use crate::video::ObjShape;
use alloc::boxed::Box;

#[test]
fn the_car_moves_with_the_ride_and_arrival_is_the_fade() {
    let up_start = pose(false, RIDE_FRAMES);
    let up_mid = pose(false, RIDE_FRAMES - 199);
    assert!(up_mid.car.0 < up_start.car.0 && up_mid.car.1 < up_start.car.1);
    assert_ne!(up_start.scroll, up_mid.scroll);
    assert_eq!(up_mid.brightness, 0);
    let arrival = pose(false, RIDE_FRAMES + 1 - 586);
    assert!(arrival.brightness >= 8);
    assert_eq!(pose(false, 1).brightness, 16);
    let down_start = pose(true, RIDE_FRAMES);
    let down_mid = pose(true, RIDE_FRAMES - 199);
    assert!(down_mid.car.0 > down_start.car.0 && down_mid.car.1 > down_start.car.1);
}

/// The car is the only 64×64 sprite. The door is 16×8 and each cable link is 16×16.
fn car(f: &crate::video::Frame) -> Option<(i16, i16)> {
    f.objs
        .iter()
        .find(|o| o.shape == ObjShape::Square && o.size == 3)
        .map(|o| (o.x, o.y))
}

#[test]
fn cable_car_scene_is_up_before_the_warp() {
    let mut w = started();
    let from = w.map();
    start(&mut w, |ctx| {
        Box::pin(async move {
            // `SPECIAL_CableCar`: the field fades out, the ride plays, then the warp.
            ctx.fade(Fade::ToBlack).await;
            ctx.cable_car(false).await;
            ctx.warp(HOUSE, None, 6, 4).await;
        })
    });
    let mut scene = false;
    let mut car_moved = false;
    let mut view = false;
    let mut riding = false;
    let mut arrival = false;
    let mut scroll = None;
    let mut car_at = None;
    for _ in 0..700 {
        let f = run(&mut w, Buttons::NONE, 1);
        if w.transition_active() || w.map() != from {
            break;
        }
        if !matches!(w.script_waiting(), Some(Request::CableCar { down: false })) {
            continue;
        }
        scene = true;
        let Some(at) = car(&f) else {
            panic!("the cable car is not on screen");
        };
        if car_at.is_some_and(|prev| prev != at) {
            car_moved = true;
        }
        car_at = Some(at);
        if scroll.is_some_and(|prev| prev != f.scroll) {
            view = true;
        }
        scroll = Some(f.scroll);
        if f.brightness == 0 {
            riding = true;
        }
        // The opening fade is also dark. Arrival is the fade after the ride is visible.
        if riding && f.brightness >= 8 {
            arrival = true;
        }
    }
    assert!(scene, "the ride never started");
    assert!(car_moved, "the car did not move");
    assert!(view, "the view did not scroll");
    assert!(arrival, "the arrival fade did not play");
    assert_eq!(w.map(), from, "the warp loaded during the ride");
    assert!(w.transition_active(), "the warp did not follow the ride");
    for _ in 0..200 {
        if w.map() != from && !w.transition_active() {
            break;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    assert_eq!(w.map(), HOUSE);
}
