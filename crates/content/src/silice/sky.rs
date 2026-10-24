//! Silice's skies (`contenu-jeu/bible/besoins-moteur.md` P1: `set_weather`, `set_palette`,
//! `set_time_tint`, `set_sky_alert`, `sky_crack_animation`), on the engine's weather and palette
//! overrides (`engine::weather`, `engine::sky`).
//!
//! - **Palettes** ([`Palette`]): the source's named palettes as blends of the field's colours
//!   (the source gives names, not colours: the colours and amounts are the port's, see the plan's
//!   author questions). A map's palette is set by its on-transition script from the story (the
//!   override goes with every map load), a scene's gradually; [`ELYSIUM_GOLD`] over `ALL` maps is
//!   kept across loads.
//! - **Weathers:** Gen3's map weathers, plus [`WEATHER_PETALS`] (Chapter 6's falling petals, a
//!   game weather kind: pink particles, no battle weather), whose look is [`PETALS_LOOK`].
//! - **The sky's crack** ([`SKY_CRACK`]): the Zenith's fake sky cracking in four stages over the
//!   screen's top (Chapter 11 `Trone_Zenith_Sc09`), drawn from [`CRACK_TILES`] (the port's own
//!   8x8 strokes, not a graphics asset).

use engine::Tile;
use engine::script::Ctx;
use engine::sky::{Overlay, OverlayCell, PaletteFx, PaletteMode};
use engine::video::ObjShape;
use engine::weather::{GAME_FIRST, Look, Particles, rgb};

/// A named palette of the source: how the field's colours change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Palette {
    pub mode: PaletteMode,
    /// BGR555.
    pub color: u16,
    /// 0..=16.
    pub amount: u8,
    /// A strobe's half period in frames (0: steady).
    pub pulse: u8,
}

const fn blend(color: u16, amount: u8) -> Palette {
    Palette {
        mode: PaletteMode::Blend,
        color,
        amount,
        pulse: 0,
    }
}

/// `set_time_tint(NUIT)` (Ch0: Brico-Bourg at night, `VAR_SIL_STORY` ≤ 2): a dark blue.
pub const NUIT: Palette = blend(rgb(2, 3, 12), 7);
/// `set_sky_alert(RED)` (Ch1 `Bois_Ether_Sc06`–`Sc07`): the sky turned red.
pub const SKY_ALERT_RED: Palette = blend(rgb(31, 3, 2), 5);
/// The Brico-Bourg Labo's strobes (Ch1 §2: a night club): white flashes, 4 frames on, 4 off.
pub const STROBES: Palette = Palette {
    pulse: 4,
    ..blend(rgb(31, 31, 31), 5)
};
/// The dawn tint (Ch6 §2: the Secteur Zêta's sky set to dawn).
pub const AUBE: Palette = blend(rgb(31, 17, 13), 4);
/// `ELYSIUM_GOLD` (Ch11: the Apex after `Sc05`, every map in the alternate ending).
pub const ELYSIUM_GOLD: Palette = blend(rgb(31, 24, 5), 6);
/// `NUIT_VRAIE` (Ch11 `Trone_Zenith_Sc09`: the true night after the rupture).
pub const NUIT_VRAIE: Palette = blend(rgb(1, 2, 9), 8);
/// `set_palette_set(DETRUITE)` (Ch12: Nébula, the Tower's ruins and the Breach from 127 to
/// 129): faded towards a rusty grey.
pub const DETRUITE: Palette = Palette {
    mode: PaletteMode::Grey,
    color: rgb(14, 7, 4),
    amount: 11,
    pulse: 0,
};
/// `set_time_of_day(DUSK)` (Ch12 `Brico_Bourg_PG_Sc20`).
pub const CREPUSCULE: Palette = blend(rgb(28, 11, 6), 6);
/// `screen_flash(NEGATIF)` (Ch7): the colours inverted.
pub const NEGATIF: Palette = Palette {
    mode: PaletteMode::Invert,
    color: 0,
    amount: 16,
    pulse: 0,
};

