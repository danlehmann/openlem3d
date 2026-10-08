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

## Frame groups

The file doesn't say which cell ranges form which action, or which viewing
direction. The table below comes from the contact sheet (`l3d-tool mhc
--labels`) and from in-game captures of Fun 1 and Mayhem 1 (level 61), ranked
against the cells with `l3d-tool mhc-match`. Mirror relations come from
`l3d-tool mhc-mirrors`.

### Layout: five viewing angles, mirrored for the rest

Almost every action is stored as **5 consecutive blocks, one per viewing
angle, each holding the action's frames in playback order**. Within an
action, the blocks run from front to back:

| Block | Viewing angle (lemming's heading relative to the camera) |
|---|---|
| A0 | facing the camera (front) |
| A1 | front three-quarter |
| A2 | side profile |
| A3 | rear three-quarter |
| A4 | walking away (back) |

Five rendered angles cover 8 headings: the three headings on the other side
(the other three-quarter views and the other profile) are the A1–A3 cells
drawn mirrored left to right. The stored A1–A3 cells go round the lemming's
**left**: the A2 profile faces screen-left (nose to the left), so a lemming
walking to the screen's right is drawn with A2 mirrored. Verified by eye on
walker cells 12–17 (and the three-quarter cells 6–11 and 18–23); drawing
them the other way round made walkers move backwards. **Verified** for walkers: lemmings seen from
the side and from behind match both plain and mirrored cells (for example
cells 15 and 18 mirrored, and 24 mirrored, in different captures), and no other
cell range holds additional walker angles. Front and back views need no
mirroring of their own, but for symmetric cycles the second half of the
cycle is (nearly) the first half mirrored: walker cells 3, 4, 5 are 0, 1, 2
mirrored, 27–29 are 24–26 mirrored (mean RGB difference 18–32, against 50+
for unrelated cells). The engine might therefore mirror those too; the file
stores them anyway.

How the engine quantises the heading into these angles (for example 45°
sectors) is **not yet measured**.

Exceptions are actions whose pose is not left–right symmetric (turner,
floater): they store 8 blocks, one per heading, see the table.

### Cell ranges

"Verified" means the action was seen in the running game and the cells
ranked best (or matched by eye) against it; the method is given. Everything
else is **guessed** from the contact sheet's appearance.

| Cells | Action | Blocks × frames | Status |
|---|---|---|---|
| 0–29 | Walker | 5 × 6 (A0 0–5, A1 6–11, A2 12–17, A3 18–23, A4 24–29) | **Verified.** A4: lemming-cam burst (camera behind a walker) ranked 24, 25, 26, 27, 28, 29 in that cyclic order (mean diff 18–20). A2: a side-on walker ranked cell 12 best (diff 14.8, next other range 20.4). A0: walkers coming towards the camera ranked cells 2–5 best (weaker, 34–35, because lemmings overlapped). A1/A3: matched 13–23 (plain and mirrored) on lemmings walking at an angle. |
| 30–85 | Turner (pointing with one arm; head turns) | 8 × 7 (30–36, 37–43, 44–50, 51–57, 58–64, 65–71, 72–78, 79–85) | **Partly verified.** A turner seen side-on in Fun 1 ranked only cells from 72–78 (diff ≈ 31) over a 3 s burst, cycling through them. The blocks go round the lemming from the front by its left (front, front-left, left, back-left, back, back-right, right, front-right; verified by eye: block 2, cells 44–50, faces screen-left, and block 6, 72–78, faces screen-right with the pointing arm towards the camera; the reverse order made the arm swap sides as the camera circled, owner's observation). Seen from the front, cells 30–36 hold out the lemming's right arm (screen left) and 79–85 its left arm; drawn that way round (stored cells for a turner pointing to its right), our turners point where they send walkers. Pairs 30–57 and 58–85 are near-mirrors (diff 30–45), not exact copies, because the pointing arm changes sides. |
| 86–120 | Blocker (arms out, head turns left and right) | 5 × 7 | Guessed. The pose matches the blocker skill icon. Drawn side-on to its heading, so the arms point back the way it came and on the way it was going (as in the original, by the owner's observation). |
| 121–160 | Climber: reaching up with both arms, then pulling the knees up, as if hauling itself up a wall | 5 × 8 | Matched by eye to the owner's description of the original's climber pulling itself up (ours showed 538–562, which looked like a lemming rising while standing). The side block (137–144) reaches forward and up. **Not** the fall from a hatch (see 399–423): those falls ranked these cells clearly worse (mean diff 34–37 against 25–29 with a uniform-scale search). |
| 161–200 | Digger (crouched, scooping with one arm) | 5 × 8 | **Verified by eye**: a digger in Mayhem 1 showed this crouched scooping pose (side view, 177–192), then fell through the hole. Too small for a reliable `mhc-match` ranking. |
| 201–240 | Basher (a mallet in each hand) | 5 × 8 | Guessed. |
| 241–270 | Miner (pickaxe, swinging down and forward) | 5 × 6 | Guessed. |
| 271–295 | Bomber swelling up (hand to mouth, then inflating) | 5 × 5 | **Verified** for the last frames: during a nuke in Fun 1, the swelling lemming ranked 290 and 295 best (diff 20–25). The first frames (hand to mouth) were not ranked reliably. |
| 296–335 | Floater hanging from the umbrella (lemming only) | 8 × 5 (296–300, …, 331–335) | Guessed. 296–315 and 316–335 are near-mirrors (diff 30–55). Might instead be 4 angles × 10 frames. |
| 336–343 | Umbrella canopy (drawn above the floater) | 8 × 1, or 1 × 8 opening frames | Guessed. |
| 344–368 | Builder laying a brick | 5 × 5 | **Verified by eye**: a builder in Mayhem 1 raised a brick with the sack on its back, as in 349–363. |
| 369–398 | Drowning (sinking, arms up) | 5 × 6 | **Verified by eye**: a lemming that fell through a dug hole into water in Mayhem 1 showed only the head and raised arms above the water, as in these cells. |
| 399–423 | **Falling** (arms out, legs dangling) | 5 × 5 (A0 399–403, A1 404–408, A2 409–413, A3 414–418, A4 419–423) | **Verified** for A0 and A4, see "Falling" below. A1–A3 follow the block layout but were not seen in game. |
| 424–428 | Feet up, seen from above or below | 5 × 1, or 1 × 5 | Guessed. |
| 429–443 | Lying on the back, feet up | 5 × 3 | Guessed. Not the splat, which leaves no body: the lemming bursts into fragments (owner). |
| 444–473 | Builder walking between bricks (sack on the back) | 5 × 6 | **Verified by eye**: the Mayhem 1 builder walked between bricks with this sack (side view, as in 450–461). |
| 474–488 | Builder out of bricks (shrug) | 5 × 3 | Owner recalls the builder shrugging when out of bricks; drawn for 6 ticks (unmeasured) before it walks on. |
| 489–513 | Standing, arms out, falling over onto the back | 5 × 5 | Guessed: splat, stunned or "oh no". |
| 514–523 | Electrocution: normal and X-ray skeleton frames alternating | 5 × 2 | Guessed (pairs 514/515 front … 522/523 back). |
| 524–533 | Electrocution cloud (gathers, lightning, shrinks) | 1 × 10 | Guessed. View-independent. Drawn over a zapped lemming, spread over its death (unmeasured). |
| 534–537 | Smoke puff, shrinking (bomber explosion) | 1 × 4 | Guessed. View-independent. |
| 538–562 | Arms spread sideways, one leg kicking up | 5 × 5 | Unknown. Earlier taken for the climber (the back block resembles the climber skill icon), but it shows a lemming standing, not hauling itself up (owner). Perhaps sliding or balancing. |
| 563–567 | Film camera (the level's camera marker, not a lemming) | 5 × 1 | Guessed. |

Not found: an exit animation (lemmings were not seen exiting), the bomber
countdown digits (they are drawn above the lemming in game, so probably not
in this file), and the "inflated lemmings flying" seen after a nuke, which
looks like the swollen 271–295 frames moving through the air rather than
separate cells.

### Falling (verified from the front and the back)

A lemming falling from a hatch uses cells **399–423**, five blocks of five
frames, and shows one cell per tick:

- **Order:** a ping-pong over the five frames that starts in the middle:
  frames 2, 3, 4, 3, 2, 1, 0, 1, … of the block (back view: 421, 422, 423,
  422, 421, 420, 419, …).
- **Mirroring:** every cell is drawn **mirrored**, in both the front and the
  back view, where walkers use their cells as stored. Within a block, frame
  0 is close to frame 4 mirrored and frame 1 to frame 3 mirrored (mean diff
  17 and 32–36), but frame 2 is not symmetric, and the mirrored copies ranked
  clearly better (frame 2: 19 against 33+ from the front, 20 against 24 from
  the back; frame 3 mirrored against frame 1 plain: 18–20 against 20–24).
- **First ticks:** the first tick below the hatch already shows frame 2. No
  other pose appears at the start of a fall.
- **Long falls:** not observed. The falls measured lasted 6–8 ticks (about
  1–1.5 units). Whether longer falls switch to another pose (121–160?) is
  open.
- **Side and three-quarter views** (A1–A3) were not captured. We assume
  they follow the same rule: the block chosen as usual, then mirrored once
  more.
- **Landing:** after these short falls, no landing or stunned pose was
  noticed by eye; the lemming walked on.

How we verified it (DOSBox-X, input only through `AUTOTYPE` and the serial
mouse, captures at about 60 per second, so each tick appears in about four
consecutive captures):

- **Back view:** Mayhem 6 (`LEVEL.065`, code `TASTEVIN`), camera 1, which
  stands 4 units behind the hatch and faces the release direction. Falling
  lemmings were 40×60 px at 640×480. The first lemming released, tick by
  tick: 421m (mean diff 20.0, next 22.7), 422m (18.4, next 20.4), 423m/419
  (16.8, a tie, as the two cells are mirror images), 422m (22.0), 421m
  (22.1), 420m (17.4, next 422 at 19.4). From the fourth tick on, the next
  lemming overlaps its head, so those scores are weaker.
- **Front view:** Practice `LEVEL.090` (hatch (21,2,8)), camera 1, 3 units
  in front of the hatch and facing it. Falling lemmings were 50×75 px. One
  lemming, tick by tick: 401m (19.0, next other cell 24.7), 402m (19.7, next
  24.1), 399/403m (17.5/17.7), 402m (19.1, next 23.8), 401m (15.7, next
  26.5), then it reached the ground.
- Rankings used `mhc-match` with the sprite's tight bounding box (found with
  `img-blobs --background` on a frame without the lemming), over all 568
  cells. With a uniform `--scale` search instead, the original's narrower
  robe made every candidate score 25–38 and the ranking unreliable.

### Playback rate

Rough estimate, **unverified**: in the lemming-cam burst, the 6-frame walk
cycle repeated every 9 captures, about 0.39 s, so each cell is shown for
about 65 ms (≈ 15 frames per second). DOSBox-X ran with `cycles=max`, and
screenshots are not synchronised to the game's frames, so this needs a
better measurement (for example a frame counter in a long recording).

### How the cells were matched

1. `l3d-tool mhc --cells A..B --labels --scale 2` for contact sheets.
2. In game: pause, aim the camera, take a burst of screenshots (about 25
   per second) while the game runs, pause again.
3. `l3d-tool mhc-match --rect X,Y,W,H --scale MIN..MAX --cells A..B` ranks
   cells (plain and mirrored) at every position and scale inside the
   rectangle. Matches are reliable when the sprite is at least about 80
   screen pixels tall (in the 1280×960 capture); smaller sprites rank
   unrelated cells, so those were judged by eye instead. When the sprite
   is isolated, passing its tight bounding box (without `--scale`) ranks
   more sharply, because each cell is stretched to the sprite's own aspect
   ratio.
4. `l3d-tool mhc-mirrors` lists, for each cell, the other cell closest to
   its mirror image.
