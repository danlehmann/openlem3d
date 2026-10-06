# Lemmings 3D — format and behaviour spec

This is our own description of the original game's data and behaviour, built
under the clean-room rules in `../GROUNDRULES.md`. Facts carry a status:

- **verified**: checked against the shipped data (with `l3d-tool`) or the
  running game. The note says how.
- **unverified**: taken from a source in [REFERENCES.md](REFERENCES.md) and not
  yet checked. Don't rely on it without checking.
- **wrong**: a claim from a source that the data contradicts. These are kept so
  nobody re-imports them.

## Documents

| File | Covers |
|---|---|
| [disc.md](disc.md) | CD image layout, file inventory |
| [rnc.md](rnc.md) | RNC ProPack method 1 compression |
| [level.md](level.md) | `LEVELS/LEVEL.nnn`: header, block grid, object grid |
| [blk.md](blk.md) | `LEVELS/BLK.nnn`: block dictionary |
| [graphics.md](graphics.md) | Palette, textures, sky and the other raw `GFX` images |
| [lemmings.md](lemmings.md) | Lemming sprite cells (`LEMM.MHC`) and frame groups |
| [ui-graphics.md](ui-graphics.md) | Title logo, menu buttons, fonts, panel icons, sprite sheets, intro slides, scenes, bitmaps |
| [behaviour.md](behaviour.md) | Lemming movement, release, exits, deaths (mostly provisional) |
| [camera.md](camera.md) | Preset cameras, the no-pitch camera, the off-centre projection |

## Conventions

- Multi-byte values are little-endian unless stated otherwise.
- Grid axes are X (0–31), Y (0–15, vertical, up) and Z (0–31). This is a
  right-handed system: viewed from above with +Y towards the viewer, turning
  from +X to +Z is clockwise. Bevy also uses a right-handed, Y-up system, so
  grid axes map directly to Bevy axes.
- Tool commands below mean `cargo run -p l3d-tool -- <command>`.
