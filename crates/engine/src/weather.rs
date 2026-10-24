//! Field weather (Gen3 `field_weather.c`, `field_weather_effect.c`, `coord_event_weather.c`).
//!
//! **State** ([`Weather`], hashed and snapshotted while it differs from the map's settled
//! default, [`Weather::is_settled`]):
//! - `saved` is Gen3 `gSaveBlock1Ptr->weather`: every map load sets it from the map header
//!   (`SetSavedWeatherFromCurrMapHeader`, through [`translate`]), before `on_transition`;
//!   `setweather` sets it ([`Weather::set_saved`]), `resetweather` takes the header's again.
//! - `current` / `next` are `gWeatherPtr->currWeather` / `nextWeather`. A warp's load and the
//!   return from a battle or a full screen switch at once (`ResumePausedWeather`,
//!   [`Weather::resume`]); a connection crossing, `doweather` and a weather coord event change
//!   gradually (`DoCurrentWeather`, `SetNextWeather`: the old weather's `Finish`, then the new
//!   one's `InitVars`, [`Weather::set_next`]).
//! - the colour map (`colorMapIndex`, `targetColorMapIndex`, `colorMapStepDelay`): 3 darkens
//!   (rain, thunderstorm, downpour, snow, shade), 19 is the lightning, negative is the drought;
//!   it steps one index every `colorMapStepDelay` frames (`UpdateWeatherColorMap`).
//! - the thunderstorm's and the downpour's lightning cycle (`Thunderstorm_Main`), with Gen3's
//!   `Random()` calls in Gen3's order; the abnormal weather's alternation
//!   (`Task_DoAbnormalWeather`: downpour and drought every 600 frames).
//!
//! **Degraded** (the frames before a weather's first lightning cycle, `Finish` lengths): Gen3
//! creates its rain sprites one a frame and shows one every `rainSpriteVisibleDelay + 1`
//! frames before the cycle starts; [`init_frames`] counts those frames instead of the sprites.
//! The thunder sound's `Random()` (`UpdateThunderSound`, `IsSEPlaying` false: no audio) is
//! kept. `WEATHER_ROUTE119_CYCLE` / `ROUTE123_CYCLE` read `cycle_stage`, which the clock's
//! daily reset advances ([`Weather::pass_days`], Gen3 `UpdateWeatherPerDay`).
//!
//! **Looks** ([`Look`]): what the frame shows for each weather, applied by `crate::sky` after
//! the frame is built: the colour map on the field's palettes, a blend, and particles (OBJs).
//! Kinds from [`GAME_FIRST`] up are the game's ([`crate::data::GameData::weather_look`], e.g.
//! Silice's petals); they behave as `WEATHER_NONE` otherwise (no colour map, no battle
//! weather).

use crate::hash::Fnv64;
use crate::rng::Rng;
use crate::video::{ObjShape, Tile};
use alloc::boxed::Box;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

// Gen3 `WEATHER_*` (`include/constants/weather.h`).
pub const NONE: u8 = 0;
pub const SUNNY_CLOUDS: u8 = 1;
pub const SUNNY: u8 = 2;
pub const RAIN: u8 = 3;
pub const SNOW: u8 = 4;
pub const RAIN_THUNDERSTORM: u8 = 5;
pub const FOG_HORIZONTAL: u8 = 6;
pub const VOLCANIC_ASH: u8 = 7;
pub const SANDSTORM: u8 = 8;
pub const FOG_DIAGONAL: u8 = 9;
pub const UNDERWATER: u8 = 10;
pub const SHADE: u8 = 11;
pub const DROUGHT: u8 = 12;
pub const DOWNPOUR: u8 = 13;
pub const UNDERWATER_BUBBLES: u8 = 14;
pub const ABNORMAL: u8 = 15;
pub const ROUTE119_CYCLE: u8 = 20;
pub const ROUTE123_CYCLE: u8 = 21;
/// The first game-defined weather ([`crate::data::GameData::weather_look`]).
pub const GAME_FIRST: u8 = 0x40;

