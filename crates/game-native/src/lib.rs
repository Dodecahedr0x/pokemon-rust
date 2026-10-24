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

// `cfg(pkr_silice)` (`build.rs`, from `content`'s `links` metadata) is the content's flavour.
const _: () = assert!(cfg!(pkr_silice) == matches!(content::FLAVOR, content::Flavor::Silice));

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
    /// The save the main menu's CONTINUE restores (Milestone 9 Task S, [`Session::boot_sav`]).
    continue_cp: Option<Checkpoint>,
    /// The last save known to the shell (the booted one, then each in-game save): what
    /// `Event::ReloadLastSave` brings back.
    last_save: Option<Checkpoint>,
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

    /// Boots on a flash image as the ROM does (Milestone 9 Task S): an in-game save opens the
    /// main menu (`World::main_menu`; CONTINUE restores it, NEW GAME starts over), another
    /// valid slot is restored at once, anything else is a new game.
    pub fn boot_sav(game: G, sav: &[u8]) -> Self {
        match flash::decode_sav(sav) {
            Ok((meta, cp)) if meta.badges & flash::IN_GAME_SAVE != 0 => {
                let shown = engine::checkpoint::SaveMeta {
                    badges: meta.badges & !flash::IN_GAME_SAVE,
                    ..meta
                };
                let world = World::main_menu(&game, shown);
                let mut s = Self::boot(game, world);
                s.last_save = Some(cp.clone());
                s.continue_cp = Some(cp);
                s
            }
            Ok((_, cp)) => match World::restore(&game, &cp) {
                Ok(world) => Self::boot(game, world),
                Err(_) => Self::new(game),
            },
            Err(_) => Self::new(game),
        }
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
            continue_cp: None,
            last_save: None,
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
        // The main menu's choice replaces the boot World, on a fresh PPU as the ROM's black
        // load does (Task S).
        if let Some(choice) = self.world.main_menu_choice() {
            let cp = self.continue_cp.take();
            let restored = match (choice, &cp) {
                (engine::world::MainMenuChoice::Continue, Some(cp)) => {
                    World::restore(&self.game, cp).ok()
                }
                _ => None,
            };
            self.world = restored.unwrap_or_else(|| {
                let mut w = World::new(&self.game);
                w.set_save_present(true);
                w
            });
            self.reset();
            return;
        }
        if (self.world.events().iter())
            .any(|e| matches!(e, engine::world::Event::Saved { ok: true }))
        {
            self.save = self.world.save_snapshot(&self.game).ok();
            if self.save.is_none() {
                self.world.save_failed();
            }
            self.last_save = self.save.clone().or(self.last_save.take());
        }
        // `Event::ReloadLastSave` (Silice's alternate ending): the last save comes back as
        // CONTINUE would bring it (a new game when there is none); nothing is written.
        if (self.world.events().iter()).any(|e| matches!(e, engine::world::Event::ReloadLastSave)) {
            self.reload_last_save();
        }
    }

    /// Replaces the world by the last save (`Event::ReloadLastSave`), or a new game when
    /// there is none, on a fresh PPU.
    fn reload_last_save(&mut self) {
        let restored = (self.last_save.as_ref()).and_then(|cp| World::restore(&self.game, cp).ok());
        self.world = restored.unwrap_or_else(|| {
            let mut w = World::new(&self.game);
            w.set_save_present(self.last_save.is_some());
            w
        });
        self.reset();
    }

    /// The in-game save the last `Event::Saved { ok: true }` asked the shell to persist (Gen3
    /// `TrySavingData`), once. The window build writes it to `saves/<flavor>/game.sav`
    /// (Milestone 9 Task S, [`flash::write_in_game_sav`]); a failed write calls
    /// [`Session::save_failed`].
    pub fn take_save(&mut self) -> Option<Checkpoint> {
        self.save.take()
    }

    /// The shell could not write the last in-game save: the dialog answers "not saved".
    pub fn save_failed(&mut self) {
        self.world.save_failed();
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

    /// Milestone 9 Task S: an in-game save boots into the main menu, and CONTINUE gives the
    /// saved World back (a restore of the save, hash for hash).
    #[test]
    fn an_in_game_save_continues_from_the_main_menu() {
        let game = content::Emerald::for_shells();
        let mut s = Session::new(content::Emerald::for_shells());
        for _ in 0..60 {
            s.step(Buttons::NONE);
        }
        let cp = s.world().save_snapshot(&game).expect("an idle save");
        let sav = flash::write_in_game_sav(&flash::blank_sav(), &cp, &game.special_ids()).unwrap();
        let mut b = Session::boot_sav(content::Emerald::for_shells(), &sav);
        assert_eq!(b.world().screens_open(), 1, "the main menu");
        for _ in 0..30 {
            b.step(Buttons::NONE);
        }
        b.step(Buttons::A);
        for _ in 0..300 {
            if b.world().screens_open() == 0 && b.world().hash() == cp.header.world_hash {
                break;
            }
            b.step(Buttons::NONE);
        }
        assert_eq!(
            b.world().hash(),
            cp.header.world_hash,
            "CONTINUE restores the save"
        );
    }

    /// `Event::ReloadLastSave`: the last save the session knows comes back, else a new game.
    #[test]
    fn reload_last_save_restores_the_last_save_or_starts_over() {
        let mut s = Session::new(content::Emerald::for_shells());
        for _ in 0..60 {
            s.step(Buttons::NONE);
        }
        let new_game = World::new(s.game()).hash();
        s.reload_last_save();
        assert_eq!(s.world().hash(), new_game, "no save: a new game");
        for _ in 0..60 {
            s.step(Buttons::NONE);
        }
        let cp = s.world().save_snapshot(s.game()).expect("an idle save");
        s.last_save = Some(cp.clone());
        for _ in 0..40 {
            s.step(Buttons::DOWN);
        }
        s.reload_last_save();
        assert_eq!(s.world().hash(), cp.header.world_hash, "the last save");
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
