//! Minimal PCM16 → WAV encoder for the loopback-captured audio. Bundling
//! a byte-level encoder avoids pulling `hound`/`wav` just to add a header.

/// Encode a mono 16 kHz PCM16 sample buffer as a complete RIFF/WAVE file.
pub fn encode_pcm16_mono_16k(samples: &[i16]) -> Vec<u8> {
    encode_pcm16(samples, 16_000, 1)
}

/// General form. Callers supply sample rate and channel count; samples
/// are assumed interleaved if `channels > 1`.
pub fn encode_pcm16(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
    let bytes_per_sample: u16 = 2;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
    let block_align = channels * bytes_per_sample;
    let data_size = (samples.len() as u32) * bytes_per_sample as u32;
    let riff_size = 36 + data_size;

    let mut out = Vec::with_capacity(44 + data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&(bytes_per_sample * 8).to_le_bytes()); // bits per sample
                                                                  // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ])
    }
    fn read_u16_le(buf: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([buf[offset], buf[offset + 1]])
    }

    #[test]
    fn header_is_44_bytes_before_samples() {
        let wav = encode_pcm16_mono_16k(&[0, 0, 0]);
        assert_eq!(wav.len(), 44 + 6);
    }

    #[test]
    fn riff_and_wave_magic_bytes_are_present() {
        let wav = encode_pcm16_mono_16k(&[]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
    }

    #[test]
    fn fmt_chunk_declares_pcm16_mono_16k() {
        let wav = encode_pcm16_mono_16k(&[0, 0, 0, 0]);
        assert_eq!(read_u16_le(&wav, 20), 1); // PCM
        assert_eq!(read_u16_le(&wav, 22), 1); // channels
        assert_eq!(read_u32_le(&wav, 24), 16_000); // sample rate
        assert_eq!(read_u32_le(&wav, 28), 32_000); // byte rate
        assert_eq!(read_u16_le(&wav, 32), 2); // block align
        assert_eq!(read_u16_le(&wav, 34), 16); // bits per sample
    }

    #[test]
    fn data_size_matches_sample_byte_count() {
        let samples: Vec<i16> = (0..128).collect();
        let wav = encode_pcm16_mono_16k(&samples);
        let data_size = read_u32_le(&wav, 40);
        assert_eq!(data_size as usize, samples.len() * 2);
        // riff_size = 36 + data_size
        assert_eq!(read_u32_le(&wav, 4), 36 + data_size);
    }

    #[test]
    fn samples_are_little_endian() {
        let wav = encode_pcm16_mono_16k(&[0x1234, -1]);
        assert_eq!(wav[44], 0x34);
        assert_eq!(wav[45], 0x12);
        assert_eq!(wav[46], 0xFF);
        assert_eq!(wav[47], 0xFF);
    }

    #[test]
    fn zero_samples_produces_44_byte_header_only() {
        let wav = encode_pcm16_mono_16k(&[]);
        assert_eq!(wav.len(), 44);
        assert_eq!(read_u32_le(&wav, 40), 0); // data size
                                              // riff_size = 36 when data_size is 0.
        assert_eq!(read_u32_le(&wav, 4), 36);
    }

    #[test]
    fn single_sample_produces_46_byte_file() {
        let wav = encode_pcm16_mono_16k(&[0]);
        assert_eq!(wav.len(), 46);
        assert_eq!(read_u32_le(&wav, 40), 2);
    }

    #[test]
    fn i16_extremes_do_not_overflow_encoding() {
        let wav = encode_pcm16_mono_16k(&[i16::MIN, i16::MAX]);
        assert_eq!(wav.len(), 48);
        // i16::MIN = 0x8000 → LE bytes 0x00, 0x80
        assert_eq!(wav[44], 0x00);
        assert_eq!(wav[45], 0x80);
        assert_eq!(wav[46], 0xFF);
        assert_eq!(wav[47], 0x7F);
    }

    #[test]
    fn stereo_48k_fmt_chunk_is_correct() {
        let wav = encode_pcm16(&[0, 0, 0, 0], 48_000, 2);
        assert_eq!(read_u16_le(&wav, 22), 2); // channels
        assert_eq!(read_u32_le(&wav, 24), 48_000);
        // byte_rate = sample_rate * channels * bytes_per_sample
        assert_eq!(read_u32_le(&wav, 28), 48_000 * 2 * 2);
        // block_align = channels * bytes_per_sample
        assert_eq!(read_u16_le(&wav, 32), 4);
    }
}
