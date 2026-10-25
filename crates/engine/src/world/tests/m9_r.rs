//! Milestone 9 Task R (SIL2's requests) and the Silice wave's game rules: screen flash, shake,
//! white fades, text on black, camera pan, the timed wait, money and coins, friendship,
//! `givemon`'s extras, the evolution request, the trade-evolution item, the input map and the
//! idle counter.

use super::*;
use crate::data::{EvoMethod, Evolution, ItemUse};
use crate::fx::ScreenFx;
use crate::item_use::EvolutionPlay;
use crate::script::{GiveMonExtra, Report};
use crate::ui::ScreenKindTag;
use crate::video::Cmd;

const VAR_INPUT_MAP: VarId = VarId(0x4050);
const VAR_IDLE: VarId = VarId(0x4051);
const LINK_STONE: ItemId = ItemId(400);
const MODULE_FLAG: FlagId = FlagId(0x0F00);
const MODULE_LABEL: &[u8] = &[0xBB, 0xBC, 0xFF];
const TRADE_ITEM: ItemId = ItemId(401);
const SPECIES: SpeciesId = crate::battle::test_data::WATER;
const EVOLVED: SpeciesId = crate::battle::test_data::NORMAL;
const ITEM_EVOLVED: SpeciesId = crate::battle::test_data::FIRE;

static EVOS: [Evolution; 1] = [Evolution {
    method: EvoMethod::Trade,
    param: 0,
    target: EVOLVED,
}];
static ITEM_EVOS: [Evolution; 1] = [Evolution {
    method: EvoMethod::Trade,
    param: TRADE_ITEM.0,
    target: ITEM_EVOLVED,
}];

/// [`Game`] with the input-map and idle vars, coins, and a trade-evolution item.
struct RGame;

impl GameData for RGame {
    fn map(&self, id: MapId) -> &'static MapData {
        Game.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            var_input_map: Some(VAR_INPUT_MAP),
            var_idle_frames: Some(VAR_IDLE),
            max_coins: 9999,
            ..Game.special_ids()
        }
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
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn item_use(&self, id: ItemId) -> ItemUse {
        if id == LINK_STONE {
            ItemUse::TradeEvolution
        } else {
            ItemUse::None
        }
    }
    fn field_move_gate(&self, m: crate::data::FieldMove) -> crate::data::FieldMoveGate {
        match m {
            crate::data::FieldMove::Fly => crate::data::FieldMoveGate::Flag(MODULE_FLAG),
            _ => crate::data::FieldMoveGate::Never,
        }
    }
    fn field_move_label(&self, m: crate::data::FieldMove) -> Option<&'static [u8]> {
        (m == crate::data::FieldMove::Fly).then_some(MODULE_LABEL)
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        match species {
            SPECIES => &EVOS,
            crate::battle::test_data::BUG => &ITEM_EVOS,
            _ => &[],
        }
    }
}

fn step(w: &mut World, keys: Buttons, frames: u32) -> Frame {
    let mut f = Frame::default();
    for _ in 0..frames {
        f = w.step(&RGame, keys);
        f.validate().unwrap();
    }
    f
}

fn new_world() -> World {
    let mut w = World::new(&RGame);
    step(&mut w, Buttons::NONE, 16);
    w
}

/// Runs frames until the script ends; the frames it took.
fn finish(w: &mut World, keys: Buttons, max: u32) -> Vec<Frame> {
    let mut frames = Vec::new();
    while w.script_running() {
        frames.push(step(w, keys, 1));
        assert!(frames.len() as u32 <= max, "script still running");
    }
    frames
}

#[test]
fn a_flash_shows_full_white_for_its_frames_and_the_script_waits() {
    let mut w = new_world();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.screen_flash(0x7FFF, 6).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    let frames = finish(&mut w, Buttons::NONE, 20);
    let white = frames
        .iter()
        .filter(|f| f.brightness == 16 && f.lighten)
        .count();
    assert_eq!(white, 6);
    assert!(w.flag(FLAG_DONE));
    assert_eq!(w.s.fx, ScreenFx::default());
    // A dark colour flashes black.
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.screen_flash(0, 2).await })
    });
    let f = step(&mut w, Buttons::NONE, 1);
    assert!(f.brightness == 16 && !f.lighten);
}

