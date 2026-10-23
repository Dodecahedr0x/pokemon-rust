//! The runner of the spine's steps: `engine::World` over `content::Game`, wrapped in a
//! `GameData` spy ([`Spy`]) that records every `text(id)` the world prints and applies the
//! steps' test hooks. A planner walks the player (BFS on collision, behaviors, ledges and
//! objects, as the explorer's `path.rs`); a settle loop presses A through scripts, messages and
//! yes/no boxes (yes), takes the [`Step::Choose`] ball on the starter screen, and fights battles
//! with [`best_move`] (the module docs of `silice_spine`).

use super::{Hook, Step};
use content::Game;
use core::cell::RefCell;
use engine::World;
use engine::battle::calc::type_effect;
use engine::battle::scene::OpenMenu;
use engine::battle::{Battle, BattleOutcome, Side};
use engine::data::{
    Behavior, Dir, Emote, Evolution, FlagId, Font, FormChange, GameData, GfxId, ItemId, ItemInfo,
    MapData, MapId, NewGame, ScriptId, SpecialIds, SpeciesId, SpriteData, TextId, TrainerId,
    UiText, WindowGfx, block_collision,
};
use engine::input::Buttons;
use engine::mon::{BattleTables, Status};
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
    /// The index the next choice takes ([`Step::Choose`]; the starter screen's ball).
    choice: u8,
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
            choice: 0,
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
                Step::Choose(i) => self.choice = i,
                Step::Train { a, b, level, heal } => self.train(a, b, level, heal),
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

    /// Presses keys every other frame while anything but the free field runs ([`Run::key`]:
    /// scripts, messages, yes/no on yes, the starter screen, battles), until the field has been
    /// free for a few frames.
    fn settle(&mut self) {
        let mut quiet = 0;
        let mut i = 0u32;
        while quiet < 6 {
            if self.busy() {
                quiet = 0;
                let press = self.key();
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

    /// The settle policy's next key: the starter screen moves to the [`Step::Choose`] ball then
    /// confirms; a battle menu fights with [`best_move`], keeps the known moves when a new one
    /// needs a slot (B), and stops learning (yes); everything else is A (text, yes/no on yes).
    fn key(&self) -> Buttons {
        if let Some(st) = self.w.starter() {
            return match st.selection().cmp(&self.choice) {
                core::cmp::Ordering::Less => Buttons::RIGHT,
                core::cmp::Ordering::Greater => Buttons::LEFT,
                core::cmp::Ordering::Equal => Buttons::A,
            };
        }
        let Some((_, battle, scene)) = self.w.battle() else {
            return Buttons::A;
        };
        match scene.open_menu() {
            Some(OpenMenu::Action(c)) => toward(c, 0),
            Some(OpenMenu::Move(c)) => toward(c, best_move(battle, Game.battle())),
            Some(OpenMenu::LearnAsk(_) | OpenMenu::LearnPick(_)) => Buttons::B,
            Some(OpenMenu::StopAsk(0)) => Buttons::A,
            Some(OpenMenu::StopAsk(_)) => Buttons::UP,
            None => Buttons::A,
        }
    }

    /// [`Step::Train`]: from `a`, steps to `b` and back (encounters are settled on the way) until
    /// the lead's level is `level`; first plays `heal` (then walks back to `a`) whenever the lead
    /// is below half its HP or has a status.
    fn train(&mut self, a: (i16, i16), b: (i16, i16), level: u8, heal: &[Step]) {
        let map = self.w.map();
        let towards = |from: (i16, i16), to: (i16, i16)| {
            Dir::ALL
                .into_iter()
                .find(|d| {
                    let (dx, dy) = d.delta();
                    (from.0 + dx, from.1 + dy) == to
                })
                .expect("Train's cells are neighbours")
        };
        loop {
            self.settle();
            let lead = *self.w.party().get(0).expect("a party to train");
            if lead.level >= level {
                return;
            }
            if lead.hp * 2 < lead.stats[0] || lead.status != Status::None {
                self.play(heal);
                assert_eq!(self.w.map(), map, "Train's heal steps end on its map");
            }
            let p = self.pos();
            if p != a && p != b {
                self.walk_to(a, &[]);
            }
            let p = self.pos();
            self.step_toward(towards(p, if p == a { b } else { a }));
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

/// The key that moves a 2x2 battle menu cursor from `c` toward `want` (column first, then row),
/// or A once it is there (the explorer's `policy::toward`).
fn toward(c: u8, want: u8) -> Buttons {
    if c & 1 != want & 1 {
        if want & 1 == 0 {
            Buttons::LEFT
        } else {
            Buttons::RIGHT
        }
    } else if c & 2 != want & 2 {
        if want & 2 == 0 {
            Buttons::UP
        } else {
            Buttons::DOWN
        }
    } else {
        Buttons::A
    }
}

/// The battle policy's move slot: of the moves with PP, the best expected damage against the
/// opponent (power × accuracy, ×1.5 for the same type, × Gen3's type chart against its types);
/// the first such slot on a tie. With no damaging move, the first with PP; with none, slot 0
/// (the engine uses Struggle).
pub fn best_move(battle: &Battle, t: &BattleTables) -> u8 {
    let me = battle.mon(Side::Player);
    let foe = t.species_info(battle.mon(Side::Opponent).species).types;
    let own = t.species_info(me.species).types;
    let usable = |i: usize| me.moves[i].is_some() && me.pp[i] > 0;
    let score = |i: usize| {
        let m = t.move_info(me.moves[i].expect("usable"));
        if m.power == 0 {
            return 0;
        }
        let typed = type_effect(
            t.type_chart,
            t.foresight_split,
            false,
            m.ty,
            foe,
            m.power,
            100,
        );
        let stab = if own.contains(&m.ty) { 3 } else { 2 };
        let accuracy = if m.accuracy == 0 { 100 } else { m.accuracy };
        i64::from(m.power) * i64::from(typed.damage) * stab * i64::from(accuracy)
    };
    let best = (0..4)
        .filter(|&i| usable(i))
        .map(|i| (score(i), i))
        .filter(|&(s, _)| s > 0)
        .max_by_key(|&(s, i)| (s, core::cmp::Reverse(i)));
    best.map(|(_, i)| i)
        .or_else(|| (0..4).find(|&i| usable(i)))
        .unwrap_or(0) as u8
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
/// as targets only, tall grass), the objects' cells, and the cells scripts repainted
/// (`setmetatile`: doors opened, brambles cut), as the explorer's `path::Grid`.
struct Grid<'a> {
    game: &'a Spy,
    map: &'static MapData,
    /// The field's `setmetatile` overrides, ascending (the world's snapshot).
    overrides: Vec<((i16, i16), u16)>,
    occupied: BTreeSet<(i16, i16)>,
    avoid: &'a [(i16, i16)],
}

impl<'a> Grid<'a> {
    fn new(game: &'a Spy, w: &World, avoid: &'a [(i16, i16)]) -> Grid<'a> {
        let occupied = (w.objects().iter())
            .filter(|o| o.local_id != engine::data::PLAYER)
            .flat_map(|o| [o.pos(), o.prev])
            .collect();
        // A settled field is capturable; otherwise the layout alone.
        let overrides = w
            .snapshot(game)
            .map(|snap| snap.field.overrides)
            .unwrap_or_default();
        Grid {
            game,
            map: game.map(w.map()),
            overrides,
            occupied,
            avoid,
        }
    }

    fn in_bounds(&self, (x, y): (i16, i16)) -> bool {
        engine::grid::in_bounds(self.map, x, y)
    }

    /// The block at an in-bounds cell, the field's overrides first.
    fn block(&self, (x, y): (i16, i16)) -> u16 {
        match self.overrides.binary_search_by_key(&(x, y), |&(c, _)| c) {
            Ok(i) => self.overrides[i].1,
            Err(_) => self.map.blocks[y as usize * self.map.width as usize + x as usize],
        }
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
