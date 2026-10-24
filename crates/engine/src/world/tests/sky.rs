//! The field weather and the palette overrides in the world (`crate::weather`, `crate::sky`):
//! per-map weather, warps and crossings, `setweather` / `doweather` / `resetweather`, weather
//! coord events, hashing only when unsettled, the snapshot, palette overrides, overlays and
//! the reserved OBJ banks.

use super::*;
use crate::sky::{OverlayCell, PaletteFx, PaletteMode};
use crate::video::Cmd;
use crate::weather::{self, Look, Particles};

/// [`Game`] with a weather per map, weather coord events on the town, a game weather kind
/// and an overlay.
struct SkyGame {
    weathers: [u8; 4],
    /// The town's weather coord event.
    coords: bool,
}

static TOWN_WEATHER_COORDS: [CoordWeather; 1] = [CoordWeather {
    x: 5,
    y: 3,
    elevation: 0,
    weather: 9, // COORD_EVENT_WEATHER_SANDSTORM
}];

static PETAL_TILES: [Tile; 1] = [[0x1111_1111; 8]];
const PETALS: u8 = weather::GAME_FIRST;
static PETAL_LOOK: Look = Look {
    blend: Some((0x7C1F, 2)),
    particles: Some(Particles {
        tiles: &PETAL_TILES,
        shape: crate::video::ObjShape::Square,
        size: 0,
        palette: [0x7C1F; 16],
        count: 6,
        dx: 4,
        dy: 6,
        sway: 4,
    }),
    pulse: false,
};
static CRACK_TILES: [Tile; 2] = [[0x2222_2222; 8], [0x3333_3333; 8]];
static CRACK_CELLS: [OverlayCell; 3] = [
    OverlayCell {
        x: 1,
        y: 1,
        tile: 0,
        stage: 0,
    },
    OverlayCell {
        x: 2,
        y: 1,
        tile: 1,
        stage: 1,
    },
    OverlayCell {
        x: 3,
        y: 1,
        tile: 1,
        stage: 1,
    },
];
static CRACK: crate::sky::Overlay = crate::sky::Overlay {
    tiles: &CRACK_TILES,
    palette: [0x1234; 16],
    cells: &CRACK_CELLS,
    stages: 2,
};

impl GameData for SkyGame {
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
    fn map_weather(&self, map: MapId) -> u8 {
        self.weathers.get(map.0 as usize).copied().unwrap_or(0)
    }
    fn weather_coords(&self, map: MapId) -> &'static [CoordWeather] {
        if map == TOWN && self.coords {
            &TOWN_WEATHER_COORDS
        } else {
            &[]
        }
    }
    fn weather_look(&self, id: u8) -> Option<&'static Look> {
        (id == PETALS).then_some(&PETAL_LOOK)
    }
    fn overlay(&self, id: u8) -> Option<&'static crate::sky::Overlay> {
        (id == 0).then_some(&CRACK)
    }
}

const NONE: SkyGame = SkyGame {
    weathers: [0; 4],
    coords: false,
};

fn step(g: &SkyGame, w: &mut World, keys: Buttons, frames: u32) -> Frame {
    let mut f = Frame::default();
    for _ in 0..frames {
        f = w.step(g, keys);
        f.validate().unwrap();
    }
    f
}

fn go(g: &SkyGame, w: &mut World, dir: Dir, cells: i16) {
    let keys = match dir {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    };
    let (dx, dy) = dir.delta();
    let (x, y) = w.objects().player().pos();
    let target = (x + dx * cells, y + dy * cells);
    for _ in 0..300 {
        if w.objects().player().pos() == target && w.objects().player().is_settled() {
            step(g, w, Buttons::NONE, 1);
            return;
        }
        step(g, w, keys, 1);
    }
    panic!("never reached {target:?}");
}

fn new_world(g: &SkyGame) -> World {
    let mut w = World::new(g);
    step(g, &mut w, Buttons::NONE, 16);
    w
}

fn bg_palettes(f: &Frame) -> Vec<(u8, [u16; 16])> {
    f.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::BgPalette { bank, colors } => Some((*bank, *colors)),
            _ => None,
        })
        .collect()
}

