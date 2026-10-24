//! Milestone 12: the sound blob (`$OUT_DIR/sound.bin`, `sound::blob`) from the extract's
//! `sound/`, and the tables the World reads (`$OUT_DIR/sound.rs`): each song's player,
//! priority and length, the fanfares, the species' cry ids, each cry's length by mode (the
//! song and cry lengths from dry runs of the real engine, `sound::dry`), the maps' music, the
//! trainers' battle and victory music, and the song names (Silice's `play_se("SE_*")`).
//!
//! Silice (`games/silice/sound.ron`): its `MUS_SIL_*` songs and invented `SE_*` names play
//! the extracted Emerald song the table gives, until the author's tracks land.
//!
//! Without an extract's `sound/` (the stub, test fixtures) the blob and tables are empty:
//! nothing plays and nothing waits.

use crate::input::World;
use anyhow::{Context, Result, bail};
use extract::export::read_ron;
use extract::games::{IdEntry, IdsFile, SoundFile};
use extract::sound::blob::SoundDir;
use extract::text::charmap::SOUNDS;
use sound::{Blob, Cry, Player, dry};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

pub struct SoundOut {
    pub bin: Vec<u8>,
    pub rs: String,
}

/// Gen3 `SPECIES_CELEBI`, `SPECIES_UNOWN`, `SPECIES_TREECKO` (`SpeciesToCryId`).
const CELEBI: usize = 251;
const UNOWN: usize = 201;
const TREECKO: usize = 277;
/// `CRY_MODE_*` count, `CRY_VOLUME`, `CRY_PRIORITY_NORMAL`.
const CRY_MODES: u8 = 13;

