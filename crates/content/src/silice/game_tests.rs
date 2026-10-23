//! SIL1 Task E: `game.ron` in the Silice root (new game, system ids, heal locations), the
//! new-game flags, and the Silice prelude helpers (as the requests they issue).

use super::Game;
use super::prelude::*;
use core::cell::RefCell;
use engine::data::{GameData, Pocket, block_collision};
use engine::input::Buttons;
use engine::script::{Request, Response, Runner, Script, ScriptHost};
use extract::games::{GameFile, IdKind, IdsFile};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::vec::Vec;

fn games() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../games/silice")
}

fn game_ron() -> GameFile {
    extract::export::read_ron(&games().join("game.ron")).unwrap()
}

/// The number of a live id of `kind` named `name` in the committed ids files.
fn id(kind: IdKind, name: &str) -> u16 {
    let f: IdsFile =
        extract::export::read_ron(&games().join(format!("ids/{}.ron", kind.file()))).unwrap();
    let e = f.ids.iter().find(|e| e.name == name && !e.removed);
    u16::try_from(e.unwrap_or_else(|| panic!("{name}")).num).unwrap()
}

fn map_of(file: &str) -> MapId {
    let f: IdsFile = extract::export::read_ron(&games().join("ids/maps.ron")).unwrap();
    let e = f
        .ids
        .iter()
        .find(|e| e.map.as_deref() == Some(file) && !e.removed);
    MapId(u16::try_from(e.unwrap().num).unwrap())
}

fn walkable(map: MapId, x: i16, y: i16) -> bool {
    let m = Game.map(map);
    x >= 0
        && y >= 0
        && (x as u16) < m.width
        && (y as u16) < m.height
        && block_collision(m.blocks[y as usize * m.width as usize + x as usize]) == 0
}

#[test]
fn a_new_game_lands_on_game_ron_s_start_cell() {
    let g = game_ron();
    let ng = Game.new_game();
    assert_eq!(
        (ng.map, ng.x, ng.y),
        (map_of(&g.start.map), g.start.x, g.start.y)
    );
    assert_eq!(ng.facing, SIL_START_FACING);
    assert!(walkable(ng.map, ng.x, ng.y));
    let mut w = engine::World::new(&Game);
    w.step(&Game, Buttons::NONE);
    assert_eq!(w.map(), ng.map);
    assert_eq!(w.objects().player().pos(), (ng.x, ng.y));
    assert_eq!(w.objects().player().facing, ng.facing);
}

#[test]
fn the_new_game_sets_game_ron_s_flags_and_nothing_else_of_the_story() {
    let g = game_ron();
    assert!(!g.new_game_flags.is_empty());
    let w = engine::World::new(&Game);
    for f in &g.new_game_flags {
        assert!(w.flag(FlagId(id(IdKind::Flags, f))), "{f}");
    }
    for (v, x) in &g.new_game_vars {
        assert_eq!(w.var(VarId(id(IdKind::Vars, v))), *x, "{v}");
    }
    assert_eq!(SIL_NEW_GAME_FLAGS.len(), g.new_game_flags.len());
    // The Prologue starts at story step 0 with no running shoes and no party.
    assert_eq!(w.var(VAR_SIL_STORY), 0);
    assert!(!w.flag(SIL_RUNNING_FLAG));
    assert_eq!(w.party().len(), 0);
    // Shown at the start: the friends in the bar, the man at the back.
    for f in [
        FLAG_SIL_HIDE_BAR_MATHIEU,
        FLAG_SIL_HIDE_BAR_GWADA,
        FLAG_SIL_HIDE_BAR_ADRIEN,
        FLAG_SIL_HIDE_BAR_JULIEN,
        // Ch1: shown until their scene (or a story step) hides them.
        FLAG_SIL_HIDE_BOIS_ETHER_TECHNICIEN,
        FLAG_SIL_HIDE_BOIS_ETHER_GARDE,
        FLAG_SIL_HIDE_GYM1_LOIC,
        FLAG_SIL_HIDE_LABO_CHIMISTE,
    ] {
        assert!(!w.flag(f), "{f:?}");
    }
    // Hidden until a scene shows them.
    for f in [
        FLAG_SIL_HIDE_BAR_SBIRE_A,
        FLAG_SIL_HIDE_BAR_SBIRE_B,
        FLAG_SIL_HIDE_BAR_MALLETTE,
        FLAG_SIL_HIDE_BRICO_SBIRE,
        // Ch1: Adrien at the meeting point, and Bois_Ether_Sc06's actors.
        FLAG_SIL_HIDE_BOIS_ETHER_ADRIEN,
        FLAG_SIL_HIDE_BOIS_ETHER_GOGO,
        FLAG_SIL_HIDE_BOIS_ETHER_ROMAIN,
        FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_1,
        FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_2,
        FLAG_SIL_HIDE_BOIS_ETHER_MATHIEU,
    ] {
        assert!(w.flag(f), "{f:?}");
    }
}

