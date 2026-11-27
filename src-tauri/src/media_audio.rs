//! Audio of a media file as a stream of 16 kHz mono blocks.
//!
//! Only the audio track is decoded: packets of every other track (the
//! video above all) are dropped unread by the demuxer loop, so the
//! resolution of a video costs nothing. One packet is decoded at a
//! time and handed out as a block of at most a second; nothing of the
//! file is kept, whatever its length.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::TimeBase;

use crate::audio::dsp::pcm16_from_f32;

/// Sample rate of the blocks.
pub const RATE: usize = 16_000;

/// Longest block [`MediaAudio::read`] returns, in samples.
pub const MAX_BLOCK: usize = RATE;

/// A packet that starts this much later than the audio decoded so far
/// ends is a gap in the track; silence fills it so that subtitle times
/// stay those of the file.
const GAP_MS: i64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The file cannot be read.
    Io(String),
    /// Not a container this build reads.
    UnknownFormat,
    /// The file has no audio track.
    NoAudio,
    /// The audio track's codec is not one this build decodes.
    UnsupportedCodec,
}

pub struct MediaAudio {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    time_base: Option<TimeBase>,
    duration_ms: Option<u64>,
    /// Interleaved samples of the packet being converted.
    scratch: Vec<f32>,
    resampler: Resampler,
    /// Length of the audio handed out so far, in 16 kHz samples.
    produced: u64,
    /// Silence still to hand out for a gap in the track, in samples.
    silence: u64,
    /// The decoded packet that follows that silence.
    pending: Option<Vec<i16>>,
}

impl MediaAudio {
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let file = File::open(path).map_err(|e| OpenError::Io(e.to_string()))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| match e {
                Error::IoError(e) if e.kind() != std::io::ErrorKind::UnexpectedEof => {
                    OpenError::Io(e.to_string())
                }
                _ => OpenError::UnknownFormat,
            })?;
        // A track whose codec is unknown is still an audio track: the
        // file then has audio this build cannot decode.
        let track = format
            .default_track(TrackType::Audio)
            .or_else(|| format.first_track(TrackType::Audio))
            .ok_or(OpenError::NoAudio)?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or(OpenError::UnsupportedCodec)?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|_| OpenError::UnsupportedCodec)?;
        let millis = |base: Option<TimeBase>, duration| {
            base.zip(duration)
                .and_then(|(base, d)| base.calc_duration(d))
                .and_then(|t| u64::try_from(t.as_millis()).ok())
                .filter(|ms| *ms > 0)
        };
        // Matroska states the length of the file, not of each track.
        let media = format.media_info();
        let duration_ms = millis(track.time_base, track.duration)
            .or_else(|| millis(media.time_base, media.duration));
        Ok(Self {
            track: track.id,
            time_base: track.time_base,
            duration_ms,
            format,
            decoder,
            scratch: Vec::new(),
            resampler: Resampler::default(),
            produced: 0,
            silence: 0,
            pending: None,
        })
    }

    /// Length of the audio track, when the container states it.
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// The next block of 16 kHz mono audio, at most [`MAX_BLOCK`]
    /// samples; `None` at the end of the track. Damaged packets are
    /// skipped.
    pub fn read(&mut self) -> Option<Vec<i16>> {
        loop {
            if self.silence > 0 {
                let n = self.silence.min(MAX_BLOCK as u64);
                self.silence -= n;
                self.produced += n;
                return Some(vec![0; n as usize]);
            }
            if let Some(block) = self.pending.take() {
                self.produced += block.len() as u64;
                return Some(block);
            }
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                // A truncated file ends where its data does.
                Ok(None) | Err(_) => return None,
            };
            if packet.track_id != self.track {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(Error::DecodeError(_)) => continue,
                Err(_) => return None,
            };
            let (rate, channels) = (decoded.spec().rate(), decoded.spec().channels().count());
            decoded.copy_to_vec_interleaved(&mut self.scratch);
            let mono: Vec<i16> = self
                .scratch
                .chunks_exact(channels.max(1))
                .map(|frame| pcm16_from_f32(frame.iter().sum::<f32>() / frame.len() as f32))
                .collect();
            let block = self.resampler.push(&mono, rate);
            if block.is_empty() {
                continue;
            }
            // What the track skipped comes before the packet's audio.
            self.silence = self.gap_before(packet.pts);
            self.pending = Some(block);
        }
    }

    /// Samples of silence missing before a packet that starts at `pts`.
    fn gap_before(&self, pts: symphonia::core::units::Timestamp) -> u64 {
        let Some(at) = self.time_base.and_then(|base| base.calc_time(pts)) else {
            return 0;
        };
        let at_ms = i64::try_from(at.as_millis()).unwrap_or(0);
        // A time past the end of the track is a damaged one.
        if (self.duration_ms).is_some_and(|d| at_ms > i64::try_from(d).unwrap_or(i64::MAX)) {
            return 0;
        }
        let have_ms = (self.produced * 1000 / RATE as u64) as i64;
        if at_ms - have_ms < GAP_MS {
            return 0;
        }
        (at_ms - have_ms) as u64 * RATE as u64 / 1000
    }
}

