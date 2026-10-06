# Camera and projection

Sources: our own observation of the running game (DOSBox-X harness,
`docs/original-game-navigation.md`), [L3DEdit], [Owner].

## Preset cameras (verified)

The four preset records in the level header (see [level.md](level.md)) map
to camera keys 1–4 **in storage order**: record *n* is camera key *n+1*.

How we verified it: we captured keys 1–4 in Fun 1 "Take a Dive"
(`LEVEL.000`) and compared them with our renderer using the records in
storage order. All four views match, and records 2 and 3 can't be swapped:
record 2 is 7 units higher than record 1 and frames the lower tower, while
record 3 is far back at z = −2.5 and frames the exit island.

**Wrong:** [L3DEdit] says the storage order is 1, 2, 4, 3. [Owner] says it is
sequential, which is correct.

**Position:** `grid = (16 + x/256, 8 + y/256, 16 + z/256)`. That is, the
stored values are relative to the grid point (16, 8, 16), where cell *c*
spans [*c*, *c*+1].

**Facing:** `rotation` counts quarter turns. 0 faces −X, 1 faces −Z, 2 faces
+X and 3 faces +Z. Rotations 0 and 3 are verified in `LEVEL.000` and
`LEVEL.001`.

## The camera never pitches (verified)

No control tilts the view. The horizon stays on the same screen row while the
camera rotates, moves or changes height. The sky is a flat band that only
slides sideways when the camera turns.

## Projection (verified, fitted)

- **The horizon is not in the middle of the screen.** Eye level is drawn at
  about **31% from the top** of the 640×480 screen, around row 150. This is
  an off-centre projection, as if the lens were shifted, rather than a
  tilted camera: vertical lines stay vertical.
- **The vertical field of view is about 51°** over the full 480-row screen,
  which is a focal length of about 500 pixels at 640×480.
- **Square pixels.** The 3D view covers the whole 640×480 screen; the HUD is
  drawn on top of it.

How we verified it: we rendered `LEVEL.000` from presets 1 and 4 at 640×480
with a 51° field of view and the horizon at 0.3125, then compared with the
original's captures. The entrance hatch, staircase edges, exit island and
the walkways in front of the tower all land within a few pixels of the
original.

Our renderer implements this in `openlem3d::projection` (`--fov` and
`--horizon` override the defaults for further fitting).

## Sky (verified for 1024×64 skies)

The sky is a flat backdrop, not part of the 3D scene:

- **Vertical extent:** the whole 64-row `SKY` image is stretched to fill the
  screen from the top edge down to the horizon (about 150 rows, a scale of
  about 2.34).
- **Horizontal scale:** each sky texel is drawn 2 screen pixels wide.
- **Panning:** turning the camera pans the image horizontally, and the
  1024-texel image wraps once per full turn. The sky therefore moves more
  slowly across the screen than the 3D world does.
- **Calibration:** facing +Z, sky column ≈928 is at the screen centre.

How we verified it:

- **Scale:** `l3d-tool img-find` locates bands of the original's sky
  (`LEVEL.000`, `SKY.006`, camera 1) in `SKY.006` scaled by 2 × 2.34. Two
  bands at different heights agree on the same column, with a mean
  difference under 3/255.
- **Pan rate:** a small turn moved the mountain strip by exactly 12 screen
  pixels (pixel-identical after shifting) while the world near the centre
  moved about 20 pixels. With a focal length of about 500 pixels, that
  corresponds to roughly 1900–2050 screen pixels per turn, consistent with
  2 × 1024.
- **All-round skies** (64,000-byte `SKY` files, used by levels with the
  "surround sky" flag, `0x0200`):
  - **Stored rotated (verified from the data):** 320 rows of 200 pixels, each
    stored row being one screen column. Read 320 wide, the file shows
    stripes. Read 200 wide, it is a coherent image whose top and bottom rows
    join seamlessly (mean difference 3.1, against 2.1 between neighbouring
    rows) while its left and right edges don't (89). Transposed, it is a
    320×200 panorama that wraps horizontally.
  - **Display (provisional):** we stretch it over the full screen height and
    pan it like the 1024×64 skies. Its scale and pan rate in the original are
    unchecked. Tiling the untransposed image showed visible seams (Fun 3).

## Ground (partly verified)

- **Sea:** drawn as an endless plane at the top of grid layer 0 (y = 1).
  Verified visually: island blocks in layer 0 sit flush with the water in
  `LEVEL.000`.
- **Sea texture, with the level flag `0x0020`:** the 16 KB `SEA` file is one
  128×128 texture.
  - Verified visually in `LEVEL.000`: read this way, the sea shows the
    original's even noise pattern. Read as 64-wide frames, it showed a
    visible tile grid.
  - Our density of 64 texels per grid unit is unverified.
- **Sea texture, without the flag:** a 64-wide strip, probably 64×64
  animation frames (unverified).
- **Land polygons:** drawn at the same height with the `LAND` texture. Shape
  verified visually in `LEVEL.000`.
  - **Texture scale:** with the level flag `0x0100` ("128×128 land"), one
    128×128 texture spans 4 grid units. Verified visually: the circuit-board
    floor of `LEVEL.065` and the grass of `LEVEL.000` match the original at
    that scale; half of it was clearly too fine.
  - **Without the flag:** 2 grid units (provisional).
