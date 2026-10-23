//! The mGBA play pane (plan: Task D): `target/game.gba` through `emu`, booted as a new game,
//! from a checkpoint node (`flash::encode_sav`) or from a `.sav` image, with the ROM's debug
//! region as a timeline, a "ROM build differs from the native build" check, and a native
//! shadow that steps a `Session` on the keys the ROM consumed and compares hashes.
//!
//! No rewind and no mid-trace seek (decision 10).

use crate::native::lock_materializer;
use crate::pane::{Pane, RunMode};
use anyhow::{Context, Result, anyhow};
use content::Emerald;
use emu::{DebugSnapshot, Emu};
use engine::Buttons;
use engine::checkpoint::{BuildHash, Checkpoint, build_hash};
use engine::debug::{INPUT_RING, boot, input_slot};
use game_native::checkpoints::Materializer;
use game_native::store::Store;
use game_native::trace::CheckpointId;
use game_native::{SCREEN_H, SCREEN_W, Session};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Timeline entries kept (newest last).
pub const TIMELINE_LEN: usize = 600;
/// Emulator frames a reset waits for the ROM to publish its boot state.
pub const BOOT_FRAMES: u32 = 600;
/// Most frames one tick runs for pending `Step`s (the rest run on later ticks).
pub const MAX_STEPS_PER_TICK: u32 = 16;
/// Ticks between two probes of the ROM file on disk (about once a second).
pub const PROBE_TICKS: u32 = 60;

/// What the ROM boots on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RomSource {
    /// Erased flash.
    NewGame,
    /// A checkpoint node of the pane's layered store, materialised natively through the
    /// shared [`Materializer`] (as the native pane loads it) and written as slot 0 of a fresh
    /// flash.
    Node(CheckpointId),
    /// A `FLASH_SIZE` flash image.
    Sav(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RomCmd {
    Keys(Buttons),
    Mode(RunMode),
    /// Run `n` more frames (they run while paused, at most `MAX_STEPS_PER_TICK` per tick).
    Step(u32),
    Reset(RomSource),
    /// `Reset` with the last source (after a rebuild of the ROM); a no-op before any reset.
    Reload,
    /// Writes the cartridge flash to `<scratch>/rom-<unix ms>.sav`.
    ExportSave,
}

/// The debug region after one emulator frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineEntry {
    /// Emulator frames since the reset (boot frames included).
    pub emu_frame: u32,
    /// The ROM's engine frame.
    pub frame: u32,
    pub hash: u64,
    pub late: u32,
    pub late_black: u32,
    pub saves: u32,
    pub save_status: u32,
    pub hash_lines_max: u32,
    pub step_lines_max: u32,
    pub loop_lines_max: u32,
    /// The native shadow's hash at `frame`, when the shadow runs (see [`RomStatus::shadow`]).
    pub native_hash: Option<u64>,
}

impl TimelineEntry {
    pub fn new(emu_frame: u32, s: &DebugSnapshot, native_hash: Option<u64>) -> TimelineEntry {
        TimelineEntry {
            emu_frame,
            frame: s.frame,
            hash: s.hash,
            late: s.late,
            late_black: s.late_black,
            saves: s.saves,
            save_status: s.save_status,
            hash_lines_max: s.hash_lines_max,
            step_lines_max: s.step_lines_max,
            loop_lines_max: s.loop_lines_max,
            native_hash,
        }
    }

    /// The shadow ran and reached another state than the ROM.
    pub fn mismatch(&self) -> bool {
        self.native_hash.is_some_and(|h| h != self.hash)
    }
}

/// `target/game.gba` on disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RomFile {
    #[default]
    Missing,
    Present {
        modified: Option<SystemTime>,
    },
}

impl RomFile {
    pub fn probe(path: &Path) -> RomFile {
        match std::fs::metadata(path) {
            Ok(m) if m.is_file() => RomFile::Present {
                modified: m.modified().ok(),
            },
            _ => RomFile::Missing,
        }
    }
}

