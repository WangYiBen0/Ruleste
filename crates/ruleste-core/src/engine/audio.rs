//! Software audio bus.
//!
//! Sounds are addressed by their FMOD/FSB5 stream name (the exact identifier
//! the original game plays as `event:/...`). Streams are decoded lazily from
//! the `audio/` tree into stereo f32 PCM at `SAMPLE_RATE` and mixed every
//! frame into a single SDL3 `AudioStream` bound to the default playback
//! device. Headless systems (no audio hardware/subsystem) degrade gracefully:
//! `AudioBus::new` returns an inert bus that accepts and discards plays.
//!
//! The mixer intentionally stays small: no dynamic streaming, loops only for
//! music-length clips, and a hard clamp instead of a limiter. Master volume,
//! per-voice pan/pitch/volume and stop-by-name cover what the current plugins
//! need; a compressor/fade-out can be layered on later.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sdl3::Sdl;
use sdl3::audio::{AudioDevice, AudioFormat, AudioSpec};

use crate::data::audio::{AudioManifest, SampleEntry};
use crate::data::ogg::{DecodedPcm, decode_ogg};

/// Fixed output rate; the converter encodes everything at 48 kHz and any other
/// rate is resampled on decode.
pub use crate::data::ogg::SAMPLE_RATE;

/// A decoded, interleaved-stereo stream.
struct SampleData {
    data: Arc<[f32]>,
    frames: usize,
}

struct Voice {
    name: Option<String>,
    sample: Arc<SampleData>,
    pos: f64,
    pitch: f32,
    gain_l: f32,
    gain_r: f32,
    looping: bool,
}

/// Pulse-summaries for debugging/counting.
#[derive(Default, Debug, Clone, Copy)]
pub struct AudioStats {
    /// Streams decoded from disk so far.
    pub decoded: usize,
    /// `play` calls for names absent from the manifest.
    pub unknown: usize,
    /// Plays dropped because the stream was unreadable/undecodable, or on
    /// headless buses.
    pub dropped: usize,
    /// Voices currently sounding.
    pub voices: usize,
}

#[derive(Default)]
pub struct AudioBus {
    inner: Option<Inner>,
}

struct Inner {
    stream: sdl3::audio::AudioStreamOwner,
    root: PathBuf,
    manifest: HashMap<String, SampleEntry>,
    cache: HashMap<String, Arc<SampleData>>,
    voices: Vec<Voice>,
    master_gain: f32,
    stats: AudioStats,
}

impl AudioBus {
    /// Opens the default playback device if audio is available. Never fails:
    /// without a device the bus stays inert and `play` calls are dropped.
    pub fn new(sdl: &Sdl) -> AudioBus {
        match Self::open(sdl) {
            Ok(inner) => AudioBus { inner: Some(inner) },
            Err(e) => {
                crate::log_warn!("audio disabled: {e:#}");
                AudioBus { inner: None }
            }
        }
    }

    fn open(sdl: &Sdl) -> anyhow::Result<Inner> {
        let audio = sdl.audio()?;
        let spec = AudioSpec::new(Some(SAMPLE_RATE as i32), Some(2), Some(AudioFormat::F32LE));
        let device = AudioDevice::open_playback(&audio, None, &spec)?;
        let device_name = device.name()?;
        let stream = device.open_device_stream(Some(&spec))?;
        stream.resume()?;
        crate::log_info!(
            "Audio: {} ({} Hz, {} ch, f32)",
            device_name,
            spec.freq.unwrap_or(0),
            spec.channels.unwrap_or(0)
        );
        Ok(Inner {
            stream,
            root: PathBuf::new(),
            manifest: HashMap::new(),
            cache: HashMap::new(),
            voices: Vec::new(),
            master_gain: 0.9,
            stats: AudioStats::default(),
        })
    }

    pub fn is_available(&self) -> bool {
        self.inner.is_some()
    }

    /// Loads the stream-name manifest. Cheap: streams decode on first play.
    pub fn load_manifest(&mut self, manifest: &AudioManifest, audio_root: &Path) {
        let Some(inner) = &mut self.inner else {
            return;
        };
        inner.root = audio_root.to_path_buf();
        inner.manifest.clear();
        for (name, entry) in manifest.iter_entries() {
            inner.manifest.insert(name.to_string(), entry.clone());
        }
        crate::log_info!("AudioBus: {} streams indexed", inner.manifest.len());
    }

