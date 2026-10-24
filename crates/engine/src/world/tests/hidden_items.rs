//! Hidden items (Gen3 `BG_EVENT_HIDDEN_ITEM`, `GetInteractedBackgroundEventScript`): A while
//! facing one whose flag is clear sets `VAR_0x8004` (the flag) and `VAR_0x8005` (the item) and
//! runs the game's `EventScript_HiddenItemScript` (`GameData::hidden_item_script`); once the
//! flag is set, or with no such script, the bg event is passed over.

use super::*;

/// The town with a hidden item under the player's start (5, 6), and the game's hidden-item
/// script (`hook`) or none.
struct HiddenGame {
    hook: bool,
}

const HIDDEN_FLAG: FlagId = FlagId(0x2A0);
const HIDDEN_ITEM: u16 = 0x0D;
const HIDDEN_SCRIPT: ScriptId = ScriptId(40);
/// What the hidden-item script saw in `VAR_0x8005`.
const VAR_SEEN_ITEM: VarId = VarId(0x4070);

static HIDDEN_BGS: [BgEvent; 1] = [BgEvent {
    x: 5,
    y: 6,
    elevation: 0,
    kind: BgKind::HiddenItem {
        item: HIDDEN_ITEM,
        flag: HIDDEN_FLAG,
    },
}];

static HIDDEN_TOWN: MapData = MapData {
    name: "town with a hidden item",
    width: MAPS[0].width,
    height: MAPS[0].height,
    blocks: MAPS[0].blocks,
    border: MAPS[0].border,
    primary: MAPS[0].primary,
    secondary: MAPS[0].secondary,
    objects: &[],
    warps: &[],
    coords: &[],
    bgs: &HIDDEN_BGS,
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: true,
    map_type: 0,
};

/// Gen3 `EventScript_HiddenItemScript`, reduced: the item seen, its flag set.
async fn hidden_item(ctx: Ctx) {
    ctx.lock_all().await;
    let flag = ctx.var(VarId(0x8004)).await;
    let item = ctx.var(VarId(0x8005)).await;
    ctx.set_var(VAR_SEEN_ITEM, item).await;
    ctx.set_flag(FlagId(flag), true).await;
    ctx.release_all().await;
}

impl GameData for HiddenGame {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == TOWN {
            &HIDDEN_TOWN
        } else {
            Game.map(id)
        }
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
        if id == HIDDEN_SCRIPT {
            Box::pin(hidden_item(ctx))
        } else {
            Game.script(id, ctx)
        }
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn hidden_item_script(&self) -> Option<ScriptId> {
        self.hook.then_some(HIDDEN_SCRIPT)
    }
}

fn step(g: &HiddenGame, w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(g, keys).validate().unwrap();
    }
}

fn started(g: &HiddenGame) -> World {
    let mut w = World::new(g);
    step(g, &mut w, Buttons::NONE, 16);
    assert_eq!(w.objects().player().pos(), (5, 5));
    assert_eq!(w.objects().player().facing, Dir::Down);
    w
}

#[test]
fn a_hidden_item_runs_the_games_script_until_its_flag_is_set() {
    let g = HiddenGame { hook: true };
    let mut w = started(&g);
    step(&g, &mut w, Buttons::A, 1);
    step(&g, &mut w, Buttons::NONE, 3);
    assert_eq!(w.var(VarId(0x8004)), HIDDEN_FLAG.0);
    assert_eq!(w.var(VarId(0x8005)), HIDDEN_ITEM);
    assert_eq!(w.var(VAR_SEEN_ITEM), HIDDEN_ITEM);
    assert!(w.flag(HIDDEN_FLAG));
    assert!(!w.script_running());
    // Picked up: the bg event is passed over.
    w.s.vars.set_var(VAR_SEEN_ITEM, 0);
    step(&g, &mut w, Buttons::A, 1);
    step(&g, &mut w, Buttons::NONE, 3);
    assert_eq!(w.var(VAR_SEEN_ITEM), 0);
    assert!(!w.script_running());
}

#[test]
fn without_a_hidden_item_script_hidden_items_are_passed_over() {
    let g = HiddenGame { hook: false };
    let mut w = started(&g);
    step(&g, &mut w, Buttons::A, 1);
    assert!(!w.script_running());
    assert!(!w.flag(HIDDEN_FLAG));
    assert_eq!(w.var(VarId(0x8004)), 0);
}