/// Gen3 `sWeatherCycleRoute119` / `sWeatherCycleRoute123`.
const CYCLE_119: [u8; 4] = [SUNNY, RAIN, RAIN_THUNDERSTORM, RAIN];
const CYCLE_123: [u8; 4] = [SUNNY, SUNNY, RAIN, SUNNY];

/// Gen3 `TranslateWeatherNum`: the cycles by `stage`, unknown values to `WEATHER_NONE`. A
/// game-defined kind ([`GAME_FIRST`] up) is kept.
pub fn translate(w: u8, stage: u8) -> u8 {
    match w {
        NONE..=ABNORMAL => w,
        ROUTE119_CYCLE => CYCLE_119[stage as usize % 4],
        ROUTE123_CYCLE => CYCLE_123[stage as usize % 4],
        w if w >= GAME_FIRST => w,
        _ => NONE,
    }
}

/// Gen3 `sCoordEventWeatherFuncs`: a weather coord event's `COORD_EVENT_WEATHER_*` to the
/// weather it sets (`SetWeather`).
pub fn from_coord(c: u8) -> Option<u8> {
    Some(match c {
        1 => SUNNY_CLOUDS,
        2 => SUNNY,
        3 => RAIN,
        4 => SNOW,
        5 => RAIN_THUNDERSTORM,
        6 => FOG_HORIZONTAL,
        7 => FOG_DIAGONAL,
        8 => VOLCANIC_ASH,
        9 => SANDSTORM,
        10 => SHADE,
        11 => DROUGHT,
        20 => ROUTE119_CYCLE,
        21 => ROUTE123_CYCLE,
        _ => return None,
    })
}

/// The battle weather a field weather starts (Gen3 `ABILITYEFFECT_SWITCH_IN_WEATHER`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BattleWeather {
    Rain,
    Sandstorm,
    Sun,
}

/// Gen3 `ABILITYEFFECT_SWITCH_IN_WEATHER` on `GetCurrentWeather()`.
pub fn battle_weather(current: u8) -> Option<BattleWeather> {
    match current {
        RAIN | RAIN_THUNDERSTORM | DOWNPOUR => Some(BattleWeather::Rain),
        SANDSTORM => Some(BattleWeather::Sandstorm),
        DROUGHT => Some(BattleWeather::Sun),
        _ => None,
    }
}

fn rainy(w: u8) -> bool {
    matches!(w, RAIN | RAIN_THUNDERSTORM | DOWNPOUR)
}

fn stormy(w: u8) -> bool {
    matches!(w, RAIN_THUNDERSTORM | DOWNPOUR)
}

/// Gen3 `*_InitVars`: `(targetColorMapIndex, colorMapStepDelay)`.
fn init_vars(w: u8) -> (i8, u8) {
    match w {
        RAIN | SNOW | RAIN_THUNDERSTORM | DOWNPOUR | SHADE => (3, 20),
        // `Drought_InitVars` targets 0 and `DroughtStateRun` brightens to -6 (below).
        DROUGHT => (-6, 4),
        NONE => (0, 0),
        _ => (0, 20),
    }
}

/// Frames a weather spends loading before its main loop (degraded, see the module docs): rain
/// sprites created one a frame (`CreateRainSprite`, 24) and shown one every
/// `rainSpriteVisibleDelay + 1` frames up to `targetRainSpriteCount`.
pub fn init_frames(w: u8) -> u16 {
    match w {
        RAIN => 1 + 24 + 9 * 10,
        RAIN_THUNDERSTORM => 1 + 24 + 5 * 16,
        DOWNPOUR => 1 + 24 + 5 * 24,
        _ => 0,
    }
}

// Gen3 `THUNDER_STATE_*`.
const T_LOADING: u8 = 0;
const T_WAIT_CHANGE: u8 = 3;
const T_NEW_CYCLE: u8 = 4;
const T_NEW_CYCLE_WAIT: u8 = 5;
const T_INIT_CYCLE_1: u8 = 6;
const T_INIT_CYCLE_2: u8 = 7;
const T_SHORT_BOLT: u8 = 8;
const T_TRY_NEW_BOLT: u8 = 9;
const T_WAIT_BOLT_SHORT: u8 = 10;
const T_INIT_BOLT_LONG: u8 = 11;
const T_WAIT_BOLT_LONG: u8 = 12;
const T_FADE_BOLT_LONG: u8 = 13;
const T_END_BOLT_LONG: u8 = 14;

