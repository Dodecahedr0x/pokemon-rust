use super::hash::hash_event;
use super::test_game::{DEFEAT, TRAINER, game};
use super::*;
use crate::battle::gfx::obj;
use crate::battle::test_data::{FIRE, GROWL, HARDEN, MEGA_HIT, NORMAL, TACKLE, WATER, mon, party};
use crate::battle::{BattleSetup, Buff, Effectiveness, Need, Str};
use crate::input::Buttons;
use crate::video::{MAP_SIZE, Tile};

const A: Buttons = Buttons::A;
const NONE: Buttons = Buttons::NONE;

/// Bytes a frame copies into VRAM / palettes.
fn upload_bytes(f: &Frame) -> usize {
    let tile = core::mem::size_of::<Tile>();
    f.cmds
        .iter()
        .map(|c| match c {
            Cmd::BgPalette { .. } | Cmd::ObjPalette { .. } => 32,
            Cmd::BgTiles { tiles, .. } | Cmd::ObjTiles { tiles, .. } => size_of_val(*tiles),
            Cmd::BgTile { .. } | Cmd::ObjTile { .. } => tile,
            Cmd::MapEntries { entries, .. } => entries.len() * 2,
            Cmd::MapFill { .. } => MAP_SIZE * MAP_SIZE * 2,
            Cmd::Bg { .. } => 2,
        })
        .sum()
}

/// A black loading frame: a chunk, never more than one textbox upload (~14 KiB).
const BLACK_FRAME_BYTES: usize = 16 * 1024;
/// A visible frame: a few OBJ tiles and at most one window redraw.
const VISIBLE_FRAME_BYTES: usize = 6 * 1024;

/// The GBA budget of a frame the player sees.
fn check_visible(f: &Frame) {
    assert!(f.validate().is_ok());
    let tiles = gfx::obj_tiles_sent(f);
    assert!(tiles <= gfx::OBJ_TILES_PER_FRAME, "{tiles} OBJ tiles");
    for c in &f.cmds {
        assert!(
            !matches!(
                c,
                Cmd::BgTiles { .. } | Cmd::MapFill { .. } | Cmd::Bg { .. }
            ),
            "bulk BG command on a visible frame: {c:?}"
        );
    }
    let bytes = upload_bytes(f);
    assert!(
        bytes <= VISIBLE_FRAME_BYTES,
        "{bytes} bytes on a visible frame"
    );
}

struct Run {
    game: test_game::Game,
    battle: Battle,
    rng: Rng,
    scene: BattleScene,
    prev: Buttons,
    frames: u32,
    choices: Vec<Choice>,
    /// Visible frames that uploaded part of a pic (`MonSprite::load_step`).
    pic_steps: u32,
    /// Drive through `BattleScene::step` (J's path) instead of `update` + `choose`; the
    /// choices are not recorded then.
    use_step: bool,
}

impl Run {
    fn new(kind: BattleKind, player: &[Pokemon], opponents: &[Pokemon], seed: u32) -> Run {
        let game = game();
        let mut rng = Rng::new(seed);
        let setup = BattleSetup {
            kind,
            opponents: party(opponents),
            environment: Environment::Grass,
        };
        let mut battle = Battle::new(&game, setup.clone(), &party(player), &mut rng);
        let mut scene = BattleScene::new(&setup);
        let mut events = Vec::new();
        battle.start(&game, &mut rng, &mut events);
        scene.push(&mut events);
        Run {
            game,
            battle,
            rng,
            scene,
            prev: NONE,
            frames: 0,
            choices: Vec::new(),
            pic_steps: 0,
            use_step: false,
        }
    }

    fn wild(player: &[Pokemon], opponents: &[Pokemon], seed: u32) -> Run {
        Run::new(
            BattleKind::Wild {
                first_battle: false,
            },
            player,
            opponents,
            seed,
        )
    }

    /// Loads while black; returns the frames it took.
    fn load(&mut self) -> u32 {
        let mut n = 0;
        loop {
            let mut f = Frame::default();
            let done = self.scene.load(&self.game, &self.battle, &mut f);
            assert!(f.is_black());
            assert!(f.validate().is_ok());
            let bytes = upload_bytes(&f);
            assert!(bytes <= BLACK_FRAME_BYTES, "{bytes} bytes in load step {n}");
            n += 1;
            if done {
                return n;
            }
            assert!(n < 64);
        }
    }