/// Linear resampling to [`RATE`] of a stream that arrives in pieces.
/// The pieces join without a seam: the output is as long as the input
/// whatever the packet size, so subtitle times do not drift over
/// hours. (`audio::dsp::resample_to_16k_linear` drops up to a sample
/// per call, which live capture can afford.)
#[derive(Default)]
struct Resampler {
    /// Last sample of the piece before.
    carry: Option<i16>,
    /// Position of the next output sample from `carry` (from the start
    /// of the piece when there is none), in 1/[`RATE`] input samples:
    /// whole numbers, so that no rounding builds up over a long file.
    position: u64,
}

impl Resampler {
    fn push(&mut self, piece: &[i16], rate: u32) -> Vec<i16> {
        if rate as usize == RATE || rate == 0 || piece.is_empty() {
            return piece.to_vec();
        }
        let unit = RATE as u64;
        let carried = usize::from(self.carry.is_some());
        let at = |i: usize| match (i, self.carry) {
            (0, Some(carry)) => carry as i64,
            _ => piece[i - carried] as i64,
        };
        let last = (piece.len() + carried - 1) as u64 * unit;
        let mut out = Vec::with_capacity(piece.len() * RATE / rate as usize + 2);
        while self.position < last {
            let i = (self.position / unit) as usize;
            let frac = (self.position % unit) as i64;
            let (a, b) = (at(i), at(i + 1));
            out.push((a + (b - a) * frac / unit as i64) as i16);
            self.position += rate as u64;
        }
        self.position -= last;
        self.carry = Some(piece[piece.len() - 1]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(len: usize) -> Vec<i16> {
        (0..len).map(|i| (i % 20_000) as i16).collect()
    }

    /// Resample `input` handed over in pieces of `piece` samples.
    fn resampled(input: &[i16], rate: u32, piece: usize) -> Vec<i16> {
        let mut resampler = Resampler::default();
        input
            .chunks(piece)
            .flat_map(|p| resampler.push(p, rate))
            .collect()
    }

    #[test]
    fn the_output_is_as_long_as_the_input_whatever_the_piece_size() {
        for rate in [8_000u32, 22_050, 44_100, 48_000, 96_000] {
            let input = ramp(rate as usize * 10);
            for piece in [1, 7, 1024, 1152, 4096] {
                let out = resampled(&input, rate, piece);
                // Ten seconds, short of the last output sample at most.
                assert!(
                    (10 * RATE - 2..=10 * RATE).contains(&out.len()),
                    "{rate} Hz in pieces of {piece}: {} samples",
                    out.len()
                );
            }
        }
    }

    #[test]
    fn pieces_join_without_a_seam() {
        let input = ramp(44_100);
        let whole = resampled(&input, 44_100, input.len());
        for piece in [1, 3, 1024] {
            assert_eq!(resampled(&input, 44_100, piece), whole, "pieces of {piece}");
        }
    }

    #[test]
    fn samples_are_interpolated_between_their_neighbours() {
        // 8 kHz to 16 kHz: every other output lies half-way.
        assert_eq!(
            resampled(&[0, 100, 200, 300], 8_000, 2),
            vec![0, 50, 100, 150, 200, 250]
        );
        // 48 kHz to 16 kHz: every third sample.
        assert_eq!(
            resampled(&[0, 1, 2, 3, 4, 5, 6, 7], 48_000, 3),
            vec![0, 3, 6]
        );
    }

    #[test]
    fn audio_at_16_khz_passes_unchanged() {
        assert_eq!(resampled(&[1, 2, 3], 16_000, 2), vec![1, 2, 3]);
    }
}