/// The lightning's colour map (`ApplyWeatherColorMapIfIdle(19)`).
pub const LIGHTNING: i8 = 19;

/// The thunderstorm's lightning cycle (`gWeatherPtr->thunder*`, `initStep`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Thunder {
    pub state: u8,
    pub timer: u16,
    pub long_bolt: bool,
    pub short_bolts: u8,
    pub allow_end: bool,
    pub enqueued: bool,
    pub se_timer: u16,
}

/// `Task_DoAbnormalWeather`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Abnormal {
    pub state: u8,
    pub delay: i16,
    pub a: u8,
    pub b: u8,
}

/// The field weather (see the module docs).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Weather {
    /// `gSaveBlock1Ptr->weather`.
    pub saved: u8,
    /// `gSaveBlock1Ptr->weatherCycleStage`.
    pub cycle_stage: u8,
    pub current: u8,
    pub next: u8,
    pub color_map: i8,
    pub target: i8,
    pub step_delay: u8,
    pub step_counter: u8,
    /// Frames of the current weather's loading left ([`init_frames`]).
    pub init: u16,
    /// `finishStep` of a weather that is ending (thunder only waits for its cycle's end).
    pub finish: u8,
    pub thunder: Thunder,
    pub abnormal: Option<Abnormal>,
    /// `sCurrentAbnormalWeather`.
    pub abnormal_current: u8,
    /// The map header's weather, translated: derived at each load and restore.
    #[serde(skip)]
    pub map_default: u8,
    /// Frames the current weather has been drawn (particles' phase; display only).
    #[serde(skip)]
    pub t: u16,
}

impl Default for Weather {
    fn default() -> Self {
        Weather {
            saved: NONE,
            cycle_stage: 0,
            current: NONE,
            next: NONE,
            color_map: 0,
            target: 0,
            step_delay: 0,
            step_counter: 0,
            init: 0,
            finish: 0,
            thunder: Thunder::default(),
            abnormal: None,
            abnormal_current: DOWNPOUR,
            map_default: NONE,
            t: 0,
        }
    }
}

impl Weather {
    /// The settled state of a map whose header says `header` (a warp's load with no script
    /// weather): [`Weather::resume`] of it.
    pub fn settled(header: u8, cycle_stage: u8) -> Self {
        let mut w = Weather {
            cycle_stage,
            ..Weather::default()
        };
        w.load_header(header);
        w.resume();
        w
    }

    /// Whether the state is the map's settled default: [`Weather::settled`] of its header, up
    /// to the display-only fields. Hashing and snapshots skip it then, so maps without weather
    /// (and maps resting in theirs) keep their hashes.
    pub fn is_settled(&self) -> bool {
        let mut d = Weather::settled(self.map_default, self.cycle_stage);
        d.t = self.t;
        d.map_default = self.map_default;
        *self == d
    }

    /// Whether a snapshot or the hash needs the state: not settled, or a weather cycle stage
    /// moved by the days.
    pub fn is_default(&self) -> bool {
        self.cycle_stage == 0 && self.is_settled()
    }

    /// A restored state on a map whose header says `header` (`map_default` is not saved).
    pub fn restored(mut self, header: u8) -> Self {
        self.map_default = translate(header, self.cycle_stage);
        self
    }

    /// Gen3 `UpdateWeatherPerDay`: the cycle moves on by the days passed.
    pub fn pass_days(&mut self, days: u16) {
        self.cycle_stage = ((u16::from(self.cycle_stage) + days % 4) % 4) as u8;
    }

