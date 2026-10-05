# Raw graphics (`GFX/`)

Sources: [L3DEdit] `Graphics.txt` (widths from Pooty's notes), [LF1590],
[Owner].

The `GFX/<KIND>.nnn` files are headerless and uncompressed. Each byte is a
palette index into `GFX/LM3D.PAL`, and rows are stored top to bottom. Images
are stored as a single vertical strip, so the width is fixed per kind and the
height is `file size / width`. Verified by rendering to PNG with
`l3d-tool png <path> --width W --out file.png`.

## Palette `GFX/LM3D.PAL` (verified)

The file is 768 bytes: 256 × (R, G, B) with 6-bit VGA components, 0–63. The
maximum found in the file is 63. Scale a component to 8 bits with
`v << 2 | v >> 4`.

Index 0 is black. Indices 1–16 are a white-to-blue ramp.

Other palettes (`SCENE*.PAL`, `.SVP`) are also 768 bytes and belong to the
preview screens.

## Per-kind layout

| Kind | Width | Layout | Status |
|---|---|---|---|
| `TEXTURE` | 64 | 100 tiles of 64×64; tile `n` is rows `64n … 64n+63` | Verified: dimensions render coherently. The face-to-tile mapping is to be checked in the viewer. |
| `SKY` | 1024 | 1024×64 horizontal panorama (65,536-byte files) | Verified with `SKY.004` |
| `SKY` (64,000 bytes) | 200 | 200×320 star field, used by all-round space skies | Verified with `SKY.002` |
| `LAND` | 128 | 128×128 per texture; 32 KB files hold two textures, `LAND.038` (8 KB) holds a half one | 128×128 verified with `LAND.004` |
| `OBJ` | 64 | 20 sprites, packed as described below | Verified with `OBJ.004` |
| `SIGNS` | 64 | 16 graphics: 8 × 64×32, then 8 × 64×64 [L3DEdit] | Unverified |
| `WALLS` | 64 | 16 × 64×64 [L3DEdit] | Unverified |
| `ANIMOBJ` | 64 | 64×256: four 64×64 animation frames? | Unverified |
| `SEA` | 64? | 16,384 bytes: 128×128 or 64×256? | Unverified |
| `TRAPS` | 64 | 64×512: animation frames | Unverified |
| `BGRD` | 320 | 320×48 | Unverified |

## `OBJ` packing (verified)

The strip is 64×704. The 1-based object numbers used by the object grid map to
slots as follows:

| Objects | Size | Rows | Arrangement |
|---|---|---|---|
| 1–4 | 32×64 | 0–127 | Two per row: left, then right |
| 5–8 | 64×64 | 128–383 | One per 64-row band |
| 9–12 | 32×64 | 384–511 | Two per row |
| 13–16 | 64×32 | 512–639 | One per 32-row band |
| 17–20 | 32×32 | 640–703 | Two per row |

How we verified it: the strip's built-in cell borders follow exactly this
grid, and `LEVEL.001`'s object 5 is the tree in the first 64×64 band.

**Transparency:** the sprite background is palette index 0. **Unverified**;
the borders between cells appear to use a separate colour. [Owner] mentions
index `0x0F` for one strip format; still to be checked.

**Wrong:** [Owner] describes `OBJ` as "11 frames of 64×64". The byte count
matches that, but the content is the mixed-size grid above.