    /// One frame with `cur` held; a choice goes to the core.
    fn frame(&mut self, cur: Buttons) -> Frame {
        let input = Input::new(self.prev, cur);
        self.prev = cur;
        let mut f = Frame::default();
        if self.use_step {
            let (g, b, rng) = (&self.game, &mut self.battle, &mut self.rng);
            self.scene.step(g, b, rng, &input, &mut f);
        } else if let Some(c) = self.scene.update(&self.game, &self.battle, &input, &mut f) {
            self.choices.push(c);
            let mut events = Vec::new();
            self.battle
                .choose(&self.game, &mut self.rng, c, &mut events);
            self.scene.push(&mut events);
        }
        if !f.is_black() {
            check_visible(&f);
        }
        let pic = |c: &Cmd| {
            matches!(c, Cmd::ObjTiles { first, .. }
                if *first < obj::TRAINER_PIC)
        };
        if !f.is_black() && f.cmds.iter().any(pic) {
            self.pic_steps += 1;
        }
        self.frames += 1;
        f
    }

    /// Frames until `done`, pressing A every 4th frame.
    fn mash_until(&mut self, done: impl Fn(&BattleScene) -> bool) {
        while !done(&self.scene) {
            let b = if self.frames.is_multiple_of(4) {
                A
            } else {
                NONE
            };
            self.frame(b);
            assert!(self.frames < 20_000, "battle never ended");
        }
    }

    fn hash(&self) -> u64 {
        let mut h = Fnv64::new();
        self.scene.hash(&mut h);
        self.battle.hash(&mut h);
        h.finish()
    }
}

fn starter() -> Pokemon {
    mon(
        FIRE,
        5,
        [20, 30, 10, 50, 10, 10],
        &[MEGA_HIT, TACKLE, GROWL, HARDEN],
    )
}

fn foe(level: u8) -> Pokemon {
    mon(NORMAL, level, [25, 5, 5, 5, 5, 5], &[TACKLE])
}

#[test]
fn prompt_detection() {
    assert!(ends_with_prompt(&[0xBB, 0xFB, 0xFF]));
    assert!(!ends_with_prompt(&[0xBB, 0xFF]));
    assert!(!ends_with_prompt(&[0xFF]));
}

#[test]
fn load_is_black_chunked_and_shows_nothing() {
    let mut r = Run::wild(&[starter()], &[foe(3)], 1);
    // Control + palettes, 3 environment chunks (300 tiles) and the map, the textbox, the pics,
    // the trainer pics and the two healthboxes.
    assert_eq!(r.load(), 10);
    let mut f = Frame::default();
    assert!(r.scene.load(&r.game, &r.battle, &mut f));
    assert!(f.cmds.is_empty());
}

/// A synthetic event list: frame counts, the frame the menu opens and input → choice.
#[test]
fn scripted_playback_and_menu_input() {
    let mut r = Run::wild(&[starter()], &[foe(3)], 1);
    assert_eq!(r.battle.need(), Need::Action);
    // Replace the core's intro with a script.
    r.scene.queue.clear();
    let msg = |id, wait| {
        BattleEvent::Msg(Msg {
            id,
            attacker: Side::Player,
            target: Side::Opponent,
            active: Side::Opponent,
            mov: None,
            buffs: [Buff::None; 3],
            wait,
            slots: [0, 0],
        })
    };
    let mut events = alloc::vec![
        BattleEvent::WildAppear,
        msg(Str::Intro1, MsgWait::Frames(64)),
        msg(Str::GoPkmn, MsgWait::Frames(0)),
        BattleEvent::SendOut {
            side: Side::Player,
            party: 0,
        },
    ];
    r.scene.push(&mut events);
    r.load();
    // Frame by frame, without input: when each event starts and when the menu opens.
    let mut starts = Vec::new();
    let mut menu_at = None;
    let mut last_len = r.scene.queue.len();
    for n in 0..1000 {
        let f = r.frame(NONE);
        assert_ne!(f.brightness, FADE_MAX);
        if r.scene.queue.len() != last_len {
            starts.push(n);
            last_len = r.scene.queue.len();
        }
        if r.scene.menu == Menu::Action {
            menu_at = Some(n);
            break;
        }
    }
    // WildAppear: 120 frames of slide (240 px at 2 px/frame), 5 frames of its gray tint
    // fading out (10 down by 2), then the healthbox slide (115 px at 5 px/frame, 23 frames):
    // 148. "Intro1" holds 64 frames after it is printed; GoPkmn has no wait; the send-out
    // waits 31 frames after the trainer starts sliding.
    assert_eq!(starts.len(), 4);
    assert_eq!(starts[0], 0);
    assert_eq!(starts[1], 120 + 5 + 23, "{starts:?}");
    assert!(starts[2] >= starts[1] + 64, "{starts:?}");
    assert!(starts[3] > starts[2]);
    let menu_at = menu_at.expect("the action menu opens");
    assert!(menu_at >= starts[3] + 31, "{menu_at}");
    assert_eq!(
        (starts.clone(), menu_at),
        (alloc::vec![0, 148, 249, 254], 341)
    );
    // Nothing is chosen without input.
    for _ in 0..10 {
        r.frame(NONE);
    }
    assert!(r.choices.is_empty());
    // FIGHT, then the first move.
    r.frame(A);
    assert_eq!(r.scene.menu, Menu::Move);
    r.frame(NONE);
    r.frame(A);
    assert_eq!(r.choices, [Choice::Move(0)]);
    assert_eq!(r.scene.menu, Menu::None);
}