    /// Feeds the state into `h` unless it is the default ([`Weather::is_default`]).
    pub fn hash(&self, h: &mut Fnv64) {
        if self.is_default() {
            return;
        }
        h.write(&[
            0xB8,
            self.saved,
            self.cycle_stage,
            self.current,
            self.next,
            self.color_map as u8,
            self.target as u8,
            self.step_delay,
            self.step_counter,
            self.finish,
            self.abnormal_current,
        ]);
        h.write_u16(self.init);
        let t = self.thunder;
        h.write(&[
            t.state,
            t.long_bolt as u8,
            t.short_bolts,
            t.allow_end as u8,
            t.enqueued as u8,
        ]);
        h.write_u16(t.timer);
        h.write_u16(t.se_timer);
        match self.abnormal {
            None => h.write(&[0]),
            Some(a) => {
                h.write(&[1, a.state, a.a, a.b]);
                h.write_u16(a.delay as u16);
            }
        }
    }

    /// `SetSavedWeatherFromCurrMapHeader` for a map whose header weather is `header` (every
    /// map load, before `on_transition`); also notes it as the map's default.
    pub fn load_header(&mut self, header: u8) {
        self.map_default = translate(header, self.cycle_stage);
        self.saved = self.map_default;
    }

    /// `setweather` (`SetSavedWeather`).
    pub fn set_saved(&mut self, w: u8) {
        self.saved = translate(w, self.cycle_stage);
    }

    /// `resetweather` (`SetSavedWeatherFromCurrMapHeader`).
    pub fn reset_saved(&mut self) {
        self.saved = self.map_default;
    }

    /// The weather `saved` asks for: the abnormal weather's current half
    /// (`sCurrentAbnormalWeather`) for `WEATHER_ABNORMAL`.
    fn effective(&mut self) -> u8 {
        if self.saved == ABNORMAL {
            if self.abnormal.is_none() {
                self.start_abnormal();
            }
            self.abnormal_current
        } else {
            self.abnormal = None;
            self.abnormal_current = DOWNPOUR;
            self.saved
        }
    }

    /// `CreateAbnormalWeatherTask`.
    fn start_abnormal(&mut self) {
        let (a, b) = if self.abnormal_current == DROUGHT {
            (DOWNPOUR, DROUGHT)
        } else {
            self.abnormal_current = DOWNPOUR;
            (DROUGHT, DOWNPOUR)
        };
        self.abnormal = Some(Abnormal {
            state: 0,
            delay: 600,
            a,
            b,
        });
    }

    /// `doweather` and a connection crossing (`DoCurrentWeather`): the saved weather comes in
    /// gradually.
    pub fn do_current(&mut self) {
        let w = self.effective();
        self.set_next(w);
    }

    /// A weather coord event or `SetWeather`: saves `w`, then [`Weather::do_current`] without
    /// the abnormal check (Gen3 `SetWeather`: `SetNextWeather(GetSavedWeather())`).
    pub fn set_weather(&mut self, w: u8) {
        self.set_saved(w);
        let w = self.saved;
        self.set_next(w);
    }

    /// `ResumePausedWeather` (a warp's load, the field back from a battle or a full screen):
    /// the saved weather at once, its loading done (`InitAll`), its colour map reached.
    pub fn resume(&mut self) {
        let w = self.effective();
        self.current = w;
        self.next = w;
        self.init_vars();
        self.init = 0;
        self.color_map = self.target;
        self.step_counter = 0;
        if stormy(w) {
            // `Thunderstorm_InitAll` runs the loading states; the cycle starts next frame.
            self.thunder.state = T_WAIT_CHANGE;
        }
    }

    /// `SetNextWeather`.
    pub fn set_next(&mut self, w: u8) {
        if self.next != w && self.current == w {
            self.init_vars();
        }
        self.next = w;
        self.finish = 0;
    }

    /// `sWeatherFuncs[currWeather].initVars()`.
    fn init_vars(&mut self) {
        let (target, delay) = init_vars(self.current);
        self.target = target;
        self.step_delay = delay;
        self.init = init_frames(self.current);
        self.t = 0;
        self.thunder = Thunder {
            state: T_LOADING,
            ..Thunder::default()
        };
    }

