# CD image

## Tracks (verified)

Checked with `l3d-tool tracks`.

- **Track 1** holds the game data: Mode 1, 92,183 sectors, with an ISO 9660
  filesystem. Our image stores raw 2352-byte sectors; the 2048 bytes of user
  data start at offset 16 of each one.
- **Tracks 2–24** are 23 CD-audio tracks (red book: 44.1 kHz, 16-bit, stereo),
  each 60–105 seconds long. These are the music.
- **Pregaps:** track 2 has a 2-second pregap (`PREGAP` in the CUE sheet) that
  isn't stored in the image. Tracks 3–24 store their 2-second pregap
  (`INDEX 00`) in the image, in front of `INDEX 01`.

Each track's extent in the image file runs from its own `INDEX 01` to the next
track's `INDEX 00` (or `INDEX 01` if it has none), or to the end of the file.

The provisional formula the viewer used, track = 2 + 2·(theme − 1) +
(music index & 1), is **wrong**: it matches none of the observations below.

### Level music (partly verified)

**Method.** DOSBox-X logs CD-audio playback when its `[log]` section sets
`misc = debug` (`tools/original/launch.ps1 -ExtraConf` loads such a file).
Each play request then shows up as `CDROM: Playing track # N` in the log
file; track numbers count the data track as 1, so audio tracks are 2–24. No
debugger is involved; this is the emulator's own device log.

**When music plays (verified).** The track starts when the level starts
(after "Continue" on the briefing), not on the briefing or in the menus.
Starting the same level again always gave the same track: Fun 1 about 17
times in one session, Practice "Turner" twice in separate sessions.

| Level | File | Theme (`0x143`) | Music (`0x151`) | Track |
|---|---|---|---|---|
| Fun 1 "Take a Dive" | `LEVEL.000` | 7 | 0 | 5 |
| Fun 2 "That's Right" | `LEVEL.001` | 10 | 1 | 12 |
| Fun 3 "The Bean Machine" | `LEVEL.002` | 4 | 2 | 14 |
| Tricky 1 "Jelly Climber" | `LEVEL.020` | 4 | 1 | 23 |
| Taxing 1 "Spaghetti Junction" | `LEVEL.040` | 8 | 2 | 9 |
| Mayhem 1 "The Five Arches" | `LEVEL.060` | 1 | 2 | 13 |
| Practice "Blocker" | `LEVEL.080` | 8 | 0 | 9 |
| Practice "Turner" | `LEVEL.081` | 8 | 1 | 20 |
| Practice "Bomber" | `LEVEL.082` | 8 | 2 | 9 |
| Practice "Builder" | `LEVEL.083` | 8 | 0 | 9 |
| Practice "Basher" | `LEVEL.084` | 8 | 1 | 20 |
| Practice "Floater" | `LEVEL.088` | 8 | 2 | 9 |
| Practice "Trampoline" | `LEVEL.098` | 8 | 0 | 9 |

All rows are **verified** (one log line per level start). They are
consistent with a fixed (theme, music) → track table: the eight theme-8
levels split exactly by music index. In theme 8, music indices 0 and 2 both
play track 9. No formula fits the five themes seen so far, so the remaining
combinations are **unknown**; the table needs one level of each remaining
(theme, music) pair, which needs more level codes.

## Filesystem inventory (verified)

Checked with `l3d-tool ls`. Every file listed below as RNC-compressed
decompresses with valid CRCs (`l3d-tool rnc-check`: 150/150).

| Path | Count | Notes |
|---|---|---|
| `L3D.EXE`, `SETUP.EXE` | 2 | Executables. **Do not disassemble** (ground rules). |
| `LEVELS/LEVEL.000`–`.099` | 100 | RNC-compressed levels, 66,048 bytes unpacked. See [level.md](level.md). |
| `LEVELS/BLK.000`–`.099` | 100 | Uncompressed, 1,408 bytes each, one per level. See [blk.md](blk.md). |
| `GFX/TEXTURE.nnn` | 26 | 409,600 bytes, uncompressed. |
| `GFX/SKY.nnn` | 15 | 65,536 or 64,000 bytes. |
| `GFX/LAND.nnn` | 14 | 8 KB, 16 KB or 32 KB. |
| `GFX/SEA.nnn` | 8 | 16,384 bytes. |
| `GFX/OBJ.nnn` | 14 | 45,056 bytes. |
| `GFX/SIGNS.nnn` | 12 | 49,152 bytes. |
| `GFX/WALLS.nnn` | 8 | 65,536 bytes. |
| `GFX/ANIMOBJ.nnn` | 24 | 16,384 bytes. |
| `GFX/TRAPS.nnn` | 9 | 32,768 bytes, except `TRAPS.008` (20,480 bytes). |
| `GFX/BGRD.000`, `GFX/OVERLAY.000` | 1 each | 15,360 and 12,288 bytes. |
| `GFX/LM3D.PAL` | 1 | Main palette. See [graphics.md](graphics.md). |
| `GFX/SCENE000`–`010.{RNC,SVG,PAL,SVP}` | 11 sets | Theme preview screens. |
| `GFX/INTRO1`–`7.{RNC,SVG}` | 7 sets | Intro screens. |
| `GFX/*.RNC` (`ICONS`, `MOUSE`, `TITLE`, `LOADING`, …) | | UI graphics. |
| `GFX/*.GFX`, `GFX/*.FNT`, `GFX/TITLE.MHC` | | UI graphics and fonts. |
| `LEMM/LEMM.MHC`, `LEMM/LEMM128.MHC` | 2 | Lemming sprites, 64 and 128 px. |
| `ANIM/*` | 11 | Large files, probably FMV cutscenes. |
| `BMPS/800BMPS/*.BMP`, `BMPS/1024BMPS/*.BMP` | 10 + 10 | Per-theme backdrops for high-resolution modes. |
| `SOUND/**` | 331 | Sound effects and sound-card music, several driver variants (e.g. `SOUND/AWE/`). |
| `REPLAYS/REP.nnn`, `REPLAYS/REPLAY.nnn` | 24 | RNC-compressed demo replays, mostly 64,802 bytes unpacked. |
