//! World tests of Milestone 8 Task I (trainer sight, emotes, movement types). Owned by that task.
//!
//! The synthetic game's town, with two trainers: trainer 1 on (8, 3) looking left and trainer 2
//! on (2, 8) looking up, both with sight 3, and a scriptless object parked out of the way.

use super::super::sight::{self, Sighting};
use super::*;
use crate::battle::test_data::WATER;
use crate::script::{Request, Response};

const FLAG_APPROACHED: FlagId = FlagId(0x303);
const FLAG_TRAINER_1: FlagId = FlagId(0x501);
const FLAG_TRAINER_2: FlagId = FlagId(0x502);
const TRAINER_1_SCRIPT: ScriptId = ScriptId(20);
const TRAINER_2_SCRIPT: ScriptId = ScriptId(21);

const fn trainer(local_id: u8, x: i16, y: i16, facing: Dir, script: ScriptId) -> ObjectEvent {
    let mut o = obj(local_id, x, y, script.0, None);
    o.movement = MovementType::Face(facing);
    o.trainer_type = sight::TRAINER_TYPE_NORMAL;
    o.trainer_sight = 3;
    o
}

static ARENA: MapData = MapData {
    name: "arena",
    width: 10,
    height: 10,
    blocks: &TOWN_BLOCKS,
    border: [WALL; 4],
    primary: &TS,
    secondary: &TS2,
    objects: &[
        trainer(1, 8, 3, Dir::Left, TRAINER_1_SCRIPT),
        trainer(2, 2, 8, Dir::Up, TRAINER_2_SCRIPT),
        scriptless(3, 1, 1),
    ],
    warps: &[],
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: true,
};

/// The trainers' script: `TrainerApproach` (as content's `trainer_battle`), then a battle.
async fn trainer_script(ctx: Ctx, obj: u8, flag: FlagId) {
    let r = ctx.request(Request::TrainerApproach { obj }).await;
    ctx.set_flag(FLAG_APPROACHED, matches!(r, Response::Unit))
        .await;
    ctx.battle_trainer(TRAINER, TextId(0)).await;
    ctx.set_flag(flag, true).await;
    ctx.release_all().await;
}

/// [`Game`] with the arena as the town and the trainer scripts and flags.
struct Arena;

impl GameData for Arena {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == TOWN { &ARENA } else { Game.map(id) }
    }
    fn special_ids(&self) -> SpecialIds {
        Game.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        Game.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        Game.player_sprite()
    }
    fn font(&self) -> &'static Font {
        Game.font()
    }
    fn window(&self) -> &'static WindowGfx {
        Game.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        Game.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Game.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        Game.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        match id {
            TRAINER_1_SCRIPT => Box::pin(trainer_script(ctx, 1, FLAG_TRAINER_1)),
            TRAINER_2_SCRIPT => Box::pin(trainer_script(ctx, 2, FLAG_TRAINER_2)),
            _ => Game.script(id, ctx),
        }
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static crate::mon::TrainerData> {
        Game.trainer(id)
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        Game.heal_location(id)
    }
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        match script {
            TRAINER_1_SCRIPT => Some(FLAG_TRAINER_1),
            TRAINER_2_SCRIPT => Some(FLAG_TRAINER_2),
            _ => None,
        }
    }
}

fn step(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(&Arena, keys).validate().unwrap();
    }
}

/// The arena after the new-game fade, the player on (5, 5) with `mons` strong mons.
fn arena(mons: usize) -> World {
    let mut w = World::new(&Arena);
    step(&mut w, Buttons::NONE, 16);
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        for _ in 0..mons {
            ctx.give_mon(WATER, 30).await;
        }
    }));
    step(&mut w, Buttons::NONE, 2);
    assert!(!w.script_running());
    assert_eq!(w.objects().player().pos(), (5, 5));
    w
}

fn facing(w: &World, local: u8) -> Dir {
    w.objects().get(local).unwrap().facing
}

/// Puts the player on `at` (as if it had walked there) and checks sight with doubles decided by
/// `double`.
fn sees(w: &mut World, at: (i16, i16), double: bool) -> Option<Sighting> {
    w.s.objects.set_pos(PLAYER, at.0, at.1);
    sight::scan(&Arena, &w.s, |_| double)
}

#[test]
fn sight_needs_range_facing_a_clear_path_and_an_unset_flag() {
    let mut w = arena(1);
    let hit = Sighting {
        obj: 1,
        script: TRAINER_1_SCRIPT,
        distance: 3,
        second: false,
    };
    assert_eq!(sees(&mut w, (5, 3), false), Some(hit));
    assert_eq!(
        sees(&mut w, (7, 3), false),
        Some(Sighting { distance: 1, ..hit }),
        "adjacent"
    );
    assert_eq!(sees(&mut w, (4, 3), false), None, "out of range");
    assert_eq!(sees(&mut w, (5, 4), false), None, "not on its line");
    // Behind it is not ahead of it.
    w.s.objects.set_pos(1, 6, 3);
    assert_eq!(sees(&mut w, (7, 3), false), None, "behind");
    w.s.objects.set_pos(1, 8, 3);
    // An object between blocks the line; the wander range does not count.
    w.s.objects.set_pos(3, 6, 3);
    assert_eq!(sees(&mut w, (5, 3), false), None, "blocked");
    w.s.objects.set_pos(3, 1, 1);
    w.s.objects.face(1, Dir::Up);
    assert_eq!(sees(&mut w, (5, 3), false), None, "facing away");
    w.s.objects.face(1, Dir::Left);
    assert_eq!(sees(&mut w, (5, 3), false), Some(hit));
    w.s.vars.set_flag(FLAG_TRAINER_1, true);
    assert_eq!(sees(&mut w, (5, 3), false), None, "beaten");
}