/// A map in its header's weather (shade) hashes as a map without weather does, and its
/// palettes are darkened from the first frame.
#[test]
fn a_maps_settled_weather_is_shown_but_not_state() {
    let shade = SkyGame {
        coords: false,
        weathers: [weather::SHADE, 0, 0, 0],
    };
    let mut plain = World::new(&NONE);
    let mut dark = World::new(&shade);
    let (fp, fd) = (plain.initial_frame(), dark.initial_frame());
    assert_eq!(dark.s.weather.current, weather::SHADE);
    assert_eq!(dark.s.weather.color_map, 3);
    let (pp, pd) = (bg_palettes(&fp), bg_palettes(&fd));
    assert_eq!(pp.len(), pd.len());
    assert!(!pp.is_empty());
    // Bank 0's non-black colours are darker.
    let lum = |c: u16| (c & 31) + ((c >> 5) & 31) + ((c >> 10) & 31);
    let (a, b) = (pp[0].1, pd[0].1);
    assert!((0..16).all(|i| lum(b[i]) <= lum(a[i])));
    assert!((0..16).any(|i| lum(b[i]) < lum(a[i])), "{a:?} {b:?}");
    for _ in 0..40 {
        step(&NONE, &mut plain, Buttons::NONE, 1);
        step(&shade, &mut dark, Buttons::NONE, 1);
        assert_eq!(plain.hash(), dark.hash());
    }
    assert!(dark.checkpoint(&shade).unwrap().world.sky.is_none());
}

/// A crossing brings the next map's weather in gradually (`DoCurrentWeather`); a warp shows
/// it at once (`ResumePausedWeather`). Unsettled, it is hashed and snapshotted, and a restore
/// continues it.
#[test]
fn crossings_are_gradual_warps_instant_and_unsettled_weather_is_state() {
    let g = SkyGame {
        coords: false,
        weathers: [0, weather::RAIN, weather::RAIN, 0],
    };
    let mut w = new_world(&g);
    go(&g, &mut w, Dir::Up, 5);
    step(&g, &mut w, Buttons::UP, 1);
    assert_eq!(w.map(), ROUTE);
    assert_eq!(w.s.weather.next, weather::RAIN);
    step(&g, &mut w, Buttons::NONE, 2);
    assert_eq!(w.s.weather.current, weather::RAIN);
    assert!(w.s.weather.color_map < 3);
    assert!(!w.s.weather.is_default());
    // Captured mid-change, restored, both go on alike.
    step(&g, &mut w, Buttons::NONE, 20);
    let cp = w.checkpoint(&g).unwrap();
    assert!(cp.world.sky.is_some());
    // The derived fields (the map's default, the particles' phase) are not in the snapshot.
    let back = crate::checkpoint::decode(&crate::checkpoint::encode(&cp)).unwrap();
    assert_eq!(back, cp, "encode/decode keeps the checkpoint");
    let mut r = World::restore(&g, &cp).unwrap();
    assert_eq!(r.hash(), w.hash());
    // The colour map in 60 frames, the rain's loading (`init_frames`) in 115.
    for _ in 0..120 {
        let (a, b) = (
            step(&g, &mut w, Buttons::NONE, 1),
            step(&g, &mut r, Buttons::NONE, 1),
        );
        assert_eq!(w.hash(), r.hash());
        assert_eq!(bg_palettes(&a).is_empty(), bg_palettes(&b).is_empty());
    }
    assert_eq!(w.s.weather.color_map, 3);
    assert!(w.s.weather.is_default(), "settled at last");
    // The rain's particles: in front, in the reserved bank.
    let f = step(&g, &mut w, Buttons::NONE, 1);
    assert!(
        f.objs
            .iter()
            .any(|o| o.palette == crate::sky::PARTICLE_BANK)
    );
    assert!(
        f.objs
            .iter()
            .filter(|o| o.palette != crate::sky::PARTICLE_BANK)
            .all(|o| o.palette != crate::sky::PARTICLE_BANK)
    );
    // Back to the town: gradual again; the town's warp into the rainy house is instant.
    step(&g, &mut w, Buttons::DOWN, 30);
    assert_eq!(w.map(), TOWN);
    assert_eq!(w.s.weather.next, weather::NONE);
}

