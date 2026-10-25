//! Evolution scene and friendship evolutions (Milestone 9 Task E).
//!
//! A level-up species and a friendship species both go through the scene (picture, cry,
//! then the species change). B cancels only when Gen3 allows it (`canStopEvo`: level and
//! friendship, not a stone). Friendship evolves at 220 and not at 219.

use super::*;
use crate::battle::BattleOutcome;
use crate::battle::gfx::test_gfx::{leak, tile};
use crate::data::{EvoMethod, Evolution, ItemInfo, ItemUse, MedicineEffect, Pocket};
use crate::item_use::EvolutionPlay;
use crate::mon::{BattleTables, Ivs, MonGfx, Pokemon, exp_for_level};
use crate::pokedex::DexFlag;
use crate::rng::Rng;
use crate::script::Report;
use crate::ui::{ScreenKind, ScreenKindTag};
use crate::video::Cmd;
use alloc::boxed::Box;
use alloc::vec::Vec;
use std::sync::OnceLock;

use crate::battle::test_data::{BUG, FIRE, NORMAL, TACKLE, WATER};

const TAG_WATER: u32 = 0xA1;
const TAG_FIRE: u32 = 0xA3;
const TAG_NORMAL: u32 = 0xA2;
const TAG_BUG: u32 = 0xA4;
const CANDY: ItemId = ItemId(70);
const STONE: ItemId = ItemId(71);

static WATER_EVOS: [Evolution; 1] = [Evolution {
    method: EvoMethod::Level,
    param: 6,
    target: FIRE,
}];
static BUG_EVOS: [Evolution; 1] = [Evolution {
    method: EvoMethod::Friendship,
    param: 0,
    target: NORMAL,
}];
static NORMAL_EVOS: [Evolution; 1] = [Evolution {
    method: EvoMethod::Item,
    param: STONE.0,
    target: BUG,
}];
static ITEM: ItemInfo = ItemInfo {
    name: &[0xFF],
    pocket: Pocket::Items,
    price: 0,
    key: false,
};

fn gfx(tag: u32) -> &'static MonGfx {
    Box::leak(Box::new(MonGfx {
        front: leak((0..64).map(|i| tile(tag, i)).collect()),
        back: leak((0..64).map(|i| tile(tag.wrapping_add(0x10), i)).collect()),
        palette: Box::leak(Box::new([0x111; 16])),
        shiny: Box::leak(Box::new([0x222; 16])),
        front_y: 0,
        back_y: 0,
        elevation: 0,
    }))
}

fn tables() -> &'static BattleTables {
    static T: OnceLock<&'static BattleTables> = OnceLock::new();
    *T.get_or_init(|| {
        let base = Game.battle();
        let mut species = base.species.to_vec();
        species[WATER.0 as usize].gfx = Some(gfx(TAG_WATER));
        species[FIRE.0 as usize].gfx = Some(gfx(TAG_FIRE));
        species[NORMAL.0 as usize].gfx = Some(gfx(TAG_NORMAL));
        species[BUG.0 as usize].gfx = Some(gfx(TAG_BUG));
        let species = Box::leak(species.into_boxed_slice());
        Box::leak(Box::new(BattleTables {
            species,
            moves: base.moves,
            type_chart: base.type_chart,
            foresight_split: base.foresight_split,
            struggle: base.struggle,
            nature_power: base.nature_power,
        }))
    })
}

struct EGame;

impl GameData for EGame {
    fn map(&self, id: MapId) -> &'static MapData {
        Game.map(id)
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
        Game.script(id, ctx)
    }
    fn battle(&self) -> &'static BattleTables {
        tables()
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        match species {
            WATER => &WATER_EVOS,
            BUG => &BUG_EVOS,
            NORMAL => &NORMAL_EVOS,
            _ => &[],
        }
    }
    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        matches!(id, CANDY | STONE).then_some(&ITEM)
    }
    fn item_use(&self, id: ItemId) -> ItemUse {
        match id {
            CANDY => ItemUse::Medicine(MedicineEffect::LevelUp),
            STONE => ItemUse::Evolution,
            _ => ItemUse::None,
        }
    }
}

