//! Typed access to the game files on the user's CD image.

use std::path::{Path, PathBuf};

use crate::blk::BlockSet;
use crate::disc::Disc;
use crate::iso9660::IsoFs;
use crate::level::Level;
use crate::{Error, rnc};

/// Environment variable naming the directory that holds the CD image.
pub const DATA_ENV: &str = "OPENLEM3D_DATA";
/// Default directory holding the CD image, relative to the working directory.
pub const DEFAULT_DATA_DIR: &str = "gamedata";

/// Resolves the game-data directory: the explicit argument if given, else
/// `$OPENLEM3D_DATA`, else `./gamedata`.
pub fn locate_data_dir(explicit: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os(DATA_ENV).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR))
}

/// A 256-colour palette with components scaled to 8 bits.
pub type Palette = [[u8; 3]; 256];

/// The mounted CD image.
pub struct GameData {
    pub disc: Disc,
    pub fs: IsoFs,
}

impl GameData {
    /// Opens the CD image in `dir` (see [`locate_data_dir`]).
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let disc = Disc::open_dir(dir)?;
        let fs = IsoFs::new(disc.data_reader()?)?;
        Ok(GameData { disc, fs })
    }

    /// Reads a file, decompressing it if it is RNC-packed.
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        rnc::unpack_if_packed(self.fs.read_path(path)?)
    }

    pub fn level(&mut self, n: u32) -> Result<Level, Error> {
        Level::parse(&self.read(&format!("LEVELS/LEVEL.{n:03}"))?)
    }

    pub fn blocks(&mut self, n: u32) -> Result<BlockSet, Error> {
        BlockSet::parse(&self.read(&format!("LEVELS/BLK.{n:03}"))?)
    }

    /// Reads `GFX/<kind>.nnn`, e.g. `gfx("TEXTURE", 4)`.
    pub fn gfx(&mut self, kind: &str, n: u8) -> Result<Vec<u8>, Error> {
        self.read(&format!("GFX/{kind}.{n:03}"))
    }

    /// Reads a 6-bit VGA palette file.
    pub fn palette(&mut self, path: &str) -> Result<Palette, Error> {
        let raw = self.read(path)?;
        if raw.len() != 768 {
            return Err(Error::Format(format!("palette {path} is {} bytes", raw.len())));
        }
        Ok(std::array::from_fn(|i| std::array::from_fn(|c| {
            let v = raw[i * 3 + c] & 0x3F;
            (v << 2) | (v >> 4)
        })))
    }
}
