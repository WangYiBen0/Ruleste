//! Ogg Vorbis decoding to interleaved stereo f32 PCM (lewton).
//!
//! The converter (`tools/bank-to-ogg.sh`) encodes everything at 48 kHz; the
//! decoder still resamples other rates so the audio bus can assume a single
//! fixed rate.

use std::io::Cursor;

use anyhow::Context;
use lewton::inside_ogg::OggStreamReader;

/// Fixed output rate the whole pipeline targets; the converter encodes
/// everything at 48 kHz and any other rate is resampled on decode.
pub const SAMPLE_RATE: u32 = 48_000;

/// Interleaved stereo PCM for one decoded stream.
pub struct DecodedPcm {
    /// Interleaved `[left, right, ...]` samples.
    pub interleaved: Vec<f32>,
    /// Number of stereo frames (== `interleaved.len() / 2`).
    pub frames: usize,
    pub sample_rate: u32,
}

/// Decodes a whole Ogg Vorbis file/memory slice to stereo f32 at
/// [`SAMPLE_RATE`].
pub fn decode_ogg(bytes: &[u8]) -> anyhow::Result<DecodedPcm> {
    let mut reader = OggStreamReader::new(Cursor::new(bytes)).context("reading ogg headers")?;
    let sample_rate = reader.ident_hdr.audio_sample_rate.max(1);

    // Collect decoded packets first so we can resize once.
    let mut plan: Vec<Vec<Vec<f32>>> = Vec::new();
    loop {
        match reader
            .read_dec_packet_generic::<Vec<Vec<f32>>>()
            .context("decoding vorbis packet")?
        {
            None => break,
            Some(pkt) => plan.push(pkt),
        }
    }
    if plan.iter().all(|p| p.is_empty() || p[0].is_empty()) {
        anyhow::bail!("no audio packets");
    }

    let frames_src: usize = plan.iter().map(|p| p[0].len()).sum();
    let mut interleaved = Vec::with_capacity(frames_src.saturating_mul(2));
    for pkt in plan {
        let left = &pkt[0];
        match pkt.get(1) {
            Some(right) => {
                for (&l, &r) in left.iter().zip(right.iter()) {
                    interleaved.push(l);
                    interleaved.push(r);
                }
            }
            None => {
                for &l in left.iter() {
                    interleaved.push(l);
                    interleaved.push(l);
                }
            }
        }
    }

    let pcm = DecodedPcm {
        interleaved,
        frames: frames_src,
        sample_rate,
    };
    if sample_rate == SAMPLE_RATE {
        Ok(pcm)
    } else {
        Ok(resample_stereo(&pcm, SAMPLE_RATE))
    }
}

/// Linear-resamples stereo PCM to `target_rate` Hz.
fn resample_stereo(src: &DecodedPcm, target_rate: u32) -> DecodedPcm {
    let out_frames = ((src.frames as u64 * target_rate as u64) / src.sample_rate as u64) as usize;
    let mut out = Vec::with_capacity(out_frames.saturating_mul(2));
    let ratio = src.sample_rate as f32 / target_rate as f32;
    for i in 0..out_frames {
        let t = i as f32 * ratio;
        let lo = (t.floor() as usize).min(src.frames - 1);
        let hi = (lo + 1).min(src.frames - 1);
        let f = t - lo as f32;
        for c in 0..2 {
            let a = src.interleaved[lo * 2 + c];
            let b = src.interleaved[hi * 2 + c];
            out.push(a + (b - a) * f);
        }
    }
    DecodedPcm {
        interleaved: out,
        frames: out_frames,
        sample_rate: target_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_preserves_stereo_and_length() {
        let src = DecodedPcm {
            interleaved: vec![0.0, 1.0, 0.5, 0.5, 1.0, 0.0], // 3 frames @1000
            frames: 3,
            sample_rate: 1000,
        };
        let out = resample_stereo(&src, 2000);
        assert_eq!(out.sample_rate, 2000);
        assert_eq!(out.frames, 6);
        assert_eq!(out.interleaved.len(), 12);
        assert!((out.interleaved[0] - 0.0).abs() < 1e-6);
        assert!((out.interleaved[5] - 0.5).abs() < 1e-6);
    }
}
