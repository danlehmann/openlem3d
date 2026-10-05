//! Parsers for the data formats of the 1995 DOS game *Lemmings 3D*.
//!
//! Format descriptions live in `docs/spec/`.

pub mod blk;
pub mod cue;
pub mod disc;
pub mod gamedata;
pub mod iso9660;
pub mod level;
pub mod rnc;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CUE sheet: {0}")]
    Cue(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("format error: {0}")]
    Format(String),
}
