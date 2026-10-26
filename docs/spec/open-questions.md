# Open questions for the original game

Behaviour we implemented provisionally and want to check against the
original (see [original-game-navigation.md](../original-game-navigation.md)).
Each names where it shows up, so a run can go straight there.

## Mechanics

- **Trampolines:** whether a red pad lengthens long hops by 8 ticks too (measured for hops up to 30), and how a hop landing on a lower pad counts the drop (inferred: as a straight drop does, 8 ticks a unit).
- **Ramp slot on "All Around the Watchtower"** (`LEVEL.027`): two half-height
  ramps peak either side of a one-cell slot (16, 1, 6) 0.75 units deep; our
  lemmings fall in and can't leave. Probably the design: with our ramp
  orientation the half-height ramps continue the full ramps below them
  seamlessly, while the reverse leaves a 1.5-unit drop at z = 8.

## Presentation

- How the minimap picks block colours (a stone path is brown there).
- Pointer shapes over the 3D view and what holding the mouse there does
  (ours moves the camera as the arrow shows).
- Whether the monitor on "Lemmings Inside" (`LEVEL.007`, level flag
  `0x0400`) shows the live game screen or the still picture in its decals
  (we show the still).