struct Seen {
    events: Vec<Event>,
    /// Front-pic tags uploaded while the lead was still `before`.
    early_pics: Vec<u32>,
    sprite: bool,
    /// The evolution screen was up before the lead's species changed.
    scene_before_change: bool,
}

fn pic_tag(f: &Frame) -> Option<u32> {
    f.cmds.iter().find_map(|c| match c {
        Cmd::ObjTiles { tiles, .. } if !tiles.is_empty() => Some(tiles[0][0] >> 16),
        _ => None,
    })
}

fn evo_open(w: &World) -> bool {
    w.screens()
        .is_some_and(|s| s.kinds().any(|k| matches!(k, ScreenKind::Evolution { .. })))
}

fn drive(
    w: &mut World,
    before: SpeciesId,
    mut keys_at: impl FnMut(&World, u32) -> Buttons,
    mut done: impl FnMut(&World) -> bool,
) -> Seen {
    let mut seen = Seen {
        events: Vec::new(),
        early_pics: Vec::new(),
        sprite: false,
        scene_before_change: false,
    };
    for n in 0..8_000 {
        if done(w) {
            return seen;
        }
        let keys = keys_at(w, n);
        let f = w.step(&EGame, keys);
        f.validate().unwrap();
        if !f.is_black() && !f.bulk {
            assert!(
                crate::ui::kit::frame_bytes(&f) <= crate::ui::kit::UPLOAD_BYTES_VISIBLE,
                "visible frame over the upload budget"
            );
        }
        seen.events.extend_from_slice(w.events());
        let species = w.party().get(0).map(|m| m.species);
        if species == Some(before) && evo_open(w) {
            seen.scene_before_change = true;
        }
        if let Some(tag) = pic_tag(&f)
            && species == Some(before)
        {
            seen.early_pics.push(tag);
        }
        if f.objs.iter().any(|o| o.size == 3) {
            seen.sprite = true;
        }
    }
    panic!(
        "not done after 8000 frames; screens {} flag {} script {} battle {} species {:?} evo {}",
        w.screens_open(),
        w.flag(FLAG_DONE),
        w.script_running(),
        w.in_battle(),
        w.party().get(0).map(|m| m.species),
        evo_open(w)
    );
}

fn mash_a(w: &World, n: u32) -> Buttons {
    let _ = w;
    if n % 2 == 0 {
        Buttons::A
    } else {
        Buttons::NONE
    }
}

fn fresh() -> World {
    let mut w = World::new(&EGame);
    for _ in 0..16 {
        w.step(&EGame, Buttons::NONE).validate().unwrap();
    }
    w
}

/// One exp short of level 6, so a win against NORMAL L2 levels it. Tackle, so a species with
/// an empty learnset can finish the battle.
fn ready(species: SpeciesId, friendship: u8) -> Pokemon {
    let t = EGame.battle();
    let mut mon = Pokemon::new(t, species, 5, Ivs::Fixed(0), 0, 0, &mut Rng::new(1));
    mon.exp = exp_for_level(t.species_info(species).growth, 6) - 1;
    mon.friendship = friendship;
    mon.moves[0] = Some(TACKLE);
    mon.pp[0] = 35;
    mon
}