#[test]
fn back_from_the_move_menu_and_run() {
    // The player is faster: Gen3 `TryRunFromBattle` always succeeds.
    let mut r = Run::wild(&[starter()], &[foe(3)], 2);
    r.load();
    r.mash_until(|s| s.menu == Menu::Action);
    r.frame(NONE);
    r.frame(A);
    assert_eq!(r.scene.menu, Menu::Move);
    r.frame(Buttons::B);
    assert_eq!(r.scene.menu, Menu::Action);
    // RUN is bottom right.
    for b in [Buttons::DOWN, NONE, Buttons::RIGHT, NONE, A] {
        r.frame(b);
    }
    assert_eq!(r.choices, [Choice::Run]);
    r.mash_until(|s| s.finished().is_some());
    assert_eq!(r.scene.finished(), Some(BattleOutcome::Ran));
    assert!(r.frame(NONE).is_black());
}

#[test]
fn bag_and_pokemon_do_nothing() {
    let mut r = Run::wild(&[starter()], &[foe(3)], 2);
    r.load();
    r.mash_until(|s| s.menu == Menu::Action);
    for b in [
        NONE,
        Buttons::RIGHT,
        NONE,
        A,
        NONE,
        Buttons::DOWN,
        NONE,
        Buttons::LEFT,
        NONE,
        A,
    ] {
        r.frame(b);
    }
    assert_eq!(r.scene.menu, Menu::Action);
    assert!(r.choices.is_empty());
}

/// A whole wild battle from the test data: a knock-out, exp, a level-up and two moves learned
/// over a full moveset, played with A pressed every 4th frame.
#[test]
fn full_wild_battle_golden() {
    let mut r = Run::wild(&[starter()], &[foe(11)], 7);
    r.load();
    r.mash_until(|s| s.finished().is_some());
    assert_eq!(r.scene.finished(), Some(BattleOutcome::Won));
    // MEGA_HIT, then "learn EMBER?" yes → forget slot 0, "learn QUICK_ATTACK?" yes → slot 0.
    assert_eq!(
        r.choices,
        [
            Choice::Move(0),
            Choice::LearnMove { forget: Some(0) },
            Choice::LearnMove { forget: Some(0) },
        ]
    );
    // The display copy matches the core's party at the end (moves aside: the menus read
    // the battle).
    let (shown, real) = (
        r.scene.parties[0].get(0).unwrap(),
        r.battle.party(Side::Player).get(0).unwrap(),
    );
    assert_eq!(
        (shown.level, shown.exp, shown.hp, shown.stats, shown.evs),
        (real.level, real.exp, real.hp, real.stats, real.evs)
    );
    assert_eq!(shown.level, 6);
    assert_eq!(r.scene.parts.exp.level(), 6);
    // Golden: a regression pin of the whole playback (scene + core state at the end).
    // E2: `Battle::hash` now also covers `charge`, `order`, `sync` and `caught_without_room`.
    assert_eq!((r.frames, r.hash()), (806, 7676758038865392259));
    // J's path, `BattleScene::step`, plays the same battle frame for frame.
    let mut j = Run::wild(&[starter()], &[foe(11)], 7);
    j.use_step = true;
    j.load();
    j.mash_until(|s| s.finished().is_some());
    assert_eq!(j.scene.finished(), Some(BattleOutcome::Won));
    assert_eq!((j.frames, j.hash()), (r.frames, r.hash()));
}

