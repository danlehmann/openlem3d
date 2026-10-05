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

**The implementation still uses the [L3DEdit] order** (rotations 0 and 2
swapped).

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
| Falling speed | about 1/6 unit per tick for the first ~4 ticks, then about 1/4 unit per tick (3.5 units/s) | **verified ±6%** (implemented: 1/6 for 4 ticks, then 1/4) |
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
doorway. The doorway is the block's +Z face, turned with the block's rotation
[L3DEdit].

**Partly verified** for rotation 1. On the Practice levels the exit
(14,1,13) has rotation 1. Its house door faces +X (camera 4, looking −X,
sees the door head-on). A climber walking −X along the path was saved
there, entering through that face.

Rotation 3 **(seen, not tested by a save):** on Practice "One Way"
(`LEVEL.093`) the exit house (21,1,13) has rotation 3, and camera 2 (looking
−Z) shows its door on the −X face. So rotations 1 and 3 put the doorway on
the +X and −X faces, as both the [L3DEdit] rule and the entrance rule above
predict. Rotations 0 and 2 are **unverified**: those two rules disagree for
them (+Z or −Z face), and the only reachable level with such exits
(Tricky 1, `LEVEL.020`) draws them as flat pads with no visible door. No
lemming reached one in our run.

## Skills

Skill ids follow the skill-panel order: blocker, turner, bomber, builder,
basher, miner, digger, climber, floater. Evidence: [L3DEdit] says the ids
follow the panel, and the Practice level titles (`LEVEL.080`–`088`) run in
the same order. **Verified** in game: the panel icons run in this order, and
the Practice menu's first nine items are these skills.

**Assigning (verified):** select the skill icon (this works while paused),
then click a lemming while the game runs. Clicks on a paused game didn't
assign.

Terrain is changed one quarter-height segment at a time. The basher and the
digger were seen to remove segments across a whole cell at once. Whether
steel stops them, and when exactly a basher stops, is untested; the
implementation never removes steel (provisional).

| Skill | Behaviour | Status |
|---|---|---|
| Blocker | Stands still. A walker turns around when its centre comes within about ½ unit of the blocker's centre, measured along the walker's path. The width of the area across the path is not measured. | **turn distance verified: 0.48 ± 0.05 unit** (implemented: ⅓ unit square) |
| Turner | Stands still and keeps standing after turning walkers. It points one arm sideways, and walkers that reach it leave in the direction the arm points. A turner assigned while walking +Z pointed +X, and walkers arriving along +Z were sent to +X: the turner's own left. Walkers arriving from other directions were not tested. | **verified for one heading** (implemented: a quarter turn the other way, +Z → −X) |
| Bomber | A countdown 5…1 over the lemming's head, each digit for about 8 ticks (0.57 s). After about 10 more ticks the lemming swells and explodes, about 3.6 s after assignment. The blast left a hole roughly one cell long in the 1-unit path blocks. | **timing verified ±1 tick per digit**; blast shape rough (implemented: 50-tick fuse, 1-unit radius) |
| Builder | Lays thin bricks, each about 0.55 unit long in the walking direction and about 0.28 unit higher than the last (¼ up per ½ forward within the measurement error), climbing onto each. One assignment laid **6 bricks**, one every **25 ± 1 ticks** (1.8 s). After the last brick the builder walked on off the end of the staircase and fell. A shrug was not seen. Brick width across the path is not measured. | **verified: count, timing, rise and run (±10%)** (implemented: 8 bricks, 11 ticks per brick) |
| Basher | Bashing a 1-unit crate took about 2.1–2.4 s (30–34 ticks): cracks spread over the crate, then the lower ½ unit (two segments) of the whole cell vanished at once, leaving the top half hanging. Walkers then walked through the gap. The basher went back to walking after the crate. | **rough** (implemented: every 7 ticks removes all segments from the feet up and advances ¼) |
| Miner | Removes a chunk about one cell long ahead of it at once after a crack animation, roughly every 3.5 s. The slope and depth per stroke could not be read from the views used. | **provisional; timing rough** (implemented: every 9 ticks, ¼ forward and ¼ down) |
| Digger | Removes the top ¼-unit segment of the **whole cell** under it at once, after a crack animation, about every 2.8 s (≈ 40 ticks, ±20%). It dug through a 1-unit block in about 8.4 s and then fell. | **rough** (implemented: every 6 ticks) |
| Climber | Climbs vertical walls at about 0.6 units/s, then walks over the top and drops off the far side like a walker (2-unit drops survived). Behaviour at a ceiling is untested. The skill is permanent: one climber climbed both walls on Practice "Climber". | **verified** (speed rough) |
| Floater | Falls normally at first; the umbrella opened about 7 ticks (0.5 s) and about 1½ units into the fall. It then falls at **1/32 unit per tick** (0.45 units/s, the walking speed) and survived a 6¼-unit fall that kills non-floaters. | **float speed verified ±5%; survival verified for 6¼ units; opening point rough** (implemented: opens after ½ unit, then 1/19 unit per tick) |
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

## Deaths

| Cause | Result |
|---|---|
| Landing in water outside land polygons, or on a block flagged liquid | Drowning (walking off the land: verified) |
| Landing after a fall longer than the splat height | Splat, unless the block is flagged "no splat". The lemming vanishes in a burst of blue fragments (verified) |
| Leaving the kill boundary (header `0x153`) or rising above the kill ceiling | Zapped |

Each death animation lasts about 1 s (provisional).
