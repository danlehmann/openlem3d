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

**The hatch opens first (observed by the owner).** A level starts with its
hatches closed; they swing open, and only then does the first lemming drop.
The opening time is unmeasured: ours opens the doors over the first 10
ticks and drops the first lemming on the 11th (stored solutions count their
ticks from then).

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
| Fall height that splats | 4 units or more | **observed** (implemented: 4 or more, as in [LOAP]). **Verified with a changed level** (Practice "Miner" with a block added where the lemmings land off the tower, in a copy of the CD; see the navigation notes): landing 3¾ units down they lived and were saved, 4 units down they all died. A gap cut in Fun 2's ramp (cells (11,10–11,17) removed, a 6-unit drop) killed every lemming. **Verified with a level built for it** (Practice "Miner" changed: the walkway lowered to 4, a 45° ramp up to 5, then a sheer 4-unit drop to the path): every lemming walking off the ramp's top edge burst on landing, none were saved, so ramps are not measured lower. The owner saw lemmings survive dropping through a *digger's* hole in Fun 2; ours explains it (inferred): those inside the shaft while the digger is still working fall only from its floor when it breaks through (3 units there); those arriving after it opens fall 4 and die. Fun 4 "It's a Run Around": lemmings falling 4 units (9 → 5) through a hole dug in the castle top on the hatch side die, those falling 3 on the east side live (owner's observation). In Practice "Miner" lemmings falling out of a miner's tunnel after four or two blocks survived (stunned), after one block died (owner's observation): with ours a quarter unit lower per block, falls of 3¼ and 3¾ survived and 4 killed |
| Fall height that stuns | Shorter falls that still go far leave the lemming lying on its back for a short time; then it gets up and walks on. Practice "Miner", lemmings falling out of a miner's tunnel where it left the tower (owner's observation; heights with ours a quarter unit lower per block): after two, four and five blocks (falls of 3¾, 3¼ and 3 units) they were stunned, after six (2¾) they walked on. Hatch drops of 2–2½ units are taken not to stun (unchecked); those of 3.3 units (Tricky 19, Mayhem 10) would | **observed**; implemented: falls of 3 units or more (as in [LOAP]), lying 28 ticks (2 s; the time is ours), drawn lying still on its back (the first frame of cells 429–443; owner: no animation). A no-splat block doesn't stun either (ours) |
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
doorway, also from a path up to a step (¼ unit) below the exit's floor
(provisional: Taxing 4 "Poles Apart" has such an approach). A lemming bouncing on trampolines enters too when a hop carries it into the block through its doorway (inferred: Tricky 2 "Which Exit?" [LemWiki] [LU3DWalk]). The doorway is the block's +Z face turned with the block's rotation
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

