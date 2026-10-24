//! Level codes from the original game. All levels are unlocked anyway; a
//! code simply jumps to its level. Only codes verified in the original belong
//! here (see `docs/spec/level.md`, "Level codes").

/// The code of each rated level, by level file number: entry `n` opens
/// `LEVEL.n` (level `n + 1`). Every one was entered in the original and
/// opened that level's briefing. Twenty per rating: Fun, Tricky, Taxing,
/// Mayhem.
pub const LEVEL_CODES: [&str; 80] = [
    "LEWISIAN", "BLIMBING", "FANAGALO", "DRICKSIE", "KURTOSIS", "GREGATIM", "WALLAROO", "AVENTAIL",
    "GAZOGENE", "JINGBANG", "DIALLAGE", "BUNODONT", "NAINSOOK", "YAKIMONA", "FUMITORY", "CINGULUM",
    "BESLAVER", "ANABLEPS", "QUINCUNX", "TARLATAN", "KAMACITE", "GUMMOSIS", "PRODNOSE", "NGULTRUM",
    "COTTABUS", "BEDAGGLE", "EPICALYX", "HOMALOID", "LALLYGAG", "BILABIAL", "CACOFOGO", "METAVURT",
    "SLOWBURN", "PELLUCID", "MAKIMONO", "KHUSKHUS", "DISPLODE", "RACAHOUT", "ORGULOUS", "DUNCEDOM",
    "CABOCEER", "GEROPIGA", "BONTEBOK", "EMPYREAL", "LANGLAUF", "NANNYGAI", "SARATOGA", "QUINTAIN",
    "MUSQUASH", "ZOMBORUK", "SKILLING", "WOBEGONE", "BINDIEYE", "FRAXINUS", "LINDWORM", "CURLICUE",
    "HANEPOOR", "IDEMQUOD", "BLANDISH", "MALAGASY", "CHORIAMB", "GARGANEY", "KAOLIANG", "MAROCAIN",
    "OBTEMPER", "TASTEVIN", "VELLOZIA", "BORACHIO", "JACKAROO", "COOLAMON", "BANAUSIC", "FABURDEN",
    "RECKLING", "MIRLITON", "OPAPANAX", "BIMBASHI", "CAATINGA", "PENSTOCK", "SPRINGAL", "BABIRUSA",
];

/// The level a code opens, if it is a known code (case-insensitive).
pub fn level_for_code(code: &str) -> Option<u32> {
    let code = code.trim();
    LEVEL_CODES
        .iter()
        .position(|c| c.eq_ignore_ascii_case(code))
        .map(|n| n as u32)
}

/// The code that starts level `n` (0-based), if known.
pub fn code_for_level(n: u32) -> Option<&'static str> {
    LEVEL_CODES.get(n as usize).copied()
}
