# Open questions for the original game

Behaviour we implemented provisionally and want to check against the
original (see [original-game-navigation.md](../original-game-navigation.md)).
Each names where it shows up, so a run can go straight there.

## Mechanics

- **Trampoline bounces beyond the first two hops**: how fast later hops grow and move (one "left much faster"), and whether a long drop bounces differently (Fun 1).
- **Skills on sliding lemmings.** Whether a turner or blocker can be given
  to a lemming on ice (Practice "Slippery", `LEVEL.095`). We refuse them.
- **Splitter.** Whether the very first lemming goes right (the first one seen did).
- **Teleporter.** The delay (Practice "Teleporter", `LEVEL.099`).
- **Turned walkers.** Whether walkers turn at a turner's arm's length or on
  its line (we use its line; Practice "Claustrophobic" needs that).
- **Half-height deflectors.** "Team Work" (`LEVEL.051`): our lemmings land
  under a deflector hanging ¼ unit above the floor, hit their heads in one
  direction and a wall in the other, and freeze.
- **Ramp slot on "All Around the Watchtower"** (`LEVEL.027`): two half-height
  ramps peak either side of a one-cell slot (16, 1, 6) 0.75 units deep; our
  lemmings fall in and can't leave. Either that is the design or the
  orientation of half-height ramps is reversed.
- **Highest step a walker climbs** (we use ¼ unit; ½ is untested).
- **Sea drift and animation:** how fast the sea slides for the header's
  water speeds, and its frame rate.

## Presentation

- The original's credit pages on the title banner (we show our own).
- How the minimap picks block colours (a stone path is brown there).
- Pointer shapes over the 3D view and what holding the mouse there does.
