//! Milestone 5: the capture predicate, snapshot, restore and the restore boot frame, on the
//! synthetic game. Hashes are always compared through `World::hash`, so these tests hold
//! whatever the hash covers.

use super::*;
use crate::battle::BattleOutcome;
use crate::battle::test_data::{NORMAL, WATER};
use crate::checkpoint::{Checkpoint, MAX_ENCODED, NotCapturable, RestoreError, SCHEMA};
use crate::field::FIELD_BGS;
use crate::hash::Fnv64;

/// `w`'s checkpoint through postcard bytes (the codec is Task C's; the round trip only needs
/// the serde types), restored.
fn round_trip(w: &World) -> World {
    let cp = w.checkpoint(&Game).expect("capturable");
    let bytes = postcard::to_allocvec(&cp).unwrap();
    let cp: Checkpoint = postcard::from_bytes(&bytes).unwrap();
    World::restore(&Game, &cp).expect("restores")
}

/// Steps both worlds on `keys`, asserting equal hashes after every frame.
fn lockstep(a: &mut World, b: &mut World, keys: impl IntoIterator<Item = Buttons>) {
    for (i, k) in keys.into_iter().enumerate() {
        a.step(&Game, k).validate().unwrap();
        b.step(&Game, k).validate().unwrap();
        assert_eq!(
            a.hash(),
            b.hash(),
            "diverged {} frames after restore",
            i + 1
        );
    }
}

/// Frames each restored world is stepped after its capture point.
const AFTER: usize = 120;

/// The core invariant along `inputs` from `new()`: at every capturable frame `n`, the restored
/// world has the uninterrupted run's hash, and keeps it on each of the next [`AFTER`] frames
/// (the inputs go on with nothing held). `at` sees each capture point. Returns their count.
fn round_trip_along(
    new: impl Fn() -> World,
    inputs: &[Buttons],
    mut at: impl FnMut(&World),
) -> u32 {
    let keys = |i: usize| inputs.get(i).copied().unwrap_or(Buttons::NONE);
    // The uninterrupted run: the hash after each input.
    let mut w = new();
    let hashes: Vec<u64> = (0..inputs.len() + AFTER)
        .map(|i| {
            w.step(&Game, keys(i)).validate().unwrap();
            w.hash()
        })
        .collect();
    let mut w = new();
    let mut n = 0;
    for i in 0..inputs.len() {
        w.step(&Game, keys(i));
        if w.capturable().is_err() {
            continue;
        }
        n += 1;
        at(&w);
        let mut r = round_trip(&w);
        assert_eq!(r.hash(), hashes[i], "frame {}", w.frame());
        r.initial_frame();
        for (j, &want) in hashes.iter().enumerate().skip(i + 1).take(AFTER) {
            r.step(&Game, keys(j)).validate().unwrap();
            assert_eq!(
                r.hash(),
                want,
                "restored at frame {}, diverged {} frames later",
                w.frame(),
                j - i
            );
        }
    }
    n
}

fn golden_inputs() -> Vec<Buttons> {
    [Buttons::RIGHT, Buttons::A, Buttons::NONE, Buttons::DOWN]
        .into_iter()
        .flat_map(|k| core::iter::repeat_n(k, 25))
        .collect()
}

fn hold(keys: Buttons, frames: usize) -> impl Iterator<Item = Buttons> {
    core::iter::repeat_n(keys, frames)
}

// ---- the predicate ----

#[test]
fn a_new_game_fading_in_is_not_capturable_then_is() {
    let mut w = World::new(&Game);
    assert_eq!(w.capturable(), Err(NotCapturable::Fade));
    assert_eq!(w.snapshot(&Game), Err(NotCapturable::Fade));
    assert!(w.checkpoint(&Game).is_err());
    run(&mut w, Buttons::NONE, 16);
    assert_eq!(w.capturable(), Ok(()));
}

#[test]
fn a_running_script_is_not_capturable() {
    let mut w = started();
    start(&mut w, |ctx| Box::pin(async move { ctx.delay(10).await }));
    assert_eq!(w.capturable(), Err(NotCapturable::Script));
    run(&mut w, Buttons::NONE, 1);
    assert_eq!(w.capturable(), Err(NotCapturable::Script));
    until_done(&mut w, 20);
    assert_eq!(w.capturable(), Ok(()));
}

#[test]
fn an_open_text_box_is_not_capturable() {
    let mut w = started();
    // Talk to the NPC: the script ends with the box closed.
    tap(&mut w, Buttons::RIGHT, 10);
    tap(&mut w, Buttons::A, 4);
    assert_eq!(w.capturable(), Err(NotCapturable::Script));
    // A message left open after its script ended.
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.message(TextId(0)).await })
    });
    until_done(&mut w, 10);
    assert!(w.text().is_open());
    assert_eq!(w.capturable(), Err(NotCapturable::TextBox));
}

