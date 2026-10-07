# openlem3d

An open-source, clean-room reimplementation of the 1995 DOS game
*Lemmings 3D*, written in Rust with Bevy. It renders at high resolution
and plays with a mouse, a keyboard or a touch screen.

openlem3d contains **no game data**. You need your own copy of the
original CD; the game reads levels, graphics and CD-audio music from it at
run time.

## Status

Playable, and still being checked against the original:

- **Working:** all 100 levels and the 20 Practice levels, with every skill
  and every trap, trampolines, teleporters, springs, rope slides and
  slippery blocks. There are the original's title, code, options, level
  list, briefing and results screens, level codes, Practice demos, replays
  of your own attempts, the lemming view, the minimap, sound effects and CD
  music.
- **Still provisional:** some behaviour that couldn't yet be measured in
  the original; `docs/spec/open-questions.md` lists it.

See `docs/spec/` for what has been verified and how.

## Game data

openlem3d needs an image of the original *Lemmings 3D* CD (the DOS
release). It reads everything from the image at run time, including the
music from the CD's audio tracks, so nothing needs installing or
extracting.

- **Format:** a `.cue` sheet with the `.bin` file(s) it names. Rip your own
  disc with any tool that writes BIN/CUE with audio tracks, for example
  ImgBurn ("Create image file from disc") on Windows or `cdrdao` on Linux.
  Data tracks may be `MODE1/2352`, `MODE1/2048` or `MODE2/2352`. A plain
  `.iso` holds no music; it works with a hand-written `.cue` naming it as
  a `MODE1/2048` track.
- **Where:** put the `.cue` and its `.bin` file(s) into a `gamedata/`
  folder in the directory you run the game from; with `cargo run` that is
  the top of the repository. File names don't matter, but the folder must
  contain exactly one `.cue`.
  Alternatively, pass `--data DIR` or set the `OPENLEM3D_DATA` environment
  variable to a directory holding them.
- `gamedata/` is git-ignored. Never commit game data, extracted files or
  screenshots of the original (see `docs/GROUNDRULES.md`).

## Getting started

1. **Install Rust** (latest stable): <https://rustup.rs>.
2. **Add the game data** as described above.
3. **Run:**

   ```
   cargo run --release -p openlem3d
   ```

   `--level N` starts a level directly (0–99; see `docs/spec/level.md` for
   the numbering).

   For testing without touching the keyboard: `--size WxH`, `--camera 1-4`,
   `--assign TICK:LEMMING:SKILL[:cw|acw]`, `--press SECONDS:KEY[:HOLD]` (Bevy key
   names such as `Escape`, `F12`, `KeyP`, or `Click`), and
   `--screenshot FILE --wait SECONDS`, which saves the window and quits.

## Controls

| Action | Keyboard / mouse | Touch |
|---|---|---|
| Move | W A S D or the arrow keys; or hold the right button still over the view: the pointer's arrow shows the way (top: forward, bottom: back, bottom corners: sideways, top corners: forward while turning); click the minimap | one-finger drag (up/down) |
| Turn | Q / E, right-drag, hold the right button at the left or right edge, or hold the turn arrows | one-finger drag (left/right), or hold the turn arrows |
| Up / down | R / F, or the mouse wheel | two-finger drag |
| Preset cameras | 1–4, or click the camera to cycle | tap the camera to cycle |
| Pause | P, or click the paws | tap the paws |
| Fast-forward | click the ▶ icon | tap the ▶ icon |
| Select skill | F1–F9, or click a skill at the bottom | tap a skill |
| Give skill to a lemming | click the lemming | tap the lemming |
| Turner | click the lemming, then click beside it on the side it should point to | tap, then tap beside it |
| Highlight a lemming | click the arrow (bottom left): the lemming nearest the middle of the view gets an arrow over it; click another lemming to move it there. Skills clicked go straight to it, and the face rides along with it. The arrow again switches it off | tap the arrow; tap a lemming to move it |
| Ride along with a lemming | V, I or the face, then click a lemming (or the face with a lemming highlighted); V, I, Esc or the face again to return | tap the face, then a lemming; tap the face again to return |
| Release rate | − / + (hold to repeat) | − / + buttons (hold to repeat) |
| Nuke | Alt+Q, or click the bomb | tap the bomb |
| Mute music and sounds | M | — |
| Options | F12 (also during a level, which waits) or the Options button on the title | tap Options |
| Briefing | click or Space: play; Enter or click Preview: preview (on Practice levels: Demo, a solution played for you; any key or click returns); right click, Esc or click Menu: back | tap to play, or tap Preview/Demo or Menu |
| Results | click, Space or Enter: next level (or retry); right click or Esc: level list | tap; or tap Menu |
| Level list | click a level; wheel, ↑/↓ or Page Up/Down to scroll; ←/→ change rating; Esc: title | tap a level; drag to scroll |
| Replay your attempt | Esc restarts the level and replays what you did ("Replaying"); click to take over ("Click to Play") | tap to take over |
| Back to the level list | Esc during a replay, or Menu on the results screen | nuke, then Menu |

## Repository layout

| Path | Contents |
|---|---|
| `crates/l3d-formats` | Parsers for the CD image and every game file format |
| `crates/l3d-sim` | Deterministic game simulation, with no engine dependency |
| `crates/openlem3d` | The game (Bevy) |
| `crates/l3d-tool` | Command-line inspector for the game data |
| `docs/GROUNDRULES.md` | The project's clean-room rules |
| `docs/spec/` | Our own description of the formats and behaviour |
| `tools/original/` | Scripts that drive the original in DOSBox-X for side-by-side comparison |

## Credits

The file-format work builds on reverse engineering published by the
Lemmings community (Pooty, GuyPerfect, namida, ccexplore and others); see
`docs/spec/REFERENCES.md`.

## Licence

openlem3d is released under the MIT licence; see `LICENSE`. It covers this
repository's code and documentation only, not the original game or its data.