**Switching skills (inferred, unverified in game).** A lemming at work as a
digger, builder, basher or miner takes another of those four in place of the
one it has, as in the 2D games; it cannot be given the one it already has.
Evidence: the published solution of Mayhem 1 "The Five Arches" [XeyeWalk]
makes a digger a builder once it has dug into the block under it, and our
simulation of that solution saves exactly the 78 needed. Implemented as
such; a basher or miner switched to starts where it stands (ours). Blockers
and turners are still given only to walkers. A builder that has laid its last
brick, or is shrugging, can be given more bricks (provisional, as in the 2D
games; otherwise only a few ticks between its last brick and stepping off
the staircase allow it, while Fun 10 "Castle Lemmalot" needs it).

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
| Blocker | Stands still. A walker turns around when its centre comes within about ½ unit of the blocker's centre, measured along the walker's path. The width of the area across the path is not measured. A walker already that close when the blocker starts turns around too if it is heading towards it (observed: a blocker set in a tight pack at release rate 99 on Practice "Blocker" turned back the lemmings right behind it; none walked through). | **turn distance verified: 0.48 ± 0.05 unit** (implemented: a 0.48-unit square) |
| Turner | Stands still and keeps standing after turning walkers. It points one arm sideways, and walkers that reach it leave in the direction the arm points. A turner assigned while walking +Z pointed +X, and walkers arriving along +Z were sent to +X: the turner's own left. Walkers arriving from other directions were not tested. | **verified for one heading** (implemented: the second click picks the side, as seen on screen, and walkers leave a quarter turn that way; Practice "Turner" needs the turner's right, −X, so both sides must be possible. A walker turns as it crosses the centre of the turner's cell, ahead of the turner standing at the cell's edge (owner's observation in Practice "Turner"); the turner turns walkers from the moment it is given, while it still walks on to the edge. A walker already past the centre at that moment walks on and is lost, and no walker ever turns at the cell's end, so all leave along its middle (owner, measured in the original's Practice "Turner" at release rate 99: the first lemming made a turner, its followers all left in single file along the middle of the path; made a turner after the next one had crossed the middle, that one walked on). A lemming that lands from a fall past the centre of a turner's cell is turned where it lands (measured on a test level made with cdcopy, 2026-10-14: hatch lemmings land about ¼ unit past the centre of the stand cell below; with the first one made a turner, which walked on to the cell's edge, each later one turned on its landing spot and none walked past). **Unverified:** what a lemming landing before the centre does (ours: it walks on and turns at the centre). **Unverified:** a walker that another turner or a deflector has just turned exactly at the centre is not turned again by a turner in the same cell, since it never walks across the centre (ours; this also keeps two turners in one cell from passing a walker back and forth). Mayhem 11 "Across the Network" had a stored route that needed such a second turn, so it is unsolved again until this is checked. Only walkers going the way the turner was walking when it was given are turned; walkers meeting it from any other side pass unaffected (owner)) |
| Bomber | A countdown 5…1 over the lemming's head, each digit for about 8 ticks (0.57 s). After about 10 more ticks the lemming swells and explodes, about 3.6 s after assignment. The blast left a hole roughly one cell long in the 1-unit path blocks. | **timing verified ±1 tick per digit**; blast shape rough (implemented: 50-tick fuse; for the last 10 ticks a bomber on its feet stands still, holding its nose and swelling, owner's observation; the blast removes, in every column whose centre lies within 1 unit of the bomber across the ground, the same band from ½ unit below its feet to 1 unit above, so walls go whole and the crater floor is level: ours, after a sphere left half-walls that trapped walkers in Practice "Bomber"). A shaped block whose top lies in the band is shaved flat there and keeps its slopes below the cut, as for the digger. **Verified with a changed level** (Practice "Trampoline" with a stone hill of 45° ramps and cubes on a lane without pads, bombers clicked onto lemmings on the hill, seen side-on): the blasts left flat steps with vertical faces and never a shallower slope; ours had squeezed what was left of a ramp into a slope of the remaining height (owner's report from Fun 6 "It's a Blast")). Blasts never break one-way blocks, not even from the side they can be bashed through: **verified with a changed level** (Practice "One Way" with walkers boxed into one cell between a one-way block and an ordinary cube). There the cube lost only its lower half, about from the bomber's feet to ½ unit above, and the stone path under the bomber was not holed; ours removes the whole cube (open question) |
| Builder | Lays thin bricks, each about 0.55 unit long in the walking direction and about 0.28 unit higher than the last (¼ up per ½ forward within the measurement error), climbing onto each. One assignment laid **6 bricks**, one every **25 ± 1 ticks** (1.8 s). After the last brick the builder walked on off the end of the staircase and fell. A shrug was not seen. Brick width across the path is not measured. | **verified: count, timing, rise and run (±10%)** (implemented: 6 bricks, 25 ticks each, ¼ up and 0.55 forward (141/256: the measured 37 px a brick at 67 px a unit; with ½, the published bridges of Mayhem 1 and Taxing 5 came up about 0.3 unit short); a builder standing less than 1/16 above a segment line (the thin edge of a ramp) lays its bricks from the line, so that walkers can step onto the first (Fun 10's published staircase starts at a ramp's foot). Each brick starts at the builder's feet, so a builder at a block's edge leaves no gap that walkers drop through (the published routes of Taxing 19 "Lemmtris" and Tricky 14 "Chocolate Drop" have followers cross that way; ours started ⅛ unit ahead and they all fell). Bricks are free-standing slabs, not grid segments: 9/16 unit long, ¼ thick, 1 unit wide (width unmeasured), drawn as thin horizontal planes at their tops with nothing vertical, as the original draws them (owner's observation), showing tile 40 of the level's texture set (wooden planks in a light frame) with the planks lying across the way the builder went (owner's comparison; seen best from the lemming view). Bricks are floors only: a walker meeting one side-on steps onto it if its top is within a step's height, otherwise passes under it, and never turns round at a staircase (owner's recollection: from the wrong side, walkers step onto the lowest step and off again). A brick that would run into terrain is cut short at it and laid, even where the builder's head would not fit on it, and the builder stops; Practice "Builder" relies on the last brick meeting the platform edge. **Seen with a changed level** (Practice "Builder" with a wall across its bridge, the original's demo): the brick running into the wall was laid and the builder then walked back; whether it is exactly flush was not seen side-on) |
| Basher | About 7 ticks after the basher reaches a block, black cracks appear on the sides of the lower ½ unit (the two segments from its feet up): frames 0–15 of `OVERLAY.000`, mapped 1:1 onto each 64×64 face, one frame every 2 ticks, drawn only on the segments being removed, so they start by the basher's corner and spread. Exactly 32 ticks after the first crack the two segments vanish at once, leaving the top half hanging, and the next stroke's cracks begin. Bursts of 25–30 pieces of the block's texture fly out of the hole at about 1, 9, 18 and 25 ticks into a stroke: a kick upwards, out away from the block, then falling faster, gone after 6–7 ticks. Walkers then walked through the gap; the basher went back to walking after the crate. The digger shows the same cracks and pieces on the top face. | **measured** in the Practice "Basher" demo (crack timing to ±0.5 tick; the pieces' motion rough; where their shapes come from unknown: ours are small squares cut from the block's +X face tile). Implemented as measured for the basher; not yet for the digger. When the basher goes back to walking after its last stroke was not timed; ours walks on at once (provisional). With more breakable wall in the next cell ours walks on through its hole to that wall and bashes on, so walls thicker than a cell can be bashed through (provisional: Tricky 3 "This Is the Army" needs a tunnel through a tower); a stroke that leaves the top and bottom of a cell leaves a gap walkers pass through. **Verified with a changed level** (Practice "Basher" with a 45° ramp up to the crate's top in the cell before it, played by the original's own demo): the basher given on the ramp walked up it and over the crate, cutting neither; ours does the same. With a **22.5° ramp** instead (same test), the basher cut level into the ramp at its feet: a notch with a vertical face, the basher standing in it, then on into the crate at that height; ours shaves the ramp cell flat at the basher's feet, keeping its slope below |
| Miner | Mines a blocky tunnel, a block at a time, each block a quarter unit lower than the last: standing at the end of its block it swings while cracks spread over the block ahead (the "tile breaking" animation) and pieces of it fly out; the block then breaks, and the miner walks to its far end and mines again. Material above the tunnel at the end can stay floating in the air. Started under the "MINE HERE" sign of Practice "Miner" it mines four blocks and comes out of the tower's west side, the lemmings falling to the ground stunned (owner's observation, with screenshots; the tunnel is blocky, not a slope, with long flat treads and low risers: a side-by-side comparison showed half a unit per block far too deep) | **observed** (owner). Implemented: a stroke of 17 ticks of cracks (one crack frame a tick; ours) then the walk across the block (32 ticks), 49 ticks in all, matching an earlier rough measurement of one block about every 3.5 s. The block ahead is cleared from a quarter unit below the feet (feet rounded to a quarter unit) up to half a unit above the feet (a walker's headroom: from the plank on the tower in Practice "Miner" the fourth block leaves ground above the tunnel, owner's observation); the miner steps the quarter unit down into it and walks to its far edge. It stops at steel, at a one-way block facing the other way, when nothing is left to remove, and at the level bottom; out in the open it falls. The miner stands a quarter unit before the end of its block (about half its width, so that it is in front of the block it mines; owner's observation), and after each break walks exactly a unit on, to the same place in the broken block. A miner set to work further back walks there first. From the sign in Practice "Miner" ours mines four blocks (5¼ → 4¼) and falls 3¼ units to the ground; after two blocks 3¾, after one 4. In the original the first two of these stun and the last kills (owner's observation), which puts the splat height at about 4 units (Fun 2's published route needs a 4-unit drop survived, so ours splats above 4). An earlier reading of the Practice demo, "about 1:1 on average", is superseded |
| Digger | Removes the top ¼-unit segment of the **whole cell** under it at once, after a crack animation, every 3.0 s (± 0.1 s; 42 ticks), the first one period after the assignment: through Practice "Digger"'s bridge seen side-on, quarters went at 2.9, 5.6, 8.6 and 11.9 s, then it fell; the level clock ran at real time. Lemmings walking onto the hole dropped into the pit and went down with the digger. | **measured** (implemented: one segment every 42 ticks; the cracks spread over the segment's sides and top over those 40 ticks, and four bursts of pieces of the block's top texture fly up and out around the digger's feet (owner: it throws debris; ours, after the basher's). A shaped block is shaved off flat and keeps its slopes below the cut: digging never adds material. **Observed** in Fun 2 "That's Right" (its ramp is a beam of a ramp over an upside-down wedge in each column; owner's comparison with ours): a dug ramp keeps its slope below a flat cut, and in the wedge under it the cut lowers its flat top while the sloped underside stays; where the underside leaves nothing under the digger, about halfway through, it falls, and the rest stays as a ledge that shortens the followers' fall. Ours cuts a ramp level with the digger's feet first (to the quarter), then a quarter per dig) |
| Climber | Climbs vertical walls at about 0.6 units/s, then walks over the top and drops off the far side like a walker (2-unit drops survived). Behaviour at a ceiling is untested. The skill is permanent: one climber climbed both walls on Practice "Climber". A climbing lemming takes no digger: nothing happens and the skill is not used up (owner, "Candyland Climber", whose way through is a climber blown up on the cane, opening it for the next). Bomber and floater can be given while climbing. | **verified** (speed rough; bomber and floater while climbing implemented, not checked). Implemented as well (provisional): no other skill while climbing; a climber reaches the top only once its whole body has room there; climbers climb only walls reaching down to their feet and turn like walkers at an overhang (terrain only: bricks are no ceiling, **verified with a changed level**, Practice "Climber" with a raised start platform: a builder at the platform's edge laid its first brick out over the lane, and a climber climbed the platform's face straight past it onto the top; ours had let go under the brick, which blocked Taxing 12 "Team Work"); a climber with a skill pending (basher, miner, blocker, turner) takes the skill up at a wall instead of climbing it (inferred: Tricky 3 "This Is the Army" [LU3DWalk] has a climber mine through a wall it walks up to) |
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
| Deflector (shape 7) | A walker meeting the diagonal face turns a quarter, as if reflected, once its centre reaches the face, wherever the face crosses its body (a deflector hanging ¼ unit above the floor in "Team Work" still turns walkers); the square backs are ordinary walls. A climber climbs the diagonal face instead of turning, if the face reaches down to its feet; one hanging ¼ unit above the floor turns it like a walker, as at an overhang (inferred: with that, the stored solutions of five levels whose climbers meet such deflectors keep working) (**verified in the original**: on Taxing 20 "Death Slide", copied into a Practice slot, a climber given as it landed from the hatch climbed the 3-high deflector column the walkers are turned at, and walked off along its top). Practice "Deflector" is a square track with deflectors at the corners. A basher meeting the diagonal face walks on into the cell until the face is within a walker's reach, then bashes it there; the cracks spread over the diagonal face and the square backs, not the cube's open sides. | provisional (implemented; the basher's stance and the cracks are ours, Practice "Deflector" marks its one breakable deflector "BASH HERE!!") |
| Splitter (block 2, non-solid) | Walkers crossing the cell's centre are sent alternately left and right [L3DEdit]. In the Practice "Splitter" demo the first lemming seen went to its own right and the next to its left; both exits received lemmings. | first side **observed**: in a second run the very first lemming to reach the splitter went to its own right (+X, towards the exit house at x ≈ 23), the camera still and no lemming having reached the cross path before; implemented: right first, then alternating |
| Liquid top (block flag `0x04`) | Walkers that step onto it drown, as well as lemmings that land on it (Practice "Mud"). | provisional (implemented) |
| Slippery top (block flag `0x40`) | Lemmings on it are in a "Sliding" state (shown when hovering) at about **3.1× walking speed** (2.9–3.4, from bursts scaled by sprite height), drawn with both arms spread; deflectors turn them and they keep sliding; off the ice they walk again. The ground plane is slippery too where header byte `0x157` says so (bit 0: the level bottom, bit 1: the land) ([L3DEdit]; inferred: only the three ice levels set it, and Fun 14 "Slippery Maze" works as published only with it). Whether skills can be given to sliding lemmings was not observed (the serial mouse could not assign skills at all). | speed and state **observed** (Practice "Slippery"); implemented: 3× walking speed, and no standing skills (blocker, turner, builder, basher, miner, digger) can be given while sliding, which makes Practice "Slippery" a lesson in placing the turner off the ice (provisional) |
| Killing traps (types 0, 1, 2, 6, 7) | Take a lemming that reaches the middle of their cell, then stay busy for 2 s, letting others pass. | provisional (implemented). Taking lemmings anywhere in the cell would let none pass on foot (crossing a cell takes 32 ticks, longer than the 28 busy ticks), yet Taxing 5 "3D - A Lemming Odyssey" lines the way to its exit with traps |
| Teleporter (type 5) | A walker reaching the centre of a pad stops there, turns to face the camera and stands about 0.3 s; it is squeezed to a thin vertical sliver and then a dot (about 0.2 s); a light-blue sparkle (zig-zag lines and stars) shows above the pad for about 0.5 s, then one at the partner (same value; with more, the last two pair [L3DEdit]) for about 0.5 s; the lemming stretches out again there (about 0.3 s), stands about 0.3 s and walks on along the path leading away. From vanishing to reappearing on the minimap took 0.80–0.88 s; about 2.1 s in all from reaching the pad centre. The pads work both ways. | **observed** (Practice "Teleporter" demo, bursts about 47 ms apart); implemented: stand 4 ticks, squeeze 3, away 12, stretch 4, stand 4 (about 1.9 s), facing the camera; the pad flat on its block (`TRAPS` frame 0), the sparkle from `TRAPS` frames 4–7, one per tick, over the pad left for the first 6 ticks away, then over the other |
| Rope slide (type 8) | A lemming reaching a sender (even value) jumps up to a handle and hangs for about 0.6 s, then slides down hanging under the rope, arms raised, to its receiver. Only one lemming rides a rope at a time: at release rate 11 in Practice "Rope Slide" every other lemming is lost, at 7 none (owner's observation; that the rope is busy is the owner's reading). The rider hangs by one raised hand from the rope (owner's picture), the floater's pose (value + 1; the last of each value works [L3DEdit]). In Practice "Rope Slide" it took **6.35 ± 0.1 s from grab to release**; it lets go at the low anchor, shows a landing pose for about 0.3 s, then walks on in the direction of the slide. Whether it accelerates is not known. | timing **observed**; implemented: 8-tick hang, then a straight line at 35/256 unit per tick, matching the total time; a lemming reaching the sender while another rides walks on. The feet follow the line from floor to floor; the sprite is drawn with its hand on the rope, the top of the solid blocks within two units above (in Practice "Rope Slide" a run of 22.5° ramps, ¾–1 unit above the line). A rope's far end needs no block in its cell, unlike other objects (ours: Taxing 20 "Death Slide" anchors its rope in an empty cell, and both published routes ride it) |
| Spring (type 4) | A walker stepping onto the red sender pad is thrown upwards in a high, flailing arc onto its receiver (the blue pad), about **1.6 ± 0.2 s** for about 16 units in Practice "Catapults", appears standing on the pad and walks on in its flight direction. After a throw the sender is not ready for a while: walkers released 46 ticks apart (rate 55) walk straight over it every other time, 49 apart (rate 52) are all thrown (owner's observation, Practice "Catapults"). | **observed**; implemented: 1.4 ticks per unit, arc peak 0.3 of the distance (unmeasured); the sender rests 2.65 ticks per unit thrown after a throw (Practice "Catapults", 18 units: 46–49 ticks, owner; Mayhem 15 "Hole in One, Two, Three!", 7 units: 17–25 ticks, observed: walkers 31 ticks apart were all thrown, 17 apart every other one; proportional to the distance is ours), and a walker reaching it meanwhile walks across it; one that turns round on it is thrown once it is ready (ours, unverified) |
| Trampoline (type 3) | **Measured tick by tick** (Practice "Trampoline", side view with the preset camera back-projected; 14 lemmings, pixel-identical paths). In the air a lemming moves forward exactly 32 sub-units (1/8 unit) per tick; on the tick it touches a pad it does not move on. Hop *n* (from 1) lasts 8*n* + 6 ticks from pad to pad (14, 22, 30), so it is *n* + 0.625 units long (1.625, 2.625, 3.625: each one a unit longer), and *k* ticks in it is 120·*k*·(*N* − *k*)/(*N* + 1) sub-units above the take-off height (apexes 1.54, 2.50, 3.33 units). There is no snapping: each hop starts where the last landed. A walker stepping down onto the first pad stands on it two ticks (drifting on at walking speed), then takes hop 1. A lemming dropping farther onto a pad stands on it only the touching tick, as after a hop (observed in the lemming view on Tricky 2 "Which Exit?": after the 1¾-unit drop onto the first pad the view rests at the bottom for one tick, then rises; ours takes a drop of more than ½ unit as farther). Hop 6's descent onto the floating cube also fits. Pads are red (objects 0x60–0x63) or blue (0x64–0x67). | **measured** (red pads, hops 1–3 directly, 4–6 by extrapolation and hop 6's descent); implemented exactly. **Inferred:** landing on a red pad gives the next hop one longer; landing on a blue pad one shorter, and off a blue pad after hop 1 the lemming walks on (with this, as in the demo, lemmings turned on the cube bounce down the column's blue pads and the blue back row and walk into the exit; one turner saves 19). **Measured** on Fun 5 "Bounce Bounce" (screenshot bursts at about 60 ms, pad touch to landing, read against the pad's width; three red pads, each one water cell short of the path): a lemming reaching a red pad makes a hop of 10 ticks plus 8 per unit it dropped onto it, to the nearest tick. A walker arriving level with the pad walks to its middle, then hops about 1 unit in about 10 ticks and comes down a little past the middle of the water cell. After a ½-unit drop it bounces about a quarter of the way into the pad, flies about 14 ticks and 1½ units, and drowns. After a 1-unit drop it bounces before the pad's middle, flies about 18 ticks, and lands before the middle of the cell beyond the water. This also gives 14 ticks (hop 1) after the ½-unit step down onto the first pad of Practice "Trampoline". It fits Fun 1 "Take a Dive" [LU3DWalk], where a 10-unit drop from the third platform must carry lemmings into the exit house (90 ticks, about 11 units). It also fits Tricky 2 "Which Exit?" [LemWiki] [LU3DWalk], where a 1¼–1¾-unit drop onto the first of a row of pads must end in the exit door. A blue pad gives 7 ticks per unit dropped, to the nearest tick, less 4, and a lemming it would give less than 10 walks on across it. **Checked on a test level** (Practice "Trampoline" changed with `cdcopy`: four hatches on stands, each dropping lemmings onto one pad with open water beyond and marker stones beside it, release rate 99, a preset camera side-on to each lane; splash points read against the stones): red pads after drops of 1 and 3¼ units, splashing down at x ≈ 8.7 and 22.1 (ours 8.6 and 22.0); blue pads after drops of 3 and 4¼ units, at x ≈ 24.4 and 14.1 (ours 24.3 and 14.2). The blue rule is **fitted** to that and to the same level's blue pads after drops of 3, 2 and 1 units: a hop of about 16 ticks and 1.6–1.75 units, landing before the middle of the cell beyond the water (seen at release rate 99, where the lemmings trace the path as one arc, against the pad's width); a hop of about 10 ticks and 1 unit into the water; and no bounce at all, the lemming walking off the far side into the water. **Inferred:** a hop landing on a pad lower than its take-off counts that drop the same way, 8 ticks more per unit. The touching tick is decided under the lemming as well as just ahead: a hop that comes down at the far edge of a pad bounces off that pad (follows from "does not move on" on that tick; needed for Tricky 2 "Which Exit?", whose second hop comes down 0.06 unit before the pad edge). A hop carrying a lemming into an exit block through its doorway saves it (inferred: Tricky 2 "Which Exit?" is solved by bouncing into a door 2¾ units above the pads). Pads show their dip (frames 1–3 of their colour, one per tick) only when touched (observed: untouched pads never change). |


The interactive-object type of a level is its trap file number
([L3DEdit]); the Practice levels "Rope Slide", "Catapults", "Trampoline"
and "Teleporter" use 8, 4, 3 and 5, which agrees.

## Deaths

| Cause | Result |
|---|---|
| Landing in water outside land polygons, or on a block flagged liquid | Drowning (walking off the land: verified) |
| Landing after a fall longer than the splat height | Splat, unless the block is flagged "no splat". The lemming vanishes in a burst of blue fragments (verified; the owner too: it bursts into many pieces, no body left lying). Drawn as 40 bits of the smock flying out over the 14-tick death (count, speeds ours) |
| Leaving the kill boundary (header `0x153`) or rising above the kill ceiling | Zapped |

Each death animation lasts about 1 s (provisional).

## When a skill starts (measured)

From the original's Practice demos (raw grabs about 37 frames/s; the
assignment tick is the one where the panel's count drops; one demo per
skill, positions ±0.05–0.1 unit):

| Skill | After assignment | Evidence |
|---|---|---|
| Digger | Starts in the middle of its cell (owner's observation); digs out exactly the cell it stands in | "Digger": given at 0.42 across its cell, crouched the next tick (a "walk to the middle" rule, 2–3 ticks away, can't be fully excluded). Implemented as for the builder: it walks on to the middle of its cell, or digs where it stands if switched from another terrain skill |
| Builder | Takes the sack at once and walks on with it to the middle of its cell or to its edge, whichever comes first, then stops; the first brick appears 8 ticks later at its feet, then one every 25 ticks | "Builder": bag and pose in the same frame as the count drop; stood still within 0.01 unit for the 8 ticks; first brick starting 0–0.08 unit from its centre at x ≈ 19.6 (±0.2), mid-cell. The owner saw a builder walk on with the sack before building. Middle or edge, whichever first, is [LOAP]'s rule; middle only (the owner's first guess) left the published routes of Fun 3, Fun 16, Fun 20 and Tricky 3 short by 0.03–0.3 unit, and with this rule they work (inferred) |
| Blocker | Walks on to the edge of its cell along its heading, then blocks there | "Blocker": 21–22 ticks from 0.23 across to the edge |
| Turner | The same, then points | "Turner": 16 ticks from 0.41 across to the edge |
| Miner | The same, then mines | "Miner": 14 ticks from mid-cell to the edge. The owner sees the miner stop a little before the block it mines, in front of it; ours stops a quarter unit before the edge, which supersedes this |
| Basher | The same (here the edge was a crate face) | "Basher": 11 ticks to the crate face |
| Bomber | Walks on until its countdown ends (time, not position) | "Bomber": stopped about 40 ticks on, not at an edge |

Implemented: builders keep the skill pending (carrying the sack) and take
it up on the step that reaches or passes the middle of the cell, standing
there, or on the step that would carry them into the next cell, standing
on the edge, whichever comes first; at a wall they turn and walk on (ours).
Diggers do the same but for the edge: they dig at the middle of the cell
(owner's observation). A builder
already at work as another terrain skill, or out of bricks, builds where it
stands. Blockers, turners, miners and bashers keep the skill pending
and take it up on the step that would carry them into the next cell,
standing on the edge (before any fall beyond it), or at a wall that would
turn them round, facing it; a miner there steps up to the edge of its cell
when the wall starts at the edge (the measured "walks on to the edge of
its cell"; bashers stay where the wall turned them, their stroke reaching
the wall either way). A deflector turning a lemming in the air puts
it at the centre of the deflector's cell (inferred: with turners now
standing at a cell edge, the Trampoline demo's lemmings land on the
one-cell back row only with this).