#[test]
fn a_fade_in_progress_or_a_black_screen_is_not_capturable() {
    // Script fades wait for their end, so start one directly, as a script that ended would
    // leave it.
    let mut w = started();
    w.s.fader.start(Fade::ToBlack);
    run(&mut w, Buttons::NONE, 2);
    assert!(!w.script_running());
    assert_eq!(w.capturable(), Err(NotCapturable::Fade));
    run(&mut w, Buttons::NONE, 20);
    assert_eq!(w.s.fader.level(), 16);
    assert_eq!(w.capturable(), Err(NotCapturable::Fade), "black, done");
}

#[test]
fn a_warp_transition_is_not_capturable() {
    let mut w = started();
    walk(&mut w, Dir::Left, 2);
    walk(&mut w, Dir::Up, 2);
    run(&mut w, Buttons::UP, 1);
    assert!(w.transition_active());
    assert_eq!(w.capturable(), Err(NotCapturable::Transition));
    run(&mut w, Buttons::NONE, 120);
    assert_eq!(w.map(), HOUSE);
    assert_eq!(w.capturable(), Ok(()));
}

#[test]
fn a_movement_script_outliving_its_script_is_not_capturable() {
    static STEPS: [Step; 2] = [
        Step::Walk(Dir::Down, Speed::Normal),
        Step::Walk(Dir::Up, Speed::Normal),
    ];
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.move_obj(1, &STEPS).await })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(!w.script_running());
    assert_eq!(w.capturable(), Err(NotCapturable::Moving(1)));
    run(&mut w, Buttons::NONE, 40);
    assert_eq!(w.capturable(), Ok(()));
}

#[test]
fn the_starter_screen_and_battles_are_not_capturable() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.choose_starter([WATER, NORMAL, WATER]).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(w.starter().is_some());
    assert_eq!(w.capturable(), Err(NotCapturable::Starter));

    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            ctx.battle_wild(NORMAL, 2, false).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(w.in_battle());
    assert_eq!(w.capturable(), Err(NotCapturable::Battle));
}

/// Makes NPC 1 wander (a template delta too) and puts a `setmetatile` override in view.
fn wander_and_override(w: &mut World) {
    start(w, |ctx| {
        Box::pin(async move {
            ctx.set_movement_type(1, MovementType::Wander).await;
            ctx.set_metatile(4, 7, GRASS, false).await;
            ctx.set_object_template_pos(3, 2, 6).await;
        })
    });
    until_done(w, 10);
}

#[test]
fn an_npc_mid_wander_and_the_player_mid_step_are_capturable() {
    let mut w = started();
    wander_and_override(&mut w);
    let mut seen = false;
    for _ in 0..400 {
        run(&mut w, Buttons::NONE, 1);
        if !w.objects().get(1).unwrap().is_settled() {
            assert_eq!(w.capturable(), Ok(()));
            assert_eq!(round_trip(&w).hash(), w.hash());
            seen = true;
            break;
        }
    }
    assert!(seen, "the NPC never stepped");
    run(&mut w, Buttons::DOWN, 12);
    assert!(!w.objects().player().is_settled());
    assert_eq!(w.capturable(), Ok(()));
    assert_eq!(round_trip(&w).hash(), w.hash());
}

// ---- round trips ----

#[test]
fn every_capturable_frame_of_the_golden_inputs_round_trips() {
    let inputs = golden_inputs();
    let n = round_trip_along(|| World::new(&Game), &inputs, |_| {});
    assert!(n >= 40, "only {n} capturable frames");
}

/// 600 frames: a wandering NPC, a moved template, a `setmetatile` override, a door warp there
/// and back, and a connection crossing.
fn walk_inputs() -> Vec<Buttons> {
    let mut v = Vec::new();
    v.extend(hold(Buttons::NONE, 100)); // the NPC wanders (templates reset on the next visit)
    v.extend(hold(Buttons::LEFT, 40)); // (3, 5)
    v.extend(hold(Buttons::UP, 60)); // (3, 3), then the door
    v.extend(hold(Buttons::NONE, 100)); // into the house, on the mat
    v.extend(hold(Buttons::UP, 30));
    v.extend(hold(Buttons::DOWN, 40)); // back onto the mat, then out
    v.extend(hold(Buttons::NONE, 120)); // out of the town door, at (3, 3)
    v.extend(hold(Buttons::RIGHT, 40)); // (5, 3)
    v.extend(hold(Buttons::UP, 70)); // across the connection
    assert_eq!(v.len(), 600);
    v
}

