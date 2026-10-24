use super::*;

fn ticks(w: &mut Weather, rng: &mut Rng, n: u32) {
    for _ in 0..n {
        w.tick(rng);
    }
}

fn hashed(w: &Weather) -> u64 {
    let mut h = Fnv64::new();
    w.hash(&mut h);
    h.finish()
}

#[test]
fn translate_keeps_known_and_game_kinds_and_cycles_by_stage() {
    assert_eq!(translate(RAIN, 0), RAIN);
    assert_eq!(translate(ABNORMAL, 3), ABNORMAL);
    assert_eq!(translate(16, 0), NONE);
    assert_eq!(translate(GAME_FIRST + 2, 0), GAME_FIRST + 2);
    assert_eq!(
        (0..4)
            .map(|s| translate(ROUTE119_CYCLE, s))
            .collect::<alloc::vec::Vec<_>>(),
        [SUNNY, RAIN, RAIN_THUNDERSTORM, RAIN]
    );
    assert_eq!(translate(ROUTE123_CYCLE, 2), RAIN);
    assert_eq!(from_coord(5), Some(RAIN_THUNDERSTORM));
    assert_eq!(from_coord(20), Some(ROUTE119_CYCLE));
    assert_eq!(from_coord(0), None);
}

#[test]
fn battle_weather_follows_the_switch_in_rule() {
    assert_eq!(battle_weather(RAIN), Some(BattleWeather::Rain));
    assert_eq!(battle_weather(DOWNPOUR), Some(BattleWeather::Rain));
    assert_eq!(battle_weather(SANDSTORM), Some(BattleWeather::Sandstorm));
    assert_eq!(battle_weather(DROUGHT), Some(BattleWeather::Sun));
    assert_eq!(battle_weather(SNOW), None);
    assert_eq!(battle_weather(GAME_FIRST), None);
}

/// A map resting in its header's weather hashes nothing (maps with weather keep their hashes).
#[test]
fn a_settled_weather_is_not_hashed() {
    let mut rng = Rng::new(1);
    for header in [NONE, SHADE, FOG_HORIZONTAL, SUNNY, GAME_FIRST] {
        let mut w = Weather::default();
        w.load_header(header);
        w.resume();
        assert!(w.is_default(), "{header}");
        assert_eq!(hashed(&w), Fnv64::new().finish());
        ticks(&mut w, &mut rng, 200);
        assert!(w.is_default(), "{header} after ticks");
    }
    let mut w = Weather::settled(SHADE, 0);
    assert_eq!(w.color_map, 3);
    w.set_saved(RAIN);
    assert!(!w.is_default());
    assert_ne!(hashed(&w), Fnv64::new().finish());
    w.pass_days(1);
    w.reset_saved();
    assert!(!w.is_default(), "a moved cycle stage is state");
}

#[test]
fn a_warp_resume_is_instant_and_a_crossing_is_gradual() {
    let mut rng = Rng::new(7);
    let mut w = Weather::default();
    w.load_header(RAIN);
    w.resume();
    assert_eq!((w.current, w.color_map), (RAIN, 3));
    // `doweather` to none: the rain's finish is immediate, the colour map steps back every
    // frame (`None_Init`: no step delay).
    w.load_header(NONE);
    w.do_current();
    assert_eq!(w.next, NONE);
    w.tick(&mut rng);
    assert_eq!(w.current, NONE);
    assert_eq!(w.color_map, 2);
    ticks(&mut w, &mut rng, 2);
    assert_eq!(w.color_map, 0);
    // A crossing into shade darkens over 3 x 20 frames.
    w.load_header(SHADE);
    w.do_current();
    ticks(&mut w, &mut rng, 1);
    assert_eq!((w.current, w.color_map), (SHADE, 0));
    ticks(&mut w, &mut rng, 58);
    assert_eq!(w.color_map, 2);
    ticks(&mut w, &mut rng, 1);
    assert_eq!(w.color_map, 3);
    assert!(w.is_default());
}

#[test]
fn setweather_is_saved_until_doweather_and_resetweather_restores_the_header() {
    let mut rng = Rng::new(3);
    let mut w = Weather::settled(NONE, 0);
    w.set_saved(SANDSTORM);
    ticks(&mut w, &mut rng, 5);
    assert_eq!(w.current, NONE);
    w.do_current();
    ticks(&mut w, &mut rng, 1);
    assert_eq!(w.current, SANDSTORM);
    w.reset_saved();
    assert_eq!(w.saved, NONE);
    w.do_current();
    ticks(&mut w, &mut rng, 1);
    assert_eq!(w.current, NONE);
}

