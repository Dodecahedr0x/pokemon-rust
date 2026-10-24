//! The runner of the spine's steps: `engine::World` over `content::Game`, wrapped in a
//! `GameData` spy ([`Spy`]) that records every `text(id)` the world prints and applies the
//! steps' test hooks. A planner walks the player (BFS on collision, behaviors, ledges and
//! objects, as the explorer's `path.rs`); a settle loop presses A through scripts, messages and
//! yes/no boxes (yes), takes the [`Step::Choose`] ball on the starter screen, and fights battles
//! with [`best_move`] (the module docs of `silice_spine`).

use super::{Hook, Step, TeachTm};
use content::Game;
use core::cell::RefCell;
use engine::World;
use engine::battle::calc::type_effect;
use engine::battle::scene::OpenMenu;
use engine::battle::{Battle, BattleOutcome, Need, Side};
use engine::data::{
    Behavior, Dir, Emote, Evolution, FlagId, Font, FormChange, GameData, GfxId, ItemId, ItemInfo,
    MapData, MapId, MoveId, NewGame, ScriptId, SpecialIds, SpeciesId, SpriteData, TextId,
    TrainerId, UiText, WindowGfx, block_collision, block_elevation,
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
    start: Option<Start>,
}

/// A chapter's start state for a run that does not play the chapters before it
/// ([`Run::starting_at`]): the new game's map, cell and facing, and a setup script run right
/// after Silice's own new-game script (before the first map loads), which sets the vars, flags,
/// bag and party the earlier chapters would have left.
#[derive(Clone, Copy)]
pub struct Start {
    pub map: MapId,
    pub x: i16,
    pub y: i16,
    pub facing: Dir,
    pub setup: fn(Ctx) -> Script,
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
        if let Some(start) = self.start
            && id == content::NEW_GAME_SCRIPT
        {
            return Box::pin(async move {
                Game.script(id, ctx.clone()).await;
                // Every start state comes after Chapter 0's starter, which sets it
                // (`prelude::choose_starter`).
                ctx.set_flag(content::consts::FLAG_SYS_POKEMON_GET, true)
                    .await;
                (start.setup)(ctx).await;
            });
        }
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
        let game = Game.new_game();
        match self.start {
            None => game,
            Some(s) => NewGame {
                map: s.map,
                x: s.x,
                y: s.y,
                facing: s.facing,
                ..game
            },
        }
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
    fn map_type(&self, map: MapId) -> engine::data::MapType {
        Game.map_type(map)
    }
    // Everything else answers as the game does (the spine plays the real game's data).
    fn map_layout(&self, id: MapId, layout: u16) -> Option<&'static MapData> {
        Game.map_layout(id, layout)
    }
    fn partner(&self) -> Option<&'static engine::mon::PartnerData> {
        Game.partner()
    }
    fn start_menu(&self) -> &'static [engine::data::StartItem] {
        Game.start_menu()
    }
    fn field_move_gate(&self, m: engine::data::FieldMove) -> engine::data::FieldMoveGate {
        Game.field_move_gate(m)
    }
    fn item_use(&self, id: ItemId) -> engine::data::ItemUse {
        Game.item_use(id)
    }
    fn mart_bonus(&self) -> Option<(ItemId, u16, ItemId)> {
        Game.mart_bonus()
    }
    fn item_description(&self, id: ItemId) -> Option<&'static [u8]> {
        Game.item_description(id)
    }
    fn move_description(&self, id: MoveId) -> Option<&'static [u8]> {
        Game.move_description(id)
    }
    fn is_hm_move(&self, m: MoveId) -> bool {
        Game.is_hm_move(m)
    }
    fn tm_move(&self, id: ItemId) -> Option<MoveId> {
        Game.tm_move(id)
    }
    fn can_learn_tm(&self, species: SpeciesId, tm: ItemId) -> bool {
        Game.can_learn_tm(species, tm)
    }
    fn ui_gfx(&self, g: engine::data::UiGfx) -> Option<&'static engine::data::UiPicture> {
        Game.ui_gfx(g)
    }
    fn window_frame(&self, n: u8) -> Option<&'static WindowGfx> {
        Game.window_frame(n)
    }
    fn item_icon(&self, id: ItemId) -> Option<&'static engine::data::Icon> {
        Game.item_icon(id)
    }
    fn mon_icon(&self, species: SpeciesId) -> Option<&'static engine::data::Icon> {
        Game.mon_icon(species)
    }
    fn in_game_trade(&self, n: u8) -> Option<&'static engine::data::InGameTrade> {
        Game.in_game_trade(n)
    }
    fn nature_name(&self, n: u8) -> Option<&'static [u8]> {
        Game.nature_name(n)
    }
    fn step_callback(&self, map: MapId) -> Option<engine::data::StepCallback> {
        Game.step_callback(map)
    }
    fn hole_warp(&self, map: MapId) -> Option<MapId> {
        Game.hole_warp(map)
    }
    fn fall_script(&self) -> Option<ScriptId> {
        Game.fall_script()
    }
    fn step_count_script(&self, step: engine::data::StepCount<'_>) -> Option<ScriptId> {
        Game.step_count_script(step)
    }
    fn field_move_script(&self, m: engine::data::FieldMove) -> Option<ScriptId> {
        Game.field_move_script(m)
    }
    fn waterfall_refused_script(&self) -> Option<ScriptId> {
        Game.waterfall_refused_script()
    }
    fn dive_emerge_script(&self) -> Option<ScriptId> {
        Game.dive_emerge_script()
    }
    fn dive_warp(&self, map: MapId, emerge: bool) -> Option<(MapId, Option<(i16, i16)>)> {
        Game.dive_warp(map, emerge)
    }
    fn fly_destination(&self, i: u8) -> Option<engine::data::FlyDestination> {
        Game.fly_destination(i)
    }
    fn water_encounters(
        &self,
        map: MapId,
        wild_set: u16,
    ) -> Option<&'static engine::mon::WaterEncounters> {
        Game.water_encounters(map, wild_set)
    }
    fn rock_smash_encounters(
        &self,
        map: MapId,
        wild_set: u16,
    ) -> Option<&'static engine::mon::WaterEncounters> {
        Game.rock_smash_encounters(map, wild_set)
    }
    fn pushable_boulder(&self) -> Option<(GfxId, FlagId)> {
        Game.pushable_boulder()
    }
    fn player_surf_sprite(&self) -> Option<&'static SpriteData> {
        Game.player_surf_sprite()
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
    /// Whether a lost (or drawn) battle is expected: a run that tests a defeat branch. Off by
    /// default: a spine wins every battle, and a loss fails the run at once.
    pub allow_losses: bool,
}

