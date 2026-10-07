# Lemming behaviour

Implemented in `crates/l3d-sim`. Each value below is marked **verified**
(measured in the running original, with method and precision) or
**provisional** (chosen to be plausible, not yet measured). Where the
implementation still uses an older provisional value, the table says so.

## How the measurements were made

All measurements come from watching the original in DOSBox-X
(`docs/original-game-navigation.md`), mostly on the Practice levels
(`LEVEL.080`–`099`). Their hatches release onto long straight paths along
z = 13.5. The tools:

- `tools/original/burst.ps1` grabs timed screenshot series (down to about
  17 ms apart) into the git-ignored `temp/shots`.
- `l3d-tool img-changes` finds the frames where a screen region changes. On
  the HUD counters this gives the release times; on a lemming it gives the
  game ticks.
- `l3d-tool img-blobs` finds lemming sprites (by colour or against a
  background frame) and prints their positions. `img-montage` makes
  contact sheets for inspection.
- `l3d-tool level-map` prints a level's block layers, giving the geometry
  the lemmings walk on; `--steel` marks steel blocks as `S`.

Screen distances are turned into grid units with the verified projection in
[camera.md](camera.md): focal length about 503 px at 640×480, with the
horizon near row 150. Preset camera positions come from the level header.
Because the focal length is only known to about ±2%, speeds derived from
screen motion carry that uncertainty. Counts and timings don't.

**HUD counters (verified):** "IN" is the number of lemmings still to be
saved. It counts down when a lemming enters the exit. "OUT" is the number of
lemmings currently alive in the level. It goes up at each release and down
at each death.

## Coordinates and timing

- **Positions** are fixed point, with 256 sub-units per grid unit (`SUB`).
  The point tracked is the lemming's feet.
- **Simulation rate: 14 ticks per second (verified, ±1%).** Lemming sprites
  and positions change in discrete steps. In dense captures (about 60 frames
  per second) of walking lemmings, 92 changes took 6.55 s, which is 71.2 ms
  per tick. The release intervals below fit 71.4–71.8 ms per tick. The
  game's clock runs in real time: 1.00 s per displayed second over 12 s.
  14 Hz is 70 Hz VGA refresh divided by 5, which may be the underlying
  timer. The implementation runs at 14 ticks per second.
- **On slow machines the game slows down (verified).** Each pass of the
  original's main loop draws one frame and runs at most one tick, once 1/14 s
  has passed; unused time carries over, but missed ticks are never made up.
  So the game runs at the lower of 14 ticks/s and the frame rate. Measured
  in DOSBox-X on Practice "Blocker" at fixed CPU speeds: at max, 60000 and
  30000 cycles (about 60, 37 and 18 frames/s) it ticked 14 times a second
  with a real-time clock; at 15000 cycles (about 8.5–9 frames/s) and 8000
  (about 4.4) it ticked once per frame, and the clock, the release interval
  and walking all slowed to 0.59 and 0.31 of real time. The lemming's steps
  were identical at every speed, only further apart. On the PCs of 1995,
  heavier views will often have dropped below 14 frames/s, so the game
  will often have run slower than its design speed; 14 ticks/s is that
  design speed.

## Collision

- **Solid geometry** is the block shapes exactly as the renderer draws them:
  cubes clipped to their segment span, ramps, pyramids, corners and
  deflectors (`world::inside_shape`). Blocks flagged "not solid to lemmings"
  are ignored.
- **Ground plane** at y = 1, the top of layer 0. Inside a land polygon it is
  land; elsewhere it is water, unless the level flag "bottom is solid" is set.
  Verified: on the Practice levels, walkers that walk off the land die (OUT
  goes down).

## Release

**Interval: `101 − rate` ticks (verified).** Releases were timed from the
OUT counter:

| Rate | Measured interval | `(101 − rate)` ticks at 14 Hz | Sample |
|---|---|---|---|
| 99 | 0.143 s | 0.143 s | 58 releases, Fun 1 |
| 89 | 0.862 s | 0.857 s | 13 intervals, Fun 1 |
| 80 | 1.500 s | 1.500 s | 7 intervals, Fun 1 |
| 50 | 3.661 s (±0.004) | 3.643 s | 14 intervals, Practice "Basher" |

The original Lemmings formula assumed before (`(99 − rate)/2 + 4` frames at
17 Hz) is **wrong** for L3D: it gives 0.79 s at rate 80.

