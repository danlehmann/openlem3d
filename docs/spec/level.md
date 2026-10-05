# `LEVELS/LEVEL.nnn`

Sources: [L3DEdit] (primary), [LF1590], [Owner].

## File layout (verified)

All 100 files are RNC-compressed and unpack to exactly 66,048 bytes. [L3DEdit]
notes that the game also accepts uncompressed level files.

| Offset | Size | Contents |
|---|---|---|
| `0x0000` | 512 | Header |
| `0x0200` | 32,768 | Block grid: 16,384 cells × 2 bytes |
| `0x8200` | 32,768 | Object grid: 16,384 cells × 2 bytes |

**File number by rating:**

| Rating | Levels | Files |
|---|---|---|
| Fun | 1–20 | `LEVEL.000`–`019` |
| Tricky | 1–20 | `LEVEL.020`–`039` |
| Taxing | 1–20 | `LEVEL.040`–`059` |
| Mayhem | 1–20 | `LEVEL.060`–`079` |
| Practice | 1–20 | `LEVEL.080`–`099` |

Level *n* of a rating is file `20·r + n − 1`, where *r* is the rating's
index in the table. How we verified it: in game, Fun 1 is "Take a Dive" =
`LEVEL.000`, and the briefing title matches. The mapping is also consistent
with every rating and level number that [L3DEdit] names (Fun 8, Fun 20,
Tricky 16, Taxing 5, Taxing 8, Mayhem 3, Mayhem 10, Mayhem 13).

### Grid order (verified)

The grid is 32 (X) × 16 (Y) × 32 (Z). Z varies fastest, then Y, then X:

`index(x, y, z) = (x·16 + y)·32 + z`

How we verified it: decoding with this order produces coherent structures (the
staircase in `LEVEL.001`), and each tree object coincides with one block-4
cell. See the BLK special blocks in [blk.md](blk.md).

## Header

The following fields are **verified** as plausible across all 100 levels with
`l3d-tool levels`: the values are in range and the titles read correctly.
Their *effects* in game are **unverified** unless stated.

| Offset | Size | Field |
|---|---|---|
| `0x000` | 1 | Time limit, seconds |
| `0x001` | 1 | Time limit, minutes |
| `0x002` | 8 | Unknown. Shaped like a camera record (see cameras below), e.g. `c0 fd 80 ff 80 0e 00 00` in `LEVEL.001` [L3DEdit] |
| `0x00A` | 18 | Skill set: 9 × (skill id, quantity) |
| `0x022` | 192 | Land polygons: 8 polygons × 8 vertices × (z, x, options) |
| `0x0E2` | 2 | Release rate (signed) |
| `0x0E4` | 2 | Number of lemmings |
| `0x0E6` | 2 | Number to save |
| `0x0E8` | 1 | Texture set: `GFX/TEXTURE.nnn` |
| `0x0E9` | 1 | Land texture: `GFX/LAND.nnn` |
| `0x0F0` | 32 | Title, NUL-terminated ASCII (verified) |
| `0x110` | 1 | Static object set: `GFX/OBJ.nnn` |
| `0x111` | 1 | Sign set: `GFX/SIGNS.nnn` |
| `0x112` | 1 | Sea texture: `GFX/SEA.nnn` |
| `0x113` | 1 | Animated object: `GFX/ANIMOBJ.nnn` |
| `0x114` | 1 | Interactive object type: `GFX/TRAPS.nnn` |
| `0x115` | 1 | Sky: `GFX/SKY.nnn` |
| `0x117` | 1 | Wall-decal set: `GFX/WALLS.nnn` |
| `0x118` | 1 | Water scroll speed, Z (signed) |
| `0x119` | 1 | Water scroll speed, X (signed) |
| `0x11B` | 2 | Render/behaviour flags (see below) |
| `0x11D` | 32 | Comment, NUL-terminated; in official levels, the author's name (verified) |
| `0x13F` | 2 | "Face render limit" (an engine budget; not relevant to us) |
| `0x143` | 1 | Theme (selects the preview screen and music) |
| `0x151` | 1 | Music variant within the theme |
| `0x153` | 4 | Kill boundary: min X, min Z, max X, max Z |
| `0x157` | 1 | More flags: bit 0 = slippery level bottom, bit 1 = slippery land |
| `0x159` | 3 | Preview-camera pivot (y, z, x) |
| `0x15C` | 1 | Kill ceiling (Y) |
| `0x15D` | 32 | Four preset cameras (see below) |

The remaining bytes are unknown. [L3DEdit] lists per-level observations for
many of them.

**Graphics-set fields:** the value `nnn` names the file `GFX/<KIND>.nnn`.
`0xFF` means "none".

**Theme:** values 0–10 are [L3DEdit]: none, Castle, Egypt, Space, Sweets, Golf,
Computer, Army, Maze, Circus, Lemgo. **Unverified.**

**Skill ids:** these follow the order of the in-game skill panel [L3DEdit].
**Unverified**; the order is still to be confirmed in game.