impl Run {
    /// A new game, at most `limit` frames.
    pub fn new(limit: u64) -> Run {
        Run::with_start(limit, None)
    }

    /// A new game that starts at a chapter's start state ([`Start`]), at most `limit` frames.
    pub fn starting_at(limit: u64, start: Start) -> Run {
        Run::with_start(limit, Some(start))
    }

    fn with_start(limit: u64, start: Option<Start>) -> Run {
        let g = Spy {
            texts: RefCell::new(Vec::new()),
            hooks: RefCell::new(Vec::new()),
            start,
        };
        Run {
            w: World::new(&g),
            g,
            frame: 0,
            limit,
            battles: Vec::new(),
            degraded: BTreeSet::new(),
            choice: 0,
            allow_losses: false,
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
                Step::Swap(a, b) => self.swap(a, b),
                Step::Teach(t) => self.teach(t),
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
                if std::env::var_os("SPINE_DEBUG").is_some() {
                    self.log_battle(*o);
                }
                assert!(
                    self.allow_losses || !matches!(o, BattleOutcome::Lost | BattleOutcome::Drew),
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

    /// `SPINE_DEBUG=1`: a battle's outcome, both parties' levels and HP, and the player's
    /// moves (for tuning the training levels and hooks).
    fn log_battle(&self, o: BattleOutcome) {
        eprintln!("battle {o:?} at frame {} on {:?}", self.frame, self.w.map());
        for m in self.w.party().iter() {
            let (hp, max) = (m.hp, m.stats[0]);
            eprintln!(
                "  {:?} lv {} hp {hp}/{max} {:?}",
                m.species, m.level, m.moves
            );
        }
        if let Some((_, b, _)) = self.w.battle() {
            for m in b.party(Side::Opponent).iter() {
                let (hp, max) = (m.hp, m.stats[0]);
                eprintln!("  foe {:?} lv {} hp {hp}/{max}", m.species, m.level);
            }
        }
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
    /// confirms; a battle menu fights with [`best_move`]; when a new move needs a slot, forgets
    /// [`learn_slot`]'s move, or keeps the known moves (B, then "stop learning?" yes);
    /// everything else is A (text, yes/no on yes).
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
            // A learn prompt: yes (then the slot) when [`learn_slot`] finds a move to forget.
            Some(OpenMenu::LearnAsk(c)) => match learn_slot(battle, Game.battle()) {
                Some(_) if c != 0 => Buttons::UP,
                Some(_) => Buttons::A,
                None => Buttons::B,
            },
            Some(OpenMenu::LearnPick(c)) => match learn_slot(battle, Game.battle()) {
                Some(_) if c == 4 => Buttons::LEFT,
                Some(slot) => toward(c, slot),
                None => Buttons::B,
            },
            Some(OpenMenu::StopAsk(0)) => Buttons::A,
            Some(OpenMenu::StopAsk(_)) => Buttons::UP,
            // A doubles target cursor: onto the foe the move was picked for.
            Some(OpenMenu::Target(c)) => target_key(battle, Game.battle(), c),
            None => Buttons::A,
        }
    }

    /// [`Step::Train`]: from `a`, steps to `b` and back (encounters are settled on the way) until
    /// the lead's level is `level`; first plays `heal` (then walks back to `a`) whenever the lead
    /// is below half its HP or has a status.
    fn train(&mut self, a: (i16, i16), b: (i16, i16), level: u8, heal: &[Step]) {
        // A tuning aid: `SPINE_LEVEL_<level>=<n>` trains to `n` instead (the neighbour rule's
        // measures, one level at a time, without editing the spine).
        let level = std::env::var(format!("SPINE_LEVEL_{level}"))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(level);
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
    /// [`Step::Swap`]: START, POKéMON, slot `a`, SWITCH, slot `b`, then B out of the party
    /// menu and the start menu (one press, one idle frame, as the settle loop).
    fn swap(&mut self, a: u8, b: u8) {
        use engine::data::StartAction;
        use engine::ui::ScreenState;
        use engine::ui::party::{Action, Prompt};
        assert!(a < b, "Swap({a}, {b}): the first slot first");
        self.settle();
        let party = |r: &Run| r.w.party().iter().map(|m| m.species).collect::<Vec<_>>();
        let mut want = party(self);
        want.swap(a as usize, b as usize);
        // The start menu's POKéMON entry among the shown ones.
        let entries: Vec<_> = (Game.start_menu().iter())
            .filter(|i| i.shown_if.is_none_or(|f| self.w.flag(f)))
            .collect();
        let pokemon = (entries.iter())
            .position(|i| i.action == StartAction::Party)
            .expect("the start menu shows POKéMON") as u8;
        let mut opened = false;
        // Down presses left to move the switch cursor from `a` to `b`.
        let mut moves = None::<u8>;
        for i in 0..4000 {
            let done = party(self) == want;
            let top = self.w.screens().and_then(|s| s.top()).map(|(_, st)| st);
            let keys = match top {
                None if done && !self.busy() => {
                    self.settle();
                    return;
                }
                None if opened => Buttons::NONE,
                None if self.busy() => Buttons::NONE,
                None => Buttons::START,
                Some(ScreenState::StartMenu(_)) if done => Buttons::B,
                Some(ScreenState::StartMenu(m)) => match m.cursor() {
                    None => Buttons::NONE,
                    Some(c) if c == pokemon => {
                        opened = true;
                        Buttons::A
                    }
                    Some(c) if c < pokemon => Buttons::DOWN,
                    Some(_) => Buttons::UP,
                },
                Some(ScreenState::Party(p)) => match p.prompt() {
                    Prompt::Slot { .. } if done => Buttons::B,
                    Prompt::Slot { cursor } if cursor == a => Buttons::A,
                    Prompt::Slot { cursor } if cursor < a => Buttons::DOWN,
                    Prompt::Slot { .. } => Buttons::UP,
                    Prompt::Actions { .. } => {
                        let want = (p.actions().iter())
                            .position(|x| *x == Action::Switch)
                            .expect("SWITCH (two mons or more)")
                            as u8;
                        match p.action_cursor() {
                            Some(c) if c == want => {
                                moves = Some(b - a);
                                Buttons::A
                            }
                            Some(c) if c < want => Buttons::DOWN,
                            _ => Buttons::UP,
                        }
                    }
                    Prompt::Switch { .. } => match moves {
                        Some(0) | None => Buttons::A,
                        Some(n) => {
                            if i % 2 == 0 {
                                moves = Some(n - 1);
                            }
                            Buttons::DOWN
                        }
                    },
                    Prompt::Message => Buttons::A,
                    p => panic!("Swap({a}, {b}): the party menu at {p:?}"),
                },
                Some(_) => panic!("Swap({a}, {b}): another screen is open"),
            };
            self.step(if i % 2 == 0 { keys } else { Buttons::NONE });
        }
        panic!("Swap({a}, {b}) never closed the menus");
    }

    /// [`Step::Teach`]: `World::use_item` (the bag's USE), then the party menu on `t.mon`, yes
    /// to "forget a move?", the summary's move `t.forget`, and the messages, until the field is
    /// back; asserts the move was learned.
    fn teach(&mut self, t: TeachTm) {
        use engine::ui::ScreenState;
        use engine::ui::party::Prompt;
        self.settle();
        let item = ItemId(t.item);
        let mov = Game.tm_move(item).expect("a TM or HM");
        assert!(
            self.w.use_item(&self.g, item),
            "Teach: item {} cannot be used here",
            t.item
        );
        for i in 0..4000u32 {
            let top = self.w.screens().and_then(|s| s.top()).map(|(_, st)| st);
            let keys = match top {
                None if !self.busy() => break,
                None => Buttons::NONE,
                Some(ScreenState::Party(p)) => match p.prompt() {
                    Prompt::Slot { cursor } if cursor == t.mon => Buttons::A,
                    Prompt::Slot { cursor } if cursor < t.mon => Buttons::DOWN,
                    Prompt::Slot { .. } => Buttons::UP,
                    Prompt::Message => Buttons::A,
                    Prompt::YesNo { cursor: 0 } => Buttons::A,
                    Prompt::YesNo { .. } => Buttons::UP,
                    Prompt::Child => Buttons::NONE,
                    p => panic!("Teach: the party menu at {p:?}"),
                },
                Some(ScreenState::Summary(s)) if s.move_cursor == t.forget => Buttons::A,
                Some(ScreenState::Summary(_)) => Buttons::DOWN,
                Some(_) => panic!("Teach: another screen is open"),
            };
            self.step(if i % 2 == 0 { keys } else { Buttons::NONE });
        }
        self.settle();
        let mon = self.w.party().get(t.mon as usize).expect("the slot");
        assert!(
            mon.moves.contains(&Some(mov)),
            "Teach: {:?} did not learn {mov:?}",
            mon.species
        );
    }

    /// Turns the player toward `d` (a tap) without stepping.
    fn face(&mut self, d: Dir) {
        self.settle();
        if self.w.objects().player().facing != d {
            self.step(self.w.input_map(&self.g).key_for(d));
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
            self.step(self.w.input_map(&self.g).key_for(d));
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
            self.step(self.w.input_map(&self.g).key_for(d));
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
        let (mut stuck, mut moves) = (0, 0);
        loop {
            self.settle();
            assert_eq!(self.w.map(), map, "walking to {target:?} left the map");
            let p = self.pos();
            if p == target {
                return;
            }
            // A wandering NPC can close the only way for a while (the Casino's (6–8, 9)): wait
            // for it, up to [`WAIT_FOR_WAY`] frames (deterministic: the world steps as always).
            let mut waited = 0;
            let d = loop {
                if let Some(d) = self.plan(p, target, avoid) {
                    break d;
                }
                assert!(
                    waited < WAIT_FOR_WAY,
                    "no way from {p:?} to {target:?} on {map:?} after {WAIT_FOR_WAY} frames"
                );
                if self.busy() {
                    self.settle();
                    assert_eq!(self.w.map(), map, "walking to {target:?} left the map");
                } else {
                    self.step(Buttons::NONE);
                }
                waited += 1;
            };
            // A walk that moves but never arrives (a slide back and forth) fails here, not at
            // the frame limit.
            moves += 1;
            assert!(
                moves < 2000,
                "walking to {target:?} on {map:?} never arrives (at {p:?})"
            );
            let mut n = 0;
            while self.pos() == p && !self.busy() && n < 40 {
                self.step(self.w.input_map(&self.g).key_for(d));
                n += 1;
            }
            if self.pos() == p {
                stuck += 1;
                assert!(stuck < 20, "stuck at {p:?} walking to {target:?}");
            }
        }
    }

    /// The first step of a shortest path, off encounter tiles (tall grass, cave floors, deep sand,
    /// `Behavior::land_encounter`) when there is such a path; through them otherwise (a cave).
    fn plan(&self, from: (i16, i16), to: (i16, i16), avoid: &[(i16, i16)]) -> Option<Dir> {
        let grid = Grid::new(&self.g, &self.w, avoid);
        let e = self.w.objects().player().elevation;
        grid.first_dir(from, e, to, true)
            .or_else(|| grid.first_dir(from, e, to, false))
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

/// How long [`Run::walk_to`] waits for a way to open (a wanderer in a one-cell corridor; the
/// Casino's has been measured closing it for 450 frames).
const WAIT_FOR_WAY: u32 = 1200;

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

/// The battle policy's move slot for the battler choosing now (`Battle::choosing`; in doubles
/// either of the player's two): of the moves it may choose, the best expected damage against
/// [`target_foe`] (power × accuracy, ×1.5 for the same type, × Gen3's type chart against its
/// types); the first such slot on a tie. With no damaging move, the first it may choose; with
/// none, slot 0 (the engine uses Struggle).
pub fn best_move(battle: &Battle, t: &BattleTables) -> u8 {
    let side = battle.choosing();
    let me = battle.mon(side);
    let foe = t.species_info(battle.mon(target_foe(battle)).species).types;
    let own = t.species_info(me.species).types;
    let usable = |i: usize| selectable(battle, t, side, i as u8);
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

/// Doubles: the foe to aim at, the standing opponent with the least HP (the explorer's
/// `policy::target_foe`); in singles the opponent.
fn target_foe(battle: &Battle) -> Side {
    let foes = [Side::Opponent, Side::OpponentRight]
        .into_iter()
        .take(battle.count() / 2);
    foes.filter(|&s| battle.is_present(s) && battle.mon(s).hp > 0)
        .min_by_key(|&s| battle.mon(s).hp)
        .unwrap_or(Side::Opponent)
}

/// Whether `side` may choose `slot` now (Disable, Torment, Taunt and Imprison refuse a move and
/// reopen the menu, so choosing it again would loop forever); in doubles not a move that also
/// hits its standing ally (Magnitude, Earthquake) while another move is usable.
fn selectable(battle: &Battle, t: &BattleTables, side: Side, slot: u8) -> bool {
    if !battle.move_selectable(t, side, slot) {
        return false;
    }
    let ally = side.partner();
    if !battle.is_double() || !battle.is_present(ally) || battle.mon(ally).hp == 0 {
        return true;
    }
    let hits_ally = |s: u8| {
        battle.mon(side).moves[s as usize]
            .is_some_and(|m| t.move_info(m).target == engine::mon::MoveTarget::FoesAndAlly)
    };
    !hits_ally(slot)
        || !(0..4).any(|s| s != slot && !hits_ally(s) && battle.move_selectable(t, side, s))
}

/// The doubles target cursor's key: RIGHT until the cursor is on [`target_foe`], then A.
fn target_key(battle: &Battle, t: &BattleTables, c: u8) -> Buttons {
    let side = battle.choosing();
    let want = battle
        .target_choices(t, side, best_move(battle, t))
        .and_then(|v| v.iter().position(|&s| s == target_foe(battle)));
    match want {
        Some(k) if k as u8 != c => Buttons::RIGHT,
        _ => Buttons::A,
    }
}

/// A move's worth outside any matchup, for learning: power × accuracy, ×1.5 for the same type
/// as `species` (0 for a move without power).
fn move_worth(t: &BattleTables, species: SpeciesId, m: MoveId) -> i64 {
    let m = t.move_info(m);
    let stab = if t.species_info(species).types.contains(&m.ty) {
        3
    } else {
        2
    };
    let accuracy = if m.accuracy == 0 { 100 } else { m.accuracy };
    i64::from(m.power) * i64::from(accuracy) * stab
}

/// The battle policy's answer to "forget a move to learn the new one?" (`Need::LearnMove`):
/// the slot of the weakest damaging move ([`move_worth`]; the lowest slot on a tie) when the
/// new move is worth more, else `None` (keep the known moves). Moves without power are never
/// forgotten nor learned this way; no field ability is a move in Silice (Cut, Strength and
/// Flash are the Mallette's modules, flags), so no move is kept for the scripts.
pub fn learn_slot(battle: &Battle, t: &BattleTables) -> Option<u8> {
    let Need::LearnMove { party, mov } = battle.need() else {
        return None;
    };
    let mon = battle.party(Side::Player).get(party as usize)?;
    let worth = |m| move_worth(t, mon.species, m);
    let (weakest, slot) = (0..4)
        .filter_map(|i| mon.moves[i].map(|m| (worth(m), i)))
        .filter(|&(w, _)| w > 0)
        .min()?;
    (worth(mov) > weakest).then_some(slot as u8)
}

/// The direction a forced-movement cell of behavior `b` carries a player facing `facing`, and
/// whether the player turns that way (`objects::forced::forced_step`), if it is one.
fn forced_dir(b: Behavior, facing: Dir) -> Option<(Dir, bool)> {
    match b {
        Behavior::Ice => Some((facing, true)),
        Behavior::ForcedWalk(d) | Behavior::Water(engine::data::Water::Current(d)) => {
            Some((d, true))
        }
        Behavior::Slide(d) => Some((d, false)),
        Behavior::MuddySlope => Some((Dir::Down, false)),
        _ => None,
    }
}


/// The planner's view of the current map: collision, behaviors (walls, ledges one way, warps
/// as targets only, encounter tiles), the objects' cells, and the cells scripts repainted
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
            && !(avoid_grass && self.behavior(c).land_encounter())
    }

    fn elevation(&self, c: (i16, i16)) -> u8 {
        block_elevation(self.block(c))
    }

    /// One planned step from `c` (the player at elevation `e`) toward `d`: the cell the player
    /// comes to rest on, and its elevation. A forced-movement cell (`objects::forced`: ice, walk
    /// and slide arrows, the muddy slope, currents) carries the player on, one cell at a time,
    /// until the next cell is blocked; a carry into a warp, or one that never rests, is refused.
    fn step(
        &self,
        c: (i16, i16),
        e: u8,
        d: Dir,
        grass: bool,
        target: (i16, i16),
    ) -> Option<((i16, i16), u8)> {
        let (n, e) = self.move_onto(c, e, d, Some(target))?;
        if n == target && !self.walkable(n, false) {
            return Some((n, e));
        }
        if !self.walkable(n, false) {
            return None;
        }
        let (mut cur, mut e, mut facing) = (n, e, d);
        for _ in 0..256 {
            let Some((fd, turns)) = forced_dir(self.behavior(cur), facing) else {
                return (cur == target || self.walkable(cur, grass)).then_some((cur, e));
            };
            match self.move_onto(cur, e, fd, None) {
                Some((nn, _)) if self.behavior(nn).enters_warp() => return None,
                Some((nn, en)) if self.walkable(nn, false) => {
                    (cur, e) = (nn, en);
                    if turns {
                        facing = fd;
                    }
                }
                _ => return (cur == target || self.walkable(cur, grass)).then_some((cur, e)),
            }
        }
        None
    }

    /// The cell one move from `c` (the player at elevation `e`) toward `d` reaches, and the
    /// elevation there, by the geometry only (bounds, walls, ledges jumped toward their
    /// direction, elevations). On foot: an elevation mismatch (Gen3 `IsElevationMismatchAt`:
    /// water is elevation 1 under the land's 3) refuses the move, except onto `target` when
    /// nobody walks onto it (a door, an object: the explorer's `first_dir_avoiding`); a 15 cell
    /// (a bridge) on either side keeps the elevation (`ObjectEventUpdateElevation`).
    fn move_onto(
        &self,
        c: (i16, i16),
        e: u8,
        d: Dir,
        target: Option<(i16, i16)>,
    ) -> Option<((i16, i16), u8)> {
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
        if !self.in_bounds(n) {
            return None;
        }
        let en = self.elevation(n);
        let onto = Some(n) != target || self.walkable(n, false);
        if onto && e != 0 && en != 0 && en != 15 && en != e {
            return None;
        }
        let e2 = if en == 15 || self.elevation(c) == 15 {
            e
        } else {
            en
        };
        Some((n, e2))
    }

    fn first_dir(&self, from: (i16, i16), e: u8, to: (i16, i16), grass: bool) -> Option<Dir> {
        let mut seen = BTreeSet::from([(from, e)]);
        let mut queue = VecDeque::from([(from, e, None::<Dir>)]);
        while let Some((c, e, d0)) = queue.pop_front() {
            if c == to {
                return d0;
            }
            for d in Dir::ALL {
                if let Some((n, en)) = self.step(c, e, d, grass, to)
                    && seen.insert((n, en))
                {
                    queue.push_back((n, en, Some(d0.unwrap_or(d))));
                }
            }
        }
        None
    }
}
