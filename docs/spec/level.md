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
| `0x153` | 4 | Kill boundary in cells: **min Z, min X, max Z, max X** (see below) |
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

### Kill boundary (`0x153`): axis order

The four bytes are `[min Z, min X, max Z, max X]`, Z before X, like the land
vertices `(z, x)` and the camera records `(y, z, x)`. [L3DEdit] gives
`[min X, min Z, max X, max Z]`, which is **wrong**.

How we verified it: in the Practice levels `LEVEL.080`, `082` and `086`,
the boundary is `[8, 8, 19, 27…30]` and the hatch sits at x ≈ 20–27,
z ≈ 13.5. Read X-first, every hatch lies outside the boundary and every
lemming would die on release. Read Z-first, all of them are inside.

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
| `0x0100` | Land texture 128×128; without it, 64×64 (consistent with the file sizes of every level's `LAND` file) |
| `0x0200` | All-round sky (space levels); no sea |
| `0x0400` | Wall decals show the game screen (the "Lemmings Inside" monitor effect; [L3DEdit], unverified: the decals themselves hold a still of the game screen) |
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

**Wall decals on cell sides** (high nibble 7–A: +X, −Z, −X, +Z) sit on the
face between the cell and its neighbour on that side, facing whichever of
the two is open. Across all levels, 55 side decals are in an empty cell with
a solid neighbour, 51 in a solid cell with an empty neighbour, 10 between
two solid cells and 15 (one column on `LEVEL.058`) between two empty cells,
which we draw on both sides. Verified by the picture: on `LEVEL.007` ("Lemmings
Inside") graphics 0–14 of `WALLS.008` form a 5×3 monitor screen (a bezel
around a still of the game screen) in empty cells in front of the
monitor's body, and they only read left to right facing into those cells.
Whether level flag `0x0400` makes that screen live is unknown.

Objects are not solid. Official levels put an invisible block 4 in the same
cell where solidity is needed (verified for `LEVEL.001`).

## Level codes

The original's level codes ("passwords") are eight letters. Codes still work
in openlem3d (see `docs/GROUNDRULES.md`), so the table below is the
verified code list. Every row was **verified** in the running game; codes
not seen in game are deliberately left out.

**Where codes appear (verified).** After a level is completed (enough
lemmings saved), the results screen shows "Password :-" and a code; its
buttons are "Next Level" (left) and "Menu" (right). The code belongs to the
*next* level. A failed attempt shows no code. Replaying an already completed
Practice level ("Floater") showed "You've played this level before" and no
code.

**Entering codes (verified).** On the code screen (F2 on the main menu), a
valid code followed by Return goes straight to that level's briefing
("Level n  TITLE"), whatever the selected rating. Both codes below were
tested on a save where only level 1 of each rating was unlocked.

| Code | Level | How verified |
|---|---|---|
| `BLIMBING` | 2 (Fun 2, "That's Right", `LEVEL.001`) | Shown after completing Fun 1; entered on the code screen, it opened the briefing "Level 2 That's Right". |
| `FANAGALO` | 3 (Fun 3, "The Bean Machine", `LEVEL.002`) | Shown after completing Fun 2; entered, it opened the briefing "Level 3 The Bean Machine". |

**Two other third-party lists don't match this edition.** A list published on
megagames.com ("Lemmings 3D – Level Passwords") gives `STARTING` for
level 2, and cheatbook.de gives `NASTALK`. The game itself gives `BLIMBING`
for level 2, and both third-party codes had no effect (no briefing opened;
the game returned to the main menu). These lists probably belong to another edition, so none of
their codes are recorded here.

**Pattern:** none visible from two codes. Both are real words (a fruit and a
pidgin language), not encodings of the level number.

Completing a level also adds the next level to its rating's level list.

### Published code list [GameFAQs-L3D]

A public list [GameFAQs-L3D] gives 80 codes, one per rated level in order;
its *n*-th code is meant to open level *n* (`LEVEL.(n−1)`). Its codes 2 and 3
are the `BLIMBING` and `FANAGALO` above. Some of its level titles are wrong
(it calls level 3 "Bounce Bounce", which is level 5). The list also ends with
five codes that are not tied to a level (`SPACEAAA`, `EGYPTAAA`, `ARMYAAAA`,
`MAZEAAAA`, `LAMPWICK`); their effect is **unverified**.

The codes below were **verified by entry**: on the code screen of the real,
unpatched image, each opened the briefing showing the expected level number
and title, and the level then started. The other 44 codes of the list are
**unverified** and are not recorded here. openlem3d does not use this list;
`crates/openlem3d/src/codes.rs` holds only the codes shown by the game itself.

| Code | List position | Level opened |
|---|---|---|
| `BLIMBING` | 2 | 2 (Fun 2, "THAT'S RIGHT", `LEVEL.001`) |
| `FANAGALO` | 3 | 3 (Fun 3, "THE BEAN MACHINE", `LEVEL.002`) |
| `DRICKSIE` | 4 | 4 (Fun 4, "IT'S A RUN AROUND", `LEVEL.003`) |
| `KURTOSIS` | 5 | 5 (Fun 5, "BOUNCE BOUNCE", `LEVEL.004`) |
| `JINGBANG` | 10 | 10 (Fun 10, "CASTLE LEMMALOT", `LEVEL.009`) |
| `BUNODONT` | 12 | 12 (Fun 12, "ALPINE ASSAULT COURSE", `LEVEL.011`) |
| `YAKIMONA` | 14 | 14 (Fun 14, "SLIPPERY MAZE", `LEVEL.013`) |
| `BESLAVER` | 17 | 17 (Fun 17, "HOLE IN TEN", `LEVEL.016`) |
| `TARLATAN` | 20 | 20 (Fun 20, "ALILEMM'S", `LEVEL.019`) |
| `GUMMOSIS` | 22 | 22 (Tricky 2, "WHICH EXIT ?", `LEVEL.021`) |
| `NGULTRUM` | 24 | 24 (Tricky 4, "FORE!", `LEVEL.023`) |
| `COTTABUS` | 25 | 25 (Tricky 5, "BREAKOUT", `LEVEL.024`) |
| `EPICALYX` | 27 | 27 (Tricky 7, "FOLLOW THE YELLOW BRICK ROAD", `LEVEL.026`) |
| `BILABIAL` | 30 | 30 (Tricky 10, "TOOTEN LEMMING", `LEVEL.029`) |
| `METAVURT` | 32 | 32 (Tricky 12, "DOT TO DOT", `LEVEL.031`) |
| `MAKIMONO` | 35 | 35 (Tricky 15, "CHOCOLATE DROP", `LEVEL.034`) |
| `DISPLODE` | 37 | 37 (Tricky 17, "GARDEN MAZE", `LEVEL.036`) |
| `RACAHOUT` | 38 | 38 (Tricky 18, "PLAY TIME", `LEVEL.037`) |
| `DUNCEDOM` | 40 | 40 (Tricky 20, "KING CODER'S TOMB", `LEVEL.039`) |
| `GEROPIGA` | 42 | 42 (Taxing 2, "PICKY PLATFORM", `LEVEL.041`) |
| `LANGLAUF` | 45 | 45 (Taxing 5, "3D - A LEMMING ODYSSEY", `LEVEL.044`) |
| `SARATOGA` | 47 | 47 (Taxing 7, "IF THE TIMING IS RIGHT!", `LEVEL.046`) |
| `SKILLING` | 51 | 51 (Taxing 11, "CHAOS MAZE", `LEVEL.050`) |
| `FRAXINUS` | 54 | 54 (Taxing 14, "AWAY TEAM", `LEVEL.053`) |
| `CURLICUE` | 56 | 56 (Taxing 16, "THE ARENA", `LEVEL.055`) |
| `BLANDISH` | 59 | 59 (Taxing 19, "LEMMTRIS", `LEVEL.058`) |
| `MALAGASY` | 60 | 60 (Taxing 20, "DEATH SLIDE", `LEVEL.059`) |
| `KAOLIANG` | 63 | 63 (Mayhem 3, "TOWER OF LEMLAB", `LEVEL.062`) |
| `OBTEMPER` | 65 | 65 (Mayhem 5, "THE PRISONER", `LEVEL.064`) |
| `TASTEVIN` | 66 | 66 (Mayhem 6, "FAMILY TREE", `LEVEL.065`) |
| `JACKAROO` | 69 | 69 (Mayhem 9, "OVER THE TOP", `LEVEL.068`) |
| `FABURDEN` | 72 | 72 (Mayhem 12, "RAIDERS OF THE LOST LEMMING", `LEVEL.071`) |
| `MIRLITON` | 74 | 74 (Mayhem 14, "JELLY BELLY ISLANDS", `LEVEL.073`) |
| `OPAPANAX` | 75 | 75 (Mayhem 15, "HOLE IN ONE, TWO, THREE!", `LEVEL.074`) |
| `PENSTOCK` | 78 | 78 (Mayhem 18, "CASTLE PERALUS", `LEVEL.077`) |
| `BABIRUSA` | 80 | 80 (Mayhem 20, "FINAL MAZE", `LEVEL.079`) |