    /// One frame of `Task_WeatherMain` (the field's frames: never during a battle or a full
    /// screen).
    pub fn tick(&mut self, rng: &mut Rng) {
        self.tick_abnormal();
        if self.current != self.next {
            if !self.finish_step(rng) {
                self.current = self.next;
                self.init_vars();
                self.step_counter = 0;
            }
        } else {
            self.main(rng);
        }
        self.update_color_map();
        self.t = self.t.wrapping_add(1);
    }

    /// `Task_DoAbnormalWeather`.
    fn tick_abnormal(&mut self) {
        let Some(mut a) = self.abnormal else { return };
        let fire = a.delay <= 0;
        a.delay -= 1;
        if fire {
            let w = if a.state == 0 { a.a } else { a.b };
            a.state ^= 1;
            a.delay = 600;
            self.abnormal = Some(a);
            self.abnormal_current = w;
            self.set_next(w);
        } else {
            self.abnormal = Some(a);
        }
    }

    /// `sWeatherFuncs[currWeather].finish()`: whether it is still finishing.
    fn finish_step(&mut self, rng: &mut Rng) -> bool {
        if !stormy(self.current) {
            return false;
        }
        // `Thunderstorm_Finish`: the cycle runs until it may end.
        match self.finish {
            0 => {
                self.thunder.allow_end = false;
                self.finish = 1;
                self.finish_step(rng)
            }
            1 => {
                self.thunder_main(rng);
                if self.thunder.allow_end {
                    if rainy(self.next) {
                        return false;
                    }
                    self.finish = 2;
                }
                true
            }
            _ => {
                self.thunder.enqueued = false;
                false
            }
        }
    }

    fn main(&mut self, rng: &mut Rng) {
        if self.init > 0 {
            self.init -= 1;
            if self.init == 0 && stormy(self.current) {
                self.thunder.state = T_WAIT_CHANGE;
            }
            return;
        }
        if stormy(self.current) {
            self.thunder_main(rng);
        }
    }

    /// Whether the palettes are idle (`palProcessingState == WEATHER_PAL_STATE_IDLE`).
    fn pal_idle(&self) -> bool {
        self.color_map == self.target
    }

    /// `ApplyWeatherColorMapIfIdle`.
    fn flash(&mut self, index: i8) {
        if self.pal_idle() {
            self.color_map = index;
            self.target = index;
        }
    }

    /// `Thunderstorm_Main`, from its cycle (the loading states are [`Weather::init`]).
    fn thunder_main(&mut self, rng: &mut Rng) {
        self.thunder_sound(rng);
        let t = &mut self.thunder;
        match t.state {
            T_WAIT_CHANGE => {
                if self.color_map == self.target {
                    self.thunder.state = T_INIT_CYCLE_1;
                }
            }
            T_NEW_CYCLE | T_NEW_CYCLE_WAIT => {
                if t.state == T_NEW_CYCLE {
                    t.allow_end = true;
                    t.timer = rng.random() % 360 + 360;
                    t.state = T_NEW_CYCLE_WAIT;
                }
                t.timer -= 1;
                if t.timer == 0 {
                    t.state = T_INIT_CYCLE_1;
                }
            }
            T_INIT_CYCLE_1 => {
                t.allow_end = true;
                t.long_bolt = rng.random() % 2 != 0;
                t.state = T_INIT_CYCLE_2;
            }
            T_INIT_CYCLE_2 | T_SHORT_BOLT => {
                if t.state == T_INIT_CYCLE_2 {
                    t.short_bolts = (rng.random() & 1) as u8 + 1;
                }
                let last = !t.long_bolt && t.short_bolts == 1;
                if last {
                    self.enqueue_thunder(rng, 20);
                }
                self.flash(LIGHTNING);
                let t = &mut self.thunder;
                t.timer = rng.random() % 3 + 6;
                t.state = T_TRY_NEW_BOLT;
            }
            T_TRY_NEW_BOLT => {
                t.timer -= 1;
                if t.timer == 0 {
                    self.flash(3);
                    let t = &mut self.thunder;
                    t.allow_end = true;
                    t.short_bolts = t.short_bolts.saturating_sub(1);
                    if t.short_bolts != 0 {
                        t.timer = rng.random() % 16 + 60;
                        t.state = T_WAIT_BOLT_SHORT;
                    } else if !t.long_bolt {
                        t.state = T_NEW_CYCLE;
                    } else {
                        t.state = T_INIT_BOLT_LONG;
                    }
                }
            }
            T_WAIT_BOLT_SHORT => {
                t.timer -= 1;
                if t.timer == 0 {
                    t.state = T_SHORT_BOLT;
                }
            }
            T_INIT_BOLT_LONG => {
                t.timer = rng.random() % 16 + 60;
                t.state = T_WAIT_BOLT_LONG;
            }
            T_WAIT_BOLT_LONG => {
                t.timer -= 1;
                if t.timer == 0 {
                    self.enqueue_thunder(rng, 100);
                    self.flash(LIGHTNING);
                    let t = &mut self.thunder;
                    t.timer = (rng.random() & 0xF) + 30;
                    t.state = T_FADE_BOLT_LONG;
                }
            }
            T_FADE_BOLT_LONG => {
                t.timer -= 1;
                if t.timer == 0 {
                    // `ApplyWeatherColorMapIfIdle_Gradual(19, 3, 5)`.
                    if self.pal_idle() {
                        self.color_map = LIGHTNING;
                        self.target = 3;
                        self.step_delay = 5;
                        self.step_counter = 0;
                    }
                    self.thunder.state = T_END_BOLT_LONG;
                }
            }
            T_END_BOLT_LONG => {
                if self.pal_idle() {
                    self.thunder.allow_end = true;
                    self.thunder.state = T_NEW_CYCLE;
                }
            }
            _ => {}
        }
    }

