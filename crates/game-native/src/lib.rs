//! Host shell: runs engine + content natively and draws each [`Frame`] with a software PPU.

pub mod checkpoints;
pub mod controls;
pub mod flash;
pub mod ppu;
pub mod scenario;
pub mod store;
pub mod trace;

use engine::checkpoint::{Checkpoint, NotCapturable, RestoreError};
use engine::data::GameData;
use engine::video::Frame;
pub use engine::video::{SCREEN_H, SCREEN_W};
use engine::{Buttons, World};
pub use ppu::Ppu;

/// A native game instance: World + game data + PPU + last frame.
///
/// Every step applies its frame's commands to the PPU; the picture is only drawn when
/// [`Session::framebuffer`] asks for it, so headless runs (explorers, long replays) skip
/// rendering.
///
/// Unlike the GBA shell there is no display lag: after step `n` the framebuffer shows the
/// `Frame` returned by step `n` (the ROM shows it one emulator frame later, see
/// `engine::debug`).
pub struct Session<G: GameData> {
    game: G,
    world: World,
    ppu: Ppu,
    frame: Frame,
    fb: Vec<u16>,
    /// The in-game save of the last step that raised `Event::Saved { ok: true }` (Milestone 9),
    /// until [`Session::take_save`] takes it.
    save: Option<Checkpoint>,
    /// `fb` shows `frame`.
    drawn: bool,
}

impl<G: GameData> Session<G> {
    /// A new game (`World::new`), its first frame applied.
    pub fn new(game: G) -> Self {
        let world = World::new(&game);
        Self::boot(game, world)
    }

    /// A session resumed from `cp` (`World::restore`): a fresh PPU gets the restored boot
    /// frame, as for a new game.
    pub fn restore(game: G, cp: &Checkpoint) -> Result<Self, RestoreError> {
        let world = World::restore(&game, cp)?;
        Ok(Self::boot(game, world))
    }

    /// Replaces the world by `cp`'s (`World::restore`) on a fresh PPU, as [`Session::restore`]
    /// does; on error the session is unchanged.
    pub fn load(&mut self, cp: &Checkpoint) -> Result<(), RestoreError> {
        self.world = World::restore(&self.game, cp)?;
        self.reset();
        Ok(())
    }

    /// Captures the world (`World::checkpoint`).
    pub fn checkpoint(&self) -> Result<Checkpoint, NotCapturable> {
        self.world.checkpoint(&self.game)
    }

    /// A session on `world`, its boot frame applied to a fresh PPU.
    fn boot(game: G, world: World) -> Self {
        let mut s = Self {
            game,
            world,
            ppu: Ppu::new(),
            frame: Frame::default(),
            fb: vec![0; SCREEN_W * SCREEN_H],
            drawn: false,
            save: None,
        };
        s.reset();
        s
    }

    /// Applies the world's boot frame to a fresh PPU (shared by [`Session::boot`] and
    /// [`Session::load`]).
    fn reset(&mut self) {
        self.frame = self.world.initial_frame();
        self.ppu = Ppu::new();
        self.ppu.apply(&self.frame);
        self.drawn = false;
    }

    pub fn step(&mut self, keys: Buttons) {
        self.frame = self.world.step(&self.game, keys);
        self.ppu.apply(&self.frame);
        self.drawn = false;
        if (self.world.events().iter())
            .any(|e| matches!(e, engine::world::Event::Saved { ok: true }))
        {
            self.save = self.world.save_snapshot(&self.game).ok();
        }
    }

    /// The in-game save the last `Event::Saved { ok: true }` asked the shell to persist (Gen3
    /// `TrySavingData`), once. The window build writes it to `saves/<flavor>/game.sav`
    /// (Milestone 9 Task S).
    pub fn take_save(&mut self) -> Option<Checkpoint> {
        self.save.take()
    }

    pub fn game(&self) -> &G {
        &self.game
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    /// The last frame (its commands already applied).
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The picture of the last frame, rendered on first request.
    pub fn framebuffer(&mut self) -> &[u16] {
        if !self.drawn {
            self.ppu.render(&self.frame, &mut self.fb);
            self.drawn = true;
        }
        &self.fb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distinct_colors(fb: &[u16]) -> usize {
        let mut c = fb.to_vec();
        c.sort_unstable();
        c.dedup();
        c.len()
    }

    #[test]
    fn boots_black_then_fades_into_the_start_map() {
        let mut s = Session::new(content::Emerald::for_shells());
        assert_eq!(
            s.world().map(),
            content::Emerald::for_shells().new_game().map
        );
        assert!(s.framebuffer().iter().all(|&c| c == 0), "frame 0 is black");
        for _ in 0..40 {
            s.step(Buttons::NONE);
        }
        assert!(distinct_colors(s.framebuffer()) > 3, "the map is visible");
    }

    #[test]
    fn headless_steps_render_only_on_request() {
        let mut s = Session::new(content::Emerald::for_shells());
        for _ in 0..5 {
            s.step(Buttons::DOWN);
        }
        assert!(!s.drawn);
        let fb = s.framebuffer().to_vec();
        assert!(s.drawn);
        let mut ppu_fb = vec![0; SCREEN_W * SCREEN_H];
        s.ppu().render(s.frame(), &mut ppu_fb);
        assert_eq!(fb, ppu_fb);
    }

    #[test]
    fn sessions_are_deterministic() {
        let (mut a, mut b) = (
            Session::new(content::Emerald::for_shells()),
            Session::new(content::Emerald::for_shells()),
        );
        for i in 0..200u32 {
            let keys =
                [Buttons::UP, Buttons::LEFT, Buttons::A, Buttons::NONE][(i / 23 % 4) as usize];
            a.step(keys);
            b.step(keys);
            assert_eq!(a.world().hash(), b.world().hash(), "frame {i}");
        }
        assert_eq!(a.framebuffer(), b.framebuffer());
    }
}