#[test]
fn every_capturable_frame_of_a_600_frame_walk_round_trips() {
    let new = || {
        let mut w = started();
        wander_and_override(&mut w);
        // The NPC's first wander step comes about 220 frames later.
        run(&mut w, Buttons::NONE, 150);
        w
    };
    let (mut maps, mut deltas, mut overrides, mut wandering) = (Vec::new(), false, false, false);
    let n = round_trip_along(new, &walk_inputs(), |w| {
        let snap = w.snapshot(&Game).unwrap();
        if maps.last() != Some(&w.map()) {
            maps.push(w.map());
        }
        deltas |= !snap.templates.is_empty();
        overrides |= !snap.field.overrides.is_empty();
        wandering |= snap.objects.objs[1..].iter().any(|o| o.action.is_some());
    });
    assert!(n >= 300, "only {n} capturable frames");
    assert_eq!(maps, [TOWN, HOUSE, TOWN, ROUTE]);
    assert!(
        deltas && overrides && wandering,
        "{deltas} {overrides} {wandering}"
    );
}

#[test]
fn a_world_after_a_battle_round_trips() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            let o = ctx.battle_wild(NORMAL, 2, false).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
    for n in 0.. {
        if !w.script_running() {
            break;
        }
        let k = if n % 2 == 0 {
            Buttons::A
        } else {
            Buttons::NONE
        };
        run(&mut w, k, 1);
        assert!(n < 20_000);
    }
    assert!(w.flag(FLAG_DONE));
    run(&mut w, Buttons::NONE, 20);
    assert_eq!(w.capturable(), Ok(()));
    assert_eq!(w.party().len(), 1);
    let mut a = round_trip(&w);
    assert_eq!(a.party(), w.party());
    assert_eq!(a.hash(), w.hash());
    let keys = golden_inputs();
    lockstep(&mut a, &mut w, keys);
}

// ---- the boot frame ----

fn field_hash(f: &Field) -> u64 {
    let mut h = Fnv64::new();
    f.hash(&mut h);
    h.finish()
}

fn entries(f: &Frame) -> usize {
    (f.cmds.iter())
        .map(|c| match c {
            Cmd::MapEntries { entries, .. } => entries.len(),
            _ => 0,
        })
        .sum()
}

#[test]
fn the_boot_frame_uploads_the_field_and_every_visible_object() {
    let mut w = started();
    wander_and_override(&mut w);
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 20); // across the connection: the ring origin moves
    let live = run(&mut w, Buttons::NONE, 1);
    assert_eq!(w.map(), ROUTE);
    assert_ne!(w.snapshot(&Game).unwrap().field.ring, (0, 0));
    let mut r = round_trip(&w);
    let boot = r.initial_frame();
    assert!(boot.bulk);
    for bg in FIELD_BGS {
        assert!(
            boot.cmds.iter().any(
                |c| matches!(c, Cmd::Bg { layer, control } if *layer == bg && control.enabled)
            ),
            "BG{bg} control"
        );
    }
    assert!(boot.cmds.iter().any(|c| matches!(c, Cmd::BgTiles { .. })));
    assert!(boot.cmds.iter().any(|c| matches!(c, Cmd::BgPalette { .. })));
    // Every drawn cell: 4 entries on each field BG.
    let d = w.snapshot(&Game).unwrap().field.drawn.unwrap();
    let cells = ((d[2] - d[0] + 1) * (d[3] - d[1] + 1)) as usize;
    assert_eq!(entries(&boot), cells * 4 * FIELD_BGS.len());
    // The same field uploads as the battle return path (restore + whole redraw) of a copy.
    let mut copy = Field::from_snapshot(&w.s.field.snapshot());
    let mut redraw = Frame::default();
    copy.restore(&Game, &mut redraw);
    copy.redraw(&Game, &mut redraw);
    let field_cmds = |f: &Frame| {
        (f.cmds.iter())
            .filter(|c| !matches!(c, Cmd::ObjTiles { .. } | Cmd::ObjPalette { .. }))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(field_cmds(&boot), field_cmds(&redraw));
    assert_eq!(boot.scroll, live.scroll);
    assert_eq!(boot.brightness, live.brightness);
    // Every visible object is drawn, where the live frame drew it.
    let place = |f: &Frame| {
        let mut v: Vec<_> = (f.objs.iter())
            .map(|o| (o.x, o.y, o.shape, o.size, o.hflip))
            .collect();
        v.sort_by_key(|&(x, y, ..)| (x, y));
        v
    };
    assert!(!boot.objs.is_empty());
    assert_eq!(place(&boot), place(&live));
    let uploads = (boot.cmds.iter())
        .filter(|c| matches!(c, Cmd::ObjTiles { .. }))
        .count();
    assert_eq!(uploads, boot.objs.len());
    assert!(
        boot.cmds
            .iter()
            .any(|c| matches!(c, Cmd::ObjPalette { .. }))
    );
    // The text box stays closed: nothing on BG0.
    assert!(
        !boot
            .cmds
            .iter()
            .any(|c| matches!(c, Cmd::Bg { layer: 0, .. }))
    );
}

