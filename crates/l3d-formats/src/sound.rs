//! Sound samples: `SOUND/SPOTFX/*.U8` and `SOUND/VOXFX/*.U8`. See
//! `docs/spec/sound.md`.

use crate::Error;

/// Bytes before the sample data.
pub const HEADER_LEN: usize = 32;

/// A decoded sample: mono, signed 8-bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    /// Samples per second.
    pub rate: u32,
    pub data: Vec<i8>,
    /// Loop range within `data` (start, end exclusive), when the sample
    /// repeats.
    pub looped: Option<(usize, usize)>,
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

impl Sample {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < HEADER_LEN {
            return Err(Error::Format(format!("sample of {} bytes is shorter than its header", bytes.len())));
        }
        let start = u32_at(bytes, 0) as usize;
        // The second field is the offset of the last byte.
        let end = (u32_at(bytes, 4) as usize + 1).min(bytes.len());
        let (loop_start, loop_end) = (u32_at(bytes, 8) as usize, u32_at(bytes, 12) as usize);
        let rate = u16::from_le_bytes([bytes[16], bytes[17]]) as u32;
        if start != HEADER_LEN || end < start || rate == 0 {
            return Err(Error::Format(format!("unexpected sample header (start {start}, end {end}, rate {rate})")));
        }
        let data = bytes[start..end].iter().map(|&b| b as i8).collect();
        let looped = (loop_end > loop_start && loop_start >= start)
            .then(|| (loop_start - start, (loop_end + 1).min(end) - start));
        Ok(Sample { rate, data, looped })
    }

    /// The sample as a mono 8-bit RIFF/WAVE file (8-bit WAVE is unsigned).
    pub fn to_wav(&self) -> Vec<u8> {
        let n = self.data.len() as u32;
        let mut out = Vec::with_capacity(44 + self.data.len());
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + n).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&self.rate.to_le_bytes()); // bytes per second
        out.extend_from_slice(&1u16.to_le_bytes()); // block align
        out.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        out.extend_from_slice(b"data");
        out.extend_from_slice(&n.to_le_bytes());
        out.extend(self.data.iter().map(|&s| (s as i16 + 128) as u8));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(len: usize, loop_range: (u32, u32), rate: u16) -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN];
        b[0..4].copy_from_slice(&32u32.to_le_bytes());
        b[4..8].copy_from_slice(&((HEADER_LEN + len - 1) as u32).to_le_bytes());
        b[8..12].copy_from_slice(&loop_range.0.to_le_bytes());
        b[12..16].copy_from_slice(&loop_range.1.to_le_bytes());
        b[16..18].copy_from_slice(&rate.to_le_bytes());
        b
    }

    #[test]
    fn parses_data_rate_and_loop() {
        let mut b = header(4, (0, 0), 22050);
        b.extend([0x00, 0x7F, 0x80, 0xFF]);
        let s = Sample::parse(&b).unwrap();
        assert_eq!((s.rate, s.data.clone(), s.looped), (22050, vec![0, 127, -128, -1], None));
        let wav = s.to_wav();
        assert_eq!(&wav[44..], &[128, 255, 0, 127]);

        let mut b = header(4, (33, 35), 11025);
        b.extend([1, 2, 3, 4]);
        assert_eq!(Sample::parse(&b).unwrap().looped, Some((1, 4)));
        assert!(Sample::parse(&b[..10]).is_err());
    }
}