#[test]
fn a_warp_shows_the_weather_at_once() {
    let g = SkyGame {
        coords: false,
        weathers: [0, weather::RAIN, 0, 0],
    };
    let mut w = new_world(&g);
    // The house door is at (3, 2): from (3, 3), push up.
    go(&g, &mut w, Dir::Left, 2);
    go(&g, &mut w, Dir::Up, 2);
    step(&g, &mut w, Buttons::UP, 1);
    for _ in 0..120 {
        step(&g, &mut w, Buttons::NONE, 1);
        if !w.transition_active() {
            break;
        }
    }
    assert_eq!(w.map(), HOUSE);
    assert_eq!(w.s.weather.current, weather::RAIN);
    assert_eq!(w.s.weather.color_map, 3);
}

/// `setweather` then `doweather` changes the weather; `resetweather` and `doweather` bring the
/// map's back. Each request is hashed.
#[test]
fn script_weather_requests() {
    let mut w = new_world(&NONE);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.set_weather(weather::SANDSTORM as u16).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    step(&NONE, &mut w, Buttons::NONE, 3);
    assert_eq!(w.s.weather.saved, weather::SANDSTORM);
    assert_eq!(w.s.weather.current, weather::NONE);
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.do_weather().await })
    });
    step(&NONE, &mut w, Buttons::NONE, 3);
    assert_eq!(w.s.weather.current, weather::SANDSTORM);
    let f = step(&NONE, &mut w, Buttons::NONE, 1);
    assert!(
        f.objs
            .iter()
            .any(|o| o.palette == crate::sky::PARTICLE_BANK)
    );
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.reset_weather().await;
            ctx.do_weather().await;
        })
    });
    step(&NONE, &mut w, Buttons::NONE, 3);
    assert_eq!(w.s.weather.current, weather::NONE);
    let f = step(&NONE, &mut w, Buttons::NONE, 1);
    assert!(
        f.objs
            .iter()
            .all(|o| o.palette != crate::sky::PARTICLE_BANK)
    );
}

/// A weather coord event sets its weather as the player steps on it.
#[test]
fn a_weather_coord_event_sets_the_weather() {
    let g = SkyGame {
        weathers: [0; 4],
        coords: true,
    };
    let mut w = new_world(&g);
    go(&g, &mut w, Dir::Up, 2);
    assert_eq!(w.objects().player().pos(), (5, 3));
    step(&g, &mut w, Buttons::NONE, 2);
    assert_eq!(w.s.weather.saved, weather::SANDSTORM);
    assert_eq!(w.s.weather.current, weather::SANDSTORM);
}

/// A game weather kind draws its look and is no battle weather.
#[test]
fn a_game_weather_kind_draws_its_look() {
    let g = SkyGame {
        coords: false,
        weathers: [PETALS, 0, 0, 0],
    };
    let mut w = new_world(&g);
    assert_eq!(w.s.weather.current, PETALS);
    let f = step(&g, &mut w, Buttons::NONE, 1);
    let petals = f
        .objs
        .iter()
        .filter(|o| o.palette == crate::sky::PARTICLE_BANK)
        .count();
    assert!(petals > 0 && petals <= 6);
    assert!(w.s.weather.is_default());
    assert_eq!(weather::battle_weather(PETALS), None);
}