#[test]
fn the_boot_frame_mutates_no_hashed_state() {
    let mut w = started();
    wander_and_override(&mut w);
    run(&mut w, Buttons::LEFT, 30);
    let before = field_hash(&w.s.field);
    let mut f = Frame::default();
    w.s.field.upload_all(&Game, &mut f);
    assert_eq!(field_hash(&w.s.field), before);
    let mut r = round_trip(&w);
    let h = r.hash();
    let boot = r.initial_frame();
    assert!(!boot.cmds.is_empty());
    assert_eq!(r.hash(), h);
    assert_eq!(r.hash(), w.hash());
    assert!(r.initial_frame().cmds.is_empty(), "taken once");
}

// ---- errors ----

fn checkpoint() -> Checkpoint {
    let mut w = started();
    wander_and_override(&mut w);
    w.checkpoint(&Game).unwrap()
}

fn restore(cp: &Checkpoint) -> Result<World, RestoreError> {
    World::restore(&Game, cp)
}

#[test]
fn restore_rejects_bad_checkpoints() {
    let good = checkpoint();
    assert!(restore(&good).is_ok());

    let mut cp = good.clone();
    cp.header.schema = SCHEMA + 1;
    assert_eq!(restore(&cp).err(), Some(RestoreError::Schema(SCHEMA + 1)));

    let mut cp = good.clone();
    cp.world.field.map = MapId(99);
    assert_eq!(
        restore(&cp).err(),
        Some(RestoreError::UnknownMap(MapId(99)))
    );
    let mut cp = good.clone();
    cp.world.objects.map = MapId(98);
    assert_eq!(
        restore(&cp).err(),
        Some(RestoreError::UnknownMap(MapId(98)))
    );

    let mut cp = good.clone();
    assert!(!cp.world.templates.is_empty());
    cp.world.templates[0].local_id = 77;
    assert_eq!(
        restore(&cp).err(),
        Some(RestoreError::UnknownTemplate {
            map: TOWN,
            local_id: 77
        })
    );

    let mut cp = good.clone();
    cp.world.objects.objs[0].local_id = 1;
    assert_eq!(restore(&cp).err(), Some(RestoreError::NoPlayer));
    let mut cp = good.clone();
    cp.world.objects.objs.clear();
    assert_eq!(restore(&cp).err(), Some(RestoreError::NoPlayer));

    let mut cp = good.clone();
    cp.world.rng = 0;
    assert_eq!(restore(&cp).err(), Some(RestoreError::BadRng));
}

#[test]
fn restore_rejects_more_than_six_party_members() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 5).await;
        })
    });
    until_done(&mut w, 5);
    let mut cp = w.checkpoint(&Game).unwrap();
    assert_eq!(cp.world.party.len(), 1);
    let mon = cp.world.party[0];
    cp.world.party = alloc::vec![mon; 6];
    assert!(matches!(
        restore(&cp),
        Err(RestoreError::HashMismatch { .. })
    ));
    cp.world.party.push(mon);
    assert_eq!(restore(&cp).err(), Some(RestoreError::PartyTooLarge));
}

#[test]
fn the_world_hash_is_checked_on_the_same_build_only() {
    let good = checkpoint();
    let mut cp = good.clone();
    cp.header.world_hash ^= 1;
    assert_eq!(
        restore(&cp).err(),
        Some(RestoreError::HashMismatch {
            want: good.header.world_hash ^ 1,
            got: good.header.world_hash
        })
    );
    cp.header.build.content ^= 1;
    let w = restore(&cp).expect("another build: informational");
    assert_eq!(w.hash(), good.header.world_hash);
}

// ---- size ----

#[test]
fn the_encoded_checkpoint_fits_the_flash_slot() {
    let mut w = started();
    wander_and_override(&mut w);
    let mut max = 0;
    for k in walk_inputs() {
        w.step(&Game, k);
        if let Ok(cp) = w.checkpoint(&Game) {
            max = max.max(postcard::to_allocvec(&cp).unwrap().len());
        }
    }
    std::println!("synthetic checkpoint: at most {max} bytes (limit {MAX_ENCODED})");
    assert!(max > 0 && max <= MAX_ENCODED);
}
