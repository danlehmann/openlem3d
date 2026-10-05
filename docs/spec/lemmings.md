# Lemming sprites (`LEMM/LEMM.MHC`, `LEMM/LEMM128.MHC`)

Sources: GuyPerfect's description in [LF1590] (copied into [L3DEdit]
"lemm mhc format.txt").

## Format (verified)

The format is uncompressed. Each file holds square cells, 64×64 in
`LEMM.MHC` and 128×128 in `LEMM128.MHC`, stored as trimmed scanlines.

The file starts with a manifest of **568 entries**, 4 bytes each (2,272
bytes):

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | Bank: index of a 16 KB area of cell data |
| 1 | 1 | Flags: always 0 in `LEMM.MHC` |
| 2 | 2 | Offset of the cell within its bank |

- **Where a cell starts:** `0x8E0 + bank × 0x4000 + offset`.
- **Cell header:** `size` 16-bit offsets, one per row, relative to the start
  of the cell.
- **Each row:** a `lead` byte, a `trail` byte, then
  `size − lead − trail` palette indices. The `lead` and `trail` pixels are
  transparent.
- **Palette:** `GFX/LM3D.PAL`; index 0 is transparent.

How we verified it: `l3d-tool mhc` decodes all 568 cells of `LEMM.MHC`
without a single out-of-bounds row, and the contact sheet shows clean,
complete lemming frames.

**Wrong:** the sources call byte 0 the "animation". It is only the storage
bank. Banks hold 8–19 consecutive cells, and their boundaries don't line up
with action boundaries. Cells are effectively one flat list, indexed 0–567.

## Frame groups (to be determined)

The file doesn't say which cell ranges form which action, or which viewing
direction. Lemmings are pre-rendered from several angles relative to the
camera. A first look at the contact sheet shows these frame sets:

- **Walking:** several blocks of about 10 frames each, from different angles.
- **Blocker:** arms spread.
- **Falling** and **splat**.
- **Climbing.**
- **Digging and mining:** pickaxe.
- **Bashing.**
- **Floater:** umbrella frames, which are separate cells.
- **Builder:** carrying bricks.
- **Bomber:** swelling up, then a smoke cloud.
- **Electrocution:** blue skeleton, lightning cloud.
- **Drowning.**
- **Exiting.**
- A few non-lemming cells: smoke puffs and a trap mechanism.

The exact cell ranges, frame order, playback rate and viewing-angle
selection will be filled in by matching in-game captures against the sheet.
