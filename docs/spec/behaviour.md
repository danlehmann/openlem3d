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
  the lemmings walk on.

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

**Direction:** each rotation step turns the release direction +Z → +X → −Z
→ −X. An unrotated entrance releases towards +Z [L3DEdit].

- Rotation 1 → +X: **verified** on Fun 1 (`LEVEL.000`, hatch (3,2,16)).
  Camera 1 faces +Z, so +X is screen left. The lemmings walk out to the
  left, along the hatch's row, onto the ramp at x = 6.
- Rotation 3 → −X: **verified** on Practice 1, 3, 5, 6, 8 and 9
  (`LEVEL.080`/`082`/`084`/`085`/`087`/`088`). Lemmings walk from the hatch
  towards the exit at x = 14.
- Rotations 0 and 2: **unverified**. One look at Practice "Slippery Block"
  (rotation 0, on ice) was inconclusive.

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
there, entering through that face. Other rotations are unverified.

## Skills

Skill ids follow the skill-panel order: blocker, turner, bomber, builder,
basher, miner, digger, climber, floater. Evidence: [L3DEdit] says the ids
follow the panel, and the Practice level titles (`LEVEL.080`–`088`) run in
the same order. **Verified** in game: the panel icons run in this order, and
the Practice menu's first nine items are these skills.

**Assigning (verified):** select the skill icon (this works while paused),
then click a lemming while the game runs. Clicks on a paused game didn't
assign.

Terrain is changed one quarter-height segment at a time. Steel blocks are
never removed (provisional).

| Skill | Behaviour | Status |
|---|---|---|
| Blocker | Stands still. Walkers entering its area (⅓ unit) turn around. | provisional |
| Turner | Stands still. Walkers are sent a quarter turn clockwise from the turner's heading. | provisional |
| Bomber | A countdown 5…1 over the lemming's head, each digit for about 8 ticks (0.57 s). After about 10 more ticks the lemming swells and explodes, about 3.6 s after assignment. The blast left a hole roughly one cell long in the 1-unit path blocks. | **timing verified ±1 tick per digit**; blast shape rough (implemented: 50-tick fuse, 1-unit radius) |
| Builder | Each brick is a thin slab about ½ unit long in the walking direction. Each one is ¼ unit higher than the previous, so the staircase rises ¼ per ½ run. Walkers walk up and down it. At least 5 bricks were seen; the total, the timing and the brick width are not measured. | **rise and run verified roughly (±10%)** (implemented: 8 bricks, each ¼ up and ½ forward; brick count provisional) |
| Basher | Every 16 ticks, removes the segments from the feet up in the cell ahead and advances ¼ unit. Stops when nothing is left to bash. | provisional |
| Miner | Every 20 ticks, removes the segments from the feet down in the cell ahead, then advances and descends ¼ unit. | provisional |
| Digger | Every 12 ticks, removes the top segment under the feet. | provisional |
| Climber | Climbs vertical walls at about 0.6 units/s, then walks over the top and drops off the far side like a walker (2-unit drops survived). Behaviour at a ceiling is untested. The skill is permanent: one climber climbed both walls on Practice "Climber". | **verified** (speed rough) |
| Floater | Opens an umbrella after falling ½ unit, then falls slowly and never splats. | provisional |
| Nuke | Stops releases and gives every lemming a fuse, staggered by one tick each. | provisional |

## Deaths

| Cause | Result |
|---|---|
| Landing in water outside land polygons, or on a block flagged liquid | Drowning (walking off the land: verified) |
| Landing after a fall longer than the splat height | Splat, unless the block is flagged "no splat". The lemming vanishes in a burst of blue fragments (verified) |
| Leaving the kill boundary (header `0x153`) or rising above the kill ceiling | Zapped |

Each death animation lasts about 1 s (provisional).
