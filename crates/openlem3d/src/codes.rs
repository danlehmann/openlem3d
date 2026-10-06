//! Level codes from the original game. All levels are unlocked anyway; a
//! code simply jumps to its level. Only codes verified in the original belong
//! here (see `docs/spec/level.md`, "Level codes").

/// Verified codes and the level file number each opens.
pub const LEVEL_CODES: &[(&str, u32)] = &[
    // Fun 2 "That's Right" (shown after completing Fun 1).
    ("BLIMBING", 1),
    // Fun 3 "The Bean Machine" (shown after completing Fun 2).
    ("FANAGALO", 2),
];

/// The level a code opens, if it is a known code (case-insensitive).
pub fn level_for_code(code: &str) -> Option<u32> {
    let code = code.trim();
    LEVEL_CODES.iter().find(|(c, _)| c.eq_ignore_ascii_case(code)).map(|&(_, n)| n)
}