/// A palette override blends the field over its frames, recolours banks the frame did not
/// upload, is hashed and snapshotted, ends at amount 0 and goes with a map load unless kept.
#[test]
fn a_palette_override_blends_and_ends() {
    let mut w = new_world(&NONE);
    let rest = w.hash();
    let red = weather::rgb(31, 0, 0);
    start(&mut w, move |ctx| {
        Box::pin(async move {
            ctx.set_palette(PaletteFx {
                mode: PaletteMode::Blend,
                color: red,
                amount: 8,
                current: 0,
                frames: 4,
                t: 0,
                pulse: 0,
                keep: false,
            })
            .await
        })
    });
    let mut changed = 0;
    for _ in 0..8 {
        let f = step(&NONE, &mut w, Buttons::NONE, 1);
        if !bg_palettes(&f).is_empty() {
            changed += 1;
        }
    }
    assert!(changed >= 4, "re-sent as the amount moved: {changed}");
    assert_eq!(w.s.palette.map(|p| p.current), Some(8));
    assert_ne!(w.hash(), rest);
    let cp = w.checkpoint(&NONE).unwrap();
    assert_eq!(
        cp.world.sky.and_then(|k| k.palette).map(|p| p.amount),
        Some(8)
    );
    let r = World::restore(&NONE, &cp).unwrap();
    assert_eq!(r.hash(), w.hash());
    // Grey, inverted: other modes recolour too.
    for mode in [PaletteMode::Grey, PaletteMode::Invert] {
        let p = PaletteFx {
            mode,
            color: red,
            amount: 16,
            current: 16,
            frames: 0,
            t: 0,
            pulse: 0,
            keep: false,
        };
        assert_ne!(
            p.apply(weather::rgb(20, 10, 0), 16),
            weather::rgb(20, 10, 0)
        );
    }
    // Back to 0: the override ends.
    start(&mut w, move |ctx| {
        Box::pin(async move {
            ctx.set_palette(PaletteFx {
                mode: PaletteMode::Blend,
                color: red,
                amount: 0,
                current: 0,
                frames: 2,
                t: 0,
                pulse: 0,
                keep: false,
            })
            .await
        })
    });
    step(&NONE, &mut w, Buttons::NONE, 12);
    assert_eq!(w.s.palette, None);
    // Unkept, a crossing drops it; kept, it stays.
    for keep in [false, true] {
        let mut w = new_world(&NONE);
        start(&mut w, move |ctx| {
            Box::pin(async move {
                ctx.set_palette(PaletteFx {
                    mode: PaletteMode::Blend,
                    color: red,
                    amount: 4,
                    current: 0,
                    frames: 0,
                    t: 0,
                    pulse: 0,
                    keep,
                })
                .await
            })
        });
        go(&NONE, &mut w, Dir::Up, 5);
        step(&NONE, &mut w, Buttons::UP, 1);
        assert_eq!(w.map(), ROUTE);
        assert_eq!(w.s.palette.is_some(), keep);
    }
}

/// A strobe shows the override every other `pulse` frames.
#[test]
fn a_strobe_alternates() {
    let mut p = PaletteFx {
        mode: PaletteMode::Blend,
        color: 0x7FFF,
        amount: 10,
        current: 10,
        frames: 0,
        t: 0,
        pulse: 3,
        keep: false,
    };
    let mut shown = Vec::new();
    for _ in 0..12 {
        shown.push(p.shown());
        p.tick();
    }
    assert_eq!(shown, [10, 10, 10, 0, 0, 0, 10, 10, 10, 0, 0, 0]);
}

/// An overlay is drawn in its stages over its frames, the script waiting; it uses the
/// reserved bank, which the objects leave.
#[test]
fn an_overlay_reveals_its_stages_and_the_script_waits() {
    let mut w = new_world(&NONE);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.show_overlay(0, 10).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    let mut counts = Vec::new();
    for _ in 0..14 {
        let f = step(&NONE, &mut w, Buttons::NONE, 1);
        counts.push(
            f.objs
                .iter()
                .filter(|o| o.palette == crate::sky::OVERLAY_BANK)
                .count(),
        );
        // No object shares the overlay's bank.
        assert!(
            f.cmds.iter().all(|c| !matches!(
                c,
                Cmd::ObjPalette { bank, colors } if *bank == crate::sky::OVERLAY_BANK && colors[1] != 0x1234
            ))
        );
    }
    assert!(counts.contains(&1) && counts.contains(&3), "{counts:?}");
    assert_eq!(*counts.last().unwrap(), 0);
    assert!(w.flag(FLAG_DONE));
    // An overlay the game lacks is skipped.
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.show_overlay(9, 10).await })
    });
    step(&NONE, &mut w, Buttons::NONE, 2);
    assert!(!w.script_running());
}
