//! BPEE from power-on to the truck, diffed against our engine's new game (Task O1's "done
//! when"). Needs `PKR_ROM_US` (BPEE, SHA-1 checked); `#[ignore]`d:
//! `cargo test -p vanilla --test boot -- --ignored --nocapture`.

use engine::Buttons;
use explore::semantic::{Semantic, Tolerance, diff, flag_name, map_name, var_name};
use vanilla::{Mode, Vanilla};

/// Generous: the boot takes about 3,000 frames.
const MAX_FRAMES: u64 = 20_000;

fn print(who: &str, s: &Semantic) {
    println!(
        "{who}: {} {:?} facing {:?}, party {}, money {}, badges {:08b}, respawn {}",
        map_name(s.map),
        s.cell,
        s.facing,
        s.party.len(),
        s.money,
        s.badges,
        s.respawn.map_or("none".into(), map_name)
    );
    let flags: Vec<_> = s.flags.iter().map(|&f| flag_name(f)).collect();
    println!("  flags ({}): {}", flags.len(), flags.join(" "));
    let vars: Vec<_> = (s.vars.iter())
        .map(|(&v, x)| format!("{}={x}", var_name(v)))
        .collect();
    println!("  vars ({}): {}", vars.len(), vars.join(" "));
    println!("  bag {:?}", s.bag);
    println!("  objects {:?}", s.objects);
}

/// Our new game, stepped until the player has control.
fn ours() -> Semantic {
    let game = content::Emerald::default();
    let mut s = game_native::Session::new(game);
    for _ in 0..600 {
        if s.world().field_active() && !s.world().script_running() {
            break;
        }
        s.step(Buttons::NONE);
    }
    Semantic::from_world(s.world(), &content::Emerald::default())
}

#[test]
#[ignore = "needs PKR_ROM_US (BPEE)"]
fn boot_to_the_truck_and_diff_the_new_game() {
    let Some(rom) = vanilla::rom_from_env() else {
        panic!("PKR_ROM_US is not set");
    };
    let (mut v, notes) = Vanilla::open(&rom).unwrap();
    for n in &notes {
        println!("sym: {n}");
    }
    let report = v.boot(MAX_FRAMES).unwrap();
    println!("boot: {} frames, phases {:?}", v.frames, report.phases);
    let state = v.state().unwrap();
    assert_eq!(state.mode, Mode::FieldIdle);
    assert_eq!(map_name(state.map), "MAP_INSIDE_OF_TRUCK");
    assert!(state.party.is_empty());
    let theirs = state.semantic();
    let ours = ours();
    print("vanilla", &theirs);
    print("ours", &ours);
    let d = diff(&ours, &theirs, Tolerance::default());
    println!("diff ({} fields):", d.len());
    for x in &d {
        println!("  {x}");
    }
    // Expected differences (plan "As built", Task O1): our new game starts outside Brendan's
    // house after the intro (`content::new_game`, `Start::BeforeRescue`), vanilla in the
    // truck, so the map, the cell, the Littleroot intro vars and flags, the respawn and the
    // objects differ by design (`tests/intro.rs` drives vanilla to our start and diffs there).
    // The rest of the diff is parity gaps, listed in the plan, not asserted here.
    assert!(ours.party.is_empty() && theirs.party.is_empty());
    assert_eq!((ours.badges, theirs.badges), (0, 0));
    assert_eq!(theirs.money, 3000, "Gen3's new game money");
}