    /// `EnqueueThunder`.
    fn enqueue_thunder(&mut self, rng: &mut Rng, wait: u16) {
        if !self.thunder.enqueued {
            self.thunder.se_timer = rng.random() % wait;
            self.thunder.enqueued = true;
        }
    }

    /// `UpdateThunderSound` (no sound plays: `IsSEPlaying` is false; the SE's draw is kept).
    fn thunder_sound(&mut self, rng: &mut Rng) {
        let t = &mut self.thunder;
        if t.enqueued {
            if t.se_timer == 0 {
                let _ = rng.random();
                t.enqueued = false;
            } else {
                t.se_timer -= 1;
            }
        }
    }

    /// `UpdateWeatherColorMap`: one index every `step_delay` frames towards the target.
    fn update_color_map(&mut self) {
        if self.color_map == self.target {
            self.step_counter = 0;
            return;
        }
        self.step_counter += 1;
        if self.step_counter >= self.step_delay {
            self.step_counter = 0;
            self.color_map += if self.color_map < self.target { 1 } else { -1 };
        }
    }
}

// ---- looks ----

/// Particles drawn over the field (degraded: Gen3 draws each weather with its own sprites,
/// blends and BG scrolling; these are moving OBJs of one tile set).
#[derive(Clone, Copy, Debug)]
pub struct Particles {
    /// One sprite's tiles (at most [`PARTICLE_TILES`]).
    pub tiles: &'static [Tile],
    pub shape: ObjShape,
    pub size: u8,
    pub palette: [u16; 16],
    pub count: u8,
    /// Pixels a frame, in eighths.
    pub dx: i8,
    pub dy: i8,
    /// A horizontal sway of this many pixels (0: none), its period 64 frames.
    pub sway: u8,
}

/// OBJ tiles a weather's particles may use.
pub const PARTICLE_TILES: usize = 8;

/// What a weather shows (see the module docs).
#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    /// A blend of the field's palettes (`(colour BGR555, amount 0..=16)`), after the colour
    /// map: fog, sand, the sea's blue.
    pub blend: Option<(u16, u8)>,
    pub particles: Option<Particles>,
    /// The drought's pulse: the colour map moves between its target and 2 indices less
    /// every 32 frames (display only).
    pub pulse: bool,
}

/// One pixel of colour `c` (1..=15) at `x` in an 8-pixel row.
const fn px(c: u32, x: u32) -> u32 {
    c << (x * 4)
}

