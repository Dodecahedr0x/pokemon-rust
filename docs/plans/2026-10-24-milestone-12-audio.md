# Milestone 12 — Audio

**Goal (user, 2026-10-24):** "audio is needed; test with the extract, I'll provide MIDI tracks
at some point (reuse extracted for now)". Gen3's music, sound effects, fanfares and cries play
natively and on the GBA from the extracted songs; scripts wait on them as Gen3 does; Silice's own
songs play an Emerald stand-in until the author's MIDI files land, then the files themselves.

## As built

| Part | Where | What |
|---|---|---|
| 1. Extract | `crates/extract/src/sound/` | the song table, song headers and MP2K tracks (decoded to `SongCmd`), voicegroups, keysplits, programmable waves, samples (PCM `.wav`), the fanfare and cry tables, under `sound/` (`FORMAT_MINOR` 14). US and FR oracle tests against the decomp (`tests/sound.rs`). |
| Blob | `extract::sound::blob`, `sound::blob` | the runtime "mini-ROM": song table, voices, keysplits, waves, samples, songs re-encoded with blob offsets (4.86 MB for Emerald). |
| 2. World model | `engine::audio`, `world/audio.rs`, `world/host/audio.rs` | per m4a player `{song, frames left, paused, priority}`, the fanfare task, the cry duck, `MapMusicMain`'s state machine and the low-HP flag; no sequencer in the World. Hashed; checkpoint schema 8 stores it unless it is what the map implies. |
| 3. Sequencer | `crates/sound` (`no_std`) | the MP2K engine ported from `m4a.c`/`m4a_1.s` (players, tracks, `ply_*`, DirectSound envelopes, `CgbSound`), dry runs, the software synth (13,379 Hz mixer with reverb, a PSG emulation). |
| Native | `game-native/src/audio.rs` | `play` (feature `window`) renders the synth on cpal's default device; `--mute`. |
| GBA | `game-gba/src/audio.rs` | the engine each frame, PSG on the hardware registers, the five DirectSound channels on agb's mixer at 10,512 Hz. |
| 4. Scripts | `extract/src/codegen/lower/emit.rs`, `prelude/items.rs`, `silice/prelude.rs` | `playse`, `waitse`, `playfanfare`, `waitfanfare`, `playbgm`, `savebgm`, `fadedefaultbgm`, `fadenewbgm`, `fadeoutbgm`, `fadeinbgm`, `playmoncry`, `waitmoncry` are native; item fanfares (`MUS_OBTAIN_ITEM`/`TMHM`), B-points; Silice `play_se`/`play_music`/`play_fanfare` by name. |
| Silice mapping | `games/silice/sound.ron` | each `MUS_SIL_*` and invented `SE_*` -> an Emerald song. |
| MIDI import | `extract::sound::midi`, `content/build/sound.rs` | `games/silice/music/<MUS_SIL_*>.mid` -> a song of its own (see below). |

### Timing (what the World waits on)

The World never runs the sequencer. Song lengths come from dry runs of the real engine at build
time (`sound::dry`): `SongInfo.frames` is the number of `SoundMain` calls until the player's
tracks end (`u16::MAX`: it loops); `CRY_FRAMES[cry][mode]` likewise for each cry mode. Each frame
`Audio::tick` (end of `World::step`) runs, in Gen3's order, `Task_Fanfare`,
`Task_DuckBGMForPokemonCry`, `MapMusicMain`, then `SoundMain`'s countdown. A script that starts a
sound on frame N therefore resumes as Gen3's does:

- `waitse`: N + the SE's length;
- `waitfanfare`: N + `sFanfares[i].duration` + 1 (the fanfare table, not the song);
- `waitmoncry`: N + max(2, the cry's length) + 1;
- `fadeoutbgm speed`: 16 × 4·speed frames, then paused.

`audio_tests.rs` pins these against hand-counted Gen3 frame traces.

### Music the engine plays by itself

- Map music on load (`Overworld_PlaySpecialMapMusic`: the saved song, surfing, underwater) and on
  a connection crossing (`TransitionMapMusic`), Route 118's two halves (`MUS_ROUTE118`, x < 24).
- On a warp, the music fades out if the destination's differs (`WarpFadeOutScreen`'s
  `FadeOutMapMusic`, speed 2 indoors / 4 outdoors).
