# Open questions for the original game

Behaviour we implemented provisionally and want to check against the
original (see [original-game-navigation.md](../original-game-navigation.md)).
Each names where it shows up, so a run can go straight there.

## Mechanics

- **Trampoline bounces beyond the third hop** and the strongest bounce, and whether a long drop bounces differently (Fun 1).
- **Skills on sliding lemmings.** Whether a turner or blocker can be given
  to a lemming on ice (Practice "Slippery", `LEVEL.095`). We refuse them.
- **Turned walkers.** Whether walkers turn at a turner's arm's length or on
  its line (we use its line; Practice "Claustrophobic" needs that).
- **Hanging deflectors.** "Team Work" (`LEVEL.051`) has a deflector hanging
  ¼ unit above the floor. We let a deflector turn a walker wherever it
  crosses the body, feet to head; before that, walkers passed under it, hit
  their heads and froze. Check that the original turns them there.
- **Ramp slot on "All Around the Watchtower"** (`LEVEL.027`): two half-height
  ramps peak either side of a one-cell slot (16, 1, 6) 0.75 units deep; our
  lemmings fall in and can't leave. Probably the design: with our ramp
  orientation the half-height ramps continue the full ramps below them
  seamlessly, while the reverse leaves a 1.5-unit drop at z = 8.
- **Highest step a walker climbs** (we use ¼ unit; ½ is untested).
- **Sea drift and animation:** the Z speed (one rough observation was 2–4×
  ours; X agrees), and whether and how fast the sea animates in place on
  levels without flag `0x0010`.

## Presentation

- How the minimap picks block colours (a stone path is brown there).
- Pointer shapes over the 3D view and what holding the mouse there does
  (ours moves the camera as the arrow shows).
- Whether the monitor on "Lemmings Inside" (`LEVEL.007`, level flag
  `0x0400`) shows the live game screen or the still picture in its decals
  (we show the still).
