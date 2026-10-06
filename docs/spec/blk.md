# `LEVELS/BLK.nnn`

Sources: [L3DEdit] (primary), [LF1590], [Owner].

## Pairing (verified)

`BLK.nnn` belongs to `LEVEL.nnn`, the file with the same number.

How we verified it: `l3d-tool blk-match` checks which block ids each level's
grid uses. Against the level's own `BLK`, every used id is defined, except for
the invisible special ids 3 and 4 and four isolated cases listed below.
Against `BLK.<texture set>`, 96 of 100 levels reference undefined entries.

**Wrong:** [Owner] says the level's texture-set byte selects the `BLK` file.
Its `BLK.004` examples for `LEVEL.001` therefore describe the wrong file.

**Exceptions to investigate:** these levels use ids whose definitions are
placeholders.

| Level | Ids |
|---|---|
| `LEVEL.004` | 43, 44, 45 |
| `LEVEL.011` | 25 |
| `LEVEL.015` | 34 |
| `LEVEL.069` | 62 |

## Layout (verified size)

Every file is exactly 1,408 bytes: 64 definitions × 22 bytes, uncompressed.

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | Unknown (one of 0, 1, 3, 6, 9, 12, 15 [L3DEdit]) |
| 1 | 1 | Unknown (always 0 [L3DEdit]) |
| 2 | 1 | Flags |
| 3 | 1 | Unknown |
| 4 | 3 | Face +Z |
| 7 | 3 | Face −Z |
| 10 | 3 | Face +X |
| 13 | 3 | Face −X |
| 16 | 3 | Face +Y (top) |
| 19 | 3 | Face −Y (bottom) |

**Unused slots:** these are filled with `FF FF 00` faces (verified in
`BLK.001`).

### Face (3 bytes)

| Byte | Field |
|---|---|
| 0 | Tile index in the level's `TEXTURE` file |
| 1 | Modifiers |
| 2 | Shading: 0 = unchanged, up to 8 = darkest [L3DEdit] |

### Flags (unverified, [L3DEdit])

| Bit | Meaning |
|---|---|
| `0x01` | Double-sided: faces are also drawn from inside |
| `0x02` | Steel (indestructible) |
| `0x04` | Liquid top: lemmings drown |
| `0x08` | Unknown |
| `0x10` | Not solid for lemmings |
| `0x20` | Lemmings never splat when landing on it |
| `0x40` | Slippery top |
| `0x80` | Not solid for the camera |

### Face modifiers (unverified, [L3DEdit])

| Bit | Meaning |
|---|---|
| `0x08` | Reverse side uses a neighbouring tile and is always transparent (special texture rules) |
| `0x10` | Ping-pong animation over this tile and the next 4 |
| `0x20` | Ping-pong animation over this tile and the next 3 |
| `0x40` | Special looping animations, keyed by the tile index (see [L3DEdit] for the table: tile 0 → four frames from tile 12, 1 → four from 16, 2 → eight from 0, 3 → four from 28, 4 → four from 32, 5 → four from 36, 6 → four from 0). Used in official levels with tiles 0–5 only. Verified by eye: on "The Bean Machine" (`LEVEL.002`) the blocks at (12–13, 5–6, 12) form a "Jelly Belly" panel only with this mapping; drawn as stored they show a plain chocolate tile |
| `0x80` | Palette index 0 is transparent |

## Special block ids (partly verified)

| Id | Meaning |
|---|---|
| 0 | **Entrance.** Works only when the top two slices are present and the bottom two are absent. At most four entrances are active. Rotation sets the lemmings' initial heading, but turns opposite to block rotation [L3DEdit]. |
| 1 | **Exit.** The doorway is the +Z face, turned with the rotation: 0 → +Z, 1 → +X, 2 → −Z, 3 → −X (verified in game for all four; see [behaviour.md](behaviour.md#exits)). Tile 8 animates to tile 11 while a lemming exits [L3DEdit]. In `BLK.001`, block 1 has tile 8 on +Z (verified). |
| 2 | **Splitter** where non-solid [L3DEdit]. |
| 3 | **Invisible solid.** Suppresses faces of neighbouring blocks that touch it, and is used around enclosed levels. Its slots hold placeholders (verified). |
| 4 | **Invisible solid**, used under objects that should be solid. Verified: in `LEVEL.001`, block 4 is steel, its faces are placeholders, and it occurs exactly at the 9 tree cells. **Wrong:** [Owner] calls it a red exit floor pad. |
| 5–8 | **One-way blocks**, destructible only towards +Z, +X, −Z and −X respectively (before rotation) [L3DEdit]. |