/// A rain streak, 8x16 (Tall, size 0): a diagonal line, light then dark.
static RAIN_TILES: [Tile; 2] = [
    [
        px(1, 7),
        px(1, 7),
        px(1, 6),
        px(1, 6),
        px(2, 5),
        px(2, 5),
        px(2, 4),
        px(2, 4),
    ],
    [
        px(2, 3),
        px(2, 3),
        px(3, 2),
        px(3, 2),
        px(3, 1),
        px(3, 1),
        0,
        0,
    ],
];

/// A 3x3 flake (snow, ash), 8x8.
static FLAKE_TILES: [Tile; 1] = [[
    0,
    px(2, 3),
    px(2, 2) | px(1, 3) | px(2, 4),
    px(2, 3),
    0,
    0,
    0,
    0,
]];

/// A sand streak, 16x8 (Wide, size 0).
static SAND_TILES: [Tile; 2] = [
    [
        0,
        0,
        0,
        px(1, 4) | px(1, 5) | px(2, 6) | px(2, 7),
        0,
        0,
        0,
        0,
    ],
    [
        0,
        0,
        0,
        px(2, 0) | px(3, 1) | px(3, 2),
        0,
        0,
        0,
        0,
    ],
];

/// A bubble, 8x8.
static BUBBLE_TILES: [Tile; 1] = [[
    0,
    px(1, 3) | px(1, 4),
    px(1, 2) | px(2, 3) | px(1, 5),
    px(1, 2) | px(1, 5),
    px(1, 3) | px(1, 4),
    0,
    0,
    0,
]];

const fn pal(c1: u16, c2: u16, c3: u16) -> [u16; 16] {
    let mut p = [0u16; 16];
    p[1] = c1;
    p[2] = c2;
    p[3] = c3;
    p
}

/// BGR555 of 0..=31 channels.
pub const fn rgb(r: u16, g: u16, b: u16) -> u16 {
    r | g << 5 | b << 10
}

/// The built-in look of Gen3 weather `w` (`None` draws nothing: none, sunny, clouds, the
/// unknown values).
pub fn builtin_look(w: u8) -> Option<Look> {
    let rain = |count| Particles {
        tiles: &RAIN_TILES,
        shape: ObjShape::Tall,
        size: 0,
        palette: pal(rgb(26, 28, 31), rgb(20, 23, 29), rgb(14, 17, 25)),
        count,
        dx: -20,
        dy: 48,
        sway: 0,
    };
    Some(match w {
        // Gen3 `targetRainSpriteCount`: 10, 16, 24.
        RAIN => Look {
            particles: Some(rain(10)),
            ..Look::default()
        },
        RAIN_THUNDERSTORM => Look {
            particles: Some(rain(16)),
            ..Look::default()
        },
        DOWNPOUR => Look {
            particles: Some(rain(24)),
            ..Look::default()
        },
        SNOW => Look {
            particles: Some(Particles {
                tiles: &FLAKE_TILES,
                shape: ObjShape::Square,
                size: 0,
                palette: pal(rgb(31, 31, 31), rgb(24, 26, 31), 0),
                count: 16,
                dx: 0,
                dy: 8,
                sway: 6,
            }),
            ..Look::default()
        },
        VOLCANIC_ASH => Look {
            blend: Some((rgb(16, 15, 14), 3)),
            particles: Some(Particles {
                tiles: &FLAKE_TILES,
                shape: ObjShape::Square,
                size: 0,
                palette: pal(rgb(20, 19, 18), rgb(12, 11, 11), 0),
                count: 20,
                dx: -4,
                dy: 10,
                sway: 3,
            }),
            ..Look::default()
        },
        SANDSTORM => Look {
            blend: Some((rgb(27, 21, 12), 5)),
            particles: Some(Particles {
                tiles: &SAND_TILES,
                shape: ObjShape::Wide,
                size: 0,
                palette: pal(rgb(30, 25, 16), rgb(25, 19, 10), rgb(19, 14, 7)),
                count: 20,
                dx: -56,
                dy: 4,
                sway: 0,
            }),
            ..Look::default()
        },
        FOG_HORIZONTAL | FOG_DIAGONAL => Look {
            blend: Some((rgb(28, 28, 28), 5)),
            ..Look::default()
        },
        UNDERWATER => Look {
            blend: Some((rgb(4, 12, 24), 4)),
            ..Look::default()
        },
        UNDERWATER_BUBBLES => Look {
            blend: Some((rgb(4, 12, 24), 4)),
            particles: Some(Particles {
                tiles: &BUBBLE_TILES,
                shape: ObjShape::Square,
                size: 0,
                palette: pal(rgb(26, 30, 31), rgb(31, 31, 31), 0),
                count: 8,
                dx: 0,
                dy: -6,
                sway: 2,
            }),
            ..Look::default()
        },
        DROUGHT => Look {
            pulse: true,
            ..Look::default()
        },
        _ => return None,
    })
}