#[test]
fn trainer_battle_second_mon_and_defeat_text() {
    let kind = BattleKind::Trainer {
        trainer: TRAINER,
        defeat: DEFEAT,
    };
    let mut r = Run::new(kind, &[starter()], &[foe(3), foe(4)], 3);
    r.load();
    let mut saw_defeat = false;
    while r.scene.finished().is_none() {
        let b = if r.frames.is_multiple_of(4) { A } else { NONE };
        r.frame(b);
        saw_defeat |= r.scene.play == Play::Defeated(true);
        assert!(r.frames < 20_000);
    }
    assert_eq!(r.scene.finished(), Some(BattleOutcome::Won));
    assert!(saw_defeat);
    assert_eq!(r.choices, [Choice::Move(0), Choice::Move(0)]);
    // The second mon's pic was uploaded in budgeted steps while visible: 64 tiles at most 16
    // per frame, one step shortened by the healthbox redraw sharing the frame.
    assert_eq!(r.pic_steps, 5);
    // E2: `Battle::hash` gained fields; the trainer AI draws its score rolls (same frames).
    assert_eq!((r.frames, r.hash()), (1020, 13928161993706818830));
}

#[test]
fn lost_battle_whiteout_and_forced_switch() {
    // Two weak mons against a fast hard hitter: the first faints, the second goes out by
    // itself (no party screen), then faints too.
    let weak = |species| mon(species, 5, [10, 1, 1, 1, 1, 1], &[GROWL]);
    let mut r = Run::wild(
        &[weak(WATER), weak(FIRE)],
        &[mon(NORMAL, 50, [200, 200, 200, 200, 200, 200], &[MEGA_HIT])],
        4,
    );
    r.load();
    r.mash_until(|s| s.finished().is_some());
    assert_eq!(r.scene.finished(), Some(BattleOutcome::Lost));
    assert!(r.choices.contains(&Choice::Switch(1)));
    assert_eq!(r.scene.out[0], 1);
}

#[test]
fn declining_a_move_asks_to_stop_learning() {
    let mut r = Run::wild(&[starter()], &[foe(11)], 7);
    r.load();
    // Up to the first "learn it?" box.
    r.mash_until(|s| matches!(s.menu, Menu::LearnAsk { .. }));
    // No → "stop learning?" → no → the question again → no → stop? → yes.
    let no = |r: &mut Run| {
        r.frame(Buttons::DOWN);
        r.frame(NONE);
        r.frame(A);
        r.frame(NONE);
    };
    no(&mut r);
    r.mash_until_menu(|m| matches!(m, Menu::StopAsk { .. }));
    no(&mut r);
    assert!(matches!(r.scene.menu, Menu::Retry { .. }));
    r.mash_until_menu(|m| matches!(m, Menu::LearnAsk { .. }));
    no(&mut r);
    r.mash_until_menu(|m| matches!(m, Menu::StopAsk { .. }));
    r.frame(A);
    assert_eq!(
        r.choices,
        [Choice::Move(0), Choice::LearnMove { forget: None }]
    );
}

impl Run {
    /// Frames without input until the menu state matches.
    fn mash_until_menu(&mut self, done: impl Fn(Menu) -> bool) {
        while !done(self.scene.menu) {
            self.frame(NONE);
            assert!(self.frames < 20_000);
        }
    }
}

#[test]
fn hash_follows_playback() {
    let mut r = Run::wild(&[starter()], &[foe(3)], 1);
    let h0 = r.hash();
    r.load();
    let h1 = r.hash();
    assert_ne!(h0, h1);
    r.frame(NONE);
    assert_ne!(r.hash(), h1);
    // Same inputs, same hashes.
    let mut a = Run::wild(&[starter()], &[foe(3)], 1);
    a.load();
    a.frame(NONE);
    assert_eq!(a.hash(), r.hash());
    // Events that differ in any field hash differently.
    let code = |e: BattleEvent| {
        let mut h = Fnv64::new();
        hash_event(&e, &mut h);
        h.finish()
    };
    let hit = |target, eff| BattleEvent::Hit { target, eff };
    let codes = [
        code(hit(Side::Player, Effectiveness::Super)),
        code(hit(Side::Opponent, Effectiveness::Super)),
        code(hit(Side::Player, Effectiveness::NotVery)),
        code(BattleEvent::End(BattleOutcome::Won)),
        code(BattleEvent::End(BattleOutcome::Drew)),
        code(script_msg(Str::GoPkmn, MsgWait::Button)),
        code(script_msg(Str::Intro1, MsgWait::Button)),
        code(script_msg(
            Str::TypeName(crate::mon::Type::Fire),
            MsgWait::Button,
        )),
        code(script_msg(
            Str::TypeName(crate::mon::Type::Water),
            MsgWait::Button,
        )),
        code(script_msg(Str::GoPkmn, MsgWait::Frames(64))),
    ];
    for (i, a) in codes.iter().enumerate() {
        assert!(
            !codes[i + 1..].contains(a),
            "event {i} hashes like a later one"
        );
    }
}