/// The native shadow's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shadow {
    /// No ROM running, or the ROM's boot cannot be mirrored natively (a rejected, corrupt or
    /// skipped save, no flash, or a boot state that differs from the native one).
    #[default]
    Off,
    /// Every compared hash matched so far.
    Matching,
    /// The first engine frame whose hash differed; the shadow keeps running.
    Mismatch(u32),
    /// The ROM ran more steps than its input ring holds between two reads; the shadow stopped.
    Lost(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RomStatus {
    /// The ROM file on disk now.
    pub rom: RomFile,
    /// The file changed on disk since the running ROM was loaded (rebuilt): `Reload`.
    pub rom_changed: bool,
    /// The running ROM's `engine::debug::boot` code.
    pub boot: Option<u32>,
    pub rom_build: Option<BuildHash>,
    pub native_build: BuildHash,
    pub run: RunMode,
    /// The last `TIMELINE_LEN` emulator frames, newest last (shared until it changes).
    pub timeline: Arc<[TimelineEntry]>,
    pub shadow: Shadow,
    /// The node's replay diverged from its recorded digest (`Materialized::diverged`).
    pub diverged: bool,
    pub error: Option<String>,
    /// The last exported `.sav`.
    pub exported: Option<PathBuf>,
}

/// Before the pane thread's first status: no ROM loaded (`pane::spawn` needs it).
impl Default for RomStatus {
    fn default() -> Self {
        RomStatus {
            rom: RomFile::Missing,
            rom_changed: false,
            boot: None,
            rom_build: None,
            native_build: BuildHash::default(),
            run: RunMode::Play,
            timeline: Arc::default(),
            shadow: Shadow::Off,
            diverged: false,
            error: None,
            exported: None,
        }
    }
}

impl RomStatus {
    /// The banner: the running ROM was built from other engine or content sources than this
    /// studio (`cargo xtask rom`).
    pub fn build_mismatch(&self) -> bool {
        self.rom_build.is_some_and(|b| b != self.native_build)
    }

    /// The worst values over the timeline.
    pub fn worst(&self) -> Worst {
        Worst::of(&self.timeline)
    }
}

/// The worst debug-region values over some timeline entries (the late counters are cumulative
/// on the ROM, so their maximum is the last value).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Worst {
    pub late: u32,
    pub late_black: u32,
    pub hash_lines_max: u32,
    pub step_lines_max: u32,
    pub loop_lines_max: u32,
    /// Entries whose native shadow hash differs.
    pub mismatches: u32,
}

impl Worst {
    pub fn of(entries: &[TimelineEntry]) -> Worst {
        entries.iter().fold(Worst::default(), |w, e| Worst {
            late: w.late.max(e.late),
            late_black: w.late_black.max(e.late_black),
            hash_lines_max: w.hash_lines_max.max(e.hash_lines_max),
            step_lines_max: w.step_lines_max.max(e.step_lines_max),
            loop_lines_max: w.loop_lines_max.max(e.loop_lines_max),
            mismatches: w.mismatches + u32::from(e.mismatch()),
        })
    }
}

/// The name of an `engine::debug::boot` code.
pub fn boot_name(code: u32) -> &'static str {
    match code {
        boot::NEW_GAME => "new game",
        boot::RESTORED => "restored",
        boot::SKIPPED => "save skipped (B held)",
        boot::REJECTED => "save rejected",
        boot::CORRUPT => "save corrupt",
        boot::NO_MEDIA => "no flash",
        _ => "unknown",
    }
}

/// Lives on its pane thread (the shadow `Session` is `!Send`).
pub struct RomPane {
    rom_path: PathBuf,
    scratch: PathBuf,
    store: Store,
    materializer: Arc<Mutex<Materializer<Emerald>>>,
    game: Emerald,
    native_build: BuildHash,
    emu: Option<Emu>,
    /// The ROM file on disk, probed every [`PROBE_TICKS`] ticks and on each reset.
    rom_file: RomFile,
    /// Ticks since the last probe.
    since_probe: u32,
    /// The ROM file's mtime when `emu` was loaded.
    loaded: Option<SystemTime>,
    source: Option<RomSource>,
    fb: Vec<u16>,
    run: RunMode,
    keys: Buttons,
    pending: u32,
    emu_frame: u32,
    timeline: VecDeque<TimelineEntry>,
    /// The timeline as `status` last returned it; `None` once it changed.
    shared_timeline: RefCell<Option<Arc<[TimelineEntry]>>>,
    boot: Option<u32>,
    rom_build: Option<BuildHash>,
    diverged: bool,
    error: Option<String>,
    exported: Option<PathBuf>,
    shadow: Option<Session<Emerald>>,
    shadow_state: Shadow,
}