// ---- colour maps ----

/// Gen3 `NUM_WEATHER_COLOR_MAPS`.
pub const COLOR_MAPS: usize = 19;

/// Gen3's colour maps: `darkenedContrastColorMaps` then `contrastColorMaps`, [`COLOR_MAPS`]
/// each, by channel value.
#[derive(Clone, Debug)]
pub struct ColorMaps(Vec<[u8; 32]>);

impl ColorMaps {
    /// The darkened contrast maps (BG palettes) or, with `obj`, the contrast maps.
    pub fn of(&self, obj: bool) -> &[[u8; 32]] {
        let at = obj as usize * COLOR_MAPS;
        &self.0[at..at + COLOR_MAPS]
    }
}

/// Gen3 `BuildColorMaps` (built on the heap, one 32-byte map at a time: the GBA's stack).
pub fn color_maps() -> Box<ColorMaps> {
    let mut all = alloc::vec![[0u8; 32]; 2 * COLOR_MAPS];
    for (i, maps) in all.chunks_mut(COLOR_MAPS).enumerate() {
        for v in 0..32u16 {
            let mut cur: u16 = v << 8;
            let delta = if i == 0 { (v << 8) / 16 } else { 0 };
            let mut idx = 0;
            while idx < 3 {
                cur -= delta;
                maps[idx][v as usize] = (cur >> 8) as u8;
                idx += 1;
            }
            let base = cur;
            let delta = (0x1f00 - cur) / (COLOR_MAPS as u16 - 3);
            while idx < COLOR_MAPS {
                cur += delta;
                if v < 12 {
                    let diff = cur as i16 - base as i16;
                    if diff > 0 {
                        cur = (cur as i16 - diff / 2) as u16;
                    }
                }
                maps[idx][v as usize] = ((cur >> 8) as u8).min(31);
                idx += 1;
            }
        }
    }
    Box::new(ColorMaps(all))
}

/// Colour `c` through colour map `index` of `maps` (one of [`color_maps`]): Gen3
/// `ApplyColorMap` for `index > 0` (`maps[index - 1]` per channel); a negative index (the
/// drought, `sDroughtWeatherColors`, degraded) brightens towards a warm white by `-index`
/// sixteenths; 0 keeps the colour.
pub fn map_color(c: u16, index: i8, maps: &[[u8; 32]]) -> u16 {
    if index > 0 {
        let m = &maps[(index as usize - 1).min(COLOR_MAPS - 1)];
        let r = m[(c & 31) as usize] as u16;
        let g = m[((c >> 5) & 31) as usize] as u16;
        let b = m[((c >> 10) & 31) as usize] as u16;
        r | g << 5 | b << 10
    } else if index < 0 {
        blend(c, rgb(31, 29, 21), (-index) as u8)
    } else {
        c
    }
}

/// `c` blended `amount`/16 towards `to` (Gen3 `BlendPalette`).
pub fn blend(c: u16, to: u16, amount: u8) -> u16 {
    let mut out = 0;
    for shift in [0, 5, 10] {
        let a = ((c >> shift) & 31) as i32;
        let b = ((to >> shift) & 31) as i32;
        let v = a + (b - a) * i32::from(amount.min(16)) / 16;
        out |= (v as u16 & 31) << shift;
    }
    out
}

#[cfg(test)]
mod tests;
