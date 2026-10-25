//! What the game asks of the sound engine: Gen3's `m4a*` calls and the cry setup of
//! `PlayCryInternal`, one [`Cmd`] each, in the order the game made them within a frame.

/// `gMPlayTable`'s players.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Player {
    Bgm = 0,
    Se1 = 1,
    Se2 = 2,
    Se3 = 3,
}

/// One Pokémon cry (`PlayCryInternal`'s `SetPokemonCry*` values, then `SetPokemonCryTone`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cry {
    /// The `gCryTable` index (`SpeciesToCryId(species - 1)`).
    pub index: u16,
    /// `gCryTable_Reverse` instead.
    pub reverse: bool,
    pub volume: u8,
    pub pan: i8,
    /// `SetPokemonCryPitch` (15,360 by default).
    pub pitch: i16,
    /// `SetPokemonCryLength`, in frames.
    pub length: u16,
    pub release: u8,
    pub chorus: i8,
    pub priority: u8,
}

impl Cry {
    /// `PlayCryInternal`'s values for `CRY_MODE_*` `mode` (`src/sound.c`), at `volume` and `pan`.
    pub fn for_mode(index: u16, mode: u8, pan: i8, volume: u8, priority: u8) -> Cry {
        let mut c = Cry {
            index,
            reverse: false,
            volume,
            pan,
            pitch: 15360,
            length: 140,
            release: 0,
            chorus: 0,
            priority,
        };
        let mut set = |length: u16, reverse: bool, release: u8, pitch: i16, chorus: i8| {
            c.length = length;
            c.reverse = reverse;
            c.release = release;
            c.pitch = pitch;
            c.chorus = chorus;
        };
        match mode {
            1 => set(20, false, 225, 15360, 0),    // CRY_MODE_DOUBLES
            2 => set(140, false, 225, 15600, 20),  // CRY_MODE_ENCOUNTER
            3 => set(50, false, 200, 15800, 20),   // CRY_MODE_HIGH_PITCH
            4 => set(25, true, 100, 15600, -64),   // CRY_MODE_ECHO_START (chorus 192)
            5 => set(140, false, 200, 14440, 0),   // CRY_MODE_FAINT
            6 => set(140, false, 220, 15555, -64), // CRY_MODE_ECHO_END
            7 => set(10, false, 100, 14848, 0),    // CRY_MODE_ROAR_1
            8 => set(60, false, 225, 15616, 0),    // CRY_MODE_ROAR_2
            9 => set(15, true, 125, 15200, 0),     // CRY_MODE_GROWL_1
            10 => set(100, false, 225, 15200, 0),  // CRY_MODE_GROWL_2
            11 => set(140, false, 0, 15000, 0),    // CRY_MODE_WEAK
            12 => set(20, false, 225, 15000, 0),   // CRY_MODE_WEAK_DOUBLES
            _ => {}
        }
        // ENCOUNTER, HIGH_PITCH, ECHO_START and ECHO_END force their volume.
        match mode {
            2..=4 => c.volume = 90,
            6 => c.volume = 70,
            _ => {}
        }
        c
    }
}

/// One `m4a*` call (see `src/m4a.c` and `src/sound.c`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    /// `m4aSongNumStart`.
    Start(u16),
    /// `m4aSongNumStartOrChange`.
    StartOrChange(u16),
    /// `m4aSongNumStop`: stops the song's player if it plays that song.
    StopSong(u16),
    /// `m4aMPlayStop`.
    Stop(Player),
    /// `m4aMPlayContinue`.
    Continue(Player),
    /// `m4aMPlayAllStop` (the cry players too).
    StopAll,
    /// `m4aMPlayFadeOut`.
    FadeOut(Player, u16),
    /// `m4aMPlayFadeOutTemporarily`.
    FadeOutTemporarily(Player, u16),
    /// `m4aMPlayFadeIn`.
    FadeIn(Player, u16),
    /// `m4aMPlayImmInit`.
    ImmInit(Player),
    /// `m4aMPlayVolumeControl(player, TRACKS_ALL, volume)`.
    Volume(Player, u16),
    /// `m4aMPlayPanpotControl(player, TRACKS_ALL, pan)`.
    Pan(Player, i8),
    /// `PlayCryInternal`.
    Cry(Cry),
    /// `m4aMPlayStop(gMPlay_PokemonCry)`.
    StopCry,
    /// `SetPokemonCryStereo`: the DirectSound and PSG panning (Emerald's option, mono by
    /// default).
    Stereo(bool),
}
