//! Milestone 5: the capture predicate, snapshot, restore and the restore boot frame, on the
//! synthetic game. Hashes are always compared through `World::hash`, so these tests hold
//! whatever the hash covers.

use super::*;
use crate::battle::BattleOutcome;
use crate::battle::test_data::{NORMAL, WATER};
use crate::checkpoint::{
    ActionKindSnap, ActionSnap, Checkpoint, MAX_ENCODED, NotCapturable, RestoreError, SCHEMA,
};
use crate::field::FIELD_BGS;
use crate::hash::Fnv64;
use crate::video::{MAP_SIZE, SCREEN_H, SCREEN_W, layout};

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

/// The BG tilemaps a shell's VRAM holds after applying frames (what the field picture is
/// made of; tiles and palettes are static per map here).
struct Screen {
    entries: Vec<u16>,
    scroll: [(u16, u16); 4],
}

impl Screen {
    fn new() -> Self {
        Self {
            entries: alloc::vec![0; 32 * MAP_SIZE * MAP_SIZE],
            scroll: [(0, 0); 4],
        }
    }

    fn apply(&mut self, f: &Frame) {
        let block = |sb: u8| sb as usize * MAP_SIZE * MAP_SIZE;
        for c in &f.cmds {
            match c {
                Cmd::MapEntries {
                    screen_block,
                    start,
                    entries,
                } => {
                    let at = block(*screen_block) + *start as usize;
                    self.entries[at..at + entries.len()].copy_from_slice(entries);
                }
                Cmd::MapFill {
                    screen_block,
                    entry,
                } => {
                    let at = block(*screen_block);
                    self.entries[at..at + MAP_SIZE * MAP_SIZE].fill(*entry);
                }
                _ => {}
            }
        }
        self.scroll = f.scroll;
    }

    /// The entries in view on each field BG: the tiles under the 240x160 screen at its
    /// scroll, wrapping.
    fn visible(&self) -> Vec<u16> {
        let mut out = Vec::new();
        for bg in FIELD_BGS {
            let base = layout::SCREEN_BLOCKS[bg as usize] as usize * MAP_SIZE * MAP_SIZE;
            let (sx, sy) = self.scroll[bg as usize];
            let (sx, sy) = (sx as usize, sy as usize);
            for ty in sy / 8..=(sy + SCREEN_H - 1) / 8 {
                for tx in sx / 8..=(sx + SCREEN_W - 1) / 8 {
                    let (col, row) = (tx % MAP_SIZE, ty % MAP_SIZE);
                    out.push(self.entries[base + row * MAP_SIZE + col]);
                }
            }
        }
        out
    }
}

/// A world whose frames are all applied to a [`Screen`], as a shell would.
struct Live {
    w: World,
    screen: Screen,
}

impl Live {
    fn new() -> Self {
        let mut w = World::new(&Game);
        let mut screen = Screen::new();
        screen.apply(&w.initial_frame());
        Live { w, screen }
    }

    /// A new game after its fade-in.
    fn started() -> Self {
        let mut l = Self::new();
        l.run(Buttons::NONE, 16);
        l
    }

    fn step(&mut self, keys: Buttons) -> Frame {
        let f = self.w.step(&Game, keys);
        f.validate().unwrap();
        self.screen.apply(&f);
        f
    }

    fn run(&mut self, keys: Buttons, frames: u32) {
        for _ in 0..frames {
            self.step(keys);
        }
    }

    fn wander_and_override(&mut self) {
        start(&mut self.w, wander_script);
        while self.w.script_running() {
            self.step(Buttons::NONE);
        }
    }
}

/// The OBJs of a frame, by place (VRAM slots may differ after a restore).
fn sprites(f: &Frame) -> Vec<(i16, i16, u8, u8, bool)> {
    let mut v: Vec<_> = (f.objs.iter())
        .map(|o| (o.x, o.y, o.shape as u8, o.size, o.hflip))
        .collect();
    v.sort_unstable();
    v
}

/// The picture checks of a restore against the live world it was captured from: `boot` (the
/// restored boot frame on a fresh screen) shows the same field tilemaps, scroll, sprites and
/// brightness as `live` after its last frame `frame`.
fn assert_same_picture(live: &Live, frame: &Frame, boot: &Frame) {
    let mut screen = Screen::new();
    screen.apply(boot);
    let at = live.w.frame();
    assert_eq!(screen.scroll, live.screen.scroll, "scroll at frame {at}");
    assert!(
        screen.visible() == live.screen.visible(),
        "field tilemaps differ at frame {at}"
    );
    assert_eq!(sprites(boot), sprites(frame), "sprites at frame {at}");
    assert_eq!(boot.brightness, frame.brightness);
}

/// Frames each restored world is stepped after its capture point.
const AFTER: usize = 120;