**− and + (verified):** the panel shows two numbers under the buttons: the
level's release rate on the left and the current rate on the right. +
raises the current rate up to 99. − lowers it, but never below the level's
rate. With the button held, the value changes very quickly: from 80 to 99 in
under 0.5 s while paused, and 12 steps in a 150 ms hold. A press shorter
than about 120 ms changed nothing. A press takes effect while the game is
paused.

**Direction (verified for all four rotations):** an unrotated entrance
releases towards **−Z**, and each rotation step turns the direction
−Z → +X → +Z → −X. [L3DEdit] says +Z → +X → −Z → −X, which is right for
rotations 1 and 3 but **wrong** for 0 and 2.

- Rotation 0 → −Z: **verified** on two levels. Practice "Rope Slide"
  (`LEVEL.096`, hatch (20,7,23)), camera 1 facing +X (screen left is −Z):
  each lemming appeared under the hatch at screen x ≈ 379 and walked
  straight to the left at about 48 px/s, onto the rope slide that runs
  towards −Z. Tricky 1 "Jelly Climber" (`LEVEL.020`, hatch (15,3,30) at the
  +Z end of a path), camera 1 facing +X: new lemmings dropped at screen
  x ≈ 449 and walked left (−Z) along the path, 14 px per 0.5 s.
- Rotation 1 → +X: **verified** on Fun 1 (`LEVEL.000`, hatch (3,2,16)).
  Camera 1 faces +Z, so +X is screen left. The lemmings walk out to the
  left, along the hatch's row, onto the ramp at x = 6.
- Rotation 2 → +Z: **verified** on Practice "Turner" (`LEVEL.081`, hatch
  (19,2,8) on a path along x = 19). Camera 2 faces −X, so +Z is screen left:
  every lemming came in from the screen right (z = 8 side) and walked left
  to the junction at z = 13. Camera 3, at the +Z end looking −Z, saw them
  walk towards the camera.
- Rotation 3 → −X: **verified** on Practice 1, 3, 5, 6, 8 and 9
  (`LEVEL.080`/`082`/`084`/`085`/`087`/`088`). Lemmings walk from the hatch
  towards the exit at x = 14.

**Implemented** as clockwise steps from −Z.

**Release point (partly verified):** the lemming drops from the hatch and
walks along the centre line of the hatch's row. On the Practice paths, the
screen speeds of walkers seen from camera 1 (z = 20.5) and camera 3
(z = 5.5) on the same path put the path at z = 13.49. That is the cell
centre, 13.5, and the result doesn't depend on the focal length. The
position along the release direction was not measured.

## Movement

| Quantity | Value | Status |
|---|---|---|
| Walking speed | 1/32 unit per tick (0.4375 units/s) | **verified ±3%** (implemented) |
| Falling speed | grows by 8 sub-units per tick each tick (8, 16, … 56), then ¼ unit (64) per tick; for the first 12 ticks the lemming drifts on at walking speed (8 per tick, 0.375 units in all), then drops straight down | **measured** tick by tick (Practice "Trampoline", lemmings walking off the cube, four identical falls; the cap could be 60–64). An earlier coarse estimate was 1/6 unit per tick for 4 ticks, then ¼ |
| Fall height that splats | more than 2 units, at most 4¼ units | **bracketed**; exact value provisional (implemented: more than 4) |
| Highest step climbed without turning | at least ¼ unit | **partly verified**: walkers climb builder steps of ¼. Steps of 1 and 2 units turn them around. ½ untested |
| Largest drop stepped down without falling | ¼ unit | provisional |
| Headroom needed to walk | ½ unit | provisional |
| Climbing speed | about 0.04 unit per tick (≈0.6 units/s) | **rough** (implemented: 1/25 per tick) |

How each value was measured:

- **Walking speed.** On Practice "Basher" (camera 1, path 7 units away), a
  walker tracked for 11 s moved 31.1 px/s, which is 2.22 px per tick or
  0.0309 units per tick. On Fun 1 at rate 89, walkers spaced 12 ticks
  apart were 24.0 px apart at 8 units' depth: 0.0318 units per tick. Both
  agree with 1/32 (8 sub-units) per tick to within the focal-length
  uncertainty.
- **Falling.** On Practice "Floater" (camera 3), a walker stepped off the
  slab at y = 7¼, x = 20 and fell 6¼ units to the ground at 10.4 units'
  depth. That took about 25 ticks. The feet moved 7–9 px per tick for the
  first four ticks, then a steady 12–13 px per tick (0.26 units per tick).