fn win_battle(w: &mut World, mon: Pokemon) {
    w.s.party.push(mon).unwrap();
    start(w, |ctx| {
        Box::pin(async move {
            let o = ctx.battle_wild(NORMAL, 2, false).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
}

fn cries(events: &[Event]) -> Vec<SpeciesId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Cry(s) => Some(*s),
            _ => None,
        })
        .collect()
}

fn no_evolution_report(events: &[Event]) {
    assert!(
        !events.iter().any(|e| {
            matches!(
                e,
                Event::Report(Report::Degraded("evolution scene"))
                    | Event::Report(Report::Degraded("friendship evolution"))
            )
        }),
        "the scene is not a report"
    );
}

#[test]
fn a_level_up_and_a_friendship_mon_evolve_through_the_scene() {
    // Level-up: the scene shows both pics and cries before the species changes.
    let mut w = fresh();
    win_battle(&mut w, ready(WATER, 70));
    let seen = drive(&mut w, WATER, mash_a, |w| {
        w.flag(FLAG_DONE) && w.screens_open() == 0
    });
    assert!(w.flag(FLAG_DONE));
    assert_eq!(w.party().get(0).unwrap().species, FIRE);
    assert_eq!(w.party().get(0).unwrap().level, 6);
    assert!(
        seen.scene_before_change,
        "scene opens before the species change"
    );
    assert!(seen.sprite, "the scene places the mon picture");
    assert!(
        seen.early_pics.contains(&TAG_WATER) && seen.early_pics.contains(&TAG_FIRE),
        "pics alternate before the species changes, saw {:?}",
        seen.early_pics
    );
    let c = cries(&seen.events);
    assert_eq!(c.first().copied(), Some(WATER));
    assert!(c.contains(&FIRE), "cry of the new species, cries {c:?}");
    assert!(seen.events.contains(&Event::Evolved(FIRE)));
    assert!(
        seen.events
            .contains(&Event::ScreenOpened(ScreenKindTag::Evolution))
    );
    assert!(w.pokedex().get(DexFlag::Caught, FIRE));
    no_evolution_report(&seen.events);

    // Friendship 219 does not evolve. 220 does, through the same scene.
    let mut w = fresh();
    win_battle(&mut w, ready(BUG, 219));
    let low = drive(&mut w, BUG, mash_a, |w| {
        w.flag(FLAG_DONE) && w.screens_open() == 0
    });
    assert_eq!(
        (
            w.party().get(0).unwrap().species,
            w.party().get(0).unwrap().level
        ),
        (BUG, 6)
    );
    assert!(
        !low.events
            .contains(&Event::ScreenOpened(ScreenKindTag::Evolution))
    );
    assert!(!low.events.iter().any(|e| matches!(e, Event::Evolved(_))));

    let mut w = fresh();
    win_battle(&mut w, ready(BUG, 220));
    let high = drive(&mut w, BUG, mash_a, |w| {
        w.flag(FLAG_DONE) && w.screens_open() == 0
    });
    assert_eq!(w.party().get(0).unwrap().species, NORMAL);
    assert!(high.scene_before_change);
    assert!(high.sprite);
    assert!(
        high.early_pics.contains(&TAG_BUG) && high.early_pics.contains(&TAG_NORMAL),
        "friendship scene pics {:?}",
        high.early_pics
    );
    let c = cries(&high.events);
    assert_eq!(c.first().copied(), Some(BUG));
    assert!(c.contains(&NORMAL), "cries {c:?}");
    assert!(high.events.contains(&Event::Evolved(NORMAL)));
    no_evolution_report(&high.events);
}

#[test]
fn b_cancels_a_level_evolution_when_gen3_allows_it() {
    let mut w = fresh();
    win_battle(&mut w, ready(WATER, 70));
    let seen = drive(
        &mut w,
        WATER,
        |w, n| {
            if evo_open(w) {
                if n % 2 == 0 {
                    Buttons::B
                } else {
                    Buttons::NONE
                }
            } else {
                mash_a(w, n)
            }
        },
        |w| w.flag(FLAG_DONE) && w.screens_open() == 0,
    );
    let mon = w.party().get(0).unwrap();
    assert_eq!(
        (mon.species, mon.level),
        (WATER, 6),
        "B stops a level evolution"
    );
    assert!(seen.scene_before_change);
    assert!(seen.events.contains(&Event::Cry(WATER)));
    assert!(!seen.events.contains(&Event::Cry(FIRE)));
    assert!(!seen.events.iter().any(|e| matches!(e, Event::Evolved(_))));
    assert!(!w.pokedex().get(DexFlag::Caught, FIRE));
    no_evolution_report(&seen.events);
}

#[test]
fn rare_candy_and_a_stone_play_the_scene() {
    let mut opened = false;
    let mut w = fresh();
    w.s.party.push(ready(WATER, 70)).unwrap();
    assert!(w.s.bag.add(Pocket::Items, CANDY, 1));
    assert!(w.use_item(&EGame, CANDY));
    let candy = drive(&mut w, WATER, mash_a, |w| {
        if w.screens_open() > 0 {
            opened = true;
            false
        } else {
            opened
        }
    });
    let mon = w.party().get(0).unwrap();
    assert_eq!((mon.species, mon.level), (FIRE, 6));
    assert!(candy.scene_before_change);
    assert!(candy.sprite);
    assert!(candy.early_pics.contains(&TAG_WATER) && candy.early_pics.contains(&TAG_FIRE));
    assert!(candy.events.contains(&Event::Cry(WATER)));
    assert!(candy.events.contains(&Event::Cry(FIRE)));
    assert_eq!(w.s.bag.count(CANDY), 0);
    no_evolution_report(&candy.events);

    // B cancels the candy's level evolution; the level stays.
    let mut opened = false;
    let mut w = fresh();
    w.s.party.push(ready(WATER, 70)).unwrap();
    assert!(w.s.bag.add(Pocket::Items, CANDY, 1));
    assert!(w.use_item(&EGame, CANDY));
    let cancelled = drive(
        &mut w,
        WATER,
        |w, n| {
            if evo_open(w) {
                if n % 2 == 0 {
                    Buttons::B
                } else {
                    Buttons::NONE
                }
            } else {
                mash_a(w, n)
            }
        },
        |w| {
            if w.screens_open() > 0 {
                opened = true;
                false
            } else {
                opened
            }
        },
    );
    let mon = w.party().get(0).unwrap();
    assert_eq!((mon.species, mon.level), (WATER, 6));
    assert!(cancelled.scene_before_change);
    assert!(
        !cancelled
            .events
            .iter()
            .any(|e| matches!(e, Event::Evolved(_)))
    );
    assert_eq!(
        w.s.bag.count(CANDY),
        0,
        "the candy is used before the scene"
    );

    // A stone cannot be cancelled (Gen3 `canStopEvo` is false).
    let mut opened = false;
    let mut w = fresh();
    let t = EGame.battle();
    let mon = Pokemon::new(t, NORMAL, 10, Ivs::Fixed(0), 0, 0, &mut Rng::new(1));
    w.s.party.push(mon).unwrap();
    assert!(w.s.bag.add(Pocket::Items, STONE, 1));
    assert!(w.use_item(&EGame, STONE));
    let stone = drive(
        &mut w,
        NORMAL,
        |w, n| {
            if evo_open(w) {
                if n % 2 == 0 {
                    Buttons::B
                } else {
                    Buttons::NONE
                }
            } else {
                mash_a(w, n)
            }
        },
        |w| {
            if w.screens_open() > 0 {
                opened = true;
                false
            } else {
                opened
            }
        },
    );
    assert_eq!(w.party().get(0).unwrap().species, BUG);
    assert!(stone.scene_before_change);
    assert!(stone.events.contains(&Event::Evolved(BUG)));
    assert!(stone.events.contains(&Event::Cry(NORMAL)));
    assert!(stone.events.contains(&Event::Cry(BUG)));
    no_evolution_report(&stone.events);
    assert_eq!(w.s.bag.count(STONE), 0);

    // The item use itself does not change the species or report the scene.
    let mut mon = ready(WATER, 70);
    let mut reports = Vec::new();
    let used = crate::item_use::use_on_mon(&EGame, &mut mon, CANDY, &mut Rng::new(1), &mut reports)
        .unwrap();
    assert_eq!(
        used.scene,
        Some(EvolutionPlay {
            into: FIRE,
            can_cancel: true
        })
    );
    assert_eq!(mon.species, WATER);
    assert!(reports.is_empty());
}
