//! Access to a CD image described by a CUE sheet: data-track sectors and
//! CD-audio tracks.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::Error;
use crate::cue::{CueSheet, TrackMode};

/// Size of the user-data portion of a data sector.
pub const SECTOR_SIZE: usize = 2048;

/// A track's location within its image file.
#[derive(Debug, Clone)]
pub struct Track {
    /// Track number (1-based).
    pub number: u8,
    pub mode: TrackMode,
    pub file: PathBuf,
    /// Byte offset of the track's first sector (`INDEX 01`) within `file`.
    pub byte_offset: u64,
    /// Number of sectors from `INDEX 01` to the end of the track as stored in
    /// the image (excludes the next track's stored pregap).
    pub sectors: u32,
}

impl Track {
    /// Duration in seconds (audio tracks).
    pub fn seconds(&self) -> f64 {
        self.sectors as f64 / crate::cue::FRAMES_PER_SECOND as f64
    }
}

/// A CD image: a CUE sheet with track extents resolved against the image files.
#[derive(Debug, Clone)]
pub struct Disc {
    pub tracks: Vec<Track>,
}

impl Disc {
    /// Opens the CD image described by the CUE sheet at `path`.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let cue = CueSheet::load(path)?;
        let mut tracks = Vec::with_capacity(cue.tracks.len());
        for (i, t) in cue.tracks.iter().enumerate() {
            let sector_size = t.mode.sector_size() as u64;
            let end = match cue.tracks.get(i + 1).filter(|n| n.file == t.file) {
                Some(next) => next.index0.unwrap_or(next.index1) as u64,
                None => std::fs::metadata(&t.file)?.len() / sector_size,
            };
            let sectors = end
                .checked_sub(t.index1 as u64)
                .ok_or_else(|| Error::Cue(format!("track {} has a negative length", t.number)))?;
            tracks.push(Track {
                number: t.number,
                mode: t.mode,
                file: t.file.clone(),
                byte_offset: t.index1 as u64 * sector_size,
                sectors: sectors as u32,
            });
        }
        Ok(Disc { tracks })
    }

    /// Opens the CD image in `dir`, which must contain exactly one `.cue` file.
    pub fn open_dir(dir: &Path) -> Result<Self, Error> {
        let mut cues = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let p = entry?.path();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cue")) {
                cues.push(p);
            }
        }
        match cues.as_slice() {
            [one] => Self::open(one),
            [] => Err(Error::NotFound(format!(
                "no .cue file in {}",
                dir.display()
            ))),
            _ => Err(Error::Cue(format!(
                "more than one .cue file in {}",
                dir.display()
            ))),
        }
    }

    /// The first data track.
    pub fn data_track(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.mode != TrackMode::Audio)
    }

    /// Audio tracks, in disc order.
    pub fn audio_tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks.iter().filter(|t| t.mode == TrackMode::Audio)
    }

    /// Opens a reader over the user data of the first data track.
    pub fn data_reader(&self) -> Result<DataTrackReader, Error> {
        let track = self
            .data_track()
            .ok_or_else(|| Error::NotFound("no data track".into()))?;
        Ok(DataTrackReader {
            file: File::open(&track.file)?,
            track: track.clone(),
        })
    }

    /// Reads a whole audio track as interleaved 16-bit stereo samples at 44.1 kHz.
    pub fn read_audio(&self, track: &Track) -> Result<Vec<i16>, Error> {
        if track.mode != TrackMode::Audio {
            return Err(Error::Format(format!(
                "track {} is not audio",
                track.number
            )));
        }
        let mut f = File::open(&track.file)?;
        f.seek(SeekFrom::Start(track.byte_offset))?;
        let mut bytes = vec![0u8; track.sectors as usize * 2352];
        f.read_exact(&mut bytes)?;
        Ok(bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect())
    }
}

/// Reads 2048-byte logical sectors from a data track.
pub struct DataTrackReader {
    file: File,
    track: Track,
}

impl DataTrackReader {
    /// Number of logical sectors in the track.
    pub fn sector_count(&self) -> u32 {
        self.track.sectors
    }

    /// Reads `buf.len() / 2048` consecutive sectors starting at `lba`.
    pub fn read_sectors(&mut self, lba: u32, buf: &mut [u8]) -> Result<(), Error> {
        assert!(buf.len().is_multiple_of(SECTOR_SIZE));
        let count = (buf.len() / SECTOR_SIZE) as u32;
        if lba
            .checked_add(count)
            .is_none_or(|end| end > self.track.sectors)
        {
            return Err(Error::Format(format!(
                "sector {lba}+{count} outside data track"
            )));
        }
        let mode = self.track.mode;
        let stride = mode.sector_size() as u64;
        if stride as usize == SECTOR_SIZE {
            self.file.seek(SeekFrom::Start(
                self.track.byte_offset + lba as u64 * stride,
            ))?;
            self.file.read_exact(buf)?;
            return Ok(());
        }
        let mut raw = vec![0u8; stride as usize * count as usize];
        self.file.seek(SeekFrom::Start(
            self.track.byte_offset + lba as u64 * stride,
        ))?;
        self.file.read_exact(&mut raw)?;
        let off = mode.user_data_offset() as usize;
        for (dst, src) in buf
            .as_chunks_mut::<SECTOR_SIZE>()
            .0
            .iter_mut()
            .zip(raw.chunks_exact(stride as usize))
        {
            dst.copy_from_slice(&src[off..off + SECTOR_SIZE]);
        }
        Ok(())
    }
}
