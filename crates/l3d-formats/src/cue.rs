//! CUE sheet parsing.
//!
//! Only the subset of the CUE format needed to describe a CD image is
//! supported: `FILE`, `TRACK`, `INDEX` and `PREGAP`/`POSTGAP` (the latter two
//! describe gaps that are not stored in the image file). Other commands are
//! ignored.

use std::path::{Path, PathBuf};

use crate::Error;

/// Number of CD frames (sectors) per second.
pub const FRAMES_PER_SECOND: u32 = 75;

/// The data layout of a track's sectors as stored in the image file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackMode {
    /// Red-book audio: 2352 bytes of 16-bit little-endian stereo PCM per sector.
    Audio,
    /// Mode 1 data, raw 2352-byte sectors (12 sync + 4 header + 2048 data + 288 EDC/ECC).
    Mode1Raw,
    /// Mode 1 data, 2048-byte sectors containing only user data.
    Mode1Cooked,
    /// Mode 2 (XA) data, raw 2352-byte sectors; form-1 user data at offset 24.
    Mode2Raw,
}

impl TrackMode {
    /// Size of one sector in the image file.
    pub fn sector_size(self) -> u32 {
        match self {
            TrackMode::Mode1Cooked => 2048,
            _ => 2352,
        }
    }

    /// Offset of the 2048 bytes of user data within a stored sector. Not
    /// meaningful for audio.
    pub fn user_data_offset(self) -> u32 {
        match self {
            TrackMode::Audio | TrackMode::Mode1Cooked => 0,
            TrackMode::Mode1Raw => 16,
            TrackMode::Mode2Raw => 24,
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "AUDIO" => TrackMode::Audio,
            "MODE1/2352" => TrackMode::Mode1Raw,
            "MODE1/2048" => TrackMode::Mode1Cooked,
            "MODE2/2352" => TrackMode::Mode2Raw,
            _ => return None,
        })
    }
}

/// A track as described by a CUE sheet.
#[derive(Debug, Clone)]
pub struct CueTrack {
    /// Track number (1-based).
    pub number: u8,
    pub mode: TrackMode,
    /// Image file the track is stored in, resolved relative to the CUE sheet.
    pub file: PathBuf,
    /// Frame offset of `INDEX 00` within `file`, if present.
    pub index0: Option<u32>,
    /// Frame offset of `INDEX 01` (the start of the track proper) within `file`.
    pub index1: u32,
    /// Frames of pregap not stored in the image (`PREGAP`).
    pub pregap: u32,
}

/// A parsed CUE sheet.
#[derive(Debug, Clone)]
pub struct CueSheet {
    pub tracks: Vec<CueTrack>,
}

/// Parses an `MM:SS:FF` timestamp into a frame count.
pub fn parse_msf(s: &str) -> Option<u32> {
    let mut it = s.split(':').map(|p| p.parse::<u32>().ok());
    let (m, sec, f) = (it.next()??, it.next()??, it.next()??);
    if it.next().is_some() || sec >= 60 || f >= FRAMES_PER_SECOND {
        return None;
    }
    Some((m * 60 + sec) * FRAMES_PER_SECOND + f)
}

/// Splits a CUE line into words, honouring double-quoted strings.
fn tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.trim().chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            out.push(chars.by_ref().take_while(|&c| c != '"').collect());
        } else {
            let mut w = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                w.push(c);
                chars.next();
            }
            out.push(w);
        }
    }
    out
}

impl CueSheet {
    /// Parses CUE sheet text. `base_dir` is the directory relative paths in
    /// `FILE` commands are resolved against.
    pub fn parse(text: &str, base_dir: &Path) -> Result<Self, Error> {
        let bad = |line: usize, msg: &str| Error::Cue(format!("line {}: {msg}", line + 1));
        let mut tracks: Vec<CueTrack> = Vec::new();
        let mut file: Option<PathBuf> = None;
        for (ln, line) in text.lines().enumerate() {
            let words = tokenize(line);
            let Some(cmd) = words.first() else { continue };
            match cmd.to_ascii_uppercase().as_str() {
                "FILE" => {
                    let name = words.get(1).ok_or_else(|| bad(ln, "FILE without name"))?;
                    file = Some(base_dir.join(name));
                }
                "TRACK" => {
                    let number = words
                        .get(1)
                        .and_then(|n| n.parse().ok())
                        .ok_or_else(|| bad(ln, "bad track number"))?;
                    let mode = words
                        .get(2)
                        .and_then(|m| TrackMode::parse(m))
                        .ok_or_else(|| bad(ln, "unsupported track mode"))?;
                    let file = file.clone().ok_or_else(|| bad(ln, "TRACK before FILE"))?;
                    tracks.push(CueTrack {
                        number,
                        mode,
                        file,
                        index0: None,
                        index1: 0,
                        pregap: 0,
                    });
                }
                "INDEX" => {
                    let t = tracks
                        .last_mut()
                        .ok_or_else(|| bad(ln, "INDEX before TRACK"))?;
                    let n: u32 = words
                        .get(1)
                        .and_then(|n| n.parse().ok())
                        .ok_or_else(|| bad(ln, "bad index"))?;
                    let at = words
                        .get(2)
                        .and_then(|s| parse_msf(s))
                        .ok_or_else(|| bad(ln, "bad time"))?;
                    match n {
                        0 => t.index0 = Some(at),
                        1 => t.index1 = at,
                        _ => {}
                    }
                }
                "PREGAP" => {
                    let t = tracks
                        .last_mut()
                        .ok_or_else(|| bad(ln, "PREGAP before TRACK"))?;
                    t.pregap = words
                        .get(1)
                        .and_then(|s| parse_msf(s))
                        .ok_or_else(|| bad(ln, "bad time"))?;
                }
                _ => {}
            }
        }
        if tracks.is_empty() {
            return Err(Error::Cue("no tracks".into()));
        }
        Ok(CueSheet { tracks })
    }

    /// Reads and parses a CUE sheet from disk.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)?;
        Self::parse(&text, path.parent().unwrap_or(Path::new(".")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msf() {
        assert_eq!(parse_msf("00:00:00"), Some(0));
        assert_eq!(parse_msf("20:29:08"), Some((20 * 60 + 29) * 75 + 8));
        assert_eq!(parse_msf("00:60:00"), None);
        assert_eq!(parse_msf("00:00:75"), None);
    }

    #[test]
    fn parse_sheet() {
        let text = "FILE \"Game Disc.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    PREGAP 00:02:00\n    FLAGS PRE\n    INDEX 01 20:29:08\n  TRACK 03 AUDIO\n    INDEX 00 21:39:47\n    INDEX 01 21:41:47\n";
        let cue = CueSheet::parse(text, Path::new("d")).unwrap();
        assert_eq!(cue.tracks.len(), 3);
        assert_eq!(cue.tracks[0].mode, TrackMode::Mode1Raw);
        assert_eq!(cue.tracks[0].file, Path::new("d").join("Game Disc.bin"));
        assert_eq!(cue.tracks[1].pregap, 150);
        assert_eq!(cue.tracks[2].index0, Some(parse_msf("21:39:47").unwrap()));
    }
}