#[test]
fn a_shake_alternates_the_field_scroll_and_ends_at_rest() {
    let mut w = new_world();
    let rest = step(&mut w, Buttons::NONE, 1).scroll;
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.screen_shake(2, 5).await })
    });
    let frames = finish(&mut w, Buttons::NONE, 20);
    let dy: Vec<i16> = frames
        .iter()
        .map(|f| f.scroll[1].1.wrapping_sub(rest[1].1) as i16)
        .collect();
    assert_eq!(&dy[..4], &[2, -2, 2, -2], "{dy:?}");
    assert!(dy[4..].iter().all(|&d| d == 0), "{dy:?}");
    // The text window does not shake.
    assert!(frames.iter().all(|f| f.scroll[0] == rest[0]));
}

#[test]
fn white_fades_lighten_the_frame() {
    let mut w = new_world();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.fade(Fade::ToWhite).await;
            ctx.fade(Fade::FromWhite).await;
        })
    });
    let frames = finish(&mut w, Buttons::NONE, 60);
    assert!(frames.iter().any(|f| f.brightness == 16 && f.lighten));
    assert!(frames.iter().all(|f| f.brightness == 0 || f.lighten));
    let last = step(&mut w, Buttons::NONE, 1);
    assert!(last.brightness == 0 && !last.lighten);
    assert!(w.capturable().is_ok());
}

#[test]
fn text_on_black_hides_the_field_and_brings_it_back() {
    let mut w = new_world();
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.text_on_black(true).await })
    });
    let f = step(&mut w, Buttons::NONE, 2);
    assert!(f.objs.is_empty());
    let off = |f: &Frame, on: bool| {
        f.cmds
            .iter()
            .filter(
                |c| matches!(c, Cmd::Bg { layer, control } if *layer > 0 && control.enabled == on),
            )
            .count()
    };
    assert_eq!(off(&f, false), 3);
    assert!(w.capturable().is_err());
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.text_on_black(false).await })
    });
    let f = step(&mut w, Buttons::NONE, 1);
    assert_eq!(off(&f, true), 3);
    assert!(!f.objs.is_empty());
    assert_eq!(w.s.fx, ScreenFx::default());
}

#[test]
fn a_camera_pan_moves_at_its_speed_waits_and_comes_back() {
    let mut w = new_world();
    let rest = step(&mut w, Buttons::NONE, 1).scroll[2];
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.camera_pan(0, -32, 4).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    let frames = finish(&mut w, Buttons::NONE, 30);
    assert_eq!(frames.len(), 9, "8 frames of 4 px, then the script goes on");
    let f = step(&mut w, Buttons::NONE, 1);
    assert_eq!(f.scroll[2].1.wrapping_sub(rest.1) as i16, -32);
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.camera_pan(0, 0, 8).await })
    });
    finish(&mut w, Buttons::NONE, 30);
    assert_eq!(step(&mut w, Buttons::NONE, 1).scroll[2], rest);
    assert_eq!(w.s.fx, ScreenFx::default());
}

#[test]
fn wait_input_or_timeout_answers_the_button_or_the_timeout() {
    for (keys, want, max) in [(Buttons::NONE, 0, 12), (Buttons::A, 1, 3)] {
        let mut w = new_world();
        start(&mut w, |ctx| {
            Box::pin(async move {
                let pressed = ctx.wait_input_or_timeout(10).await;
                ctx.set_var(VAR_RESULT, 10 + pressed as u16).await;
            })
        });
        step(&mut w, Buttons::NONE, 1);
        let frames = finish(&mut w, keys, max);
        assert_eq!(w.var(VAR_RESULT), 10 + want);
        if want == 0 {
            assert_eq!(frames.len(), 10);
        }
    }
}

