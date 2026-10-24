//! A game's own in-game partners (`Request::BattleMulti`'s `partner`: a trainer's party
//! beside the player's, not only the game's one `GameData::partner`) and wild double battles
//! (`Request::BattleWildDouble`, which Emerald lacks), on the synthetic game. The player's
//! mons past the first three sit on the bench while the partner's fight, and come back.

use super::*;
use crate::battle::BattleOutcome;
use crate::battle::test_data::{NORMAL, TACKLE, WATER};

/// A partner of the game's own, from its `gTrainers` party (four mons: three join).
const SIDEKICK: TrainerId = TrainerId(9);
static SIDEKICK_DATA: crate::mon::TrainerData = crate::mon::TrainerData {
    class_name: &[1, 0xFF],
    name: &[3, 0xFF],
    pic: &TRAINER_PIC,
    female: false,
    ai: 0,
    double: false,
    party: &[sidekick_mon(31); 4],
};

const fn sidekick_mon(level: u8) -> crate::mon::TrainerMon {
    crate::mon::TrainerMon {
        species: WATER,
        level,
        iv: 0,
        moves: Some([Some(TACKLE), None, None, None]),
        item: None,
    }
}

/// [`Game`] with [`SIDEKICK`].
struct Partners;

impl GameData for Partners {
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
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static crate::mon::TrainerData> {
        if id == SIDEKICK {
            Some(&SIDEKICK_DATA)
        } else {
            Game.trainer(id)
        }
    }
    fn partner(&self) -> Option<&'static crate::mon::PartnerData> {
        Game.partner()
    }
}

fn step(w: &mut World, keys: Buttons) {
    w.step(&Partners, keys).validate().unwrap();
}

fn begin(script: impl FnOnce(Ctx) -> Script) -> World {
    let mut w = World::new(&Partners);
    for _ in 0..16 {
        step(&mut w, Buttons::NONE);
    }
    let ctx = w.runner.ctx();
    w.runner.start(script(ctx));
    for _ in 0..200 {
        if w.battle().is_some() {
            return w;
        }
        step(&mut w, Buttons::NONE);
    }
    panic!("no battle");
}

/// Presses A every other frame until the script ends.
fn finish(w: &mut World) {
    for n in 0..40_000 {
        if !w.script_running() {
            return;
        }
        step(
            w,
            if n % 2 == 0 {
                Buttons::A
            } else {
                Buttons::NONE
            },
        );
    }
    panic!("the battle never ended");
}

/// Gives five NORMAL mons (levels 40..45), then runs `battle`; FLAG_DONE: won.
async fn five_then(ctx: Ctx, battle: impl core::future::Future<Output = BattleOutcome>) {
    for l in 40..45 {
        ctx.give_mon(NORMAL, l).await;
    }
    let o = battle.await;
    ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
}

fn levels(w: &World) -> Vec<(SpeciesId, u8)> {
    w.party().iter().map(|m| (m.species, m.level)).collect()
}

#[test]
fn a_games_partner_fights_with_its_trainer_party_and_the_bench_comes_back() {
    let mut w = begin(|ctx| {
        Box::pin(async move {
            let c = ctx.clone();
            let b = ctx.battle_multi_with(SIDEKICK, (TRAINER, TextId(0)), (TRAINER, TextId(0)));
            five_then(c, b).await;
        })
    });
    let (_, b, _) = w.battle().expect("a battle");
    let d = b.setup().double.expect("a double battle");
    assert_eq!(d.partner, Some(SIDEKICK));
    assert_eq!(d.split[0], 3);
    let party = b.party(crate::battle::Side::Player);
    let mons: Vec<_> = party.iter().map(|m| (m.species, m.level)).collect();
    assert_eq!(
        mons,
        [
            (NORMAL, 40),
            (NORMAL, 41),
            (NORMAL, 42),
            (WATER, 31),
            (WATER, 31),
            (WATER, 31)
        ],
        "the player's first three, then three of the partner's party"
    );
    finish(&mut w);
    assert!(w.flag(FLAG_DONE), "won");
    let back = levels(&w);
    assert_eq!(
        back.len(),
        5,
        "the partner left, the bench came back: {back:?}"
    );
    assert!(back.iter().all(|&(s, _)| s == NORMAL), "{back:?}");
    assert_eq!(back[3..], [(NORMAL, 43), (NORMAL, 44)]);
}

#[test]
fn the_games_one_partner_is_still_the_default() {
    let mut w = begin(|ctx| {
        Box::pin(async move {
            let c = ctx.clone();
            let b = ctx.battle_multi((TRAINER, TextId(0)), (TRAINER, TextId(0)));
            five_then(c, b).await;
        })
    });
    let (_, b, _) = w.battle().expect("a battle");
    assert_eq!(b.setup().double.and_then(|d| d.partner), Some(TRAINER));
    let p = b.party(crate::battle::Side::Player).get(3).unwrap();
    assert_eq!((p.species, p.ot_id), (WATER, 61226), "FillPartnerParty");
    finish(&mut w);
    assert_eq!(levels(&w).len(), 5);
}

#[test]
fn a_wild_double_against_two_wild_mons_with_the_players_two() {
    let mut w = begin(|ctx| {
        Box::pin(async move {
            let c = ctx.clone();
            let b = ctx.battle_wild_double([(NORMAL, 2), (WATER, 3)], None, None);
            five_then(c, b).await;
        })
    });
    let (_, b, _) = w.battle().expect("a battle");
    assert_eq!(b.count(), 4);
    assert!(matches!(
        b.setup().kind,
        crate::battle::BattleKind::Wild { .. }
    ));
    let foes: Vec<_> = b
        .party(crate::battle::Side::Opponent)
        .iter()
        .map(|m| (m.species, m.level))
        .collect();
    assert_eq!(foes, [(NORMAL, 2), (WATER, 3)]);
    assert_eq!(b.setup().double.and_then(|d| d.partner), None);
    finish(&mut w);
    assert!(w.flag(FLAG_DONE), "won");
    assert_eq!(levels(&w).len(), 5);
}

#[test]
fn a_wild_double_beside_a_partner() {
    let mut w = begin(|ctx| {
        Box::pin(async move {
            let c = ctx.clone();
            let b = ctx.battle_wild_double([(NORMAL, 2), (NORMAL, 2)], Some(SIDEKICK), None);
            five_then(c, b).await;
        })
    });
    let (_, b, _) = w.battle().expect("a battle");
    let d = b.setup().double.expect("a double battle");
    assert_eq!((d.partner, d.split[0]), (Some(SIDEKICK), 3));
    assert_eq!(b.party(crate::battle::Side::Player).len(), 6);
    finish(&mut w);
    assert!(w.flag(FLAG_DONE), "won");
    let back = levels(&w);
    assert_eq!(back.len(), 5, "{back:?}");
    assert!(back.iter().all(|&(s, _)| s == NORMAL), "{back:?}");
}