pub fn generate(world: &World, silice: Option<&Path>) -> Result<SoundOut> {
    let dir = world.dir.as_ref().map(|d| d.join("sound"));
    let Some(dir) = dir.filter(|d| d.join("songs.ron").exists()) else {
        return Ok(SoundOut {
            bin: Vec::new(),
            rs: empty(),
        });
    };
    let sd = SoundDir::read(&dir)?;
    let bin = sd.blob()?;
    let blob = Blob::new(&bin).context("the sound blob's header")?;
    let names: BTreeMap<&str, u16> = SOUNDS.iter().map(|&(v, n)| (n, v)).collect();
    let song = |n: &str| -> Result<u16> {
        names
            .get(n)
            .copied()
            .with_context(|| format!("no Emerald song {n}"))
    };

    // Songs: player, priority, frames.
    let mut rs = String::new();
    writeln!(
        rs,
        "pub static SONGS: [::engine::audio::SongInfo; {}] = [",
        sd.songs.songs.len()
    )?;
    for (i, s) in sd.songs.songs.iter().enumerate() {
        let (priority, frames) = match &s.song {
            None => (0, 0),
            Some(f) => {
                let p = match s.player {
                    0 => Player::Bgm,
                    1 => Player::Se1,
                    2 => Player::Se2,
                    _ => Player::Se3,
                };
                let n = dry::song_frames(blob, i as u16, p);
                let n = if n >= dry::LIMIT { u16::MAX } else { n as u16 };
                (sd.song_files[f].priority, n)
            }
        };
        writeln!(
            rs,
            "    ::engine::audio::SongInfo {{ player: {}, priority: {priority}, frames: {frames} }},",
            s.player
        )?;
    }
    writeln!(rs, "];")?;

    let tables: extract::schema::SoundTablesFile = read_ron(&dir.join("tables.ron"))?;
    let fanfares: Vec<String> = (tables.fanfares.iter())
        .map(|(n, f)| Ok(format!("({}, {f})", song(n)?)))
        .collect::<Result<_>>()?;
    writeln!(
        rs,
        "pub static FANFARES: [(u16, u16); {}] = [{}];",
        fanfares.len(),
        fanfares.join(", ")
    )?;

    // `SpeciesToCryId(species - 1)` by species number.
    let n_species = (TREECKO + tables.cry_ids.len()).max(world.battle.species.len());
    let cry_of = |species: usize| -> u16 {
        let Some(x) = species.checked_sub(1) else {
            return 0;
        };
        if x < CELEBI {
            x as u16
        } else if x < TREECKO - 1 {
            (UNOWN - 1) as u16
        } else {
            tables.cry_ids.get(x - (TREECKO - 1)).copied().unwrap_or(0)
        }
    };
    let cries: Vec<String> = (0..n_species).map(|s| cry_of(s).to_string()).collect();
    writeln!(
        rs,
        "pub static SPECIES_CRY: [u16; {}] = [{}];",
        cries.len(),
        cries.join(", ")
    )?;
    let count = blob.cry_count() as u16;
    writeln!(
        rs,
        "pub static CRY_FRAMES: [[u16; {CRY_MODES}]; {count}] = ["
    )?;
    for id in 0..count {
        let frames: Vec<String> = (0..CRY_MODES)
            .map(|mode| {
                let c = Cry::for_mode(id, mode, 0, 120, 10);
                dry::cry_frames(blob, c)
                    .min(u16::MAX as u32 - 1)
                    .to_string()
            })
            .collect();
        writeln!(rs, "    [{}],", frames.join(", "))?;
    }
    writeln!(rs, "];")?;

    // Silice's songs and effects.
    let sil: SoundFile = match silice {
        Some(g) => read_ron(&g.join("sound.ron"))?,
        None => SoundFile::default(),
    };
    // Song names (Gen3's, then Silice's effects as the songs they play), for `song_named`.
    let mut by_name: BTreeMap<String, u16> =
        names.iter().map(|(n, v)| (n.to_string(), *v)).collect();
    for (n, to) in &sil.effects {
        if by_name.contains_key(n) {
            bail!("games/silice/sound.ron: {n} is an Emerald sound already");
        }
        by_name.insert(n.clone(), song(to)?);
    }
    // Silice's numbered songs (`ids/songs.ron`, from 610) play their table's Emerald song.
    let mut sil_songs: BTreeMap<u16, u16> = BTreeMap::new();
    if let Some(g) = silice {
        let ids: IdsFile = read_ron(&g.join("ids/songs.ron"))?;
        for IdEntry {
            name, num, removed, ..
        } in ids.ids
        {
            if removed {
                continue;
            }
            let num = num as u16;
            let to = sil
                .songs
                .get(&name)
                .with_context(|| format!("games/silice/sound.ron: no Emerald song for {name}"))?;
            sil_songs.insert(num, song(to)?);
            by_name.insert(name, num);
        }
        if let Some(n) = sil.songs.keys().find(|n| !by_name.contains_key(*n)) {
            bail!("games/silice/sound.ron: {n} is not in ids/songs.ron");
        }
    }
    let resolve = |n: u16| sil_songs.get(&n).copied().unwrap_or(n);
    writeln!(
        rs,
        "pub static SONG_NAMES: [(&str, u16); {}] = [",
        by_name.len()
    )?;
    for (n, v) in &by_name {
        writeln!(rs, "    ({n:?}, {}),", resolve(*v))?;
    }
    writeln!(rs, "];")?;

    // Map music by map id (`MUS_NONE` 0xFFFF kept).
    let mut maps: Vec<(u16, u16)> = (world.maps.iter())
        .map(|m| {
            let id = (m.group as u16) << 8 | m.num as u16;
            let music = m.header.music;
            (
                id,
                if music >= 0x7FFF {
                    music
                } else {
                    resolve(music)
                },
            )
        })
        .collect();
    maps.sort();
    maps.dedup_by_key(|m| m.0);
    writeln!(
        rs,
        "pub static ROUTE118_HALVES: (u16, u16) = ({}, {});",
        song("MUS_ROUTE110")?,
        song("MUS_ROUTE119")?
    )?;
    writeln!(rs, "pub static MAP_MUSIC: [(u16, u16); {}] = [", maps.len())?;
    for (id, m) in &maps {
        writeln!(rs, "    ({id:#06X}, {m}),")?;
    }
    writeln!(rs, "];")?;

    // Trainers: `GetBattleBGM` and the victory music of `HandleEndTurn_BattleWon`.
    writeln!(
        rs,
        "pub static TRAINER_MUSIC: [(u16, u16); {}] = [",
        world.battle.trainers.len()
    )?;
    for t in &world.battle.trainers {
        let class = t.class.strip_prefix("TRAINER_CLASS_").unwrap_or(&t.class);
        let vs = match class {
            "AQUA_LEADER" | "MAGMA_LEADER" => "MUS_VS_AQUA_MAGMA_LEADER",
            "TEAM_AQUA" | "TEAM_MAGMA" | "AQUA_ADMIN" | "MAGMA_ADMIN" => "MUS_VS_AQUA_MAGMA",
            "LEADER" => "MUS_VS_GYM_LEADER",
            "CHAMPION" => "MUS_VS_CHAMPION",
            // `gText_BattleWallyName`: Wally fights to the trainer music.
            "RIVAL" if t.id.starts_with("TRAINER_WALLY") => "MUS_VS_TRAINER",
            "RIVAL" => "MUS_VS_RIVAL",
            "ELITE_FOUR" => "MUS_VS_ELITE_FOUR",
            "SALON_MAIDEN" | "DOME_ACE" | "PALACE_MAVEN" | "ARENA_TYCOON" | "FACTORY_HEAD"
            | "PIKE_QUEEN" | "PYRAMID_KING" => "MUS_VS_FRONTIER_BRAIN",
            _ => "MUS_VS_TRAINER",
        };
        let win = match class {
            "ELITE_FOUR" | "CHAMPION" => "MUS_VICTORY_LEAGUE",
            "TEAM_AQUA" | "TEAM_MAGMA" | "AQUA_ADMIN" | "AQUA_LEADER" | "MAGMA_ADMIN"
            | "MAGMA_LEADER" => "MUS_VICTORY_AQUA_MAGMA",
            "LEADER" => "MUS_VICTORY_GYM_LEADER",
            _ => "MUS_VICTORY_TRAINER",
        };
        writeln!(rs, "    ({}, {}),", song(vs)?, song(win)?)?;
    }
    writeln!(rs, "];")?;

    writeln!(
        rs,
        "pub static AUDIO: ::engine::audio::AudioData = ::engine::audio::AudioData {{\n    \
         blob: &BLOB.0,\n    songs: &SONGS,\n    fanfares: &FANFARES,\n    species_cry: &SPECIES_CRY,\n    \
         cry_frames: &CRY_FRAMES,\n    ids: ::engine::audio::AudioIds {{ vs_wild: {}, victory_wild: {}, \
         low_health: {}, surf: {}, underwater: {}, abnormal_weather: {} }},\n}};",
        song("MUS_VS_WILD")?,
        song("MUS_VICTORY_WILD")?,
        song("SE_LOW_HEALTH")?,
        song("MUS_SURF")?,
        song("MUS_UNDERWATER")?,
        song("MUS_ABNORMAL_WEATHER")?,
    )?;
    Ok(SoundOut {
        rs: format!("{}{rs}", blob_decl(bin.len())),
        bin,
    })
}

fn blob_decl(len: usize) -> String {
    format!(
        "/// The sound blob (`sound::blob`), 4-aligned.\npub static BLOB: crate::blob::Blob<{len}> = \
         crate::blob::Blob(*include_bytes!(concat!(env!(\"OUT_DIR\"), \"/sound.bin\")));\n"
    )
}

/// No sound: empty tables.
fn empty() -> String {
    format!(
        "{}pub static SONG_NAMES: [(&str, u16); 0] = [];\npub static MAP_MUSIC: [(u16, u16); 0] = [];\n\
         pub static ROUTE118_HALVES: (u16, u16) = (0, 0);\n\
         pub static TRAINER_MUSIC: [(u16, u16); 0] = [];\n\
         pub static AUDIO: ::engine::audio::AudioData = ::engine::audio::EMPTY_AUDIO;\n",
        blob_decl(0)
    )
}