#[test]
fn money_coins_friendship_and_party_species() {
    let mut w = new_world();
    w.s.money = 1234;
    start(&mut w, |ctx| {
        Box::pin(async move {
            let m = ctx.money_get().await;
            ctx.set_var(VarId(0x4060), m as u16).await;
            let fit = ctx.coins_add(9000).await;
            let over = ctx.coins_add(1000).await;
            let paid = ctx.coins_remove(500).await;
            let broke = ctx.coins_remove(9000).await;
            ctx.set_var(
                VarId(0x4061),
                fit as u16 | (over as u16) << 1 | (paid as u16) << 2 | (broke as u16) << 3,
            )
            .await;
            let c = ctx.coins_get().await;
            ctx.set_var(VarId(0x4062), c).await;
            ctx.give_mon(SPECIES, 5).await;
            ctx.add_friendship(0, 100).await;
            ctx.add_friendship(0, 100).await;
            ctx.add_friendship(0xFF, 100).await;
            let s = ctx.party_mon_species(0).await;
            let none = ctx.party_mon_species(3).await;
            ctx.set_var(VarId(0x4063), s).await;
            ctx.set_var(VarId(0x4064), none).await;
        })
    });
    finish(&mut w, Buttons::NONE, 10);
    assert_eq!(w.var(VarId(0x4060)), 1234);
    assert_eq!(w.var(VarId(0x4061)), 0b0101);
    assert_eq!(w.var(VarId(0x4062)), 8500);
    assert_eq!(w.coins(), 8500);
    assert_eq!(w.party().get(0).unwrap().friendship, 255, "clamped");
    assert_eq!(w.var(VarId(0x4063)), SPECIES.0);
    assert_eq!(w.var(VarId(0x4064)), 0);
}

static EXTRA: GiveMonExtra = GiveMonExtra {
    moves: Some([
        crate::battle::test_data::SWIFT,
        crate::battle::test_data::BITE,
        MoveId(0),
        MoveId(0),
    ]),
    item: Some(TRADE_ITEM),
    ot: Some((&[1, 0xFF], 4242)),
    nickname: Some(TextId(0)),
};

#[test]
fn givemon_extras_set_the_moves_item_ot_and_nickname() {
    let mut w = new_world();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon_with(SPECIES, 20, &EXTRA).await;
        })
    });
    finish(&mut w, Buttons::NONE, 10);
    let m = w.party().get(0).unwrap();
    let t = RGame.battle();
    assert_eq!(
        m.moves,
        [
            Some(crate::battle::test_data::SWIFT),
            Some(crate::battle::test_data::BITE),
            None,
            None
        ]
    );
    assert_eq!(m.pp[0], t.move_info(crate::battle::test_data::SWIFT).pp);
    assert_eq!(m.pp[2], 0);
    assert_eq!(m.held, Some(TRADE_ITEM));
    assert_eq!(m.ot_id, 4242);
    assert_eq!(&m.nickname[..2], &RGame.text(TextId(0))[..2]);
}

#[test]
fn the_evolve_request_plays_the_scene() {
    let mut w = new_world();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(SPECIES, 10).await;
            let yes = ctx.evolve(0, EVOLVED, true).await;
            let no = ctx.evolve(4, EVOLVED, true).await;
            ctx.set_var(VAR_RESULT, yes as u16 | (no as u16) << 1).await;
        })
    });
    let mut frames = 0u32;
    let mut events = Vec::new();
    while w.script_running() {
        let keys = if frames % 2 == 0 {
            Buttons::A
        } else {
            Buttons::NONE
        };
        step(&mut w, keys, 1);
        events.extend_from_slice(w.events());
        frames += 1;
        assert!(frames <= 4_000, "script still running");
    }
    assert_eq!(w.var(VAR_RESULT), 1);
    assert_eq!(w.party().get(0).unwrap().species, EVOLVED);
    assert!(events.contains(&Event::ScreenOpened(ScreenKindTag::Evolution)));
    assert!(events.iter().any(|e| matches!(e, Event::Cry(_))));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Report(Report::Degraded("evolution scene"))))
    );
}