/// A message about the opponent, no buffers.
fn script_msg(id: Str, wait: MsgWait) -> BattleEvent {
    BattleEvent::Msg(Msg {
        id,
        attacker: Side::Player,
        target: Side::Opponent,
        active: Side::Opponent,
        mov: None,
        buffs: [Buff::None; 3],
        wait,
        slots: [0, 0],
    })
}

/// A loaded battle at its first action menu, the menu dropped so injected events play.
fn at_first_menu() -> Run {
    let mut r = Run::wild(&[starter()], &[foe(3)], 1);
    r.load();
    r.mash_until(|s| s.menu == Menu::Action);
    r.frame(NONE);
    r.scene.menu = Menu::None;
    r
}

#[test]
fn a_draw_finishes_as_drew() {
    let mut r = at_first_menu();
    r.scene
        .queue
        .push_back(BattleEvent::End(BattleOutcome::Drew));
    // `End` starts the 16-frame fade in the frame it is played.
    for _ in 0..16 {
        assert_eq!(r.scene.finished(), None);
        r.frame(NONE);
    }
    assert_eq!(r.scene.finished(), Some(BattleOutcome::Drew));
    assert!(r.frame(NONE).is_black());
}

/// A `Button` message whose text ends with `\p` (the test game's `PlayerWhiteout2`) needs one
/// press, not two.
#[test]
fn prompt_ending_button_message_needs_one_press() {
    let mut r = at_first_menu();
    r.scene.queue.extend([
        script_msg(Str::PlayerWhiteout2, MsgWait::Button),
        BattleEvent::End(BattleOutcome::Lost),
    ]);
    for _ in 0..200 {
        r.frame(NONE);
    }
    assert_eq!(r.scene.play, Play::Msg, "waits for the prompt");
    r.frame(A);
    r.frame(NONE);
    assert_eq!(r.scene.play, Play::Ending);
}

#[test]
fn back_from_the_learn_move_menu_asks_to_stop_learning() {
    let mut r = Run::wild(&[starter()], &[foe(11)], 7);
    r.load();
    r.mash_until(|s| matches!(s.menu, Menu::LearnAsk { .. }));
    r.frame(NONE);
    r.frame(A);
    assert!(matches!(r.scene.menu, Menu::LearnPick { .. }));
    r.frame(NONE);
    r.frame(Buttons::B);
    assert!(matches!(r.scene.menu, Menu::StopMsg { .. }));
    r.mash_until_menu(|m| matches!(m, Menu::StopAsk { .. }));
    r.frame(A);
    assert_eq!(
        r.choices,
        [Choice::Move(0), Choice::LearnMove { forget: None }]
    );
}

#[test]
fn hp_drain_moves_the_numbers_with_the_bar() {
    let mut r = Run::wild(&[starter()], &[foe(3)], 1);
    r.load();
    r.mash_until(|s| s.menu == Menu::Action);
    r.scene.queue.push_back(BattleEvent::Hp {
        side: Side::Player,
        from: 20,
        to: 4,
    });
    r.scene.menu = Menu::None;
    let mut seen = Vec::new();
    while r.scene.queue.len() == 1 || r.scene.play == Play::Hp(Side::Player) {
        r.frame(NONE);
        let (bar, shown) = (r.scene.parts.bars[0].value(), r.scene.parts.boxes[0].hp());
        // The healthbox numbers follow the bar frame by frame.
        if r.scene.play == Play::Hp(Side::Player) {
            assert_eq!(shown, bar);
        }
        seen.push(shown);
        assert!(r.frames < 5000);
    }
    assert_eq!(*seen.last().unwrap(), 4);
    assert!(seen.len() > 2, "{seen:?}");
    assert!(seen.windows(2).all(|w| w[1] <= w[0]));
    assert_eq!(r.scene.parties[0].get(0).unwrap().hp, 4);
}
