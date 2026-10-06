# openlem3d

An open-source, clean-room reimplementation of the 1995 DOS game
*Lemmings 3D*, written in Rust with Bevy. It renders at high resolution
and plays with a mouse, a keyboard or a touch screen.

openlem3d contains **no game data**. You need your own copy of the
original CD; the game reads levels, graphics and CD-audio music from it at
run time.

## Status

Early development:

- **Working:** all 100 levels load and render, with blocks, sky, sea, land,
  objects, signs, wall decorations and traps. Lemmings walk, fall, climb,
  float, block, dig and explode. There's a level-select screen, a skill
  bar, level timers, nuke, and CD music.
- **Not yet done:** builder, basher and miner. Lemming sprites don't use
  the correct frames for their direction yet. Behaviour timings are still
  being measured against the original.

See `docs/spec/` for what has been verified and how.

## Getting started

1. **Install Rust** (latest stable): <https://rustup.rs>.
2. **Put the CD image in `gamedata/`:** copy your *Lemmings 3D* image, a
   `.cue` file plus its `.bin`, into `gamedata/` at the top of the
   repository. The file names don't matter. Alternatively, pass
   `--data DIR` or set `OPENLEM3D_DATA`.
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
| Move | W A S D or the arrow keys | one-finger drag (up/down) |
| Turn | Q / E, right-drag, or hold the turn arrows | one-finger drag (left/right), or hold the turn arrows |
| Up / down | R / F | two-finger drag |
| Preset cameras | 1–4, or click the camera to cycle | tap the camera to cycle |
| Pause | P, or click the paws | tap the paws |
| Fast-forward | click the ▶ icon | tap the ▶ icon |
| Select skill | F1–F9, or click a skill at the bottom | tap a skill |
| Give skill to a lemming | click the lemming | tap the lemming |
| Turner | click the lemming, then click beside it on the side it should point to | tap, then tap beside it |
| Ride along with a lemming | V, I or the face, then click a lemming; V, I, Esc or the arrow to return | tap the face, then a lemming; the arrow returns |
| Release rate | − / + (hold to repeat) | − / + buttons (hold to repeat) |
| Nuke | Alt+Q, or click the bomb | tap the bomb |
| Mute music and sounds | M | — |
| Options | F12 (also during a level, which waits) or the Options button on the title | tap Options |
| Briefing | click or Space: play; Enter or click Preview: preview; right click, Esc or click Menu: back | tap to play, or tap Preview or Menu |
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