    /// Maps an FMOD-style `event:/...` path back to a playable stream name.
    ///
    /// The converted audio tree is keyed by the raw FSB5 stream name
    /// (`game_<NN>_<sound>`, e.g. `game_04_arrowblock_activate`), while
    /// gameplay plugins trigger the original `event:/...` path
    /// (`event:/game/04_cliffside/arrowblock_activate`). The authoritative
    /// `event -> stream` relation lives in the original `SFX.cs`; the table
    /// below is the subset used by the current plugins, inlined so the runtime
    /// never depends on `references/`.
    ///
    /// Streams that are exported as numbered variants in the manifest
    /// (`game_05_seeker_aggro_01`, `_02`, ...) are picked by probing a `_NN`
    /// suffix on the base stream name.
    ///
    /// Unknown paths return `None` so `play` falls through to its usual
    /// unknown-name stats bump.
    pub fn resolve(&mut self, name: &str) -> Option<&str> {
        let inner = self.inner.as_ref()?;
        if let Some((name, _)) = inner.manifest.get_key_value(name) {
            return Some(name);
        }
        // Exact event -> base stream table (from the original SFX.cs); keep in
        // sync with what the plugins actually play.
        let mapped =
            match name {
                "event:/game/00_prologue/bridge_rumble_loop" => "game_00_bridge_rumble_loop",
                "event:/game/00_prologue/bridge_stop"
                | "event:/game/00_prologue/bridge_support_break" => "game_00_bridge_supportbreak",
                "event:/game/00_prologue/fallblock_first_impact" => {
                    "game_00_fallingblock_prologue_impact"
                }
                "event:/game/00_prologue/fallblock_first_shake" => {
                    "game_00_fallingblock_prologue_shake"
                }
                "event:/game/03_resort/lantern_bump" => "game_03_lantern_bump",
                "event:/game/04_cliffside/arrowblock_activate" => "game_04_arrowblock_activate",
                "event:/game/04_cliffside/arrowblock_break" => "game_04_arrowblock_break",
                "event:/game/04_cliffside/arrowblock_reappear" => "game_04_arrowblock_reappear",
                "event:/game/04_cliffside/greenbooster_dash"
                | "event:/game/04_cliffside/wallbooster_boost"
                | "event:/game/09_core/icewall_boost" => "game_04_greenbooster_dash",
                "event:/game/04_cliffside/whiteblock_fallthru" => "game_04_whiteblock_fallthru",
                "event:/game/05_mirror/seeker_attack"
                | "event:/game/05_mirror_temple/seeker_dash" => "game_05_seeker_dash",
                "event:/game/05_mirror/seeker_killed"
                | "event:/game/05_mirror_temple/seeker_death" => "game_05_seeker_death",
                "event:/game/05_mirror/seeker_locate"
                | "event:/game/05_mirror_temple/seeker_aggro" => "game_05_seeker_aggro",
                "event:/game/05_mirror/seeker_regenerate"
                | "event:/game/05_mirror_temple/seeker_revive" => "game_05_seeker_booped",
                "event:/game/09_core/bounceblock_break" => "game_09_bounceblock_break",
                "event:/game/09_core/bounceblock_reappear" => "game_09_bounceblock_reappear",
                "event:/game/09_core/bounceblock_touch" => "game_09_bounceblock_touch",
                "event:/game/09_core/frontdoor_unlock" => "game_09_frontdoor_unlock",
                "event:/game/09_core/iceball_break"
                | "event:/game/09_core/iceblock_break"
                | "event:/game/09_core/iceblock_death" => "game_09_iceball_break",
                "event:/game/09_core/iceblock_touch" => "game_09_iceblock_touch",
                "event:/game/general/strawberry_blue_touch" => "game_gen_strawberry_blue_touch",
                "event:/game/general/strawberry_get" => "game_gen_strawberry_red_get_1000",
                "event:/game/general/strawberry_touch" => "game_gen_strawberry_touch",
                _ => return None,
            };
        // Prefer the exact base stream; otherwise probe numbered variants.
        if let Some(name) = inner.manifest.get_key_value(mapped) {
            return Some(name.0.as_str());
        }
        for i in 1..=12u32 {
            let variant = format!("{mapped}_{i:02}");
            if let Some(name) = inner.manifest.get_key_value(&variant) {
                return Some(name.0.as_str());
            }
        }
        // Some converter names use underscores before the two-digit variant
        // (for example `game_00_bridge_supportbreak_01`).  Accept that form
        // as well so a mapped event is not silently dropped just because the
        // export naming differs.
        for i in 1..=12u32 {
            let variant = format!("{mapped}_{i}");
            if let Some(name) = inner.manifest.get_key_value(&variant) {
                return Some(name.0.as_str());
            }
        }
        None
    }

    /// Plays every request a Wasm plugin queued this frame and clears the queue.
    ///
    /// `requests` is `wasm_host::AudioBus::drain()`. Unknown / unmapped names
    /// bump `AudioStats::unknown` without allocating voices. Each request
    /// carries volume/pan/pitch/looping straight through to the mixer.
    pub fn play_plugin_requests(&mut self, requests: Vec<crate::hotload::wasm_host::AudioRequest>) {
        for req in requests {
            let resolved = self.resolve(&req.name).map(str::to_owned);
            let stream = resolved.as_deref().unwrap_or(&req.name);
            self.play(stream, req.volume, req.pan, req.pitch, req.looping);
        }
    }