### Land polygons

Each of the 8 polygons has up to 8 vertices `(z, x, options)`. A vertex with
`z == 0` ends the polygon (verified: decoding `LEVEL.001` this way gives one
clean octagon). Coordinates name grid corners.

The following is **unverified** [L3DEdit]:
- Polygons are convex and wound anticlockwise.
- They are drawn at the bottom of the level with the `LAND` texture.
- The top 3 bits of `options` darken the texture; the low 3 bits pick a
  sub-texture in multi-texture `LAND` files.

### Flags at `0x11B` (unverified, [L3DEdit])

| Bit | Meaning |
|---|---|
| `0x0001` | Sea drawn as a solid colour |
| `0x0002` | Sky drawn as a solid colour |
| `0x0004` | Land invisible |
| `0x0008` | Minimap click-drag camera placement disabled |
| `0x0010` | Sea not animated |
| `0x0020` | Sea texture 128×128 |
| `0x0040` | Preview camera static |
| `0x0080` | Whole level bottom is solid |
| `0x0100` | Land texture 128×128 |
| `0x0200` | All-round sky (space levels); no sea |
| `0x0400` | Wall decals show the game screen (the "Lemmings Inside" monitor effect) |
| `0x0800` | Land texture drawn at half scale |
| `0x1000` | Minimap disabled |
| `0x2000`, `0x4000` | Unknown |
| `0x8000` | Lemmings are zapped instead of drowning |

### Preset cameras

There are four records of 8 bytes: `y:i16, z:i16, x:i16, rotation:u8, pad:u8`.

- **Positions** are signed 8.8 fixed point, relative to the grid point
  (16, 8, 16). So `grid = (16 + x/256, 8 + y/256, 16 + z/256)`. The fractional
  parts are always .5 for X and Z and .75 for Y in official levels.
  **Verified** for `LEVEL.001`.
- **Rotation** is quarter turns (0–3): 0 faces −X, 1 −Z, 2 +X, 3 +Z.
- **Storage order:** keys 1–4 in order (verified; [L3DEdit]'s "1, 2, 4, 3" is
  wrong).

See [camera.md](camera.md) for verification details and the projection.

## Block grid (cell format verified)

Each cell is a `u16` split as `iiiiii tttt rr ssss`, from MSB to LSB:

| Bits | Field |
|---|---|
| 15–10 | Block id: index into the level's `BLK` file |
| 9–6 | Shape |
| 5–4 | Rotation: quarter turns about Y. [L3DEdit] says each step is anticlockwise seen from above (unverified) |
| 3–0 | Segment mask: four vertical quarter-height slices, bit 3 = top. 0 means the cell is empty |

How we verified it: with this split, every used block id in every level
resolves to a defined `BLK` entry, or to the invisible special ids 3 and 4.
The few exceptions are listed in [blk.md](blk.md).

### Shapes (unverified, [L3DEdit])

"Facing +Z/+Y" means the sloped face's normal points towards +Z and +Y,
assuming rotation 0.

| Value | Shape |
|---|---|
| 0 | Cube |
| 1 / 2 | Half-height square pyramid, apex up / down (special segment rules) |
| 3 / 4 | Full-height square pyramid, apex up / down |
| 5 / 6 | 45° ramp facing +Z/+Y, or +Z/−Y (uses the +Z face texture) |
| 7 | Vertical 45° deflector wall facing +Z/+X (uses the +Z face texture) |
| 8 / 9 | 22.5° ramp on the upper / lower two slices |
| 10 / 11 | Outer corner where two 45° ramps meet (top / bottom) |
| 12 / 13 | Triangular-prism corner piece (top / bottom) |
| 14 / 15 | Crash the original game |

## Object grid (structure verified, semantics unverified)

Each cell is two bytes, `(kind, extra)`, in the same order as the block grid.
`extra`'s meaning is unknown.

| `kind` | Graphic |
|---|---|
| `0x00` | None |
| `0x01`–`0x14` | Static object N (1-based) from `OBJ`. See [graphics.md](graphics.md) |
| `0x20`–`0x3F` | Sign from `SIGNS`: high bits pick the graphic, low 2 bits pick the cell edge |
| `0x50`–`0x5B` | Animated object variants (centred, corner and half-width) |
| `0x60`–`0x67` | Interactive object (trap, trampoline, teleporter, …) from `TRAPS`; pairing by value |
| `0x70`–`0xEF` | Wall decal from `WALLS`: high nibble picks the face, low nibble picks the graphic |

**Verified:** in `LEVEL.001`, the nine `kind = 5` objects use the 64×64 tree
(the first 64×64 slot in `OBJ.004`, which is object #5 counting from 1).
[L3DEdit] covers placement and height rules, and the per-type behaviour of
interactive objects, in detail.

Objects are not solid. Official levels put an invisible block 4 in the same
cell where solidity is needed (verified for `LEVEL.001`).