#[test]
fn the_system_ids_and_heal_locations_are_game_ron_s() {
    let g = game_ron();
    let ids = Game.special_ids();
    assert_eq!(
        ids.flag_running,
        Some(FlagId(id(IdKind::Flags, &g.running_flag)))
    );
    assert_eq!(ids.flag_running, Some(FLAG_SIL_RUNNING_SHOES));
    let cleared: Vec<&str> = (ids.flags_cleared_on_map_change.iter())
        .map(|f| extract::names::FLAGS.name(u32::from(f.0)).unwrap())
        .collect();
    assert_eq!(cleared, g.cleared_on_map_change);
    assert_eq!(Some(ids.respawn_default), g.heal_location(&g.respawn));
    assert_eq!(Game.heal_location(0), None);
    for (i, h) in g.heal_locations.iter().enumerate() {
        let n = u8::try_from(i + 1).unwrap();
        assert_eq!(
            Game.heal_location(n),
            Some((map_of(&h.map), h.x, h.y)),
            "{}",
            h.name
        );
        assert!(walkable(map_of(&h.map), h.x, h.y), "{}", h.name);
    }
    assert_eq!(Game.heal_location(g.heal_locations.len() as u8 + 1), None);
    assert_eq!(
        HEAL_LOCATION_SIL_BRICO_BOURG_PLAYER_HOUSE,
        ids.respawn_default
    );
    let (map, _, _) = Game.heal_location(ids.respawn_default).unwrap();
    assert_eq!(map, MAP_BRICO_BOURG_PLAYER_HOUSE_2F);
    assert_eq!(engine::World::new(&Game).respawn(), ids.respawn_default);
}

/// Answers every request at once (`ChooseStarter` with `choice`), records them and keeps the
/// flags and vars.
struct Host {
    log: Vec<Request>,
    flags: BTreeMap<u16, bool>,
    vars: BTreeMap<u16, u16>,
    choice: u16,
    fits: bool,
}

impl Host {
    fn new() -> Host {
        Host {
            log: Vec::new(),
            flags: BTreeMap::new(),
            vars: BTreeMap::new(),
            choice: 0,
            fits: true,
        }
    }

    fn count(&self, pred: impl Fn(&Request) -> bool) -> usize {
        self.log.iter().filter(|r| pred(r)).count()
    }
}

impl ScriptHost for Host {
    fn begin(&mut self, r: &Request) -> Option<Response> {
        self.log.push(r.clone());
        Some(match *r {
            Request::GetFlag(f) => Response::Bool(self.flags.get(&f.0).copied().unwrap_or(false)),
            Request::SetFlag(f, on) => {
                self.flags.insert(f.0, on);
                Response::Unit
            }
            Request::GetVar(v) => Response::Value(self.vars.get(&v.0).copied().unwrap_or(0)),
            Request::SetVar(v, x) => {
                self.vars.insert(v.0, x);
                Response::Unit
            }
            Request::ChooseStarter(_) => Response::Value(self.choice),
            Request::GiveMon { .. } => Response::Value(0),
            Request::BattleWild { .. } => Response::Value(BattleOutcome::Won as u16),
            Request::YesNo => Response::Bool(self.choice == 1),
            Request::ItemAdd { .. } | Request::ItemSpace { .. } => Response::Bool(self.fits),
            _ => Response::Unit,
        })
    }

