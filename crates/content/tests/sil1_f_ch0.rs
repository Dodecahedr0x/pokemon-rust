//! SIL1 Task F: Chapter 0 end to end, native, in the Silice flavour (`PKR_GAME=silice`).
//!
//! From a new game, buttons only: the bar (Bar_Sc01 to Sc04, with the exit guard while the
//! Mallette is left behind), the night chase, the house (Player_House_Sc01 to Sc03), the morning
//! (Brico_Bourg_Sc02 and Sc03: the starter and the gifts), the hidden Potion, the north exit
//! (Sc04), then Route S1: the first wild battle, the walker's berries, a trainer by sight, the
//! items, and `VAR_SIL_STORY` 9 at the Bois-Éther edge. It asserts the story steps, flags,
//! vars, bag and party, and the scene texts by text id (a `GameData` wrapper records every
//! `text(id)` the world prints), never a French literal.
//!
//! The engine directly (`engine::World` over `content::Game`), since the native shells do not
//! build as Silice yet (SIL1 Task D). A planner walks the player (BFS on collision, behaviors
//! and objects, as the explorer's `path.rs`), and a settle loop presses A through scripts,
//! messages, yes/no boxes (yes), the starter screen and battles (the first move).

#![cfg(pkr_silice)]

use content::Game;
use content::consts::*;
use core::cell::{Cell, RefCell};
use engine::World;
use engine::battle::BattleOutcome;
use engine::data::{
    Behavior, Dir, Emote, Evolution, FlagId, Font, FormChange, GameData, GfxId, ItemId, ItemInfo,
    MapData, MapId, NewGame, ScriptId, SpecialIds, SpeciesId, SpriteData, TextId, TrainerId,
    UiText, WindowGfx, block_collision,
};
use engine::input::Buttons;
use engine::script::{Ctx, Report, Script};
use engine::world::Event;
use std::collections::{BTreeSet, VecDeque};

// ---- a GameData that records the texts the world prints ----

struct Spy {
    texts: RefCell<Vec<TextId>>,
    /// Chapter 1's Lame-Frequence, granted from the test: the brambles' script then runs with
    /// `FLAG_SIL_MODULE_COUPE` set (the world has no flag setter).
    module: Cell<bool>,
}

/// The `ScriptId` of script `name`.
fn script_id(name: &str) -> ScriptId {
    let i = (content::data::SCRIPT_NAMES.iter())
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("no script {name}"));
    ScriptId(i as u16)
}

impl GameData for Spy {
    fn map(&self, id: MapId) -> &'static MapData {
        Game.map(id)
    }
    fn has_map(&self, id: MapId) -> bool {
        Game.has_map(id)
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
        let mut t = self.texts.borrow_mut();
        if t.last() != Some(&id) {
            t.push(id);
        }
        Game.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Game.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        Game.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if self.module.get() && id == script_id("BricoBourg_EventScript_Ronces") {
            return Box::pin(async move {
                ctx.set_flag(FLAG_SIL_MODULE_COUPE, true).await;
                Game.script(id, ctx).await;
            });
        }
        Game.script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn build_id(&self) -> u64 {
        Game.build_id()
    }
    fn battle(&self) -> &'static engine::mon::BattleTables {
        Game.battle()
    }
    fn wild_encounters(
        &self,
        map: MapId,
        wild_set: u16,
    ) -> Option<&'static engine::mon::LandEncounters> {
        Game.wild_encounters(map, wild_set)
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static engine::mon::TrainerData> {
        Game.trainer(id)
    }
    fn battle_text(&self, s: engine::battle::Str) -> &'static [u8] {
        Game.battle_text(s)
    }
    fn battle_gfx(&self) -> &'static engine::battle::BattleGfx {
        Game.battle_gfx()
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        Game.heal_location(id)
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        Game.item(id)
    }
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
        Game.multichoice(list)
    }
    fn std_string(&self, id: u8) -> Option<&'static [u8]> {
        Game.std_string(id)
    }
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        Game.trainer_flag_of(script)
    }
    fn trainer_battle_kind(&self, script: ScriptId) -> Option<u8> {
        Game.trainer_battle_kind(script)
    }
    fn emote(&self, e: Emote) -> Option<&'static SpriteData> {
        Game.emote(e)
    }
    fn trainer_money(&self, id: TrainerId) -> Option<u8> {
        Game.trainer_money(id)
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        Game.evolutions(species)
    }
    fn hold_effect(&self, id: ItemId) -> Option<engine::mon::HoldEffect> {
        Game.hold_effect(id)
    }
    fn form_changes(&self) -> &'static [FormChange] {
        Game.form_changes()
    }
}