- Battles: `MUS_VS_WILD` or the trainer class's `GetBattleBGM`; the victory music on the win
  (`HandleEndTurn_BattleWon`, Wally gets the trainer one); `SE_LOW_HEALTH` while the player's mon
  is at or below 9/48 of its HP; the field music back after the battle and after a whiteout.

## Budgets (GBA, Emerald)

- ROM: 14,796,768 bytes (of 16 MiB gated); the sound blob is 4.86 MB of it.
- IWRAM: 2,736 bytes of the 2,836 gated (agb's mixer code, `mixer.s`, is linked into IWRAM);
  100 bytes left.
- Scanlines: see "Parity" below (loop ≤ 217, hash ≤ 65 lines).

## The MIDI path (author tracks)

Drop a standard MIDI file named after its song into `games/silice/music/`, e.g.
`games/silice/music/MUS_SIL_FIN.mid`. The content build (Silice flavour) converts it with
`extract::sound::midi::convert`, appends it to the song table as a BGM, and points `MUS_SIL_FIN`
at it instead of its stand-in. Nothing else changes: maps and `play_music("MUS_SIL_FIN")` play it.

The conversion follows `mid2agb` the way Emerald builds songs (`-E`, exact gate time):
- 24 MP2K clocks per quarter note, ticks truncated; tempo meta events -> `TEMPO` (bpm / 2) on the
  first track. Tempo and `[` / `]` are read from the first MIDI track only;
- one MP2K track per MIDI channel that plays notes (at most 10, the BGM player's tracks);
- program change -> `VOICE`; CC 7 `VOL`, CC 10 `PAN`, CC 1 `MOD`, CC 20 `BENDR`, CC 21 `LFOS`,
  CC 22 `MODT`, CC 24 `TUNE`, CC 26 `LFODL`, CC 30 then CC 29 -> `xIECV` / `xIECL`; the pitch
  wheel `BEND` (its high 7 bits). Note velocity goes through mid2agb's velocity table. A track
  with no volume before its first note starts at `VOL 127`;
- notes up to 96 clocks are one `Nxx` (+1-3 gate ticks), longer ones `TIE` ... `EOT`;
- markers `[` and `]` loop the song (`GOTO`); without them it ends.

The voicegroup is the stand-in's (`sound.ron`), so the author picks instruments by General-MIDI-
like program numbers within that Emerald voicegroup. To use another voicegroup, change the
stand-in. Not handled yet (as in `mid2agb` without its `midi.cfg` options): `-V` master volume,
`-R` reverb, `-P` priority, drum-kit channel mapping beyond the voicegroup's own keysplits, and
sound effects (only `MUS_*` tracks are imported). `content/tests/m12_audio.rs` checks that every
file present becomes its own song and makes a sound.

## Deviations from Gen3 (known, accepted)

- The warp does not hold until the fade-out ends; the fade starts with the warp.
- Route 111's sandstorm music, trainer encounter ("eye") music and the Pokémon Center's heal
  jingle are not played.
- The victory music starts on the win frame for wild battles too (Gen3 starts it from the battle
  end script).
- The low-HP beep follows the battle state's HP, not the displayed bar's.
- Cry lengths are the dry run's (sample end or `FINE`), Gen3's real cry ends when the BGM's next
  note steals the channel.
- GBA: agb's mixer sets volumes in sixteenths (Gen3: 1/256), plays reversed samples forwards and
  has no reverb; the native synth does both.
- Native: the sound engine follows the audio device's clock, not the game's (at most a callback
  buffer apart).

## Parity

`cargo xtask parity` and `parity --long`: see the merge report for the measured loop and hash
lines. The sound waits move the Emerald spine: badge 1 at frame 105,865 (was 104,523), the Hall
of Fame at 3,234,311 (was 3,167,220). The spine gained two Fortree training rounds and ten Great
Balls (Fortree Mart) as the RNG moved.

## Left

- Trainer encounter music, the Center heal jingle, Route 111's sandstorm music.
- GBA reverb and reversed cries (a custom mixer in IWRAM, as Gen3's `SoundMainRAM`).
- MIDI: SE import, drum maps, per-song voicegroup override in `sound.ron`.