    fn resume(&mut self, _: &Request) -> Option<Response> {
        Some(Response::Unit)
    }
}

fn run(host: &mut Host, script: impl FnOnce(Ctx) -> Script) {
    let mut r = Runner::new();
    r.start(script(r.ctx()));
    for _ in 0..100 {
        if !r.step(host) {
            return;
        }
    }
    panic!("script still running");
}

#[test]
fn the_starter_choice_gives_game_ron_s_species() {
    let g = game_ron();
    assert_eq!(
        SIL_STARTERS,
        [SPECIES_CHIKORITA, SPECIES_CYNDAQUIL, SPECIES_TOTODILE]
    );
    for (i, name) in g.starters.iter().enumerate() {
        let want = SpeciesId(u16::try_from(extract::names::SPECIES.value(name).unwrap()).unwrap());
        assert_eq!(SIL_STARTERS[i], want, "{name}");
        // The species has battle data, so the starter screen and the party can show it.
        assert!(Game.battle().species.get(usize::from(want.0)).is_some());
        let mut h = Host::new();
        h.choice = i as u16;
        let got = Rc::new(RefCell::new(None));
        let out = got.clone();
        run(&mut h, |ctx| {
            Box::pin(async move {
                let i = choose_starter(&ctx).await;
                *out.borrow_mut() = Some(i);
            })
        });
        assert_eq!(*got.borrow(), Some(i as u8));
        assert_eq!(h.log[0], Request::ChooseStarter(SIL_STARTERS));
        assert!(h.log.contains(&Request::GiveMon {
            species: want,
            level: g.starter_level,
        }));
        assert_eq!(h.vars[&VAR_SIL_STARTER.0], i as u16);
        // No battle follows (unlike Emerald's `ChooseStarter` special).
        assert_eq!(h.count(|r| matches!(r, Request::BattleWild { .. })), 0);
    }
    // An out-of-range answer is the first starter, as Gen3.
    let mut h = Host::new();
    h.choice = 7;
    run(&mut h, |ctx| {
        Box::pin(async move {
            choose_starter(&ctx).await;
        })
    });
    assert_eq!(h.vars[&VAR_SIL_STARTER.0], 0);
    assert!(h.log.contains(&Request::GiveMon {
        species: SIL_STARTERS[0],
        level: g.starter_level,
    }));
}

#[test]
fn fallback_reports_and_the_script_goes_on() {
    let mut h = Host::new();
    run(&mut h, |ctx| {
        Box::pin(async move {
            fallback(&ctx, "set_sky_alert").await;
            screen_shake(&ctx, 8, 20).await;
            play_se(&ctx, "SE_M_EXPLOSION").await;
            mallette_call(&ctx, "JULIEN").await;
            end_call(&ctx).await;
            set_story(&ctx, 4).await;
        })
    });
    let degraded: Vec<&str> = (h.log.iter())
        .filter_map(|r| match r {
            Request::Report(Report::Degraded(n)) => Some(*n),
            _ => None,
        })
        .collect();
    assert_eq!(
        degraded,
        [
            "set_sky_alert",
            "screen_shake",
            "play_se",
            "mallette_call",
            "end_call"
        ]
    );
    // The shake stands in as a fade out and back in.
    assert_eq!(h.count(|r| matches!(r, Request::Fade(Fade::ToBlack))), 1);
    assert_eq!(h.count(|r| matches!(r, Request::Fade(Fade::FromBlack))), 1);
    // The script ran to its end.
    assert_eq!(h.log.last(), Some(&Request::SetVar(VAR_SIL_STORY, 4)));
}