#[test]
fn the_trade_evolution_item_evolves_trade_evolvers_and_uses_the_held_item() {
    let mut rng = crate::Rng::new(1);
    let t = RGame.battle();
    let mut reports = Vec::new();
    let mut mon =
        crate::mon::Pokemon::new(t, SPECIES, 20, crate::mon::Ivs::Fixed(0), 1, 1, &mut rng);
    let used = crate::item_use::use_on_mon(&RGame, &mut mon, LINK_STONE, &mut rng, &mut reports);
    assert_eq!(
        used.unwrap().scene,
        Some(EvolutionPlay {
            into: EVOLVED,
            can_cancel: false
        })
    );
    assert_eq!(mon.species, SPECIES);
    assert!(reports.is_empty());
    mon.evolve(t, EVOLVED);
    // No trade evolution: no effect, the item is kept.
    assert!(
        crate::item_use::use_on_mon(&RGame, &mut mon, LINK_STONE, &mut rng, &mut reports).is_none()
    );
    // A trade with an item: only while holding it, and it is used up. The scene changes the species.
    let bug = crate::battle::test_data::BUG;
    let mut mon = crate::mon::Pokemon::new(t, bug, 20, crate::mon::Ivs::Fixed(0), 1, 1, &mut rng);
    assert!(
        crate::item_use::use_on_mon(&RGame, &mut mon, LINK_STONE, &mut rng, &mut reports).is_none()
    );
    mon.held = Some(TRADE_ITEM);
    let used = crate::item_use::use_on_mon(&RGame, &mut mon, LINK_STONE, &mut rng, &mut reports);
    assert_eq!(
        used.unwrap().scene,
        Some(EvolutionPlay {
            into: ITEM_EVOLVED,
            can_cancel: false
        })
    );
    assert_eq!(mon.species, bug);
    assert_eq!(mon.held, None);
    assert_eq!(
        crate::item_use::field_use(&RGame, LINK_STONE),
        crate::item_use::FieldUse::Party
    );
}

#[test]
fn the_input_map_inverts_the_field_dpad_and_is_cleared_on_map_change() {
    let mut w = new_world();
    let (x, y) = w.objects().player().pos();
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.set_input_map(crate::InputMap::Inverted).await })
    });
    step(&mut w, Buttons::NONE, 2);
    assert_eq!(w.var(VAR_INPUT_MAP), crate::InputMap::Inverted.code());
    // Holding UP walks down; holding RIGHT walks left.
    for _ in 0..40 {
        if w.objects().player().pos() != (x, y) && w.objects().player().is_settled() {
            break;
        }
        step(&mut w, Buttons::UP, 1);
    }
    assert_eq!(w.objects().player().pos(), (x, y + 1));
    assert_eq!(w.objects().player().facing, Dir::Down);
    // A map change clears it.
    w.s.clear_temp(&RGame.special_ids());
    assert_eq!(w.var(VAR_INPUT_MAP), 0);
}

#[test]
fn the_idle_counter_counts_field_frames_without_buttons() {
    let mut w = new_world();
    step(&mut w, Buttons::NONE, 30);
    let idle = w.var(VAR_IDLE);
    assert!(idle >= 30, "{idle}");
    step(&mut w, Buttons::B, 1);
    assert_eq!(w.var(VAR_IDLE), 0);
    step(&mut w, Buttons::NONE, 5);
    assert_eq!(w.var(VAR_IDLE), 5);
    // A running script freezes it.
    start(&mut w, |ctx| Box::pin(async move { ctx.delay(10).await }));
    step(&mut w, Buttons::NONE, 5);
    assert_eq!(w.var(VAR_IDLE), 5);
}

#[test]
fn the_new_requests_hash_and_hash_differently() {
    use crate::script::Request;
    let h = |r: Request| {
        let mut f = crate::hash::Fnv64::new();
        r.hash_into(&mut f);
        f.finish()
    };
    let all = [
        h(Request::TextOnBlack(true)),
        h(Request::TextOnBlack(false)),
        h(Request::CameraPan {
            x: 0,
            y: -32,
            speed: 4,
        }),
        h(Request::CameraPan {
            x: 0,
            y: 0,
            speed: 4,
        }),
        h(Request::SetInputMap(crate::InputMap::Inverted)),
        h(Request::SetInputMap(crate::InputMap::Normal)),
    ];
    for i in 0..all.len() {
        for j in i + 1..all.len() {
            assert_ne!(all[i], all[j], "{i} {j}");
        }
    }
}

#[test]
fn a_flag_gated_field_move_is_listed_on_every_mon_with_the_games_label() {
    use crate::data::FieldMove;
    use crate::ui::party::{Action, action_label, field_moves};
    let mon = crate::battle::test_data::mon(SPECIES, 10, [20; 6], &[]);
    assert!(field_moves(&RGame, &mon, |_| false).is_empty());
    let listed = field_moves(&RGame, &mon, |f| f == MODULE_FLAG);
    assert_eq!(listed, vec![FieldMove::Fly]);
    assert_eq!(
        action_label(&RGame, Action::FieldMove(FieldMove::Fly)),
        MODULE_LABEL
    );
}
