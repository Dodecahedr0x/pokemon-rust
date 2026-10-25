use super::*;
use crate::battle::gfx::obj;
use crate::battle::gfx::test_gfx::{game, image, mon_gfx};
use crate::data::SpeciesId;
use crate::video::Frame;
use alloc::boxed::Box;

#[test]
fn tackle_runs_the_gen3_anim() {
    let game = game(Some(mon_gfx(0, 0, 0)));
    let mut mons = [
        MonSprite::new(Side::Player, game.gfx),
        MonSprite::new(Side::Opponent, game.gfx),
    ];
    mons[0].load(&game, SpeciesId(1), false, &mut Frame::default());
    mons[1].load(&game, SpeciesId(1), false, &mut Frame::default());
    mons[0].show();
    mons[1].show();
    let gfx: &'static GfxImage = Box::leak(Box::new(image(10135, 16, 1)));
    let mut player = Player::start(tackle(), Side::Player, Side::Opponent);
    let mut peak = 0i16;
    let mut saw_splat = false;
    let mut shook = false;
    let mut frames = 0;
    while !player.done() {
        let mut f = Frame::default();
        player.step(&mut mons, |tag| (tag == 10135).then_some(gfx), &mut f);
        peak = peak.max(mons[0].offset().0);
        shook |= mons[1].offset().0 == 3;
        saw_splat |= f
            .objs
            .iter()
            .any(|o| o.scale == 0xB0 && o.tile == obj::FREE);
        frames += 1;
        assert!(frames < 80, "tackle did not finish");
    }
    assert_eq!(peak, 16, "lunge should peak at 4×4 px");
    assert!(saw_splat, "impact sprite at Gen3 scale 0xB0");
    assert!(shook, "target shakes by 3 px");
    assert!(player.sounds().contains(&(139, 63)));
    assert_eq!(mons[0].offset(), (0, 0));
    assert_eq!(mons[1].offset(), (0, 0));
}
