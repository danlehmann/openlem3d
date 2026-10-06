# Open questions for the original game

Behaviour we implemented provisionally and want to check against the
original (see [original-game-navigation.md](../original-game-navigation.md)).
Each names where it shows up, so a run can go straight there.

## Mechanics

- **Trampoline bounces.** Height and length of a bounce, and whether they
  depend on the drop. Practice "Trampoline" (`LEVEL.098`) has a cube 5 units
  up and deflectors 6–8 units up at its corners, so bounces probably go much
  higher than ours; that level is not solvable in our model. "Over the Top"
  (`LEVEL.068`) traps our bouncing lemmings in a pit after the second pad.
- **Skills on sliding lemmings.** Whether a turner or blocker can be given
  to a lemming on ice (Practice "Slippery", `LEVEL.095`). We refuse them.
- **Miner slope.** The tunnel's slope and where Practice "Miner"
  (`LEVEL.085`) expects the miner to start: its "MINE HERE" sign stands
  near the hatch, but our 45° tunnel from there ends inside the tower.
- **Splitter.** Which way the first lemming goes (Practice "Splitter",
  `LEVEL.094`).
- **Teleporter.** Any delay, and where the lemming reappears (Practice
  "Teleporter", `LEVEL.099`).
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
