//! Strength pushes beyond Gen3's one boulder graphic (`GameData::pushable_objects`), a game's
//! rule on where a pushed object may go (`GameData::push_allowed`), and the script a push runs
//! (`GameData::pushed_script`, with `VAR_0x8004` the object, `VAR_0x8005`/`VAR_0x8006` its new
//! cell): what a game's own pushable things (crates, barrels) need on top of `TryPushBoulder`.

use super::*;

/// The town with a crate at (5, 7) whose graphics are not the boulder's.
struct Yard {
    rule: bool,
}

const FLAG_STRENGTH: FlagId = FlagId(0x880);
const BOULDER: GfxId = GfxId(77);
const CRATE: GfxId = NPC_GFX;
const PUSHED: ScriptId = ScriptId(41);
const SEEN: [VarId; 3] = [VarId(0x4072), VarId(0x4073), VarId(0x4074)];
/// The cell the rule refuses.
const REFUSED: (i16, i16) = (4, 8);

static YARD_OBJECTS: [ObjectEvent; 1] = [{
    let mut o = scriptless(1, 5, 7);
    o.gfx = Gfx::Fixed(CRATE);
    o
}];

static YARD: MapData = MapData {
    name: "yard",
    width: MAPS[0].width,
    height: MAPS[0].height,
    blocks: MAPS[0].blocks,
    border: MAPS[0].border,
    primary: MAPS[0].primary,
    secondary: MAPS[0].secondary,
    objects: &YARD_OBJECTS,
    warps: &[],
    coords: &[],
    bgs: &[],
    connections: &[],
    scripts: NO_SCRIPTS,
    allow_running: true,
    map_type: 0,
};

/// Records the push it is told about.
async fn pushed(ctx: Ctx) {
    for (i, v) in SEEN.into_iter().enumerate() {
        let n = ctx.var(VarId(0x8004 + i as u16)).await;
        ctx.set_var(v, n).await;
    }
}

impl GameData for Yard {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == TOWN { &YARD } else { Game.map(id) }
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
        if id == PUSHED {
            Box::pin(pushed(ctx))
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
    fn pushable_boulder(&self) -> Option<(GfxId, FlagId)> {
        Some((BOULDER, FLAG_STRENGTH))
    }
    fn pushable_objects(&self) -> &'static [GfxId] {
        if self.rule { &[CRATE] } else { &[] }
    }
    fn push_allowed(&self, map: MapId, local_id: u8, _: (i16, i16), to: (i16, i16)) -> bool {
        !(self.rule && map == TOWN && local_id == 1 && to == REFUSED)
    }
    fn pushed_script(&self, map: MapId) -> Option<ScriptId> {
        (self.rule && map == TOWN).then_some(PUSHED)
    }
}

fn keys(d: Dir) -> Buttons {
    match d {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    }
}

fn hold(g: &Yard, w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(g, keys).validate().unwrap();
    }
}

fn walk(g: &Yard, w: &mut World, d: Dir) {
    let from = w.objects().player().pos();
    for _ in 0..100 {
        hold(g, w, keys(d), 1);
        let p = w.objects().player();
        if p.pos() != from && p.is_settled() {
            return;
        }
    }
    panic!("stuck at {from:?} going {d:?}");
}

fn yard(g: &Yard) -> World {
    let mut w = World::new(g);
    hold(g, &mut w, Buttons::NONE, 16);
    assert_eq!(w.objects().player().pos(), (5, 5));
    w
}

fn crate_pos(w: &World) -> (i16, i16) {
    w.objects().get(1).unwrap().pos()
}

/// One push attempt towards `d`, then the push (or bump) played out.
fn push(g: &Yard, w: &mut World, d: Dir) {
    hold(g, w, keys(d), 1);
    hold(g, w, Buttons::NONE, 40);
}

#[test]
fn a_games_own_pushable_objects_obey_its_rule_and_report_each_push() {
    let g = Yard { rule: true };
    let mut w = yard(&g);
    walk(&g, &mut w, Dir::Down);
    // Without Strength in use: a bump.
    push(&g, &mut w, Dir::Down);
    assert_eq!(crate_pos(&w), (5, 7));
    w.s.vars.set_flag(FLAG_STRENGTH, true);
    push(&g, &mut w, Dir::Down);
    assert_eq!(crate_pos(&w), (5, 8));
    assert_eq!(
        SEEN.map(|v| w.var(v)),
        [1, 5, 8],
        "the pushed script saw the push"
    );
    assert!(!w.script_running());
    // Left onto the refused cell: a bump, no script.
    for v in SEEN {
        w.s.vars.set_var(v, 0);
    }
    walk(&g, &mut w, Dir::Right);
    walk(&g, &mut w, Dir::Down);
    assert_eq!(w.objects().player().pos(), (6, 7));
    walk(&g, &mut w, Dir::Down);
    assert_eq!(w.objects().player().pos(), (6, 8));
    push(&g, &mut w, Dir::Left);
    assert_eq!(crate_pos(&w), (5, 8));
    assert_eq!(w.objects().player().pos(), (6, 8));
    assert_eq!(SEEN.map(|v| w.var(v)), [0, 0, 0]);
}

#[test]
fn without_the_hooks_only_the_boulder_graphic_is_pushed() {
    let g = Yard { rule: false };
    let mut w = yard(&g);
    walk(&g, &mut w, Dir::Down);
    w.s.vars.set_flag(FLAG_STRENGTH, true);
    push(&g, &mut w, Dir::Down);
    assert_eq!(crate_pos(&w), (5, 7));
}