impl RomPane {
    /// An idle pane on `rom_path` (normally `target/game.gba`); `ExportSave` writes only under
    /// `scratch_dir` (`target/studio/`). Nodes come from `store` (the layered store) through
    /// the materializer the native pane shares. Nothing runs until a `Reset`.
    pub fn new(
        rom_path: PathBuf,
        scratch_dir: PathBuf,
        store: Store,
        materializer: Arc<Mutex<Materializer<Emerald>>>,
    ) -> RomPane {
        let game = Emerald::for_shells();
        RomPane {
            rom_file: RomFile::probe(&rom_path),
            since_probe: 0,
            rom_path,
            scratch: scratch_dir,
            store,
            materializer,
            game,
            native_build: build_hash(&game),
            emu: None,
            loaded: None,
            source: None,
            fb: vec![0; SCREEN_W * SCREEN_H],
            run: RunMode::Play,
            keys: Buttons::NONE,
            pending: 0,
            emu_frame: 0,
            timeline: VecDeque::with_capacity(TIMELINE_LEN),
            shared_timeline: RefCell::new(None),
            boot: None,
            rom_build: None,
            diverged: false,
            error: None,
            exported: None,
            shadow: None,
            shadow_state: Shadow::Off,
        }
    }

    fn reset(&mut self, src: RomSource) {
        self.emu = None;
        self.loaded = None;
        self.shadow = None;
        self.shadow_state = Shadow::Off;
        self.timeline.clear();
        self.shared_timeline.take();
        self.emu_frame = 0;
        self.boot = None;
        self.rom_build = None;
        self.diverged = false;
        self.error = None;
        self.pending = 0;
        self.fb.fill(0);
        let result = self.open(&src);
        self.source = Some(src);
        if let Err(e) = result {
            self.emu = None;
            self.error = Some(format!("{e:#}"));
        }
    }

    fn open(&mut self, src: &RomSource) -> Result<()> {
        self.rom_file = RomFile::probe(&self.rom_path);
        let RomFile::Present { modified } = self.rom_file else {
            return Err(anyhow!(
                "no ROM at {} (build it: cargo xtask rom)",
                self.rom_path.display()
            ));
        };
        // The checkpoint the ROM should restore (for the shadow), and the flash image.
        let (cp, sav): (Option<Checkpoint>, Option<Vec<u8>>) = match src {
            RomSource::NewGame => (None, None),
            RomSource::Node(id) => {
                // One `get` under the lock (it replays only what no cache holds).
                let m = lock_materializer(&self.materializer)
                    .get(&self.store, *id)
                    .map_err(|e| anyhow!("materialize {id}: {e}"))?;
                self.diverged = m.diverged;
                let sav = game_native::flash::encode_sav(&m.checkpoint)
                    .map_err(|e| anyhow!("encode {id}: {e}"))?;
                (Some(m.checkpoint), Some(sav))
            }
            RomSource::Sav(sav) => (
                game_native::flash::decode_sav(sav).ok().map(|(_, cp)| cp),
                Some(sav.clone()),
            ),
        };
        let mut emu = Emu::open_with_save(&self.rom_path, sav.as_deref())?;
        let (snap, ran) = emu.boot_published(BOOT_FRAMES)?;
        self.loaded = modified;
        self.emu_frame = ran;
        self.on_boot(&snap, cp.as_ref());
        emu.framebuffer_into(&mut self.fb);
        self.emu = Some(emu);
        Ok(())
    }