    /// Starts a stream by name. `volume` 0..1, `pan` -1(left)..1(right),
    /// `pitch` 1.0 = normal speed, `looping` repeats forever. Unknown names
    /// and inert buses are dropped with a stats bump.
    pub fn play(&mut self, name: &str, volume: f32, pan: f32, pitch: f32, looping: bool) {
        let Some(inner) = &mut self.inner else {
            return;
        };
        let Some(entry) = inner.manifest.get(name).cloned() else {
            inner.stats.unknown += 1;
            return;
        };
        let Some(sample) = inner.sample_or_load(name, &entry) else {
            inner.stats.dropped += 1;
            return;
        };
        let v = volume.clamp(0.0, 1.0);
        // Constant-power pan: -1 full left, 0 center, +1 full right.
        let theta = ((pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::PI) / 4.0;
        let (gain_l, gain_r) = (v * theta.cos(), v * theta.sin());
        inner.voices.push(Voice {
            name: Some(name.to_string()),
            sample,
            pos: 0.0,
            pitch: pitch.max(0.01),
            gain_l,
            gain_r,
            looping,
        });
    }

    /// Stops every voice playing `name`.
    pub fn stop(&mut self, name: &str) {
        let Some(inner) = &mut self.inner else {
            return;
        };
        inner.voices.retain(|v| v.name.as_deref() != Some(name));
    }

    /// Stops all voices.
    pub fn stop_all(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.voices.clear();
        }
    }

    /// Global bus gain (0..1), applied on top of per-voice volumes.
    pub fn set_master_gain(&mut self, gain: f32) {
        if let Some(inner) = &mut self.inner {
            inner.master_gain = gain.clamp(0.0, 1.0);
        }
    }

    pub fn stats(&self) -> AudioStats {
        match &self.inner {
            Some(inner) => AudioStats {
                decoded: inner.stats.decoded,
                unknown: inner.stats.unknown,
                dropped: inner.stats.dropped,
                voices: inner.voices.len(),
            },
            None => AudioStats::default(),
        }
    }

    /// Mixes the current voices for `dt` seconds and feeds the device. Call
    /// once per frame.
    pub fn update(&mut self, dt: f32) {
        let Some(inner) = &mut self.inner else {
            return;
        };
        let dt = dt.clamp(0.0, 0.1);
        if inner.voices.is_empty() || dt <= 0.0 {
            return;
        }
        let frames = ((SAMPLE_RATE as f32) * dt) as usize;
        if frames == 0 {
            return;
        }
        let mut mix = vec![0.0f32; frames * 2];
        let mut alive: Vec<Voice> = Vec::with_capacity(inner.voices.len());
        for mut voice in std::mem::take(&mut inner.voices) {
            let frames_total = voice.sample.frames;
            if frames_total == 0 {
                continue;
            }
            let end = frames_total as f64;
            let mut finished = false;
            for i in 0..frames {
                while voice.pos >= end {
                    if voice.looping {
                        voice.pos -= end;
                    } else {
                        finished = true;
                        break;
                    }
                }
                if finished {
                    break;
                }
                let base = (voice.pos.floor() as usize).min(frames_total - 1) * 2;
                let l = voice.sample.data[base] * voice.gain_l;
                let r = voice.sample.data[base + 1] * voice.gain_r;
                mix[i * 2] += l;
                mix[i * 2 + 1] += r;
                voice.pos += voice.pitch as f64;
            }
            if !finished {
                alive.push(voice);
            }
        }
        inner.voices = alive;

        let gain = inner.master_gain.clamp(0.0, 1.0);
        for s in mix.iter_mut() {
            let v = *s * gain;
            *s = v.clamp(-1.0, 1.0);
        }
        if let Err(e) = inner.stream.put_data_f32(&mix) {
            inner.stats.dropped += 1;
            crate::log_warn!("audio feed failed: {e}");
        }
    }
}

impl Inner {
    /// Returns the decoded stereo sample for `name`, decoding from disk on the
    /// first request (blocking `update` once per unique stream).
    fn sample_or_load(&mut self, name: &str, entry: &SampleEntry) -> Option<Arc<SampleData>> {
        if let Some(s) = self.cache.get(name) {
            return Some(s.clone());
        }
        let path = self.root.join(&entry.file);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                crate::log_warn!("audio: cannot read {}: {e}", path.display());
                self.stats.dropped += 1;
                return None;
            }
        };
        let pcm: DecodedPcm = match decode_ogg(&bytes) {
            Ok(p) => p,
            Err(e) => {
                crate::log_warn!("audio: bad ogg {}: {e:#}", path.display());
                self.stats.dropped += 1;
                return None;
            }
        };
        self.stats.decoded += 1;
        let sample = Arc::new(SampleData {
            data: pcm.interleaved.into(),
            frames: pcm.frames,
        });
        self.cache.insert(name.to_string(), sample.clone());
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inert_bus_drops_without_allocation() {
        let mut bus = AudioBus::default();
        assert!(!bus.is_available());
        bus.play("nope", 0.5, 0.0, 1.0, false);
        bus.stop_all();
        bus.update(1.0 / 60.0);
        assert_eq!(bus.stats().unknown, 0);
    }

    #[test]
    fn sample_rate_constant() {
        assert_eq!(SAMPLE_RATE, 48_000);
    }
}