#[test]
fn buried_trainers_look_all_four_ways_and_other_types_do_not_look() {
    let mut w = arena(1);
    let t = w.s.templates.iter_mut().find(|t| t.local_id == 1).unwrap();
    t.trainer_type = sight::TRAINER_TYPE_BURIED;
    assert_eq!(sees(&mut w, (8, 5), false).map(|s| s.distance), Some(2));
    // `TRAINER_TYPE_SEE_ALL_DIRECTIONS` (2) and plain objects are skipped.
    for ty in [0, 2] {
        let t = w.s.templates.iter_mut().find(|t| t.local_id == 1).unwrap();
        t.trainer_type = ty;
        assert_eq!(sees(&mut w, (5, 3), false), None, "type {ty}");
    }
}

#[test]
fn double_trainers_need_two_usable_mons_and_a_second_trainer_needs_them_too() {
    let mut w = arena(1);
    assert_eq!(sees(&mut w, (5, 3), true), None, "double with one mon");
    // Trainer 2 under (5, 3) as well.
    w.s.objects.set_pos(2, 5, 6);
    let one = sees(&mut w, (5, 3), false).unwrap();
    assert_eq!((one.obj, one.second), (1, false), "one mon: the scan stops");
    let mut w2 = arena(2);
    assert_eq!(sees(&mut w2, (5, 3), true).map(|s| s.obj), Some(1));
    w2.s.objects.set_pos(2, 5, 6);
    let two = sees(&mut w2, (5, 3), false).unwrap();
    assert_eq!((two.obj, two.second), (1, true));
    // A fainted mon is not usable.
    w2.s.party.get_mut(1).unwrap().hp = 0;
    assert_eq!(sees(&mut w2, (5, 3), true), None);
}

#[test]
fn a_seen_player_is_approached_faced_and_battled_once() {
    let mut w = arena(1);
    // Walk up two cells: the step onto (5, 3) ends in trainer 1's line of sight.
    let mut frames = 0;
    while !w.script_running() {
        step(&mut w, Buttons::UP, 1);
        frames += 1;
        assert!(frames < 100, "never seen");
    }
    assert_eq!(w.objects().player().pos(), (5, 3));
    // The `!` (no emote sprite in the synthetic game: nothing drawn), then two steps.
    let mut n = 0;
    while !w.flag(FLAG_APPROACHED) && w.battle().is_none() {
        step(&mut w, Buttons::NONE, 1);
        n += 1;
        assert!(n < 300, "never approached");
    }
    assert!(w.flag(FLAG_APPROACHED), "answered `Unit`");
    assert_eq!(w.objects().get(1).unwrap().pos(), (6, 3));
    assert_eq!(facing(&w, 1), Dir::Left);
    assert_eq!(facing(&w, PLAYER), Dir::Right);
    // The approach of `objects::tests::m8_i::approach_frames_follow_gen3` (done 113 frames
    // after the sighting), counted here from the frame after it, plus the flag's frame.
    assert_eq!(n, 112, "approach frames");
    // The battle (mash A), then the flag.
    let mut m = 0;
    while w.script_running() {
        step(
            &mut w,
            if m % 2 == 0 {
                Buttons::A
            } else {
                Buttons::NONE
            },
            1,
        );
        m += 1;
        assert!(m < 5000, "battle never ended");
    }
    assert!(w.flag(FLAG_TRAINER_1));
    // The trainer keeps its new spot and facing for the visit, and does not look again.
    let t = w.s.templates.iter().find(|t| t.local_id == 1).unwrap();
    assert_eq!(
        ((t.x, t.y), t.movement),
        ((6, 3), MovementType::Face(Dir::Left))
    );
    step(&mut w, Buttons::DOWN, 20);
    step(&mut w, Buttons::UP, 20);
    assert!(!w.script_running());
}

#[test]
fn talking_to_a_trainer_skips_the_approach() {
    let mut w = arena(1);
    // Next to trainer 1 from below, facing up: out of its line.
    w.s.objects.set_pos(PLAYER, 8, 4);
    w.s.objects.face(PLAYER, Dir::Up);
    step(&mut w, Buttons::A, 1);
    assert!(w.script_running());
    step(&mut w, Buttons::NONE, 3);
    assert!(!w.flag(FLAG_APPROACHED), "answered `Value(usable)`");
    assert_eq!(w.objects().get(1).unwrap().pos(), (8, 3));
}

#[test]
fn two_trainers_seeing_report_the_second_degraded() {
    let mut w = arena(2);
    w.s.objects.set_pos(2, 5, 6);
    w.s.objects.set_pos(PLAYER, 4, 3);
    // Step right onto (5, 3): trainer 1 at 3 cells, trainer 2 (now on (5, 6)) at 3 cells below.
    let mut seen = false;
    for _ in 0..40 {
        step(&mut w, Buttons::RIGHT, 1);
        if w.events()
            .contains(&Event::Report(Report::Degraded("double sight")))
        {
            seen = true;
            break;
        }
    }
    assert!(seen, "double sight reported");
}
