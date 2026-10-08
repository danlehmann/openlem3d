//! Interactive objects: object-grid kinds `0x60`–`0x67`, whose behaviour is
//! set level-wide by the level's trap type (the `TRAPS` file number;
//! [L3DEdit] "LEVEL.xxx", interactive objects).

use l3d_formats::level::{BlockCell, Level};

use crate::SUB;

/// What a level's interactive objects are, by trap type ([L3DEdit]; the
/// Practice levels "Rope Slide", "Catapults", "Trampoline" and "Teleporter"
/// use types 8, 4, 3 and 5, which agrees).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    FlameBlower,
    Squasher,
    BearTrap,
    Trampoline,
    Spring,
    Teleporter,
    WeirdTrap,
    Laser,
    RopeSlide,
}

impl ObjectKind {
    pub fn from_trap_type(t: u8) -> Option<Self> {
        use ObjectKind::*;
        Some(match t {
            0 => FlameBlower,
            1 => Squasher,
            2 => BearTrap,
            3 => Trampoline,
            4 => Spring,
            5 => Teleporter,
            6 => WeirdTrap,
            7 => Laser,
            8 => RopeSlide,
            _ => return None,
        })
    }

    /// Traps that kill the lemming that sets them off.
    pub fn kills(self) -> bool {
        use ObjectKind::*;
        matches!(self, FlameBlower | Squasher | BearTrap | WeirdTrap | Laser)
    }
}

/// One interactive object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Object {
    pub cell: [i32; 3],
    /// The object-grid value, `0x60`–`0x67`; pairs objects that work
    /// together (teleporters, rope slides, springs).
    pub value: u8,
    /// Height of the surface the object sits on, in sub-units.
    pub surface: i32,
    /// Ticks left before a trap can fire again.
    pub busy: u32,
}

impl Object {
    /// Whether a lemming with its feet at `p` is on the object.
    pub fn touches(&self, p: [i32; 3]) -> bool {
        p[0].div_euclid(SUB) == self.cell[0]
            && p[2].div_euclid(SUB) == self.cell[2]
            && (self.surface - SUB / 8..=self.surface + SUB / 4).contains(&p[1])
    }
}

/// The level's interactive objects. Objects that need a block to stand on
/// (everything but the killing traps) are skipped where their cell is empty
/// ([L3DEdit] "Objects vs Blocks").
pub fn level_objects(level: &Level) -> Option<(ObjectKind, Vec<Object>)> {
    let kind = ObjectKind::from_trap_type(level.trap_type)?;
    let objects = level
        .cells()
        .filter(|(_, _, _, _, o)| (0x60..=0x67).contains(&o.kind))
        .filter_map(|(x, y, z, b, o)| {
            let surface = match top(y as i32, b) {
                Some(t) => t,
                None if kind.kills() => y as i32 * SUB,
                None => return None,
            };
            Some(Object {
                cell: [x as i32, y as i32, z as i32],
                value: o.kind,
                surface,
                busy: 0,
            })
        })
        .collect();
    Some((kind, objects))
}

/// Top of the segments present in a cell, in sub-units.
fn top(y: i32, b: BlockCell) -> Option<i32> {
    (b.segments != 0).then(|| y * SUB + (8 - b.segments.leading_zeros() as i32) * SUB / 4)
}
