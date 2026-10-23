//! Warp exits on real content: Petalburg Woods' north exit leads onto Route 104's
//! `MB_DEEP_SOUTH_WARP` cells, which Gen3 leaves facing and walking north (the cell south of
//! them is solid). Returns at once on the stub and on Silice (Emerald maps only).

use content::Emerald;
use engine::data::{Dir, GameData, MapId, NewGame, ScriptId, block_collision};
use engine::script::{Ctx, Script};
use engine::{Buttons, World};

/// Emerald, starting a new game in Petalburg Woods south of its north exit, without the
/// new-game script.
struct InWoods {
    inner: Emerald,
    woods: MapId,
}

impl GameData for InWoods {
    explore::forward_game_data!(inner; map has_map special_ids sprite player_sprite font window
        text ui_text behavior build_id battle trainer battle_text battle_gfx heal_location
        wild_encounters m8);

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        self.inner.script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: self.woods,
            x: 14,
            y: 6,
            facing: Dir::Up,
            script: None,
            ..self.inner.new_game()
        }
    }
}

fn map_named(game: &Emerald, name: &str) -> MapId {
    content::data::MAP_IDS
        .iter()
        .copied()
        .find(|&id| game.map(id).name == name)
        .unwrap_or_else(|| panic!("no map {name}"))
}

/// Holds `keys` until the player stands still on `to` with control, then releases for a
/// frame. Panics after 600 frames.
fn go(game: &InWoods, w: &mut World, keys: Buttons, to: (MapId, (i16, i16))) {
    for _ in 0..600 {
        let p = w.objects().player();
        if (w.map(), p.pos()) == to && p.is_settled() && !w.transition_active() {
            w.step(game, Buttons::NONE);
            return;
        }
        w.step(game, keys);
    }
    let p = w.objects().player().pos();
    panic!("never reached {to:?}, at {:?} {p:?}", w.map());
}

#[test]
fn walking_north_out_of_petalburg_woods_lands_on_route_104_and_goes_on_north() {
    if content::FLAVOR != content::Flavor::Emerald {
        return;
    }
    let inner = Emerald::for_shells();
    let woods = map_named(&inner, "PetalburgWoods");
    let route104 = map_named(&inner, "Route104");
    let game = InWoods { inner, woods };
    let mut w = World::new(&game);
    for _ in 0..30 {
        w.step(&game, Buttons::NONE);
    }
    assert_eq!((w.map(), w.objects().player().pos()), (woods, (14, 6)));
    // Woods (14, 5) warps to Route 104's warp 2, the deep south warp (10, 30); the exit walk
    // goes north to (10, 29), not south into the solid (10, 31).
    let m = game.map(route104);
    assert_ne!(block_collision(m.blocks[31 * m.width as usize + 10]), 0);
    go(&game, &mut w, Buttons::UP, (route104, (10, 29)));
    assert_eq!(w.objects().player().facing, Dir::Up);
    let below = engine::flow::behavior_at(&game, m, 10, 30);
    assert_eq!(below, engine::data::Behavior::NonAnimDoor(Dir::Up));
    // The cell is walkable and the player has control: on north, towards Rustboro.
    go(&game, &mut w, Buttons::UP, (route104, (10, 28)));
}