    /// Records the ROM's boot state and starts the shadow where the ROM's boot can be
    /// mirrored: a new game, or a restore of `cp`.
    fn on_boot(&mut self, snap: &DebugSnapshot, cp: Option<&Checkpoint>) {
        self.boot = Some(snap.boot);
        self.rom_build = Some(snap.build);
        self.shadow = match (snap.boot, cp) {
            (boot::NEW_GAME, _) => Some(Session::new(self.game)),
            (boot::RESTORED, Some(cp)) => Session::restore(self.game, cp).ok(),
            _ => None,
        }
        .filter(|s| s.world().frame() == snap.frame && s.world().hash() == snap.hash);
        self.shadow_state = if self.shadow.is_some() {
            Shadow::Matching
        } else {
            Shadow::Off
        };
        let native = self.shadow.as_ref().map(|_| snap.hash);
        self.push(TimelineEntry::new(self.emu_frame, snap, native));
    }

    fn push(&mut self, e: TimelineEntry) {
        if self.timeline.len() == TIMELINE_LEN {
            self.timeline.pop_front();
        }
        self.timeline.push_back(e);
        self.shared_timeline.take();
    }

    /// Steps the shadow over the frames the ROM ran since the last sample, on the keys the
    /// ROM consumed (as `emu::run_rom_from` aligns them), and returns its hash.
    fn shadow_step(&mut self, snap: &DebugSnapshot) -> Option<u64> {
        let s = self.shadow.as_mut()?;
        let have = s.world().frame();
        if snap.frame < have || snap.frame - have > INPUT_RING as u32 {
            self.shadow = None;
            self.shadow_state = Shadow::Lost(snap.frame);
            return None;
        }
        for f in have + 1..=snap.frame {
            s.step(Buttons(snap.inputs[input_slot(f)]));
        }
        let h = s.world().hash();
        if h != snap.hash && self.shadow_state == Shadow::Matching {
            self.shadow_state = Shadow::Mismatch(snap.frame);
        }
        Some(h)
    }