// ---- the driver ----

/// Every frame of the run, at most (about 14 minutes of play).
const FRAME_LIMIT: u64 = 50_000;

struct Run {
    w: World,
    g: Spy,
    frame: u64,
    battles: Vec<BattleOutcome>,
    /// The fallbacks the scripts reported (`Report::Degraded`).
    degraded: BTreeSet<&'static str>,
    /// Frames spent on the starter screen (the first ones move the hand right).
    starter_frames: u32,
}

impl Run {
    fn new() -> Run {
        let g = Spy {
            texts: RefCell::new(Vec::new()),
            module: Cell::new(false),
        };
        Run {
            w: World::new(&g),
            g,
            frame: 0,
            battles: Vec::new(),
            degraded: BTreeSet::new(),
            starter_frames: 0,
        }
    }

    fn step(&mut self, keys: Buttons) {
        self.w.step(&self.g, keys);
        self.frame += 1;
        for e in self.w.events() {
            if let Event::Report(Report::Degraded(name)) = e {
                self.degraded.insert(*name);
            }
            if let Event::BattleEnded(o) = e {
                assert!(
                    !matches!(o, BattleOutcome::Lost | BattleOutcome::Drew),
                    "a battle was lost at frame {} on {:?}",
                    self.frame,
                    self.w.map()
                );
                self.battles.push(*o);
            }
        }
        assert!(
            self.frame < FRAME_LIMIT,
            "out of frames on {:?} at {:?}, waiting on {:?}",
            self.w.map(),
            self.pos(),
            self.w.script_waiting()
        );
    }

    fn pos(&self) -> (i16, i16) {
        self.w.objects().player().pos()
    }

    fn busy(&self) -> bool {
        let w = &self.w;
        w.script_running()
            || w.in_battle()
            || w.transition_active()
            || w.starter().is_some()
            || !w.field_active()
            || w.text().is_open()
            || !w.objects().player().is_settled()
    }

    /// Presses A every other frame while anything but the free field runs (scripts, messages,
    /// yes/no on yes, the starter screen, battles on the first move), until the field has been
    /// free for a few frames.
    fn settle(&mut self) {
        let mut quiet = 0;
        let mut i = 0u32;
        while quiet < 6 {
            if self.busy() {
                quiet = 0;
                // The starter screen: the right ball (TOTODILE, the sturdiest at level 5 for the
                // route's battles on the first move), then A.
                let press = if self.w.starter().is_some() && self.starter_frames < 120 {
                    self.starter_frames += 1;
                    Buttons::RIGHT
                } else {
                    Buttons::A
                };
                self.step(if i.is_multiple_of(2) {
                    press
                } else {
                    Buttons::NONE
                });
            } else {
                quiet += 1;
                self.step(Buttons::NONE);
            }
            i += 1;
        }
    }

    /// Turns the player toward `d` (a tap) without stepping.
    fn face(&mut self, d: Dir) {
        self.settle();
        if self.w.objects().player().facing != d {
            self.step(dir_button(d));
            self.settle();
        }
        assert_eq!(self.w.objects().player().facing, d);
    }

    /// Faces `d` and presses A (talk, read, pick up), then settles.
    fn interact(&mut self, d: Dir) {
        self.face(d);
        self.step(Buttons::A);
        self.step(Buttons::NONE);
        self.settle();
    }

    /// One step toward `d` (held until the player leaves its cell or a script starts), settled.
    fn step_toward(&mut self, d: Dir) {
        self.settle();
        let p = self.pos();
        let mut n = 0;
        while self.pos() == p && !self.w.script_running() && n < 40 {
            self.step(dir_button(d));
            n += 1;
        }
        self.settle();
    }

