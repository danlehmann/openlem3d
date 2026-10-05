# Lemming behaviour

Implemented in `crates/l3d-sim`. Almost everything here is **provisional**:
the values are chosen to be plausible and have yet to be measured against
the running game. Each item says what has been checked.

## Coordinates and timing

- **Positions** are fixed point, with 256 sub-units per grid unit (`SUB`).
  The point tracked is the lemming's feet.
- **Tick rate:** the simulation runs at 30 ticks per second. The original's
  rate is still to be measured.

## Collision

- **Solid geometry** is the block shapes exactly as the renderer draws them:
  cubes clipped to their segment span, ramps, pyramids, corners and
  deflectors (`world::inside_shape`). Blocks flagged "not solid to lemmings"
  are ignored.
- **Ground plane** at y = 1, the top of layer 0. Inside a land polygon it is
  land; elsewhere it is water, unless the level flag "bottom is solid" is set.

## Movement (provisional)

| Quantity | Value |
|---|---|
| Walking speed | 1/48 unit per tick (0.625 units/s) |
| Falling speed | 1/12 unit per tick (2.5 units/s) |
| Highest step climbed without turning | ¼ unit (one segment) |
| Largest drop stepped down without falling | ¼ unit |
| Fall height that splats | more than 4 units |
| Headroom needed to walk | ½ unit |

- **Headings:** lemmings walk along one of the four axis directions. A wall,
  meaning a step higher than ¼ unit or no headroom, turns them around (180°).
- **Release:** lemmings appear at the centre of the entrance cell, just below
  the hatch, falling. Entrances alternate when there are several (up to four).
- **Release interval:** `(99 − rate)/2 + 4` frames at 17 Hz. This is the
  original Lemmings formula, assumed for L3D.

## Release direction (inferred)

An unrotated entrance releases towards +Z [L3DEdit]. Each rotation step turns
the release direction +Z → +X → −Z → −X.

How we inferred it: `LEVEL.000`'s hatch has rotation 1 and sits on the west
edge of its island. Only +X keeps the lemmings on land. Still to be confirmed
by watching the original.

## Exits

A walker enters an exit block (id 1) when it walks into the block through its
doorway. The doorway is the block's +Z face, turned with the block's rotation
[L3DEdit].

## Skills (provisional, not yet observed in the original)

Skill ids follow the skill-panel order: blocker, turner, bomber, builder,
basher, miner, digger, climber, floater. Evidence: [L3DEdit] says the ids
follow the panel, and the Practice level titles (`LEVEL.080`–`088`) run in
the same order.

Terrain is changed one quarter-height segment at a time. Steel blocks are
never removed.

| Skill | Behaviour |
|---|---|
| Blocker | Stands still. Walkers entering its area (⅓ unit) turn around. |
| Turner | Stands still. Walkers are sent a quarter turn clockwise from the turner's heading. |
| Bomber | 5-second fuse, then removes segments within 1 unit and dies. |
| Builder | 8 bricks, one every 24 ticks. Each brick is one segment in the cell ahead, at feet height, and the builder steps half a cell onto it. Bricks copy the block id below the builder, or the level's most common ordinary block. |
| Basher | Every 16 ticks, removes the segments from the feet up in the cell ahead and advances ¼ unit. Stops when nothing is left to bash. |
| Miner | Every 20 ticks, removes the segments from the feet down in the cell ahead, then advances and descends ¼ unit. |
| Digger | Every 12 ticks, removes the top segment under the feet. |
| Climber | Climbs vertical walls. Falls back off at a ceiling. |
| Floater | Opens an umbrella after falling ½ unit, then falls slowly and never splats. |
| Nuke | Stops releases and gives every lemming a fuse, staggered by one tick each. |

## Deaths

| Cause | Result |
|---|---|
| Landing in water outside land polygons, or on a block flagged liquid | Drowning |
| Landing after a fall longer than the splat height | Splat, unless the block is flagged "no splat" |
| Leaving the kill boundary (header `0x153`) or rising above the kill ceiling | Zapped |

Each death animation lasts about 1 s.
