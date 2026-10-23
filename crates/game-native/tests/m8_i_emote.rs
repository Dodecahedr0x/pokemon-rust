//! Milestone 8 Task I: on the extracted game, the parity route's emotes (Littleroot's `!`) are
//! drawn with `GameData::emote`'s sprite, in the emote tail of OBJ VRAM, in front of objects.

use content::Emerald;
use engine::video::OBJ_TILES;
use game_native::Session;
use game_native::scenario::cached_route;

/// First tile of the emote slots (4 emotes of 4 tiles at the end of OBJ VRAM).
const EMOTE_FIRST_TILE: usize = OBJ_TILES - 16;

#[test]
fn route_emotes_draw_their_sprite() {
    if content::STUB {
        eprintln!("stub world: no emote sprites, nothing to check");
        return;
    }
    let rec = cached_route();
    let mut s = Session::new(Emerald::for_shells());
    let (mut shown, mut drawn) = (0u32, 0u32);
    for n in 0..rec.frames() {
        s.step(rec.keys_at(n));
        let active = (1..=u8::MAX).any(|l| s.world().objects().emote_active(l));
        if !active {
            continue;
        }
        shown += 1;
        let f = s.frame();
        if let Some(i) = f
            .objs
            .iter()
            .position(|o| o.tile as usize >= EMOTE_FIRST_TILE)
        {
            drawn += 1;
            assert_eq!(i, 0, "frame {n}: the bubble is drawn in front");
        }
    }
    eprintln!("emote shown {shown} frames, drawn {drawn}");
    assert!(shown > 0, "the route shows an emote");
    // Littleroot's `!`: 61 frames, each drawing the bubble.
    assert_eq!(
        drawn, shown,
        "every frame showing an emote draws its bubble"
    );
}
