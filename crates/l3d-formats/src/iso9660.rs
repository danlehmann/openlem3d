//! Minimal read-only ISO 9660 filesystem (no Joliet/Rock Ridge extensions,
//! single-extent files).

use crate::Error;
use crate::disc::{DataTrackReader, SECTOR_SIZE};

/// A file or directory in the filesystem.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Name with any `;1` version suffix and trailing `.` removed.
    pub name: String,
    pub is_dir: bool,
    /// First logical sector of the extent.
    pub lba: u32,
    /// Size in bytes.
    pub size: u32,
}

/// An ISO 9660 filesystem on a data track.
pub struct IsoFs {
    reader: DataTrackReader,
    root: Entry,
    /// Volume identifier from the primary volume descriptor.
    pub volume_id: String,
}

fn parse_record(rec: &[u8]) -> Option<Entry> {
    let len = rec[0] as usize;
    if len < 34 || rec.len() < len {
        return None;
    }
    let lba = u32::from_le_bytes(rec[2..6].try_into().unwrap());
    let size = u32::from_le_bytes(rec[10..14].try_into().unwrap());
    let is_dir = rec[25] & 0x02 != 0;
    let name_len = rec[32] as usize;
    let raw_name = &rec[33..33 + name_len];
    let name = match raw_name {
        [0] => ".".to_string(),
        [1] => "..".to_string(),
        _ => {
            let s = String::from_utf8_lossy(raw_name);
            let s = s.split(';').next().unwrap_or("");
            s.strip_suffix('.').unwrap_or(s).to_string()
        }
    };
    Some(Entry {
        name,
        is_dir,
        lba,
        size,
    })
}

impl IsoFs {
    /// Mounts the filesystem by reading the primary volume descriptor at sector 16.
    pub fn new(mut reader: DataTrackReader) -> Result<Self, Error> {
        let mut pvd = vec![0u8; SECTOR_SIZE];
        reader.read_sectors(16, &mut pvd)?;
        if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
            return Err(Error::Format(
                "no ISO 9660 primary volume descriptor".into(),
            ));
        }
        let volume_id = String::from_utf8_lossy(&pvd[40..72]).trim_end().to_string();
        let root = parse_record(&pvd[156..190])
            .ok_or_else(|| Error::Format("bad root directory record".into()))?;
        Ok(IsoFs {
            reader,
            root,
            volume_id,
        })
    }

    pub fn root(&self) -> &Entry {
        &self.root
    }

    /// Lists a directory, excluding the `.` and `..` entries.
    pub fn read_dir(&mut self, dir: &Entry) -> Result<Vec<Entry>, Error> {
        let data = self.read_file(dir)?;
        let mut out = Vec::new();
        for sector in data.chunks(SECTOR_SIZE) {
            let mut pos = 0;
            // Records never span sectors; a zero length byte pads to the next sector.
            while pos < sector.len() && sector[pos] != 0 {
                let e = parse_record(&sector[pos..])
                    .ok_or_else(|| Error::Format("bad directory record".into()))?;
                pos += sector[pos] as usize;
                if e.name != "." && e.name != ".." {
                    out.push(e);
                }
            }
        }
        Ok(out)
    }

    /// Looks up a `/`-separated path, case-insensitively.
    pub fn lookup(&mut self, path: &str) -> Result<Entry, Error> {
        let mut cur = self.root.clone();
        for part in path.split(['/', '\\']).filter(|p| !p.is_empty()) {
            cur = self
                .read_dir(&cur)?
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case(part))
                .ok_or_else(|| Error::NotFound(path.to_string()))?;
        }
        Ok(cur)
    }

    /// Reads an entry's whole extent.
    pub fn read_file(&mut self, e: &Entry) -> Result<Vec<u8>, Error> {
        let sectors = (e.size as usize).div_ceil(SECTOR_SIZE);
        let mut buf = vec![0u8; sectors * SECTOR_SIZE];
        self.reader.read_sectors(e.lba, &mut buf)?;
        buf.truncate(e.size as usize);
        Ok(buf)
    }

    /// Reads a file by path.
    pub fn read_path(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        let e = self.lookup(path)?;
        self.read_file(&e)
    }

    /// Lists every file and directory in the filesystem recursively, as
    /// `(path, entry)` pairs with `/`-separated paths.
    pub fn walk(&mut self) -> Result<Vec<(String, Entry)>, Error> {
        let mut out = Vec::new();
        let mut stack = vec![(String::new(), self.root.clone())];
        while let Some((prefix, dir)) = stack.pop() {
            let mut entries = self.read_dir(&dir)?;
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for e in entries.into_iter().rev() {
                let path = if prefix.is_empty() {
                    e.name.clone()
                } else {
                    format!("{prefix}/{}", e.name)
                };
                if e.is_dir {
                    stack.push((path.clone(), e.clone()));
                }
                out.push((path, e));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}