    /// Holds `d` until the map changes (doors, mats, stairs, connections).
    fn enter(&mut self, d: Dir, to: MapId) {
        self.settle();
        let from = self.w.map();
        let mut n = 0;
        while self.w.map() == from {
            self.step(dir_button(d));
            n += 1;
            assert!(
                n < 200,
                "holding {d:?} at {:?} never left {from:?}",
                self.pos()
            );
        }
        self.settle();
        assert_eq!(self.w.map(), to);
    }

    /// Walks to `target` on the current map (re-planned every step; scripts that start on the
    /// way are settled). `avoid`: cells never planned through (trainers' sight lines).
    fn walk_to(&mut self, target: (i16, i16), avoid: &[(i16, i16)]) {
        let map = self.w.map();
        let mut stuck = 0;
        loop {
            self.settle();
            assert_eq!(self.w.map(), map, "walking to {target:?} left the map");
            let p = self.pos();
            if p == target {
                return;
            }
            let d = self.plan(p, target, avoid).unwrap_or_else(|| {
                panic!("no way from {p:?} to {target:?} on {map:?}");
            });
            let mut n = 0;
            while self.pos() == p && !self.busy() && n < 40 {
                self.step(dir_button(d));
                n += 1;
            }
            if self.pos() == p {
                stuck += 1;
                assert!(stuck < 20, "stuck at {p:?} walking to {target:?}");
            }
        }
    }

    /// The first step of a shortest path, through non-grass cells when there is such a path.
    fn plan(&self, from: (i16, i16), to: (i16, i16), avoid: &[(i16, i16)]) -> Option<Dir> {
        let grid = Grid::new(&self.g, &self.w, avoid);
        grid.first_dir(from, to, true)
            .or_else(|| grid.first_dir(from, to, false))
    }

    fn has_text(&self, t: TextId) -> bool {
        self.g.texts.borrow().contains(&t)
    }

    /// How many times `t` was printed (repeats in a row count once).
    fn times(&self, t: TextId) -> usize {
        self.g.texts.borrow().iter().filter(|x| **x == t).count()
    }
}

fn dir_button(d: Dir) -> Buttons {
    match d {
        Dir::Up => Buttons::UP,
        Dir::Down => Buttons::DOWN,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    }
}

/// The planner's view of the current map: collision, behaviors (walls, ledges one way, warps
/// as targets only, tall grass), the objects' cells.
struct Grid<'a> {
    game: &'a Spy,
    map: &'static MapData,
    occupied: BTreeSet<(i16, i16)>,
    avoid: &'a [(i16, i16)],
}

impl<'a> Grid<'a> {
    fn new(game: &'a Spy, w: &World, avoid: &'a [(i16, i16)]) -> Grid<'a> {
        let occupied = (w.objects().iter())
            .filter(|o| o.local_id != engine::data::PLAYER)
            .flat_map(|o| [o.pos(), o.prev])
            .collect();
        Grid {
            game,
            map: game.map(w.map()),
            occupied,
            avoid,
        }
    }

    fn in_bounds(&self, (x, y): (i16, i16)) -> bool {
        engine::grid::in_bounds(self.map, x, y)
    }

    fn block(&self, (x, y): (i16, i16)) -> u16 {
        self.map.blocks[y as usize * self.map.width as usize + x as usize]
    }

    fn behavior(&self, c: (i16, i16)) -> Behavior {
        if !self.in_bounds(c) {
            return Behavior::Blocked;
        }
        engine::flow::block_behavior(self.game, self.map, self.block(c))
    }

    fn walkable(&self, c: (i16, i16), avoid_grass: bool) -> bool {
        self.in_bounds(c)
            && block_collision(self.block(c)) == 0
            && self.behavior(c) != Behavior::Blocked
            && !self.behavior(c).enters_warp()
            && !self.occupied.contains(&c)
            && !self.avoid.contains(&c)
            && !(avoid_grass && self.behavior(c) == Behavior::TallGrass)
    }

    fn step(&self, c: (i16, i16), d: Dir, grass: bool, target: (i16, i16)) -> Option<(i16, i16)> {
        let (dx, dy) = d.delta();
        let mut n = (c.0 + dx, c.1 + dy);
        if !self.in_bounds(n) || Behavior::walled(self.behavior(c), self.behavior(n), d) {
            return None;
        }
        if let Behavior::Ledge(ld) = self.behavior(n) {
            if ld != d {
                return None;
            }
            n = (n.0 + dx, n.1 + dy);
        }
        (self.in_bounds(n) && (n == target || self.walkable(n, grass))).then_some(n)
    }

    fn first_dir(&self, from: (i16, i16), to: (i16, i16), grass: bool) -> Option<Dir> {
        let mut seen = BTreeSet::from([from]);
        let mut queue = VecDeque::from([(from, None::<Dir>)]);
        while let Some((c, d0)) = queue.pop_front() {
            if c == to {
                return d0;
            }
            for d in Dir::ALL {
                if let Some(n) = self.step(c, d, grass, to)
                    && seen.insert(n)
                {
                    queue.push_back((n, Some(d0.unwrap_or(d))));
                }
            }
        }
        None
    }
}

