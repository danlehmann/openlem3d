# openlem3d — Ground Rules

openlem3d is an open-source reimplementation of the DOS game *Lemmings 3D*
(1995). This file holds the project's standing decisions. It is the source of
truth: read it before starting work, and update it when a rule changes.

## Clean-room policy

1. **Never disassemble or decompile the original executables.** We don't step
   through their code in a debugger either. Behaviour is learned only from:
   - observing the running game (in DOSBox-X): playing it, taking screenshots
     and making side-by-side comparisons;
   - analysing the **data files** on the CD (levels, graphics, palettes, audio);
   - public documentation and reverse-engineering notes from others.
2. **Verify, then write our own spec.** Treat all external information as
   unverified until it has been checked against the actual data or the running
   game. `docs/spec/` records formats and behaviour in our own words, along with
   how each fact was verified. The implementation follows that spec.
3. **Credit others' reverse-engineering work.** Every source we draw on (forum
   threads, notes, tools, people) is listed in `docs/spec/REFERENCES.md`. Spec
   sections cite the source of each fact they rely on, alongside how we
   verified it.
4. **Never commit original game assets.** That includes no extracted files and
   no screenshots of the original game. At run time the game reads everything,
   including the CD-audio music, from the user's own copy of the CD image.

## Game data location

Users supply their own *Lemmings 3D* CD image as a `.cue` file plus its
`.bin`. The game looks for it in this order:

1. the `--data <dir>` command-line argument;
2. the `OPENLEM3D_DATA` environment variable;
3. `gamedata/` in the current working directory. This is the standard location
   for development, and it is git-ignored.

The directory must contain one `.cue` file. The file names can be anything.

## Technical direction

- **Language and engine:** Rust (latest stable) and Bevy.
- **Portable:** the game must run on Windows, Linux and macOS (x86_64 and
  arm64). Web/WASM would be nice to have. Platform-specific code is only
  allowed in small, isolated modules.
- **Input:** mouse and keyboard, and touch. Both are first-class; the game must
  be fully playable without a keyboard.
- **Rendering:** a modern, high-resolution 3D renderer. Stay true to the
  original's look, but don't limit ourselves to its resolution.
- **Camera quirk to preserve:** the original camera can't pitch up or down. The
  sky is essentially a 2D panorama image that pans as the camera turns.
  Reproduce this behaviour (at least as the default mode).
- **Structure:** a Cargo workspace with three layers:
  - format parsers, with no Bevy dependency and unit-tested;
  - deterministic simulation, with no Bevy dependency and a fixed timestep;
  - the Bevy application on top of both.

## Gameplay decisions

- **Never force the player to watch anything.** Intros, title animations,
  briefings, result screens and transitions are all skippable at once by any
  key, click or tap. Menus accept input from their very first frame, and
  animations play alongside input instead of gating it. Slow original
  sequences (such as the title animation) are kept for the look, but never
  as waiting time.

- **All levels are unlocked from the start.** A level-select screen gives
  direct access to every level.
- **Level codes still work.** Entering an original level code jumps to the
  matching level, but codes aren't needed for progression.
