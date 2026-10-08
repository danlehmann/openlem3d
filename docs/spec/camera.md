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

The virtual-lemming view looks out of the followed lemming's eyes (the
project owner: it is meant to be its eyes). Ours puts the camera 0.4 units
above its feet, the top of its half-unit-tall sprite, facing its heading,
and does not draw that lemming.

The virtual-lemming view does not tilt either, as far as observed: through
about 3 minutes of lemming view in the Practice "Virtual Lemming" demo and
an attract-mode demo (100–150 ms bursts), the sea horizon stayed level on
the same row as the preset cameras (row 163 of 480), vertical edges stayed
vertical, walking showed no bob beyond 1–3 px, and turns snapped without
roll. Not covered: the horizon was hidden while the lemming climbed a steep
path, and no fall, float, bounce, slide or climb was seen in the lemming
view.
The project owner sees a slight roll while the lemming walks in the
original (below what the bursts above resolved). Ours rolls the whole
camera, sky included, about 1.5° to each side once per walk cycle (6
ticks) while the followed lemming walks, easing back to level otherwise;
the original's 2D sky does not turn with it, a renderer quirk we do not
keep. Preset cameras never roll.

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
    pan it like the 1024×64 skies, but at 960 texels per turn (exactly
    three repeats) so a full turn shows no seam. Its scale and pan rate in
    the original are unchecked. Tiling the untransposed image showed visible seams (Fun 3).

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
  animation frames (unverified). We cycle them at 8 frames per second unless
  the level flag `0x0010` ("sea not animated") is set.
- **Sea drift (partly verified):** the header's water speeds (`water_speed_x`,
  `_z`, values −3 to 4, one level −33) are taken as texels per tick, the
  sea moving against their signs: on `LEVEL.080` (X −1, Z +1, 128×128 sea
  over 2 units) that is 0.22 units/s towards +X and towards −Z. Observed in
  the original's Practice "Blocker" demo with the camera still (camera 1,
  1.75 units above the sea, facing −Z): the near sea slid about 18 px/s
  right and 5 px/s up on the 640×480 screen, which works out to about
  +0.2 units/s in X and 0.4–0.9 units/s towards −Z. Directions and the X
  speed agree; the Z estimate rests on a 2–5 pixel vertical shift of a
  finely shrunk texture (±30 % or worse) and is not taken as a correction.
  A second run on `LEVEL.098` (same speeds and sea as `LEVEL.080`; camera 1
  held still, sea in front of the trampoline row) measured a steady shift of
  (+13, −7) px per 0.5 s, about 0.5 units/s towards +X (±15 %) and 0.8
  units/s towards −Z (±35 %). Directions agree again; the X speed disagrees
  with the first run (0.2), and both runs find Z faster than X although the
  header gives them equal magnitude. Left unchanged until a cleaner
  measurement. Whether the sea also changes shape in place stays open (the
  leftover mismatch grew with time, which perspective could also explain).
- **Land polygons:** drawn at the same height with the `LAND` texture. Shape
  verified visually in `LEVEL.000`.
  - **Texture scale:** with the level flag `0x0100` ("128×128 land"), one
    128×128 texture spans 4 grid units. Verified visually: the circuit-board
    floor of `LEVEL.065` and the grass of `LEVEL.000` match the original at
    that scale; half of it was clearly too fine.
  - **Without the flag:** the file is a strip of 64×64 textures, each
    spanning 2 grid units (the same texel density; provisional). Each polygon
    picks its texture with the low 3 bits of its options byte.
  - **Flag `0x0800`:** halves the span. The Practice levels use 64×64 grass
    at 1 grid unit per texture.

## Collision

Blocks stop the camera unless their definition has the flag `0x80` ("not
solid for the camera", [L3DEdit]); the original keeps Claustrophobic's camera
inside its starting room (observed by the project owner). We test the
cells within 0.2 units of the camera against the present segments and slide
along blocks axis by axis (provisional). Of the 400 preset cameras only
`LEVEL.062`'s camera 3 starts inside a block; a camera that starts
blocked moves freely until it is clear.

## Level preview (measured)

Measured in the original on Fun 1, Fun 2, Fun 10, Tricky 2 and Practice
"Blocker" (raw grabs at about 40 frames/s; yaw from the sky panorama's
position, the camera path fitted to block silhouettes):

- **Centre and height.** With a pivot set (header `0x159`, stored y, z, x),
  the camera circles the point (x, z) taken as grid coordinates (not cell
  centres) at height y. With the pivot unset (all zero, 54 levels), it circles
  the middle of the x/z extent of the visible blocks (block definitions whose
  six textures are all 255 excluded) at height 8.
- **Path.** Radius about 32 units (31.5 ± 0.5 with our projection), always
  looking at the centre, never pitching. It starts where camera 1 looks from
  and turns left without end (facing +Z, +X, −Z, −X in turn).
- **Speed.** 2.8125° (8 sky texels) per drawn frame: one turn in 1.8 s at
  70 frames/s, about 16°/s at 5.6. Ours turns at 45°/s, roughly what a
  15-frames-per-second machine showed.
- **Flag `0x0040`:** the camera holds camera 1's view still (`LEVEL.062`).
- **On screen:** no panel. At the top left, in white with a dark shadow:
  "Level N TITLE", "Number Of Lemmings", "N To Be Saved", "Release Rate",
  "Time M:SS Minutes", "Rating X" (they appear one by one from about 1.2 s
  in; ours shows them at once); along the bottom the left-mouse icon with
  "Continue" and the right-mouse icon with "Menu". The briefing fades out
  in about 0.65 s, black for 0.25 s, then the view fades in over 0.4 s
  (ours: the fade-in).
- **Input.** Left click or Space starts the level; right click returns to
  the menu; Enter and Esc do nothing. Ours: right click and Esc return,
  any other key, click or tap starts.
- Practice briefings offer "Demo" instead of a preview, with the same
  flyover running behind the briefing (ours too: no theme picture).