/// The core invariant along `inputs` from `new()`: at every capturable frame `n`, the restored
/// world has the uninterrupted run's hash and picture, and keeps the hash on each of the next
/// [`AFTER`] frames (the inputs go on with nothing held). `at` sees each capture point.
/// Returns their count.
fn round_trip_along(new: impl Fn() -> Live, inputs: &[Buttons], mut at: impl FnMut(&World)) -> u32 {
    let keys = |i: usize| inputs.get(i).copied().unwrap_or(Buttons::NONE);
    // The uninterrupted run: the hash after each input.
    let mut w = new().w;
    let hashes: Vec<u64> = (0..inputs.len() + AFTER)
        .map(|i| {
            w.step(&Game, keys(i)).validate().unwrap();
            w.hash()
        })
        .collect();
    let mut live = new();
    let mut n = 0;
    for i in 0..inputs.len() {
        let frame = live.step(keys(i));
        let w = &live.w;
        if w.capturable().is_err() {
            continue;
        }
        n += 1;
        at(w);
        let mut r = round_trip(w);
        assert_eq!(r.hash(), hashes[i], "frame {}", w.frame());
        assert_same_picture(&live, &frame, &r.initial_frame());
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
fn a_refused_checkpoint_never_hashes() {
    let mut w = World::new(&Game);
    assert_eq!(hash_calls(|| assert!(w.checkpoint(&Game).is_err())), 0);
    run(&mut w, Buttons::NONE, 16);
    let mut cp = None;
    assert_eq!(hash_calls(|| cp = Some(w.checkpoint(&Game).unwrap())), 1);
    assert_eq!(cp.unwrap().header.world_hash, w.hash());
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

/// Makes NPC 1 wander (a template delta too), moves template 3 and puts a `setmetatile`
/// override in view.
fn wander_script(ctx: Ctx) -> Script {
    Box::pin(async move {
        ctx.set_movement_type(1, MovementType::Wander).await;
        ctx.set_metatile(4, 7, GRASS, false).await;
        ctx.set_object_template_pos(3, 2, 6).await;
    })
}

fn wander_and_override(w: &mut World) {
    start(w, wander_script);
    until_done(w, 10);
}

/// Holds `keys` (at most `max` frames) until `done`.
fn hold_until(l: &mut Live, keys: Buttons, max: u32, done: impl Fn(&World) -> bool) {
    for _ in 0..max {
        if done(&l.w) {
            return;
        }
        l.step(keys);
    }
    panic!("not done after {max} frames, at {:?}", pos(&l.w));
}

#[test]
fn a_crossing_blocks_capture_until_the_old_cells_leave_the_view() {
    let mut l = Live::started();
    hold_until(&mut l, Buttons::UP, 200, |w| {
        pos(w) == (5, 0) && w.objects().player().is_settled()
    });
    l.step(Buttons::UP);
    assert_eq!(l.w.map(), ROUTE, "crossed as the step starts");
    assert_eq!(l.w.capturable(), Err(NotCapturable::StaleView));
    // Redrawn from the route, the view would differ: the town's border is still shown where
    // the route has its own (and the cells left and right of the connection).
    let mut f = Frame::default();
    l.w.s.field.upload_all(&Game, &mut f);
    let mut redrawn = Screen::new();
    redrawn.apply(&f);
    assert!(redrawn.visible() != l.screen.visible());
    // Walking up the route, the cells drawn from the town scroll out of view.
    let mut frames = 0;
    let mut frame = Frame::default();
    while l.w.capturable().is_err() {
        assert_eq!(l.w.capturable(), Err(NotCapturable::StaleView));
        frame = l.step(Buttons::UP);
        frames += 1;
        assert!(frames < 400, "still stale at {:?}", pos(&l.w));
    }
    assert_eq!(l.w.map(), ROUTE);
    let mut r = round_trip(&l.w);
    assert_same_picture(&l, &frame, &r.initial_frame());
    lockstep(&mut r, &mut l.w, golden_inputs());
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
    let n = round_trip_along(Live::new, &inputs, |_| {});
    assert!(n >= 40, "only {n} capturable frames");
}

/// 700 frames: a wandering NPC, a moved template, a `setmetatile` override, a door warp there
/// and back, and a connection crossing walked up until the town's cells leave the view.
fn walk_inputs() -> Vec<Buttons> {
    let mut v = Vec::new();
    v.extend(hold(Buttons::NONE, 80)); // the NPC wanders (templates reset on the next visit)
    v.extend(hold(Buttons::LEFT, 40)); // (3, 5)
    v.extend(hold(Buttons::UP, 60)); // (3, 3), then the door
    v.extend(hold(Buttons::NONE, 100)); // into the house, on the mat
    v.extend(hold(Buttons::UP, 30));
    v.extend(hold(Buttons::DOWN, 40)); // back onto the mat, then out
    v.extend(hold(Buttons::NONE, 100)); // out of the town door, at (3, 3)
    v.extend(hold(Buttons::RIGHT, 40)); // (5, 3)
    v.extend(hold(Buttons::UP, 210)); // across the connection, up to the route's top row
    assert_eq!(v.len(), 700);
    v
}

#[test]
fn every_capturable_frame_of_a_700_frame_walk_round_trips() {
    let new = || {
        let mut l = Live::started();
        l.wander_and_override();
        // The NPC's first wander step comes about 220 frames later.
        l.run(Buttons::NONE, 150);
        l
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
    let mut l = Live::started();
    start(&mut l.w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
            let o = ctx.battle_wild(NORMAL, 2, false).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
    for n in 0.. {
        if !l.w.script_running() {
            break;
        }
        l.step(if n % 2 == 0 {
            Buttons::A
        } else {
            Buttons::NONE
        });
        assert!(n < 20_000);
    }
    assert!(l.w.flag(FLAG_DONE));
    l.run(Buttons::NONE, 19);
    let frame = l.step(Buttons::NONE);
    assert_eq!(l.w.capturable(), Ok(()));
    assert_eq!(l.w.party().len(), 1);
    let mut a = round_trip(&l.w);
    assert_eq!(a.party(), l.w.party());
    assert_eq!(a.hash(), l.w.hash());
    assert_same_picture(&l, &frame, &a.initial_frame());
    lockstep(&mut a, &mut l.w, golden_inputs());
}

#[test]
fn the_player_name_round_trips() {
    let mut w = started();
    w.set_player_name(b"ASH");
    let r = round_trip(&w);
    assert_eq!(r.player_name(), w.player_name());
    assert_eq!(&r.player_name()[..4], b"ASH\xFF");
    assert_eq!(r.hash(), w.hash());
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
    let mut l = Live::started();
    l.wander_and_override();
    // Across the connection and up the route: the ring origin moves.
    hold_until(&mut l, Buttons::UP, 600, |w| {
        w.map() == ROUTE && w.capturable().is_ok()
    });
    let live = l.step(Buttons::NONE);
    let w = &l.w;
    assert_ne!(w.snapshot(&Game).unwrap().field.ring, (0, 0));
    let mut r = round_trip(w);
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
            .filter(|c| match c {
                Cmd::ObjTiles { .. } | Cmd::ObjPalette { .. } | Cmd::MapFill { .. } => false,
                Cmd::Bg { layer, .. } => *layer != layout::TEXT_BG,
                _ => true,
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(field_cmds(&boot), field_cmds(&redraw));
    assert_same_picture(&l, &live, &boot);
    // Every visible object is drawn.
    assert!(!boot.objs.is_empty());
    let uploads = (boot.cmds.iter())
        .filter(|c| matches!(c, Cmd::ObjTiles { .. }))
        .count();
    assert_eq!(uploads, boot.objs.len());
    let visible = (w.objects().iter()).filter(|o| o.visible).count();
    assert_eq!(boot.objs.len(), visible);
    assert!(
        boot.cmds
            .iter()
            .any(|c| matches!(c, Cmd::ObjPalette { .. }))
    );
    // The text box is closed, as `TextBox::close` leaves it: BG0 cleared and disabled.
    let mut closed = Frame::default();
    TextBox::new().close(&mut closed);
    assert_eq!(boot.cmds[..closed.cmds.len()], closed.cmds[..]);
    assert!(
        boot.cmds
            .iter()
            .any(|c| matches!(c, Cmd::Bg { layer: 0, control } if !control.enabled))
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
    // On the live world (its render caches full) and on a restored copy.
    let h = w.hash();
    assert!(!w.restore_frame(&Game).cmds.is_empty());
    assert_eq!(w.hash(), h);
    let mut r = round_trip(&w);
    assert!(!r.restore_frame(&Game).cmds.is_empty());
    assert_eq!(r.hash(), h);
    let boot = r.initial_frame();
    assert!(!boot.cmds.is_empty());
    assert_eq!(r.hash(), h);
    assert!(r.initial_frame().cmds.is_empty(), "taken once");
    // Both keep stepping as before.
    lockstep(&mut r, &mut w, golden_inputs());
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

/// A forgery: `forge` applied to a capture, claiming another build so the world hash (which
/// would catch most of them on this build) is not checked.
type Forge = fn(&mut Checkpoint);

/// Each case restores without panicking, and is refused for the expected reason.
fn assert_refused(cases: &[(&str, Forge, Option<RestoreError>)]) {
    let good = checkpoint();
    for &(name, forge, want) in cases {
        let mut cp = good.clone();
        cp.header.build.content ^= 1;
        forge(&mut cp);
        let got = restore(&cp).err();
        match want {
            Some(e) => assert_eq!(got, Some(e), "{name}"),
            None => assert!(
                matches!(got, Some(RestoreError::Invalid(_))),
                "{name}: {got:?}"
            ),
        }
    }
}

#[test]
fn restore_refuses_a_forged_header_or_maps() {
    assert_refused(&[
        ("frame", |cp| cp.header.frame += 1, None),
        ("objects map", |cp| cp.world.objects.map = HOUSE, None),
    ]);
}

#[test]
fn restore_refuses_a_forged_field() {
    assert_refused(&[
        (
            "drawn shifted",
            |cp| cp.world.field.drawn.as_mut().unwrap()[0] += 1,
            None,
        ),
        ("camera off drawn", |cp| cp.world.field.camera.0 += 16, None),
        (
            "camera far away",
            |cp| {
                cp.world.field.camera = (100_000, 0);
                cp.world.field.drawn = None;
            },
            None,
        ),
        ("ring x", |cp| cp.world.field.ring.0 = 16, None),
        ("ring y", |cp| cp.world.field.ring.1 = -1, None),
        (
            "override outside",
            |cp| cp.world.field.overrides.push(((99, 99), 0)),
            None,
        ),
        (
            "overrides unsorted",
            |cp| cp.world.field.overrides.insert(0, ((5, 5), 0)),
            None,
        ),
    ]);
    // Not yet drawn is a state the field has between a load and its first draw.
    let mut cp = checkpoint();
    cp.header.build.content ^= 1;
    cp.world.field.drawn = None;
    assert!(restore(&cp).is_ok());
}

/// NPC 1 (`objs[1]`) walking left, `t` frames in.
fn walking(cp: &mut Checkpoint, kind: ActionKindSnap, t: u8, off: (i16, i16), jump_y: i8) {
    cp.world.objects.objs[1].action = Some(ActionSnap {
        kind,
        dir: Dir::Left,
        t,
        off,
        jump_y,
    });
}

#[test]
fn restore_refuses_forged_actions_and_animations() {
    use ActionKindSnap::*;
    // Consistent actions restore.
    let good: [Forge; 4] = [
        |cp| walking(cp, Walk(Speed::Normal), 3, (13, 0), 0),
        |cp| walking(cp, Jump(2), 20, (20, 0), -10),
        |cp| walking(cp, InPlace(16), 15, (0, 0), 0),
        |cp| walking(cp, Delay(0), 0, (0, 0), 0),
    ];
    for (i, forge) in good.into_iter().enumerate() {
        let mut cp = checkpoint();
        cp.header.build.content ^= 1;
        forge(&mut cp);
        let ok = restore(&cp).map(|_| ()).map_err(|e| (i, e));
        // `Delay(0)` lasts 1 frame: no frame of it can be pending.
        if i == 3 {
            assert!(ok.is_err());
        } else {
            assert_eq!(ok, Ok(()));
        }
    }
    assert_refused(&[
        (
            "t 0",
            |cp| walking(cp, Walk(Speed::Normal), 0, (16, 0), 0),
            None,
        ),
        (
            "t past the end",
            |cp| walking(cp, Walk(Speed::Normal), 16, (0, 0), 0),
            None,
        ),
        (
            "off across dir",
            |cp| walking(cp, Walk(Speed::Normal), 3, (13, 1), 0),
            None,
        ),
        (
            "off ahead",
            |cp| walking(cp, Walk(Speed::Normal), 3, (-13, 0), 0),
            None,
        ),
        (
            "off too far",
            |cp| walking(cp, Walk(Speed::Normal), 3, (17, 0), 0),
            None,
        ),
        (
            "off for a far jump",
            |cp| walking(cp, Jump(2), 20, (33, 0), -10),
            None,
        ),
        (
            "off in place",
            |cp| walking(cp, InPlace(16), 3, (1, 0), 0),
            None,
        ),
        (
            "jump height",
            |cp| walking(cp, Jump(2), 20, (20, 0), -9),
            None,
        ),
        (
            "height off a jump",
            |cp| walking(cp, Run, 3, (10, 0), -1),
            None,
        ),
        ("long jump", |cp| walking(cp, Jump(3), 3, (0, 0), 0), None),
        ("anim cmd", |cp| cp.world.objects.objs[1].anim.cmd = 4, None),
    ]);
}

#[test]
fn restore_refuses_objects_unlike_their_templates() {
    assert_refused(&[
        (
            "no template",
            |cp| cp.world.objects.objs[1].local_id = 50,
            Some(RestoreError::UnknownTemplate {
                map: TOWN,
                local_id: 50,
            }),
        ),
        (
            "other graphics",
            |cp| cp.world.objects.objs[1].gfx = Some(GfxId(9)),
            None,
        ),
    ]);
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
