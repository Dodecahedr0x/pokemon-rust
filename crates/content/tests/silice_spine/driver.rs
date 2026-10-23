//! The runner of the spine's steps: `engine::World` over `content::Game`, wrapped in a
//! `GameData` spy ([`Spy`]) that records every `text(id)` the world prints and applies the
//! steps' test hooks. A planner walks the player (BFS on collision, behaviors, ledges and
//! objects, as the explorer's `path.rs`); a settle loop presses A through scripts, messages,
//! yes/no boxes (yes), the starter screen (the right ball) and battles (the first move).

use super::{Hook, Step};
use content::Game;
use core::cell::RefCell;
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

/// A hook armed by [`Step::Hook`], applied around the next run of its script.
#[derive(Clone, Copy)]
struct Armed {
    script: ScriptId,
    hook: Hook,
}

pub struct Spy {
    texts: RefCell<Vec<TextId>>,
    hooks: RefCell<Vec<Armed>>,
}

/// The `ScriptId` of script `name`.
pub fn script_id(name: &str) -> ScriptId {
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
        let armed = {
            let mut hooks = self.hooks.borrow_mut();
            let i = hooks.iter().position(|h| h.script == id);
            i.map(|i| hooks.remove(i))
        };
        match armed.map(|a| a.hook) {
            None => Game.script(id, ctx),
            Some(Hook::FlagDuring(_, flag)) => Box::pin(async move {
                ctx.set_flag(flag, true).await;
                Game.script(id, ctx.clone()).await;
                ctx.set_flag(flag, false).await;
            }),
            Some(Hook::MonBefore(_, species, level)) => Box::pin(async move {
                ctx.give_mon(species, level).await;
                Game.script(id, ctx).await;
            }),
        }
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

// ---- the runner ----

pub struct Run {
    pub w: World,
    pub g: Spy,
    pub frame: u64,
    /// Every frame of the run, at most.
    pub limit: u64,
    pub battles: Vec<BattleOutcome>,
    /// The fallbacks the scripts reported (`Report::Degraded`).
    pub degraded: BTreeSet<&'static str>,
    /// Frames spent on the starter screen (the first ones move the hand right).
    starter_frames: u32,
}

impl Run {
    /// A new game, at most `limit` frames.
    pub fn new(limit: u64) -> Run {
        let g = Spy {
            texts: RefCell::new(Vec::new()),
            hooks: RefCell::new(Vec::new()),
        };
        Run {
            w: World::new(&g),
            g,
            frame: 0,
            limit,
            battles: Vec::new(),
            degraded: BTreeSet::new(),
            starter_frames: 0,
        }
    }

    /// Plays `steps` in order; each [`Step::Check`] panics on a failed assertion, naming its
    /// label.
    pub fn play(&mut self, steps: &[Step]) {
        for s in steps {
            match *s {
                Step::Goto { map, x, y, avoid } => {
                    self.settle();
                    assert_eq!(self.w.map(), map, "Goto({x}, {y}) on the wrong map");
                    self.walk_to((x, y), avoid);
                }
                Step::Enter { dir, map } => self.enter(dir, map),
                Step::Interact(d) => self.interact(d),
                Step::Walk(d) => self.step_toward(d),
                Step::Settle => self.settle(),
                Step::Hook(h) => {
                    let name = match h {
                        Hook::FlagDuring(n, _) | Hook::MonBefore(n, _, _) => n,
                    };
                    self.g.hooks.borrow_mut().push(Armed {
                        script: script_id(name),
                        hook: h,
                    });
                }
                Step::Check(label, check) => {
                    self.settle();
                    eprintln!("check {label} (frame {})", self.frame);
                    check(self);
                }
            }
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
            self.frame < self.limit,
            "out of frames on {:?} at {:?}, waiting on {:?} (in battle {}, text open {}, \
             transition {}, field {}, battles {})",
            self.w.map(),
            self.pos(),
            self.w.script_waiting(),
            self.w.in_battle(),
            self.w.text().is_open(),
            self.w.transition_active(),
            self.w.field_active(),
            self.battles.len(),
        );
    }

    pub fn pos(&self) -> (i16, i16) {
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
    /// way are settled). `avoid`: cells never planned through (sight lines, spore grates).
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

    pub fn has_text(&self, t: TextId) -> bool {
        self.g.texts.borrow().contains(&t)
    }

    /// How many times `t` was printed (repeats in a row count once).
    pub fn times(&self, t: TextId) -> usize {
        self.g.texts.borrow().iter().filter(|x| **x == t).count()
    }

    /// Asserts that `want` were printed in this order (other texts may come between).
    pub fn printed_in_order(&self, want: &[TextId]) {
        let texts = self.g.texts.borrow();
        let mut it = texts.iter();
        for t in want {
            assert!(
                it.any(|x| x == t),
                "{t:?} not printed (in order); printed: {:?}",
                *texts
            );
        }
    }

    /// Whether trainer `t` is beaten (Gen3 `TRAINER_FLAG`).
    pub fn beaten(&self, t: TrainerId) -> bool {
        self.w.flag(FlagId(0x500 + t.0))
    }

    /// How many of `item` the bag holds.
    pub fn count(&self, item: u16) -> u32 {
        self.w.bag().count(ItemId(item))
    }

    /// Whether the whole party is at full HP.
    pub fn healed(&self) -> bool {
        self.w.party().iter().all(|m| m.hp == m.stats[0])
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

    /// The layout's block: `setmetatile` changes (doors opened, brambles cut) are not seen, so
    /// the steps cross them with [`Step::Walk`].
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
