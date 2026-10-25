//! Tackle's script and the impact sprite, located on the French ROM (and the same
//! finder is what `export` runs for BPEE).

use extract::fixtures;
use extract::move_anim::{self, Cmd};

#[test]
fn french_tackle_is_the_gen3_script_with_impact_gfx() {
    let file = move_anim::extract(&fixtures::fr()).unwrap();
    let tackle = &file.scripts[33];
    assert!(
        tackle.windows(4).any(|w| {
            matches!(
                w,
                [
                    Cmd::LoadGfx(10135),
                    Cmd::MonBg(1),
                    Cmd::SetAlpha(12, 8),
                    Cmd::Lunge {
                        frames: 4,
                        delta: 4
                    },
                ]
            )
        }),
        "{tackle:?}"
    );
    assert!(tackle.contains(&Cmd::Delay(6)));
    assert!(tackle.contains(&Cmd::HitSplat {
        tag: 10135,
        x: 0,
        y: 0,
        relative: 1,
        anim: 2,
    }));
    assert!(tackle.contains(&Cmd::Shake {
        who: 1,
        x: 3,
        y: 0,
        times: 6,
        delay: 1,
    }));
    assert!(tackle.contains(&Cmd::PlaySe { se: 139, pan: 63 }));
    assert_eq!(tackle.last().copied(), Some(Cmd::End));
    let impact = file.gfx.iter().find(|g| g.tag == 10135).unwrap();
    assert_eq!(impact.tiles.len(), 16);
    assert!(impact.tiles.iter().any(|t| t.iter().any(|&w| w != 0)));
    assert!(impact.palette.iter().any(|&c| c != 0));
}