/// Asserts that `want` were printed in this order (other texts may come between).
fn assert_printed_in_order(r: &Run, want: &[TextId]) {
    let texts = r.g.texts.borrow();
    let mut it = texts.iter();
    for t in want {
        assert!(
            it.any(|x| x == t),
            "{t:?} not printed (in order); printed: {:?}",
            *texts
        );
    }
}

fn beaten(r: &Run, t: TrainerId) -> bool {
    r.w.flag(FlagId(0x500 + t.0))
}

/// The sight lines of the Route S1 trainers (§5), left out of the plans.
const ROUTE_S01_SIGHT: &[(i16, i16)] = &[
    (4, 24),
    (5, 24),
    (6, 24),
    (12, 19),
    (12, 20),
    (12, 21),
    (4, 12),
    (5, 12),
    (6, 12),
    (8, 4),
    (9, 4),
    (10, 4),
    (11, 4),
];

#[test]
fn chapter_0_plays_from_a_new_game_to_the_bois_ether_edge() {
    assert!(!content::scripts::TRAINER_SCRIPTS.is_empty());
    let mut r = Run::new();

    // ---- the Shaker, the evening (story 0) ----
    r.settle();
    assert_eq!(r.w.map(), MAP_BRICO_BOURG_BAR);
    assert!(r.w.flag(FLAG_SIL_INTRO_DONE));
    assert_eq!(r.w.var(VAR_SIL_STORY), 0);
    assert_printed_in_order(&r, &[Bar_Sc01_T1, Bar_Sc01_T2, Bar_Sc01_T3, Bar_Sc01_T4]);

    // A friend's line (Bar_Sc02): GWADA at (6, 4), from (6, 5).
    r.walk_to((6, 5), &[]);
    r.interact(Dir::Up);
    assert!(r.has_text(Bar_Sc02_T2));

    // Julien (Bar_Sc03): the explosion, story 1, the Mallette left on his table.
    r.walk_to((12, 4), &[]);
    r.interact(Dir::Up);
    assert_eq!(r.w.var(VAR_SIL_STORY), 1);
    assert!(r.w.flag(FLAG_SIL_HIDE_BAR_JULIEN));
    assert!(r.w.flag(FLAG_SIL_HIDE_BAR_SBIRE_A));
    assert!(r.w.flag(FLAG_SIL_HIDE_BAR_SBIRE_B));
    assert!(!r.w.flag(FLAG_SIL_HIDE_BAR_MALLETTE));
    assert_printed_in_order(
        &r,
        &[
            Bar_Sc03_T1,
            Bar_Sc03_T2,
            Bar_Sc03_T3,
            Bar_Sc03_T4,
            Bar_Sc03_T5,
            Bar_Sc03_T6,
        ],
    );

    // Out without the Mallette: Mathieu calls the player back to the Shaker's door.
    r.walk_to((7, 9), &[]);
    r.enter(Dir::Down, MAP_BRICO_BOURG);
    assert_eq!(r.pos(), (14, 14));
    r.step_toward(Dir::Down);
    r.settle();
    assert_eq!(r.pos(), (14, 14));
    assert!(r.has_text(Bar_Sc04_T2));
    assert!(!r.w.flag(FLAG_SIL_BRICO_CHASE));
    r.enter(Dir::Up, MAP_BRICO_BOURG_BAR);

    // The Mallette (Bar_Sc04), yes.
    r.walk_to((13, 4), &[]);
    r.interact(Dir::Up);
    assert!(r.w.flag(FLAG_SIL_HAS_MALLETTE));
    assert!(r.w.flag(FLAG_SIL_HIDE_BAR_MALLETTE));
    assert_eq!(r.w.bag().count(ItemId(ITEM_SIL_MALLETTE)), 1);
    assert_printed_in_order(&r, &[Bar_Sc04_T1, Bar_Sc04_T3, Bar_Sc04_T4, Bar_Sc04_T5]);
    // Taken once: the object is gone and the scene does not run again.
    r.interact(Dir::Up);
    assert_eq!(r.times(Bar_Sc04_T1), 1);
    assert_eq!(r.w.bag().count(ItemId(ITEM_SIL_MALLETTE)), 1);
    r.walk_to((7, 9), &[]);
    r.enter(Dir::Down, MAP_BRICO_BOURG);

    // ---- the night chase (Brico_Bourg_Sc01) ----
    r.walk_to((14, 15), &[]);
    assert!(r.w.flag(FLAG_SIL_BRICO_CHASE));
    assert!(r.w.flag(FLAG_SIL_HIDE_BRICO_SBIRE));
    assert_printed_in_order(
        &r,
        &[
            Brico_Bourg_Sc01_T1,
            Brico_Bourg_Sc01_T2,
            Brico_Bourg_Sc01_T3,
        ],
    );
    // The north exit is guarded before the starter.
    r.walk_to((11, 3), &[]);
    r.step_toward(Dir::Up);
    assert!(r.has_text(Brico_Bourg_Block_T1));
    assert_eq!(r.pos(), (11, 3));

    // ---- home (Player_House_Sc01 to Sc03) ----
    r.walk_to((5, 15), &[]);
    r.enter(Dir::Up, MAP_BRICO_BOURG_PLAYER_HOUSE_1F);
    assert_eq!(r.w.var(VAR_SIL_STORY), 2);
    assert!(r.has_text(Player_House_Sc01_T1));
    r.walk_to((8, 2), &[]);
    r.enter(Dir::Up, MAP_BRICO_BOURG_PLAYER_HOUSE_2F);
    // The bed first: not yet.
    r.walk_to((2, 2), &[]);
    r.interact(Dir::Left);
    assert!(r.has_text(Player_House_Sc02_T3));
    assert_eq!(r.w.var(VAR_SIL_STORY), 2);
    // The desk, then the bed.
    r.walk_to((5, 3), &[]);
    r.interact(Dir::Up);
    assert!(r.w.flag(FLAG_SIL_MALLETTE_TRIED));
    r.walk_to((2, 2), &[]);
    r.interact(Dir::Left);
    assert_eq!(r.w.var(VAR_SIL_STORY), 3);
    assert_printed_in_order(
        &r,
        &[
            Player_House_Sc02_T1,
            Player_House_Sc02_T4,
            Player_House_Sc02_T5,
            Player_House_Sc03_T1,
        ],
    );
    r.walk_to((7, 2), &[]);
    r.enter(Dir::Up, MAP_BRICO_BOURG_PLAYER_HOUSE_1F);
    // Maman: the running shoes, once.
    r.walk_to((3, 2), &[]);
    r.interact(Dir::Left);
    assert!(r.w.flag(FLAG_SIL_RUNNING_SHOES));
    assert_printed_in_order(&r, &[Player_House_Sc03_T2, Player_House_Sc03_T3]);

    // ---- the morning: click, the call, the starter (Brico_Bourg_Sc02, Sc03) ----
    r.walk_to((4, 7), &[]);
    r.enter(Dir::Down, MAP_BRICO_BOURG);
    assert_eq!(r.w.var(VAR_SIL_STORY), 5);
    assert_eq!(r.w.var(VAR_SIL_CH0_GATE), 1);
    for f in [
        FLAG_SIL_MALLETTE_OPEN,
        FLAG_SIL_MALLETTE_MAP,
        FLAG_SIL_MALLETTE_DEX,
        FLAG_SIL_MALLETTE_CALLS,
        FLAG_SIL_ITEM_PROLOGUE_POTIONS,
        FLAG_SIL_ITEM_PROLOGUE_BALLS,
        FLAG_SIL_HIDE_BRICO_MATHIEU,
        FLAG_SIL_HIDE_BRICO_GWADA,
        FLAG_SIL_HIDE_BRICO_ADRIEN,
    ] {
        assert!(r.w.flag(f), "{f:?}");
    }
    let starter = r.w.var(VAR_SIL_STARTER);
    assert_eq!(starter, 2);
    assert_eq!(r.w.party().len(), 1);
    assert_eq!(
        r.w.party().get(0).unwrap().species,
        SIL_STARTERS[usize::from(starter)]
    );
    assert_eq!(r.w.bag().count(ItemId(ITEM_POTION)), 2);
    assert_eq!(r.w.bag().count(ItemId(ITEM_POKE_BALL)), 5);
    assert_printed_in_order(
        &r,
        &[
            Brico_Bourg_Sc02_T1,
            Brico_Bourg_Sc02_T2,
            Brico_Bourg_Sc02_T3,
            Brico_Bourg_Sc02_T4,
            Brico_Bourg_Sc03_T1,
            Brico_Bourg_Sc03_T2,
            Brico_Bourg_Sc03_T3,
            Brico_Bourg_Sc03_T4,
            Brico_Bourg_Sc03_T5,
            Brico_Bourg_Sc03_T6,
            Brico_Bourg_Sc03_T7,
            Brico_Bourg_Sc03_T8,
            Brico_Bourg_Sc03_T9,
            Brico_Bourg_Sc03_T10,
            Brico_Bourg_Sc03_T11,
            Brico_Bourg_Sc03_T12,
            Brico_Bourg_Sc03_T13,
        ],
    );

    // The hidden Potion behind the fountain, once.
    r.walk_to((10, 7), &[]);
    r.interact(Dir::Down);
    assert!(r.w.flag(FLAG_SIL_ITEM_BRICO_POTION));
    assert_eq!(r.w.bag().count(ItemId(ITEM_POTION)), 3);
    r.interact(Dir::Down);
    assert_eq!(r.w.bag().count(ItemId(ITEM_POTION)), 3);

    // The Labo's brambles (4, 8): Ch0's sign; with Chapter 1's Lame-Frequence, cut (the
    // generic obstacle), and still cut after a map load (BricoBourg_OnLoad).
    r.walk_to((4, 9), &[]);
    r.interact(Dir::Up);
    assert!(r.has_text(Brico_Bourg_Sign_Ronces));
    assert!(!r.w.flag(FLAG_SIL_BRICO_LABO_RONCES_CUT));
    r.g.module.set(true);
    r.interact(Dir::Up);
    r.g.module.set(false);
    assert!(r.w.flag(FLAG_SIL_BRICO_LABO_RONCES_CUT));
    assert_printed_in_order(&r, &[Bois_Ether_Sc08_T2, Bois_Ether_Sc08_T3]);
    r.step_toward(Dir::Up);
    assert_eq!(r.pos(), (4, 8));
    r.walk_to((5, 15), &[]);
    r.enter(Dir::Up, MAP_BRICO_BOURG_PLAYER_HOUSE_1F);
    r.walk_to((4, 7), &[]);
    r.enter(Dir::Down, MAP_BRICO_BOURG);
    r.walk_to((4, 9), &[]);
    r.step_toward(Dir::Up);
    assert_eq!(r.pos(), (4, 8));
    r.step_toward(Dir::Down);

    // The north exit: the signal (Brico_Bourg_Sc04), then Route S1.
    r.walk_to((11, 1), &[]);
    assert!(r.w.flag(FLAG_SIL_S01_SIGNAL_SEEN));
    assert!(r.has_text(Brico_Bourg_Sc04_T1));
    r.enter(Dir::Up, MAP_ROUTE_S01);

    // ---- Route S1 ----
    // The first wild battle (Route_S01_Sc01).
    let battles = r.battles.len();
    r.walk_to((9, 30), ROUTE_S01_SIGHT);
    assert_eq!(r.w.var(VAR_SIL_STORY), 6);
    assert!(r.battles.len() > battles);
    assert_printed_in_order(&r, &[Route_S01_Sc01_T1, Route_S01_Sc01_T2]);

    // The walker's Oran Berries, after it, once.
    r.walk_to((7, 32), ROUTE_S01_SIGHT);
    r.interact(Dir::Left);
    assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_ORAN));
    assert_eq!(r.w.bag().count(ItemId(ITEM_ORAN_BERRY)), 2);
    assert!(r.has_text(Route_S01_Npc_Promeneuse_Oran));
    r.interact(Dir::Left);
    assert_eq!(r.w.bag().count(ItemId(ITEM_ORAN_BERRY)), 2);

    // Home to heal before the trainers (Maman, from the morning on), back by the south edge.
    let full = |r: &Run| {
        let m = r.w.party().get(0).unwrap();
        m.hp == m.stats[0]
    };
    r.walk_to((8, 39), ROUTE_S01_SIGHT);
    r.enter(Dir::Down, MAP_BRICO_BOURG);
    r.walk_to((5, 15), &[]);
    r.enter(Dir::Up, MAP_BRICO_BOURG_PLAYER_HOUSE_1F);
    r.walk_to((3, 2), &[]);
    r.interact(Dir::Left);
    assert!(full(&r));
    r.walk_to((4, 7), &[]);
    r.enter(Dir::Down, MAP_BRICO_BOURG);
    r.walk_to((11, 0), &[]);
    r.enter(Dir::Up, MAP_ROUTE_S01);

    // A trainer by sight: TÉO sees the player on (6, 24).
    assert!(!beaten(&r, TRAINER_SIL_GAMIN_TEO));
    r.walk_to((6, 25), ROUTE_S01_SIGHT);
    r.walk_to((6, 24), &[]);
    assert!(beaten(&r, TRAINER_SIL_GAMIN_TEO));
    assert!(r.has_text(Route_S01_Teo_Intro));

    // The hidden Antidote, then the item balls.
    r.walk_to((5, 22), ROUTE_S01_SIGHT);
    r.interact(Dir::Left);
    assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_ANTIDOTE));
    assert_eq!(r.w.bag().count(ItemId(ITEM_ANTIDOTE)), 1);
    r.walk_to((14, 23), ROUTE_S01_SIGHT);
    r.interact(Dir::Up);
    assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_POTION));
    assert_eq!(r.w.bag().count(ItemId(ITEM_POTION)), 4);
    r.walk_to((14, 7), ROUTE_S01_SIGHT);
    r.interact(Dir::Up);
    assert!(r.w.flag(FLAG_SIL_ITEM_ROUTE_S01_POKE_BALL));
    assert_eq!(r.w.bag().count(ItemId(ITEM_POKE_BALL)), 6);

    // In sight of Bois-Éther (Route_S01_Sc02): story 9.
    r.walk_to((10, 2), ROUTE_S01_SIGHT);
    assert_eq!(r.w.var(VAR_SIL_STORY), 9);
    assert!(r.has_text(Route_S01_Sc02_T1));

    // No event was dropped, and the fallbacks reported themselves.
    assert_eq!(r.w.events_dropped(), 0);
    for name in [
        "screen_shake",
        "play_se",
        "mallette_call",
        "end_call",
        "set_time_tint",
        "hidden item",
    ] {
        assert!(r.degraded.contains(name), "{name}: {:?}", r.degraded);
    }
}