    fn run_frames(&mut self, n: u32) {
        let Some(mut emu) = self.emu.take() else {
            return;
        };
        let result = (|| -> Result<()> {
            for _ in 0..n {
                emu.run_frame(self.keys)?;
                self.emu_frame += 1;
                let snap = emu
                    .debug()
                    .ok_or_else(|| anyhow!("debug region vanished"))?;
                let native = self.shadow_step(&snap);
                self.push(TimelineEntry::new(self.emu_frame, &snap, native));
            }
            Ok(())
        })();
        if n > 0 {
            emu.framebuffer_into(&mut self.fb);
        }
        match result {
            Ok(()) => self.emu = Some(emu),
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn export_save(&mut self) -> Result<PathBuf> {
        let emu = self.emu.as_mut().ok_or_else(|| anyhow!("no ROM running"))?;
        let data = emu.save_data()?;
        std::fs::create_dir_all(&self.scratch)
            .with_context(|| format!("create {}", self.scratch.display()))?;
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let mut path = self.scratch.join(format!("rom-{ms}.sav"));
        let mut n = 1;
        while path.exists() {
            path = self.scratch.join(format!("rom-{ms}-{n}.sav"));
            n += 1;
        }
        std::fs::write(&path, data).with_context(|| format!("write {}", path.display()))?;
        Ok(path)
    }
}

impl Pane for RomPane {
    type Cmd = RomCmd;
    type Status = RomStatus;

    fn handle(&mut self, cmd: RomCmd) {
        match cmd {
            RomCmd::Keys(k) => self.keys = k,
            RomCmd::Mode(m) => self.run = m,
            RomCmd::Step(n) => self.pending = self.pending.saturating_add(n),
            RomCmd::Reset(src) => self.reset(src),
            RomCmd::Reload => {
                if let Some(src) = self.source.clone() {
                    self.reset(src);
                }
            }
            RomCmd::ExportSave => match self.export_save() {
                Ok(p) => self.exported = Some(p),
                Err(e) => self.error = Some(format!("export: {e:#}")),
            },
        }
    }

    fn tick(&mut self) {
        let n = match self.run {
            RunMode::Play => 1,
            RunMode::Fast(k) => u32::from(k.clamp(2, 16)),
            RunMode::Pause => {
                let n = self.pending.min(MAX_STEPS_PER_TICK);
                self.pending -= n;
                n
            }
        };
        self.run_frames(n);
        self.since_probe += 1;
        if self.since_probe >= PROBE_TICKS {
            self.since_probe = 0;
            self.rom_file = RomFile::probe(&self.rom_path);
        }
    }

    fn framebuffer(&mut self) -> &[u16] {
        &self.fb
    }

    fn status(&self) -> RomStatus {
        let rom = self.rom_file;
        let rom_changed = self.emu.is_some()
            && match rom {
                RomFile::Present { modified } => modified != self.loaded,
                RomFile::Missing => true,
            };
        RomStatus {
            rom,
            rom_changed,
            boot: self.boot,
            rom_build: self.rom_build,
            native_build: self.native_build,
            run: self.run,
            timeline: (self.shared_timeline.borrow_mut())
                .get_or_insert_with(|| self.timeline.iter().copied().collect())
                .clone(),
            shadow: self.shadow_state,
            diverged: self.diverged,
            error: self.error.clone(),
            exported: self.exported.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("pkr-studio-rom-{tag}-{}-{n}", std::process::id()))
    }

    fn snap(frame: u32, hash: u64, build: BuildHash, boot: u32) -> DebugSnapshot {
        DebugSnapshot {
            frame,
            hash,
            inputs: [0; INPUT_RING],
            late: 0,
            late_black: 0,
            build,
            boot,
            saves: 0,
            save_status: 0,
            hash_lines_max: 0,
            step_lines_max: 0,
            loop_lines_max: 0,
            ..DebugSnapshot::default()
        }
    }

    /// A pane over an empty store (nothing is created until a node is added).
    fn pane(rom: PathBuf, scratch: PathBuf) -> RomPane {
        let store = Store::open(scratch.join("store")).unwrap();
        let m = Arc::new(Mutex::new(Materializer::new(Emerald::for_shells(), 4)));
        RomPane::new(rom, scratch, store, m)
    }

    /// A node reset materialises through the materializer it shares with the native pane, in
    /// the layered store (the local root only holds the snapshot cache).
    #[test]
    fn node_reset_goes_through_the_shared_materializer() {
        if content::STALE_EXTRACT {
            println!("skipped: {}", content::STALE_EXTRACT_MSG);
            return;
        }
        let dir = temp("node");
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("game.gba");
        std::fs::write(&rom, b"not a rom").unwrap();
        let store = Store::layered(Store::default_root(content::STUB), dir.join("local")).unwrap();
        let Some(node) = store.nodes().unwrap().into_iter().next() else {
            println!("skipped: no committed nodes");
            return;
        };
        let m = Arc::new(Mutex::new(Materializer::new(Emerald::for_shells(), 4)));
        let mut p = RomPane::new(rom, dir.join("studio"), store, Arc::clone(&m));
        p.handle(RomCmd::Reset(RomSource::Node(node.meta.id)));
        // Materialised (cached in the shared LRU), then refused by mGBA.
        assert!(p.status().error.is_some());
        assert!(!lock_materializer(&m).is_empty());
        // An unknown node is an error status too.
        p.handle(RomCmd::Reset(RomSource::Node(CheckpointId(9999))));
        assert!(p.status().error.unwrap().contains("materialize"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_rom_is_a_status_not_a_panic() {
        let dir = temp("missing");
        let mut p = pane(dir.join("game.gba"), dir.join("studio"));
        let s = p.status();
        assert_eq!(s.rom, RomFile::Missing);
        assert!(s.error.is_none() && !s.rom_changed && s.timeline.is_empty());
        p.handle(RomCmd::Reset(RomSource::NewGame));
        p.tick();
        let s = p.status();
        assert_eq!(s.rom, RomFile::Missing);
        assert!(s.error.as_deref().is_some_and(|e| e.contains("xtask rom")));
        assert!(s.boot.is_none() && s.timeline.is_empty());
        assert_eq!(p.framebuffer().len(), SCREEN_W * SCREEN_H);
        // Export without a running ROM is an error status too, and writes nothing.
        p.handle(RomCmd::ExportSave);
        assert!(p.status().error.unwrap().starts_with("export"));
        assert!(!dir.exists());
    }

    #[test]
    fn present_rom_file_is_probed() {
        let dir = temp("present");
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("game.gba");
        std::fs::write(&rom, b"not a rom").unwrap();
        assert!(matches!(RomFile::probe(&rom), RomFile::Present { .. }));
        assert_eq!(
            RomFile::probe(&dir),
            RomFile::Missing,
            "a directory is no ROM"
        );
        // A file mGBA rejects is an error status.
        let mut p = pane(rom, dir.join("studio"));
        p.handle(RomCmd::Reset(RomSource::NewGame));
        assert!(p.status().error.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn timeline_ring_keeps_the_newest_entries() {
        let mut p = pane(temp("ring").join("game.gba"), temp("ring"));
        for f in 0..TIMELINE_LEN as u32 + 50 {
            p.push(TimelineEntry::new(
                f,
                &snap(f, 1, BuildHash::default(), 0),
                None,
            ));
        }
        let t = p.status().timeline;
        assert_eq!(t.len(), TIMELINE_LEN);
        assert_eq!(t[0].frame, 50);
        assert_eq!(t.last().unwrap().frame, TIMELINE_LEN as u32 + 49);
        // Statuses share one copy until the timeline changes.
        assert!(Arc::ptr_eq(&t, &p.status().timeline));
        p.push(TimelineEntry::new(
            0,
            &snap(0, 1, BuildHash::default(), 0),
            None,
        ));
        let t2 = p.status().timeline;
        assert!(!Arc::ptr_eq(&t, &t2));
        assert_eq!(t2.last().unwrap().frame, 0);
    }

    #[test]
    fn the_rom_file_is_probed_about_once_a_second() {
        let dir = temp("probe");
        let rom = dir.join("game.gba");
        let mut p = pane(rom.clone(), dir.join("studio"));
        p.handle(RomCmd::Mode(RunMode::Pause));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&rom, b"x").unwrap();
        for _ in 1..PROBE_TICKS {
            p.tick();
        }
        assert_eq!(p.status().rom, RomFile::Missing, "not probed yet");
        p.tick();
        assert!(matches!(p.status().rom, RomFile::Present { .. }));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn build_mismatch_from_the_boot_snapshot() {
        let mut p = pane(temp("build").join("game.gba"), temp("build"));
        assert!(!p.status().build_mismatch(), "no ROM, no banner");
        let native = build_hash(&Emerald::for_shells());
        // A new-game boot of this build: no banner, and the shadow matches its boot state.
        let world = engine::World::new(&Emerald::for_shells());
        p.on_boot(
            &snap(world.frame(), world.hash(), native, boot::NEW_GAME),
            None,
        );
        let s = p.status();
        assert!(!s.build_mismatch());
        assert_eq!(s.shadow, Shadow::Matching);
        assert_eq!(s.timeline.len(), 1);
        assert!(!s.timeline[0].mismatch());
        // Another build: banner; another boot state: no shadow.
        let other = BuildHash {
            engine: native.engine ^ 1,
            content: native.content,
        };
        p.on_boot(&snap(0, 7, other, boot::NEW_GAME), None);
        let s = p.status();
        assert!(s.build_mismatch());
        assert_eq!(s.shadow, Shadow::Off);
        assert_eq!(s.timeline.last().unwrap().native_hash, None);
    }

    #[test]
    fn shadow_follows_the_consumed_keys_and_flags_a_mismatch() {
        let game = Emerald::for_shells();
        let mut p = pane(temp("shadow").join("game.gba"), temp("shadow"));
        let world = engine::World::new(&game);
        let start = world.frame();
        p.on_boot(
            &snap(start, world.hash(), build_hash(&game), boot::NEW_GAME),
            None,
        );
        // A straight native run gives the hashes the ROM would report.
        let mut native = Session::new(game);
        let mut s = snap(start, 0, build_hash(&game), boot::NEW_GAME);
        for i in 1..=3u32 {
            let f = start + i;
            let keys = Buttons(if i == 2 { 1 } else { 0 });
            native.step(keys);
            s.inputs[input_slot(f)] = keys.0;
            s.frame = f;
            s.hash = native.world().hash();
            assert_eq!(p.shadow_step(&s), Some(s.hash));
        }
        assert_eq!(p.shadow_state, Shadow::Matching);
        s.frame += 1;
        s.hash ^= 1;
        let h = p.shadow_step(&s);
        assert!(h.is_some_and(|h| h != s.hash));
        assert_eq!(p.shadow_state, Shadow::Mismatch(s.frame));
        // More steps than the input ring between two reads: the shadow is lost.
        s.frame += INPUT_RING as u32 + 1;
        assert_eq!(p.shadow_step(&s), None);
        assert_eq!(p.shadow_state, Shadow::Lost(s.frame));
    }

    #[test]
    fn worst_values_and_boot_names() {
        let build = BuildHash::default();
        let mut a = snap(1, 5, build, 0);
        a.late = 2;
        a.loop_lines_max = 200;
        let mut b = snap(2, 6, build, 0);
        b.hash_lines_max = 60;
        b.loop_lines_max = 190;
        let entries = [
            TimelineEntry::new(1, &a, Some(5)),
            TimelineEntry::new(2, &b, Some(7)),
        ];
        let w = Worst::of(&entries);
        assert_eq!(
            (w.late, w.hash_lines_max, w.loop_lines_max, w.mismatches),
            (2, 60, 200, 1)
        );
        assert_eq!(Worst::of(&[]), Worst::default());
        assert_eq!(boot_name(boot::RESTORED), "restored");
        assert_eq!(boot_name(99), "unknown");
    }

    #[test]
    fn pause_runs_pending_steps_only() {
        let mut p = pane(temp("pause").join("game.gba"), temp("pause"));
        p.handle(RomCmd::Mode(RunMode::Pause));
        p.handle(RomCmd::Step(MAX_STEPS_PER_TICK + 3));
        p.tick();
        assert_eq!(p.pending, 3);
        p.tick();
        assert_eq!(p.pending, 0);
        assert_eq!(p.status().run, RunMode::Pause);
    }

    // ---- With the ROM (`cargo xtask rom` first): `cargo test -p studio -- --ignored`. ----

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn rom_pane(tag: &str) -> (RomPane, PathBuf) {
        let scratch = temp(tag);
        let store =
            Store::layered(Store::default_root(content::STUB), scratch.join("local")).unwrap();
        let m = Arc::new(Mutex::new(Materializer::new(Emerald::for_shells(), 4)));
        (
            RomPane::new(
                workspace().join("target/game.gba"),
                scratch.clone(),
                store,
                m,
            ),
            scratch,
        )
    }

    #[test]
    #[ignore = "needs target/game.gba (cargo xtask rom)"]
    fn new_game_boots_and_runs_on_time() {
        let (mut p, scratch) = rom_pane("rom-new");
        p.handle(RomCmd::Reset(RomSource::NewGame));
        let s = p.status();
        assert_eq!(s.error, None);
        assert_eq!(s.boot, Some(boot::NEW_GAME));
        assert!(!s.build_mismatch(), "rebuild the ROM: cargo xtask rom");
        for _ in 0..TIMELINE_LEN {
            p.tick();
        }
        let s = p.status();
        assert_eq!(s.error, None);
        assert_eq!(s.timeline.len(), TIMELINE_LEN);
        assert!(
            s.timeline
                .windows(2)
                .all(|w| w[0].emu_frame < w[1].emu_frame)
        );
        assert!(s.timeline.windows(2).all(|w| w[0].frame <= w[1].frame));
        assert!(s.timeline.last().unwrap().frame > s.timeline[0].frame);
        assert!(s.timeline.iter().all(|e| e.late == 0));
        assert_eq!(
            s.shadow,
            Shadow::Matching,
            "the native shadow follows the ROM"
        );
        assert!(s.timeline.iter().all(|e| e.native_hash == Some(e.hash)));
        assert!(
            p.framebuffer().iter().any(|&c| c != 0),
            "the ROM shows something"
        );
        // Export lands in the scratch dir only.
        p.handle(RomCmd::ExportSave);
        let s = p.status();
        assert_eq!(s.error, None);
        let path = s.exported.unwrap();
        assert!(path.starts_with(&scratch));
        assert_eq!(
            std::fs::metadata(&path).unwrap().len() as usize,
            engine::checkpoint::FLASH_SIZE
        );
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// On its pane thread (as the studio runs it), a new game keeps publishing frames.
    #[test]
    #[ignore = "needs target/game.gba (cargo xtask rom)"]
    fn a_spawned_rom_pane_publishes_frames() {
        let scratch = temp("rom-spawn");
        let s = scratch.clone();
        let h = crate::pane::spawn("rom", move || {
            let store = Store::open(s.join("store")).unwrap();
            let m = Arc::new(Mutex::new(Materializer::new(Emerald::for_shells(), 4)));
            RomPane::new(workspace().join("target/game.gba"), s, store, m)
        });
        h.send(RomCmd::Reset(RomSource::NewGame));
        std::thread::sleep(std::time::Duration::from_secs(3));
        let s = h.status.get();
        let seq = h.frames.seq();
        // Walking changes the picture (an idle field may not).
        h.send(RomCmd::Keys(Buttons::LEFT));
        std::thread::sleep(std::time::Duration::from_secs(1));
        let more = h.frames.seq() - seq;
        let s2 = h.status.get();
        h.stop();
        println!(
            "error {:?} boot {:?} timeline {} -> {} frames published in 1 s: {more}",
            s.error,
            s.boot,
            s.timeline.last().map(|e| e.frame).unwrap_or(0),
            s2.timeline.last().map(|e| e.frame).unwrap_or(0),
        );
        assert_eq!(s.error, None);
        assert!(
            s2.timeline.last().unwrap().frame > s.timeline.last().unwrap().frame + 30,
            "the ROM runs"
        );
        assert!(more > 30, "walking publishes frames: {more}");
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    #[ignore = "needs target/game.gba (cargo xtask rom)"]
    /// Runs on the committed tree layered under a temp local root: the committed tree is never
    /// written (a warm checkout reads its git-ignored snapshot cache and writes nothing).
    fn node_boots_restored_at_the_node_frame() {
        let (mut p, scratch) = rom_pane("rom-node");
        // The committed tree's git-ignored files: names and modification times.
        let base = Store::default_root(content::STUB);
        let written = || {
            let mut files: Vec<_> = std::fs::read_dir(base.join("snapshots"))
                .into_iter()
                .flatten()
                .map(|e| e.unwrap().path())
                .chain([base.join("status.ron")])
                .filter_map(|f| Some((std::fs::metadata(&f).ok()?.modified().ok()?, f)))
                .collect();
            files.sort();
            files
        };
        let before = written();
        let node = p.store.nodes().unwrap().into_iter().next().expect("a node");
        p.handle(RomCmd::Reset(RomSource::Node(node.meta.id)));
        let s = p.status();
        assert_eq!(s.error, None);
        assert_eq!(s.boot, Some(boot::RESTORED));
        assert_eq!(s.timeline[0].frame, node.meta.frame);
        assert!(!s.diverged);
        for _ in 0..120 {
            p.tick();
        }
        let s = p.status();
        assert_eq!(s.error, None);
        assert_eq!(s.shadow, Shadow::Matching);
        // Reload re-boots the same node.
        p.handle(RomCmd::Reload);
        let s = p.status();
        assert_eq!(s.boot, Some(boot::RESTORED));
        assert_eq!(s.timeline.len(), 1);
        assert_eq!(written(), before, "the committed tree was written");
        // A warm checkout wrote nothing there either.
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