- **Splat.** Lemmings died with a burst of blue fragments after falling
  6¼ units (Practice "Floater") and 4¼ units (Practice "Miner", from
  y = 5¼). A climber on Practice "Climber" dropped 2 units off each of two
  walls and was then saved. Falls of 1 unit (hatch on "Basher") and
  1½ units (hatch on "Bomber") were survived.
- **Turning.** Walkers turn around at a 1-unit crate (Practice "Basher") and
  at a 2-unit wall (Practice "Climber").

- **Headings:** lemmings walk along one of the four axis directions. A wall,
  meaning a step higher than the step limit or no headroom, turns them around
  (180°).

## Exits

A walker enters an exit block (id 1) when it walks into the block through its
doorway. The doorway is the block's +Z face turned with the block's rotation
[L3DEdit], one quarter turn per step: rotation 0 → +Z, 1 → +X, 2 → −Z,
3 → −X. **Verified for all four rotations** (the same turning order as the
entrance's release direction, but starting from +Z instead of −Z):

- Rotation 0 → +Z: Taxing 1 "Spaghetti Junction" (`LEVEL.040`), exit house
  (8,1,7). Camera 4 (looking −Z from z = 14.5) shows the house door head-on,
  with the path leading up to it from the +Z side.
- Rotation 1 → +X: on the Practice levels the exit (14,1,13) has rotation 1.
  Its house door faces +X (camera 4, looking −X, sees the door head-on). A
  climber walking −X along the path was saved there, entering through that
  face.
- Rotation 2 → −Z: Fun 3 "The Bean Machine" (`LEVEL.002`), exit (8,5,22) on
  top of a cake. Camera 4 (looking +Z from z = 15.5, x = 8.5) shows the
  doorway in the cake's −Z side, facing the camera.
- Rotation 3 → −X: Practice "One Way" (`LEVEL.093`), exit house (21,1,13);
  camera 2 (looking −Z) shows its door on the −X face.

Only the rotation-1 case was confirmed by a lemming being saved through the
door; the others are seen in the graphics.

**Corroborated by the level data.** `l3d-tool level N` prints which side
neighbours of each exit are empty. Over all 100 levels, exits with exactly
one open side mostly have it on the predicted face: rotation 0 → +Z (17
of 17), 1 → +X (39 of 41), 2 → −Z (7 of 7), 3 → −X (10 of 11). The
exceptions (`LEVEL.009` and `LEVEL.027`: rotation 1 with only −X open; `LEVEL.012`: rotation 3 with only +X open;
`LEVEL.011` and `LEVEL.064`: rotation 2 with +X, −X and +Z open but −Z
blocked) were not examined; the blocking neighbour may be a non-solid or
decorative block. Exits drawn as flat pads (Tricky 1, `LEVEL.020`) show no door.

## Skills

Skill ids follow the skill-panel order: blocker, turner, bomber, builder,
basher, miner, digger, climber, floater. Evidence: [L3DEdit] says the ids
follow the panel, and the Practice level titles (`LEVEL.080`–`088`) run in
the same order. **Verified** in game: the panel icons run in this order, and
the Practice menu's first nine items are these skills.

**Assigning (verified):** select the skill icon (this works while paused),
then click a lemming while the game runs. Clicks on a paused game didn't
assign.

**A turner takes two clicks (verified).** The first click on a lemming (with
the turner selected) puts a white arrow over it; the lemming keeps walking
and the arrow follows it. A second click, with the pointer moved to one side
of the lemming, makes it a turner where it then stands. On Fun 1 (camera 3,
looking +Z), a second click about 50 px to the screen left of the lemming
made the following walkers turn towards screen left (+X). Which side gives
which turn in general was not tested further.

Terrain is changed one quarter-height segment at a time. The basher and the
digger were seen to remove segments across a whole cell at once. Whether
steel stops them, and when exactly a basher stops, is untested; the
implementation never removes steel (provisional). None of the levels
reachable so far (level 1 of each rating, Fun 2 and 3, the Practice levels)
puts a basher, miner or digger next to steel: their only steel cells
(`level-map --steel`) are the entrance and exit blocks, which are flagged
steel, and solid blocks around the exits (the Practice exit house; invisible
block 4 under the trees by the Fun 2 exit). Testing it needs a later level code.

| Skill | Behaviour | Status |
|---|---|---|
| Blocker | Stands still. A walker turns around when its centre comes within about ½ unit of the blocker's centre, measured along the walker's path. The width of the area across the path is not measured. | **turn distance verified: 0.48 ± 0.05 unit** (implemented: a 0.48-unit square) |
| Turner | Stands still and keeps standing after turning walkers. It points one arm sideways, and walkers that reach it leave in the direction the arm points. A turner assigned while walking +Z pointed +X, and walkers arriving along +Z were sent to +X: the turner's own left. Walkers arriving from other directions were not tested. | **verified for one heading** (implemented: the second click picks the side, as seen on screen, and walkers leave a quarter turn that way; Practice "Turner" needs the turner's right, −X, so both sides must be possible. A walker turns once it is level with the turner, so it leaves along the turner's own line; Practice "Claustrophobic" needs that, since a line half a unit short runs into a wall) |
| Bomber | A countdown 5…1 over the lemming's head, each digit for about 8 ticks (0.57 s). After about 10 more ticks the lemming swells and explodes, about 3.6 s after assignment. The blast left a hole roughly one cell long in the 1-unit path blocks. | **timing verified ±1 tick per digit**; blast shape rough (implemented: 50-tick fuse, 1-unit radius) |
| Builder | Lays thin bricks, each about 0.55 unit long in the walking direction and about 0.28 unit higher than the last (¼ up per ½ forward within the measurement error), climbing onto each. One assignment laid **6 bricks**, one every **25 ± 1 ticks** (1.8 s). After the last brick the builder walked on off the end of the staircase and fell. A shrug was not seen. Brick width across the path is not measured. | **verified: count, timing, rise and run (±10%)** (implemented: 6 bricks, 25 ticks each, ¼ up and ½ forward. Bricks are free-standing slabs, not grid segments: 9/16 unit long, ¼ thick, 1 unit wide (width unmeasured). A brick that would run into terrain is cut short at it, and the builder stops; Practice "Builder" relies on the last brick meeting the platform edge) |
| Basher | Bashing a 1-unit crate took about 2.1–2.4 s (30–34 ticks): cracks spread over the crate, then the lower ½ unit (two segments) of the whole cell vanished at once, leaving the top half hanging. Walkers then walked through the gap. The basher went back to walking after the crate. | **rough** (implemented: every 32 ticks removes the two segments from the feet up in the cell ahead) |
| Miner | Removes a chunk about one cell long ahead of it at once after a crack animation, roughly every 3.5 s. The slope and depth per stroke could not be read from the views used. | **provisional; timing rough** (implemented: every 49 ticks the miner moves ¼ forward and ¼ down (45°) after clearing the space its body needs there, from the new feet to ¾ unit up and from the feet to ½ unit ahead. It stops at steel, at a one-way block facing the other way, when nothing is left to remove, and at the level bottom; it falls if it breaks out into the open. In the Practice "Miner" demo one miner started about 2.5 units west of the hatch (x ≈ 24.4, under the "MINE HERE!!" arrow) and dug an open-topped, stepped trench down to the ground at the tower's west end, about 1:1 on average (observed); ours from the same spot does the same) |
| Digger | Removes the top ¼-unit segment of the **whole cell** under it at once, after a crack animation, about every 2.8 s (≈ 40 ticks, ±20%). It dug through a 1-unit block in about 8.4 s and then fell. | **rough** (implemented: one segment every 40 ticks) |
| Climber | Climbs vertical walls at about 0.6 units/s, then walks over the top and drops off the far side like a walker (2-unit drops survived). Behaviour at a ceiling is untested. The skill is permanent: one climber climbed both walls on Practice "Climber". | **verified** (speed rough) |
| Floater | Falls normally at first; the umbrella opened about 7 ticks (0.5 s) and about 1½ units into the fall. It then falls at **1/32 unit per tick** (0.45 units/s, the walking speed) and survived a 6¼-unit fall that kills non-floaters. | **float speed verified ±5%; survival verified for 6¼ units; opening point rough** (implemented: opens after 1½ units, then 1/32 unit per tick) |
| Nuke | Stops releases and gives every lemming a fuse, staggered by one tick each. | provisional |

### How the skills were measured

All on the Practice levels, with timed screenshot series (`burst.ps1`,
100 ms apart unless noted) started just before the assigning click. Screen
distances use the focal length of about 503 px; the depth is the distance
from the preset camera to the lemmings' row.

- **Blocker** (`LEVEL.080`, camera 1 facing −Z, path 7.5 units away, so
  67 px per unit; 150 ms frames). The blocker's sprite centre stayed at
  screen x = 426.3. Walkers coming from the hatch reversed with their
  centre at x = 388.5–394.0 (four approaches), 32–38 px short of it:
  0.48–0.56 unit. With the 2 px walking step per tick and the sprites'
  different shapes, that is 0.5 ± 0.05.
- **Turner** (`LEVEL.081`). Camera 3 sits at the +Z end of the x = 19 path
  looking −Z. The turner, assigned to a walker heading +Z, faced the camera
  with its arm out to screen right (+X). The next walkers left the path
  towards screen right. Camera 2 (looking −X) showed the same walkers turning
  at the junction towards the camera (+X) while the turner stayed put for
  more than 10 s and at least three turned walkers.
- **Builder** (`LEVEL.083`, camera 1 facing −Z, 7.5 units, 67 px per unit).
  Thin brown bricks were found by differencing against the frame before the
  click. Their tops are at screen y = 262, 239, 223, 203, 187 and 167
  (spacing 16–23 px, mean 19 px ≈ 0.28 unit) and they are 35–40 px
  (≈ 0.55 unit) long, stepping 37 px sideways each. Bricks 1–5 first
  appeared 4.4, 6.3, 8.1, 9.9 and 11.6 s after the start of the series:
  1.80 s apart, 25 ticks. The sixth brick is hidden by the builder until it
  walks off at 14.9 s. No seventh brick appeared in the next 25 s. A first
  run gave the same staircase.
- **Basher** (`LEVEL.084`, camera 2, the crate at 3.5 units). The crate
  (one full cube at (20,1,13)) cracked for about 2.1–2.4 s, then its lower
  part vanished in one frame. The remaining bottom edge projects to
  y ≈ 1.4, close to the 1.5 expected for two removed segments.
- **Digger** (`LEVEL.086`, camera 1, 8.5 units, 59 px per unit). The bottom
  of the hole dropped in steps of 15–17 px (≈ ¼ unit) at about 5.7, 9.6,
  13.0 and 14.2 s into the series; the irregular spacing (crack animation,
  occlusion by the digger) gives the ±20%.
- **Miner** (`LEVEL.085`, camera 2 from the side). Two chunks were removed,
  about 3.5 s apart, each about one unit long. The near-side blocks along the
  path hide the floor of the cut, so slope and depth are not measured.
- **Floater** (`LEVEL.088`, camera 3 facing +X, the tower 10.5 units away,
  48 px per unit). A floater stepped off the slab at y = 7¼. Its feet fell
  97 → 148 px in four frames (full falling speed), and the umbrella canopy
  was first visible 0.6 s after leaving the edge, about 75 px (1½ units)
  down. The canopy top then moved from y = 110 at 6.01 s to y = 300 at
  14.80 s: 21.6 px/s, 0.45 units/s, 0.032 unit per tick. The OUT counter
  dropped only at the regular deaths of the non-floaters that followed it.

## Terrain and objects

| Feature | Behaviour | Status |
|---|---|---|
| Deflector (shape 7) | A walker meeting the diagonal face turns a quarter, as if reflected, once its centre reaches the face, wherever the face crosses its body (a deflector hanging ¼ unit above the floor in "Team Work" still turns walkers); the square backs are ordinary walls. Practice "Deflector" is a square track with deflectors at the corners. | provisional (implemented) |
| Splitter (block 2, non-solid) | Walkers crossing the cell's centre are sent alternately left and right [L3DEdit]. In the Practice "Splitter" demo the first lemming seen went to its own right and the next to its left; both exits received lemmings. | first side **observed**: in a second run the very first lemming to reach the splitter went to its own right (+X, towards the exit house at x ≈ 23), the camera still and no lemming having reached the cross path before; implemented: right first, then alternating |
| Liquid top (block flag `0x04`) | Walkers that step onto it drown, as well as lemmings that land on it (Practice "Mud"). | provisional (implemented) |
| Slippery top (block flag `0x40`) | Lemmings on it are in a "Sliding" state (shown when hovering) at about **3.1× walking speed** (2.9–3.4, from bursts scaled by sprite height), drawn with both arms spread; deflectors turn them and they keep sliding; off the ice they walk again. Whether skills can be given to sliding lemmings was not observed (the serial mouse could not assign skills at all). | speed and state **observed** (Practice "Slippery"); implemented: 3× walking speed, and no standing skills (blocker, turner, builder, basher, miner, digger) can be given while sliding, which makes Practice "Slippery" a lesson in placing the turner off the ice (provisional) |
| Killing traps (types 0, 1, 2, 6, 7) | Take a lemming that steps onto their cell, then stay busy for 2 s, letting others pass. | provisional (implemented) |
| Teleporter (type 5) | A walker reaching the centre of a pad stops there, turns to face the camera and stands about 0.3 s; it is squeezed to a thin vertical sliver and then a dot (about 0.2 s); a light-blue sparkle (zig-zag lines and stars) shows above the pad for about 0.5 s, then one at the partner (same value; with more, the last two pair [L3DEdit]) for about 0.5 s; the lemming stretches out again there (about 0.3 s), stands about 0.3 s and walks on along the path leading away. From vanishing to reappearing on the minimap took 0.80–0.88 s; about 2.1 s in all from reaching the pad centre. The pads work both ways. | **observed** (Practice "Teleporter" demo, bursts about 47 ms apart); implemented: stand 4 ticks, squeeze 3, away 12, stretch 4, stand 4 (about 1.9 s), facing the camera; the pad flat on its block (`TRAPS` frame 0), the sparkle from `TRAPS` frames 4–7, one per tick, over the pad left for the first 6 ticks away, then over the other |
| Rope slide (type 8) | A lemming reaching a sender (even value) jumps up to a handle and hangs for about 0.6 s, then slides down hanging under the rope, arms raised, to its receiver (value + 1; the last of each value works [L3DEdit]). In Practice "Rope Slide" it took **6.35 ± 0.1 s from grab to release**; it lets go at the low anchor, shows a landing pose for about 0.3 s, then walks on in the direction of the slide. Whether it accelerates is not known. | timing **observed**; implemented: 8-tick hang, then a straight line at 35/256 unit per tick, matching the total time |
| Spring (type 4) | A walker stepping onto the red sender pad is thrown upwards in a high, flailing arc onto its receiver (the blue pad), about **1.6 ± 0.2 s** for about 16 units in Practice "Catapults", appears standing on the pad and walks on in its flight direction. | **observed**; implemented: 1.4 ticks per unit, arc peak 0.3 of the distance (unmeasured) |
| Trampoline (type 3) | **Measured tick by tick** (Practice "Trampoline", side view with the preset camera back-projected; 14 lemmings, pixel-identical paths). In the air a lemming moves forward exactly 32 sub-units (1/8 unit) per tick; on the tick it touches a pad it does not move on. Hop *n* (from 1) lasts 8*n* + 6 ticks from pad to pad (14, 22, 30), so it is *n* + 0.625 units long (1.625, 2.625, 3.625: each one a unit longer), and *k* ticks in it is 120·*k*·(*N* − *k*)/(*N* + 1) sub-units above the take-off height (apexes 1.54, 2.50, 3.33 units). There is no snapping: each hop starts where the last landed. A walker stepping down onto the first pad stands on it two ticks (drifting on at walking speed), then takes hop 1. Hop 6's descent onto the floating cube also fits. Pads are red (objects 0x60–0x63) or blue (0x64–0x67). | **measured** (red pads, hops 1–3 directly, 4–6 by extrapolation and hop 6's descent); implemented exactly. **Inferred:** landing on a red pad gives the next hop one longer; landing on a blue pad one shorter, and off a blue pad after hop 1 the lemming walks on (with this, as in the demo, lemmings turned on the cube bounce down the column's blue pads and the blue back row and walk into the exit; one turner saves 19). A drop onto a pad gives the shortest hop that rises higher than the drop (fits hop 1 from the first pad and Fun 1 "Take a Dive" [LU3DWalk], where a 10-unit drop from the third platform must carry lemmings into the exit house). Pads show their dip (frames 1–3 of their colour, one per tick) only when touched (observed: untouched pads never change) |


The interactive-object type of a level is its trap file number
([L3DEdit]); the Practice levels "Rope Slide", "Catapults", "Trampoline"
and "Teleporter" use 8, 4, 3 and 5, which agrees.

## Deaths

| Cause | Result |
|---|---|
| Landing in water outside land polygons, or on a block flagged liquid | Drowning (walking off the land: verified) |
| Landing after a fall longer than the splat height | Splat, unless the block is flagged "no splat". The lemming vanishes in a burst of blue fragments (verified) |
| Leaving the kill boundary (header `0x153`) or rising above the kill ceiling | Zapped |

Each death animation lasts about 1 s (provisional).
