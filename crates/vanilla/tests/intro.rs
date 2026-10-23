//! Past the truck (the coordinator's "go as far as possible"): BPEE is driven through the
//! Littleroot intro (Mom, the clock, May's house, meeting May) to the point our new game starts
//! at (`content::new_game`, `Start::BeforeRescue`: outside the houses, May met), then diffed
//! against our new game. Needs `PKR_ROM_US`; `#[ignore]`d:
//! `cargo test -p vanilla --test intro -- --ignored --nocapture`.

use engine::Buttons;
use engine::data::Dir;
use explore::semantic::{Semantic, Tolerance, diff, map_name, var_name};
use vanilla::Vanilla;
use vanilla::drive::Walk;

const STEP: u64 = 6_000;

fn ours() -> Semantic {
    ours_from(content::Emerald::default())
}

fn ours_from(game: content::Emerald) -> Semantic {
    let mut s = game_native::Session::new(game);
    for _ in 0..600 {
        if s.world().field_active() && !s.world().script_running() {
            break;
        }
        s.step(Buttons::NONE);
    }
    Semantic::from_world(s.world(), &content::Emerald::default())
}

fn log(v: &mut Vanilla, what: &str) {
    let s = v.state().unwrap();
    let vars: Vec<_> = (s.vars.iter())
        .filter(|(n, _)| var_name(*n).starts_with("VAR_LITTLEROOT"))
        .map(|(n, x)| format!("{}={x}", var_name(*n)))
        .collect();
    println!(
        "[{:>6}] {what}: {} {:?} {:?} {}",
        v.frames,
        map_name(s.map),
        s.cell,
        s.mode,
        vars.join(" ")
    );
}

/// Walks to `to`, going on after triggers (a coord event interrupts a walk).
fn go(v: &mut Vanilla, game: &content::Emerald, to: (i16, i16)) -> Walk {
    for _ in 0..8 {
        match v.walk_to(game, to, STEP).unwrap() {
            Walk::Interrupted(at) => println!("  interrupted at {at:?}"),
            w => {
                v.settle(STEP).unwrap();
                return w;
            }
        }
    }
    panic!("walk to {to:?}: interrupted 8 times")
}

fn on(v: &mut Vanilla, map: &str) {
    let s = v.state().unwrap();
    assert_eq!(map_name(s.map), map, "at {:?}", s.cell);
}

#[test]
#[ignore = "needs PKR_ROM_US (BPEE)"]
fn intro_to_our_new_game_start() {
    let rom = vanilla::rom_from_env().expect("PKR_ROM_US");
    let (mut v, _) = Vanilla::open(&rom).unwrap();
    let game = content::Emerald::default();
    v.boot(20_000).unwrap();
    log(&mut v, "truck");
    // Out of the truck (its door is the warp column x = 4); the coord event at x = 3 sets the
    // male intro flags. Mom then walks the player into the house.
    let w = go(&mut v, &game, (4, 2));
    println!("  {w:?}");
    v.push(Dir::Right, STEP).unwrap();
    log(&mut v, "after the truck");
    on(&mut v, "MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F");
    // Upstairs to set the clock.
    let w = go(&mut v, &game, (8, 2));
    println!("  {w:?}");
    log(&mut v, "2F");
    on(&mut v, "MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F");
    assert_eq!(go(&mut v, &game, (5, 2)), Walk::Reached);
    v.interact(Dir::Up, STEP).unwrap();
    log(&mut v, "clock set");
    // Downstairs: the TV.
    let w = go(&mut v, &game, (7, 1));
    println!("  {w:?}");
    log(&mut v, "1F again");
    on(&mut v, "MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F");
    // Out, then to May's house.
    let w = go(&mut v, &game, (8, 8));
    println!("  {w:?}");
    v.push(Dir::Down, STEP).unwrap(); // the door mat is an arrow warp
    log(&mut v, "outside");
    on(&mut v, "MAP_LITTLEROOT_TOWN");
    let w = go(&mut v, &game, (14, 8));
    println!("  {w:?}");
    log(&mut v, "May's 1F");
    on(&mut v, "MAP_LITTLEROOT_TOWN_MAYS_HOUSE_1F");
    let w = go(&mut v, &game, (2, 2));
    println!("  {w:?}");
    log(&mut v, "May's 2F");
    on(&mut v, "MAP_LITTLEROOT_TOWN_MAYS_HOUSE_2F");
    // May's Poké Ball (5, 4): May comes in.
    assert_eq!(go(&mut v, &game, (5, 3)), Walk::Reached);
    v.interact(Dir::Down, STEP).unwrap();
    log(&mut v, "met May");
    let w = go(&mut v, &game, (1, 1));
    println!("  {w:?}");
    log(&mut v, "May's 1F again");
    let w = go(&mut v, &game, (2, 8));
    println!("  {w:?}");
    v.push(Dir::Down, STEP).unwrap(); // the door mat is an arrow warp
    log(&mut v, "outside again");
    on(&mut v, "MAP_LITTLEROOT_TOWN");

    let theirs = v.state().unwrap().semantic();
    let ours = ours();
    let d = diff(&ours, &theirs, Tolerance::default());
    println!("diff at our new game's start ({} fields):", d.len());
    for x in &d {
        println!("  {x}");
    }
    assert!(theirs.party.is_empty());
    // Expected: our new game stands outside Brendan's house, vanilla has just left May's.
    let unexpected: Vec<_> = d.iter().filter(|x| x.field != "cell").collect();
    assert!(unexpected.is_empty(), "new-game state gaps: {unexpected:?}");

    // On to Route 101: Birch's rescue (the bag's starter screen picks Mudkip, the battle
    // uses the first move), then the lab, where Birch gives the starter for good.
    // The trigger at (11, 1) ("go save Birch") may walk the player on by itself.
    if go(&mut v, &game, (10, 0)) == Walk::Reached {
        v.push(Dir::Up, STEP).unwrap();
    }
    log(&mut v, "Route 101");
    on(&mut v, "MAP_ROUTE101");
    assert_eq!(go(&mut v, &game, (7, 15)), Walk::Reached);
    v.interact(Dir::Up, 4 * STEP).unwrap();
    log(&mut v, "after the rescue");
    let st = v.state().unwrap();
    println!("  party {:?}", st.party);

    let theirs = v.state().unwrap().semantic();
    let ours = ours_from(content::Emerald::after_starter());
    // Vanilla's starter fought the Zigzagoon (exp) and may be hurt: levels and HP are not
    // compared here.
    let tol = Tolerance {
        levels: false,
        ..Tolerance::default()
    };
    let d = diff(&ours, &theirs, tol);
    println!("diff at our after-starter start ({} fields):", d.len());
    for x in &d {
        println!("  {x}");
    }
    assert_eq!(theirs.party.len(), 1);
    // Expected: vanilla stands in the lab (its objects are the lab's), ours outside Brendan's
    // house.
    let placement = |f: &str| ["map", "cell", "facing"].contains(&f) || f.starts_with("object ");
    let unexpected: Vec<_> = d.iter().filter(|x| !placement(&x.field)).collect();
    assert!(
        unexpected.is_empty(),
        "after-starter state gaps: {unexpected:?}"
    );
}