/// A scene's change of palette: frames it takes.
pub const PALETTE_FRAMES: u16 = 30;

/// `set_palette(p)`: the field towards `p` over `frames` (0: at once); `keep` keeps it across
/// map loads (`set_palette(ALL, …)`), else the next map load ends it.
pub async fn set_palette(ctx: &Ctx, p: Palette, frames: u16, keep: bool) {
    ctx.set_palette(PaletteFx {
        mode: p.mode,
        color: p.color,
        amount: p.amount,
        current: 0,
        frames,
        t: 0,
        pulse: p.pulse,
        keep,
    })
    .await;
}

/// The map's own palette back over `frames` (`clear_sky_alert`, `set_palette_set(ORIGINALE)`).
pub async fn clear_palette(ctx: &Ctx, frames: u16) {
    ctx.set_palette(PaletteFx {
        mode: PaletteMode::Blend,
        color: 0,
        amount: 0,
        current: 0,
        frames,
        t: 0,
        pulse: 0,
        keep: false,
    })
    .await;
}

/// The source's `screen_flash(NEGATIF)`: the colours inverted for `frames`, then back.
pub async fn negatif(ctx: &Ctx, frames: u16) {
    set_palette(ctx, NEGATIF, 0, false).await;
    ctx.delay(frames).await;
    clear_palette(ctx, 0).await;
}

/// The source's `set_weather(W)` in a scene: Gen3's `setweather` then `doweather` (the new
/// weather comes in gradually). The map's header weather is back on the next load unless its
/// on-transition script sets it again (`setweather` alone: a load shows it at once).
pub async fn set_weather(ctx: &Ctx, w: u8) {
    ctx.set_weather(u16::from(w)).await;
    ctx.do_weather().await;
}

/// `WEATHER_PETALS` (Ch6: the Bois des Soupirs and the gym): pink petals falling and swaying.
pub const WEATHER_PETALS: u8 = GAME_FIRST;

/// A pixel row of an 8x8 tile: colour `c` at columns `xs` (4bpp, the low nibble leftmost).
const fn row(c: u32, xs: &[u32]) -> u32 {
    let mut r = 0;
    let mut i = 0;
    while i < xs.len() {
        r |= c << (4 * xs[i]);
        i += 1;
    }
    r
}

/// A petal: a small rounded blot, light pink with a deeper edge.
static PETAL_TILES: [Tile; 1] = [[
    0,
    row(2, &[2, 3]),
    row(2, &[1]) | row(1, &[2, 3]) | row(2, &[4]),
    row(2, &[1]) | row(1, &[2, 3, 4]),
    row(2, &[2]) | row(1, &[3]) | row(2, &[4]),
    row(2, &[3]),
    0,
    0,
]];

/// [`WEATHER_PETALS`]'s look: 14 petals drifting down and right with a sway, a faint pink blend
/// (the source's own fallback is Gen3's ash recoloured pink).
pub static PETALS_LOOK: Look = Look {
    blend: Some((rgb(31, 22, 26), 1)),
    particles: Some(Particles {
        tiles: &PETAL_TILES,
        shape: ObjShape::Square,
        size: 0,
        palette: {
            let mut p = [0u16; 16];
            p[1] = rgb(31, 24, 27);
            p[2] = rgb(28, 14, 20);
            p
        },
        count: 14,
        dx: 3,
        dy: 6,
        sway: 6,
    }),
    pulse: false,
};

/// The game's weather looks (`GameData::weather_look`).
pub fn weather_look(id: u8) -> Option<&'static Look> {
    (id == WEATHER_PETALS).then_some(&PETALS_LOOK)
}

/// `Request::ShowOverlay` id of the sky's crack.
pub const SKY_CRACK: u8 = 0;
/// How long the crack runs (`sky_crack_animation`: "3–4 images").
pub const SKY_CRACK_FRAMES: u16 = 96;