#[test]
fn story_defeated_and_gift_helpers() {
    let mut h = Host::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    run(&mut h, |ctx| {
        Box::pin(async move {
            let v = story(&ctx).await;
            out.borrow_mut().push(v);
            set_story(&ctx, 5).await;
            let v = story(&ctx).await;
            out.borrow_mut().push(v);
        })
    });
    assert_eq!(*seen.borrow(), [0, 5]);
    assert_eq!(h.vars[&VAR_SIL_STORY.0], 5);

    let trainer = TrainerId(3);
    let mut h = Host::new();
    h.flags.insert(trainer_flag(trainer).0, true);
    let beaten = Rc::new(RefCell::new(Vec::new()));
    let out = beaten.clone();
    run(&mut h, |ctx| {
        Box::pin(async move {
            let v = defeated(&ctx, trainer).await;
            out.borrow_mut().push(v);
            let v = defeated(&ctx, TrainerId(4)).await;
            out.borrow_mut().push(v);
        })
    });
    assert_eq!(*beaten.borrow(), [true, false]);

    // Given once: the flag gates it; quiet means no message.
    let flag = FLAG_SIL_ITEM_PROLOGUE_POTIONS;
    let mut h = Host::new();
    let given = Rc::new(RefCell::new(Vec::new()));
    let out = given.clone();
    run(&mut h, |ctx| {
        Box::pin(async move {
            for _ in 0..2 {
                let g = gift(&ctx, flag, ITEM_POTION, 2, Obtain::Quiet).await;
                out.borrow_mut().push(g);
            }
        })
    });
    assert_eq!(*given.borrow(), [true, false]);
    assert_eq!(h.count(|r| matches!(r, Request::ItemAdd { .. })), 1);
    assert_eq!(h.count(|r| matches!(r, Request::Message(_))), 0);
    assert_eq!(h.flags.get(&flag.0), Some(&true));
    // A full bag gives nothing and leaves the flag clear, so the gift can be retried.
    let mut h = Host::new();
    h.fits = false;
    run(&mut h, |ctx| {
        Box::pin(async move {
            gift(&ctx, flag, ITEM_POTION, 2, Obtain::Quiet).await;
        })
    });
    assert_eq!(h.flags.get(&flag.0), None);
}

#[test]
fn scene_helpers_issue_the_ctx_requests() {
    const PATH: &[Step] = &[walk(Dir::Up), walk_in_place(Dir::Left), face(Dir::Down)];
    let mut h = Host::new();
    h.choice = 1; // yes
    let yes = Rc::new(RefCell::new(false));
    let out = yes.clone();
    run(&mut h, |ctx| {
        Box::pin(async move {
            move_wait(&ctx, 2, PATH).await;
            show(&ctx, 8, FLAG_SIL_HIDE_BAR_SBIRE_A).await;
            hide(&ctx, 9).await;
            let v = ask(&ctx, TextId(0)).await;
            *out.borrow_mut() = v;
            say(&ctx, TextId(1)).await;
            battle_wild(&ctx, SPECIES_SENTRET, 3).await;
        })
    });
    assert!(*yes.borrow());
    let want = [
        Request::Move {
            obj: 2,
            steps: PATH,
        },
        Request::WaitMove(2),
        Request::SetFlag(FLAG_SIL_HIDE_BAR_SBIRE_A, false),
        Request::AddObject(8),
        Request::RemoveObject(9),
    ];
    assert_eq!(h.log[..want.len()], want);
    assert!(h.log.contains(&Request::Message(TextId(1))));
    assert!(h.log.contains(&Request::BattleWild {
        species: SPECIES_SENTRET,
        level: 3,
        first_battle: false,
    }));
    assert_eq!(
        PATH,
        [
            Step::Walk(Dir::Up, Speed::Normal),
            Step::WalkInPlace(Dir::Left, Speed::Normal),
            Step::Face(Dir::Down),
        ]
    );
}

/// The Gen3 std texts and strings the shared prelude prints (`give_item`, `find_item`, the
/// pocket names) are in the Silice build.
#[test]
fn the_gen3_std_texts_and_strings_are_kept() {
    for name in [
        "gText_ObtainedTheItem",
        "gText_PutItemInPocket",
        "gText_PlayerFoundOneItem",
        "gText_TooBadBagIsFull",
    ] {
        let t = text_named(name).unwrap_or_else(|| panic!("{name}"));
        assert!(Game.text(t).len() > 1, "{name}");
    }
    for p in [
        Pocket::Items,
        Pocket::PokeBalls,
        Pocket::TmHm,
        Pocket::Berries,
        Pocket::KeyItems,
    ] {
        let s = Game.std_string(pocket_std_string(p));
        assert!(s.is_some_and(|s| s.len() > 1), "{p:?}");
    }
}
