// audio.rs — ProTracker MOD playback + 8-bit PCM sound effects
// Language: Rust, file: src/audio.rs

use anyhow::Result;
use sdl2::mixer::{Chunk, InitFlag};

// SDL2_mixer handles ProTracker MOD natively when compiled with libmikmod / libmodplug.
// We load each tim file as a Music object and switch on level change.
// PCM noises are loaded as Chunks and fired on the 8 dedicated effect channels.

pub const SFX_CHANNELS:   i32 = 8;
pub const MUSIC_VOLUME:   i32 = 64;  // 0–128
pub const SFX_VOLUME:     i32 = 100;

// Sound effect indices (order matches noises file layout from ADF)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    LemmingWalk    = 0,
    LemmingFall    = 1,
    LemmingDigDown = 2,
    LemmingBash    = 3,
    LemmingMine    = 4,
    LemmingBuild   = 5,
    LemmingBlocker = 6,
    LemmingClimb   = 7,
    LemmingSplat   = 8,
    LemmingOhNo    = 9,
    LemmingExplode = 10,
    LemmingExit    = 11,
    DoorOpen       = 12,
    Nuke           = 13,
    Trap           = 14,
    Drown          = 15,
}

// 8-bit signed PCM at 11025 Hz (Amiga Paula) — SDL2_mixer expects 22050 Hz signed 8-bit
fn resample_8bit_11025_to_22050(src: &[u8]) -> Vec<u8> {
    // Linear upsample ×2 with simple lerp
    let mut out = Vec::with_capacity(src.len() * 2);
    for i in 0..src.len() {
        out.push(src[i]);
        let next = if i + 1 < src.len() { src[i + 1] } else { src[i] };
        let a    = src[i]  as i16;
        let b    = next    as i16;
        let mid  = ((a + b) / 2) as u8;
        out.push(mid);
    }
    out
}

pub struct AudioEngine {
    sfx:    Vec<Option<Chunk>>,   // indexed by Sfx variant
    tracks: Vec<Option<String>>,  // temp-file paths for Music (SDL2_mixer loads from file)
    _tracker: (),
}

impl AudioEngine {
    /// Build the audio engine from the raw ADF file data.
    /// `noises_data` = decompressed noises file (raw signed-8-bit PCM blocks)
    /// `track_data`  = vec of (track_index, raw_mod_bytes)
    pub fn new(noises_data: &[u8], track_data: Vec<(usize, Vec<u8>)>) -> Result<Self> {
        sdl2::mixer::init(InitFlag::MOD | InitFlag::MP3)
            .map_err(|e| anyhow::anyhow!("SDL2_mixer init: {}", e))?;

        // ── Sound effects ────────────────────────────────────────────────────
        // The noises file is a sequence of raw 8-bit signed PCM samples.
        // Each sound is stored with a 4-byte big-endian length prefix, then samples.
        let sfx = parse_noises(noises_data)?;

        // ── Music ────────────────────────────────────────────────────────────
        // SDL2_mixer Music objects need files on disk in practice with the C API.
        // We write each MOD to a temp file and record the path.
        let mut tracks = Vec::with_capacity(10);
        let temp_dir = std::env::temp_dir().join("lemmings_mods");
        std::fs::create_dir_all(&temp_dir)?;

        for (idx, mod_bytes) in track_data {
            let path = temp_dir.join(format!("tim{}.mod", idx));
            std::fs::write(&path, &mod_bytes)?;
            // Pad vec to accommodate sparse indices
            while tracks.len() <= idx {
                tracks.push(None);
            }
            tracks[idx] = Some(path.to_string_lossy().to_string());
        }

        sdl2::mixer::Music::set_volume(MUSIC_VOLUME);
        sdl2::mixer::Channel::all().set_volume(SFX_VOLUME);

        Ok(Self { sfx, tracks, _tracker: () })
    }

    /// Play a music track (1-based index, matching Amiga level header).
    pub fn play_music(&self, track: u8) {
        let idx = (track as usize).saturating_sub(1);
        if let Some(Some(path)) = self.tracks.get(idx) {
            match sdl2::mixer::Music::from_file(path) {
                Ok(music) => { let _ = music.play(-1); }
                Err(e)    => log::warn!("Could not load music track {}: {}", track, e),
            }
        }
    }

    pub fn stop_music(&self) {
        sdl2::mixer::Music::halt();
    }

    /// Trigger a one-shot SFX on the next free channel.
    pub fn play_sfx(&self, sfx: Sfx) {
        let idx = sfx as usize;
        if let Some(Some(chunk)) = self.sfx.get(idx) {
            let _ = sdl2::mixer::Channel::all().play(chunk, 0);
        }
    }

    pub fn is_music_playing(&self) -> bool {
        sdl2::mixer::Music::is_playing()
    }
}

/// Parse the Amiga noises file into SDL2 Chunks.
/// Format: repeated blocks of  [u32_be length][i8 samples × length]
fn parse_noises(data: &[u8]) -> Result<Vec<Option<Chunk>>> {
    let mut out: Vec<Option<Chunk>> = Vec::new();
    let mut pos = 0;

    while pos + 4 <= data.len() {
        let len = u32::from_be_bytes([
            data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
        ]) as usize;
        pos += 4;

        if len == 0 || pos + len > data.len() {
            out.push(None);
            break;
        }

        let samples = resample_8bit_11025_to_22050(&data[pos..pos + len]);
        pos += len;

        // Build a WAV in memory: 8-bit signed, mono, 22050 Hz
        let wav = build_wav_header(samples.len(), 22050, 1, 8);
        let mut full_wav = wav;
        full_wav.extend_from_slice(&samples);

        match Chunk::from_raw_buffer(full_wav.into_boxed_slice()) {
            Ok(chunk) => out.push(Some(chunk)),
            Err(e) => {
                log::warn!("SFX chunk {} load error: {}", out.len(), e);
                out.push(None);
            }
        }
    }

    Ok(out)
}

/// Minimal RIFF WAV header for PCM audio in memory.
fn build_wav_header(samples: usize, rate: u32, channels: u16, bits: u16) -> Vec<u8> {
    let byte_rate    = rate * channels as u32 * (bits as u32 / 8);
    let block_align  = channels * (bits / 8);
    let data_size    = samples as u32;
    let chunk_size   = 36 + data_size;

    let mut hdr = Vec::with_capacity(44);
    hdr.extend_from_slice(b"RIFF");
    hdr.extend_from_slice(&chunk_size.to_le_bytes());
    hdr.extend_from_slice(b"WAVE");
    hdr.extend_from_slice(b"fmt ");
    hdr.extend_from_slice(&16u32.to_le_bytes());     // PCM subchunk size
    hdr.extend_from_slice(&1u16.to_le_bytes());      // PCM format
    hdr.extend_from_slice(&channels.to_le_bytes());
    hdr.extend_from_slice(&rate.to_le_bytes());
    hdr.extend_from_slice(&byte_rate.to_le_bytes());
    hdr.extend_from_slice(&block_align.to_le_bytes());
    hdr.extend_from_slice(&bits.to_le_bytes());
    hdr.extend_from_slice(b"data");
    hdr.extend_from_slice(&data_size.to_le_bytes());
    hdr
}