/// The crack's strokes: `\`, `/`, `-`, `|`, and a falling shard.
pub static CRACK_TILES: [Tile; 5] = [
    [
        row(1, &[0]) | row(2, &[1]),
        row(1, &[1]) | row(2, &[2]),
        row(1, &[2]) | row(2, &[3]),
        row(1, &[3]) | row(2, &[4]),
        row(1, &[4]) | row(2, &[5]),
        row(1, &[5]) | row(2, &[6]),
        row(1, &[6]) | row(2, &[7]),
        row(1, &[7]),
    ],
    [
        row(1, &[7]) | row(2, &[6]),
        row(1, &[6]) | row(2, &[5]),
        row(1, &[5]) | row(2, &[4]),
        row(1, &[4]) | row(2, &[3]),
        row(1, &[3]) | row(2, &[2]),
        row(1, &[2]) | row(2, &[1]),
        row(1, &[1]) | row(2, &[0]),
        row(1, &[0]),
    ],
    [
        0,
        0,
        0,
        row(1, &[0, 1, 2, 3, 4, 5, 6, 7]),
        row(2, &[0, 1, 2, 3, 4, 5, 6, 7]),
        0,
        0,
        0,
    ],
    [
        row(1, &[3]) | row(2, &[4]),
        row(1, &[3]) | row(2, &[4]),
        row(1, &[4]) | row(2, &[5]),
        row(1, &[4]) | row(2, &[5]),
        row(1, &[3]) | row(2, &[4]),
        row(1, &[3]) | row(2, &[4]),
        row(1, &[4]) | row(2, &[5]),
        row(1, &[4]) | row(2, &[5]),
    ],
    [
        0,
        row(3, &[3]),
        row(3, &[2, 3, 4]),
        row(3, &[1, 2, 3]) | row(1, &[4]),
        row(3, &[2, 3]) | row(1, &[4, 5]),
        row(1, &[3, 4]),
        0,
        0,
    ],
];

const fn cell(x: u8, y: u8, tile: u8, stage: u8) -> OverlayCell {
    OverlayCell { x, y, tile, stage }
}

/// The crack across the screen's top: from the centre (stage 0) out to both sides (1, 2), then
/// shards falling loose (3).
static CRACK_CELLS: [OverlayCell; 30] = [
    // Stage 0: the first split, above the throne.
    cell(14, 2, 3, 0),
    cell(14, 3, 1, 0),
    cell(13, 4, 1, 0),
    cell(15, 1, 0, 0),
    // Stage 1: west and east.
    cell(12, 5, 2, 1),
    cell(11, 5, 0, 1),
    cell(16, 2, 0, 1),
    cell(17, 3, 2, 1),
    cell(18, 3, 2, 1),
    cell(10, 4, 1, 1),
    cell(15, 0, 3, 1),
    // Stage 2: to the edges.
    cell(9, 3, 1, 2),
    cell(8, 3, 2, 2),
    cell(7, 2, 1, 2),
    cell(6, 2, 2, 2),
    cell(5, 1, 1, 2),
    cell(19, 4, 0, 2),
    cell(20, 5, 0, 2),
    cell(21, 5, 2, 2),
    cell(22, 4, 1, 2),
    cell(23, 3, 1, 2),
    cell(24, 3, 2, 2),
    cell(11, 6, 3, 2),
    // Stage 3: shards.
    cell(13, 6, 4, 3),
    cell(16, 5, 4, 3),
    cell(8, 5, 4, 3),
    cell(21, 7, 4, 3),
    cell(6, 4, 4, 3),
    cell(18, 6, 4, 3),
    cell(11, 8, 4, 3),
];

/// The sky's crack.
pub static SKY_CRACK_OVERLAY: Overlay = Overlay {
    tiles: &CRACK_TILES,
    palette: {
        let mut p = [0u16; 16];
        p[1] = rgb(31, 31, 31);
        p[2] = rgb(12, 16, 24);
        p[3] = rgb(26, 28, 31);
        p
    },
    cells: &CRACK_CELLS,
    stages: 4,
};

/// The game's overlays (`GameData::overlay`).
pub fn overlay(id: u8) -> Option<&'static Overlay> {
    (id == SKY_CRACK).then_some(&SKY_CRACK_OVERLAY)
}