/// The thunderstorm's lightning: Gen3's cycle, deterministic from the RNG, flashing colour map
/// 19 and back to 3; the RNG moves only through the cycle's draws.
#[test]
fn thunderstorm_flashes_deterministically() {
    let run = |seed| {
        let mut rng = Rng::new(seed);
        let mut w = Weather::settled(RAIN_THUNDERSTORM, 0);
        let mut flashes = 0;
        let mut was = false;
        for _ in 0..3000 {
            w.tick(&mut rng);
            let lit = w.color_map > 3;
            if lit && !was {
                flashes += 1;
            }
            was = lit;
        }
        (flashes, rng.state(), w)
    };
    let (a, sa, wa) = run(11);
    let (b, sb, wb) = run(11);
    assert_eq!((a, sa), (b, sb));
    assert_eq!(wa, wb);
    assert!(a >= 3, "{a} flashes in 3000 frames");
    let (_, sc, _) = run(12);
    assert_ne!(sa, sc);
    // Rain draws nothing.
    let mut rng = Rng::new(5);
    let mut w = Weather::settled(RAIN, 0);
    ticks(&mut w, &mut rng, 1000);
    assert_eq!(rng.state(), Rng::new(5).state());
}

/// A storm ending waits for its cycle (`Thunderstorm_Finish`) unless rain follows.
#[test]
fn a_storm_ends_at_its_cycle_end() {
    let mut rng = Rng::new(9);
    let mut w = Weather::settled(RAIN_THUNDERSTORM, 0);
    w.set_weather(2); // sunny
    let mut n = 0;
    while w.current != SUNNY {
        w.tick(&mut rng);
        n += 1;
        assert!(n < 2000);
    }
    let mut w = Weather::settled(RAIN_THUNDERSTORM, 0);
    w.set_weather(RAIN);
    ticks(&mut w, &mut rng, 3);
    assert_eq!(w.current, RAIN);
}

/// `WEATHER_ABNORMAL`: drought and downpour swap every 600 frames.
#[test]
fn abnormal_weather_alternates() {
    let mut rng = Rng::new(2);
    let mut w = Weather::settled(ABNORMAL, 0);
    assert_eq!(w.current, DOWNPOUR);
    let mut seen = alloc::vec::Vec::new();
    for _ in 0..2500 {
        w.tick(&mut rng);
        if seen.last() != Some(&w.current) {
            seen.push(w.current);
        }
    }
    assert!(seen.starts_with(&[DOWNPOUR, DROUGHT]), "{seen:?}");
    assert!(seen.contains(&DOWNPOUR) && seen.len() >= 3, "{seen:?}");
}

#[test]
fn color_maps_match_gen3() {
    let all = color_maps();
    let m = [all.of(false), all.of(true)];
    // Contrast maps 0..3 are the identity; the darkened ones scale down by sixteenths.
    for v in 0..32u8 {
        assert_eq!(m[1][0][v as usize], v);
    }
    assert_eq!(
        m[0][2][31],
        (((31u16 << 8) - 3 * ((31u16 << 8) / 16)) >> 8) as u8
    );
    // The last map is white.
    assert_eq!(m[1][COLOR_MAPS - 1][31], 31);
    assert!(m[1][COLOR_MAPS - 1][20] > 20);
    let red = rgb(20, 0, 0);
    assert_eq!(map_color(red, 0, m[0]), red);
    assert!(map_color(red, 3, m[0]) & 31 < 20);
    assert!(map_color(red, LIGHTNING, m[1]) & 31 > 20);
    assert!(map_color(red, -6, m[0]) & 31 > 20);
}

#[test]
fn builtin_looks() {
    assert!(builtin_look(NONE).is_none());
    assert!(builtin_look(SUNNY).is_none());
    assert_eq!(builtin_look(DOWNPOUR).unwrap().particles.unwrap().count, 24);
    assert!(builtin_look(SANDSTORM).unwrap().blend.is_some());
    assert!(builtin_look(DROUGHT).unwrap().pulse);
    for w in 0..=ABNORMAL {
        if let Some(p) = builtin_look(w).and_then(|l| l.particles) {
            assert!(p.tiles.len() <= PARTICLE_TILES);
        }
    }
}

#[test]
fn days_move_the_cycle() {
    let mut w = Weather::default();
    w.pass_days(1);
    w.load_header(ROUTE119_CYCLE);
    assert_eq!(w.saved, RAIN);
    w.pass_days(5);
    w.load_header(ROUTE119_CYCLE);
    assert_eq!(w.saved, RAIN_THUNDERSTORM);
    w.pass_days(2);
    w.load_header(ROUTE119_CYCLE);
    assert_eq!(w.saved, SUNNY);
}
