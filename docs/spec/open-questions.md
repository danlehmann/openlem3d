# Open questions for the original game

Behaviour we implemented provisionally and want to check against the
original (see [original-game-navigation.md](../original-game-navigation.md)).
Each names where it shows up, so a run can go straight there.

## Mechanics

- **Trampolines:** the largest hop, what blue pads do (inferred: shorten the next hop), and how a drop onto a pad picks its hop (inferred: the shortest one rising higher than the drop).
- **Turner pose.** In the original's "Turner" demo the turner was seen side on, facing the
  walkers coming towards it, one arm straight out along its facing, while
  the walkers turned to its own left (one view; medium confidence). Ours
  draws the turner turned round to face the way it came (the walkers
  behind it), holding out the arm on the side it sends them; kept facing
  its walking direction, turners showed their backs on "Take a Dive".
- **Ramp slot on "All Around the Watchtower"** (`LEVEL.027`): two half-height
  ramps peak either side of a one-cell slot (16, 1, 6) 0.75 units deep; our
  lemmings fall in and can't leave. Probably the design: with our ramp
  orientation the half-height ramps continue the full ramps below them
  seamlessly, while the reverse leaves a 1.5-unit drop at z = 8.
- **Sea animation:** whether and how fast the sea animates in place on
  levels without flag `0x0010`.

## Presentation

- How the minimap picks block colours (a stone path is brown there).
- Pointer shapes over the 3D view and what holding the mouse there does
  (ours moves the camera as the arrow shows).
- Whether the monitor on "Lemmings Inside" (`LEVEL.007`, level flag
  `0x0400`) shows the live game screen or the still picture in its decals
  (we show the still).
